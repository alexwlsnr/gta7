//! Pedestrian NPC: wanders sidewalks, flees gunfire, drops cash on death.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::world::city::City;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PedState { Wander, Flee, Dead }

use crate::render::models::HairStyle;
use raylib::color::Color;

pub struct Ped {
    pub pos: Vector3,
    pub prev_pos: Vector3,
    pub vel: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub health: f32,
    pub state: PedState,
    pub dead_timer: f32,
    pub color: Color, // Shirt color
    pub pants_color: Color,
    pub hair_color: Color,
    pub hair_style: HairStyle,
    pub has_glasses: bool,
    pub cash: i32,
    pub wander_timer: f32,
    pub sw_axis: i32,   // 0 = walk along X, 1 = walk along Z
    pub sw_dir: f32,    // +1 or -1
    pub turn_cd: f32,   // cooldown between turns at intersections
    pub flee_dir: Vector3,
}

impl Ped {
    pub fn new(pos: Vector3, color: Color) -> Self {
        let pants_colors = [
            Color::new(45, 52, 85, 255),  // blue jeans
            Color::new(30, 30, 30, 255),  // black pants
            Color::new(100, 70, 50, 255), // brown khaki
            Color::new(80, 85, 90, 255),  // grey pants
        ];
        let pants_color = pants_colors[rand::random::<usize>() % pants_colors.len()];

        let hair_colors = [
            Color::new(20, 15, 10, 255),   // black
            Color::new(80, 50, 30, 255),   // brown
            Color::new(210, 180, 80, 255), // blonde
            Color::new(180, 70, 30, 255),  // ginger
            Color::new(180, 180, 180, 255), // grey
        ];
        let hair_color = hair_colors[rand::random::<usize>() % hair_colors.len()];

        let styles = [HairStyle::Bald, HairStyle::ShortHair, HairStyle::Afro, HairStyle::Cap];
        let hair_style = styles[rand::random::<usize>() % styles.len()];
        let has_glasses = rand::random::<f32>() < 0.4;

        Ped {
            pos,
            prev_pos: pos,
            vel: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            yaw: rand::random::<f32>() * std::f32::consts::TAU,
            prev_yaw: 0.0,
            health: 35.0,
            state: PedState::Wander,
            dead_timer: 0.0,
            color,
            pants_color,
            hair_color,
            hair_style,
            has_glasses,
            cash: (rand::random::<u32>() % 80) as i32 + 10,
            wander_timer: rand::random::<f32>() * 3.0,
            sw_axis: (rand::random::<u32>() % 2) as i32,
            sw_dir: if rand::random::<f32>() < 0.5 { 1.0 } else { -1.0 },
            turn_cd: 0.0,
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
            self.pos = vadd(self.pos, vscale(self.vel, dt));
            self.vel = vscale(self.vel, 1.0 - 5.0 * dt);
            self.pos.y = city.get_ground_height(self.pos);
            let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 0.4);
            self.pos.x += push.x;
            self.pos.z += push.z;
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
                let speed = 2.0;
                let origin = -city.ground_half;
                let bs = city.block_size;
                let sw_off = city.road_width * 0.5 + city.sidewalk_width * 0.5;

                // Walk along current axis.
                let fwd = if self.sw_axis == 0 {
                    Vector3 { x: self.sw_dir, y: 0.0, z: 0.0 }
                } else {
                    Vector3 { x: 0.0, y: 0.0, z: self.sw_dir }
                };
                let prev_walk = if self.sw_axis == 0 { self.pos.x } else { self.pos.z };
                self.pos = vadd(self.pos, vscale(fwd, speed * dt));
                self.yaw = yaw_from_dir(fwd);

                // Snap cross-axis to sidewalk center.
                if self.sw_axis == 0 {
                    let line = origin + ((self.pos.z - origin) / bs).round() as f32 * bs;
                    let side = if self.pos.z >= line { sw_off } else { -sw_off };
                    self.pos.z = line + side;
                } else {
                    let line = origin + ((self.pos.x - origin) / bs).round() as f32 * bs;
                    let side = if self.pos.x >= line { sw_off } else { -sw_off };
                    self.pos.x = line + side;
                }

                // At intersections, maybe turn.
                self.turn_cd -= dt;
                let cur_walk = if self.sw_axis == 0 { self.pos.x } else { self.pos.z };
                let prev_block = ((prev_walk - origin) / bs).floor() as i32;
                let cur_block = ((cur_walk - origin) / bs).floor() as i32;
                if prev_block != cur_block && self.turn_cd <= 0.0 {
                    self.turn_cd = 2.0 + rand::random::<f32>() * 4.0;
                    let turn = rand::random::<u32>() % 5;
                    match turn {
                        0 | 1 | 2 => {} // 60% continue straight
                        3 => {
                            // Turn: switch axis, random direction.
                            self.sw_axis = 1 - self.sw_axis;
                            self.sw_dir = if rand::random::<f32>() < 0.5 { 1.0 } else { -1.0 };
                        }
                        4 => { self.sw_dir *= -1.0; } // Reverse
                        _ => {}
                    }
                }
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
                    self.turn_cd = 1.0;
                    // Snap to nearest sidewalk.
                    let (sw_pos, axis) = city.nearest_sidewalk(self.pos.x, self.pos.z);
                    self.pos.x = sw_pos.x;
                    self.pos.z = sw_pos.z;
                    self.sw_axis = axis;
                    self.sw_dir = if rand::random::<f32>() < 0.5 { 1.0 } else { -1.0 };
                }
            }
            PedState::Dead => {}
        }
        // Building collision in 3D.
        self.pos.y = city.get_ground_height(self.pos);
        let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 0.4);
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
