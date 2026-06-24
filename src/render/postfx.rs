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
    /// Per-frame time uniform for the animated film grain in the CRT shader.
    loc_crt_time: i32,
    /// Resolution uniform for the CRT scanline spacing.
    loc_crt_resolution: i32,

    width: i32,
    height: i32,
    half_width: i32,
    half_height: i32,
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
        let output_fbo = rl
            .load_render_texture(thread, width as u32, height as u32)
            .unwrap();

        // Bilinear filtering on every intermediate target so downsampling and
        // blur ping-pongs sample smoothly.
        bright_fbo
            .texture()
            .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        for bf in &blur_fbo {
            bf.texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        }
        output_fbo
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

        // Cache uniform locations (-1 = not found / inactive).
        let loc_threshold = bright_shader.get_shader_location("u_threshold");
        let loc_soft_knee = bright_shader.get_shader_location("u_softKnee");
        let loc_blur_direction = blur_shader.get_shader_location("u_direction");
        let loc_bloom_strength = bloom_shader.get_shader_location("u_bloomStrength");
        let loc_bloom_bloom = bloom_shader.get_shader_location("texture1");
        let loc_crt_time = crt_shader.get_shader_location("u_time");
        let loc_crt_resolution = crt_shader.get_shader_location("u_resolution");

        // Default uniform values.
        bright_shader.set_shader_value(loc_threshold, 0.85f32);
        bright_shader.set_shader_value(loc_soft_knee, 0.15f32);
        bloom_shader.set_shader_value(loc_bloom_strength, 0.4f32);
        crt_shader.set_shader_value(
            loc_crt_resolution,
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
            loc_threshold,
            loc_soft_knee,
            loc_blur_direction,
            loc_bloom_strength,
            loc_bloom_bloom,
            loc_crt_time,
            loc_crt_resolution,
            width,
            height,
            half_width,
            half_height,
        }
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
        // Pass 5: CRT post filter (output_fbo -> scene_fbo temp -> output_fbo).
        // `begin_texture_mode` borrows the destination FBO mutably, so we can't
        // read and write `output_fbo` in one pass. `scene_fbo` is free after the
        // bloom composite, so use it as a scratch target, then copy back.
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

            self.width = screen_w;
            self.height = screen_h;
            self.half_width = half_w;
            self.half_height = half_h;
        }
    }
}
