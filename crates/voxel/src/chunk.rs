//! Chunk storage. A chunk is a `16 x 128 x 16` column of `u8` block ids plus a
//! per-column height map for fast surface queries.

use crate::block::{self, id};

pub const CHUNK_X: usize = 16;
pub const CHUNK_Y: usize = 128;
pub const CHUNK_Z: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_X * CHUNK_Y * CHUNK_Z;
pub const SEA_LEVEL: i32 = 62;

/// Chunk grid coordinates.
pub type ChunkPos = (i32, i32);

#[inline]
pub fn chunk_of(x: i32, z: i32) -> ChunkPos {
    // `div_euclid` keeps negative coordinates stable.
    (x.div_euclid(CHUNK_X as i32), z.div_euclid(CHUNK_Z as i32))
}

#[inline]
pub fn local_coord(v: i32) -> usize {
    v.rem_euclid(CHUNK_X as i32) as usize
}

#[inline]
pub fn block_index(x: usize, y: usize, z: usize) -> usize {
    (y * CHUNK_Z + z) * CHUNK_X + x
}

#[derive(Clone)]
pub struct Chunk {
    pub pos: ChunkPos,
    pub blocks: Vec<u8>,
    /// Highest non-air block per column, `z * 16 + x`.
    pub height: [u8; CHUNK_X * CHUNK_Z],
}

impl Chunk {
    pub fn new(pos: ChunkPos) -> Self {
        Self {
            pos,
            blocks: vec![id::AIR; CHUNK_VOLUME],
            height: [0; CHUNK_X * CHUNK_Z],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        if x >= CHUNK_X || y >= CHUNK_Y || z >= CHUNK_Z {
            return id::AIR;
        }
        self.blocks[block_index(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, block: u8) {
        if x >= CHUNK_X || y >= CHUNK_Y || z >= CHUNK_Z {
            return;
        }
        self.blocks[block_index(x, y, z)] = block;
        let col = z * CHUNK_X + x;
        let current = self.height[col] as usize;
        if block != id::AIR {
            if y > current {
                self.height[col] = y as u8;
            }
        } else if y >= current && y > 0 {
            // walk down to the next solid layer
            let mut new_top = 0usize;
            for yy in (0..y).rev() {
                if self.blocks[block_index(x, yy, z)] != id::AIR {
                    new_top = yy;
                    break;
                }
            }
            self.height[col] = new_top as u8;
        }
    }

    /// Recompute the height map from scratch (used after generation).
    pub fn rebuild_height(&mut self) {
        for z in 0..CHUNK_Z {
            for x in 0..CHUNK_X {
                let mut top = 0usize;
                for y in (0..CHUNK_Y).rev() {
                    if self.blocks[block_index(x, y, z)] != id::AIR {
                        top = y;
                        break;
                    }
                }
                self.height[z * CHUNK_X + x] = top as u8;
            }
        }
    }

    pub fn top_at(&self, x: usize, z: usize) -> u8 {
        self.height[z * CHUNK_X + x]
    }

    /// True if the column has any non-air block above sea level (used by the
    /// sky light approximation / surface spawn search).
    pub fn is_underwater(&self, x: usize, z: usize) -> bool {
        let top = self.top_at(x, z) as usize;
        self.get(x, top, z) == id::WATER
    }

    /// Fill a box, clipped to the chunk, without maintaining the height map.
    pub fn fill(&mut self, x0: i32, y0: i32, z0: i32, w: i32, h: i32, d: i32, block: u8) {
        for y in y0..y0 + h {
            if y < 0 || y >= CHUNK_Y as i32 {
                continue;
            }
            for z in z0..z0 + d {
                if z < 0 || z >= CHUNK_Z as i32 {
                    continue;
                }
                for x in x0..x0 + w {
                    if x < 0 || x >= CHUNK_X as i32 {
                        continue;
                    }
                    self.blocks[block_index(x as usize, y as usize, z as usize)] = block;
                }
            }
        }
    }
}

/// True when `b` is a block the player can stand in.
pub fn is_passable(b: u8) -> bool {
    !block::is_solid(b)
}
