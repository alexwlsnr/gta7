//! Third-person follow camera: orbit on foot, chase in vehicle.
use raylib::ffi::{Vector3, Camera3D};
use crate::mathx::*;
use crate::player::Player;
use crate::vehicle::Vehicle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Follow,
    Free,
}

pub struct FollowCamera {
    pub pos: Vector3,
    pub target: Vector3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub height: f32,
    pub mode: Mode,
}

impl FollowCamera {
    pub fn is_free(&self) -> bool {
        matches!(self.mode, Mode::Free)
    }

    pub fn set_follow(&mut self) {
        self.mode = Mode::Follow;
    }

    /// Place the camera in free-fly mode at `pos` looking along `(yaw, pitch)`.
    /// Convention: yaw=0 -> forward=+Z, yaw=PI/2 -> forward=+X. Matches the
    /// existing follow camera (which sits at pivot.z - dist*cos(yaw) at yaw=0).
    pub fn set_free(&mut self, pos: Vector3, yaw: f32, pitch: f32) {
        self.mode = Mode::Free;
        self.pos = pos;
        let cp = pitch.cos();
        let sp = pitch.sin();
        self.target = Vector3 {
            x: pos.x + yaw.sin() * cp,
            y: pos.y + sp,
            z: pos.z + yaw.cos() * cp,
        };
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// Free-fly input. `input` carries keyboard + mouse state.
    /// `dt` is real time since the last update.
    ///
    /// Convention: yaw=0 -> forward=+Z, yaw=PI/2 -> forward=+X.
    /// Forward on the XZ plane is `(sin(yaw), 0, cos(yaw))`; right is
    /// `(cos(yaw), 0, -sin(yaw))`. Pitch is clamped to +/- 1.4 rad.
    pub fn update_free(&mut self, input: &crate::input::Input, dt: f32) {
        let speed = 8.0; // m/s
        let rot_speed = 1.5; // rad/s

        // Translation: WASD on horizontal plane relative to current yaw; E=up, Q=down.
        // Input convention: move_y is -1..+1 forward/back; move_x is -1..+1 strafe.
        let (mut mx, mut mz, mut my) = (0.0_f32, 0.0_f32, 0.0_f32);
        if input.move_y > 0.0 { mz += 1.0; }
        if input.move_y < 0.0 { mz -= 1.0; }
        if input.move_x > 0.0 { mx += 1.0; }
        if input.move_x < 0.0 { mx -= 1.0; }
        if input.ascend  { my += 1.0; }
        if input.descend { my -= 1.0; }

        // Normalize to unit length so diagonal speed is bounded.
        let len = (mx * mx + mz * mz + my * my).sqrt();
        if len > 0.0 {
            mx /= len; mz /= len; my /= len;
        }

        let sy = self.yaw.sin();
        let cy = self.yaw.cos();
        // pos += (mz * forward + mx * right) * speed * dt
        self.pos.x += (sy * mz + cy * mx) * speed * dt;
        self.pos.z += (cy * mz - sy * mx) * speed * dt;
        self.pos.y += my * speed * dt;

        // Yaw/pitch from mouse drag.
        self.yaw   -= input.look_dx * rot_speed;
        self.pitch += input.look_dy * rot_speed;
        self.pitch = clamp(self.pitch, -1.4, 1.4);

        // Update target so `forward()` and `to_camera3d()` work in free mode.
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        self.target = Vector3 {
            x: self.pos.x + sy * cp,
            y: self.pos.y + sp,
            z: self.pos.z + cy * cp,
        };
    }
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
            mode: Mode::Follow,
        }
    }
}


impl Default for FollowCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl FollowCamera {
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
        self.pitch += look_dy * sensitivity;

        let is_car = player.in_vehicle.is_some();
        // Clamping pitch: allow negative pitch (looking up) while keeping ground safety.
        self.pitch = clamp(self.pitch, if is_car { -0.3 } else { -0.85 }, if is_car { 0.8 } else { 1.2 });

        let (pivot, pivot_yaw): (Vector3, f32) = if let Some(vi) = player.in_vehicle {
            let v = &vehicles[vi];
            (v.pos, v.yaw)
        } else {
            (player.pos, player.yaw)
        };

        if is_car {
            // Chase cam: lag behind vehicle heading.
            self.dist = 11.0;
            self.height = 2.5; // base height at car level
            let target_yaw = pivot_yaw;
            self.yaw = lerp_angle(self.yaw, target_yaw, 3.0 * dt);
            self.pitch = lerp(self.pitch, 0.25, 2.0 * dt);
        } else {
            self.dist = 7.0;
            self.height = 1.5; // base height at player chest level
        }

        // Spherical to cartesian offset — NEGATED XZ so camera is BEHIND the target.
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        let offset = Vector3 {
            x: -sy * cp * self.dist,
            y: sp * self.dist + self.height,
            z: -cy * cp * self.dist,
        };
        let mut desired = Vector3 {
            x: pivot.x + offset.x,
            y: pivot.y + offset.y,
            z: pivot.z + offset.z,
        };
        
        // Ground safety: prevent camera from clipping underground.
        desired.y = desired.y.max(0.4);

        // Smooth follow.
        self.pos = vlerp(self.pos, desired, 8.0 * dt);

        // Dynamic target shifting: shift look target vertically in the opposite direction
        // of the camera pitch to allow looking straight up at the sky or down at the ground.
        let target_offset_y = if is_car {
            1.2 - self.pitch * 2.0
        } else {
            1.5 - self.pitch * 3.5 + player.recoil
        };

        self.target = vlerp(self.target, Vector3 {
            x: pivot.x,
            y: pivot.y + target_offset_y,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Input;

    #[test]
    fn free_mode_moves_on_w() {
        let mut cam = FollowCamera::new();
        cam.set_free(Vector3 { x: 0.0, y: 1.5, z: 0.0 }, 0.0, 0.0);
        let before = cam.pos.z;
        let mut input = Input::default();
        input.move_y = 1.0; // W: forward
        cam.update_free(&input, 0.1);
        // yaw=0 -> +Z direction.
        assert!(cam.pos.z > before, "forward should move +Z, got {} -> {}", before, cam.pos.z);
    }

    #[test]
    fn free_mode_pitch_is_clamped() {
        let mut cam = FollowCamera::new();
        cam.set_free(Vector3 { x: 0.0, y: 1.5, z: 0.0 }, 0.0, 0.0);
        let mut input = Input::default();
        input.look_dy = 10.0;
        for _ in 0..20 { cam.update_free(&input, 0.016); }
        assert!(cam.pitch <= 1.4, "pitch must clamp at +1.4, got {}", cam.pitch);
        assert!(cam.pitch >= -1.4, "pitch must clamp at -1.4, got {}", cam.pitch);
    }
}
