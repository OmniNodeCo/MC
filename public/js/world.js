// Voxelheim — world: terrain generation, chunk meshing, raycasting, physics.
import * as THREE from '../vendor/three.module.js';
import { Simplex, fbm2, fbm3, hash2, hash3, clamp, clamp01, smoothstep } from './noise.js';
import { B, BLOCKS, tileFor } from './blocks.js';
import { tileUV } from './textures.js';

export const CS = 16;    // chunk size (x,z)
export const CH = 80;    // world height
export const SEA = 30;   // sea level

// face tables: dir + 4 corners (x,y,z,u,v). indices (0,1,2),(2,1,3)
const FACES = [
  { dir: [-1, 0, 0], shade: 0.62, corners: [[0, 1, 0, 0, 1], [0, 0, 0, 0, 0], [0, 1, 1, 1, 1], [0, 0, 1, 1, 0]] },
  { dir: [1, 0, 0],  shade: 0.62, corners: [[1, 1, 1, 0, 1], [1, 0, 1, 0, 0], [1, 1, 0, 1, 1], [1, 0, 0, 1, 0]] },
  { dir: [0, -1, 0], shade: 0.50, corners: [[1, 0, 1, 1, 0], [0, 0, 1, 0, 0], [1, 0, 0, 1, 1], [0, 0, 0, 0, 1]] },
  { dir: [0, 1, 0],  shade: 1.00, corners: [[0, 1, 1, 1, 1], [1, 1, 1, 0, 1], [0, 1, 0, 1, 0], [1, 1, 0, 0, 0]] },
  { dir: [0, 0, -1], shade: 0.78, corners: [[1, 0, 0, 0, 0], [0, 0, 0, 1, 0], [1, 1, 0, 0, 1], [0, 1, 0, 1, 1]] },
  { dir: [0, 0, 1],  shade: 0.78, corners: [[0, 0, 1, 0, 0], [1, 0, 1, 1, 0], [0, 1, 1, 0, 1], [1, 1, 1, 1, 1]] },
];
const AO_B = [1.0, 0.72, 0.56, 0.42];

function makeBucket() { return { pos: [], col: [], uv: [], idx: [], n: 0 }; }

function pushQuad(bk, verts, uvs, cols, flip = false) {
  const base = bk.n;
  for (let i = 0; i < 4; i++) {
    bk.pos.push(verts[i][0], verts[i][1], verts[i][2]);
    bk.uv.push(uvs[i][0], uvs[i][1]);
    const c = cols[i];
    bk.col.push(c, c, c);
  }
  if (flip) bk.idx.push(base, base + 1, base + 3, base, base + 3, base + 2);
  else bk.idx.push(base, base + 1, base + 2, base + 2, base + 1, base + 3);
  bk.n += 4;
}

function bucketToGeometry(bk) {
  if (bk.n === 0) return null;
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(bk.pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(bk.col, 3));
  g.setAttribute('uv', new THREE.Float32BufferAttribute(bk.uv, 2));
  g.setIndex(bk.idx);
  g.computeBoundingSphere();
  return g;
}

// ------------------------------------------------------------------ World
export class World {
  constructor(seed) {
    this.seed = seed | 0;
    this.n = new Simplex(this.seed);
    this.n2 = new Simplex(this.seed ^ 0x9E3779B9);
    this.n3 = new Simplex(this.seed ^ 0x85EBCA6B);
    this.chunks = new Map();       // "cx,cz" -> { blocks: Uint8Array, dirty, meshes }
    this.edits = new Map();        // "cx,cz" -> Map(idx -> blockId)
    this.colCache = new Map();
    this.group = new THREE.Group();
    this.group.matrixAutoUpdate = false;
    this.dirtyQueue = new Set();
  }

  key(cx, cz) { return cx + ',' + cz; }

  // ------------------------------------------------------------ terrain shape
  columnInfo(wx, wz) {
    const k = wx + ',' + wz;
    const cached = this.colCache.get(k);
    if (cached) return cached;
    const cont = fbm2(this.n, wx * 0.0016, wz * 0.0016, 4);
    const hills = fbm2(this.n, wx * 0.009, wz * 0.009, 3);
    const mm = fbm2(this.n2, wx * 0.0035, wz * 0.0035, 2) * 0.5 + 0.5;
    const mtn = smoothstep(0.56, 0.82, mm);
    const ridge = 1 - Math.abs(fbm2(this.n2, wx * 0.011, wz * 0.011, 3));
    let h = 26 + cont * 15 + hills * 7 + mtn * ridge * 27;
    h = Math.floor(clamp(h, 6, CH - 10));

    let temp = fbm2(this.n2, wx * 0.0011 + 500, wz * 0.0011 + 500, 2);
    temp -= Math.max(0, h - 32) * 0.006;                 // altitude cools
    const moist = fbm2(this.n3, wx * 0.0013 - 800, wz * 0.0013 + 300, 2);

    let biome;
    if (h < SEA - 1) biome = 'ocean';
    else if (mtn > 0.4 && h > 50) biome = 'mountain';
    else if (h <= SEA + 1) biome = 'beach';
    else if (temp > 0.32 && moist < 0.05) biome = 'desert';
    else if (temp < -0.38) biome = 'snow';
    else if (moist > 0.12) biome = 'forest';
    else biome = 'plains';

    const info = { h, biome, mtn };
    if (this.colCache.size > 60000) this.colCache.clear();
    this.colCache.set(k, info);
    return info;
  }

  carveAt(wx, y, wz, h) {
    if (y < 4) return false;
    if (h <= SEA + 1 && y > h - 7) return false;          // keep ocean floors sealed
    const a = this.n.noise3(wx * 0.052, y * 0.078, wz * 0.052);
    const b = this.n2.noise3(wx * 0.052 + 31.7, y * 0.078, wz * 0.052 + 11.3);
    if (a * a + b * b < 0.0105) return true;              // spaghetti tunnels
    if (y < 26) {
      const c = fbm3(this.n3, wx * 0.031, y * 0.05, wz * 0.031, 2);
      if (c > 0.56) return true;                          // caverns
    }
    return false;
  }

  topBlockFor(info) {
    const { h, biome } = info;
    if (biome === 'ocean') return h > SEA - 7 ? B.SAND : B.STONE;
    if (biome === 'beach' || biome === 'desert') return B.SAND;
    if (biome === 'mountain') return h > 60 ? B.SNOW : B.STONE;
    if (biome === 'snow') return B.SNOWGRASS;
    return B.GRASS;
  }

  // ------------------------------------------------------------ chunk data
  ensureData(cx, cz) {
    const k = this.key(cx, cz);
    let c = this.chunks.get(k);
    if (c) return c;
    const blocks = new Uint8Array(CS * CS * CH);
    const x0 = cx * CS, z0 = cz * CS;

    for (let lz = 0; lz < CS; lz++) {
      for (let lx = 0; lx < CS; lx++) {
        const wx = x0 + lx, wz = z0 + lz;
        const info = this.columnInfo(wx, wz);
        const h = info.h;
        const top = this.topBlockFor(info);
        const subsurface = (info.biome === 'desert' || info.biome === 'beach' || info.biome === 'ocean')
          ? B.SAND : (info.biome === 'mountain' ? B.STONE : B.DIRT);

        for (let y = 0; y < CH; y++) {
          let id = B.AIR;
          if (y === 0) id = B.BEDROCK;
          else if (y <= 2 && hash3(wx, y, wz, this.seed ^ 0xBED) < 0.5) id = B.BEDROCK;
          else if (y <= h) {
            if (this.carveAt(wx, y, wz, h)) { id = B.AIR; }
            else if (y === h) id = top;
            else if (y > h - 4) id = subsurface;
            else {
              id = B.STONE;
              const r = hash3(wx, y, wz, this.seed ^ 0x0BE);
              if (r < 0.0035 && y < 13) id = B.CRYSTAL_ORE;
              else if (r < 0.0085 && y < 20) id = B.GOLD_ORE;
              else if (r < 0.0175 && y < 34) id = B.IRON_ORE;
              else if (r < 0.0305 && y < 52) id = B.COAL_ORE;
            }
          } else if (y <= SEA) id = B.WATER;
          blocks[lx + lz * CS + y * CS * CS] = id;
        }
      }
    }

    // ---- decorations (trees, cacti, plants, crystal spires) ----
    // extended loop so trees spill across chunk borders deterministically
    for (let tz = -3; tz < CS + 3; tz++) {
      for (let tx = -3; tx < CS + 3; tx++) {
        const wx = x0 + tx, wz = z0 + tz;
        const info = this.columnInfo(wx, wz);
        const h = info.h;
        const biome = info.biome;
        const inside = tx >= 0 && tx < CS && tz >= 0 && tz < CS;
        const put = (x, y, z, id, overwriteLeaves = false) => {
          const lx = x - x0, lz = z - z0;
          if (lx < 0 || lx >= CS || lz < 0 || lz >= CS || y < 0 || y >= CH) return;
          const i = lx + lz * CS + y * CS * CS;
          const cur = blocks[i];
          if (cur === B.AIR || (overwriteLeaves && cur === B.LEAVES)) blocks[i] = id;
        };
        const onSurface = h > SEA && !this.carveAt(wx, h, wz, h);

        const r = hash2(wx, wz, this.seed ^ 0x7EE);
        if (onSurface && (biome === 'forest' || biome === 'plains' || biome === 'snow')) {
          const treeP = biome === 'forest' ? 0.045 : biome === 'plains' ? 0.008 : 0.02;
          if (r < treeP) {
            if (biome === 'snow') {
              // spruce: tall trunk + conical diamond canopy
              const th = 6 + Math.floor(hash2(wx, wz, this.seed ^ 0x77) * 3);
              for (let y = h + 1; y <= h + th; y++) put(wx, y, wz, B.LOG, true);
              const layers = [[th, 1], [th - 1, 2], [th - 2, 1], [th - 3, 2], [th - 4, 1], [2, 1]];
              for (const [dy, rr] of layers) {
                if (dy < 2) continue;
                for (let dx = -rr; dx <= rr; dx++) for (let dz = -rr; dz <= rr; dz++) {
                  if (Math.abs(dx) + Math.abs(dz) > rr) continue;
                  if (dx === 0 && dz === 0 && dy <= th) continue;
                  put(wx + dx, h + dy + 1, wz + dz, B.LEAVES);
                }
              }
              put(wx, h + th + 2, wz, B.LEAVES);
            } else {
              // oak: trunk + blob canopy
              const th = 4 + Math.floor(hash2(wx, wz, this.seed ^ 0x77) * 3);
              for (let y = h + 1; y <= h + th; y++) put(wx, y, wz, B.LOG, true);
              for (let dy = th - 2; dy <= th + 1; dy++) {
                const rr = dy >= th ? 1 : 2;
                for (let dx = -rr; dx <= rr; dx++) for (let dz = -rr; dz <= rr; dz++) {
                  if (dx === 0 && dz === 0 && dy <= th) continue;
                  const corner = Math.abs(dx) === rr && Math.abs(dz) === rr;
                  if (corner && hash3(wx + dx, h + dy, wz + dz, this.seed ^ 0x99) < 0.55) continue;
                  put(wx + dx, h + dy + 1, wz + dz, B.LEAVES);
                }
              }
            }
            continue;
          }
        }

        if (!inside) continue;
        // single-column decorations
        const topId = blocks[tx + tz * CS + h * CS * CS];
        const above = h + 1 < CH ? blocks[tx + tz * CS + (h + 1) * CS * CS] : B.AIR;
        if (above !== B.AIR) continue;

        const r2 = hash2(wx, wz, this.seed ^ 0xF10);
        if ((biome === 'plains' || biome === 'forest') && topId === B.GRASS) {
          const gp = biome === 'plains' ? 0.06 : 0.1;
          if (r2 < gp) { blocks[tx + tz * CS + (h + 1) * CS * CS] = B.TALLGRASS; continue; }
          if (r2 < gp + 0.012) {
            blocks[tx + tz * CS + (h + 1) * CS * CS] =
              hash2(wx, wz, this.seed ^ 0xF11) < 0.5 ? B.FLOWER_RED : B.FLOWER_YELLOW;
            continue;
          }
        }
        if (biome === 'desert' && topId === B.SAND && r2 < 0.006) {
          const ch2 = 2 + Math.floor(hash2(wx, wz, this.seed ^ 0xCA) * 2);
          for (let y = 1; y <= ch2; y++) put(wx, h + y, wz, B.CACTUS);
          continue;
        }
        if (biome === 'mountain' && topId === B.STONE && r > 0.9986) {
          const sp = 3 + Math.floor(hash2(wx, wz, this.seed ^ 0xC12) * 3);
          for (let y = 1; y <= sp; y++) put(wx, h + y, wz, B.CRYSTAL);
        }
      }
    }

    // ---- player edits ----
    const ed = this.edits.get(k);
    if (ed) for (const [i, id] of ed) blocks[i] = id;

    c = { blocks, dirty: true, cx, cz };
    this.chunks.set(k, c);
    this.dirtyQueue.add(k);
    return c;
  }

  getBlock(x, y, z) {
    if (y < 0) return B.BEDROCK;
    if (y >= CH) return B.AIR;
    const cx = x >> 4, cz = z >> 4;
    let c = this.chunks.get(this.key(cx, cz));
    if (!c) c = this.ensureData(cx, cz);
    return c.blocks[(x & 15) + (z & 15) * CS + y * CS * CS];
  }

  setBlock(x, y, z, id) {
    if (y < 1 || y >= CH) return [];
    const cx = x >> 4, cz = z >> 4;
    const k = this.key(cx, cz);
    const c = this.ensureData(cx, cz);
    const lx = x & 15, lz = z & 15;
    const i = lx + lz * CS + y * CS * CS;
    c.blocks[i] = id;
    let ed = this.edits.get(k);
    if (!ed) { ed = new Map(); this.edits.set(k, ed); }
    ed.set(i, id);
    c.dirty = true;

    const affected = [k];
    const mark = (ax, az) => {
      const kk = this.key(ax, az);
      const cc = this.chunks.get(kk);
      if (cc) { cc.dirty = true; affected.push(kk); this.dirtyQueue.add(kk); }
    };
    if (lx === 0) mark(cx - 1, cz);
    if (lx === 15) mark(cx + 1, cz);
    if (lz === 0) mark(cx, cz - 1);
    if (lz === 15) mark(cx, cz + 1);
    if (lx === 0 && lz === 0) mark(cx - 1, cz - 1);
    if (lx === 0 && lz === 15) mark(cx - 1, cz + 1);
    if (lx === 15 && lz === 0) mark(cx + 1, cz - 1);
    if (lx === 15 && lz === 15) mark(cx + 1, cz + 1);
    this.dirtyQueue.add(k);
    return affected;
  }

  surfaceY(x, z) {
    const cx = x >> 4, cz = z >> 4;
    const c = this.ensureData(cx, cz);
    for (let y = CH - 1; y >= 0; y--) {
      const id = c.blocks[(x & 15) + (z & 15) * CS + y * CS * CS];
      if (id !== B.AIR && id !== B.WATER && !BLOCKS[id].cross) return y;
    }
    return 0;
  }

  biomeAt(x, z) { return this.columnInfo(x, z).biome; }

  // ------------------------------------------------------------ meshing
  disposeChunkMeshes(c) {
    if (!c.meshes) return;
    for (const m of c.meshes) {
      this.group.remove(m);
      m.geometry.dispose();
    }
    c.meshes = null;
  }

  buildChunkMesh(cx, cz) {
    const k = this.key(cx, cz);
    const c = this.chunks.get(k) || this.ensureData(cx, cz);
    // neighbors needed for border faces + AO
    for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) this.ensureData(cx + dx, cz + dz);
    this.disposeChunkMeshes(c);

    const x0 = cx * CS, z0 = cz * CS;
    const bkOpaque = makeBucket(), bkCutout = makeBucket(), bkWater = makeBucket(), bkGlow = makeBucket();
    const gb = (x, y, z) => this.getBlock(x, y, z);
    const opaqueAt = (x, y, z) => BLOCKS[this.getBlock(x, y, z)].opaque === 1;

    for (let y = 0; y < CH; y++) {
      for (let lz = 0; lz < CS; lz++) {
        for (let lx = 0; lx < CS; lx++) {
          const id = c.blocks[lx + lz * CS + y * CS * CS];
          if (id === B.AIR) continue;
          const bdef = BLOCKS[id];
          const wx = x0 + lx, wz = z0 + lz;

          if (bdef.cross) {
            const t = tileUV(bdef.side);
            const jx = (hash2(wx, wz, 7) - 0.5) * 0.24, jz = (hash2(wx, wz, 13) - 0.5) * 0.24;
            const px = wx + 0.5 + jx, pz = wz + 0.5 + jz;
            const r = 0.42;
            const q = [
              [[px - r, y, pz - r], [px + r, y, pz + r], [px + r, y + 1, pz + r], [px - r, y + 1, pz - r]],
              [[px + r, y, pz - r], [px - r, y, pz + r], [px - r, y + 1, pz + r], [px + r, y + 1, pz - r]],
            ];
            const uvs = [[t[0], t[1]], [t[2], t[1]], [t[2], t[3]], [t[0], t[3]]];
            for (const qq of q) pushQuad(bkCutout, qq, uvs, [0.95, 0.95, 0.95, 0.95]);
            continue;
          }

          if (bdef.liquid) {
            const above = gb(wx, y + 1, wz);
            const yt = above === B.WATER ? 1 : 0.875;
            for (const f of FACES) {
              const nb = gb(wx + f.dir[0], y + f.dir[1], wz + f.dir[2]);
              const ndef = BLOCKS[nb];
              if (nb === B.WATER) continue;
              if (ndef.opaque) continue;
              if (f.dir[1] === 1 && above === B.WATER) continue;
              if (f.dir[1] === -1 && nb === B.AIR && y > 0) { /* draw cave undersides */ }
              else if (f.dir[1] !== 1 && f.dir[1] !== -1 && nb !== B.AIR && !ndef.cross && !ndef.cutout && !ndef.liquid) continue;
              if (f.dir[1] === -1 && nb === B.AIR && y === 0) continue;
              const verts = [], uvs = [], cols = [];
              for (const cn of f.corners) {
                let vx = wx + cn[0], vy = y + cn[1], vz = wz + cn[2];
                if (cn[1] === 1) vy = y + yt;
                let u, v;
                if (f.dir[1] !== 0) { u = (f.dir[1] > 0 ? vx : vx) * 0.25; v = vz * 0.25; }
                else if (f.dir[0] !== 0) { u = vz * 0.25; v = vy * 0.25; }
                else { u = vx * 0.25; v = vy * 0.25; }
                verts.push([vx, vy, vz]);
                uvs.push([u, v]);
                cols.push(f.dir[1] === 1 ? 1 : f.dir[1] === -1 ? 0.6 : 0.8);
              }
              pushQuad(bkWater, verts, uvs, cols);
            }
            continue;
          }

          // solid block
          for (const f of FACES) {
            const nx = wx + f.dir[0], ny = y + f.dir[1], nz = wz + f.dir[2];
            const nb = gb(nx, ny, nz);
            const ndef = BLOCKS[nb];
            let draw;
            if (nb === B.AIR) draw = true;
            else if (ndef.opaque) draw = false;
            else if (nb === id) draw = false;              // glass-next-to-glass
            else draw = true;                              // water / cross / cutout
            if (!draw) continue;

            const face = f.dir[1] > 0 ? 'top' : f.dir[1] < 0 ? 'bottom' : 'side';
            const t = tileUV(tileFor(id, face));
            const axis = f.dir[0] !== 0 ? 0 : f.dir[1] !== 0 ? 1 : 2;
            const t1 = axis === 0 ? 1 : 0, t2 = axis === 2 ? 1 : 2;
            const verts = [], uvs = [], cols = [], aos = [];
            for (const cn of f.corners) {
              verts.push([wx + cn[0], y + cn[1], wz + cn[2]]);
              uvs.push([t[0] + cn[3] * (t[2] - t[0]), t[1] + cn[4] * (t[3] - t[1])]);
              // ambient occlusion
              const p = [nx, ny, nz];
              const s1 = cn[t1] === 1 ? 1 : -1, s2 = cn[t2] === 1 ? 1 : -1;
              const q1 = [...p]; q1[t1] += s1;
              const q2 = [...p]; q2[t2] += s2;
              const qc = [...p]; qc[t1] += s1; qc[t2] += s2;
              const o1 = opaqueAt(q1[0], q1[1], q1[2]) ? 1 : 0;
              const o2 = opaqueAt(q2[0], q2[1], q2[2]) ? 1 : 0;
              const oc = opaqueAt(qc[0], qc[1], qc[2]) ? 1 : 0;
              const ao = (o1 && o2) ? 3 : o1 + o2 + oc;
              aos.push(ao);
              cols.push(f.shade * AO_B[ao]);
            }
            const flip = aos[0] + aos[3] > aos[1] + aos[2];
            const bk = bdef.glow ? bkGlow : bdef.cutout ? bkCutout : bkOpaque;
            pushQuad(bk, verts, uvs, cols, flip);
          }
        }
      }
    }

    const meshes = [];
    const mk = (bk, mat, opts = {}) => {
      const g = bucketToGeometry(bk);
      if (!g) return;
      const m = new THREE.Mesh(g, mat);
      m.matrixAutoUpdate = false;
      if (opts.renderOrder) m.renderOrder = opts.renderOrder;
      this.group.add(m);
      meshes.push(m);
    };
    mk(bkOpaque, this.mats?.opaque || null);
    mk(bkCutout, this.mats?.cutout || null);
    mk(bkGlow, this.mats?.glow || null);
    mk(bkWater, this.mats?.water || null, { renderOrder: 2 });

    // meshes need materials — main.js assigns world.mats before first build
    c.meshes = meshes;
    c.dirty = false;
    this.dirtyQueue.delete(k);
    return meshes;
  }

  // ------------------------------------------------------------ raycast (DDA)
  raycast(ox, oy, oz, dx, dy, dz, maxDist) {
    let x = Math.floor(ox), y = Math.floor(oy), z = Math.floor(oz);
    const stepX = dx >= 0 ? 1 : -1, stepY = dy >= 0 ? 1 : -1, stepZ = dz >= 0 ? 1 : -1;
    const tdx = dx !== 0 ? Math.abs(1 / dx) : Infinity;
    const tdy = dy !== 0 ? Math.abs(1 / dy) : Infinity;
    const tdz = dz !== 0 ? Math.abs(1 / dz) : Infinity;
    let tmx = dx !== 0 ? (stepX > 0 ? x + 1 - ox : ox - x) * tdx : Infinity;
    let tmy = dy !== 0 ? (stepY > 0 ? y + 1 - oy : oy - y) * tdy : Infinity;
    let tmz = dz !== 0 ? (stepZ > 0 ? z + 1 - oz : oz - z) * tdz : Infinity;
    let nx = 0, ny = 0, nz = 0, t = 0;
    for (let i = 0; i < 512; i++) {
      const id = this.getBlock(x, y, z);
      if (id !== B.AIR && !BLOCKS[id].liquid) return { x, y, z, nx, ny, nz, dist: t, id };
      if (tmx < tmy && tmx < tmz) {
        x += stepX; t = tmx; tmx += tdx; nx = -stepX; ny = 0; nz = 0;
      } else if (tmy < tmz) {
        y += stepY; t = tmy; tmy += tdy; ny = -stepY; nx = 0; nz = 0;
      } else {
        z += stepZ; t = tmz; tmz += tdz; nz = -stepZ; nx = 0; ny = 0;
      }
      if (t > maxDist) return null;
      if (y < -2 || y > CH + 2) return null;
    }
    return null;
  }
}

// ------------------------------------------------------------- shared physics
// pos = feet-center, half = half-width, height = full height. Returns onGround.
export function collideMove(world, pos, vel, dt, half, height) {
  const EPS = 0.001;
  let onGround = false;
  const maxV = Math.max(Math.abs(vel.x), Math.abs(vel.y), Math.abs(vel.z));
  const steps = Math.min(8, Math.max(1, Math.ceil((maxV * dt) / 0.4)));
  const sdt = dt / steps;

  const resolve = (axis) => {
    const x0 = Math.floor(pos.x - half + EPS), x1 = Math.floor(pos.x + half - EPS);
    const y0 = Math.floor(pos.y + EPS), y1 = Math.floor(pos.y + height - EPS);
    const z0 = Math.floor(pos.z - half + EPS), z1 = Math.floor(pos.z + half - EPS);
    for (let by = y0; by <= y1; by++) {
      for (let bz = z0; bz <= z1; bz++) {
        for (let bx = x0; bx <= x1; bx++) {
          const id = world.getBlock(bx, by, bz);
          if (!BLOCKS[id].solid) continue;
          if (axis === 0) {
            if (vel.x > 0) pos.x = bx - half - EPS;
            else if (vel.x < 0) pos.x = bx + 1 + half + EPS;
            vel.x = 0;
          } else if (axis === 1) {
            if (vel.y < 0) { pos.y = by + 1 + EPS; onGround = true; }
            else if (vel.y > 0) pos.y = by - height - EPS;
            vel.y = 0;
          } else {
            if (vel.z > 0) pos.z = bz - half - EPS;
            else if (vel.z < 0) pos.z = bz + 1 + half + EPS;
            vel.z = 0;
          }
          return;
        }
      }
    }
  };

  for (let s = 0; s < steps; s++) {
    pos.x += vel.x * sdt; resolve(0);
    pos.z += vel.z * sdt; resolve(2);
    pos.y += vel.y * sdt; resolve(1);
  }
  return onGround;
}
