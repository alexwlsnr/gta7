//! Small math helpers used across the codebase.
use raylib::ffi::Vector3;

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut diff = (b - a) % std::f32::consts::TAU;
    if diff > std::f32::consts::PI {
        diff -= std::f32::consts::TAU;
    } else if diff < -std::f32::consts::PI {
        diff += std::f32::consts::TAU;
    }
    a + diff * t
}

pub fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

pub fn vec(x: f32, y: f32, z: f32) -> Vector3 {
    Vector3 { x, y, z }
}

pub fn vadd(a: Vector3, b: Vector3) -> Vector3 {
    Vector3 { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
}

pub fn vsub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3 { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}

pub fn vscale(a: Vector3, s: f32) -> Vector3 {
    Vector3 { x: a.x * s, y: a.y * s, z: a.z * s }
}

pub fn vlerp(a: Vector3, b: Vector3, t: f32) -> Vector3 {
    Vector3 {
        x: lerp(a.x, b.x, t),
        y: lerp(a.y, b.y, t),
        z: lerp(a.z, b.z, t),
    }
}

pub fn vlen(a: Vector3) -> f32 {
    (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
}

pub fn vlen_xz(a: Vector3) -> f32 {
    (a.x * a.x + a.z * a.z).sqrt()
}

pub fn vnorm(a: Vector3) -> Vector3 {
    let l = vlen(a);
    if l > 1e-6 {
        vscale(a, 1.0 / l)
    } else {
        Vector3 { x: 0.0, y: 0.0, z: 0.0 }
    }
}

pub fn vnorm_xz(a: Vector3) -> Vector3 {
    let l = vlen_xz(a);
    if l > 1e-6 {
        vscale(a, 1.0 / l)
    } else {
        Vector3 { x: 0.0, y: a.y, z: 0.0 }
    }
}

pub fn vdist(a: Vector3, b: Vector3) -> f32 {
    vlen(vsub(a, b))
}

pub fn vdist_xz(a: Vector3, b: Vector3) -> f32 {
    vlen_xz(vsub(a, b))
}

pub fn vdot(a: Vector3, b: Vector3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Angle (radians) of a direction in the XZ plane. 0 = +Z, PI/2 = +X.
pub fn yaw_from_dir(d: Vector3) -> f32 {
    d.x.atan2(d.z)
}

/// Unit direction in XZ plane from a yaw angle.
pub fn dir_from_yaw(yaw: f32) -> Vector3 {
    Vector3 { x: yaw.sin(), y: 0.0, z: yaw.cos() }
}

/// Smoothly approach `cur` toward `target` by at most `max_delta`.
pub fn approach(cur: f32, target: f32, max_delta: f32) -> f32 {
    if cur < target {
        (cur + max_delta).min(target)
    } else {
        (cur - max_delta).max(target)
    }
}

pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lerp_angle_wrap() {
        // 0.1 -> 6.2: shortest path wraps through -0.083 (6.2 ≈ TAU - 0.083)
        let a = lerp_angle(0.1, 6.2, 1.0);
        assert!(a.abs() < 0.15, "got {a}");
    }
    #[test] fn clamp_works() { assert_eq!(clamp(5.0, 0.0, 3.0), 3.0); }
    #[test] fn approach_clamps() {
        assert_eq!(approach(2.0, 5.0, 10.0), 5.0);
        assert_eq!(approach(2.0, 5.0, 1.0), 3.0);
    }
}
