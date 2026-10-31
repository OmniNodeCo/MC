//! Block break particles.

use glam::Vec3;

use crate::block;
use crate::noise::Rng;
use crate::world::World;

#[derive(Clone, Debug)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub tile: u32,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
}

#[derive(Default, Clone, Debug)]
pub struct Particles {
    pub items: Vec<Particle>,
}

pub const MAX_PARTICLES: usize = 512;

impl Particles {
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Spawn a shower of little textured cubes at a broken block.
    pub fn spawn_break(&mut self, block_pos: [i32; 3], block_id: u8, rng: &mut Rng) {
        let def = block::def(block_id);
        let center = Vec3::new(
            block_pos[0] as f32 + 0.5,
            block_pos[1] as f32 + 0.5,
            block_pos[2] as f32 + 0.5,
        );
        let count = 10 + rng.pick(6);
        for _ in 0..count {
            if self.items.len() >= MAX_PARTICLES {
                break;
            }
            let jitter = Vec3::new(
                rng.next_f32() - 0.5,
                rng.next_f32() - 0.2,
                rng.next_f32() - 0.5,
            ) * 0.7;
            let dir = Vec3::new(
                rng.next_f32() - 0.5,
                rng.next_f32() * 0.8 + 0.2,
                rng.next_f32() - 0.5,
            ) * 3.2;
            let life = 0.45 + rng.next_f32() * 0.6;
            self.items.push(Particle {
                pos: center + jitter,
                vel: dir,
                tile: def.tile_for(block::Face::South),
                life,
                max_life: life,
                size: 0.06 + rng.next_f32() * 0.06,
            });
        }
    }

    pub fn update(&mut self, dt: f32, world: &World) {
        for p in &mut self.items {
            p.life -= dt;
            p.vel.y -= 18.0 * dt;
            let next = p.pos + p.vel * dt;
            if block::is_solid(world.get(
                next.x.floor() as i32,
                next.y.floor() as i32,
                next.z.floor() as i32,
            )) {
                p.vel = p.vel * 0.25;
                p.vel.y = 0.0;
            } else {
                p.pos = next;
            }
            p.vel = p.vel * (1.0 - dt * 1.5);
        }
        self.items.retain(|p| p.life > 0.0);
    }
}
