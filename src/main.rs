use gta7::config::Config;
use gta7::game::Game;
use gta7::input::Input;
use gta7::time::Clock;
use raylib::consts::KeyboardKey;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("GTA7")
        .build();
    let cfg = Config::default();
    rl.set_target_fps(cfg.logic_rate.hz() as u32);
    rl.disable_cursor();
    // ESC opens the pause menu instead of closing the window.
    rl.set_exit_key(None);

    // Initialize audio device for retro sound effects.
    let audio = raylib::prelude::RaylibAudio::init_audio_device().unwrap();

    let mut game = Game::new(&mut rl, &thread, cfg, &audio);
    let mut clock = Clock::new(game.cfg.logic_rate);

    while !rl.window_should_close() {
        // ESC toggles pause.
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) && game.screen_state == gta7::game::ScreenState::Playing {
            game.paused = !game.paused;
            if game.paused {
                rl.enable_cursor();
            } else {
                rl.disable_cursor();
            }
        }

        // Fullscreen toggle with F11.
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            rl.toggle_fullscreen();
        }

        // Hotkeys (only when not paused).
        if !game.paused {
            game.handle_hotkeys(&rl);
        }

        // Re-sync clock and target FPS if rate changed.
        if clock.rate() != game.cfg.logic_rate {
            clock.set_rate(game.cfg.logic_rate);
            rl.set_target_fps(game.cfg.logic_rate.hz() as u32);
        }

        // Sample input fresh each frame.
        let mut input = Input::sample(&rl);

        // Accumulate look deltas across frames so they survive logic-timestep skips.
        if !game.paused {
            game.look_accum_x += input.look_dx;
            game.look_accum_y += input.look_dy;
        }

        // Advance clock.
        let steps = clock.tick(rl.get_frame_time());

        // Run logic steps only when not paused.
        if !game.paused {
            for _ in 0..steps {
                game.update(&mut input, clock.dt());
            }
        }

        // Render with interpolation.
        let fps = rl.get_fps();
        game.render(&mut rl, &thread, clock.alpha, fps as i32);

        // Apply pending fullscreen toggle from pause menu.
        if game.pending_fullscreen {
            game.pending_fullscreen = false;
            rl.toggle_fullscreen();
        }

        // Check quit flag from pause menu.
        if game.quit {
            break;
        }
    }
}
