//! GPU setup shared by the native and Wasm builds: device creation, the
//! procedural texture atlas and every render pipeline.

use bytemuck::{Pod, Zeroable};

use voxel::textures;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
pub const TERRAIN_SHADER: &str = include_str!("shaders/terrain.wgsl");
pub const SKY_SHADER: &str = include_str!("shaders/sky.wgsl");
pub const UI_SHADER: &str = include_str!("shaders/ui.wgsl");
pub const LINE_SHADER: &str = include_str!("shaders/lines.wgsl");

/// Uniform shared by every pipeline. Field order mirrors the WGSL struct.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SceneUniform {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    /// xyz = camera position, w = elapsed seconds.
    pub cam_pos: [f32; 4],
    /// xyz = sun direction, w = daylight (0.12 .. 1.0).
    pub sun: [f32; 4],
    /// rgb = fog colour, a = fog end distance.
    pub fog: [f32; 4],
    /// rgb = sky colour, a = camera under water (0/1).
    pub sky: [f32; 4],
    /// xy = screen size, z = unused, w = sRGB output encode flag.
    pub screen: [f32; 4],
}

const TERRAIN_ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x2,
    2 => Float32,
    3 => Float32,
];
const UI_ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x2,
    2 => Float32x4,
];
const LINE_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
    0 => Float32x3,
    1 => Float32x4,
];

pub struct Gfx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_view: wgpu::TextureView,
    pub srgb_output: bool,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub scene_buffer: wgpu::Buffer,
    pub scene_bind_group: wgpu::BindGroup,
    pub hand_buffer: wgpu::Buffer,
    pub hand_bind_group: wgpu::BindGroup,

    pub atlas: wgpu::Texture,
    pub atlas_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,

    pub terrain_pipeline: wgpu::RenderPipeline,
    pub cutout_pipeline: wgpu::RenderPipeline,
    pub water_pipeline: wgpu::RenderPipeline,
    pub hand_pipeline: wgpu::RenderPipeline,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,
    pub ui_pipeline: wgpu::RenderPipeline,
}

impl Gfx {
    /// Create the renderer for a window / canvas surface.
    pub async fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let width = width.max(1);
        let height = height.max(1);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| "no compatible GPU adapter found".to_string())?;

        let (device, queue) = request_device(&adapter).await?;

        let caps = surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            return Err("surface exposes no texture formats".to_string());
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self::build(device, queue, Some(surface), format, width, height)
    }

    /// Create the renderer without a window. Used by the GPU smoke test.
    pub async fn new_headless(width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or_else(|| "no GPU adapter available".to_string())?;
        let (device, queue) = request_device(&adapter).await?;
        Self::build(
            device,
            queue,
            None,
            wgpu::TextureFormat::Rgba8Unorm,
            width.max(1),
            height.max(1),
        )
    }

    fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        let depth_view = create_depth_view(&device, width, height);

        // ---- shared bind group ------------------------------------------
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let scene_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene-uniform"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hand_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hand-uniform"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---- atlas -------------------------------------------------------
        let atlas_size = textures::ATLAS_PX;
        let pixels = textures::build_atlas();
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("block-atlas"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas_size),
                rows_per_image: Some(atlas_size),
            },
            wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let make_bind_group = |buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene-bind-group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&atlas_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let scene_bind_group = make_bind_group(&scene_buffer);
        let hand_bind_group = make_bind_group(&hand_buffer);

        // ---- shaders & pipelines -----------------------------------------
        let terrain_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_SHADER.into()),
        });
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(SKY_SHADER.into()),
        });
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(UI_SHADER.into()),
        });
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lines"),
            source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let terrain_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<voxel::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &TERRAIN_ATTRS,
        };
        let ui_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<voxel::UiVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &UI_ATTRS,
        };
        let line_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &LINE_ATTRS,
        };

        let depth_state = Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let color_target = Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let blend_target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });

        let make_pipeline = |label: &str,
                             module: &wgpu::ShaderModule,
                             vs: &str,
                             fs: &str,
                             buffers: &[wgpu::VertexBufferLayout],
                             topology: wgpu::PrimitiveTopology,
                             cull: Option<wgpu::Face>,
                             depth: Option<wgpu::DepthStencilState>,
                             target: Option<wgpu::ColorTargetState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: vs,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: fs,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[target],
                }),
                primitive: wgpu::PrimitiveState {
                    topology,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: depth,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };

        let terrain_buffers = [terrain_layout];
        let terrain_pipeline = make_pipeline(
            "terrain",
            &terrain_shader,
            "vs_main",
            "fs_opaque",
            &terrain_buffers,
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
            depth_state.clone(),
            color_target.clone(),
        );
        let cutout_pipeline = make_pipeline(
            "cutout",
            &terrain_shader,
            "vs_main",
            "fs_cutout",
            &terrain_buffers,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            depth_state.clone(),
            color_target.clone(),
        );
        let water_pipeline = make_pipeline(
            "water",
            &terrain_shader,
            "vs_main",
            "fs_water",
            &terrain_buffers,
            wgpu::PrimitiveTopology::TriangleList,
            None,
            Some(wgpu::DepthStencilState {
                depth_write_enabled: false,
                ..depth_state.clone().unwrap()
            }),
            blend_target.clone(),
        );
        let hand_pipeline = make_pipeline(
            "hand",
            &terrain_shader,
            "vs_main",
            "fs_opaque",
            &terrain_buffers,
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
            None,
            color_target.clone(),
        );
        let sky_pipeline = make_pipeline(
            "sky",
            &sky_shader,
            "vs_main",
            "fs_main",
            &[],
            wgpu::PrimitiveTopology::TriangleList,
            None,
            None,
            color_target.clone(),
        );
        let line_pipeline = make_pipeline(
            "lines",
            &line_shader,
            "vs_main",
            "fs_main",
            &[line_layout],
            wgpu::PrimitiveTopology::LineList,
            None,
            Some(wgpu::DepthStencilState {
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                ..depth_state.unwrap()
            }),
            blend_target.clone(),
        );
        let ui_pipeline = make_pipeline(
            "ui",
            &ui_shader,
            "vs_main",
            "fs_main",
            &[ui_layout],
            wgpu::PrimitiveTopology::TriangleList,
            None,
            None,
            blend_target,
        );

        if let Some(surface) = &surface {
            surface.configure(&device, &config);
        }

        Ok(Self {
            device,
            queue,
            surface,
            config,
            depth_view,
            srgb_output: format.is_srgb(),
            bind_group_layout,
            scene_buffer,
            scene_bind_group,
            hand_buffer,
            hand_bind_group,
            atlas,
            atlas_view,
            sampler,
            terrain_pipeline,
            cutout_pipeline,
            water_pipeline,
            hand_pipeline,
            sky_pipeline,
            line_pipeline,
            ui_pipeline,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.config.width && height == self.config.height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        self.depth_view = create_depth_view(&self.device, width, height);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn write_scene(&self, uniform: &SceneUniform) {
        self.queue
            .write_buffer(&self.scene_buffer, 0, bytemuck::bytes_of(uniform));
    }

    pub fn write_hand(&self, uniform: &SceneUniform) {
        self.queue
            .write_buffer(&self.hand_buffer, 0, bytemuck::bytes_of(uniform));
    }
}

async fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
    adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("mc-device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
            },
            None,
        )
        .await
        .map_err(|e| format!("could not create GPU device: {e}"))
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Vertex for the block selection wireframe.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct LineVertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}
