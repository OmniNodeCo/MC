// Voxelheim — dependency-free zip packager.
// Bundles the game (public/ + server + manifests) into dist/voxelheim-<version>.zip
// Usage: node tools/package.mjs [versionTag]
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// ---- CRC32 (same polynomial as PNG) ----
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

// ---- collect files ----
function walk(dir, base = '') {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = base ? `${base}/${e.name}` : e.name;
    if (e.isDirectory()) out.push(...walk(path.join(dir, e.name), rel));
    else if (e.isFile()) out.push({ rel, abs: path.join(dir, e.name) });
  }
  return out;
}

const files = [...walk(path.join(ROOT, 'public'))];
for (const rel of ['server.js', 'package.json', 'package-lock.json', 'README.md']) {
  const abs = path.join(ROOT, rel);
  if (fs.existsSync(abs)) files.push({ rel, abs });
}
files.sort((a, b) => a.rel.localeCompare(b.rel));

// ---- version ----
const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
const tag = process.argv[2] ? process.argv[2].replace(/^v/, '') : pkg.version;
const outName = `voxelheim-${tag}.zip`;

// ---- write zip (store method — zero deps, deterministic) ----
const { time, date } = (() => {
  const d = new Date();
  const year = Math.max(0, Math.min(127, d.getFullYear() - 1980));
  return {
    date: (year << 9) | ((d.getMonth() + 1) << 5) | d.getDate(),
    time: (d.getHours() << 11) | (d.getMinutes() << 5) | (d.getSeconds() >> 1),
  };
})();

const chunks = [];
const central = [];
let offset = 0;

for (const f of files) {
  const data = fs.readFileSync(f.abs);
  const name = Buffer.from(f.rel, 'utf8');
  const crc = crc32(data);

  const lh = Buffer.alloc(30);
  lh.writeUInt32LE(0x04034b50, 0);  // local file header signature
  lh.writeUInt16LE(20, 4);          // version needed
  lh.writeUInt16LE(0x0800, 6);      // UTF-8 names
  lh.writeUInt16LE(0, 8);           // method: store
  lh.writeUInt16LE(time, 10);
  lh.writeUInt16LE(date, 12);
  lh.writeUInt32LE(crc, 14);
  lh.writeUInt32LE(data.length, 18);
  lh.writeUInt32LE(data.length, 22);
  lh.writeUInt16LE(name.length, 26);
  lh.writeUInt16LE(0, 28);
  chunks.push(lh, name, data);

  const ch = Buffer.alloc(46);
  ch.writeUInt32LE(0x02014b50, 0);  // central directory signature
  ch.writeUInt16LE(20, 4);          // version made by
  ch.writeUInt16LE(20, 6);          // version needed
  ch.writeUInt16LE(0x0800, 8);
  ch.writeUInt16LE(0, 10);          // method: store
  ch.writeUInt16LE(time, 12);
  ch.writeUInt16LE(date, 14);
  ch.writeUInt32LE(crc, 16);
  ch.writeUInt32LE(data.length, 20);
  ch.writeUInt32LE(data.length, 24);
  ch.writeUInt16LE(name.length, 28);
  ch.writeUInt32LE(((0o100644 << 16) >>> 0), 38); // external attrs: -rw-r--r--
  ch.writeUInt32LE(offset, 42);
  central.push(ch, name);

  offset += 30 + name.length + data.length;
}

const cdStart = offset;
const cdBuf = Buffer.concat(central);
offset += cdBuf.length;

const eocd = Buffer.alloc(22);
eocd.writeUInt32LE(0x06054b50, 0); // end of central directory
eocd.writeUInt16LE(files.length, 8);
eocd.writeUInt16LE(files.length, 10);
eocd.writeUInt32LE(cdBuf.length, 12);
eocd.writeUInt32LE(cdStart, 16);

const zip = Buffer.concat([...chunks, cdBuf, eocd]);

const distDir = path.join(ROOT, 'dist');
fs.mkdirSync(distDir, { recursive: true });
const outPath = path.join(distDir, outName);
fs.writeFileSync(outPath, zip);

const kb = (zip.length / 1024).toFixed(0);
console.log(`packaged ${files.length} files → dist/${outName} (${kb} KB)`);
for (const f of files) console.log(`  ${f.rel}`);
