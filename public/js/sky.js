// Voxelheim — sky: day/night cycle, sun & moon, stars, drifting clouds.
import * as THREE from '../vendor/three.module.js';
import { sunPx, moonPx } from './textures.js';
import { clamp01, lerp, smoothstep } from './noise.js';

const DAY_LEN = 420; // seconds for a full cycle

export class Sky {
  constructor(scene) {
    this.scene = scene;
    this.time = 0.08; // start just after dawn
    this.dayFactor = 1;
    this.nightFactor = 0;

    this.skyDay = new THREE.Color('#7fb3e8');
    this.skyNight = new THREE.Color('#0a0d1e');
    this.skyDusk = new THREE.Color('#e8935a');
    this.fogDay = new THREE.Color('#a5c6ea');
    this.fogNight = new THREE.Color('#0d1126');
    this.fogDusk = new THREE.Color('#d99a68');
    this.waterFog = new THREE.Color('#1c4a8c');
    this._color = new THREE.Color();
    this._color2 = new THREE.Color();

    scene.background = new THREE.Color('#7fb3e8');
    scene.fog = new THREE.Fog(0xa5c6ea, 42, 84);

    // ---- sun & moon sprites ----
    const mkSprite = (px, scale) => {
      const tex = new THREE.CanvasTexture(px.toCanvas());
      tex.magFilter = THREE.NearestFilter;
      tex.minFilter = THREE.NearestFilter;
      tex.generateMipmaps = false;
      tex.colorSpace = THREE.SRGBColorSpace;
      const m = new THREE.SpriteMaterial({ map: tex, fog: false, transparent: true, depthWrite: false });
      const s = new THREE.Sprite(m);
      s.scale.setScalar(scale);
      return s;
    };
    this.sun = mkSprite(sunPx(), 34);
    this.moon = mkSprite(moonPx(), 24);
    scene.add(this.sun);
    scene.add(this.moon);

    // ---- stars ----
    const starPos = [];
    for (let i = 0; i < 420; i++) {
      const a = Math.random() * Math.PI * 2;
      const b = Math.acos(Math.random() * 2 - 1);
      const r = 400;
      starPos.push(r * Math.cos(a) * Math.sin(b), Math.abs(r * Math.cos(b)) * (Math.random() < 0.5 ? 1 : 0.35), r * Math.sin(a) * Math.sin(b));
    }
    const sg = new THREE.BufferGeometry();
    sg.setAttribute('position', new THREE.Float32BufferAttribute(starPos, 3));
    this.starMat = new THREE.PointsMaterial({
      color: 0xdfe8ff, size: 1.8, sizeAttenuation: false,
      transparent: true, opacity: 0, fog: false, depthWrite: false,
    });
    this.stars = new THREE.Points(sg, this.starMat);
    scene.add(this.stars);

    // ---- clouds ----
    this.clouds = [];
    this.cloudMat = new THREE.MeshBasicMaterial({
      color: 0xffffff, transparent: true, opacity: 0.5, depthWrite: false,
    });
    for (let i = 0; i < 26; i++) {
      const w = 10 + Math.random() * 16, d = 6 + Math.random() * 10;
      const g = new THREE.BoxGeometry(w, 1.4, d);
      const m = new THREE.Mesh(g, this.cloudMat);
      m.position.set((Math.random() - 0.5) * 360, 76 + Math.random() * 7, (Math.random() - 0.5) * 360);
      scene.add(m);
      this.clouds.push(m);
    }
  }

  update(dt, playerPos, underwater, viewFar = 84) {
    this.time = (this.time + dt / DAY_LEN) % 1;
    const elev = Math.sin(this.time * Math.PI * 2);
    const horiz = Math.cos(this.time * Math.PI * 2);
    this.dayFactor = smoothstep(-0.08, 0.22, elev);
    this.nightFactor = 1 - this.dayFactor;

    // sky + fog color
    const c = this._color.copy(this.skyNight).lerp(this.skyDay, this.dayFactor);
    const duskAmt = Math.max(0, 1 - Math.abs(elev) / 0.26) * 0.75;
    c.lerp(this.skyDusk, duskAmt);
    let fogNear = viewFar * 0.5, fogFar = viewFar * 0.98;
    const f = this._color2.copy(this.fogNight).lerp(this.fogDay, this.dayFactor);
    f.lerp(this.fogDusk, duskAmt);
    if (underwater) {
      f.copy(this.waterFog).multiplyScalar(0.35 + 0.65 * this.dayFactor);
      fogNear = 2; fogFar = 22;
    }
    this.scene.background.copy(c);
    this.scene.fog.color.copy(f);
    this.scene.fog.near = fogNear;
    this.scene.fog.far = fogFar;

    // sun & moon orbit (east-west)
    const R = 330;
    this.sun.position.set(playerPos.x + horiz * R, playerPos.y + elev * R, playerPos.z - 90);
    this.moon.position.set(playerPos.x - horiz * R, playerPos.y - elev * R, playerPos.z - 90);
    this.sun.material.opacity = clamp01(elev + 0.25);
    this.moon.material.opacity = clamp01(-elev + 0.25);

    // stars
    this.stars.position.set(playerPos.x, 0, playerPos.z);
    this.stars.rotation.y += dt * 0.006;
    this.starMat.opacity = this.nightFactor * 0.9;

    // clouds drift
    this.cloudMat.color.setScalar(0.45 + 0.55 * this.dayFactor);
    for (const cl of this.clouds) {
      cl.position.x += dt * 1.4;
      if (cl.position.x > playerPos.x + 200) cl.position.x -= 400;
      if (cl.position.x < playerPos.x - 200) cl.position.x += 400;
      if (cl.position.z > playerPos.z + 200) cl.position.z -= 400;
      if (cl.position.z < playerPos.z - 200) cl.position.z += 400;
    }
  }

  // multiply into world materials for the global day/night brightness
  worldBrightness() {
    return 0.34 + 0.66 * this.dayFactor;
  }
}
