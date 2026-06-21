//! Traffic AI: civilian cars follow the lane graph, stop at red lights.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::world::city::{City, Lane, Axis};
use crate::vehicle::{Vehicle, VehicleKind};

/// A traffic car with lane-following state.
pub struct TrafficCar {
    pub vehicle_idx: usize,   // index into Game.vehicles
    pub current_lane: usize,  // index into city.lanes
    pub lane_progress: f32,   // 0..1 along the lane
    pub speed: f32,
}

impl TrafficCar {
    pub fn new(vehicle_idx: usize, lane: usize) -> Self {
        TrafficCar {
            vehicle_idx,
            current_lane: lane,
            lane_progress: 0.0,
            speed: 12.0,
        }
    }

    /// Update traffic car along the lane graph. Returns new lane if it reached the end.
    pub fn update(
        &mut self,
        city: &City,
        vehicles: &mut [Vehicle],
        player_pos: Vector3,
        dt: f32,
    ) {
        let lane = city.lanes[self.current_lane];
        let from = city.intersection(lane.from.0, lane.from.1);
        let to = city.intersection(lane.to.0, lane.to.1);
        let lane_len = vdist_xz(from, to);
        if lane_len < 1.0 {
            self.pick_next_lane(city);
            return;
        }

        let near_end = self.lane_progress > 0.85;
        let mut should_stop = false;
        if near_end {
            for l in &city.lights {
                if (l.pos.x - to.x).abs() < 1.0 && (l.pos.z - to.z).abs() < 1.0 {
                    if l.state == crate::world::city::LightState::Red {
                        should_stop = true;
                    }
                }
            }
        }

        // Slow down if player car is very close ahead.
        let v = &vehicles[self.vehicle_idx];
        let dist_to_player = vdist_xz(v.pos, player_pos);
        let mut target_speed = self.speed;
        if should_stop {
            target_speed = 0.0;
        }
        if dist_to_player < 8.0 {
            target_speed = target_speed * 0.3;
        }

        // Advance along lane.
        let advance = (target_speed * dt) / lane_len;
        self.lane_progress += advance;

        if self.lane_progress >= 1.0 {
            self.lane_progress = 0.0;
            self.pick_next_lane(city);
            return;
        }

        // Compute position along lane with right-side offset.
        let (cx, cz) = lane_offset(&lane, city.road_width);
        let t = self.lane_progress;
        let pos = Vector3 {
            x: lerp(from.x, to.x, t) + cx,
            y: 0.0,
            z: lerp(from.z, to.z, t) + cz,
        };
        let target_yaw = yaw_from_dir(vnorm_xz(vsub(to, from)));

        let v = &mut vehicles[self.vehicle_idx];
        v.update_ai(target_speed, target_yaw, city, &crate::config::Config::default(), dt);
        // Snap to lane position to prevent drift.
        v.pos.x = pos.x;
        v.pos.z = pos.z;
    }

    fn pick_next_lane(&mut self, city: &City) {
        let cur = &city.lanes[self.current_lane];
        let dest = cur.to;
        // Find lanes starting from our destination, prefer going straight.
        let mut candidates: Vec<usize> = Vec::new();
        for (i, l) in city.lanes.iter().enumerate() {
            if l.from == dest {
                candidates.push(i);
            }
        }
        if candidates.is_empty() {
            // U-turn: pick the reverse of current lane.
            for (i, l) in city.lanes.iter().enumerate() {
                if l.from == cur.to && l.to == cur.from {
                    self.current_lane = i;
                    self.lane_progress = 0.0;
                    return;
                }
            }
            return;
        }
        // Prefer straight (same axis).
        let straight: Vec<&usize> = candidates
            .iter()
            .filter(|&&i| city.lanes[i].axis == cur.axis)
            .collect();
        if !straight.is_empty() && rand::random::<f32>() < 0.6 {
            self.current_lane = *straight[rand::random::<usize>() % straight.len()];
        } else {
            self.current_lane = candidates[rand::random::<usize>() % candidates.len()];
        }
        self.lane_progress = 0.0;
    }
}

fn lane_offset(lane: &Lane, rw: f32) -> (f32, f32) {
    let offset = rw * 0.25;
    match lane.axis {
        Axis::X => (0.0, -offset * lane.dir as f32),
        Axis::Z => (offset * lane.dir as f32, 0.0),
    }
}

/// Spawn a traffic car on a random lane.
pub fn spawn_traffic(city: &City, vehicles: &mut Vec<Vehicle>, traffic: &mut Vec<TrafficCar>) {
    if city.lanes.is_empty() {
        return;
    }
    let lane_idx = rand::random::<usize>() % city.lanes.len();
    let lane = &city.lanes[lane_idx];
    let from = city.intersection(lane.from.0, lane.from.1);
    let to = city.intersection(lane.to.0, lane.to.1);
    let (cx, cz) = lane_offset(lane, city.road_width);
    let pos = Vector3 { x: from.x + cx, y: 0.0, z: from.z + cz };
    let yaw = yaw_from_dir(vnorm_xz(vsub(to, from)));
    let colors = [
        raylib::color::Color::new(200, 60, 60, 255),
        raylib::color::Color::new(60, 120, 200, 255),
        raylib::color::Color::new(220, 220, 220, 255),
        raylib::color::Color::new(80, 180, 100, 255),
        raylib::color::Color::new(200, 200, 60, 255),
        raylib::color::Color::new(40, 40, 40, 255),
    ];
    let color = colors[rand::random::<usize>() % colors.len()];
    let mut v = Vehicle::new(pos, yaw, color, VehicleKind::Civilian);
    v.is_traffic = true;
    let idx = vehicles.len();
    vehicles.push(v);
    traffic.push(TrafficCar::new(idx, lane_idx));
}
