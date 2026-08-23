//! Puente incremental de composición global para parents con bounds exactos.
//!
//! El renderer 2D conservaba sólo una profundidad por fila diagonal, de modo
//! que edificios y fundaciones con cajas que se solapan nunca llegaban juntos
//! a `ViewportSortParentSprites`. Esta capa mantiene sus cajas OpenTTD, orden
//! de inserción lógico y slots Bevy originales para volver a asignarlos según
//! el sorter. Otras familias se incorporan cuando ya tengan el mismo contrato
//! de parent y children; no se inventa geometría desde el atlas.

use bevy::prelude::*;

use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order,
};

/// `SPR_EMPTY_BOUNDING_BOX` de OpenTTD.
///
/// No tiene imagen: entra en `ViewportSortParentSprites` sólo para separar
/// prismas de infraestructura (en particular puentes y túneles) antes de que
/// se dibujen sus vecinos. Mantener el ID explícito permite que el puente entre
/// en el mismo sorter de runtime sin inventar un sprite Bevy transparente.
pub(crate) const EMPTY_BOUNDING_BOX_SPRITE_ID: u32 = 6_139;

/// Parent con bounds exactos que participa en el sort de la vista cargada.
///
/// `source_depth` no se recalcula después de ordenar: es el slot Bevy que la
/// parent tenía al generarse y permite repetir el sort de forma idempotente
/// tras recargar o recortar chunks.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct ViewportSortableParent {
    pub(crate) sprite_id: u32,
    pub(crate) bounds: ParentSpriteBounds,
    pub(crate) insertion_key: u64,
    pub(crate) source_depth: f32,
}

/// Child visual que debe conservar el delta de profundidad de su parent.
///
/// Por ahora sólo lo usa el ascensor de Large Office, que OpenTTD agrega como
/// `AddChildSpriteScreen` después del edificio. Al mover el parent entre
/// slots, el ascensor tiene que acompañarlo incluso cuando su animación
/// actualiza la posición vertical en cada frame.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct ViewportSortableChild {
    pub(crate) parent: Entity,
    pub(crate) source_depth: f32,
}

/// Clave de inserción de `ViewportAddLandscape`: fila `x + y`, luego `x`
/// descendente y finalmente el ordinal del parent dentro de su tesela.
///
/// El ordinal conserva, por ejemplo, `DrawFoundation` antes del edificio de
/// una casa. No se usa el orden de entidades ECS para desempatar parents.
pub(crate) const fn viewport_insertion_key(tx: u32, ty: u32, local_ordinal: u8) -> u64 {
    ((tx as u64 + ty as u64) << 40) | ((u32::MAX - tx) as u64) << 8 | local_ordinal as u64
}

/// Micro-slot estable dentro de una fila diagonal.
///
/// Bevy necesita Z distintos para aplicar un intercambio de parents que
/// originalmente compartían la misma fila. El rango queda por debajo de una
/// fila siguiente (`0.01`) y se normaliza por el ancho del mapa para no
/// agrandarse en mundos grandes.
pub(crate) fn viewport_source_depth(base_depth: f32, tx: u32, map_width: u32) -> f32 {
    const ROW_FRACTION: f32 = 0.005;
    let max_column = map_width.saturating_sub(1);
    if max_column == 0 {
        return base_depth;
    }
    let rank = max_column.saturating_sub(tx).min(max_column);
    base_depth + rank as f32 / max_column as f32 * ROW_FRACTION
}

/// Aplica el ordenador de OpenTTD a todos los parents instrumentados visibles.
///
/// Sólo se ejecuta cuando se agregan, actualizan o eliminan parents; el coste
/// del algoritmo se paga durante un remap/cambio de chunks, nunca por frame.
pub(crate) fn sort_viewport_sortable_parents(
    mut parents: Query<(Entity, Ref<ViewportSortableParent>, &mut Transform)>,
    mut removed: RemovedComponents<ViewportSortableParent>,
) {
    let mut needs_sort = removed.read().next().is_some();
    let mut input = Vec::new();
    for (entity, parent, transform) in &mut parents {
        needs_sort |= parent.is_added() || parent.is_changed();
        input.push((entity, *parent, transform.translation.z));
    }
    if !needs_sort || input.len() < 2 {
        return;
    }

    // El query ECS no ofrece un orden contractual. Recuperar el barrido
    // diagonal de `ViewportAddLandscape` es necesario tanto para desempates
    // del C++ como para que dos ejecuciones del mismo save sean idénticas.
    input.sort_unstable_by_key(|(_, parent, _)| parent.insertion_key);
    let sprite_parents: Vec<_> = input
        .iter()
        .map(|(entity, parent, _)| {
            if parent.sprite_id == EMPTY_BOUNDING_BOX_SPRITE_ID {
                ParentSprite::empty_bounding_box(entity.to_bits(), parent.bounds)
            } else {
                ParentSprite::sprite(entity.to_bits(), parent.sprite_id, parent.bounds)
            }
        })
        .collect();
    let source_depths: Vec<_> = input
        .iter()
        .map(|(_, parent, _)| parent.source_depth)
        .collect();
    let sorted_depths = depths_in_viewport_sort_order(&sprite_parents, &source_depths);

    for ((entity, _, current_depth), sorted_depth) in input.into_iter().zip(sorted_depths) {
        if (current_depth - sorted_depth).abs() > f32::EPSILON
            && let Ok((_, _, mut transform)) = parents.get_mut(entity)
        {
            transform.translation.z = sorted_depth;
        }
    }
}

/// Actualiza los children tras la animación de elevadores y el sort de padres.
pub(crate) fn sync_viewport_sortable_children(
    parents: Query<
        (&ViewportSortableParent, &Transform),
        (With<ViewportSortableParent>, Without<ViewportSortableChild>),
    >,
    mut children: Query<(&ViewportSortableChild, &mut Transform), With<ViewportSortableChild>>,
) {
    for (child, mut transform) in &mut children {
        let Ok((parent, parent_transform)) = parents.get(child.parent) else {
            continue;
        };
        let depth = child.source_depth + (parent_transform.translation.z - parent.source_depth);
        if (transform.translation.z - depth).abs() > f32::EPSILON {
            transform.translation.z = depth;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sortable_parents_keep_the_viewport_sorter_order() {
        // Dos edificios que se solapan: el segundo aparece primero después
        // del mismo fast path que usa `ViewportSortParentSprites` en C++.
        let parents = [
            ParentSprite::sprite(1, 1422, ParentSpriteBounds::new(16, 16, 0, 30, 30, 60)),
            ParentSprite::sprite(2, 1423, ParentSpriteBounds::new(0, 0, 0, 20, 20, 60)),
        ];
        assert_eq!(
            depths_in_viewport_sort_order(&parents, &[1.0, 1.000_5]),
            vec![1.000_5, 1.0]
        );
    }

    #[test]
    fn insertion_key_keeps_tile_scan_and_local_draw_order() {
        // En una misma fila diagonal OpenTTD visita primero la mayor X; en
        // una tesela, la fundación queda antes del edificio que la cubre.
        assert!(viewport_insertion_key(8, 2, 0) < viewport_insertion_key(7, 3, 0));
        assert!(viewport_insertion_key(7, 3, 0) < viewport_insertion_key(7, 3, 2));
        assert!(viewport_insertion_key(7, 3, 2) < viewport_insertion_key(6, 4, 0));
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_sort_moves_parent_and_screen_child_together() {
        let mut world = World::new();
        let first = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1422,
                    bounds: ParentSpriteBounds::new(16, 16, 0, 30, 30, 60),
                    insertion_key: 0,
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
            ))
            .id();
        let second = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1423,
                    bounds: ParentSpriteBounds::new(0, 0, 0, 20, 20, 60),
                    insertion_key: 1,
                    source_depth: 1.000_5,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_5),
            ))
            .id();
        let child = world
            .spawn((
                ViewportSortableChild {
                    parent: first,
                    source_depth: 1.000_05,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_05),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                sort_viewport_sortable_parents,
                sync_viewport_sortable_children,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let first_depth = world
            .entity(first)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let second_depth = world
            .entity(second)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let child_depth = world
            .entity(child)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        assert!((first_depth - 1.000_5).abs() < 1e-6);
        assert!((second_depth - 1.0).abs() < 1e-6);
        assert!((child_depth - 1.000_55).abs() < 1e-6);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_empty_bounding_box_consumes_a_sort_slot_without_a_sprite() {
        let mut world = World::new();
        let visible = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1422,
                    bounds: ParentSpriteBounds::new(4, 4, 4, 6, 6, 6),
                    insertion_key: 0,
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
            ))
            .id();
        // En producción esta entidad sólo tiene Transform + el parent: no se
        // rasteriza, pero la caja debe mover el slot de profundidad del sprite
        // visible igual que `SPR_EMPTY_BOUNDING_BOX` en OpenTTD.
        let empty = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: EMPTY_BOUNDING_BOX_SPRITE_ID,
                    bounds: ParentSpriteBounds::new(0, 0, 0, 2, 2, 2),
                    insertion_key: 1,
                    source_depth: 1.000_5,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_5),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        let visible_depth = world
            .entity(visible)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let empty_depth = world
            .entity(empty)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        assert!((visible_depth - 1.000_5).abs() < 1e-6);
        assert!((empty_depth - 1.0).abs() < 1e-6);
    }
}
