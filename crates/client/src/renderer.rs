//! Frame rendering: chunk mesh streaming, sky, terrain, water, particles,
//! selection outline, held block and HUD.

use std::collections::HashMap;

use bytemuck::cast_slice;
use glam::{EulerRot, Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use voxel::block;
use voxel::chunk::{chunk_of, ChunkPos};
use voxel::game::Game;
use voxel::mesher::{self, ChunkMesh, MeshData, Vertex};
use voxel::textures;
use voxel::ui;
use voxel::UiVertex;

use crate::gfx::{Gfx, LineVertex, SceneUniform};

const UI_VERTEX_CAPACITY: usize = 32_768;
const PARTICLE_VERTEX_CAPACITY: usize = 8_192;
const OUTLINE_VERTEX_CAPACITY: usize = 128;
/// A cube always has six quads.
const CUBE_INDICES: u32 = 36;

pub struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct ChunkGpu {
    opaque: Option<GpuMesh>,
    cutout: Option<GpuMesh>,
    water: Option<GpuMesh>,
}

pub struct Renderer {
    pub gfx: Gfx,
    chunks: HashMap<ChunkPos, ChunkGpu>,
    ui_buffer: wgpu::Buffer,
    particle_buffer: wgpu::Buffer,
    overlay_buffer: wgpu::Buffer,
    outline_buffer: wgpu::Buffer,
    cube_indices: wgpu::Buffer,
    hand_mesh: Option<GpuMesh>,
    hand_block: u8,
    pub visible_chunks: usize,
}

impl Renderer {
    pub fn new(gfx: Gfx) -> Self {
        let device = &gfx.device;
        let ui_buffer = create_dynamic_buffer(
            device,
            "ui-vertices",
            UI_VERTEX_CAPACITY,
            std::mem::size_of::<UiVertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let particle_buffer = create_dynamic_buffer(
            device,
            "particles",
            PARTICLE_VERTEX_CAPACITY,
            std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let overlay_buffer = create_dynamic_buffer(
            device,
            "crack-overlay",
            32,
            std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let outline_buffer = create_dynamic_buffer(
            device,
            "outline",
            OUTLINE_VERTEX_CAPACITY,
            std::mem::size_of::<LineVertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        // Index buffer shared by every cube shaped overlay (indices never change).
        let cube_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube-indices"),
            contents: cast_slice(&mesher::tile_cube(0, 0.0).indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut renderer = Self {
            gfx,
            chunks: HashMap::new(),
            ui_buffer,
            particle_buffer,
            overlay_buffer,
            outline_buffer,
            cube_indices,
            hand_mesh: None,
            hand_block: u8::MAX,
            visible_chunks: 0,
        };
        renderer.ensure_hand(block::id::GRASS);
        renderer
    }

    pub fn clear_chunks(&mut self) {
        self.chunks.clear();
    }

    fn ensure_hand(&mut self, block_id: u8) {
        if block_id == self.hand_block && self.hand_mesh.is_some() {
            return;
        }
        let mesh = mesher::unit_cube(block_id);
        self.hand_mesh = upload_mesh(&self.gfx.device, &mesh, "hand");
        self.hand_block = block_id;
    }

    /// Rebuild meshes that changed and mesh newly streamed chunks.
    fn refresh_chunks(&mut self, game: &mut Game) {
        let player = game.player.pos;
        let center = chunk_of(player.x.floor() as i32, player.z.floor() as i32);

        // 1. Explicitly edited chunks: rebuild them right away (nearest first).
        let mut dirty: Vec<ChunkPos> = game
            .world
            .take_dirty()
            .into_iter()
            .filter(|p| game.world.is_loaded(*p))
            .collect();
        dirty.sort_unstable_by_key(|p| (p.0 - center.0).pow(2) + (p.1 - center.1).pow(2));
        for pos in dirty.into_iter().take(8) {
            if !game.world.neighbors_loaded(pos) {
                game.world.mark_dirty(pos);
                continue;
            }
            let mesh = mesher::mesh_chunk(&game.world, pos);
            self.upload_chunk(pos, mesh);
        }

        // 2. Fresh chunks, nearest first, a few per frame.
        let mut missing: Vec<(i32, ChunkPos)> = game
            .world
            .positions()
            .filter(|p| !self.chunks.contains_key(p) && game.world.neighbors_loaded(*p))
            .map(|p| ((p.0 - center.0).pow(2) + (p.1 - center.1).pow(2), p))
            .collect();
        missing.sort_unstable_by_key(|(d, _)| *d);
        for (_, pos) in missing.into_iter().take(3) {
            let mesh = mesher::mesh_chunk(&game.world, pos);
            self.upload_chunk(pos, mesh);
        }

        // 3. Forget chunks that the world dropped.
        let keep_radius = game.render_distance + 3;
        self.chunks.retain(|pos, _| {
            let dx = pos.0 - center.0;
            let dz = pos.1 - center.1;
            game.world.is_loaded(*pos) && dx * dx + dz * dz <= keep_radius * keep_radius
        });
    }

    fn upload_chunk(&mut self, pos: ChunkPos, mesh: ChunkMesh) {
        let device = &self.gfx.device;
        let entry = ChunkGpu {
            opaque: upload_mesh(device, &mesh.opaque, "chunk-opaque"),
            cutout: upload_mesh(device, &mesh.cutout, "chunk-cutout"),
            water: upload_mesh(device, &mesh.water, "chunk-water"),
        };
        self.chunks.insert(pos, entry);
    }

    pub fn render(&mut self, game: &mut Game, elapsed: f32) {
        let frame = {
            let Some(surface) = self.gfx.surface.as_ref() else {
                return;
            };
            match surface.get_current_texture() {
                Ok(frame) => frame,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    surface.configure(&self.gfx.device, &self.gfx.config);
                    return;
                }
                Err(_) => return,
            }
        };
        let view_tex = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.draw(game, elapsed, &view_tex);
        frame.present();
    }

    /// Draw one complete frame into `view_tex`. Split out from [`Self::render`]
    /// so the pipeline can be exercised without a window (see the GPU smoke test).
    pub fn draw(&mut self, game: &mut Game, elapsed: f32, view_tex: &wgpu::TextureView) {
        self.refresh_chunks(game);

        let eye = game.player.eye() + Vec3::new(0.0, game.player.bob, 0.0);
        let look = game.player.look_dir();
        let fov = (72.0_f32 + if game.player.sprinting { 8.0 } else { 0.0 }).to_radians();
        let aspect = self.gfx.aspect();
        let view = Mat4::look_to_rh(eye, look, Vec3::Y);
        let proj = Mat4::perspective_rh(fov, aspect, 0.06, 1400.0);
        let view_proj = proj * view;

        let sky = game.sky_color();
        let underwater = game.player.head_in_water;
        let fog_color = if underwater {
            [0.055, 0.20, 0.32]
        } else {
            game.fog_color()
        };
        let fog_distance = if underwater { 24.0 } else { game.fog_distance() };
        let sun = game.sun_dir();
        let daylight = game.daylight();
        let screen = [
            self.gfx.config.width as f32,
            self.gfx.config.height as f32,
        ];

        let scene = SceneUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, elapsed],
            sun: [sun.x, sun.y, sun.z, daylight],
            fog: [fog_color[0], fog_color[1], fog_color[2], fog_distance],
            sky: [sky[0], sky[1], sky[2], if underwater { 1.0 } else { 0.0 }],
            screen: [
                screen[0],
                screen[1],
                0.0,
                if self.gfx.srgb_output { 1.0 } else { 0.0 },
            ],
        };
        self.gfx.write_scene(&scene);

        // ---- held block view model --------------------------------------
        self.ensure_hand(game.selected_block());
        let swing = game.swing.clamp(0.0, 1.0);
        let swing_curve = (swing * std::f32::consts::PI).sin();
        let hand_pos = Vec3::new(
            0.46 + game.player.bob * 0.8,
            -0.44 + game.player.bob * 0.5 - swing_curve * 0.16,
            -0.72 + swing_curve * 0.10,
        );
        let hand_rotation = Quat::from_euler(
            EulerRot::YXZ,
            -0.62,
            0.42 + swing_curve * 0.35,
            swing_curve * 0.25,
        );
        let hand_model =
            Mat4::from_scale_rotation_translation(Vec3::splat(0.30), hand_rotation, hand_pos);
        let hand_vp = proj * hand_model;
        let mut hand_scene = scene;
        hand_scene.view_proj = hand_vp.to_cols_array_2d();
        hand_scene.inv_view_proj = hand_vp.inverse().to_cols_array_2d();
        self.gfx.write_hand(&hand_scene);

        // ---- particles (billboarded quads) -------------------------------
        let mut particle_verts: Vec<Vertex> = Vec::new();
        {
            let right = {
                let r = look.cross(Vec3::Y);
                if r.length_squared() < 1e-5 {
                    Vec3::X
                } else {
                    r.normalize()
                }
            };
            let up = right.cross(look).normalize();
            for p in &game.particles.items {
                let fade = (p.life / p.max_life).clamp(0.0, 1.0);
                let size = p.size * (0.55 + 0.45 * fade);
                let [u0, v0, u1, v1] = textures::tile_uv(p.tile);
                let corners = [
                    (p.pos - right * size - up * size, [u0, v1]),
                    (p.pos + right * size - up * size, [u1, v1]),
                    (p.pos + right * size + up * size, [u1, v0]),
                    (p.pos - right * size - up * size, [u0, v1]),
                    (p.pos + right * size + up * size, [u1, v0]),
                    (p.pos - right * size + up * size, [u0, v0]),
                ];
                for (pos, uv) in corners {
                    if particle_verts.len() >= PARTICLE_VERTEX_CAPACITY {
                        break;
                    }
                    particle_verts.push(Vertex {
                        pos: pos.to_array(),
                        uv,
                        shade: 0.85,
                        wave: 0.0,
                    });
                }
            }
        }
        let particle_count = particle_verts.len();
        if particle_count > 0 {
            self.gfx.queue.write_buffer(
                &self.particle_buffer,
                0,
                cast_slice(&particle_verts[..particle_count]),
            );
        }

        // ---- selection outline -------------------------------------------
        let mut outline: Vec<LineVertex> = Vec::new();
        if let Some(hit) = game.target {
            let [bx, by, bz] = hit.block;
            let e = 0.0035;
            let lo = Vec3::new(bx as f32 - e, by as f32 - e, bz as f32 - e);
            let hi = Vec3::new(
                bx as f32 + 1.0 + e,
                by as f32 + 1.0 + e,
                bz as f32 + 1.0 + e,
            );
            let c = [0.04, 0.04, 0.05, 0.6];
            let corners = [
                Vec3::new(lo.x, lo.y, lo.z),
                Vec3::new(hi.x, lo.y, lo.z),
                Vec3::new(hi.x, lo.y, hi.z),
                Vec3::new(lo.x, lo.y, hi.z),
                Vec3::new(lo.x, hi.y, lo.z),
                Vec3::new(hi.x, hi.y, lo.z),
                Vec3::new(hi.x, hi.y, hi.z),
                Vec3::new(lo.x, hi.y, hi.z),
            ];
            let edges = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            for (a, b) in edges {
                for corner in [corners[a], corners[b]] {
                    if outline.len() >= OUTLINE_VERTEX_CAPACITY {
                        break;
                    }
                    outline.push(LineVertex {
                        pos: corner.to_array(),
                        color: c,
                    });
                }
            }
        }
        let outline_count = outline.len();
        if outline_count > 0 {
            self.gfx
                .queue
                .write_buffer(&self.outline_buffer, 0, cast_slice(&outline));
        }

        // ---- crack overlay -----------------------------------------------
        let mut overlay: Vec<Vertex> = Vec::new();
        if game.break_progress > 0.0 {
            if let Some(hit) = game.target {
                let stage = ((game.break_progress * textures::DESTROY_STAGES as f32) as u32)
                    .min(textures::DESTROY_STAGES - 1);
                let cube = mesher::tile_cube(textures::DESTROY_TILE_BASE + stage, 0.004);
                overlay = cube.verts;
                for v in &mut overlay {
                    v.pos[0] += hit.block[0] as f32;
                    v.pos[1] += hit.block[1] as f32;
                    v.pos[2] += hit.block[2] as f32;
                }
            }
        }
        let overlay_count = overlay.len();
        if overlay_count > 0 {
            self.gfx
                .queue
                .write_buffer(&self.overlay_buffer, 0, cast_slice(&overlay));
        }

        // ---- HUD -----------------------------------------------------------
        let ui_vertices = ui::build(game);
        let ui_count = ui_vertices.len().min(UI_VERTEX_CAPACITY);
        if ui_count > 0 {
            self.gfx
                .queue
                .write_buffer(&self.ui_buffer, 0, cast_slice(&ui_vertices[..ui_count]));
        }

        // ---- draw ----------------------------------------------------------
        let clear = wgpu::Color {
            r: sky[0] as f64 * 0.55,
            g: sky[1] as f64 * 0.55,
            b: sky[2] as f64 * 0.55,
            a: 1.0,
        };
        let mut encoder = self
            .gfx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let mut draw_calls = 0usize;
        let mut triangles = 0usize;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_tex,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gfx.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.gfx.scene_bind_group, &[]);

            // sky
            pass.set_pipeline(&self.gfx.sky_pipeline);
            pass.draw(0..3, 0..1);

            let frustum = Frustum::from_matrix(&view_proj);
            let mut visible = 0usize;

            // opaque terrain
            pass.set_pipeline(&self.gfx.terrain_pipeline);
            for (pos, chunk) in &self.chunks {
                let Some(mesh) = &chunk.opaque else { continue };
                if !frustum.intersects(chunk_min(*pos), chunk_max(*pos)) {
                    continue;
                }
                visible += 1;
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                draw_calls += 1;
                triangles += mesh.index_count as usize / 3;
            }

            // cutout terrain + crack overlay + particles
            pass.set_pipeline(&self.gfx.cutout_pipeline);
            for (pos, chunk) in &self.chunks {
                let Some(mesh) = &chunk.cutout else { continue };
                if !frustum.intersects(chunk_min(*pos), chunk_max(*pos)) {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                draw_calls += 1;
                triangles += mesh.index_count as usize / 3;
            }
            if overlay_count > 0 {
                pass.set_vertex_buffer(0, self.overlay_buffer.slice(..));
                pass.set_index_buffer(self.cube_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..CUBE_INDICES, 0, 0..1);
                draw_calls += 1;
                triangles += CUBE_INDICES as usize / 3;
            }
            if particle_count > 0 {
                pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
                pass.draw(0..particle_count as u32, 0..1);
                draw_calls += 1;
                triangles += particle_count as usize / 6 * 2;
            }

            // water
            pass.set_pipeline(&self.gfx.water_pipeline);
            for (pos, chunk) in &self.chunks {
                let Some(mesh) = &chunk.water else { continue };
                if !frustum.intersects(chunk_min(*pos), chunk_max(*pos)) {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                draw_calls += 1;
                triangles += mesh.index_count as usize / 3;
            }

            // selection outline
            if outline_count > 0 {
                pass.set_pipeline(&self.gfx.line_pipeline);
                pass.set_vertex_buffer(0, self.outline_buffer.slice(..));
                pass.draw(0..outline_count as u32, 0..1);
                draw_calls += 1;
            }

            // held block
            if let Some(hand) = &self.hand_mesh {
                pass.set_bind_group(0, &self.gfx.hand_bind_group, &[]);
                pass.set_pipeline(&self.gfx.hand_pipeline);
                pass.set_vertex_buffer(0, hand.vertex_buffer.slice(..));
                pass.set_index_buffer(hand.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..hand.index_count, 0, 0..1);
                draw_calls += 1;
                triangles += hand.index_count as usize / 3;
                pass.set_bind_group(0, &self.gfx.scene_bind_group, &[]);
            }

            // HUD
            if ui_count > 0 {
                pass.set_pipeline(&self.gfx.ui_pipeline);
                pass.set_vertex_buffer(0, self.ui_buffer.slice(..));
                pass.draw(0..ui_count as u32, 0..1);
                draw_calls += 1;
                triangles += ui_count as usize / 3;
            }

            self.visible_chunks = visible;
        }
        self.gfx.queue.submit(Some(encoder.finish()));

        game.draw_calls = draw_calls;
        game.triangles_drawn = triangles;
        game.meshed_chunks = self.chunks.len();
    }
}

fn chunk_min(pos: ChunkPos) -> Vec3 {
    Vec3::new(pos.0 as f32 * 16.0, 0.0, pos.1 as f32 * 16.0)
}

fn chunk_max(pos: ChunkPos) -> Vec3 {
    chunk_min(pos) + Vec3::new(16.0, voxel::CHUNK_Y as f32, 16.0)
}

fn create_dynamic_buffer(
    device: &wgpu::Device,
    label: &str,
    capacity: usize,
    stride: usize,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (capacity * stride).max(stride) as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn upload_mesh(device: &wgpu::Device, mesh: &MeshData, label: &str) -> Option<GpuMesh> {
    if mesh.is_empty() {
        return None;
    }
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: cast_slice(&mesh.verts),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    Some(GpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
    })
}

/// View frustum extracted from the view-projection matrix.
struct Frustum {
    planes: [[f32; 4]; 6],
}

impl Frustum {
    fn from_matrix(m: &Mat4) -> Self {
        let rows = [
            m.row(0).to_array(),
            m.row(1).to_array(),
            m.row(2).to_array(),
            m.row(3).to_array(),
        ];
        let mut planes = [[0.0f32; 4]; 6];
        for i in 0..3 {
            for j in 0..4 {
                planes[i * 2][j] = rows[3][j] + rows[i][j];
                planes[i * 2 + 1][j] = rows[3][j] - rows[i][j];
            }
        }
        for p in &mut planes {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
            for value in p.iter_mut() {
                *value /= len;
            }
        }
        Self { planes }
    }

    fn intersects(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let x = if p[0] > 0.0 { max.x } else { min.x };
            let y = if p[1] > 0.0 { max.y } else { min.y };
            let z = if p[2] > 0.0 { max.z } else { min.z };
            if p[0] * x + p[1] * y + p[2] * z + p[3] < 0.0 {
                return false;
            }
        }
        true
    }
}
