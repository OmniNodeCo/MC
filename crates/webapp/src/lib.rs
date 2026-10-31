#![cfg(target_arch = "wasm32")]

//! WebAssembly entry point. HTML/JS owns input capture, pointer lock and
//! localStorage; this module only translates those events into the shared
//! client.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use mc_client::App;

thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn boot() {
    console_error_panic_hook::set_once();
}

/// Create the renderer for `canvas` and start the game loop.
#[wasm_bindgen]
pub async fn start(
    canvas: HtmlCanvasElement,
    width: u32,
    height: u32,
    seed: u32,
) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..Default::default()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| JsValue::from_str(&format!("canvas surface: {e}")))?;

    let mut app = App::new(instance, surface, width, height, seed, 6)
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    app.input.cursor_locked = false;
    app.game.stream_chunks();

    APP.with(|slot| *slot.borrow_mut() = Some(app));
    Ok(())
}

fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow_mut().as_mut() {
            f(app);
        }
    });
}

/// Advance and draw one frame. Returns true when the world should be saved.
#[wasm_bindgen]
pub fn frame(dt_seconds: f32) -> bool {
    let mut autosave = false;
    with_app(|app| {
        app.frame(dt_seconds);
        autosave = app.take_autosave();
    });
    autosave
}

#[wasm_bindgen]
pub fn resize(width: u32, height: u32) {
    with_app(|app| app.resize(width.max(1), height.max(1)));
}

#[wasm_bindgen]
pub fn set_key(code: &str, down: bool) {
    with_app(|app| {
        let input = &mut app.input;
        match code {
            "KeyW" => input.forward = down,
            "KeyS" => input.back = down,
            "KeyA" => input.left = down,
            "KeyD" => input.right = down,
            "Space" => input.jump = down,
            "ShiftLeft" | "ShiftRight" => input.sneak = down,
            "ControlLeft" | "ControlRight" => input.sprint = down,
            "KeyF" if down => input.toggle_fly = true,
            "KeyG" if down => input.toggle_mode = true,
            "KeyE" if down => input.toggle_inventory = true,
            "F3" if down => input.toggle_debug = true,
            "KeyR" if down => input.respawn = true,
            "Escape" if down => input.pause = true,
            "Digit1" if down => input.hotbar = Some(0),
            "Digit2" if down => input.hotbar = Some(1),
            "Digit3" if down => input.hotbar = Some(2),
            "Digit4" if down => input.hotbar = Some(3),
            "Digit5" if down => input.hotbar = Some(4),
            "Digit6" if down => input.hotbar = Some(5),
            "Digit7" if down => input.hotbar = Some(6),
            "Digit8" if down => input.hotbar = Some(7),
            "Digit9" if down => input.hotbar = Some(8),
            _ => {}
        }
    });
}

#[wasm_bindgen]
pub fn mouse_delta(dx: f32, dy: f32) {
    with_app(|app| {
        app.input.mouse_dx += dx;
        app.input.mouse_dy += dy;
    });
}

#[wasm_bindgen]
pub fn mouse_button(button: u8, down: bool) {
    with_app(|app| match button {
        0 => {
            app.input.break_held = down;
        }
        1 => {
            if down {
                app.input.pick = true;
            }
        }
        2 => {
            if down {
                app.input.place = true;
            }
        }
        _ => {}
    });
}

#[wasm_bindgen]
pub fn wheel(delta: f32) {
    with_app(|app| app.input.wheel += delta);
}

/// The pointer is captured, or the block palette is open (both mean "playing").
#[wasm_bindgen]
pub fn pointer_lock(locked: bool) {
    with_app(|app| {
        app.input.cursor_locked = locked;
        if !locked {
            app.input.break_held = false;
        }
    });
}

/// Click in CSS pixels; used by the block palette.
#[wasm_bindgen]
pub fn click(x: f32, y: f32) {
    // `x`/`y` are physical canvas pixels, matching the HUD layout.
    with_app(|app| app.input.click = Some([x, y]));
}

#[wasm_bindgen]
pub fn inventory_open() -> bool {
    let mut open = false;
    with_app(|app| open = app.game.inventory_open);
    open
}

#[wasm_bindgen]
pub fn persist_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    with_app(|app| bytes = app.save_bytes());
    bytes
}

#[wasm_bindgen]
pub fn restore_bytes(data: &[u8]) -> bool {
    let mut ok = false;
    with_app(|app| ok = app.load_bytes(data));
    ok
}

#[wasm_bindgen]
pub fn mark_saved() {
    with_app(|app| app.mark_saved());
}

#[wasm_bindgen]
pub fn set_render_distance(distance: i32) {
    with_app(|app| app.set_render_distance(distance));
}

#[wasm_bindgen]
pub fn fps() -> f32 {
    let mut fps = 0.0;
    with_app(|app| fps = app.game.fps);
    fps
}
