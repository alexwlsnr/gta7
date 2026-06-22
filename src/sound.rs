//! Procedural audio synthesis for retro GTA-style sounds (no external assets).
use std::f32::consts::TAU;
use raylib::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::os::raw::c_void;

use crate::sound_tracks::{RADIO_TRACKS, WALK_TRACKS, WANTED_TRACKS};

pub static CURRENT_AMPLITUDE: AtomicU32 = AtomicU32::new(0);
pub static CALLBACK_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub static BAR_AMPLITUDES: [AtomicU32; 6] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

static FILTER_V0: AtomicU32 = AtomicU32::new(0);
static FILTER_V1: AtomicU32 = AtomicU32::new(0);
static FILTER_V2: AtomicU32 = AtomicU32::new(0);
static FILTER_V3: AtomicU32 = AtomicU32::new(0);
static FILTER_PREV_X: AtomicU32 = AtomicU32::new(0);
static FILTER_PREV_PREV_X: AtomicU32 = AtomicU32::new(0);

pub unsafe extern "C" fn audio_processor_callback(data: *mut c_void, frames: u32) {
    if data.is_null() || frames == 0 {
        return;
    }
    CALLBACK_COUNT.fetch_add(1, Ordering::Relaxed);
    let samples = data as *const f32;
    let sample_count = (frames * 2) as usize; // Stereo: 2 channels
    
    // Load current filter states
    let mut v0 = f32::from_bits(FILTER_V0.load(Ordering::Relaxed));
    let mut v1 = f32::from_bits(FILTER_V1.load(Ordering::Relaxed));
    let mut v2 = f32::from_bits(FILTER_V2.load(Ordering::Relaxed));
    let mut v3 = f32::from_bits(FILTER_V3.load(Ordering::Relaxed));
    let mut prev_x = f32::from_bits(FILTER_PREV_X.load(Ordering::Relaxed));
    let mut prev_prev_x = f32::from_bits(FILTER_PREV_PREV_X.load(Ordering::Relaxed));
    
    // Load current envelopes
    let mut envs = [0.0f32; 6];
    for i in 0..6 {
        envs[i] = f32::from_bits(BAR_AMPLITUDES[i].load(Ordering::Relaxed));
    }
    
    let mut overall_max = 0.0f32;

    for i in (0..sample_count).step_by(2) {
        let left = *samples.add(i);
        let right = if i + 1 < sample_count { *samples.add(i + 1) } else { left };
        let x = (left + right) * 0.5;
        
        let abs_x = x.abs();
        if abs_x > overall_max {
            overall_max = abs_x;
        }
        
        // Filter equations
        v0 = v0 * 0.90 + x * 0.10;
        v1 = v1 * 0.75 + x * 0.25;
        v2 = v2 * 0.50 + x * 0.50;
        v3 = v3 * 0.20 + x * 0.80;
        
        let d1 = x - prev_x;
        let d2 = x - 2.0 * prev_x + prev_prev_x;
        
        // Update history
        prev_prev_x = prev_x;
        prev_x = x;
        
        // Rectified bands
        let b0 = v0.abs();
        let b1 = (v1 - v0).abs();
        let b2 = (v2 - v1).abs();
        let b3 = (v3 - v2).abs();
        let b4 = d1.abs();
        let b5 = d2.abs();
        
        // Peak followers (fast attack, slow decay)
        envs[0] = if b0 > envs[0] { b0 } else { envs[0] * 0.995 + b0 * 0.005 };
        envs[1] = if b1 > envs[1] { b1 } else { envs[1] * 0.995 + b1 * 0.005 };
        envs[2] = if b2 > envs[2] { b2 } else { envs[2] * 0.995 + b2 * 0.005 };
        envs[3] = if b3 > envs[3] { b3 } else { envs[3] * 0.995 + b3 * 0.005 };
        envs[4] = if b4 > envs[4] { b4 } else { envs[4] * 0.995 + b4 * 0.005 };
        envs[5] = if b5 > envs[5] { b5 } else { envs[5] * 0.995 + b5 * 0.005 };
    }
    
    // Save filter states
    FILTER_V0.store(v0.to_bits(), Ordering::Relaxed);
    FILTER_V1.store(v1.to_bits(), Ordering::Relaxed);
    FILTER_V2.store(v2.to_bits(), Ordering::Relaxed);
    FILTER_V3.store(v3.to_bits(), Ordering::Relaxed);
    FILTER_PREV_X.store(prev_x.to_bits(), Ordering::Relaxed);
    FILTER_PREV_PREV_X.store(prev_prev_x.to_bits(), Ordering::Relaxed);
    
    // Save envelopes
    for i in 0..6 {
        BAR_AMPLITUDES[i].store(envs[i].to_bits(), Ordering::Relaxed);
    }
    
    // Overall amplitude smoothing
    let current_bits = CURRENT_AMPLITUDE.load(Ordering::Relaxed);
    let current_amp = f32::from_bits(current_bits);
    let smoothed = current_amp * 0.8 + overall_max * 0.2;
    CURRENT_AMPLITUDE.store(smoothed.to_bits(), Ordering::Relaxed);
}

use std::sync::mpsc::{channel, Sender, Receiver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundMode {
    Walk,
    Drive,
    Wanted,
}

pub struct SoundEffects<'a> {
    pub shoot: Sound<'a>,
    pub explosion: Sound<'a>,
    pub crash: Sound<'a>,
    pub complete: Sound<'a>,
    pub enter_exit: Sound<'a>,
    pub engine: Sound<'a>,
    pub sfx_volume: f32,
    pub music_volume: f32,
    
    // Dynamic stream loading state
    pub audio_device: &'a RaylibAudio,
    pub current_music: Option<Music<'a>>,
    pub current_bytes: Option<Vec<u8>>,
    
    pub rx_bytes: Receiver<Result<(String, Vec<u8>), String>>,
    pub tx_bytes: Sender<Result<(String, Vec<u8>), String>>,
    pub is_loading: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub current_loading_url: Option<String>,
    
    pub current_mode: SoundMode,
    pub active_track_idx: usize,
    pub music_paused: bool,
}

impl<'a> SoundEffects<'a> {
    pub fn load(audio: &'a RaylibAudio) -> Self {
        // 1. Shoot sound
        let shoot_samples = gen_shoot();
        let shoot_wav = make_wav_mono_16bit(22050, &shoot_samples);
        let wave_shoot = audio.new_wave_from_memory(".wav", &shoot_wav).unwrap();
        let shoot = audio.new_sound_from_wave(&wave_shoot).unwrap();

        // 2. Explosion sound
        let explosion_samples = gen_explosion();
        let explosion_wav = make_wav_mono_16bit(22050, &explosion_samples);
        let wave_explosion = audio.new_wave_from_memory(".wav", &explosion_wav).unwrap();
        let explosion = audio.new_sound_from_wave(&wave_explosion).unwrap();

        // 3. Crash sound
        let crash_samples = gen_crash();
        let crash_wav = make_wav_mono_16bit(22050, &crash_samples);
        let wave_crash = audio.new_wave_from_memory(".wav", &crash_wav).unwrap();
        let crash = audio.new_sound_from_wave(&wave_crash).unwrap();

        // 4. Complete sound
        let complete_samples = gen_complete();
        let complete_wav = make_wav_mono_16bit(22050, &complete_samples);
        let wave_complete = audio.new_wave_from_memory(".wav", &complete_wav).unwrap();
        let complete = audio.new_sound_from_wave(&wave_complete).unwrap();

        // 5. Enter/Exit beep sound
        let enter_exit_samples = gen_beep();
        let enter_exit_wav = make_wav_mono_16bit(22050, &enter_exit_samples);
        let wave_enter_exit = audio.new_wave_from_memory(".wav", &enter_exit_wav).unwrap();
        let enter_exit = audio.new_sound_from_wave(&wave_enter_exit).unwrap();

        // 6. Engine loop
        let engine_samples = gen_engine();
        let engine_wav = make_wav_mono_16bit(22050, &engine_samples);
        let wave_engine = audio.new_wave_from_memory(".wav", &engine_wav).unwrap();
        let engine = audio.new_sound_from_wave(&wave_engine).unwrap();

        let (tx, rx) = channel();

        let walk_len = WALK_TRACKS.len();
        let initial_idx = if walk_len > 0 { rand::random::<usize>() % walk_len } else { 0 };

        let mut sfx = SoundEffects {
            shoot,
            explosion,
            crash,
            complete,
            enter_exit,
            engine,
            sfx_volume: 0.7,
            music_volume: 0.3,
            
            audio_device: audio,
            current_music: None,
            current_bytes: None,
            
            rx_bytes: rx,
            tx_bytes: tx,
            is_loading: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            current_loading_url: None,
            
            current_mode: SoundMode::Walk,
            active_track_idx: initial_idx,
            music_paused: false,
        };

        // Start loading the initial track
        let url = sfx.current_track_url().to_string();
        sfx.start_loading_track(url);

        sfx
    }

    pub fn update_engine(&mut self, in_vehicle: bool, speed: f32, throttle: f32) {
        if in_vehicle {
            if !self.engine.is_playing() {
                self.engine.play();
            }
            let speed_ratio = (speed.abs() / 40.0).clamp(0.0, 1.0);
            let pitch = 0.7 + speed_ratio * 1.3;
            let volume = (0.15 + speed_ratio * 0.2 + throttle * 0.05).min(0.4) * self.sfx_volume;
            self.engine.set_pitch(pitch);
            self.engine.set_volume(volume);
        } else {
            self.engine.stop();
        }
    }

    pub fn update_music(&mut self) {
        // Poll for downloaded bytes
        if let Ok(res) = self.rx_bytes.try_recv() {
            self.is_loading.store(false, Ordering::Relaxed);
            match res {
                Ok((url, bytes)) => {
                    if Some(url) == self.current_loading_url {
                        self.current_bytes = Some(bytes);
                        if let Some(ref b) = self.current_bytes {
                            match self.audio_device.new_music_from_memory(".mp3", b) {
                                Ok(mut m) => {
                                    m.set_looping(true);
                                    m.set_volume(self.music_volume * if self.current_mode == SoundMode::Wanted { 1.2 } else { 1.0 });
                                    unsafe {
                                        raylib::ffi::AttachAudioStreamProcessor(m.stream, Some(audio_processor_callback));
                                    }
                                    m.play_stream();
                                    self.current_music = Some(m);
                                }
                                Err(e) => {
                                    println!("Failed to load music from memory: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Download error: {}", e);
                }
            }
        }

        if self.music_paused { return; }
        if let Some(ref mut m) = self.current_music {
            m.update_stream();
        }
    }

    pub fn start_radio(&mut self) {
        self.update_audio_mode(false, 0);
    }

    pub fn update_radio(&mut self) {
        if self.music_paused {
            return;
        }
        if self.current_music.is_none() && !self.is_loading.load(Ordering::Relaxed) {
            let url = self.current_track_url().to_string();
            self.start_loading_track(url);
        }
        if let Some(ref mut m) = self.current_music {
            if !m.is_stream_playing() {
                m.play_stream();
            }
            m.set_volume(self.music_volume * if self.current_mode == SoundMode::Wanted { 1.2 } else { 1.0 });
        }
    }

    pub fn update_audio_mode(&mut self, in_vehicle: bool, wanted_stars: u8) {
        let target_mode = if wanted_stars > 0 {
            SoundMode::Wanted
        } else if in_vehicle {
            SoundMode::Drive
        } else {
            SoundMode::Walk
        };

        if target_mode != self.current_mode {
            self.stop_all_music();
            self.current_mode = target_mode;
            let count = match target_mode {
                SoundMode::Walk => WALK_TRACKS.len(),
                SoundMode::Drive => RADIO_TRACKS.len(),
                SoundMode::Wanted => WANTED_TRACKS.len(),
            };
            if count > 0 {
                self.active_track_idx = rand::random::<usize>() % count;
            } else {
                self.active_track_idx = 0;
            }
            if !self.music_paused {
                let url = self.current_track_url().to_string();
                self.start_loading_track(url);
            }
        }
    }

    pub fn stop_all_music(&mut self) {
        if let Some(ref mut m) = self.current_music {
            if m.is_stream_playing() {
                m.stop_stream();
            }
        }
        self.current_music = None;
        self.current_bytes = None;
        self.current_loading_url = None;
        self.is_loading.store(false, Ordering::Relaxed);
    }

    pub fn start_current_track(&mut self) {
        if self.music_paused {
            return;
        }
        if self.current_music.is_none() && !self.is_loading.load(Ordering::Relaxed) {
            let url = self.current_track_url().to_string();
            self.start_loading_track(url);
        }
    }

    pub fn cycle_track(&mut self, forward: bool) {
        self.stop_all_music();
        let len = match self.current_mode {
            SoundMode::Walk => WALK_TRACKS.len(),
            SoundMode::Drive => RADIO_TRACKS.len(),
            SoundMode::Wanted => WANTED_TRACKS.len(),
        };
        if len > 0 {
            if forward {
                self.active_track_idx = (self.active_track_idx + 1) % len;
            } else {
                self.active_track_idx = (self.active_track_idx + len - 1) % len;
            }
        }
        if !self.music_paused {
            let url = self.current_track_url().to_string();
            self.start_loading_track(url);
        }
    }

    pub fn current_track_title(&self) -> &str {
        match self.current_mode {
            SoundMode::Walk => {
                if WALK_TRACKS.is_empty() { return "No Track"; }
                let idx = self.active_track_idx % WALK_TRACKS.len();
                WALK_TRACKS[idx].0
            }
            SoundMode::Drive => {
                if RADIO_TRACKS.is_empty() { return "No Track"; }
                let idx = self.active_track_idx % RADIO_TRACKS.len();
                RADIO_TRACKS[idx].0
            }
            SoundMode::Wanted => {
                if WANTED_TRACKS.is_empty() { return "No Track"; }
                let idx = self.active_track_idx % WANTED_TRACKS.len();
                WANTED_TRACKS[idx].0
            }
        }
    }

    pub fn current_track_url(&self) -> &str {
        match self.current_mode {
            SoundMode::Walk => {
                if WALK_TRACKS.is_empty() { return ""; }
                let idx = self.active_track_idx % WALK_TRACKS.len();
                WALK_TRACKS[idx].1
            }
            SoundMode::Drive => {
                if RADIO_TRACKS.is_empty() { return ""; }
                let idx = self.active_track_idx % RADIO_TRACKS.len();
                RADIO_TRACKS[idx].1
            }
            SoundMode::Wanted => {
                if WANTED_TRACKS.is_empty() { return ""; }
                let idx = self.active_track_idx % WANTED_TRACKS.len();
                WANTED_TRACKS[idx].1
            }
        }
    }

    pub fn start_loading_track(&mut self, url: String) {
        self.is_loading.store(true, Ordering::Relaxed);
        self.current_loading_url = Some(url.clone());
        let tx = self.tx_bytes.clone();
        std::thread::spawn(move || {
            let output = std::process::Command::new("curl")
                .args(&["-s", "-L", &url])
                .output();
            match output {
                Ok(out) if out.status.success() && out.stdout.len() > 10000 => {
                    let _ = tx.send(Ok((url, out.stdout)));
                }
                Ok(out) => {
                    let err = format!("Download failed: status={}, len={}", out.status, out.stdout.len());
                    let _ = tx.send(Err(err));
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        });
    }

    pub fn toggle_pause(&mut self) {
        self.music_paused = !self.music_paused;
        if self.music_paused {
            if let Some(ref mut m) = self.current_music {
                m.pause_stream();
            }
            CURRENT_AMPLITUDE.store(0.0f32.to_bits(), Ordering::Relaxed);
        } else {
            if let Some(ref mut m) = self.current_music {
                m.resume_stream();
            } else {
                let url = self.current_track_url().to_string();
                self.start_loading_track(url);
            }
        }
    }

    pub fn set_sfx_volume(&mut self, vol: f32) {
        self.sfx_volume = vol;
        self.shoot.set_volume(vol);
        self.explosion.set_volume(vol);
        self.crash.set_volume(vol);
        self.complete.set_volume(vol);
        self.enter_exit.set_volume(vol);
    }

    pub fn set_music_volume(&mut self, vol: f32) {
        self.music_volume = vol;
        if let Some(ref mut m) = self.current_music {
            m.set_volume(vol * if self.current_mode == SoundMode::Wanted { 1.2 } else { 1.0 });
        }
    }
}

fn make_wav_mono_16bit(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    wav.extend_from_slice(b"RIFF");
    let file_size = 36 + (samples.len() * 2) as u32;
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    let data_size = (samples.len() * 2) as u32;
    wav.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

fn gen_shoot() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 0.15;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let freq = 900.0 - (t / duration) * 700.0;
        let phase = t * freq * TAU;
        let val = phase.sin();
        let env = 1.0 - t / duration;
        let s = (val * env * 18000.0) as i16;
        samples.push(s);
    }
    samples
}

fn gen_explosion() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 0.6;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let noise = (rand::random::<f32>() - 0.5) * 2.0;
        let env = (-t * 6.0).exp();
        let s = (noise * env * 15000.0) as i16;
        samples.push(s);
    }
    samples
}

fn gen_crash() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 0.35;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let noise = (rand::random::<f32>() - 0.5) * 1.5;
        let phase = t * 90.0 * TAU;
        let val = phase.sin() * 0.4 + noise * 0.6;
        let env = (-t * 11.0).exp();
        let s = (val * env * 18000.0) as i16;
        samples.push(s);
    }
    samples
}

fn gen_complete() -> Vec<i16> {
    let mut samples = Vec::new();
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * 0.8) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let freq = if t < 0.2 { 440.0 } else { 659.25 };
        let phase = t * freq * TAU;
        let val = phase.sin() * 0.5 + (phase * 2.0).sin() * 0.25;
        let env = if t < 0.2 {
            1.0 - t / 0.2
        } else {
            let dt = t - 0.2;
            (-dt * 3.5).exp()
        };
        let s = (val * env * 14000.0) as i16;
        samples.push(s);
    }
    samples
}

fn gen_beep() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 0.08;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let phase = t * 650.0 * TAU;
        let val = phase.sin();
        let env = 1.0 - t / duration;
        let s = (val * env * 12000.0) as i16;
        samples.push(s);
    }
    samples
}

fn gen_engine() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 1.0;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    let base_freq = 80.0;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let phase = (t * base_freq) % 1.0;
        let saw = 2.0 * phase - 1.0;
        let h2 = ((t * base_freq * 2.0) % 1.0 * 2.0 - 1.0) * 0.3;
        let sub = (t * base_freq * 0.5 * TAU).sin() * 0.2;
        let noise = (rand::random::<f32>() - 0.5) * 0.15;
        let val = saw * 0.5 + h2 + sub + noise;
        let wobble = 0.9 + 0.1 * (t * 8.0 * TAU).sin();
        let s = (val * wobble * 8000.0) as i16;
        samples.push(s);
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_audio_callback() {
        // Initialize audio device - does not require a window
        let audio = RaylibAudio::init_audio_device().unwrap();
        let mut effects = SoundEffects::load(&audio);
        effects.update_audio_mode(false, 0); // Walk mode
        effects.update_radio(); // starts playing the current track
        
        let start_count = CALLBACK_COUNT.load(Ordering::Relaxed);
        println!("Start callback count: {}", start_count);
        
        // Loop up to 10 seconds waiting for the music to download and load
        let start = std::time::Instant::now();
        while effects.current_music.is_none() && start.elapsed().as_secs() < 10 {
            effects.update_music();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(effects.current_music.is_some(), "Music failed to download and load from URL!");
        
        // Now loop a bit to process data
        for _ in 0..100 {
            effects.update_music();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        let end_count = CALLBACK_COUNT.load(Ordering::Relaxed);
        println!("End callback count: {}", end_count);
        
        let amp_bits = CURRENT_AMPLITUDE.load(Ordering::Relaxed);
        let amp = f32::from_bits(amp_bits);
        println!("End amplitude: {}", amp);
        
        assert!(end_count > start_count, "Audio processor callback was not invoked!");
    }
}


