//! Post-processing pipeline: renders scene to FBO, chains fullscreen shader passes.
use raylib::prelude::*;

pub struct PostFx {
    pub scene_fbo: RenderTexture2D,
    width: i32,
    height: i32,
}

impl PostFx {
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, width: i32, height: i32) -> Self {
        let scene_fbo = rl.load_render_texture(thread, width as u32, height as u32).unwrap();
        scene_fbo.texture().set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        Self { scene_fbo, width, height }
    }

    /// Blit the scene FBO to the screen. In this scaffold, just copies it.
    /// Later tasks will replace this with the full post-processing chain.
    /// Uses the actual screen dimensions for the destination so fullscreen
    /// works correctly — the FBO may be a different resolution than the window.
    pub fn apply(&self, d: &mut RaylibDrawHandle) {
        let src = Rectangle::new(0.0, 0.0, self.width as f32, -self.height as f32);
        let screen_w = d.get_screen_width() as f32;
        let screen_h = d.get_screen_height() as f32;
        let dst = Rectangle::new(0.0, 0.0, screen_w, screen_h);
        d.draw_texture_pro(
            self.scene_fbo.texture(),
            src,
            dst,
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
    }

    /// Recreate the scene FBO if the window size has changed. Call this each
    /// frame before rendering — it's a no-op when the size matches.
    pub fn resize_if_needed(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, screen_w: i32, screen_h: i32) {
        if screen_w != self.width || screen_h != self.height {
            self.scene_fbo = rl.load_render_texture(thread, screen_w as u32, screen_h as u32).unwrap();
            self.scene_fbo.texture().set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
            self.width = screen_w;
            self.height = screen_h;
        }
    }
}
