//! Fixed-timestep accumulator with interpolation alpha.
use crate::config::LogicRate;

pub struct Clock {
    rate: LogicRate,
    accumulator: f32,
    pub alpha: f32, // 0..1 for render interpolation
}

impl Clock {
    pub fn new(rate: LogicRate) -> Self {
        Clock { rate, accumulator: 0.0, alpha: 0.0 }
    }

    pub fn rate(&self) -> LogicRate { self.rate }
    pub fn set_rate(&mut self, r: LogicRate) {
        self.rate = r;
        self.accumulator = 0.0;
    }

    /// Advance by real frame time. Returns number of logic steps to run.
    pub fn tick(&mut self, frame_dt: f32) -> usize {
        // Clamp to avoid spiral-of-death after stalls.
        self.accumulator += frame_dt.min(0.25);
        let dt = self.rate.dt();
        let mut steps = 0;
        while self.accumulator >= dt {
            self.accumulator -= dt;
            steps += 1;
            if steps > 8 {
                // Drop backlog if we got too far behind.
                self.accumulator = 0.0;
                break;
            }
        }
        self.alpha = self.accumulator / dt;
        steps
    }

    pub fn dt(&self) -> f32 { self.rate.dt() }
}
