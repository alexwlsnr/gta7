//! Cop NPC: chases player on foot, shoots at wanted >= 2.
use raylib::ffi::Vector3;
use crate::mathx::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopState { Chase, Shoot, Dead }

pub struct Cop {
    pub pos: Vector3,
    pub prev_pos: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub health: f32,
    pub state: CopState,
    pub dead_timer: f32,
    pub fire_cooldown: f32,
    pub in_car: Option<usize>, // index into vehicles if in a police car
}

impl Cop {
    pub fn new(pos: Vector3) -> Self {
        Cop {
            pos,
            prev_pos: pos,
            yaw: 0.0,
            prev_yaw: 0.0,
            health: 60.0,
            state: CopState::Chase,
            dead_timer: 0.0,
            fire_cooldown: 0.0,
            in_car: None,
        }
    }

    pub fn take_damage(&mut self, dmg: f32) {
        self.health -= dmg;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.state = CopState::Dead;
            self.dead_timer = 10.0;
        }
    }

    /// `player_pos` = where the player is. `can_shoot` = wanted >= 2.
    /// Returns true if the cop fires this tick (caller applies damage).
    pub fn update(&mut self, dt: f32, player_pos: Vector3, can_shoot: bool) -> bool {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
        if self.state == CopState::Dead {
            self.dead_timer -= dt;
            return false;
        }
        let to_player = vsub(player_pos, self.pos);
        let dist = vlen_xz(to_player);
        self.yaw = lerp_angle(self.yaw, yaw_from_dir(vnorm_xz(to_player)), 5.0 * dt);

        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);

        if can_shoot && dist < 40.0 && dist > 2.0 {
            self.state = CopState::Shoot;
            // Stop and shoot.
            if self.fire_cooldown <= 0.0 {
                self.fire_cooldown = 0.8 + rand::random::<f32>() * 0.4;
                return true;
            }
        } else if dist > 1.8 {
            self.state = CopState::Chase;
            // Chase but keep minimum separation.
            let speed = 6.0;
            let fwd = vnorm_xz(to_player);
            if vlen_xz(fwd) > 0.0 {
                self.pos = vadd(self.pos, vscale(fwd, speed * dt));
            }
        }
        // If dist <= 1.8, stand still (don't overlap player).
        false
    }

    pub fn should_despawn(&self) -> bool {
        self.state == CopState::Dead && self.dead_timer <= 0.0
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

    pub fn dead(&self) -> bool { self.state == CopState::Dead }
}
