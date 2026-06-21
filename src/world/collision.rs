//! Collision primitives: circle/AABB, ray/AABB, capsule/box.
use raylib::ffi::Vector3;

#[derive(Clone, Copy, Debug)]
pub struct AABB {
    pub min: Vector3,
    pub max: Vector3,
}

impl AABB {
    pub fn new(min: Vector3, max: Vector3) -> Self { AABB { min, max } }
    pub fn from_center(cx: f32, cy: f32, cz: f32, hx: f32, hy: f32, hz: f32) -> Self {
        AABB {
            min: Vector3 { x: cx - hx, y: cy - hy, z: cz - hz },
            max: Vector3 { x: cx + hx, y: cy + hy, z: cz + hz },
        }
    }
    pub fn center(&self) -> Vector3 {
        Vector3 {
            x: (self.min.x + self.max.x) * 0.5,
            y: (self.min.y + self.max.y) * 0.5,
            z: (self.min.z + self.max.z) * 0.5,
        }
    }
    pub fn half(&self) -> Vector3 {
        Vector3 {
            x: (self.max.x - self.min.x) * 0.5,
            y: (self.max.y - self.min.y) * 0.5,
            z: (self.max.z - self.min.z) * 0.5,
        }
    }
}

/// Resolve a circle (in XZ plane) against an AABB. Returns a corrective
/// translation to apply to the circle center to push it out, or zero.
pub fn circle_vs_aabb(px: f32, pz: f32, radius: f32, b: AABB) -> Vector3 {
    // Closest point on box to circle center (XZ).
    let cx = px.clamp(b.min.x, b.max.x);
    let cz = pz.clamp(b.min.z, b.max.z);
    let dx = px - cx;
    let dz = pz - cz;
    let d2 = dx * dx + dz * dz;
    if d2 >= radius * radius {
        return Vector3 { x: 0.0, y: 0.0, z: 0.0 };
    }
    if d2 > 1e-8 {
        let d = d2.sqrt();
        let push = radius - d;
        Vector3 { x: dx / d * push, y: 0.0, z: dz / d * push }
    } else {
        // Center is inside the box: push out along the least-penetration axis.
        let to_min_x = (px - b.min.x).abs();
        let to_max_x = (b.max.x - px).abs();
        let to_min_z = (pz - b.min.z).abs();
        let to_max_z = (b.max.z - pz).abs();
        let m = to_min_x.min(to_max_x).min(to_min_z).min(to_max_z);
        if m == to_min_x {
            Vector3 { x: -(to_min_x + radius), y: 0.0, z: 0.0 }
        } else if m == to_max_x {
            Vector3 { x: to_max_x + radius, y: 0.0, z: 0.0 }
        } else if m == to_min_z {
            Vector3 { x: 0.0, y: 0.0, z: -(to_min_z + radius) }
        } else {
            Vector3 { x: 0.0, y: 0.0, z: to_max_z + radius }
        }
    }
}

/// Slab method ray/AABB test. Returns hit distance `t` (>=0) and the face
/// normal (unit) if the ray intersects the box within `max_t`.
pub fn ray_vs_aabb(ro: Vector3, rd: Vector3, b: AABB, max_t: f32) -> Option<(f32, Vector3)> {
    let inv = |v: f32| if v.abs() < 1e-8 { f32::INFINITY.copysign(v) } else { 1.0 / v };
    let invx = inv(rd.x);
    let invy = inv(rd.y);
    let invz = inv(rd.z);

    let tx1 = (b.min.x - ro.x) * invx;
    let tx2 = (b.max.x - ro.x) * invx;
    let (tmin_x, tmax_x) = (tx1.min(tx2), tx1.max(tx2));
    let ty1 = (b.min.y - ro.y) * invy;
    let ty2 = (b.max.y - ro.y) * invy;
    let (tmin_y, tmax_y) = (ty1.min(ty2), ty1.max(ty2));
    let tz1 = (b.min.z - ro.z) * invz;
    let tz2 = (b.max.z - ro.z) * invz;
    let (tmin_z, tmax_z) = (tz1.min(tz2), tz1.max(tz2));

    let tmin = tmin_x.max(tmin_y).max(tmin_z);
    let tmax = tmax_x.min(tmax_y).min(tmax_z);

    if tmax < 0.0 || tmin > tmax || tmin > max_t {
        return None;
    }
    let t = if tmin >= 0.0 { tmin } else { tmax };
    if t < 0.0 || t > max_t {
        return None;
    }
    // Determine normal from which slab we entered.
    let p = Vector3 {
        x: ro.x + rd.x * t,
        y: ro.y + rd.y * t,
        z: ro.z + rd.z * t,
    };
    let c = b.center();
    let h = b.half();
    let n = Vector3 {
        x: if (p.x - c.x).abs() > h.x - 1e-3 { (p.x - c.x).signum() } else { 0.0 },
        y: 0.0,
        z: if (p.z - c.z).abs() > h.z - 1e-3 { (p.z - c.z).signum() } else { 0.0 },
    };
    Some((t, n))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_at(x: f32, z: f32, hx: f32, hz: f32) -> AABB {
        AABB::from_center(x, 1.0, z, hx, 1.0, hz)
    }

    #[test]
    fn circle_outside_pushes_back() {
        let b = box_at(0.0, 0.0, 1.0, 1.0);
        let p = circle_vs_aabb(3.0, 0.0, 0.5, b);
        assert!(p.x > 0.0);
        assert!((p.x - 1.5).abs() < 0.05, "push {}", p.x);
    }

    #[test]
    fn circle_clear_returns_zero() {
        let b = box_at(0.0, 0.0, 1.0, 1.0);
        let p = circle_vs_aabb(5.0, 5.0, 0.5, b);
        assert_eq!(p.x, 0.0);
        assert_eq!(p.z, 0.0);
    }

    #[test]
    fn circle_inside_pushes_least_axis() {
        let b = box_at(0.0, 0.0, 1.0, 1.0); // spans -1..1
        let p = circle_vs_aabb(0.0, 0.5, 0.5, b);
        // closer to +Z face (0.5) than others; push +Z
        assert!(p.z > 0.0, "got {:?}", p);
    }

    #[test]
    fn ray_hits_box() {
        let b = box_at(0.0, 0.0, 1.0, 1.0);
        let ro = Vector3 { x: -5.0, y: 1.0, z: 0.0 };
        let rd = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
        let r = ray_vs_aabb(ro, rd, b, 100.0);
        assert!(r.is_some());
        let (t, n) = r.unwrap();
        assert!((t - 4.0).abs() < 0.01, "t={t}");
        assert_eq!(n.x, -1.0);
    }

    #[test]
    fn ray_misses_box() {
        let b = box_at(0.0, 0.0, 1.0, 1.0);
        let ro = Vector3 { x: -5.0, y: 1.0, z: 5.0 };
        let rd = Vector3 { x: 1.0, y: 0.0, z: 0.0 };
        assert!(ray_vs_aabb(ro, rd, b, 100.0).is_none());
    }
}
