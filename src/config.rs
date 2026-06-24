//! Game configuration and runtime settings.
use raylib::ffi::Vector3;
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
    pub fn road(&self) -> Color { Color::new(15, 12, 28, 255) }
    pub fn sidewalk(&self) -> Color { Color::new(30, 20, 50, 255) }
    pub fn grass(&self) -> Color { Color::new(220, 20, 120, 255) }
    pub fn building(&self, i: u32) -> Color {
        // Neon vaporwave palette: pink, cyan, purple, lime green, orange
        let colors = [
            Color::new(255, 20, 147, 255), // Hot pink
            Color::new(0, 220, 255, 255),   // Neon cyan
            Color::new(160, 32, 240, 255),  // Electric purple
            Color::new(50, 255, 50, 255),   // Neon lime green
            Color::new(255, 110, 0, 255),   // Laser orange
        ];
        colors[(i as usize) % colors.len()]
    }
    pub fn building_top(&self, i: u32) -> Color {
        let body = self.building(i);
        Color::new(
            (body.r as f32 * 0.45) as u8,
            (body.g as f32 * 0.45) as u8,
            (body.b as f32 * 0.45) as u8,
            255,
        )
    }
    pub fn sky_top(&self) -> Color { Color::new(40, 20, 110, 255) }
    pub fn sky_bottom(&self) -> Color { Color::new(0, 220, 240, 255) }
}

/// Compute sky top/bottom colors for a given time of day (0..24 hours).
/// Interpolates between keyframes: midnight, dawn, morning, noon, dusk, evening, night.
pub fn sky_colors_for_hour(hour: f32) -> (Color, Color) {
    // (hour, sky_top, sky_bottom)
    let keyframes: [(f32, Color, Color); 7] = [
        (0.0,  Color::new(18, 8, 38, 255),    Color::new(80, 10, 65, 255)),   // midnight
        (6.5,  Color::new(60, 10, 80, 255),   Color::new(255, 90, 120, 255)), // dawn
        (8.0,  Color::new(40, 20, 110, 255),  Color::new(0, 220, 240, 255)),  // morning
        (13.0, Color::new(160, 0, 180, 255),  Color::new(0, 240, 255, 255)),  // noon
        (18.5, Color::new(90, 10, 120, 255),  Color::new(255, 80, 140, 255)), // dusk
        (20.0, Color::new(25, 12, 55, 255),   Color::new(120, 15, 90, 255)),  // evening
        (24.0, Color::new(18, 8, 38, 255),    Color::new(80, 10, 65, 255)),   // midnight (wraps)
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

/// Compute the sun direction (normalized vector pointing FROM the sun toward the scene)
/// for a given hour (0..24). At noon the sun is overhead, so the light direction points
/// down (negative Y). At night the direction flips upward (moonlight from below horizon).
pub fn sun_direction(hour: f32) -> Vector3 {
    let h = hour.rem_euclid(24.0);
    // Sun elevation angle: sunrise at 6h (angle 0), noon at 12h (angle PI/2, highest),
    // sunset at 18h (angle PI). Full cycle wraps over 24h.
    let angle = ((h - 6.0) / 24.0) * std::f32::consts::TAU;
    // Sun position on the arc: X sweeps east(-)→west(+), Y is elevation (positive = above).
    let px = angle.cos();
    let py = angle.sin();
    // Slight tilt so shadows aren't purely along one axis.
    let z = 0.3;
    // Direction FROM the sun TOWARD the scene = inverse of the sun's position direction.
    let x = -px;
    let y = -py;
    let len = (x * x + y * y + z * z).sqrt();
    Vector3 { x: x / len, y: y / len, z: z / len }
}

/// Compute the sun/moon light color for a given hour.
/// Warm at dawn/dusk, bright white at noon, dim cool moonlight at night.
pub fn sun_color(hour: f32) -> Color {
    let h = hour.rem_euclid(24.0);
    let keyframes: [(f32, Color); 6] = [
        (0.0,  Color::new(50, 20, 75, 255)),    // midnight — dim neon violet moonlight
        (6.0,  Color::new(130, 40, 90, 255)),   // pre-dawn — warm dark pink
        (7.5,  Color::new(255, 120, 150, 255)), // dawn — hot pink-orange
        (13.0, Color::new(220, 255, 255, 255)), // noon — glowing bright electric cyan/white
        (18.5, Color::new(255, 80, 160, 255)),  // dusk — blazing magenta
        (24.0, Color::new(50, 20, 75, 255)),    // wraps to midnight
    ];
    let mut i = 0;
    while i < keyframes.len() - 1 && keyframes[i + 1].0 <= h {
        i += 1;
    }
    let (t0, c0) = keyframes[i];
    let (t1, c1) = keyframes[i + 1];
    let t = if t1 > t0 { (h - t0) / (t1 - t0) } else { 0.0 };
    Color::new(
        (c0.r as f32 + (c1.r as f32 - c0.r as f32) * t) as u8,
        (c0.g as f32 + (c1.g as f32 - c0.g as f32) * t) as u8,
        (c0.b as f32 + (c1.b as f32 - c0.b as f32) * t) as u8,
        255,
    )
}

/// Compute the sun's world position for shadow camera placement.
/// `hour` = time of day (0..24). `player_pos` = camera target.
/// The sun is placed 200 units away in the opposite of the light direction
/// derived from `sun_direction(hour)`.
pub fn sun_position(hour: f32, player_pos: Vector3) -> Vector3 {
    let dir = sun_direction(hour);
    Vector3 {
        x: player_pos.x - dir.x * 200.0,
        y: player_pos.y - dir.y * 200.0,
        z: player_pos.z - dir.z * 200.0,
    }
}

/// God ray intensity from sun elevation. Zero at noon and night, peak at dawn/dusk.
pub fn god_ray_intensity(hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    if h < 5.0 || h > 20.0 {
        0.0
    } else {
        let angle = ((h - 6.0) / 24.0) * std::f32::consts::TAU;
        let elevation = angle.sin().abs();
        (1.0 - elevation).max(0.0).min(1.0) * 0.6
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_direction_at_noon_is_downward() {
        let dir = sun_direction(13.0);
        // At noon (13h), sun should be roughly overhead — direction pointing down.
        assert!(dir.y < -0.5, "sun should point downward at noon, got y={}", dir.y);
    }

    #[test]
    fn sun_direction_at_midnight_is_dim() {
        let dir = sun_direction(0.0);
        // At midnight, sun is below horizon — direction pointing up (moonlight from below).
        assert!(dir.y > 0.0, "sun should point upward at midnight (below horizon), got y={}", dir.y);
    }

    #[test]
    fn sun_color_at_noon_is_bright() {
        let col = sun_color(13.0);
        assert!(col.r > 200 && col.g > 200 && col.b > 180,
            "noon sun should be bright white, got {:?}", col);
    }

    #[test]
    fn sun_color_at_dusk_is_warm() {
        let col = sun_color(18.5);
        // Dusk should be warm — more red than blue.
        assert!(col.r > col.b, "dusk sun should be warmer (r > b), got r={} b={}", col.r, col.b);
    }

    #[test]
    fn sun_color_at_night_is_dim() {
        let col = sun_color(0.0);
        // Night sun (moonlight) should be very dim.
        assert!(col.r < 80 && col.g < 80 && col.b < 100,
            "night sun should be dim, got {:?}", col);
    }

    #[test]
    fn god_ray_intensity_zero_at_night() {
        assert_eq!(god_ray_intensity(0.0), 0.0);
        assert_eq!(god_ray_intensity(4.0), 0.0);
        assert_eq!(god_ray_intensity(21.0), 0.0);
    }

    #[test]
    fn god_ray_intensity_zero_at_noon() {
        // At noon (12h, sin(angle)=1) the elevation is 1 so (1-1)*0.6 = 0.
        assert_eq!(god_ray_intensity(12.0), 0.0);
    }

    #[test]
    fn god_ray_intensity_peaks_at_dawn_and_dusk() {
        // At dawn (6h) and dusk (18h), angle is 0 or PI so sin = 0 and intensity = 0.6.
        let dawn = god_ray_intensity(6.0);
        let dusk = god_ray_intensity(18.0);
        assert!((dawn - 0.6).abs() < 1e-5, "dawn should peak at 0.6, got {}", dawn);
        assert!((dusk - 0.6).abs() < 1e-5, "dusk should peak at 0.6, got {}", dusk);
    }

    #[test]
    fn god_ray_intensity_smooth_between_extremes() {
        // At 9h (mid-morning), elevation is sin(3/24 * TAU) = sin(PI/4) ~ 0.707,
        // so intensity = (1 - 0.707) * 0.6 ~ 0.176.
        let mid = god_ray_intensity(9.0);
        assert!(mid > 0.0 && mid < 0.6, "mid-morning should be between 0 and 0.6, got {}", mid);
    }
}
