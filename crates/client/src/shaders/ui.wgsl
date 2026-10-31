// Textured, coloured HUD quads in pixel space.

struct Scene {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_and_light: vec4<f32>,
    fog_and_distance: vec4<f32>,
    sky_and_water: vec4<f32>,
    screen_and_misc: vec4<f32>,
};

@group(0) @binding(0) var<uniform> scene: Scene;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct UiIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct UiOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: UiIn) -> UiOut {
    var out: UiOut;
    let size = max(scene.screen_and_misc.xy, vec2<f32>(1.0, 1.0));
    let ndc = vec2<f32>(
        in.pos.x / size.x * 2.0 - 1.0,
        1.0 - in.pos.y / size.y * 2.0,
    );
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}


fn encode(c: vec3<f32>) -> vec3<f32> {
    if (scene.screen_and_misc.w > 0.5) {
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
    }
    return c;
}

@fragment
fn fs_main(in: UiOut) -> @location(0) vec4<f32> {
    let c = textureSample(atlas, atlas_sampler, in.uv);
    let alpha = c.a * in.color.a;
    if (alpha < 0.01) {
        discard;
    }
    return vec4<f32>(encode(c.rgb * in.color.rgb), alpha);
}
