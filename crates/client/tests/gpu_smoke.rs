//! Headless GPU smoke test.
//!
//! Builds every pipeline, renders a real frame of a generated world into an
//! offscreen texture and reads the pixels back. This catches WGSL errors,
//! vertex-layout/bind-group mismatches and uniform-layout bugs on CI machines
//! (which use Mesa's software Vulkan driver) without needing a window.
//!
//! When no GPU adapter is available the test reports that and passes, so it is
//! safe to run anywhere.

use std::collections::HashSet;

use mc_client::{Gfx, Renderer};
use voxel::Game;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 384;

#[test]
fn renders_a_world_frame_offscreen() {
    let Ok(gfx) = pollster::block_on(Gfx::new_headless(WIDTH, HEIGHT)) else {
        eprintln!("no GPU adapter available - skipping offscreen render test");
        return;
    };

    let device = &gfx.device;
    let queue = &gfx.queue;

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke-target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // 640 * 4 == 2560 bytes per row, already a multiple of the 256 byte copy
    // alignment required by `copy_texture_to_buffer`.
    let bytes_per_row = WIDTH * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke-readback"),
        size: (bytes_per_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut renderer = Renderer::new(gfx);
    let mut game = Game::new(1337, 3);
    game.set_screen(WIDTH as f32, HEIGHT as f32);
    game.paused = false;
    game.time = 0.28; // mid-morning
    if game.player.on_ground {
        // Look slightly down so terrain fills a good part of the frame.
        game.player.pitch = -0.30;
    }

    // A handful of frames so the chunk streamer can mesh and upload the world.
    for frame in 0..10 {
        renderer.draw(&mut game, frame as f32 * 0.05, &view);
    }
    assert!(
        game.draw_calls > 0,
        "renderer recorded no draw calls at all"
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("smoke-copy"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &readback,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .expect("map callback never fired")
        .expect("failed to map readback buffer");

    let data = slice.get_mapped_range();
    let mut buckets: HashSet<(u8, u8, u8)> = HashSet::new();
    let mut sky_pixels = 0usize;
    for pixel in data.chunks_exact(4) {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        buckets.insert((r / 12, g / 12, b / 12));
        // Sky is a blue gradient: noticeably more blue than red.
        if b > r.saturating_add(20) && b > 90 {
            sky_pixels += 1;
        }
    }
    let total = (WIDTH * HEIGHT) as usize;
    let mid = (total / 2) * 4;
    let bottom = (total - WIDTH as usize / 2) * 4;
    let sample = [
        [data[0], data[1], data[2]],
        [data[mid], data[mid + 1], data[mid + 2]],
        [data[bottom], data[bottom + 1], data[bottom + 2]],
    ];

    println!(
        "rendered {WIDTH}x{HEIGHT}: {} distinct colour buckets, {} sky-ish pixels \
         ({:.1}%), first pixel {:?}, samples {:?}, draws {}, tris {}",
        buckets.len(),
        sky_pixels,
        100.0 * sky_pixels as f32 / total as f32,
        &data[0..3],
        sample,
        game.draw_calls,
        game.triangles_drawn
    );

    assert!(
        buckets.len() > 12,
        "frame looks blank: only {} distinct colour buckets",
        buckets.len()
    );
    assert!(
        sky_pixels > total / 100,
        "expected sky to cover part of the frame, got {} pixels",
        sky_pixels
    );
    assert!(
        game.triangles_drawn > 100,
        "expected terrain geometry, drew {} triangles",
        game.triangles_drawn
    );

    drop(data);
    readback.unmap();
}
