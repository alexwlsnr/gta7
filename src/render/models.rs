//! Procedural textures and cached models/meshes. All assets generated in code.
use raylib::prelude::*;
use raylib::ffi::Vector3;

use crate::config::Config;
use crate::world::city::{Building, City, Axis};

/// Cached GPU assets built once at startup.
pub struct Assets {
    pub road_tex: Texture2D,
    pub window_tex: Texture2D,
    pub ground_tex: Texture2D,
    pub sky_top: Color,
    pub sky_bottom: Color,
}

impl Assets {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: &Config) -> Self {
        let p = cfg.palette();
        // Road texture: dark asphalt with a dashed center line.
        let mut road = Image::gen_image_color(64, 64, p.road());
        // Center dashed yellow line.
        let yellow = Color::new(210, 180, 60, 255);
        for y in (0..64).step_by(16) {
            for x in 28..36 {
                road.draw_pixel(x, y, yellow);
                road.draw_pixel(x, y + 1, yellow);
                road.draw_pixel(x, y + 2, yellow);
                road.draw_pixel(x, y + 3, yellow);
            }
        }
        let road_tex = rl.load_texture_from_image(thread, &road).unwrap();

        // Window texture: building facade grid of windows.
        let mut win = Image::gen_image_color(64, 64, Color::new(40, 50, 70, 255));
        let lit = Color::new(255, 230, 150, 255);
        let dark = Color::new(20, 25, 40, 255);
        for by in 0..8 {
            for bx in 0..8 {
                let x0 = bx * 8 + 2;
                let y0 = by * 8 + 2;
                let col = if ((bx + by) % 3 == 0) && (bx % 2 == 0) { lit } else { dark };
                for yy in 0..4 {
                    for xx in 0..4 {
                        win.draw_pixel(x0 + xx, y0 + yy, col);
                    }
                }
            }
        }
        let window_tex = rl.load_texture_from_image(thread, &win).unwrap();

        // Ground texture: mottled grey-green for non-road ground (sidewalk/grass blend).
        let mut ground = Image::gen_image_color(128, 128, p.sidewalk());
        let grass = p.grass();
        for _ in 0..400 {
            let x = (rand::random::<u32>() % 128) as i32;
            let y = (rand::random::<u32>() % 128) as i32;
            ground.draw_pixel(x, y, grass);
        }
        let ground_tex = rl.load_texture_from_image(thread, &ground).unwrap();

        Assets {
            road_tex,
            window_tex,
            ground_tex,
            sky_top: p.sky_top(),
            sky_bottom: p.sky_bottom(),
        }
    }
}

/// Draw the ground plane + roads + sidewalks + parks.
pub fn draw_world(d3: &mut impl RaylibDraw3D, city: &City, assets: &Assets, cfg: &Config) {
    let half = city.ground_half;
    let p = cfg.palette();

    // Ground base plane (sidewalk-ish).
    d3.draw_plane(
        Vector3 { x: 0.0, y: 0.02, z: 0.0 },
        Vector2::new(half * 2.0, half * 2.0),
        p.sidewalk(),
    );

    let n = city.blocks;
    let bs = city.block_size;
    let rw = city.road_width;
    let origin = -half;

    // Roads: strips along grid lines.
    let road_col = p.road();
    for i in 0..=n {
        let line = origin + i as f32 * bs;
        // Horizontal road (runs along X) centered on z=line
        d3.draw_plane(
            Vector3 { x: 0.0, y: 0.03, z: line },
            Vector2::new(half * 2.0, rw),
            road_col,
        );
        // Vertical road (runs along Z) centered on x=line
        d3.draw_plane(
            Vector3 { x: line, y: 0.03, z: 0.0 },
            Vector2::new(rw, half * 2.0),
            road_col,
        );
    }

    // Lane center dashes (yellow) — simplified: one dash per lane segment.
    let yellow = Color::new(220, 190, 70, 255);
    for lane in &city.lanes {
        let a = city.intersection(lane.from.0, lane.from.1);
        let b = city.intersection(lane.to.0, lane.to.1);
        // Offset to the right side of travel for lane center.
        let (cx, cz) = lane_center(a, b, lane, rw);
        let mid = Vector3 { x: (a.x + b.x) * 0.5 + cx, y: 0.05, z: (a.z + b.z) * 0.5 + cz };
        d3.draw_plane(mid, Vector2::new(2.0, 0.3), yellow);
    }

    // Parks (green blocks) and sidewalks already covered by ground; draw grass on park lots.
    for bi in 0..n {
        for bj in 0..n {
            if !city.parks[bi * n + bj] { continue; }
            let cx = origin + (bi as f32 + 0.5) * bs;
            let cz = origin + (bj as f32 + 0.5) * bs;
            let lh = cfg.lot_half();
            d3.draw_plane(
                Vector3 { x: cx, y: 0.04, z: cz },
                Vector2::new(lh * 2.0, lh * 2.0),
                p.grass(),
            );
            // A couple of "trees": green spheres on brown trunks.
            for k in 0..3 {
                let tx = cx + (k as f32 - 1.0) * 6.0;
                let tz = cz + 2.0;
                d3.draw_cylinder(Vector3 { x: tx, y: 1.0, z: tz }, 0.3, 0.3, 2.0, 6, Color::new(90, 60, 40, 255));
                d3.draw_sphere(Vector3 { x: tx, y: 2.6, z: tz }, 1.2, Color::new(40, 120, 50, 255));
            }
        }
    }

    // Buildings.
    for b in &city.buildings {
        draw_building(d3, b, assets, &p);
    }
}

fn lane_center(_a: Vector3, _b: Vector3, lane: &crate::world::city::Lane, rw: f32) -> (f32, f32) {
    // Right-hand traffic: offset lane center to the right of travel direction.
    let offset = rw * 0.25;
    match lane.axis {
        Axis::X => (0.0, -offset * lane.dir as f32),
        Axis::Z => (offset * lane.dir as f32, 0.0),
    }
}

fn draw_building(d3: &mut impl RaylibDraw3D, b: &Building, assets: &Assets, p: &crate::config::Palette) {
    let c = b.box3d.center();
    let h = b.box3d.half();
    let w = h.x * 2.0;
    let hgt = h.y * 2.0;
    let l = h.z * 2.0;
    let body = p.building(b.color_index);
    // Body box.
    d3.draw_cube(c, w, hgt, l, body);
    // Window facade: a slightly inset bluish box to suggest lit windows.
    if b.has_windows {
        d3.draw_cube(
            Vector3 { x: c.x, y: c.y, z: c.z },
            w * 0.92, hgt * 0.96, l * 0.92,
            Color::new(70, 90, 130, 255),
        );
    }
    // Roof cap slightly darker.
    let top = Vector3 { x: c.x, y: c.y + h.y + 0.1, z: c.z };
    d3.draw_cube(top, w * 0.9, 0.4, l * 0.9, p.building_top(b.color_index));
    // Edge wires for definition.
    d3.draw_cube_wires(c, w, hgt, l, Color::new(15, 15, 20, 255));
}

/// Draw a car body at a position with a yaw (radians) and a color.
pub fn draw_car(d3: &mut impl RaylibDraw3D, pos: Vector3, yaw: f32, color: Color, damaged: f32) {
    let body_w = 2.0;
    let body_h = 0.8;
    let body_l = 4.2;
    // Body box.
    d3.draw_cube(pos, body_w, body_h, body_l, color);
    d3.draw_cube_wires(pos, body_w, body_h, body_l, Color::new(20, 20, 20, 255));
    // Cabin.
    let cabin = Vector3 { x: pos.x, y: pos.y + 0.7, z: pos.z - 0.2 };
    d3.draw_cube(cabin, 1.6, 0.6, 2.0, Color::new(60, 80, 110, 255));
    // Wheels (4 cylinders), oriented along the car's X axis.
    let wheel_offs = [
        ( body_w * 0.5,  body_l * 0.32),
        (-body_w * 0.5,  body_l * 0.32),
        ( body_w * 0.5, -body_l * 0.32),
        (-body_w * 0.5, -body_l * 0.32),
    ];
    let (sx, sz) = (yaw.sin(), yaw.cos());
    for (ox, oz) in wheel_offs {
        let wx = pos.x + ox * sz + oz * sx; // rotate local offset by yaw
        let wz = pos.z - ox * sx + oz * sz;
        d3.draw_cylinder(
            Vector3 { x: wx, y: pos.y - 0.4, z: wz },
            0.4, 0.4, 0.3, 10,
            Color::new(25, 25, 25, 255),
        );
    }
    // Headlights + taillights (small cubes) for life.
    let (fx, fz) = (pos.x + sz * body_l * 0.5, pos.z + sz * 0.0 + sz * body_l * 0.5);
    let _ = (fx, fz);
    // Damage smoke tint: darken with damage.
    if damaged > 0.4 {
        let dark = Color::new(60, 40, 30, 255);
        d3.draw_cube_wires(pos, body_w + 0.05, body_h + 0.05, body_l + 0.05, dark);
    }
}

/// Draw a humanoid character: capsule body + head, tinted by `color`.
pub fn draw_character(d3: &mut impl RaylibDraw3D, pos: Vector3, yaw: f32, color: Color, dead: bool) {
    if dead {
        // Lying down: a flat box.
        d3.draw_cube(
            Vector3 { x: pos.x, y: 0.3, z: pos.z },
            0.8, 0.4, 1.8,
            Color::new(color.r / 2, color.g / 2, color.b / 2, 255),
        );
        return;
    }
    let body = Vector3 { x: pos.x, y: pos.y + 0.1, z: pos.z };
    // Torso.
    d3.draw_cylinder(body, 0.35, 0.35, 1.0, 8, color);
    // Head.
    d3.draw_sphere(Vector3 { x: body.x, y: body.y + 0.75, z: body.z }, 0.28, Color::new(220, 180, 150, 255));
    // Facing indicator: a small forward nub.
    let (sx, sz) = (yaw.sin(), yaw.cos());
    let nub = Vector3 { x: body.x + sx * 0.3, y: body.y + 0.2, z: body.z + sz * 0.3 };
    d3.draw_cube(nub, 0.2, 0.2, 0.2, Color::new(30, 30, 40, 255));
    // Legs (two small boxes).
    d3.draw_cube(
        Vector3 { x: body.x - 0.15, y: body.y - 0.7, z: body.z },
        0.2, 0.8, 0.25,
        Color::new(40, 45, 70, 255),
    );
    d3.draw_cube(
        Vector3 { x: body.x + 0.15, y: body.y - 0.7, z: body.z },
        0.2, 0.8, 0.25,
        Color::new(40, 45, 70, 255),
    );
}

/// Draw a vertical pickup marker: glowing cylinder + floating icon cube.
pub fn draw_pickup(d3: &mut impl RaylibDraw3D, pos: Vector3, color: Color, t: f32) {
    let bob = (t * 3.0).sin() * 0.2;
    d3.draw_cylinder(
        Vector3 { x: pos.x, y: 0.6, z: pos.z },
        0.5, 0.5, 1.2, 12,
        Color::new(color.r, color.g, color.b, 90),
    );
    d3.draw_cube(
        Vector3 { x: pos.x, y: 1.0 + bob, z: pos.z },
        0.5, 0.5, 0.5,
        color,
    );
}

/// Draw a mission marker: a tall translucent cylinder pillar.
pub fn draw_mission_marker(d3: &mut impl RaylibDraw3D, pos: Vector3, color: Color, t: f32) {
    let pulse = 0.8 + 0.2 * (t * 4.0).sin();
    d3.draw_cylinder(
        Vector3 { x: pos.x, y: 2.0, z: pos.z },
        1.2 * pulse, 1.2 * pulse, 4.0, 16,
        Color::new(color.r, color.g, color.b, 80),
    );
}
