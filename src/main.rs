use gta7::config::Config;
use gta7::game::Game;
use gta7::input::Input;
use gta7::time::Clock;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("GTA7")
        .build(); // Removed .vsync() to allow target FPS up to 120fps regardless of monitor refresh rate.
    let cfg = Config::default();
    rl.set_target_fps(cfg.logic_rate.hz() as u32);
    rl.disable_cursor();

    let mut game = Game::new(&mut rl, &thread, cfg);
    let mut clock = Clock::new(game.cfg.logic_rate);

    while !rl.window_should_close() {
        // Fullscreen toggle with F11 (scales to native resolution properly).
        if rl.is_key_pressed(raylib::consts::KeyboardKey::KEY_F11) {
            if rl.is_window_fullscreen() {
                rl.toggle_fullscreen();
                rl.set_window_size(1280, 720);
            } else {
                let monitor = raylib::core::window::get_current_monitor();
                let w = raylib::core::window::get_monitor_width(monitor);
                let h = raylib::core::window::get_monitor_height(monitor);
                rl.set_window_size(w, h);
                rl.toggle_fullscreen();
            }
        }
        // Hotkeys.
        game.handle_hotkeys(&rl);

        // Re-sync clock and target FPS if rate changed.
        if clock.rate() != game.cfg.logic_rate {
            clock.set_rate(game.cfg.logic_rate);
            rl.set_target_fps(game.cfg.logic_rate.hz() as u32);
        }

        // Sample input fresh each frame.
        let mut input = Input::sample(&rl);
        
        // Accumulate look deltas across frames so they survive logic-timestep skips.
        game.look_accum_x += input.look_dx;
        game.look_accum_y += input.look_dy;

        // Advance clock.
        let steps = clock.tick(rl.get_frame_time());

        // Run logic steps.
        for _ in 0..steps {
            game.update(&mut input, clock.dt());
        }

        // Render with interpolation.
        let fps = rl.get_fps();
        game.render(&mut rl, &thread, clock.alpha, fps as i32);
    }
}
