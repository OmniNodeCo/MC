// MC-RS browser glue: pointer lock, input forwarding, autosave and the frame
// loop. All game logic lives in Rust/WebAssembly.

import init, * as mc from './pkg/mc.js';

const canvas = document.getElementById('game');
const overlay = document.getElementById('overlay');
const title = document.getElementById('title');
const subtitle = document.getElementById('subtitle');
const playButton = document.getElementById('play');
const statusEl = document.getElementById('status');

const SAVE_KEY = 'mc-rs-world-v1';
const SEED = 1337;

let started = false;
let playing = false;
let inventoryOpen = false;
let lastFrame = 0;
let failed = false;

const dpr = () => Math.min(window.devicePixelRatio || 1, 2);

function resizeCanvas() {
  const ratio = dpr();
  const w = Math.max(1, Math.floor(window.innerWidth * ratio));
  const h = Math.max(1, Math.floor(window.innerHeight * ratio));
  canvas.width = w;
  canvas.height = h;
  canvas.style.width = `${window.innerWidth}px`;
  canvas.style.height = `${window.innerHeight}px`;
  if (started) mc.resize(w, h);
}

function bytesToBase64(bytes) {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function base64ToBytes(text) {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function persist(silent = false) {
  if (!started) return;
  try {
    const bytes = mc.persist_bytes();
    localStorage.setItem(SAVE_KEY, bytesToBase64(bytes));
    if (!silent) mc.mark_saved();
  } catch (err) {
    console.warn('autosave failed', err);
  }
}

function setStatus(text, isError = false) {
  statusEl.textContent = text;
  statusEl.classList.toggle('error', isError);
}

function showMenu(heading, buttonLabel) {
  title.innerHTML = 'MC<span>-RS</span>';
  subtitle.textContent = heading;
  playButton.textContent = buttonLabel;
  overlay.classList.remove('hidden');
}

function requestLock() {
  const result = canvas.requestPointerLock();
  if (result && typeof result.catch === 'function') {
    result.catch(() => showMenu('Click the page to capture the mouse', 'Resume'));
  }
}

function refreshInventory() {
  inventoryOpen = started && mc.inventory_open();
  document.body.classList.toggle('palette-open', inventoryOpen);
  if (inventoryOpen) {
    if (document.pointerLockElement === canvas) document.exitPointerLock();
    mc.pointer_lock(true);
    overlay.classList.add('hidden');
  } else {
    mc.pointer_lock(document.pointerLockElement === canvas);
  }
  return inventoryOpen;
}

playButton.addEventListener('click', () => {
  if (!started) return;
  playing = true;
  overlay.classList.add('hidden');
  requestLock();
});

window.addEventListener('resize', resizeCanvas);

document.addEventListener('pointerlockchange', () => {
  const locked = document.pointerLockElement === canvas;
  inventoryOpen = started && mc.inventory_open();
  if (locked) {
    overlay.classList.add('hidden');
    mc.pointer_lock(true);
  } else {
    mc.pointer_lock(inventoryOpen);
    if (!inventoryOpen) {
      showMenu('Paused', 'Resume');
    }
  }
});

document.addEventListener('pointerlockerror', () => {
  showMenu('The browser refused to capture the mouse. Click Resume.', 'Resume');
});

window.addEventListener('keydown', (event) => {
  if (!started) return;
  if (event.code === 'F3' || event.code === 'Space' || event.code === 'Tab') {
    event.preventDefault();
  }
  if (event.repeat) return;
  mc.set_key(event.code, true);
  if (event.code === 'KeyE' || event.code === 'Escape') {
    refreshInventory();
    if (event.code === 'Escape' && !inventoryOpen) {
      showMenu('Paused', 'Resume');
      mc.pointer_lock(false);
    }
  }
});

window.addEventListener('keyup', (event) => {
  if (!started) return;
  mc.set_key(event.code, false);
});

canvas.addEventListener('mousedown', (event) => {
  if (!started) return;
  if (inventoryOpen && document.pointerLockElement !== canvas) {
    const rect = canvas.getBoundingClientRect();
    const ratio = dpr();
    mc.click((event.clientX - rect.left) * ratio, (event.clientY - rect.top) * ratio);
    event.preventDefault();
    refreshInventory();
    if (!inventoryOpen && playing) requestLock();
    return;
  }
  if (document.pointerLockElement !== canvas) {
    if (playing) requestLock();
    return;
  }
  mc.mouse_button(event.button, true);
  event.preventDefault();
});

window.addEventListener('mouseup', (event) => {
  if (!started || document.pointerLockElement !== canvas) return;
  mc.mouse_button(event.button, false);
});

window.addEventListener('mousemove', (event) => {
  if (!started || document.pointerLockElement !== canvas) return;
  mc.mouse_delta(event.movementX, event.movementY);
});

canvas.addEventListener(
  'wheel',
  (event) => {
    if (!started || document.pointerLockElement !== canvas) return;
    mc.wheel(Math.sign(event.deltaY));
    event.preventDefault();
  },
  { passive: false },
);

canvas.addEventListener('contextmenu', (event) => event.preventDefault());
window.addEventListener('blur', () => mc.pointer_lock(false));
window.addEventListener('beforeunload', () => persist(true));

function loop(now) {
  const dt = lastFrame ? Math.min((now - lastFrame) / 1000, 0.1) : 0;
  lastFrame = now;
  try {
    if (mc.frame(dt)) persist();
  } catch (err) {
    fail(err);
    return;
  }
  requestAnimationFrame(loop);
}

function fail(err) {
  if (failed) return;
  failed = true;
  console.error(err);
  showMenu('Something went wrong', 'Reload');
  setStatus(String(err && err.message ? err.message : err), true);
  playButton.disabled = false;
  playButton.textContent = 'Reload';
  playButton.onclick = () => window.location.reload();
}

async function boot() {
  try {
    resizeCanvas();
    setStatus('Compiling Rust to WebAssembly …');
    await init();
    setStatus('Carving a world …');
    await mc.start(canvas, canvas.width, canvas.height, SEED);

    const saved = localStorage.getItem(SAVE_KEY);
    if (saved) {
      try {
        if (mc.restore_bytes(base64ToBytes(saved))) setStatus('World restored.');
      } catch (err) {
        console.warn('could not restore save', err);
      }
    }

    started = true;
    playing = false;
    playButton.disabled = false;
    playButton.textContent = 'Play';
    setStatus('');
    showMenu('a Minecraft clone written in Rust → WebAssembly', 'Play');
    requestAnimationFrame(loop);
  } catch (err) {
    fail(err);
  }
}

boot();
