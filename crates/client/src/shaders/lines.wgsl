// Block selection outline.

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

struct LineIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: LineIn) -> LineOut {
    var out: LineOut;
    out.clip = scene.view_proj * vec4<f32>(in.pos, 1.0);
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
fn fs_main(in: LineOut) -> @location(0) vec4<f32> {
    return vec4<f32>(encode(in.color.rgb), in.color.a);
}
