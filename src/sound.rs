//! Procedural audio synthesis for retro GTA-style sounds (no external assets).
use std::f32::consts::TAU;
use raylib::prelude::*;

pub struct SoundEffects<'a> {
    pub shoot: Sound<'a>,
    pub explosion: Sound<'a>,
    pub crash: Sound<'a>,
    pub complete: Sound<'a>,
    pub enter_exit: Sound<'a>,
    pub engine: Music<'a>,
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

        // 6. Engine loop — continuous low rumble, pitch/volume modulated at runtime.
        let engine_samples = gen_engine();
        let engine_wav = make_wav_mono_16bit(22050, &engine_samples);
        let mut engine = audio.new_music_from_memory(".wav", &engine_wav).unwrap();
        engine.set_looping(true);
        engine.set_volume(0.0); // silent until player enters a vehicle

        SoundEffects {
            shoot,
            explosion,
            crash,
            complete,
            enter_exit,
            engine,
        }
    }

    /// Update engine sound based on vehicle speed and throttle.
    /// `speed` is signed forward speed (m/s). `throttle` is 0..1 (how much gas).
    /// `in_vehicle` = true if player is driving.
    pub fn update_engine(&mut self, in_vehicle: bool, speed: f32, throttle: f32) {
        if in_vehicle {
            if !self.engine.is_stream_playing() {
                self.engine.play_stream();
            }
            // Speed ratio: 0 (idle) to 1 (max speed ~40 m/s).
            let speed_ratio = (speed.abs() / 40.0).clamp(0.0, 1.0);
            // Pitch: idle at 0.7, redline at 2.0.
            let pitch = 0.7 + speed_ratio * 1.3;
            // Volume: idle hum at 0.15, full at 0.4. Throttle adds a bit.
            let volume = 0.15 + speed_ratio * 0.2 + throttle * 0.05;
            self.engine.set_pitch(pitch);
            self.engine.set_volume(volume.min(0.4));
        } else {
            // Fade out and stop when not in vehicle.
            if self.engine.is_stream_playing() {
                self.engine.stop_stream();
            }
        }
    }

    /// Must be called every frame to keep the music stream fed.
    pub fn update_music(&self) {
        self.engine.update_stream();
    }
}

fn make_wav_mono_16bit(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    
    // RIFF header
    wav.extend_from_slice(b"RIFF");
    let file_size = 36 + (samples.len() * 2) as u32;
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    
    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
    wav.extend_from_slice(&1u16.to_le_bytes());  // PCM format
    wav.extend_from_slice(&1u16.to_le_bytes());  // Mono (1 channel)
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());  // Block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample
    
    // data chunk
    wav.extend_from_slice(b"data");
    let data_size = (samples.len() * 2) as u32;
    wav.extend_from_slice(&data_size.to_le_bytes());
    
    // PCM samples
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
        // Slide frequency down quickly (classic pew-pew).
        let freq = 900.0 - (t / duration) * 700.0;
        let phase = t * freq * TAU;
        let val = phase.sin();
        let env = 1.0 - t / duration; // linear decay
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
        // White noise.
        let noise = (rand::random::<f32>() - 0.5) * 2.0;
        // Exponential decay envelope for explosion rumble.
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
        // Low frequency thud + white noise mix.
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
    // Succession of two chime notes: A4 (440Hz) then E5 (659.25Hz).
    let num_samples = (sample_rate * 0.8) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        let freq = if t < 0.2 { 440.0 } else { 659.25 };
        let phase = t * freq * TAU;
        let val = phase.sin() * 0.5 + (phase * 2.0).sin() * 0.25; // Rich harmonics
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

/// Generate a 1-second looping engine rumble.
/// Low-frequency sawtooth base + harmonics + slight noise for texture.
/// Pitch and volume are modulated at runtime via Music::set_pitch/set_volume.
fn gen_engine() -> Vec<i16> {
    let mut samples = Vec::new();
    let duration = 1.0;
    let sample_rate = 22050.0;
    let num_samples = (sample_rate * duration) as usize;
    // Base idle frequency — low rumble.
    let base_freq = 80.0;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate;
        // Sawtooth: rich harmonics for engine character.
        let phase = (t * base_freq) % 1.0;
        let saw = 2.0 * phase - 1.0;
        // Second harmonic for a deeper growl.
        let h2 = ((t * base_freq * 2.0) % 1.0 * 2.0 - 1.0) * 0.3;
        // Sub-bass sine for body.
        let sub = (t * base_freq * 0.5 * TAU).sin() * 0.2;
        // Slight noise for mechanical texture.
        let noise = (rand::random::<f32>() - 0.5) * 0.15;
        let val = saw * 0.5 + h2 + sub + noise;
        // Gentle amplitude wobble for realism.
        let wobble = 0.9 + 0.1 * (t * 8.0 * TAU).sin();
        let s = (val * wobble * 8000.0) as i16;
        samples.push(s);
    }
    samples
}
