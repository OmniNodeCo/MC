// Voxelheim — desktop icon generator.
// Composes the app icon from the game's own pixel art (grass block + gloomling
// + crystal shards) and emits icon.png / icon.ico / icon.icns into build-res/.
// Run: node tools/icon.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCanvas, ImageData } from '@napi-rs/canvas';
import png2icons from 'png2icons';
import { buildAtlas, mobPx, TILES } from '../public/js/textures.js';

const OUT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'build-res');
fs.mkdirSync(OUT, { recursive: true });

const SIZE = 1024;
const TILE = 16;

const atlas = buildAtlas();
const gloom = mobPx('gloomling');

// --- nearest-scaler blitting directly from Px data ---
const atlasData = atlas.px.data, AW = atlas.px.w;
function crop(tile) {
  const sx = (tile % 8) * TILE, sy = Math.floor(tile / 8) * TILE;
  const d = new Uint8ClampedArray(TILE * TILE * 4);
  for (let y = 0; y < TILE; y++) {
    for (let x = 0; x < TILE; x++) {
      const si = ((sy + y) * AW + (sx + x)) * 4;
      const di = (y * TILE + x) * 4;
      d[di] = atlasData[si]; d[di + 1] = atlasData[si + 1];
      d[di + 2] = atlasData[si + 2]; d[di + 3] = atlasData[si + 3];
    }
  }
  return d;
}
function cropPx(px) { return new Uint8ClampedArray(px.data); }

function blitScaled(ctx, data, dx, dy, scale) {
  const big = new Uint8ClampedArray(TILE * scale * TILE * scale * 4);
  for (let y = 0; y < TILE * scale; y++) {
    for (let x = 0; x < TILE * scale; x++) {
      const si = (Math.floor(y / scale) * TILE + Math.floor(x / scale)) * 4;
      const di = (y * TILE * scale + x) * 4;
      big[di] = data[si]; big[di + 1] = data[si + 1];
      big[di + 2] = data[si + 2]; big[di + 3] = data[si + 3];
    }
  }
  ctx.putImageData(new ImageData(big, TILE * scale, TILE * scale), dx, dy);
}

// --- compose ---
const cv = createCanvas(SIZE, SIZE);
const ctx = cv.getContext('2d');

// rounded-corner background
const R = 170;
ctx.beginPath();
ctx.moveTo(R, 0);
ctx.arcTo(SIZE, 0, SIZE, SIZE, R);
ctx.arcTo(SIZE, SIZE, 0, SIZE, R);
ctx.arcTo(0, SIZE, 0, 0, R);
ctx.arcTo(0, 0, SIZE, 0, R);
ctx.closePath();
ctx.fillStyle = '#0b0e1a';
ctx.fill();
ctx.save();
ctx.clip();

// teal inner ring
ctx.lineWidth = 26;
ctx.strokeStyle = '#58e0c8';
ctx.strokeRect(58, 58, SIZE - 116, SIZE - 116);

// big grass block face (top tile), centered
const grassScale = 44;                       // 16*44 = 704
blitScaled(ctx, crop(TILES.GRASS_TOP), (SIZE - TILE * grassScale) / 2, (SIZE - TILE * grassScale) / 2 - 20, grassScale);

// gloomling peeking from the bottom-right corner
const gloScale = 22;                         // 16*22 = 352
blitScaled(ctx, cropPx(gloom.face), SIZE - TILE * gloScale - 66, SIZE - TILE * gloScale - 66, gloScale);

// crystal shards, top-left
ctx.fillStyle = '#3fe0d2';
const diamond = (cx, cy, r) => {
  ctx.beginPath();
  ctx.moveTo(cx, cy - r); ctx.lineTo(cx + r * 0.62, cy);
  ctx.lineTo(cx, cy + r); ctx.lineTo(cx - r * 0.62, cy);
  ctx.closePath(); ctx.fill();
};
diamond(200, 216, 52);
diamond(286, 176, 30);
diamond(152, 300, 22);
ctx.fillStyle = '#aefcf4';
diamond(200, 216, 20);
diamond(286, 176, 12);
ctx.restore();

const pngBuf = cv.toBuffer('image/png');
fs.writeFileSync(path.join(OUT, 'icon.png'), pngBuf);

// --- ico + icns from the PNG ---
const alg = png2icons.NEAREST_NEIGHBOR ?? png2icons.BICUBIC;
const ico = png2icons.createICO(pngBuf, alg, 0, true);
const icns = png2icons.createICNS(pngBuf, alg, 0);
if (!ico || !icns) throw new Error('png2icons conversion failed');
fs.writeFileSync(path.join(OUT, 'icon.ico'), ico);
fs.writeFileSync(path.join(OUT, 'icon.icns'), icns);

console.log(`icons written to build-res/: icon.png (${pngBuf.length} B), icon.ico (${ico.length} B), icon.icns (${icns.length} B)`);
