//! The chunked world: streaming, block access and persistent edit tracking.

use std::collections::{HashMap, HashSet};

use crate::block::{self, id};
use crate::chunk::{block_index, chunk_of, Chunk, ChunkPos, CHUNK_X, CHUNK_Y, CHUNK_Z};
use crate::worldgen;

/// Packed chunk-local position, used as the edit-map key.
#[inline]
pub fn local_key(x: usize, y: usize, z: usize) -> u32 {
    ((y as u32) << 8) | ((z as u32) << 4) | (x as u32)
}

#[inline]
pub fn local_key_coords(key: u32) -> (usize, usize, usize) {
    (
        (key & 0xF) as usize,
        ((key >> 8) & 0x7F) as usize,
        ((key >> 4) & 0xF) as usize,
    )
}

pub struct World {
    pub seed: u32,
    chunks: HashMap<ChunkPos, Chunk>,
    /// Every player modification, kept even when its chunk is unloaded.
    edits: HashMap<ChunkPos, HashMap<u32, u8>>,
    /// Chunks whose GPU mesh must be rebuilt.
    dirty: HashSet<ChunkPos>,
}

impl World {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            chunks: HashMap::new(),
            edits: HashMap::new(),
            dirty: HashSet::new(),
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Number of chunks currently held in memory.
    pub fn loaded_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_loaded(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    pub fn positions(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.chunks.keys().copied()
    }

    /// Block id, or `AIR` outside the world / in unloaded chunks.
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 || y >= CHUNK_Y as i32 {
            return id::AIR;
        }
        let pos = chunk_of(x, z);
        match self.chunks.get(&pos) {
            Some(chunk) => chunk.get(
                x.rem_euclid(CHUNK_X as i32) as usize,
                y as usize,
                z.rem_euclid(CHUNK_Z as i32) as usize,
            ),
            None => id::AIR,
        }
    }

    /// Like [`World::get`] but distinguishes "block is air" from "not loaded".
    pub fn get_loaded(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if y < 0 || y >= CHUNK_Y as i32 {
            return None;
        }
        let pos = chunk_of(x, z);
        self.chunks.get(&pos).map(|chunk| {
            chunk.get(
                x.rem_euclid(CHUNK_X as i32) as usize,
                y as usize,
                z.rem_euclid(CHUNK_Z as i32) as usize,
            )
        })
    }

    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        block::is_solid(self.get(x, y, z))
    }

    pub fn is_liquid(&self, x: i32, y: i32, z: i32) -> bool {
        block::is_liquid(self.get(x, y, z))
    }

    /// Height of the highest non-air block in a column (uses terrain sampling
    /// when the chunk is not loaded yet).
    pub fn surface_height(&self, x: i32, z: i32) -> i32 {
        let pos = chunk_of(x, z);
        if let Some(chunk) = self.chunks.get(&pos) {
            chunk.top_at(
                x.rem_euclid(CHUNK_X as i32) as usize,
                z.rem_euclid(CHUNK_Z as i32) as usize,
            ) as i32
        } else {
            worldgen::height_at(self.seed, x, z)
        }
    }

    /// Place or remove a block, recording the change so it survives unload and
    /// save/load cycles.
    pub fn set(&mut self, x: i32, y: i32, z: i32, block_id: u8) -> bool {
        if y < 0 || y >= CHUNK_Y as i32 {
            return false;
        }
        let pos = chunk_of(x, z);
        let lx = x.rem_euclid(CHUNK_X as i32) as usize;
        let lz = z.rem_euclid(CHUNK_Z as i32) as usize;
        let key = local_key(lx, y as usize, lz);

        let Some(chunk) = self.chunks.get_mut(&pos) else {
            return false;
        };
        if chunk.get(lx, y as usize, lz) == block_id {
            return false;
        }
        chunk.set(lx, y as usize, lz, block_id);
        self.edits.entry(pos).or_default().insert(key, block_id);

        // A changed border block also changes the neighbouring chunk meshes.
        let touches_x = lx == 0 || lx == CHUNK_X - 1;
        let touches_z = lz == 0 || lz == CHUNK_Z - 1;
        self.dirty.insert(pos);
        if touches_x {
            self.dirty.insert((pos.0 - if lx == 0 { 1 } else { -1 }, pos.1));
        }
        if touches_z {
            self.dirty.insert((pos.0, pos.1 - if lz == 0 { 1 } else { -1 }));
        }
        if touches_x && touches_z {
            let dx = if lx == 0 { -1 } else { 1 };
            let dz = if lz == 0 { -1 } else { 1 };
            self.dirty.insert((pos.0 + dx, pos.1 + dz));
        }
        true
    }

    /// Generate a chunk if it is missing. Returns true when work was done.
    pub fn load_chunk(&mut self, pos: ChunkPos) -> bool {
        if self.chunks.contains_key(&pos) {
            return false;
        }
        let mut chunk = worldgen::generate_chunk(self.seed, pos);
        if let Some(edits) = self.edits.get(&pos) {
            for (key, block_id) in edits {
                let (x, y, z) = local_key_coords(*key);
                chunk.set(x, y, z, *block_id);
            }
            chunk.rebuild_height();
        }
        self.chunks.insert(pos, chunk);
        self.mark_dirty(pos);
        for (dx, dz) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
        ] {
            let n = (pos.0 + dx, pos.1 + dz);
            if self.chunks.contains_key(&n) {
                self.dirty.insert(n);
            }
        }
        true
    }

    pub fn unload_chunk(&mut self, pos: ChunkPos) -> bool {
        self.dirty.remove(&pos);
        self.chunks.remove(&pos).is_some()
    }

    /// Drop chunks further away than `radius` (in chunks) from `center`.
    pub fn unload_outside(&mut self, center: ChunkPos, radius: i32) -> usize {
        let mut removed = 0;
        let keep_sq = (radius + 1) * (radius + 1);
        let to_remove: Vec<ChunkPos> = self
            .chunks
            .keys()
            .filter(|p| {
                let dx = p.0 - center.0;
                let dz = p.1 - center.1;
                dx * dx + dz * dz > keep_sq
            })
            .copied()
            .collect();
        for pos in to_remove {
            if self.unload_chunk(pos) {
                removed += 1;
            }
        }
        removed
    }

    /// All eight neighbours must exist before a chunk can be meshed with
    /// correct ambient occlusion.
    pub fn neighbors_loaded(&self, pos: ChunkPos) -> bool {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                if !self.chunks.contains_key(&(pos.0 + dx, pos.1 + dz)) {
                    return false;
                }
            }
        }
        true
    }

    pub fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty.insert(pos);
    }

    pub fn is_dirty(&self, pos: ChunkPos) -> bool {
        self.dirty.contains(&pos)
    }

    pub fn take_dirty(&mut self) -> HashSet<ChunkPos> {
        std::mem::take(&mut self.dirty)
    }

    /// Raw edit list for the save file: `[(x, y, z, block)]`.
    pub fn edits(&self) -> Vec<(i32, i32, i32, u8)> {
        let mut out = Vec::new();
        for (pos, map) in &self.edits {
            for (key, block_id) in map {
                let (x, y, z) = local_key_coords(*key);
                out.push((
                    pos.0 * CHUNK_X as i32 + x as i32,
                    y as i32,
                    pos.1 * CHUNK_Z as i32 + z as i32,
                    *block_id,
                ));
            }
        }
        out.sort_by_key(|(x, y, z, _)| (*x, *z, *y));
        out
    }

    /// Replace the edit set (used when loading a save).
    pub fn set_edits(&mut self, edits: Vec<(i32, i32, i32, u8)>) {
        self.edits.clear();
        for (x, y, z, block_id) in edits {
            if y < 0 || y >= CHUNK_Y as i32 {
                continue;
            }
            let pos = chunk_of(x, z);
            let lx = x.rem_euclid(CHUNK_X as i32) as usize;
            let lz = z.rem_euclid(CHUNK_Z as i32) as usize;
            self.edits
                .entry(pos)
                .or_default()
                .insert(local_key(lx, y as usize, lz), block_id);
        }
        // Apply to any already loaded chunk.
        let loaded: Vec<ChunkPos> = self.chunks.keys().copied().collect();
        for pos in loaded {
            if let (Some(chunk), Some(edits)) = (self.chunks.get_mut(&pos), self.edits.get(&pos)) {
                for (key, block_id) in edits {
                    let (x, y, z) = local_key_coords(*key);
                    chunk.set(x, y, z, *block_id);
                }
                chunk.rebuild_height();
            }
            self.dirty.insert(pos);
        }
    }

    /// Chunks that are loaded but should have a mesh.
    pub fn mesh_candidates(&self) -> Vec<ChunkPos> {
        let mut out: Vec<ChunkPos> = self
            .chunks
            .keys()
            .filter(|p| self.neighbors_loaded(**p))
            .copied()
            .collect();
        out.sort_unstable();
        out
    }

    /// Debug helper: the block index of a position inside a loaded chunk.
    pub fn index_in_chunk(x: i32, y: i32, z: i32) -> usize {
        block_index(
            x.rem_euclid(CHUNK_X as i32) as usize,
            y as usize,
            z.rem_euclid(CHUNK_Z as i32) as usize,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_survive_unload() {
        let mut world = World::new(3);
        world.load_chunk((0, 0));
        assert!(world.set(4, 80, 4, id::GLASS));
        assert_eq!(world.get(4, 80, 4), id::GLASS);
        world.unload_chunk((0, 0));
        world.load_chunk((0, 0));
        assert_eq!(world.get(4, 80, 4), id::GLASS);
    }

    #[test]
    fn out_of_range_access_is_air() {
        let world = World::new(3);
        assert_eq!(world.get(0, -1, 0), id::AIR);
        assert_eq!(world.get(0, CHUNK_Y as i32, 0), id::AIR);
        assert_eq!(world.get(999_999, 30, 999_999), id::AIR);
    }

    #[test]
    fn loading_marks_neighbours_dirty() {
        let mut world = World::new(3);
        world.load_chunk((0, 0));
        world.take_dirty();
        world.load_chunk((1, 0));
        assert!(world.is_dirty((0, 0)));
    }

    #[test]
    fn edit_roundtrip() {
        let mut world = World::new(11);
        world.load_chunk((0, 0));
        world.load_chunk((-1, 0));
        world.set(-3, 70, 2, id::BRICKS);
        world.set(6, 71, 9, id::PLANKS);
        let edits = world.edits();
        assert_eq!(edits.len(), 2);

        let mut other = World::new(11);
        other.set_edits(edits);
        other.load_chunk((0, 0));
        other.load_chunk((-1, 0));
        assert_eq!(other.get(-3, 70, 2), id::BRICKS);
        assert_eq!(other.get(6, 71, 9), id::PLANKS);
    }
}
