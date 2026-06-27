//! Named scene presets for the test harness. Each preset mutates a `Game`
//! into a deterministic state for screenshot/inspection.
use crate::ai::traffic::spawn_traffic;
use crate::camera::Mode;
use crate::cli_args::Args;
use crate::game::Game;
use crate::vehicle::{Vehicle, VehicleKind, VehicleVariant};
use raylib::color::Color;
use raylib::ffi::Vector3;

pub const SCENES: &[(&str, fn(&mut Game, &Args))] = &[
    ("headlight_closeup", scene_headlight_closeup),
    ("night_street",       scene_night_street),
    ("dawn_drive",         scene_dawn_drive),
    ("parking_lot",        scene_parking_lot),
];

/// Apply a named scene. Unknown names print a helpful list and pick
/// `headlight_closeup` as a safe default.
pub fn apply_scene(game: &mut Game, args: &Args) {
    if let Some((_, f)) = SCENES.iter().find(|(name, _)| *name == args.scene) {
        f(game, args);
        return;
    }
    eprintln!("Unknown scene `{}`. Available:", args.scene);
    for (name, _) in SCENES { eprintln!("  {name}"); }
    let (_, f) = SCENES[0];
    f(game, args);
}

fn vehicle_with_variant(
    pos: Vector3, yaw: f32, color: Color, kind: VehicleKind, variant: VehicleVariant,
) -> Vehicle {
    let mut v = Vehicle::new(pos, yaw, color, kind);
    v.variant = variant;
    v
}

/// Place player on foot at origin, two cars 6m apart along +X pointed +Z,
/// camera 3m behind & at eye level looking down +Z.
fn scene_headlight_closeup(game: &mut Game, _args: &Args) {
    if game.args.as_ref().map_or(true, |a| a.time.is_none()) {
        game.set_time(22.0);
    }
    game.player.in_vehicle = None;
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    game.player.yaw = 0.0;
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: -3.0, y: 0.0, z: 4.0 }, 0.0,
        Color::new(60, 120, 200, 255), VehicleKind::Civilian, VehicleVariant::Sedan,
    ));
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: 3.0, y: 0.0, z: 4.0 }, 0.0,
        Color::new(200, 60, 60, 255), VehicleKind::Civilian, VehicleVariant::Sports,
    ));
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_free(
            Vector3 { x: 0.0, y: 1.5, z: -3.0 }, 0.0, 0.0,
        );
    }
}

fn scene_night_street(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(21.0); }
    game.player.in_vehicle = Some(0);
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    game.vehicles.push(vehicle_with_variant(
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }, 0.0,
        Color::new(255, 20, 147, 255), VehicleKind::Civilian, VehicleVariant::Sedan,
    ));
    for _ in 0..args.cars.max(4) {
        spawn_traffic(&game.city, &mut game.vehicles, &mut game.traffic);
    }
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_follow();
    }
}

fn scene_dawn_drive(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(6.5); }
    // Reuse night_street setup but earlier time.
    scene_night_street(game, args);
}

fn scene_parking_lot(game: &mut Game, args: &Args) {
    if args.time.is_none() { game.set_time(19.5); }
    game.player.in_vehicle = None;
    game.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    let layout = [
        (-9.0, 0.0, 6.0, 0.0,  VehicleVariant::Sedan,  Color::new(220, 60, 60, 255)),
        (-3.0, 0.0, 6.0, 0.0,  VehicleVariant::Sports, Color::new(255, 110, 0, 255)),
        ( 3.0, 0.0, 6.0, 0.0,  VehicleVariant::SUV,    Color::new(60, 180, 220, 255)),
        ( 9.0, 0.0, 6.0, 0.0,  VehicleVariant::Pickup, Color::new(80, 220, 80, 255)),
        ( 0.0, 0.0,-6.0, std::f32::consts::PI, VehicleVariant::Sedan,  Color::new(160, 60, 220, 255)),
        ( 6.0, 0.0,-6.0, std::f32::consts::PI, VehicleVariant::Sports, Color::new(220, 220, 80, 255)),
    ];
    for (x, _y, z, yaw, variant, color) in layout {
        game.vehicles.push(vehicle_with_variant(
            Vector3 { x, y: 0.0, z }, yaw, color, VehicleKind::Civilian, variant,
        ));
    }
    if !matches!(game.camera.mode, Mode::Free) {
        game.camera.set_free(
            Vector3 { x: 0.0, y: 8.0, z: 12.0 }, -std::f32::consts::FRAC_PI_2, 0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four presets the spec ships. Locking this list guards against
    /// accidental renames/deletions.
    #[test]
    fn scenes_table_lists_documented_presets() {
        let names: Vec<&str> = SCENES.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec!["headlight_closeup", "night_street", "dawn_drive", "parking_lot"],
        );
    }

    /// `apply_scene` falls back to SCENES[0] on unknown names. The fallback
    /// must always be the deterministic safe default (headlight_closeup) so
    /// typo'd --scene values don't land in a half-applied state.
    #[test]
    fn fallback_target_is_headlight_closeup() {
        assert_eq!(SCENES[0].0, "headlight_closeup");
    }

    /// Every entry has a non-null fn pointer. (The compiler already enforces
    /// this for SCENES as a `const` table; the test documents the contract.)
    #[test]
    fn every_scene_has_an_applicator() {
        for (name, f) in SCENES {
            assert!(!name.is_empty(), "empty scene name");
            assert!(!(*f as *const ()).is_null(), "null fn for {name}");
        }
    }

    // Note: apply_scene() takes &mut Game, which requires a live
    // RaylibHandle to construct. The fallback path is covered by the
    // screenshot-mode integration smoke (see main.rs --screenshot). This
    // module guards the public SCENES table; per-preset functions are
    // exercised through the screenshot smoke and the F5 cycle hotkey.
}
