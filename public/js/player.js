// Voxelheim — player: movement, camera, health, swimming, flying.
import * as THREE from '../vendor/three.module.js';
import { collideMove, CH } from './world.js';
import { B, BLOCKS } from './blocks.js';
import { clamp, lerp } from './noise.js';

const GRAVITY = 27;
const WALK = 4.4, SPRINT = 6.6, FLY = 12, SWIM = 3.2;
const JUMP_V = 8.8;
const EYE = 1.62, HALF = 0.3, HEIGHT = 1.8;

export class Player {
  constructor(world, camera) {
    this.world = world;
    this.cam = camera;
    this.pos = new THREE.Vector3(8.5, 60, 8.5);
    this.vel = new THREE.Vector3();
    this.yaw = 0.6; this.pitch = -0.1;
    this.onGround = false;
    this.flying = false;
    this.sprinting = false;
    this.hp = 20;
    this.air = 10;
    this.dead = false;
    this.inWater = false;
    this.headInWater = false;
    this.bobPhase = 0;
    this.bobAmt = 0;
    this.fov = 74;
    this.lastSpace = -1;
    this.invuln = 0;
    this.drownTick = 0;
    this.spawn = new THREE.Vector3(8.5, 60, 8.5);
    this.onDamage = null;
    this.onDeath = null;
    this.keys = new Set();
    this.cam.rotation.order = 'YXZ';
  }

  onKey(code, down, now) {
    if (down) this.keys.add(code); else this.keys.delete(code);
    if (code === 'Space' && down) {
      if (now - this.lastSpace < 0.3) {
        this.flying = !this.flying;
        this.vel.y = 0;
        this.lastSpace = -1;
      } else this.lastSpace = now;
    }
  }

  onMouse(dx, dy) {
    this.yaw -= dx * 0.0023;
    this.pitch = clamp(this.pitch - dy * 0.0023, -Math.PI / 2 + 0.01, Math.PI / 2 - 0.01);
  }

  blockAt(dx, dy, dz) {
    return this.world.getBlock(
      Math.floor(this.pos.x + dx), Math.floor(this.pos.y + dy), Math.floor(this.pos.z + dz)
    );
  }

  update(dt) {
    const k = this.keys;
    const fwd = (k.has('KeyW') ? 1 : 0) - (k.has('KeyS') ? 1 : 0);
    const strafe = (k.has('KeyD') ? 1 : 0) - (k.has('KeyA') ? 1 : 0);
    const jump = k.has('Space');
    const shift = k.has('ShiftLeft') || k.has('ShiftRight');
    this.sprinting = shift && fwd > 0 && !this.flying;

    // wish direction in world space
    let wx = 0, wz = 0;
    if (fwd || strafe) {
      const sin = Math.sin(this.yaw), cos = Math.cos(this.yaw);
      // forward is -Z rotated by yaw
      wx = (-sin * fwd) + (cos * strafe);
      wz = (-cos * fwd) + (-sin * strafe);
      const l = Math.hypot(wx, wz);
      wx /= l; wz /= l;
    }

    this.inWater = this.blockAt(0, 0.4, 0) === B.WATER || this.blockAt(0, 0.9, 0) === B.WATER;
    this.headInWater = this.blockAt(0, EYE, 0) === B.WATER;

    // vertical
    if (this.flying) {
      const target = jump ? 9 : shift ? -9 : 0;
      this.vel.y += (target - this.vel.y) * Math.min(1, dt * 12);
    } else if (this.inWater) {
      this.vel.y -= 14 * dt;
      if (jump) this.vel.y += 40 * dt;
      this.vel.y = clamp(this.vel.y, -4.5, 4.2);
    } else {
      this.vel.y -= GRAVITY * dt;
      if (jump && this.onGround) this.vel.y = JUMP_V;
    }

    // horizontal
    const speed = this.flying ? FLY : this.inWater ? SWIM : this.sprinting ? SPRINT : WALK;
    const accel = (this.onGround || this.flying || this.inWater) ? 13 : 3.5;
    this.vel.x += (wx * speed - this.vel.x) * Math.min(1, accel * dt);
    this.vel.z += (wz * speed - this.vel.z) * Math.min(1, accel * dt);

    const wasOnGround = this.onGround;
    const vyBefore = this.vel.y;
    this.onGround = collideMove(this.world, this.pos, this.vel, dt, HALF, HEIGHT);

    // fall damage
    if (!wasOnGround && this.onGround && !this.inWater && !this.flying) {
      const impact = -vyBefore;
      if (impact > 12.5) {
        const dmg = Math.floor((impact - 12.5) * 0.62);
        if (dmg > 0) this.damage(dmg, null);
      }
    }
    if (this.flying) this.flying = this.flying; // no-op, clarity

    // drowning
    if (this.headInWater) {
      this.air -= dt;
      if (this.air <= 0) {
        this.drownTick += dt;
        if (this.drownTick >= 1) { this.drownTick -= 1; this.damage(2, null); }
      }
    } else {
      this.air = Math.min(10, this.air + dt * 3.5);
      this.drownTick = 0;
    }

    // void
    if (this.pos.y < -12) this.damage(100, null);

    // camera
    const hSpeed = Math.hypot(this.vel.x, this.vel.z);
    const moving = this.onGround && hSpeed > 1.2;
    this.bobAmt = lerp(this.bobAmt, moving ? Math.min(1, hSpeed / SPRINT) : 0, Math.min(1, dt * 8));
    if (moving) this.bobPhase += dt * hSpeed * 1.7;
    const bobY = Math.sin(this.bobPhase * 2) * 0.045 * this.bobAmt;

    this.cam.position.set(this.pos.x, this.pos.y + EYE + bobY, this.pos.z);
    this.cam.rotation.y = this.yaw;
    this.cam.rotation.x = this.pitch;

    const targetFov = 74 + (this.sprinting ? 8 : 0) + (this.flying ? 5 : 0);
    if (Math.abs(targetFov - this.fov) > 0.05) {
      this.fov = lerp(this.fov, targetFov, Math.min(1, dt * 8));
      this.cam.fov = this.fov;
      this.cam.updateProjectionMatrix();
    }

    if (this.invuln > 0) this.invuln -= dt;
  }

  damage(dmg, knock) {
    if (this.dead || this.invuln > 0) return;
    if (dmg >= 100) { this.hp = 0; }
    else this.hp = Math.max(0, this.hp - dmg);
    this.invuln = 0.5;
    if (knock) {
      this.vel.x += knock.x; this.vel.z += knock.z;
      this.vel.y += Math.max(2, knock.y ?? 3);
    }
    if (this.onDamage) this.onDamage(dmg);
    if (this.hp <= 0) {
      this.dead = true;
      if (this.onDeath) this.onDeath();
    }
  }

  respawn() {
    this.pos.copy(this.spawn);
    this.pos.y = this.world.surfaceY(Math.floor(this.pos.x), Math.floor(this.pos.z)) + 1.01;
    this.vel.set(0, 0, 0);
    this.hp = 20;
    this.air = 10;
    this.dead = false;
    this.flying = false;
  }

  aabbOverlapsBlock(bx, by, bz) {
    return bx + 1 > this.pos.x - HALF && bx < this.pos.x + HALF &&
      by + 1 > this.pos.y && by < this.pos.y + HEIGHT &&
      bz + 1 > this.pos.z - HALF && bz < this.pos.z + HALF;
  }
}
