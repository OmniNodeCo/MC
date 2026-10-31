// Procedural sky: gradient, sun, moon, stars, drifting clouds.

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

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) dir: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> SkyOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = corners[index];
    var out: SkyOut;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    let world = scene.inv_view_proj * vec4<f32>(p, 1.0, 1.0);
    let origin = scene.cam_pos.xyz;
    out.dir = normalize(world.xyz / world.w - origin);
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    var h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var total = 0.0;
    var amp = 1.0;
    var freq = 1.0;
    var norm = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        total = total + noise2(p * freq) * amp;
        norm = norm + amp;
        amp = amp * 0.5;
        freq = freq * 2.03;
    }
    return total / norm;
}


fn encode(c: vec3<f32>) -> vec3<f32> {
    if (scene.screen_and_misc.w > 0.5) {
        return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
    }
    return c;
}

@fragment
fn fs_main(in: SkyOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.dir);
    let sun_dir = normalize(scene.sun_and_light.xyz);
    let daylight = scene.sun_and_light.w;
    let horizon = scene.fog_and_distance.rgb;
    let zenith = mix(vec3<f32>(0.012, 0.018, 0.055), vec3<f32>(0.16, 0.36, 0.80), daylight);

    var col = mix(horizon, zenith, pow(clamp(dir.y, 0.0, 1.0), 0.55));
    if (dir.y < 0.0) {
        col = mix(horizon, vec3<f32>(0.09, 0.11, 0.13), clamp(-dir.y * 2.5, 0.0, 1.0));
    }

    // sun & moon
    let sun_dot = dot(dir, sun_dir);
    let sun_glow = pow(max(sun_dot, 0.0), 48.0);
    col = col + vec3<f32>(1.0, 0.82, 0.48) * sun_glow * 0.30 * daylight;
    let sun_disc = smoothstep(0.9972, 0.9990, sun_dot);
    col = mix(col, vec3<f32>(1.0, 0.96, 0.82), sun_disc * smoothstep(0.0, 0.2, daylight));
    let moon_dot = dot(dir, -sun_dir);
    let moon_disc = smoothstep(0.9985, 0.9995, moon_dot);
    col = mix(col, vec3<f32>(0.93, 0.95, 1.0), moon_disc * (1.0 - daylight) * 0.9);

    // stars
    let night = 1.0 - clamp(daylight * 1.4, 0.0, 1.0);
    if (night > 0.02 && dir.y > 0.02) {
        let sp = dir.xz / (dir.y + 0.35) * 34.0;
        let h = hash21(floor(sp));
        let twinkle = 0.55 + 0.45 * sin(scene.cam_pos.w * 2.0 + h * 42.0);
        let star = step(0.9935, h) * twinkle;
        col = col + vec3<f32>(0.9, 0.93, 1.0) * star * night * clamp(dir.y * 2.0, 0.0, 1.0);
    }

    // clouds on a plane above the world
    if (dir.y > 0.015) {
        let plane_y = 168.0;
        let t = (plane_y - scene.cam_pos.y) / dir.y;
        if (t > 0.0 && t < 5000.0) {
            let p = scene.cam_pos.xz + dir.xz * t;
            let drift = vec2<f32>(scene.cam_pos.w * 1.6, scene.cam_pos.w * 0.4);
            let c = fbm(p * 0.0035 + drift * 0.0035);
            let cover = smoothstep(0.50, 0.70, c);
            let fade = smoothstep(5000.0, 1200.0, t) * smoothstep(0.015, 0.10, dir.y);
            let cloud_color = mix(vec3<f32>(0.62, 0.66, 0.74), vec3<f32>(1.0, 0.99, 0.97), daylight);
            col = mix(col, cloud_color, cover * fade * 0.82);
        }
    }

    // under water everything turns blue-green and dark
    let underwater = scene.sky_and_water.a;
    col = mix(col, vec3<f32>(0.05, 0.20, 0.32) * (0.25 + daylight * 0.75), underwater);
    return vec4<f32>(encode(col), 1.0);
}
