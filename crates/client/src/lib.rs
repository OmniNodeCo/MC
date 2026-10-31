//! `mc-client` is the renderer and game loop. The platform crates (`mc-desktop`
//! and `mc-web`) only have to provide a `wgpu::Surface` and feed input events
//! into [`App`].

pub mod app;
pub mod gfx;
pub mod renderer;

pub use app::App;
pub use gfx::{Gfx, LineVertex, SceneUniform};
pub use renderer::Renderer;

/// Seed used when the player starts a brand new world.
pub const DEFAULT_SEED: u32 = 1337;
