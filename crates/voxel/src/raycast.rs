//! Voxel ray casting (Amanatides & Woo DDA) for block selection.

use glam::Vec3;

use crate::block::{self, Face};
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayHit {
    pub block: [i32; 3],
    /// Face normal of the block that was hit (points at the player).
    pub normal: [i32; 3],
}

impl RayHit {
    pub fn face(&self) -> Face {
        match self.normal {
            [0, 1, 0] => Face::Top,
            [0, -1, 0] => Face::Bottom,
            [0, 0, -1] => Face::North,
            [0, 0, 1] => Face::South,
            [1, 0, 0] => Face::East,
            _ => Face::West,
        }
    }

    /// Where a new block would be placed.
    pub fn placement(&self) -> [i32; 3] {
        [
            self.block[0] + self.normal[0],
            self.block[1] + self.normal[1],
            self.block[2] + self.normal[2],
        ]
    }
}

/// Cast a ray through the voxel grid. Water does not stop the ray, plants do.
pub fn raycast(world: &World, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
    let dir = dir.normalize_or_zero();
    if dir.length_squared() < 0.5 {
        return None;
    }

    let mut cell = [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ];

    let step: [i32; 3] = [
        if dir.x > 0.0 {
            1
        } else if dir.x < 0.0 {
            -1
        } else {
            0
        },
        if dir.y > 0.0 {
            1
        } else if dir.y < 0.0 {
            -1
        } else {
            0
        },
        if dir.z > 0.0 {
            1
        } else if dir.z < 0.0 {
            -1
        } else {
            0
        },
    ];

    let origin_arr = [origin.x, origin.y, origin.z];
    let dir_arr = [dir.x, dir.y, dir.z];
    let mut t_max = [f32::INFINITY; 3];
    let mut t_delta = [f32::INFINITY; 3];
    for a in 0..3 {
        if dir_arr[a] > 0.0 {
            t_max[a] = ((origin_arr[a].floor() + 1.0) - origin_arr[a]) / dir_arr[a];
            t_delta[a] = 1.0 / dir_arr[a];
        } else if dir_arr[a] < 0.0 {
            t_max[a] = (origin_arr[a] - origin_arr[a].floor()) / -dir_arr[a];
            t_delta[a] = -1.0 / dir_arr[a];
        }
    }

    let mut normal = [0i32; 3];
    let mut travelled = 0.0f32;
    // Safety valve against pathological directions.
    for _ in 0..1024 {
        let id = world.get(cell[0], cell[1], cell[2]);
        if block::is_solid(id) || block::is_plant(id) {
            return Some(RayHit { block: cell, normal });
        }

        let axis = if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
            0
        } else if t_max[1] < t_max[2] {
            1
        } else {
            2
        };
        travelled = t_max[axis];
        if travelled > max_dist {
            return None;
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        normal = [0, 0, 0];
        normal[axis] = -step[axis];

        if cell[1] < 0 || cell[1] >= crate::chunk::CHUNK_Y as i32 {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::id;

    #[test]
    fn hits_the_block_below() {
        let mut world = World::new(4);
        world.load_chunk((0, 0));
        for y in 0..crate::chunk::CHUNK_Y as i32 {
            world.set(8, y, 8, id::AIR);
        }
        world.set(8, 30, 8, id::STONE);
        let hit = raycast(&world, Vec3::new(8.5, 40.0, 8.5), Vec3::NEG_Y, 20.0).unwrap();
        assert_eq!(hit.block, [8, 30, 8]);
        assert_eq!(hit.normal, [0, 1, 0]);
        assert_eq!(hit.placement(), [8, 31, 8]);
    }

    #[test]
    fn misses_when_out_of_range() {
        let mut world = World::new(4);
        world.load_chunk((0, 0));
        for y in 0..crate::chunk::CHUNK_Y as i32 {
            world.set(2, y, 2, id::AIR);
        }
        world.set(2, 5, 2, id::STONE);
        assert!(raycast(&world, Vec3::new(2.5, 40.0, 2.5), Vec3::NEG_Y, 8.0).is_none());
    }

    #[test]
    fn water_is_transparent_to_rays() {
        let mut world = World::new(4);
        world.load_chunk((0, 0));
        for y in 0..crate::chunk::CHUNK_Y as i32 {
            world.set(1, y, 1, id::AIR);
        }
        world.set(1, 20, 1, id::WATER);
        world.set(1, 10, 1, id::STONE);
        let hit = raycast(&world, Vec3::new(1.5, 30.0, 1.5), Vec3::NEG_Y, 40.0).unwrap();
        assert_eq!(hit.block, [1, 10, 1]);
    }
}
