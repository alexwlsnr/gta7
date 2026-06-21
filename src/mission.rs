//! Mission system: rotating objectives with rewards.
use raylib::ffi::Vector3;
use crate::mathx::vdist_xz;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionPhase {
    Inactive,
    GoToMarker,     // reach the pink marker
    Active,         // sub-objective in progress
    Complete,       // reward given, wait for next
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionType {
    ReachPoint,
    KillTarget,
    DeliverCar,
    Survive,
}

pub struct MissionState {
    pub phase: MissionPhase,
    pub mission_type: MissionType,
    pub marker: Vector3,
    pub target_pos: Vector3,
    pub target_idx: usize,    // index of target ped/cop
    pub timer: f32,           // for survive or timeout
    pub reward: i64,
    pub banner: String,
    pub banner_timer: f32,
}

impl MissionState {
    pub fn new() -> Self {
        MissionState {
            phase: MissionPhase::Inactive,
            mission_type: MissionType::ReachPoint,
            marker: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            target_pos: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            target_idx: 0,
            timer: 0.0,
            reward: 0,
            banner: String::new(),
            banner_timer: 0.0,
        }
    }

    pub fn show_banner(&mut self, text: &str) {
        self.banner = text.to_string();
        self.banner_timer = 4.0;
    }

    pub fn start_new(&mut self, player_pos: Vector3, world_half: f32) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mission_type = match rng.gen_range(0..4) {
            0 => MissionType::ReachPoint,
            1 => MissionType::KillTarget,
            2 => MissionType::DeliverCar,
            _ => MissionType::Survive,
        };
        // Place marker a good distance from player.
        let angle = rng.gen::<f32>() * std::f32::consts::TAU;
        let dist = rng.gen_range(world_half * 0.3..world_half * 0.8);
        let marker = Vector3 {
            x: (player_pos.x + angle.cos() * dist).clamp(-world_half + 5.0, world_half - 5.0),
            y: 0.0,
            z: (player_pos.z + angle.sin() * dist).clamp(-world_half + 5.0, world_half - 5.0),
        };
        self.mission_type = mission_type;
        self.marker = marker;
        self.phase = MissionPhase::GoToMarker;
        self.timer = 0.0;
        self.reward = match mission_type {
            MissionType::ReachPoint => 200,
            MissionType::KillTarget => 500,
            MissionType::DeliverCar => 400,
            MissionType::Survive => 350,
        };
        self.show_banner("New mission! Follow the pink marker.");
    }

    /// Update mission state. Returns (reward_to_give, should_spawn_target).
    pub fn update(
        &mut self,
        dt: f32,
        player_pos: Vector3,
        target_alive: Option<bool>,
        in_vehicle: bool,
    ) -> (i64, bool) {
        if self.banner_timer > 0.0 {
            self.banner_timer -= dt;
        }
        if self.phase == MissionPhase::Inactive {
            return (0, false);
        }
        match self.phase {
            MissionPhase::GoToMarker => {
                if vdist_xz(player_pos, self.marker) < 5.0 {
                    self.phase = MissionPhase::Active;
                    self.timer = 0.0;
                    match self.mission_type {
                        MissionType::ReachPoint => {
                            self.phase = MissionPhase::Complete;
                            self.show_banner("Reached! Reward earned.");
                            return (self.reward, false);
                        }
                        MissionType::KillTarget => {
                            self.show_banner("Kill the target!");
                            return (0, true); // spawn target
                        }
                        MissionType::DeliverCar => {
                            if in_vehicle {
                                self.show_banner("Deliver this car to the marker!");
                                // Set a new delivery point.
                                self.marker = Vector3 {
                                    x: -player_pos.x * 0.5,
                                    y: 0.0,
                                    z: -player_pos.z * 0.5,
                                };
                            } else {
                                self.show_banner("Get in a car first!");
                                self.phase = MissionPhase::GoToMarker;
                            }
                            return (0, false);
                        }
                        MissionType::Survive => {
                            self.show_banner("Survive for 30 seconds!");
                            self.timer = 30.0;
                            return (0, false);
                        }
                    }
                }
            }
            MissionPhase::Active => {
                match self.mission_type {
                    MissionType::KillTarget => {
                        if let Some(alive) = target_alive {
                            if !alive {
                                self.phase = MissionPhase::Complete;
                                self.show_banner("Target eliminated! Reward earned.");
                                return (self.reward, false);
                            }
                        }
                    }
                    MissionType::DeliverCar => {
                        if in_vehicle && vdist_xz(player_pos, self.marker) < 5.0 {
                            self.phase = MissionPhase::Complete;
                            self.show_banner("Delivered! Reward earned.");
                            return (self.reward, false);
                        }
                    }
                    MissionType::Survive => {
                        self.timer -= dt;
                        if self.timer <= 0.0 {
                            self.phase = MissionPhase::Complete;
                            self.show_banner("Survived! Reward earned.");
                            return (self.reward, false);
                        }
                    }
                    _ => {}
                }
            }
            MissionPhase::Complete => {
                self.phase = MissionPhase::Inactive;
                self.show_banner("Mission complete! Find next marker.");
            }
            MissionPhase::Inactive => {}
        }
        (0, false)
    }

    pub fn has_active_marker(&self) -> bool {
        matches!(self.phase, MissionPhase::GoToMarker | MissionPhase::Active)
    }
}
