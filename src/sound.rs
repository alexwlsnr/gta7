//! Procedural audio synthesis for retro GTA-style sounds (no external assets).
use std::f32::consts::TAU;
use raylib::prelude::*;

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
    
    // Baked MP3 streams
    pub radio: Vec<Music<'a>>,
    pub walk_tracks: Vec<Music<'a>>,
    pub wanted_tracks: Vec<Music<'a>>,
    
    pub current_mode: SoundMode,
    pub active_track_idx: usize,
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

        // 7. Radio (Kevin MacLeod MP3 files baked into the binary)
        let mut radio = Vec::new();
        
        let radio_files = &[
            include_bytes!("../assets/music/GoCart.mp3").as_slice(),
            include_bytes!("../assets/music/LaserGroove.mp3").as_slice(),
            include_bytes!("../assets/music/RetroFutureClean.mp3").as_slice(),
            include_bytes!("../assets/music/RetroFutureDirty.mp3").as_slice(),
            include_bytes!("../assets/music/FunkGameLoop.mp3").as_slice(),
            include_bytes!("../assets/music/SpaceFighterLoop.mp3").as_slice(),
            include_bytes!("../assets/music/Loopster.mp3").as_slice(),
            include_bytes!("../assets/music/RocketPower.mp3").as_slice(),
            include_bytes!("../assets/music/SonOfARocket.mp3").as_slice(),
            include_bytes!("../assets/music/HappyHappyGameShow.mp3").as_slice(),
        ];
        
        for bytes in radio_files {
            if let Ok(mut m) = audio.new_music_from_memory(".mp3", bytes) {
                m.set_looping(true);
                m.set_volume(0.3);
                radio.push(m);
            }
        }

        // 8. Walk tracks (embedded)
        let mut walk_tracks = Vec::new();
        let walk_files = &[
            include_bytes!("../assets/music/BassaIslandGameLoop.mp3").as_slice(),
            include_bytes!("../assets/music/TownieLoop.mp3").as_slice(),
        ];
        for bytes in walk_files {
            if let Ok(mut m) = audio.new_music_from_memory(".mp3", bytes) {
                m.set_looping(true);
                m.set_volume(0.3);
                walk_tracks.push(m);
            }
        }

        // 9. Wanted chase tracks (embedded)
        let mut wanted_tracks = Vec::new();
        let wanted_files = &[
            include_bytes!("../assets/music/ZombieChase.mp3").as_slice(),
            include_bytes!("../assets/music/ChasePulse.mp3").as_slice(),
        ];
        for bytes in wanted_files {
            if let Ok(mut m) = audio.new_music_from_memory(".mp3", bytes) {
                m.set_looping(true);
                m.set_volume(0.3);
                wanted_tracks.push(m);
            }
        }

        SoundEffects {
            shoot,
            explosion,
            crash,
            complete,
            enter_exit,
            engine,
            sfx_volume: 0.7,
            music_volume: 0.3,
            
            radio,
            walk_tracks,
            wanted_tracks,
            current_mode: SoundMode::Walk,
            active_track_idx: 0,
        }
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
        match self.current_mode {
            SoundMode::Walk => {
                if !self.walk_tracks.is_empty() {
                    let idx = self.active_track_idx % self.walk_tracks.len();
                    self.walk_tracks[idx].update_stream();
                }
            }
            SoundMode::Drive => {
                if !self.radio.is_empty() {
                    let idx = self.active_track_idx % self.radio.len();
                    self.radio[idx].update_stream();
                }
            }
            SoundMode::Wanted => {
                if !self.wanted_tracks.is_empty() {
                    let idx = self.active_track_idx % self.wanted_tracks.len();
                    self.wanted_tracks[idx].update_stream();
                }
            }
        }
    }

    pub fn start_radio(&mut self) {
        self.update_audio_mode(false, 0);
    }

    pub fn update_radio(&mut self) {
        match self.current_mode {
            SoundMode::Walk => {
                if !self.walk_tracks.is_empty() {
                    let idx = self.active_track_idx % self.walk_tracks.len();
                    let m = &mut self.walk_tracks[idx];
                    if !m.is_stream_playing() {
                        m.play_stream();
                    }
                    m.set_volume(self.music_volume);
                }
            }
            SoundMode::Drive => {
                if !self.radio.is_empty() {
                    let idx = self.active_track_idx % self.radio.len();
                    let m = &mut self.radio[idx];
                    if !m.is_stream_playing() {
                        m.play_stream();
                    }
                    m.set_volume(self.music_volume);
                }
            }
            SoundMode::Wanted => {
                if !self.wanted_tracks.is_empty() {
                    let idx = self.active_track_idx % self.wanted_tracks.len();
                    let m = &mut self.wanted_tracks[idx];
                    if !m.is_stream_playing() {
                        m.play_stream();
                    }
                    m.set_volume(self.music_volume * 1.2);
                }
            }
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
                SoundMode::Walk => self.walk_tracks.len(),
                SoundMode::Drive => self.radio.len(),
                SoundMode::Wanted => self.wanted_tracks.len(),
            };
            if count > 0 {
                self.active_track_idx = rand::random::<usize>() % count;
            } else {
                self.active_track_idx = 0;
            }
            self.start_current_track();
        }
    }

    pub fn stop_all_music(&mut self) {
        for m in &mut self.radio {
            if m.is_stream_playing() {
                m.stop_stream();
            }
        }
        for m in &mut self.walk_tracks {
            if m.is_stream_playing() {
                m.stop_stream();
            }
        }
        for m in &mut self.wanted_tracks {
            if m.is_stream_playing() {
                m.stop_stream();
            }
        }
    }

    pub fn start_current_track(&mut self) {
        match self.current_mode {
            SoundMode::Walk => {
                if !self.walk_tracks.is_empty() {
                    let idx = self.active_track_idx % self.walk_tracks.len();
                    self.walk_tracks[idx].set_volume(self.music_volume);
                    self.walk_tracks[idx].play_stream();
                }
            }
            SoundMode::Drive => {
                if !self.radio.is_empty() {
                    let idx = self.active_track_idx % self.radio.len();
                    self.radio[idx].set_volume(self.music_volume);
                    self.radio[idx].play_stream();
                }
            }
            SoundMode::Wanted => {
                if !self.wanted_tracks.is_empty() {
                    let idx = self.active_track_idx % self.wanted_tracks.len();
                    self.wanted_tracks[idx].set_volume(self.music_volume * 1.25);
                    self.wanted_tracks[idx].play_stream();
                }
            }
        }
    }

    pub fn cycle_track(&mut self, forward: bool) {
        self.stop_all_music();
        match self.current_mode {
            SoundMode::Walk => {
                let len = self.walk_tracks.len();
                if len > 0 {
                    if forward {
                        self.active_track_idx = (self.active_track_idx + 1) % len;
                    } else {
                        self.active_track_idx = (self.active_track_idx + len - 1) % len;
                    }
                }
            }
            SoundMode::Drive => {
                let len = self.radio.len();
                if len > 0 {
                    if forward {
                        self.active_track_idx = (self.active_track_idx + 1) % len;
                    } else {
                        self.active_track_idx = (self.active_track_idx + len - 1) % len;
                    }
                }
            }
            SoundMode::Wanted => {
                let len = self.wanted_tracks.len();
                if len > 0 {
                    if forward {
                        self.active_track_idx = (self.active_track_idx + 1) % len;
                    } else {
                        self.active_track_idx = (self.active_track_idx + len - 1) % len;
                    }
                }
            }
        }
        self.start_current_track();
    }

    pub fn current_track_title(&self) -> &str {
        match self.current_mode {
            SoundMode::Walk => {
                if self.walk_tracks.is_empty() { return "No Track"; }
                match self.active_track_idx % self.walk_tracks.len() {
                    0 => "Bassa Island Game Loop",
                    _ => "Townie Loop",
                }
            }
            SoundMode::Drive => {
                if self.radio.is_empty() { return "No Track"; }
                match self.active_track_idx % self.radio.len() {
                    0 => "Go Cart (Loop Mix)",
                    1 => "Laser Groove",
                    2 => "RetroFuture Clean",
                    3 => "RetroFuture Dirty",
                    4 => "Funk Game Loop",
                    5 => "Space Fighter Loop",
                    6 => "Loopster",
                    7 => "Rocket Power",
                    8 => "Son Of A Rocket",
                    _ => "Happy Happy Game Show",
                }
            }
            SoundMode::Wanted => {
                if self.wanted_tracks.is_empty() { return "No Track"; }
                match self.active_track_idx % self.wanted_tracks.len() {
                    0 => "Zombie Chase",
                    _ => "Chase Pulse",
                }
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

