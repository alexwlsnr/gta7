//! Game configuration and runtime settings.
use raylib::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogicRate {
    R30,
    R60,
    R90,
    R120,
}

impl LogicRate {
    pub fn hz(self) -> f32 {
        match self {
            LogicRate::R30 => 30.0,
            LogicRate::R60 => 60.0,
            LogicRate::R90 => 90.0,
            LogicRate::R120 => 120.0,
        }
    }
    pub fn dt(self) -> f32 {
        1.0 / self.hz()
    }
    pub fn label(self) -> &'static str {
        match self {
            LogicRate::R30 => "30Hz",
            LogicRate::R60 => "60Hz",
            LogicRate::R90 => "90Hz",
            LogicRate::R120 => "120Hz",
        }
    }
    pub fn next(self) -> Self {
        match self {
            LogicRate::R30 => LogicRate::R60,
            LogicRate::R60 => LogicRate::R90,
            LogicRate::R90 => LogicRate::R120,
            LogicRate::R120 => LogicRate::R30,
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub logic_rate: LogicRate,
    pub debug_overlay: bool,
    pub seed: u64,
    pub city_blocks: usize,   // grid is N x N blocks
    pub block_size: f32,      // size of a full block incl. road (meters)
    pub sidewalk_width: f32,  // sidewalk strip between road and building lots
    pub road_width: f32,      // drivable road width
    pub max_peds: usize,
    pub max_traffic: usize,
    pub max_cops: usize,
    pub mouse_sensitivity: f32,
    pub sfx_volume: f32,      // 0..1
    pub music_volume: f32,    // 0..1
    pub time_scale: f32,      // game-hours per real-second (day/night speed)
}

impl Default for Config {
    fn default() -> Self {
        Config {
            logic_rate: LogicRate::R60,
            debug_overlay: false,
            seed: 1337,
            city_blocks: 10,
            block_size: 60.0,
            sidewalk_width: 4.0,
            road_width: 12.0,
            mouse_sensitivity: 0.0025,
            sfx_volume: 0.7,
            music_volume: 0.3,
            time_scale: 0.2,  // ~120s per full day cycle
            max_peds: 40,
            max_traffic: 24,
            max_cops: 16,
        }
    }
}

impl Config {
    /// Half extent of a building lot (sidewalk inset on each side).
    pub fn lot_half(&self) -> f32 {
        (self.block_size - self.road_width) * 0.5 - self.sidewalk_width
    }
    /// Road width half.
    pub fn road_half(&self) -> f32 {
        self.road_width * 0.5
    }
    /// Sidewalk center offset from road centerline.
    pub fn sidewalk_offset(&self) -> f32 {
        self.road_half() + self.sidewalk_width * 0.5
    }
    pub fn world_half(&self) -> f32 {
        self.block_size * (self.city_blocks as f32) * 0.5
    }
    /// Map a color name to a Color.
    pub fn palette(&self) -> Palette {
        Palette
    }
}

pub struct Palette;
impl Palette {
    pub fn road(&self) -> Color { Color::new(72, 72, 78, 255) }
    pub fn sidewalk(&self) -> Color { Color::new(150, 148, 145, 255) }
    pub fn grass(&self) -> Color { Color::new(54, 110, 60, 255) }
    pub fn building(&self, i: u32) -> Color {
        // Deterministic muted building colors.
        let h = ((i.wrapping_mul(2654435761)) % 360) as f32;
        hsl_to_rgb(h, 0.10, 0.30 + ((i % 5) as f32) * 0.04)
    }
    pub fn building_top(&self, i: u32) -> Color {
        let h = ((i.wrapping_mul(2654435761)) % 360) as f32;
        hsl_to_rgb(h, 0.08, 0.18)
    }
    pub fn sky_top(&self) -> Color { Color::new(96, 140, 200, 255) }
    pub fn sky_bottom(&self) -> Color { Color::new(190, 210, 230, 255) }
}

/// Compute sky top/bottom colors for a given time of day (0..24 hours).
/// Interpolates between keyframes: midnight, dawn, morning, noon, dusk, evening, night.
pub fn sky_colors_for_hour(hour: f32) -> (Color, Color) {
    // (hour, sky_top, sky_bottom)
    let keyframes: [(f32, Color, Color); 7] = [
        (0.0,  Color::new(8, 12, 25, 255),    Color::new(15, 18, 35, 255)),   // midnight
        (6.5,  Color::new(80, 50, 60, 255),   Color::new(200, 130, 90, 255)), // dawn
        (8.0,  Color::new(96, 140, 200, 255), Color::new(190, 210, 230, 255)),// morning
        (13.0, Color::new(80, 130, 200, 255), Color::new(180, 200, 230, 255)),// noon
        (18.5, Color::new(120, 60, 40, 255),  Color::new(220, 130, 80, 255)), // dusk
        (20.0, Color::new(30, 30, 60, 255),   Color::new(50, 50, 80, 255)),   // evening
        (24.0, Color::new(8, 12, 25, 255),    Color::new(15, 18, 35, 255)),   // midnight (wraps)
    ];
    let h = hour.rem_euclid(24.0);
    // Find surrounding keyframes.
    let mut i = 0;
    while i < keyframes.len() - 1 && keyframes[i + 1].0 <= h {
        i += 1;
    }
    let (t0, top0, bot0) = keyframes[i];
    let (t1, top1, bot1) = keyframes[i + 1];
    let t = if t1 > t0 { (h - t0) / (t1 - t0) } else { 0.0 };
    let lerp_c = |a: Color, b: Color, t: f32| Color::new(
        (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
        255,
    );
    (lerp_c(top0, top1, t), lerp_c(bot0, bot1, t))
}

/// Format game time as "HH:MM" from accumulated seconds and time scale.
pub fn format_game_time(time: f32, time_scale: f32) -> String {
    let total_hours = (time * time_scale).rem_euclid(24.0);
    let hours = total_hours.floor() as i32;
    let minutes = ((total_hours - hours as f32) * 60.0) as i32;
    format!("{:02}:{:02}", hours, minutes)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h60 = h / 60.0;
    let x = c * (1.0 - (h60 % 2.0 - 1.0).abs());
    let (r, g, b) = match h60 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c * 0.5;
    Color::new(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
        255,
    )
}
