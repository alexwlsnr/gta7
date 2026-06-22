//! Procedural grid city: blocks, buildings, roads, lane graph, traffic lights.
use crate::config::Config;
use crate::world::collision::AABB;
use crate::mathx::*;
use rand_chacha::ChaCha8Rng;
use rand::{Rng, SeedableRng};
use raylib::ffi::Vector3;

use crate::pickup::{Pickup, Shop, ShopKind};
use crate::player::Weapon;

/// A building is a collidable box plus visual params.
#[derive(Clone, Debug)]
pub struct Building {
    pub box3d: AABB,
    pub color_index: u32,
    pub floors: u32,
    pub has_windows: bool,
}

/// A directed lane on the road grid (graph edge between intersections).
#[derive(Clone, Copy, Debug)]
pub struct Lane {
    pub from: (i32, i32), // grid intersection coords
    pub to: (i32, i32),
    pub axis: Axis,       // X or Z travel
    pub dir: i32,         // +1 or -1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis { X, Z }

#[derive(Clone, Debug)]
pub struct TrafficLight {
    pub pos: Vector3,
    pub state: LightState,
    pub timer: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightState { Red, Green }

#[derive(Clone, Copy, Debug)]
pub struct Ramp {
    pub pos: Vector3,
    pub yaw: f32,
    pub width: f32,
    pub length: f32,
    pub height: f32,
}

pub struct City {
    pub blocks: usize,
    pub block_size: f32,
    pub road_width: f32,
    pub sidewalk_width: f32,
    pub buildings: Vec<Building>,
    /// Set of block coordinates (bi, bj) that are parks.
    pub parks: std::collections::HashSet<(i32, i32)>,
    pub lanes: Vec<Lane>,
    pub lights: Vec<TrafficLight>,
    pub ramps: Vec<Ramp>,
    pub generated_blocks: std::collections::HashSet<(i32, i32)>,
    pub ground_half: f32,
}

/// Helper function to create a deterministic RNG for a given seed and block coordinate.
fn get_block_rng(seed: u64, bi: i32, bj: i32) -> ChaCha8Rng {
    let mut h = seed;
    h = h.wrapping_add(bi as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    h = h ^ (h >> 30);
    h = h.wrapping_add(bj as u64).wrapping_mul(0x94d049bb133111eb);
    h = h ^ (h >> 27);
    h = h.wrapping_add(0x9e3779b97f4a7c15);
    ChaCha8Rng::seed_from_u64(h)
}

impl City {
    pub fn get_block_coords(&self, x: f32, z: f32) -> (i32, i32) {
        let origin = -self.ground_half;
        let bi = ((x - origin) / self.block_size).floor() as i32;
        let bj = ((z - origin) / self.block_size).floor() as i32;
        (bi, bj)
    }

    pub fn ensure_blocks_around(
        &mut self,
        pos: Vector3,
        radius: i32,
        cfg: &Config,
        shops: &mut Vec<Shop>,
        pickups: &mut Vec<Pickup>,
    ) {
        let (pi, pj) = self.get_block_coords(pos.x, pos.z);
        for i in (pi - radius)..=(pi + radius) {
            for j in (pj - radius)..=(pj + radius) {
                self.ensure_block_generated(i, j, cfg, shops, pickups);
            }
        }
    }

    pub fn ensure_block_generated(
        &mut self,
        bi: i32,
        bj: i32,
        cfg: &Config,
        shops: &mut Vec<Shop>,
        pickups: &mut Vec<Pickup>,
    ) {
        if !self.generated_blocks.insert((bi, bj)) {
            return;
        }

        let mut rng = get_block_rng(cfg.seed, bi, bj);
        let bs = self.block_size;
        let origin = -self.ground_half;
        let lot_half = cfg.lot_half();

        let block_center_x = origin + (bi as f32 + 0.5) * bs;
        let block_center_z = origin + (bj as f32 + 0.5) * bs;

        // Deciding if it is a park (12% chance)
        let is_park = rng.gen_bool(0.12);
        if is_park {
            self.parks.insert((bi, bj));
            // Let's spawn unique decorations or pickups in the park!
        } else {
            // Subdivide block into up to 4 lots.
            let subdivisions = rng.gen_range(1..=4);
            let (sx, sz) = match subdivisions {
                1 => (1, 1),
                2 => if rng.gen_bool(0.5) { (2, 1) } else { (1, 2) },
                _ => (2, 2),
            };
            let lot_w = (lot_half * 2.0) / sx as f32;
            let lot_d = (lot_half * 2.0) / sz as f32;
            for si in 0..sx {
                for sj in 0..sz {
                    let lot_cx = block_center_x - lot_half + (si as f32 + 0.5) * lot_w;
                    let lot_cz = block_center_z - lot_half + (sj as f32 + 0.5) * lot_d;

                    // Empty lot chance: 15% (allows spawning shops, pickups)
                    if rng.gen_bool(0.15) {
                        let feature_roll = rng.gen_range(0..10);
                        let pos = Vector3 { x: lot_cx, y: 0.0, z: lot_cz };
                        if feature_roll < 3 {
                            // Spawn armor, weapon, health or ammo shops
                            let kind = match rng.gen_range(0..4) {
                                0 => ShopKind::Weapon,
                                1 => ShopKind::Health,
                                2 => ShopKind::Armor,
                                _ => ShopKind::Ammo,
                            };
                            shops.push(Shop::new(pos, kind));
                        } else if feature_roll < 6 {
                            // Spawn interesting collectibles
                            let kind = rng.gen_range(0..4);
                            match kind {
                                0 => pickups.push(Pickup::health(pos)),
                                1 => pickups.push(Pickup::armor(pos)),
                                2 => pickups.push(Pickup::money(pos, rng.gen_range(150..600))),
                                _ => pickups.push(Pickup::weapon(pos, if rng.gen_bool(0.5) { Weapon::Smg } else { Weapon::Pistol })),
                            }
                        }
                        continue;
                    }

                    // Otherwise, spawn building
                    let inset = rng.gen_range(0.5..1.5);
                    let hx = (lot_w * 0.5) - inset;
                    let hz = (lot_d * 0.5) - inset;
                    if hx < 1.0 || hz < 1.0 { continue; }
                    let floors = rng.gen_range(2..=14);
                    let height = floors as f32 * 3.2;
                    let cy = height * 0.5;
                    let color_index = rng.gen_range(0..360);
                    self.buildings.push(Building {
                        box3d: AABB::from_center(lot_cx, cy, lot_cz, hx, height * 0.5, hz),
                        color_index,
                        floors,
                        has_windows: rng.gen_bool(0.8),
                    });
                }
            }
        }

        // Lanes associated with the corner intersection (bi, bj)
        self.lanes.push(Lane { from: (bi, bj), to: (bi + 1, bj), axis: Axis::X, dir: 1 });
        self.lanes.push(Lane { from: (bi + 1, bj), to: (bi, bj), axis: Axis::X, dir: -1 });
        self.lanes.push(Lane { from: (bi, bj), to: (bi, bj + 1), axis: Axis::Z, dir: 1 });
        self.lanes.push(Lane { from: (bi, bj + 1), to: (bi, bj), axis: Axis::Z, dir: -1 });

        // Traffic lights at intersections
        if rng.gen_bool(0.4) {
            let x = origin + bi as f32 * bs;
            let z = origin + bj as f32 * bs;
            let state = if rng.gen_bool(0.5) { LightState::Red } else { LightState::Green };
            self.lights.push(TrafficLight {
                pos: Vector3 { x, y: 4.5, z },
                state,
                timer: rng.gen_range(0.0..8.0),
            });
        }

        // Ramps on lane segments
        // Midpoint of X lane
        if rng.gen_bool(0.08) {
            let from_pos = Vector3 { x: origin + bi as f32 * bs, y: 0.0, z: origin + bj as f32 * bs };
            let to_pos = Vector3 { x: origin + (bi + 1) as f32 * bs, y: 0.0, z: origin + bj as f32 * bs };
            let mid = vscale(vadd(from_pos, to_pos), 0.5);
            let dir = vnorm_xz(vsub(to_pos, from_pos));
            let yaw = yaw_from_dir(dir);
            self.ramps.push(Ramp {
                pos: mid,
                yaw,
                width: 5.5,
                length: 12.0,
                height: 3.5,
            });
        }
        // Midpoint of Z lane
        if rng.gen_bool(0.08) {
            let from_pos = Vector3 { x: origin + bi as f32 * bs, y: 0.0, z: origin + bj as f32 * bs };
            let to_pos = Vector3 { x: origin + bi as f32 * bs, y: 0.0, z: origin + (bj + 1) as f32 * bs };
            let mid = vscale(vadd(from_pos, to_pos), 0.5);
            let dir = vnorm_xz(vsub(to_pos, from_pos));
            let yaw = yaw_from_dir(dir);
            self.ramps.push(Ramp {
                pos: mid,
                yaw,
                width: 5.5,
                length: 12.0,
                height: 3.5,
            });
        }
    }

    pub fn generate(cfg: &Config) -> Self {
        let n = cfg.city_blocks;
        let bs = cfg.block_size;
        let rw = cfg.road_width;
        let ground_half = bs * n as f32 * 0.5;

        let mut city = City {
            blocks: n,
            block_size: bs,
            road_width: rw,
            sidewalk_width: cfg.sidewalk_width,
            buildings: Vec::new(),
            parks: std::collections::HashSet::new(),
            lanes: Vec::new(),
            lights: Vec::new(),
            ramps: Vec::new(),
            generated_blocks: std::collections::HashSet::new(),
            ground_half,
        };

        // For backward compatibility and initial setup, generate the n x n block grid.
        let mut dummy_shops = Vec::new();
        let mut dummy_pickups = Vec::new();
        for bi in 0..n {
            for bj in 0..n {
                city.ensure_block_generated(bi as i32, bj as i32, cfg, &mut dummy_shops, &mut dummy_pickups);
            }
        }

        city
    }

    /// World position of an intersection (grid coords i,j in 0..=n).
    pub fn intersection(&self, i: i32, j: i32) -> Vector3 {
        let origin = -self.ground_half;
        Vector3 {
            x: origin + i as f32 * self.block_size,
            y: 0.0,
            z: origin + j as f32 * self.block_size,
        }
    }

    pub fn get_random_lane_near(&self, player_pos: Vector3, min_dist: f32, max_dist: f32) -> Option<usize> {
        let mut candidates = Vec::new();
        for (idx, lane) in self.lanes.iter().enumerate() {
            let from_pos = self.intersection(lane.from.0, lane.from.1);
            let dist = vdist_xz(from_pos, player_pos);
            if dist >= min_dist && dist <= max_dist {
                candidates.push(idx);
            }
        }
        if candidates.is_empty() {
            None
        } else {
            Some(candidates[rand::random::<usize>() % candidates.len()])
        }
    }

    /// Resolve a circle (XZ) against all nearby buildings. Returns total push.
    pub fn resolve_circle(&self, mut px: f32, mut pz: f32, radius: f32) -> Vector3 {
        let mut push = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        for b in &self.buildings {
            // Broad phase: skip buildings far away.
            let dx = (b.box3d.center().x - px).abs();
            let dz = (b.box3d.center().z - pz).abs();
            let h = b.box3d.half();
            if dx > h.x + radius + 2.0 || dz > h.z + radius + 2.0 {
                continue;
            }
            let p = crate::world::collision::circle_vs_aabb(px, pz, radius, b.box3d);
            push.x += p.x;
            push.z += p.z;
            // Apply incrementally so stacked pushes don't overcorrect.
            px += p.x;
            pz += p.z;
        }
        push
    }

    /// Resolve a circle (XZ) against all nearby buildings in 3D.
    /// If the entity's Y is above the building roof, horizontal collision is ignored (allowing rooftop driving/climbing).
    pub fn resolve_circle_3d(&self, mut px: f32, py: f32, mut pz: f32, radius: f32) -> Vector3 {
        let mut push = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        for b in &self.buildings {
            // If the entity's Y is above or at the building roof level, ignore horizontal collision.
            if py >= b.box3d.max.y - 0.2 {
                continue;
            }
            // Broad phase: skip buildings far away.
            let dx = (b.box3d.center().x - px).abs();
            let dz = (b.box3d.center().z - pz).abs();
            let h = b.box3d.half();
            if dx > h.x + radius + 2.0 || dz > h.z + radius + 2.0 {
                continue;
            }
            let p = crate::world::collision::circle_vs_aabb(px, pz, radius, b.box3d);
            push.x += p.x;
            push.z += p.z;
            px += p.x;
            pz += p.z;
        }
        push
    }

    /// Checks if a position is on any ramp, returning the ground height at that point
    /// and the ramp inclination angle.
    pub fn get_ramp_height_and_angle(&self, pos: Vector3) -> Option<(f32, f32)> {
        for r in &self.ramps {
            // Convert pos to ramp local space
            let rel = vsub(pos, r.pos);
            let (sin, cos) = r.yaw.sin_cos();
            // Rotate back into local coords
            let local_x = rel.x * cos - rel.z * sin;
            let local_z = rel.z * cos + rel.x * sin;
            
            if local_x.abs() <= r.width * 0.5 && local_z >= -r.length * 0.5 && local_z <= r.length * 0.5 {
                let t = (local_z + r.length * 0.5) / r.length;
                let height = t * r.height;
                let angle = (r.height / r.length).atan();
                return Some((height, angle));
            }
        }
        None
    }

    /// Get the ground/solid surface height at a world position, taking into account flat roads,
    /// ramp slopes, and building roofs.
    pub fn get_ground_height(&self, pos: Vector3) -> f32 {
        // 1. Check ramps
        if let Some((h, _)) = self.get_ramp_height_and_angle(pos) {
            return h;
        }
        
        // 2. Check buildings (roofs)
        let mut highest_roof = 0.0;
        for b in &self.buildings {
            // Broad phase: skip buildings whose roof is well below the entity
            // (can't stand on it) or that are far away in XZ.
            let roof_y = b.box3d.max.y;
            if pos.y < roof_y - 0.5 {
                continue;
            }
            let dx = (b.box3d.center().x - pos.x).abs();
            let dz = (b.box3d.center().z - pos.z).abs();
            let h = b.box3d.half();
            if dx > h.x + 2.0 || dz > h.z + 2.0 {
                continue;
            }
            if pos.x >= b.box3d.min.x && pos.x <= b.box3d.max.x
               && pos.z >= b.box3d.min.z && pos.z <= b.box3d.max.z {
                // Entity is above or near this roof (guaranteed by broad phase).
                if roof_y > highest_roof {
                    highest_roof = roof_y;
                }
            }
        }
        if highest_roof > 0.0 {
            return highest_roof;
        }

        0.0
    }

    /// Find nearest intersection coords to a world position.
    pub fn nearest_intersection(&self, x: f32, z: f32) -> (i32, i32) {
        let origin = -self.ground_half;
        let i = ((x - origin) / self.block_size).round() as i32;
        let j = ((z - origin) / self.block_size).round() as i32;
        (i, j)
    }

    /// Is a world position on a road (between blocks)?
    pub fn on_road(&self, x: f32, z: f32) -> bool {
        let origin = -self.ground_half;
        let local_x = x - origin;
        let local_z = z - origin;
        let in_block_x = (local_x % self.block_size).abs();
        let in_block_z = (local_z % self.block_size).abs();
        let rw = self.road_width;
        // Within road width band of any grid line.
        let near_x_line = (in_block_x - self.block_size).abs() < rw * 0.5 || in_block_x < rw * 0.5;
        let near_z_line = (in_block_z - self.block_size).abs() < rw * 0.5 || in_block_z < rw * 0.5;
        near_x_line || near_z_line
    }

    /// Is a world position on a sidewalk strip (between road edge and building lot)?
    pub fn on_sidewalk(&self, x: f32, z: f32) -> bool {
        let origin = -self.ground_half;
        let lx = (x - origin).rem_euclid(self.block_size);
        let lz = (z - origin).rem_euclid(self.block_size);
        let dx = lx.min(self.block_size - lx);
        let dz = lz.min(self.block_size - lz);
        let r = self.road_width * 0.5;
        let sw = self.sidewalk_width;
        // On sidewalk if within the sidewalk band of a grid line in one axis
        // and not inside a road in the other.
        let on_sw_x = dx >= r && dx <= r + sw;
        let on_sw_z = dz >= r && dz <= r + sw;
        on_sw_x || on_sw_z
    }

    /// Snap a position to the nearest sidewalk center point.
    /// Returns the sidewalk position and the direction the sidewalk runs (0 = along X, 1 = along Z).
    pub fn nearest_sidewalk(&self, x: f32, z: f32) -> (Vector3, i32) {
        let origin = -self.ground_half;
        let lx = (x - origin).rem_euclid(self.block_size);
        let lz = (z - origin).rem_euclid(self.block_size);
        // Distance to nearest grid line in each axis
        let dx_low = lx;
        let dx_high = self.block_size - lx;
        let dz_low = lz;
        let dz_high = self.block_size - lz;
        let r = self.road_width * 0.5;
        let sw_off = r + self.sidewalk_width * 0.5;

        // Find closest sidewalk: compare nearest X-line sidewalk vs Z-line sidewalk
        let dx_nearest = dx_low.min(dx_high);
        let dz_nearest = dz_low.min(dz_high);

        if dx_nearest <= dz_nearest {
            // Snap to sidewalk along an X grid line (sidewalk runs in X direction)
            let grid_i = ((x - origin) / self.block_size).round() as i32;
            let line = origin + grid_i as f32 * self.block_size;
            let side = if z >= line { sw_off } else { -sw_off };
            (Vector3 { x, y: 0.0, z: line + side }, 0)
        } else {
            // Snap to sidewalk along a Z grid line (sidewalk runs in Z direction)
            let grid_j = ((z - origin) / self.block_size).round() as i32;
            let line = origin + grid_j as f32 * self.block_size;
            let side = if x >= line { sw_off } else { -sw_off };
            (Vector3 { x: line + side, y: 0.0, z }, 1)
        }
    }

    pub fn step_lights(&mut self, dt: f32) {
        for l in &mut self.lights {
            l.timer -= dt;
            if l.timer <= 0.0 {
                l.state = match l.state {
                    LightState::Red => LightState::Green,
                    LightState::Green => LightState::Red,
                };
                l.timer = 8.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn procgen_deterministic() {
        let cfg = Config { seed: 42, city_blocks: 6, ..Config::default() };
        let a = City::generate(&cfg);
        let b = City::generate(&cfg);
        assert_eq!(a.buildings.len(), b.buildings.len());
        for (x, y) in a.buildings.iter().zip(b.buildings.iter()) {
            assert_eq!(x.box3d.min.x, y.box3d.min.x);
            assert_eq!(x.color_index, y.color_index);
            assert_eq!(x.floors, y.floors);
        }
    }
    #[test]
    fn different_seeds_differ() {
        let cfg1 = Config { seed: 1, ..Config::default() };
        let cfg2 = Config { seed: 2, ..Config::default() };
        let a = City::generate(&cfg1);
        let b = City::generate(&cfg2);
        // Almost certainly different.
        assert!(a.buildings.len() != b.buildings.len()
            || a.buildings.first().map(|b| b.color_index) != b.buildings.first().map(|b| b.color_index));
    }

    #[test]
    fn sidewalk_detection() {
        let cfg = Config { seed: 42, city_blocks: 6, ..Config::default() };
        let city = City::generate(&cfg);
        let r = cfg.road_width * 0.5;
        let sw = cfg.sidewalk_width;
        // Center of a road (on a grid line) — not sidewalk.
        assert!(!city.on_sidewalk(0.0, 0.0));
        // In the sidewalk band (between road edge and lot edge).
        let sw_mid = r + sw * 0.5;
        assert!(city.on_sidewalk(0.0, sw_mid));
        assert!(city.on_sidewalk(sw_mid, 0.0 + r + 0.1));
        // Deep inside a block — not sidewalk.
        let lot_mid = cfg.block_size * 0.5;
        assert!(!city.on_sidewalk(lot_mid, lot_mid));
    }

    #[test]
    fn nearest_sidewalk_snaps() {
        let cfg = Config { seed: 42, city_blocks: 6, ..Config::default() };
        let city = City::generate(&cfg);
        let sw_off = cfg.road_width * 0.5 + cfg.sidewalk_width * 0.5;
        // A point near the center should snap to a sidewalk.
        let (pos, axis) = city.nearest_sidewalk(3.0, 3.0);
        // The snapped position should be on a sidewalk.
        assert!(city.on_sidewalk(pos.x, pos.z), "snapped pos {:?} not on sidewalk", pos);
        // Axis should be 0 or 1.
        assert!(axis == 0 || axis == 1);
        // The cross-axis should be at grid_line ± sw_off.
        let origin = -city.ground_half;
        if axis == 0 {
            let line = origin + ((pos.z - origin) / city.block_size).round() * city.block_size;
            assert!((pos.z - line).abs() - sw_off < 0.1);
        } else {
            let line = origin + ((pos.x - origin) / city.block_size).round() * city.block_size;
            assert!((pos.x - line).abs() - sw_off < 0.1);
        }
    }
}
