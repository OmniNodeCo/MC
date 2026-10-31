//! Native desktop entry point (winit + wgpu).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use mc_client::App;
use winit::event::{DeviceEvent, ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowBuilder};

const SAVE_FILE: &str = "mc_world.dat";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("MC-RS - a Minecraft clone in Rust")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)?,
    );

    let size = window.inner_size();
    let start = pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("could not create a surface: {e}"))?;
        App::new(
            instance,
            surface,
            size.width.max(1),
            size.height.max(1),
            mc_client::DEFAULT_SEED,
            8,
        )
        .await
    });

    let mut app = start.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Resume the last session when a save file is present.
    let save_path = save_path();
    if let Ok(data) = std::fs::read(&save_path) {
        if app.load_bytes(&data) {
            println!("Loaded {}", save_path.display());
        }
    }

    grab_pointer(&window, false);
    let mut last_frame = Instant::now();
    let mut pointer_locked = false;

    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    persist(&app, &save_path);
                    target.exit();
                }
                WindowEvent::Resized(size) => {
                    if size.width > 0 && size.height > 0 {
                        app.resize(size.width, size.height);
                    }
                }
                WindowEvent::Focused(false) => {
                    pointer_locked = false;
                    app.input.cursor_locked = false;
                    app.input.break_held = false;
                    grab_pointer(&window, false);
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if event.state == ElementState::Pressed && !event.repeat {
                            match code {
                                KeyCode::Escape => {
                                    pointer_locked = false;
                                    app.input.cursor_locked = false;
                                    app.input.break_held = false;
                                    grab_pointer(&window, false);
                                }
                                KeyCode::F11 => toggle_fullscreen(&window),
                                KeyCode::F5 => {
                                    persist(&app, &save_path);
                                    app.mark_saved();
                                }
                                KeyCode::F2 => {
                                    if let Ok(data) = std::fs::read(&save_path) {
                                        if app.load_bytes(&data) {
                                            println!("Reloaded {}", save_path.display());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        apply_key(&mut app, code, event.state == ElementState::Pressed);
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if !pointer_locked {
                        if state == ElementState::Pressed {
                            pointer_locked = true;
                            app.input.cursor_locked = true;
                            grab_pointer(&window, true);
                        }
                        return;
                    }
                    match button {
                        MouseButton::Left => {
                            app.input.break_held = state == ElementState::Pressed;
                        }
                        MouseButton::Right => {
                            if state == ElementState::Pressed {
                                app.input.place = true;
                            }
                        }
                        MouseButton::Middle => {
                            if state == ElementState::Pressed {
                                app.input.pick = true;
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 60.0,
                    };
                    if amount != 0.0 {
                        app.input.wheel += amount;
                    }
                }
                _ => {}
            },
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } => {
                if pointer_locked {
                    app.input.mouse_dx += delta.0 as f32;
                    app.input.mouse_dy += delta.1 as f32;
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();
                let dt = (now - last_frame).as_secs_f32();
                last_frame = now;

                app.input.cursor_locked = pointer_locked;
                app.frame(dt);
                if app.take_autosave() {
                    persist(&app, &save_path);
                    app.mark_saved();
                }
            }
            _ => {}
        }
    })?;
}

fn apply_key(app: &mut App, code: KeyCode, down: bool) {
    let input = &mut app.input;
    match code {
        KeyCode::KeyW => input.forward = down,
        KeyCode::KeyS => input.back = down,
        KeyCode::KeyA => input.left = down,
        KeyCode::KeyD => input.right = down,
        KeyCode::Space => input.jump = down,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => input.sneak = down,
        KeyCode::ControlLeft | KeyCode::ControlRight => input.sprint = down,
        KeyCode::KeyF if down => input.toggle_fly = true,
        KeyCode::KeyG if down => input.toggle_mode = true,
        KeyCode::KeyE if down => input.toggle_inventory = true,
        KeyCode::F3 if down => input.toggle_debug = true,
        KeyCode::KeyR if down => input.respawn = true,
        KeyCode::Digit1 if down => input.hotbar = Some(0),
        KeyCode::Digit2 if down => input.hotbar = Some(1),
        KeyCode::Digit3 if down => input.hotbar = Some(2),
        KeyCode::Digit4 if down => input.hotbar = Some(3),
        KeyCode::Digit5 if down => input.hotbar = Some(4),
        KeyCode::Digit6 if down => input.hotbar = Some(5),
        KeyCode::Digit7 if down => input.hotbar = Some(6),
        KeyCode::Digit8 if down => input.hotbar = Some(7),
        KeyCode::Digit9 if down => input.hotbar = Some(8),
        _ => {}
    }
}

fn grab_pointer(window: &Window, grab: bool) {
    if grab {
        let locked = window.set_cursor_grab(CursorGrabMode::Locked);
        if locked.is_err() {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
        }
        window.set_cursor_visible(false);
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
    }
}

fn toggle_fullscreen(window: &Window) {
    if window.fullscreen().is_some() {
        window.set_fullscreen(None);
    } else {
        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    }
}

fn save_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SAVE_FILE)
}

fn persist(app: &App, path: &PathBuf) {
    let bytes = app.save_bytes();
    match std::fs::write(path, &bytes) {
        Ok(()) => println!("Saved {} ({} bytes)", path.display(), bytes.len()),
        Err(err) => eprintln!("could not save: {err}"),
    }
}
