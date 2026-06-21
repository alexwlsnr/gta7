use gta7::config::Config;
use gta7::game::Game;
use gta7::input::Input;
use gta7::time::Clock;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("GTA7")
        .vsync()
        .build();
    rl.set_target_fps(60);
    rl.disable_cursor();

    let cfg = Config::default();
    let mut game = Game::new(&mut rl, &thread, cfg);
    let mut clock = Clock::new(game.cfg.logic_rate);

    while !rl.window_should_close() {
        // Hotkeys.
        game.handle_hotkeys(&rl);

        // Re-sync clock if rate changed.
        if clock.rate() != game.cfg.logic_rate {
            clock.set_rate(game.cfg.logic_rate);
        }

        // Sample input fresh each frame.
        let mut input = Input::sample(&rl);

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
