//! Puente de composición global para parents de casas vanilla.
//!
//! `DrawTile_Town` emite un parent sortable por edificio. El renderer 2D
//! conservaba sólo una profundidad por fila diagonal, de modo que edificios
//! altos de la misma vista nunca llegaban a `ViewportSortParentSprites`.
//! Esta capa mantiene su caja OpenTTD, su orden de inserción lógico y el
//! slot de profundidad original para volver a asignarlo según el sorter.

use bevy::prelude::*;

use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order,
};

/// Parent de una casa que participa en el sort global de la vista cargada.
///
/// `source_depth` no se recalcula después de ordenar: es el slot Bevy que la
/// casa tenía al generarse y permite repetir el sort de forma idempotente
/// tras recargar o recortar chunks.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct HouseViewportParent {
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
pub(crate) struct HouseViewportChild {
    pub(crate) parent: Entity,
    pub(crate) source_depth: f32,
}

/// Aplica el ordenador de OpenTTD a todos los edificios de casa visibles.
///
/// Sólo se ejecuta cuando se agregan, actualizan o eliminan parents; el coste
/// del algoritmo se paga durante un remap/cambio de chunks, nunca por frame.
pub(crate) fn sort_house_viewport_parents(
    mut parents: Query<(Entity, Ref<HouseViewportParent>, &mut Transform)>,
    mut removed: RemovedComponents<HouseViewportParent>,
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
            ParentSprite::sprite(entity.to_bits(), parent.sprite_id, parent.bounds)
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
pub(crate) fn sync_house_viewport_children(
    parents: Query<
        (&HouseViewportParent, &Transform),
        (With<HouseViewportParent>, Without<HouseViewportChild>),
    >,
    mut children: Query<(&HouseViewportChild, &mut Transform), With<HouseViewportChild>>,
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
    fn house_parents_keep_the_viewport_sorter_order() {
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
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_sort_moves_parent_and_screen_child_together() {
        let mut world = World::new();
        let first = world
            .spawn((
                HouseViewportParent {
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
                HouseViewportParent {
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
                HouseViewportChild {
                    parent: first,
                    source_depth: 1.000_05,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_05),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((sort_house_viewport_parents, sync_house_viewport_children).chain());
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
}
