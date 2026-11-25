// Voxelheim — block registry. Ids, per-face tiles, physics/render flags.
import { TILES } from './textures.js';

export const B = {
  AIR: 0, GRASS: 1, DIRT: 2, STONE: 3, COBBLE: 4, SAND: 5, LOG: 6, LEAVES: 7,
  PLANKS: 8, WATER: 9, SNOWGRASS: 10, COAL_ORE: 11, IRON_ORE: 12, GOLD_ORE: 13,
  CRYSTAL_ORE: 14, CRYSTAL: 15, LANTERN: 16, GLASS: 17, BRICK: 18, BEDROCK: 19,
  TALLGRASS: 20, FLOWER_RED: 21, FLOWER_YELLOW: 22, CACTUS: 23, SNOW: 24,
};

const T = TILES;

function def(name, top, bottom, side, o = {}) {
  return { name, top, bottom, side, solid: 0, opaque: 0, cross: 0, liquid: 0, glow: 0, cutout: 0, hard: 1, ...o };
}

export const BLOCKS = [
  def('Air', 0, 0, 0),
  def('Grass', T.GRASS_TOP, T.DIRT, T.GRASS_SIDE, { solid: 1, opaque: 1, hard: 0.55 }),
  def('Dirt', T.DIRT, T.DIRT, T.DIRT, { solid: 1, opaque: 1, hard: 0.5 }),
  def('Stone', T.STONE, T.STONE, T.STONE, { solid: 1, opaque: 1, hard: 1.4 }),
  def('Cobblestone', T.COBBLE, T.COBBLE, T.COBBLE, { solid: 1, opaque: 1, hard: 1.5 }),
  def('Sand', T.SAND, T.SAND, T.SAND, { solid: 1, opaque: 1, hard: 0.45 }),
  def('Oak Log', T.LOG_TOP, T.LOG_TOP, T.LOG_SIDE, { solid: 1, opaque: 1, hard: 0.9 }),
  def('Leaves', T.LEAVES, T.LEAVES, T.LEAVES, { solid: 1, opaque: 1, hard: 0.25 }),
  def('Planks', T.PLANKS, T.PLANKS, T.PLANKS, { solid: 1, opaque: 1, hard: 0.9 }),
  def('Water', T.SAND, T.SAND, T.SAND, { liquid: 1, hard: Infinity }),
  def('Snowy Grass', T.SNOW, T.DIRT, T.SNOW_SIDE, { solid: 1, opaque: 1, hard: 0.55 }),
  def('Coal Ore', T.COAL_ORE, T.COAL_ORE, T.COAL_ORE, { solid: 1, opaque: 1, hard: 1.7 }),
  def('Iron Ore', T.IRON_ORE, T.IRON_ORE, T.IRON_ORE, { solid: 1, opaque: 1, hard: 1.8 }),
  def('Gold Ore', T.GOLD_ORE, T.GOLD_ORE, T.GOLD_ORE, { solid: 1, opaque: 1, hard: 1.8 }),
  def('Crystal Ore', T.CRYSTAL_ORE, T.CRYSTAL_ORE, T.CRYSTAL_ORE, { solid: 1, opaque: 1, glow: 1, hard: 1.8 }),
  def('Crystal Block', T.CRYSTAL, T.CRYSTAL, T.CRYSTAL, { solid: 1, opaque: 1, glow: 1, hard: 1.4 }),
  def('Lantern', T.LANTERN, T.LANTERN, T.LANTERN, { solid: 1, opaque: 1, glow: 1, hard: 0.5 }),
  def('Glass', T.GLASS, T.GLASS, T.GLASS, { solid: 1, cutout: 1, hard: 0.35 }),
  def('Brick', T.BRICK, T.BRICK, T.BRICK, { solid: 1, opaque: 1, hard: 1.5 }),
  def('Bedrock', T.BEDROCK, T.BEDROCK, T.BEDROCK, { solid: 1, opaque: 1, hard: Infinity }),
  def('Wild Grass', T.TALLGRASS, T.TALLGRASS, T.TALLGRASS, { cross: 1, cutout: 1, hard: 0.05 }),
  def('Ember Bloom', T.FLOWER_RED, T.FLOWER_RED, T.FLOWER_RED, { cross: 1, cutout: 1, hard: 0.05 }),
  def('Sunpetal', T.FLOWER_YELLOW, T.FLOWER_YELLOW, T.FLOWER_YELLOW, { cross: 1, cutout: 1, hard: 0.05 }),
  def('Cactus', T.CACTUS_TOP, T.CACTUS_TOP, T.CACTUS_SIDE, { solid: 1, opaque: 1, hard: 0.5 }),
  def('Snow Block', T.SNOW, T.SNOW, T.SNOW, { solid: 1, opaque: 1, hard: 0.4 }),
];

export const tileFor = (id, face) => {
  const b = BLOCKS[id];
  return face === 'top' ? b.top : face === 'bottom' ? b.bottom : b.side;
};

export const PLACEABLE = [
  B.GRASS, B.DIRT, B.STONE, B.COBBLE, B.SAND, B.LOG, B.LEAVES, B.PLANKS,
  B.WATER, B.SNOWGRASS, B.COAL_ORE, B.IRON_ORE, B.GOLD_ORE, B.CRYSTAL_ORE,
  B.CRYSTAL, B.LANTERN, B.GLASS, B.BRICK, B.TALLGRASS, B.FLOWER_RED,
  B.FLOWER_YELLOW, B.CACTUS, B.SNOW,
];
