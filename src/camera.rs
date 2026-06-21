//! Third-person follow camera: orbit on foot, chase in vehicle.
use raylib::ffi::{Vector3, Camera3D};
use crate::mathx::*;
use crate::player::Player;
use crate::vehicle::Vehicle;

pub struct FollowCamera {
    pub pos: Vector3,
    pub target: Vector3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub height: f32,
}

impl FollowCamera {
    pub fn new() -> Self {
        FollowCamera {
            pos: Vector3 { x: 0.0, y: 10.0, z: -15.0 },
            target: Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            yaw: 0.0,
            pitch: 0.35,
            dist: 8.0,
            height: 3.0,
        }
    }

    /// Update camera based on player state (on foot or in vehicle).
    /// `look_dx/look_dy` are accumulated mouse deltas for this logic step.
    pub fn update(
        &mut self,
        player: &Player,
        vehicles: &[Vehicle],
        look_dx: f32,
        look_dy: f32,
        sensitivity: f32,
        dt: f32,
    ) {
        // Orbit control via mouse.
        self.yaw -= look_dx * sensitivity;
        self.pitch -= look_dy * sensitivity;
        self.pitch = clamp(self.pitch, 0.05, 1.3);

        let (pivot, pivot_yaw): (Vector3, f32) = if let Some(vi) = player.in_vehicle {
            let v = &vehicles[vi];
            (v.pos, v.yaw)
        } else {
            (player.pos, player.yaw)
        };

        if player.in_vehicle.is_some() {
            // Chase cam: lag behind vehicle heading.
            self.dist = 11.0;
            self.height = 5.0;
            let target_yaw = pivot_yaw + std::f32::consts::PI;
            self.yaw = lerp_angle(self.yaw, target_yaw, 3.0 * dt);
            self.pitch = lerp(self.pitch, 0.35, 2.0 * dt);
        } else {
            self.dist = 7.0;
            self.height = 3.5;
        }

        // Spherical to cartesian offset.
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        let offset = Vector3 {
            x: sy * cp * self.dist,
            y: sp * self.dist + self.height,
            z: cy * cp * self.dist,
        };
        let desired = Vector3 {
            x: pivot.x + offset.x,
            y: pivot.y + offset.y,
            z: pivot.z + offset.z,
        };
        // Smooth follow.
        self.pos = vlerp(self.pos, desired, 8.0 * dt);
        self.target = vlerp(self.target, Vector3 {
            x: pivot.x,
            y: pivot.y + 1.5 + player.recoil,
            z: pivot.z,
        }, 10.0 * dt);
    }

    pub fn to_camera3d(&self) -> Camera3D {
        Camera3D {
            position: self.pos,
            target: self.target,
            up: Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            fovy: 60.0,
            projection: raylib::ffi::CameraProjection::CAMERA_PERSPECTIVE as i32,
        }
    }

    /// Forward direction (for shooting / aiming).
    pub fn forward(&self) -> Vector3 {
        vnorm(vsub(self.target, self.pos))
    }
}
