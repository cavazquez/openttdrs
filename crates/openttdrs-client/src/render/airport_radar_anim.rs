//! Radar animado de aeropuerto (`SPR_AIRPORT_RADAR_*`).
//!
//! La simulación avanza `m7` en `step_airport_tiles`; el cliente lee el frame
//! vivo del mapa cada tick visual. Los doce frames cambian tanto de PNG como
//! de ancla NFO, por lo que deben actualizar también su parent global.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{airport_radar_frame, is_airport_radar_station_gfx, is_airport_tower_tile};

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

pub(crate) struct AirportRadarAnimPlugin;

impl Plugin for AirportRadarAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_airport_radar
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Origen de la capa radar: la ruta simplificada antigua o un `StationGfx`
/// importado, donde las entradas 31/51/52 contienen el `TILE_SEQ_LINE` real.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AirportRadarSource {
    LegacyTower,
    StationGfx,
}

/// Overlay del radar. Conserva el contexto de dibujo porque las aspas no
/// comparten ancla con el frame 0 y porque la Z del parent pertenece al
/// compositor, no al `Transform` ya reordenado.
#[derive(Component, Clone, Copy)]
pub(crate) struct AirportRadarAnim {
    pos: TileCoord,
    source: AirportRadarSource,
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
pub(crate) struct AirportRadarFrame {
    pub(crate) sprite_id: u32,
    pub(crate) translation: Vec3,
    pub(crate) parent: ViewportSortableParent,
}

impl AirportRadarAnim {
    fn new(
        ctx: &TileRenderContext,
        base_z: u8,
        map_width: u32,
        source: AirportRadarSource,
        local_ordinal: u8,
    ) -> Self {
        Self {
            pos: ctx.coord,
            source,
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
    pub(crate) fn legacy_tower(ctx: &TileRenderContext, base_z: u8, map_width: u32) -> Self {
        Self::new(ctx, base_z, map_width, AirportRadarSource::LegacyTower, 0)
    }

    /// Radar dentro de la secuencia importada; `local_ordinal` conserva el
    /// orden nativo respecto de las cercas que siguen en la misma tesela.
    #[must_use]
    pub(crate) fn station_gfx(
        ctx: &TileRenderContext,
        base_z: u8,
        map_width: u32,
        local_ordinal: u8,
    ) -> Self {
        Self::new(
            ctx,
            base_z,
            map_width,
            AirportRadarSource::StationGfx,
            local_ordinal,
        )
    }

    fn is_still_radar(&self, tile: &Tile) -> bool {
        match self.source {
            AirportRadarSource::LegacyTower => is_airport_tower_tile(tile.kind, tile.m5),
            AirportRadarSource::StationGfx => is_airport_radar_station_gfx(tile.m5),
        }
    }

    fn layer_for_station_gfx(
        &self,
        station_gfx: u8,
    ) -> Option<&'static crate::sprites::AirportStationLayer> {
        // La ruta legacy usa el mismo `TILE_SEQ_LINE` que APT_RADAR_GRASS_FENCE_SW.
        let gfx = match self.source {
            AirportRadarSource::LegacyTower => 31,
            AirportRadarSource::StationGfx => station_gfx,
        };
        airport_station_layers_for_gfx(gfx)
            .iter()
            .find(|layer| layer.sprite_id == FIRST_AIRPORT_RADAR_SPRITE_ID)
    }

    /// Construye el sprite, ancla y prisma de un frame de radar. Cada frame
    /// conserva la misma caja `M(7,7,0,2,2,8)`, pero no el mismo PNG ni el
    /// mismo desplazamiento de pantalla.
    #[must_use]
    pub(crate) fn frame_for_m7(&self, m7: u8, station_gfx: u8) -> Option<AirportRadarFrame> {
        let layer = self.layer_for_station_gfx(station_gfx)?;
        let sprite_id = FIRST_AIRPORT_RADAR_SPRITE_ID + u32::from(airport_radar_frame(m7));
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
        Some(AirportRadarFrame {
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

/// La Z efectiva pertenece al sorter global; la rotación no puede devolver
/// el radar a su depth fuente entre dos pases de composición.
fn set_airport_radar_translation_if_changed(
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

fn animate_airport_radar(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &AirportRadarAnim,
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
        if !anim.is_still_radar(&tile) {
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
        set_airport_radar_translation_if_changed(
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
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::schedule::Schedule;

    use super::*;
    use crate::render::grid::TileRenderInfo;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn radar_ctx() -> TileRenderContext {
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
        }
    }

    #[test]
    fn station_radar_uses_native_prism_and_frame_specific_anchor() {
        let anim = AirportRadarAnim::station_gfx(&radar_ctx(), 1, 256, 0);
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
        let legacy = AirportRadarAnim::legacy_tower(&radar_ctx(), 1, 256);
        let frame = legacy.frame_for_m7(11, 6).expect("frame de tower legacy");
        assert_eq!(frame.sprite_id, 2_691);
        assert_eq!(
            frame.parent.bounds,
            ParentSpriteBounds::new(2983, 23, 8, 2984, 24, 15)
        );
    }

    #[test]
    fn airport_radar_enters_global_sorter_and_keeps_its_resolved_depth() {
        let anim = AirportRadarAnim::station_gfx(&radar_ctx(), 1, 256, 0);
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
        set_airport_radar_translation_if_changed(&mut transform, frame.translation, true);
        assert_eq!(transform.translation.z, sorted_depth);
    }
}
