//! HUD: health/armor bars, money, wanted stars, weapon/ammo, minimap, mission banner.
use raylib::prelude::*;
use raylib::ffi::Vector3;
use crate::player::Player;
use crate::wanted::WantedSystem;
use crate::mission::MissionState;
use crate::config::Config;
use crate::world::city::City;
use crate::vehicle::Vehicle;

#[allow(clippy::too_many_arguments)]
pub fn draw_hud(
    d: &mut RaylibDrawHandle,
    player: &Player,
    wanted: &WantedSystem,
    mission: &MissionState,
    vehicles: &[Vehicle],
    city: &City,
    cfg: &Config,
    cam_pos: Vector3,
    cam_yaw: f32,
    assets: &crate::render::models::Assets,
    rate_label: &str,
    debug: bool,
    fps: i32,
) {
    let sw = d.get_screen_width();
    let sh = d.get_screen_height();

    // --- Crosshair (when armed) ---
    if player.weapon != crate::player::Weapon::Unarmed && player.in_vehicle.is_none() {
        let cx = sw / 2;
        let cy = sh / 2;
        d.draw_line(cx - 8, cy, cx - 3, cy, Color::new(255, 255, 255, 200));
        d.draw_line(cx + 3, cy, cx + 8, cy, Color::new(255, 255, 255, 200));
        d.draw_line(cx, cy - 8, cx, cy - 3, Color::new(255, 255, 255, 200));
        d.draw_line(cx, cy + 3, cx, cy + 8, Color::new(255, 255, 255, 200));
        d.draw_circle(cx, cy, 1.0, Color::new(255, 255, 255, 200));
    }

    // --- Top-right: money + wanted stars + weapon ---
    let money_text = format!("${}", player.money);
    d.draw_text(&money_text, sw - 140, 16, 28, Color::new(80, 220, 80, 255));

    // Wanted stars
    for i in 0..6u8 {
        let x = sw - 140 + (i as i32) * 22;
        let y = 52;
        let active = i < wanted.stars;
        let color = if active {
            Color::new(255, 220, 60, 255)
        } else {
            Color::new(80, 80, 80, 180)
        };
        draw_star(d, x + 8, y + 8, 8, color);
    }

    // Weapon + ammo
    let weapon_text = if player.weapon == crate::player::Weapon::Unarmed {
        "Fists".to_string()
    } else if player.reloading > 0.0 {
        format!("{} RELOADING", player.weapon.name())
    } else {
        format!("{} {}/{}", player.weapon.name(), player.ammo, player.reserve)
    };
    d.draw_text(&weapon_text, sw - 140, 78, 20, Color::new(220, 220, 220, 255));

    // --- Bottom-left: health + armor bars ---
    let bar_x = 20;
    let bar_y = sh - 60;
    let bar_w = 200;
    let bar_h = 18;
    // Health
    d.draw_rectangle(bar_x, bar_y, bar_w, bar_h, Color::new(30, 30, 30, 200));
    let hp_w = ((player.health / 100.0) * bar_w as f32) as i32;
    let hp_color = if player.health > 30.0 {
        Color::new(60, 200, 60, 255)
    } else {
        Color::new(220, 60, 60, 255)
    };
    d.draw_rectangle(bar_x, bar_y, hp_w, bar_h, hp_color);
    d.draw_rectangle_lines(bar_x, bar_y, bar_w, bar_h, Color::new(200, 200, 200, 255));
    d.draw_text("HP", bar_x + 4, bar_y + 1, 14, Color::new(255, 255, 255, 230));

    // Armor
    d.draw_rectangle(bar_x, bar_y + 24, bar_w, bar_h, Color::new(30, 30, 30, 200));
    let ar_w = ((player.armor / 100.0) * bar_w as f32) as i32;
    d.draw_rectangle(bar_x, bar_y + 24, ar_w, bar_h, Color::new(60, 100, 220, 255));
    d.draw_rectangle_lines(bar_x, bar_y + 24, bar_w, bar_h, Color::new(200, 200, 200, 255));
    d.draw_text("AR", bar_x + 4, bar_y + 25, 14, Color::new(255, 255, 255, 230));

    // --- Bottom-right: minimap ---
    let mm_size = 160;
    let mm_x = sw - mm_size - 20;
    let mm_y = sh - mm_size - 20;
    draw_minimap(d, city, cam_pos, cam_yaw, vehicles, player, mm_x, mm_y, mm_size, cfg);

    // --- Mission banner (top-center) ---
    if mission.banner_timer > 0.0 && !mission.banner.is_empty() {
        let text = &mission.banner;
        let tw = d.measure_text(text, 22);
        let bx = sw / 2 - tw / 2 - 12;
        let by = 90;
        let alpha = if mission.banner_timer > 0.5 { 255 } else {
            (mission.banner_timer / 0.5 * 255.0) as u8
        };
        d.draw_rectangle(bx, by, tw + 24, 34, Color::new(0, 0, 0, alpha));
        d.draw_text(text, bx + 12, by + 6, 22, Color::new(255, 255, 100, alpha));
    }

    // --- Dead overlay ---
    if !player.alive {
        let msg = if player.respawn_timer > 0.0 {
            format!("WASTED  --  respawning in {:.1}s", player.respawn_timer)
        } else {
            "WASTED".to_string()
        };
        let tw = d.measure_text(&msg, 36);
        d.draw_text(&msg, sw / 2 - tw / 2, sh / 2 - 20, 36, Color::new(220, 40, 40, 255));
    }

    // --- Debug overlay ---
    if debug {
        let lines = [
            format!("FPS: {}", fps),
            format!("Logic: {}", rate_label),
            format!("Pos: ({:.1}, {:.1}, {:.1})", player.pos.x, player.pos.y, player.pos.z),
            format!("Health: {:.0}  Armor: {:.0}", player.health, player.armor),
            format!("Wanted: {} stars ({:.1} heat)", wanted.stars, wanted.heat),
            format!("Peds/Cops/Vehicles: _/_/{}", vehicles.len()),
        ];
        for (i, line) in lines.iter().enumerate() {
            d.draw_text(line, 20, 20 + (i * 18) as i32, 16, Color::new(255, 255, 0, 220));
        }
    }

    // --- Controls hint (bottom-center, first few seconds) ---
    let _ = assets; // textures available for future minimap texture use
}

fn draw_star(d: &mut RaylibDrawHandle, cx: i32, cy: i32, r: i32, color: Color) {
    // Simple 5-point star using two triangles + circle center.
    d.draw_circle(cx, cy, r as f32, color);
    d.draw_circle(cx, cy, (r as f32) * 0.5, Color::new(color.r / 2, color.g / 2, color.b / 2, 255));
}

#[allow(clippy::too_many_arguments)]
fn draw_minimap(
    d: &mut RaylibDrawHandle,
    city: &City,
    cam_pos: Vector3,
    cam_yaw: f32,
    vehicles: &[Vehicle],
    player: &Player,
    mx: i32,
    my: i32,
    size: i32,
    cfg: &Config,
) {
    // Background
    d.draw_rectangle(mx, my, size, size, Color::new(20, 20, 25, 220));
    d.draw_rectangle_lines(mx, my, size, size, Color::new(200, 200, 200, 255));

    let scale = size as f32 / (cfg.block_size * 6.0); // show ~6 blocks around player
    let cx = mx + size / 2;
    let cy = my + size / 2;

    // Draw roads as lines along grid
    let n = city.blocks;
    let bs = city.block_size;
    let origin = -city.ground_half;
    let road_col = Color::new(80, 80, 90, 255);

    for i in 0..=n {
        let line = origin + i as f32 * bs;
        // World->minimap: relative to cam, rotated by -cam_yaw, scaled
        let rel = line - cam_pos.z;
        let screen_rel = rel * scale;
        if screen_rel.abs() > size as f32 / 2.0 + 10.0 {
            continue;
        }
        d.draw_line(mx, cy + screen_rel as i32, mx + size, cy + screen_rel as i32, road_col);
    }
    for i in 0..=n {
        let line = origin + i as f32 * bs;
        let rel = line - cam_pos.x;
        let screen_rel = rel * scale;
        if screen_rel.abs() > size as f32 / 2.0 + 10.0 {
            continue;
        }
        d.draw_line(cx + screen_rel as i32, my, cx + screen_rel as i32, my + size, road_col);
    }

    // Vehicle blips (yellow)
    for v in vehicles {
        if v.destroyed {
            continue;
        }
        let dx = (v.pos.x - cam_pos.x) * scale;
        let dz = (v.pos.z - cam_pos.z) * scale;
        let px = cx + dx as i32;
        let py = cy + dz as i32;
        if px >= mx && px < mx + size && py >= my && py < my + size {
            let col = if v.kind == crate::vehicle::VehicleKind::Police {
                Color::new(80, 120, 255, 255)
            } else {
                Color::new(200, 200, 80, 255)
            };
            d.draw_rectangle(px - 2, py - 2, 4, 4, col);
        }
    }

    // Player arrow (center, pointing in yaw direction)
    let yaw = if let Some(vi) = player.in_vehicle {
        vehicles[vi].yaw
    } else {
        player.yaw
    };
    let (sx, sz) = (yaw.sin(), yaw.cos());
    let ax = cx as f32 + sx * 8.0;
    let ay = cy as f32 + sz * 8.0;
    let _ = cam_yaw;
    d.draw_triangle(
        Vector2::new(ax, ay),
        Vector2::new(cx as f32 - sx * 4.0 + sz * 4.0, cy as f32 - sz * 4.0 - sx * 4.0),
        Vector2::new(cx as f32 - sx * 4.0 - sz * 4.0, cy as f32 - sz * 4.0 + sx * 4.0),
        Color::new(80, 220, 80, 255),
    );

    // Mission marker blip (pink)
    // (Handled by caller if mission active)
}
