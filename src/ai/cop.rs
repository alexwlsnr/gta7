//! Cop NPC: chases player on foot, shoots at wanted >= 2.
use raylib::ffi::Vector3;
use raylib::color::Color;
use crate::mathx::*;
use crate::world::city::{City, Lane, Axis};
use crate::vehicle::{Vehicle, VehicleKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopState { Chase, Shoot, Dead }

pub struct Cop {
    pub pos: Vector3,
    pub prev_pos: Vector3,
    pub vel: Vector3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub health: f32,
    pub state: CopState,
    pub dead_timer: f32,
    pub fire_cooldown: f32,
    pub in_car: Option<usize>,
}

impl Cop {
    pub fn new(pos: Vector3) -> Self {
        Cop {
            pos,
            prev_pos: pos,
            vel: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
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
    /// Returns true if the cop fires this tick.
    pub fn update(&mut self, dt: f32, city: &crate::world::city::City, player_pos: Vector3, stars: u8) -> bool {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
        if self.state == CopState::Dead {
            self.dead_timer -= dt;
            self.pos = vadd(self.pos, vscale(self.vel, dt));
            self.vel = vscale(self.vel, 1.0 - 5.0 * dt);
            self.pos.y = city.get_ground_height(self.pos);
            let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 0.4);
            self.pos.x += push.x;
            self.pos.z += push.z;
            return false;
        }

        if stars == 0 {
            // No wanted level: stand still and do nothing (patrol/idle)
            self.vel = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
            self.pos.y = city.get_ground_height(self.pos);
            let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 0.4);
            self.pos.x += push.x;
            self.pos.z += push.z;
            return false;
        }

        let to_player = vsub(player_pos, self.pos);
        let dist = vlen_xz(to_player);
        self.yaw = lerp_angle(self.yaw, yaw_from_dir(vnorm_xz(to_player)), 5.0 * dt);

        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);

        let can_shoot = stars >= 2;

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
        // Building collision in 3D.
        self.pos.y = city.get_ground_height(self.pos);
        let push = city.resolve_circle_3d(self.pos.x, self.pos.y, self.pos.z, 0.4);
        self.pos.x += push.x;
        self.pos.z += push.z;
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

#[derive(Clone, Debug)]
pub struct PoliceCar {
    pub vehicle_idx: usize,
    pub current_lane: usize,
    pub lane_progress: f32,
}

impl PoliceCar {
    pub fn update(
        &mut self,
        city: &City,
        vehicles: &mut [Vehicle],
        cops: &mut [Cop],
        player_pos: Vector3,
        player_in_vehicle: bool,
        dt: f32,
    ) {
        let v = &vehicles[self.vehicle_idx];
        if v.destroyed {
            // Cops exit the destroyed vehicle
            for cop in cops.iter_mut() {
                if cop.in_car == Some(self.vehicle_idx) {
                    cop.in_car = None;
                    cop.pos = v.pos;
                    cop.state = CopState::Chase;
                }
            }
            return;
        }

        // If the player is on foot, and the cop car is very close, the cops should exit the car to chase on foot!
        // Also if the car health is low, they exit.
        let dist = vdist_xz(v.pos, player_pos);
        if (dist < 12.0 && !player_in_vehicle) || v.health < 40.0 {
            for cop in cops.iter_mut() {
                if cop.in_car == Some(self.vehicle_idx) {
                    cop.in_car = None;
                    cop.pos = v.pos;
                    cop.state = CopState::Chase;
                }
            }
            return;
        }

        let target_speed = 22.0; // Fast chase speed!
        let target_yaw;
        let mut force_direct_aim = false;

        if dist < 25.0 {
            // Direct steer towards player (ramming mode!)
            let to_player = vsub(player_pos, v.pos);
            target_yaw = yaw_from_dir(vnorm_xz(to_player));
            force_direct_aim = true;
        } else {
            // Lane follow towards player
            let lane = city.lanes[self.current_lane];
            let from = city.intersection(lane.from.0, lane.from.1);
            let to = city.intersection(lane.to.0, lane.to.1);
            let lane_len = vdist_xz(from, to);
            if lane_len < 1.0 {
                self.pick_chase_lane(city, player_pos);
                return;
            }

            let advance = (target_speed * dt) / lane_len;
            self.lane_progress += advance;

            if self.lane_progress >= 1.0 {
                self.lane_progress = 0.0;
                self.pick_chase_lane(city, player_pos);
                return;
            }

            let (cx, cz) = lane_offset(&lane, city.road_width);
            let t = self.lane_progress;
            let target_pos = Vector3 {
                x: lerp(from.x, to.x, t) + cx,
                y: 0.0,
                z: lerp(from.z, to.z, t) + cz,
            };
            target_yaw = yaw_from_dir(vnorm_xz(vsub(to, from)));

            let v = &mut vehicles[self.vehicle_idx];
            v.update_ai(target_speed, target_yaw, city, &crate::config::Config::default(), dt);
            v.pos.x = target_pos.x;
            v.pos.z = target_pos.z;
        }

        if force_direct_aim {
            let v = &mut vehicles[self.vehicle_idx];
            v.update_ai(target_speed, target_yaw, city, &crate::config::Config::default(), dt);
        }
    }

    fn pick_chase_lane(&mut self, city: &City, player_pos: Vector3) {
        let cur = &city.lanes[self.current_lane];
        let dest = cur.to;
        let mut candidates: Vec<usize> = Vec::new();
        for (i, l) in city.lanes.iter().enumerate() {
            if l.from == dest {
                candidates.push(i);
            }
        }
        if candidates.is_empty() {
            for (i, l) in city.lanes.iter().enumerate() {
                if l.from == cur.to && l.to == cur.from {
                    self.current_lane = i;
                    self.lane_progress = 0.0;
                    return;
                }
            }
            return;
        }

        let mut best_lane = candidates[0];
        let mut best_dist = f32::MAX;
        for &i in &candidates {
            let l = &city.lanes[i];
            let to_pos = city.intersection(l.to.0, l.to.1);
            let d = vdist_xz(to_pos, player_pos);
            if d < best_dist {
                best_dist = d;
                best_lane = i;
            }
        }
        self.current_lane = best_lane;
        self.lane_progress = 0.0;
    }
}

pub fn spawn_police_car(
    city: &City,
    vehicles: &mut Vec<Vehicle>,
    cops: &mut Vec<Cop>,
    police_cars: &mut Vec<PoliceCar>,
    player_pos: Vector3,
) {
    if city.lanes.is_empty() {
        return;
    }

    let mut candidates = Vec::new();
    for (idx, lane) in city.lanes.iter().enumerate() {
        let from = city.intersection(lane.from.0, lane.from.1);
        let dist = vdist_xz(from, player_pos);
        if dist > 40.0 && dist < 70.0 {
            candidates.push(idx);
        }
    }

    let lane_idx = if candidates.is_empty() {
        rand::random::<usize>() % city.lanes.len()
    } else {
        candidates[rand::random::<usize>() % candidates.len()]
    };

    let lane = &city.lanes[lane_idx];
    let from = city.intersection(lane.from.0, lane.from.1);
    let to = city.intersection(lane.to.0, lane.to.1);
    let (cx, cz) = lane_offset(lane, city.road_width);
    let pos = Vector3 { x: from.x + cx, y: 0.0, z: from.z + cz };
    let yaw = yaw_from_dir(vnorm_xz(vsub(to, from)));

    let mut v = Vehicle::new(pos, yaw, Color::new(20, 20, 20, 255), VehicleKind::Police);
    v.is_traffic = true;
    let v_idx = vehicles.len();
    vehicles.push(v);

    let mut cop_driver = Cop::new(pos);
    cop_driver.in_car = Some(v_idx);
    cops.push(cop_driver);

    let mut cop_passenger = Cop::new(pos);
    cop_passenger.in_car = Some(v_idx);
    cops.push(cop_passenger);

    police_cars.push(PoliceCar {
        vehicle_idx: v_idx,
        current_lane: lane_idx,
        lane_progress: 0.0,
    });
}

fn lane_offset(lane: &Lane, rw: f32) -> (f32, f32) {
    let offset = rw * 0.25;
    match lane.axis {
        Axis::X => (0.0, -offset * lane.dir as f32),
        Axis::Z => (offset * lane.dir as f32, 0.0),
    }
}
