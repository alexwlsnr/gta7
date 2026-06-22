//! Procedural textures and cached models/meshes. All assets generated in code.
use raylib::prelude::*;
use raylib::ffi::Vector3;
use raylib::consts::MaterialMapIndex;

use crate::config::Config;
use crate::world::city::{Building, City, Axis};
use crate::vehicle::{Vehicle, VehicleKind};
use crate::ai::ped::Ped;
use crate::ai::cop::Cop;
use crate::player::Player;
use crate::mathx::{vadd, vsub, vscale};

/// Cached GPU assets built once at startup. Textures are kept as fields because
/// the `Model`s hold raw pointers to them — they must outlive the models.
pub struct Assets {
    pub building_model: Model,   // unit cube with window texture
    pub plain_cube_model: Model, // unit cube, lit via shader with a 1x1 white albedo texture
    pub window_tex: Texture2D,
    pub white_tex: Texture2D,
    pub ground_model: Model,     // large plane with ground texture
    pub ground_tex: Texture2D,
    pub road_tex: Texture2D,     // for HUD minimap
    pub sky_top: Color,
    pub sky_bottom: Color,
}

impl Assets {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: &Config) -> Self {
        let p = cfg.palette();

        // --- Window facade texture ---
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

        // --- Ground texture ---
        let mut ground = Image::gen_image_color(128, 128, p.sidewalk());
        let grass = p.grass();
        for _ in 0..400 {
            let x = (rand::random::<u32>() % 128) as i32;
            let y = (rand::random::<u32>() % 128) as i32;
            ground.draw_pixel(x, y, grass);
        }
        let ground_tex = rl.load_texture_from_image(thread, &ground).unwrap();

        // --- Road texture (for minimap) ---
        let mut road = Image::gen_image_color(64, 64, p.road());
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

        // --- Building model: unit cube with window texture ---
        let bm_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let bm_weak = unsafe { bm_mesh.make_weak() };
        let mut building_model = rl.load_model_from_mesh(thread, bm_weak).unwrap();
        building_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &window_tex);

        // --- Ground model: large textured plane ---
        let half = cfg.world_half() * 2.0;
        let gm_mesh = Mesh::gen_mesh_plane(thread, half, half, 1, 1);
        let gm_weak = unsafe { gm_mesh.make_weak() };
        let mut ground_model = rl.load_model_from_mesh(thread, gm_weak).unwrap();
        ground_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &ground_tex);

        // --- Plain cube model (for car bodies, character parts) ---
        // Give it a 1x1 white albedo texture so the lighting shader's texture0
        // sample multiplies by white instead of black. Tint comes from colDiffuse.
        let white = Image::gen_image_color(1, 1, Color::WHITE);
        let white_tex = rl.load_texture_from_image(thread, &white).unwrap();
        let pc_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let pc_weak = unsafe { pc_mesh.make_weak() };
        let mut plain_cube_model = rl.load_model_from_mesh(thread, pc_weak).unwrap();
        plain_cube_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &white_tex);
        Assets {
            building_model,
            plain_cube_model,
            window_tex,
            white_tex,
            ground_model,
            ground_tex,
            road_tex,
            sky_top: p.sky_top(),
            sky_bottom: p.sky_bottom(),
        }
    }
}

/// Draw the ground plane + roads + sidewalks + parks.
pub fn draw_world(d3: &mut impl RaylibDraw3D, city: &City, assets: &Assets, cfg: &Config) {
    let half = city.ground_half;
    let p = cfg.palette();

    // Textured ground plane.
    d3.draw_model(
        &assets.ground_model,
        Vector3 { x: 0.0, y: 0.0, z: 0.0 },
        1.0,
        Color::WHITE,
    );

    let n = city.blocks;
    let bs = city.block_size;
    let rw = city.road_width;
    let origin = -half;

    // Roads: colored strips along grid lines.
    let road_col = p.road();
    for i in 0..=n {
        let line = origin + i as f32 * bs;
        d3.draw_plane(
            Vector3 { x: 0.0, y: 0.03, z: line },
            Vector2::new(half * 2.0, rw),
            road_col,
        );
        d3.draw_plane(
            Vector3 { x: line, y: 0.03, z: 0.0 },
            Vector2::new(rw, half * 2.0),
            road_col,
        );
    }

    // Sidewalks: strips parallel to roads, offset on each side.
    let sw = cfg.sidewalk_width;
    let sw_off = cfg.sidewalk_offset();
    let sw_col = p.sidewalk();
    for i in 0..=n {
        let line = origin + i as f32 * bs;
        // Horizontal roads (along X): sidewalks at z = line ± sw_off
        d3.draw_plane(
            Vector3 { x: 0.0, y: 0.02, z: line - sw_off },
            Vector2::new(half * 2.0, sw),
            sw_col,
        );
        d3.draw_plane(
            Vector3 { x: 0.0, y: 0.02, z: line + sw_off },
            Vector2::new(half * 2.0, sw),
            sw_col,
        );
        // Vertical roads (along Z): sidewalks at x = line ± sw_off
        d3.draw_plane(
            Vector3 { x: line - sw_off, y: 0.02, z: 0.0 },
            Vector2::new(sw, half * 2.0),
            sw_col,
        );
        d3.draw_plane(
            Vector3 { x: line + sw_off, y: 0.02, z: 0.0 },
            Vector2::new(sw, half * 2.0),
            sw_col,
        );
    }

    // Lane center dashes (yellow).
    let yellow = Color::new(220, 190, 70, 255);
    for lane in &city.lanes {
        let a = city.intersection(lane.from.0, lane.from.1);
        let b = city.intersection(lane.to.0, lane.to.1);
        let (cx, cz) = lane_center(lane, rw);
        let mid = Vector3 {
            x: (a.x + b.x) * 0.5 + cx,
            y: 0.05,
            z: (a.z + b.z) * 0.5 + cz,
        };
        d3.draw_plane(mid, Vector2::new(2.0, 0.3), yellow);
    }

    // Parks: grass planes + trees.
    for bi in 0..n {
        for bj in 0..n {
            if !city.parks[bi * n + bj] {
                continue;
            }
            let cx = origin + (bi as f32 + 0.5) * bs;
            let cz = origin + (bj as f32 + 0.5) * bs;
            let lh = cfg.lot_half();
            d3.draw_plane(
                Vector3 { x: cx, y: 0.04, z: cz },
                Vector2::new(lh * 2.0, lh * 2.0),
                p.grass(),
            );
            for k in 0..3 {
                let tx = cx + (k as f32 - 1.0) * 6.0;
                let tz = cz + 2.0;
                d3.draw_cylinder(
                    Vector3 { x: tx, y: 1.0, z: tz },
                    0.3, 0.3, 2.0, 6,
                    Color::new(90, 60, 40, 255),
                );
                d3.draw_sphere(
                    Vector3 { x: tx, y: 2.6, z: tz },
                    1.2,
                    Color::new(40, 120, 50, 255),
                );
            }
        }
    }

    // Ramps (bright orange wedges).
    for r in &city.ramps {
        let slope = (r.height / r.length).atan();
        let hypot = (r.height * r.height + r.length * r.length).sqrt();
        
        let half_yaw = r.yaw * 0.5;
        let q_yaw = Quat {
            w: half_yaw.cos(),
            x: 0.0,
            y: half_yaw.sin(),
            z: 0.0,
        };
        let half_pitch = -slope * 0.5;
        let q_pitch = Quat {
            w: half_pitch.cos(),
            x: half_pitch.sin(),
            y: 0.0,
            z: 0.0,
        };
        let q = q_yaw * q_pitch;
        let (axis, angle_deg) = quat_to_axis_angle(q);
        
        let mid_pos = Vector3 { x: r.pos.x, y: r.pos.y + r.height * 0.5, z: r.pos.z };
        
        d3.draw_model_ex(
            &assets.plain_cube_model,
            mid_pos,
            axis, angle_deg,
            Vector3 { x: r.width, y: 0.2, z: hypot },
            Color::new(240, 110, 20, 255), // Bright orange deck
        );
        d3.draw_model_wires_ex(
            &assets.plain_cube_model,
            mid_pos,
            axis, angle_deg,
            Vector3 { x: r.width, y: 0.2, z: hypot },
            Color::new(30, 30, 30, 255),
        );
        
        // Support wall at high end
        let high_local_offset = Vector3 { x: 0.0, y: 0.0, z: r.length * 0.5 };
        let (sin_y, cos_y) = r.yaw.sin_cos();
        let high_world_offset = Vector3 {
            x: high_local_offset.z * sin_y,
            y: 0.0,
            z: high_local_offset.z * cos_y,
        };
        let high_pos = vadd(r.pos, high_world_offset);
        
        d3.draw_model_ex(
            &assets.plain_cube_model,
            Vector3 { x: high_pos.x, y: r.height * 0.5, z: high_pos.z },
            Vector3 { x: 0.0, y: 1.0, z: 0.0 }, r.yaw.to_degrees(),
            Vector3 { x: r.width, y: r.height, z: 0.2 },
            Color::new(100, 100, 105, 255),
        );
    }

    // Buildings: textured model tinted per-building.
    for b in &city.buildings {
        draw_building(d3, b, assets, &p);
    }
}

fn lane_center(lane: &crate::world::city::Lane, rw: f32) -> (f32, f32) {
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

    if b.has_windows {
        // Textured building: window facade tinted by body color.
        d3.draw_model_ex(
            &assets.building_model,
            c,
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            0.0,
            Vector3 { x: w, y: hgt, z: l },
            body,
        );
    } else {
        d3.draw_cube(c, w, hgt, l, body);
    }

    // Roof cap.
    let top = Vector3 { x: c.x, y: c.y + h.y + 0.1, z: c.z };
    d3.draw_cube(top, w * 0.9, 0.4, l * 0.9, p.building_top(b.color_index));
    // Edge wires for definition.
    d3.draw_cube_wires(c, w, hgt, l, Color::new(15, 15, 20, 255));
}

/// Draw a car body at a position with a yaw (radians) and a color.
/// Uses draw_model_ex for proper yaw rotation of the body + cabin.
#[allow(clippy::too_many_arguments)]
pub fn draw_car(
    d3: &mut impl RaylibDraw3D,
    assets: &Assets,
    pos: Vector3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    color: Color,
    damaged: f32,
    kind: VehicleKind,
    time: f32,
) {
    let body_w = 2.0;
    let body_h = 0.8;
    let body_l = 4.2;

    let half_yaw = yaw * 0.5;
    let q_yaw = Quat {
        w: half_yaw.cos(),
        x: 0.0,
        y: half_yaw.sin(),
        z: 0.0,
    };
    let half_pitch = pitch * 0.5;
    let q_pitch = Quat {
        w: half_pitch.cos(),
        x: half_pitch.sin(),
        y: 0.0,
        z: 0.0,
    };
    let half_roll = roll * 0.5;
    let q_roll = Quat {
        w: half_roll.cos(),
        x: 0.0,
        y: 0.0,
        z: half_roll.sin(),
    };
    
    let q = q_yaw * q_pitch * q_roll;
    let (axis, angle_deg) = quat_to_axis_angle(q);

    let body_color = if kind == VehicleKind::Police {
        Color::new(20, 20, 20, 255)
    } else {
        color
    };
    let cabin_color = if kind == VehicleKind::Police {
        Color::new(245, 245, 245, 255)
    } else {
        Color::new(60, 80, 110, 255)
    };

    // Body
    d3.draw_model_ex(
        &assets.plain_cube_model,
        pos,
        axis, angle_deg,
        Vector3 { x: body_w, y: body_h, z: body_l },
        body_color,
    );
    // Body outline
    d3.draw_model_wires_ex(
        &assets.plain_cube_model,
        pos,
        axis, angle_deg,
        Vector3 { x: body_w, y: body_h, z: body_l },
        Color::new(20, 20, 20, 255),
    );

    // Cabin: offset in local coordinates: Z = -0.2, Y = 0.7
    let cabin_local = Vector3 { x: 0.0, y: 0.7, z: -0.2 };
    let cabin_world = vadd(pos, rotate_vector(cabin_local, q));
    d3.draw_model_ex(
        &assets.plain_cube_model,
        cabin_world,
        axis, angle_deg,
        Vector3 { x: 1.6, y: 0.6, z: 2.0 },
        cabin_color,
    );

    if kind == VehicleKind::Police {
        // Dark bar base
        let bar_local = Vector3 { x: 0.0, y: 1.05, z: -0.2 };
        let bar_world = vadd(pos, rotate_vector(bar_local, q));
        d3.draw_model_ex(
            &assets.plain_cube_model,
            bar_world,
            axis, angle_deg,
            Vector3 { x: 1.2, y: 0.1, z: 0.25 },
            Color::new(30, 30, 30, 255),
        );

        // Flashing lights (alternate red and blue)
        let red_flash = (time * 12.0).sin() > 0.0;
        let left_color = if red_flash { Color::new(255, 30, 30, 255) } else { Color::new(50, 0, 0, 255) };
        let right_color = if !red_flash { Color::new(30, 30, 255, 255) } else { Color::new(0, 0, 50, 255) };

        // Left siren dome
        let left_local = Vector3 { x: -0.35, y: 1.15, z: -0.2 };
        let left_world = vadd(pos, rotate_vector(left_local, q));
        d3.draw_model_ex(
            &assets.plain_cube_model,
            left_world,
            axis, angle_deg,
            Vector3 { x: 0.3, y: 0.12, z: 0.2 },
            left_color,
        );

        // Right siren dome
        let right_local = Vector3 { x: 0.35, y: 1.15, z: -0.2 };
        let right_world = vadd(pos, rotate_vector(right_local, q));
        d3.draw_model_ex(
            &assets.plain_cube_model,
            right_world,
            axis, angle_deg,
            Vector3 { x: 0.3, y: 0.12, z: 0.2 },
            right_color,
        );
    }

    // Wheels (4 cylinders), positioned using rotated local offsets and drawn along wheel axis
    let wheel_local_offsets = [
        Vector3 { x: body_w * 0.5, y: -0.4, z: body_l * 0.32 },
        Vector3 { x: -body_w * 0.5, y: -0.4, z: body_l * 0.32 },
        Vector3 { x: body_w * 0.5, y: -0.4, z: -body_l * 0.32 },
        Vector3 { x: -body_w * 0.5, y: -0.4, z: -body_l * 0.32 },
    ];
    let local_wheel_axis = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
    let world_wheel_axis = rotate_vector(local_wheel_axis, q);

    for off in wheel_local_offsets {
        let wheel_center = vadd(pos, rotate_vector(off, q));
        let start = vsub(wheel_center, vscale(world_wheel_axis, 0.15));
        let end = vadd(wheel_center, vscale(world_wheel_axis, 0.15));
        
        d3.draw_cylinder_ex(
            start, end,
            0.4, 0.4, 10,
            Color::new(25, 25, 25, 255),
        );
    }

    // Headlights (front, white) + taillights (rear, red)
    let light_offsets_color = [
        (Vector3 { x: body_w * 0.4, y: 0.0, z: body_l * 0.5 }, Color::new(255, 255, 200, 255)),
        (Vector3 { x: -body_w * 0.4, y: 0.0, z: body_l * 0.5 }, Color::new(255, 255, 200, 255)),
        (Vector3 { x: body_w * 0.4, y: 0.0, z: -body_l * 0.5 }, Color::new(200, 40, 40, 255)),
        (Vector3 { x: -body_w * 0.4, y: 0.0, z: -body_l * 0.5 }, Color::new(200, 40, 40, 255)),
    ];
    for (off, col) in light_offsets_color {
        let light_pos = vadd(pos, rotate_vector(off, q));
        d3.draw_model_ex(
            &assets.plain_cube_model,
            light_pos,
            axis, angle_deg,
            Vector3 { x: 0.3, y: 0.2, z: 0.2 },
            col,
        );
    }

    // Damage smoke
    if damaged > 0.4 {
        d3.draw_model_wires_ex(
            &assets.plain_cube_model,
            pos,
            axis, angle_deg,
            Vector3 { x: body_w + 0.05, y: body_h + 0.05, z: body_l + 0.05 },
            Color::new(60, 40, 30, 255),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairStyle {
    Bald,
    ShortHair,
    Afro,
    Cap,
    PoliceHat,
}

/// Draw a humanoid character: capsule body + head, tinted by `color`.
/// Supports variable shirt/pants/hair colors, hairstyles, and sunglasses.
#[allow(clippy::too_many_arguments)]
pub fn draw_character(
    d3: &mut impl RaylibDraw3D,
    assets: &Assets,
    pos: Vector3,
    yaw: f32,
    shirt_color: Color,
    pants_color: Color,
    hair_color: Color,
    hair_style: HairStyle,
    has_glasses: bool,
    dead: bool,
    time: f32,
    is_moving: bool,
) {
    let up = Vector3 { x: 0.0, y: 1.0, z: 0.0 };
    let yaw_deg = yaw.to_degrees();
    
    if dead {
        // Lying down: flat rotated box (darkened shirt color).
        d3.draw_model_ex(
            &assets.plain_cube_model,
            Vector3 { x: pos.x, y: 0.2, z: pos.z },
            up, yaw_deg,
            Vector3 { x: 0.8, y: 0.4, z: 1.8 },
            Color::new(shirt_color.r / 2, shirt_color.g / 2, shirt_color.b / 2, 255),
        );
        return;
    }

    let (sx, sz) = (yaw.sin(), yaw.cos());

    // --- 1. Torso (Shirt / Jacket) ---
    let torso_pos = Vector3 { x: pos.x, y: pos.y + 1.25, z: pos.z };
    d3.draw_model_ex(
        &assets.plain_cube_model,
        torso_pos,
        up, yaw_deg,
        Vector3 { x: 0.5, y: 0.8, z: 0.26 },
        shirt_color,
    );

    // --- 2. Pelvis / Pants top ---
    let pelvis_pos = Vector3 { x: pos.x, y: pos.y + 0.80, z: pos.z };
    d3.draw_model_ex(
        &assets.plain_cube_model,
        pelvis_pos,
        up, yaw_deg,
        Vector3 { x: 0.48, y: 0.12, z: 0.24 },
        pants_color,
    );

    // --- 3. Head (Skin color) ---
    let head_pos = Vector3 { x: pos.x, y: pos.y + 1.76, z: pos.z };
    d3.draw_sphere(
        head_pos,
        0.24,
        Color::new(225, 185, 150, 255),
    );

    // --- 4. Sunglasses (Cool GTA glasses) ---
    if has_glasses {
        let glasses_pos = Vector3 {
            x: head_pos.x + sx * 0.18,
            y: head_pos.y + 0.05,
            z: head_pos.z + sz * 0.18,
        };
        d3.draw_model_ex(
            &assets.plain_cube_model,
            glasses_pos,
            up, yaw_deg,
            Vector3 { x: 0.32, y: 0.08, z: 0.1 },
            Color::new(20, 20, 20, 255), // Dark lenses
        );
    }

    // --- 5. Hair / Headwear styles ---
    match hair_style {
        HairStyle::Bald => {
            // No hair, just skin.
        }
        HairStyle::ShortHair => {
            // Simple hair crop.
            let hair_pos = Vector3 { x: head_pos.x, y: head_pos.y + 0.1, z: head_pos.z };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                hair_pos,
                up, yaw_deg,
                Vector3 { x: 0.26, y: 0.16, z: 0.26 },
                hair_color,
            );
        }
        HairStyle::Afro => {
            // Large round afro sphere.
            let afro_pos = Vector3 { x: head_pos.x, y: head_pos.y + 0.08, z: head_pos.z };
            d3.draw_sphere(afro_pos, 0.29, hair_color);
        }
        HairStyle::Cap => {
            // Baseball cap.
            let cap_pos = Vector3 { x: head_pos.x, y: head_pos.y + 0.22, z: head_pos.z };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                cap_pos,
                up, yaw_deg,
                Vector3 { x: 0.28, y: 0.1, z: 0.28 },
                hair_color, // cap dome
            );
            let brim_pos = Vector3 {
                x: head_pos.x + sx * 0.18,
                y: head_pos.y + 0.20,
                z: head_pos.z + sz * 0.18,
            };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                brim_pos,
                up, yaw_deg,
                Vector3 { x: 0.24, y: 0.02, z: 0.18 },
                hair_color, // cap brim
            );
        }
        HairStyle::PoliceHat => {
            // Cop hat: peaked cap.
            let hat_pos = Vector3 { x: head_pos.x, y: head_pos.y + 0.22, z: head_pos.z };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                hat_pos,
                up, yaw_deg,
                Vector3 { x: 0.32, y: 0.1, z: 0.32 },
                hair_color, // Dark blue cap
            );
            let visor_pos = Vector3 {
                x: head_pos.x + sx * 0.20,
                y: head_pos.y + 0.18,
                z: head_pos.z + sz * 0.20,
            };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                visor_pos,
                up, yaw_deg,
                Vector3 { x: 0.28, y: 0.02, z: 0.16 },
                Color::new(10, 10, 10, 255), // Black visor peak
            );
            // Small gold badge in front.
            let badge_pos = Vector3 {
                x: head_pos.x + sx * 0.17,
                y: head_pos.y + 0.24,
                z: head_pos.z + sz * 0.17,
            };
            d3.draw_model_ex(
                &assets.plain_cube_model,
                badge_pos,
                up, yaw_deg,
                Vector3 { x: 0.06, y: 0.06, z: 0.04 },
                Color::new(255, 215, 0, 255), // Gold badge
            );
        }
    }

    // --- Animation logic ---
    let swing = if is_moving {
        (time * 12.0).sin() * 0.32
    } else {
        0.0
    };

    // --- 6. Legs (Jeans) ---
    let left_leg_pos = Vector3 {
        x: pos.x + 0.13 * sz + swing * sx,
        y: pos.y + 0.375,
        z: pos.z - 0.13 * sx + swing * sz,
    };
    let right_leg_pos = Vector3 {
        x: pos.x - 0.13 * sz - swing * sx,
        y: pos.y + 0.375,
        z: pos.z + 0.13 * sx - swing * sz,
    };

    d3.draw_model_ex(
        &assets.plain_cube_model,
        left_leg_pos,
        up, yaw_deg,
        Vector3 { x: 0.18, y: 0.75, z: 0.2 },
        pants_color,
    );
    d3.draw_model_ex(
        &assets.plain_cube_model,
        right_leg_pos,
        up, yaw_deg,
        Vector3 { x: 0.18, y: 0.75, z: 0.2 },
        pants_color,
    );

    // --- 7. Arms ---
    let left_arm_pos = Vector3 {
        x: pos.x + 0.3 * sz - swing * 0.7 * sx,
        y: pos.y + 1.25,
        z: pos.z - 0.3 * sx - swing * 0.7 * sz,
    };
    let right_arm_pos = Vector3 {
        x: pos.x - 0.3 * sz + swing * 0.7 * sx,
        y: pos.y + 1.25,
        z: pos.z + 0.3 * sx + swing * 0.7 * sz,
    };

    d3.draw_model_ex(
        &assets.plain_cube_model,
        left_arm_pos,
        up, yaw_deg,
        Vector3 { x: 0.14, y: 0.65, z: 0.16 },
        shirt_color,
    );
    d3.draw_model_ex(
        &assets.plain_cube_model,
        right_arm_pos,
        up, yaw_deg,
        Vector3 { x: 0.14, y: 0.65, z: 0.16 },
        shirt_color,
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

/// Draw all shadow-casting geometry for the shadow map pass.
///
/// Renders simplified boxes for buildings, vehicles, and characters. Everything
/// is drawn white — the depth shader only cares about depth, not color. Called
/// inside the shadow map's render-texture mode with the depth shader active.
#[allow(clippy::too_many_arguments)]
pub fn draw_shadow_casters(
    d3: &mut impl RaylibDraw3D,
    city: &City,
    _assets: &Assets,
    _cfg: &Config,
    vehicles: &[Vehicle],
    peds: &[Ped],
    cops: &[Cop],
    player: &Player,
) {
    // Buildings.
    for b in &city.buildings {
        let c = b.box3d.center();
        let h = b.box3d.half();
        d3.draw_cube(c, h.x * 2.0, h.y * 2.0, h.z * 2.0, Color::WHITE);
    }
    // Vehicles (simple boxes for shadow).
    for v in vehicles {
        if v.destroyed {
            continue;
        }
        d3.draw_cube(v.pos, 2.0, 0.8, 4.2, Color::WHITE);
    }
    // Characters (simple boxes for shadow).
    for ped in peds {
        if ped.dead() {
            continue;
        }
        d3.draw_cube(
            Vector3 { x: ped.pos.x, y: ped.pos.y + 0.9, z: ped.pos.z },
            0.4, 1.8, 0.4, Color::WHITE,
        );
    }
    for cop in cops {
        if cop.dead() {
            continue;
        }
        d3.draw_cube(
            Vector3 { x: cop.pos.x, y: cop.pos.y + 0.9, z: cop.pos.z },
            0.4, 1.8, 0.4, Color::WHITE,
        );
    }
    // Player.
    if player.alive {
        d3.draw_cube(
            Vector3 { x: player.pos.x, y: player.pos.y + 0.9, z: player.pos.z },
            0.4, 1.8, 0.4, Color::WHITE,
        );
    }
}

// --- Quaternion Math Helpers for 3D Rotations ---

#[derive(Clone, Copy, Debug)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl std::ops::Mul for Quat {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Quat {
            w: self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
            x: self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            y: self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            z: self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
        }
    }
}

pub fn quat_to_axis_angle(q: Quat) -> (Vector3, f32) {
    let len = (q.w * q.w + q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
    if len < 1e-6 {
        return (Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 0.0);
    }
    let w = q.w / len;
    let x = q.x / len;
    let y = q.y / len;
    let z = q.z / len;

    let sin_half = (1.0 - w * w).sqrt();
    if sin_half < 1e-6 {
        (Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 0.0)
    } else {
        let angle = 2.0 * w.clamp(-1.0, 1.0).acos().to_degrees();
        let axis = Vector3 {
            x: x / sin_half,
            y: y / sin_half,
            z: z / sin_half,
        };
        (axis, angle)
    }
}

pub fn rotate_vector(v: Vector3, q: Quat) -> Vector3 {
    let q_vec = Quat { w: 0.0, x: v.x, y: v.y, z: v.z };
    let q_conj = Quat { w: q.w, x: -q.x, y: -q.y, z: -q.z };
    let rotated = q * q_vec * q_conj;
    Vector3 { x: rotated.x, y: rotated.y, z: rotated.z }
}
