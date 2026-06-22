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

pub struct Game<'a> {
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
    pub panic_pos: Option<Vector3>,
    pub mission_target_idx: Option<usize>,
    pub look_accum_x: f32,
    pub look_accum_y: f32,
    pub paused: bool,
    pub quit: bool,
    pub pending_fullscreen: bool,
    pub sfx: crate::sound::SoundEffects<'a>,
}

impl<'a> Game<'a> {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: Config, audio: &'a RaylibAudio) -> Self {
        let city = City::generate(&cfg);
        let assets = Assets::load(rl, thread, &cfg);
        let mut sfx = crate::sound::SoundEffects::load(audio);
        sfx.set_sfx_volume(cfg.sfx_volume);
        sfx.set_music_volume(cfg.music_volume);
        sfx.start_radio();

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
                x: angle.cos() * 4.5,
                y: 0.0,
                z: angle.sin() * 4.5,
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
            // Spawn on a random sidewalk: pick a random grid line and offset.
            let (pos, _axis) = city.nearest_sidewalk(
                rand::random::<f32>() * cfg.world_half() * 2.0 - cfg.world_half(),
                rand::random::<f32>() * cfg.world_half() * 2.0 - cfg.world_half(),
            );
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
            look_accum_x: 0.0,
            look_accum_y: 0.0,
            paused: false,
            quit: false,
            pending_fullscreen: false,
            sfx,
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
                self.sfx.enter_exit.play();
            } else {
                // Try to enter nearest vehicle within range.
                let mut best: Option<(usize, f32)> = None;
                for (i, v) in self.vehicles.iter().enumerate() {
                    if v.destroyed {
                        continue;
                    }
                    let d = vdist_xz(v.pos, self.player.pos);
                    if d < 5.0 && best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((i, d));
                    }
                }
                if let Some((vi, _)) = best {
                    self.player.in_vehicle = Some(vi);
                    self.vehicles[vi].occupied = true;
                    self.sfx.enter_exit.play();
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
        // Consume accumulated look deltas (survives frames with no logic step).
        let look_dx = self.look_accum_x;
        let look_dy = self.look_accum_y;
        self.look_accum_x = 0.0;
        self.look_accum_y = 0.0;
        if let Some(vi) = self.player.in_vehicle {
            // Reconstruct previous wheel positions (using state before update_driven runs)
            let prev_q_yaw = crate::render::models::Quat {
                w: (self.vehicles[vi].yaw * 0.5).cos(),
                x: 0.0,
                y: (self.vehicles[vi].yaw * 0.5).sin(),
                z: 0.0,
            };
            let prev_q_pitch = crate::render::models::Quat {
                w: (self.vehicles[vi].pitch * 0.5).cos(),
                x: (self.vehicles[vi].pitch * 0.5).sin(),
                y: 0.0,
                z: 0.0,
            };
            let prev_q_roll = crate::render::models::Quat {
                w: (self.vehicles[vi].roll * 0.5).cos(),
                x: 0.0,
                y: 0.0,
                z: (self.vehicles[vi].roll * 0.5).sin(),
            };
            let prev_q = prev_q_yaw * prev_q_pitch * prev_q_roll;
            
            let wl_local = Vector3 { x: -0.9, y: -0.4, z: -1.3 };
            let wr_local = Vector3 { x: 0.9, y: -0.4, z: -1.3 };
            
            let prev_wl_pos = vadd(self.vehicles[vi].pos, crate::render::models::rotate_vector(wl_local, prev_q));
            let prev_wr_pos = vadd(self.vehicles[vi].pos, crate::render::models::rotate_vector(wr_local, prev_q));

            let crashed = self.vehicles[vi].update_driven(input, &self.city, &self.cfg, dt);
            if crashed {
                self.sfx.crash.play();
            }
            // Stunt Jump Detection
            if let Some(air_time) = self.vehicles[vi].just_landed_stunt {
                let reward = (air_time * 300.0) as i64;
                self.player.money += reward;
                self.sfx.complete.play();
                self.mission.show_banner(&format!("STUNT JUMP! +${}", reward));
            }
            
            let car = &self.vehicles[vi];
            let is_sliding = input.handbrake 
                && car.speed.abs() > 4.0 
                && car.pos.y <= self.city.get_ground_height(car.pos) + 0.05;
            // Spawn drift smoke/skid particles when handbraking at speed
            if is_sliding {
                let fwd = dir_from_yaw(car.yaw);
                let rear_pos = vsub(car.pos, vscale(fwd, 1.3));
                self.fx.burst(rear_pos, 2, 1.2, Color::new(200, 200, 202, 130), 0.4, 0.2);

                // Add skidmark segment slightly raised above the ground to avoid z-fighting
                let cur_q_yaw = crate::render::models::Quat {
                    w: (car.yaw * 0.5).cos(),
                    x: 0.0,
                    y: (car.yaw * 0.5).sin(),
                    z: 0.0,
                };
                let cur_q_pitch = crate::render::models::Quat {
                    w: (car.pitch * 0.5).cos(),
                    x: (car.pitch * 0.5).sin(),
                    y: 0.0,
                    z: 0.0,
                };
                let cur_q_roll = crate::render::models::Quat {
                    w: (car.roll * 0.5).cos(),
                    x: 0.0,
                    y: 0.0,
                    z: (car.roll * 0.5).sin(),
                };
                let cur_q = cur_q_yaw * cur_q_pitch * cur_q_roll;

                let cur_wl_pos = vadd(car.pos, crate::render::models::rotate_vector(wl_local, cur_q));
                let cur_wr_pos = vadd(car.pos, crate::render::models::rotate_vector(wr_local, cur_q));

                let prev_car_pos = car.prev_pos;
                let cur_car_pos = car.pos;

                let wl_from_ground = self.city.get_ground_height(Vector3 { x: prev_wl_pos.x, y: prev_car_pos.y, z: prev_wl_pos.z });
                let wl_to_ground = self.city.get_ground_height(Vector3 { x: cur_wl_pos.x, y: cur_car_pos.y, z: cur_wl_pos.z });
                let wr_from_ground = self.city.get_ground_height(Vector3 { x: prev_wr_pos.x, y: prev_car_pos.y, z: prev_wr_pos.z });
                let wr_to_ground = self.city.get_ground_height(Vector3 { x: cur_wr_pos.x, y: cur_car_pos.y, z: cur_wr_pos.z });

                let mut wl_from = prev_wl_pos;
                let mut wl_to = cur_wl_pos;
                let mut wr_from = prev_wr_pos;
                let mut wr_to = cur_wr_pos;

                wl_from.y = wl_from_ground + 0.04;
                wl_to.y = wl_to_ground + 0.04;
                wr_from.y = wr_from_ground + 0.04;
                wr_to.y = wr_to_ground + 0.04;

                self.fx.add_skidmark(wl_from, wl_to, 0.35, 10.0);
                self.fx.add_skidmark(wr_from, wr_to, 0.35, 10.0);
            }
            // Player position follows vehicle.
            self.player.pos = self.vehicles[vi].pos;
            self.player.yaw = self.vehicles[vi].yaw;
        } else {
            self.player.update_on_foot(input, &self.city, &self.cfg, dt);
        }

        // --- Engine sound ---
        if let Some(vi) = self.player.in_vehicle {
            let car = &self.vehicles[vi];
            let throttle = input.move_y.abs();
            self.sfx.update_engine(true, car.speed, throttle);
        } else {
            self.sfx.update_engine(false, 0.0, 0.0);
        }

        // --- Camera ---
        self.camera.update(
            &self.player, &self.vehicles,
            look_dx, look_dy,
            self.cfg.mouse_sensitivity, dt,
        );
        // Sync player facing to camera yaw (camera is the rotation authority on foot).
        if self.player.in_vehicle.is_none() {
            self.player.yaw = self.camera.yaw;
        }

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
            self.sfx.shoot.play();
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
            let (pos, _axis) = self.city.nearest_sidewalk(
                (self.player.pos.x + angle.cos() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
                (self.player.pos.z + angle.sin() * dist).clamp(-self.cfg.world_half() + 5.0, self.cfg.world_half() - 5.0),
            );
            let col = Color::new(
                100 + (rand::random::<u32>() % 120) as u8,
                100 + (rand::random::<u32>() % 120) as u8,
                100 + (rand::random::<u32>() % 120) as u8,
                255,
            );
            self.peds.push(Ped::new(pos, col));
        }

        // --- Cops ---
        let stars = self.wanted.stars;
        let player_pos = self.player.pos;
        for cop in self.cops.iter_mut() {
            let fired = cop.update(dt, &self.city, player_pos, stars);
            if fired {
                let hit = cop_fire(cop.pos, player_pos, &mut self.fx);
                if hit {
                    let dmg = 10.0;
                    if let Some(vi) = self.player.in_vehicle {
                        self.vehicles[vi].take_damage(dmg);
                        // Spawn sparks on the car
                        let hit_point = vadd(self.player.pos, Vector3 { x: 0.0, y: 0.8, z: 0.0 });
                        self.fx.burst(hit_point, 5, 2.0, Color::new(255, 200, 80, 255), 0.2, 5.0);
                    } else {
                        self.player.take_damage(dmg);
                        self.fx.blood(vadd(player_pos, Vector3 { x: 0.0, y: 1.0, z: 0.0 }));
                    }
                }
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
        let limit = if stars == 0 { 0 } else { target_cops + 2 };
        if self.cops.len() > limit {
            // Remove farthest cops.
            self.cops.sort_by(|a, b| {
                vdist_xz(a.pos, player_pos).partial_cmp(&vdist_xz(b.pos, player_pos)).unwrap()
            });
            let despawn_dist = if stars == 0 { 35.0 } else { 80.0 };
            while self.cops.len() > limit {
                if let Some(last) = self.cops.last() {
                    if vdist_xz(last.pos, player_pos) > despawn_dist {
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

        // --- Car-vs-Car Collisions ---
        let num_vehicles = self.vehicles.len();
        for i in 0..num_vehicles {
            for j in (i + 1)..num_vehicles {
                if self.vehicles[i].destroyed || self.vehicles[j].destroyed {
                    continue;
                }
                let dist = vdist_xz(self.vehicles[i].pos, self.vehicles[j].pos);
                let col_dist = 2.5; // combined vehicle radii
                if dist < col_dist && dist > 0.05 {
                    let normal = vnorm_xz(vsub(self.vehicles[i].pos, self.vehicles[j].pos));
                    let overlap = col_dist - dist;
                    // Push vehicles apart
                    self.vehicles[i].pos.x += normal.x * overlap * 0.5;
                    self.vehicles[i].pos.z += normal.z * overlap * 0.5;
                    self.vehicles[j].pos.x -= normal.x * overlap * 0.5;
                    self.vehicles[j].pos.z -= normal.z * overlap * 0.5;

                    // Elastic impulse collision resolution
                    let rel_vel = vsub(self.vehicles[i].vel, self.vehicles[j].vel);
                    let impulse = vdot(rel_vel, normal);
                    if impulse < 0.0 {
                        let bounce = 0.35;
                        let impulse_vec = vscale(normal, impulse * 0.5 * (1.0 + bounce));
                        self.vehicles[i].vel = vsub(self.vehicles[i].vel, impulse_vec);
                        self.vehicles[j].vel = vadd(self.vehicles[j].vel, impulse_vec);

                        // Take damage from high impact speed
                        let impact = impulse.abs();
                        if impact > 4.0 {
                            self.vehicles[i].take_damage(impact * 0.5);
                            self.vehicles[j].take_damage(impact * 0.5);
                            if self.vehicles[i].occupied || self.vehicles[j].occupied {
                                self.sfx.crash.play();
                            }
                            // Spawn sparks at contact midpoint
                            let mid = vscale(vadd(self.vehicles[i].pos, self.vehicles[j].pos), 0.5);
                            self.fx.burst(mid, 8, 4.0, Color::new(255, 200, 80, 255), 0.25, 6.0);
                        }
                    }
                }
            }
        }

        // --- Car-vs-Ped & Car-vs-Cop Collisions ---
        let mut hit_sound = false;
        for v in self.vehicles.iter_mut() {
            if v.destroyed { continue; }
            let v_speed = v.speed.abs();

            // Player on foot vs car
            if self.player.in_vehicle.is_none() && self.player.alive {
                let d = vdist_xz(v.pos, self.player.pos);
                let col_dist = 1.7; // car radius 1.3 + player 0.4
                if d < col_dist && d > 0.05 {
                    let normal = vnorm_xz(vsub(self.player.pos, v.pos));
                    let overlap = col_dist - d;
                    self.player.pos.x += normal.x * overlap;
                    self.player.pos.z += normal.z * overlap;

                    if v_speed > 4.0 {
                        self.player.take_damage(v_speed * 1.5);
                        self.player.vel = vadd(self.player.vel, vscale(normal, v_speed * 0.8));
                        hit_sound = true;
                        self.fx.blood(self.player.pos);
                    }
                }
            }

            // Peds vs car
            for ped in self.peds.iter_mut() {
                if ped.dead() { continue; }
                let d = vdist_xz(v.pos, ped.pos);
                let col_dist = 1.7;
                if d < col_dist && d > 0.05 {
                    let normal = vnorm_xz(vsub(ped.pos, v.pos));
                    let overlap = col_dist - d;
                    ped.pos.x += normal.x * overlap;
                    ped.pos.z += normal.z * overlap;

                    if v_speed > 4.0 {
                        ped.take_damage(v_speed * 2.0);
                        ped.vel = vscale(normal, v_speed * 0.9 + 2.0); // Throw ped!
                        self.fx.blood(ped.pos);
                        hit_sound = true;
                        if v.occupied { self.wanted.add_heat(0.3); } // player's car — crime gets heat
                    }
                }
            }

            // Cops vs car
            for cop in self.cops.iter_mut() {
                if cop.dead() { continue; }
                let d = vdist_xz(v.pos, cop.pos);
                let col_dist = 1.7;
                if d < col_dist && d > 0.05 {
                    let normal = vnorm_xz(vsub(cop.pos, v.pos));
                    let overlap = col_dist - d;
                    cop.pos.x += normal.x * overlap;
                    cop.pos.z += normal.z * overlap;

                    if v_speed > 4.0 {
                        cop.take_damage(v_speed * 2.0);
                        cop.vel = vscale(normal, v_speed * 0.9 + 2.0); // Throw cop!
                        self.fx.blood(cop.pos);
                        hit_sound = true;
                        if v.occupied { self.wanted.add_heat(1.0); } // player's car — hitting cops is severe
                    }
                }
            }
        }
        if hit_sound {
            self.sfx.crash.play();
        }

        // --- Vehicle fire & explosions ---
        let mut explosions: Vec<(Vector3, bool)> = Vec::new();
        for v in self.vehicles.iter_mut() {
            // If car is down to 20% health or less, it catches fire and degrades over 30s
            if !v.destroyed && v.health <= v.max_health * 0.20 && v.health > 0.0 {
                let decay = (v.max_health * 0.20) / 30.0 * dt;
                v.take_damage(decay);
                
                // Spawn fire particles at the engine bay (front of car)
                let fwd = dir_from_yaw(v.yaw);
                let engine_pos = vadd(v.pos, vscale(fwd, 1.2));
                // Rises up (negative gravity)
                self.fx.burst(engine_pos, 2, 2.0, Color::new(255, 120, 20, 220), 0.4, -4.0);
            }

            let exploded = v.step_explosion(dt);
            if exploded {
                explosions.push((v.pos, v.occupied));
            }
        }
        for (ex, was_occupied) in &explosions {
            self.sfx.explosion.play();
            self.fx.explosion(*ex);
            // Damage nearby entities (lethal explosion damage)
            for ped in self.peds.iter_mut() {
                if vdist_xz(ped.pos, *ex) < 6.0 {
                    ped.take_damage(180.0); // lethal
                    if *was_occupied { self.wanted.add_heat(0.5); }
                }
            }
            for cop in self.cops.iter_mut() {
                if vdist_xz(cop.pos, *ex) < 6.0 {
                    cop.take_damage(180.0); // lethal
                }
            }
            if vdist_xz(self.player.pos, *ex) < 6.0 {
                self.player.take_damage(250.0); // lethal
            }
            for v in self.vehicles.iter_mut() {
                if vdist_xz(v.pos, *ex) < 5.0 && !v.destroyed {
                    v.take_damage(110.0); // can trigger chain reaction
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
            self.sfx.complete.play();
            // Start next mission after a delay.
            self.mission.start_new(player_pos, self.cfg.world_half());
            self.mission_target_idx = None;
        }
        if spawn_target {
            // Spawn a target ped near the marker.
            let marker = self.mission.marker;
            let (pos, _axis) = self.city.nearest_sidewalk(marker.x + 5.0, marker.z + 5.0);
            let mut target = Ped::new(pos, Color::new(255, 80, 80, 255));
            target.cash = 0;
            self.peds.push(target);
            self.mission_target_idx = Some(self.peds.len() - 1);
        }

        // --- NPC-vs-player separation (prevent overlap) ---
        let pp = self.player.pos;
        for ped in self.peds.iter_mut() {
            if ped.dead() { continue; }
            let d = vdist_xz(ped.pos, pp);
            if d < 1.2 && d > 0.01 {
                let push_dir = vnorm_xz(vsub(ped.pos, pp));
                ped.pos.x += push_dir.x * (1.2 - d);
                ped.pos.z += push_dir.z * (1.2 - d);
            }
        }
        for cop in self.cops.iter_mut() {
            if cop.dead() { continue; }
            let d = vdist_xz(cop.pos, pp);
            if d < 1.2 && d > 0.01 {
                let push_dir = vnorm_xz(vsub(cop.pos, pp));
                cop.pos.x += push_dir.x * (1.2 - d);
                cop.pos.z += push_dir.z * (1.2 - d);
            }
        }
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
        self.sfx.update_music();
        self.sfx.update_radio();
        let cam = self.camera.to_camera3d();
        let cam_pos = self.camera.pos;
        let cam_fwd = self.camera.forward();
        let rate_label = self.cfg.logic_rate.label();
        let debug = self.cfg.debug_overlay;

        // Pre-calculate screen coordinates for floating vehicle health bars (prevents borrow issues inside draw block)
        let mut vehicle_health_bars = Vec::new();
        for v in &self.vehicles {
            if v.destroyed || v.health >= v.max_health {
                continue;
            }
            let bar_world_pos = Vector3 { x: v.pos.x, y: v.pos.y + 1.4, z: v.pos.z };
            let to_point = vsub(bar_world_pos, cam_pos);
            // Frustum check: only draw if in front of camera
            if vdot(to_point, cam_fwd) > 0.1 {
                let screen_pos = rl.get_world_to_screen(bar_world_pos, cam);
                vehicle_health_bars.push((screen_pos, v.health / v.max_health));
            }
        }

        let mut d = rl.begin_drawing(thread);
        // Clear color + depth buffer (depth clear is essential — without it 3D
        // geometry fails the depth test against stale values and renders nothing).
        // Day/night sky colors computed from game time.
        let total_hours = (self.time * self.cfg.time_scale).rem_euclid(24.0);
        let (sky_top, sky_bottom) = crate::config::sky_colors_for_hour(total_hours);
        d.clear_background(sky_bottom);
        let sh = d.get_screen_height();
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
                let rp_pitch = v.render_pitch(alpha);
                let rp_roll = v.render_roll(alpha);
                draw_car(
                    &mut d3,
                    &self.assets,
                    rp,
                    ry,
                    rp_pitch,
                    rp_roll,
                    v.color,
                    v.damage_level(),
                );
            }

            // Peds.
            for ped in &self.peds {
                let rp = ped.render_pos(alpha);
                let ry = ped.render_yaw(alpha);
                let is_moving = !ped.dead();
                draw_character(
                    &mut d3,
                    &self.assets,
                    rp,
                    ry,
                    ped.color,
                    ped.pants_color,
                    ped.hair_color,
                    ped.hair_style,
                    ped.has_glasses,
                    ped.dead(),
                    self.time,
                    is_moving,
                );
            }

            // Cops (blue uniform).
            for cop in &self.cops {
                let rp = cop.render_pos(alpha);
                let ry = cop.render_yaw(alpha);
                let is_moving = !cop.dead() && cop.state == crate::ai::cop::CopState::Chase;
                draw_character(
                    &mut d3,
                    &self.assets,
                    rp,
                    ry,
                    Color::new(30, 45, 110, 255), // Shirt
                    Color::new(20, 20, 20, 255),   // Pants
                    Color::new(20, 30, 80, 255),   // Hat color
                    crate::render::models::HairStyle::PoliceHat,
                    false,
                    cop.dead(),
                    self.time,
                    is_moving,
                );
            }

            // Player (signature green shirt, jeans, red cap, sunglasses).
            if self.player.in_vehicle.is_none() && self.player.alive {
                let rp = self.player.render_pos(alpha);
                let ry = self.player.render_yaw(alpha);
                let is_moving = vlen_xz(self.player.vel) > 0.1;
                draw_character(
                    &mut d3,
                    &self.assets,
                    rp,
                    ry,
                    Color::new(60, 180, 80, 255), // Green shirt
                    Color::new(45, 52, 85, 255),  // Blue jeans
                    Color::new(200, 40, 40, 255), // Red cap
                    crate::render::models::HairStyle::Cap,
                    true,
                    !self.player.alive,
                    self.time,
                    is_moving,
                );
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

        // Draw floating vehicle health bars above damaged vehicles.
        for (screen_pos, hp_ratio) in vehicle_health_bars {
            let bar_w = 46;
            let bar_h = 6;
            let bx = (screen_pos.x as i32) - bar_w / 2;
            let by = (screen_pos.y as i32) - bar_h / 2;
            
            // Background
            d.draw_rectangle(bx, by, bar_w, bar_h, Color::new(40, 40, 40, 180));
            // Health color (Red for fire <=20%, Orange <=50%, Green fine)
            let color = if hp_ratio <= 0.20 {
                Color::new(230, 40, 40, 255)
            } else if hp_ratio <= 0.50 {
                Color::new(230, 130, 30, 255)
            } else {
                Color::new(40, 200, 60, 255)
            };
            let hp_w = (hp_ratio * (bar_w - 2) as f32) as i32;
            d.draw_rectangle(bx + 1, by + 1, hp_w.max(0).min(bar_w - 2), bar_h - 2, color);
            // Border
            d.draw_rectangle_lines(bx, by, bar_w, bar_h, Color::new(10, 10, 10, 255));
        }

        // Clock display (day/night cycle time).
        let time_str = crate::config::format_game_time(self.time, self.cfg.time_scale);
        let clock_w = d.measure_text(&time_str, 20);
        d.draw_text(&time_str, d.get_screen_width() - clock_w - 16, 8, 20, Color::new(255, 255, 255, 200));
        d.draw_text(&time_str, d.get_screen_width() - clock_w - 17, 7, 20, Color::new(0, 0, 0, 150));

        // Pause menu overlay.
        if self.paused {
            self.render_pause_menu(&mut d);
        }
    }

    /// Render the pause menu overlay with mouse interaction.
    fn render_pause_menu(&mut self, d: &mut RaylibDrawHandle) {
        use raylib::consts::MouseButton;

        let sw = d.get_screen_width();
        let sh = d.get_screen_height();
        let mx = d.get_mouse_x();
        let my = d.get_mouse_y();
        let mouse_down = d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_pressed = d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        // Dim overlay.
        d.draw_rectangle(0, 0, sw, sh, Color::new(0, 0, 0, 180));

        // Panel.
        let pw = 400;
        let ph = 460;
        let px = (sw - pw) / 2;
        let py = (sh - ph) / 2;
        d.draw_rectangle(px, py, pw, ph, Color::new(30, 30, 40, 240));
        d.draw_rectangle_lines(px, py, pw, ph, Color::new(80, 80, 100, 255));

        // Title.
        let title = "PAUSED";
        let tw = d.measure_text(title, 36);
        d.draw_text(title, px + (pw - tw) / 2, py + 20, 36, Color::new(255, 255, 255, 255));

        let mut y = py + 80;
        let label_x = px + 30;
        let item_h = 44;
        let btn_w = 200;
        let btn_h = 40;
        let btn_x = px + (pw - btn_w) / 2;

        // Helper: click-in-rect check.
        let in_rect = |x: i32, y: i32, rx: i32, ry: i32, rw: i32, rh: i32| {
            x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
        };

        // Fullscreen toggle.
        let fs_text = format!("Fullscreen: {}", if d.is_window_fullscreen() { "ON" } else { "OFF" });
        d.draw_text(&fs_text, label_x, y, 20, Color::new(200, 200, 220, 255));
        if mouse_pressed && in_rect(mx, my, label_x, y - 4, 200, 24) {
            self.pending_fullscreen = true;
        }
        y += item_h;

        // Framerate cycle.
        let fps_text = format!("Framerate: {}", self.cfg.logic_rate.label());
        d.draw_text(&fps_text, label_x, y, 20, Color::new(200, 200, 220, 255));
        if mouse_pressed && in_rect(mx, my, label_x, y - 4, 200, 24) {
            self.cfg.logic_rate = self.cfg.logic_rate.next();
        }
        y += item_h;

        // SFX Volume slider.
        d.draw_text("SFX Volume", label_x, y, 20, Color::new(200, 200, 220, 255));
        let slider_x = label_x + 130;
        let slider_w = 200;
        let slider_y = y + 4;
        d.draw_rectangle(slider_x, slider_y, slider_w, 16, Color::new(60, 60, 70, 255));
        let sfx_fill = (self.sfx.sfx_volume * slider_w as f32) as i32;
        d.draw_rectangle(slider_x, slider_y, sfx_fill, 16, Color::new(80, 160, 220, 255));
        d.draw_rectangle_lines(slider_x, slider_y, slider_w, 16, Color::new(120, 120, 140, 255));
        if mouse_down && in_rect(mx, my, slider_x - 4, slider_y - 4, slider_w + 8, 24) {
            let v = ((mx - slider_x) as f32 / slider_w as f32).clamp(0.0, 1.0);
            self.cfg.sfx_volume = v;
            self.sfx.set_sfx_volume(v);
        }
        y += item_h;

        // Music Volume slider.
        d.draw_text("Music", label_x, y, 20, Color::new(200, 200, 220, 255));
        d.draw_rectangle(slider_x, slider_y + item_h, slider_w, 16, Color::new(60, 60, 70, 255));
        let mus_y = y + 4;
        let mus_fill = (self.sfx.music_volume * slider_w as f32) as i32;
        d.draw_rectangle(slider_x, mus_y, mus_fill, 16, Color::new(160, 80, 200, 255));
        d.draw_rectangle_lines(slider_x, mus_y, slider_w, 16, Color::new(120, 120, 140, 255));
        if mouse_down && in_rect(mx, my, slider_x - 4, mus_y - 4, slider_w + 8, 24) {
            let v = ((mx - slider_x) as f32 / slider_w as f32).clamp(0.0, 1.0);
            self.cfg.music_volume = v;
            self.sfx.set_music_volume(v);
        }
        y += item_h + 10;

        // Resume button.
        let resume_hover = in_rect(mx, my, btn_x, y, btn_w, btn_h);
        let resume_col = if resume_hover { Color::new(60, 140, 80, 255) } else { Color::new(40, 100, 60, 255) };
        d.draw_rectangle(btn_x, y, btn_w, btn_h, resume_col);
        d.draw_rectangle_lines(btn_x, y, btn_w, btn_h, Color::new(120, 200, 140, 255));
        let rt = "RESUME";
        let rtw = d.measure_text(rt, 22);
        d.draw_text(rt, btn_x + (btn_w - rtw) / 2, y + 10, 22, Color::new(255, 255, 255, 255));
        if mouse_pressed && resume_hover {
            self.paused = false;
        }
        y += btn_h + 12;

        // Quit button.
        let quit_hover = in_rect(mx, my, btn_x, y, btn_w, btn_h);
        let quit_col = if quit_hover { Color::new(160, 50, 50, 255) } else { Color::new(120, 35, 35, 255) };
        d.draw_rectangle(btn_x, y, btn_w, btn_h, quit_col);
        d.draw_rectangle_lines(btn_x, y, btn_w, btn_h, Color::new(220, 120, 120, 255));
        let qt = "QUIT";
        let qtw = d.measure_text(qt, 22);
        d.draw_text(qt, btn_x + (btn_w - qtw) / 2, y + 10, 22, Color::new(255, 255, 255, 255));
        if mouse_pressed && quit_hover {
            self.quit = true;
        }
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
