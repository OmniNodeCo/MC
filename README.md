# Voxelheim

A hand-built voxel sandbox game that runs entirely in the browser — original engine, original creatures, and every texture painted pixel-by-pixel in code. No image assets, no AI-generated art.

![gameplay](https://img.shields.io/badge/play-localhost%3A3000-58e0c8)

## Run it

**Desktop (Windows / macOS / Linux):** grab the latest release —
`.exe` installer or portable exe (Windows), `.dmg` (macOS, Intel + Apple Silicon),
`.rpm` / `.deb` / `.AppImage` (Linux). Every release also ships a portable web bundle.

**From source:**

```bash
npm install
npm start          # web: http://localhost:3000
npm run dist       # native desktop packages for this OS (electron-builder)
```

Open the page (or launch the app), press **PLAY**, and you're in the world.

## Controls

| Input | Action |
|---|---|
| **WASD** + mouse | Move / look |
| **Space** | Jump — double-tap to toggle flight |
| **Shift** | Sprint (or descend while flying) |
| **LMB** hold | Mine blocks / attack creatures |
| **RMB** | Place block |
| **Middle-click** | Pick targeted block into hotbar |
| **1–9 / wheel** | Select hotbar slot |
| **E** | Block palette (bind any block to the selected slot) |
| **F3** | Debug overlay · **M** mute · **Esc** pause |

## The world

Seeded procedural terrain across six biomes — plains, forest, desert, snowfields, mountains and oceans — with spaghetti caves, caverns, ore veins (coal, iron, gold, and glowing crystal), oak and spruce trees, cacti, flowers, and drifting clouds under a full day/night cycle. The world auto-saves to your browser, including every edit you make.

## The creatures (all original)

| Creature | Behavior |
|---|---|
| **Gloomling** | Hostile night-stalker with cyan eyes. Hops after you in the dark and melts away in sunlight. |
| **Sand Skitter** | Six-legged desert crab with snapping mandibles. Fast, nippy, always angry. |
| **Moss Hopper** | Passive green blob of the meadows. Squashes and stretches with every hop. |
| **Sky Wisp** | Glowing mountain spirit with three orbiting shards. |
| **Puffbird** | Chatty bird that circles the skies flapping its wings. |

## Architecture

```
public/
  index.html          entry page + HUD DOM
  style.css           HUD / overlay styling
  js/
    noise.js          seeded PRNG, simplex noise, fBm
    textures.js       the whole texture pack — every tile & mob skin painted in code
    blocks.js         block registry (26 blocks)
    world.js          terrain gen, biomes, caves, ores, trees, greedy face-culled
                      meshing with ambient occlusion, DDA raycasting, AABB physics
    player.js         movement, swimming, flight, fall damage, drowning
    entities.js       creature AI (chase/wander/flee), spawning rules, combat
    sky.js            day/night cycle, sun, moon, stars, clouds
    particles.js      pooled point-sprite particles
    audio.js          WebAudio-synthesized sound effects (no audio files)
    hud.js            hotbar, hearts, air bubbles, palette, overlays
    main.js           boot, input, chunk streaming, save/load, game loop
  vendor/             three.js (r171, vendored)
tools/
  selftest.mjs        headless engine tests → writes texture sheets to /tmp for review
  sim.mjs             headless integration sim: drives the real game modules through
                      a full play session (walk, mine, night raids, combat, respawn)
server.js             zero-dependency static server
```

### Rendering

Chunks (16×80×16) mesh into four buckets (opaque / cutout / glow / water) with per-face
directional shading and vertex ambient occlusion. Chunk data generates around the player
on demand and unloads behind them; player edits persist across sessions.

### Tests

```bash
npm test             # engine tests + integration sim (all headless)
```

The selftest also renders the full texture atlas and the creature/HUD sprite sheet to
`/tmp/atlas_big.png` and `/tmp/mobs.png` for pixel-level review (set `VOXELHEIM_TEX_OUT`
to change the destination).

## CI & releases

- **`.github/workflows/build.yml`** — on every push/PR: runs both test suites, renders
  texture previews, packages the web bundle, and builds native desktop packages on
  Windows, macOS and Linux runners (Electron). All artifacts are uploaded.
- **`.github/workflows/release.yml`** — on a `v*` tag push (or manual dispatch with a
  version input): runs tests, builds **all platform packages**, verifies every expected
  asset exists, and publishes a GitHub release with installers for every OS attached.
  Run it with: `gh workflow run release.yml -f version=v1.1.1`.

Release assets per version:

| OS | Assets |
|---|---|
| Windows | `Voxelheim-<v>-Windows-Setup-x64.exe`, `...-Portable-x64.exe` |
| macOS | `Voxelheim-<v>-macOS-x64.dmg`, `...-arm64.dmg` (+ portable `.zip`s) |
| Linux | `Voxelheim-<v>-Linux-x64.rpm`, `.deb`, `.AppImage` |
| Web | `voxelheim-<v>.zip` (+ texture preview PNGs) |

The desktop app icon is generated from the game's own pixel art (`tools/icon.mjs`) —
grass block, Gloomling, crystal shards — into `icon.png/.ico/.icns`.

To package locally: `node tools/package.mjs v1.1.0` → `dist/voxelheim-1.1.0.zip`.
Unzip, `npm install && npm start`, open http://localhost:3000.
Desktop packaging requires network access to Electron binaries; it runs in CI.
