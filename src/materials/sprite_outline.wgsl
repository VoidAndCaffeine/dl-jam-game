// Draws a textured quad whose silhouette can be outlined to mark it as the
// current interaction target.
//
// The texture's alpha carries the sprite silhouette, so the shader never needs
// the geometry's shape. When `highlight` is on, a ring of samples around each
// pixel finds the edge of that silhouette and paints an outline just outside it,
// pulsing gently. The same material backs the dirt mounds and the farm props.

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

/// Number of taps used to find the silhouette edge. Eight is enough to keep the
/// outline smooth without a visible starburst.
const OUTLINE_TAPS: i32 = 8;
const TAU: f32 = 6.2831855;

struct SpriteOutlineMaterial {
    // 0 bare, 1 fully outlined.
    highlight: f32,
    // Seconds since start, drives the pulse.
    time: f32,
    // One texel in UV space.
    texel: f32,
    // Outline reach past the silhouette, in texels.
    outline_width: f32,
    // RGBA outline colour.
    color: vec4<f32>,
    // Sub-rectangle of the texture this quad samples: xy is the top-left uv and
    // zw its size in uv. (0,0,1,1) draws the whole texture; a sprite-sheet frame
    // sets its own cell so the outline follows that frame instead of the sheet.
    uv_rect: vec4<f32>,
    // RGBA multiplier over the sampled texture. White is untouched; a soaked
    // crop tints this to read as watered.
    tint: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: SpriteOutlineMaterial;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var sprite_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var sprite_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // Map the quad's unit uv into the sampled sub-rectangle of the texture.
    let rect_min = material.uv_rect.xy;
    let rect_max = material.uv_rect.xy + material.uv_rect.zw;
    let uv = rect_min + mesh.uv * material.uv_rect.zw;
    let center = textureSample(sprite_texture, sprite_sampler, uv) * material.tint;
    var color = center;

    if (material.highlight > 0.001) {
        // Largest alpha anywhere near this pixel. Where the centre is
        // transparent but a neighbour is not, this pixel sits on the outer rim
        // of the silhouette and earns an outline.
        let radius = material.texel * material.outline_width;
        // Keep the taps inside the sub-rectangle, or a sheet frame would sample
        // its neighbours and paint a false edge along the frame border.
        let tap_min = rect_min + material.texel;
        let tap_max = rect_max - material.texel;
        var max_alpha = center.a;
        for (var i = 0; i < OUTLINE_TAPS; i = i + 1) {
            let angle = f32(i) * TAU / f32(OUTLINE_TAPS);
            let offset = vec2(cos(angle), sin(angle)) * radius;
            let tap = textureSample(
                sprite_texture,
                sprite_sampler,
                clamp(uv + offset, tap_min, tap_max),
            );
            max_alpha = max(max_alpha, tap.a);
        }

        let edge = clamp(max_alpha - center.a, 0.0, 1.0);
        let pulse = 0.7 + 0.3 * sin(material.time * 6.0);
        let outline_alpha = edge * material.color.a * pulse * material.highlight;
        color = vec4(
            mix(center.rgb, material.color.rgb, edge),
            max(center.a, outline_alpha),
        );
    }

    var out = color;
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
