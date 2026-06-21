//! Pedestrian NPC: wanders sidewalks, flees gunfire, drops cash on death.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::world::city::City;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PedState { Wander, Flee, Dead }

pub struct Ped {
    pub pos: Vector3,
    pub prev_pos: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub health: f32,
    pub state: PedState,
    pub dead_timer: f32,
    pub color: raylib::color::Color,
    pub cash: i32,
    pub wander_timer: f32,
    pub flee_dir: Vector3,
}

impl Ped {
    pub fn new(pos: Vector3, color: raylib::color::Color) -> Self {
        Ped {
            pos,
            prev_pos: pos,
            yaw: rand::random::<f32>() * std::f32::consts::TAU,
            prev_yaw: 0.0,
            health: 35.0,
            state: PedState::Wander,
            dead_timer: 0.0,
            color,
            cash: (rand::random::<u32>() % 80) as i32 + 10,
            wander_timer: rand::random::<f32>() * 3.0,
            flee_dir: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
        }
    }

    pub fn take_damage(&mut self, dmg: f32) {
        self.health -= dmg;
        if self.health <= 0.0 && self.state != PedState::Dead {
            self.health = 0.0;
            self.state = PedState::Dead;
            self.dead_timer = 10.0; // body stays for 10s then despawns
        } else if self.state != PedState::Dead {
            self.state = PedState::Flee;
        }
    }

    /// `panic_pos` = position of gunfire/explosion to flee from. None = no threat.
    pub fn update(&mut self, dt: f32, city: &City, panic_pos: Option<Vector3>) {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
        if self.state == PedState::Dead {
            self.dead_timer -= dt;
            return;
        }
        match self.state {
            PedState::Wander => {
                if let Some(pp) = panic_pos {
                    if vdist_xz(self.pos, pp) < 25.0 {
                        self.state = PedState::Flee;
                        let away = vnorm_xz(vsub(self.pos, pp));
                        self.flee_dir = away;
                        self.yaw = yaw_from_dir(away);
                        return;
                    }
                }
                self.wander_timer -= dt;
                if self.wander_timer <= 0.0 {
                    self.wander_timer = 2.0 + rand::random::<f32>() * 3.0;
                    self.yaw = rand::random::<f32>() * std::f32::consts::TAU;
                }
                let fwd = dir_from_yaw(self.yaw);
                let speed = 2.0;
                self.pos = vadd(self.pos, vscale(fwd, speed * dt));
            }
            PedState::Flee => {
                let fwd = dir_from_yaw(self.yaw);
                let speed = 5.5;
                self.pos = vadd(self.pos, vscale(fwd, speed * dt));
                // Flee for a while then calm down.
                self.wander_timer -= dt;
                if self.wander_timer < -4.0 {
                    self.state = PedState::Wander;
                    self.wander_timer = 2.0;
                }
            }
            PedState::Dead => {}
        }
        // Building collision.
        let push = city.resolve_circle(self.pos.x, self.pos.z, 0.4);
        if vlen_xz(push) > 0.01 {
            self.pos.x += push.x;
            self.pos.z += push.z;
            // Turn away from wall.
            self.yaw += 1.5 + rand::random::<f32>() * 0.5;
        }
    }

    pub fn should_despawn(&self) -> bool {
        self.state == PedState::Dead && self.dead_timer <= 0.0
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

    pub fn dead(&self) -> bool { self.state == PedState::Dead }
}
