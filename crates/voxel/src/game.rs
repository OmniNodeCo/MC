//! Game rules: player movement, block interaction, inventory, day/night cycle
//! and chunk streaming. Completely platform independent.

use glam::Vec3;

use crate::block::{self, id};
use crate::chunk::{chunk_of, ChunkPos, SEA_LEVEL};
use crate::noise::{smoothstep, Rng};
use crate::particles::Particles;
use crate::physics::{self, Player};
use crate::raycast::{self, RayHit};
use crate::world::World;
use crate::worldgen::{self, Biome};

/// Seconds for one full day/night cycle.
pub const DAY_LENGTH: f32 = 900.0;
/// How far the player can reach to break / place blocks.
pub const REACH: f32 = 5.5;
pub const HOTBAR_SLOTS: usize = 9;

const TAU: f32 = std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Creative,
    Survival,
}

impl GameMode {
    pub fn name(self) -> &'static str {
        match self {
            GameMode::Creative => "Creative",
            GameMode::Survival => "Survival",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            GameMode::Creative => GameMode::Survival,
            GameMode::Survival => GameMode::Creative,
        }
    }
}

/// Everything the platform layer feeds into the game for one frame.
#[derive(Default, Clone, Debug)]
pub struct Input {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    /// Left mouse button held (mining).
    pub break_held: bool,
    /// Right mouse button pressed this frame (place).
    pub place: bool,
    /// Middle mouse button pressed this frame (pick block).
    pub pick: bool,
    pub wheel: f32,
    pub hotbar: Option<usize>,
    pub toggle_fly: bool,
    pub toggle_mode: bool,
    pub toggle_debug: bool,
    pub toggle_inventory: bool,
    pub pause: bool,
    pub respawn: bool,
    pub save: bool,
    /// Click position in pixels, when the cursor is free.
    pub click: Option<[f32; 2]>,
    /// False while the pointer is not captured (menu / pause).
    pub cursor_locked: bool,
}

impl Input {
    /// Drop one-shot events, keeping held state. Platforms call this after a
    /// frame has been consumed.
    pub fn clear_events(&mut self) {
        self.place = false;
        self.pick = false;
        self.wheel = 0.0;
        self.hotbar = None;
        self.toggle_fly = false;
        self.toggle_mode = false;
        self.toggle_debug = false;
        self.toggle_inventory = false;
        self.pause = false;
        self.respawn = false;
        self.save = false;
        self.click = None;
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
    }
}

pub struct Game {
    pub world: World,
    pub player: Player,
    pub mode: GameMode,
    /// 0.0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight.
    pub time: f32,
    pub hotbar: [u8; HOTBAR_SLOTS],
    pub selected: usize,
    /// Survival inventory counts per block id.
    pub counts: [u32; 256],
    pub break_progress: f32,
    pub break_target: Option<[i32; 3]>,
    pub target: Option<RayHit>,
    pub particles: Particles,
    pub debug: bool,
    pub inventory_open: bool,
    pub paused: bool,
    pub render_distance: i32,
    pub loaded_chunks: usize,
    pub quads: usize,
    pub triangles_drawn: usize,
    pub draw_calls: usize,
    pub meshed_chunks: usize,
    pub fps: f32,
    pub biome: Biome,
    pub message: Option<(String, f32)>,
    pub swing: f32,
    pub screen: [f32; 2],
    rng: Rng,
    biome_timer: f32,
    pub saved_flash: f32,
}

impl Game {
    pub fn new(seed: u32, render_distance: i32) -> Self {
        let mut world = World::new(seed);
        let (sx, sz) = worldgen::find_spawn(seed);
        let center = chunk_of(sx, sz);
        for dz in -2..=2 {
            for dx in -2..=2 {
                world.load_chunk((center.0 + dx, center.1 + dz));
            }
        }
        let spawn = Game::surface_spawn(&world, sx, sz);
        let mut player = Player::new(spawn);
        player.spawn = spawn;
        player.yaw = 0.35;

        let biome = worldgen::column(seed, sx, sz).biome;
        Self {
            world,
            player,
            mode: GameMode::Creative,
            time: 0.12,
            hotbar: [
                id::GRASS,
                id::DIRT,
                id::STONE,
                id::COBBLESTONE,
                id::PLANKS,
                id::OAK_LOG,
                id::GLASS,
                id::SAND,
                id::SNOW,
            ],
            selected: 0,
            counts: [0; 256],
            break_progress: 0.0,
            break_target: None,
            target: None,
            particles: Particles::default(),
            debug: false,
            inventory_open: false,
            paused: true,
            render_distance: render_distance.clamp(2, 16),
            loaded_chunks: 0,
            quads: 0,
            triangles_drawn: 0,
            draw_calls: 0,
            meshed_chunks: 0,
            fps: 0.0,
            biome,
            message: Some(("Press E for the block palette - G toggles survival".into(), 6.0)),
            swing: 1.0,
            screen: [1280.0, 720.0],
            rng: Rng::new(seed ^ 0xA5A5_1234),
            biome_timer: 0.0,
            saved_flash: 0.0,
        }
    }

    fn surface_spawn(world: &World, x: i32, z: i32) -> Vec3 {
        for radius in 0..12 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let wx = x + dx * 3;
                    let wz = z + dz * 3;
                    let top = world.surface_height(wx, wz);
                    if top <= SEA_LEVEL {
                        continue;
                    }
                    if block::is_solid(world.get(wx, top + 1, wz)) {
                        continue;
                    }
                    return Vec3::new(wx as f32 + 0.5, (top + 1) as f32, wz as f32 + 0.5);
                }
            }
        }
        Vec3::new(x as f32 + 0.5, 80.0, z as f32 + 0.5)
    }

    pub fn set_screen(&mut self, w: f32, h: f32) {
        self.screen = [w.max(1.0), h.max(1.0)];
    }

    pub fn selected_block(&self) -> u8 {
        self.hotbar[self.selected.min(HOTBAR_SLOTS - 1)]
    }

    pub fn notify(&mut self, text: impl Into<String>) {
        self.message = Some((text.into(), 3.5));
    }

    /// Distance to the current view distance, used by the fog.
    pub fn fog_distance(&self) -> f32 {
        (self.render_distance as f32) * 16.0 * 0.92
    }

    /// Generate / unload chunks around the player.
    pub fn stream_chunks(&mut self) {
        let center = chunk_of(
            self.player.pos.x.floor() as i32,
            self.player.pos.z.floor() as i32,
        );
        let radius = self.render_distance;

        let mut wanted: Vec<(i32, ChunkPos)> = Vec::new();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let dist_sq = dx * dx + dz * dz;
                if dist_sq > (radius + 1) * (radius + 1) {
                    continue;
                }
                let pos = (center.0 + dx, center.1 + dz);
                if !self.world.is_loaded(pos) {
                    wanted.push((dist_sq, pos));
                }
            }
        }
        wanted.sort_unstable_by_key(|(d, p)| (*d, *p));

        let budget = if self.world.loaded_count() < 30 { 12 } else { 2 };
        for (_, pos) in wanted.into_iter().take(budget) {
            self.world.load_chunk(pos);
        }
        self.world.unload_outside(center, radius + 2);
        self.loaded_chunks = self.world.loaded_count();
    }

    /// Advance one frame.
    pub fn update(&mut self, dt: f32, input: &mut Input) {
        let dt = dt.clamp(0.0, 0.1);
        self.saved_flash = (self.saved_flash - dt).max(0.0);

        // --- one-shot commands -------------------------------------------
        if input.toggle_debug {
            self.debug = !self.debug;
        }
        if input.toggle_inventory {
            self.inventory_open = !self.inventory_open;
            self.break_progress = 0.0;
        }
        if input.toggle_mode {
            self.mode = self.mode.toggled();
            if self.mode == GameMode::Survival {
                self.player.flying = false;
            }
            let text = format!("{} mode", self.mode.name());
            self.notify(text);
        }
        if input.toggle_fly && self.mode == GameMode::Creative {
            self.player.flying = !self.player.flying;
            let text = if self.player.flying {
                "Flying enabled"
            } else {
                "Flying disabled"
            };
            self.notify(text);
        }
        if let Some(slot) = input.hotbar {
            self.selected = slot.min(HOTBAR_SLOTS - 1);
        }
        if input.wheel != 0.0 {
            let dir = if input.wheel > 0.0 { 1 } else { -1 };
            let n = HOTBAR_SLOTS as i32;
            self.selected = (((self.selected as i32 + dir) % n + n) % n) as usize;
        }
        if input.pause {
            self.inventory_open = false;
        }
        if input.respawn {
            self.respawn();
        }
        if let Some(click) = input.click {
            self.handle_click(click);
        }
        if input.save {
            self.notify("Saving...");
        }

        self.paused = !input.cursor_locked;
        // Keep streaming chunks while the menu is up so the world fills in.
        self.stream_chunks();

        if self.paused {
            self.player.vel = Vec3::ZERO;
            return;
        }

        // --- camera -------------------------------------------------------
        if !self.inventory_open {
            self.player
                .rotate(input.mouse_dx, input.mouse_dy, 0.0024);
        }

        self.update_movement(dt, input);
        self.update_interaction(dt, input);
        self.particles.update(dt, &self.world);

        // --- world state --------------------------------------------------
        self.time = (self.time + dt / DAY_LENGTH).fract();
        if self.message.as_ref().map(|(_, t)| *t <= 0.0).unwrap_or(false) {
            self.message = None;
        }
        if let Some((_, t)) = &mut self.message {
            *t -= dt;
        }

        self.biome_timer -= dt;
        if self.biome_timer <= 0.0 {
            self.biome_timer = 0.5;
            let x = self.player.pos.x.floor() as i32;
            let z = self.player.pos.z.floor() as i32;
            self.biome = worldgen::column(self.world.seed(), x, z).biome;
        }

        if self.player.pos.y < -24.0 {
            self.respawn();
        }

        self.stream_chunks();
    }

    fn respawn(&mut self) {
        let mut pos = self.player.spawn;
        let top = self.world.surface_height(
            pos.x.floor() as i32,
            pos.z.floor() as i32,
        );
        pos.y = (top + 1) as f32 + 0.2;
        self.player.pos = pos;
        self.player.vel = Vec3::ZERO;
        self.break_progress = 0.0;
        self.notify("Respawned");
    }

    fn update_movement(&mut self, dt: f32, input: &mut Input) {
        let player = &mut self.player;

        let feet = player.pos + Vec3::new(0.0, 0.2, 0.0);
        let in_water = self
            .world
            .is_liquid(feet.x.floor() as i32, feet.y.floor() as i32, feet.z.floor() as i32);
        player.in_water = in_water;
        let eye = player.eye();
        player.head_in_water = self.world.is_liquid(
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );

        let mut wish = Vec3::ZERO;
        if input.forward {
            wish += player.forward();
        }
        if input.back {
            wish -= player.forward();
        }
        if input.right {
            wish += player.right();
        }
        if input.left {
            wish -= player.right();
        }
        let wish = wish.normalize_or_zero();

        player.sneaking = input.sneak && !player.flying;
        player.sprinting = input.sprint && wish.length_squared() > 0.5 && !player.sneaking;

        let target_speed = if player.flying {
            if player.sprinting {
                physics::FLY_SPRINT_SPEED
            } else {
                physics::FLY_SPEED
            }
        } else if in_water {
            physics::SWIM_SPEED
        } else if player.sneaking {
            physics::SNEAK_SPEED
        } else if player.sprinting {
            physics::SPRINT_SPEED
        } else {
            physics::WALK_SPEED
        };

        let accel = if player.flying {
            14.0
        } else if in_water {
            10.0
        } else if player.on_ground {
            26.0
        } else {
            6.0
        };
        let desired = wish * target_speed;
        let horiz = Vec3::new(player.vel.x, 0.0, player.vel.z);
        let new_horiz = horiz.lerp(desired, (accel * dt).min(1.0));
        player.vel.x = new_horiz.x;
        player.vel.z = new_horiz.z;

        if player.flying {
            player.vel.y = if input.jump {
                target_speed * 0.75
            } else if input.sneak {
                -target_speed * 0.75
            } else {
                0.0
            };
        } else if in_water {
            player.vel.y -= 9.0 * dt;
            if input.jump {
                player.vel.y = 4.2;
            }
            player.vel.y = player.vel.y.clamp(-4.5, 5.0);
        } else {
            player.vel.y -= physics::GRAVITY * dt;
            player.vel.y = player.vel.y.max(-physics::TERMINAL_VELOCITY);
            if input.jump && player.on_ground {
                player.vel.y = physics::JUMP_VELOCITY;
                player.on_ground = false;
            }
        }

        let result = physics::move_and_collide(
            &self.world,
            &mut player.pos,
            &mut player.vel,
            dt,
        );
        player.on_ground = result.on_ground;
        if player.on_ground && player.vel.y < 0.0 {
            player.vel.y = 0.0;
        }
        if result.hit_ceiling && player.vel.y > 0.0 {
            player.vel.y = 0.0;
        }

        // eye height easing, sprint fov / walk bob bookkeeping
        let target_eye = if player.sneaking {
            physics::SNEAK_EYE_HEIGHT
        } else {
            physics::EYE_HEIGHT
        };
        player.eye_height += (target_eye - player.eye_height) * (dt * 12.0).min(1.0);

        let speed = Vec3::new(player.vel.x, 0.0, player.vel.z).length();
        if player.on_ground && speed > 0.5 {
            player.walk_time += dt * speed * 1.6;
            player.bob = (player.walk_time * 2.0).sin() * 0.035;
        } else {
            player.bob *= 1.0 - (dt * 6.0).min(1.0);
        }

        self.swing = (self.swing + dt * 3.5).min(1.0);
    }

    fn update_interaction(&mut self, dt: f32, input: &mut Input) {
        if self.inventory_open {
            self.target = None;
            self.break_progress = 0.0;
            return;
        }

        let eye = self.player.eye();
        let hit = raycast::raycast(&self.world, eye, self.player.look_dir(), REACH);
        self.target = hit;

        if input.pick {
            if let Some(hit) = hit {
                let picked = hit.block;
                let target_id = self.world.get(picked[0], picked[1], picked[2]);
                if target_id != id::AIR {
                    self.hotbar[self.selected] = target_id;
                    let text = format!("Picked {}", block::name(target_id));
                    self.notify(text);
                }
            }
        }

        if input.place {
            self.try_place();
        }

        if input.break_held {
            if let Some(hit) = hit {
                if self.break_target != Some(hit.block) {
                    self.break_target = Some(hit.block);
                    self.break_progress = 0.0;
                }
                let block_id = self.world.get(hit.block[0], hit.block[1], hit.block[2]);
                let hardness = block::hardness(block_id).max(0.01);
                if self.mode == GameMode::Creative {
                    // A short cooldown so holding the mouse digs smoothly
                    // instead of deleting a column every frame.
                    if self.swing >= 1.0 {
                        self.break_block(hit.block, block_id);
                        self.swing = 0.0;
                    }
                    self.break_progress = 0.0;
                } else {
                    self.break_progress += dt / hardness;
                    if self.break_progress >= 1.0 {
                        self.break_block(hit.block, block_id);
                        self.break_progress = 0.0;
                    }
                }
            } else {
                self.break_progress = 0.0;
                self.break_target = None;
            }
        } else {
            self.break_progress = 0.0;
            self.break_target = None;
        }
    }

    fn try_place(&mut self) {
        let Some(hit) = self.target else { return };
        let place_id = self.selected_block();
        if place_id == id::AIR {
            return;
        }
        if self.mode == GameMode::Survival && self.counts[place_id as usize] == 0 {
            self.notify(format!("No {} left", block::name(place_id).to_lowercase()));
            return;
        }
        let pos = hit.placement();
        let existing = self.world.get(pos[0], pos[1], pos[2]);
        if !(existing == id::AIR || block::is_plant(existing) || block::is_liquid(existing)) {
            return;
        }
        // Never place a solid block inside the player.
        if block::is_solid(place_id) {
            let pb = self.player.aabb();
            let bb = physics::Aabb::new(
                Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32),
                Vec3::new(pos[0] as f32 + 1.0, pos[1] as f32 + 1.0, pos[2] as f32 + 1.0),
            );
            if bb.intersects(&pb) {
                return;
            }
        }
        if self.world.set(pos[0], pos[1], pos[2], place_id) {
            if self.mode == GameMode::Survival {
                self.counts[place_id as usize] =
                    self.counts[place_id as usize].saturating_sub(1);
            }
            self.swing = 0.0;
        }
    }

    fn break_block(&mut self, pos: [i32; 3], block_id: u8) {
        if block_id == id::BEDROCK {
            return;
        }
        if self.world.set(pos[0], pos[1], pos[2], id::AIR) {
            self.particles
                .spawn_break(pos, block_id, &mut self.rng);
            if self.mode == GameMode::Survival {
                self.counts[block_id as usize] = self.counts[block_id as usize].saturating_add(1);
            }
        }
    }

    fn handle_click(&mut self, pos: [f32; 2]) {
        if !self.inventory_open {
            return;
        }
        if let Some(index) = crate::ui::palette_hit(self.screen, pos) {
            if let Some(&block_id) = block::INVENTORY_BLOCKS.get(index) {
                self.hotbar[self.selected] = block_id;
                let text = format!("{} -> slot {}", block::name(block_id), self.selected + 1);
                self.notify(text);
                self.inventory_open = false;
            }
        }
    }

    // ---------------------------------------------------------------------
    // Sky
    // ---------------------------------------------------------------------

    /// Unit vector pointing at the sun.
    pub fn sun_dir(&self) -> Vec3 {
        let a = self.time * TAU;
        Vec3::new(a.cos() * 0.42, a.sin(), a.cos() * 0.88).normalize_or_zero()
    }

    /// 0.12 at midnight, 1.0 at noon.
    pub fn daylight(&self) -> f32 {
        let y = self.sun_dir().y;
        0.12 + 0.88 * smoothstep(-0.14, 0.30, y)
    }

    /// Colour of the sky at the horizon / the fog colour.
    pub fn sky_color(&self) -> [f32; 3] {
        let day = [0.44, 0.66, 0.98];
        let night = [0.015, 0.02, 0.06];
        let dusk = [0.95, 0.45, 0.22];
        let dl = self.daylight();
        let mut c = lerp3(night, day, dl);
        let sun_y = self.sun_dir().y;
        let horizon = (1.0 - sun_y.abs()).powi(2) * if sun_y > -0.25 { 1.0 } else { 0.15 };
        c = lerp3(c, dusk, horizon * 0.65);
        c
    }

    pub fn fog_color(&self) -> [f32; 3] {
        let sky = self.sky_color();
        lerp3(sky, [0.62, 0.72, 0.86], 0.25)
    }

    pub fn clock_string(&self) -> String {
        // 0.0 = 06:00 in Minecraft terms
        let hours = (self.time * 24.0 + 6.0) % 24.0;
        let h = hours.floor() as u32;
        let m = ((hours - h as f32) * 60.0) as u32;
        format!("{h:02}:{m:02}")
    }

    pub fn time_of_day(&self) -> &'static str {
        let y = self.sun_dir().y;
        if y > 0.25 {
            "Day"
        } else if y > -0.05 {
            "Dusk"
        } else if y > -0.45 {
            "Night"
        } else {
            "Midnight"
        }
    }
}

#[inline]
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(1234, 3)
    }

    #[test]
    fn spawn_is_above_solid_ground() {
        let g = game();
        let x = g.player.pos.x.floor() as i32;
        let y = g.player.pos.y.floor() as i32;
        let z = g.player.pos.z.floor() as i32;
        assert!(
            block::is_solid(g.world.get(x, y - 1, z)),
            "spawn block below is {:?}",
            g.world.get(x, y - 1, z)
        );
        assert!(!block::is_solid(g.world.get(x, y, z)));
    }

    #[test]
    fn creative_break_is_instant() {
        let mut g = game();
        let x = g.player.pos.x.floor() as i32;
        let z = g.player.pos.z.floor() as i32;
        let y = g.player.pos.y.floor() as i32 - 1;
        g.player.pitch = -std::f32::consts::FRAC_PI_2 + 0.01;
        g.player.yaw = 0.0;
        let mut input = Input {
            cursor_locked: true,
            break_held: true,
            ..Default::default()
        };
        g.update(1.0 / 60.0, &mut input);
        assert_eq!(g.world.get(x, y, z), id::AIR);
    }

    #[test]
    fn survival_collects_blocks() {
        let mut g = game();
        g.mode = GameMode::Survival;
        g.player.pitch = -std::f32::consts::FRAC_PI_2 + 0.01;
        let mut input = Input {
            cursor_locked: true,
            break_held: true,
            ..Default::default()
        };
        for _ in 0..240 {
            g.update(1.0 / 60.0, &mut input);
        }
        let total: u32 = g.counts.iter().sum();
        assert!(total > 0, "expected some blocks in the inventory");
    }

    #[test]
    fn hotbar_wraps_around() {
        let mut g = game();
        g.selected = 0;
        let mut input = Input {
            cursor_locked: true,
            wheel: -1.0,
            ..Default::default()
        };
        g.update(1.0 / 60.0, &mut input);
        assert_eq!(g.selected, HOTBAR_SLOTS - 1);
    }

    #[test]
    fn daylight_is_darker_at_midnight() {
        let mut g = game();
        g.time = 0.25;
        let noon = g.daylight();
        g.time = 0.75;
        let midnight = g.daylight();
        assert!(noon > midnight);
        assert!(midnight > 0.0);
    }

    #[test]
    fn streaming_loads_chunks_around_the_player() {
        let mut g = game();
        g.stream_chunks();
        assert!(g.world.loaded_count() > 9);
        assert!(g.world.is_loaded((0, 0)) || g.world.loaded_count() > 20);
    }
}
