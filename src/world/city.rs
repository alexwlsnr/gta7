//! Procedural grid city: blocks, buildings, roads, lane graph, traffic lights.
use crate::config::Config;
use crate::world::collision::AABB;
use rand_chacha::ChaCha8Rng;
use rand::{Rng, SeedableRng};
use raylib::ffi::Vector3;

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

pub struct City {
    pub blocks: usize,
    pub block_size: f32,
    pub road_width: f32,
    pub buildings: Vec<Building>,
    /// Flattened grid: which lots are parks.
    pub parks: Vec<bool>,
    pub lanes: Vec<Lane>,
    pub lights: Vec<TrafficLight>,
    pub ground_half: f32,
}

impl City {
    pub fn generate(cfg: &Config) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);
        let n = cfg.city_blocks;
        let bs = cfg.block_size;
        let rw = cfg.road_width;
        let origin = -(bs * n as f32) * 0.5;
        let lot_half = cfg.lot_half();

        let mut buildings = Vec::new();
        let mut parks = vec![false; n * n];

        for bi in 0..n {
            for bj in 0..n {
                let block_center_x = origin + (bi as f32 + 0.5) * bs;
                let block_center_z = origin + (bj as f32 + 0.5) * bs;
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
                        // Park chance
                        if rng.gen_bool(0.12) {
                            parks[bi * n + bj] = true;
                            continue;
                        }
                        // Empty lot chance
                        if rng.gen_bool(0.08) {
                            continue;
                        }
                        // Building footprint inset within the lot.
                        let inset = rng.gen_range(0.5..1.5);
                        let hx = (lot_w * 0.5) - inset;
                        let hz = (lot_d * 0.5) - inset;
                        if hx < 1.0 || hz < 1.0 { continue; }
                        let floors = rng.gen_range(2..=14);
                        let height = floors as f32 * 3.2;
                        let cy = height * 0.5;
                        let color_index = rng.gen_range(0..360);
                        buildings.push(Building {
                            box3d: AABB::from_center(lot_cx, cy, lot_cz, hx, height * 0.5, hz),
                            color_index,
                            floors,
                            has_windows: rng.gen_bool(0.8),
                        });
                    }
                }
            }
        }

        // Lane graph: directed lanes along each road between intersections.
        // Intersections are at (bi*bs, bj*bs) + origin, for bi,bj in 0..=n.
        let mut lanes = Vec::new();
        for bi in 0..=n {
            for bj in 0..=n {
                // Horizontal lanes (along X)
                if bi < n {
                    // forward (+X) at +Z offset of road
                    lanes.push(Lane { from: (bi as i32, bj as i32), to: ((bi+1) as i32, bj as i32), axis: Axis::X, dir: 1 });
                    lanes.push(Lane { from: ((bi+1) as i32, bj as i32), to: (bi as i32, bj as i32), axis: Axis::X, dir: -1 });
                }
                // Vertical lanes (along Z)
                if bj < n {
                    lanes.push(Lane { from: (bi as i32, bj as i32), to: (bi as i32, (bj+1) as i32), axis: Axis::Z, dir: 1 });
                    lanes.push(Lane { from: (bi as i32, (bj+1) as i32), to: (bi as i32, bj as i32), axis: Axis::Z, dir: -1 });
                }
            }
        }

        // Traffic lights at internal intersections (not on border).
        let mut lights = Vec::new();
        for bi in 1..n {
            for bj in 1..n {
                let x = origin + bi as f32 * bs;
                let z = origin + bj as f32 * bs;
                let state = if rng.gen_bool(0.5) { LightState::Red } else { LightState::Green };
                lights.push(TrafficLight {
                    pos: Vector3 { x, y: 4.5, z },
                    state,
                    timer: rng.gen_range(0.0..8.0),
                });
            }
        }

        City {
            blocks: n,
            block_size: bs,
            road_width: rw,
            buildings,
            parks,
            lanes,
            lights,
            ground_half: bs * n as f32 * 0.5,
        }
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
}
