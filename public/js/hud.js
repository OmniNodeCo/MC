// Voxelheim — HUD: hotbar, hearts, air, toasts, overlays, block palette.
import { BLOCKS, PLACEABLE } from './blocks.js';
import { heartPx, bubblePx } from './textures.js';
import { tilePxRect } from './textures.js';

const $ = (id) => document.getElementById(id);

function drawTile(cv, atlasCanvas, tile) {
  const ctx = cv.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  const { sx, sy } = tilePxRect(tile);
  ctx.clearRect(0, 0, cv.width, cv.height);
  ctx.drawImage(atlasCanvas, sx, sy, 16, 16, 0, 0, cv.width, cv.height);
}

function dataUrl(px) { return px.toCanvas().toDataURL(); }

export class HUD {
  constructor(atlasCanvas, tileFor) {
    this.atlasCanvas = atlasCanvas;
    this.tileFor = tileFor;
    this.slots = [];
    this.selected = 0;
    this.heartUrls = {
      full: dataUrl(heartPx('full')),
      half: dataUrl(heartPx('half')),
      empty: dataUrl(heartPx('empty')),
    };
    this.bubbleUrl = dataUrl(bubblePx());
    this.debugT = 0;

    this.heartsEl = $('hearts');
    for (let i = 0; i < 10; i++) {
      const img = document.createElement('img');
      img.src = this.heartUrls.full;
      this.heartsEl.appendChild(img);
    }
    this.bubblesEl = $('bubbles');
    for (let i = 0; i < 10; i++) {
      const img = document.createElement('img');
      img.src = this.bubbleUrl;
      this.bubblesEl.appendChild(img);
    }
    this.hotbarEl = $('hotbar');
    this.paletteEl = $('palette');
    this.paletteGrid = $('palette-grid');
    this.overlayEl = $('overlay');
    this.overlayContent = $('overlay-content');
    this.mineEl = $('mineprog');
    this.debugEl = $('debug');
    this.toastsEl = $('toasts');
    this.vignetteEl = $('vignette');
    this.waterEl = $('waterfx');
  }

  // ------------------------------------------------------------ hotbar
  buildHotbar(ids, onSelect) {
    this.hotbarEl.innerHTML = '';
    this.slots = [];
    ids.forEach((id, i) => {
      const slot = document.createElement('div');
      slot.className = 'slot' + (i === this.selected ? ' sel' : '');
      const cv = document.createElement('canvas');
      cv.width = 40; cv.height = 40;
      drawTile(cv, this.atlasCanvas, this.tileFor(id, 'side'));
      const k = document.createElement('span');
      k.className = 'k';
      k.textContent = i + 1;
      const nm = document.createElement('span');
      nm.className = 'nm';
      nm.textContent = BLOCKS[id].name;
      slot.appendChild(cv); slot.appendChild(k); slot.appendChild(nm);
      slot.addEventListener('click', () => onSelect(i));
      this.hotbarEl.appendChild(slot);
      this.slots.push(slot);
    });
  }

  setSlot(i, id) {
    const slot = this.slots[i];
    if (!slot) return;
    drawTile(slot.querySelector('canvas'), this.atlasCanvas, this.tileFor(id, 'side'));
    slot.querySelector('.nm').textContent = BLOCKS[id].name;
  }

  select(i) {
    this.selected = i;
    this.slots.forEach((s, j) => s.classList.toggle('sel', j === i));
  }

  // ------------------------------------------------------------ vitals
  updateHearts(hp) {
    const imgs = this.heartsEl.children;
    for (let i = 0; i < 10; i++) {
      const v = hp - i * 2;
      imgs[i].src = v >= 2 ? this.heartUrls.full : v === 1 ? this.heartUrls.half : this.heartUrls.empty;
    }
  }

  updateAir(show, air) {
    this.bubblesEl.classList.toggle('hidden', !show);
    if (!show) return;
    const imgs = this.bubblesEl.children;
    const n = Math.ceil(air);
    for (let i = 0; i < 10; i++) imgs[i].style.visibility = i < n ? 'visible' : 'hidden';
  }

  vignette() {
    this.vignetteEl.style.opacity = 0.85;
    setTimeout(() => { this.vignetteEl.style.opacity = 0; }, 120);
  }

  waterFx(on) {
    this.waterEl.style.opacity = on ? 1 : 0;
  }

  // ------------------------------------------------------------ misc
  toast(msg, cls = '', ms = 3200) {
    const t = document.createElement('div');
    t.className = 'toast ' + cls;
    t.textContent = msg;
    this.toastsEl.appendChild(t);
    setTimeout(() => t.classList.add('out'), ms);
    setTimeout(() => t.remove(), ms + 600);
  }

  setMine(frac) {
    if (frac === null) { this.mineEl.classList.add('hidden'); return; }
    this.mineEl.classList.remove('hidden');
    this.mineEl.style.setProperty('--p', `${Math.min(100, frac * 100)}%`);
  }

  debug(text) {
    if (text === null) this.debugEl.classList.add('hidden');
    else {
      this.debugEl.classList.remove('hidden');
      this.debugEl.textContent = text;
    }
  }

  // ------------------------------------------------------------ palette
  openPalette(onPick, onClose) {
    this.paletteGrid.innerHTML = '';
    for (const id of PLACEABLE) {
      const item = document.createElement('div');
      item.className = 'pal-item';
      const cv = document.createElement('canvas');
      cv.width = 40; cv.height = 40;
      drawTile(cv, this.atlasCanvas, this.tileFor(id, 'side'));
      const nm = document.createElement('span');
      nm.className = 'nm';
      nm.textContent = BLOCKS[id].name;
      item.appendChild(cv); item.appendChild(nm);
      item.addEventListener('click', () => { onPick(id); });
      this.paletteGrid.appendChild(item);
    }
    this.paletteEl.classList.remove('hidden');
    this._paletteOnClose = onClose;
  }

  closePalette() {
    this.paletteEl.classList.add('hidden');
    if (this._paletteOnClose) { const f = this._paletteOnClose; this._paletteOnClose = null; f(); }
  }

  paletteOpen() { return !this.paletteEl.classList.contains('hidden'); }

  // ------------------------------------------------------------ overlays
  hideOverlay() { this.overlayEl.classList.add('hidden'); }

  _btn(label, cls, fn) {
    const b = document.createElement('button');
    b.className = 'btn ' + (cls || '');
    b.textContent = label;
    b.addEventListener('click', fn);
    return b;
  }

  showStart(seed, onPlay, onNew) {
    this.overlayEl.classList.remove('hidden');
    this.overlayContent.innerHTML = `
      <h1>VOXELHEIM</h1>
      <div class="tag">A HAND-BUILT VOXEL WORLD</div>
      <div class="bestiary">
        Meet the locals:
        <span class="passive">Moss Hopper</span> ·
        <span class="passive">Puffbird</span> ·
        <span class="magic">Sky Wisp</span> ·
        <span class="hostile">Sand Skitter</span> —
        and when night falls… <span class="hostile">the Gloomlings</span> emerge.
      </div>
      <div class="controls">
        <b>WASD</b> move &nbsp;·&nbsp; <b>Mouse</b> look &nbsp;·&nbsp; <b>Space</b> jump (double-tap to fly)<br>
        <b>LMB</b> mine / attack &nbsp;·&nbsp; <b>RMB</b> place block &nbsp;·&nbsp; <b>Middle-click</b> pick block<br>
        <b>1–9 / wheel</b> hotbar &nbsp;·&nbsp; <b>E</b> block palette &nbsp;·&nbsp; <b>Shift</b> sprint / fly down<br>
        <b>F3</b> debug &nbsp;·&nbsp; <b>M</b> mute &nbsp;·&nbsp; <b>Esc</b> pause
      </div>
      <div id="ov-buttons"></div>
      <div class="seed">world seed <b>${seed}</b> · auto-saves locally</div>
      <div class="note">every texture hand-coded pixel by pixel — no image files, no AI art</div>`;
    const btns = document.getElementById('ov-buttons');
    btns.appendChild(this._btn('▶  PLAY', 'primary', onPlay));
    btns.appendChild(this._btn('New World', '', onNew));
  }

  showPause(onResume, onNew) {
    this.overlayEl.classList.remove('hidden');
    this.overlayContent.innerHTML = `
      <h1 style="font-size:30px">PAUSED</h1>
      <div class="tag">THE WORLD HOLDS ITS BREATH</div>
      <div class="controls" style="text-align:center">
        Your world auto-saved just now.<br>Mine, build, fight — the night is short.
      </div>
      <div id="ov-buttons"></div>`;
    const btns = document.getElementById('ov-buttons');
    btns.appendChild(this._btn('▶  RESUME', 'primary', onResume));
    btns.appendChild(this._btn('New World', '', onNew));
  }

  showDead(onRespawn) {
    this.overlayEl.classList.remove('hidden');
    this.overlayContent.innerHTML = `
      <h1 style="font-size:34px;color:#ff8a7a">YOU DIED</h1>
      <div class="tag" style="color:#ff8a7a">THE VOXELS CLAIM ANOTHER</div>
      <div id="ov-buttons"></div>`;
    const btns = document.getElementById('ov-buttons');
    btns.appendChild(this._btn('RESPAWN', 'danger', onRespawn));
  }
}
