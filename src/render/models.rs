//! Procedural textures and cached models/meshes. All assets generated in code.
use raylib::prelude::*;
use raylib::ffi::Vector3;
use raylib::consts::MaterialMapIndex;

use crate::config::Config;
use crate::world::city::{Building, City, Axis};
use crate::vehicle::{Vehicle, VehicleKind, VehicleVariant};
use crate::ai::ped::Ped;
use crate::ai::cop::Cop;
use crate::player::Player;
use crate::render::lighting::LightingSystem;
use crate::mathx::vadd;

/// Cached GPU assets built once at startup. Textures are kept as fields because
/// the `Model`s hold raw pointers to them — they must outlive the models.
pub struct Assets {
    pub building_model: Model,   // unit cube with window texture
    pub plain_cube_model: Model, // unit cube, lit via shader with a 1x1 white albedo texture
    pub carbon_cube_model: Model, // unit cube with carbon fiber texture
    pub grill_cube_model: Model,  // unit cube with grille texture
    pub cylinder_model: Model,    // cylinder model for wheels and exhaust pipes
    pub sphere_model: Model,      // sphere model for headlights, taillights, sirens
    pub tire_model: Model,        // cylinder model with tire tread texture
    pub headlight_model: Model,   // cube model with headlight texture
    pub taillight_model: Model,   // cube model with taillight texture
    pub plate_model: Model,       // thin cube model with license plate texture
    pub dash_model: Model,        // cube model with dashboard texture
    pub window_tex: Texture2D,
    pub white_tex: Texture2D,
    pub ground_model: Model,     // large plane with ground texture
    pub ground_tex: Texture2D,
    pub road_tex: Texture2D,     // for HUD minimap
    pub carbon_tex: Texture2D,
    pub grill_tex: Texture2D,
    pub tire_tex: Texture2D,
    pub hl_tex: Texture2D,
    pub tl_tex: Texture2D,
    pub plate_tex: Texture2D,
    pub dash_tex: Texture2D,
    pub sky_top: Color,
    pub sky_bottom: Color,
    pub sun_model: Model,
    pub sun_tex: Texture2D,
    pub underglow_model: Model,
    pub underglow_tex: Texture2D,
}

impl Assets {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: &Config) -> Self {
        let p = cfg.palette();

        // --- Window facade texture (128x128 with realistic detail) ---
        let mut win = Image::gen_image_color(128, 128, Color::new(45, 52, 72, 255));
        // Each window cell is 16x16 pixels, giving an 8x8 grid of windows
        for by in 0..8 {
            for bx in 0..8 {
                let x0 = bx * 16;
                let y0 = by * 16;
                // Window frame border (dark)
                let frame = Color::new(30, 34, 48, 255);
                for yy in 0..16 {
                    for xx in 0..16 {
                        win.draw_pixel(x0 + xx, y0 + yy, frame);
                    }
                }
                // Determine window state using a deterministic hash
                let hash = ((bx * 7 + by * 13 + bx * by * 3) % 11) as u8;
                let (pane_col, has_curtain) = match hash {
                    0..=2 => (Color::new(0, 255, 255, 254), false),    // neon cyan window
                    3 => (Color::new(255, 0, 180, 254), false),        // hot pink/magenta window
                    4 => (Color::new(50, 255, 50, 254), true),         // neon lime green window
                    5 => (Color::new(255, 110, 0, 254), true),         // laser orange window
                    _ => (Color::new(18, 12, 38, 255), false),         // dark unlit windows (standard alpha)
                };
                // Inner window pane (with 2px frame inset)
                for yy in 2..14 {
                    for xx in 2..14 {
                        let mut c = pane_col;
                        // Horizontal blinds effect for curtained windows
                        if has_curtain && yy % 3 == 0 {
                            c = Color::new(
                                (c.r as f32 * 0.6) as u8,
                                (c.g as f32 * 0.6) as u8,
                                (c.b as f32 * 0.6) as u8,
                                c.a, // Preserve tagged alpha (254 or 255)
                            );
                        }
                        // Vertical mullion (center divider)
                        if xx == 7 || xx == 8 {
                            c = frame;
                        }
                        // Horizontal transom (center divider)
                        if yy == 7 || yy == 8 {
                            c = frame;
                        }
                        win.draw_pixel(x0 + xx, y0 + yy, c);
                    }
                }
                // Window sill at bottom (light concrete strip)
                for xx in 1..15 {
                    win.draw_pixel(x0 + xx, y0 + 14, Color::new(80, 82, 90, 255));
                    win.draw_pixel(x0 + xx, y0 + 15, Color::new(70, 72, 80, 255));
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

        // --- Carbon Fiber Texture (Alternating dark grey / black blocks) ---
        let mut carbon = Image::gen_image_color(16, 16, Color::new(35, 35, 35, 255));
        for y in 0..16 {
            for x in 0..16 {
                // A diagonal herringbone weave pattern
                let is_dark = (x / 2 + y / 2) % 2 == 0;
                let val = if is_dark { 18 } else { 32 };
                // Add minor texture variation
                let noise = (x % 2) * 4 - 2;
                let c = (val + noise).clamp(0, 255) as u8;
                carbon.draw_pixel(x, y, Color::new(c, c, c, 255));
            }
        }
        let carbon_tex = rl.load_texture_from_image(thread, &carbon).unwrap();

        // --- Radiator Grille Texture (Horizontal slats with vertical dividers) ---
        let mut grill = Image::gen_image_color(16, 16, Color::new(15, 15, 15, 255));
        for y in 0..16 {
            for x in 0..16 {
                let is_slat = (y % 4 == 0) || (y % 4 == 1);
                let is_mesh_vertical = x % 4 == 0;
                if is_slat || is_mesh_vertical {
                    // Mesh bar (light grey)
                    grill.draw_pixel(x, y, Color::new(55, 55, 60, 255));
                } else {
                    // Empty space (darker)
                    grill.draw_pixel(x, y, Color::new(10, 10, 12, 255));
                }
            }
        }
        let grill_tex = rl.load_texture_from_image(thread, &grill).unwrap();

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

        // --- Carbon cube model ---
        let cc_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let cc_weak = unsafe { cc_mesh.make_weak() };
        let mut carbon_cube_model = rl.load_model_from_mesh(thread, cc_weak).unwrap();
        carbon_cube_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &carbon_tex);

        // --- Grill cube model ---
        let gc_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let gc_weak = unsafe { gc_mesh.make_weak() };
        let mut grill_cube_model = rl.load_model_from_mesh(thread, gc_weak).unwrap();
        grill_cube_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &grill_tex);

        // --- Cylinder model (unit: radius 0.5, height 1.0) ---
        let cyl_mesh = Mesh::gen_mesh_cylinder(thread, 0.5, 1.0, 16);
        let cyl_weak = unsafe { cyl_mesh.make_weak() };
        let mut cylinder_model = rl.load_model_from_mesh(thread, cyl_weak).unwrap();
        cylinder_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &white_tex);

        // --- Sphere model (unit: radius 0.5) ---
        let sph_mesh = Mesh::gen_mesh_sphere(thread, 0.5, 16, 16);
        let sph_weak = unsafe { sph_mesh.make_weak() };
        let mut sphere_model = rl.load_model_from_mesh(thread, sph_weak).unwrap();
        sphere_model
            .materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &white_tex);

        // --- 1. Tire Tread Texture & Model ---
        let mut tire_img = Image::gen_image_color(16, 16, Color::new(28, 28, 28, 255));
        for y in 0..16 {
            for x in 0..16 {
                // herringbone tread pattern
                let is_groove = (x == 4) || (x == 12) || ((x + y) % 4 == 0 && (x > 4 && x < 12));
                if is_groove {
                    tire_img.draw_pixel(x, y, Color::new(14, 14, 14, 255));
                }
            }
        }
        let tire_tex = rl.load_texture_from_image(thread, &tire_img).unwrap();
        let tire_mesh = Mesh::gen_mesh_cylinder(thread, 0.5, 1.0, 16);
        let tire_weak = unsafe { tire_mesh.make_weak() };
        let mut tire_model = rl.load_model_from_mesh(thread, tire_weak).unwrap();
        tire_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &tire_tex);

        // --- 2. Headlight Texture & Model ---
        let mut hl_img = Image::gen_image_color(16, 16, Color::new(230, 230, 235, 255));
        for y in 0..16 {
            for x in 0..16 {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 7.5;
                let r2 = dx*dx + dy*dy;
                if r2 < 12.0 {
                    hl_img.draw_pixel(x, y, Color::new(0, 240, 255, 255)); // cyan bulb glow
                } else if r2 < 24.0 {
                    hl_img.draw_pixel(x, y, Color::new(170, 170, 175, 255)); // silver reflector
                } else {
                    hl_img.draw_pixel(x, y, Color::new(40, 40, 42, 255)); // dark housing
                }
            }
        }
        let hl_tex = rl.load_texture_from_image(thread, &hl_img).unwrap();
        let hl_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let hl_weak = unsafe { hl_mesh.make_weak() };
        let mut headlight_model = rl.load_model_from_mesh(thread, hl_weak).unwrap();
        headlight_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &hl_tex);

        // --- 3. Taillight Texture & Model ---
        let mut tl_img = Image::gen_image_color(16, 16, Color::new(180, 20, 20, 255));
        for y in 0..16 {
            for x in 0..16 {
                let is_grid = (x % 3 == 0) || (y % 3 == 0);
                if is_grid {
                    tl_img.draw_pixel(x, y, Color::new(255, 0, 180, 255)); // bright neon pink grid
                }
                // indicators / reverse
                if x >= 11 && y >= 11 {
                    tl_img.draw_pixel(x, y, Color::new(245, 245, 245, 255)); // white reverse
                } else if x >= 11 && y >= 6 && y < 11 {
                    tl_img.draw_pixel(x, y, Color::new(255, 140, 0, 255)); // amber turn signal
                }
            }
        }
        let tl_tex = rl.load_texture_from_image(thread, &tl_img).unwrap();
        let tl_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let tl_weak = unsafe { tl_mesh.make_weak() };
        let mut taillight_model = rl.load_model_from_mesh(thread, tl_weak).unwrap();
        taillight_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &tl_tex);

        // --- 4. License Plate Texture & Model ---
        let mut plate_img = Image::gen_image_color(32, 16, Color::new(240, 220, 40, 255)); // yellow license plate
        // border
        for x in 0..32 {
            plate_img.draw_pixel(x, 0, Color::new(10, 10, 10, 255));
            plate_img.draw_pixel(x, 15, Color::new(10, 10, 10, 255));
        }
        for y in 0..16 {
            plate_img.draw_pixel(0, y, Color::new(10, 10, 10, 255));
            plate_img.draw_pixel(31, y, Color::new(10, 10, 10, 255));
        }
        // Draw "GTA 7" text using pixel matrices
        let draw_char_pixel = |img: &mut Image, c: char, ox: i32, oy: i32| {
            let pixels: &[(i32, i32)] = match c {
                'G' => &[(1,0),(2,0),(3,0),(0,1),(0,2),(0,3),(3,2),(4,2),(1,4),(2,4),(3,4),(4,3),(4,1)],
                'T' => &[(0,0),(1,0),(2,0),(3,0),(4,0),(2,1),(2,2),(2,3),(2,4)],
                'A' => &[(2,0),(1,1),(3,1),(0,2),(4,2),(0,3),(1,3),(2,3),(3,3),(4,3),(0,4),(4,4)],
                '7' => &[(0,0),(1,0),(2,0),(3,0),(4,0),(4,1),(3,2),(2,3),(1,4)],
                _ => &[],
            };
            for &(px, py) in pixels {
                img.draw_pixel(ox + px, oy + py, Color::new(10, 10, 12, 255));
            }
        };
        draw_char_pixel(&mut plate_img, 'G', 4, 5);
        draw_char_pixel(&mut plate_img, 'T', 10, 5);
        draw_char_pixel(&mut plate_img, 'A', 16, 5);
        draw_char_pixel(&mut plate_img, '7', 23, 5);

        let plate_tex = rl.load_texture_from_image(thread, &plate_img).unwrap();
        let plate_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let plate_weak = unsafe { plate_mesh.make_weak() };
        let mut plate_model = rl.load_model_from_mesh(thread, plate_weak).unwrap();
        plate_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &plate_tex);

        // --- 5. Dashboard Texture & Model ---
        let mut dash_img = Image::gen_image_color(32, 16, Color::new(25, 25, 28, 255)); // dark dash
        // speedo dial outline
        for a in 0..360 {
            let rad = (a as f32).to_radians();
            let sx = (8.0 + 4.5 * rad.cos()) as i32;
            let sy = (8.0 + 4.5 * rad.sin()) as i32;
            dash_img.draw_pixel(sx, sy, Color::new(0, 180, 255, 255)); // cyan glow speedo
            let tx = (22.0 + 4.5 * rad.cos()) as i32;
            let ty = (8.0 + 4.5 * rad.sin()) as i32;
            dash_img.draw_pixel(tx, ty, Color::new(0, 180, 255, 255)); // cyan glow tacho
        }
        // Speedo needle (pointing up-right)
        dash_img.draw_pixel(8, 8, Color::new(255, 100, 0, 255));
        dash_img.draw_pixel(9, 7, Color::new(255, 100, 0, 255));
        dash_img.draw_pixel(10, 6, Color::new(255, 100, 0, 255));
        // Tacho needle (pointing up-left)
        dash_img.draw_pixel(22, 8, Color::new(255, 100, 0, 255));
        dash_img.draw_pixel(21, 7, Color::new(255, 100, 0, 255));
        dash_img.draw_pixel(20, 6, Color::new(255, 100, 0, 255));

        let dash_tex = rl.load_texture_from_image(thread, &dash_img).unwrap();
        let dash_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let dash_weak = unsafe { dash_mesh.make_weak() };
        let mut dash_model = rl.load_model_from_mesh(thread, dash_weak).unwrap();
        dash_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &dash_tex);

        // --- 5. Vaporwave Sun Texture & Model ---
        let mut sun_img = Image::gen_image_color(64, 64, Color::new(0, 0, 0, 0));
        for y in 0..64 {
            let fy = y as f32 / 63.0; // 0 at top, 1 at bottom
            for x in 0..64 {
                let dx = x as f32 - 31.5;
                let dy = y as f32 - 31.5;
                let dist = (dx*dx + dy*dy).sqrt();
                if dist < 31.5 {
                    // OutRun styled scanline stripes
                    let stripe_pitch = 8.0;
                    let stripe_val = (y as f32 % stripe_pitch) / stripe_pitch;
                    let threshold = fy * 0.85;
                    if stripe_val < threshold && y > 12 {
                        continue;
                    }
                    // Color gradient: yellow/orange at top to hot pink/magenta at bottom
                    let r = 255;
                    let g = ((1.0 - fy) * 230.0) as u8;
                    let b = (fy * 150.0) as u8;
                    let edge_alpha = ((31.5 - dist).clamp(0.0, 1.0) * 255.0) as u8;
                    sun_img.draw_pixel(x, y, Color::new(r, g, b, edge_alpha));
                }
            }
        }
        let sun_tex = rl.load_texture_from_image(thread, &sun_img).unwrap();
        let sun_mesh = Mesh::gen_mesh_cube(thread, 1.0, 1.0, 1.0);
        let sun_weak = unsafe { sun_mesh.make_weak() };
        let mut sun_model = rl.load_model_from_mesh(thread, sun_weak).unwrap();
        sun_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &sun_tex);

        // --- 6. Underglow Texture & Model ---
        let mut ug_img = Image::gen_image_color(32, 32, Color::new(0, 0, 0, 0));
        for y in 0..32 {
            for x in 0..32 {
                let dx = x as f32 - 15.5;
                let dy = y as f32 - 15.5;
                let dist = (dx*dx + dy*dy).sqrt();
                let alpha = (1.0 - (dist / 15.5).clamp(0.0, 1.0)).powf(2.0);
                let a_val = (alpha * 255.0) as u8;
                ug_img.draw_pixel(x, y, Color::new(255, 255, 255, a_val));
            }
        }
        let underglow_tex = rl.load_texture_from_image(thread, &ug_img).unwrap();
        let ug_mesh = Mesh::gen_mesh_plane(thread, 1.0, 1.0, 1, 1);
        let ug_weak = unsafe { ug_mesh.make_weak() };
        let mut underglow_model = rl.load_model_from_mesh(thread, ug_weak).unwrap();
        underglow_model.materials_mut()[0]
            .set_material_texture(MaterialMapIndex::MATERIAL_MAP_ALBEDO, &underglow_tex);

        Assets {
            building_model,
            plain_cube_model,
            carbon_cube_model,
            grill_cube_model,
            cylinder_model,
            sphere_model,
            tire_model,
            headlight_model,
            taillight_model,
            plate_model,
            dash_model,
            window_tex,
            white_tex,
            ground_model,
            ground_tex,
            road_tex,
            carbon_tex,
            grill_tex,
            tire_tex,
            hl_tex,
            tl_tex,
            plate_tex,
            dash_tex,
            sky_top: p.sky_top(),
            sky_bottom: p.sky_bottom(),
            sun_model,
            sun_tex,
            underglow_model,
            underglow_tex,
        }
    }
}

/// Draw the ground plane + roads + sidewalks + parks + streetlights.
pub fn draw_world(d3: &mut impl RaylibDraw3D, city: &City, assets: &Assets, cfg: &Config, hour: f32, cam_pos: Vector3) {
    let p = cfg.palette();
    let bs = city.block_size;
    let rw = city.road_width;
    let origin = -city.ground_half;

    // Textured ground plane, snapped to block size to avoid texture sliding.
    let snap_x = (cam_pos.x / bs).round() * bs;
    let snap_z = (cam_pos.z / bs).round() * bs;
    d3.draw_model(
        &assets.ground_model,
        Vector3 { x: snap_x, y: 0.0, z: snap_z },
        1.0,
        Color::WHITE,
    );

    // Determine the block range around the camera (render radius of 5 blocks).
    let radius = 5;
    let (cam_bi, cam_bj) = city.get_block_coords(cam_pos.x, cam_pos.z);

    // Roads: colored strips along grid lines.
    let road_col = p.road();
    for i in (cam_bi - radius)..=(cam_bi + radius) {
        for j in (cam_bj - radius)..=(cam_bj + radius) {
            // Horizontal road segment for cell (i, j) along X, from i to i+1 at z = j
            let line_z = origin + j as f32 * bs;
            let center_x = origin + (i as f32 + 0.5) * bs;
            d3.draw_plane(
                Vector3 { x: center_x, y: 0.03, z: line_z },
                Vector2::new(bs, rw),
                road_col,
            );

            // Vertical road segment for cell (i, j) along Z, from j to j+1 at x = i
            let line_x = origin + i as f32 * bs;
            let center_z = origin + (j as f32 + 0.5) * bs;
            d3.draw_plane(
                Vector3 { x: line_x, y: 0.03, z: center_z },
                Vector2::new(rw, bs),
                road_col,
            );
        }
    }

    // Sidewalks: strips parallel to roads, offset on each side.
    let sw = cfg.sidewalk_width;
    let sw_off = cfg.sidewalk_offset();
    let sw_col = p.sidewalk();
    for i in (cam_bi - radius)..=(cam_bi + radius) {
        for j in (cam_bj - radius)..=(cam_bj + radius) {
            let line_z = origin + j as f32 * bs;
            let center_x = origin + (i as f32 + 0.5) * bs;
            d3.draw_plane(
                Vector3 { x: center_x, y: 0.02, z: line_z - sw_off },
                Vector2::new(bs, sw),
                sw_col,
            );
            d3.draw_plane(
                Vector3 { x: center_x, y: 0.02, z: line_z + sw_off },
                Vector2::new(bs, sw),
                sw_col,
            );

            let line_x = origin + i as f32 * bs;
            let center_z = origin + (j as f32 + 0.5) * bs;
            d3.draw_plane(
                Vector3 { x: line_x - sw_off, y: 0.02, z: center_z },
                Vector2::new(sw, bs),
                sw_col,
            );
            d3.draw_plane(
                Vector3 { x: line_x + sw_off, y: 0.02, z: center_z },
                Vector2::new(sw, bs),
                sw_col,
            );
        }
    }

    // Lane center dashes (neon cyan).
    let neon_cyan = Color::new(0, 255, 255, 255);
    for lane in &city.lanes {
        let a = city.intersection(lane.from.0, lane.from.1);
        let b = city.intersection(lane.to.0, lane.to.1);
        let (cx, cz) = lane_center(lane, rw);
        let mid = Vector3 {
            x: (a.x + b.x) * 0.5 + cx,
            y: 0.05,
            z: (a.z + b.z) * 0.5 + cz,
        };
        // Distance cull lane dashes.
        if (mid.x - cam_pos.x).abs() > 180.0 || (mid.z - cam_pos.z).abs() > 180.0 {
            continue;
        }
        d3.draw_plane(mid, Vector2::new(2.0, 0.3), neon_cyan);
    }

    // Parks: grass planes + trees + fountains/statues.
    for i in (cam_bi - radius)..=(cam_bi + radius) {
        for j in (cam_bj - radius)..=(cam_bj + radius) {
            if !city.parks.contains(&(i, j)) {
                continue;
            }
            let cx = origin + (i as f32 + 0.5) * bs;
            let cz = origin + (j as f32 + 0.5) * bs;
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
                let tree_color = match k % 3 {
                    0 => Color::new(0, 240, 255, 255),   // neon cyan tree
                    1 => Color::new(255, 0, 180, 255),   // neon magenta tree
                    _ => Color::new(255, 230, 0, 255),   // glowing neon yellow tree
                };
                d3.draw_sphere(
                    Vector3 { x: tx, y: 2.6, z: tz },
                    1.2,
                    tree_color,
                );
            }
            // Pond
            d3.draw_plane(
                Vector3 { x: cx, y: 0.045, z: cz - 2.0 },
                Vector2::new(6.0, 6.0),
                Color::new(50, 150, 220, 255),
            );
            // Statue
            d3.draw_model_ex(
                &assets.plain_cube_model,
                Vector3 { x: cx, y: 1.0, z: cz - 2.0 },
                Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 45.0,
                Vector3 { x: 1.0, y: 2.0, z: 1.0 },
                Color::new(140, 140, 145, 255),
            );
            d3.draw_sphere(
                Vector3 { x: cx, y: 2.5, z: cz - 2.0 },
                0.8,
                Color::new(200, 190, 180, 255),
            );
        }
    }

    // Ramps (bright orange wedges).
    for r in &city.ramps {
        // Distance cull ramps
        if (r.pos.x - cam_pos.x).abs() > 180.0 || (r.pos.z - cam_pos.z).abs() > 180.0 {
            continue;
        }
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

    // Buildings: textured model tinted per-building (culled by camera distance to optimize main pass)
    for b in &city.buildings {
        let c = b.box3d.center();
        let dist_sq = (c.x - cam_pos.x).powi(2) + (c.z - cam_pos.z).powi(2);
        if dist_sq > 130.0 * 130.0 {
            continue;
        }
        draw_building(d3, b, assets, &p, hour);
    }

    // Streetlights at intersection corners (culled by camera distance to optimize draw calls)
    let is_night = !(6.5..=18.5).contains(&hour);
    let bulb_color = if is_night { Color::new(255, 0, 180, 255) } else { Color::new(180, 180, 180, 255) };
    let sw_offset = rw * 0.5 + sw * 0.5;

    for i in (cam_bi - radius)..=(cam_bi + radius) {
        for j in (cam_bj - radius)..=(cam_bj + radius) {
            let cx = origin + i as f32 * bs;
            let cz = origin + j as f32 * bs;
            if (cx - cam_pos.x).abs() > 80.0 || (cz - cam_pos.z).abs() > 80.0 {
                continue;
            }
            
            // Render 2 diagonal corner streetlights per intersection
            let offsets = [
                (-sw_offset, -sw_offset),
                (sw_offset, sw_offset),
            ];
            for (ox, oz) in offsets {
                let sx = cx + ox;
                let sz = cz + oz;
                // Draw pole
                d3.draw_model_ex(
                    &assets.plain_cube_model,
                    Vector3 { x: sx, y: 2.0, z: sz },
                    Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 0.0,
                    Vector3 { x: 0.12, y: 4.0, z: 0.12 },
                    Color::new(55, 55, 60, 255),
                );
                // Arm pointing diagonally inwards to the intersection center
                let dir_x = -ox.signum() * 0.8;
                let dir_z = -oz.signum() * 0.8;
                d3.draw_model_ex(
                    &assets.plain_cube_model,
                    Vector3 { x: sx + dir_x * 0.5, y: 4.0, z: sz + dir_z * 0.5 },
                    Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 0.0,
                    Vector3 {
                        x: if dir_x.abs() > 0.01 { 1.0 } else { 0.15 },
                        y: 0.07,
                        z: if dir_z.abs() > 0.01 { 1.0 } else { 0.15 },
                    },
                    Color::new(65, 65, 70, 255),
                );
                // Warm bulb (using plain_cube_model)
                let bulb_pos = Vector3 { x: sx + dir_x, y: 3.85, z: sz + dir_z };
                d3.draw_model_ex(
                    &assets.plain_cube_model,
                    bulb_pos,
                    Vector3 { x: 0.0, y: 1.0, z: 0.0 }, 0.0,
                    Vector3 { x: 0.3, y: 0.3, z: 0.3 },
                    bulb_color,
                );
            }
        }
    }
}

fn lane_center(lane: &crate::world::city::Lane, rw: f32) -> (f32, f32) {
    let offset = rw * 0.25;
    match lane.axis {
        Axis::X => (0.0, -offset * lane.dir as f32),
        Axis::Z => (offset * lane.dir as f32, 0.0),
    }
}

fn draw_building(d3: &mut impl RaylibDraw3D, b: &Building, assets: &Assets, p: &crate::config::Palette, hour: f32) {
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
        d3.draw_model_ex(
            &assets.plain_cube_model,
            c,
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            0.0,
            Vector3 { x: w, y: hgt, z: l },
            body,
        );
    }

    // Roof cap (using plain_cube_model instead of draw_cube to avoid CPU vertex generation)
    let top = Vector3 { x: c.x, y: c.y + h.y + 0.1, z: c.z };
    d3.draw_model_ex(
        &assets.plain_cube_model,
        top,
        Vector3 { x: 0.0, y: 1.0, z: 0.0 },
        0.0,
        Vector3 { x: w * 0.9, y: 0.4, z: l * 0.9 },
        p.building_top(b.color_index),
    );
    // Edge wires (using plain_cube_model wires to avoid CPU lines generation)
    d3.draw_model_wires_ex(
        &assets.plain_cube_model,
        c,
        Vector3 { x: 0.0, y: 1.0, z: 0.0 },
        0.0,
        Vector3 { x: w, y: hgt, z: l },
        Color::new(body.r, body.g, body.b, 255),
    );

    // Floating neon holographic sign on top of tall buildings!
    if hgt > 18.0 {
        let time = hour * 12.0; // progress animation
        let time_offset = b.color_index as f32 * 2.0;
        let sign_y = top.y + 4.0 + (time + time_offset).sin() * 0.7; // float animation!
        let sign_pos = Vector3 { x: c.x, y: sign_y, z: c.z };
        let sign_color = if b.color_index % 2 == 0 {
            Color::new(0, 255, 255, 200) // Cyan hologram
        } else {
            Color::new(255, 0, 180, 200) // Magenta hologram
        };
        // Draw rotating double-nested wireframe cube/diamond
        let rotate_angle = time * 25.0 + time_offset * 45.0;
        d3.draw_model_wires_ex(
            &assets.plain_cube_model,
            sign_pos,
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            rotate_angle,
            Vector3 { x: 4.5, y: 4.5, z: 4.5 },
            sign_color,
        );
        d3.draw_model_wires_ex(
            &assets.plain_cube_model,
            sign_pos,
            Vector3 { x: 1.0, y: 0.0, z: 1.0 },
            rotate_angle * 0.6,
            Vector3 { x: 6.0, y: 6.0, z: 6.0 },
            Color::new(sign_color.r / 3, sign_color.g / 3, sign_color.b / 3, 100),
        );
    }
}

/// Draw a car body at a position with a yaw (radians) and a color.
/// Uses draw_model_ex for proper yaw rotation of the body + cabin.
#[allow(clippy::too_many_arguments)]
pub fn draw_car(
    d3: &mut impl RaylibDraw3D,
    assets: &Assets,
    lighting: &mut LightingSystem,
    pos: Vector3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    wheel_rot: f32,
    color: Color,
    damaged: f32,
    kind: VehicleKind,
    variant: VehicleVariant,
    time: f32,
) {
    let (h_val, w_rad_val) = match variant {
        VehicleVariant::Sports => (0.65, 0.38),
        VehicleVariant::SUV => (1.1, 0.52),
        VehicleVariant::Pickup => (0.9, 0.5),
        VehicleVariant::Sedan => (0.8, 0.4),
    };
    let mut pos = pos;
    pos.y += h_val * 0.5 + w_rad_val;

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

    // Color definitions based on kind and damage
    let mut body_color = if kind == VehicleKind::Police {
        Color::new(20, 20, 20, 255)
    } else {
        color
    };
    let cabin_color = if kind == VehicleKind::Police {
        Color::new(245, 245, 245, 255)
    } else {
        Color::new(60, 80, 110, 255)
    };

    if damaged > 0.0 {
        // Darken color based on damage
        let factor = (1.0 - damaged * 0.75).clamp(0.2, 1.0);
        body_color = Color::new(
            (body_color.r as f32 * factor) as u8,
            (body_color.g as f32 * factor) as u8,
            (body_color.b as f32 * factor) as u8,
            255,
        );
    }

    // Set dimensions based on variant
    let (body_w, body_h, body_l) = match variant {
        VehicleVariant::Sports => (2.05, 0.65, 4.3),
        VehicleVariant::SUV => (2.2, 1.1, 4.4),
        VehicleVariant::Pickup => (2.1, 0.9, 4.6),
        VehicleVariant::Sedan => (2.0, 0.8, 4.2),
    };

    let (cabin_w, cabin_h, cabin_l, cabin_offset_z, cabin_offset_y) = match variant {
        VehicleVariant::Sports => (1.6, 0.5, 2.3, -0.1, 0.57),
        VehicleVariant::SUV => (1.8, 0.8, 2.4, -0.3, 0.95),
        VehicleVariant::Pickup => (1.7, 0.85, 1.6, 0.6, 0.87), // Moved forward!
        VehicleVariant::Sedan => (1.6, 0.6, 2.0, -0.2, 0.7),
    };

    // Macro for drawing models with local coordinate offsets aligned to vehicle's rotation
    macro_rules! local_draw {
        ($model:expr, $local_pos:expr, $scale:expr, $tint:expr) => {
            d3.draw_model_ex(
                $model,
                vadd(pos, rotate_vector($local_pos, q)),
                axis, angle_deg,
                $scale,
                $tint,
            );
        };
    }

    // Macro for drawing models with local rotation offsets aligned to vehicle's rotation
    macro_rules! local_rot_draw {
        ($model:expr, $local_pos:expr, $q_rel:expr, $scale:expr, $tint:expr) => {
            let q_part = q * $q_rel;
            let (p_axis, p_angle) = quat_to_axis_angle(q_part);
            d3.draw_model_ex(
                $model,
                vadd(pos, rotate_vector($local_pos, q)),
                p_axis, p_angle,
                $scale,
                $tint,
            );
        };
    }

    // Cyberpunk dynamic underglow color
    let underglow_color = if kind == VehicleKind::Police {
        let is_red = (time * 8.0).sin() > 0.0;
        if is_red {
            Color::new(255, 10, 10, 180)
        } else {
            Color::new(10, 50, 255, 180)
        }
    } else {
        let mut c = body_color;
        let max_ch = c.r.max(c.g).max(c.b) as f32;
        if max_ch > 0.0 {
            c.r = ((c.r as f32 / max_ch) * 255.0) as u8;
            c.g = ((c.g as f32 / max_ch) * 255.0) as u8;
            c.b = ((c.b as f32 / max_ch) * 255.0) as u8;
        }
        Color::new(c.r, c.g, c.b, 140)
    };

    // Draw underglow plane on the ground
    let underglow_offset_y = -(h_val * 0.5 + w_rad_val) + 0.03;
    local_draw!(
        &assets.underglow_model,
        Vector3 { x: 0.0, y: underglow_offset_y, z: 0.0 },
        Vector3 { x: body_w * 1.35, y: 1.0, z: body_l * 1.15 },
        underglow_color
    );

    // 1. Draw High-Poly Split Body Components to form open wheel wells & realistic contours
    lighting.set_material_properties(0.8, 0.15, 1.0); // Next-gen metallic paint

    // Chassis frame bottom plate (dark black metal)
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: -body_h * 0.5 + 0.04, z: 0.0 },
        Vector3 { x: body_w * 0.94, y: 0.08, z: body_l * 0.96 },
        Color::new(25, 25, 25, 255)
    );

    // Front engine nose block
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: -body_h * 0.15, z: body_l * 0.38 },
        Vector3 { x: body_w * 0.96, y: body_h * 0.7, z: body_l * 0.24 },
        body_color
    );

    // Aerodynamically slanted hood cover
    let q_hood_rel = Quat {
        w: (-4.0f32.to_radians() * 0.5).cos(),
        x: (-4.0f32.to_radians() * 0.5).sin(),
        y: 0.0,
        z: 0.0,
    };
    local_rot_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: body_h * 0.22, z: body_l * 0.32 },
        q_hood_rel,
        Vector3 { x: body_w * 0.95, y: 0.03, z: body_l * 0.26 },
        body_color
    );

    // Extra detail: Carbon hood stripe for Sports variant
    if variant == VehicleVariant::Sports {
        lighting.set_material_properties(0.2, 0.5, 0.4); // Carbon material
        local_rot_draw!(
            &assets.carbon_cube_model,
            Vector3 { x: 0.0, y: body_h * 0.225, z: body_l * 0.32 },
            q_hood_rel,
            Vector3 { x: body_w * 0.25, y: 0.031, z: body_l * 0.26 },
            Color::WHITE
        );
        lighting.set_material_properties(0.8, 0.15, 1.0); // Reset to paint
    }

    // Front bumper / lower fascia
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: -body_h * 0.3, z: body_l * 0.48 },
        Vector3 { x: body_w, y: body_h * 0.4, z: 0.08 },
        body_color
    );

    // Front left & right wheel arches (fenders)
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -body_w * 0.48, y: -body_h * 0.05, z: body_l * 0.32 },
        Vector3 { x: 0.04, y: body_h * 0.9, z: body_l * 0.22 },
        body_color
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: body_w * 0.48, y: -body_h * 0.05, z: body_l * 0.32 },
        Vector3 { x: 0.04, y: body_h * 0.9, z: body_l * 0.22 },
        body_color
    );

    // Middle cabin floor frame and side skirts
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: -body_h * 0.2, z: -body_l * 0.02 },
        Vector3 { x: body_w * 0.96, y: body_h * 0.6, z: body_l * 0.44 },
        body_color
    );

    // Left & right detailed doors with handle slots
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -body_w * 0.49, y: -body_h * 0.05, z: -body_l * 0.02 },
        Vector3 { x: 0.02, y: body_h * 0.9, z: body_l * 0.42 },
        body_color
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: body_w * 0.49, y: -body_h * 0.05, z: -body_l * 0.02 },
        Vector3 { x: 0.02, y: body_h * 0.9, z: body_l * 0.42 },
        body_color
    );

    // Chrome door handles
    lighting.set_material_properties(0.9, 0.2, 1.2);
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -body_w * 0.502, y: body_h * 0.12, z: -body_l * 0.12 },
        Vector3 { x: 0.015, y: 0.025, z: 0.12 },
        Color::new(200, 200, 205, 255)
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: body_w * 0.502, y: body_h * 0.12, z: -body_l * 0.12 },
        Vector3 { x: 0.015, y: 0.025, z: 0.12 },
        Color::new(200, 200, 205, 255)
    );
    lighting.set_material_properties(0.8, 0.15, 1.0); // Reset to paint

    // Rear left & right wheel arches (fenders)
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -body_w * 0.48, y: -body_h * 0.05, z: -body_l * 0.32 },
        Vector3 { x: 0.04, y: body_h * 0.9, z: body_l * 0.22 },
        body_color
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: body_w * 0.48, y: -body_h * 0.05, z: -body_l * 0.32 },
        Vector3 { x: 0.04, y: body_h * 0.9, z: body_l * 0.22 },
        body_color
    );

    // Rear bumper & trunk deck block
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: -body_h * 0.3, z: -body_l * 0.48 },
        Vector3 { x: body_w, y: body_h * 0.4, z: 0.08 },
        body_color
    );

    // Rear trunk deck block (except Pickup)
    if variant != VehicleVariant::Pickup {
        local_draw!(
            &assets.plain_cube_model,
            Vector3 { x: 0.0, y: -body_h * 0.15, z: -body_l * 0.38 },
            Vector3 { x: body_w * 0.96, y: body_h * 0.7, z: body_l * 0.24 },
            body_color
        );

        let q_trunk_rel = Quat {
            w: (4.0f32.to_radians() * 0.5).cos(),
            x: (4.0f32.to_radians() * 0.5).sin(),
            y: 0.0,
            z: 0.0,
        };
        local_rot_draw!(
            &assets.plain_cube_model,
            Vector3 { x: 0.0, y: body_h * 0.22, z: -body_l * 0.32 },
            q_trunk_rel,
            Vector3 { x: body_w * 0.95, y: 0.03, z: body_l * 0.26 },
            body_color
        );
    }

    // 1.5 Draw Front Radiator Grille (dents slightly when damaged)
    let grille_rot = if damaged > 0.2 { (damaged - 0.2) * 15.0 } else { 0.0 };
    let grille_offset = if damaged > 0.2 {
        Vector3 { x: -0.05 * damaged, y: -0.1 * damaged, z: -0.15 * damaged }
    } else {
        Vector3::zero()
    };
    let grille_local = Vector3 { x: 0.0, y: -body_h * 0.1, z: body_l * 0.463 };
    
    lighting.set_material_properties(0.9, 0.2, 1.2); // Polished chrome grille
    let q_grille_rel = Quat {
        w: (grille_rot.to_radians() * 0.5).cos(),
        x: (grille_rot.to_radians() * 0.5).sin(),
        y: 0.0,
        z: 0.0,
    };
    local_rot_draw!(
        &assets.grill_cube_model,
        vadd(grille_local, grille_offset),
        q_grille_rel,
        Vector3 { x: body_w * 0.6, y: body_h * 0.4, z: 0.02 },
        Color::WHITE
    );

    // 2. Draw Cabin Structure (roof and window framing)
    lighting.set_material_properties(0.8, 0.15, 1.0); // Next-gen metallic paint for cabin
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: cabin_offset_y + cabin_h * 0.5, z: cabin_offset_z - cabin_l * 0.1 },
        Vector3 { x: cabin_w, y: 0.06, z: cabin_l * 0.6 },
        cabin_color
    );
    // Left A-pillar & C-pillar side frames
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -cabin_w * 0.49, y: cabin_offset_y, z: cabin_offset_z + cabin_l * 0.48 },
        Vector3 { x: 0.03, y: cabin_h, z: 0.04 },
        cabin_color
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -cabin_w * 0.49, y: cabin_offset_y, z: cabin_offset_z - cabin_l * 0.48 },
        Vector3 { x: 0.03, y: cabin_h, z: 0.04 },
        cabin_color
    );
    // Right A-pillar & C-pillar side frames
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: cabin_w * 0.49, y: cabin_offset_y, z: cabin_offset_z + cabin_l * 0.48 },
        Vector3 { x: 0.03, y: cabin_h, z: 0.04 },
        cabin_color
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: cabin_w * 0.49, y: cabin_offset_y, z: cabin_offset_z - cabin_l * 0.48 },
        Vector3 { x: 0.03, y: cabin_h, z: 0.04 },
        cabin_color
    );

    // 3. Windshield and Side Windows (with transparent glass shader + specular highlights!)
    lighting.set_material_properties(0.0, 0.05, 1.5); // High glossy reflection on glass

    // Angled Front Windshield
    let q_windshield_rel = Quat {
        w: (32.0f32.to_radians() * 0.5).cos(),
        x: (32.0f32.to_radians() * 0.5).sin(),
        y: 0.0,
        z: 0.0,
    };
    local_rot_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: cabin_offset_y, z: cabin_offset_z + cabin_l * 0.5 - 0.04 },
        q_windshield_rel,
        Vector3 { x: cabin_w * 0.94, y: 0.02, z: cabin_h * 1.3 },
        Color::new(25, 30, 45, 120) // semi-transparent window glass
    );

    // Windshield wipers
    lighting.set_material_properties(0.1, 0.6, 0.2); // matte wiper plastic
    local_rot_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -0.22, y: cabin_offset_y - cabin_h * 0.22, z: cabin_offset_z + cabin_l * 0.54 },
        q_windshield_rel,
        Vector3 { x: 0.015, y: 0.015, z: cabin_h * 0.8 },
        Color::new(10, 10, 10, 255)
    );
    local_rot_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.22, y: cabin_offset_y - cabin_h * 0.22, z: cabin_offset_z + cabin_l * 0.54 },
        q_windshield_rel,
        Vector3 { x: 0.015, y: 0.015, z: cabin_h * 0.8 },
        Color::new(10, 10, 10, 255)
    );
    lighting.set_material_properties(0.0, 0.05, 1.5); // restore glass settings

    // Angled Rear Window (except Pickup)
    if variant != VehicleVariant::Pickup {
        let q_rear_win_rel = Quat {
            w: (-32.0f32.to_radians() * 0.5).cos(),
            x: (-32.0f32.to_radians() * 0.5).sin(),
            y: 0.0,
            z: 0.0,
        };
        local_rot_draw!(
            &assets.plain_cube_model,
            Vector3 { x: 0.0, y: cabin_offset_y, z: cabin_offset_z - cabin_l * 0.4 + 0.04 },
            q_rear_win_rel,
            Vector3 { x: cabin_w * 0.94, y: 0.02, z: cabin_h * 1.3 },
            Color::new(25, 30, 45, 120)
        );
    } else {
        // Vertical Pickup cabin back window
        local_draw!(
            &assets.plain_cube_model,
            Vector3 { x: 0.0, y: cabin_offset_y, z: cabin_offset_z - cabin_l * 0.5 + 0.01 },
            Vector3 { x: cabin_w * 0.9, y: cabin_h * 0.7, z: 0.02 },
            Color::new(25, 30, 45, 120)
        );
    }

    // Left and Right Side Windows
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -cabin_w * 0.5, y: cabin_offset_y - 0.02, z: cabin_offset_z },
        Vector3 { x: 0.02, y: cabin_h * 0.75, z: cabin_l * 0.7 },
        Color::new(25, 30, 45, 120)
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: cabin_w * 0.5, y: cabin_offset_y - 0.02, z: cabin_offset_z },
        Vector3 { x: 0.02, y: cabin_h * 0.75, z: cabin_l * 0.7 },
        Color::new(25, 30, 45, 120)
    );

    // 3.5. Draw Detailed Interior (visible inside the transparent glass!)
    // Main dashboard body (matte plastic structure)
    lighting.set_material_properties(0.1, 0.7, 0.15);
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.0, y: cabin_offset_y - 0.12, z: cabin_offset_z + cabin_l * 0.44 },
        Vector3 { x: cabin_w * 0.92, y: 0.16, z: 0.28 },
        Color::new(22, 22, 24, 255)
    );

    // Glowing instrument panel (rotated by 180 deg around Y-axis so the texture faces the driver correctly)
    let q_inst_rel = Quat {
        w: 0.0,
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };
    lighting.set_material_properties(0.5, 0.15, 1.4); // highly glossy, glowing material
    local_rot_draw!(
        &assets.dash_model,
        Vector3 { x: -0.32, y: cabin_offset_y - 0.08, z: cabin_offset_z + cabin_l * 0.44 - 0.138 },
        q_inst_rel,
        Vector3 { x: 0.32, y: 0.08, z: 0.015 },
        Color::WHITE
    );

    // Steering Column
    lighting.set_material_properties(0.1, 0.7, 0.1);
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -0.32, y: cabin_offset_y - 0.14, z: cabin_offset_z + cabin_l * 0.35 },
        Vector3 { x: 0.04, y: 0.04, z: 0.3 },
        Color::new(15, 15, 15, 255)
    );

    // Steering Wheel (slanted cylinder model)
    lighting.set_material_properties(0.4, 0.3, 0.8);
    let q_wheel_tilt = Quat {
        w: (-20.0f32.to_radians() * 0.5).cos(),
        x: (-20.0f32.to_radians() * 0.5).sin(),
        y: 0.0,
        z: 0.0,
    };
    local_rot_draw!(
        &assets.cylinder_model,
        Vector3 { x: -0.32, y: cabin_offset_y - 0.06, z: cabin_offset_z + cabin_l * 0.24 },
        q_wheel_tilt,
        Vector3 { x: 0.24, y: 0.04, z: 0.24 },
        Color::new(30, 30, 35, 255)
    );

    // Front L-shaped Leather Seats (Driver left, Passenger right)
    lighting.set_material_properties(0.1, 0.8, 0.2); // matte leather
    // Driver seat
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -0.32, y: cabin_offset_y - 0.22, z: cabin_offset_z + cabin_l * 0.05 },
        Vector3 { x: 0.44, y: 0.1, z: 0.44 },
        Color::new(35, 35, 38, 255)
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: -0.32, y: cabin_offset_y - 0.02, z: cabin_offset_z - 0.12 },
        Vector3 { x: 0.44, y: 0.54, z: 0.1 },
        Color::new(35, 35, 38, 255)
    );
    // Passenger seat
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.32, y: cabin_offset_y - 0.22, z: cabin_offset_z + cabin_l * 0.05 },
        Vector3 { x: 0.44, y: 0.1, z: 0.44 },
        Color::new(35, 35, 38, 255)
    );
    local_draw!(
        &assets.plain_cube_model,
        Vector3 { x: 0.32, y: cabin_offset_y - 0.02, z: cabin_offset_z - 0.12 },
        Vector3 { x: 0.44, y: 0.54, z: 0.1 },
        Color::new(35, 35, 38, 255)
    );

    // 4. Variant-Specific High-Poly Detail Models
    match variant {
        VehicleVariant::Sports => {
            // Rear spoiler wing columns
            let spoiler_z = -body_l * 0.46;
            let col_y = body_h * 0.5 + 0.15;
            let left_col = Vector3 { x: -body_w * 0.4, y: col_y, z: spoiler_z };
            
            lighting.set_material_properties(0.8, 0.15, 1.0); // Paint for spoiler columns
            local_draw!(&assets.plain_cube_model, left_col, Vector3 { x: 0.08, y: 0.3, z: 0.08 }, body_color);
            let right_col = Vector3 { x: body_w * 0.4, y: col_y, z: spoiler_z };
            local_draw!(&assets.plain_cube_model, right_col, Vector3 { x: 0.08, y: 0.3, z: 0.08 }, body_color);
            
            // Spoiler wing bar (textured with carbon fiber, tilts/hangs loose when damaged)
            let wing_rot = if damaged > 0.3 { (damaged - 0.3) * 25.0 } else { 0.0 };
            let wing_offset = if damaged > 0.3 {
                Vector3 { x: 0.0, y: -0.15 * (damaged - 0.3), z: -0.05 * (damaged - 0.3) }
            } else {
                Vector3::zero()
            };
            let wing_local = Vector3 { x: 0.0, y: body_h * 0.5 + 0.3, z: spoiler_z };
            
            lighting.set_material_properties(0.2, 0.5, 0.4); // Carbon fiber spoiler
            let q_wing_rel = Quat {
                w: (wing_rot.to_radians() * 0.5).cos(),
                x: (wing_rot.to_radians() * 0.5).sin(),
                y: 0.0,
                z: 0.0,
            };
            local_rot_draw!(
                &assets.carbon_cube_model,
                vadd(wing_local, wing_offset),
                q_wing_rel,
                Vector3 { x: body_w * 1.05, y: 0.06, z: 0.35 },
                Color::WHITE
            );
            
            // Double exhaust pipes at back (using cylinder model for shadows + chrome reflection!)
            lighting.set_material_properties(0.95, 0.1, 1.5); // Highly polished chrome
            let q_exhaust_rel = Quat {
                w: (std::f32::consts::FRAC_PI_4).cos(),
                x: (std::f32::consts::FRAC_PI_4).sin(),
                y: 0.0,
                z: 0.0,
            };
            let ex_z = -body_l * 0.5;
            let ex_left = Vector3 { x: -0.4, y: -body_h * 0.4, z: ex_z - 0.1 };
            local_rot_draw!(&assets.cylinder_model, ex_left, q_exhaust_rel, Vector3 { x: 0.16, y: 0.2, z: 0.16 }, Color::new(180, 180, 180, 255));
            let ex_right = Vector3 { x: 0.4, y: -body_h * 0.4, z: ex_z - 0.1 };
            local_rot_draw!(&assets.cylinder_model, ex_right, q_exhaust_rel, Vector3 { x: 0.16, y: 0.2, z: 0.16 }, Color::new(180, 180, 180, 255));
        }
        VehicleVariant::SUV => {
            // Roof rack rails
            let rail_y = cabin_offset_y + cabin_h * 0.5 + 0.05;
            lighting.set_material_properties(0.6, 0.3, 0.8);
            local_draw!(&assets.plain_cube_model, Vector3 { x: -cabin_w * 0.45, y: rail_y, z: cabin_offset_z }, Vector3 { x: 0.06, y: 0.06, z: cabin_l * 0.9 }, Color::new(30, 30, 30, 255));
            local_draw!(&assets.plain_cube_model, Vector3 { x: cabin_w * 0.45, y: rail_y, z: cabin_offset_z }, Vector3 { x: 0.06, y: 0.06, z: cabin_l * 0.9 }, Color::new(30, 30, 30, 255));
            
            // Spare tire on the back trunk
            let tire_local = Vector3 { x: 0.0, y: 0.1, z: -body_l * 0.5 - 0.1 };
            let q_exhaust_rel = Quat {
                w: (std::f32::consts::FRAC_PI_4).cos(),
                x: (std::f32::consts::FRAC_PI_4).sin(),
                y: 0.0,
                z: 0.0,
            };
            lighting.set_material_properties(0.0, 0.9, 0.1);
            local_rot_draw!(&assets.tire_model, tire_local, q_exhaust_rel, Vector3 { x: 0.9, y: 0.24, z: 0.9 }, Color::new(25, 25, 25, 255));
            lighting.set_material_properties(0.9, 0.2, 1.2);
            local_rot_draw!(&assets.cylinder_model, vadd(tire_local, Vector3 { x: 0.0, y: 0.0, z: 0.12 }), q_exhaust_rel, Vector3 { x: 0.5, y: 0.02, z: 0.5 }, Color::new(160, 160, 160, 255));
        }
        VehicleVariant::Pickup => {
            lighting.set_material_properties(0.8, 0.15, 1.0);
            local_draw!(&assets.plain_cube_model, Vector3 { x: -body_w * 0.46, y: body_h * 0.5 + 0.35, z: -1.0 }, Vector3 { x: 0.1, y: 0.7, z: 2.6 }, body_color);
            local_draw!(&assets.plain_cube_model, Vector3 { x: body_w * 0.46, y: body_h * 0.5 + 0.35, z: -1.0 }, Vector3 { x: 0.1, y: 0.7, z: 2.6 }, body_color);
            local_draw!(&assets.plain_cube_model, Vector3 { x: 0.0, y: body_h * 0.5 + 0.35, z: -2.3 }, Vector3 { x: body_w * 0.9, y: 0.7, z: 0.1 }, body_color);
            lighting.set_material_properties(0.1, 0.8, 0.2);
            local_draw!(&assets.plain_cube_model, Vector3 { x: 0.0, y: 0.05, z: -1.0 }, Vector3 { x: body_w * 0.9, y: 0.1, z: 2.6 }, Color::new(40, 40, 40, 255));
        }
        _ => {}
    }

    // 5. Draw Police Light Bar / Siren domes
    if kind == VehicleKind::Police {
        let bar_local = Vector3 { x: 0.0, y: cabin_offset_y + cabin_h * 0.5 + 0.05, z: cabin_offset_z };
        
        lighting.set_material_properties(0.1, 0.5, 0.5); // Plastic siren mount base
        local_draw!(
            &assets.plain_cube_model,
            bar_local,
            Vector3 { x: 1.2, y: 0.1, z: 0.25 },
            Color::new(30, 30, 30, 255)
        );

        let red_flash = (time * 12.0).sin() > 0.0;
        let left_color = if red_flash { Color::new(255, 30, 30, 255) } else { Color::new(50, 0, 0, 255) };
        let right_color = if !red_flash { Color::new(30, 30, 255, 255) } else { Color::new(0, 0, 50, 255) };

        lighting.set_material_properties(0.0, 0.05, 1.5); // Glossy glowing siren dome glass
        let left_local = Vector3 { x: -0.35, y: cabin_offset_y + cabin_h * 0.5 + 0.15, z: cabin_offset_z };
        local_draw!(
            &assets.sphere_model,
            left_local,
            Vector3 { x: 0.3, y: 0.12, z: 0.2 },
            left_color
        );

        let right_local = Vector3 { x: 0.35, y: cabin_offset_y + cabin_h * 0.5 + 0.15, z: cabin_offset_z };
        local_draw!(
            &assets.sphere_model,
            right_local,
            Vector3 { x: 0.3, y: 0.12, z: 0.2 },
            right_color
        );
    }

    // 6. Draw Wheels with metal rims and spokes
    let (w_rad, w_width) = match variant {
        VehicleVariant::Sports => (0.38, 0.35),
        VehicleVariant::SUV => (0.52, 0.36),
        VehicleVariant::Pickup => (0.5, 0.34),
        VehicleVariant::Sedan => (0.4, 0.3),
    };

    let wheel_local_offsets = [
        Vector3 { x: body_w * 0.5, y: -body_h * 0.5, z: body_l * 0.32 },
        Vector3 { x: -body_w * 0.5, y: -body_h * 0.5, z: body_l * 0.32 },
        Vector3 { x: body_w * 0.5, y: -body_h * 0.5, z: -body_l * 0.32 },
        Vector3 { x: -body_w * 0.5, y: -body_h * 0.5, z: -body_l * 0.32 },
    ];
    
    let q_wheel_rel = Quat {
        w: (std::f32::consts::FRAC_PI_4).cos(),
        x: 0.0,
        y: 0.0,
        z: (std::f32::consts::FRAC_PI_4).sin(),
    };
    let (w_axis, w_angle) = quat_to_axis_angle(q * q_wheel_rel);

    for off in wheel_local_offsets {
        let wheel_center = vadd(pos, rotate_vector(off, q));
        
        // Draw black tire with tread texture
        lighting.set_material_properties(0.0, 0.9, 0.1);
        d3.draw_model_ex(
            &assets.tire_model,
            wheel_center,
            w_axis, w_angle,
            Vector3 { x: w_rad * 2.0, y: w_width, z: w_rad * 2.0 },
            Color::new(25, 25, 25, 255),
        );

        // Draw metal rim inside the tire outer face (shiny metal chrome)
        lighting.set_material_properties(0.9, 0.2, 1.2);
        let rim_rad = w_rad * 0.65;
        let rim_width = 0.04;
        let rim_offset_dist = w_width * 0.5 - rim_width * 0.5;
        let rim_center = if off.x < 0.0 {
            vadd(wheel_center, rotate_vector(Vector3 { x: -rim_offset_dist, y: 0.0, z: 0.0 }, q))
        } else {
            vadd(wheel_center, rotate_vector(Vector3 { x: rim_offset_dist, y: 0.0, z: 0.0 }, q))
        };
        // Rim rotates around local axle (Y-axis of cylinder)
        let q_rim_rot = Quat {
            w: (wheel_rot * 0.5).cos(),
            x: 0.0,
            y: (wheel_rot * 0.5).sin(),
            z: 0.0,
        };
        let q_rim = q * q_wheel_rel * q_rim_rot;
        let (rim_axis, rim_angle) = quat_to_axis_angle(q_rim);
        d3.draw_model_ex(
            &assets.cylinder_model,
            rim_center,
            rim_axis, rim_angle,
            Vector3 { x: rim_rad * 2.0, y: rim_width, z: rim_rad * 2.0 },
            Color::new(200, 200, 205, 255),
        );

        // Draw red brake caliper (does not rotate with wheel, only with body q!)
        lighting.set_material_properties(0.5, 0.3, 0.8); // painted brake caliper
        let caliper_offset_local = if off.x < 0.0 {
            Vector3 { x: -rim_offset_dist + 0.01, y: rim_rad * 0.6 * 0.707, z: -rim_rad * 0.6 * 0.707 }
        } else {
            Vector3 { x: rim_offset_dist - 0.01, y: rim_rad * 0.6 * 0.707, z: -rim_rad * 0.6 * 0.707 }
        };
        let caliper_world = vadd(wheel_center, rotate_vector(caliper_offset_local, q));
        d3.draw_model_ex(
            &assets.plain_cube_model,
            caliper_world,
            axis, angle_deg,
            Vector3 { x: 0.03, y: rim_rad * 0.35, z: rim_rad * 0.45 },
            Color::new(220, 20, 20, 255), // Brembo red!
        );

        // Draw 5 metal spokes inside the rim outer face
        lighting.set_material_properties(0.95, 0.15, 1.3); // Chrome spokes
        for i in 0..5 {
            let angle_rad = (i as f32 * 72.0).to_radians() + wheel_rot;
            let q_spoke_rel = Quat {
                w: (angle_rad * 0.5).cos(),
                x: 0.0,
                y: (angle_rad * 0.5).sin(), // rotate around local cylinder Y-axis (axle)
                z: 0.0,
            };
            
            // Spoke extends along local X-axis. Center offset by half length.
            let spoke_local = Vector3 { x: rim_rad * 0.4, y: 0.0, z: 0.0 };
            let rotated_spoke_local = rotate_vector(spoke_local, q_spoke_rel);
            let spoke_center = vadd(rim_center, rotate_vector(rotated_spoke_local, q * q_wheel_rel));
            
            let q_spoke = q * q_wheel_rel * q_spoke_rel;
            let (sp_axis, sp_angle) = quat_to_axis_angle(q_spoke);
            
            d3.draw_model_ex(
                &assets.plain_cube_model,
                spoke_center,
                sp_axis, sp_angle,
                Vector3 { x: rim_rad * 0.8, y: 0.015, z: 0.035 }, // scale along spoke length (X), thin spoke thickness
                Color::new(200, 200, 205, 255),
            );
        }
    }

    // 7. Headlights & Taillights (deform and flicker when damaged) - using sphere model for correct shader lighting!
    let mut hl_left_off = Vector3 { x: -body_w * 0.38, y: -body_h * 0.12, z: body_l * 0.50 + 0.01 };
    let mut hl_right_off = Vector3 { x: body_w * 0.38, y: -body_h * 0.12, z: body_l * 0.50 + 0.01 };
    let mut tl_left_off = Vector3 { x: -body_w * 0.38, y: -body_h * 0.12, z: -body_l * 0.50 - 0.01 };
    let mut tl_right_off = Vector3 { x: body_w * 0.38, y: -body_h * 0.12, z: -body_l * 0.50 - 0.01 };

    if damaged > 0.3 {
        hl_left_off.y -= damaged * 0.2; // left headlight droops
        hl_left_off.z -= damaged * 0.1;
        hl_right_off.y -= damaged * 0.15; // right headlight droops
        hl_right_off.x += damaged * 0.08;
        tl_left_off.y -= damaged * 0.1;  // left taillight skewed
        tl_right_off.y -= damaged * 0.12; // right taillight skewed
    }
    
    // Front Headlights
    lighting.set_material_properties(0.0, 0.05, 1.5); // Shiny glass covers
    let show_hl_left = damaged < 0.7 || (time * 18.0).sin() > 0.0;
    if show_hl_left {
        local_draw!(&assets.headlight_model, hl_left_off, Vector3 { x: 0.24, y: 0.2, z: 0.02 }, Color::WHITE);
    }
    let show_hl_right = damaged < 0.8 || (time * 22.0).sin() > 0.0;
    if show_hl_right {
        local_draw!(&assets.headlight_model, hl_right_off, Vector3 { x: 0.24, y: 0.2, z: 0.02 }, Color::WHITE);
    }

    // Rear Taillights
    local_draw!(&assets.taillight_model, tl_left_off, Vector3 { x: 0.24, y: 0.2, z: 0.02 }, Color::WHITE);
    local_draw!(&assets.taillight_model, tl_right_off, Vector3 { x: 0.24, y: 0.2, z: 0.02 }, Color::WHITE);

    // Front & Rear License Plates
    local_draw!(
        &assets.plate_model,
        Vector3 { x: 0.0, y: -body_h * 0.3, z: body_l * 0.52 + 0.01 },
        Vector3 { x: 0.45, y: 0.2, z: 0.02 },
        Color::WHITE
    );
    local_draw!(
        &assets.plate_model,
        Vector3 { x: 0.0, y: -body_h * 0.3, z: -body_l * 0.52 - 0.01 },
        Vector3 { x: 0.45, y: 0.2, z: 0.02 },
        Color::WHITE
    );

    // Damage smoke wires
    if damaged > 0.4 {
        lighting.set_material_properties(0.0, 0.8, 0.2); // Non-metal rough soot/damage mesh
        d3.draw_model_wires_ex(
            &assets.plain_cube_model,
            pos,
            axis, angle_deg,
            Vector3 { x: body_w + 0.05, y: body_h + 0.05, z: body_l + 0.05 },
            Color::new(60, 40, 30, 255),
        );
    }

    // Restore default material properties for other rendering in the frame
    lighting.set_material_properties(0.0, 0.8, 0.15);
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
    assets: &Assets,
    _cfg: &Config,
    vehicles: &[Vehicle],
    peds: &[Ped],
    cops: &[Cop],
    player: &Player,
) {
    let play_pos = player.pos;

    // Buildings.
    for b in &city.buildings {
        let c = b.box3d.center();
        
        // Distance culling: Skip buildings further than 90m from the player.
        // The shadow map only covers a 120m volume centered around the player,
        // so buildings further away cannot cast visible shadows in the view.
        let dist_sq = (c.x - play_pos.x).powi(2) + (c.z - play_pos.z).powi(2);
        if dist_sq > 90.0 * 90.0 {
            continue;
        }

        let h = b.box3d.half();
        // Use plain_cube_model instead of immediate-mode draw_cube to avoid CPU vertex generation!
        d3.draw_model_ex(
            &assets.plain_cube_model,
            c,
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            0.0,
            Vector3 { x: h.x * 2.0, y: h.y * 2.0, z: h.z * 2.0 },
            Color::WHITE,
        );
    }
    // Vehicles (simple boxes for shadow).
    for v in vehicles {
        if v.destroyed {
            continue;
        }
        let dist_sq = (v.pos.x - play_pos.x).powi(2) + (v.pos.z - play_pos.z).powi(2);
        if dist_sq > 90.0 * 90.0 {
            continue;
        }
        d3.draw_model_ex(
            &assets.plain_cube_model,
            v.pos,
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            v.yaw.to_degrees(),
            Vector3 { x: 2.0, y: 0.8, z: 4.2 },
            Color::WHITE,
        );
    }
    // Characters (simple boxes for shadow).
    for ped in peds {
        if ped.dead() {
            continue;
        }
        let dist_sq = (ped.pos.x - play_pos.x).powi(2) + (ped.pos.z - play_pos.z).powi(2);
        if dist_sq > 90.0 * 90.0 {
            continue;
        }
        d3.draw_model_ex(
            &assets.plain_cube_model,
            Vector3 { x: ped.pos.x, y: ped.pos.y + 0.9, z: ped.pos.z },
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            ped.yaw.to_degrees(),
            Vector3 { x: 0.4, y: 1.8, z: 0.4 },
            Color::WHITE,
        );
    }
    for cop in cops {
        if cop.dead() {
            continue;
        }
        let dist_sq = (cop.pos.x - play_pos.x).powi(2) + (cop.pos.z - play_pos.z).powi(2);
        if dist_sq > 90.0 * 90.0 {
            continue;
        }
        d3.draw_model_ex(
            &assets.plain_cube_model,
            Vector3 { x: cop.pos.x, y: cop.pos.y + 0.9, z: cop.pos.z },
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            cop.yaw.to_degrees(),
            Vector3 { x: 0.4, y: 1.8, z: 0.4 },
            Color::WHITE,
        );
    }
    // Player.
    if player.alive {
        d3.draw_model_ex(
            &assets.plain_cube_model,
            Vector3 { x: player.pos.x, y: player.pos.y + 0.9, z: player.pos.z },
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            player.yaw.to_degrees(),
            Vector3 { x: 0.4, y: 1.8, z: 0.4 },
            Color::WHITE,
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
