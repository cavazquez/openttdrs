//! Keep nearest-sampled map sprites on Bevy's retained quad pipeline.
//!
//! Bevy 0.20 uses Mesh2d for Sprite, with different quad diagonals, UV rounding
//! and mask/material batching. Its text renderer retains the previous sprite
//! pipeline. Feed ordinary pixel sprites into that pipeline through its public
//! extraction API. Map occlusion masks retain the previous uncompressed quad
//! for atlas sampling. Other masked, sliced, tiled and scaled sprites use the
//! new backend.

use bevy::asset::AssetEventSystems;
use bevy::camera::visibility::{ViewVisibility, VisibilityClass};
use bevy::mesh::Mesh2d;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::sync_world::RenderEntity;
use bevy::render::{Extract, ExtractSchedule, RenderApp};
use bevy::sprite::{Anchor, SpriteAlphaMode};
use bevy::sprite_render::{
    ExtractedSlice, ExtractedSlices, ExtractedSprite, ExtractedSpriteKind, ExtractedSprites,
    MaterialExtension2d, MeshMaterial2d, SpriteMaterial, SpriteMeshMaterial, extract_text2d_sprite,
};
use std::any::TypeId;

#[derive(Component)]
struct OriginalPixelMesh(Mesh2d);

/// Opt out of the default Sprite material. This renderer owns extraction and
/// deliberately has no SpriteMaterialPlugin or per-sprite material asset.
#[derive(Asset, AsBindGroup, Reflect, Clone)]
struct PixelSpriteRenderer {}

impl MaterialExtension2d for PixelSpriteRenderer {}

pub(crate) fn install_pixel_sprite_renderer(app: &mut App) {
    app.add_systems(
        Update,
        select_pixel_renderer
            .after(crate::bevy_app::UpdateSet::RenderRefresh)
            .after(super::rail_glass_compositor::RailGlassProxySync),
    )
    .add_systems(
        PostUpdate,
        (remove_default_material, preserve_mask_quad).after(AssetEventSystems),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(
            ExtractSchedule,
            extract_pixel_sprites.after(extract_text2d_sprite),
        );
    }
}

fn eligible(sprite: &Sprite, proxy: bool) -> bool {
    (!proxy && sprite.alpha_mode == SpriteAlphaMode::Blend)
        && sprite.image_mode == SpriteImageMode::Auto
}

#[allow(clippy::type_complexity)]
fn select_pixel_renderer(
    mut commands: Commands,
    mut sprites: Query<
        (
            Entity,
            &Sprite,
            Has<SpriteMaterial<PixelSpriteRenderer>>,
            &mut VisibilityClass,
            Option<&OriginalPixelMesh>,
            Has<super::RailGlassMaskProxy>,
        ),
        Changed<Sprite>,
    >,
) {
    for (entity, sprite, selected, mut class, original, proxy) in &mut sprites {
        if eligible(sprite, proxy) && !selected {
            if !class.0.contains(&TypeId::of::<Sprite>()) {
                class.0.push(TypeId::of::<Sprite>());
            }
            commands
                .entity(entity)
                .insert(SpriteMaterial::<PixelSpriteRenderer>(Handle::default()));
        } else if !eligible(sprite, proxy) && selected {
            class.0.retain(|class| *class != TypeId::of::<Sprite>());
            if let Some(original) = original {
                if !class.0.contains(&TypeId::of::<Mesh2d>()) {
                    class.0.push(TypeId::of::<Mesh2d>());
                }
                commands
                    .entity(entity)
                    .insert(original.0.clone())
                    .remove::<OriginalPixelMesh>();
            }
            commands
                .entity(entity)
                .remove::<SpriteMaterial<PixelSpriteRenderer>>();
        }
    }
}

#[allow(clippy::type_complexity)]
fn remove_default_material(
    mut commands: Commands,
    mut sprites: Query<
        (Entity, Option<&Mesh2d>, &mut VisibilityClass),
        (
            With<SpriteMaterial<PixelSpriteRenderer>>,
            Or<(With<Mesh2d>, With<MeshMaterial2d<SpriteMeshMaterial>>)>,
        ),
    >,
) {
    for (entity, mesh, mut class) in &mut sprites {
        if let Some(mesh) = mesh {
            commands
                .entity(entity)
                .insert(OriginalPixelMesh(mesh.clone()))
                .remove::<Mesh2d>();
            class.0.retain(|class| *class != TypeId::of::<Mesh2d>());
        }
        commands
            .entity(entity)
            .remove::<MeshMaterial2d<SpriteMeshMaterial>>();
    }
}

fn preserve_mask_quad(
    mut meshes: ResMut<Assets<Mesh>>,
    mut quad: Local<Option<Handle<Mesh>>>,
    mut masks: Query<&mut Mesh2d, (With<super::RailGlassMaskProxy>, Changed<Mesh2d>)>,
) {
    for mut mesh in &mut masks {
        let quad = quad.get_or_insert_with(|| meshes.add(Mesh::from(Rectangle::new(1.0, 1.0))));
        mesh.set_if_neq(Mesh2d(quad.clone()));
    }
}
fn pixel_slice(
    sprite: &Sprite,
    anchor: Anchor,
    image_size: Vec2,
    layouts: &Assets<TextureAtlasLayout>,
) -> ExtractedSlice {
    let atlas_rect = sprite
        .texture_atlas
        .as_ref()
        .and_then(|atlas| atlas.texture_rect(layouts))
        .map(|rect| rect.as_rect());
    let mut rect = match (atlas_rect, sprite.rect) {
        (None, None) => Rect::from_corners(Vec2::ZERO, image_size),
        (None, Some(rect)) | (Some(rect), None) => rect,
        (Some(atlas), Some(mut rect)) => {
            rect.min += atlas.min;
            rect.max += atlas.min;
            rect
        }
    };
    let size = sprite.custom_size.unwrap_or_else(|| rect.size());
    if sprite.flip_x {
        std::mem::swap(&mut rect.min.x, &mut rect.max.x);
    }
    ExtractedSlice {
        offset: -size * anchor.as_vec(),
        rect,
        size,
    }
}

#[allow(clippy::type_complexity)]
fn extract_pixel_sprites(
    mut sprites_out: ResMut<ExtractedSprites>,
    mut slices: ResMut<ExtractedSlices>,
    images: Extract<Res<Assets<Image>>>,
    layouts: Extract<Res<Assets<TextureAtlasLayout>>>,
    sprites: Extract<
        Query<
            (
                Entity,
                RenderEntity,
                &ViewVisibility,
                &Sprite,
                &GlobalTransform,
                &Anchor,
            ),
            With<SpriteMaterial<PixelSpriteRenderer>>,
        >,
    >,
) {
    // Text extraction clears both buffers; append after it to preserve Text2d.
    for (entity, render, visibility, sprite, transform, anchor) in &sprites {
        if !visibility.get() {
            continue;
        }
        let Some(image) = images.get(&sprite.image) else {
            continue;
        };
        let start = slices.slices.len();
        slices.slices.push(pixel_slice(
            sprite,
            *anchor,
            image.size().as_vec2(),
            &layouts,
        ));
        let color: LinearRgba = sprite.color.into();
        sprites_out.sprites.push(ExtractedSprite {
            main_entity: entity,
            render_entity: render,
            transform: *transform,
            color,
            image_handle_id: sprite.image.id(),
            flip_y: sprite.flip_y,
            kind: ExtractedSpriteKind::Slices {
                indices: start..start + 1,
            },
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn atlas_subrect_size_anchor_and_horizontal_flip_are_preserved() {
        let mut layouts = Assets::default();
        let mut layout = TextureAtlasLayout::new_empty(UVec2::new(73, 53));
        layout.add_texture(URect::new(10, 11, 60, 48));
        let mut sprite = Sprite {
            texture_atlas: Some(TextureAtlas {
                layout: layouts.add(layout),
                index: 0,
            }),
            rect: Some(Rect::new(7.0, 2.0, 28.0, 20.0)),
            custom_size: Some(Vec2::new(42.0, 36.0)),
            ..default()
        };
        let slice = pixel_slice(&sprite, Anchor::TOP_LEFT, Vec2::new(73.0, 53.0), &layouts);
        assert_eq!(slice.rect, Rect::new(17.0, 13.0, 38.0, 31.0));
        assert_eq!(slice.size, Vec2::new(42.0, 36.0));
        assert_eq!(slice.offset, Vec2::new(21.0, -18.0));
        sprite.flip_x = true;
        let flipped = pixel_slice(&sprite, Anchor::TOP_LEFT, Vec2::new(73.0, 53.0), &layouts);
        assert_eq!(flipped.rect.min, Vec2::new(38.0, 13.0));
        assert_eq!(flipped.rect.max, Vec2::new(17.0, 31.0));
        assert_eq!(flipped.size, slice.size);
        assert_eq!(flipped.offset, slice.offset);
    }

    #[test]
    fn renderer_selection_is_stable_and_restores_stock_modes() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (select_pixel_renderer, remove_default_material).chain(),
        );
        let source = app
            .world_mut()
            .spawn((
                Sprite::default(),
                MeshMaterial2d::<SpriteMeshMaterial>(Handle::default()),
                Mesh2d(Handle::default()),
            ))
            .id();
        let mask = app
            .world_mut()
            .spawn(Sprite {
                alpha_mode: SpriteAlphaMode::Mask(0.5),
                ..default()
            })
            .id();
        let proxy = app
            .world_mut()
            .spawn((
                Sprite {
                    alpha_mode: SpriteAlphaMode::Mask(0.5),
                    ..default()
                },
                super::super::RailGlassMaskProxy { source },
                Mesh2d(Handle::default()),
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<SpriteMaterial<PixelSpriteRenderer>>(source)
                .is_some()
        );
        assert!(
            app.world()
                .get::<MeshMaterial2d<SpriteMeshMaterial>>(source)
                .is_none()
        );
        assert!(app.world().get::<Mesh2d>(source).is_none());
        assert!(
            app.world()
                .get::<VisibilityClass>(source)
                .unwrap()
                .0
                .contains(&TypeId::of::<Sprite>())
        );
        assert!(
            app.world()
                .get::<SpriteMaterial<PixelSpriteRenderer>>(mask)
                .is_none()
        );
        assert!(
            app.world()
                .get::<SpriteMaterial<PixelSpriteRenderer>>(proxy)
                .is_none()
        );
        assert!(app.world().get::<Mesh2d>(proxy).is_some());
        app.world_mut().clear_trackers();
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, Changed<SpriteMaterial<PixelSpriteRenderer>>>()
                .iter(app.world())
                .count(),
            0
        );
        app.world_mut()
            .get_mut::<Sprite>(source)
            .unwrap()
            .image_mode = SpriteImageMode::Tiled {
            tile_x: true,
            tile_y: false,
            stretch_value: 1.0,
        };
        app.update();
        assert!(
            app.world()
                .get::<SpriteMaterial<PixelSpriteRenderer>>(source)
                .is_none()
        );
        assert!(app.world().get::<Mesh2d>(source).is_some());
        assert!(
            app.world()
                .get::<VisibilityClass>(source)
                .unwrap()
                .0
                .contains(&TypeId::of::<Mesh2d>())
        );
        assert!(
            !app.world()
                .get::<VisibilityClass>(source)
                .unwrap()
                .0
                .contains(&TypeId::of::<Sprite>())
        );
    }

    #[test]
    fn masks_share_the_original_quad_without_repeated_mesh_changes() {
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default())
            .add_systems(Update, preserve_mask_quad);
        let source = app.world_mut().spawn_empty().id();
        let first = app
            .world_mut()
            .spawn((
                Mesh2d(Handle::default()),
                super::super::RailGlassMaskProxy { source },
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                Mesh2d(Handle::default()),
                super::super::RailGlassMaskProxy { source },
            ))
            .id();
        app.update();
        let quad = app.world().get::<Mesh2d>(first).unwrap().0.clone();
        assert_eq!(app.world().get::<Mesh2d>(second).unwrap().0, quad);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        app.world_mut().clear_trackers();
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, Changed<Mesh2d>>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
