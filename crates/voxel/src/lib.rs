//! `voxel` is the whole game simulation: a deterministic, endlessly generated
//! block world with lighting, physics, interaction, an inventory and a HUD.
//!
//! It has no platform dependencies (no `wgpu`, no `winit`, no `web-sys`), which
//! keeps it testable, reusable and identical on desktop and in the browser.

pub mod block;
pub mod chunk;
pub mod font;
pub mod game;
pub mod mesher;
pub mod noise;
pub mod particles;
pub mod physics;
pub mod raycast;
pub mod save;
pub mod textures;
pub mod ui;
pub mod world;
pub mod worldgen;

pub use block::{BlockDef, Face};
pub use chunk::{Chunk, ChunkPos, CHUNK_X, CHUNK_Y, CHUNK_Z, SEA_LEVEL};
pub use game::{Game, GameMode, Input};
pub use mesher::{ChunkMesh, MeshData, Vertex};
pub use physics::Player;
pub use ui::UiVertex;
pub use world::World;

/// Crate version, surfaced in the HUD.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
