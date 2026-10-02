//! Overlays animados de aeropuerto (`SPR_AIRPORT_RADAR_*` y manga de viento).
//!
//! La simulación avanza `m7` en `step_airport_tiles`; el cliente lee el frame
//! vivo del mapa cada tick visual. Los frames cambian tanto de PNG como de
//! ancla NFO, por lo que deben actualizar también su parent global.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    airport_radar_frame, is_airport_flag_station_gfx, is_airport_radar_station_gfx,
    is_airport_tower_tile,
};

use crate::bevy_app::UpdateSet;
use crate::iso::overlay_pos;
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    TileRenderContext, ViewportSortableParent, WorldAssets, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{
    airport_station_layers_for_gfx, airport_station_overlay_rel_for_sprite,
    airport_station_sprite_for_id,
};
use crate::state::{ClientScreen, SimWorld};

const FIRST_AIRPORT_RADAR_SPRITE_ID: u32 = 2_680;
const FIRST_AIRPORT_WIND_SPRITE_ID: u32 = 2_676;

pub(crate) struct AirportStationAnimPlugin;

impl Plugin for AirportStationAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_airport_station_overlays
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Origen de la capa animada: la ruta simplificada antigua del radar o un
/// `StationGfx` importado, que conserva el `TILE_SEQ_LINE` real.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AirportStationAnimSource {
    LegacyTower,
    StationGfx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AirportStationAnimKind {
    Radar,
    Wind,
}

/// Overlay animado de una estación airport. Conserva el contexto de dibujo
/// porque sus frames pueden no compartir ancla con el frame 0 y porque la Z
/// del parent pertenece al compositor, no al `Transform` ya reordenado.
#[derive(Component, Clone, Copy)]
pub(crate) struct AirportStationAnim {
    pos: TileCoord,
    source: AirportStationAnimSource,
    kind: AirportStationAnimKind,
    iso_pos: Vec2,
    base_z: u8,
    tx: i32,
    ty: i32,
    map_width: u32,
    local_ordinal: u8,
}

/// Estado puro de un frame: permite que el spawner y la animación compartan
/// exactamente la misma geometría `TILE_SEQ_LINE`.
#[derive(Clone, Copy)]
pub(crate) struct AirportStationAnimFrame {
    pub(crate) sprite_id: u32,
    pub(crate) translation: Vec3,
    pub(crate) parent: ViewportSortableParent,
}

impl AirportStationAnim {
    fn new(
        ctx: &TileRenderContext,
        base_z: u8,
        map_width: u32,
        source: AirportStationAnimSource,
        kind: AirportStationAnimKind,
        local_ordinal: u8,
    ) -> Self {
        Self {
            pos: ctx.coord,
            source,
            kind,
            iso_pos: ctx.iso_pos,
            base_z,
            tx: ctx.tx_i32(),
            ty: ctx.ty_i32(),
            map_width,
            local_ordinal,
        }
    }

    /// Radar de la ruta de aeropuerto procedural anterior a `StationGfx`.
    #[must_use]
    pub(crate) fn legacy_radar_tower(ctx: &TileRenderContext, base_z: u8, map_width: u32) -> Self {
        Self::new(
            ctx,
            base_z,
            map_width,
            AirportStationAnimSource::LegacyTower,
            AirportStationAnimKind::Radar,
            0,
        )
    }

    /// Radar dentro de la secuencia importada; `local_ordinal` conserva el
    /// orden nativo respecto de las cercas que siguen en la misma tesela.
    #[must_use]
    pub(crate) fn station_radar(
        ctx: &TileRenderContext,
        base_z: u8,
        map_width: u32,
        local_ordinal: u8,
    ) -> Self {
        Self::new(
            ctx,
            base_z,
            map_width,
            AirportStationAnimSource::StationGfx,
            AirportStationAnimKind::Radar,
            local_ordinal,
        )
    }

    /// Manga dentro de la secuencia importada; `local_ordinal` conserva el
    /// orden nativo respecto de la cerca que la precede en la misma tesela.
    #[must_use]
    pub(crate) fn station_wind(
        ctx: &TileRenderContext,
        base_z: u8,
        map_width: u32,
        local_ordinal: u8,
    ) -> Self {
        Self::new(
            ctx,
            base_z,
            map_width,
            AirportStationAnimSource::StationGfx,
            AirportStationAnimKind::Wind,
            local_ordinal,
        )
    }

    fn is_still_animated(&self, tile: &Tile) -> bool {
        match (self.source, self.kind) {
            (AirportStationAnimSource::LegacyTower, AirportStationAnimKind::Radar) => {
                is_airport_tower_tile(tile.kind, tile.m5)
            }
            (AirportStationAnimSource::StationGfx, AirportStationAnimKind::Radar) => {
                is_airport_radar_station_gfx(tile.m5)
            }
            (AirportStationAnimSource::StationGfx, AirportStationAnimKind::Wind) => {
                is_airport_flag_station_gfx(tile.m5)
            }
            (AirportStationAnimSource::LegacyTower, AirportStationAnimKind::Wind) => false,
        }
    }

    const fn first_sprite_id(self) -> u32 {
        match self.kind {
            AirportStationAnimKind::Radar => FIRST_AIRPORT_RADAR_SPRITE_ID,
            AirportStationAnimKind::Wind => FIRST_AIRPORT_WIND_SPRITE_ID,
        }
    }

    fn sprite_id_for_m7(self, m7: u8) -> u32 {
        match self.kind {
            AirportStationAnimKind::Radar => {
                FIRST_AIRPORT_RADAR_SPRITE_ID + u32::from(airport_radar_frame(m7))
            }
            AirportStationAnimKind::Wind => FIRST_AIRPORT_WIND_SPRITE_ID + u32::from(m7 % 4),
        }
    }

    fn layer_for_station_gfx(
        &self,
        station_gfx: u8,
    ) -> Option<&'static crate::sprites::AirportStationLayer> {
        // La ruta legacy usa el mismo `TILE_SEQ_LINE` que APT_RADAR_GRASS_FENCE_SW.
        let gfx = match self.source {
            AirportStationAnimSource::LegacyTower => 31,
            AirportStationAnimSource::StationGfx => station_gfx,
        };
        airport_station_layers_for_gfx(gfx)
            .iter()
            .find(|layer| layer.sprite_id == self.first_sprite_id())
    }

    /// Construye el sprite, ancla y prisma de un frame. La caja `TILE_SEQ`
    /// se conserva entre frames, aunque cambien el PNG o su ancla NFO.
    #[must_use]
    pub(crate) fn frame_for_m7(&self, m7: u8, station_gfx: u8) -> Option<AirportStationAnimFrame> {
        let layer = self.layer_for_station_gfx(station_gfx)?;
        let sprite_id = self.sprite_id_for_m7(m7);
        let sprite = airport_station_sprite_for_id(sprite_id)?;
        let (xrel, yrel) = airport_station_overlay_rel_for_sprite(layer, sprite);
        let source_translation = overlay_pos(
            self.iso_pos,
            xrel,
            yrel,
            sprite.w,
            sprite.h,
            self.base_z,
            layer.z,
            self.tx,
            self.ty,
        );
        let source_x = u32::try_from(self.tx).unwrap_or(0);
        let source_y = u32::try_from(self.ty).unwrap_or(0);
        let source_depth = viewport_source_depth(source_translation.z, source_x, self.map_width);
        let x = self.tx * 16 + layer.dx as i32;
        let y = self.ty * 16 + layer.dy as i32;
        let z = i32::from(self.base_z) * 8 + layer.dz as i32;
        let parent = ViewportSortableParent {
            sprite_id,
            bounds: ParentSpriteBounds::new(
                x,
                y,
                z,
                x + layer.sx - 1,
                y + layer.sy - 1,
                z + layer.sz - 1,
            ),
            insertion_key: viewport_insertion_key(source_x, source_y, self.local_ordinal),
            source_depth,
        };
        Some(AirportStationAnimFrame {
            sprite_id,
            translation: Vec3::new(
                source_translation.x,
                source_translation.y,
                parent.source_depth,
            ),
            parent,
        })
    }
}

/// La Z efectiva pertenece al sorter global; la animación no puede devolver
/// una capa a su depth fuente entre dos pases de composición.
fn set_airport_station_translation_if_changed(
    transform: &mut Mut<Transform>,
    source_translation: Vec3,
    preserves_sorted_depth: bool,
) {
    let translation = if preserves_sorted_depth {
        Vec3::new(
            source_translation.x,
            source_translation.y,
            transform.translation.z,
        )
    } else {
        source_translation
    };
    if transform.translation != translation {
        transform.translation = translation;
    }
}

fn animate_airport_station_overlays(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &AirportStationAnim,
        &mut Sprite,
        &mut Transform,
        &mut Visibility,
        Option<&mut ViewportSortableParent>,
    )>,
) {
    let Some(assets) = assets else {
        return;
    };
    for (entity, anim, mut sprite, mut transform, mut visibility, sortable_parent) in &mut q {
        let Some(tile) = sim.state.map.get(anim.pos) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        if !anim.is_still_animated(&tile) {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        }
        let Some(frame) = anim.frame_for_m7(tile.m7, tile.m5) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        let Some(frame_sprite) = assets.airport_station_sprite(frame.sprite_id) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        if !frame_sprite.matches(&sprite) {
            frame_sprite.apply_to(&mut sprite);
        }
        let preserves_sorted_depth = sortable_parent.is_some();
        set_airport_station_translation_if_changed(
            &mut transform,
            frame.translation,
            preserves_sorted_depth,
        );
        if let Some(mut parent) = sortable_parent {
            parent.set_if_neq(frame.parent);
        } else {
            commands.entity(entity).insert(frame.parent);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::ecs::schedule::Schedule;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;

    use super::*;
    use crate::render::grid::TileRenderInfo;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn airport_ctx() -> TileRenderContext {
        TileRenderContext {
            tx: 186,
            ty: 1,
            coord: TileCoord::new(186, 1),
            tile: None,
            object_type: None,
            kind: TileKind::Airport,
            info: TileRenderInfo {
                tileh: 0,
                base_z: 1,
                use_shore: false,
            },
            iso_pos: Vec2::ZERO,
            climate: openttdrs_core::Climate::Temperate,
            snow_line_height: openttdrs_core::DEF_SNOW_LINE_HEIGHT,
        }
    }

    #[test]
    fn station_radar_uses_native_prism_and_frame_specific_anchor() {
        let anim = AirportStationAnim::station_radar(&airport_ctx(), 1, 256, 0);
        let first = anim.frame_for_m7(0, 31).expect("frame inicial");
        let fourth = anim.frame_for_m7(3, 31).expect("frame rotado");

        assert_eq!(first.sprite_id, 2_680);
        assert_eq!(fourth.sprite_id, 2_683);
        assert_eq!(
            first.parent.bounds,
            ParentSpriteBounds::new(2983, 23, 8, 2984, 24, 15)
        );
        assert_eq!(first.parent.bounds, fourth.parent.bounds);
        assert_ne!(
            first.translation.truncate(),
            fourth.translation.truncate(),
            "los frames de radar tienen offsets NFO distintos"
        );
        assert_eq!(
            first.parent.insertion_key,
            viewport_insertion_key(186, 1, 0)
        );
    }

    #[test]
    fn legacy_tower_reuses_the_same_native_radar_contract() {
        let legacy = AirportStationAnim::legacy_radar_tower(&airport_ctx(), 1, 256);
        let frame = legacy.frame_for_m7(11, 6).expect("frame de tower legacy");
        assert_eq!(frame.sprite_id, 2_691);
        assert_eq!(
            frame.parent.bounds,
            ParentSpriteBounds::new(2983, 23, 8, 2984, 24, 15)
        );
    }

    #[test]
    fn live_airport_frames_update_sprite_and_anchor_without_replacing_entities() {
        let dir = tempfile::tempdir().expect("asset fixture");
        crate::render::assets::stub_opengfx_tiles_for_tests(dir.path());
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin {
            file_path: dir.path().to_str().expect("asset path").into(),
            ..default()
        });
        app.add_plugins(ImagePlugin::default());
        app.init_asset::<TextureAtlasLayout>();
        app.update();
        let world = app.world_mut();
        let atlas = world.resource_scope(|world, mut layouts: Mut<Assets<TextureAtlasLayout>>| {
            crate::render::TileAtlas::build(world.resource::<AssetServer>(), &mut layouts)
        });
        let assets = WorldAssets::load(&atlas, &mut world.resource_mut::<Assets<Image>>());
        world.insert_resource(assets.clone());
        let ctx = airport_ctx();
        let mut sim = SimWorld::default();
        sim.state.map = Map::new_flat(256, 256, 1);
        world.insert_resource(sim);
        for (gfx, anim, first, frames) in [
            (
                31,
                AirportStationAnim::station_radar(&ctx, 1, 256, 0),
                2680,
                12,
            ),
            (
                39,
                AirportStationAnim::station_wind(&ctx, 1, 256, 1),
                2676,
                4,
            ),
            (
                6,
                AirportStationAnim::legacy_radar_tower(&ctx, 1, 256),
                2680,
                12,
            ),
        ] {
            let frame = anim.frame_for_m7(0, gfx).expect("first frame");
            let entity = world
                .spawn((
                    anim,
                    assets
                        .airport_station_sprite(first)
                        .expect("first sprite")
                        .sprite(),
                    Transform::from_xyz(frame.translation.x, frame.translation.y, 42.0),
                    Visibility::Inherited,
                    frame.parent,
                ))
                .id();
            for m7 in 0..12 {
                let mut tile = world
                    .resource::<SimWorld>()
                    .state
                    .map
                    .get(ctx.coord)
                    .unwrap();
                tile.kind = TileKind::Airport;
                tile.m5 = gfx;
                tile.m7 = m7;
                world
                    .resource_mut::<SimWorld>()
                    .state
                    .map
                    .set_tile(ctx.coord, tile)
                    .unwrap();
                world
                    .run_system_once(animate_airport_station_overlays)
                    .expect("retained frame update");
                let expected = first + u32::from(m7 % frames);
                let current = world.entity(entity);
                assert_eq!(
                    current.get::<ViewportSortableParent>().unwrap().sprite_id,
                    expected
                );
                assert!(
                    assets
                        .airport_station_sprite(expected)
                        .unwrap()
                        .matches(current.get::<Sprite>().unwrap())
                );
                let expected_frame = anim.frame_for_m7(m7, gfx).unwrap();
                let transform = current.get::<Transform>().unwrap();
                assert_eq!(
                    transform.translation.truncate(),
                    expected_frame.translation.truncate()
                );
                assert_eq!(
                    transform.translation.z, 42.0,
                    "the global sorter's depth survives animation"
                );
                assert_eq!(*current.get::<Visibility>().unwrap(), Visibility::Visible);
            }
            world.entity_mut(entity).despawn();
        }
    }

    #[test]
    fn station_wind_uses_native_prism_and_frame_specific_anchor() {
        let anim = AirportStationAnim::station_wind(&airport_ctx(), 1, 256, 1);
        let first = anim.frame_for_m7(0, 39).expect("frame inicial");
        let fourth = anim.frame_for_m7(3, 39).expect("frame rotado");

        assert_eq!(first.sprite_id, 2_676);
        assert_eq!(fourth.sprite_id, 2_679);
        assert_eq!(
            first.parent.bounds,
            ParentSpriteBounds::new(2980, 27, 8, 2980, 27, 27)
        );
        assert_eq!(first.parent.bounds, fourth.parent.bounds);
        assert_ne!(
            first.translation.truncate(),
            fourth.translation.truncate(),
            "los frames de manga tienen offsets NFO distintos"
        );
        assert_eq!(
            first.parent.insertion_key,
            viewport_insertion_key(186, 1, 1)
        );
    }

    #[test]
    fn airport_radar_enters_global_sorter_and_keeps_its_resolved_depth() {
        let anim = AirportStationAnim::station_radar(&airport_ctx(), 1, 256, 0);
        let frame = anim.frame_for_m7(0, 31).expect("frame radar");
        let parent = frame.parent;
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let radar = world
            .spawn((parent, Transform::from_translation(frame.translation)))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 9_998,
                bounds: ParentSpriteBounds::new(
                    parent.bounds.xmin.saturating_sub(1),
                    parent.bounds.ymin,
                    parent.bounds.zmin,
                    parent.bounds.xmin,
                    parent.bounds.ymax,
                    parent.bounds.zmax,
                ),
                insertion_key: parent.insertion_key + 1,
                source_depth: parent.source_depth + 0.000_5,
            },
            Transform::from_xyz(0.0, 0.0, parent.source_depth + 0.000_5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);
        let sorted_depth = world
            .entity(radar)
            .get::<Transform>()
            .expect("transform radar")
            .translation
            .z;
        assert!(
            sorted_depth > parent.source_depth,
            "el radar debe recibir la profundidad del compositor global"
        );

        let mut transform = world
            .get_mut::<Transform>(radar)
            .expect("transform para rotación");
        set_airport_station_translation_if_changed(&mut transform, frame.translation, true);
        assert_eq!(transform.translation.z, sorted_depth);
    }
}
