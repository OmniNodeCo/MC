//! Procedurally generated 512x512 texture atlas.
//!
//! No binary assets ship with the game: every block texture, the destroy-stage
//! crack overlay and the bitmap font are painted into one atlas at start-up.

use crate::noise::Rng;

pub const ATLAS_COLS: u32 = 32;
pub const TILE_PX: u32 = 16;
pub const ATLAS_PX: u32 = ATLAS_COLS * TILE_PX;
/// Index of the first destroy-stage (crack) tile.
pub const DESTROY_TILE_BASE: u32 = 18 * ATLAS_COLS;
pub const DESTROY_STAGES: u32 = 10;
/// Index of the atlas cell holding the glyph for ASCII 32 (space).
pub const FONT_CELL_BASE: u32 = 20 * ATLAS_COLS;

/// Tile indices inside the atlas.
pub mod tiles {
    pub const STONE: u32 = 0;
    pub const DIRT: u32 = 1;
    pub const GRASS_TOP: u32 = 2;
    pub const GRASS_SIDE: u32 = 3;
    pub const COBBLESTONE: u32 = 4;
    pub const PLANKS: u32 = 5;
    pub const BEDROCK: u32 = 6;
    pub const SAND: u32 = 7;
    pub const GRAVEL: u32 = 8;
    pub const LOG_SIDE: u32 = 9;
    pub const LOG_TOP: u32 = 10;
    pub const LEAVES: u32 = 11;
    pub const GLASS: u32 = 12;
    pub const WATER: u32 = 13;
    pub const COAL_ORE: u32 = 14;
    pub const IRON_ORE: u32 = 15;
    pub const GOLD_ORE: u32 = 16;
    pub const DIAMOND_ORE: u32 = 17;
    pub const SNOW: u32 = 18;
    pub const CACTUS_SIDE: u32 = 19;
    pub const CACTUS_TOP: u32 = 20;
    pub const TALL_GRASS: u32 = 21;
    pub const POPPY: u32 = 22;
    pub const DANDELION: u32 = 23;
    pub const SANDSTONE: u32 = 24;
    pub const ICE: u32 = 25;
    pub const BRICKS: u32 = 26;
    pub const CRAFTING_TOP: u32 = 27;
    pub const CRAFTING_SIDE: u32 = 28;
    pub const BOOKSHELF: u32 = 29;
    pub const MOSSY_COBBLESTONE: u32 = 30;
    pub const STONE_BRICKS: u32 = 31;
    pub const BIRCH_SIDE: u32 = 32;
    pub const BIRCH_TOP: u32 = 33;
    pub const BIRCH_LEAVES: u32 = 34;
    pub const GLOWSTONE: u32 = 35;
    /// Solid white tile, used for HUD rectangles.
    pub const WHITE: u32 = 36;
    pub const COUNT: u32 = 37;
}

/// UV rectangle `[u0, v0, u1, v1]` of a tile, with a small inset so nearest
/// filtered sampling never picks up the neighbouring tile.
pub fn tile_uv(tile: u32) -> [f32; 4] {
    cell_uv(tile)
}

/// UV of the centre of the white tile, for untextured HUD quads.
pub fn white_uv() -> [f32; 2] {
    let [u0, v0, u1, v1] = cell_uv(tiles::WHITE);
    [(u0 + u1) * 0.5, (v0 + v1) * 0.5]
}

/// UV rectangle of an arbitrary atlas cell (used for font glyphs too).
pub fn cell_uv(cell: u32) -> [f32; 4] {
    let col = (cell % ATLAS_COLS) as f32;
    let row = (cell / ATLAS_COLS) as f32;
    let s = 1.0 / ATLAS_PX as f32;
    let inset = 0.35;
    [
        (col * TILE_PX as f32 + inset) * s,
        (row * TILE_PX as f32 + inset) * s,
        ((col + 1.0) * TILE_PX as f32 - inset) * s,
        ((row + 1.0) * TILE_PX as f32 - inset) * s,
    ]
}

type Rgba = [u8; 4];

fn clamp8(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v as u8
    }
}

fn rgb(c: [u8; 3]) -> Rgba {
    [c[0], c[1], c[2], 255]
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        clamp8(a[0] as f32 * (1.0 - t) + b[0] as f32 * t),
        clamp8(a[1] as f32 * (1.0 - t) + b[1] as f32 * t),
        clamp8(a[2] as f32 * (1.0 - t) + b[2] as f32 * t),
    ]
}

fn jitter(base: [u8; 3], amount: f32, rng: &mut Rng) -> Rgba {
    let d = (rng.next_f32() * 2.0 - 1.0) * amount;
    [
        clamp8(base[0] as f32 + d),
        clamp8(base[1] as f32 + d),
        clamp8(base[2] as f32 + d),
        255,
    ]
}

/// A writable view onto one 16x16 tile of the atlas.
struct Tile<'a> {
    pixels: &'a mut [u8],
    origin: usize,
}

impl<'a> Tile<'a> {
    fn new(atlas: &'a mut [u8], index: u32) -> Self {
        let origin = tile_origin(index);
        Self {
            pixels: atlas,
            origin,
        }
    }

    fn put(&mut self, x: u32, y: u32, c: Rgba) {
        if x >= TILE_PX || y >= TILE_PX {
            return;
        }
        let p = self.origin + ((y * ATLAS_PX + x) * 4) as usize;
        self.pixels[p..p + 4].copy_from_slice(&c);
    }

    fn get(&self, x: u32, y: u32) -> Rgba {
        let p = self.origin + ((y * ATLAS_PX + x) * 4) as usize;
        [
            self.pixels[p],
            self.pixels[p + 1],
            self.pixels[p + 2],
            self.pixels[p + 3],
        ]
    }

    fn fill(&mut self, c: Rgba) {
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                self.put(x, y, c);
            }
        }
    }

    fn rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: Rgba) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }

    fn noise_fill(&mut self, base: [u8; 3], amount: f32, rng: &mut Rng) {
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let c = jitter(base, amount, rng);
                self.put(x, y, c);
            }
        }
    }

    fn speckles(&mut self, count: u32, c: [u8; 3], rng: &mut Rng) {
        for _ in 0..count {
            let x = rng.range(0, TILE_PX as i32 - 1) as u32;
            let y = rng.range(0, TILE_PX as i32 - 1) as u32;
            let a = 0.55 + rng.next_f32() * 0.45;
            let cur = self.get(x, y);
            let mixed = mix([cur[0], cur[1], cur[2]], c, a);
            self.put(x, y, rgb(mixed));
        }
    }

    fn blobs(&mut self, count: u32, color: [u8; 3], size: u32, rng: &mut Rng) {
        for _ in 0..count {
            let cx = rng.range(2, TILE_PX as i32 - 3) as i32;
            let cy = rng.range(2, TILE_PX as i32 - 3) as i32;
            let r = size as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy > r * r {
                        continue;
                    }
                    let x = (cx + dx).rem_euclid(TILE_PX as i32) as u32;
                    let y = (cy + dy).rem_euclid(TILE_PX as i32) as u32;
                    let cur = self.get(x, y);
                    let shade = 0.8 + rng.next_f32() * 0.4;
                    let m = mix([cur[0], cur[1], cur[2]], color, shade.min(1.0));
                    self.put(x, y, rgb(m));
                }
            }
        }
    }

    fn ore(&mut self, ore: [u8; 3], highlight: [u8; 3], rng: &mut Rng) {
        self.noise_fill([128, 128, 128], 14.0, rng);
        for _ in 0..5 {
            let cx = rng.range(2, TILE_PX as i32 - 3);
            let cy = rng.range(2, TILE_PX as i32 - 3);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx * dx + dy * dy > 2 {
                        continue;
                    }
                    self.put((cx + dx) as u32, (cy + dy) as u32, rgb(ore));
                }
            }
            self.put(cx as u32, cy as u32, rgb(highlight));
        }
    }
}

fn tile_origin(index: u32) -> usize {
    let col = index % ATLAS_COLS;
    let row = index / ATLAS_COLS;
    ((row * TILE_PX * ATLAS_PX) + col * TILE_PX) as usize * 4
}

/// Paint every block texture, the destroy overlay and the font into a fresh
/// RGBA8 atlas.
pub fn build_atlas() -> Vec<u8> {
    let mut atlas = vec![0u8; (ATLAS_PX * ATLAS_PX * 4) as usize];

    // ---- stone family -----------------------------------------------------
    {
        let mut rng = Rng::new(1);
        let mut t = Tile::new(&mut atlas, tiles::STONE);
        t.noise_fill([128, 128, 128], 10.0, &mut rng);
        t.speckles(40, [112, 112, 112], &mut rng);
        t.speckles(20, [146, 146, 146], &mut rng);
    }
    {
        let mut rng = Rng::new(2);
        let mut t = Tile::new(&mut atlas, tiles::COBBLESTONE);
        t.noise_fill([120, 120, 120], 8.0, &mut rng);
        // dark mortar grid with irregular stones
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let cell_x = (x / 5) as i32;
                let cell_y = (y / 4) as i32;
                let jitter = (crate::noise::hash2(9, cell_x, cell_y) % 3) as u32;
                let on_x = x % 5 == 0 && (y + jitter) % 4 != 3;
                let on_y = y % 4 == 0;
                if on_x || on_y {
                    let cur = t.get(x, y);
                    let m = mix([cur[0], cur[1], cur[2]], [70, 70, 70], 0.85);
                    t.put(x, y, rgb(m));
                }
            }
        }
        t.speckles(50, [154, 154, 154], &mut rng);
    }
    {
        let mut rng = Rng::new(3);
        let mut t = Tile::new(&mut atlas, tiles::STONE_BRICKS);
        t.noise_fill([124, 124, 124], 8.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let row = y / 8;
                let offset = row * 8;
                if y % 8 == 0 || (x + offset) % 16 == 0 {
                    t.put(x, y, rgb([84, 84, 84]));
                }
            }
        }
        t.speckles(24, [150, 150, 150], &mut rng);
    }
    {
        let mut rng = Rng::new(4);
        let mut t = Tile::new(&mut atlas, tiles::MOSSY_COBBLESTONE);
        t.noise_fill([112, 118, 104], 9.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let on_x = x % 5 == 0;
                let on_y = y % 4 == 0;
                if on_x || on_y {
                    t.put(x, y, rgb([70, 76, 62]));
                }
            }
        }
        t.blobs(8, [74, 116, 52], 2, &mut rng);
        t.speckles(20, [138, 146, 126], &mut rng);
    }
    {
        let mut rng = Rng::new(5);
        let mut t = Tile::new(&mut atlas, tiles::BEDROCK);
        t.noise_fill([56, 56, 60], 18.0, &mut rng);
        t.blobs(10, [30, 30, 34], 2, &mut rng);
        t.blobs(6, [96, 96, 100], 1, &mut rng);
    }
    {
        let mut rng = Rng::new(6);
        let mut t = Tile::new(&mut atlas, tiles::GRAVEL);
        t.noise_fill([126, 120, 116], 16.0, &mut rng);
        t.blobs(14, [96, 90, 86], 1, &mut rng);
        t.blobs(10, [158, 152, 146], 1, &mut rng);
    }
    {
        let mut rng = Rng::new(7);
        let mut t = Tile::new(&mut atlas, tiles::GOLD_ORE);
        t.ore([212, 172, 48], [248, 226, 122], &mut rng);
    }
    {
        let mut rng = Rng::new(8);
        let mut t = Tile::new(&mut atlas, tiles::COAL_ORE);
        t.ore([38, 38, 38], [74, 74, 74], &mut rng);
    }
    {
        let mut rng = Rng::new(9);
        let mut t = Tile::new(&mut atlas, tiles::IRON_ORE);
        t.ore([196, 152, 122], [226, 190, 166], &mut rng);
    }
    {
        let mut rng = Rng::new(10);
        let mut t = Tile::new(&mut atlas, tiles::DIAMOND_ORE);
        t.ore([78, 226, 226], [176, 250, 250], &mut rng);
    }

    // ---- soil -------------------------------------------------------------
    {
        let mut rng = Rng::new(11);
        let mut t = Tile::new(&mut atlas, tiles::DIRT);
        t.noise_fill([134, 96, 67], 12.0, &mut rng);
        t.speckles(30, [108, 76, 52], &mut rng);
        t.speckles(18, [156, 116, 84], &mut rng);
    }
    {
        let mut rng = Rng::new(12);
        let mut t = Tile::new(&mut atlas, tiles::GRASS_TOP);
        t.noise_fill([106, 170, 64], 12.0, &mut rng);
        t.speckles(34, [88, 146, 50], &mut rng);
        t.speckles(22, [128, 190, 80], &mut rng);
    }
    {
        // grass side: dirt with a ragged grass fringe on top
        let mut rng = Rng::new(13);
        {
            let mut t = Tile::new(&mut atlas, tiles::GRASS_SIDE);
            t.noise_fill([134, 96, 67], 12.0, &mut rng);
            t.speckles(24, [108, 76, 52], &mut rng);
        }
        let mut t = Tile::new(&mut atlas, tiles::GRASS_SIDE);
        for x in 0..TILE_PX {
            let depth = 3 + (crate::noise::hash2(21, x as i32, 3) % 4);
            for y in 0..depth {
                let c = jitter([102, 166, 62], 10.0, &mut rng);
                t.put(x, y, c);
            }
        }
    }
    {
        let mut rng = Rng::new(14);
        let mut t = Tile::new(&mut atlas, tiles::SAND);
        t.noise_fill([219, 207, 163], 8.0, &mut rng);
        t.speckles(24, [204, 190, 146], &mut rng);
    }
    {
        let mut rng = Rng::new(15);
        let mut t = Tile::new(&mut atlas, tiles::SANDSTONE);
        t.noise_fill([222, 210, 170], 6.0, &mut rng);
        for y in [0u32, 1, 14, 15] {
            for x in 0..TILE_PX {
                let cur = t.get(x, y);
                let m = mix([cur[0], cur[1], cur[2]], [196, 184, 144], 0.8);
                t.put(x, y, rgb(m));
            }
        }
    }
    {
        let mut rng = Rng::new(16);
        let mut t = Tile::new(&mut atlas, tiles::SNOW);
        t.noise_fill([240, 246, 252], 5.0, &mut rng);
        t.speckles(18, [222, 232, 244], &mut rng);
    }
    {
        let mut rng = Rng::new(17);
        let mut t = Tile::new(&mut atlas, tiles::ICE);
        t.noise_fill([150, 200, 240], 6.0, &mut rng);
        for i in 0..TILE_PX {
            let cur = t.get(i, (i + 4) % TILE_PX);
            let m = mix([cur[0], cur[1], cur[2]], [210, 238, 255], 0.7);
            t.put(i, (i + 4) % TILE_PX, [m[0], m[1], m[2], 205]);
        }
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let mut c = t.get(x, y);
                c[3] = 200;
                t.put(x, y, c);
            }
        }
    }

    // ---- wood -------------------------------------------------------------
    {
        let mut rng = Rng::new(18);
        let mut t = Tile::new(&mut atlas, tiles::LOG_SIDE);
        t.noise_fill([104, 82, 50], 8.0, &mut rng);
        for x in 0..TILE_PX {
            if x % 4 == 0 {
                for y in 0..TILE_PX {
                    let cur = t.get(x, y);
                    let m = mix([cur[0], cur[1], cur[2]], [74, 58, 34], 0.8);
                    t.put(x, y, rgb(m));
                }
            }
        }
        t.speckles(30, [122, 98, 62], &mut rng);
    }
    {
        let mut rng = Rng::new(19);
        let mut t = Tile::new(&mut atlas, tiles::LOG_TOP);
        t.noise_fill([168, 138, 88], 6.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 7.5;
                let d = (dx * dx + dy * dy).sqrt();
                let ring = (d * 2.1).sin();
                if ring > 0.55 {
                    let cur = t.get(x, y);
                    let m = mix([cur[0], cur[1], cur[2]], [136, 108, 64], 0.75);
                    t.put(x, y, rgb(m));
                }
            }
        }
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                if x == 0 || y == 0 || x == TILE_PX - 1 || y == TILE_PX - 1 {
                    t.put(x, y, rgb([96, 74, 44]));
                }
            }
        }
    }
    {
        let mut rng = Rng::new(20);
        let mut t = Tile::new(&mut atlas, tiles::BIRCH_SIDE);
        t.noise_fill([214, 212, 200], 6.0, &mut rng);
        for _ in 0..14 {
            let x = rng.range(0, TILE_PX as i32 - 3) as u32;
            let y = rng.range(0, TILE_PX as i32 - 1) as u32;
            let len = rng.range(1, 3) as u32;
            t.rect(x, y, len, 1, rgb([54, 52, 48]));
        }
    }
    {
        let mut rng = Rng::new(21);
        let mut t = Tile::new(&mut atlas, tiles::BIRCH_TOP);
        t.noise_fill([206, 196, 168], 5.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let dx = x as f32 - 7.5;
                let dy = y as f32 - 7.5;
                let d = (dx * dx + dy * dy).sqrt();
                if (d * 2.0).sin() > 0.5 {
                    t.put(x, y, rgb([176, 164, 136]));
                }
            }
        }
    }
    {
        let mut rng = Rng::new(22);
        let mut t = Tile::new(&mut atlas, tiles::PLANKS);
        t.noise_fill([162, 130, 78], 7.0, &mut rng);
        for y in [0u32, 4, 8, 12] {
            for x in 0..TILE_PX {
                let cur = t.get(x, y);
                let m = mix([cur[0], cur[1], cur[2]], [116, 90, 50], 0.85);
                t.put(x, y, rgb(m));
            }
        }
        for y in 0..TILE_PX {
            let seam = if y < 4 {
                5
            } else if y < 8 {
                11
            } else if y < 12 {
                3
            } else {
                9
            };
            let cur = t.get(seam, y);
            let m = mix([cur[0], cur[1], cur[2]], [116, 90, 50], 0.7);
            t.put(seam, y, rgb(m));
        }
        t.speckles(24, [182, 150, 96], &mut rng);
    }
    {
        let mut rng = Rng::new(23);
        let mut t = Tile::new(&mut atlas, tiles::BOOKSHELF);
        t.noise_fill([162, 130, 78], 6.0, &mut rng);
        t.rect(0, 0, TILE_PX, 3, rgb([134, 106, 62]));
        t.rect(0, 13, TILE_PX, 3, rgb([134, 106, 62]));
        let books = [
            [156, 52, 48],
            [60, 92, 156],
            [214, 196, 92],
            [72, 132, 72],
            [138, 72, 150],
        ];
        for shelf in 0..2 {
            let y0 = 3 + shelf * 7;
            let mut x = 1;
            let mut i = 0;
            while x < TILE_PX - 1 {
                let c = books[(i + shelf * 2) % books.len()];
                let w = 2;
                t.rect(x, y0, w.min(TILE_PX - 1 - x), 4, rgb(c));
                x += w + 1;
                i += 1;
            }
        }
    }
    {
        let mut rng = Rng::new(24);
        let mut t = Tile::new(&mut atlas, tiles::CRAFTING_TOP);
        t.noise_fill([162, 130, 78], 6.0, &mut rng);
        for i in 0..TILE_PX {
            t.put(i, 0, rgb([116, 90, 50]));
            t.put(i, TILE_PX - 1, rgb([116, 90, 50]));
            t.put(0, i, rgb([116, 90, 50]));
            t.put(TILE_PX - 1, i, rgb([116, 90, 50]));
            t.put(i, 5, rgb([120, 94, 54]));
            t.put(i, 10, rgb([120, 94, 54]));
            t.put(5, i, rgb([120, 94, 54]));
            t.put(10, i, rgb([120, 94, 54]));
        }
    }
    {
        let mut rng = Rng::new(25);
        let mut t = Tile::new(&mut atlas, tiles::CRAFTING_SIDE);
        t.noise_fill([150, 120, 72], 7.0, &mut rng);
        t.rect(0, 0, TILE_PX, 4, rgb([134, 106, 62]));
        t.rect(2, 6, 5, 6, rgb([96, 74, 44]));
        t.rect(9, 6, 4, 3, rgb([176, 148, 96]));
        t.rect(9, 11, 4, 3, rgb([70, 62, 52]));
    }

    // ---- foliage ----------------------------------------------------------
    {
        let mut rng = Rng::new(26);
        let mut t = Tile::new(&mut atlas, tiles::LEAVES);
        t.noise_fill([62, 112, 42], 16.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                if rng.next_f32() < 0.22 {
                    let mut c = t.get(x, y);
                    c[3] = 0;
                    t.put(x, y, c);
                }
            }
        }
        t.speckles(40, [44, 88, 30], &mut rng);
        t.speckles(20, [92, 142, 58], &mut rng);
    }
    {
        let mut rng = Rng::new(27);
        let mut t = Tile::new(&mut atlas, tiles::BIRCH_LEAVES);
        t.noise_fill([112, 158, 76], 14.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                if rng.next_f32() < 0.24 {
                    let mut c = t.get(x, y);
                    c[3] = 0;
                    t.put(x, y, c);
                }
            }
        }
        t.speckles(36, [86, 130, 58], &mut rng);
    }
    {
        let mut rng = Rng::new(28);
        let mut t = Tile::new(&mut atlas, tiles::TALL_GRASS);
        t.fill([0, 0, 0, 0]);
        for blade in 0..7i32 {
            let bx = 1 + (crate::noise::hash2(31, blade, 7) % 14) as u32;
            let height = 6 + (crate::noise::hash2(32, blade, 9) % 8) as u32;
            let lean = if rng.chance(0.5) { 1i32 } else { -1 };
            for i in 0..height {
                let x = (bx as i32 + (i as i32 / 4) * lean).rem_euclid(TILE_PX as i32) as u32;
                let y = TILE_PX - 1 - i;
                let c = jitter([84, 148, 54], 12.0, &mut rng);
                t.put(x, y, c);
            }
        }
    }
    {
        let mut rng = Rng::new(29);
        let mut t = Tile::new(&mut atlas, tiles::POPPY);
        t.fill([0, 0, 0, 0]);
        for y in 6..TILE_PX {
            t.put(7, y, rgb([66, 122, 44]));
        }
        t.put(6, 10, rgb([66, 122, 44]));
        t.put(8, 12, rgb([66, 122, 44]));
        for dy in 0..3u32 {
            for dx in 0..3u32 {
                let c = jitter([214, 62, 58], 14.0, &mut rng);
                t.put(6 + dx, 3 + dy, c);
            }
        }
        t.put(7, 4, rgb([44, 26, 26]));
    }
    {
        let mut rng = Rng::new(30);
        let mut t = Tile::new(&mut atlas, tiles::DANDELION);
        t.fill([0, 0, 0, 0]);
        for y in 7..TILE_PX {
            t.put(7, y, rgb([66, 122, 44]));
        }
        t.put(6, 11, rgb([66, 122, 44]));
        t.put(8, 9, rgb([66, 122, 44]));
        for dy in 0..3u32 {
            for dx in 0..3u32 {
                let c = jitter([236, 208, 62], 12.0, &mut rng);
                t.put(6 + dx, 3 + dy, c);
            }
        }
    }
    {
        let mut rng = Rng::new(31);
        let mut t = Tile::new(&mut atlas, tiles::CACTUS_SIDE);
        t.noise_fill([58, 124, 52], 6.0, &mut rng);
        for x in [2u32, 7, 12] {
            for y in 0..TILE_PX {
                t.put(x, y, rgb([44, 100, 40]));
            }
        }
        for y in (1..TILE_PX).step_by(3) {
            t.put(4, y, rgb([206, 214, 176]));
            t.put(10, y, rgb([206, 214, 176]));
        }
    }
    {
        let mut rng = Rng::new(32);
        let mut t = Tile::new(&mut atlas, tiles::CACTUS_TOP);
        t.noise_fill([74, 145, 64], 6.0, &mut rng);
        t.rect(5, 5, 6, 6, rgb([96, 168, 84]));
        t.rect(7, 7, 2, 2, rgb([58, 124, 52]));
    }

    // ---- artificial -------------------------------------------------------
    {
        let mut rng = Rng::new(33);
        let mut t = Tile::new(&mut atlas, tiles::WATER);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let wave = ((x as f32 * 0.6 + y as f32 * 0.3).sin() * 6.0) as i32;
                let c = [
                    clamp8(46.0 + wave as f32),
                    clamp8(104.0 + wave as f32),
                    clamp8(196.0 + wave as f32),
                    198,
                ];
                t.put(x, y, c);
            }
        }
        t.noise_fill([52, 110, 200], 6.0, &mut rng);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let mut c = t.get(x, y);
                c[3] = 198;
                t.put(x, y, c);
            }
        }
    }
    {
        let mut rng = Rng::new(34);
        let mut t = Tile::new(&mut atlas, tiles::GLASS);
        t.fill([214, 236, 244, 0]);
        for i in 0..TILE_PX {
            let edge = [206, 232, 244, 220];
            t.put(i, 0, edge);
            t.put(i, TILE_PX - 1, edge);
            t.put(0, i, edge);
            t.put(TILE_PX - 1, i, edge);
        }
        for i in 2..6 {
            t.put(i, i, [236, 250, 255, 160]);
        }
        for i in 9..12 {
            t.put(i + 2, i, [236, 250, 255, 120]);
        }
        t.speckles(6, [255, 255, 255], &mut rng);
    }
    {
        let mut rng = Rng::new(35);
        let mut t = Tile::new(&mut atlas, tiles::BRICKS);
        t.fill([150, 80, 70, 255]);
        for y in 0..TILE_PX {
            for x in 0..TILE_PX {
                let row = y / 4;
                let mortar_row = y % 4 == 3;
                let offset = if row % 2 == 0 { 0 } else { 4 };
                let mortar_col = (x + offset) % 8 == 7;
                if mortar_row || mortar_col {
                    t.put(x, y, rgb([168, 160, 148]));
                } else {
                    let c = jitter([150, 80, 70], 10.0, &mut rng);
                    t.put(x, y, c);
                }
            }
        }
    }
    {
        let mut rng = Rng::new(36);
        let mut t = Tile::new(&mut atlas, tiles::GLOWSTONE);
        t.noise_fill([224, 196, 124], 14.0, &mut rng);
        t.blobs(12, [252, 236, 170], 1, &mut rng);
        t.blobs(8, [186, 148, 84], 1, &mut rng);
    }

    {
        let mut t = Tile::new(&mut atlas, tiles::WHITE);
        t.fill([255, 255, 255, 255]);
    }

    crate::font::paint_glyphs(&mut atlas);
    paint_destroy_stages(&mut atlas);

    atlas
}

/// Ten crack overlays that get darker and denser as a block breaks.
fn paint_destroy_stages(atlas: &mut [u8]) {
    for stage in 0..DESTROY_STAGES {
        let index = DESTROY_TILE_BASE + stage;
        let mut t = Tile::new(atlas, index);
        t.fill([0, 0, 0, 0]);
        let mut rng = Rng::new(100 + stage);
        let lines = 2 + stage as u32;
        for _ in 0..lines {
            let mut x = rng.range(2, 13) as i32;
            let mut y = rng.range(2, 13) as i32;
            let mut dx = if rng.chance(0.5) { 1 } else { -1 };
            let mut dy = if rng.chance(0.5) { 1 } else { -1 };
            let length = 3 + stage as i32;
            for _ in 0..length {
                if (0..16).contains(&x) && (0..16).contains(&y) {
                    t.put(x as u32, y as u32, [0, 0, 0, 190]);
                }
                if rng.chance(0.5) {
                    x += dx;
                } else {
                    y += dy;
                }
                if rng.chance(0.2) {
                    dx = -dx;
                }
                if rng.chance(0.2) {
                    dy = -dy;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_has_expected_size() {
        let atlas = build_atlas();
        assert_eq!(atlas.len(), (ATLAS_PX * ATLAS_PX * 4) as usize);
    }

    #[test]
    fn stone_tile_is_not_empty() {
        let atlas = build_atlas();
        let origin = tile_origin(tiles::STONE);
        assert!(atlas[origin + 3] > 0);
    }

    #[test]
    fn uvs_stay_in_unit_range() {
        for tile in 0..tiles::COUNT + DESTROY_STAGES + 100 {
            let uv = tile_uv(tile);
            assert!(uv[0] >= 0.0 && uv[2] <= 1.0 && uv[1] >= 0.0 && uv[3] <= 1.0);
        }
    }
}
