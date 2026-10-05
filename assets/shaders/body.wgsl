#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

struct BodyUniform {
    starts: array<vec4<f32>, 32>,
    ends: array<vec4<f32>, 32>,
    colors: array<vec4<f32>, 32>,
    bounds_min: vec4<f32>,
    bounds_max: vec4<f32>,
    settings: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> body: BodyUniform;

struct BodyOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

fn primitive_distance(p: vec3<f32>, index: u32) -> f32 {
    let a = body.starts[index].xyz;
    let ab = body.ends[index].xyz - a;
    let h = clamp(dot(p - a, ab) / max(dot(ab, ab), 0.000001), 0.0, 1.0);
    return length(p - a - ab * h) - body.starts[index].w;
}

fn scene_distance(p: vec3<f32>) -> f32 {
    var distance = 10000.0;
    let smoothing = body.settings.y;
    for (var i = 0u; i < u32(body.settings.x); i += 1u) {
        let next = primitive_distance(p, i);
        let h = clamp(0.5 + 0.5 * (next - distance) / smoothing, 0.0, 1.0);
        distance = mix(next, distance, h) - smoothing * h * (1.0 - h);
    }
    return distance;
}

fn scene_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2(0.0007, -0.0007);
    return normalize(e.xyy * scene_distance(p + e.xyy)
        + e.yyx * scene_distance(p + e.yyx)
        + e.yxy * scene_distance(p + e.yxy)
        + e.xxx * scene_distance(p + e.xxx));
}

fn scene_color(p: vec3<f32>) -> vec3<f32> {
    var sum = vec3(0.0);
    var weight = 0.0;
    for (var i = 0u; i < u32(body.settings.x); i += 1u) {
        let w = exp(clamp(-primitive_distance(p, i) / 0.12, -20.0, 20.0));
        sum += body.colors[i].rgb * w;
        weight += w;
    }
    return sum / max(weight, 0.000001);
}

@fragment
fn fragment(in: VertexOutput) -> BodyOutput {
    let orthographic = view.clip_from_view[3][3] == 1.0;
    var origin = view.world_position;
    var direction = normalize(in.world_position.xyz - origin);
    if orthographic {
        direction = normalize(-view.world_from_view[2].xyz);
        origin = in.world_position.xyz - direction * dot(in.world_position.xyz - origin, direction);
    }
    // Stable reciprocal avoids infinity * zero at a proxy plane.
    let inverse_direction = sign(direction + vec3(0.00000001)) / max(abs(direction), vec3(0.00000001));
    let first = (body.bounds_min.xyz - origin) * inverse_direction;
    let last = (body.bounds_max.xyz - origin) * inverse_direction;
    let near = min(first, last);
    let far = max(first, last);
    var t = max(max(near.x, max(near.y, near.z)), 0.0);
    let end = min(far.x, min(far.y, far.z));
    if t >= end { discard; }
    var hit = false;
    var closest = 10000.0;
    var closest_t = t;
    for (var step = 0u; step < 80u; step += 1u) {
        let distance = scene_distance(origin + direction * t);
        if distance < closest {
            closest = distance;
            closest_t = t;
        }
        if distance < 0.001 {
            hit = true;
            break;
        }
        t += max(distance * 0.9, 0.0005);
        if t > end { break; }
    }
    var outline = false;
    if !hit {
        let closest_view = view.view_from_world * vec4(origin + direction * closest_t, 1.0);
        var pixel_size = 2.0 / (abs(view.clip_from_view[1][1]) * view.viewport.w);
        if !orthographic { pixel_size *= max(-closest_view.z, 0.0001); }
        if closest > body.settings.z * pixel_size { discard; }
        outline = true;
        t = closest_t;
    }
    let point = origin + direction * t;
    let clip = view.clip_from_world * vec4(point, 1.0);
    var out: BodyOutput;
    out.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    out.color = vec4(0.008, 0.012, 0.019, 1.0);
    if !outline {
        let diffuse = max(dot(scene_normal(point), normalize(vec3(-0.45, 0.8, 0.65))), 0.0);
        let band = floor(min(diffuse, 0.999) * 3.0) / 2.0;
        out.color = vec4(scene_color(point) * (0.40 + 0.60 * band), 1.0);
    }
    return out;
}
