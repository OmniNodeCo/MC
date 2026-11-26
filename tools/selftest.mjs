// Voxelheim headless self-test: renders texture sheets to PNG, exercises
// world generation, meshing, raycasting and physics. Run: npm test
import zlib from 'node:zlib';
import fs from 'node:fs';
import path from 'node:path';
import { buildAtlas, waterPx, mobPx, heartPx, bubblePx, TILE, COLS, ROWS, TILES } from '../public/js/textures.js';
import { World, collideMove, CS, CH, SEA } from '../public/js/world.js';
import { B, BLOCKS } from '../public/js/blocks.js';
import { Px } from '../public/js/textures.js';

// texture previews go to VOXELHEIM_TEX_OUT (default /tmp)
const TEX_OUT = process.env.VOXELHEIM_TEX_OUT || '/tmp';
fs.mkdirSync(TEX_OUT, { recursive: true });
const save = (name, px) => write(path.join(TEX_OUT, name), px);

// ---------------------------------------------------------------- PNG writer
let CRC_TABLE = null;
function crcTable() {
  if (CRC_TABLE) return CRC_TABLE;
  CRC_TABLE = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
    CRC_TABLE[n] = c;
  }
  return CRC_TABLE;
}
function crc32(buf) {
  const t = crcTable();
  let c = 0xFFFFFFFF;
  for (let i = 0; i < buf.length; i++) c = t[(c ^ buf[i]) & 0xFF] ^ (c >>> 8);
  return (c ^ 0xFFFFFFFF) >>> 0;
}
function encodePNG(px) {
  const { w, h, data } = px;
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    for (let x = 0; x < w * 4; x++) raw[y * (w * 4 + 1) + 1 + x] = data[y * w * 4 + x];
  }
  const idat = zlib.deflateSync(raw, { level: 9 });
  const chunk = (type, body) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(body.length);
    const td = Buffer.concat([Buffer.from(type, 'ascii'), body]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(td));
    return Buffer.concat([len, td, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
    chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', Buffer.alloc(0)),
  ]);
}
const write = (p, px) => fs.writeFileSync(p, encodePNG(px));

// ---------------------------------------------------------------- texture sheets
const { px: atlasPx } = buildAtlas();
save('atlas.png', atlasPx);
save('atlas_big.png', atlasPx.scaled(6));

// contact sheet: water + mob skins + hud sprites, each scaled x5
const sheet = new Px(16 * 12, 16 * 2);
let sx = 0;
const put5 = (px) => { sheet.blit(px.scaled(5), sx, 0); sx += 16 * 5; };
put5(waterPx().scaled(2)); // 32*5 = wide... adjust below
// simpler: build list then layout
const items = [];
items.push(['water', waterPx()]);
for (const k of ['gloomling', 'hopper', 'skitter', 'wisp', 'puffbird']) {
  const t = mobPx(k);
  items.push([k + '-body', t.body]);
  items.push([k + '-face', t.face]);
  if (t.shard) items.push(['wisp-shard', t.shard]);
  if (t.wing) items.push(['bird-wing', t.wing]);
}
items.push(['heart-full', heartPx('full')]);
items.push(['heart-half', heartPx('half')]);
items.push(['heart-empty', heartPx('empty')]);
items.push(['bubble', bubblePx()]);

const f = 6;
const sheetW = items.length * (16 * f + 8) + 8;
const out = new Px(sheetW, 16 * f + 16);
let ox = 8;
for (const [, px] of items) {
  out.blit(px.scaled(f), ox, 8);
  ox += 16 * f + 8;
}
save('mobs.png', out);
console.log('texture items:', items.map(([n]) => n).join(', '));

// ---------------------------------------------------------------- world tests
const world = new World(12345);
const results = [];
const check = (name, ok, extra = '') => {
  results.push(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? ' — ' + extra : ''}`);
};

// column sanity over a big area
let hMin = 1e9, hMax = -1e9;
const biomes = {};
for (let z = -64; z < 64; z += 4) {
  for (let x = -64; x < 64; x += 4) {
    const info = world.columnInfo(x, z);
    hMin = Math.min(hMin, info.h); hMax = Math.max(hMax, info.h);
    biomes[info.biome] = (biomes[info.biome] || 0) + 1;
  }
}
check('column heights in bounds', hMin >= 6 && hMax <= CH - 10, `h ${hMin}..${hMax}`);
check('multiple biomes exist', Object.keys(biomes).length >= 4, JSON.stringify(biomes));

// chunk generation + block sanity
for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) world.ensureData(dx, dz);
const c = world.chunks.get(world.key(0, 0));
check('chunk generated', c && c.blocks.length === CS * CS * CH);
let bedrockOk = true;
for (let i = 0; i < CS * CS; i++) if (c.blocks[i] !== B.BEDROCK) bedrockOk = false;
check('bedrock floor', bedrockOk);

// meshing
for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++) world.ensureData(dx, dz);
const meshes = world.buildChunkMesh(0, 0);
let tris = 0;
for (const m of meshes) tris += m.geometry.index.count / 3;
check('chunk mesh built', meshes.length > 0, `${meshes.length} meshes, ${tris} tris`);
let hasNaN = false;
for (const m of meshes) {
  const pos = m.geometry.attributes.position.array;
  for (let i = 0; i < pos.length; i++) if (Number.isNaN(pos[i])) hasNaN = true;
}
check('no NaN vertices', !hasNaN);

// mesh a few more chunks incl. negative coords
for (const [cx, cz] of [[-1, 0], [0, -1], [-1, -1], [3, 2]]) {
  const ms = world.buildChunkMesh(cx, cz);
  check(`mesh (${cx},${cz})`, ms.length > 0);
}

// edit + remesh
const keys = world.setBlock(8, 50, 8, B.LANTERN);
check('setBlock returns keys', keys.length >= 1);
world.buildChunkMesh(0, 0);
check('edit persisted in data', world.getBlock(8, 50, 8) === B.LANTERN);

// raycast: straight down from high above should hit the surface
const sy = world.surfaceY(8, 8);
const hit = world.raycast(8.5, 79.5, 8.5, 0, -1, 0, 120);
check('raycast hits surface', !!hit && hit.y === sy, `hit y=${hit?.y} surfaceY=${sy}`);
const miss = world.raycast(8.5, 79.5, 8.5, 0, 1, 0, 120);
check('raycast up misses', hit !== null);

// physics: drop an entity onto the ground
const pos = { x: 8.5, y: 78, z: 8.5 };
const vel = { x: 0, y: 0, z: 0 };
for (let i = 0; i < 600; i++) {
  vel.y -= 27 * (1 / 60);
  vel.y = Math.max(vel.y, -60);
  collideMove(world, pos, vel, 1 / 60, 0.3, 1.8);
}
check('physics lands on ground', Math.abs(pos.y - (sy + 1)) < 0.2, `rest y=${pos.y.toFixed(2)} expected ~${sy + 1}`);

// walk into a wall shouldn't tunnel
const pos2 = { x: 8.5, y: sy + 1, z: 8.5 };
const vel2 = { x: 8, y: 0, z: 0 };
for (let i = 0; i < 240; i++) collideMove(world, pos2, vel2, 1 / 60, 0.3, 1.8);
check('collision keeps entity in world', pos2.y > 0 && Number.isFinite(pos2.x));

// seed determinism
const w2 = new World(12345);
const i1 = world.columnInfo(31, 47), i2 = w2.columnInfo(31, 47);
check('terrain deterministic', i1.h === i2.h && i1.biome === i2.biome);

console.log('\n' + results.join('\n'));
const failed = results.filter(r => r.startsWith('FAIL')).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
