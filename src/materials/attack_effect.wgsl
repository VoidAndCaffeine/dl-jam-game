// Procedural swing graphics for the player's attacks.
//
// The mesh is a unit quad scaled so its local axes span `(reach, 2 * reach)`
// world units, centred on the swing's hitbox. Everything is drawn in the
// character's frame: the character sits at local `(-reach / 2, 0)` and forward
// is `+X`. One material therefore serves every weapon; `theme` recolours it.
//
// Two shapes share the shader:
//   * poke  - a tapered blade thrust straight ahead,
//   * slash - a crescent that sweeps from the character's right to their left.

#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::view,
}

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif
#ifdef SRGB_OUTPUT
#import bevy_render::color_operations::linear_to_srgb
#endif
#ifdef OKLAB_OUTPUT
#import bevy_render::color_operations::linear_rgb_to_oklab
#endif

const ATTACK_POKE: u32 = 0u;

/// Half the angular span of a slash, in radians (a little under a half turn).
const SLASH_HALF: f32 = 1.35;
/// How far the bright trail lags behind the leading edge, in radians.
const SLASH_TRAIL: f32 = 1.15;
/// Inner radius of the slash arc, as a fraction of `reach`.
const SLASH_INNER: f32 = 0.3;

struct AttackEffectMaterial {
    // 0 = poke, 1 = slash.
    attack_type: u32,
    // 0 starter, 1 boss a, 2 boss b, 3 master.
    theme: u32,
    // 0..1 across the swing's lifetime.
    progress: f32,
    // World reach of the swing.
    reach: f32,
    // Base width of a poke, in world units.
    width: f32,
    // Seconds since the swing began, drives the shimmer.
    time: f32,
    pad0: f32,
    pad1: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: AttackEffectMaterial;

fn ease_out_cubic(t: f32) -> f32 {
    let u = clamp(t, 0.0, 1.0) - 1.0;
    return u * u * u + 1.0;
}

/// `1` deep inside the shape, `0` outside, feathering over `soft` world units.
fn band(d: f32, soft: f32) -> f32 {
    return 1.0 - smoothstep(-soft, soft, d);
}

fn theme_core(theme: u32) -> vec3<f32> {
    switch theme {
        case 0u: { return vec3(1.0, 0.93, 0.80); } // starter: warm cream
        case 1u: { return vec3(1.0, 0.96, 0.80); } // fire: white hot
        case 2u: { return vec3(0.80, 0.62, 1.0); } // shadow: violet light
        default: { return vec3(1.0, 0.98, 1.0); }  // dream: pearl
    }
}

fn theme_accent(theme: u32, t: f32) -> vec3<f32> {
    switch theme {
        case 0u: { return vec3(0.74, 0.53, 0.28); } // wood
        case 1u: { return vec3(1.0, 0.36, 0.05); }  // ember
        case 2u: { return vec3(0.20, 0.04, 0.38); } // deep violet
        default: {
            // dream: a slow cycle through cyan -> rose -> gold.
            let a = vec3(0.45, 0.85, 1.0);
            let b = vec3(1.0, 0.55, 0.85);
            let c = vec3(1.0, 0.85, 0.4);
            let k = fract(t * 0.35);
            if (k < 0.333) { return mix(a, b, k * 3.0); }
            if (k < 0.666) { return mix(b, c, (k - 0.333) * 3.0); }
            return mix(c, a, (k - 0.666) * 3.0);
        }
    }
}

/// Overall opacity across the swing's life: snap in, hold, fall away.
fn life_curve(progress: f32) -> f32 {
    return smoothstep(0.0, 0.12, progress) * (1.0 - smoothstep(0.5, 1.0, progress));
}

/// A tapered blade thrust ahead of the character, growing out then dissolving.
fn poke_shape(q: vec2<f32>, reach: f32, width: f32, progress: f32, time: f32) -> f32 {
    let grow = ease_out_cubic(clamp(progress * 2.2, 0.0, 1.0));
    let r = reach * max(grow, 0.001);
    let t = q.x / r;
    if (t < 0.0 || t > 1.0) { return 0.0; }

    // Narrow to a point at the tip, with a gentle belly so it reads as a
    // leaf-shaped flash rather than a needle.
    let profile = width * 0.5 * (1.0 - t) + width * 0.16 * sin(t * 3.14159);
    let body = band(abs(q.y) - profile, 2.0);

    let spine_sigma = width * 0.18;
    let spine = exp(-(q.y * q.y) / (2.0 * spine_sigma * spine_sigma));
    let tip = smoothstep(0.55, 1.0, t);
    let flicker = 0.85 + 0.15 * sin(time * 40.0 + q.x * 0.3);
    return (body * (0.55 + 0.45 * spine) + body * tip * 0.5) * flicker;
}

/// A crescent that sweeps from the character's right (-Y) to their left (+Y).
fn slash_shape(q: vec2<f32>, reach: f32, progress: f32, time: f32) -> f32 {
    let sweep = ease_out_cubic(progress);
    let head = mix(-SLASH_HALF, SLASH_HALF, sweep);
    let tail = head - SLASH_TRAIL;

    let ang = atan2(q.y, q.x);
    let lo = max(tail, -SLASH_HALF);
    let hi = min(head, SLASH_HALF);
    // `band` is 1 *inside* (negative argument), so the window needs `lo - ang`
    // and `ang - hi`; the reversed forms would light up everything but the arc.
    let ang_mask = band(lo - ang, 0.06) * band(ang - hi, 0.06);
    if (ang_mask <= 0.0) { return 0.0; }

    let r = length(q);
    let outer = reach;
    let inner = reach * SLASH_INNER;
    let radial = band(r - outer, 4.0) * band(inner - r, 6.0);
    if (radial <= 0.0) { return 0.0; }

    // Brightest at the leading edge, fading back along the trail.
    let span = max(hi - lo, 0.001);
    let lag = clamp((hi - ang) / span, 0.0, 1.0);
    let lead = 1.0 - lag;
    let flicker = 0.85 + 0.15 * sin(time * 45.0 + ang * 6.0);
    return ang_mask * radial * (0.35 + 0.65 * lead * lead) * flicker;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let reach = material.reach;
    let width = material.width;
    let progress = material.progress;
    let time = material.time;

    // Local space, y up, forward = +X, centred on the hitbox. The uv's y axis
    // points down the sheet, so it is flipped to match the world.
    let local = vec2(mesh.uv.x - 0.5, 0.5 - mesh.uv.y) * vec2(reach, 2.0 * reach);
    // The character stands half a reach behind the hitbox centre.
    let q = local - vec2(-0.5 * reach, 0.0);

    var shape: f32;
    if (material.attack_type == ATTACK_POKE) {
        shape = poke_shape(q, reach, width, progress, time);
    } else {
        shape = slash_shape(q, reach, progress, time);
    }

    let alpha = clamp(shape, 0.0, 1.0) * life_curve(progress);
    if (alpha <= 0.001) { discard; }

    let core = theme_core(material.theme);
    let accent = theme_accent(material.theme, time);
    let color = mix(accent, core, clamp(shape, 0.0, 1.0)) + core * 0.2 * clamp(shape, 0.0, 1.0);

    var out = vec4(color, alpha * 0.92);
#ifdef TONEMAP_IN_SHADER
    out = tonemapping::tone_mapping(out, view.color_grading);
#endif
#ifdef SRGB_OUTPUT
    out = vec4(linear_to_srgb(out.rgb), out.a);
#endif
#ifdef OKLAB_OUTPUT
    out = vec4(linear_rgb_to_oklab(out.rgb), out.a);
#endif
    return out;
}
