// Voxelheim headless integration sim: drives the real game modules
// (world, player, entities, sky, particles) through a simulated session.
import { createCanvas, ImageData as NapiImageData } from '@napi-rs/canvas';

// ---- minimal DOM shim so texture code can build canvases ----
globalThis.document = {
  createElement(tag) {
    if (tag === 'canvas') return createCanvas(2, 2);
    return {};
  },
};
globalThis.ImageData = NapiImageData;

import * as THREE from '../public/vendor/three.module.js';
import { World, CS, CH, SEA } from '../public/js/world.js';
import { B, BLOCKS } from '../public/js/blocks.js';
import { Player } from '../public/js/player.js';
import { Sky } from '../public/js/sky.js';
import { Particles } from '../public/js/particles.js';
import { Entities, MOBS } from '../public/js/entities.js';

const results = [];
const check = (name, ok, extra = '') => {
  results.push(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? ' — ' + extra : ''}`);
};

const sfxStub = new Proxy({}, { get: () => () => {} });

// ---------------------------------------------------------------- setup
const world = new World(777);
world.mats = { opaque: {}, cutout: {}, glow: {}, water: {} };

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(74, 1.7, 0.1, 900);
camera.rotation.order = 'YXZ';
const sky = new Sky(scene);
const particles = new Particles(scene);
const player = new Player(world, camera);
const entities = new Entities(scene, world, particles, sfxStub);

// spawn search (mirrors main.js)
let sx = 8, sz = 8;
for (let i = 0; i < 120; i++) {
  const tx = 8 + i * 9;
  const info = world.columnInfo(tx, 8);
  if (info.biome !== 'ocean' && info.h > SEA) { sx = tx; sz = 8; break; }
}
player.spawn.set(sx + 0.5, world.surfaceY(sx, sz) + 1.01, sz + 0.5);
player.pos.copy(player.spawn);
player.onDamage = () => {};
player.onDeath = () => {};

// pregen
const pcx = Math.floor(player.pos.x / CS), pcz = Math.floor(player.pos.z / CS);
for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++) world.ensureData(pcx + dx, pcz + dz);
for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) world.buildChunkMesh(pcx + dx, pcz + dz);

const t0 = world.surfaceY(Math.floor(player.pos.x), Math.floor(player.pos.z)) + 1;
check('player spawns on land', world.biomeAt(Math.floor(player.pos.x), Math.floor(player.pos.z)) !== 'ocean' &&
  Math.abs(player.pos.y - t0) < 0.01, `y=${player.pos.y.toFixed(1)}`);

// ---------------------------------------------------------------- walk sim
player.onKey('KeyW', true, 0);
let meshCount = 0, simT = 0;
const DT = 1 / 60;
for (let f = 0; f < 1800; f++) {           // 30 simulated seconds
  simT += DT;
  player.update(DT);
  if (f % 30 === 0) {                      // stream a bit
    const cx = Math.floor(player.pos.x / CS), cz = Math.floor(player.pos.z / CS);
    outer:
    for (let r = 0; r <= 4; r++) {
      for (let dz = -r; dz <= r; dz++) for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dz)) !== r) continue;
        const c = world.chunks.get(world.key(cx + dx, cz + dz));
        if (!c || c.dirty) { world.buildChunkMesh(cx + dx, cz + dz); meshCount++; break outer; }
      }
    }
  }
  sky.update(DT, player.pos, player.headInWater, 84);
  entities.update(DT, player, sky);
  particles.update(DT);
  camera.rotation.y = player.yaw;
  camera.rotation.x = player.pitch;
  camera.position.set(player.pos.x, player.pos.y + 1.62, player.pos.z);
  camera.quaternion.setFromEuler(camera.rotation);
}
check('walked 30s without NaN', Number.isFinite(player.pos.x) && Number.isFinite(player.pos.y) && Number.isFinite(player.pos.z));
check('player actually moved', Math.hypot(player.pos.x - (sx + 0.5), player.pos.z - 8.5) > 8,
  `moved ${Math.hypot(player.pos.x - (sx + 0.5), player.pos.z - 8.5).toFixed(1)} blocks`);
check('chunks meshed while streaming', meshCount > 10, `${meshCount} meshes`);
check('no NaN in meshed geometry', (() => {
  for (const [, c] of world.chunks) {
    if (!c.meshes) continue;
    for (const m of c.meshes) {
      const a = m.geometry.attributes.position.array;
      for (let i = 0; i < a.length; i++) if (Number.isNaN(a[i])) return false;
    }
  }
  return true;
})());
// coastal starts may have no land for natural spawns nearby — seed wildlife on
// nearby land (mirrors main.js's initial wildlife, which always picks a land spot)
if (entities.count('hopper') + entities.count('puffbird') === 0) {
  const px0 = Math.floor(player.pos.x), pz0 = Math.floor(player.pos.z);
  let spot = null;
  for (let r = 2; r < 48 && !spot; r += 2) {
    for (let a = 0; a < 12 && !spot; a++) {
      const x = px0 + Math.round(Math.cos((a / 12) * Math.PI * 2) * r);
      const z = pz0 + Math.round(Math.sin((a / 12) * Math.PI * 2) * r);
      const info = world.columnInfo(x, z);
      if (info.biome !== 'ocean' && info.h > SEA) spot = { x, z };
    }
  }
  if (spot) {
    entities.spawn('hopper', spot.x + 0.5, world.surfaceY(spot.x, spot.z) + 1.5, spot.z + 0.5);
    entities.spawn('puffbird', spot.x + 0.5, world.surfaceY(spot.x, spot.z) + 8, spot.z + 0.5);
  }
}
check('wildlife present', entities.count('hopper') + entities.count('puffbird') > 0,
  `${entities.list.map(m => m.kind).join(', ') || 'none'}`);

// ---------------------------------------------------------------- mining & placing
const dir = new THREE.Vector3(0, -1, 0);
const hit = world.raycast(player.pos.x, player.pos.y + 1.62, player.pos.z, dir.x, dir.y, dir.z, 6);
check('can target block underfoot', !!hit);
if (hit) {
  const id = hit.id;
  const keys = world.setBlock(hit.x, hit.y, hit.z, B.AIR);
  for (const k of keys) { const c = world.chunks.get(k); if (c && c.meshes) world.buildChunkMesh(c.cx, c.cz); }
  check('mined block removed', world.getBlock(hit.x, hit.y, hit.z) === B.AIR);
  // place a lantern where we mined
  const keys2 = world.setBlock(hit.x, hit.y, hit.z, B.LANTERN);
  let before = 0, after = 0;
  for (const k of keys2) {
    const c = world.chunks.get(k);
    if (c) {
      before = (c.meshes || []).length;
      const ms = world.buildChunkMesh(c.cx, c.cz);
      after = ms.length;
    }
  }
  check('placed lantern adds glow bucket', after === before + 1, `${before}→${after} buckets`);
  world.setBlock(hit.x, hit.y, hit.z, B.AIR);
}

// ---------------------------------------------------------------- night: gloomlings
sky.time = 0.62; // deep night
sky.update(0.01, player.pos, false, 84);
check('night detected', sky.nightFactor > 0.8, `night=${sky.nightFactor.toFixed(2)}`);
let spawnedNaturally = false;
for (let f = 0; f < 60 * 25 && !spawnedNaturally; f++) {
  player.update(DT);
  sky.update(DT, player.pos, false, 84);
  entities.update(DT, player, sky);
  if (entities.count('gloomling') > 0) spawnedNaturally = true;
}
check('gloomlings spawn at night', spawnedNaturally,
  entities.list.map(m => m.kind).join(', ') || 'none');

// forced close encounter: revive the player (night wildlife may have killed the AFK player),
// then drop a gloomling 5 blocks away
player.dead = false;
player.hp = 20;
player.invuln = 0;
const g = entities.spawn('gloomling', player.pos.x + 5, player.pos.y + 2, player.pos.z);
let sawChase = false, gotHit = false;
const hpBefore = player.hp;
for (let f = 0; f < 60 * 14; f++) {
  player.update(DT);
  sky.update(DT, player.pos, false, 84);
  entities.update(DT, player, sky);
  camera.position.set(player.pos.x, player.pos.y + 1.62, player.pos.z);
  camera.quaternion.setFromEuler(camera.rotation);
  if (g.pos.distanceTo(player.pos) < 3) sawChase = true;
  if (player.hp < hpBefore) gotHit = true;
  if (gotHit && sawChase) break;
}
check('gloomling chases player', sawChase || gotHit, `dist=${g.pos.distanceTo(player.pos).toFixed(1)}`);
check('gloomling damages player', gotHit || player.hp < hpBefore, `hp ${hpBefore}→${player.hp}`);

// player fights back: spawn a mob right next to the player and aim at it (deterministic)
const target = entities.spawn('hopper', player.pos.x + 3, player.pos.y + 0.5, player.pos.z);
{
  camera.lookAt(target.pos.x, target.pos.y + 0.3, target.pos.z);
  const picked = entities.raypick(camera.position, camera.getWorldDirection(new THREE.Vector3()), 10);
  check('raypick returns a mob', !!picked);
  if (picked) {
    const mhp = picked.mob.hp;
    picked.mob.damage(3, null, particles, sfxStub);
    check('mob takes damage', picked.mob.hp < mhp);
    picked.mob.damage(99, null, particles, sfxStub);
    check('mob dies and is removed', (() => {
      for (let f = 0; f < 60; f++) entities.update(DT, player, sky);
      return !entities.list.includes(picked.mob);
    })());
  }
}

// ---------------------------------------------------------------- daylight burn
sky.time = 0.25; // noon
sky.update(0.01, player.pos, false, 84);
const g2 = entities.spawn('gloomling', player.pos.x + 2, player.pos.y + 1, player.pos.z);
// make sure it's sky-exposed: clear a column above it if needed
const gx = Math.floor(g2.pos.x), gz = Math.floor(g2.pos.z);
for (let y = Math.floor(g2.pos.y) + 2; y < CH; y++) {
  if (BLOCKS[world.getBlock(gx, y, gz)].opaque) world.setBlock(gx, y, gz, B.AIR);
}
let burned = false;
for (let f = 0; f < 60 * 6; f++) {
  entities.update(DT, player, sky);
  if (g2.dying > 0 || !entities.list.includes(g2)) { burned = true; break; }
}
check('gloomling burns in sunlight', burned);

// ---------------------------------------------------------------- death & respawn
player.damage(100, null);
check('player death fires', player.dead);
player.respawn();
check('respawn restores hp/position', !player.dead && player.hp === 20 &&
  Math.abs(player.pos.y - (world.surfaceY(Math.floor(player.pos.x), Math.floor(player.pos.z)) + 1)) < 2.5);

// ---------------------------------------------------------------- day cycle sanity
let minB = 1, maxB = 0;
for (let t = 0; t < 1; t += 1 / 96) {
  sky.time = t;
  sky.update(0.001, player.pos, false, 84);
  const b = sky.worldBrightness();
  minB = Math.min(minB, b); maxB = Math.max(maxB, b);
}
check('day/night brightness cycles', minB < 0.45 && maxB > 0.95, `${minB.toFixed(2)}..${maxB.toFixed(2)}`);
check('underwater fog switches', (() => {
  sky.time = 0.25; sky.update(0.001, player.pos, true, 84);
  return sky.scene.fog.near < 5 && sky.scene.fog.far < 30;
})());

console.log('\n' + results.join('\n'));
const failed = results.filter(r => r.startsWith('FAIL')).length;
console.log(`\n${results.length - failed}/${results.length} checks passed`);
process.exit(failed ? 1 : 0);
