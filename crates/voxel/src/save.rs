//! Compact little-endian save format (no serde, no external crates).
//!
//! Layout:
//!   magic "MCRS" | version u32 | seed u32 | time f32 | mode u8 | selected u8
//!   hotbar[9] | pos 3xf32 | yaw f32 | pitch f32 | flying u8
//!   counts[256] u32 | edit_count u32 | (x i32, y i32, z i32, id u8)*

use crate::game::{Game, GameMode, HOTBAR_SLOTS};

pub const MAGIC: [u8; 4] = *b"MCRS";
pub const VERSION: u32 = 1;

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn u8(&mut self) -> Option<u8> {
        let v = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(v)
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }

    fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }
}

/// Serialise the interesting parts of a game session.
pub fn encode(game: &Game) -> Vec<u8> {
    let edits = game.world.edits();
    let mut out = Vec::with_capacity(64 + edits.len() * 13);
    out.extend_from_slice(&MAGIC);
    push_u32(&mut out, VERSION);
    push_u32(&mut out, game.world.seed());
    push_f32(&mut out, game.time);
    out.push(match game.mode {
        GameMode::Creative => 0,
        GameMode::Survival => 1,
    });
    out.push(game.selected as u8);
    for b in game.hotbar {
        out.push(b);
    }
    push_f32(&mut out, game.player.pos.x);
    push_f32(&mut out, game.player.pos.y);
    push_f32(&mut out, game.player.pos.z);
    push_f32(&mut out, game.player.yaw);
    push_f32(&mut out, game.player.pitch);
    out.push(game.player.flying as u8);
    for count in game.counts {
        push_u32(&mut out, count);
    }
    push_u32(&mut out, edits.len() as u32);
    for (x, y, z, block_id) in edits {
        push_i32(&mut out, x);
        push_i32(&mut out, y);
        push_i32(&mut out, z);
        out.push(block_id);
    }
    out
}

/// Restore a session encoded by [`encode`]. Returns false for corrupt data.
pub fn decode(data: &[u8], game: &mut Game) -> bool {
    let mut r = Reader::new(data);
    if data.len() < 8 || data[..4] != MAGIC {
        return false;
    }
    r.pos = 4;
    let Some(version) = r.u32() else { return false };
    if version != VERSION {
        return false;
    }
    let Some(seed) = r.u32() else { return false };
    let Some(time) = r.f32() else { return false };
    let Some(mode) = r.u8() else { return false };
    let Some(selected) = r.u8() else { return false };
    let mut hotbar = [0u8; HOTBAR_SLOTS];
    for slot in &mut hotbar {
        let Some(v) = r.u8() else { return false };
        *slot = v;
    }
    let (Some(px), Some(py), Some(pz)) = (r.f32(), r.f32(), r.f32()) else {
        return false;
    };
    let (Some(yaw), Some(pitch)) = (r.f32(), r.f32()) else {
        return false;
    };
    let Some(flying) = r.u8() else { return false };
    let mut counts = [0u32; 256];
    for count in &mut counts {
        let Some(v) = r.u32() else { return false };
        *count = v;
    }
    let Some(edit_count) = r.u32() else { return false };
    let mut edits = Vec::with_capacity(edit_count.min(4_000_000) as usize);
    for _ in 0..edit_count {
        let (Some(x), Some(y), Some(z), Some(block_id)) = (r.i32(), r.i32(), r.i32(), r.u8())
        else {
            return false;
        };
        edits.push((x, y, z, block_id));
    }

    game.world = crate::world::World::new(seed);
    game.world.set_edits(edits);
    game.time = time.rem_euclid(1.0);
    game.mode = if mode == 1 {
        GameMode::Survival
    } else {
        GameMode::Creative
    };
    game.selected = (selected as usize).min(HOTBAR_SLOTS - 1);
    game.hotbar = hotbar;
    game.player.pos = glam::Vec3::new(px, py, pz);
    game.player.vel = glam::Vec3::ZERO;
    game.player.yaw = yaw;
    game.player.pitch = pitch;
    game.player.flying = flying != 0;
    game.counts = counts;
    game.particles.clear();
    game.loaded_chunks = 0;
    game.stream_chunks();
    let mut pos = game.player.pos;
    crate::physics::unstick(&game.world, &mut pos);
    game.player.pos = pos;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::id;

    #[test]
    fn roundtrip_preserves_state() {
        let mut game = Game::new(4242, 3);
        game.mode = GameMode::Survival;
        game.counts[id::STONE as usize] = 17;
        game.player.pos = glam::Vec3::new(12.5, 70.0, -3.5);
        game.player.yaw = 1.25;
        game.time = 0.42;
        let x = game.player.pos.x.floor() as i32;
        let z = game.player.pos.z.floor() as i32;
        game.world.load_chunk((x.div_euclid(16), z.div_euclid(16)));
        game.world.set(x, 64, z, id::GLASS);

        let bytes = encode(&game);
        let mut restored = Game::new(1, 3);
        assert!(decode(&bytes, &mut restored));

        assert_eq!(restored.world.seed(), 4242);
        assert_eq!(restored.mode, GameMode::Survival);
        assert_eq!(restored.counts[id::STONE as usize], 17);
        assert!((restored.time - 0.42).abs() < 1e-6);
        assert!((restored.player.yaw - 1.25).abs() < 1e-6);
        assert_eq!(restored.world.get(x, 64, z), id::GLASS);
    }

    #[test]
    fn rejects_garbage() {
        let mut game = Game::new(1, 2);
        assert!(!decode(b"not a save file", &mut game));
        assert!(!decode(&[], &mut game));
    }
}
