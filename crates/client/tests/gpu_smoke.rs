//! Headless GPU smoke test.
//!
//! Builds every pipeline, renders a real frame of a generated world into an
//! offscreen texture, reads the pixels back, writes a PNG screenshot
//! (`ci/frame.png`) and asserts that the image is not blank.
//!
//! This catches WGSL errors, vertex-layout/bind-group mismatches, uniform
//! layout bugs and attachment incompatibilities on CI machines (which use
//! Mesa's software Vulkan driver) without needing a window.
//!
//! When no GPU adapter is available the test reports that and passes, so it is
//! safe to run anywhere.

use std::collections::HashSet;
use std::path::Path;

use mc_client::{Gfx, Renderer};
use voxel::Game;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

#[test]
fn renders_a_world_frame_offscreen() {
    let Ok(gfx) = pollster::block_on(Gfx::new_headless(WIDTH, HEIGHT)) else {
        eprintln!("no GPU adapter available - skipping offscreen render test");
        return;
    };

    let target = gfx.device.create_texture(&wgpu::TextureDescriptor {
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

    // 1280 * 4 == 5120 bytes per row, already a multiple of the 256 byte copy
    // alignment required by `copy_texture_to_buffer`.
    let bytes_per_row = WIDTH * 4;
    let readback = gfx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke-readback"),
        size: (bytes_per_row * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut renderer = Renderer::new(gfx);
    let mut game = Game::new(1337, 4);
    game.set_screen(WIDTH as f32, HEIGHT as f32);
    game.paused = false;
    game.time = 0.30; // mid morning
    game.player.pitch = -0.22;

    // Several frames so the chunk streamer can generate, mesh and upload the
    // world around the player.
    for frame in 0..30 {
        renderer.draw(&mut game, frame as f32 * 0.05, &view);
    }
    assert!(game.draw_calls > 0, "renderer recorded no draw calls at all");

    let device = &renderer.gfx.device;
    let queue = &renderer.gfx.queue;
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

    let pixels: Vec<u8> = slice.get_mapped_range().to_vec();
    readback.unmap();

    let mut buckets: HashSet<(u8, u8, u8)> = HashSet::new();
    let mut sky_pixels = 0usize;
    for pixel in pixels.chunks_exact(4) {
        let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
        buckets.insert((r / 12, g / 12, b / 12));
        // Sky is a blue gradient: noticeably more blue than red.
        if b > r.saturating_add(20) && b > 90 {
            sky_pixels += 1;
        }
    }
    let total = (WIDTH * HEIGHT) as usize;
    let mid = (total / 2) * 4;
    println!(
        "rendered {WIDTH}x{HEIGHT}: {} colour buckets, sky {:.1}%, centre pixel {:?}, \
         draws {}, tris {}",
        buckets.len(),
        100.0 * sky_pixels as f32 / total as f32,
        &pixels[mid..mid + 3],
        game.draw_calls,
        game.triangles_drawn
    );

    // Save a genuine screenshot of the frame for humans to look at.
    let shot = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ci/frame.png");
    if let Some(dir) = shot.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = write_png(&shot, WIDTH, HEIGHT, &pixels) {
        println!("could not write {}: {err}", shot.display());
    } else {
        println!("wrote {}", shot.display());
    }

    assert!(
        buckets.len() > 12,
        "frame looks blank: only {} colour buckets",
        buckets.len()
    );
    assert!(
        sky_pixels > total / 100,
        "expected sky to cover part of the frame, got {sky_pixels} pixels"
    );
    assert!(
        game.triangles_drawn > 500,
        "expected terrain geometry, drew {} triangles",
        game.triangles_drawn
    );
}

// ---------------------------------------------------------------------------
// Minimal PNG writer (RGB8, stored/no-compression deflate) so the test does not
// need an image crate.
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                0xEDB8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    // Raw scanlines with filter byte 0, RGB (alpha dropped).
    let stride = width as usize * 3;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in 0..height as usize {
        raw.push(0u8);
        for col in 0..width as usize {
            let i = (row * width as usize + col) * 4;
            raw.extend_from_slice(&rgba[i..i + 3]);
        }
    }

    // zlib stream with stored deflate blocks.
    let mut zlib = vec![0x78u8, 0x01];
    let mut offset = 0usize;
    while offset < raw.len() {
        let len = (raw.len() - offset).min(65535);
        let last = offset + len == raw.len();
        zlib.push(if last { 1u8 } else { 0u8 });
        zlib.extend_from_slice(&(len as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(len as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw[offset..offset + len]);
        offset += len;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit, truecolour, no interlace
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &zlib);
    push_chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png)
}
