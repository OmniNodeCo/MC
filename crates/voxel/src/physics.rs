//! Player body, AABB collision against the voxel grid and camera helpers.

use glam::Vec3;

use crate::block;
use crate::chunk::CHUNK_Y;
use crate::world::World;

pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const SNEAK_EYE_HEIGHT: f32 = 1.42;

pub const GRAVITY: f32 = 30.0;
pub const TERMINAL_VELOCITY: f32 = 78.0;
pub const JUMP_VELOCITY: f32 = 9.2;
pub const WALK_SPEED: f32 = 4.6;
pub const SPRINT_SPEED: f32 = 6.9;
pub const SNEAK_SPEED: f32 = 1.5;
pub const FLY_SPEED: f32 = 12.0;
pub const FLY_SPRINT_SPEED: f32 = 26.0;
pub const SWIM_SPEED: f32 = 3.4;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = size * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }

    pub fn collides_with_world(&self, world: &World) -> bool {
        let x0 = self.min.x.floor() as i32;
        let y0 = self.min.y.floor() as i32;
        let z0 = self.min.z.floor() as i32;
        let x1 = (self.max.x - 1e-4).floor() as i32;
        let y1 = (self.max.y - 1e-4).floor() as i32;
        let z1 = (self.max.z - 1e-4).floor() as i32;
        for y in y0..=y1 {
            if y < 0 || y >= CHUNK_Y as i32 {
                continue;
            }
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if block::is_solid(world.get(x, y, z)) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    /// Feet position (centre of the AABB on the horizontal plane).
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub sneaking: bool,
    pub sprinting: bool,
    pub in_water: bool,
    pub head_in_water: bool,
    pub eye_height: f32,
    /// View bob phase.
    pub bob: f32,
    pub walk_time: f32,
    pub spawn: Vec3,
}

impl Player {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            sneaking: false,
            sprinting: false,
            in_water: false,
            head_in_water: false,
            eye_height: EYE_HEIGHT,
            bob: 0.0,
            walk_time: 0.0,
            spawn: pos,
        }
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::new(
            Vec3::new(
                self.pos.x - PLAYER_WIDTH * 0.5,
                self.pos.y,
                self.pos.z - PLAYER_WIDTH * 0.5,
            ),
            Vec3::new(
                self.pos.x + PLAYER_WIDTH * 0.5,
                self.pos.y + PLAYER_HEIGHT,
                self.pos.z + PLAYER_WIDTH * 0.5,
            ),
        )
    }

    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.pos.y + self.eye_height, self.pos.z)
    }

    /// Unit vector the camera looks along.
    pub fn look_dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(-sy * cp, sp, -cy * cp)
    }

    /// Horizontal forward vector (yaw only).
    pub fn forward(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    /// Horizontal right vector.
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    pub fn rotate(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        self.yaw -= dx * sensitivity;
        self.pitch -= dy * sensitivity;
        let limit = std::f32::consts::FRAC_PI_2 - 0.001;
        self.pitch = self.pitch.clamp(-limit, limit);
        // keep yaw bounded so the f32 stays precise over long sessions
        if self.yaw > std::f32::consts::TAU {
            self.yaw -= std::f32::consts::TAU;
        } else if self.yaw < -std::f32::consts::TAU {
            self.yaw += std::f32::consts::TAU;
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct MoveResult {
    pub on_ground: bool,
    pub hit_ceiling: bool,
    pub hit_wall: bool,
}

pub fn collides_at(world: &World, pos: Vec3) -> bool {
    Aabb::new(
        Vec3::new(pos.x - PLAYER_WIDTH * 0.5, pos.y, pos.z - PLAYER_WIDTH * 0.5),
        Vec3::new(
            pos.x + PLAYER_WIDTH * 0.5,
            pos.y + PLAYER_HEIGHT,
            pos.z + PLAYER_WIDTH * 0.5,
        ),
    )
    .collides_with_world(world)
}

/// Integrate velocity with axis-by-axis, sub-stepped collision resolution.
pub fn move_and_collide(world: &World, pos: &mut Vec3, vel: &mut Vec3, dt: f32) -> MoveResult {
    let mut result = MoveResult::default();
    let mut p = pos.to_array();
    let mut v = vel.to_array();
    let delta = [v[0] * dt, v[1] * dt, v[2] * dt];

    let longest = delta[0].abs().max(delta[1].abs()).max(delta[2].abs());
    let steps = (longest / 0.05).ceil().clamp(1.0, 64.0) as i32;
    let mut step_delta = [delta[0] / steps as f32, delta[1] / steps as f32, delta[2] / steps as f32];

    for _ in 0..steps {
        for axis in 0..3 {
            let d = step_delta[axis];
            if d == 0.0 {
                continue;
            }
            p[axis] += d;
            if collides_at(world, Vec3::from_array(p)) {
                p[axis] -= d;
                if axis == 1 {
                    if d < 0.0 {
                        result.on_ground = true;
                    } else {
                        result.hit_ceiling = true;
                    }
                } else {
                    result.hit_wall = true;
                }
                v[axis] = 0.0;
                step_delta[axis] = 0.0;
            }
        }
    }

    *pos = Vec3::from_array(p);
    *vel = Vec3::from_array(v);

    if !result.on_ground && v[1] <= 0.0 {
        let probe = *pos + Vec3::new(0.0, -0.03, 0.0);
        if collides_at(world, probe) {
            result.on_ground = true;
        }
    }
    result
}

/// Lift the player out of blocks (spawn safety / breaking a block under
/// yourself is fine, but loading a save inside terrain is not).
pub fn unstick(world: &World, pos: &mut Vec3) {
    let mut tries = 0;
    while collides_at(world, *pos) && tries < 64 {
        pos.y += 0.25;
        tries += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::id;

    fn flat_world() -> World {
        let mut world = World::new(1);
        world.load_chunk((0, 0));
        // build a clean stone floor at y = 40
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..CHUNK_Y as i32 {
                    world.set(x, y, z, id::AIR);
                }
                world.set(x, 40, z, id::STONE);
            }
        }
        world
    }

    #[test]
    fn falls_until_it_lands() {
        let world = flat_world();
        let mut player = Player::new(Vec3::new(8.0, 60.0, 8.0));
        for _ in 0..240 {
            player.vel.y -= GRAVITY / 60.0;
            player.vel.y = player.vel.y.max(-TERMINAL_VELOCITY);
            let hit = move_and_collide(&world, &mut player.pos, &mut player.vel, 1.0 / 60.0);
            player.on_ground = hit.on_ground;
        }
        assert!(player.on_ground, "player should be standing");
        assert!((player.pos.y - 41.0).abs() < 0.1, "y = {}", player.pos.y);
    }

    #[test]
    fn walls_block_movement() {
        let mut world = flat_world();
        for z in 0..16 {
            world.set(10, 41, z, id::STONE);
            world.set(10, 42, z, id::STONE);
        }
        let mut player = Player::new(Vec3::new(8.0, 41.0, 8.0));
        for _ in 0..120 {
            player.vel.x = 5.0;
            move_and_collide(&world, &mut player.pos, &mut player.vel, 1.0 / 60.0);
        }
        assert!(player.pos.x < 9.75, "x = {}", player.pos.x);
    }

    #[test]
    fn look_direction_is_unit_length() {
        let mut p = Player::new(Vec3::ZERO);
        p.rotate(123.0, -45.0, 0.002);
        assert!((p.look_dir().length() - 1.0).abs() < 1e-4);
    }
}
