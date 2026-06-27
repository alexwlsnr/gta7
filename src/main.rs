use gta7::cli_args::parse_args;
use gta7::config::Config;
use gta7::game::Game;
use gta7::input::Input;
use gta7::time::Clock;
use raylib::consts::KeyboardKey;

fn main() {
    let args = parse_args();

    let title = if args.test {
        format!("GTA7 [test: {}]", args.scene)
    } else {
        "GTA7".to_string()
    };
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title(&title)
        .build();
    let cfg = Config::default();
    rl.set_target_fps(cfg.logic_rate.hz() as u32);
    rl.enable_cursor();
    // ESC opens the pause menu instead of closing the window.
    rl.set_exit_key(None);

    // Initialize audio device for retro sound effects.
    let audio = raylib::prelude::RaylibAudio::init_audio_device().unwrap();

    let mut game = Game::new(&mut rl, &thread, cfg, &audio);
    let mut clock = Clock::new(game.cfg.logic_rate);
    let mut cursor_enabled = true;

    if args.test {
        // Capture screenshot path up front.
        let screenshot_path = args.screenshot.clone();
        game.enter_test_mode(args);

        if let Some(path) = screenshot_path {
            // Screenshot mode: run one logic step + one render, save, exit.
            let mut input = Input::sample(&rl);
            game.update(&mut input, clock.dt());
            let fps = rl.get_fps() as i32;
            game.render(&mut rl, &thread, 1.0, fps);
            rl.take_screenshot(&thread, path.to_str().expect("screenshot path"));
            return;
        }

        // Interactive test mode: regular main loop, test hotkeys active.
        interactive_loop(&mut rl, &thread, &mut game, &mut clock, &mut cursor_enabled);
        return;
    }

    // Normal game loop (unchanged path).
    normal_loop(&mut rl, &thread, &mut game, &mut clock, &mut cursor_enabled);
}

fn normal_loop(
    rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread,
    game: &mut Game, clock: &mut Clock, cursor_enabled: &mut bool,
) {
    while !rl.window_should_close() {
        // Sync cursor state based on game screen/pause state transitions
        let target_cursor = game.paused || game.screen_state == gta7::game::ScreenState::Title;
        if target_cursor != *cursor_enabled {
            *cursor_enabled = target_cursor;
            if *cursor_enabled {
                rl.enable_cursor();
            } else {
                rl.disable_cursor();
            }
        }

        // ESC toggles pause in-game, or exits the game from the start menu.
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            if game.screen_state == gta7::game::ScreenState::Playing {
                game.paused = !game.paused;
            } else if game.screen_state == gta7::game::ScreenState::Title {
                game.quit = true;
            }
        }

        // Fullscreen toggle with F11.
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            rl.toggle_fullscreen();
        }

        // Hotkeys (only when not paused).
        if !game.paused {
            game.handle_hotkeys(rl);
        }

        // Re-sync clock and target FPS if rate changed.
        if clock.rate() != game.cfg.logic_rate {
            clock.set_rate(game.cfg.logic_rate);
            rl.set_target_fps(game.cfg.logic_rate.hz() as u32);
        }

        // Sample input fresh each frame.
        let mut input = Input::sample(rl);

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
        game.render(rl, thread, clock.alpha, fps as i32);

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

fn interactive_loop(
    rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread,
    game: &mut Game, clock: &mut Clock, cursor_enabled: &mut bool,
) {
    let mut cycle_scene_idx: usize = 0;
    while !rl.window_should_close() {
        // Always enable cursor in interactive test mode (no auto-hide).
        if !*cursor_enabled {
            rl.enable_cursor();
            *cursor_enabled = true;
        }

        // Fullscreen toggle with F11.
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            rl.toggle_fullscreen();
        }

        // F1: toggle debug overlay.
        if rl.is_key_pressed(KeyboardKey::KEY_F1) {
            game.cfg.debug_overlay = !game.cfg.debug_overlay;
        }

        // F3: bounds overlay (stub for now).
        if rl.is_key_pressed(KeyboardKey::KEY_F3) { /* bounds: stub for now */ }

        // F5: cycle through scene presets.
        if rl.is_key_pressed(KeyboardKey::KEY_F5) {
            cycle_scene_idx = (cycle_scene_idx + 1) % gta7::test_scene::SCENES.len();
            let (name, _) = gta7::test_scene::SCENES[cycle_scene_idx];
            let mut a = game.args.clone().unwrap_or_default();
            a.scene = name.to_string();
            game.enter_test_mode(a);
        }

        // F6: toggle free-fly vs follow camera.
        if rl.is_key_pressed(KeyboardKey::KEY_F6) {
            if game.camera.is_free() {
                game.camera.set_follow();
            } else {
                game.camera.set_free(game.camera.pos, game.camera.yaw, game.camera.pitch);
            }
        }

        // P: save a timestamped screenshot.
        if rl.is_key_pressed(KeyboardKey::KEY_P) {
            take_screenshot(rl, thread);
        }

        // Numpad +/- to advance time.
        if rl.is_key_pressed(KeyboardKey::KEY_KP_ADD) {
            game.set_time(game.time * game.cfg.time_scale + 0.5);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_KP_SUBTRACT) {
            game.set_time(game.time * game.cfg.time_scale - 0.5);
        }

        // Sample input fresh each frame.
        let mut input = Input::sample(rl);

        // Accumulate look deltas.
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
        game.render(rl, thread, clock.alpha, fps as i32);
    }
}

fn take_screenshot(rl: &mut raylib::RaylibHandle, thread: &raylib::RaylibThread) {
    use std::time::{SystemTime, UNIX_EPOCH};
    std::fs::create_dir_all("screenshots").ok();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = format!("screenshots/{stamp}.png");
    rl.take_screenshot(thread, &path);
    eprintln!("Saved screenshot: {path}");
}
