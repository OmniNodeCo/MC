// Voxelheim — main: boot, input, interactions, chunk streaming, save/load, loop.
import * as THREE from '../vendor/three.module.js';
import { buildAtlas, waterPx, tileUV } from './textures.js';
import { B, BLOCKS, tileFor, PLACEABLE } from './blocks.js';
import { World, collideMove, CS, CH, SEA } from './world.js';
import { Player } from './player.js';
import { Sky } from './sky.js';
import { Particles } from './particles.js';
import { Entities } from './entities.js';
import { HUD } from './hud.js';
import { Sfx } from './audio.js';
import { clamp } from './noise.js';

const $ = (id) => document.getElementById(id);
const canvas = $('game');

// ---------------------------------------------------------------- save
const SAVE_KEY = 'voxelheim_save_v1';
const loadSave = () => {
  try { const s = localStorage.getItem(SAVE_KEY); return s ? JSON.parse(s) : null; } catch { return null; }
};
const saveGame = () => {
  try {
    const edits = {};
    for (const [k, m] of world.edits) edits[k] = [...m];
    localStorage.setItem(SAVE_KEY, JSON.stringify({
      seed: world.seed,
      time: sky.time,
      hotbar, sel,
      hp: player.hp,
      player: { x: player.pos.x, y: player.pos.y, z: player.pos.z, yaw: player.yaw, pitch: player.pitch, flying: player.flying },
      spawn: { x: player.spawn.x, y: player.spawn.y, z: player.spawn.z },
      edits,
    }));
  } catch { /* storage unavailable */ }
};

const saved = loadSave();
const seed = saved?.seed ?? ((Math.random() * 0x7fffffff) | 0);

// ---------------------------------------------------------------- textures & materials
const { px: atlasPx, avg } = buildAtlas();
const atlasCanvas = atlasPx.toCanvas();

function makeAtlasTexture() {
  const t = new THREE.CanvasTexture(atlasCanvas);
  t.magFilter = THREE.NearestFilter;
  t.minFilter = THREE.NearestFilter;
  t.generateMipmaps = false;
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}
const atlasTex = makeAtlasTexture();

const matOpaque = new THREE.MeshBasicMaterial({ map: atlasTex, vertexColors: true });
const matCutout = new THREE.MeshBasicMaterial({ map: atlasTex, vertexColors: true, alphaTest: 0.45, side: THREE.DoubleSide });
const matGlow = new THREE.MeshBasicMaterial({ map: atlasTex, vertexColors: true });
const waterTexCanvas = waterPx().toCanvas();
const waterTex = new THREE.CanvasTexture(waterTexCanvas);
waterTex.magFilter = THREE.NearestFilter;
waterTex.minFilter = THREE.NearestFilter;
waterTex.generateMipmaps = false;
waterTex.wrapS = waterTex.wrapT = THREE.RepeatWrapping;
waterTex.colorSpace = THREE.SRGBColorSpace;
const matWater = new THREE.MeshBasicMaterial({
  map: waterTex, vertexColors: true, transparent: true, opacity: 0.72,
  side: THREE.DoubleSide, depthWrite: false,
});

// favicon from the grass tile
try {
  const fav = document.createElement('canvas');
  fav.width = 16; fav.height = 16;
  const fc = fav.getContext('2d');
  fc.imageSmoothingEnabled = false;
  fc.drawImage(atlasCanvas, 0, 0, 16, 16, 0, 0, 16, 16);
  const link = document.createElement('link');
  link.rel = 'icon';
  link.href = fav.toDataURL();
  document.head.appendChild(link);
} catch { /* noop */ }

// ---------------------------------------------------------------- renderer / scene
let renderer;
try {
  renderer = new THREE.WebGLRenderer({ canvas, antialias: false });
} catch (err) {
  $('overlay-content').innerHTML = `<h1 style="font-size:26px">VOXELHEIM</h1>
    <div class="tag">WEBGL UNAVAILABLE</div>
    <div class="controls">This game needs WebGL. Try a different browser or enable hardware acceleration.</div>`;
  throw err;
}
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
renderer.setSize(window.innerWidth, window.innerHeight);

const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(74, window.innerWidth / window.innerHeight, 0.1, 900);
scene.add(camera);

window.addEventListener('resize', () => {
  renderer.setSize(window.innerWidth, window.innerHeight);
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
});

// ---------------------------------------------------------------- world
const world = new World(seed);
world.mats = { opaque: matOpaque, cutout: matCutout, glow: matGlow, water: matWater };
scene.add(world.group);

if (saved?.edits) {
  for (const k in saved.edits) world.edits.set(k, new Map(saved.edits[k]));
}

// find spawn (land, not ocean)
let sx = 8, sz = 8;
for (let i = 0; i < 80; i++) {
  const tx = 8 + i * 12;
  const info = world.columnInfo(tx, 8);
  if (info.biome !== 'ocean' && info.h > SEA) { sx = tx; sz = 8; break; }
}

// ---------------------------------------------------------------- systems
const sky = new Sky(scene);
const particles = new Particles(scene);
const sfx = new Sfx();
const hud = new HUD(atlasCanvas, tileFor);
const player = new Player(world, camera);
const entities = new Entities(scene, world, particles, sfx);

player.spawn.set(sx + 0.5, 0, sz + 0.5);
if (saved?.player) {
  player.pos.set(saved.player.x, saved.player.y, saved.player.z);
  player.yaw = saved.player.yaw ?? player.yaw;
  player.pitch = saved.player.pitch ?? player.pitch;
  player.flying = !!saved.player.flying;
  player.hp = saved.hp ?? 20;
  player.spawn.set(saved.spawn?.x ?? sx + 0.5, saved.spawn?.y ?? 0, saved.spawn?.z ?? sz + 0.5);
} else {
  player.spawn.y = world.surfaceY(sx, sz) + 1.01;
  player.pos.copy(player.spawn);
}
if (saved?.time !== undefined) sky.time = saved.time;

// pre-generate area around spawn so the ground exists on frame one
{
  const scx = Math.floor(player.pos.x / CS), scz = Math.floor(player.pos.z / CS);
  for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++) world.ensureData(scx + dx, scz + dz);
  for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) world.buildChunkMesh(scx + dx, scz + dz);
}
if (!saved) player.pos.y = world.surfaceY(sx, sz) + 1.01;

// initial friendly wildlife
for (let i = 0; i < 2; i++) {
  const ax = sx + 4 + i * 5, az = sz - 6 - i * 3;
  const sy = world.surfaceY(Math.floor(ax), Math.floor(az));
  if (sy > SEA) entities.spawn('hopper', ax, sy + 1, az);
}
for (let i = 0; i < 2; i++) {
  entities.spawn('puffbird', sx + (i ? -8 : 6), world.surfaceY(sx + (i ? -8 : 6), sz + (i ? 7 : -5)) + 8, sz + (i ? 7 : -5));
}

// ---------------------------------------------------------------- hotbar
let hotbar = saved?.hotbar ?? [B.GRASS, B.DIRT, B.STONE, B.PLANKS, B.LOG, B.GLASS, B.LANTERN, B.CRYSTAL, B.BRICK];
let sel = saved?.sel ?? 0;
hud.buildHotbar(hotbar, (i) => { sel = i; hud.select(sel); rebuildHeld(); });
hud.select(sel);
hud.updateHearts(player.hp);

// ---------------------------------------------------------------- held block
const heldMat = new THREE.MeshBasicMaterial({ map: atlasTex });
let heldMesh = null;

function remapFaceUV(geo, faceIndex, tile) {
  const t = tileUV(tile);
  const uv = geo.attributes.uv;
  const o = faceIndex * 4;
  uv.setXY(o + 0, t[0], t[3]);
  uv.setXY(o + 1, t[2], t[3]);
  uv.setXY(o + 2, t[0], t[1]);
  uv.setXY(o + 3, t[2], t[1]);
  uv.needsUpdate = true;
}

function rebuildHeld() {
  if (heldMesh) {
    camera.remove(heldMesh);
    heldMesh.geometry.dispose();
    heldMesh = null;
  }
  const id = hotbar[sel];
  const def = BLOCKS[id];
  let geo;
  if (def.cross) {
    geo = new THREE.PlaneGeometry(0.42, 0.42);
    remapFaceUV(geo, 0, def.side);
  } else {
    geo = new THREE.BoxGeometry(0.4, 0.4, 0.4);
    remapFaceUV(geo, 0, def.side);
    remapFaceUV(geo, 1, def.side);
    remapFaceUV(geo, 2, def.top);
    remapFaceUV(geo, 3, def.bottom);
    remapFaceUV(geo, 4, def.side);
    remapFaceUV(geo, 5, def.side);
  }
  heldMesh = new THREE.Mesh(geo, heldMat);
  heldMesh.rotation.set(0.12, -0.55, 0.04);
  camera.add(heldMesh);
}
rebuildHeld();

// ---------------------------------------------------------------- chunk streaming
const VIEW = 5;
const OFFSETS = [];
for (let dz = -(VIEW + 1); dz <= VIEW + 1; dz++) {
  for (let dx = -(VIEW + 1); dx <= VIEW + 1; dx++) {
    if (Math.max(Math.abs(dx), Math.abs(dz)) <= VIEW + 1) OFFSETS.push([dx, dz]);
  }
}
OFFSETS.sort((a, b) => (a[0] * a[0] + a[1] * a[1]) - (b[0] * b[0] + b[1] * b[1]));

let frame = 0;
function streamChunks() {
  const pcx = Math.floor(player.pos.x / CS), pcz = Math.floor(player.pos.z / CS);
  let gen = 2, mesh = 2;
  for (const [dx, dz] of OFFSETS) {
    if (gen <= 0 && mesh <= 0) break;
    const cx = pcx + dx, cz = pcz + dz;
    const k = world.key(cx, cz);
    const c = world.chunks.get(k);
    if (!c) {
      if (gen > 0) { world.ensureData(cx, cz); gen--; }
      continue;
    }
    if (Math.max(Math.abs(dx), Math.abs(dz)) <= VIEW && c.dirty && mesh > 0) {
      world.buildChunkMesh(cx, cz);
      mesh--;
    }
  }
  if ((frame++ % 180) === 0) {
    for (const [k, c] of world.chunks) {
      if (!c.meshes) continue;
      if (Math.max(Math.abs(c.cx - pcx), Math.abs(c.cz - pcz)) > VIEW + 1) world.disposeChunkMeshes(c);
    }
  }
}

function rebuildNow(keys) {
  for (const k of keys) {
    const c = world.chunks.get(k);
    if (!c) continue;
    if (c.meshes) world.buildChunkMesh(c.cx, c.cz);
    else c.dirty = true;
  }
}

// ---------------------------------------------------------------- interactions
const mouse = { left: false, right: false };
let mode = 'start';
let fallbackLook = false;
let mineTarget = null, mineT = 0, digSnd = 0;
let placeCd = 0, attackCd = 0, swing = 0;
let debugOn = false;
let wasInWater = false;

const _dir = new THREE.Vector3();
function camDir() {
  return _dir.set(0, 0, -1).applyQuaternion(camera.quaternion).clone();
}

function breakBlock(hit) {
  const def = BLOCKS[hit.id];
  const keys = world.setBlock(hit.x, hit.y, hit.z, B.AIR);
  const aboveId = world.getBlock(hit.x, hit.y + 1, hit.z);
  if (BLOCKS[aboveId].cross) {
    for (const k of world.setBlock(hit.x, hit.y + 1, hit.z, B.AIR)) keys.push(k);
  }
  rebuildNow([...new Set(keys)]);
  particles.burst(hit.x + 0.5, hit.y + 0.5, hit.z + 0.5, avg[def.side], 16, { speed: 3.2 });
  sfx.break_();
  swing = 1;
}

function tryPlace() {
  const dir = camDir();
  const hit = world.raycast(camera.position.x, camera.position.y, camera.position.z, dir.x, dir.y, dir.z, 5.5);
  if (!hit || (hit.nx === 0 && hit.ny === 0 && hit.nz === 0)) return;
  const id = hotbar[sel];
  if (!PLACEABLE.includes(id)) return;
  const bx = hit.x + hit.nx, by = hit.y + hit.ny, bz = hit.z + hit.nz;
  if (by < 1 || by >= CH) return;
  const cur = world.getBlock(bx, by, bz);
  const curDef = BLOCKS[cur];
  if (cur !== B.AIR && !curDef.liquid && !curDef.cross) return;
  const def = BLOCKS[id];
  if (def.cross && !BLOCKS[world.getBlock(bx, by - 1, bz)].solid) return;
  if (def.solid && player.aabbOverlapsBlock(bx, by, bz)) return;
  const keys = world.setBlock(bx, by, bz, id);
  rebuildNow([...new Set(keys)]);
  sfx.place();
  swing = 1;
}

function pickBlock() {
  const dir = camDir();
  const hit = world.raycast(camera.position.x, camera.position.y, camera.position.z, dir.x, dir.y, dir.z, 5.5);
  if (hit && PLACEABLE.includes(hit.id)) {
    hotbar[sel] = hit.id;
    hud.setSlot(sel, hit.id);
    rebuildHeld();
    sfx.click();
  }
}

function updateInteraction(dt) {
  placeCd -= dt;
  attackCd -= dt;
  if (swing > 0) swing = Math.max(0, swing - dt * 3.4);

  // held block animation
  if (heldMesh) {
    const sw = Math.sin(swing * Math.PI);
    heldMesh.position.set(0.52, -0.44 + Math.sin(player.bobPhase * 2) * 0.02 * player.bobAmt - sw * 0.16, -0.85 - sw * 0.12);
    heldMesh.rotation.x = 0.12 - sw * 0.9;
  }

  const dir = camDir();
  const o = camera.position;

  if (mouse.left && mode === 'play') {
    const hitEnt = entities.raypick(o, dir, 3.6);
    const hitBlk = world.raycast(o.x, o.y, o.z, dir.x, dir.y, dir.z, 5.5);
    const blockDist = hitBlk ? hitBlk.dist : Infinity;

    if (hitEnt && hitEnt.dist < Math.min(blockDist, 3.6) + 0.35) {
      if (attackCd <= 0) {
        attackCd = 0.4;
        swing = 1;
        sfx.swing();
        const kb = dir.clone().setY(0).normalize().multiplyScalar(6.5);
        kb.y = 3.5;
        hitEnt.mob.damage(3, kb, particles, sfx);
      }
      mineTarget = null;
      hud.setMine(null);
    } else if (hitBlk && BLOCKS[hitBlk.id].hard !== Infinity) {
      if (!mineTarget || mineTarget.x !== hitBlk.x || mineTarget.y !== hitBlk.y || mineTarget.z !== hitBlk.z) {
        mineTarget = hitBlk;
        mineT = 0;
      }
      mineT += dt;
      swing = Math.max(swing, 0.55);
      digSnd -= dt;
      if (digSnd <= 0) { sfx.dig(); digSnd = 0.22; }
      hud.setMine(mineT / BLOCKS[hitBlk.id].hard);
      if (mineT >= BLOCKS[hitBlk.id].hard) {
        breakBlock(hitBlk);
        mineTarget = null;
        hud.setMine(null);
      }
    } else {
      mineTarget = null;
      hud.setMine(null);
      if (attackCd <= 0) { attackCd = 0.4; swing = 1; sfx.swing(); }
    }
  } else {
    mineTarget = null;
    hud.setMine(null);
  }

  if (mouse.right && mode === 'play' && placeCd <= 0) {
    tryPlace();
    placeCd = 0.24;
  }
}

// ---------------------------------------------------------------- input
function requestLock() {
  try {
    const p = canvas.requestPointerLock();
    if (p && p.catch) p.catch(() => { fallbackLook = true; });
  } catch { fallbackLook = true; }
  setTimeout(() => {
    if (mode === 'play' && document.pointerLockElement !== canvas && !fallbackLook) {
      fallbackLook = true;
      hud.toast('Pointer lock unavailable — free-look mode enabled');
    }
  }, 1500);
}

function setMode(m) {
  mode = m;
  if (m === 'play') {
    hud.hideOverlay();
    if (!fallbackLook && document.pointerLockElement !== canvas) requestLock();
    hud.updateHearts(player.hp);
  } else if (m === 'pause') {
    saveGame();
    hud.showPause(() => setMode('play'), newWorld);
  } else if (m === 'dead') {
    saveGame();
    hud.showDead(() => {
      player.respawn();
      hud.updateHearts(player.hp);
      setMode('play');
    });
  }
}

function newWorld() {
  try { localStorage.removeItem(SAVE_KEY); } catch { }
  location.reload();
}

document.addEventListener('pointerlockchange', () => {
  const locked = document.pointerLockElement === canvas;
  if (locked) {
    if (mode === 'play') hud.hideOverlay();
  } else if (mode === 'play' && !fallbackLook) {
    setMode('pause');
  }
});
document.addEventListener('pointerlockerror', () => { fallbackLook = true; });

canvas.addEventListener('mousedown', (e) => {
  sfx.ensure();
  if (mode !== 'play') return;
  if (e.button === 0) mouse.left = true;
  else if (e.button === 2) mouse.right = true;
  else if (e.button === 1) {
    e.preventDefault();
    pickBlock();
  }
});
document.addEventListener('mouseup', (e) => {
  if (e.button === 0) mouse.left = false;
  if (e.button === 2) mouse.right = false;
});
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

document.addEventListener('mousemove', (e) => {
  if (mode !== 'play') return;
  const locked = document.pointerLockElement === canvas;
  if (locked || fallbackLook) player.onMouse(e.movementX, e.movementY);
});

document.addEventListener('keydown', (e) => {
  if (e.code === 'F3') {
    e.preventDefault();
    debugOn = !debugOn;
    if (!debugOn) hud.debug(null);
    return;
  }
  if (e.code === 'KeyM' && !e.repeat) {
    hud.toast(sfx.toggleMute() ? 'Sound muted' : 'Sound on');
    return;
  }
  if (e.code === 'KeyE' && !e.repeat) {
    if (mode === 'play') {
      mode = 'palette';
      if (document.pointerLockElement) document.exitPointerLock();
      hud.openPalette(
        (id) => {
          hotbar[sel] = id;
          hud.setSlot(sel, id);
          rebuildHeld();
          hud.closePalette();
        },
        () => setMode('play')
      );
    } else if (mode === 'palette') {
      hud.closePalette();
    }
    return;
  }
  if (e.code === 'Escape') {
    if (mode === 'palette') hud.closePalette();
    else if (mode === 'play' && fallbackLook) setMode('pause');
    return;
  }
  if (mode === 'play') {
    if (e.code === 'Space') e.preventDefault();
    if (/^Digit[1-9]$/.test(e.code)) {
      sel = Number(e.code.slice(5)) - 1;
      hud.select(sel);
      rebuildHeld();
    }
    player.onKey(e.code, true, performance.now() / 1000);
  }
});
document.addEventListener('keyup', (e) => {
  player.onKey(e.code, false, 0);
});

window.addEventListener('blur', () => {
  mouse.left = mouse.right = false;
  player.keys.clear();
});

document.addEventListener('wheel', (e) => {
  if (mode !== 'play') return;
  sel = (sel + (e.deltaY > 0 ? 1 : -1) + 9) % 9;
  hud.select(sel);
  rebuildHeld();
}, { passive: true });

window.addEventListener('beforeunload', () => { if (mode !== 'start') saveGame(); });

// ---------------------------------------------------------------- hooks
player.onDamage = () => {
  hud.vignette();
  sfx.hurt();
  hud.updateHearts(player.hp);
};
player.onDeath = () => {
  particles.burst(player.pos.x, player.pos.y + 1, player.pos.z, [220, 60, 50], 26, { speed: 4 });
  setMode('dead');
};
entities.onFirstGloom = () => {
  hud.toast('A Gloomling has emerged. It hunts in the dark — and melts in the light.', 'gold', 5000);
};

let wasNight = false;
let saveT = 15;

// ---------------------------------------------------------------- start UI
hud.showStart(
  seed,
  () => { sfx.ensure(); sfx.click(); setMode('play'); },
  newWorld
);
hud.toast('Welcome to Voxelheim', 'gold', 3600);

// ---------------------------------------------------------------- debug
let fpsAcc = 0, fpsN = 0, fpsShown = 0, fpsT = 0;
function debugText() {
  const p = player.pos;
  const b = world.biomeAt(Math.floor(p.x), Math.floor(p.z));
  return [
    `fps ${fpsShown}`,
    `xyz ${p.x.toFixed(1)} ${p.y.toFixed(1)} ${p.z.toFixed(1)}`,
    `biome ${b}  time ${(sky.time * 24).toFixed(1)}h`,
    `chunks ${world.chunks.size}  ents ${entities.list.length}`,
    `draws ${renderer.info.render.calls}  tris ${(renderer.info.render.triangles / 1000).toFixed(0)}k`,
    `mode ${mode}${fallbackLook ? ' (free-look)' : ''}${player.flying ? ' flying' : ''}`,
  ].join('\n');
}

// ---------------------------------------------------------------- loop
let last = performance.now();
renderer.setAnimationLoop(() => {
  const now = performance.now();
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  frame++;
  fpsAcc += dt; fpsN++;
  if (fpsAcc >= 0.5) { fpsShown = Math.round(fpsN / fpsAcc); fpsAcc = 0; fpsN = 0; }

  if (mode === 'play') {
    player.update(dt);
    updateInteraction(dt);
    entities.update(dt, player, sky);
    streamChunks();
    particles.update(dt);

    if (player.inWater !== wasInWater) {
      if (player.headInWater || player.inWater) sfx.splash();
      wasInWater = player.inWater;
    }
    hud.waterFx(player.headInWater);
    hud.updateAir(player.headInWater || player.air < 9.8, player.air);

    const night = sky.nightFactor > 0.5;
    if (night && !wasNight) hud.toast('Night falls… the Gloomlings stir.', 'gold');
    if (!night && wasNight) hud.toast('Dawn breaks over Voxelheim.');
    wasNight = night;

    saveT -= dt;
    if (saveT <= 0) { saveT = 15; saveGame(); }
  }

  sky.update(mode === 'play' ? dt : 0, player.pos, player.headInWater, 84);
  const b = sky.worldBrightness();
  matOpaque.color.setRGB(b, b, b);
  matCutout.color.setRGB(b, b, b);
  matWater.color.setRGB(b, b, b);

  if (debugOn && (frame % 12 === 0)) hud.debug(debugText());

  renderer.render(scene, camera);
});
