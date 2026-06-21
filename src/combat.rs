//! Combat: hitscan shooting, melee, damage application.
use raylib::ffi::Vector3;
use crate::mathx::*;
use crate::player::Player;
use crate::vehicle::Vehicle;
use crate::world::collision::{ray_vs_aabb, AABB};
use crate::world::city::City;
use crate::render::fx::Fx;
use crate::ai::ped::Ped;
use crate::ai::cop::Cop;

/// Result of a weapon shot: who/what was hit.
#[derive(Debug)]
pub struct HitResult {
    pub distance: f32,
    pub point: Vector3,
    pub kind: HitKind,
}

#[derive(Debug, Clone, Copy)]
pub enum HitKind {
    Building,
    Ped(usize),
    Cop(usize),
    Vehicle(usize),
    Miss,
}

/// Fire the player's weapon. Returns the hit result for game logic (wanted, money).
pub fn fire_weapon(
    player: &Player,
    cam_pos: Vector3,
    cam_fwd: Vector3,
    city: &City,
    peds: &mut [Ped],
    cops: &mut [Cop],
    vehicles: &mut [Vehicle],
    fx: &mut Fx,
) -> HitResult {
    let weapon = player.weapon;
    let range = weapon.range();
    let spread = weapon.spread();

    // Apply spread to direction.
    let mut dir = cam_fwd;
    if spread > 0.0 {
        dir.x += (rand::random::<f32>() - 0.5) * spread;
        dir.y += (rand::random::<f32>() - 0.5) * spread;
        dir.z += (rand::random::<f32>() - 0.5) * spread;
        dir = vnorm(dir);
    }

    let muzzle = vadd(cam_pos, vscale(dir, 0.5));
    // The visual origin of the shot is from the player's gun position.
    let shoot_origin = if player.in_vehicle.is_some() {
        vadd(player.pos, Vector3 { x: 0.0, y: 0.8, z: 0.0 })
    } else {
        vadd(player.pos, Vector3 { x: 0.0, y: 1.2, z: 0.0 })
    };
    let flash_pos = vadd(shoot_origin, vscale(dir, 0.6));
    fx.muzzle(flash_pos);

    // Find closest hit among all targets.
    let mut best: Option<(f32, HitKind)> = None;

    // Buildings (block shots).
    for b in &city.buildings {
        if let Some((t, _n)) = ray_vs_aabb(muzzle, dir, b.box3d, range) {
            if best.map_or(true, |(bt, _)| t < bt) {
                best = Some((t, HitKind::Building));
            }
        }
    }

    // Peds.
    for (i, ped) in peds.iter().enumerate() {
        if ped.dead() {
            continue;
        }
        let box3d = ped_aabb(ped.pos);
        if let Some((t, _n)) = ray_vs_aabb(muzzle, dir, box3d, range) {
            if best.map_or(true, |(bt, _)| t < bt) {
                best = Some((t, HitKind::Ped(i)));
            }
        }
    }

    // Cops.
    for (i, cop) in cops.iter().enumerate() {
        if cop.dead() {
            continue;
        }
        let box3d = cop_aabb(cop.pos);
        if let Some((t, _n)) = ray_vs_aabb(muzzle, dir, box3d, range) {
            if best.map_or(true, |(bt, _)| t < bt) {
                best = Some((t, HitKind::Cop(i)));
            }
        }
    }

    // Vehicles.
    for (i, v) in vehicles.iter().enumerate() {
        if v.destroyed {
            continue;
        }
        let box3d = vehicle_aabb(v.pos, v.yaw);
        if let Some((t, _n)) = ray_vs_aabb(muzzle, dir, box3d, range) {
            if best.map_or(true, |(bt, _)| t < bt) {
                best = Some((t, HitKind::Vehicle(i)));
            }
        }
    }

    let (dist, kind) = best.unwrap_or((range, HitKind::Miss));
    let end = vadd(muzzle, vscale(dir, dist.min(range)));
    // Draw the tracer from the player's gun to the hit point
    let shoot_origin = if player.in_vehicle.is_some() {
        vadd(player.pos, Vector3 { x: 0.0, y: 0.8, z: 0.0 })
    } else {
        vadd(player.pos, Vector3 { x: 0.0, y: 1.2, z: 0.0 })
    };
    fx.tracer(vadd(shoot_origin, vscale(dir, 0.6)), end);

    // Apply damage.
    match kind {
        HitKind::Ped(i) => {
            peds[i].take_damage(weapon.damage());
            fx.blood(end);
        }
        HitKind::Cop(i) => {
            cops[i].take_damage(weapon.damage());
            fx.blood(end);
        }
        HitKind::Vehicle(i) => {
            vehicles[i].take_damage(weapon.damage() * 0.8);
            // Spark effect.
            fx.burst(end, 6, 4.0, raylib::color::Color::new(255, 200, 80, 255), 0.3, 5.0);
        }
        _ => {}
    }

    HitResult { distance: dist, point: end, kind }
}

/// Melee attack: short-range knockback + small damage to nearby NPCs.
pub fn melee_attack(
    player: &Player,
    peds: &mut [Ped],
    cops: &mut [Cop],
    vehicles: &mut [Vehicle],
    fx: &mut Fx,
) {
    let fwd = dir_from_yaw(player.yaw);
    let reach = 2.0;
    let hit_pos = vadd(player.pos, vscale(fwd, reach));
    for ped in peds.iter_mut() {
        if ped.dead() {
            continue;
        }
        if vdist_xz(ped.pos, hit_pos) < 1.5 {
            ped.take_damage(15.0);
            fx.blood(ped.pos);
        }
    }
    for cop in cops.iter_mut() {
        if cop.dead() {
            continue;
        }
        if vdist_xz(cop.pos, hit_pos) < 1.5 {
            cop.take_damage(15.0);
            fx.blood(cop.pos);
        }
    }
    for v in vehicles.iter_mut() {
        if v.destroyed {
            continue;
        }
        if vdist_xz(v.pos, hit_pos) < 2.5 {
            v.take_damage(5.0);
            fx.burst(hit_pos, 3, 3.0, raylib::color::Color::new(255, 200, 80, 255), 0.2, 5.0);
        }
    }
}

/// Cop shoots at player (hitscan with accuracy based on distance).
/// Returns true if the shot connects (damage applied by game orchestrator).
pub fn cop_fire(
    cop_pos: Vector3,
    player_pos: Vector3,
    fx: &mut Fx,
) -> bool {
    let dist = vdist_xz(cop_pos, player_pos);
    let hit_chance = (0.5 - dist * 0.005).max(0.05);
    let dir = vnorm(vsub(vadd(player_pos, Vector3 { x: 0.0, y: 1.0, z: 0.0 }), cop_pos));
    let muzzle = vadd(cop_pos, vscale(dir, 0.5));
    fx.muzzle(muzzle);
    fx.tracer(muzzle, vadd(muzzle, vscale(dir, dist)));
    rand::random::<f32>() < hit_chance
}

fn ped_aabb(pos: Vector3) -> AABB {
    AABB::from_center(pos.x, pos.y + 0.9, pos.z, 0.4, 0.9, 0.4)
}
fn cop_aabb(pos: Vector3) -> AABB {
    AABB::from_center(pos.x, pos.y + 0.9, pos.z, 0.4, 0.9, 0.4)
}
fn vehicle_aabb(pos: Vector3, _yaw: f32) -> AABB {
    AABB::from_center(pos.x, pos.y + 0.4, pos.z, 1.2, 0.8, 2.5)
}
