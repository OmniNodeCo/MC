// Chunk geometry: textured, baked face lighting, fog and day/night tint.

struct Scene {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,   // xyz = eye, w = time
    sun_and_light: vec4<f32>, // xyz = sun direction, w = daylight 0..1
    fog_and_distance: vec4<f32>, // rgb = fog colour, a = fog end
    sky_and_water: vec4<f32>, // rgb = sky colour, a = camera underwater
    screen_and_misc: vec4<f32>, // xy = screen size
};

@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) shade: f32,
    @location(3) wave: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
    @location(2) dist: f32,
    @location(3) world: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var p = in.pos;
    let t = scene.cam_pos.w;
    if (in.wave > 0.0) {
        p.x = p.x + sin(t * 1.3 + p.z * 0.7 + p.y * 2.0) * 0.035 * in.wave;
        p.y = p.y + sin(t * 1.7 + p.x * 0.6 + p.z * 0.4) * 0.05 * in.wave;
    }
    var out: VsOut;
    out.clip = scene.view_proj * vec4<f32>(p, 1.0);
    out.uv = in.uv;
    out.shade = in.shade;
    out.world = p;
    out.dist = length(p - scene.cam_pos.xyz);
    return out;
}


fn encode(c: vec3<f32>) -> vec3<f32> {
    if (scene.screen_and_misc.w > 0.5) {
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
    }
    return c;
}

fn lit(color: vec3<f32>, shade: f32, dist: f32) -> vec3<f32> {
    let daylight = scene.sun_and_light.w;
    let light = mix(0.26, 1.0, daylight);
    var rgb = color * shade * light;
    let fog_end = max(scene.fog_and_distance.a, 8.0);
    let fog_start = fog_end * 0.42;
    let f = clamp((dist - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    rgb = mix(rgb, scene.fog_and_distance.rgb, f * 0.95);
    return encode(rgb);
}

@fragment
fn fs_opaque(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, atlas_sampler, in.uv);
    return vec4<f32>(lit(c.rgb, in.shade, in.dist), 1.0);
}

@fragment
fn fs_cutout(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, atlas_sampler, in.uv);
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(lit(c.rgb, in.shade, in.dist), 1.0);
}

@fragment
fn fs_water(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, atlas_sampler, in.uv);
    let rgb = lit(c.rgb, in.shade, in.dist);
    return vec4<f32>(rgb, clamp(c.a, 0.0, 1.0) * 0.82);
}


