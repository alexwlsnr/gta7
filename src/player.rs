//! Player controller: on-foot movement, health, inventory, enter/exit vehicle.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::input::Input;
use crate::world::city::City;
use crate::config::Config;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weapon {
    Unarmed,
    Pistol,
    Smg,
}

impl Weapon {
    pub fn damage(self) -> f32 {
        match self {
            Weapon::Unarmed => 0.0,
            Weapon::Pistol => 25.0,
            Weapon::Smg => 18.0,
        }
    }
    pub fn fire_rate(self) -> f32 {
        match self {
            Weapon::Unarmed => 0.0,
            Weapon::Pistol => 0.28,
            Weapon::Smg => 0.09,
        }
    }
    pub fn mag_size(self) -> u32 {
        match self {
            Weapon::Unarmed => 0,
            Weapon::Pistol => 17,
            Weapon::Smg => 30,
        }
    }
    pub fn range(self) -> f32 {
        match self {
            Weapon::Unarmed => 2.5,
            Weapon::Pistol => 120.0,
            Weapon::Smg => 90.0,
        }
    }
    pub fn spread(self) -> f32 {
        match self {
            Weapon::Unarmed => 0.0,
            Weapon::Pistol => 0.015,
            Weapon::Smg => 0.04,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Weapon::Unarmed => "Fists",
            Weapon::Pistol => "Pistol",
            Weapon::Smg => "SMG",
        }
    }
}

pub struct Player {
    pub pos: Vector3,
    pub prev_pos: Vector3,      // for render interpolation
    pub vel: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub on_ground: bool,
    pub health: f32,
    pub armor: f32,
    pub money: i64,
    pub weapon: Weapon,
    pub ammo: u32,
    pub reserve: u32,
    pub fire_cooldown: f32,
    pub reloading: f32,         // >0 while reloading
    pub in_vehicle: Option<usize>, // index into Game.vehicles
    pub alive: bool,
    pub respawn_timer: f32,
    pub recoil: f32,            // vertical kick for camera
    pub want_fire: bool,
}

impl Player {
    pub fn new(pos: Vector3) -> Self {
        Player {
            pos,
            prev_pos: pos,
            vel: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            yaw: 0.0,
            prev_yaw: 0.0,
            on_ground: true,
            health: 100.0,
            armor: 0.0,
            money: 500,
            weapon: Weapon::Pistol,
            ammo: 17,
            reserve: 68,
            fire_cooldown: 0.0,
            reloading: 0.0,
            in_vehicle: None,
            alive: true,
            respawn_timer: 0.0,
            recoil: 0.0,
            want_fire: false,
        }
    }

    pub fn switch_weapon(&mut self) {
        self.weapon = match self.weapon {
            Weapon::Unarmed => Weapon::Pistol,
            Weapon::Pistol => Weapon::Smg,
            Weapon::Smg => Weapon::Unarmed,
        };
        self.ammo = self.weapon.mag_size();
        self.reserve = self.ammo * 4;
        self.reloading = 0.0;
    }

    pub fn start_reload(&mut self) {
        if self.weapon == Weapon::Unarmed { return; }
        if self.reloading > 0.0 { return; }
        if self.ammo >= self.weapon.mag_size() { return; }
        if self.reserve == 0 { return; }
        self.reloading = 1.2;
    }

    pub fn take_damage(&mut self, dmg: f32) {
        if !self.alive { return; }
        let mut d = dmg;
        if self.armor > 0.0 {
            let absorbed = d.min(self.armor);
            self.armor -= absorbed;
            d -= absorbed;
        }
        self.health -= d;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.alive = false;
            self.respawn_timer = 3.0;
        }
    }

    pub fn heal(&mut self, amt: f32) {
        self.health = (self.health + amt).min(100.0);
    }
    pub fn add_armor(&mut self, amt: f32) {
        self.armor = (self.armor + amt).min(100.0);
    }

    /// On-foot update. Returns events (fire request) via mutable fields.
    pub fn update_on_foot(&mut self, input: &Input, city: &City, cfg: &Config, dt: f32) {
        // Look
        self.yaw -= input.look_dx * cfg.mouse_sensitivity;
        // Recoil recovery
        self.recoil = (self.recoil - dt * 4.0).max(0.0);

        // Movement relative to yaw
        let fwd = dir_from_yaw(self.yaw);
        let right = Vector3 { x: fwd.z, y: 0.0, z: -fwd.x };
        let mut move_dir = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        move_dir = vadd(move_dir, vscale(fwd, input.move_y));
        move_dir = vadd(move_dir, vscale(right, input.move_x));
        let speed = if input.sprint { 9.0 } else { 4.5 };
        let horiz = vnorm_xz(move_dir);
        if vlen_xz(horiz) > 0.0 {
            self.vel.x = horiz.x * speed;
            self.vel.z = horiz.z * speed;
        } else {
            self.vel.x *= 0.6;
            self.vel.z *= 0.6;
        }

        // Jump + gravity
        if input.jump && self.on_ground {
            self.vel.y = 6.5;
            self.on_ground = false;
        }
        self.vel.y -= 20.0 * dt;

        // Integrate
        self.pos = vadd(self.pos, vscale(self.vel, dt));
        // Ground
        if self.pos.y <= 0.0 {
            self.pos.y = 0.0;
            self.vel.y = 0.0;
            self.on_ground = true;
        }
        // World bounds
        let lim = cfg.world_half() - 2.0;
        self.pos.x = clamp(self.pos.x, -lim, lim);
        self.pos.z = clamp(self.pos.z, -lim, lim);
        // Building collision (radius 0.5)
        let push = city.resolve_circle(self.pos.x, self.pos.z, 0.5);
        self.pos.x += push.x;
        self.pos.z += push.z;

        // Weapon timers
        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        if self.reloading > 0.0 {
            self.reloading -= dt;
            if self.reloading <= 0.0 {
                let need = self.weapon.mag_size() - self.ammo;
                let take = need.min(self.reserve);
                self.ammo += take;
                self.reserve -= take;
                self.reloading = 0.0;
            }
        }
        self.want_fire = input.fire_held && self.weapon != Weapon::Unarmed;
    }

    /// Called every tick regardless of state: recoil, cooldown, respawn.
    pub fn update_meta(&mut self, dt: f32) {
        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        self.recoil = (self.recoil - dt * 4.0).max(0.0);
        if !self.alive {
            self.respawn_timer -= dt;
        }
    }

    /// Save state for interpolation.
    pub fn snapshot(&mut self) {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
    }

    /// Interpolated render transform.
    pub fn render_pos(&self, alpha: f32) -> Vector3 {
        vlerp(self.prev_pos, self.pos, alpha)
    }
    pub fn render_yaw(&self, alpha: f32) -> f32 {
        lerp_angle(self.prev_yaw, self.yaw, alpha)
    }
}
