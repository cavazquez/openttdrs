//! The In2x Kale capture contains distinct parent/child depths that collide
//! after projection. A unique world-space depth does not guarantee painter
//! order in the alpha-mask depth buffer.

use bevy::camera::CameraProjection;
use bevy::prelude::*;

#[test]
fn distinct_parent_and_child_world_depths_collapse_after_projection() {
    let projection = OrthographicProjection {
        near: -2000.0,
        far: 2000.0,
        ..OrthographicProjection::default_2d()
    };
    let camera = GlobalTransform::from(Transform::from_xyz(0.0, -4104.0, 999.9));
    let clip_from_world = projection.get_clip_from_view() * camera.to_matrix().inverse();
    let parent = f32::from_bits(1075726681);
    let child = f32::from_bits(1075726728);
    assert!(parent < child);
    assert_eq!(
        clip_from_world
            .transform_point3(Vec3::new(0.0, 0.0, parent))
            .z
            .to_bits(),
        clip_from_world
            .transform_point3(Vec3::new(0.0, 0.0, child))
            .z
            .to_bits()
    );
}
