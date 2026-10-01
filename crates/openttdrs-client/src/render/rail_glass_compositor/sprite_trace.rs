//! Opt-in capture of the inputs not covered by the parent-sort trace.
//! Runs once, on the screenshot frame, after transform and visibility propagation.
//! CPU image bytes and both glass targets are retained alongside the JSON.

use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
};

use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::Screenshot;
use bevy::render::view::screenshot::ScreenshotCaptured;
use bevy::sprite::{Anchor, SpriteMesh};
use bevy::{asset::AssetId, prelude::*, render::sync_world::RenderEntity};
use serde_json::{Value, json};

use super::{
    RailGlassMaskCamera, RailGlassMaskProxy, RailGlassMaskSource, RailGlassOcclusionCamera,
    RailGlassPostProcessAssets,
};
use crate::render::{
    MapDynamicVisual, MapTileChunk, MapVisualLayer, PrimaryGameCamera, ViewportSortableChild,
    ViewportSortableParent,
};

#[derive(Resource)]
pub(crate) struct MapSpriteTraceRequest(pub(crate) PathBuf);

fn save_mask_bytes(image: &Image, path: &std::path::Path) -> io::Result<()> {
    if image.texture_descriptor.format != TextureFormat::Rgba8Unorm {
        return Err(io::Error::other(
            "glass trace requires an RGBA8 unorm target",
        ));
    }
    let bytes = image
        .data
        .as_ref()
        .ok_or_else(|| io::Error::other("mask capture has no CPU bytes"))?;
    image::save_buffer_with_format(
        path,
        bytes,
        image.width(),
        image.height(),
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(io::Error::other)
}

fn save_mask_to_disk(path: PathBuf) -> impl FnMut(On<ScreenshotCaptured>) {
    move |capture| {
        if let Err(error) = save_mask_bytes(&capture.image, &path) {
            error!("glass mask capture {}: {error}", path.display());
        }
    }
}

#[derive(Default)]
struct TraceAssets {
    images: Vec<Handle<Image>>,
    image_indices: HashMap<AssetId<Image>, usize>,
    layouts: Vec<Handle<TextureAtlasLayout>>,
    layout_indices: HashMap<AssetId<TextureAtlasLayout>, usize>,
}

impl TraceAssets {
    fn sprite(&mut self, sprite: &Sprite) -> Value {
        let image = *self
            .image_indices
            .entry(sprite.image.id())
            .or_insert_with(|| {
                self.images.push(sprite.image.clone());
                self.images.len() - 1
            });
        let atlas = sprite.texture_atlas.as_ref().map(|atlas| {
            let layout = *self
                .layout_indices
                .entry(atlas.layout.id())
                .or_insert_with(|| {
                    self.layouts.push(atlas.layout.clone());
                    self.layouts.len() - 1
                });
            json!({"layout": layout, "index": atlas.index})
        });
        json!({
            "image": image, "atlas": atlas, "color": format!("{:?}", sprite.color),
            "flip": [sprite.flip_x, sprite.flip_y],
            "size_bits": sprite.custom_size.map(|size| size.to_array().map(f32::to_bits)),
            "rect_bits": sprite.rect.map(|rect| [rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(f32::to_bits)),
            "image_mode": format!("{:?}", sprite.image_mode),
        })
    }

    fn write(&self, world: &World, directory: &std::path::Path) -> io::Result<Value> {
        // Refuse to overwrite the asset bytes of an earlier diagnostic capture.
        std::fs::create_dir(directory)?;
        let images = world.resource::<Assets<Image>>();
        let mut image_rows = Vec::new();
        for (index, handle) in self.images.iter().enumerate() {
            let row = if let Some(image) = images.get(handle) {
                let data_file = image.data.as_ref().map(|_| format!("image-{index}.bin"));
                if let (Some(bytes), Some(file)) = (&image.data, &data_file) {
                    std::fs::write(directory.join(file), bytes)?;
                }
                json!({
                    "asset_id": format!("{:?}", handle.id()),
                    "path": world.get_resource::<AssetServer>().and_then(|server| server.get_path(handle.id())).map(|path| path.to_string()),
                    "descriptor": format!("{:?}", image.texture_descriptor),
                    "sampler": format!("{:?}", image.sampler),
                    "data_order": format!("{:?}", image.data_order),
                    "data_file": data_file,
                    "data_bytes": image.data.as_ref().map(Vec::len),
                })
            } else {
                json!({"asset_id": format!("{:?}", handle.id()), "missing": true})
            };
            image_rows.push(row);
        }
        let layouts = world.resource::<Assets<TextureAtlasLayout>>();
        let layout_rows: Vec<_> = self.layouts.iter().map(|handle| {
            let layout = layouts.get(handle);
            json!({
                "asset_id": format!("{:?}", handle.id()),
                "size": layout.map(|layout| layout.size.to_array()),
                "textures": layout.map(|layout| layout.textures.iter().map(|rect| [rect.min.x, rect.min.y, rect.max.x, rect.max.y]).collect::<Vec<_>>()),
            })
        }).collect();
        Ok(json!({"images": image_rows, "layouts": layout_rows}))
    }
}

fn entity_inputs(world: &World, entity: Entity) -> Value {
    let parent = world.get::<ViewportSortableParent>(entity);
    let child = world.get::<ViewportSortableChild>(entity);
    json!({
        "entity": entity.to_bits(),
        "render_entity": world.get::<RenderEntity>(entity).map(|render| render.id().to_bits()),
        "transform_bits": world.get::<GlobalTransform>(entity).map(|transform| transform.to_matrix().to_cols_array().map(f32::to_bits)),
        "anchor_bits": world.get::<Anchor>(entity).map(|anchor| anchor.as_vec().to_array().map(f32::to_bits)),
        "visibility": world.get::<Visibility>(entity).map(|value| format!("{value:?}")),
        "inherited_visibility": world.get::<InheritedVisibility>(entity).map(|value| value.get()),
        "view_visibility": world.get::<ViewVisibility>(entity).map(|value| value.get()),
        "layers": world.get::<bevy::camera::visibility::RenderLayers>(entity).map(|layers| layers.iter().collect::<Vec<_>>()).unwrap_or_else(|| vec![0]),
        "map": world.get::<MapVisualLayer>(entity).is_some(),
        "dynamic": world.get::<MapDynamicVisual>(entity).is_some(),
        "glass": world.get::<RailGlassMaskSource>(entity).is_some(),
        "proxy_source": world.get::<RailGlassMaskProxy>(entity).map(|proxy| proxy.source.to_bits()),
        "chunk": world.get::<MapTileChunk>(entity).map(|chunk| [chunk.cx, chunk.cy]),
        "parent": parent.map(|parent| json!({"sprite_id": parent.sprite_id, "insertion_key": parent.insertion_key, "source_depth_bits": parent.source_depth.to_bits(), "bounds": format!("{:?}", parent.bounds)})),
        "child": child.map(|child| json!({"parent": child.parent.to_bits(), "source_depth_bits": child.source_depth.to_bits()})),
    })
}

fn capture_inputs(world: &mut World) -> (Value, TraceAssets) {
    let mut assets = TraceAssets::default();
    let mut sprites_query =
        world.query::<(Entity, &Sprite, &GlobalTransform, &Anchor, &ViewVisibility)>();
    let sprites: Vec<_> = sprites_query.iter(world).map(|(entity, sprite, ..)| {
        json!({"inputs": entity_inputs(world, entity), "sprite": assets.sprite(sprite)})
    }).collect();
    let mut meshes_query = world.query::<(Entity, &SpriteMesh)>();
    let meshes: Vec<_> = meshes_query.iter(world).map(|(entity, mesh)| {
        let sprite = Sprite {
            image: mesh.image.clone(), texture_atlas: mesh.texture_atlas.clone(),
            color: mesh.color, flip_x: mesh.flip_x, flip_y: mesh.flip_y,
            custom_size: mesh.custom_size, rect: mesh.rect, image_mode: mesh.image_mode.clone(),
        };
        json!({"inputs": entity_inputs(world, entity), "sprite": assets.sprite(&sprite), "alpha_mode": format!("{:?}", mesh.alpha_mode)})
    }).collect();
    let mut cameras_query = world.query::<(Entity, &Camera, &Projection)>();
    let cameras: Vec<_> = cameras_query
        .iter(world)
        .map(|(entity, camera, projection)| {
            json!({
                "inputs": entity_inputs(world, entity), "order": camera.order,
                "active": camera.is_active, "viewport": format!("{:?}", camera.viewport),
                "projection": format!("{projection:?}"),
                "clip_from_view_bits": projection.get_clip_from_view().to_cols_array().map(f32::to_bits),
                "primary": world.get::<PrimaryGameCamera>(entity).is_some(),
                "coverage": world.get::<RailGlassMaskCamera>(entity).is_some(),
                "occlusion": world.get::<RailGlassOcclusionCamera>(entity).is_some(),
            })
        })
        .collect();
    (
        json!({"schema": 1, "sprites": sprites, "meshes": meshes, "cameras": cameras}),
        assets,
    )
}

pub(super) fn export_requested_sprite_trace(world: &mut World) {
    let Some(MapSpriteTraceRequest(path)) = world.remove_resource::<MapSpriteTraceRequest>() else {
        return;
    };
    let (mut document, assets) = capture_inputs(world);
    let result = (|| -> io::Result<()> {
        document["assets"] = assets.write(world, &path.with_extension("images"))?;
        let file = std::fs::File::create(&path)?;
        let mut writer = io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, &document).map_err(io::Error::other)?;
        writer.flush()
    })();
    if let Err(error) = result {
        error!("map sprite trace {}: {error}", path.display());
        return;
    }
    if let Some(targets) = world.get_resource::<RailGlassPostProcessAssets>().cloned() {
        world
            .spawn(Screenshot::image(targets.mask))
            .observe(save_mask_to_disk(path.with_extension("coverage.png")));
        world
            .spawn(Screenshot::image(targets.visibility))
            .observe(save_mask_to_disk(path.with_extension("occlusion.png")));
    }
    info!("map sprite trace saved to {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_retains_proxy_aliases_asset_sharing_and_exact_float_bits() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::default());
        world.insert_resource(images);
        world.insert_resource(Assets::<TextureAtlasLayout>::default());
        let source_sprite = Sprite::from_image(image.clone());
        let source_transform = Transform::from_xyz(-0.0, 1.0, f32::from_bits(0x3f800001));
        let source = world
            .spawn((
                source_sprite.clone(),
                MapVisualLayer,
                RailGlassMaskSource,
                source_transform,
            ))
            .id();
        let transform = GlobalTransform::from(source_transform);
        world.entity_mut(source).insert(transform);
        let proxy = world
            .spawn((
                super::super::mask_sprite_from_source(&source_sprite, true),
                RailGlassMaskProxy { source },
                transform,
            ))
            .id();
        let (document, assets) = capture_inputs(&mut world);
        let sprite = &document["sprites"][0];
        let mesh = &document["meshes"][0];
        assert_eq!(sprite["inputs"]["entity"], source.to_bits());
        assert_eq!(mesh["inputs"]["entity"], proxy.to_bits());
        assert_eq!(mesh["inputs"]["proxy_source"], sprite["inputs"]["entity"]);
        assert_eq!(mesh["sprite"]["image"], sprite["sprite"]["image"]);
        assert_eq!(assets.images.len(), 1);
        assert_eq!(sprite["inputs"]["transform_bits"][14], 0x3f800001_u32);
        assert_eq!(
            sprite["inputs"]["transform_bits"],
            mesh["inputs"]["transform_bits"]
        );
        assert_eq!(mesh["alpha_mode"], "Mask(0.5)");
    }

    #[test]
    fn trace_asset_bytes_are_exact_and_existing_capture_is_preserved() -> io::Result<()> {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let mut image = Image::default();
        let bytes = vec![17, 42, 89, 255];
        image.data = Some(bytes.clone());
        let handle = images.add(image);
        world.insert_resource(images);
        world.insert_resource(Assets::<TextureAtlasLayout>::default());
        world.spawn(Sprite::from_image(handle));
        let (_, assets) = capture_inputs(&mut world);
        let temporary = tempfile::tempdir()?;
        let directory = temporary.path().join("images");
        let table = assets.write(&world, &directory)?;
        assert_eq!(table["images"][0]["data_bytes"], bytes.len());
        assert_eq!(std::fs::read(directory.join("image-0.bin"))?, bytes);
        assert!(assets.write(&world, &directory).is_err());
        assert_eq!(std::fs::read(directory.join("image-0.bin"))?, bytes);
        Ok(())
    }

    #[test]
    fn mask_png_preserves_unorm_bytes_without_srgb_conversion() -> io::Result<()> {
        let mut mask = Image::default();
        mask.texture_descriptor.format = TextureFormat::Rgba8Unorm;
        let bytes = vec![17, 42, 89, 128];
        mask.data = Some(bytes.clone());
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("mask.png");
        save_mask_bytes(&mask, &path)?;
        let decoded = image::open(&path).map_err(io::Error::other)?.into_rgba8();
        assert_eq!(decoded.into_raw(), bytes);
        mask.texture_descriptor.format = TextureFormat::Rgba8UnormSrgb;
        assert!(save_mask_bytes(&mask, &path).is_err());
        Ok(())
    }
}
