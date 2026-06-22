//! Particles, tracers, muzzle flashes, explosions, blood.
use raylib::prelude::*;
use raylib::ffi::Vector3;
use crate::mathx::{vadd, vsub, vscale};

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: Vector3,
    pub vel: Vector3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: Color,
    pub gravity: f32,
}

#[derive(Clone, Debug)]
pub struct Tracer {
    pub from: Vector3,
    pub to: Vector3,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Flash {
    pub pos: Vector3,
    pub life: f32,
}

#[derive(Clone, Debug)]
pub struct Skidmark {
    pub from: Vector3,
    pub to: Vector3,
    pub width: f32,
    pub life: f32,
    pub max_life: f32,
}

pub struct Fx {
    pub particles: Vec<Particle>,
    pub tracers: Vec<Tracer>,
    pub flashes: Vec<Flash>,
    pub skidmarks: Vec<Skidmark>,
}

impl Fx {
    pub fn new() -> Self {
        Fx {
            particles: Vec::new(),
            tracers: Vec::new(),
            flashes: Vec::new(),
            skidmarks: Vec::new(),
        }
    }
}

impl Default for Fx {
    fn default() -> Self {
        Self::new()
    }
}

impl Fx {

    pub fn burst(&mut self, pos: Vector3, count: usize, speed: f32, color: Color, life: f32, gravity: f32) {
        for _ in 0..count {
            let a = rand::random::<f32>() * std::f32::consts::TAU;
            let e = rand::random::<f32>() * std::f32::consts::PI * 0.5;
            let s = speed * (0.5 + rand::random::<f32>());
            let vel = Vector3 {
                x: a.cos() * e.cos() * s,
                y: e.sin() * s + 1.0,
                z: a.sin() * e.cos() * s,
            };
            self.particles.push(Particle {
                pos,
                vel,
                life,
                max_life: life,
                size: 0.1 + rand::random::<f32>() * 0.1,
                color,
                gravity,
            });
        }
    }

    pub fn blood(&mut self, pos: Vector3) {
        self.burst(pos, 14, 3.0, Color::new(170, 20, 20, 255), 0.6, 9.0);
    }

    pub fn explosion(&mut self, pos: Vector3) {
        self.burst(pos, 40, 6.0, Color::new(255, 160, 40, 255), 0.8, 4.0);
        self.burst(pos, 20, 3.0, Color::new(80, 80, 80, 255), 1.4, 1.0);
    }

    pub fn muzzle(&mut self, pos: Vector3) {
        self.flashes.push(Flash { pos, life: 0.05 });
    }

    pub fn tracer(&mut self, from: Vector3, to: Vector3) {
        self.tracers.push(Tracer { from, to, life: 0.08 });
    }

    pub fn add_skidmark(&mut self, from: Vector3, to: Vector3, width: f32, life: f32) {
        self.skidmarks.push(Skidmark {
            from,
            to,
            width,
            life,
            max_life: life,
        });
    }

    pub fn step(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.life -= dt;
            p.vel.y -= p.gravity * dt;
            p.pos = vadd(p.pos, vscale(p.vel, dt));
        }
        self.particles.retain(|p| p.life > 0.0);
        for t in &mut self.tracers { t.life -= dt; }
        self.tracers.retain(|t| t.life > 0.0);
        for f in &mut self.flashes { f.life -= dt; }
        self.flashes.retain(|f| f.life > 0.0);
        for s in &mut self.skidmarks { s.life -= dt; }
        self.skidmarks.retain(|s| s.life > 0.0);
    }

    pub fn draw(&self, d3: &mut impl RaylibDraw3D) {
        // Particles as small cubes.
        for p in &self.particles {
            let a = (p.life / p.max_life).clamp(0.0, 1.0);
            let c = Color::new(p.color.r, p.color.g, p.color.b, (255.0 * a) as u8);
            d3.draw_cube(p.pos, p.size, p.size, p.size, c);
        }
        // Tracers as bright lines.
        for t in &self.tracers {
            let a = (t.life / 0.08).clamp(0.0, 1.0);
            let c = Color::new(255, 240, 160, (200.0 * a) as u8);
            d3.draw_line3D(t.from, t.to, c);
        }
        // Muzzle flashes as bright spheres.
        for f in &self.flashes {
            d3.draw_sphere(f.pos, 0.18, Color::new(255, 240, 180, 220));
        }
        // Skidmarks as flat quads (made of two triangles).
        for s in &self.skidmarks {
            let a = (s.life / s.max_life).clamp(0.0, 1.0);
            let col = Color::new(20, 20, 20, (180.0 * a) as u8);
            
            let dir = vsub(s.to, s.from);
            let len = (dir.x * dir.x + dir.z * dir.z).sqrt();
            if len > 0.01 {
                let norm = Vector3 { x: dir.x / len, y: 0.0, z: dir.z / len };
                let perp = Vector3 { x: -norm.z, y: 0.0, z: norm.x };
                let w_half = s.width * 0.5;
                
                let p1 = vsub(s.from, vscale(perp, w_half));
                let p2 = vadd(s.from, vscale(perp, w_half));
                let p3 = vadd(s.to, vscale(perp, w_half));
                let p4 = vsub(s.to, vscale(perp, w_half));
                
                d3.draw_triangle3D(p1, p2, p3, col);
                d3.draw_triangle3D(p1, p3, p4, col);
                // Draw with reverse winding to handle backface culling
                d3.draw_triangle3D(p3, p2, p1, col);
                d3.draw_triangle3D(p4, p3, p1, col);
            }
        }
    }
}

/// Interpolate between two particle-system snapshots for render smoothing.
/// (We don't snapshot particles individually; fx are short-lived and visual-only,
/// so we skip interpolation and draw live state — they flicker at most one frame.)
pub fn _unused_interpolate(_a: &Fx, _b: &Fx, _t: f32) {}
