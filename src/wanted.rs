//! Wanted system: heat accumulation, star levels, decay.

pub struct WantedSystem {
    pub heat: f32,       // 0..=6 (continuous, maps to stars)
    pub stars: u8,       // 0..=6 (integer)
    pub decay_timer: f32, // time since last crime
    pub visible: bool,   // player in cop line-of-sight
}

impl WantedSystem {
    pub fn new() -> Self {
        WantedSystem { heat: 0.0, stars: 0, decay_timer: 0.0, visible: false }
    }
}

impl Default for WantedSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl WantedSystem {
    pub fn add_heat(&mut self, amount: f32) {
        self.heat = (self.heat + amount).min(6.0);
        self.decay_timer = 0.0;
        self.update_stars();
    }

    pub fn update(&mut self, dt: f32, visible: bool) {
        self.visible = visible;
        if visible {
            self.decay_timer = 0.0;
        } else {
            self.decay_timer += dt;
            // Decay starts after 5 seconds out of sight.
            if self.decay_timer > 5.0 {
                let rate = 0.15; // heat per second
                self.heat = (self.heat - rate * dt).max(0.0);
            }
        }
        self.update_stars();
    }

    fn update_stars(&mut self) {
        self.stars = match self.heat {
            h if h < 0.5 => 0,
            h if h < 1.5 => 1,
            h if h < 2.5 => 2,
            h if h < 3.5 => 3,
            h if h < 4.5 => 4,
            h if h < 5.5 => 5,
            _ => 6,
        };
    }

    pub fn clear(&mut self) {
        self.heat = 0.0;
        self.stars = 0;
        self.decay_timer = 0.0;
    }

    /// How many cops should be active for the current star level.
    pub fn target_cop_count(&self) -> usize {
        match self.stars {
            0 => 0,
            1 => 2,
            2 => 4,
            3 => 6,
            4 => 8,
            5 => 12,
            _ => 16,
        }
    }

    /// Whether cops shoot at the player.
    pub fn cops_shoot(&self) -> bool {
        self.stars >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_adds_and_maps_to_stars() {
        let mut w = WantedSystem::new();
        w.add_heat(1.0);
        assert_eq!(w.stars, 1);
        w.add_heat(1.0);
        assert_eq!(w.stars, 2);
        w.add_heat(10.0);
        assert_eq!(w.stars, 6);
    }

    #[test]
    fn heat_decays_when_not_visible() {
        let mut w = WantedSystem::new();
        w.add_heat(3.0);
        assert_eq!(w.stars, 3);
        // 10 seconds out of sight, decay starts at 5s, rate 0.15/s => 0.15 * 5 = 0.75
        w.update(10.0, false);
        assert!(w.heat < 3.0, "heat should decay: {}", w.heat);
    }

    #[test]
    fn heat_does_not_decay_when_visible() {
        let mut w = WantedSystem::new();
        w.add_heat(3.0);
        w.update(10.0, true);
        assert_eq!(w.heat, 3.0);
    }

    #[test]
    fn cop_count_scales() {
        let mut w = WantedSystem::new();
        assert_eq!(w.target_cop_count(), 0);
        w.add_heat(1.0);
        assert_eq!(w.target_cop_count(), 2);
        w.add_heat(10.0);
        assert_eq!(w.target_cop_count(), 16);
    }
}
