//! Game state: owns all entities, runs logic steps, renders the scene.
use raylib::prelude::*;
use raylib::ffi::Vector3;

use crate::config::Config;
use crate::input::Input;
use crate::mathx::*;
use crate::world::city::City;
use crate::player::{Player, Weapon};
use crate::vehicle::{Vehicle, VehicleKind};
use crate::camera::FollowCamera;
use crate::combat::{fire_weapon, melee_attack, cop_fire, HitKind};
use crate::wanted::WantedSystem;
use crate::ai::ped::Ped;
use crate::ai::cop::Cop;
use crate::ai::traffic::{TrafficCar, spawn_traffic};
use crate::pickup::{Pickup, Shop, ShopKind};
use crate::mission::MissionState;
use crate::render::models::{Assets, draw_world, draw_car, draw_character, draw_pickup, draw_mission_marker};
use crate::render::fx::Fx;
use crate::hud;

pub struct Game {
    pub cfg: Config,
    pub city: City,
    pub assets: Assets,
    pub player: Player,
    pub vehicles: Vec<Vehicle>,
    pub peds: Vec<Ped>,
    pub cops: Vec<Cop>,
    pub traffic: Vec<TrafficCar>,
    pub pickups: Vec<Pickup>,
    pub shops: Vec<Shop>,
    pub wanted: WantedSystem,
    pub mission: MissionState,
    pub fx: Fx,
    pub camera: FollowCamera,
    pub time: f32,
    pub panic_pos: Option<Vector3>, // last gunfire position (for ped panic)
    pub mission_target_idx: Option<usize>, // index into peds for kill mission
}

impl Game {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: Config) -> Self {
        let city = City::generate(&cfg);
        let assets = Assets::load(rl, thread, &cfg);

        // Player at center on a road.
        let player_pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        let player = Player::new(player_pos);

        let mut vehicles = Vec::new();
        let mut traffic = Vec::new();

        // Spawn some traffic.
        for _ in 0..cfg.max_traffic {
            spawn_traffic(&city, &mut vehicles, &mut traffic);
        }

        // Spawn a few parked cars near player for quick access.
        for i in 0..3 {
            let angle = i as f32 * 2.1;
            let pos = Vector3 {
                x: angle.cos() * 8.0,
                y: 0.0,
                z: angle.sin() * 8.0,
            };
            let colors = [
                Color::new(200, 60, 60, 255),
                Color::new(60, 120, 200, 255),
                Color::new(220, 220, 220, 255),
            ];
            vehicles.push(Vehicle::new(pos, angle, colors[i], VehicleKind::Civilian));
        }

        // Spawn pedestrians.
        let mut peds = Vec::new();
        let ped_colors = [
            Color::new(180, 120, 80, 255),
            Color::new(120, 160, 180, 255),
            Color::new(200, 180, 100, 255),
            Color::new(160, 100, 160, 255),
            Color::new(100, 200, 120, 255),
        ];
        for _ in 0..cfg.max_peds {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let dist = rand::random::<f32>() * 80.0 + 10.0;
            let pos = Vector3 {
                x: (angle.cos() * dist).clamp(-cfg.world_half() + 5.0, cfg.world_half() - 5.0),
                y: 0.0,
                z: (angle.sin() * dist).clamp(-cfg.world_half() + 5.0, cfg.world_half() - 5.0),
            };
            let col = ped_colors[rand::random::<usize>() % ped_colors.len()];
            peds.push(Ped::new(pos, col));
        }

        // Pickups: health, armor, weapon scattered around.
        let mut pickups = Vec::new();
        for i in 0..6 {
            let angle = i as f32 * 1.05;
            let dist = 30.0 + i as f32 * 8.0;
            let pos = Vector3 {
                x: (angle.cos() * dist).clamp(-cfg.world_half() + 5.0, cfg.world_half() - 5.0),
                y: 0.0,
                z: (angle.sin() * dist).clamp(-cfg.world_half() + 5.0, cfg.world_half() - 5.0),
            };
            match i % 4 {
                0 => pickups.push(Pickup::health(pos)),
                1 => pickups.push(Pickup::armor(pos)),
                2 => pickups.push(Pickup::money(pos, 200)),
                _ => pickups.push(Pickup::weapon(pos, Weapon::Smg)),
            }
        }

        // Shops: weapon + health at fixed locations.
        let shops = vec![
            Shop::new(Vector3 { x: cfg.world_half() * 0.5, y: 0.0, z: cfg.world_half() * 0.5 }, ShopKind::Weapon),
            Shop::new(Vector3 { x: -cfg.world_half() * 0.5, y: 0.0, z: -cfg.world_half() * 0.5 }, ShopKind::Health),
        ];

        // Start first mission.
        let mut mission = MissionState::new();
        mission.start_new(player_pos, cfg.world_half());

        Game {
            cfg,
            city,
            assets,
            player,
            vehicles,
            peds,
            cops: Vec::new(),
            traffic,
            pickups,
            shops,
            wanted: WantedSystem::new(),
            mission,
            fx: Fx::new(),
            camera: FollowCamera::new(),
            time: 0.0,
            panic_pos: None,
            mission_target_idx: None,
        }
    }

    /// One fixed-timestep logic step.
    pub fn update(&mut self, input: &mut Input, dt: f32) {
        self.time += dt;
        self.fx.step(dt);
        self.city.step_lights(dt);

        // --- Player meta (recoil, cooldown, respawn) ---
        self.player.update_meta(dt);

        // --- Respawn if dead ---
        if !self.player.alive {
            if self.player.respawn_timer <= 0.0 {
                self.respawn_player();
            }
            // Still update camera and drain edges.
            self.camera.update(
                &self.player, &self.vehicles, 0.0, 0.0, self.cfg.mouse_sensitivity, dt,
            );
            input.drain_edges();
            return;
        }

        // --- Enter/Exit vehicle ---
        if input.enter_exit {
            if let Some(vi) = self.player.in_vehicle {
                // Exit.
                self.player.in_vehicle = None;
                self.vehicles[vi].occupied = false;
                let car = &self.vehicles[vi];
                let right = Vector3 { x: car.yaw.sin(), y: 0.0, z: -car.yaw.cos() };
                self.player.pos = vadd(car.pos, vscale(right, 2.5));
                self.player.pos.y = 0.0;
                self.player.vel = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
            } else {
                // Try to enter nearest vehicle within range.
                let mut best: Option<(usize, f32)> = None;
                for (i, v) in self.vehicles.iter().enumerate() {
                    if v.destroyed {
                        continue;
                    }
                    let d = vdist_xz(v.pos, self.player.pos);
                    if d < 4.0 && best.map_or(true, |(_, bd)| d < bd) {
                        best = Some((i, d));
                    }
                }
                if let Some((vi, _)) = best {
                    self.player.in_vehicle = Some(vi);
                    self.vehicles[vi].occupied = true;
                }
            }
        }

        // --- Switch weapon ---
        if input.switch_weapon {
            self.player.switch_weapon();
        }

        // --- Reload ---
        if input.reload {
            self.player.start_reload();
        }

        // --- Player update (on foot or driven vehicle) ---
        let look_dx = input.look_dx;
        let look_dy = input.look_dy;
        if let Some(vi) = self.player.in_vehicle {
            self.vehicles[vi].update_driven(input, &self.city, &self.cfg, dt);
            // Player position follows vehicle.
            self.player.pos = self.vehicles[vi].pos;
            self.player.yaw = self.vehicles[vi].yaw;
        } else {
            self.player.update_on_foot(input, &self.city, &self.cfg, dt);
        }

        // --- Camera ---
        self.camera.update(
            &self.player, &self.vehicles,
            look_dx, look_dy,
            self.cfg.mouse_sensitivity, dt,
        );

        // --- Shooting ---
        self.panic_pos = None;
        if self.player.want_fire
            && self.player.fire_cooldown <= 0.0
            && self.player.reloading <= 0.0
            && self.player.ammo > 0
        {
            let weapon = self.player.weapon;
            self.player.fire_cooldown = weapon.fire_rate();
            self.player.ammo -= 1;
            self.player.recoil = 0.15;
            if self.player.ammo == 0 {
                self.player.start_reload();
            }

            let cam_pos = self.camera.pos;
            let cam_fwd = self.camera.forward();
            let result = fire_weapon(
                &self.player, cam_pos, cam_fwd,
                &self.city, &mut self.peds, &mut self.cops, &mut self.vehicles,
                &mut self.fx,
            );
            self.panic_pos = Some(self.player.pos);

            // Wanted heat for shooting in public / killing.
            match result.kind {
                HitKind::Ped(_) => {
                    self.wanted.add_heat(0.5);
                }
                HitKind::Cop(_) => {
                    self.wanted.add_heat(2.0);
                }
                _ => {
                    // Firing a weapon adds a tiny bit of heat.
                    if self.wanted.stars == 0 {
                        self.wanted.add_heat(0.1);
                    }
                }
            }
        }

        // --- Melee ---
        if input.melee && self.player.weapon == Weapon::Unarmed {
            melee_attack(&self.player, &mut self.peds, &mut self.cops, &mut self.vehicles, &mut self.fx);
        }

        // --- Peds ---
        for ped in self.peds.iter_mut() {
            ped.update(dt, &self.city, self.panic_pos);
        }
        // Collect cash from dead peds.
        for ped in self.peds.iter_mut() {
            if ped.should_despawn() {
                // Drop cash pickup.
                let cash = ped.cash;
                if cash > 0 {
                    self.pickups.push(Pickup::money(ped.pos, cash as i64));
                }
            }
        }
        self.peds.retain(|p| !p.should_despawn());

        // Respawn peds up to max.
        while self.peds.len() < self.cfg.max_peds {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let dist = rand::random::<f32>() * 40.0 + 50.0;
            let pos = Vector3 {
                x: (self.player.pos.x + angle.cos() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
                y: 0.0,
                z: (self.player.pos.z + angle.sin() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
            };
            let col = Color::new(
                100 + (rand::random::<u32>() % 120) as u8,
                100 + (rand::random::<u32>() % 120) as u8,
                100 + (rand::random::<u32>() % 120) as u8,
                255,
            );
            self.peds.push(Ped::new(pos, col));
        }

        // --- Cops ---
        let cops_shoot = self.wanted.cops_shoot();
        let player_pos = self.player.pos;
        for cop in self.cops.iter_mut() {
            let fired = cop.update(dt, player_pos, cops_shoot);
            if fired {
                cop_fire(cop.pos, player_pos, &mut self.player, &mut self.fx);
            }
        }
        // Despawn dead cops.
        for cop in self.cops.iter_mut() {
            if cop.should_despawn() {
                self.pickups.push(Pickup::money(cop.pos, 50));
            }
        }
        self.cops.retain(|c| !c.should_despawn());

        // Spawn/despawn cops based on wanted level.
        let target_cops = self.wanted.target_cop_count();
        if self.cops.len() < target_cops {
            let to_spawn = target_cops - self.cops.len();
            for _ in 0..to_spawn.min(2) { // spawn max 2 per tick
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let dist = 40.0 + rand::random::<f32>() * 20.0;
                let pos = Vector3 {
                    x: (player_pos.x + angle.cos() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
                    y: 0.0,
                    z: (player_pos.z + angle.sin() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
                };
                self.cops.push(Cop::new(pos));
            }
        }
        // Despawn excess cops when wanted drops.
        if self.cops.len() > target_cops + 2 {
            // Remove farthest cops.
            self.cops.sort_by(|a, b| {
                vdist_xz(a.pos, player_pos).partial_cmp(&vdist_xz(b.pos, player_pos)).unwrap()
            });
            while self.cops.len() > target_cops + 2 {
                if let Some(last) = self.cops.last() {
                    if vdist_xz(last.pos, player_pos) > 80.0 {
                        self.cops.pop();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        // --- Traffic ---
        for tc in self.traffic.iter_mut() {
            tc.update(&self.city, &mut self.vehicles, player_pos, dt);
        }

        // --- Vehicle explosions ---
        let mut explosions = Vec::new();
        for v in self.vehicles.iter_mut() {
            let exploded = v.step_explosion(dt);
            if exploded {
                explosions.push(v.pos);
            }
        }
        for ex in &explosions {
            self.fx.explosion(*ex);
            // Damage nearby entities.
            for ped in self.peds.iter_mut() {
                if vdist_xz(ped.pos, *ex) < 6.0 {
                    ped.take_damage(80.0);
                    self.wanted.add_heat(0.5);
                }
            }
            for cop in self.cops.iter_mut() {
                if vdist_xz(cop.pos, *ex) < 6.0 {
                    cop.take_damage(80.0);
                }
            }
            if vdist_xz(self.player.pos, *ex) < 6.0 {
                self.player.take_damage(40.0);
            }
            for v in self.vehicles.iter_mut() {
                if vdist_xz(v.pos, *ex) < 5.0 && !v.destroyed {
                    v.take_damage(50.0);
                }
            }
        }

        // --- Pickups ---
        for p in self.pickups.iter_mut() {
            p.update(dt);
        }
        for p in self.pickups.iter_mut() {
            p.try_collect(&mut self.player);
        }
        self.pickups.retain(|p| p.active || p.respawn_timer > 0.0);

        // --- Shops ---
        if input.interact {
            for shop in &self.shops {
                if vdist_xz(shop.pos, self.player.pos) < 4.0 {
                    if let Some(msg) = shop.try_buy(&mut self.player) {
                        self.mission.show_banner(msg);
                    }
                }
            }
        }

        // --- Wanted system ---
        // Check if player is visible to any cop.
        let visible = self.cops.iter().any(|c| {
            !c.dead() && vdist_xz(c.pos, player_pos) < 50.0
        });
        self.wanted.update(dt, visible);

        // --- Missions ---
        let in_vehicle = self.player.in_vehicle.is_some();
        let target_alive = self.mission_target_idx.and_then(|i| {
            self.peds.get(i).map(|p| !p.dead())
        });
        let (reward, spawn_target) = self.mission.update(dt, player_pos, target_alive, in_vehicle);
        if reward > 0 {
            self.player.money += reward;
            // Start next mission after a delay.
            self.mission.start_new(player_pos, self.cfg.world_half());
            self.mission_target_idx = None;
        }
        if spawn_target {
            // Spawn a target ped near the marker.
            let marker = self.mission.marker;
            let pos = vadd(marker, Vector3 { x: 5.0, y: 0.0, z: 5.0 });
            let mut target = Ped::new(pos, Color::new(255, 80, 80, 255));
            target.cash = 0;
            self.peds.push(target);
            self.mission_target_idx = Some(self.peds.len() - 1);
        }

        // --- Snapshot for interpolation ---
        self.player.snapshot();
        for v in self.vehicles.iter_mut() {
            v.snapshot();
        }
        for ped in self.peds.iter_mut() {
            ped.snapshot();
        }
        for cop in self.cops.iter_mut() {
            cop.snapshot();
        }

        // Drain consumed edges.
        input.drain_edges();
    }

    fn respawn_player(&mut self) {
        self.player.alive = true;
        self.player.health = 100.0;
        self.player.armor = 0.0;
        self.player.vel = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        self.player.pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        self.player.in_vehicle = None;
        // Lose a fraction of money, clear wanted.
        let loss = (self.player.money as f32 * 0.1) as i64;
        self.player.money -= loss;
        self.wanted.clear();
        self.mission.show_banner(&format!("Respawned. Lost ${}", loss));
    }

    /// Render one frame with interpolation alpha.
    pub fn render(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, alpha: f32, fps: i32) {
        let cam = self.camera.to_camera3d();
        let rate_label = self.cfg.logic_rate.label();
        let debug = self.cfg.debug_overlay;

        let mut d = rl.begin_drawing(thread);
        // Clear color + depth buffer (depth clear is essential — without it 3D
        // geometry fails the depth test against stale values and renders nothing).
        d.clear_background(self.assets.sky_bottom);
        // Sky gradient (top-to-bottom) drawn over the cleared color buffer.
        let sh = d.get_screen_height();
        let sky_top = self.assets.sky_top;
        let sky_bottom = self.assets.sky_bottom;
        for y in (0..sh).step_by(2) {
            let t = y as f32 / sh as f32;
            let c = Color::new(
                (sky_top.r as f32 + (sky_bottom.r as f32 - sky_top.r as f32) * t) as u8,
                (sky_top.g as f32 + (sky_bottom.g as f32 - sky_top.g as f32) * t) as u8,
                (sky_top.b as f32 + (sky_bottom.b as f32 - sky_top.b as f32) * t) as u8,
                255,
            );
            d.draw_rectangle(0, y, d.get_screen_width(), 2, c);
        }

        // 3D scene.
        {
            let mut d3 = d.begin_mode3D(cam);
            // World.
            draw_world(&mut d3, &self.city, &self.assets, &self.cfg);

            // Pickups.
            for p in &self.pickups {
                if p.active {
                    draw_pickup(&mut d3, p.pos, p.color(), self.time);
                }
            }

            // Mission marker.
            if self.mission.has_active_marker() {
                draw_mission_marker(&mut d3, self.mission.marker, Color::new(255, 80, 255, 255), self.time);
            }

            // Shop markers.
            for shop in &self.shops {
                draw_mission_marker(&mut d3, shop.pos, Color::new(80, 200, 255, 255), self.time + 1.5);
            }

            // Vehicles.
            for v in &self.vehicles {
                let rp = v.render_pos(alpha);
                let ry = v.render_yaw(alpha);
                draw_car(&mut d3, rp, ry, v.color, v.damage_level());
            }

            // Peds.
            for ped in &self.peds {
                let rp = ped.render_pos(alpha);
                let ry = ped.render_yaw(alpha);
                draw_character(&mut d3, rp, ry, ped.color, ped.dead());
            }

            // Cops (blue uniform).
            for cop in &self.cops {
                let rp = cop.render_pos(alpha);
                let ry = cop.render_yaw(alpha);
                draw_character(&mut d3, rp, ry, Color::new(40, 60, 140, 255), cop.dead());
            }

            // Player (only if on foot and alive).
            if self.player.in_vehicle.is_none() && self.player.alive {
                let rp = self.player.render_pos(alpha);
                let ry = self.player.render_yaw(alpha);
                draw_character(&mut d3, rp, ry, Color::new(60, 180, 80, 255), !self.player.alive);
            }

            // FX.
            self.fx.draw(&mut d3);
        }

        // HUD (2D).
        let cam_pos = self.camera.pos;
        let cam_yaw = self.camera.yaw;
        hud::draw_hud(
            &mut d,
            &self.player,
            &self.wanted,
            &self.mission,
            &self.vehicles,
            &self.city,
            &self.cfg,
            cam_pos, cam_yaw,
            &self.assets,
            rate_label,
            debug,
            fps,
        );
    }

    /// Handle hotkeys (called per render frame).
    pub fn handle_hotkeys(&mut self, rl: &RaylibHandle) {
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            self.cfg.debug_overlay = !self.cfg.debug_overlay;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F2) {
            self.cfg.logic_rate = self.cfg.logic_rate.next();
        }
    }
}
