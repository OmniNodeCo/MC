//! Deterministic world generation: continents, biomes, caves, ores and
//! vegetation. The same seed always produces the same world, on every
//! platform.

use crate::block::id;
use crate::chunk::{Chunk, ChunkPos, CHUNK_X, CHUNK_Y, CHUNK_Z, SEA_LEVEL};
use crate::noise::{fbm2, hash2, hash3, perlin3, rand01, smoothstep, Rng};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Ocean,
    Beach,
    Plains,
    Forest,
    Desert,
    Snowy,
    Mountains,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "Ocean",
            Biome::Beach => "Beach",
            Biome::Plains => "Plains",
            Biome::Forest => "Forest",
            Biome::Desert => "Desert",
            Biome::Snowy => "Snowy Tundra",
            Biome::Mountains => "Mountains",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub height: i32,
    pub biome: Biome,
    /// -1.0 (cold) .. 1.0 (hot)
    pub temperature: f32,
}

/// Sample the terrain column at world coordinates `x, z`.
pub fn column(seed: u32, x: i32, z: i32) -> Column {
    let fx = x as f32;
    let fz = z as f32;

    let continent = fbm2(seed ^ 0x1111_1111, fx * 0.0022, fz * 0.0022, 4);
    let hills = fbm2(seed ^ 0x2222_2222, fx * 0.012, fz * 0.012, 4);
    let ridge = 1.0 - fbm2(seed ^ 0x3333_3333, fx * 0.0055, fz * 0.0055, 3).abs();
    let detail = fbm2(seed ^ 0x4444_4444, fx * 0.06, fz * 0.06, 2);

    let base = SEA_LEVEL as f32 + continent * 44.0;
    let mountain = smoothstep(0.30, 0.92, continent) * ridge * 54.0;
    let height = base + hills * 9.0 + mountain + detail * 2.5;
    let height = height.clamp(3.0, (CHUNK_Y - 8) as f32) as i32;

    let temperature = fbm2(seed ^ 0x5555_5555, fx * 0.0035, fz * 0.0035, 3);
    let humidity = fbm2(seed ^ 0x6666_6666, fx * 0.0030 + 91.7, fz * 0.0030 - 34.1, 3);

    let biome = if height < SEA_LEVEL - 2 {
        Biome::Ocean
    } else if height <= SEA_LEVEL + 1 {
        Biome::Beach
    } else if height > SEA_LEVEL + 26 {
        Biome::Mountains
    } else if temperature > 0.34 && humidity < 0.08 {
        Biome::Desert
    } else if temperature < -0.30 {
        Biome::Snowy
    } else if humidity > 0.10 {
        Biome::Forest
    } else {
        Biome::Plains
    };

    Column {
        height,
        biome,
        temperature,
    }
}

/// Cheap surface height without generating a chunk.
pub fn height_at(seed: u32, x: i32, z: i32) -> i32 {
    column(seed, x, z).height
}

/// Find a pleasant spawn point: walk outwards in rings until we hit dry land.
pub fn find_spawn(seed: u32) -> (i32, i32) {
    for ring in 0..64 {
        let r = ring * 8;
        let steps = (r * 2).max(1);
        for i in 0..steps {
            let t = i as f32 / steps as f32 * std::f32::consts::TAU;
            let x = (t.cos() * r as f32) as i32;
            let z = (t.sin() * r as f32) as i32;
            let c = column(seed, x, z);
            if c.biome != Biome::Ocean && c.height > SEA_LEVEL + 1 && c.height < SEA_LEVEL + 20 {
                return (x, z);
            }
        }
    }
    (0, 0)
}

#[inline]
fn is_cave(seed: u32, x: i32, y: i32, z: i32) -> bool {
    if y < 2 {
        return false;
    }
    let yf = y as f32;
    // Two crossing Perlin fields carve winding spaghetti tunnels.
    let n1 = perlin3(seed ^ 0xCAFE_0001, x as f32 * 0.045, yf * 0.085, z as f32 * 0.045);
    let n2 = perlin3(
        seed ^ 0xBEEF_0002,
        x as f32 * 0.045 + 51.0,
        yf * 0.085,
        z as f32 * 0.045 - 17.0,
    );
    if n1.abs() < 0.070 && n2.abs() < 0.070 {
        return true;
    }
    // Deeper open caverns.
    if y < 46 {
        let caves = perlin3(seed ^ 0xF00D_0003, x as f32 * 0.02, yf * 0.035, z as f32 * 0.02);
        if caves > 0.60 {
            return true;
        }
    }
    false
}

#[inline]
fn ore_for(seed: u32, x: i32, y: i32, z: i32) -> u8 {
    // Coarse 2x2x2 cells turn single blocks into little veins.
    let h = hash3(seed ^ 0x0BE5_0004, x >> 1, y >> 1, z >> 1);
    let r = h % 1000;
    if y < 15 && r < 4 {
        id::DIAMOND_ORE
    } else if y < 32 && (10..18).contains(&r) {
        id::GOLD_ORE
    } else if y < 64 && (30..52).contains(&r) {
        id::IRON_ORE
    } else if (78..132).contains(&r) {
        id::COAL_ORE
    } else {
        id::STONE
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TreeKind {
    Oak,
    Birch,
    Spruce,
    Cactus,
}

#[inline]
fn set_world(chunk: &mut Chunk, base_x: i32, base_z: i32, x: i32, y: i32, z: i32, block: u8) {
    let lx = x - base_x;
    let lz = z - base_z;
    if lx < 0 || lx >= CHUNK_X as i32 || lz < 0 || lz >= CHUNK_Z as i32 {
        return;
    }
    if y < 0 || y >= CHUNK_Y as i32 {
        return;
    }
    chunk.set(lx as usize, y as usize, lz as usize, block);
}

#[inline]
fn set_if_air(chunk: &mut Chunk, base_x: i32, base_z: i32, x: i32, y: i32, z: i32, block: u8) {
    let lx = x - base_x;
    let lz = z - base_z;
    if lx < 0 || lx >= CHUNK_X as i32 || lz < 0 || lz >= CHUNK_Z as i32 {
        return;
    }
    if y < 0 || y >= CHUNK_Y as i32 {
        return;
    }
    if chunk.get(lx as usize, y as usize, lz as usize) == id::AIR {
        chunk.set(lx as usize, y as usize, lz as usize, block);
    }
}

fn place_tree(
    chunk: &mut Chunk,
    base_x: i32,
    base_z: i32,
    x: i32,
    y: i32,
    z: i32,
    kind: TreeKind,
) {
    let mut rng = Rng::new(hash2(0x7EE7_0005, x, z));
    match kind {
        TreeKind::Cactus => {
            let h = rng.range(2, 4);
            for i in 0..h {
                set_world(chunk, base_x, base_z, x, y + i, z, id::CACTUS);
            }
        }
        TreeKind::Oak | TreeKind::Birch => {
            let (log, leaves, trunk_h) = if kind == TreeKind::Oak {
                (id::OAK_LOG, id::OAK_LEAVES, rng.range(4, 6))
            } else {
                (id::BIRCH_LOG, id::BIRCH_LEAVES, rng.range(5, 7))
            };
            for i in 0..trunk_h {
                set_world(chunk, base_x, base_z, x, y + i, z, log);
            }
            let top = y + trunk_h - 1;
            for dy in -2i32..=1 {
                let r = if dy <= 0 { 2 } else { 1 };
                for dx in -r..=r {
                    for dz in -r..=r {
                        let corner = dx.abs() == r && dz.abs() == r;
                        if corner && (dy == 1 || rng.chance(0.55)) {
                            continue;
                        }
                        set_if_air(chunk, base_x, base_z, x + dx, top + dy, z + dz, leaves);
                    }
                }
            }
        }
        TreeKind::Spruce => {
            let trunk_h = rng.range(6, 9);
            for i in 0..trunk_h {
                set_world(chunk, base_x, base_z, x, y + i, z, id::OAK_LOG);
            }
            let top = y + trunk_h - 1;
            let mut radius = 0i32;
            for dy in (0..trunk_h - 1).rev() {
                let level = (trunk_h - 1 - dy) as i32;
                radius = if level % 3 == 0 { 2 } else { 1 };
                if level > trunk_h - 3 {
                    radius = 1;
                }
                let ly = y + dy + 1;
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        if dx.abs() == radius && dz.abs() == radius {
                            continue;
                        }
                        set_if_air(chunk, base_x, base_z, x + dx, ly, z + dz, id::OAK_LEAVES);
                    }
                }
            }
            set_if_air(chunk, base_x, base_z, x, top + 1, z, id::OAK_LEAVES);
        }
    }
}

fn decorate(seed: u32, chunk: &mut Chunk, pos: ChunkPos) {
    let base_x = pos.0 * CHUNK_X as i32;
    let base_z = pos.1 * CHUNK_Z as i32;

    for dz in -2i32..CHUNK_Z as i32 + 2 {
        for dx in -2i32..CHUNK_X as i32 + 2 {
            let wx = base_x + dx;
            let wz = base_z + dz;
            let col = column(seed, wx, wz);
            let h = col.height;
            if h <= SEA_LEVEL {
                // frozen oceans
                if col.biome == Biome::Snowy && h == SEA_LEVEL {
                    set_if_air(
                        chunk,
                        base_x,
                        base_z,
                        wx,
                        SEA_LEVEL,
                        wz,
                        id::ICE,
                    );
                }
                continue;
            }

            let roll = rand01(hash2(seed ^ 0x7E7E_0006, wx, wz));
            let tree_chance = match col.biome {
                Biome::Forest => 0.030,
                Biome::Plains => 0.004,
                Biome::Snowy => 0.012,
                Biome::Mountains => 0.004,
                Biome::Desert => 0.006,
                _ => 0.0,
            };

            if roll < tree_chance && h < 108 {
                let kind = match col.biome {
                    Biome::Forest => {
                        if rand01(hash2(seed ^ 0x1234_0007, wx, wz)) < 0.35 {
                            TreeKind::Birch
                        } else {
                            TreeKind::Oak
                        }
                    }
                    Biome::Desert => TreeKind::Cactus,
                    Biome::Snowy | Biome::Mountains => TreeKind::Spruce,
                    _ => TreeKind::Oak,
                };
                place_tree(chunk, base_x, base_z, wx, h + 1, wz, kind);
            } else if roll > 0.86 {
                // ground cover
                let ground = match col.biome {
                    Biome::Plains => id::GRASS,
                    Biome::Forest => id::GRASS,
                    Biome::Snowy => id::SNOW,
                    _ => continue,
                };
                let plant_roll = rand01(hash2(seed ^ 0x9999_0008, wx, wz));
                let plant = if plant_roll < 0.10 {
                    id::TALL_GRASS
                } else if plant_roll < 0.115 {
                    id::POPPY
                } else if plant_roll < 0.13 {
                    id::DANDELION
                } else {
                    continue;
                };
                if col.biome == Biome::Snowy {
                    continue;
                }
                let lx = wx - base_x;
                let lz = wz - base_z;
                if lx >= 0
                    && lx < CHUNK_X as i32
                    && lz >= 0
                    && lz < CHUNK_Z as i32
                    && chunk.get(lx as usize, h as usize, lz as usize) == ground
                {
                    chunk.set(lx as usize, (h + 1) as usize, lz as usize, plant);
                }
            }
        }
    }
}

/// Generate a whole chunk from the seed.
pub fn generate_chunk(seed: u32, pos: ChunkPos) -> Chunk {
    let mut chunk = Chunk::new(pos);
    let base_x = pos.0 * CHUNK_X as i32;
    let base_z = pos.1 * CHUNK_Z as i32;

    let mut heights = [0i32; CHUNK_X * CHUNK_Z];
    let mut biomes = [Biome::Plains; CHUNK_X * CHUNK_Z];
    let mut temps = [0.0f32; CHUNK_X * CHUNK_Z];

    for z in 0..CHUNK_Z {
        for x in 0..CHUNK_X {
            let c = column(seed, base_x + x as i32, base_z + z as i32);
            let i = z * CHUNK_X + x;
            heights[i] = c.height;
            biomes[i] = c.biome;
            temps[i] = c.temperature;
        }
    }

    for z in 0..CHUNK_Z {
        for x in 0..CHUNK_X {
            let i = z * CHUNK_X + x;
            let h = heights[i];
            let biome = biomes[i];
            let cold = temps[i];

            // bedrock floor
            chunk.set(x, 0, z, id::BEDROCK);
            if rand01(hash3(seed ^ 0xBD00_0009, base_x + x as i32, 1, base_z + z as i32)) < 0.5 {
                chunk.set(x, 1, z, id::BEDROCK);
            }

            let (surface, filler) = match biome {
                Biome::Ocean => {
                    if h < SEA_LEVEL - 12 {
                        (id::GRAVEL, id::STONE)
                    } else {
                        (id::SAND, id::SANDSTONE)
                    }
                }
                Biome::Beach => (id::SAND, id::SANDSTONE),
                Biome::Desert => (id::SAND, id::SANDSTONE),
                Biome::Snowy => (id::SNOW, id::DIRT),
                Biome::Mountains => {
                    if h > 96 {
                        (id::SNOW, id::STONE)
                    } else if h > SEA_LEVEL + 14 {
                        (id::STONE, id::STONE)
                    } else {
                        (id::GRASS, id::DIRT)
                    }
                }
                Biome::Forest | Biome::Plains => {
                    if h <= SEA_LEVEL + 1 {
                        (id::SAND, id::SAND)
                    } else {
                        (id::GRASS, id::DIRT)
                    }
                }
            };

            let soil = if biome == Biome::Desert || biome == Biome::Beach {
                3
            } else {
                4
            };

            for y in 0..=h {
                let block = if y == h {
                    surface
                } else if y > h - soil {
                    if y > h - soil - 1 && y == h - 1 {
                        filler
                    } else {
                        filler
                    }
                } else {
                    ore_for(seed, base_x + x as i32, y, base_z + z as i32)
                };
                chunk.set(x, y as usize, z, block);
            }

            // carve caves
            for y in 2..h {
                if is_cave(seed, base_x + x as i32, y, base_z + z as i32) {
                    chunk.set(x, y as usize, z, id::AIR);
                }
            }

            // ocean / lake water
            for y in (h + 1)..=SEA_LEVEL {
                chunk.set(x, y as usize, z, id::WATER);
            }

            // snow caps on very high mountains
            if h > 100 && biome == Biome::Mountains {
                chunk.set(x, h as usize, z, id::SNOW);
            }
        }
    }

    decorate(seed, &mut chunk, pos);
    chunk.rebuild_height();
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let a = generate_chunk(99, (2, -3));
        let b = generate_chunk(99, (2, -3));
        assert_eq!(a.blocks, b.blocks);
    }

    #[test]
    fn different_seeds_differ() {
        let a = generate_chunk(1, (0, 0));
        let b = generate_chunk(2, (0, 0));
        assert_ne!(a.blocks, b.blocks);
    }

    #[test]
    fn surface_is_solid_and_covers_the_column() {
        let c = generate_chunk(7, (0, 0));
        let col = column(7, 3, 9);
        let top = c.top_at(3, 9) as i32;
        assert!(top >= col.height, "top {top} height {}", col.height);
        assert_ne!(c.get(3, 0, 9), id::AIR);
    }

    #[test]
    fn spawn_is_on_land() {
        for seed in [1u32, 2, 3, 12345] {
            let (x, z) = find_spawn(seed);
            let c = column(seed, x, z);
            assert!(c.height > SEA_LEVEL, "seed {seed} spawned in water");
        }
    }

    #[test]
    fn height_map_matches_blocks() {
        let c = generate_chunk(5, (1, 1));
        for z in 0..CHUNK_Z {
            for x in 0..CHUNK_X {
                let top = c.top_at(x, z) as usize;
                assert_ne!(c.get(x, top, z), id::AIR);
                for y in top + 1..CHUNK_Y {
                    assert_eq!(c.get(x, y, z), id::AIR);
                }
            }
        }
    }
}
