//! Procedural audio synthesis for retro GTA-style sounds (no external assets).
use std::f32::consts::TAU;
use raylib::prelude::*;

pub struct SoundEffects<'a> {
    pub shoot: Sound<'a>,
    pub explosion: Sound<'a>,
    pub crash: Sound<'a>,
    pub complete: Sound<'a>,
    pub enter_exit: Sound<'a>,
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

        SoundEffects {
            shoot,
            explosion,
            crash,
            complete,
            enter_exit,
        }
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
