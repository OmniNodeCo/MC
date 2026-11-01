//! Mobs: blocky creatures in the style of Minecraft, with simple AI, their own
//! collision physics and a mesh builder that turns them into textured cuboids.

use glam::{Mat4, Vec3};

use crate::mesher::Vertex;
use crate::noise::Rng;
use crate::physics::Aabb;
use crate::textures;
use crate::world::World;

/// Hard cap on living mobs; spawning stops when it is reached.
pub const MAX_MOBS: usize = 72;

fn tau() -> f32 {
    std::f32::consts::TAU
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Pig,
    Cow,
    Sheep,
    Chicken,
    Zombie,
    Skeleton,
    Creeper,
    Spider,
}

impl MobKind {
    pub const ALL: [MobKind; 8] = [
        MobKind::Pig,
        MobKind::Cow,
        MobKind::Sheep,
        MobKind::Chicken,
        MobKind::Zombie,
        MobKind::Skeleton,
        MobKind::Creeper,
        MobKind::Spider,
    ];

    /// Passive animals that spawn on grass during the day.
    pub const PASSIVE: [MobKind; 4] = [
        MobKind::Pig,
        MobKind::Cow,
        MobKind::Sheep,
        MobKind::Chicken,
    ];

    /// Hostile mobs that spawn in the dark.
    pub const HOSTILE: [MobKind; 4] = [
        MobKind::Zombie,
        MobKind::Skeleton,
        MobKind::Creeper,
        MobKind::Spider,
    ];

    pub fn is_hostile(self) -> bool {
        matches!(
            self,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper | MobKind::Spider
        )
    }

    pub fn is_undead(self) -> bool {
        matches!(self, MobKind::Zombie | MobKind::Skeleton)
    }

    pub fn name(self) -> &'static str {
        match self {
            MobKind::Pig => "Pig",
            MobKind::Cow => "Cow",
            MobKind::Sheep => "Sheep",
            MobKind::Chicken => "Chicken",
            MobKind::Zombie => "Zombie",
            MobKind::Skeleton => "Skeleton",
            MobKind::Creeper => "Creeper",
            MobKind::Spider => "Spider",
        }
    }

    pub fn index(self) -> u32 {
        match self {
            MobKind::Pig => 0,
            MobKind::Cow => 1,
            MobKind::Sheep => 2,
            MobKind::Chicken => 3,
            MobKind::Zombie => 4,
            MobKind::Skeleton => 5,
            MobKind::Creeper => 6,
            MobKind::Spider => 7,
        }
    }

    /// Base atlas tile for this mob: body, face, limb, accent.
    pub fn tile_base(self) -> u32 {
        textures::tiles::MOB_BASE + self.index() * textures::tiles::MOB_TILES
    }

    pub fn width(self) -> f32 {
        match self {
            MobKind::Pig | MobKind::Sheep => 0.9,
            MobKind::Cow => 1.0,
            MobKind::Chicken => 0.5,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 0.6,
            MobKind::Spider => 1.2,
        }
    }

    pub fn height(self) -> f32 {
        match self {
            MobKind::Chicken => 0.72,
            MobKind::Pig | MobKind::Sheep => 0.95,
            MobKind::Cow => 1.32,
            MobKind::Spider => 0.85,
            MobKind::Zombie | MobKind::Skeleton => 1.95,
            MobKind::Creeper => 1.66,
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Pig => 10.0,
            MobKind::Cow => 10.0,
            MobKind::Sheep => 8.0,
            MobKind::Chicken => 4.0,
            MobKind::Zombie => 20.0,
            MobKind::Skeleton => 20.0,
            MobKind::Creeper => 20.0,
            MobKind::Spider => 16.0,
        }
    }

    pub fn speed(self) -> f32 {
        match self {
            MobKind::Chicken => 1.1,
            MobKind::Pig | MobKind::Sheep | MobKind::Cow => 1.25,
            MobKind::Zombie => 2.1,
            MobKind::Creeper => 2.3,
            MobKind::Skeleton => 2.4,
            MobKind::Spider => 3.3,
        }
    }

    pub fn attack_damage(self) -> f32 {
        match self {
            MobKind::Zombie => 3.0,
            MobKind::Skeleton => 2.0,
            MobKind::Spider => 2.0,
            MobKind::Creeper => 9.0,
            _ => 0.0,
        }
    }

    /// Seconds between attacks.
    pub fn attack_interval(self) -> f32 {
        match self {
            MobKind::Skeleton => 2.2,
            MobKind::Zombie => 1.0,
            MobKind::Spider => 0.9,
            _ => 1.2,
        }
    }

    pub fn reach(self) -> f32 {
        match self {
            MobKind::Skeleton => 13.0,
            MobKind::Spider => 2.2,
            _ => 2.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mob {
    pub kind: MobKind,
    /// Feet position, centre of the AABB on the horizontal plane.
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub on_ground: bool,
    pub hurt_timer: f32,
    pub attack_timer: f32,
    pub walk_phase: f32,
    pub wander_timer: f32,
    pub target_yaw: f32,
    pub moving: bool,
    /// Creeper fuse, 0 when not priming.
    pub fuse: f32,
    age: f32,
    pub alive: bool,
    blocked: bool,
}

impl Mob {
    pub fn new(kind: MobKind, pos: Vec3, rng: &mut Rng) -> Self {
        Self {
            kind,
            pos,
            vel: Vec3::ZERO,
            yaw: rng.next_f32() * tau(),
            health: kind.max_health(),
            on_ground: false,
            hurt_timer: 0.0,
            attack_timer: rng.next_f32() * 0.6,
            walk_phase: rng.next_f32() * tau(),
            wander_timer: 0.5 + rng.next_f32() * 3.0,
            target_yaw: rng.next_f32() * tau(),
            moving: true,
            fuse: 0.0,
            age: 0.0,
            alive: true,
            blocked: false,
        }
    }

    pub fn aabb(&self) -> Aabb {
        let half = self.kind.width() * 0.5;
        Aabb::new(
            Vec3::new(self.pos.x - half, self.pos.y, self.pos.z - half),
            Vec3::new(
                self.pos.x + half,
                self.pos.y + self.kind.height(),
                self.pos.z + half,
            ),
        )
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, self.kind.height() * 0.85, 0.0)
    }

    pub fn hit(&mut self, damage: f32, from: Vec3) -> bool {
        self.health -= damage;
        self.hurt_timer = 0.35;
        let away = (self.pos - from).normalize_or_zero();
        self.vel += away * 5.0 + Vec3::new(0.0, 4.2, 0.0);
        if self.health <= 0.0 {
            self.alive = false;
            true
        } else {
            false
        }
    }
}

/// What a mob wants to do to the world this tick.
pub enum MobAction {
    None,
    Attack(f32),
    Explode,
}

fn collides(world: &World, pos: Vec3, half: f32, height: f32) -> bool {
    let bb = Aabb::new(
        pos - Vec3::new(half, 0.0, half),
        pos + Vec3::new(half, height, half),
    );
    bb.collides_with_world(world)
}

/// Integrate physics for one mob: gravity, axis separated collision and a small
/// automatic hop so mobs can climb one block steps like in Minecraft.
pub fn physics_step(world: &World, mob: &mut Mob, dt: f32) {
    let half = mob.kind.width() * 0.5;
    let height = mob.kind.height();
    mob.blocked = false;

    let in_water = world.is_liquid(
        mob.pos.x.floor() as i32,
        (mob.pos.y + 0.2).floor() as i32,
        mob.pos.z.floor() as i32,
    );

    if in_water {
        mob.vel.y += 12.0 * dt;
        mob.vel.y = mob.vel.y.min(3.0);
    } else {
        mob.vel.y -= 26.0 * dt;
        mob.vel.y = mob.vel.y.max(-50.0);
    }

    let next_x = mob.pos.x + mob.vel.x * dt;
    if !collides(world, Vec3::new(next_x, mob.pos.y, mob.pos.z), half, height) {
        mob.pos.x = next_x;
    } else {
        mob.vel.x = 0.0;
        mob.blocked = true;
    }
    let next_z = mob.pos.z + mob.vel.z * dt;
    if !collides(world, Vec3::new(mob.pos.x, mob.pos.y, next_z), half, height) {
        mob.pos.z = next_z;
    } else {
        mob.vel.z = 0.0;
        mob.blocked = true;
    }

    let next_y = mob.pos.y + mob.vel.y * dt;
    if !collides(world, Vec3::new(mob.pos.x, next_y, mob.pos.z), half, height) {
        mob.pos.y = next_y;
        mob.on_ground = false;
    } else {
        if mob.vel.y < 0.0 {
            mob.on_ground = true;
        }
        mob.vel.y = 0.0;
    }
    if !mob.on_ground {
        mob.on_ground = collides(
            world,
            mob.pos - Vec3::new(0.0, 0.06, 0.0),
            half,
            height,
        );
    }
    if mob.on_ground && mob.blocked {
        mob.vel.y = 7.4;
        mob.on_ground = false;
    }

    let speed = Vec3::new(mob.vel.x, 0.0, mob.vel.z).length();
    mob.walk_phase += dt * (1.0 + speed * 2.4);
    mob.walk_phase %= tau();
}

fn angle_lerp(from: f32, to: f32, t: f32) -> f32 {
    let mut delta = (to - from) % tau();
    if delta > std::f32::consts::PI {
        delta -= tau();
    } else if delta < -std::f32::consts::PI {
        delta += tau();
    }
    from + delta * t.clamp(0.0, 1.0)
}

/// One AI tick. Returns what the mob does to the player / world.
pub fn think(
    world: &World,
    mob: &mut Mob,
    player: Vec3,
    player_alive: bool,
    daylight: f32,
    dt: f32,
    rng: &mut Rng,
) -> MobAction {
    mob.hurt_timer = (mob.hurt_timer - dt).max(0.0);
    mob.attack_timer = (mob.attack_timer - dt).max(0.0);
    mob.age += dt;

    let flat = Vec3::new(player.x - mob.pos.x, 0.0, player.z - mob.pos.z);
    let dist = flat.length();

    // Undead burn in daylight when they are not under cover.
    if mob.kind.is_undead() && daylight > 0.8 {
        let exposed = mob.pos.y
            > world.surface_height(mob.pos.x.floor() as i32, mob.pos.z.floor() as i32) as f32;
        if exposed {
            mob.health -= dt * 1.5;
            mob.hurt_timer = 0.2;
            if mob.health <= 0.0 {
                mob.alive = false;
            }
        }
    }

    let chasing = mob.kind.is_hostile() && player_alive && dist < 26.0;
    let mut desired = Vec3::ZERO;

    if chasing {
        desired = flat.normalize_or_zero();
        if mob.kind == MobKind::Skeleton {
            // Archers keep their distance.
            if dist < 5.0 {
                desired = -desired;
            } else if dist < 9.0 {
                desired = Vec3::ZERO;
            }
        }
        if mob.kind == MobKind::Creeper && dist < 2.4 {
            desired = Vec3::ZERO;
        }
    } else {
        mob.wander_timer -= dt;
        if mob.wander_timer <= 0.0 {
            mob.wander_timer = 2.5 + rng.next_f32() * 6.0;
            mob.target_yaw = if rng.next_f32() < 0.35 {
                mob.yaw + std::f32::consts::PI
            } else {
                rng.next_f32() * tau()
            };
            mob.moving = rng.next_f32() < 0.72;
        }
        if mob.moving {
            desired = Vec3::new(mob.target_yaw.sin(), 0.0, mob.target_yaw.cos());
        }
    }

    let speed = mob.kind.speed() * if chasing { 1.0 } else { 0.6 };
    let wish = desired * speed;
    let blend = (6.0 * dt).min(1.0);
    mob.vel.x += (wish.x - mob.vel.x) * blend;
    mob.vel.z += (wish.z - mob.vel.z) * blend;

    if desired.length_squared() > 1e-4 {
        mob.yaw = angle_lerp(mob.yaw, desired.x.atan2(desired.z), dt * 5.0);
    }

    physics_step(world, mob, dt);

    // ---- actions ---------------------------------------------------------
    if mob.kind == MobKind::Creeper {
        if chasing && dist < 2.6 && player_alive {
            mob.fuse += dt;
            if mob.fuse >= 1.4 {
                return MobAction::Explode;
            }
        } else {
            mob.fuse = (mob.fuse - dt).max(0.0);
        }
        return MobAction::None;
    }

    if mob.kind.is_hostile()
        && player_alive
        && dist < mob.kind.reach()
        && mob.attack_timer <= 0.0
        && (mob.pos.y - player.y).abs() < 2.5
    {
        mob.attack_timer = mob.kind.attack_interval();
        return MobAction::Attack(mob.kind.attack_damage());
    }

    MobAction::None
}

// ---------------------------------------------------------------------------
// Meshing
// ---------------------------------------------------------------------------

/// Corner offsets (0/1 cube space) per face, wound counter clockwise when seen
/// from outside, matching the chunk mesher.
const FACE_CORNERS: [[[f32; 3]; 4]; 6] = [
    // top
    [
        [0.0, 1.0, 0.0],
        [0.0, 1.0, 1.0],
        [1.0, 1.0, 1.0],
        [1.0, 1.0, 0.0],
    ],
    // bottom
    [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ],
    // north
    [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    ],
    // south
    [
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0],
    ],
    // east
    [
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
    ],
    // west
    [
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
    ],
];

const FACE_UVS: [[[f32; 2]; 4]; 6] = [
    [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
    [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
];

fn push_triangle_quad(
    out: &mut Vec<Vertex>,
    points: [Vec3; 4],
    uvs: [[f32; 2]; 4],
    tile: u32,
    tint: f32,
    shade: f32,
) {
    let [u0, v0, u1, v1] = textures::tile_uv(tile);
    let order = [0usize, 1, 2, 0, 2, 3];
    for &i in order.iter() {
        out.push(Vertex {
            pos: points[i].to_array(),
            uv: [u0 + (u1 - u0) * uvs[i][0], v0 + (v1 - v0) * uvs[i][1]],
            shade: shade * tint,
            wave: 0.0,
        });
    }
}

/// Append a textured box. `center` and `size` are in the space of `transform`.
fn push_box(
    out: &mut Vec<Vertex>,
    transform: Mat4,
    center: Vec3,
    size: Vec3,
    tile: u32,
    tint: f32,
) {
    for (f, corners) in FACE_CORNERS.iter().enumerate() {
        let mut points = [Vec3::ZERO; 4];
        for (i, corner) in corners.iter().enumerate() {
            let local = (Vec3::from(*corner) - Vec3::splat(0.5)) * size + center;
            points[i] = transform.transform_point3(local);
        }
        let shade = match f {
            0 => 1.0,
            1 => 0.55,
            2 | 3 => 0.82,
            _ => 0.68,
        };
        push_triangle_quad(out, points, FACE_UVS[f], tile, tint, shade);
    }
}

/// Append a single quad facing `+Z` in the space of `transform`.
fn push_front(
    out: &mut Vec<Vertex>,
    transform: Mat4,
    center: Vec3,
    half: [f32; 2],
    tile: u32,
    tint: f32,
) {
    let [hw, hh] = half;
    let z = center.z + 0.002;
    let points = [
        transform.transform_point3(Vec3::new(center.x - hw, center.y - hh, z)),
        transform.transform_point3(Vec3::new(center.x - hw, center.y + hh, z)),
        transform.transform_point3(Vec3::new(center.x + hw, center.y + hh, z)),
        transform.transform_point3(Vec3::new(center.x + hw, center.y - hh, z)),
    ];
    let uvs = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
    push_triangle_quad(out, points, uvs, tile, tint, 0.95);
}

struct Proportions {
    body: Vec3,
    body_y: f32,
    head: Vec3,
    head_y: f32,
    head_z: f32,
    leg_h: f32,
    leg_w: f32,
    leg_gap: f32,
    legs: usize,
    arms: bool,
    /// Feet offset for birds / spiders.
    splay: f32,
}

fn quadruped(out: &mut Vec<Vertex>, mob: &Mob, tint: f32, swing: f32, p: &Proportions) {
    let base = Mat4::from_translation(mob.pos) * Mat4::from_rotation_y(mob.yaw);
    let tb = mob.kind.tile_base();
    let body_tile = tb;
    let face_tile = tb + 1;
    let limb_tile = tb + 2;
    let head_tile = tb + 3;

    // body
    push_box(
        out,
        base,
        Vec3::new(0.0, p.body_y, 0.0),
        p.body,
        body_tile,
        tint,
    );

    // head + face plane
    let head_y = p.head_y + swing.abs() * 0.02;
    push_box(
        out,
        base,
        Vec3::new(0.0, head_y, p.head_z),
        p.head,
        head_tile,
        tint,
    );
    push_front(
        out,
        base,
        Vec3::new(0.0, head_y, p.head_z + p.head.z * 0.5),
        [p.head.x * 0.5, p.head.y * 0.5],
        face_tile,
        tint,
    );

    // legs
    let hip = p.body_y - p.body.y * 0.5;
    for leg in 0..p.legs {
        let side = if leg % 2 == 0 { -1.0 } else { 1.0 };
        let front = if leg < 2 { 1.0 } else { -1.0 };
        let phase = if leg % 2 == 0 { swing } else { -swing };
        let pivot = Vec3::new(side * p.leg_gap, hip, front * p.splay);
        let angle = phase * 0.55;
        let leg_transform = base
            * Mat4::from_translation(pivot)
            * Mat4::from_rotation_x(angle);
        push_box(
            out,
            leg_transform,
            Vec3::new(0.0, -p.leg_h * 0.5, 0.0),
            Vec3::new(p.leg_w, p.leg_h, p.leg_w),
            limb_tile,
            tint,
        );
    }

    if p.arms {
        for arm in 0..2 {
            let side = if arm == 0 { -1.0 } else { 1.0 };
            let pivot = Vec3::new(side * (p.body.x * 0.5 + 0.08), p.body_y + p.body.y * 0.35, 0.0);
            let angle = -swing * 0.7;
            let arm_transform = base
                * Mat4::from_translation(pivot)
                * Mat4::from_rotation_x(angle);
            push_box(
                out,
                arm_transform,
                Vec3::new(0.0, -0.28, 0.0),
                Vec3::new(0.18, 0.55, 0.18),
                limb_tile,
                tint,
            );
        }
    }
}

fn pig_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.64, 0.5, 0.9),
        body_y: 0.5,
        head: Vec3::new(0.46, 0.46, 0.44),
        head_y: 0.62,
        head_z: 0.62,
        leg_h: 0.3,
        leg_w: 0.16,
        leg_gap: 0.2,
        legs: 4,
        arms: false,
        splay: 0.3,
    }
}

fn cow_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.74, 0.62, 1.15),
        body_y: 0.82,
        head: Vec3::new(0.48, 0.48, 0.46),
        head_y: 1.04,
        head_z: 0.78,
        leg_h: 0.5,
        leg_w: 0.2,
        leg_gap: 0.24,
        legs: 4,
        arms: false,
        splay: 0.44,
    }
}

fn sheep_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.7, 0.6, 0.95),
        body_y: 0.7,
        head: Vec3::new(0.42, 0.42, 0.42),
        head_y: 1.02,
        head_z: 0.66,
        leg_h: 0.38,
        leg_w: 0.16,
        leg_gap: 0.22,
        legs: 4,
        arms: false,
        splay: 0.36,
    }
}

fn chicken_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.4, 0.4, 0.46),
        body_y: 0.42,
        head: Vec3::new(0.26, 0.28, 0.26),
        head_y: 0.68,
        head_z: 0.14,
        leg_h: 0.24,
        leg_w: 0.1,
        leg_gap: 0.12,
        legs: 2,
        arms: false,
        splay: 0.08,
    }
}

fn zombie_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.5, 0.72, 0.26),
        body_y: 1.18,
        head: Vec3::new(0.5, 0.5, 0.5),
        head_y: 1.78,
        head_z: 0.0,
        leg_h: 0.72,
        leg_w: 0.18,
        leg_gap: 0.13,
        legs: 4,
        arms: true,
        splay: 0.0,
    }
}

fn skeleton_parts() -> Proportions {
    let mut p = zombie_parts();
    p.body = Vec3::new(0.42, 0.72, 0.22);
    p
}

fn creeper_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.5, 0.82, 0.34),
        body_y: 0.86,
        head: Vec3::new(0.5, 0.5, 0.5),
        head_y: 1.42,
        head_z: 0.0,
        leg_h: 0.4,
        leg_w: 0.2,
        leg_gap: 0.16,
        legs: 4,
        arms: false,
        splay: 0.02,
    }
}

fn spider_parts() -> Proportions {
    Proportions {
        body: Vec3::new(0.72, 0.42, 0.86),
        body_y: 0.44,
        head: Vec3::new(0.5, 0.36, 0.36),
        head_y: 0.42,
        head_z: 0.56,
        leg_h: 0.4,
        leg_w: 0.1,
        leg_gap: 0.34,
        legs: 4,
        arms: false,
        splay: 0.2,
    }
}

/// Build every visible mob as unindexed triangle soup.
pub fn build_vertices(mobs: &[Mob]) -> Vec<Vertex> {
    let mut out: Vec<Vertex> = Vec::with_capacity(mobs.len() * 220);
    for mob in mobs {
        if !mob.alive {
            continue;
        }
        let flash = if mob.hurt_timer > 0.0 { 1.55 } else { 1.0 };
        let mut tint = flash;
        if mob.kind == MobKind::Creeper && mob.fuse > 0.0 {
            tint = 1.0 + (mob.fuse * 6.0).sin().abs() * 0.6;
        }
        let swing = mob.walk_phase.sin();
        let parts = match mob.kind {
            MobKind::Pig => pig_parts(),
            MobKind::Cow => cow_parts(),
            MobKind::Sheep => sheep_parts(),
            MobKind::Chicken => chicken_parts(),
            MobKind::Zombie => zombie_parts(),
            MobKind::Skeleton => skeleton_parts(),
            MobKind::Creeper => creeper_parts(),
            MobKind::Spider => spider_parts(),
        };
        quadruped(&mut out, mob, tint, swing, &parts);
    }
    out
}

/// Ray against every mob, returning `(index, distance)` of the closest hit.
pub fn raycast_mobs(mobs: &[Mob], origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, mob) in mobs.iter().enumerate() {
        if !mob.alive {
            continue;
        }
        if let Some(t) = ray_aabb(origin, dir, &mob.aabb()) {
            if t <= max_dist && best.map(|(_, d)| t < d).unwrap_or(true) {
                best = Some((i, t));
            }
        }
    }
    best
}

fn ray_aabb(origin: Vec3, dir: Vec3, bb: &Aabb) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::INFINITY;
    let axes = [
        (origin.x, dir.x, bb.min.x, bb.max.x),
        (origin.y, dir.y, bb.min.y, bb.max.y),
        (origin.z, dir.z, bb.min.z, bb.max.z),
    ];
    for (o, d, lo, hi) in axes {
        if d.abs() < 1e-6 {
            if o < lo || o > hi {
                return None;
            }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (lo - o) * inv;
            let mut t2 = (hi - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

/// Damage a mob with knockback and return true when it died.
pub fn damage(mob: &mut Mob, amount: f32, from: Vec3) -> bool {
    mob.hit(amount, from)
}

/// Line of sight helper used by the archers.
pub fn clear_line(world: &World, from: Vec3, to: Vec3) -> bool {
    let delta = to - from;
    let len = delta.length();
    if len < 1e-3 {
        return true;
    }
    let dir = delta / len;
    let steps = (len * 2.0) as i32;
    for i in 1..steps {
        let p = from + dir * (i as f32 / steps as f32 * len);
        if world.is_solid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32) {
            return false;
        }
    }
    true
}
