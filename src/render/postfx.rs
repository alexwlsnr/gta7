//! Post-processing pipeline: renders scene to FBO, chains fullscreen shader passes.
//!
//! Bloom pipeline (Task 2):
//!   scene_fbo --bright_extract--> bright_fbo (half-res)
//!   bright_fbo --blur H--> blur_fbo[0] --blur V--> blur_fbo[1]   (x2 iterations)
//!   scene_fbo + blur_fbo[1] --bloom_composite--> output_fbo (full-res)
//!
//! `process()` runs all FBO passes (needs `&mut RaylibHandle` for
//! `begin_texture_mode`) and must be called BEFORE `begin_drawing`. `blit()`
//! copies the final output_fbo to the screen inside `begin_drawing`.
use raylib::prelude::*;
use crate::postfx_mask::PostFxMask;

pub struct PostFx {
    pub scene_fbo: RenderTexture2D,
    /// Half-res: holds the bright-extracted pixels downsampled from the scene.
    bright_fbo: RenderTexture2D,
    /// Half-res ping-pong pair for separable Gaussian blur.
    blur_fbo: [RenderTexture2D; 2],
    /// Full-res final composited result; blitted to screen by `blit()`.
    output_fbo: RenderTexture2D,

    bright_shader: Shader,
    blur_shader: Shader,
    bloom_shader: Shader,
    /// CRT aesthetic pass (chromatic aberration, scanlines, vignette, ACES,
    /// film grain) — the final post-processing pass before `blit()`.
    crt_shader: Shader,
    /// Sky dome shader (gradient + procedural starfield). Applied per-frame
    /// via `begin_shader_mode` around the sky dome `draw_model` call.
    pub sky_shader: Shader,

    // Cached uniform locations. `loc_blur_direction` and `loc_bloom_bloom` are
    // used per-frame; the rest are set once at load and retained for future
    // runtime tuning (e.g. a brightness/bloom-strength slider).
    #[allow(dead_code)]
    loc_threshold: i32,
    #[allow(dead_code)]
    loc_soft_knee: i32,
    loc_blur_direction: i32,
    #[allow(dead_code)]
    loc_bloom_strength: i32,
    /// Sampler location for `texture1` (the bloom buffer) in the composite shader.
    loc_bloom_bloom: i32,
    loc_crt_time: i32,
    /// Resolution uniform for the CRT scanline spacing.
    loc_crt_resolution: i32,
    /// Sky gradient top color uniform (`u_skyTop`).
    loc_sky_top: i32,
    /// Sky gradient bottom color uniform (`u_skyBottom`).
    loc_sky_bottom: i32,
    /// Starfield visibility uniform (`u_starAlpha`): 0 = day, 1 = night.
    loc_star_alpha: i32,
    /// God ray radial-blur shader (32 samples from the sun's screen position).
    god_rays_shader: Shader,
    /// `u_sunScreenPos` uniform location — the sun's projected position in UV space.
    loc_gr_sun_pos: i32,
    /// `u_intensity` uniform location — 0 disables the pass, up to 0.6 at dawn/dusk.
    loc_gr_intensity: i32,
    /// Cached sun position in screen UV space (0..1, 0..1), set per-frame via `set_god_rays`.
    sun_screen_pos: Vector2,
    /// Cached god ray intensity (0..0.6), set per-frame via `set_god_rays`. Values
    /// below 0.01 short-circuit the shader pass entirely.
    god_ray_intensity: f32,
    /// Screen-space reflection shader. Simplified first pass: no depth texture
    /// (raylib's `load_render_texture` creates a depth renderbuffer, not a
    /// sampleable texture). Uses a 24-step screen-space vertical march with
    /// color-based sky detection and a Sobel-like normal estimate. Skipped
    /// entirely when `ssr_wetness < 0.01`.
    ssr_shader: Shader,
    /// Full-res scratch FBO for the SSR pass. Read from `output_fbo`, written
    /// to `ssr_fbo`, then blitted back to `output_fbo`. We need a separate
    /// FBO because `begin_texture_mode` mutably borrows its target and we
    /// can't read+write `output_fbo` in the same pass (mirrors the CRT and
    /// god_rays patterns).
    ssr_fbo: RenderTexture2D,
    /// `u_wetness` uniform location — 0 disables the pass entirely.
    loc_ssr_wetness: i32,
    /// `u_resolution` uniform location — texel size for the Sobel kernel.
    loc_ssr_resolution: i32,
    /// `u_proj` uniform location — projection matrix. Reserved for a future
    /// depth-texture upgrade; not currently used by the simplified shader.
    #[allow(dead_code)]
    loc_ssr_proj: i32,
    /// `u_invViewProj` uniform location — inverse view-projection. Reserved
    /// for the depth-texture upgrade.
    #[allow(dead_code)]
    loc_ssr_inv_view_proj: i32,
    /// `u_cameraPos` uniform location — world-space camera position. Reserved
    /// for the depth-texture upgrade.
    #[allow(dead_code)]
    loc_ssr_camera_pos: i32,
    /// Cached wetness scalar (0..0.8), set per-frame via `set_ssr_data`.
    /// Below 0.01 the SSR pass is short-circuited.
    ssr_wetness: f32,
    /// Cached projection matrix, set per-frame. Currently unused by the
    /// simplified shader but stored for a future depth-texture pass.
    #[allow(dead_code)]
    ssr_proj: Matrix,
    /// Cached inverse view-projection matrix, set per-frame. Currently unused
    /// by the simplified shader.
    #[allow(dead_code)]
    ssr_inv_view_proj: Matrix,
    /// Cached world-space camera position, set per-frame. Currently unused.
    #[allow(dead_code)]
    ssr_camera_pos: Vector3,
    width: i32,
    height: i32,
    half_width: i32,
    half_height: i32,
    /// Per-pass disable flags set by the test harness. When a flag is set,
    /// the corresponding pass is skipped; bloom-off additionally performs
    /// a verbatim scene-fbo -> output-fbo copy so output_fbo stays valid.
    pub disabled: PostFxMask,
}

impl PostFx {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let scene_fbo = rl.load_render_texture(thread, width as u32, height as u32).unwrap();
        scene_fbo.texture().set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);

        let half_width = (width / 2).max(1);
        let half_height = (height / 2).max(1);
        let bright_fbo = rl
            .load_render_texture(thread, half_width as u32, half_height as u32)
            .unwrap();
        let blur_fbo = [
            rl.load_render_texture(thread, half_width as u32, half_height as u32)
                .unwrap(),
            rl.load_render_texture(thread, half_width as u32, half_height as u32)
                .unwrap(),
        ];

        // Bilinear filtering on every intermediate target so downsampling and
        // blur ping-pongs sample smoothly.
        bright_fbo
            .texture()
            .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        for bf in &blur_fbo {
            bf.texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        }
        let output_fbo = rl
            .load_render_texture(thread, width as u32, height as u32)
            .unwrap();
        // Full-res scratch FBO for the SSR pass. Created once at load and
        // recreated in `resize_if_needed` when the window resizes.
        let ssr_fbo = rl
            .load_render_texture(thread, width as u32, height as u32)
            .unwrap();
        output_fbo
            .texture()
            .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        ssr_fbo
            .texture()
            .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);

        // Load shaders, falling back to the passthrough default if a file is
        // missing or fails to compile (matches LightingSystem's convention —
        // keeps the game rendering instead of crashing).
        let mut bright_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/bright_extract.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        let blur_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/blur.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        let mut bloom_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/bloom_composite.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        let mut crt_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/crt_post.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        // Sky dome shader: vertex passes the world-space direction to the
        // fragment stage; fragment builds the gradient + samples the starfield.
        // Loaded with both stages (unlike the fullscreen post passes, which are
        // fragment-only and reuse raylib's default billboard vertex shader).
        let sky_shader = {
            let s = rl.load_shader(
                thread,
                Some("assets/shaders/sky.vs"),
                Some("assets/shaders/sky.fs"),
            );
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        // God rays shader: 32-sample radial blur from the sun's projected screen
        // position. Loads as a passthrough fallback if the file is missing.
        let god_rays_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/god_rays.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };
        // SSR shader: 24-step screen-space vertical march with color-based
        // sky detection and a Sobel-like normal estimate. Passthrough
        // fallback if the file is missing or fails to compile.
        let mut ssr_shader = {
            let s = rl.load_shader(thread, None, Some("assets/shaders/ssr.fs"));
            if s.is_shader_valid() { s } else { rl.load_shader(thread, None, None) }
        };

        // Cache uniform locations (-1 = not found / inactive).
        let loc_threshold = bright_shader.get_shader_location("u_threshold");
        let loc_soft_knee = bright_shader.get_shader_location("u_softKnee");
        let loc_blur_direction = blur_shader.get_shader_location("u_direction");
        let loc_bloom_strength = bloom_shader.get_shader_location("u_bloomStrength");
        let loc_bloom_bloom = bloom_shader.get_shader_location("texture1");
        let loc_crt_time = crt_shader.get_shader_location("u_time");
        let loc_crt_resolution = crt_shader.get_shader_location("u_resolution");
        let loc_sky_top = sky_shader.get_shader_location("u_skyTop");
        let loc_sky_bottom = sky_shader.get_shader_location("u_skyBottom");
        let loc_star_alpha = sky_shader.get_shader_location("u_starAlpha");
        let loc_gr_sun_pos = god_rays_shader.get_shader_location("u_sunScreenPos");
        let loc_gr_intensity = god_rays_shader.get_shader_location("u_intensity");
        // SSR uniforms. `u_wetness` and `u_resolution` are the only ones the
        // simplified shader actually reads; `u_proj` / `u_invViewProj` /
        // `u_cameraPos` locations are cached so a future depth-texture
        // upgrade can upload them without re-querying the shader.
        let loc_ssr_wetness = ssr_shader.get_shader_location("u_wetness");
        let loc_ssr_resolution = ssr_shader.get_shader_location("u_resolution");
        let loc_ssr_proj = ssr_shader.get_shader_location("u_proj");
        let loc_ssr_inv_view_proj = ssr_shader.get_shader_location("u_invViewProj");
        let loc_ssr_camera_pos = ssr_shader.get_shader_location("u_cameraPos");

        // Default uniform values.
        bright_shader.set_shader_value(loc_threshold, 0.85f32);
        bright_shader.set_shader_value(loc_soft_knee, 0.15f32);
        bloom_shader.set_shader_value(loc_bloom_strength, 0.4f32);
        crt_shader.set_shader_value(
            loc_crt_resolution,
            Vector2::new(width as f32, height as f32),
        );
        // SSR defaults. Wetness is updated per-frame via `set_ssr_data`;
        // resolution is fixed at load (Texel size only changes on resize).
        ssr_shader.set_shader_value(loc_ssr_wetness, 0.0f32);
        ssr_shader.set_shader_value(
            loc_ssr_resolution,
            Vector2::new(width as f32, height as f32),
        );

        Self {
            scene_fbo,
            bright_fbo,
            blur_fbo,
            output_fbo,
            bright_shader,
            blur_shader,
            bloom_shader,
            crt_shader,
            sky_shader,
            god_rays_shader,
            loc_threshold,
            loc_soft_knee,
            loc_blur_direction,
            loc_bloom_strength,
            loc_bloom_bloom,
            loc_crt_time,
            loc_crt_resolution,
            loc_sky_top,
            loc_sky_bottom,
            loc_star_alpha,
            loc_gr_sun_pos,
            loc_gr_intensity,
            sun_screen_pos: Vector2::new(0.5, 0.5),
            god_ray_intensity: 0.0,
            ssr_shader,
            ssr_fbo,
            loc_ssr_wetness,
            loc_ssr_resolution,
            loc_ssr_proj,
            loc_ssr_inv_view_proj,
            loc_ssr_camera_pos,
            ssr_wetness: 0.0,
            ssr_proj: Matrix::identity(),
            ssr_inv_view_proj: Matrix::identity(),
            ssr_camera_pos: Vector3::new(0.0, 0.0, 0.0),
            width,
            height,
            half_width,
            half_height,
            disabled: PostFxMask::none(),
        }
    }

    /// Replace the per-pass disable mask. The test harness calls this once
    /// after construction to apply the `--disable` CSV from the CLI; from
    /// that point on `process()` gates each pass behind the corresponding
    /// flag for the lifetime of the `PostFx`.
    pub fn set_disabled(&mut self, mask: PostFxMask) {
        self.disabled = mask;
    }

    /// Set the per-frame sky dome uniforms: gradient top/bottom colors and the
    /// starfield visibility alpha (0 = day, 1 = night). Call this just before
    /// drawing the sky dome inside `begin_shader_mode(sky_shader)`.
    pub fn set_sky_uniforms(
        &mut self,
        sky_top: Vector3,
        sky_bottom: Vector3,
        star_alpha: f32,
    ) {
        self.sky_shader.set_shader_value(self.loc_sky_top, sky_top);
        self.sky_shader
            .set_shader_value(self.loc_sky_bottom, sky_bottom);
        self.sky_shader
            .set_shader_value(self.loc_star_alpha, star_alpha);
    }

    /// Set the god-ray inputs for the current frame: the sun's screen UV position
    /// (0..1, 0..1) and the intensity scalar (0..0.6). Call this BEFORE `process()`
    /// so the values are cached when the god ray pass runs. The pass is skipped
    /// entirely when intensity is below 0.01, so this is effectively free at
    /// noon and night.
    pub fn set_god_rays(&mut self, sun_pos: Vector2, intensity: f32) {
        self.sun_screen_pos = sun_pos;
        self.god_ray_intensity = intensity;
    }

    /// Set the SSR inputs for the current frame: camera projection matrix,
    /// inverse view-projection, world-space camera position, and a wetness
    /// scalar (0 = no reflection / daytime, 0.8 = full nighttime). Call this
    /// BEFORE `process()` so the values are cached when the SSR pass runs.
    /// The pass is short-circuited entirely when `wetness < 0.01`, so it's
    /// effectively free during the day.
    ///
    /// `proj`, `inv_view_proj`, and `camera_pos` are currently unused by the
    /// simplified shader (no depth texture), but are stored so a future
    /// depth-texture upgrade can upload them without re-querying the shader.
    pub fn set_ssr_data(
        &mut self,
        proj: Matrix,
        inv_view_proj: Matrix,
        camera_pos: Vector3,
        wetness: f32,
    ) {
        self.ssr_proj = proj;
        self.ssr_inv_view_proj = inv_view_proj;
        self.ssr_camera_pos = camera_pos;
        self.ssr_wetness = wetness;
        self.ssr_shader.set_shader_value(self.loc_ssr_wetness, wetness);
    }

    /// Borrow the sky shader for `begin_shader_mode` around the sky dome draw.
    pub fn sky_shader(&mut self) -> &mut Shader {
        &mut self.sky_shader
    }

    /// Run all post-processing passes, outputting to the internal output FBO.
    /// Call this BEFORE `begin_drawing`, while `rl` is still available.
    pub fn process(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let full_src = Rectangle::new(0.0, 0.0, self.width as f32, -(self.height as f32));
        let full_dst = Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32);
        let half_src = Rectangle::new(
            0.0,
            0.0,
            self.half_width as f32,
            -(self.half_height as f32),
        );
        let half_dst = Rectangle::new(0.0, 0.0, self.half_width as f32, self.half_height as f32);

        if self.disabled.bloom {
            // Bloom off: copy scene_fbo verbatim into output_fbo so the
            // downstream passes (SSR / god rays / CRT) and the final blit
            // always have a valid texture to read.
            let scene_tex = self.scene_fbo.texture().clone();
            {
                let mut ot = rl.begin_texture_mode(thread, &mut self.output_fbo);
                ot.clear_background(Color::BLACK);
                ot.draw_texture_pro(
                    scene_tex,
                    full_src,
                    full_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        } else {
        // Pass 1: Bright extract (scene_fbo -> bright_fbo, downsampled to half-res).
        {
            let mut bt = rl.begin_texture_mode(thread, &mut self.bright_fbo);
            bt.clear_background(Color::BLACK);
            {
                let mut bs = bt.begin_shader_mode(&mut self.bright_shader);
                bs.draw_texture_pro(
                    self.scene_fbo.texture(),
                    full_src,
                    half_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        }

        // Passes 2-3: Separable Gaussian blur, two iterations (H then V each).
        // Iteration 0 reads from bright_fbo; iteration 1 re-blurs the prior result.
        for iteration in 0..2 {
            // `texture()` returns a borrow into the FBO; clone the (Copy-like)
            // handle so the borrow ends before we mutably borrow `blur_fbo[0]`.
            let src_tex = if iteration == 0 {
                self.bright_fbo.texture().clone()
            } else {
                self.blur_fbo[1].texture().clone()
            };

            // Horizontal: src -> blur_fbo[0]
            self.blur_shader.set_shader_value(
                self.loc_blur_direction,
                Vector2::new(1.0 / self.half_width as f32, 0.0),
            );
            {
                let mut bt = rl.begin_texture_mode(thread, &mut self.blur_fbo[0]);
                bt.clear_background(Color::BLACK);
                {
                    let mut bs = bt.begin_shader_mode(&mut self.blur_shader);
                    bs.draw_texture_pro(
                        src_tex,
                        half_src,
                        half_dst,
                        Vector2::zero(),
                        0.0,
                        Color::WHITE,
                    );
                }
            }
            self.blur_shader.set_shader_value(
                self.loc_blur_direction,
                Vector2::new(0.0, 1.0 / self.half_height as f32),
            );
            // Snapshot blur_fbo[0]'s texture before mutably borrowing blur_fbo[1].
            let blur0_tex = self.blur_fbo[0].texture().clone();
            {
                let mut bt = rl.begin_texture_mode(thread, &mut self.blur_fbo[1]);
                bt.clear_background(Color::BLACK);
                {
                    let mut bs = bt.begin_shader_mode(&mut self.blur_shader);
                    bs.draw_texture_pro(
                        blur0_tex,
                        half_src,
                        half_dst,
                        Vector2::zero(),
                        0.0,
                        Color::WHITE,
                    );
                }
            }
        }

        // Pass 4: Bloom composite (scene + bloom -> output_fbo).
        // raylib auto-binds the draw_texture_pro source as `texture0` (scene).
        // For `texture1` (bloom), we must manually bind it to texture unit 1
        // and set the uniform to the UNIT INDEX (1), not the texture ID.
        // raylib's SetShaderValueTexture sets the wrong value (texture ID),
        // so we use FFI to bind properly and set_shader_value for the unit index.
        let bloom_tex = self.blur_fbo[1].texture().clone();
        self.bloom_shader.set_shader_value(self.loc_bloom_bloom, 1i32);
        unsafe {
            raylib::ffi::rlActiveTextureSlot(1);
            raylib::ffi::rlEnableTexture(bloom_tex.id);
        }
        {
            let mut ct = rl.begin_texture_mode(thread, &mut self.output_fbo);
            ct.clear_background(Color::BLACK);
            {
                let mut cs = ct.begin_shader_mode(&mut self.bloom_shader);
                cs.draw_texture_pro(
                    self.scene_fbo.texture(),
                    full_src,
                    full_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        }
        // Unbind unit 1 so it doesn't leak into subsequent passes
        unsafe {
            raylib::ffi::rlActiveTextureSlot(1);
            raylib::ffi::rlDisableTexture();
        }
        }
        // Pass 4b: SSR (output_fbo -> ssr_fbo -> output_fbo).
        // 24-step screen-space vertical march that mixes a small fraction of
        // the colors above each pixel back into the base, weighted by
        // `ssr_wetness`. Skipped entirely when wetness is below 0.01 so the
        // day-time path is a no-op. Uses `ssr_fbo` as the scratch target so
        // we don't conflict with the CRT pass's `scene_fbo` scratch usage.
        if !self.disabled.ssr && self.ssr_wetness > 0.01 {
            // Snapshot output_fbo's texture so the borrow ends before we
            // mutably borrow ssr_fbo (same pattern as the god_rays pass).
            let output_tex = self.output_fbo.texture().clone();
            {
                let mut st = rl.begin_texture_mode(thread, &mut self.ssr_fbo);
                st.clear_background(Color::BLACK);
                {
                    let mut ss = st.begin_shader_mode(&mut self.ssr_shader);
                    ss.draw_texture_pro(
                        output_tex,
                        full_src,
                        full_dst,
                        Vector2::zero(),
                        0.0,
                        Color::WHITE,
                    );
                }
            }
            // Blit ssr_fbo (SSR result) back to output_fbo without a shader.
            let ssr_tex = self.ssr_fbo.texture().clone();
            {
                let mut ot = rl.begin_texture_mode(thread, &mut self.output_fbo);
                ot.draw_texture_pro(
                    ssr_tex,
                    full_src,
                    full_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        }
        // Pass 4c: God rays (output_fbo -> scene_fbo temp -> output_fbo).
        // Runs only when the configured intensity clears 0.01 (i.e. at dawn/dusk);
        // otherwise output_fbo is left untouched. Uses scene_fbo as a scratch
        // target for the same reason as the CRT pass — `begin_texture_mode`
        // mutably borrows its target, so we cannot read and write output_fbo
        // in the same pass.
        if !self.disabled.god_rays && self.god_ray_intensity > 0.01 {
            self.god_rays_shader
                .set_shader_value(self.loc_gr_sun_pos, self.sun_screen_pos);
            self.god_rays_shader
                .set_shader_value(self.loc_gr_intensity, self.god_ray_intensity);
            // Snapshot output_fbo's texture so the borrow ends before we
            // mutably borrow scene_fbo (same pattern as the CRT pass).
            let output_tex = self.output_fbo.texture().clone();
            {
                let mut st = rl.begin_texture_mode(thread, &mut self.scene_fbo);
                st.clear_background(Color::BLACK);
                {
                    let mut gs = st.begin_shader_mode(&mut self.god_rays_shader);
                    gs.draw_texture_pro(
                        output_tex,
                        full_src,
                        full_dst,
                        Vector2::zero(),
                        0.0,
                        Color::WHITE,
                    );
                }
            }
            // Blit scene_fbo (god ray result) back to output_fbo without a shader.
            let scene_tex = self.scene_fbo.texture().clone();
            {
                let mut ot = rl.begin_texture_mode(thread, &mut self.output_fbo);
                ot.draw_texture_pro(
                    scene_tex,
                    full_src,
                    full_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        }
        // Pass 5: CRT post filter (output_fbo -> scene_fbo temp -> output_fbo).
        // `begin_texture_mode` borrows the destination FBO mutably, so we can't
        // read and write `output_fbo` in one pass. `scene_fbo` is free after the
        // bloom composite, so use it as a scratch target, then copy back.
        // Skipped entirely when `disabled.crt` is set; output_fbo then keeps
        // whatever the previous pass (bloom / SSR / god rays) wrote.
        if !self.disabled.crt {
            self.crt_shader
                .set_shader_value(self.loc_crt_time, rl.get_time() as f32);
            self.crt_shader.set_shader_value(
                self.loc_crt_resolution,
                Vector2::new(self.width as f32, self.height as f32),
            );
            // Snapshot output_fbo's texture so the borrow ends before we mutably
            // borrow scene_fbo (matches the blur pass pattern).
            let output_tex = self.output_fbo.texture().clone();
            {
                let mut st = rl.begin_texture_mode(thread, &mut self.scene_fbo);
                st.clear_background(Color::BLACK);
                {
                    let mut cs = st.begin_shader_mode(&mut self.crt_shader);
                    cs.draw_texture_pro(
                        output_tex,
                        full_src,
                        full_dst,
                        Vector2::zero(),
                        0.0,
                        Color::WHITE,
                    );
                }
            }
            // Blit scene_fbo (CRT result) back to output_fbo without a shader.
            let scene_tex = self.scene_fbo.texture().clone();
            {
                let mut ot = rl.begin_texture_mode(thread, &mut self.output_fbo);
                ot.draw_texture_pro(
                    scene_tex,
                    full_src,
                    full_dst,
                    Vector2::zero(),
                    0.0,
                    Color::WHITE,
                );
            }
        }
    }

    /// Blit the final processed result (output_fbo) to the screen.
    /// Call this inside `begin_drawing`. Uses the actual screen dimensions for
    /// the destination so fullscreen works when the FBO and window differ.
    pub fn blit(&self, d: &mut RaylibDrawHandle) {
        let src = Rectangle::new(0.0, 0.0, self.width as f32, -(self.height as f32));
        let screen_w = d.get_screen_width() as f32;
        let screen_h = d.get_screen_height() as f32;
        let dst = Rectangle::new(0.0, 0.0, screen_w, screen_h);
        d.draw_texture_pro(
            self.output_fbo.texture(),
            src,
            dst,
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
    }

    /// Recreate all FBOs if the window size has changed. Call this each frame
    /// before rendering — it's a no-op when the size matches.
    pub fn resize_if_needed(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        screen_w: i32,
        screen_h: i32,
    ) {
        if screen_w != self.width || screen_h != self.height {
            self.scene_fbo = rl
                .load_render_texture(thread, screen_w as u32, screen_h as u32)
                .unwrap();
            self.scene_fbo
                .texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);

            let half_w = (screen_w / 2).max(1);
            let half_h = (screen_h / 2).max(1);
            self.bright_fbo = rl
                .load_render_texture(thread, half_w as u32, half_h as u32)
                .unwrap();
            self.bright_fbo
                .texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            self.blur_fbo = [
                rl.load_render_texture(thread, half_w as u32, half_h as u32)
                    .unwrap(),
                rl.load_render_texture(thread, half_w as u32, half_h as u32)
                    .unwrap(),
            ];
            for bf in &self.blur_fbo {
                bf.texture()
                    .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            }
            self.output_fbo = rl
                .load_render_texture(thread, screen_w as u32, screen_h as u32)
                .unwrap();
            self.output_fbo
                .texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            // SSR scratch FBO matches output_fbo's resolution (full-res).
            self.ssr_fbo = rl
                .load_render_texture(thread, screen_w as u32, screen_h as u32)
                .unwrap();
            self.ssr_fbo
                .texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            // Update the resolution uniform so the Sobel kernel uses the
            // correct texel size for the new dimensions.
            self.ssr_shader.set_shader_value(
                self.loc_ssr_resolution,
                Vector2::new(screen_w as f32, screen_h as f32),
            );

            self.width = screen_w;
            self.height = screen_h;
            self.half_width = half_w;
            self.half_height = half_h;
        }
    }
}
