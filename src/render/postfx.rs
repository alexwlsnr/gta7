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
    pub fn apply(&self, d: &mut RaylibDrawHandle) {
        let src = Rectangle::new(0.0, 0.0, self.width as f32, -self.height as f32);
        let dst = Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32);
        d.draw_texture_pro(
            self.scene_fbo.texture(),
            src,
            dst,
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
    }
}
