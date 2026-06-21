use raylib::prelude::*;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("GTA7")
        .vsync()
        .build();
    rl.set_target_fps(60);
    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::new(20, 20, 28, 255));
        d.draw_text("GTA7 bootstrap OK", 40, 40, 28, Color::RAYWHITE);
    }
}
