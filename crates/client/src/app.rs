//! Platform independent application object: owns the game and the renderer and
//! advances one frame at a time.

use voxel::game::{Game, Input};
use voxel::save;

use crate::gfx::Gfx;
use crate::renderer::Renderer;

pub struct App {
    pub game: Game,
    pub renderer: Renderer,
    pub input: Input,
    elapsed: f32,
    autosave_timer: f32,
    pub paused: bool,
}

impl App {
    pub async fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        seed: u32,
        render_distance: i32,
    ) -> Result<Self, String> {
        let gfx = Gfx::new(instance, surface, width, height).await?;
        let mut game = Game::new(seed, render_distance);
        game.set_screen(width as f32, height as f32);
        game.stream_chunks();
        let renderer = Renderer::new(gfx);
        Ok(Self {
            game,
            renderer,
            input: Input::default(),
            elapsed: 0.0,
            autosave_timer: 0.0,
            paused: true,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.renderer.gfx.resize(width, height);
        self.game
            .set_screen(width as f32, height as f32);
    }

    /// Advance and draw one frame.
    pub fn frame(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        self.elapsed += dt;
        if dt > 0.0001 {
            let instant = 1.0 / dt;
            self.game.fps = if self.game.fps <= 0.5 {
                instant
            } else {
                self.game.fps * 0.9 + instant * 0.1
            };
        }

        self.game
            .set_screen(self.renderer.gfx.config.width as f32, self.renderer.gfx.config.height as f32);
        self.paused = !self.input.cursor_locked;
        self.game.update(dt, &mut self.input);
        self.input.clear_events();

        self.renderer.render(&mut self.game, self.elapsed);
        self.autosave_timer += dt;
    }

    pub fn render_distance(&self) -> i32 {
        self.game.render_distance
    }

    pub fn set_render_distance(&mut self, distance: i32) {
        self.game.render_distance = distance.clamp(2, 16);
    }

    /// Serialise the world + player for persistence.
    pub fn save_bytes(&self) -> Vec<u8> {
        save::encode(&self.game)
    }

    pub fn load_bytes(&mut self, data: &[u8]) -> bool {
        let ok = save::decode(data, &mut self.game);
        if ok {
            self.renderer.clear_chunks();
            self.game
                .set_screen(self.renderer.gfx.config.width as f32, self.renderer.gfx.config.height as f32);
        }
        ok
    }

    pub fn take_autosave(&mut self) -> bool {
        if self.autosave_timer >= 45.0 {
            self.autosave_timer = 0.0;
            true
        } else {
            false
        }
    }

    pub fn mark_saved(&mut self) {
        self.game.saved_flash = 1.5;
    }
}
