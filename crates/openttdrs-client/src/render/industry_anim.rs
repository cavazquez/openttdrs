//! Animación de capas estáticas de industria con `anim_state` (torres, pozos).
//!
//! OpenTTD indexa `_industry_draw_tile_data` con `m4 & 3`. P7 avanza `m3hi` en
//! la simulación; el cliente lee el frame vivo del mapa cada frame.

use bevy::ecs::change_detection::DetectChangesMut;
use bevy::prelude::*;
use openttdrs_core::industry_gfx as core_industry_gfx;
use openttdrs_core::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, wang_hash};
use crate::render::tiles::leveled_foundation_overlay_pos;
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    MapVisualLayer, TileRenderContext, ViewportSortableParent, WorldAssets, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{industry_effective_m4_for_draw, industry_gfx_entry_for_tile};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct IndustryBuildingAnimPlugin;

impl Plugin for IndustryBuildingAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_industry_building_layers
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Contexto para recalcular posición al cambiar sprite (offsets NFO por frame).
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryOverlayContext {
    pub(crate) iso_pos: Vec2,
    pub(crate) base_z: u8,
    pub(crate) overlay_z: u8,
    pub(crate) leveled: bool,
    pub(crate) tx: i32,
    pub(crate) ty: i32,
}

/// Capa de suelo o edificio que cicla con `anim_state` + frame `m4`.
#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryBuildingAnim {
    gfx: u16,
    m1: u8,
    /// Fase inicial: `m3hi` del save o hash de tesela.
    phase: u8,
    ground: bool,
    ctx: IndustryOverlayContext,
    /// Dimensión del mapa que produjo esta capa; forma parte del slot fuente
    /// del compositor y no debe inferirse desde la Z ya ordenada.
    map_width: u32,
}

impl IndustryBuildingAnim {
    pub(crate) fn new(
        gfx: u16,
        m1: u8,
        phase: u8,
        ground: bool,
        ctx: IndustryOverlayContext,
        map_width: u32,
    ) -> Self {
        Self {
            gfx,
            m1,
            phase,
            ground,
            ctx,
            map_width,
        }
    }
}

impl IndustryOverlayContext {
    pub(crate) fn from_tile_ctx(
        ctx: &TileRenderContext,
        base_z: u8,
        overlay_z: u8,
        leveled: bool,
    ) -> Self {
        Self {
            iso_pos: ctx.iso_pos,
            base_z,
            overlay_z,
            leveled,
            tx: ctx.tx_i32(),
            ty: ctx.ty_i32(),
        }
    }

    fn overlay_at(&self, xrel: f32, yrel: f32, w: f32, h: f32, layer: f32) -> Vec3 {
        if self.leveled {
            leveled_foundation_overlay_pos(
                self.iso_pos,
                xrel,
                yrel,
                w,
                h,
                self.base_z,
                layer,
                self.tx,
                self.ty,
            )
        } else {
            overlay_pos(
                self.iso_pos,
                xrel,
                yrel,
                w,
                h,
                self.overlay_z,
                layer,
                self.tx,
                self.ty,
            )
        }
    }
}

/// Resultado visual de un frame de edificio animado. La capa de suelo sigue
/// siendo un draw de terreno; sólo el edificio plano se convierte en parent
/// porque `DrawTile_Industry` le entrega la caja `M(...)` al compositor.
#[derive(Clone, Copy)]
struct IndustryBuildingFrame {
    sprite_id: u32,
    translation: Vec3,
    parent: Option<ViewportSortableParent>,
}

/// Bounds inclusivos del `AddSortableSpriteToDraw` de una fila vanilla de
/// `industry_land.h`. Cada frame puede cambiar la caja, por eso el producer
/// animado la reconstruye en vez de heredar la de su spawn inicial.
fn industry_building_parent_bounds(
    ctx: IndustryOverlayContext,
    spec: &crate::sprites::IndustryGfxSprite,
) -> ParentSpriteBounds {
    let x = ctx.tx * 16 + spec.sort_ox;
    let y = ctx.ty * 16 + spec.sort_oy;
    let z = i32::from(ctx.base_z) * 8 + spec.sort_oz;
    ParentSpriteBounds::new(
        x,
        y,
        z,
        x + spec.sort_ex - 1,
        y + spec.sort_ey - 1,
        z + spec.sort_ez - 1,
    )
}

/// Materializa el frame actual sin consultar el atlas. Mantener esta parte
/// pura permite verificar el contrato de orden aunque el PNG llegue tarde.
fn industry_building_frame(
    anim: &IndustryBuildingAnim,
    entry: &crate::sprites::IndustryGfxSprite,
) -> Option<IndustryBuildingFrame> {
    let (sprite_id, w, h, xrel, yrel, layer) = if anim.ground {
        (
            entry.ground_sprite_id,
            entry.ground_w,
            entry.ground_h,
            entry.ground_xrel,
            entry.ground_yrel,
            0.45,
        )
    } else {
        (
            entry.sprite_id,
            entry.w,
            entry.h,
            entry.xrel,
            entry.yrel,
            0.5,
        )
    };
    if sprite_id == 0 || w <= 0.0 || h <= 0.0 {
        return None;
    }

    let source_translation = anim.ctx.overlay_at(xrel, yrel, w, h, layer);
    let parent = (!anim.ground && !anim.ctx.leveled).then(|| {
        let source_x = u32::try_from(anim.ctx.tx).unwrap_or(0);
        let source_y = u32::try_from(anim.ctx.ty).unwrap_or(0);
        let source_depth = viewport_source_depth(source_translation.z, source_x, anim.map_width);
        ViewportSortableParent {
            sprite_id,
            bounds: industry_building_parent_bounds(anim.ctx, entry),
            // `DrawTile_Industry` emite primero suelo y luego el edificio.
            insertion_key: viewport_insertion_key(source_x, source_y, 2),
            source_depth,
        }
    });
    let translation = parent.map_or(source_translation, |parent| {
        Vec3::new(
            source_translation.x,
            source_translation.y,
            parent.source_depth,
        )
    });
    Some(IndustryBuildingFrame {
        sprite_id,
        translation,
        parent,
    })
}

/// La Z efectiva pertenece al compositor global. Al avanzar un frame no se
/// puede restaurar el slot fuente antes de que corra el siguiente sort.
fn set_industry_building_translation_if_changed(
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

/// Fase estable por tesela (desincroniza torres/pozos adyacentes).
#[must_use]
pub(crate) fn industry_anim_phase(tx: i32, ty: i32, m4: u8) -> u8 {
    let tx = tx.max(0) as u32;
    let ty = ty.max(0) as u32;
    (wang_hash(tx, ty, 0x1A07) as u8).wrapping_add(m4 & 3)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_industry_anim_layer(
    commands: &mut Commands,
    assets: &WorldAssets,
    chunk: crate::render::MapTileChunk,
    anim: IndustryBuildingAnim,
    entry: &crate::sprites::IndustryGfxSprite,
) {
    let Some(frame) = industry_building_frame(&anim, entry) else {
        return;
    };
    let Some(img) = assets.industries.get(&frame.sprite_id) else {
        return;
    };
    let mut entity = commands.spawn((
        MapVisualLayer,
        chunk,
        anim,
        img.sprite(),
        Transform::from_translation(frame.translation),
        Visibility::Visible,
    ));
    if let Some(parent) = frame.parent {
        entity.insert(parent);
    }
}

pub(crate) fn animate_industry_building_layers(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &IndustryBuildingAnim,
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
        let coord = TileCoord::new(anim.ctx.tx, anim.ctx.ty);
        let (gfx, m1, m3hi) = sim
            .state
            .map
            .get(coord)
            .map(|t| (core_industry_gfx(&t), t.m1, t.m3hi))
            .unwrap_or((anim.gfx, anim.m1, anim.phase));
        let m4 = industry_effective_m4_for_draw(gfx, m1, m3hi, 0.0, 0);
        let Some(entry) = industry_gfx_entry_for_tile(gfx, m1, m4) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        let Some(frame) = industry_building_frame(anim, entry) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        let Some(img) = assets.industries.get(&frame.sprite_id) else {
            visibility.set_if_neq(Visibility::Hidden);
            if sortable_parent.is_some() {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            continue;
        };
        visibility.set_if_neq(Visibility::Visible);
        if !img.matches(&sprite) {
            img.apply_to(&mut sprite);
        }
        let preserves_sorted_depth = sortable_parent.is_some() && frame.parent.is_some();
        set_industry_building_translation_if_changed(
            &mut transform,
            frame.translation,
            preserves_sorted_depth,
        );
        match (sortable_parent, frame.parent) {
            (Some(mut current), Some(next)) => {
                current.set_if_neq(next);
            }
            (Some(_), None) => {
                commands.entity(entity).remove::<ViewportSortableParent>();
            }
            (None, Some(next)) => {
                commands.entity(entity).insert(next);
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::schedule::Schedule;

    use super::*;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};
    use crate::sprites::industry_gfx_entry_for_tile;

    #[test]
    fn anim_phase_varies_by_tile() {
        assert_ne!(industry_anim_phase(0, 0, 0), industry_anim_phase(1, 0, 0));
    }

    fn flat_anim(gfx: u16, ground: bool) -> IndustryBuildingAnim {
        IndustryBuildingAnim::new(
            gfx,
            0x80,
            0,
            ground,
            IndustryOverlayContext {
                iso_pos: Vec2::ZERO,
                base_z: 1,
                overlay_z: 1,
                leveled: false,
                tx: 186,
                ty: 1,
            },
            256,
        )
    }

    #[test]
    fn flat_animated_building_uses_its_current_native_prism() {
        // Gfx 1 es una torre animada. El frame 0 conserva `M(7,0,0,9,9,30)`
        // de `industry_land.h`, desplazado a la tesela (186, 1).
        let anim = flat_anim(1, false);
        let entry = industry_gfx_entry_for_tile(1, 0x80, 0).expect("frame animado vanilla");
        let frame = industry_building_frame(&anim, entry).expect("edificio visible");
        let parent = frame.parent.expect("edificio plano sortable");

        assert_eq!(parent.sprite_id, entry.sprite_id);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(2983, 16, 8, 2991, 24, 37)
        );
        assert_eq!(parent.insertion_key, viewport_insertion_key(186, 1, 2));
        assert_eq!(
            parent.source_depth,
            viewport_source_depth(
                anim.ctx
                    .overlay_at(entry.xrel, entry.yrel, entry.w, entry.h, 0.5)
                    .z,
                186,
                256,
            )
        );
        assert_eq!(frame.translation.z, parent.source_depth);
    }

    #[test]
    fn animated_building_updates_its_parent_and_leaves_non_parent_layers_local() {
        let anim = flat_anim(1, false);
        let first = industry_building_frame(
            &anim,
            industry_gfx_entry_for_tile(1, 0x80, 0).expect("primer frame"),
        )
        .expect("primer frame visible")
        .parent
        .expect("parent inicial");
        let second = industry_building_frame(
            &anim,
            industry_gfx_entry_for_tile(1, 0x80, 1).expect("segundo frame"),
        )
        .expect("segundo frame visible")
        .parent
        .expect("parent siguiente");
        assert_ne!(first.sprite_id, second.sprite_id);
        assert_ne!(first, second, "el cambio de frame debe invalidar el sort");

        let ground = flat_anim(1, true);
        let entry = industry_gfx_entry_for_tile(1, 0x80, 0).expect("frame de suelo");
        assert!(
            industry_building_frame(&ground, entry)
                .expect("suelo visible")
                .parent
                .is_none(),
            "DrawGroundSprite no entra en parent_sprites_to_draw"
        );

        let mut leveled = flat_anim(1, false);
        leveled.ctx.leveled = true;
        assert!(
            industry_building_frame(&leveled, entry)
                .expect("edificio sobre cimiento visible")
                .parent
                .is_none(),
            "un cimiento legacy requiere su bloque parent/children propio"
        );
    }

    #[test]
    fn animated_industry_building_enters_the_global_viewport_sorter() {
        let anim = flat_anim(1, false);
        let entry = industry_gfx_entry_for_tile(1, 0x80, 0).expect("frame animado");
        let frame = industry_building_frame(&anim, entry).expect("edificio visible");
        let parent = frame.parent.expect("parent plano");
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let building = world
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
            .entity(building)
            .get::<Transform>()
            .expect("transform del edificio")
            .translation
            .z;
        assert!(
            sorted_depth > parent.source_depth,
            "el edificio debe recibir la profundidad resuelta por el compositor"
        );

        let mut transform = world
            .get_mut::<Transform>(building)
            .expect("transform para siguiente frame");
        set_industry_building_translation_if_changed(&mut transform, frame.translation, true);
        assert_eq!(
            transform.translation.z, sorted_depth,
            "la animación no puede restaurar source_depth entre dos sorts"
        );
    }
}
