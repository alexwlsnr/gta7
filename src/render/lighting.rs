//! Lighting system: shadow mapping + directional light + fog via custom shaders.
use raylib::consts::CameraProjection;
use raylib::ffi::Vector3;
use raylib::prelude::*;
use crate::config::{sun_color, sun_direction, sun_position};

#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub pos: Vector3,
    pub color: Vector3,
    pub radius: f32,
}

pub struct LightingSystem {
    pub lit_shader: Shader,
    pub depth_shader: Shader,
    pub shadow_map: RenderTexture2D,
    pub shadow_camera: Camera3D,
    // Cached uniform locations for lit shader.
    loc_light_dir: i32,
    loc_light_color: i32,
    loc_ambient_color: i32,
    loc_fog_color: i32,
    loc_fog_density: i32,
    loc_camera_pos: i32,
    loc_shadow_map: i32,
    loc_light_space: i32,
    loc_window_glow: i32,
    loc_light_count: i32,
    loc_point_lights: [(i32, i32, i32); 6],
    // Cached uniform locations for depth shader.
    pub loc_depth_mvp: i32,
    // Current light space matrix (updated each frame).
    light_space: Matrix,
}

impl LightingSystem {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread) -> Self {
        let lit_shader = rl.load_shader(
            thread,
            Some("assets/shaders/lighting.vs"),
            Some("assets/shaders/lighting.fs"),
        );
        let lit_shader = if lit_shader.is_shader_valid() {
            lit_shader
        } else {
            rl.load_shader(thread, None, None)
        };

        let depth_shader = rl.load_shader(
            thread,
            Some("assets/shaders/depth.vs"),
            Some("assets/shaders/depth.fs"),
        );
        let depth_shader = if depth_shader.is_shader_valid() {
            depth_shader
        } else {
            rl.load_shader(thread, None, None)
        };

        let shadow_map = rl.load_render_texture(thread, 1024, 1024).unwrap();

        // Shadow camera: orthographic, follows player.
        let shadow_camera = Camera3D {
            position: Vector3 { x: 0.0, y: 200.0, z: 0.0 },
            target: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
            up: Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            fovy: 120.0,
            projection: CameraProjection::CAMERA_ORTHOGRAPHIC,
        };

        // Cache uniform locations.
        let loc_light_dir = lit_shader.get_shader_location("u_lightDir");
        let loc_light_color = lit_shader.get_shader_location("u_lightColor");
        let loc_ambient_color = lit_shader.get_shader_location("u_ambientColor");
        let loc_fog_color = lit_shader.get_shader_location("u_fogColor");
        let loc_fog_density = lit_shader.get_shader_location("u_fogDensity");
        let loc_camera_pos = lit_shader.get_shader_location("u_cameraPos");
        let loc_shadow_map = lit_shader.get_shader_location("u_shadowMap");
        let loc_light_space = lit_shader.get_shader_location("u_lightSpaceMatrix");
        let loc_depth_mvp = depth_shader.get_shader_location("mvp");

        let loc_window_glow = lit_shader.get_shader_location("u_windowGlow");
        let loc_light_count = lit_shader.get_shader_location("u_light_count");
        let mut loc_point_lights = [(0, 0, 0); 6];
        for (i, item) in loc_point_lights.iter_mut().enumerate() {
            let pos_loc = lit_shader.get_shader_location(&format!("u_light{}_pos", i));
            let col_loc = lit_shader.get_shader_location(&format!("u_light{}_color", i));
            let rad_loc = lit_shader.get_shader_location(&format!("u_light{}_radius", i));
            *item = (pos_loc, col_loc, rad_loc);
        }

        LightingSystem {
            lit_shader,
            depth_shader,
            shadow_map,
            shadow_camera,
            loc_light_dir,
            loc_light_color,
            loc_ambient_color,
            loc_fog_color,
            loc_fog_density,
            loc_camera_pos,
            loc_shadow_map,
            loc_light_space,
            loc_window_glow,
            loc_light_count,
            loc_point_lights,
            loc_depth_mvp,
            light_space: Matrix::identity(),
        }
    }

    /// Set the lit shader on model materials so draw_model/draw_model_ex use it.
    /// This avoids begin_shader_mode (which breaks immediate-mode draws).
    pub fn apply_to_materials(&self, assets: &mut crate::render::models::Assets) {
        use raylib::core::models::RaylibMaterial;
        assets.building_model.materials_mut()[0].set_shader(&self.lit_shader);
        assets.ground_model.materials_mut()[0].set_shader(&self.lit_shader);
        assets.plain_cube_model.materials_mut()[0].set_shader(&self.lit_shader);
        assets.carbon_cube_model.materials_mut()[0].set_shader(&self.lit_shader);
        assets.grill_cube_model.materials_mut()[0].set_shader(&self.lit_shader);
    }

    /// Compute the light space matrix (view + projection) for shadow mapping.
    fn compute_light_space_matrix(&self) -> Matrix {
        let view = Matrix::look_at(
            self.shadow_camera.position,
            self.shadow_camera.target,
            self.shadow_camera.up,
        );
        let half = self.shadow_camera.fovy as f64 / 2.0;
        let proj = Matrix::ortho(-half, half, -half, half, 1.0, 400.0);
        proj * view
    }

    /// Prepare the shadow pass: update the shadow camera and recompute the light
    /// space matrix. Does NOT enter render-texture mode — the caller owns that
    /// RAII guard so shadow casters can be drawn while the shadow map is active.
    pub fn prepare_shadow(&mut self, player_pos: Vector3, hour: f32) {
        let sun_pos = sun_position(hour, player_pos);

        // Update shadow camera to follow player, positioned at sun.
        self.shadow_camera.position = sun_pos;
        self.shadow_camera.target = player_pos;

        // Compute and store light space matrix.
        self.light_space = self.compute_light_space_matrix();
    }

    /// Returns the shadow camera for the caller to use in `begin_mode3D`.
    pub fn shadow_camera(&self) -> Camera3D {
        self.shadow_camera
    }

    /// Update lighting uniforms for the current frame.
    pub fn update_uniforms(&mut self, hour: f32, sky_bottom: Color, camera_pos: Vector3) {
        let dir = sun_direction(hour);
        let sun_col = sun_color(hour);

        // Light direction (from sun toward scene).
        self.lit_shader.set_shader_value(self.loc_light_dir, dir);
        // Light color (0..1 range).
        self.lit_shader.set_shader_value(
            self.loc_light_color,
            Vector3 {
                x: sun_col.r as f32 / 255.0,
                y: sun_col.g as f32 / 255.0,
                z: sun_col.b as f32 / 255.0,
            },
        );
        // Ambient = sky bottom color, dimmed more at night.
        let h = hour.rem_euclid(24.0);
        let ambient_mult = if !(6.0..=20.0).contains(&h) {
            0.2 // Night: low ambient
        } else if !(8.0..=18.0).contains(&h) {
            0.3 // Dawn/dusk
        } else {
            0.45 // Day: brighter ambient
        };
        self.lit_shader.set_shader_value(
            self.loc_ambient_color,
            Vector3 {
                x: sky_bottom.r as f32 / 255.0 * ambient_mult,
                y: sky_bottom.g as f32 / 255.0 * ambient_mult,
                z: sky_bottom.b as f32 / 255.0 * ambient_mult,
            },
        );
        // Fog color = sky bottom.
        self.lit_shader.set_shader_value(
            self.loc_fog_color,
            Vector3 {
                x: sky_bottom.r as f32 / 255.0,
                y: sky_bottom.g as f32 / 255.0,
                z: sky_bottom.b as f32 / 255.0,
            },
        );
        // Fog density: higher at night for atmosphere.
        let density: f32 = if !(6.0..=20.0).contains(&h) {
            0.015 // Night: thicker fog
        } else if !(8.0..=18.0).contains(&h) {
            0.010 // Dawn/dusk: moderate
        } else {
            0.005 // Day: light fog
        };
        self.lit_shader
            .set_shader_value(self.loc_fog_density, density);
        // Camera position for fog distance.
        self.lit_shader
            .set_shader_value(self.loc_camera_pos, camera_pos);
        // Light space matrix for shadow lookup.
        self.lit_shader
            .set_shader_value_matrix(self.loc_light_space, self.light_space);
        // Shadow map texture.
        self.lit_shader
            .set_shader_value_texture(self.loc_shadow_map, self.shadow_map.texture());

        // Window glow logic based on hour
        let window_glow = if !(6.0..=20.0).contains(&h) {
            1.0
        } else if (6.0..8.0).contains(&h) {
            1.0 - (h - 6.0) / 2.0
        } else if (18.0..=20.0).contains(&h) {
            (h - 18.0) / 2.0
        } else {
            0.0
        };
        self.lit_shader.set_shader_value(self.loc_window_glow, window_glow);
    }

    /// Get the light space matrix (for use in draw functions if needed).
    pub fn get_light_space_matrix(&self) -> Matrix {
        self.light_space
    }

    /// Update dynamic point lights in the shader.
    pub fn update_point_lights(&mut self, lights: &[PointLight]) {
        let count = lights.len().min(6);
        self.lit_shader.set_shader_value(self.loc_light_count, count as i32);
        for (i, light) in lights.iter().enumerate().take(count) {
            let (pos_loc, col_loc, rad_loc) = self.loc_point_lights[i];
            self.lit_shader.set_shader_value(pos_loc, light.pos);
            self.lit_shader.set_shader_value(col_loc, light.color);
            self.lit_shader.set_shader_value(rad_loc, light.radius);
        }
        // Fill remaining shader slots with inactive lights
        for i in count..6 {
            let (pos_loc, col_loc, rad_loc) = self.loc_point_lights[i];
            self.lit_shader.set_shader_value(pos_loc, Vector3::zero());
            self.lit_shader.set_shader_value(col_loc, Vector3::zero());
            self.lit_shader.set_shader_value(rad_loc, 0.0f32);
        }
    }
}
