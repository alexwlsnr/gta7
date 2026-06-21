//! Arcade vehicle physics + enter/exit + damage.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::input::Input;
use crate::world::city::City;
use crate::config::Config;

pub struct Vehicle {
    pub pos: Vector3,
    pub prev_pos: Vector3,
    pub vel: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub steer: f32,
    pub speed: f32,       // signed forward speed
    pub health: f32,
    pub max_health: f32,
    pub color: raylib::color::Color,
    pub is_traffic: bool, // AI-controlled
    pub destroyed: bool,
    pub explode_timer: f32,
    pub occupied: bool,
    pub kind: VehicleKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VehicleKind {
    Civilian,
    Police,
}

impl Vehicle {
    pub fn new(pos: Vector3, yaw: f32, color: raylib::color::Color, kind: VehicleKind) -> Self {
        let max_health = match kind {
            VehicleKind::Civilian => 100.0,
            VehicleKind::Police => 140.0,
        };
        Vehicle {
            pos,
            prev_pos: pos,
            vel: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            yaw,
            prev_yaw: yaw,
            steer: 0.0,
            speed: 0.0,
            health: max_health,
            max_health,
            color,
            is_traffic: false,
            destroyed: false,
            explode_timer: 0.0,
            occupied: false,
            kind,
        }
    }

    /// Player-driven update.
    pub fn update_driven(&mut self, input: &Input, city: &City, cfg: &Config, dt: f32) {
        if self.destroyed {
            return;
        }
        // Throttle: W=forward, S=reverse/brake
        let throttle = input.move_y;
        let max_fwd = 28.0;
        let max_rev = -10.0;
        let accel = 30.0;
        if throttle > 0.0 {
            self.speed += accel * throttle * dt;
        } else if throttle < 0.0 {
            if self.speed > 0.0 {
                self.speed += -50.0 * dt; // brake
            } else {
                self.speed += accel * throttle * dt; // reverse
            }
        } else {
            // Engine drag
            self.speed *= 1.0 - 1.5 * dt;
            if self.speed.abs() < 0.3 {
                self.speed = 0.0;
            }
        }
        self.speed = clamp(self.speed, max_rev, max_fwd);

        // Steering: proportional to speed, reduced at very low speed.
        let steer_input = -input.move_x; // A=left (+steer)
        let speed_factor = clamp(self.speed.abs() / 10.0, 0.0, 1.0);
        let steer_rate = 2.2 * speed_factor;
        self.steer = approach(self.steer, steer_input, 4.0 * dt);
        let turn = self.steer * steer_rate * (if self.speed >= 0.0 { 1.0 } else { -1.0 });
        self.yaw += turn * dt;

        // Handbrake: kill speed faster, allow sharper turn.
        if input.handbrake {
            self.speed *= 1.0 - 3.0 * dt;
        }

        // Integrate position.
        let fwd = dir_from_yaw(self.yaw);
        self.pos = vadd(self.pos, vscale(fwd, self.speed * dt));

        // World bounds.
        let lim = cfg.world_half() - 3.0;
        self.pos.x = clamp(self.pos.x, -lim, lim);
        self.pos.z = clamp(self.pos.z, -lim, lim);

        // Building collision (radius ~ car half-width).
        let push = city.resolve_circle(self.pos.x, self.pos.z, 1.5);
        if vlen_xz(push) > 0.01 {
            self.pos.x += push.x;
            self.pos.z += push.z;
            // Crash damage proportional to speed.
            let impact = self.speed.abs() * 0.15;
            if impact > 5.0 {
                self.take_damage(impact * 0.5);
                self.speed *= 0.3;
            }
        }
    }

    /// AI-driven update (traffic / police car). Uses a target velocity + yaw.
    pub fn update_ai(&mut self, target_speed: f32, target_yaw: f32, city: &City, cfg: &Config, dt: f32) {
        if self.destroyed {
            return;
        }
        self.speed = approach(self.speed, target_speed, 15.0 * dt);
        self.yaw = lerp_angle(self.yaw, target_yaw, 3.0 * dt);
        let fwd = dir_from_yaw(self.yaw);
        self.pos = vadd(self.pos, vscale(fwd, self.speed * dt));
        let lim = cfg.world_half() - 3.0;
        self.pos.x = clamp(self.pos.x, -lim, lim);
        self.pos.z = clamp(self.pos.z, -lim, lim);
        let push = city.resolve_circle(self.pos.x, self.pos.z, 1.5);
        self.pos.x += push.x;
        self.pos.z += push.z;
    }

    pub fn take_damage(&mut self, dmg: f32) {
        if self.destroyed {
            return;
        }
        self.health -= dmg;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.destroyed = true;
            self.explode_timer = 1.5;
            self.speed = 0.0;
        }
    }

    pub fn step_explosion(&mut self, dt: f32) -> bool {
        if !self.destroyed {
            return false;
        }
        if self.explode_timer > 0.0 {
            self.explode_timer -= dt;
            if self.explode_timer <= 0.0 {
                return true; // explode now
            }
        }
        false
    }

    pub fn snapshot(&mut self) {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
    }

    pub fn render_pos(&self, alpha: f32) -> Vector3 {
        vlerp(self.prev_pos, self.pos, alpha)
    }
    pub fn render_yaw(&self, alpha: f32) -> f32 {
        lerp_angle(self.prev_yaw, self.yaw, alpha)
    }

    pub fn damage_level(&self) -> f32 {
        1.0 - (self.health / self.max_health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;
    use crate::config::Config;

    #[test]
    fn vehicle_accelerates_forward() {
        let cfg = Config::default();
        let city = City::generate(&cfg);
        let mut v = Vehicle::new(
            Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            0.0,
            raylib::color::Color::RED,
            VehicleKind::Civilian,
        );
        let mut input = Input::default();
        input.move_y = 1.0;
        v.update_driven(&input, &city, &cfg, 0.1);
        assert!(v.speed > 0.0, "speed should be positive after throttle, got {}", v.speed);
    }

    #[test]
    fn vehicle_steers_at_speed() {
        let cfg = Config::default();
        let city = City::generate(&cfg);
        let mut v = Vehicle::new(
            Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            0.0,
            raylib::color::Color::RED,
            VehicleKind::Civilian,
        );
        v.speed = 20.0;
        let mut input = Input::default();
        input.move_x = 1.0; // D=right -> steer left (negative steer_input=-move_x)
        v.update_driven(&input, &city, &cfg, 0.1);
        assert!(v.yaw != 0.0, "yaw should change when steering at speed");
    }

    #[test]
    fn vehicle_takes_damage_and_destroys() {
        let mut v = Vehicle::new(
            Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            0.0,
            raylib::color::Color::RED,
            VehicleKind::Civilian,
        );
        v.take_damage(200.0);
        assert!(v.destroyed);
        assert_eq!(v.health, 0.0);
    }
}
