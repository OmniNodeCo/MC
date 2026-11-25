// Voxelheim — pooled point-sprite particles for breaks, hits and poofs.
import * as THREE from '../vendor/three.module.js';

const MAX = 420;

export class Particles {
  constructor(scene) {
    this.pos = new Float32Array(MAX * 3);
    this.col = new Float32Array(MAX * 3);
    this.vel = new Float32Array(MAX * 3);
    this.life = new Float32Array(MAX);
    this.grav = new Float32Array(MAX);
    for (let i = 0; i < MAX; i++) {
      this.pos[i * 3 + 1] = -1000;
      this.life[i] = 0;
    }
    this.cursor = 0;

    const g = new THREE.BufferGeometry();
    this.posAttr = new THREE.BufferAttribute(this.pos, 3);
    this.colAttr = new THREE.BufferAttribute(this.col, 3);
    g.setAttribute('position', this.posAttr);
    g.setAttribute('color', this.colAttr);
    const m = new THREE.PointsMaterial({ size: 0.14, vertexColors: true, sizeAttenuation: true });
    this.points = new THREE.Points(g, m);
    this.points.frustumCulled = false;
    scene.add(this.points);
  }

  burst(x, y, z, rgb, count, { speed = 3, spread = 0.4, life = 0.7, gravity = 14, up = 2 } = {}) {
    for (let k = 0; k < count; k++) {
      const i = this.cursor;
      this.cursor = (this.cursor + 1) % MAX;
      const i3 = i * 3;
      this.pos[i3] = x + (Math.random() - 0.5) * spread * 2;
      this.pos[i3 + 1] = y + (Math.random() - 0.5) * spread * 2;
      this.pos[i3 + 2] = z + (Math.random() - 0.5) * spread * 2;
      const a = Math.random() * Math.PI * 2;
      const r = Math.random() * speed;
      this.vel[i3] = Math.cos(a) * r;
      this.vel[i3 + 1] = Math.random() * up + 1;
      this.vel[i3 + 2] = Math.sin(a) * r;
      const f = 0.75 + Math.random() * 0.5;
      this.col[i3] = Math.min(1, rgb[0] / 255 * f);
      this.col[i3 + 1] = Math.min(1, rgb[1] / 255 * f);
      this.col[i3 + 2] = Math.min(1, rgb[2] / 255 * f);
      this.life[i] = life * (0.7 + Math.random() * 0.6);
      this.grav[i] = gravity;
    }
  }

  update(dt) {
    for (let i = 0; i < MAX; i++) {
      if (this.life[i] <= 0) continue;
      this.life[i] -= dt;
      const i3 = i * 3;
      if (this.life[i] <= 0) {
        this.pos[i3 + 1] = -1000;
        continue;
      }
      this.vel[i3 + 1] -= this.grav[i] * dt;
      this.pos[i3] += this.vel[i3] * dt;
      this.pos[i3 + 1] += this.vel[i3 + 1] * dt;
      this.pos[i3 + 2] += this.vel[i3 + 2] * dt;
    }
    this.posAttr.needsUpdate = true;
    this.colAttr.needsUpdate = true;
  }
}
