//! Immediate mode HUD drawing. Produces a flat list of textured quads that the
//! renderer uploads into one dynamic vertex buffer.

use crate::block;
use crate::font;
use crate::game::{Game, GameMode, HOTBAR_SLOTS};
use crate::textures;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

pub type Rect = [f32; 4];

pub const PALETTE_COLS: usize = 7;
pub const PALETTE_ROWS: usize = 4;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const PANEL: [f32; 4] = [0.06, 0.07, 0.09, 0.82];
const SLOT_BG: [f32; 4] = [0.14, 0.15, 0.18, 0.72];
const ACCENT: [f32; 4] = [1.0, 0.86, 0.35, 1.0];

/// Scales the HUD with the window height (720p is the reference).
pub fn ui_scale(screen: [f32; 2]) -> f32 {
    (screen[1] / 720.0).clamp(0.72, 2.4)
}

pub struct Ui {
    verts: Vec<UiVertex>,
    screen: [f32; 2],
    scale: f32,
}

impl Ui {
    pub fn new(screen: [f32; 2]) -> Self {
        Self {
            verts: Vec::with_capacity(4096),
            screen,
            scale: ui_scale(screen),
        }
    }

    fn push(&mut self, pos: [f32; 2], uv: [f32; 2], color: [f32; 4]) {
        if self.verts.len() < 60_000 {
            self.verts.push(UiVertex { pos, uv, color });
        }
    }

    /// Solid quad; UVs point at a single transparent atlas pixel.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let uv = textures::white_uv();
        self.push([x, y], uv, color);
        self.push([x + w, y], uv, color);
        self.push([x + w, y + h], uv, color);
        self.push([x, y], uv, color);
        self.push([x + w, y + h], uv, color);
        self.push([x, y + h], uv, color);
    }

    /// Textured quad using an atlas tile.
    pub fn tile(&mut self, x: f32, y: f32, w: f32, h: f32, tile: u32, tint: [f32; 4]) {
        let [u0, v0, u1, v1] = textures::tile_uv(tile);
        self.tile_uv(x, y, w, h, [u0, v0, u1, v1], tint);
    }

    pub fn tile_uv(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], tint: [f32; 4]) {
        let [u0, v0, u1, v1] = uv;
        self.push([x, y], [u0, v0], tint);
        self.push([x + w, y], [u1, v0], tint);
        self.push([x + w, y + h], [u1, v1], tint);
        self.push([x, y], [u0, v0], tint);
        self.push([x + w, y + h], [u1, v1], tint);
        self.push([x, y + h], [u0, v1], tint);
    }

    /// Rectangle outline of `thickness` pixels.
    pub fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, color: [f32; 4]) {
        self.rect(x, y, w, t, color);
        self.rect(x, y + h - t, w, t, color);
        self.rect(x, y, t, h, color);
        self.rect(x + w - t, y, t, h, color);
    }

    pub fn text_raw(&mut self, x: f32, y: f32, size: f32, color: [f32; 4], text: &str) {
        let mut cx = x;
        for ch in text.chars() {
            if ch == ' ' {
                cx += 6.0 * size;
                continue;
            }
            let cell = font::glyph_cell(ch);
            let uv = font::glyph_uv(cell);
            // The glyph occupies x=1..6 of its 8px cell.
            self.tile_uv(cx - size, y, 8.0 * size, 8.0 * size, uv, color);
            cx += 6.0 * size;
        }
    }

    pub fn text(&mut self, x: f32, y: f32, size: f32, color: [f32; 4], text: &str) {
        self.text_raw(x + size, y + size, size, SHADOW, text);
        self.text_raw(x, y, size, color, text);
    }

    pub fn text_centered(&mut self, cx: f32, y: f32, size: f32, color: [f32; 4], text: &str) {
        let w = font::text_width(text, size);
        self.text(cx - w * 0.5, y, size, color, text);
    }

    pub fn finish(self) -> Vec<UiVertex> {
        self.verts
    }
}

/// Panel + slot rectangles of the creative palette, shared by drawing and hit
/// testing.
pub fn palette_layout(screen: [f32; 2]) -> (Rect, Vec<Rect>) {
    let s = ui_scale(screen);
    let slot = 44.0 * s;
    let gap = 4.0 * s;
    let pad = 16.0 * s;
    let header = 30.0 * s;
    let w = PALETTE_COLS as f32 * (slot + gap) - gap + pad * 2.0;
    let h = header + PALETTE_ROWS as f32 * (slot + gap) - gap + pad;
    let x = (screen[0] - w) * 0.5;
    let y = (screen[1] - h) * 0.5;

    let mut slots = Vec::with_capacity(PALETTE_COLS * PALETTE_ROWS);
    for row in 0..PALETTE_ROWS {
        for col in 0..PALETTE_COLS {
            let sx = x + pad + col as f32 * (slot + gap);
            let sy = y + header + row as f32 * (slot + gap);
            slots.push([sx, sy, slot, slot]);
        }
    }
    ([x, y, w, h], slots)
}

/// Which palette entry is under a screen position.
pub fn palette_hit(screen: [f32; 2], pos: [f32; 2]) -> Option<usize> {
    let (_, slots) = palette_layout(screen);
    slots.iter().position(|r| {
        pos[0] >= r[0] && pos[0] <= r[0] + r[2] && pos[1] >= r[1] && pos[1] <= r[1] + r[3]
    })
}

fn hotbar_layout(screen: [f32; 2]) -> (Vec<Rect>, f32) {
    let s = ui_scale(screen);
    let slot = 46.0 * s;
    let gap = 3.0 * s;
    let total = HOTBAR_SLOTS as f32 * (slot + gap) - gap;
    let x0 = (screen[0] - total) * 0.5;
    let y0 = screen[1] - slot - 14.0 * s;
    let mut rects = Vec::with_capacity(HOTBAR_SLOTS);
    for i in 0..HOTBAR_SLOTS {
        rects.push([x0 + i as f32 * (slot + gap), y0, slot, slot]);
    }
    (rects, s)
}

/// Build the whole HUD for one frame.
pub fn build(game: &Game) -> Vec<UiVertex> {
    let mut ui = Ui::new(game.screen);
    let s = ui.scale;
    let [sw, sh] = game.screen;

    if game.dead {
        draw_death(&mut ui, game, s);
        return ui.finish();
    }

    if game.paused {
        draw_pause(&mut ui, game, s);
        return ui.finish();
    }

    // ---- crosshair ------------------------------------------------------
    let cx = sw * 0.5;
    let cy = sh * 0.5;
    let cross = 8.0 * s;
    let thick = 2.0 * s;
    let cross_color = [1.0, 1.0, 1.0, 0.85];
    ui.rect(cx - cross, cy - thick * 0.5, cross * 2.0, thick, cross_color);
    ui.rect(cx - thick * 0.5, cy - cross, thick, cross * 2.0, cross_color);
    ui.rect(cx - cross, cy - thick * 0.5, cross * 2.0, thick, [0.0, 0.0, 0.0, 0.25]);

    // ---- break progress -------------------------------------------------
    if game.break_progress > 0.0 {
        let w = 54.0 * s;
        let h = 5.0 * s;
        let x = cx - w * 0.5;
        let y = cy + 18.0 * s;
        ui.rect(x - s, y - s, w + 2.0 * s, h + 2.0 * s, [0.0, 0.0, 0.0, 0.55]);
        ui.rect(x, y, w * game.break_progress.clamp(0.0, 1.0), h, ACCENT);
    }

    // ---- hotbar ---------------------------------------------------------
    let (slots, _) = hotbar_layout(game.screen);
    for (i, r) in slots.iter().enumerate() {
        let [x, y, w, h] = *r;
        ui.rect(x, y, w, h, SLOT_BG);
        ui.frame(x, y, w, h, 2.0 * s, [0.0, 0.0, 0.0, 0.45]);
        let block_id = game.hotbar[i];
        if block_id != block::id::AIR {
            let def = block::def(block_id);
            let pad = 6.0 * s;
            ui.tile(x + pad, y + pad, w - pad * 2.0, h - pad * 2.0, def.top, WHITE);
        }
        if game.mode == GameMode::Survival {
            let count = game.counts[block_id as usize];
            if count > 0 {
                let text = count.to_string();
                let tw = font::text_width(&text, s);
                ui.text(x + w - tw - 3.0 * s, y + h - 10.0 * s, s, WHITE, &text);
            }
        }
        ui.text(x + 4.0 * s, y + 3.0 * s, s, [1.0, 1.0, 1.0, 0.65], &(i + 1).to_string());
    }
    if let Some(r) = slots.get(game.selected) {
        ui.frame(r[0] - 2.0 * s, r[1] - 2.0 * s, r[2] + 4.0 * s, r[3] + 4.0 * s, 3.0 * s, ACCENT);
    }

    // ---- hearts and hunger ----------------------------------------------
    if game.mode == GameMode::Survival {
        draw_status_bars(&mut ui, game, &slots, s);
    }

    // ---- damage flash ----------------------------------------------------
    if game.hurt_flash > 0.0 {
        let a = (game.hurt_flash / 0.45).clamp(0.0, 1.0) * 0.35;
        ui.rect(0.0, 0.0, sw, sh, [0.65, 0.02, 0.02, a]);
    }

    // ---- selected block name -------------------------------------------
    let selected = game.selected_block();
    if selected != block::id::AIR {
        let name = block::name(selected).to_uppercase();
        let top = slots.first().map(|r| r[1]).unwrap_or(sh - 100.0 * s);
        ui.text_centered(cx, top - 20.0 * s, s, WHITE, &name);
    }

    // ---- toast ----------------------------------------------------------
    if let Some((text, _)) = &game.message {
        let y = sh - 150.0 * s;
        let w = font::text_width(text, s);
        ui.rect(cx - w * 0.5 - 10.0 * s, y - 5.0 * s, w + 20.0 * s, 18.0 * s, PANEL);
        ui.text_centered(cx, y, s, ACCENT, text);
    }

    // ---- palette --------------------------------------------------------
    if game.inventory_open {
        draw_palette(&mut ui, game, s);
    }

    // ---- debug ----------------------------------------------------------
    if game.debug {
        draw_debug(&mut ui, game, s);
    }

    if game.saved_flash > 0.0 {
        let a = (game.saved_flash / 1.5).clamp(0.0, 1.0);
        ui.text(sw - 110.0 * s, 14.0 * s, s, [0.7, 1.0, 0.7, a], "WORLD SAVED");
    }

    ui.finish()
}

fn draw_status_bars(ui: &mut Ui, game: &Game, slots: &[Rect], s: f32) {
    let Some(hotbar) = slots.first() else { return };
    let Some(last) = slots.last() else { return };
    let icon = 9.0 * s;
    let gap = 1.0 * s;
    let y = hotbar[1] - icon - 3.0 * s;

    // hearts, left to right
    let mut x = hotbar[0];
    for i in 0..10 {
        let value = game.health - (i as f32) * 2.0;
        let tile = if value >= 2.0 {
            textures::tiles::HEART_FULL
        } else if value >= 1.0 {
            textures::tiles::HEART_HALF
        } else {
            textures::tiles::HEART_EMPTY
        };
        ui.tile(x, y, icon, icon, tile, WHITE);
        x += icon + gap;
    }

    // hunger, right to left
    let mut x = last[0] + last[2] - icon;
    for i in 0..10 {
        let value = game.hunger - (i as f32) * 2.0;
        let tile = if value >= 2.0 {
            textures::tiles::FOOD_FULL
        } else if value >= 1.0 {
            textures::tiles::FOOD_HALF
        } else {
            textures::tiles::FOOD_EMPTY
        };
        ui.tile(x, y, icon, icon, tile, WHITE);
        x -= icon + gap;
    }

    if game.hunger < 3.0 {
        let warning = if (game.player.walk_time * 2.0).sin() > 0.0 {
            [1.0, 0.3, 0.3, 1.0]
        } else {
            WHITE
        };
        ui.text_centered(game.screen[0] * 0.5, y - 14.0 * s, s, warning, "STARVING");
    }
}

fn draw_death(ui: &mut Ui, game: &Game, s: f32) {
    let [sw, sh] = game.screen;
    ui.rect(0.0, 0.0, sw, sh, [0.35, 0.02, 0.02, 0.55]);
    ui.text_centered(sw * 0.5, sh * 0.32, 3.0 * s, WHITE, "YOU DIED!");
    ui.text_centered(
        sw * 0.5,
        sh * 0.32 + 42.0 * s,
        1.1 * s,
        [0.92, 0.9, 0.86, 1.0],
        &format!("{} MOBS DEFEATED", game.kills),
    );
    ui.text_centered(sw * 0.5, sh * 0.30 + 96.0 * s, 1.6 * s, ACCENT, "PRESS R TO RESPAWN");
    ui.text_centered(
        sw * 0.5,
        sh * 0.30 + 130.0 * s,
        s,
        [0.86, 0.9, 0.98, 0.85],
        "ESC RELEASES THE MOUSE",
    );
}

fn draw_palette(ui: &mut Ui, game: &Game, s: f32) {
    let (panel, slots) = palette_layout(game.screen);
    let [px, py, pw, ph] = panel;
    ui.rect(px - 3.0 * s, py - 3.0 * s, pw + 6.0 * s, ph + 6.0 * s, [0.0, 0.0, 0.0, 0.5]);
    ui.rect(px, py, pw, ph, PANEL);
    ui.frame(px, py, pw, ph, 2.0 * s, [1.0, 1.0, 1.0, 0.25]);
    ui.text(px + 16.0 * s, py + 10.0 * s, s * 1.3, WHITE, "BLOCK PALETTE");

    for (i, r) in slots.iter().enumerate() {
        let [x, y, w, h] = *r;
        ui.rect(x, y, w, h, SLOT_BG);
        ui.frame(x, y, w, h, s, [0.0, 0.0, 0.0, 0.35]);
        if let Some(&block_id) = block::INVENTORY_BLOCKS.get(i) {
            let def = block::def(block_id);
            let pad = 5.0 * s;
            ui.tile(x + pad, y + pad, w - pad * 2.0, h - pad * 2.0, def.top, WHITE);
            if game.mode == GameMode::Survival {
                let count = game.counts[block_id as usize];
                let text = count.to_string();
                let tw = font::text_width(&text, s);
                ui.text(x + w - tw - 3.0 * s, y + h - 10.0 * s, s, WHITE, &text);
            }
        }
    }

    let hint = "CLICK A BLOCK TO PUT IT IN THE ACTIVE SLOT";
    ui.text_centered(px + pw * 0.5, py + ph - 14.0 * s, s, [0.85, 0.88, 0.95, 1.0], hint);
}

fn draw_debug(ui: &mut Ui, game: &Game, s: f32) {
    let p = &game.player;
    let lines: Vec<(String, [f32; 4])> = vec![
        (
            format!(
                "MC-RS  {} FPS   {} MS",
                game.fps.round() as i32,
                if game.fps > 0.5 {
                    (1000.0 / game.fps).round() as i32
                } else {
                    0
                }
            ),
            ACCENT,
        ),
        (
            format!(
                "XYZ {:.1} / {:.1} / {:.1}",
                p.pos.x, p.pos.y, p.pos.z
            ),
            WHITE,
        ),
        (
            format!(
                "CHUNK {} {}   BIOME {}",
                (p.pos.x.floor() as i32).div_euclid(16),
                (p.pos.z.floor() as i32).div_euclid(16),
                game.biome.name().to_uppercase()
            ),
            WHITE,
        ),
        (
            format!("TIME {} ({})", game.clock_string(), game.time_of_day().to_uppercase()),
            WHITE,
        ),
        (
            format!(
                "MODE {}   {}   {}",
                game.mode.name().to_uppercase(),
                if p.flying { "FLYING" } else { "WALKING" },
                if p.on_ground { "GROUND" } else { "AIR" }
            ),
            WHITE,
        ),
        (
            format!(
                "CHUNKS {} LOADED / {} MESHED   DRAWS {}   TRIS {}",
                game.loaded_chunks, game.meshed_chunks, game.draw_calls, game.triangles_drawn
            ),
            WHITE,
        ),
        (
            format!(
                "HEALTH {:.1}/20   HUNGER {:.1}/20   MOBS {}   KILLS {}",
                game.health,
                game.hunger,
                game.mobs_alive(),
                game.kills
            ),
            WHITE,
        ),
        (
            format!("SEED {}", game.world.seed()),
            [0.8, 0.85, 0.95, 1.0],
        ),
    ];

    let mut target_line = "LOOKING AT -".to_string();
    if let Some(hit) = game.target {
        let id = game.world.get(hit.block[0], hit.block[1], hit.block[2]);
        target_line = format!(
            "TARGET {} @ {},{},{}",
            block::name(id).to_uppercase(),
            hit.block[0],
            hit.block[1],
            hit.block[2]
        );
    }

    let x = 10.0 * s;
    let mut y = 10.0 * s;
    ui.rect(
        x - 5.0 * s,
        y - 5.0 * s,
        330.0 * s,
        8.0 * s * (lines.len() as f32 + 1.6),
        [0.0, 0.0, 0.0, 0.42],
    );
    for (line, color) in lines {
        ui.text(x, y, s, color, &line);
        y += 9.0 * s;
    }
    ui.text(x, y, s, [0.85, 0.9, 1.0, 0.9], &target_line);
}

fn draw_pause(ui: &mut Ui, game: &Game, s: f32) {
    let [sw, sh] = game.screen;
    ui.rect(0.0, 0.0, sw, sh, [0.02, 0.03, 0.05, 0.62]);
    ui.text_centered(sw * 0.5, sh * 0.30, 3.2 * s, WHITE, "MC-RS");
    ui.text_centered(
        sw * 0.5,
        sh * 0.30 + 40.0 * s,
        1.2 * s,
        ACCENT,
        "A MINECRAFT CLONE WRITTEN IN RUST",
    );
    ui.text_centered(sw * 0.5, sh * 0.44, 1.6 * s, WHITE, "CLICK TO PLAY");

    let controls = [
        "WASD MOVE    SPACE JUMP    SHIFT SNEAK/DESCEND",
        "CTRL SPRINT    F FLY (CREATIVE)    G GAME MODE",
        "LEFT CLICK BREAK    RIGHT CLICK PLACE    MIDDLE PICK",
        "1-9 / WHEEL HOTBAR    E BLOCK PALETTE",
        "LEFT CLICK HIT MOBS  -  F3 DEBUG  -  ESC RELEASE MOUSE",
    ];
    let mut y = sh * 0.56;
    for line in controls {
        ui.text_centered(sw * 0.5, y, s, [0.86, 0.9, 0.98, 1.0], line);
        y += 14.0 * s;
    }
}
