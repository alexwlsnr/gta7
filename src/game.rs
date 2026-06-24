//! Game state: owns all entities, runs logic steps, renders the scene.
use raylib::prelude::*;
use raylib::ffi::Vector3;

use crate::config::Config;
use crate::input::Input;
use crate::mathx::*;
use crate::world::city::City;
use crate::player::{Player, Weapon};
use crate::vehicle::{Vehicle, VehicleKind, VehicleVariant};
use crate::camera::FollowCamera;
use crate::combat::{fire_weapon, melee_attack, cop_fire, HitKind};
use crate::wanted::WantedSystem;
use crate::ai::ped::Ped;
use crate::ai::cop::{Cop, PoliceCar, spawn_police_car};
use crate::ai::traffic::{TrafficCar, spawn_traffic};
use crate::pickup::{Pickup, Shop, ShopKind};
use crate::mission::MissionState;
use crate::render::models::{Assets, draw_world, draw_car, draw_character, draw_pickup, draw_mission_marker, draw_shadow_casters};
use crate::render::fx::Fx;
use crate::hud;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState {
    Title,
    Intro,
    Playing,
}

struct DialogLine {
    speaker: &'static str,
    text: &'static str,
    color: Color,
}

const INTRO_DIALOG: &[DialogLine] = &[
    DialogLine {
        speaker: "OFFICER KOWALSKI",
        text: "Well, well. Look who's back on the streets of Silicon Valley. Jimmy 'The Compiler' Vance.",
        color: Color { r: 60, g: 150, b: 255, a: 255 }, // Blue
    },
    DialogLine {
        speaker: "JIMMY VANCE",
        text: "Hey, Kowalski. I served my sentence. I'm clean now. Just trying to compile some Rust.",
        color: Color { r: 100, g: 255, b: 100, a: 255 }, // Green
    },
    DialogLine {
        speaker: "OFFICER KOWALSKI",
        text: "Clean? A clean compile is a myth in this city, Jimmy. I've got my eyes on you.",
        color: Color { r: 60, g: 150, b: 255, a: 255 },
    },
    DialogLine {
        speaker: "JIMMY VANCE",
        text: "I'm just a freelance developer now, Kowalski. No more illegal pointer arithmetic.",
        color: Color { r: 100, g: 255, b: 100, a: 255 },
    },
    DialogLine {
        speaker: "OFFICER KOWALSKI",
        text: "You think you can dereference raw pointers in my district and get away with it?",
        color: Color { r: 60, g: 150, b: 255, a: 255 },
    },
    DialogLine {
        speaker: "JIMMY VANCE",
        text: "It was a safe abstraction, Kowalski! You set me up!",
        color: Color { r: 100, g: 255, b: 100, a: 255 },
    },
    DialogLine {
        speaker: "OFFICER KOWALSKI",
        text: "Save the stack trace for the judge. I catch you garbage collecting without a license...",
        color: Color { r: 60, g: 150, b: 255, a: 255 },
    },
    DialogLine {
        speaker: "JIMMY VANCE",
        text: "Yeah, yeah. What's the catch? Why am I out of the sandbox?",
        color: Color { r: 100, g: 255, b: 100, a: 255 },
    },
    DialogLine {
        speaker: "OFFICER KOWALSKI",
        text: "Let's just say a thread got terminated early. Now get out of my sight before I panic!",
        color: Color { r: 60, g: 150, b: 255, a: 255 },
    },
    DialogLine {
        speaker: "JIMMY VANCE",
        text: "Still the same old Kowalski. Time to boot up the IDE and see who's still online.",
        color: Color { r: 100, g: 255, b: 100, a: 255 },
    },
];

pub struct Game<'a> {
    pub cfg: Config,
    pub city: City,
    pub assets: Assets,
    pub player: Player,
    pub vehicles: Vec<Vehicle>,
    pub peds: Vec<Ped>,
    pub cops: Vec<Cop>,
    pub traffic: Vec<TrafficCar>,
    pub police_cars: Vec<PoliceCar>,
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
    pub lighting: crate::render::lighting::LightingSystem,
    pub screen_state: ScreenState,
    pub intro_dialog_idx: usize,
    pub intro_timer: f32,
}

impl<'a> Game<'a> {
    pub fn new(rl: &mut RaylibHandle, thread: &RaylibThread, cfg: Config, audio: &'a RaylibAudio) -> Self {
        let mut city = City::generate(&cfg);
        let mut assets = Assets::load(rl, thread, &cfg);
        let mut sfx = crate::sound::SoundEffects::load(audio);
        sfx.set_sfx_volume(cfg.sfx_volume);
        sfx.set_music_volume(cfg.music_volume);

        let lighting = crate::render::lighting::LightingSystem::load(rl, thread);
        lighting.apply_to_materials(&mut assets);
        sfx.start_radio();

        // Player at center on a road.
        let player_pos = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
        let player = Player::new(player_pos);

        let mut shops = Vec::new();
        let mut pickups = Vec::new();

        // Regenerate starting block area to properly populate initial shops and pickups around the player
        city.generated_blocks.clear();
        city.buildings.clear();
        city.parks.clear();
        city.lanes.clear();
        city.lights.clear();
        city.ramps.clear();
        city.ensure_blocks_around(player_pos, 6, &cfg, &mut shops, &mut pickups);

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
                Color::new(255, 20, 147, 255), // Neon pink
                Color::new(0, 240, 255, 255),   // Neon cyan
                Color::new(180, 0, 255, 255),   // Neon purple
            ];
            vehicles.push(Vehicle::new(pos, angle, colors[i], VehicleKind::Civilian));
        }

        // Spawn the parked police car for the intro cutscene
        vehicles.push(Vehicle::new(
            Vector3 { x: 3.2, y: 0.0, z: 3.5 },
            2.3,
            Color::new(20, 20, 20, 255),
            VehicleKind::Police,
        ));

        // Spawn pedestrians.
        let mut peds = Vec::new();
        let ped_colors = [
            Color::new(255, 20, 147, 255), // Neon pink
            Color::new(0, 240, 255, 255),   // Neon cyan
            Color::new(180, 0, 255, 255),   // Neon purple
            Color::new(50, 255, 50, 255),   // Neon lime green
            Color::new(255, 110, 0, 255),   // Laser orange
        ];
        for _ in 0..cfg.max_peds {
            let (pos, _axis) = city.nearest_sidewalk(
                rand::random::<f32>() * 200.0 - 100.0,
                rand::random::<f32>() * 200.0 - 100.0,
            );
            let col = ped_colors[rand::random::<usize>() % ped_colors.len()];
            peds.push(Ped::new(pos, col));
        }

        // Additional static/starting pickups.
        for i in 0..6 {
            let angle = i as f32 * 1.05;
            let dist = 30.0 + i as f32 * 8.0;
            let pos = Vector3 {
                x: angle.cos() * dist,
                y: 0.0,
                z: angle.sin() * dist,
            };
            match i % 4 {
                0 => pickups.push(Pickup::health(pos)),
                1 => pickups.push(Pickup::armor(pos)),
                2 => pickups.push(Pickup::money(pos, 200)),
                _ => pickups.push(Pickup::weapon(pos, Weapon::Smg)),
            }
        }

        // Shops: weapon + health + armor + ammo at starting locations.
        shops.push(Shop::new(Vector3 { x: 15.0, y: 0.0, z: 15.0 }, ShopKind::Weapon));
        shops.push(Shop::new(Vector3 { x: -15.0, y: 0.0, z: -15.0 }, ShopKind::Health));
        shops.push(Shop::new(Vector3 { x: 15.0, y: 0.0, z: -15.0 }, ShopKind::Armor));
        shops.push(Shop::new(Vector3 { x: -15.0, y: 0.0, z: 15.0 }, ShopKind::Ammo));

        // Start first mission.
        let mut mission = MissionState::new();
        mission.start_new(player_pos, cfg.world_half());
        // Start the day at 13:00 so the first impression is bright daylight.
        let initial_time = 13.0 / cfg.time_scale;

        Game {
            cfg,
            city,
            assets,
            player,
            vehicles,
            peds,
            cops: Vec::new(),
            traffic,
            police_cars: Vec::new(),
            pickups,
            shops,
            wanted: WantedSystem::new(),
            mission,
            fx: Fx::new(),
            camera: FollowCamera::new(),
            time: initial_time,
            panic_pos: None,
            mission_target_idx: None,
            look_accum_x: 0.0,
            look_accum_y: 0.0,
            paused: false,
            quit: false,
            pending_fullscreen: false,
            lighting,
            sfx,
            screen_state: ScreenState::Title,
            intro_dialog_idx: 0,
            intro_timer: 0.0,
        }
    }

    /// One fixed-timestep logic step.
    pub fn update(&mut self, input: &mut Input, dt: f32) {
        match self.screen_state {
            ScreenState::Title => {
                self.time += dt;
                self.fx.step(dt);
                self.city.step_lights(dt);

                // Update traffic so city feels alive
                let player_pos = self.player.pos;
                for tc in self.traffic.iter_mut() {
                    tc.update(&self.city, &mut self.vehicles, player_pos, dt);
                }

                // Slow orbit of camera around city center
                let orbit_radius = 55.0;
                let speed = 0.12;
                let angle = self.time * speed;
                self.camera.pos = Vector3 {
                    x: angle.cos() * orbit_radius,
                    y: 22.0 + (angle * 2.0).sin() * 5.0,
                    z: angle.sin() * orbit_radius,
                };
                self.camera.target = Vector3 { x: 0.0, y: 2.0, z: 0.0 };

                // Note: Title screen menu input (mouse/keyboard selection) is handled in render_title_screen

                self.player.snapshot();
                for v in self.vehicles.iter_mut() {
                    v.snapshot();
                }
                for ped in self.peds.iter_mut() {
                    ped.snapshot();
                }
                input.drain_edges();
                return;
            }
            ScreenState::Intro => {
                self.time += dt;
                self.fx.step(dt);
                self.city.step_lights(dt);

                // Keep player positioned outside jail (stationary)
                self.player.pos = Vector3 { x: 0.0, y: 0.0, z: 2.0 };
                self.player.yaw = 0.0;
                self.player.vel = Vector3 { x: 0.0, y: 0.0, z: 0.0 };

                self.intro_timer += dt;

                let next_dialog = input.key_space_pressed || input.key_enter_pressed || self.intro_timer >= 5.0;
                let skip = input.key_s_pressed;

                if skip {
                    self.screen_state = ScreenState::Playing;
                    self.sfx.complete.play();
                } else if next_dialog {
                    self.intro_dialog_idx += 1;
                    self.intro_timer = 0.0;
                    if self.intro_dialog_idx >= INTRO_DIALOG.len() {
                        self.screen_state = ScreenState::Playing;
                        self.sfx.complete.play();
                    } else {
                        self.sfx.enter_exit.play();
                    }
                }

                // Cinematic camera orbiting Jimmy and Kowalski
                let jimmy_pos = Vector3 { x: 0.0, y: 0.0, z: 2.0 };
                let kowalski_pos = Vector3 { x: 0.0, y: 0.0, z: 4.5 };
                let center = Vector3 {
                    x: (jimmy_pos.x + kowalski_pos.x) * 0.5,
                    y: 1.1,
                    z: (jimmy_pos.z + kowalski_pos.z) * 0.5,
                };
                
                let orbit_radius = 4.5;
                let speed = 0.18;
                let angle = self.time * speed;
                self.camera.pos = Vector3 {
                    x: center.x + angle.cos() * orbit_radius,
                    y: 1.5 + (self.time * 0.5).sin() * 0.15,
                    z: center.z + angle.sin() * orbit_radius,
                };
                self.camera.target = center;

                self.player.snapshot();
                for v in self.vehicles.iter_mut() {
                    v.snapshot();
                }
                for ped in self.peds.iter_mut() {
                    ped.snapshot();
                }
                input.drain_edges();
                return;
            }
            ScreenState::Playing => {}
        }

        self.time += dt;
        self.fx.step(dt);
        self.city.step_lights(dt);

        // --- Endless procedural block generation ---
        let player_pos = self.player.pos;
        self.city.ensure_blocks_around(player_pos, 6, &self.cfg, &mut self.shops, &mut self.pickups);

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
                let stars = self.wanted.stars;
                self.sfx.update_audio_mode(false, stars);
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
                    let mut had_driver = false;

                    // 1. Check if it is a civilian traffic car
                    let is_traffic = self.traffic.iter().any(|tc| tc.vehicle_idx == vi);
                    if is_traffic {
                        had_driver = true;
                        let car = &self.vehicles[vi];
                        let ped_colors = [
                            Color::new(255, 20, 147, 255),
                            Color::new(0, 240, 255, 255),
                            Color::new(180, 0, 255, 255),
                            Color::new(50, 255, 50, 255),
                            Color::new(255, 110, 0, 255),
                        ];
                        let col = ped_colors[rand::random::<usize>() % ped_colors.len()];
                        // Spawn a pedestrian driver getting thrown out
                        let mut ped = Ped::new(car.pos, col);
                        ped.state = crate::ai::ped::PedState::Dead;
                        ped.dead_timer = 3.5;
                        ped.health = 30.0; // injured

                        // Throw out to the left of the vehicle (driver side)
                        let fwd = dir_from_yaw(car.yaw);
                        let left = Vector3 { x: fwd.z, y: 0.0, z: -fwd.x };
                        ped.pos = vadd(car.pos, vscale(left, 1.2));
                        ped.vel = vadd(vscale(left, 9.0), Vector3 { x: 0.0, y: 4.5, z: 0.0 });
                        
                        self.peds.push(ped);
                        
                        // Play impact/crash sound
                        self.sfx.crash.play();
                        
                        // Remove from active traffic list so AI updates stop
                        self.traffic.retain(|tc| tc.vehicle_idx != vi);
                    }

                    // 2. Check if police car with cops inside
                    let mut thrown_cops = Vec::new();
                    for cop in &mut self.cops {
                        if cop.in_car == Some(vi) {
                            had_driver = true;
                            cop.in_car = None;
                            cop.state = crate::ai::cop::CopState::Chase; // Aggro player
                            
                            // Throw cop out of the vehicle
                            let car = &self.vehicles[vi];
                            let fwd = dir_from_yaw(car.yaw);
                            let left = Vector3 { x: fwd.z, y: 0.0, z: -fwd.x };
                            cop.pos = vadd(car.pos, vscale(left, 1.2));
                            cop.vel = vadd(vscale(left, 8.0), Vector3 { x: 0.0, y: 4.0, z: 0.0 });
                            cop.health = 45.0; // injured
                            
                            thrown_cops.push(cop.pos);
                        }
                    }
                    if !thrown_cops.is_empty() {
                        self.sfx.crash.play();
                        // Hijacking a cop car triggers wanted stars
                        let needed_heat = 2.0 - self.wanted.heat;
                        if needed_heat > 0.0 {
                            self.wanted.add_heat(needed_heat);
                        }
                    }

                    self.vehicles[vi].is_traffic = false;
                    self.vehicles[vi].occupied = true;

                    // Enter.
                    self.player.in_vehicle = Some(vi);
                    self.sfx.enter_exit.play();
                    
                    let stars = self.wanted.stars;
                    self.sfx.update_audio_mode(true, stars);

                    if had_driver {
                        self.wanted.add_heat(0.6);
                        self.mission.show_banner("Hijacked! Vehicle stolen.");
                    }
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
            // Nitro exhaust flames
            if self.vehicles[vi].nitro_active {
                let v = &self.vehicles[vi];
                let fwd = crate::mathx::dir_from_yaw(v.yaw);
                let exhaust_pos = Vector3 {
                    x: v.pos.x - fwd.x * 2.5,
                    y: v.pos.y + 0.3,
                    z: v.pos.z - fwd.z * 2.5,
                };
                self.fx.burst(
                    exhaust_pos, 3, 4.0,
                    Color::new(0, 255, 255, 220), 0.25, 2.0, // Neon cyan flame
                );
                // Hot violet inner flame
                self.fx.burst(
                    exhaust_pos, 1, 2.5,
                    Color::new(180, 0, 255, 240), 0.15, 1.5,
                );
            }
            // Handbrake drift smoke
            if input.handbrake && self.vehicles[vi].speed.abs() > 3.0 {
                let v = &self.vehicles[vi];
                let fwd = crate::mathx::dir_from_yaw(v.yaw);
                let right = Vector3 { x: -fwd.z, y: 0.0, z: fwd.x };
                for side in [-1.0f32, 1.0] {
                    let tire_pos = Vector3 {
                        x: v.pos.x - fwd.x * 1.8 + right.x * side * 0.9,
                        y: v.pos.y + 0.1,
                        z: v.pos.z - fwd.z * 1.8 + right.z * side * 0.9,
                    };
                    self.fx.burst(
                        tire_pos, 2, 1.5,
                        Color::new(255, 20, 147, 180), 0.6, 0.5, // Hot pink drift smoke!
                    );
                }
            }
            if crashed {
                self.sfx.crash.play();
                // Reconstruct rotation quaternion of the car for spark emitter direction
                let cry = self.vehicles[vi].yaw;
                let crp = self.vehicles[vi].pitch;
                let crr = self.vehicles[vi].roll;
                let cq_yaw = crate::render::models::Quat {
                    w: (cry * 0.5).cos(),
                    x: 0.0,
                    y: (cry * 0.5).sin(),
                    z: 0.0,
                };
                let cq_pitch = crate::render::models::Quat {
                    w: (crp * 0.5).cos(),
                    x: (crp * 0.5).sin(),
                    y: 0.0,
                    z: 0.0,
                };
                let cq_roll = crate::render::models::Quat {
                    w: (crr * 0.5).cos(),
                    x: 0.0,
                    y: 0.0,
                    z: (crr * 0.5).sin(),
                };
                let cq = cq_yaw * cq_pitch * cq_roll;
                let front_pos = vadd(self.vehicles[vi].pos, crate::render::models::rotate_vector(Vector3 { x: 0.0, y: 0.0, z: 1.5 }, cq));
                self.fx.burst(
                    front_pos,
                    15, // count
                    3.0, // speed
                    Color::new(255, 180, 50, 230), // bright spark orange
                    0.5, // size
                    0.3, // lifetime
                );
            }
            // Stunt Jump Detection
            if let Some(air_time) = self.vehicles[vi].just_landed_stunt {
                let reward = (air_time * 300.0) as i64;
                self.player.money += reward;
                self.sfx.complete.play();
                self.mission.show_banner(&format!("STUNT JUMP! +${}", reward));
            }
            
            let car = &self.vehicles[vi];
            
            // Spawn nitro rocket trail particles out of the twin exhausts
            if car.nitro_active {
                let fwd = dir_from_yaw(car.yaw);
                let right = Vector3 { x: -fwd.z, y: 0.0, z: fwd.x };
                let body_l = match car.variant {
                    VehicleVariant::Sports => 4.3,
                    VehicleVariant::SUV => 4.4,
                    VehicleVariant::Pickup => 4.6,
                    VehicleVariant::Sedan => 4.2,
                };
                let rear_center = vsub(car.pos, vscale(fwd, body_l * 0.5));
                let exhausts = [
                    vadd(vsub(rear_center, vscale(right, 0.65)), Vector3 { x: 0.0, y: 0.2, z: 0.0 }),
                    vadd(vadd(rear_center, vscale(right, 0.65)), Vector3 { x: 0.0, y: 0.2, z: 0.0 }),
                ];

                for &exhaust_pos in &exhausts {
                    // Flame particles shoot backward from exhaust direction
                    let flame_speed = 14.0;
                    let spread_x = (rand::random::<f32>() - 0.5) * 1.5;
                    let spread_y = (rand::random::<f32>() - 0.5) * 1.5;
                    let spread_z = (rand::random::<f32>() - 0.5) * 1.5;
                    let p_vel = Vector3 {
                        x: car.vel.x - fwd.x * flame_speed + spread_x,
                        y: car.vel.y - fwd.y * flame_speed + spread_y + 0.4,
                        z: car.vel.z - fwd.z * flame_speed + spread_z,
                    };

                    let rand_val = rand::random::<f32>();
                    let p_color = if rand_val < 0.65 {
                        Color::new(0, 200, 255, 230) // bright cyan core
                    } else if rand_val < 0.85 {
                        Color::new(0, 80, 255, 180) // blue secondary
                    } else {
                        Color::new(245, 250, 255, 255) // white hot sparks
                    };

                    self.fx.particles.push(crate::render::fx::Particle {
                        pos: exhaust_pos,
                        vel: p_vel,
                        life: 0.16 + rand::random::<f32>() * 0.12,
                        max_life: 0.28,
                        size: 0.16 + rand::random::<f32>() * 0.14,
                        color: p_color,
                        gravity: -1.0, // floats up slightly
                    });
                }
            }

            let is_sliding = input.handbrake 
                && car.speed.abs() > 4.0 
                && car.pos.y <= self.city.get_ground_height(car.pos) + 0.05;
            // Spawn drift smoke/skid particles when handbraking at speed
            if is_sliding {
                let fwd = dir_from_yaw(car.yaw);
                let rear_pos = vsub(car.pos, vscale(fwd, 1.3));
                self.fx.burst(rear_pos, 4, 1.5, Color::new(210, 210, 215, 160), 0.6, 0.35); // thick grey smoke
                self.fx.burst(rear_pos, 1, 0.5, Color::new(40, 40, 42, 200), 0.3, 0.25);   // black rubber fragments

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
        let target_idx = self.mission_target_idx;
        let mut new_target_idx = None;
        let mut next_idx = 0;
        let mut i = 0;
        let p_pos = self.player.pos;
        self.peds.retain(|p| {
            let is_target = Some(i) == target_idx;
            let keep = !p.should_despawn() && (vdist_xz(p.pos, p_pos) < 120.0 || is_target);
            if keep {
                if is_target {
                    new_target_idx = Some(next_idx);
                }
                next_idx += 1;
            }
            i += 1;
            keep
        });
        self.mission_target_idx = new_target_idx;

        // Respawn peds up to max.
        while self.peds.len() < self.cfg.max_peds {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            let dist = rand::random::<f32>() * 40.0 + 50.0;
            let (pos, _axis) = self.city.nearest_sidewalk(
                self.player.pos.x + angle.cos() * dist,
                self.player.pos.z + angle.sin() * dist,
            );
            let ped_colors = [
                Color::new(255, 20, 147, 255),
                Color::new(0, 240, 255, 255),
                Color::new(180, 0, 255, 255),
                Color::new(50, 255, 50, 255),
                Color::new(255, 110, 0, 255),
            ];
            let col = ped_colors[rand::random::<usize>() % ped_colors.len()];
            self.peds.push(Ped::new(pos, col));
        }

        // --- Cops ---
        let stars = self.wanted.stars;
        let player_pos = self.player.pos;
        for cop in self.cops.iter_mut() {
            // If in car, sync position to vehicle and skip on-foot update
            if let Some(vi) = cop.in_car {
                cop.pos = self.vehicles[vi].pos;
                cop.yaw = self.vehicles[vi].yaw;
                cop.prev_pos = cop.pos;
                cop.prev_yaw = cop.yaw;
                continue;
            }
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

        // --- Police Cars ---
        // 1. Despawn excess police cars when wanted drops or after death.
        let target_police_cars = match stars {
            0..=2 => 0,
            3 => 1,
            4 => 2,
            5 => 3,
            _ => 4,
        };

        let pc_limit = if stars == 0 { 0 } else { target_police_cars + 1 };
        if self.police_cars.len() > pc_limit {
            // Remove farthest police cars
            self.police_cars.sort_by(|a, b| {
                let va = &self.vehicles[a.vehicle_idx];
                let vb = &self.vehicles[b.vehicle_idx];
                vdist_xz(va.pos, player_pos).partial_cmp(&vdist_xz(vb.pos, player_pos)).unwrap()
            });
            let despawn_dist = if stars == 0 { 35.0 } else { 80.0 };
            while self.police_cars.len() > pc_limit {
                if let Some(pc) = self.police_cars.last() {
                    let v = &self.vehicles[pc.vehicle_idx];
                    if vdist_xz(v.pos, player_pos) > despawn_dist {
                        // Clear the cops associated with this car
                        let car_idx = pc.vehicle_idx;
                        self.cops.retain(|c| c.in_car != Some(car_idx));
                        self.police_cars.pop();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        // 2. Retain only active police cars that are not destroyed and still have cops inside.
        let vehicles = &self.vehicles;
        let cops = &self.cops;
        self.police_cars.retain(|pc| {
            let v = &vehicles[pc.vehicle_idx];
            let has_cop = cops.iter().any(|c| c.in_car == Some(pc.vehicle_idx));
            !v.destroyed && has_cop
        });

        // 3. Update active police cars
        let player_in_vehicle = self.player.in_vehicle.is_some();
        for i in 0..self.police_cars.len() {
            let mut pc = self.police_cars[i].clone();
            pc.update(&self.city, &mut self.vehicles, &mut self.cops, player_pos, player_in_vehicle, dt);
            self.police_cars[i] = pc;
        }

        // 4. Spawn police cars if we are below the target count
        if self.police_cars.len() < target_police_cars {
            spawn_police_car(
                &self.city,
                &mut self.vehicles,
                &mut self.cops,
                &mut self.police_cars,
                player_pos,
            );
        }

        // Spawn/despawn cops based on wanted level.
        let target_cops = self.wanted.target_cop_count();
        if self.cops.len() < target_cops {
            let to_spawn = target_cops - self.cops.len();
            for _ in 0..to_spawn.min(2) { // spawn max 2 per tick
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                let dist = 40.0 + rand::random::<f32>() * 20.0;
                let pos = Vector3 {
                    x: player_pos.x + angle.cos() * dist,
                    y: 0.0,
                    z: player_pos.z + angle.sin() * dist,
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

        // Teleport far-away traffic cars near the player
        for tc in self.traffic.iter_mut() {
            let v = &mut self.vehicles[tc.vehicle_idx];
            if vdist_xz(v.pos, player_pos) > 150.0 {
                if let Some(lane_idx) = self.city.get_random_lane_near(player_pos, 50.0, 100.0) {
                    let lane = &self.city.lanes[lane_idx];
                    let from = self.city.intersection(lane.from.0, lane.from.1);
                    let to = self.city.intersection(lane.to.0, lane.to.1);
                    let offset = self.city.road_width * 0.25;
                    let (cx, cz) = match lane.axis {
                        crate::world::city::Axis::X => (0.0, -offset * lane.dir as f32),
                        crate::world::city::Axis::Z => (offset * lane.dir as f32, 0.0),
                    };
                    let pos = Vector3 { x: from.x + cx, y: 0.0, z: from.z + cz };
                    let yaw = yaw_from_dir(vnorm_xz(vsub(to, from)));
                    
                    v.pos = pos;
                    v.yaw = yaw;
                    v.prev_pos = pos;
                    v.prev_yaw = yaw;
                    v.vel = Vector3 { x: 0.0, y: 0.0, z: 0.0 };
                    v.speed = 12.0;
                    v.health = v.max_health;
                    v.destroyed = false;
                    v.explode_timer = 0.0;
                    
                    tc.current_lane = lane_idx;
                    tc.lane_progress = 0.0;
                }
            }
        }

        // Periodic cleanup of inactive/far-away vehicles to prevent memory growth
        self.cleanup_inactive_vehicles();

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
                        if v.occupied { self.wanted.add_heat(0.8); } // player's car — crime gets heat
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
        self.sfx.update_audio_mode(self.player.in_vehicle.is_some(), self.wanted.stars);
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



    fn cleanup_inactive_vehicles(&mut self) {
        let player_pos = self.player.pos;
        let player_vehicle_idx = self.player.in_vehicle;

        let mut referenced = std::collections::HashSet::new();
        if let Some(idx) = player_vehicle_idx {
            referenced.insert(idx);
        }
        for tc in &self.traffic {
            referenced.insert(tc.vehicle_idx);
        }
        for pc in &self.police_cars {
            referenced.insert(pc.vehicle_idx);
        }

        let mut new_vehicles = Vec::new();
        let mut index_map = std::collections::HashMap::new();

        for (old_idx, v) in self.vehicles.drain(..).enumerate() {
            let keep = referenced.contains(&old_idx) || vdist_xz(v.pos, player_pos) < 120.0;
            if keep {
                let new_idx = new_vehicles.len();
                index_map.insert(old_idx, new_idx);
                new_vehicles.push(v);
            }
        }
        self.vehicles = new_vehicles;

        if let Some(old_idx) = self.player.in_vehicle {
            self.player.in_vehicle = index_map.get(&old_idx).copied();
        }
        for tc in &mut self.traffic {
            if let Some(&new_idx) = index_map.get(&tc.vehicle_idx) {
                tc.vehicle_idx = new_idx;
            }
        }
        for pc in &mut self.police_cars {
            if let Some(&new_idx) = index_map.get(&pc.vehicle_idx) {
                pc.vehicle_idx = new_idx;
            }
        }
        for cop in &mut self.cops {
            if let Some(old_idx) = cop.in_car {
                cop.in_car = index_map.get(&old_idx).copied();
            }
        }
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

        // --- Shadow Pass ---
        // Render shadow-casting geometry to the shadow map BEFORE the main draw
        // frame. `begin_texture_mode` and `begin_drawing` both borrow `rl`
        // mutably, so the shadow pass must complete (RAII drop) before the main
        // pass begins.
        let total_hours = (self.time * self.cfg.time_scale).rem_euclid(24.0);
        let player_pos = self.player.pos;
        self.lighting.prepare_shadow(player_pos, total_hours);
        // Snapshot the shadow camera (a Copy) before borrowing `shadow_map` —
        // `shadow_camera()` borrows &self.lighting, which would conflict with
        // the mutable `shadow_map` borrow held by the texture-mode guard below.
        let shadow_cam = self.lighting.shadow_camera();
        {
            let mut dt = rl.begin_texture_mode(thread, &mut self.lighting.shadow_map);
            dt.clear_background(Color::new(255, 255, 255, 255));
            let mut d3 = dt.begin_mode3D(shadow_cam);
            {
                let mut d3s = d3.begin_shader_mode(&mut self.lighting.depth_shader);
                draw_shadow_casters(
                    &mut d3s,
                    &self.city,
                    &self.assets,
                    &self.cfg,
                    &self.vehicles,
                    &self.peds,
                    &self.cops,
                    &self.player,
                );
            }
        }

        let mut d = rl.begin_drawing(thread);
        // Clear color + depth buffer (depth clear is essential — without it 3D
        // geometry fails the depth test against stale values and renders nothing).
        // Day/night sky colors computed from game time (total_hours set above
        // for the shadow pass).
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

        // Gather dynamic point lights
        let mut gathered_lights = Vec::new();

        // 1. Player car lights
        if let Some(pv) = self.player.in_vehicle {
            if let Some(v) = self.vehicles.get(pv) {
                let p = v.render_pos(alpha);
                let q_yaw = crate::render::models::Quat {
                    w: (v.render_yaw(alpha) * 0.5).cos(),
                    x: 0.0,
                    y: (v.render_yaw(alpha) * 0.5).sin(),
                    z: 0.0,
                };
                let q_pitch = crate::render::models::Quat {
                    w: (v.render_pitch(alpha) * 0.5).cos(),
                    x: (v.render_pitch(alpha) * 0.5).sin(),
                    y: 0.0,
                    z: 0.0,
                };
                let q_roll = crate::render::models::Quat {
                    w: (v.render_roll(alpha) * 0.5).cos(),
                    x: 0.0,
                    y: 0.0,
                    z: (v.render_roll(alpha) * 0.5).sin(),
                };
                let q = q_yaw * q_pitch * q_roll;
                
                let (_body_w, _body_h, body_l) = match v.variant {
                    VehicleVariant::Sports => (2.05, 0.65, 4.3),
                    VehicleVariant::SUV => (2.2, 1.1, 4.4),
                    VehicleVariant::Pickup => (2.1, 0.9, 4.6),
                    VehicleVariant::Sedan => (2.0, 0.8, 4.2),
                };

                let is_night = !(6.5..=18.5).contains(&total_hours);
                if is_night {
                    // Headlights point light: slightly in front of the vehicle
                    let headlight_offset = Vector3 { x: 0.0, y: 0.0, z: body_l * 0.5 + 2.0 };
                    let headlight_pos = vadd(p, crate::render::models::rotate_vector(headlight_offset, q));
                    gathered_lights.push(crate::render::lighting::PointLight {
                        pos: headlight_pos,
                        color: Vector3 { x: 0.1, y: 1.6, z: 1.8 }, // neon cyan headlights
                        radius: 20.0,
                    });

                    // Taillights point light: slightly behind the vehicle
                    let taillight_offset = Vector3 { x: 0.0, y: 0.0, z: -body_l * 0.5 - 1.0 };
                    let taillight_pos = vadd(p, crate::render::models::rotate_vector(taillight_offset, q));
                    gathered_lights.push(crate::render::lighting::PointLight {
                        pos: taillight_pos,
                        color: Vector3 { x: 1.8, y: 0.1, z: 1.2 }, // neon hot pink taillights
                        radius: 10.0,
                    });
                }
            }
        }

        // 2. Police Sirens
        for v in &self.vehicles {
            if v.kind == VehicleKind::Police && !v.destroyed {
                let is_red = (self.time * 12.0).sin() > 0.0;
                let col = if is_red {
                    Vector3 { x: 1.8, y: 0.1, z: 0.1 }
                } else {
                    Vector3 { x: 0.1, y: 0.1, z: 1.8 }
                };
                let p = v.render_pos(alpha);
                let siren_pos = Vector3 { x: p.x, y: p.y + 1.2, z: p.z };
                gathered_lights.push(crate::render::lighting::PointLight {
                    pos: siren_pos,
                    color: col,
                    radius: 15.0,
                });
            }
        }

        // 3. Streetlights
        let play_pos = self.player.render_pos(alpha);
        let bs = self.city.block_size;
        let rw = self.city.road_width;
        let sw = self.cfg.sidewalk_width;
        let sw_offset = rw * 0.5 + sw * 0.5;
        let half_extent = self.city.ground_half;
        let origin = -half_extent;
        let n = self.city.blocks;

        // Collect streetlights around the player
        let p_grid_x = ((play_pos.x - origin) / bs).round() as i32;
        let p_grid_z = ((play_pos.z - origin) / bs).round() as i32;

        // Loop over nearby intersections (within 2 blocks)
        for dx in -2..=2 {
            let i = p_grid_x + dx;
            if i < 0 || i > n as i32 { continue; }
            let cx = origin + i as f32 * bs;
            for dz in -2..=2 {
                let j = p_grid_z + dz;
                if j < 0 || j > n as i32 { continue; }
                let cz = origin + j as f32 * bs;

                let offsets = [
                    (-sw_offset, -sw_offset),
                    (sw_offset, -sw_offset),
                    (-sw_offset, sw_offset),
                    (sw_offset, sw_offset),
                ];
                for (ox, oz) in offsets {
                    let sx = cx + ox;
                    let sz = cz + oz;
                    
                    let arm_dir_x = -ox.signum() * 0.8;
                    let arm_dir_z = -oz.signum() * 0.8;
                    let bulb_pos = Vector3 { x: sx + arm_dir_x, y: 3.85, z: sz + arm_dir_z };

                    let is_night = !(6.5..=18.5).contains(&total_hours);
                    if is_night {
                        gathered_lights.push(crate::render::lighting::PointLight {
                            pos: bulb_pos,
                            color: Vector3 { x: 1.8, y: 0.1, z: 1.2 }, // neon magenta/pink streetlight
                            radius: 16.0,
                        });
                    }
                }
            }
        }

        // Sort all gathered lights by distance to the player
        let ref_pos = play_pos;
        gathered_lights.sort_by(|a, b| {
            let da = (a.pos.x - ref_pos.x).powi(2) + (a.pos.y - ref_pos.y).powi(2) + (a.pos.z - ref_pos.z).powi(2);
            let db = (b.pos.x - ref_pos.x).powi(2) + (b.pos.y - ref_pos.y).powi(2) + (b.pos.z - ref_pos.z).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Pass closest 6 point lights to the shader
        self.lighting.update_point_lights(&gathered_lights);

        // Update lit shader uniforms for this frame (sun direction, color, fog,
        // shadow matrix). Must happen before entering shader mode so the values
        // are set on the shader object itself.
        self.lighting.update_uniforms(total_hours, sky_bottom, cam_pos);
        // 3D scene. Models with the lit shader set on their materials will
        // use it automatically. Immediate-mode draws use raylib's default.
        {
            let mut d3 = d.begin_mode3D(cam);

            // Draw the giant Vaporwave Sun on the horizon
            {
                let sun_dir = crate::config::sun_direction(total_hours);
                // Sun is visible when it is above the horizon
                let is_day = sun_dir.y < 0.15;
                if is_day {
                    let sun_yaw_rad = sun_dir.z.atan2(sun_dir.x);
                    let sun_dist = 390.0;
                    let sun_pos = Vector3 {
                        x: cam_pos.x - sun_yaw_rad.cos() * sun_dist,
                        y: cam_pos.y + 45.0, // Fixed horizon height relative to player
                        z: cam_pos.z - sun_yaw_rad.sin() * sun_dist,
                    };
                    let yaw_deg = -sun_yaw_rad.to_degrees();
                    
                    // Draw giant flat billboard sun
                    d3.draw_model_ex(
                        &self.assets.sun_model,
                        sun_pos,
                        Vector3 { x: 0.0, y: 1.0, z: 0.0 },
                        yaw_deg,
                        Vector3 { x: 150.0, y: 150.0, z: 0.1 },
                        Color::WHITE,
                    );
                }
            }

            // World.
            draw_world(&mut d3, &self.city, &self.assets, &self.cfg, total_hours, cam_pos);

            // Pickups.
            if self.screen_state == ScreenState::Playing {
                for p in &self.pickups {
                    if p.active {
                        draw_pickup(&mut d3, p.pos, p.color(), self.time);
                    }
                }
            }

            // Mission marker.
            if self.screen_state == ScreenState::Playing && self.mission.has_active_marker() {
                draw_mission_marker(&mut d3, self.mission.marker, Color::new(255, 80, 255, 255), self.time);
            }

            // Shop markers.
            if self.screen_state == ScreenState::Playing {
                for shop in &self.shops {
                    draw_mission_marker(&mut d3, shop.pos, Color::new(80, 200, 255, 255), self.time + 1.5);
                }
            }

            // Vehicles.
            for v in &self.vehicles {
                let rp = v.render_pos(alpha);
                let ry = v.render_yaw(alpha);
                let rp_pitch = v.render_pitch(alpha);
                let rp_roll = v.render_roll(alpha);
                let rp_wheel_rot = v.render_wheel_rot(alpha);
                draw_car(
                    &mut d3,
                    &self.assets,
                    &mut self.lighting,
                    rp,
                    ry,
                    rp_pitch,
                    rp_roll,
                    rp_wheel_rot,
                    v.color,
                    v.damage_level(),
                    v.kind,
                    v.variant,
                    self.time,
                );
            }

            // Vehicle headlight halos at night
            {
                let is_night = !(6.5..=18.5).contains(&total_hours);
                if is_night {
                    for v in &self.vehicles {
                        if v.destroyed { continue; }
                        let rp = v.render_pos(alpha);
                        let ry = v.render_yaw(alpha);
                        let (_, body_h_val, body_l_val) = match v.variant {
                            VehicleVariant::Sports => (2.05f32, 0.65f32, 4.3f32),
                            VehicleVariant::SUV => (2.2, 1.1, 4.4),
                            VehicleVariant::Pickup => (2.1, 0.9, 4.6),
                            VehicleVariant::Sedan => (2.0, 0.8, 4.2),
                        };
                        let (_, w_rad_val) = match v.variant {
                            VehicleVariant::Sports => (0.65f32, 0.38f32),
                            VehicleVariant::SUV => (1.1, 0.52),
                            VehicleVariant::Pickup => (0.9, 0.5),
                            VehicleVariant::Sedan => (0.8, 0.4),
                        };
                        let y_off = body_h_val * 0.5 + w_rad_val - body_h_val * 0.12;
                        let fwd = crate::mathx::dir_from_yaw(ry);
                        let hl_z_off = body_l_val * 0.5 + 0.05;
                        let hl_pos = Vector3 {
                            x: rp.x + fwd.x * hl_z_off,
                            y: rp.y + y_off,
                            z: rp.z + fwd.z * hl_z_off,
                        };
                        // Neon cyan glow halo
                        d3.draw_sphere(hl_pos, 0.35, Color::new(0, 240, 255, 75));
                        d3.draw_sphere(hl_pos, 0.6, Color::new(0, 240, 255, 35));
                    }
                }
            }

            // Peds.
            if self.screen_state == ScreenState::Playing {
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
            }

            // Cops (blue uniform).
            if self.screen_state == ScreenState::Playing {
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
            }

            // Player (signature green shirt, jeans, red cap, sunglasses).
            if self.screen_state != ScreenState::Title && self.player.in_vehicle.is_none() && self.player.alive {
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

            // During Intro, draw Officer Kowalski explicitly.
            if self.screen_state == ScreenState::Intro {
                let kowalski_pos = Vector3 { x: 0.0, y: 0.0, z: 4.5 };
                let kowalski_yaw = std::f32::consts::PI; // facing Jimmy
                draw_character(
                    &mut d3,
                    &self.assets,
                    kowalski_pos,
                    kowalski_yaw,
                    Color::new(30, 45, 110, 255), // Police blue shirt
                    Color::new(20, 20, 20, 255),   // Black pants
                    Color::new(20, 30, 80, 255),   // Police hat
                    crate::render::models::HairStyle::PoliceHat,
                    false,
                    false, // alive
                    self.time,
                    false, // not moving
                );
            }

            // FX.
            if self.screen_state == ScreenState::Playing {
                self.fx.draw(&mut d3);
            }
        }

        // HUD (2D).
        if self.screen_state == ScreenState::Playing {
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
                &self.sfx,
                self.time,
            );
        }

        // Draw floating vehicle health bars above damaged vehicles.
        if self.screen_state == ScreenState::Playing {
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
        }

        // Clock display (day/night cycle time).
        if self.screen_state == ScreenState::Playing {
            let time_str = crate::config::format_game_time(self.time, self.cfg.time_scale);
            let clock_w = d.measure_text(&time_str, 20);
            d.draw_text(&time_str, d.get_screen_width() - clock_w - 16, 8, 20, Color::new(255, 255, 255, 200));
            d.draw_text(&time_str, d.get_screen_width() - clock_w - 17, 7, 20, Color::new(0, 0, 0, 150));
        }

        // Render Title/Intro overlays
        if self.screen_state == ScreenState::Title {
            self.render_title_screen(&mut d);
        } else if self.screen_state == ScreenState::Intro {
            self.render_intro_cutscene(&mut d);
        }

        // Pause menu overlay.
        if self.paused {
            self.render_pause_menu(&mut d);
        }
    }

    fn render_title_screen(&mut self, d: &mut RaylibDrawHandle) {
        use raylib::consts::MouseButton;
        use raylib::consts::KeyboardKey;

        let sw = d.get_screen_width();
        let sh = d.get_screen_height();
        let mx = d.get_mouse_x();
        let my = d.get_mouse_y();
        let mouse_pressed = d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        // Dark tint overlay for atmospheric feel and text contrast
        d.draw_rectangle(0, 0, sw, sh, Color::new(10, 10, 18, 120));

        // Glassmorphic panel for Logo
        let panel_w = 600;
        let panel_h = 240;
        let panel_x = (sw - panel_w) / 2;
        let panel_y = (sh - panel_h) / 2 - 130;

        d.draw_rectangle(panel_x, panel_y, panel_w, panel_h, Color::new(20, 20, 35, 180));
        d.draw_rectangle_lines(panel_x, panel_y, panel_w, panel_h, Color::new(0, 220, 255, 180));
        d.draw_rectangle_lines(panel_x - 1, panel_y - 1, panel_w + 2, panel_h + 2, Color::new(0, 220, 255, 100));

        let main_title = "GRAND THEFT";
        let sub_title = "ALGORITHM VII";
        let tagline = "SILICON VALLEY OF SIN";

        let mt_size = 50;
        let st_size = 55;
        let mt_w = d.measure_text(main_title, mt_size);
        let st_w = d.measure_text(sub_title, st_size);
        let tag_w = d.measure_text(tagline, 24);

        let start_y = panel_y + 35;

        // Shadow glow
        d.draw_text(main_title, panel_x + (panel_w - mt_w) / 2 + 3, start_y + 3, mt_size, Color::new(255, 0, 128, 120));
        d.draw_text(main_title, panel_x + (panel_w - mt_w) / 2, start_y, mt_size, Color::WHITE);

        d.draw_text(sub_title, panel_x + (panel_w - st_w) / 2 + 3, start_y + mt_size + 13, st_size, Color::new(0, 180, 255, 120));
        d.draw_text(sub_title, panel_x + (panel_w - st_w) / 2, start_y + mt_size + 10, st_size, Color::new(255, 200, 0, 255));

        let div_y = start_y + mt_size + st_size + 20;
        d.draw_line(panel_x + 80, div_y, panel_x + panel_w - 80, div_y, Color::new(0, 220, 255, 150));

        // Tagline (bold neon pink)
        d.draw_text(tagline, panel_x + (panel_w - tag_w) / 2 + 2, div_y + 12, 24, Color::new(0, 0, 0, 150));
        d.draw_text(tagline, panel_x + (panel_w - tag_w) / 2, div_y + 10, 24, Color::new(255, 60, 140, 255));

        // --- MENU INTERACTION ---
        let menu_w = 400;
        let menu_h = 220;
        let menu_x = (sw - menu_w) / 2;
        let menu_y = panel_y + panel_h + 30;

        d.draw_rectangle(menu_x, menu_y, menu_w, menu_h, Color::new(15, 15, 25, 220));
        d.draw_rectangle_lines(menu_x, menu_y, menu_w, menu_h, Color::new(255, 60, 140, 150));
        d.draw_rectangle_lines(menu_x - 1, menu_y - 1, menu_w + 2, menu_h + 2, Color::new(255, 60, 140, 80));

        let in_rect = |x: i32, y: i32, rx: i32, ry: i32, rw: i32, rh: i32| {
            x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
        };

        let fs_label = if d.is_window_fullscreen() { "FULLSCREEN: ON" } else { "FULLSCREEN: OFF" };
        let rate_label = format!("FRAMERATE: {}", self.cfg.logic_rate.label());
        let menu_options = [
            "START STORY",
            fs_label,
            &rate_label,
            "EXIT GAME"
        ];

        // Navigate with UP/DOWN or W/S
        if d.is_key_pressed(KeyboardKey::KEY_UP) || d.is_key_pressed(KeyboardKey::KEY_W) {
            if self.intro_dialog_idx == 0 {
                self.intro_dialog_idx = menu_options.len() - 1;
            } else {
                self.intro_dialog_idx -= 1;
            }
        }
        if d.is_key_pressed(KeyboardKey::KEY_DOWN) || d.is_key_pressed(KeyboardKey::KEY_S) {
            self.intro_dialog_idx = (self.intro_dialog_idx + 1) % menu_options.len();
        }

        // Clamp in case index got corrupted
        if self.intro_dialog_idx >= menu_options.len() {
            self.intro_dialog_idx = 0;
        }

        let item_h = 44;
        let start_item_y = menu_y + 20;

        for (i, opt) in menu_options.iter().enumerate() {
            let item_y = start_item_y + i as i32 * item_h;
            let opt_w = d.measure_text(opt, 20);
            let opt_x = menu_x + (menu_w - opt_w) / 2;

            let hovered = in_rect(mx, my, menu_x + 10, item_y, menu_w - 20, item_h - 8);
            let selected = self.intro_dialog_idx == i;

            if hovered {
                self.intro_dialog_idx = i;
            }

            let text_color = if selected {
                Color::new(0, 220, 255, 255)
            } else {
                Color::new(220, 220, 240, 255)
            };

            if selected {
                d.draw_rectangle(menu_x + 15, item_y, menu_w - 30, item_h - 10, Color::new(255, 60, 140, 45));
                d.draw_rectangle_lines(menu_x + 15, item_y, menu_w - 30, item_h - 10, Color::new(255, 60, 140, 120));
            }

            d.draw_text(opt, opt_x, item_y + 6, 20, text_color);

            let triggered = (hovered && mouse_pressed) || (selected && (d.is_key_pressed(KeyboardKey::KEY_ENTER) || d.is_key_pressed(KeyboardKey::KEY_SPACE)));
            if triggered {
                match i {
                    0 => {
                        self.screen_state = ScreenState::Intro;
                        self.intro_dialog_idx = 0;
                        self.intro_timer = 0.0;
                        self.player.pos = Vector3 { x: 0.0, y: 0.0, z: 2.0 };
                        self.player.yaw = 0.0;
                        self.sfx.complete.play();
                    }
                    1 => {
                        self.pending_fullscreen = true;
                    }
                    2 => {
                        self.cfg.logic_rate = self.cfg.logic_rate.next();
                    }
                    3 => {
                        self.quit = true;
                    }
                    _ => {}
                }
            }
        }
    }

    fn render_intro_cutscene(&self, d: &mut RaylibDrawHandle) {
        let sw = d.get_screen_width();
        let sh = d.get_screen_height();

        // Cinematic bars
        let bar_h = (sh as f32 * 0.12) as i32;
        d.draw_rectangle(0, 0, sw, bar_h, Color::BLACK);
        d.draw_rectangle(0, sh - bar_h, sw, bar_h, Color::BLACK);

        d.draw_line(0, bar_h, sw, bar_h, Color::new(0, 200, 255, 100));
        d.draw_line(0, sh - bar_h, sw, sh - bar_h, Color::new(0, 200, 255, 100));

        if let Some(line) = INTRO_DIALOG.get(self.intro_dialog_idx) {
            let box_w = (sw as f32 * 0.82) as i32;
            let box_h = 135;
            let box_x = (sw - box_w) / 2;
            let box_y = sh - bar_h - box_h - 15;

            d.draw_rectangle(box_x, box_y, box_w, box_h, Color::new(15, 15, 25, 235));
            d.draw_rectangle_lines(box_x, box_y, box_w, box_h, Color::new(255, 60, 140, 180));
            d.draw_rectangle_lines(box_x - 1, box_y - 1, box_w + 2, box_h + 2, Color::new(255, 60, 140, 100));

            let name_size = 28;
            let text_size = 22;
            let text_color = Color::new(245, 245, 250, 255);

            // Speaker name shadow + main
            d.draw_text(line.speaker, box_x + 27, box_y + 17, name_size, Color::BLACK);
            d.draw_text(line.speaker, box_x + 25, box_y + 15, name_size, line.color);

            // Robust wrapping for large text
            let words = line.text.split(' ');
            let mut wrapped_lines = Vec::new();
            let mut current_line = String::new();
            
            for word in words {
                if current_line.is_empty() {
                    current_line.push_str(word);
                } else if current_line.len() + word.len() < 52 {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    wrapped_lines.push(current_line);
                    current_line = String::from(word);
                }
            }
            if !current_line.is_empty() {
                wrapped_lines.push(current_line);
            }

            let mut text_y = box_y + 48;
            for wrapped in wrapped_lines {
                // Drop shadow
                d.draw_text(&wrapped, box_x + 27, text_y + 2, text_size, Color::BLACK);
                // Main text
                d.draw_text(&wrapped, box_x + 25, text_y, text_size, text_color);
                text_y += 28;
            }

            let prompt = "[SPACE] CONTINUE  /  [S] SKIP INTRO";
            let pr_w = d.measure_text(prompt, 14);
            d.draw_text(prompt, box_x + box_w - pr_w - 20, box_y + box_h - 22, 14, Color::new(160, 160, 180, 255));
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
        if rl.is_key_pressed(KeyboardKey::KEY_LEFT_BRACKET) {
            self.sfx.cycle_track(false);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_RIGHT_BRACKET) {
            self.sfx.cycle_track(true);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            self.sfx.toggle_pause();
        }
    }
}
