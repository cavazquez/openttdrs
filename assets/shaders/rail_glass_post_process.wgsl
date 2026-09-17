#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var nearest_sampler: sampler;
@group(0) @binding(2) var glass_mask_texture: texture_2d<f32>;
@group(0) @binding(3) var transparent_lut_texture: texture_2d<f32>;

fn srgb_to_linear(value: vec3<f32>) -> vec3<f32> {
    return select(
        pow((value + vec3(0.055)) / vec3(1.055), vec3(2.4)),
        value / vec3(12.92),
        value <= vec3(0.04045),
    );
}

fn linear_to_srgb(value: vec3<f32>) -> vec3<f32> {
    let non_negative = max(value, vec3(0.0));
    return select(
        vec3(1.055) * pow(non_negative, vec3(1.0 / 2.4)) - vec3(0.055),
        non_negative * vec3(12.92),
        non_negative <= vec3(0.0031308),
    );
}

@fragment
fn fs_main(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_texture, nearest_sampler, input.uv);
    let mask = textureSample(glass_mask_texture, nearest_sampler, input.uv).a;
    if mask <= 0.0001 {
        return scene;
    }

    // ViewTarget is normally an sRGB surface. Convert its linear sample back
    // to the byte-domain used by OpenTTD's 8bpp palette lookup.
    let scene_srgb = clamp(linear_to_srgb(scene.rgb), vec3(0.0), vec3(1.0));
    let buckets = vec3<u32>(
        min(u32(floor(scene_srgb.r * 64.0)), 63u),
        min(u32(floor(scene_srgb.g * 64.0)), 63u),
        min(u32(floor(scene_srgb.b * 64.0)), 63u),
    );
    let lut_coord = vec2<i32>(
        i32(buckets.r * 64u + buckets.g),
        i32(buckets.b),
    );
    let mapped_srgb = textureLoad(transparent_lut_texture, lut_coord, 0).rgb;
    let mapped_linear = srgb_to_linear(mapped_srgb);
    return vec4(mix(scene.rgb, mapped_linear, clamp(mask, 0.0, 1.0)), scene.a);
}
