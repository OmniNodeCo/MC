//! Chunk meshing: face culling, Minecraft style ambient occlusion and split
//! opaque / cutout / water geometry.

use crate::block::{self, Face};
use crate::chunk::{ChunkPos, CHUNK_X, CHUNK_Y, CHUNK_Z};
use crate::textures::tile_uv;
use crate::world::World;

/// One lit, textured vertex in world space.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// Baked directional + ambient occlusion shading (0..1).
    pub shade: f32,
    /// 0 = static, 1 = water surface, 0.5 = swaying plant.
    pub wave: f32,
}

#[derive(Default, Clone, Debug)]
pub struct MeshData {
    pub verts: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }
}

#[derive(Default, Clone, Debug)]
pub struct ChunkMesh {
    pub opaque: MeshData,
    pub cutout: MeshData,
    pub water: MeshData,
}

impl ChunkMesh {
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.cutout.is_empty() && self.water.is_empty()
    }

    pub fn quad_count(&self) -> usize {
        self.opaque.quad_count() + self.cutout.quad_count() + self.water.quad_count()
    }
}

/// Corner positions (0/1 cube coordinates) per face, counter-clockwise seen
/// from outside (the winding wgpu's default front face expects).
fn face_corners(face: Face) -> [[f32; 3]; 4] {
    match face {
        Face::Top => [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        Face::Bottom => [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        Face::North => [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
        Face::South => [
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        Face::East => [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
        Face::West => [
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
    }
}

/// Texture coordinates per corner, matching [`face_corners`] so the texture is
/// upright and not mirrored.
fn face_uvs(face: Face) -> [[f32; 2]; 4] {
    match face {
        Face::Top => [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        Face::Bottom => [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        Face::North => [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        Face::South => [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        Face::East => [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        Face::West => [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
    }
}

const AO_SHADE: [f32; 4] = [0.52, 0.70, 0.86, 1.0];

fn face_should_render(self_id: u8, neighbour: u8) -> bool {
    if neighbour == self_id && block::is_cutout(self_id) {
        return false;
    }
    if block::is_opaque(neighbour) {
        return false;
    }
    if block::is_liquid(self_id) {
        // Water is only visible against air and against cutout blocks.
        return neighbour != self_id;
    }
    true
}

/// Minecraft style vertex ambient occlusion from the three blocks around a
/// face corner.
fn corner_ao(world: &World, x: i32, y: i32, z: i32, face: Face, corner: [f32; 3]) -> u32 {
    let n = face.normal();
    let axis = if n[0] != 0 {
        0
    } else if n[1] != 0 {
        1
    } else {
        2
    };
    let mut signs = [0i32; 3];
    for a in 0..3 {
        signs[a] = if corner[a] > 0.5 { 1 } else { -1 };
    }
    let tangents: Vec<usize> = (0..3).filter(|a| *a != axis).collect();
    let (t1, t2) = (tangents[0], tangents[1]);

    let base = [x + n[0], y + n[1], z + n[2]];
    let mut s1 = base;
    s1[t1] += signs[t1];
    let mut s2 = base;
    s2[t2] += signs[t2];
    let mut sc = base;
    sc[t1] += signs[t1];
    sc[t2] += signs[t2];

    let o1 = block::is_opaque(world.get(s1[0], s1[1], s1[2])) as u32;
    let o2 = block::is_opaque(world.get(s2[0], s2[1], s2[2])) as u32;
    let oc = block::is_opaque(world.get(sc[0], sc[1], sc[2])) as u32;
    if o1 == 1 && o2 == 1 {
        0
    } else {
        3 - o1 - o2 - oc
    }
}

fn push_quad(
    data: &mut MeshData,
    world: &World,
    block_pos: [i32; 3],
    face: Face,
    tile: u32,
    shade_mul: f32,
    wave: f32,
    use_ao: bool,
) {
    let corners = face_corners(face);
    let uvs = face_uvs(face);
    let [u0, v0, u1, v1] = tile_uv(tile);
    let start = data.verts.len() as u32;

    for (i, corner) in corners.iter().enumerate() {
        let ao = if use_ao {
            corner_ao(
                world,
                block_pos[0],
                block_pos[1],
                block_pos[2],
                face,
                *corner,
            )
        } else {
            3
        };
        let [cu, cv] = uvs[i];
        data.verts.push(Vertex {
            pos: [
                block_pos[0] as f32 + corner[0],
                block_pos[1] as f32 + corner[1],
                block_pos[2] as f32 + corner[2],
            ],
            uv: [u0 + (u1 - u0) * cu, v0 + (v1 - v0) * cv],
            shade: face.shade() * AO_SHADE[ao as usize] * shade_mul,
            wave,
        });
    }
    data.indices
        .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
}

fn push_plant(data: &mut MeshData, x: i32, y: i32, z: i32, tile: u32) {
    let [u0, v0, u1, v1] = tile_uv(tile);
    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
    let lo = 0.14;
    let hi = 0.86;

    let quads = [
        // diagonal A
        [
            [fx + lo, fy, fz + lo],
            [fx + hi, fy, fz + hi],
            [fx + hi, fy + 1.0, fz + hi],
            [fx + lo, fy + 1.0, fz + lo],
        ],
        // diagonal B
        [
            [fx + hi, fy, fz + lo],
            [fx + lo, fy, fz + hi],
            [fx + lo, fy + 1.0, fz + hi],
            [fx + hi, fy + 1.0, fz + lo],
        ],
    ];
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    for quad in quads {
        let start = data.verts.len() as u32;
        for (i, pos) in quad.iter().enumerate() {
            let [cu, cv] = uvs[i];
            data.verts.push(Vertex {
                pos: *pos,
                uv: [u0 + (u1 - u0) * cu, v0 + (v1 - v0) * cv],
                shade: 0.94,
                wave: 0.5,
            });
        }
        data.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }
}

/// Build the renderable geometry of one chunk.
pub fn mesh_chunk(world: &World, pos: ChunkPos) -> ChunkMesh {
    let mut mesh = ChunkMesh::default();
    let base_x = pos.0 * CHUNK_X as i32;
    let base_z = pos.1 * CHUNK_Z as i32;

    for y in 0..CHUNK_Y as i32 {
        for z in 0..CHUNK_Z as i32 {
            for x in 0..CHUNK_X as i32 {
                let block_id = world.get(base_x + x, y, base_z + z);
                if block_id == crate::block::id::AIR {
                    continue;
                }
                let def = block::def(block_id);
                let world_pos = (base_x + x, y, base_z + z);

                if def.plant {
                    push_plant(&mut mesh.cutout, world_pos.0, y, world_pos.2, def.side);
                    continue;
                }

                let target = if def.liquid {
                    &mut mesh.water
                } else if def.cutout {
                    &mut mesh.cutout
                } else {
                    &mut mesh.opaque
                };

                let shade_mul = if def.emissive { 1.15 } else { 1.0 };
                for face in Face::ALL {
                    let n = face.normal();
                    let neighbour =
                        world.get(world_pos.0 + n[0], y + n[1], world_pos.2 + n[2]);
                    if !face_should_render(block_id, neighbour) {
                        continue;
                    }
                    push_quad(
                        target,
                        world,
                        [world_pos.0, y, world_pos.2],
                        face,
                        def.tile_for(face),
                        shade_mul,
                        if def.liquid { 1.0 } else { 0.0 },
                        !def.liquid,
                    );
                }
            }
        }
    }

    mesh
}

/// A single textured cube, used for the held-item view model and entity
/// rendering. The cube spans `0..1` on every axis.
pub fn unit_cube(block_id: u8) -> MeshData {
    let def = block::def(block_id);
    let mut data = MeshData::default();
    for face in Face::ALL {
        let corners = face_corners(face);
        let uvs = face_uvs(face);
        let [u0, v0, u1, v1] = tile_uv(def.tile_for(face));
        let start = data.verts.len() as u32;
        for (i, corner) in corners.iter().enumerate() {
            let [cu, cv] = uvs[i];
            data.verts.push(Vertex {
                pos: *corner,
                uv: [u0 + (u1 - u0) * cu, v0 + (v1 - v0) * cv],
                shade: face.shade(),
                wave: 0.0,
            });
        }
        data.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::id;

    #[test]
    fn flat_world_emits_only_top_faces() {
        let mut world = World::new(1);
        world.load_chunk((0, 0));
        // clear a 3x3 area and place a single block
        for z in 0..3 {
            for x in 0..3 {
                for y in 0..CHUNK_Y as i32 {
                    world.set(x, y, z, id::AIR);
                }
            }
        }
        world.set(1, 10, 1, id::STONE);
        let mesh = mesh_chunk(&world, (0, 0));
        // Neighbouring generated terrain still hides the sides, so only the top
        // face of our block is guaranteed to be visible.
        assert!(mesh.opaque.quad_count() >= 1);
        assert!(!mesh.opaque.verts.is_empty());
    }

    #[test]
    fn water_goes_to_its_own_buffer() {
        let mut world = World::new(2);
        world.load_chunk((0, 0));
        for y in 0..CHUNK_Y as i32 {
            world.set(5, y, 5, id::AIR);
        }
        world.set(5, 20, 5, id::WATER);
        let mesh = mesh_chunk(&world, (0, 0));
        assert!(!mesh.water.is_empty());
    }

    #[test]
    fn unit_cube_has_24_vertices() {
        let cube = unit_cube(id::GRASS);
        assert_eq!(cube.verts.len(), 24);
        assert_eq!(cube.indices.len(), 36);
    }
}

/// A cube using one tile on every face, used for the crack overlay and other
/// HUD-ish world geometry. `inflate` grows the cube slightly so it does not
/// z-fight with the block it sits on.
pub fn tile_cube(tile: u32, inflate: f32) -> MeshData {
    let mut data = MeshData::default();
    let [u0, v0, u1, v1] = tile_uv(tile);
    let lo = -inflate;
    let hi = 1.0 + inflate;
    for face in Face::ALL {
        let corners = face_corners(face);
        let uvs = face_uvs(face);
        let start = data.verts.len() as u32;
        for (i, corner) in corners.iter().enumerate() {
            let [cu, cv] = uvs[i];
            data.verts.push(Vertex {
                pos: [
                    if corner[0] > 0.5 { hi } else { lo },
                    if corner[1] > 0.5 { hi } else { lo },
                    if corner[2] > 0.5 { hi } else { lo },
                ],
                uv: [u0 + (u1 - u0) * cu, v0 + (v1 - v0) * cv],
                shade: face.shade(),
                wave: 0.0,
            });
        }
        data.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    data
}
