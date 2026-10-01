//! Pass 2D para reproducir `PALETTE_TO_TRANSPARENT` sobre el framebuffer.
//!
//! El blitter 8bpp de OpenTTD no mezcla un color fijo: busca el color que ya
//! está en destino y lo reemplaza por su entrada correspondiente de la tabla
//! de transparencia. La máscara se renderiza en una cámara separada para que
//! el pass pueda conservar el mapa debajo y aplicar la misma transformación
//! dependiente del destino.

use bevy::asset::RenderAssetUsages;
use bevy::camera::{RenderTarget, visibility::RenderLayers};
use bevy::core_pipeline::{
    Core2dSystems, FullscreenShader, schedule::Core2d as Core2dSchedule, tonemapping::tonemapping,
};
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AddressMode, BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
    CachedRenderPipelineId, Canonical, ColorTargetState, ColorWrites, FilterMode, FragmentState,
    MipmapFilterMode, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderStages, Specializer, SpecializerKey, TextureFormat, TextureSampleType, TextureViewId,
    Variants, binding_types::sampler, binding_types::texture_2d,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use bevy::sprite::{Anchor, SpriteAlphaMode, SpriteMesh};
use bevy::window::PrimaryWindow;
use openttdrs_core::newgrf_sprites::{
    PALETTE_LOOKUP_TEXTURE_HEIGHT, PALETTE_LOOKUP_TEXTURE_WIDTH, palette_to_transparent_lut_rgba8,
};

use crate::render::{MapDynamicVisual, MapTileChunk, MapVisualLayer};

/// Capa reservada para la máscara del vidrio. Las entidades sin `RenderLayers`
/// siguen perteneciendo a la capa 0, que es la cámara principal.
pub(crate) const RAIL_GLASS_RENDER_LAYER: usize = 1;

/// Capa del pase auxiliar que sólo calcula qué píxeles del vidrio quedan
/// delante de los sprites opacos del mapa.
const RAIL_GLASS_OCCLUSION_RENDER_LAYER: usize = 2;

const RAIL_GLASS_SHADER_PATH: &str = "assets/shaders/rail_glass_post_process.wgsl";

/// Marca la cámara principal cuyo framebuffer necesita el pass de vidrio.
#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub(crate) struct RailGlassPostProcessSettings;

#[derive(Component)]
struct RailGlassMaskCamera;

#[derive(Component)]
struct RailGlassOcclusionCamera;

/// Identifica el `Sprite` de mapa que representa una cubierta de vidrio.
///
/// El sprite conserva su backend original para que la cobertura final siga
/// coincidiendo con el atlas. El pase auxiliar crea un proxy con depth test.
#[derive(Component, Clone, Copy)]
pub(crate) struct RailGlassMaskSource;

#[derive(Component, Clone, Copy)]
struct RailGlassMaskProxy {
    source: Entity,
}

struct CachedMaskProxy {
    entity: Entity,
    seen: bool,
    is_glass: Option<bool>,
}

/// Only this system creates mask proxies. Keep their association across frames;
/// entity generations and a live query check also cover chunk remaps/despawns.
#[derive(Default)]
struct RailGlassMaskProxyCache {
    initialized: bool,
    by_source: EntityHashMap<CachedMaskProxy>,
}

/// Handles compartidos entre el mundo principal y el render world.
#[derive(Resource, Clone, ExtractResource)]
struct RailGlassPostProcessAssets {
    mask: Handle<Image>,
    visibility: Handle<Image>,
    lut: Handle<Image>,
}

pub(crate) struct RailGlassCompositorPlugin;

impl Plugin for RailGlassCompositorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<RailGlassPostProcessSettings>::default(),
            ExtractResourcePlugin::<RailGlassPostProcessAssets>::default(),
        ))
        .add_systems(Startup, setup_rail_glass_targets)
        .add_systems(
            Update,
            (
                sync_rail_glass_mask_camera,
                sync_rail_glass_mask_proxies.after(crate::bevy_app::UpdateSet::RenderRefresh),
            ),
        );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(RenderStartup, init_rail_glass_pipeline)
            .add_systems(
                Render,
                prepare_rail_glass_pipelines.in_set(RenderSystems::Prepare),
            )
            .add_systems(
                Core2dSchedule,
                apply_rail_glass_post_process
                    .in_set(Core2dSystems::PostProcess)
                    .before(tonemapping),
            );
    }
}

fn mask_sprite_from_source(source: &Sprite, is_glass: bool) -> SpriteMesh {
    SpriteMesh {
        image: source.image.clone(),
        texture_atlas: source.texture_atlas.clone(),
        color: if is_glass {
            // Rojo = fragmento de vidrio que ganó el depth test.
            Color::srgb(1.0, 0.0, 0.0)
        } else {
            // Los oclusores sólo escriben profundidad.
            Color::srgb(0.0, 0.0, 0.0)
        },
        flip_x: source.flip_x,
        flip_y: source.flip_y,
        custom_size: source.custom_size,
        rect: source.rect,
        image_mode: source.image_mode.clone(),
        alpha_mode: SpriteAlphaMode::Mask(0.5),
    }
}

fn mask_sprite_matches_source(mask: &SpriteMesh, source: &Sprite, is_glass: bool) -> bool {
    mask.image == source.image
        && mask.texture_atlas == source.texture_atlas
        && mask.color
            == if is_glass {
                Color::srgb(1.0, 0.0, 0.0)
            } else {
                Color::srgb(0.0, 0.0, 0.0)
            }
        && mask.flip_x == source.flip_x
        && mask.flip_y == source.flip_y
        && mask.custom_size == source.custom_size
        && mask.rect == source.rect
        && mask.image_mode == source.image_mode
        && mask.alpha_mode == SpriteAlphaMode::Mask(0.5)
}

/// Duplica los sprites de mapa en una cámara auxiliar con depth test.
///
/// El resultado de este pase no se usa como cobertura: sólo indica si el
/// vidrio ganó frente a un sprite normal. La cobertura continúa viniendo de
/// la cámara original `Sprite`, que conserva exactamente el muestreo del atlas.
fn sync_rail_glass_mask_proxies(
    mut commands: Commands,
    mut cache: Local<RailGlassMaskProxyCache>,
    sources: Query<
        (
            Entity,
            Ref<Sprite>,
            Ref<Anchor>,
            Ref<Transform>,
            Ref<Visibility>,
            Option<Ref<MapTileChunk>>,
            Option<&RailGlassMaskSource>,
        ),
        (
            Or<(With<MapVisualLayer>, With<MapDynamicVisual>)>,
            Without<RailGlassMaskProxy>,
        ),
    >,
    mut proxies: Query<(
        Entity,
        &RailGlassMaskProxy,
        &mut SpriteMesh,
        &mut Anchor,
        &mut Transform,
        &mut Visibility,
        Option<&mut MapTileChunk>,
    )>,
) {
    let _measurement = crate::performance::measure(crate::performance::Phase::Glass);
    if !cache.initialized {
        for (entity, proxy, ..) in &mut proxies {
            cache.by_source.insert(
                proxy.source,
                CachedMaskProxy {
                    entity,
                    seen: false,
                    is_glass: None,
                },
            );
        }
        cache.initialized = true;
    }

    for (
        source_entity,
        source_sprite,
        source_anchor,
        source_transform,
        source_visibility,
        source_chunk,
        glass_source,
    ) in &sources
    {
        if let Some(cached) = cache.by_source.get_mut(&source_entity)
            && let Ok((
                _,
                _,
                mut proxy_sprite,
                mut proxy_anchor,
                mut proxy_transform,
                mut proxy_visibility,
                proxy_chunk,
            )) = proxies.get_mut(cached.entity)
        {
            cached.seen = true;
            let is_glass = glass_source.is_some();
            let initialize = cached.is_glass.is_none();
            // Compare the borrowed fields before cloning handles/atlas/mode.
            // Proxy change ticks detect external edits. Remember classification
            // separately: removing the glass marker does not change the Sprite.
            if (source_sprite.is_changed()
                || proxy_sprite.is_changed()
                || cached.is_glass != Some(is_glass))
                && !mask_sprite_matches_source(&proxy_sprite, &source_sprite, is_glass)
            {
                *proxy_sprite = mask_sprite_from_source(&source_sprite, is_glass);
            }
            cached.is_glass = Some(is_glass);
            if (initialize || source_anchor.is_changed() || proxy_anchor.is_changed())
                && *proxy_anchor != *source_anchor
            {
                *proxy_anchor = *source_anchor;
            }
            if (initialize || source_transform.is_changed() || proxy_transform.is_changed())
                && *proxy_transform != *source_transform
            {
                *proxy_transform = *source_transform;
            }
            if (initialize || source_visibility.is_changed() || proxy_visibility.is_changed())
                && *proxy_visibility != *source_visibility
            {
                *proxy_visibility = *source_visibility;
            }
            if let (Some(source_chunk), Some(mut proxy_chunk)) = (source_chunk, proxy_chunk)
                && (initialize || source_chunk.is_changed() || proxy_chunk.is_changed())
                && *proxy_chunk != *source_chunk
            {
                *proxy_chunk = *source_chunk;
            }
            continue;
        }

        let proxy_entity = commands
            .spawn((
                RailGlassMaskProxy {
                    source: source_entity,
                },
                MapVisualLayer,
                mask_sprite_from_source(&source_sprite, glass_source.is_some()),
                *source_anchor,
                *source_transform,
                *source_visibility,
                RenderLayers::layer(RAIL_GLASS_OCCLUSION_RENDER_LAYER),
            ))
            .id();
        if let Some(source_chunk) = source_chunk {
            commands.entity(proxy_entity).insert(*source_chunk);
        }
        cache.by_source.insert(
            source_entity,
            CachedMaskProxy {
                entity: proxy_entity,
                seen: true,
                is_glass: Some(glass_source.is_some()),
            },
        );
    }

    cache.by_source.retain(|_, cached| {
        let live = cached.seen;
        cached.seen = false;
        if !live && proxies.contains(cached.entity) {
            commands.entity(cached.entity).despawn();
        }
        live
    });
}

fn setup_rail_glass_targets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Some(window) = windows.iter().next() else {
        // Headless tests do not have a render target and do not materialize
        // the world camera, so there is nothing to configure here.
        return;
    };
    let (width, height) = physical_window_size(window);
    let mask = images.add(Image::new_target_texture(
        width,
        height,
        TextureFormat::Rgba8Unorm,
        None,
    ));
    let visibility = images.add(Image::new_target_texture(
        width,
        height,
        TextureFormat::Rgba8Unorm,
        None,
    ));
    let lut = images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: PALETTE_LOOKUP_TEXTURE_WIDTH,
            height: PALETTE_LOOKUP_TEXTURE_HEIGHT,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        palette_to_transparent_lut_rgba8(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    ));
    commands.insert_resource(RailGlassPostProcessAssets {
        mask: mask.clone(),
        visibility: visibility.clone(),
        lut,
    });
    commands.spawn((
        Camera2d,
        RailGlassMaskCamera,
        Camera {
            order: -100,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        RenderTarget::from(mask),
        RenderLayers::layer(RAIL_GLASS_RENDER_LAYER),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
    commands.spawn((
        Camera2d,
        RailGlassOcclusionCamera,
        Camera {
            order: -101,
            is_active: false,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        RenderTarget::from(visibility),
        RenderLayers::layer(RAIL_GLASS_OCCLUSION_RENDER_LAYER),
        Transform::default(),
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
}

fn physical_window_size(window: &Window) -> (u32, u32) {
    (
        window.physical_width().max(1),
        window.physical_height().max(1),
    )
}

fn sync_rail_glass_mask_camera(
    windows: Query<&Window, With<PrimaryWindow>>,
    targets: Option<Res<RailGlassPostProcessAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: ParamSet<(
        Query<(&Transform, &Projection), With<crate::render::PrimaryGameCamera>>,
        Query<(&mut Camera, &mut Transform, &mut Projection), With<RailGlassMaskCamera>>,
        Query<(&mut Camera, &mut Transform, &mut Projection), With<RailGlassOcclusionCamera>>,
    )>,
) {
    let Ok((primary_transform, primary_projection)) = cameras
        .p0()
        .single()
        .map(|(transform, projection)| (*transform, projection.clone()))
    else {
        return;
    };

    let Some(targets) = targets else {
        if let Ok((mut mask_camera, ..)) = cameras.p1().single_mut() {
            mask_camera.is_active = false;
        }
        if let Ok((mut occlusion_camera, ..)) = cameras.p2().single_mut() {
            occlusion_camera.is_active = false;
        }
        return;
    };
    if let Some(window) = windows.iter().next() {
        let (width, height) = physical_window_size(window);
        if let Some(mut mask) = images.get_mut(&targets.mask)
            && (mask.width(), mask.height()) != (width, height)
        {
            *mask = Image::new_target_texture(width, height, TextureFormat::Rgba8Unorm, None);
        }
        if let Some(mut visibility) = images.get_mut(&targets.visibility)
            && (visibility.width(), visibility.height()) != (width, height)
        {
            *visibility = Image::new_target_texture(width, height, TextureFormat::Rgba8Unorm, None);
        }
    }

    {
        let mut mask_query = cameras.p1();
        let Ok((mut mask_camera, mut mask_transform, mut mask_projection)) =
            mask_query.single_mut()
        else {
            return;
        };
        mask_camera.is_active = true;
        *mask_transform = primary_transform;
        *mask_projection = primary_projection.clone();
    }
    {
        let mut occlusion_query = cameras.p2();
        let Ok((mut occlusion_camera, mut occlusion_transform, mut occlusion_projection)) =
            occlusion_query.single_mut()
        else {
            return;
        };
        occlusion_camera.is_active = true;
        *occlusion_transform = primary_transform;
        *occlusion_projection = primary_projection;
    }
}

#[derive(Resource)]
struct RailGlassPostProcessPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    variants: Variants<RenderPipeline, RailGlassPipelineSpecializer>,
}

struct RailGlassPipelineSpecializer;

#[derive(PartialEq, Eq, Hash, Clone, Copy, SpecializerKey)]
struct RailGlassPipelineKey {
    target_format: TextureFormat,
}

impl Specializer<RenderPipeline> for RailGlassPipelineSpecializer {
    type Key = RailGlassPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        descriptor: &mut RenderPipelineDescriptor,
    ) -> Result<Canonical<Self::Key>, BevyError> {
        let fragment = descriptor.fragment_mut()?;
        fragment.set_target(
            0,
            ColorTargetState {
                format: key.target_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
        );
        Ok(key)
    }
}

fn init_rail_glass_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "rail_glass_post_process_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                sampler(SamplerBindingType::NonFiltering),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        mipmap_filter: MipmapFilterMode::Nearest,
        ..default()
    });
    let shader = asset_server.load::<Shader>(RAIL_GLASS_SHADER_PATH);
    let descriptor = RenderPipelineDescriptor {
        label: Some("rail_glass_post_process_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    commands.insert_resource(RailGlassPostProcessPipeline {
        layout,
        sampler,
        variants: Variants::new(RailGlassPipelineSpecializer, descriptor),
    });
}

#[derive(Component)]
struct RailGlassPostProcessPipelineId(CachedRenderPipelineId);

fn prepare_rail_glass_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipeline: ResMut<RailGlassPostProcessPipeline>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            Option<&RailGlassPostProcessSettings>,
        ),
        With<ExtractedCamera>,
    >,
) -> Result<(), BevyError> {
    for (entity, view, settings) in &views {
        if settings.is_none() {
            continue;
        }
        let pipeline_id = pipeline.variants.specialize(
            &pipeline_cache,
            RailGlassPipelineKey {
                target_format: view.target_format,
            },
        )?;
        commands
            .entity(entity)
            .insert(RailGlassPostProcessPipelineId(pipeline_id));
    }
    Ok(())
}

#[derive(Default)]
struct RailGlassBindGroupCache {
    key: Option<(TextureViewId, TextureViewId, TextureViewId, TextureViewId)>,
    bind_group: Option<BindGroup>,
}

#[allow(clippy::too_many_arguments)] // sistema de render ECS: bind group y pass requieren recursos separados
fn apply_rail_glass_post_process(
    view: ViewQuery<
        (&ViewTarget, &RailGlassPostProcessPipelineId),
        With<RailGlassPostProcessSettings>,
    >,
    assets: Option<Res<RailGlassPostProcessAssets>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    pipeline: Option<Res<RailGlassPostProcessPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    mut cache: Local<RailGlassBindGroupCache>,
    mut ctx: RenderContext,
) {
    let Some(assets) = assets else {
        return;
    };
    let Some(pipeline_resource) = pipeline else {
        return;
    };
    let (view_target, pipeline_id) = view.into_inner();
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else {
        return;
    };
    let Some(mask) = gpu_images.get(&assets.mask) else {
        return;
    };
    let Some(visibility) = gpu_images.get(&assets.visibility) else {
        return;
    };
    let Some(lut) = gpu_images.get(&assets.lut) else {
        return;
    };

    let post_process = view_target.post_process_write();
    let key = (
        post_process.source.id(),
        mask.texture_view.id(),
        visibility.texture_view.id(),
        lut.texture_view.id(),
    );
    if cache.key != Some(key) {
        cache.bind_group = Some(render_device.create_bind_group(
            "rail_glass_post_process_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline_resource.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline_resource.sampler,
                &mask.texture_view,
                &visibility.texture_view,
                &lut.texture_view,
            )),
        ));
        cache.key = Some(key);
    }
    let Some(bind_group) = cache.bind_group.as_ref() else {
        return;
    };

    let pass_descriptor = RenderPassDescriptor {
        label: Some("rail_glass_post_process_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post_process.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::default(),
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut render_pass = ctx.command_encoder().begin_render_pass(&pass_descriptor);
    render_pass.set_pipeline(render_pipeline);
    render_pass.set_bind_group(0, bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use super::*;

    #[derive(Component)]
    struct SourceId(u32);

    #[derive(Debug, PartialEq)]
    struct MaskState {
        sprite: SpriteMesh,
        anchor: Anchor,
        transform: Transform,
        visibility: Visibility,
        chunk: Option<MapTileChunk>,
        layers: RenderLayers,
    }

    fn spawn_source(world: &mut World, id: u32, image: &Handle<Image>) {
        let mut source = world.spawn((
            SourceId(id),
            MapVisualLayer,
            Sprite::from_image(image.clone()),
            Anchor::CENTER,
            Transform::from_xyz(id as f32, 7.0, 3.0),
            Visibility::Inherited,
        ));
        if id.is_multiple_of(2) {
            source.insert(MapTileChunk { cx: id / 2, cy: 3 });
        }
        if id.is_multiple_of(3) {
            source.insert(RailGlassMaskSource);
        }
        if id.is_multiple_of(4) {
            source.insert(MapDynamicVisual);
        }
    }

    fn mutate_sources(world: &mut World, phase: u32, images: &[Handle<Image>; 2]) {
        let sources: Vec<_> = world
            .query::<(Entity, &SourceId)>()
            .iter(world)
            .map(|(entity, id)| (entity, id.0))
            .collect();
        for (entity, id) in sources {
            match phase % 16 {
                0 => {
                    let mut sprite = world.get_mut::<Sprite>(entity).unwrap();
                    sprite.flip_x = !sprite.flip_x;
                    sprite.flip_y = id.is_multiple_of(2);
                    sprite.color = Color::srgba(0.2, 0.3, 0.4, 0.5);
                }
                1 => {
                    let mut sprite = world.get_mut::<Sprite>(entity).unwrap();
                    sprite.custom_size = Some(Vec2::new(10.0 + id as f32, 13.0));
                    sprite.rect = Some(Rect::new(1.0, 2.0, 8.0, 9.0));
                }
                2 => {
                    world.entity_mut(entity).insert((
                        Anchor(Vec2::new(0.3, -0.2)),
                        Transform::from_xyz(id as f32 + phase as f32, 11.0, -2.0),
                    ));
                }
                3 => {
                    if world.get::<RailGlassMaskSource>(entity).is_some() {
                        world.entity_mut(entity).remove::<RailGlassMaskSource>();
                    } else {
                        world.entity_mut(entity).insert(RailGlassMaskSource);
                    }
                }
                4 => {
                    world.entity_mut(entity).insert(if id.is_multiple_of(2) {
                        Visibility::Hidden
                    } else {
                        Visibility::Visible
                    });
                }
                5 => {
                    world.entity_mut(entity).remove::<MapTileChunk>();
                }
                6 => {
                    world
                        .entity_mut(entity)
                        .insert(MapTileChunk { cx: phase, cy: id });
                }
                7 => {
                    let mut sprite = world.get_mut::<Sprite>(entity).unwrap();
                    sprite.image = images[1].clone();
                    sprite.texture_atlas = Some(TextureAtlas {
                        layout: Handle::default(),
                        index: id as usize,
                    });
                }
                8 => {
                    world.get_mut::<Sprite>(entity).unwrap().image_mode = SpriteImageMode::Tiled {
                        tile_x: true,
                        tile_y: id.is_multiple_of(2),
                        stretch_value: 1.5,
                    };
                }
                9 => {
                    world.entity_mut(entity).remove::<MapVisualLayer>();
                }
                10 => {
                    world.entity_mut(entity).insert(MapVisualLayer);
                }
                11 => {
                    let proxy = world
                        .query::<(Entity, &RailGlassMaskProxy)>()
                        .iter(world)
                        .find(|(_, proxy)| proxy.source == entity)
                        .map(|(proxy_entity, _)| proxy_entity);
                    if let Some(proxy) = proxy {
                        world.despawn(proxy);
                    }
                }
                12 => {
                    world.entity_mut(entity).remove::<Sprite>();
                }
                13 => {
                    world
                        .entity_mut(entity)
                        .insert(Sprite::from_image(images[0].clone()));
                }
                14 => {
                    world.despawn(entity);
                    spawn_source(world, id, &images[0]);
                }
                _ => {}
            }
        }
    }

    fn mask_states(world: &mut World) -> BTreeMap<u32, MaskState> {
        let mut query = world.query::<(
            &RailGlassMaskProxy,
            &SpriteMesh,
            &Anchor,
            &Transform,
            &Visibility,
            Option<&MapTileChunk>,
            &RenderLayers,
        )>();
        let mut result = BTreeMap::new();
        for (proxy, sprite, anchor, transform, visibility, chunk, layers) in query.iter(world) {
            let id = world
                .get::<SourceId>(proxy.source)
                .expect("no mask may outlive its source")
                .0;
            assert!(
                result
                    .insert(
                        id,
                        MaskState {
                            sprite: sprite.clone(),
                            anchor: *anchor,
                            transform: *transform,
                            visibility: *visibility,
                            chunk: chunk.copied(),
                            layers: layers.clone(),
                        }
                    )
                    .is_none(),
                "one mask per source"
            );
        }
        result
    }

    #[test]
    fn persistent_masks_match_legacy_across_source_and_proxy_lifecycles() {
        let mut assets = Assets::<Image>::default();
        let images = [assets.add(Image::default()), assets.add(Image::default())];
        let mut legacy = App::new();
        legacy.add_systems(Update, sync_legacy);
        let mut cached = App::new();
        cached.add_systems(Update, sync_rail_glass_mask_proxies);
        for app in [&mut legacy, &mut cached] {
            for id in 0..64 {
                spawn_source(app.world_mut(), id, &images[0]);
            }
            app.update();
        }
        assert_eq!(
            mask_states(legacy.world_mut()),
            mask_states(cached.world_mut())
        );
        for phase in 0..48 {
            for app in [&mut legacy, &mut cached] {
                mutate_sources(app.world_mut(), phase, &images);
                app.update();
            }
            assert_eq!(
                mask_states(legacy.world_mut()),
                mask_states(cached.world_mut()),
                "phase {phase}"
            );
        }
        assert_eq!(mask_states(cached.world_mut()).len(), 64);
    }

    #[test]
    fn cached_masks_repair_external_edits_without_source_changes() {
        let mut app = App::new();
        app.add_systems(Update, sync_rail_glass_mask_proxies);
        spawn_source(app.world_mut(), 0, &Handle::default());
        let source = app
            .world_mut()
            .query_filtered::<Entity, With<SourceId>>()
            .single(app.world())
            .unwrap();
        // Seed an existing mask to cover initialization as well as recovery.
        app.world_mut().spawn((
            RailGlassMaskProxy { source },
            MapVisualLayer,
            SpriteMesh::default(),
            MapTileChunk { cx: 0, cy: 3 },
            RenderLayers::layer(RAIL_GLASS_OCCLUSION_RENDER_LAYER),
        ));
        app.update();
        let expected = mask_states(app.world_mut());
        let proxy = app
            .world_mut()
            .query_filtered::<Entity, With<RailGlassMaskProxy>>()
            .single(app.world())
            .unwrap();
        {
            let mut mask = app.world_mut().get_mut::<SpriteMesh>(proxy).unwrap();
            mask.color = Color::WHITE;
            mask.alpha_mode = SpriteAlphaMode::Blend;
            mask.flip_x = true;
        }
        app.world_mut().entity_mut(proxy).insert((
            Anchor::TOP_LEFT,
            Transform::from_xyz(999.0, 888.0, 777.0),
            Visibility::Hidden,
            MapTileChunk { cx: 999, cy: 888 },
        ));
        app.update();
        assert_eq!(mask_states(app.world_mut()), expected);
        // An unchanged frame must not mark the mesh/pose changed for extraction.
        app.update();
        let count = app
            .world_mut()
            .query_filtered::<Entity, (
                With<RailGlassMaskProxy>,
                Or<(
                    Changed<SpriteMesh>,
                    Changed<Anchor>,
                    Changed<Transform>,
                    Changed<Visibility>,
                )>,
            )>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    fn sync_legacy(
        mut commands: Commands,
        sources: Query<
            (
                Entity,
                &Sprite,
                &Anchor,
                &Transform,
                &Visibility,
                Option<&MapTileChunk>,
                Option<&RailGlassMaskSource>,
            ),
            (
                Or<(With<MapVisualLayer>, With<MapDynamicVisual>)>,
                Without<RailGlassMaskProxy>,
            ),
        >,
        mut proxies: Query<(
            Entity,
            &RailGlassMaskProxy,
            &mut SpriteMesh,
            &mut Anchor,
            &mut Transform,
            &mut Visibility,
            Option<&mut MapTileChunk>,
        )>,
    ) {
        let _measurement = crate::performance::measure(crate::performance::Phase::Glass);
        let mut proxies_by_source = HashMap::with_capacity(proxies.iter().len());
        for (proxy_entity, proxy, _sprite, _anchor, _transform, _visibility, _chunk) in &mut proxies
        {
            proxies_by_source.insert(proxy.source, proxy_entity);
        }

        let mut live_sources = HashMap::with_capacity(sources.iter().len());
        for (
            source_entity,
            source_sprite,
            source_anchor,
            source_transform,
            source_visibility,
            source_chunk,
            glass_source,
        ) in &sources
        {
            live_sources.insert(source_entity, ());
            let mask_sprite = mask_sprite_from_source(source_sprite, glass_source.is_some());

            if let Some(&proxy_entity) = proxies_by_source.get(&source_entity) {
                let Ok((
                    _,
                    _,
                    mut proxy_sprite,
                    mut proxy_anchor,
                    mut proxy_transform,
                    mut proxy_visibility,
                    proxy_chunk,
                )) = proxies.get_mut(proxy_entity)
                else {
                    continue;
                };
                if *proxy_sprite != mask_sprite {
                    *proxy_sprite = mask_sprite;
                }
                if *proxy_anchor != *source_anchor {
                    *proxy_anchor = *source_anchor;
                }
                if *proxy_transform != *source_transform {
                    *proxy_transform = *source_transform;
                }
                if *proxy_visibility != *source_visibility {
                    *proxy_visibility = *source_visibility;
                }
                if let (Some(source_chunk), Some(mut proxy_chunk)) = (source_chunk, proxy_chunk)
                    && *proxy_chunk != *source_chunk
                {
                    *proxy_chunk = *source_chunk;
                }
                continue;
            }

            let proxy_entity = commands
                .spawn((
                    RailGlassMaskProxy {
                        source: source_entity,
                    },
                    MapVisualLayer,
                    mask_sprite,
                    *source_anchor,
                    *source_transform,
                    *source_visibility,
                    RenderLayers::layer(RAIL_GLASS_OCCLUSION_RENDER_LAYER),
                ))
                .id();
            if let Some(source_chunk) = source_chunk {
                commands.entity(proxy_entity).insert(*source_chunk);
            }
        }

        for (source_entity, proxy_entity) in proxies_by_source {
            if !live_sources.contains_key(&source_entity) {
                commands.entity(proxy_entity).despawn();
            }
        }
    }
}
