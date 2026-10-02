// Experimental mapping retained after the scene candidate was withdrawn.
// BEGIN_MASK_DEPTH_MAPPING
fn mask_ranked_clip_position(original: vec4<f32>, tag: u32, flat: bool) -> vec4<f32> {
    let rank = tag & 0x007fffffu;
    if (tag & 0x80000000u) == 0u || rank == 0u || !flat || original.w != 1.0 {
        return original;
    }
    // Preserve the original clipping decision before assigning a depth rank.
    if !(original.z >= 0.0 && original.z <= original.w) {
        return original;
    }
    var ranked = original;
    ranked.z = bitcast<f32>(0x3e800000u + rank);
    return ranked;
}
// END_MASK_DEPTH_MAPPING
