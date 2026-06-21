//! Input mapping. Reads raw raylib input into a snapshot each render frame.
use raylib::prelude::*;

/// Held-state + edge inputs sampled per render frame. Edges are consumed by
/// the logic step that processes them (best-effort; edges persist until the
/// next logic step drains them via `take_*`).
#[derive(Clone, Default)]
pub struct Input {
    // Held axes
    pub move_x: f32, // -1 left .. +1 right
    pub move_y: f32, // -1 back .. +1 forward
    pub look_dx: f32,
    pub look_dy: f32,
    pub sprint: bool,
    pub jump: bool,
    pub handbrake: bool,
    pub fire_held: bool,
    // Edges (set when first pressed, cleared after a logic step consumes them)
    pub enter_exit: bool,
    pub reload: bool,
    pub interact: bool,
    pub switch_weapon: bool,
    pub melee: bool,
}

impl Input {
    pub fn sample(rl: &RaylibHandle) -> Self {
        let mut i = Input::default();
        // Movement
        let mut mx = 0.0;
        let mut my = 0.0;
        if rl.is_key_down(KeyboardKey::KEY_W) { my += 1.0; }
        if rl.is_key_down(KeyboardKey::KEY_S) { my -= 1.0; }
        if rl.is_key_down(KeyboardKey::KEY_A) { mx -= 1.0; }
        if rl.is_key_down(KeyboardKey::KEY_D) { mx += 1.0; }
        i.move_x = mx;
        i.move_y = my;
        // Look (mouse delta)
        let md = rl.get_mouse_delta();
        i.look_dx = md.x;
        i.look_dy = md.y;
        // Holds
        i.sprint = rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT);
        i.jump = rl.is_key_down(KeyboardKey::KEY_SPACE);
        i.handbrake = rl.is_key_down(KeyboardKey::KEY_SPACE);
        i.fire_held = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        // Edges
        i.enter_exit = rl.is_key_pressed(KeyboardKey::KEY_F);
        i.reload = rl.is_key_pressed(KeyboardKey::KEY_R);
        i.interact = rl.is_key_pressed(KeyboardKey::KEY_E);
        i.switch_weapon = rl.is_key_pressed(KeyboardKey::KEY_TAB)
            || rl.is_key_pressed(KeyboardKey::KEY_Q);
        i.melee = rl.is_key_pressed(KeyboardKey::KEY_V);
        i
    }

    /// Clear edge inputs after a logic step consumes them.
    pub fn drain_edges(&mut self) {
        self.enter_exit = false;
        self.reload = false;
        self.interact = false;
        self.switch_weapon = false;
        self.melee = false;
        // look deltas are per-render-frame; drain so they don't accumulate
        // across multiple logic steps in one frame.
        self.look_dx = 0.0;
        self.look_dy = 0.0;
    }
}
