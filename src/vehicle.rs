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
    pub pitch: f32,
    pub prev_pitch: f32,
    pub roll: f32,
    pub prev_roll: f32,
    pub air_time: f32,
    pub just_landed_stunt: Option<f32>,
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
            pitch: 0.0,
            prev_pitch: 0.0,
            roll: 0.0,
            prev_roll: 0.0,
            air_time: 0.0,
            just_landed_stunt: None,
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
    pub fn update_driven(&mut self, input: &Input, city: &City, cfg: &Config, dt: f32) -> bool {
        self.just_landed_stunt = None;
        if self.destroyed {
            return false;
        }

        let fwd = dir_from_yaw(self.yaw);
        let right = Vector3 { x: -fwd.z, y: 0.0, z: fwd.x };

        // 1. Project current velocity to get forward and lateral speeds
        let mut fwd_speed = vdot(self.vel, fwd);
        let mut lat_speed = vdot(self.vel, right);

        // 2. Accelerate / brake along forward direction
        let throttle = input.move_y;
        let accel = 35.0;
        let max_fwd = 30.0;
        let max_rev = -12.0;

        if throttle > 0.0 {
            fwd_speed += accel * throttle * dt;
        } else if throttle < 0.0 {
            if fwd_speed > 0.1 {
                fwd_speed += -55.0 * dt; // strong brake
            } else {
                fwd_speed += accel * throttle * dt; // reverse
            }
        } else {
            fwd_speed *= 1.0 - 1.2 * dt; // engine drag
        }
        fwd_speed = clamp(fwd_speed, max_rev, max_fwd);

        // 3. Steering: turn rate based on speed
        let steer_input = -input.move_x; // A=left, D=right
        self.steer = approach(self.steer, steer_input, 4.0 * dt);
        
        let is_moving = fwd_speed.abs() > 0.5;
        let speed_ratio = fwd_speed.abs() / max_fwd;
        let turn_speed = if is_moving {
            let base_turn = 2.4 * clamp(speed_ratio * 2.0, 0.15, 1.0);
            if input.handbrake {
                base_turn * 1.6 // sharper turn when handbraking
            } else {
                base_turn
            }
        } else {
            0.0
        };
        let turn = self.steer * turn_speed * (if fwd_speed >= 0.0 { 1.0 } else { -1.0 });
        self.yaw += turn * dt;

        // 4. Grip and Lateral slip (Drift!)
        let grip = if input.handbrake {
            1.8 // low lateral grip -> slide/drift!
        } else {
            7.5 // high grip -> align velocity to wheels
        };
        lat_speed = approach(lat_speed, 0.0, grip * dt * 10.0);

        if input.handbrake {
            fwd_speed *= 1.0 - 1.2 * dt;
        }

        // 5. Reconstruct 3D velocity
        let target_vel = vadd(vscale(fwd, fwd_speed), vscale(right, lat_speed));
        self.speed = fwd_speed;

        let ground_h = city.get_ground_height(self.pos);
        let is_airborne = self.pos.y > ground_h + 0.05;

        let mut crashed = false;

        if is_airborne {
            // Apply gravity to vertical velocity
            self.vel.y -= 18.0 * dt;
            // Airborne rotational control for flips and spins!
            self.pitch += input.move_y * 2.5 * dt;
            self.roll += input.move_x * 2.5 * dt;
            self.yaw += -input.move_x * 1.5 * dt;
            self.air_time += dt;

            // Retain horizontal speed but apply gravity
            self.vel.x = target_vel.x;
            self.vel.z = target_vel.z;
        } else {
            // On ground or ramp
            if let Some((ramp_h, ramp_angle)) = city.get_ramp_height_and_angle(self.pos) {
                // Climb ramp
                self.pos.y = ramp_h;
                self.pitch = -ramp_angle; // Tilt nose up
                self.roll = 0.0;

                // Adjust velocities based on slope climb
                self.vel.y = self.speed * ramp_angle.sin();
                self.vel.x = target_vel.x * ramp_angle.cos();
                self.vel.z = target_vel.z * ramp_angle.cos();
            } else {
                // Flat surface (road or building roof)
                self.pos.y = ground_h;
                self.vel.y = 0.0;
                self.pitch = 0.0;
                self.roll = 0.0;
                self.vel = target_vel;
            }
        }

        // 6. Integrate position
        self.pos = vadd(self.pos, vscale(self.vel, dt));

        // Limit position to world bounds
        let lim = cfg.world_half() - 3.0;
        self.pos.x = clamp(self.pos.x, -lim, lim);
        self.pos.z = clamp(self.pos.z, -lim, lim);

        // Check landing transition
        let next_ground_h = city.get_ground_height(self.pos);
        if self.pos.y < next_ground_h {
            self.pos.y = next_ground_h;
            
            let landing_impact = -self.vel.y;
            self.vel.y = 0.0;
            
            // Re-align car on landing
            self.pitch = 0.0;
            self.roll = 0.0;

            if landing_impact > 5.0 {
                self.take_damage(landing_impact * 1.5);
                crashed = true;
            }

            if self.air_time > 0.4 {
                self.just_landed_stunt = Some(self.air_time);
            }
            self.air_time = 0.0;
        }

        // 7. Building collision in 3D (radius 1.5)
        let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 1.5);
        if vlen_xz(push) > 0.01 {
            self.pos.x += push.x;
            self.pos.z += push.z;
            
            let impact = vlen_xz(self.vel);
            if impact > 5.0 {
                self.take_damage(impact * 0.4);
                self.vel = vscale(self.vel, -0.2); // bounce slightly backward
                self.speed *= -0.2;
                crashed = true;
            }
        }
        crashed
    }

    pub fn update_ai(&mut self, target_speed: f32, target_yaw: f32, city: &City, cfg: &Config, dt: f32) {
        if self.destroyed {
            return;
        }
        self.speed = approach(self.speed, target_speed, 15.0 * dt);
        self.yaw = lerp_angle(self.yaw, target_yaw, 3.0 * dt);
        let fwd = dir_from_yaw(self.yaw);
        self.vel = vscale(fwd, self.speed);
        self.pos = vadd(self.pos, vscale(self.vel, dt));
        // Keep traffic on road or ramp height
        self.pos.y = city.get_ground_height(self.pos);
        let lim = cfg.world_half() - 3.0;
        self.pos.x = clamp(self.pos.x, -lim, lim);
        self.pos.z = clamp(self.pos.z, -lim, lim);
        let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 1.5);
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
        self.prev_pitch = self.pitch;
        self.prev_roll = self.roll;
    }

    pub fn render_pos(&self, alpha: f32) -> Vector3 {
        vlerp(self.prev_pos, self.pos, alpha)
    }
    pub fn render_yaw(&self, alpha: f32) -> f32 {
        lerp_angle(self.prev_yaw, self.yaw, alpha)
    }
    pub fn render_pitch(&self, alpha: f32) -> f32 {
        lerp(self.prev_pitch, self.pitch, alpha)
    }
    pub fn render_roll(&self, alpha: f32) -> f32 {
        lerp(self.prev_roll, self.roll, alpha)
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
        let input = Input { move_y: 1.0, ..Default::default() };
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
        v.vel = Vector3 { x: 0.0, y: 0.0, z: 20.0 };
        let input = Input { move_x: 1.0, ..Default::default() }; // D=right -> steer left (negative steer_input=-move_x)
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
