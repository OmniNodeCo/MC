// Voxelheim — hand-coded pixel-art textures.
// Every tile is painted procedurally with a seeded RNG and tuned palettes.
// No image assets, no AI generation — just careful pixel work.

import { hash2, mulberry32, clamp01, lerp } from './noise.js';

export const TILE = 16, COLS = 8, ROWS = 4;
export const ATLAS_W = COLS * TILE, ATLAS_H = ROWS * TILE;

export const TILES = {
  GRASS_TOP: 0, GRASS_SIDE: 1, DIRT: 2, STONE: 3, COBBLE: 4, SAND: 5,
  LOG_SIDE: 6, LOG_TOP: 7, PLANKS: 8, LEAVES: 9, SNOW: 10, SNOW_SIDE: 11,
  COAL_ORE: 12, IRON_ORE: 13, GOLD_ORE: 14, CRYSTAL_ORE: 15, CRYSTAL: 16,
  LANTERN: 17, GLASS: 18, BRICK: 19, BEDROCK: 20, TALLGRASS: 21,
  FLOWER_RED: 22, FLOWER_YELLOW: 23, CACTUS_SIDE: 24, CACTUS_TOP: 25,
};

// ---------------------------------------------------------------- pixel buffer
export class Px {
  constructor(w, h) {
    this.w = w; this.h = h;
    this.data = new Uint8ClampedArray(w * h * 4); // starts fully transparent
  }
  set(x, y, c, a = 255) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = (y * this.w + x) * 4;
    this.data[i] = c[0]; this.data[i + 1] = c[1]; this.data[i + 2] = c[2]; this.data[i + 3] = a;
  }
  get(x, y) {
    const i = (y * this.w + x) * 4;
    return [this.data[i], this.data[i + 1], this.data[i + 2], this.data[i + 3]];
  }
  fillRect(x, y, w, h, c, a = 255) {
    for (let yy = y; yy < y + h; yy++) for (let xx = x; xx < x + w; xx++) this.set(xx, yy, c, a);
  }
  blit(src, ox, oy) {
    for (let y = 0; y < src.h; y++) for (let x = 0; x < src.w; x++) {
      const p = src.get(x, y);
      this.set(ox + x, oy + y, p, p[3]);
    }
  }
  clone() {
    const p = new Px(this.w, this.h);
    p.data.set(this.data);
    return p;
  }
  scaled(f) {
    const p = new Px(this.w * f, this.h * f);
    for (let y = 0; y < this.h; y++) for (let x = 0; x < this.w; x++) {
      const c = this.get(x, y);
      p.fillRect(x * f, y * f, f, f, c, c[3]);
    }
    return p;
  }
  toCanvas() {
    if (typeof document === 'undefined') throw new Error('pxToCanvas requires a DOM');
    const cv = document.createElement('canvas');
    cv.width = this.w; cv.height = this.h;
    const ctx = cv.getContext('2d');
    ctx.putImageData(new ImageData(new Uint8ClampedArray(this.data), this.w, this.h), 0, 0);
    return cv;
  }
}

const hex = (s) => [parseInt(s.slice(1, 3), 16), parseInt(s.slice(3, 5), 16), parseInt(s.slice(5, 7), 16)];
const mul = (c, f) => [Math.min(255, c[0] * f), Math.min(255, c[1] * f), Math.min(255, c[2] * f)];
const mixC = (a, b, t) => [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)];

// smooth value noise on an integer lattice — gives painterly clumps, not TV static
function vnoise(x, y, seed) {
  const xi = Math.floor(x), yi = Math.floor(y);
  const xf = x - xi, yf = y - yi;
  const sx = xf * xf * (3 - 2 * xf), sy = yf * yf * (3 - 2 * yf);
  const a = hash2(xi, yi, seed), b = hash2(xi + 1, yi, seed);
  const c = hash2(xi, yi + 1, seed), d = hash2(xi + 1, yi + 1, seed);
  return lerp(lerp(a, b, sx), lerp(c, d, sx), sy);
}

function pick(tones, v) {
  return tones[Math.min(tones.length - 1, Math.floor(clamp01(v) * tones.length))];
}

function base(px, tones, seed, scale = 3.2, jitter = 0.16, fn = null) {
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) {
    let n = vnoise(x / scale, y / scale, seed) + (hash2(x, y, seed ^ 0x5bd1) - 0.5) * jitter;
    let c = pick(tones, n);
    if (fn) c = fn(x, y, c, n);
    px.set(x, y, c);
  }
}

function speckle(px, color, seed, p, fLo = 0.88, fHi = 1.1) {
  for (let y = 0; y < px.h; y++) for (let x = 0; x < px.w; x++) {
    if (hash2(x, y, seed) < p) {
      const f = fLo + hash2(x, y, seed ^ 77) * (fHi - fLo);
      px.set(x, y, mul(color, f));
    }
  }
}

// subtle bevel: lighter top row, darker bottom rows (for side tiles)
function bevel(px) {
  for (let x = 0; x < px.w; x++) {
    px.set(x, 0, mul(px.get(x, 0), 1.1));
    px.set(x, px.h - 1, mul(px.get(x, px.h - 1), 0.82));
    px.set(x, px.h - 2, mul(px.get(x, px.h - 2), 0.92));
  }
}

// ---------------------------------------------------------------- palettes
const P = {
  grass: ['#357a28', '#3f8f2f', '#4aa536', '#57b83f'].map(hex),
  grassLight: hex('#7fd95c'),
  dirt: ['#6e4d33', '#7a5638', '#8a6242', '#966f4d'].map(hex),
  dirtDark: hex('#5a3f2c'), dirtLight: hex('#a5825e'),
  stone: ['#707070', '#7d7d7d', '#8b8b8b', '#969696'].map(hex),
  stoneDark: hex('#5c5c5c'), stoneLight: hex('#a3a3a3'),
  sand: ['#c7ba85', '#d1c493', '#dbcfa1', '#e3d8ab'].map(hex),
  bark: ['#5d4429', '#6b4f31', '#77592f'].map(hex),
  barkDark: hex('#4a3720'), barkKnot: hex('#3e2d1b'),
  rings: ['#6e5230', '#8a663d', '#96703f', '#a57c4f'].map(hex),
  plank: ['#8a663d', '#9c7442', '#a87e4c', '#b28754'].map(hex),
  plankSeam: hex('#6e5230'),
  leaf: ['#27571d', '#2f6b23', '#3a7d2c', '#458e35'].map(hex),
  leafDark: hex('#1e4416'), leafLight: hex('#5fb04a'),
  snow: ['#d8e1ec', '#e2e9f2', '#eef3f8', '#f6fafd'].map(hex),
  snowShadow: hex('#c3cfdd'),
  crystal: ['#1f9a90', '#26b1a6', '#2ec7bb', '#3fe0d2'].map(hex),
  crystalLight: hex('#8ff5ea'), crystalSpark: hex('#e9fffc'),
  lanternCore: ['#f19a36', '#ffb84d', '#ffdf8a', '#fff3c2'].map(hex),
  lanternFrame: hex('#5c3a1d'),
  glassFrame: hex('#dceef4'), glassDark: hex('#9fc3cf'),
  brickA: ['#8a4535', '#9a4f3c', '#a85a44'].map(hex),
  brickMortar: hex('#b0aca6'),
  bedrock: ['#2c2c2c', '#3a3a3a', '#4c4c4c', '#565656'].map(hex),
  plant: ['#3f8f2f', '#4aa536', '#57b83f'].map(hex),
  cactus: ['#267a31', '#2e8b3a', '#37a047'].map(hex),
  cactusRib: hex('#1f6428'), cactusSpine: hex('#dfe8d8'),
  coalGem: [hex('#23232a'), hex('#2e2e36'), hex('#45454d')],
  ironGem: [hex('#a87f62'), hex('#c99a7c'), hex('#e0b898')],
  goldGem: [hex('#d9a325'), hex('#f3c33c'), hex('#ffe98a')],
  crystalGem: [hex('#1fb3a8'), hex('#37e2d4'), hex('#aefcf4')],
};

// ---------------------------------------------------------------- painters
function paintStone(px, rng, seed) {
  base(px, P.stone, seed, 4.2, 0.14);
  speckle(px, P.stoneDark, seed ^ 11, 0.06, 0.85, 1.0);
  speckle(px, P.stoneLight, seed ^ 22, 0.05, 1.02, 1.12);
  // cracks: a couple of short random walks
  for (let k = 0; k < 2; k++) {
    let x = Math.floor(rng() * 14) + 1, y = Math.floor(rng() * 12) + 2;
    const steps = 3 + Math.floor(rng() * 4);
    for (let s = 0; s < steps; s++) {
      px.set(x, y, P.stoneDark);
      if (rng() < 0.6) px.set(x + 1, y, mul(P.stoneDark, 0.85));
      x += rng() < 0.5 ? 1 : rng() < 0.5 ? -1 : 0;
      y += rng() < 0.7 ? 1 : 0;
      if (x < 0 || y < 0 || x > 15 || y > 15) break;
    }
  }
}

function paintDirt(px, rng, seed) {
  base(px, P.dirt, seed, 3.0, 0.18);
  speckle(px, P.dirtDark, seed ^ 31, 0.09, 0.85, 1.0);
  speckle(px, P.dirtLight, seed ^ 32, 0.05, 1.0, 1.12);
  bevel(px);
}

function gemBlobs(px, rng, gems) {
  const n = 3 + Math.floor(rng() * 2);
  for (let k = 0; k < n; k++) {
    const cx = 2 + Math.floor(rng() * 11), cy = 2 + Math.floor(rng() * 11);
    px.set(cx, cy, gems[1]);
    px.set(cx, cy - 1, gems[2]);           // bright top
    px.set(cx - 1, cy, gems[0]);           // dark sides
    px.set(cx + 1, cy, gems[0]);
    px.set(cx, cy + 1, gems[0]);
    if (rng() < 0.5) px.set(cx + 1, cy - 1, gems[1]);
    if (rng() < 0.5) px.set(cx - 1, cy + 1, mul(gems[0], 0.8));
  }
}

const PAINTERS = {
  [TILES.GRASS_TOP](px, rng, s) {
    base(px, P.grass, s, 3.4, 0.15);
    speckle(px, P.grassLight, s ^ 41, 0.07, 0.94, 1.06);
    speckle(px, mul(P.grass[0], 0.9), s ^ 42, 0.08);
  },
  [TILES.GRASS_SIDE](px, rng, s) {
    paintDirt(px, rng, s);
    // jagged grass fringe
    for (let x = 0; x < 16; x++) {
      const depth = 3 + Math.floor(hash2(x, 0, s ^ 77) * 2.6);
      for (let y = 0; y < depth; y++) {
        const t = y === depth - 1 ? P.grass[0] : pick(P.grass, 0.45 + hash2(x, y, s) * 0.5);
        px.set(x, y, y === 0 ? mul(t, 1.08) : t);
      }
      px.set(x, depth, mul(P.dirt[1], 0.72)); // shadow under fringe
    }
  },
  [TILES.DIRT]: paintDirt,
  [TILES.STONE]: paintStone,
  [TILES.COBBLE](px, rng, s) {
    // rounded stones on a mortar bed
    const cells = [];
    for (let gy = 0; gy < 3; gy++) for (let gx = 0; gx < 3; gx++) {
      cells.push({
        x: 2 + gx * 6 + Math.floor(rng() * 2) - 1,
        y: 2 + gy * 6 + Math.floor(rng() * 2) - 1,
        r: 2.4 + rng() * 1.1,
        t: 0.75 + rng() * 0.35,
      });
    }
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      let best = null, bd = 1e9;
      for (const c of cells) {
        const d = Math.hypot(x - c.x, y - c.y);
        if (d < bd) { bd = d; best = c; }
      }
      if (bd <= best.r) {
        let f = best.t * (0.9 + vnoise(x / 2.2, y / 2.2, s) * 0.25);
        if (bd > best.r - 1) f *= 0.72;            // stone edge
        if (bd < best.r - 1.6 && x <= best.x && y <= best.y) f *= 1.14; // highlight
        px.set(x, y, mul(P.stone[1 + (x + y) % 2], f));
      } else {
        px.set(x, y, mul(hex('#545454'), 0.9 + hash2(x, y, s) * 0.25));
      }
    }
  },
  [TILES.SAND](px, rng, s) {
    base(px, P.sand, s, 2.6, 0.14, (x, y, c, n) => mul(c, 0.97 + Math.sin(y * 0.9 + x * 0.2) * 0.03));
    speckle(px, hex('#b3a678'), s ^ 51, 0.07, 0.9, 1.0);
    speckle(px, hex('#eee3bb'), s ^ 52, 0.05, 1.0, 1.06);
  },
  [TILES.LOG_SIDE](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const stripe = hash2(x, 0, s);
      let c = pick(P.bark, stripe);
      const n = vnoise(x / 1.2, y / 4.5, s ^ 3);
      if (stripe < 0.18 && n > 0.3) c = P.barkDark;  // dark seams with breaks
      px.set(x, y, mul(c, 0.92 + n * 0.16));
    }
    // knot
    const kx = 3 + Math.floor(rng() * 9), ky = 4 + Math.floor(rng() * 8);
    px.set(kx, ky, P.barkKnot); px.set(kx + 1, ky, P.barkKnot);
    px.set(kx, ky + 1, mul(P.barkKnot, 1.3)); px.set(kx + 1, ky + 1, P.barkKnot);
    bevel(px);
  },
  [TILES.LOG_TOP](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const d = Math.hypot(x - 7.5, y - 7.5);
      let c;
      if (d > 6.6) c = pick(P.bark, 0.4 + hash2(x, y, s) * 0.4);        // bark ring
      else if (d > 1.2) c = pick(P.rings, ((Math.floor(d) % 2) * 0.5 + vnoise(x / 3, y / 3, s) * 0.4));
      else c = P.rings[0];
      px.set(x, y, mul(c, 0.94 + hash2(x, y, s ^ 9) * 0.1));
    }
  },
  [TILES.PLANKS](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const board = Math.floor(y / 4);
      let c = pick(P.plank, 0.3 + hash2(0, board, s ^ 5) * 0.6);
      const n = vnoise(x / 5, y / 1.4, s ^ 6);
      if (y % 4 === 3) c = P.plankSeam;                                 // horizontal seams
      else if (hash2(board, Math.floor(x / 8), s ^ 7) < 0.5 && x === (board % 2 === 0 ? 7 : 15) && y % 4 !== 3) c = mul(P.plankSeam, 1.1);
      px.set(x, y, mul(c, 0.93 + n * 0.14));
    }
    // grain flecks
    for (let k = 0; k < 14; k++) {
      const x = Math.floor(rng() * 16), y = Math.floor(rng() * 16);
      if (y % 4 !== 3) px.set(x, y, mul(P.plankSeam, 1.25));
    }
    bevel(px);
  },
  [TILES.LEAVES](px, rng, s) {
    base(px, P.leaf, s, 2.2, 0.2, (x, y, c, n) => (vnoise(x / 1.6, y / 1.6, s ^ 13) < 0.24 ? P.leafDark : c));
    speckle(px, P.leafLight, s ^ 14, 0.06, 0.96, 1.08);
  },
  [TILES.SNOW](px, rng, s) {
    base(px, P.snow, s, 3.0, 0.12);
    speckle(px, hex('#ffffff'), s ^ 61, 0.05, 1.0, 1.0);
    speckle(px, P.snowShadow, s ^ 62, 0.06, 0.94, 1.0);
  },
  [TILES.SNOW_SIDE](px, rng, s) {
    paintDirt(px, rng, s);
    for (let x = 0; x < 16; x++) {
      const depth = 4 + Math.floor(hash2(x, 0, s ^ 81) * 2.2);
      for (let y = 0; y < depth; y++) px.set(x, y, pick(P.snow, 0.4 + hash2(x, y, s) * 0.5));
      px.set(x, depth, mul(P.snowShadow, 0.85));
    }
  },
  [TILES.COAL_ORE](px, rng, s) { paintStone(px, rng, s); gemBlobs(px, rng, P.coalGem); },
  [TILES.IRON_ORE](px, rng, s) { paintStone(px, rng, s); gemBlobs(px, rng, P.ironGem); },
  [TILES.GOLD_ORE](px, rng, s) { paintStone(px, rng, s); gemBlobs(px, rng, P.goldGem); },
  [TILES.CRYSTAL_ORE](px, rng, s) { paintStone(px, rng, s); gemBlobs(px, rng, P.crystalGem); },
  [TILES.CRYSTAL](px, rng, s) {
    base(px, P.crystal, s, 2.6, 0.14);
    for (let k = 0; k < 2; k++) {  // facet streaks
      let x = Math.floor(rng() * 12) + 2, y = 2 + Math.floor(rng() * 4);
      for (let d = 0; d < 7; d++) { px.set(x, y, P.crystalLight); x += rng() < 0.7 ? 1 : 0; y += 1; if (x > 15 || y > 15) break; }
    }
    speckle(px, P.crystalSpark, s ^ 71, 0.04, 1, 1);
    bevel(px);
  },
  [TILES.LANTERN](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const d = Math.hypot(x - 7.5, y - 7.5) / 8;
      px.set(x, y, pick(P.lanternCore, clamp01(1 - d) * 0.8 + hash2(x, y, s) * 0.2));
    }
    // frame + corner rivets
    for (let i = 0; i < 16; i++) {
      px.set(i, 0, P.lanternFrame); px.set(i, 15, P.lanternFrame);
      px.set(0, i, P.lanternFrame); px.set(15, i, P.lanternFrame);
    }
    for (const [cx, cy] of [[2, 2], [13, 2], [2, 13], [13, 13]]) px.set(cx, cy, hex('#3e2a14'));
  },
  [TILES.GLASS](px, rng, s) {
    // mostly transparent with a crisp frame + diagonal streak
    for (let i = 0; i < 16; i++) {
      px.set(i, 0, P.glassFrame); px.set(i, 15, P.glassDark);
      px.set(0, i, P.glassFrame); px.set(15, i, P.glassDark);
    }
    px.set(0, 0, P.glassDark); px.set(15, 0, P.glassDark);
    px.set(0, 15, P.glassFrame); px.set(15, 15, P.glassFrame);
    for (let k = 0; k < 5; k++) {
      const x0 = 3 + k * 2, y0 = 13 - k * 2;
      px.set(x0, y0, hex('#ffffff'), 235);
      if (k < 3) px.set(x0 + 1, y0 - 1, hex('#eef7fa'), 200);
    }
    px.set(4, 3, hex('#ffffff'), 220); px.set(5, 2, hex('#eef7fa'), 180);
  },
  [TILES.BRICK](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const row = Math.floor(y / 4);
      const joint = (x + row * 4) % 8 === 0;
      const mortar = y % 4 === 3 || joint;
      if (mortar) px.set(x, y, mul(P.brickMortar, 0.85 + hash2(x, y, s) * 0.2));
      else px.set(x, y, mul(P.brickA[(x + y) % 3], 0.9 + vnoise(x / 2.5, y / 2.5, s) * 0.18));
    }
    bevel(px);
  },
  [TILES.BEDROCK](px, rng, s) {
    base(px, P.bedrock, s, 2.2, 0.3);
    speckle(px, hex('#222222'), s ^ 91, 0.12, 0.8, 1.0);
    speckle(px, hex('#6a6a6a'), s ^ 92, 0.08, 1.0, 1.1);
  },
  [TILES.TALLGRASS](px, rng, s) {
    // transparent bg, blades reaching up
    const blades = 6 + Math.floor(rng() * 3);
    for (let b = 0; b < blades; b++) {
      let x = 1 + Math.floor(rng() * 14);
      const h = 6 + Math.floor(rng() * 8);
      const c = pick(P.plant, rng());
      for (let i = 0; i < h; i++) {
        const y = 15 - i;
        px.set(x, y, i === h - 1 ? mul(c, 1.2) : c);
        if (i > 2 && rng() < 0.3) x += rng() < 0.5 ? 1 : -1;
        x = Math.max(0, Math.min(15, x));
      }
    }
  },
  [TILES.FLOWER_RED](px, rng, s) {
    const stem = P.plant[1];
    for (let y = 15; y >= 8; y--) px.set(7, y, y === 9 ? mul(stem, 0.8) : stem);
    px.set(5, 11, stem); px.set(6, 11, mul(stem, 1.1)); // leaf
    px.set(9, 10, mul(stem, 0.9)); px.set(10, 10, stem);
    const petal = hex('#d33a2c'), dark = hex('#a52820'), core = hex('#f6d341');
    px.fillRect(6, 4, 4, 4, petal);
    px.set(6, 4, dark); px.set(9, 4, dark); px.set(6, 7, dark); px.set(9, 7, dark);
    px.set(5, 5, petal); px.set(10, 5, petal); px.set(5, 6, petal); px.set(10, 6, petal);
    px.fillRect(7, 5, 2, 2, core);
  },
  [TILES.FLOWER_YELLOW](px, rng, s) {
    const stem = P.plant[0];
    for (let y = 15; y >= 9; y--) px.set(8, y, stem);
    px.set(6, 12, stem); px.set(7, 12, mul(stem, 1.1));
    const petal = hex('#f2cf43'), core = hex('#c98f2c'), hi = hex('#fbe98a');
    px.fillRect(6, 5, 5, 4, petal);
    px.set(7, 4, petal); px.set(8, 4, petal); px.set(9, 4, petal);
    px.fillRect(7, 6, 2, 2, core);
    px.set(6, 5, hi); px.set(10, 6, hi);
  },
  [TILES.CACTUS_SIDE](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      let c = pick(P.cactus, vnoise(x / 3, y / 3, s) * 0.7 + 0.15);
      if (x % 4 === 1) c = P.cactusRib;                                  // ribs
      if (x === 0 || x === 15) c = mul(P.cactusRib, 1.15);
      px.set(x, y, mul(c, 0.95 + hash2(x, y, s) * 0.1));
    }
    for (let k = 0; k < 6; k++) {
      const x = 3 + Math.floor(rng() * 10), y = Math.floor(rng() * 16);
      if (x % 4 !== 1) continue;
      px.set(x, y, P.cactusSpine);
    }
  },
  [TILES.CACTUS_TOP](px, rng, s) {
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const d = Math.max(Math.abs(x - 7.5), Math.abs(y - 7.5));
      let c = pick(P.cactus, vnoise(x / 3, y / 3, s) * 0.6 + 0.2);
      if (d > 6.4) c = mul(P.cactusRib, 1.1);
      px.set(x, y, c);
    }
    for (let k = 0; k < 5; k++) px.set(3 + Math.floor(rng() * 10), 3 + Math.floor(rng() * 10), P.cactusSpine);
  },
};

// ---------------------------------------------------------------- atlas
export function buildAtlas() {
  const atlas = new Px(ATLAS_W, ATLAS_H);
  const avg = [];
  for (let t = 0; t < COLS * ROWS; t++) {
    const painter = PAINTERS[t];
    const px = new Px(TILE, TILE);
    if (painter) painter(px, mulberry32(0xA53F + t * 7919), 0x51EE + t * 104729);
    atlas.blit(px, (t % COLS) * TILE, Math.floor(t / COLS) * TILE);
    let r = 0, g = 0, b = 0, n = 0;
    for (let i = 0; i < px.data.length; i += 4) {
      if (px.data[i + 3] > 128) { r += px.data[i]; g += px.data[i + 1]; b += px.data[i + 2]; n++; }
    }
    avg[t] = n ? [r / n, g / n, b / n] : [255, 0, 255];
  }
  return { px: atlas, avg };
}

// [u0, v0, u1, v1] with v0 = bottom edge of the tile (three.js flipY textures)
export function tileUV(t) {
  const c = t % COLS, r = Math.floor(t / COLS);
  const eu = 0.35 / ATLAS_W, ev = 0.35 / ATLAS_H;
  return [c / COLS + eu, 1 - (r + 1) / ROWS + ev, (c + 1) / COLS - eu, 1 - r / ROWS - ev];
}

export function tilePxRect(t) {
  return { sx: (t % COLS) * TILE, sy: Math.floor(t / COLS) * TILE };
}

// ---------------------------------------------------------------- water texture (separate, tileable 32x32)
export function waterPx() {
  const px = new Px(32, 32);
  const tones = ['#3560b8', '#3a67c4', '#3f6fd0', '#4677da'].map(hex);
  for (let y = 0; y < 32; y++) for (let x = 0; x < 32; x++) {
    // tileable: sample noise with wrapped lattice
    const n = vnoise(x / 6, y / 6, 0x3A7) * 0.7 + vnoise(x / 3, y / 3, 0x3A8) * 0.3;
    let c = pick(tones, n);
    if (vnoise(x / 4, y / 2.2, 0x3A9) > 0.74) c = hex('#5b8ae6');   // wave crest
    if (vnoise(x / 5, y / 2.6, 0x3AA) < 0.14) c = hex('#2e54a4');   // trough
    px.set(x, y, c);
  }
  return px;
}

// ---------------------------------------------------------------- creature skins
function mobBase(tones, seed, scale = 2.6, spots = null, spotColor = null) {
  const px = new Px(16, 16);
  base(px, tones, seed, scale, 0.18);
  if (spots) for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
    if (vnoise(x / 2.4, y / 2.4, seed ^ 99) < 0.3) px.set(x, y, spotColor);
  }
  return px;
}

export function mobPx(kind) {
  if (kind === 'gloomling') {
    const tones = ['#1d1529', '#241b33', '#2c2140', '#332649'].map(hex);
    const body = mobBase(tones, 0x61AA);
    speckle(body, hex('#3b2d55'), 0x61AB, 0.08, 1, 1);
    const face = body.clone();
    face.fillRect(3, 6, 3, 2, hex('#59f2e6'));
    face.fillRect(4, 6, 1, 1, hex('#dcfffa'));
    face.fillRect(10, 6, 3, 2, hex('#59f2e6'));
    face.fillRect(11, 6, 1, 1, hex('#dcfffa'));
    face.fillRect(7, 11, 2, 1, hex('#0d0718'));
    return { body, face };
  }
  if (kind === 'hopper') {
    const tones = ['#4c9338', '#59a544', '#66b551', '#74c25e'].map(hex);
    const body = mobBase(tones, 0x62BB, 2.6, true, hex('#356e26'));
    const face = body.clone();
    face.fillRect(3, 6, 3, 3, hex('#f2f6ea'));
    face.fillRect(4, 7, 1, 1, hex('#1c2a14'));
    face.fillRect(10, 6, 3, 3, hex('#f2f6ea'));
    face.fillRect(11, 7, 1, 1, hex('#1c2a14'));
    face.fillRect(7, 11, 2, 1, hex('#26421b'));
    return { body, face };
  }
  if (kind === 'skitter') {
    const tones = ['#b3925c', '#c2a26b', '#d0b27c', '#dbc08e'].map(hex);
    const body = mobBase(tones, 0x63CC);
    for (let y = 3; y < 16; y += 4) for (let x = 0; x < 16; x++) {
      if ((x + y) % 5 < 2) pxGuard(body, x, y, hex('#8a6f45'));
    }
    const face = body.clone();
    face.fillRect(2, 7, 2, 2, hex('#e5484a'));
    face.fillRect(12, 7, 2, 2, hex('#e5484a'));
    face.fillRect(2, 7, 1, 1, hex('#4a0d0d'));
    face.fillRect(13, 7, 1, 1, hex('#4a0d0d'));
    face.fillRect(6, 12, 4, 2, hex('#6e5330'));
    return { body, face };
  }
  if (kind === 'wisp') {
    const px = new Px(16, 16);
    for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
      const d = Math.hypot(x - 7.5, y - 7.5);
      let c;
      if (d < 3) c = hex('#eafcff');
      else if (d < 6) c = hex('#cdeef6');
      else if (d < 8.5) c = hex('#a9dbe8');
      else c = hex('#8fc6d6');
      if (hash2(x, y, 0x64DD) > 0.93) c = hex('#ffffff');
      px.set(x, y, c);
    }
    const face = px.clone();
    face.fillRect(4, 7, 3, 1, hex('#4f8a99'));
    face.fillRect(9, 7, 3, 1, hex('#4f8a99'));
    const shard = new Px(8, 8);
    for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
      const d = Math.abs(x - 3.5) + Math.abs(y - 3.5);
      shard.set(x, y, d < 2.5 ? hex('#bfe9f2') : d < 3.5 ? hex('#7cc4d4') : [0, 0, 0], d < 3.5 ? 255 : 0);
    }
    return { body: px, face, shard };
  }
  // puffbird
  const tones = ['#e4e4dd', '#f2f2ee', '#fafaf6', '#d6d6cd'].map(hex);
  const body = mobBase(tones, 0x65EE);
  const face = body.clone();
  face.fillRect(4, 5, 2, 2, hex('#20242c'));
  face.fillRect(10, 5, 2, 2, hex('#20242c'));
  face.fillRect(7, 7, 2, 3, hex('#e8a13c'));
  const wing = new Px(12, 8);
  base(wing, ['#a9a9a1', '#c2c2ba', '#d9d9d2'].map(hex), 0x66FF, 2.4);
  return { body, face, wing };
}

function pxGuard(px, x, y, c) { px.set(x, y, c); }

// ---------------------------------------------------------------- HUD sprites
const HEART = [
  '.HH..HH.',
  'HHHHHHHH',
  'HHHHHHHH',
  'HHHHHHHH',
  '.HHHHHH.',
  '..HHHH..',
  '...HH...',
  '........',
];

export function heartPx(kind) { // 'full' | 'half' | 'empty'
  const px = new Px(8, 8);
  const full = hex('#e23b3b'), shade = hex('#9c1f1f'), empty = hex('#3d3d46'), edge = hex('#1d1d26');
  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    if (HEART[y][x] !== 'H') continue;
    let c;
    if (kind === 'empty') c = empty;
    else if (kind === 'half') c = x < 4 ? full : empty;
    else c = (y >= 5) ? shade : full;
    px.set(x, y, c);
  }
  return px;
}

export function bubblePx() {
  const px = new Px(8, 8);
  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    const d = Math.hypot(x - 3.5, y - 3.5);
    if (d < 3.4) px.set(x, y, d < 2 ? hex('#bfe4ff') : hex('#7db8e8'));
  }
  px.set(2, 2, hex('#eaf6ff'));
  return px;
}

// sun / moon / cloud canvases
export function sunPx() {
  const px = new Px(32, 32);
  px.fillRect(4, 4, 24, 24, hex('#ffd978'));
  px.fillRect(7, 7, 18, 18, hex('#fff3c2'));
  for (let i = 0; i < 32; i++) { px.set(i, 3, hex('#f19a36')); px.set(i, 28, hex('#f19a36')); px.set(3, i, hex('#f19a36')); px.set(28, i, hex('#f19a36')); }
  return px;
}

export function moonPx() {
  const px = new Px(32, 32);
  px.fillRect(5, 5, 22, 22, hex('#d8e1ec'));
  px.fillRect(8, 8, 16, 16, hex('#eef3f8'));
  px.fillRect(12, 10, 4, 4, hex('#c3cfdd'));
  px.fillRect(18, 16, 3, 3, hex('#c3cfdd'));
  px.fillRect(10, 18, 2, 2, hex('#c3cfdd'));
  return px;
}
