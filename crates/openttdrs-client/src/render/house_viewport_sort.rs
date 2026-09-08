//! Puente incremental de composición global para parents con bounds exactos.
//!
//! El renderer 2D conservaba sólo una profundidad por fila diagonal, de modo
//! que edificios y fundaciones con cajas que se solapan nunca llegaban juntos
//! a `ViewportSortParentSprites`. Esta capa mantiene sus cajas OpenTTD, orden
//! de inserción lógico y slots Bevy originales para volver a asignarlos según
//! el sorter. Otras familias se incorporan cuando ya tengan el mismo contrato
//! de parent y children; no se inventa geometría desde el atlas.

use std::collections::HashMap;
use std::path::Path;

use bevy::ecs::change_detection::DetectChanges;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;

use crate::render::viewport::{TileViewportBounds, ortho_visible_tile_bounds};
#[cfg(test)]
use crate::render::viewport_sort::depths_in_viewport_sort_order;
use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order_from_order,
    viewport_sort_parent_sprites,
};
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;

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
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewportSortableParent {
    pub(crate) sprite_id: u32,
    pub(crate) bounds: ParentSpriteBounds,
    pub(crate) insertion_key: u64,
    pub(crate) source_depth: f32,
}

/// Child visual que debe conservar el delta de profundidad de su parent.
///
/// Lo usan el ascensor de Large Office y el suelo de una casa con fundación,
/// que `OpenTTD` agrega mediante `AddChildSpriteScreen`. Al mover el parent
/// entre slots, ambos deben acompañarlo incluso cuando el ascensor actualiza
/// su posición vertical en cada frame.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewportSortableChild {
    pub(crate) parent: Entity,
    pub(crate) source_depth: f32,
}

/// Límite superior de la secuencia de children de cada parent ordenado.
///
/// `ViewportDrawParentSprites` emite un parent y todos sus children como un
/// bloque atómico antes del parent siguiente. Reusar sin más el delta Z local
/// del child funciona sólo mientras el hueco entre dos slots Bevy sea mayor
/// que ese delta. Tras ordenar un mapa real, dos parents consecutivos pueden
/// quedar separados por un micro-slot menor y un child terminaba por encima
/// del edificio siguiente. Esta caché reserva el intervalo exacto hasta el
/// siguiente parent del stream de OpenTTD.
#[derive(Resource, Default)]
pub(crate) struct ViewportSortableChildDepthWindows {
    next_parent_depth: HashMap<Entity, f32>,
    /// Sonda de tests para distinguir el fast path de una ejecución real del sorter.
    #[cfg(test)]
    pub(crate) sort_runs: usize,
}

/// Clave de inserción de `ViewportAddLandscape`: fila `x + y`, luego `x`
/// descendente y finalmente el ordinal del parent dentro de su tesela.
///
/// El ordinal conserva, por ejemplo, `DrawFoundation` antes del edificio de
/// una casa. No se usa el orden de entidades ECS para desempatar parents.
pub(crate) const fn viewport_insertion_key(tx: u32, ty: u32, local_ordinal: u8) -> u64 {
    ((tx as u64 + ty as u64) << 40) | ((u32::MAX - tx) as u64) << 8 | local_ordinal as u64
}

/// Recupera la tesela que emitió un parent desde la clave de inserción.
///
/// `ViewportAddLandscape` entrega parents al sorter desde el draw-proc de la
/// tesela, no desde la intersección posterior de su caja. El valor queda
/// codificado en [`viewport_insertion_key`] para que el culling no necesite
/// inferir un productor desde bounds que pueden cubrir varias teselas.
#[must_use]
const fn viewport_parent_source_tile(insertion_key: u64) -> Option<(u32, u32)> {
    let row = (insertion_key >> 40) as u32;
    let inverted_tx = ((insertion_key >> 8) & u32::MAX as u64) as u32;
    let tx = u32::MAX - inverted_tx;
    if tx > row {
        return None;
    }
    Some((tx, row - tx))
}

/// Margen de producers alrededor del rectángulo visual geométrico.
///
/// `ViewportAddLandscape` llega a considerar edificios de hasta 200 px por
/// encima de la tesela; doce filas de 16 px más el solape interno de dos
/// filas de `ortho_visible_tile_bounds` cubren ese máximo sin volver a incluir
/// el rectángulo de prefetch de 18 teselas usado sólo para materializar chunks.
const VIEWPORT_SORT_BUILDING_MARGIN_TILES: u32 = 11;

/// Productores que OpenTTD puede entregar al sorter para la vista actual.
///
/// La representación Bevy mantiene un borde mayor para que el paneo no deje
/// huecos. Ese borde no forma parte de `ViewportDoDraw`: si se ordena junto
/// con la vista, un parent lejano puede desplazar el slot de otro que sí se
/// rasteriza. Este scope recupera el rectángulo de dibujo y deja el margen
/// suficiente para la altura máxima de edificio de OpenTTD.
fn viewport_sort_scope(
    sim: Option<&SimWorld>,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<
        (&Transform, &Projection),
        (
            With<PrimaryGameCamera>,
            Without<MapPreviewCamera>,
            Without<ViewportSortableParent>,
        ),
    >,
) -> Option<TileViewportBounds> {
    let sim = sim?;
    let Ok((transform, projection)) = cameras.single() else {
        return None;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return None;
    };
    let (width, height) = windows
        .iter()
        .next()
        .map(|window| (window.width(), window.height()))
        .unwrap_or((1280.0, 720.0));
    let (map_width, map_height) = sim.state.map.dimensions();
    Some(ortho_visible_tile_bounds(
        transform.translation.truncate(),
        orthographic.scale,
        width,
        height,
        map_width,
        map_height,
        VIEWPORT_SORT_BUILDING_MARGIN_TILES,
    ))
}

fn parent_is_in_viewport_sort_scope(
    parent: &ViewportSortableParent,
    scope: Option<TileViewportBounds>,
) -> bool {
    let Some(scope) = scope else {
        // Las pruebas del sorter puro y los contextos sin cámara continúan
        // representando el stream completo, como el C++ cuando el draw-proc
        // recibe explícitamente todos los producers.
        return true;
    };
    viewport_parent_source_tile(parent.insertion_key).is_some_and(|(tx, ty)| {
        scope.tx0 <= tx && tx < scope.tx1 && scope.ty0 <= ty && ty < scope.ty1
    })
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

/// Escribe el orden efectivo del sorter Rust cuando se pide explícitamente.
///
/// El stream es diagnóstico: a diferencia de `world-draw`, refleja las
/// entidades que llegaron al compositor Bevy, su orden de inserción nativo y
/// la reasignación final de slots. Permite contrastar una captura raster con
/// el `ViewportDoDraw` real de OpenTTD sin inferir el orden desde un PNG.
fn export_viewport_sort_trace(
    input: &[(Entity, ViewportSortableParent, f32)],
    order: &[usize],
    sorted_depths: &[f32],
    scope: Option<TileViewportBounds>,
) {
    let Some(path) = std::env::var_os("OPENTTDRS_VIEWPORT_SORT_TRACE_OUT") else {
        return;
    };
    let path = Path::new(&path);
    let parents: Vec<_> = order
        .iter()
        .enumerate()
        .map(|(final_ordinal, &input_index)| {
            let (entity, parent, input_depth) = input[input_index];
            json!({
                "final_ordinal": final_ordinal,
                "input_index": input_index,
                "entity": entity.to_bits(),
                "sprite_id": parent.sprite_id,
                "insertion_key": parent.insertion_key,
                "source_depth": parent.source_depth,
                "input_depth": input_depth,
                "sorted_depth": sorted_depths[input_index],
                "world_bounds": {
                    "xmin": parent.bounds.xmin,
                    "ymin": parent.bounds.ymin,
                    "zmin": parent.bounds.zmin,
                    "xmax": parent.bounds.xmax,
                    "ymax": parent.bounds.ymax,
                    "zmax": parent.bounds.zmax,
                },
            })
        })
        .collect();
    let document = json!({
        "contract": "openttdrs-viewport-sort",
        "schema_version": 1,
        "stage": "post_viewport_sprite_sorter",
        "scope": scope.map(|scope| json!({
            "tx0": scope.tx0,
            "ty0": scope.ty0,
            "tx1": scope.tx1,
            "ty1": scope.ty1,
        })),
        "parents_before_sort": input.len(),
        "parents": parents,
    });
    match serde_json::to_vec(&document)
        .and_then(|bytes| std::fs::write(path, bytes).map_err(serde_json::Error::io))
    {
        Ok(()) => {}
        Err(error) => warn!(
            "No se pudo escribir OPENTTDRS_VIEWPORT_SORT_TRACE_OUT={}: {error}",
            path.display()
        ),
    }
}

/// Aplica el ordenador de OpenTTD a los parents del viewport actual.
///
/// La lista nativa es local a cada `ViewportDoDraw`. El renderer puede retener
/// chunks de prefetch fuera de la cámara, pero esos producers no pueden ocupar
/// slots del pase visible. El coste se paga durante un remap, una modificación
/// de parent, un cambio de visibilidad o un desplazamiento de viewport; nunca
/// por frame estable.
pub(crate) fn sort_viewport_sortable_parents(
    mut parents: Query<(
        Entity,
        Ref<ViewportSortableParent>,
        Option<Ref<Visibility>>,
        &mut Transform,
    )>,
    mut removed: RemovedComponents<ViewportSortableParent>,
    mut child_depth_windows: ResMut<ViewportSortableChildDepthWindows>,
    sim: Option<Res<SimWorld>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<
        (&Transform, &Projection),
        (
            With<PrimaryGameCamera>,
            Without<MapPreviewCamera>,
            Without<ViewportSortableParent>,
        ),
    >,
    mut previous_scope: Local<Option<Option<TileViewportBounds>>>,
) {
    let scope = viewport_sort_scope(sim.as_deref(), &windows, &cameras);
    let scope_changed = previous_scope.as_ref() != Some(&scope);
    *previous_scope = Some(scope);

    let mut needs_sort = scope_changed || removed.read().next().is_some();
    let mut input = Vec::new();
    for (entity, parent, visibility, transform) in &mut parents {
        needs_sort |= parent.is_added()
            || parent.is_changed()
            || visibility.as_ref().is_some_and(DetectChanges::is_changed);
        if visibility.is_some_and(|visibility| *visibility == Visibility::Hidden)
            || !parent_is_in_viewport_sort_scope(&parent, scope)
        {
            continue;
        }
        input.push((entity, *parent, transform.translation.z));
    }
    if !needs_sort {
        return;
    }

    #[cfg(test)]
    {
        child_depth_windows.sort_runs += 1;
    }

    // La caché sólo es válida para el conjunto actual de parents. Limpiarla
    // también al quedar uno (o ninguno) evita que un child de un chunk
    // descargado conserve el límite de una escena anterior.
    child_depth_windows.next_parent_depth.clear();
    if input.is_empty() {
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
    let order = viewport_sort_parent_sprites(&sprite_parents);
    let sorted_depths = depths_in_viewport_sort_order_from_order(&order, &source_depths);
    export_viewport_sort_trace(&input, &order, &sorted_depths, scope);

    // En el stream final, cada parent reserva el espacio hasta el siguiente.
    // El último no tiene techo y conserva el delta histórico de sus children.
    for pair in order.windows(2) {
        let parent_index = pair[0];
        let next_parent_index = pair[1];
        child_depth_windows
            .next_parent_depth
            .insert(input[parent_index].0, sorted_depths[next_parent_index]);
    }

    for ((entity, _, current_depth), sorted_depth) in input.into_iter().zip(sorted_depths) {
        if (current_depth - sorted_depth).abs() > f32::EPSILON
            && let Ok((_, _, _, mut transform)) = parents.get_mut(entity)
        {
            transform.translation.z = sorted_depth;
        }
    }
}

/// Actualiza los children tras la animación de elevadores y el sort de padres.
///
/// Cada conjunto de children ocupa el intervalo entre su parent y el siguiente
/// parent del sorter. De este modo un sprite de suelo con transparencias no
/// puede cubrir el edificio que OpenTTD dibuja inmediatamente después.
pub(crate) fn sync_viewport_sortable_children(
    parents: Query<
        (Entity, &ViewportSortableParent, &Transform),
        (With<ViewportSortableParent>, Without<ViewportSortableChild>),
    >,
    children: Query<(Entity, &ViewportSortableChild), With<ViewportSortableChild>>,
    mut child_transforms: Query<&mut Transform, With<ViewportSortableChild>>,
    child_depth_windows: Res<ViewportSortableChildDepthWindows>,
) {
    let mut children_by_parent: HashMap<Entity, Vec<(Entity, f32)>> = HashMap::new();
    for (entity, child) in &children {
        children_by_parent
            .entry(child.parent)
            .or_default()
            .push((entity, child.source_depth));
    }

    for (parent_entity, children) in &mut children_by_parent {
        let Ok((_, parent, parent_transform)) = parents.get(*parent_entity) else {
            continue;
        };

        // `AddChildSpriteScreen` preserva la inserción de children. La
        // profundidad de origen es el desempate estable que ya usaban los
        // spawners; `Entity` sólo resuelve dos capas con la misma profundidad.
        children.sort_unstable_by(|(left_entity, left_depth), (right_entity, right_depth)| {
            left_depth
                .total_cmp(right_depth)
                .then_with(|| left_entity.to_bits().cmp(&right_entity.to_bits()))
        });
        let child_count = children.len();
        let next_parent_depth = child_depth_windows
            .next_parent_depth
            .get(parent_entity)
            .copied();

        for (rank, (entity, source_depth)) in children.iter().copied().enumerate() {
            let historical_depth =
                source_depth + (parent_transform.translation.z - parent.source_depth);
            let depth = child_depth_in_parent_interval(
                parent_transform.translation.z,
                next_parent_depth,
                rank,
                child_count,
            )
            .unwrap_or(historical_depth);
            if let Ok(mut transform) = child_transforms.get_mut(entity)
                && (transform.translation.z - depth).abs() > f32::EPSILON
            {
                transform.translation.z = depth;
            }
        }
    }
}

/// Devuelve un micro-slot para un child dentro del bloque de su parent.
///
/// `None` conserva el desplazamiento histórico cuando el parent es el último
/// de la vista o el formato `f32` no deja un valor representable entre ambos
/// slots. En el caso normal todos los children quedan estrictamente entre los
/// dos parents, como en el stream C++.
fn child_depth_in_parent_interval(
    parent_depth: f32,
    next_parent_depth: Option<f32>,
    rank: usize,
    child_count: usize,
) -> Option<f32> {
    let next_parent_depth = next_parent_depth?;
    if parent_depth.partial_cmp(&next_parent_depth) != Some(std::cmp::Ordering::Less)
        || child_count == 0
    {
        return None;
    }
    let fraction = (rank + 1) as f32 / (child_count + 1) as f32;
    let depth = parent_depth + (next_parent_depth - parent_depth) * fraction;
    (parent_depth < depth && depth < next_parent_depth).then_some(depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::PrimaryWindow;
    use openttdrs_core::GameState;

    fn viewport_scope_test_world(map_width: u32, map_height: u32) -> World {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        world.insert_resource(SimWorld {
            state: GameState::new(map_width, map_height),
            loaded_file: false,
            ottdmap_extras: None,
        });
        world.spawn((Window::default(), PrimaryWindow));
        world.spawn((
            PrimaryGameCamera,
            Transform::from_xyz(0.0, 0.0, 999.0),
            Projection::Orthographic(OrthographicProjection {
                scale: 1.0,
                ..OrthographicProjection::default_2d()
            }),
        ));
        world
    }

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
    fn insertion_key_recovers_the_parent_source_tile() {
        assert_eq!(
            viewport_parent_source_tile(viewport_insertion_key(73, 41, 7)),
            Some((73, 41))
        );
        assert_eq!(viewport_parent_source_tile(0), None);
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_sort_moves_parent_and_screen_child_together() {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
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
    fn runtime_sort_uses_the_camera_scope_and_reacts_to_pan() {
        let mut world = viewport_scope_test_world(128, 128);
        // `local` se solapa con un producer lejano. En el sorter global
        // antiguo, el producer de (100,100) lo intercambiaba aunque no pudiera
        // llegar al framebuffer de la cámara situada en (0,0).
        let local = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1422,
                    bounds: ParentSpriteBounds::new(16, 16, 0, 30, 30, 60),
                    insertion_key: viewport_insertion_key(5, 5, 0),
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
            ))
            .id();
        let remote_first = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1422,
                    bounds: ParentSpriteBounds::new(16, 16, 0, 30, 30, 60),
                    insertion_key: viewport_insertion_key(100, 100, 0),
                    source_depth: 2.0,
                },
                Transform::from_xyz(0.0, 0.0, 2.0),
            ))
            .id();
        let remote_second = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1423,
                    bounds: ParentSpriteBounds::new(0, 0, 0, 20, 20, 60),
                    insertion_key: viewport_insertion_key(101, 100, 0),
                    source_depth: 2.000_5,
                },
                Transform::from_xyz(0.0, 0.0, 2.000_5),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        assert!(
            (world
                .entity(local)
                .get::<Transform>()
                .unwrap()
                .translation
                .z
                - 1.0)
                .abs()
                < 1e-6,
            "un parent fuera de cámara no puede cambiar la profundidad local"
        );
        assert!(
            (world
                .entity(remote_first)
                .get::<Transform>()
                .unwrap()
                .translation
                .z
                - 2.0)
                .abs()
                < 1e-6,
            "el scope inicial no incluye los producers remotos"
        );

        // Al panear, ningún `ViewportSortableParent` cambió. El cambio de
        // scope debe disparar una nueva ordenación y ahora sí intercambiar los
        // dos producers que entraron en el viewport.
        let target = crate::iso::iso(100, 100);
        let mut camera_query = world.query_filtered::<&mut Transform, With<PrimaryGameCamera>>();
        camera_query.single_mut(&mut world).unwrap().translation =
            Vec3::new(target.x, target.y, 999.0);
        schedule.run(&mut world);

        assert!(
            (world
                .entity(remote_first)
                .get::<Transform>()
                .unwrap()
                .translation
                .z
                - 2.000_5)
                .abs()
                < 1e-6,
            "el paneo debe recalcular la secuencia de parents recién visibles"
        );
        assert!(
            (world
                .entity(remote_second)
                .get::<Transform>()
                .unwrap()
                .translation
                .z
                - 2.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_sort_skips_a_parent_hidden_from_the_draw_pass() {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let visible = world
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
        world.spawn((
            ViewportSortableParent {
                sprite_id: 1423,
                bounds: ParentSpriteBounds::new(0, 0, 0, 20, 20, 60),
                insertion_key: 1,
                source_depth: 1.000_5,
            },
            Transform::from_xyz(0.0, 0.0, 1.000_5),
            Visibility::Hidden,
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        assert!(
            (world
                .entity(visible)
                .get::<Transform>()
                .unwrap()
                .translation
                .z
                - 1.0)
                .abs()
                < 1e-6,
            "un sprite oculto no puede reservar un slot en el sorter"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_empty_bounding_box_consumes_a_sort_slot_without_a_sprite() {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
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

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures creados arriba dentro del mismo World.
    fn runtime_children_stay_before_the_next_sorted_parent() {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();

        // El parent de la fundación se pinta antes del edificio. En un mapa
        // real los slots globales contiguos pueden estar mucho más cerca que
        // el delta local del suelo (0.00014): el cálculo histórico lo ponía
        // por delante de `building` y dejaba visible su transparencia negra.
        let foundation = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 5473,
                    bounds: ParentSpriteBounds::new(0, 0, 0, 15, 15, 15),
                    insertion_key: 0,
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
            ))
            .id();
        let building = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1432,
                    bounds: ParentSpriteBounds::new(0, 0, 16, 15, 15, 31),
                    insertion_key: 1,
                    source_depth: 1.000_05,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_05),
            ))
            .id();
        let ground = world
            .spawn((
                ViewportSortableChild {
                    parent: foundation,
                    source_depth: 1.000_14,
                },
                Transform::from_xyz(0.0, 0.0, 1.000_14),
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

        let foundation_depth = world
            .entity(foundation)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let ground_depth = world
            .entity(ground)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let building_depth = world
            .entity(building)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        assert!(
            foundation_depth < ground_depth && ground_depth < building_depth,
            "el child debe quedar dentro de la secuencia foundation → ground → building; got {foundation_depth}, {ground_depth}, {building_depth}"
        );
    }

    #[test]
    fn child_depth_uses_only_representable_parent_intervals() {
        assert_eq!(child_depth_in_parent_interval(2.0, None, 0, 1), None);
        assert_eq!(child_depth_in_parent_interval(2.0, Some(2.0), 0, 1), None);
        assert_eq!(child_depth_in_parent_interval(2.0, Some(1.0), 0, 1), None);
        let (Some(first), Some(second)) = (
            child_depth_in_parent_interval(1.0, Some(1.000_1), 0, 2),
            child_depth_in_parent_interval(1.0, Some(1.000_1), 1, 2),
        ) else {
            panic!("un intervalo finito debe admitir slots de child");
        };
        assert!(1.0 < first && first < second && second < 1.000_1);
    }
}
