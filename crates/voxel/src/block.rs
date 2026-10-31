//! Block registry. Ids are `u8`, so the whole registry stays cache friendly.

use crate::textures::tiles;

/// Block ids. `AIR` is always 0 / the implicit empty value.
pub mod id {
    pub const AIR: u8 = 0;
    pub const STONE: u8 = 1;
    pub const GRASS: u8 = 2;
    pub const DIRT: u8 = 3;
    pub const COBBLESTONE: u8 = 4;
    pub const PLANKS: u8 = 5;
    pub const BEDROCK: u8 = 6;
    pub const WATER: u8 = 7;
    pub const SAND: u8 = 8;
    pub const GRAVEL: u8 = 9;
    pub const OAK_LOG: u8 = 10;
    pub const OAK_LEAVES: u8 = 11;
    pub const GLASS: u8 = 12;
    pub const COAL_ORE: u8 = 13;
    pub const IRON_ORE: u8 = 14;
    pub const GOLD_ORE: u8 = 15;
    pub const DIAMOND_ORE: u8 = 16;
    pub const SNOW: u8 = 17;
    pub const CACTUS: u8 = 18;
    pub const TALL_GRASS: u8 = 19;
    pub const POPPY: u8 = 20;
    pub const DANDELION: u8 = 21;
    pub const SANDSTONE: u8 = 22;
    pub const ICE: u8 = 23;
    pub const BRICKS: u8 = 24;
    pub const CRAFTING_TABLE: u8 = 25;
    pub const BOOKSHELF: u8 = 26;
    pub const MOSSY_COBBLESTONE: u8 = 27;
    pub const STONE_BRICKS: u8 = 28;
    pub const BIRCH_LOG: u8 = 29;
    pub const BIRCH_LEAVES: u8 = 30;
    pub const GLOWSTONE: u8 = 31;
}

/// The six faces of a cube, in the order used by the mesher.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Top,
        Face::Bottom,
        Face::North,
        Face::South,
        Face::East,
        Face::West,
    ];

    /// Outward facing normal.
    pub fn normal(self) -> [i32; 3] {
        match self {
            Face::Top => [0, 1, 0],
            Face::Bottom => [0, -1, 0],
            Face::North => [0, 0, -1],
            Face::South => [0, 0, 1],
            Face::East => [1, 0, 0],
            Face::West => [-1, 0, 0],
        }
    }

    /// Baked directional shading (Minecraft style: top is fully lit).
    pub fn shade(self) -> f32 {
        match self {
            Face::Top => 1.0,
            Face::Bottom => 0.5,
            Face::North | Face::South => 0.8,
            Face::East | Face::West => 0.62,
        }
    }
}

/// Static description of a block type.
#[derive(Clone, Copy, Debug)]
pub struct BlockDef {
    pub name: &'static str,
    /// Tile index in the texture atlas.
    pub top: u32,
    pub bottom: u32,
    pub side: u32,
    /// Player and mobs collide with it.
    pub solid: bool,
    /// Hides the neighbouring face and does not let sky through.
    pub opaque: bool,
    /// Rendered in the alpha-tested pass (leaves, glass, plants).
    pub cutout: bool,
    /// Rendered as a cross of two quads.
    pub plant: bool,
    /// Flowing / swimmable.
    pub liquid: bool,
    /// Seconds to break bare handed.
    pub hardness: f32,
    /// Slightly glowing (used by the shader).
    pub emissive: bool,
}

impl BlockDef {
    const fn new(name: &'static str, tile: u32, hardness: f32) -> Self {
        Self {
            name,
            top: tile,
            bottom: tile,
            side: tile,
            solid: true,
            opaque: true,
            cutout: false,
            plant: false,
            liquid: false,
            hardness,
            emissive: false,
        }
    }

    const fn column(name: &'static str, top: u32, bottom: u32, side: u32, hardness: f32) -> Self {
        Self {
            top,
            bottom,
            side,
            ..Self::new(name, top, hardness)
        }
    }

    const fn cutout(mut self) -> Self {
        self.cutout = true;
        self.opaque = false;
        self
    }

    const fn plant(mut self) -> Self {
        self.plant = true;
        self.cutout = true;
        self.opaque = false;
        self.solid = false;
        self
    }

    const fn liquid(mut self) -> Self {
        self.liquid = true;
        self.opaque = false;
        self.solid = false;
        self
    }

    const fn air(name: &'static str) -> Self {
        Self {
            name,
            top: 0,
            bottom: 0,
            side: 0,
            solid: false,
            opaque: false,
            cutout: false,
            plant: false,
            liquid: false,
            hardness: 0.0,
            emissive: false,
        }
    }

    pub fn tile_for(&self, face: Face) -> u32 {
        match face {
            Face::Top => self.top,
            Face::Bottom => self.bottom,
            _ => self.side,
        }
    }
}

use id::*;

/// Indexed by block id.
pub static BLOCKS: [BlockDef; 32] = [
    BlockDef::air("Air"),
    BlockDef::new("Stone", tiles::STONE, 1.5),
    BlockDef::column("Grass Block", tiles::GRASS_TOP, tiles::DIRT, tiles::GRASS_SIDE, 0.6),
    BlockDef::new("Dirt", tiles::DIRT, 0.5),
    BlockDef::new("Cobblestone", tiles::COBBLESTONE, 2.0),
    BlockDef::new("Oak Planks", tiles::PLANKS, 2.0),
    BlockDef::new("Bedrock", tiles::BEDROCK, 3_600_000.0),
    BlockDef::new("Water", tiles::WATER, 0.0).liquid(),
    BlockDef::new("Sand", tiles::SAND, 0.5),
    BlockDef::new("Gravel", tiles::GRAVEL, 0.6),
    BlockDef::column("Oak Log", tiles::LOG_TOP, tiles::LOG_TOP, tiles::LOG_SIDE, 2.0),
    BlockDef::new("Oak Leaves", tiles::LEAVES, 0.2).cutout(),
    BlockDef::new("Glass", tiles::GLASS, 0.3).cutout(),
    BlockDef::new("Coal Ore", tiles::COAL_ORE, 3.0),
    BlockDef::new("Iron Ore", tiles::IRON_ORE, 3.0),
    BlockDef::new("Gold Ore", tiles::GOLD_ORE, 3.0),
    BlockDef::new("Diamond Ore", tiles::DIAMOND_ORE, 3.5),
    BlockDef::new("Snow Block", tiles::SNOW, 0.3),
    BlockDef::column("Cactus", tiles::CACTUS_TOP, tiles::CACTUS_TOP, tiles::CACTUS_SIDE, 0.4),
    BlockDef::new("Tall Grass", tiles::TALL_GRASS, 0.0).plant(),
    BlockDef::new("Poppy", tiles::POPPY, 0.0).plant(),
    BlockDef::new("Dandelion", tiles::DANDELION, 0.0).plant(),
    BlockDef::new("Sandstone", tiles::SANDSTONE, 0.8),
    BlockDef::new("Ice", tiles::ICE, 0.5).cutout(),
    BlockDef::new("Bricks", tiles::BRICKS, 2.0),
    BlockDef::column(
        "Crafting Table",
        tiles::CRAFTING_TOP,
        tiles::PLANKS,
        tiles::CRAFTING_SIDE,
        2.5,
    ),
    BlockDef::new("Bookshelf", tiles::BOOKSHELF, 1.5),
    BlockDef::new("Mossy Cobblestone", tiles::MOSSY_COBBLESTONE, 2.0),
    BlockDef::new("Stone Bricks", tiles::STONE_BRICKS, 1.5),
    BlockDef::column(
        "Birch Log",
        tiles::BIRCH_TOP,
        tiles::BIRCH_TOP,
        tiles::BIRCH_SIDE,
        2.0,
    ),
    BlockDef::new("Birch Leaves", tiles::BIRCH_LEAVES, 0.2).cutout(),
    BlockDef::new("Glowstone", tiles::GLOWSTONE, 0.3).cutout(),
];

#[inline]
pub fn def(id: u8) -> &'static BlockDef {
    match BLOCKS.get(id as usize) {
        Some(d) => d,
        None => &BLOCKS[0],
    }
}

#[inline]
pub fn is_air(id: u8) -> bool {
    id == AIR
}

#[inline]
pub fn is_solid(id: u8) -> bool {
    def(id).solid
}

#[inline]
pub fn is_opaque(id: u8) -> bool {
    def(id).opaque
}

#[inline]
pub fn is_plant(id: u8) -> bool {
    def(id).plant
}

#[inline]
pub fn is_liquid(id: u8) -> bool {
    def(id).liquid
}

#[inline]
pub fn is_cutout(id: u8) -> bool {
    def(id).cutout
}

#[inline]
pub fn name(id: u8) -> &'static str {
    def(id).name
}

#[inline]
pub fn hardness(id: u8) -> f32 {
    def(id).hardness
}

/// Blocks the player can build with (shown in the creative inventory).
pub const INVENTORY_BLOCKS: [u8; 28] = [
    GRASS, DIRT, STONE, COBBLESTONE, MOSSY_COBBLESTONE, STONE_BRICKS, BRICKS, SAND, SANDSTONE,
    GRAVEL, SNOW, ICE, OAK_LOG, BIRCH_LOG, PLANKS, OAK_LEAVES, BIRCH_LEAVES, GLASS, BOOKSHELF,
    CRAFTING_TABLE, GLOWSTONE, COAL_ORE, IRON_ORE, GOLD_ORE, DIAMOND_ORE, WATER, CACTUS, TALL_GRASS,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_has_a_def() {
        for i in 0..BLOCKS.len() as u8 {
            assert!(!name(i).is_empty());
        }
    }

    #[test]
    fn inventory_only_lists_real_blocks() {
        for b in INVENTORY_BLOCKS {
            assert!(b != AIR && (b as usize) < BLOCKS.len());
        }
    }

    #[test]
    fn water_and_plants_are_not_solid() {
        assert!(!is_solid(WATER));
        assert!(!is_solid(TALL_GRASS));
        assert!(is_solid(STONE));
    }
}
