// Voxelheim — creatures. Five original mobs built from animated voxel parts.
//   Gloomling  — hostile night-stalker that dissolves in sunlight
//   Sand Skitter — hostile desert crab, fast and nippy
//   Moss Hopper — passive squishy hopper of the meadows
//   Sky Wisp   — glowing mountain spirit that drifts near the peaks
//   Puffbird   — chatty ambient bird that circles the skies

import * as THREE from '../vendor/three.module.js';
import { collideMove, CH, SEA } from './world.js';
import { B, BLOCKS } from './blocks.js';
import { mobPx } from './textures.js';
import { clamp, lerp } from './noise.js';

const S = 1 / 16;
const texCache = new Map();

function texOf(px, key) {
  let t = texCache.get(key);
  if (!t) {
    t = new THREE.CanvasTexture(px.toCanvas());
    t.magFilter = THREE.NearestFilter;
    t.minFilter = THREE.NearestFilter;
    t.generateMipmaps = false;
    t.colorSpace = THREE.SRGBColorSpace;
    texCache.set(key, t);
  }
  return t;
}

// one material per mob instance (so hurt-flash tinting is per-mob)
function mat(px, key, glow = false) {
  const m = new THREE.MeshBasicMaterial({ map: texOf(px, key) });
  return { m, glow };
}

function boxMesh(wPx, hPx, dPx, mats, pivotTop = false) {
  const g = new THREE.BoxGeometry(wPx * S, hPx * S, dPx * S);
  if (pivotTop) g.translate(0, -hPx * S / 2, 0);
  const m = new THREE.Mesh(g, mats.map(x => x.m));
  return { mesh: m, mats };
}

// body texture on all faces, face texture on +z
function partMesh(wPx, hPx, dPx, bodyPx, bodyKey, facePx, faceKey, pivotTop = false) {
  const b = mat(bodyPx, bodyKey);
  const f = mat(facePx, faceKey);
  const all = [b, b, b, b, f, b];
  return boxMesh(wPx, hPx, dPx, all, pivotTop);
}

function solidMat(hexColor) {
  return { m: new THREE.MeshBasicMaterial({ color: hexColor }), glow: false };
}

function lerpAngle(a, b, t) {
  let d = (b - a) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  if (d < -Math.PI) d += Math.PI * 2;
  return a + d * t;
}

// ------------------------------------------------------------------ builders
function buildGloomling(mob) {
  const { body, face } = mobPx('gloomling');
  const torso = partMesh(10, 9, 10, body, 'gl-b', face, 'gl-f');
  torso.mesh.position.y = 7.5 * S;
  mob.group.add(torso.mesh);
  mob.anim.torso = torso;
  mob.anim.feet = [];
  for (const sx of [-2.5, 2.5]) {
    const foot = partMesh(3, 3, 4, body, 'gl-b', body, 'gl-b', true);
    foot.mesh.position.set(sx * S, 3 * S, 0);
    mob.group.add(foot.mesh);
    mob.anim.feet.push(foot);
  }
  mob.mats = [torso, ...mob.anim.feet].flatMap(p => p.mats);
}

function buildHopper(mob) {
  const { body, face } = mobPx('hopper');
  const torso = partMesh(12, 10, 12, body, 'ho-b', face, 'ho-f');
  torso.mesh.position.y = 7 * S;
  mob.group.add(torso.mesh);
  mob.anim.torso = torso;
  mob.anim.feet = [];
  for (const sx of [-3, 3]) {
    const foot = partMesh(4, 2, 4, body, 'ho-b', body, 'ho-b', true);
    foot.mesh.position.set(sx * S, 2 * S, 1 * S);
    mob.group.add(foot.mesh);
    mob.anim.feet.push(foot);
  }
  mob.mats = [torso, ...mob.anim.feet].flatMap(p => p.mats);
}

function buildSkitter(mob) {
  const { body, face } = mobPx('skitter');
  const shell = partMesh(14, 6, 14, body, 'sk-b', face, 'sk-f');
  shell.mesh.position.y = 8 * S;
  mob.group.add(shell.mesh);
  mob.anim.shell = shell;
  mob.anim.legs = [];
  const legBody = mat(body, 'sk-b');
  for (const sz of [-4, 0, 4]) {
    for (const sx of [-7, 7]) {
      const leg = boxMesh(1.5, 5, 1.5, [legBody, legBody, legBody, legBody, legBody, legBody], true);
      leg.mesh.position.set(sx * S, 5.5 * S, sz * S);
      mob.group.add(leg.mesh);
      mob.anim.legs.push(leg);
    }
  }
  mob.anim.mandibles = [];
  for (const sx of [-2, 2]) {
    const mand = boxMesh(2, 2, 3, [legBody, legBody, legBody, legBody, legBody, legBody]);
    mand.mesh.position.set(sx * S, 5 * S, 8 * S);
    mob.group.add(mand.mesh);
    mob.anim.mandibles.push(mand);
  }
  mob.mats = [shell, ...mob.anim.legs, ...mob.anim.mandibles].flatMap(p => p.mats);
}

function buildWisp(mob) {
  const { body, face, shard } = mobPx('wisp');
  const core = partMesh(8, 8, 8, body, 'wi-b', face, 'wi-f');
  core.mesh.position.y = 0;
  for (const p of core.mats) p.glow = true;
  mob.group.add(core.mesh);
  mob.anim.core = core;
  mob.anim.shards = [];
  mob.anim.orbit = new THREE.Group();
  for (let i = 0; i < 3; i++) {
    const sh = boxMesh(3, 3, 3, [mat(shard, 'wi-s'), mat(shard, 'wi-s'), mat(shard, 'wi-s'), mat(shard, 'wi-s'), mat(shard, 'wi-s'), mat(shard, 'wi-s')]);
    for (const p of sh.mats) p.glow = true;
    const a = (i / 3) * Math.PI * 2;
    sh.mesh.position.set(Math.cos(a) * 12 * S, Math.sin(a * 2) * 2 * S, Math.sin(a) * 12 * S);
    mob.anim.orbit.add(sh.mesh);
    mob.anim.shards.push(sh);
  }
  mob.group.add(mob.anim.orbit);
  mob.mats = [core, ...mob.anim.shards].flatMap(p => p.mats);
}

function buildPuffbird(mob) {
  const { body, face, wing } = mobPx('puffbird');
  const torso = partMesh(7, 6, 10, body, 'pb-b', face, 'pb-f');
  torso.mesh.position.z = -1 * S;
  mob.group.add(torso.mesh);
  const head = partMesh(5, 5, 5, body, 'pb-b', face, 'pb-f');
  head.mesh.position.set(0, 4.5 * S, 4 * S);
  mob.group.add(head.mesh);
  const beakM = solidMat(0xe8a13c);
  const beak = boxMesh(2, 2, 3, [beakM, beakM, beakM, beakM, beakM, beakM]);
  beak.mesh.position.set(0, 4 * S, 7.5 * S);
  mob.group.add(beak.mesh);
  mob.anim.wings = [];
  for (const sgn of [1, -1]) {
    const wm = mat(wing, 'pb-w');
    const wgeo = new THREE.BoxGeometry(8 * S, 1 * S, 6 * S);
    wgeo.translate(sgn * 4 * S, 0, 0);
    const wmesh = new THREE.Mesh(wgeo, [wm.m, wm.m, wm.m, wm.m, wm.m, wm.m]);
    wmesh.position.set(sgn * 3.5 * S, 2 * S, -1 * S);
    mob.group.add(wmesh);
    mob.anim.wings.push({ mesh: wmesh, mats: [wm], sgn });
  }
  const tailM = mat(wing, 'pb-w');
  const tail = boxMesh(1, 3, 5, [tailM, tailM, tailM, tailM, tailM, tailM]);
  tail.mesh.position.set(0, 1 * S, -7 * S);
  mob.group.add(tail.mesh);
  mob.mats = [torso, head, beak, tail, ...mob.anim.wings].flatMap(p => p.mats);
}

// ------------------------------------------------------------------ defs
export const MOBS = {
  gloomling: {
    name: 'Gloomling', hostile: true, nightOnly: true, burnInSun: true,
    hp: 8, speed: 2.1, chaseSpeed: 3.8, dmg: 3, aggro: 15,
    half: 0.32, h: 0.8, hopper: true, build: buildGloomling,
    pcolor: [0.25, 0.16, 0.38], brightMin: 0.55,
  },
  skitter: {
    name: 'Sand Skitter', hostile: true, desertOnly: true,
    hp: 6, speed: 3.0, chaseSpeed: 4.4, dmg: 2, aggro: 12,
    half: 0.44, h: 0.62, hopper: false, build: buildSkitter,
    pcolor: [0.72, 0.6, 0.4], brightMin: 0.4,
  },
  hopper: {
    name: 'Moss Hopper', hostile: false,
    hp: 4, speed: 2.6, chaseSpeed: 2.6, dmg: 0, aggro: 0,
    half: 0.38, h: 0.75, hopper: true, build: buildHopper,
    pcolor: [0.3, 0.55, 0.25], brightMin: 0.4,
  },
  wisp: {
    name: 'Sky Wisp', hostile: false, fly: true, centerPivot: true,
    hp: 3, speed: 1.1, chaseSpeed: 1.1, dmg: 0, aggro: 0,
    half: 0.3, h: 0.6, build: buildWisp,
    pcolor: [0.7, 0.9, 0.95], brightMin: 1.0,
  },
  puffbird: {
    name: 'Puffbird', hostile: false, fly: true, centerPivot: true,
    hp: 2, speed: 3.4, chaseSpeed: 3.4, dmg: 0, aggro: 0,
    half: 0.3, h: 0.5, build: buildPuffbird,
    pcolor: [0.9, 0.9, 0.88], brightMin: 0.4,
  },
};

// ------------------------------------------------------------------ Mob
export class Mob {
  constructor(kind, world, x, y, z) {
    this.kind = kind;
    this.def = MOBS[kind];
    this.world = world;
    this.pos = new THREE.Vector3(x, y, z);
    this.vel = new THREE.Vector3();
    this.hp = this.def.hp;
    this.onGround = false;
    this.yaw = Math.random() * Math.PI * 2;
    this.timer = 0;
    this.moveDir = Math.random() * Math.PI * 2;
    this.moving = Math.random() < 0.7;
    this.atkCd = 0;
    this.flash = 0;
    this.dying = 0;
    this.burn = 0;
    this.hopCd = Math.random() * 1.5;
    this.age = Math.random() * 10;
    this.wp = null;
    this.wpTimer = 0;
    this.chasing = false;
    this.group = new THREE.Group();
    this.anim = {};
    this.mats = [];
    this.def.build(this);
    this.group.position.copy(this.pos);
    this.firstSeenHook = null;
  }

  skyExposed() {
    const x = Math.floor(this.pos.x), z = Math.floor(this.pos.z);
    for (let y = Math.floor(this.pos.y + 2); y < CH; y++) {
      if (BLOCKS[this.world.getBlock(x, y, z)].opaque) return false;
    }
    return true;
  }

  damage(d, knock, particles, sfx) {
    if (this.dying > 0) return;
    this.hp -= d;
    this.flash = 0.18;
    if (knock) { this.vel.x += knock.x; this.vel.z += knock.z; this.vel.y += knock.y ?? 3; }
    if (sfx) sfx.mobHurt();
    if (this.hp <= 0) {
      this.dying = 0.45;
      if (sfx) sfx.die();
      const c = this.pos;
      particles.burst(c.x, c.y + this.def.h * 0.6, c.z, this.def.pcolor.map(v => v * 255), 16, { speed: 3.5 });
    }
  }

  update(dt, player, dayFactor, nightFactor, particles, sfx) {
    this.age += dt;
    if (this.atkCd > 0) this.atkCd -= dt;
    if (this.flash > 0) this.flash -= dt;

    if (this.dying > 0) {
      this.dying -= dt;
      const t = Math.max(0.02, this.dying / 0.45);
      this.group.scale.setScalar(t);
      this.group.rotation.y += dt * 12;
      return this.dying <= 0 ? 'dead' : 'dying';
    }

    const def = this.def;
    const toPlayer = new THREE.Vector3().subVectors(player.pos, this.pos);
    const distXZ = Math.hypot(toPlayer.x, toPlayer.z);
    const distY = Math.abs(toPlayer.y);

    if (def.fly) {
      this.updateFly(dt, player, dayFactor);
      this.animate(dt);
      return 'alive';
    }

    // ---- ground AI ----
    this.chasing = def.hostile && !player.dead && distXZ < def.aggro && distY < 6 &&
      (!def.nightOnly || nightFactor > 0.45);

    if (def.burnInSun && dayFactor > 0.8 && this.skyExposed()) {
      this.burn += dt;
      if (Math.floor(this.burn * 8) % 2 === 0) this.flash = Math.max(this.flash, 0.05);
      if (this.burn > 1.6) {
        particles.burst(this.pos.x, this.pos.y + 0.5, this.pos.z, [90, 70, 140], 14, { speed: 2.5 });
        this.dying = 0.45;
        return 'dying';
      }
    } else {
      this.burn = Math.max(0, this.burn - dt * 2);
    }

    if (this.chasing) {
      this.moveDir = Math.atan2(toPlayer.x, toPlayer.z);
      this.moving = distXZ > 1.1;
      if (distXZ < 1.4 && distY < 2 && this.atkCd <= 0) {
        this.atkCd = 1.1;
        const kn = toPlayer.clone().setY(0).normalize().multiplyScalar(7);
        kn.y = 4;
        player.damage(def.dmg, kn);
      }
    } else {
      this.timer -= dt;
      if (this.timer <= 0) {
        this.timer = 1.5 + Math.random() * 2.5;
        this.moving = Math.random() < 0.65;
        this.moveDir = Math.random() * Math.PI * 2;
      }
    }

    const inWater = this.world.getBlock(Math.floor(this.pos.x), Math.floor(this.pos.y + 0.3), Math.floor(this.pos.z)) === B.WATER;

    if (def.hopper) {
      this.hopCd -= dt;
      if (this.onGround && this.hopCd <= 0 && this.moving) {
        const sp = (this.chasing ? def.chaseSpeed : def.speed) * 0.95;
        this.vel.x = Math.sin(this.moveDir) * sp;
        this.vel.z = Math.cos(this.moveDir) * sp;
        this.vel.y = 7.6;
        this.hopCd = this.chasing ? 0.45 + Math.random() * 0.35 : 0.9 + Math.random() * 1.1;
        if (sfx && distXZ < 20) sfx.hop();
      }
      if (this.onGround) { this.vel.x *= Math.max(0, 1 - dt * 6); this.vel.z *= Math.max(0, 1 - dt * 6); }
    } else {
      const sp = this.chasing ? def.chaseSpeed : def.speed;
      const tx = this.moving ? Math.sin(this.moveDir) * sp : 0;
      const tz = this.moving ? Math.cos(this.moveDir) * sp : 0;
      this.vel.x += (tx - this.vel.x) * Math.min(1, dt * 8);
      this.vel.z += (tz - this.vel.z) * Math.min(1, dt * 8);
    }

    if (inWater) {
      this.vel.y += 26 * dt;
      this.vel.y = clamp(this.vel.y, -3, 2.6);
      this.vel.x *= Math.max(0, 1 - dt * 1.5);
      this.vel.z *= Math.max(0, 1 - dt * 1.5);
    } else {
      this.vel.y -= 26 * dt;
    }
    this.vel.y = Math.max(this.vel.y, -38);

    this.onGround = collideMove(this.world, this.pos, this.vel, dt, def.half, def.h);

    // unstick: if embedded in a block, hop out
    if (!this.onGround && Math.abs(this.vel.x) < 0.05 && Math.abs(this.vel.z) < 0.05 && this.vel.y === 0) {
      this.vel.y = 6;
    }

    // facing
    const hSp = Math.hypot(this.vel.x, this.vel.z);
    if (hSp > 0.4 || this.chasing) {
      this.yaw = lerpAngle(this.yaw, this.chasing && hSp < 0.4 ? this.moveDir : Math.atan2(this.vel.x, this.vel.z), Math.min(1, dt * 8));
    }
    this.group.rotation.y = this.yaw;
    this.animate(dt);
    return 'alive';
  }

  updateFly(dt, player, dayFactor) {
    const def = this.def;
    this.wpTimer -= dt;
    if (!this.wp || this.wpTimer <= 0 || this.pos.distanceTo(this.wp) < 2) {
      this.wpTimer = 6 + Math.random() * 6;
      const ang = Math.random() * Math.PI * 2;
      const r = def.name === 'Puffbird' ? 8 + Math.random() * 16 : 5 + Math.random() * 12;
      const x = player.pos.x + Math.cos(ang) * r;
      const z = player.pos.z + Math.sin(ang) * r;
      const sy = this.world.surfaceY(Math.floor(x), Math.floor(z));
      const y = def.name === 'Puffbird' ? sy + 6 + Math.random() * 7 : sy + 7 + Math.random() * 9;
      this.wp = new THREE.Vector3(x, clamp(y, 8, CH - 8), z);
    }
    const dir = new THREE.Vector3().subVectors(this.wp, this.pos);
    const d = dir.length();
    if (d > 0.01) dir.multiplyScalar(def.speed / d);
    this.vel.lerp(dir, Math.min(1, dt * 2.5));
    this.pos.addScaledVector(this.vel, dt);

    // gentle avoidance of solid ground
    const bx = Math.floor(this.pos.x), by = Math.floor(this.pos.y), bz = Math.floor(this.pos.z);
    if (BLOCKS[this.world.getBlock(bx, by, bz)].solid) this.pos.y += 3 * dt;

    this.pos.y = clamp(this.pos.y, 4, CH - 4);
    if (Math.hypot(this.vel.x, this.vel.z) > 0.3) {
      this.yaw = lerpAngle(this.yaw, Math.atan2(this.vel.x, this.vel.z), Math.min(1, dt * 4));
    }
    this.group.rotation.y = this.yaw;
  }

  animate(dt) {
    const def = this.def;
    if (this.kind === 'gloomling') {
      for (let i = 0; i < this.anim.feet.length; i++) {
        this.anim.feet[i].mesh.rotation.x = Math.sin(this.age * 10 + i * Math.PI) * (this.onGround ? 0.15 : 0.5);
      }
      this.anim.torso.mesh.position.y = 7.5 * S + Math.sin(this.age * 3) * 0.4 * S;
    } else if (this.kind === 'hopper') {
      const target = this.onGround ? 0.88 : 1.15;
      const s = lerp(this.group.scale.y, target, Math.min(1, dt * 12));
      this.group.scale.y = s;
      this.group.scale.x = lerp(this.group.scale.x, 2 - s, Math.min(1, dt * 12));
      this.group.scale.z = lerp(this.group.scale.z, 2 - s, Math.min(1, dt * 12));
    } else if (this.kind === 'skitter') {
      for (let i = 0; i < this.anim.legs.length; i++) {
        this.anim.legs[i].mesh.rotation.x = Math.sin(this.age * 14 + (i % 2) * Math.PI + Math.floor(i / 2)) * (this.moving ? 0.55 : 0.08);
      }
      for (let i = 0; i < this.anim.mandibles.length; i++) {
        this.anim.mandibles[i].mesh.rotation.y = Math.sin(this.age * (this.chasing ? 16 : 4) + i * Math.PI) * (this.chasing ? 0.3 : 0.1);
      }
    } else if (this.kind === 'wisp') {
      this.anim.orbit.rotation.y = this.age * 2.2;
      this.anim.orbit.rotation.x = Math.sin(this.age * 0.9) * 0.3;
      this.anim.core.mesh.scale.setScalar(1 + Math.sin(this.age * 3) * 0.07);
    } else if (this.kind === 'puffbird') {
      for (const w of this.anim.wings) w.mesh.rotation.z = w.sgn * Math.sin(this.age * 11) * 0.65;
      this.group.rotation.x = clamp(-this.vel.y * 0.06, -0.3, 0.3);
    }
  }

  applyBrightness(b) {
    const def = this.def;
    const bb = Math.max(def.brightMin, b);
    for (const { m, glow } of this.mats) {
      if (this.flash > 0 && !glow) {
        m.color.setRGB(1.5, 0.4, 0.35);
      } else if (glow) {
        m.color.setRGB(1, 1, 1);
      } else if (this.burn > 0) {
        m.color.setRGB(bb * (1 + Math.sin(this.age * 20) * 0.3), bb * 0.55, bb * 0.4);
      } else {
        m.color.setRGB(bb, bb, bb);
      }
    }
  }
}

// ------------------------------------------------------------------ manager
export class Entities {
  constructor(scene, world, particles, sfx) {
    this.scene = scene;
    this.world = world;
    this.particles = particles;
    this.sfx = sfx;
    this.list = [];
    this.spawnT = 2;
    this.onFirstGloom = null;
    this.gloomSeen = false;
    this._ray = new THREE.Ray();
    this._box = new THREE.Box3();
    this._v = new THREE.Vector3();
  }

  count(kind) {
    let n = 0;
    for (const m of this.list) if (m.kind === kind && m.dying <= 0) n++;
    return n;
  }

  trySpawn(player, sky) {
    const p = player.pos;
    const ang = Math.random() * Math.PI * 2;
    const d = 24 + Math.random() * 20;
    const x = Math.floor(p.x + Math.cos(ang) * d);
    const z = Math.floor(p.z + Math.sin(ang) * d);
    const info = this.world.columnInfo(x, z);
    if (info.biome === 'ocean') return;
    const sy = this.world.surfaceY(x, z);
    if (sy <= SEA) return;
    if (this.world.getBlock(x, sy + 1, z) === B.WATER) return;

    const night = sky.nightFactor > 0.55;
    if (night && this.count('gloomling') < 7 && Math.random() < 0.8 && info.biome !== 'desert') {
      this.spawn('gloomling', x + 0.5, sy + 1, z + 0.5);
      if (!this.gloomSeen) {
        this.gloomSeen = true;
        if (this.onFirstGloom) this.onFirstGloom();
      }
      return;
    }
    if (info.biome === 'desert' && this.count('skitter') < 4 && Math.random() < 0.6) {
      this.spawn('skitter', x + 0.5, sy + 1, z + 0.5);
      return;
    }
    if (sky.dayFactor > 0.55 && Math.random() < 0.55) {
      if (info.biome === 'mountain' && this.count('wisp') < 3 && Math.random() < 0.7) {
        this.spawn('wisp', x + 0.5, sy + 6 + Math.random() * 6, z + 0.5);
      } else if ((info.biome === 'forest' || info.biome === 'plains') && this.count('hopper') < 5 && Math.random() < 0.55) {
        this.spawn('hopper', x + 0.5, sy + 1, z + 0.5);
      } else if ((info.biome === 'forest' || info.biome === 'plains' || info.biome === 'mountain') && this.count('puffbird') < 4) {
        this.spawn('puffbird', x + 0.5, sy + 6 + Math.random() * 6, z + 0.5);
      }
    }
  }

  spawn(kind, x, y, z) {
    const m = new Mob(kind, this.world, x, y, z);
    this.scene.add(m.group);
    this.list.push(m);
    return m;
  }

  update(dt, player, sky) {
    this.spawnT -= dt;
    if (this.spawnT <= 0) {
      this.spawnT = 1.6;
      this.trySpawn(player, sky);
    }

    const dead = [];
    for (const m of this.list) {
      const st = m.update(dt, player, sky.dayFactor, sky.nightFactor, this.particles, this.sfx);
      m.group.position.copy(m.pos);
      if (st === 'dead') dead.push(m);
    }

    const b = 0.32 + 0.68 * sky.dayFactor;
    for (const m of this.list) m.applyBrightness(b);

    if (dead.length) {
      for (const m of dead) this.remove(m);
      this.list = this.list.filter(m => !dead.includes(m));
    }
    // despawn far mobs
    for (const m of [...this.list]) {
      if (m.pos.distanceTo(player.pos) > 72) this.remove(m), this.list.splice(this.list.indexOf(m), 1);
    }
  }

  remove(m) {
    this.scene.remove(m.group);
    m.group.traverse(o => {
      if (o.geometry) o.geometry.dispose();
    });
    for (const { m: mat } of m.mats) mat.dispose();
  }

  // nearest mob under a crosshair ray
  raypick(origin, dir, maxDist) {
    this._ray.origin.copy(origin);
    this._ray.direction.copy(dir);
    let best = null;
    for (const m of this.list) {
      if (m.dying > 0) continue;
      const center = this._v.set(m.pos.x, m.pos.y + (m.def.centerPivot ? 0 : m.def.h / 2), m.pos.z);
      this._box.setFromCenterAndSize(center, new THREE.Vector3(m.def.half * 2 + 0.15, m.def.h + 0.15, m.def.half * 2 + 0.15));
      const hit = this._ray.intersectBox(this._box, this._v);
      if (hit) {
        const d = hit.distanceTo(origin);
        if (d <= maxDist && (!best || d < best.dist)) best = { mob: m, dist: d, point: hit.clone() };
      }
    }
    return best;
  }
}
