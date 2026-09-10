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

use bevy::asset::{AssetEvent, AssetId};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use serde_json::json;

use crate::iso::{HEIGHT_PX, ISO_HW, ISO_QH};
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

/// Límite vertical de un edificio que `ViewportAddLandscape` usa al decidir
/// si una tesela situada debajo del framebuffer aún puede dibujar en él.
const VIEWPORT_SORT_MAX_BUILDING_HEIGHT_PX: f32 = 200.0;

/// `construction.max_bridge_height` por defecto de OpenTTD. El estado Rust
/// todavía no expone esa preferencia de construcción, por lo que éste es el
/// mismo límite conservador que ve un save normal al calcular el alcance del
/// compositor.
const VIEWPORT_SORT_MAX_BRIDGE_HEIGHT_LEVELS: f32 = 12.0;

/// Dos columnas/filas adicionales que `ViewportAddLandscape` recorre antes
/// de decidir la visibilidad concreta de la tesela.
const VIEWPORT_SORT_EDGE_TILES: i64 = 2;

/// Margen de producers alrededor del rectángulo visual geométrico.
///
/// `ViewportAddLandscape` llega a considerar edificios de hasta 200 px por
/// encima de la tesela; doce filas de 16 px más el solape interno de dos
/// filas de `ortho_visible_tile_bounds` cubren ese máximo sin volver a incluir
/// el rectángulo de prefetch de 18 teselas usado sólo para materializar chunks.
const VIEWPORT_SORT_BUILDING_MARGIN_TILES: u32 = 11;

/// El culling preciso se validó contra el raster nativo hasta `Normal`; al
/// alejar la cámara se conserva el conjunto AABB ya existente.
const VIEWPORT_SORT_PRECISE_MAX_ORTHO_SCALE: f32 = 1.0;

#[must_use]
fn precise_sort_scope_enabled(ortho_scale: f32) -> bool {
    ortho_scale <= VIEWPORT_SORT_PRECISE_MAX_ORTHO_SCALE
}

/// Ventana de producers expresada en las coordenadas diagonales que usa
/// `ViewportAddLandscape`: `row = x + y`, `column = y - x`.
///
/// Un AABB de `(x, y)` contiene dos triángulos que no proyectan dentro de la
/// pantalla. Ordenarlos junto con la banda visible modifica el resultado de
/// `ViewportSortParentSprites`, aunque nunca puedan llegar al framebuffer.
/// Mantener aquí el rectángulo en `(row, column)` reproduce el barrido del
/// viewport nativo sin confundirlo con el AABB de prefetch de chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DiagonalViewportSortScope {
    row_min: i64,
    row_max: i64,
    column_min: i64,
    column_max: i64,
    screen_left: i64,
    screen_right: i64,
    screen_bottom: i64,
    screen_top: i64,
}

/// Alcances del último pase de sorter.
///
/// Se guardan juntos porque ambos describen una misma vista: el AABB conserva
/// el contrato histórico y la banda diagonal sólo lo refina cuando aplica.
/// Un único estado evita ejecutar el sort dos veces al cambiar cualquiera de
/// los dos límites.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ViewportSortScopeState {
    scope: Option<TileViewportBounds>,
    precise_scope: Option<DiagonalViewportSortScope>,
}

/// Entradas ECS que describen una única vista de mundo para el sorter.
///
/// Mantener cámara, ventana y simulación juntas evita que el sistema de sort
/// mezcle dos vistas distintas y conserva su firma como un sistema pequeño.
#[derive(SystemParam)]
pub(crate) struct ViewportSortScopeInputs<'w, 's> {
    sim: Option<Res<'w, SimWorld>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<
        'w,
        's,
        (&'static Transform, &'static Projection),
        (
            With<PrimaryGameCamera>,
            Without<MapPreviewCamera>,
            Without<ViewportSortableParent>,
        ),
    >,
}

/// Rectángulo de píxeles que Bevy compone para un parent no vacío.
///
/// `AddSortableSpriteToDraw` decide si entrega un parent al sorter con el
/// rectángulo del PNG (`x_offs`, `y_offs`, ancho y alto), no con su prisma 3D.
/// Mantener la misma representación aquí evita convertir un desajuste del
/// ancla del sprite en un margen arbitrario de viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SpriteScreenBounds {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
}

/// Tamaño compuesto de un [`Sprite`] antes de aplicar su `Transform`.
///
/// Coincide con el tamaño que usa el renderer de Bevy: `custom_size` tiene
/// prioridad, un recorte usa su rectángulo y un atlas usa la entrada resuelta.
/// Cuando el asset aún no está disponible se devuelve `None` para conservar el
/// fallback geométrico de bounds; un sprite sin tamaño aún no puede aportar
/// píxeles al framebuffer.
fn sprite_render_size(
    sprite: &Sprite,
    images: Option<&Assets<Image>>,
    texture_atlases: Option<&Assets<TextureAtlasLayout>>,
) -> Option<Vec2> {
    if let Some(size) = sprite.custom_size {
        return Some(size);
    }
    if let Some(rect) = sprite.rect {
        return Some(rect.size());
    }
    if let Some(atlas) = sprite.texture_atlas.as_ref() {
        return texture_atlases
            .and_then(|layouts| atlas.texture_rect(layouts))
            .map(|rect| rect.as_rect().size());
    }
    images
        .and_then(|images| images.get(&sprite.image))
        .map(|image| image.size().as_vec2())
}

/// AABB de la geometría que Bevy va a rasterizar para un sprite.
///
/// Los parents del mapa son normalmente ejes-alineados, pero transformar los
/// cuatro vértices conserva la semántica correcta si una familia añade escala
/// o rotación. El `Anchor` forma parte de la malla real de Bevy y no se puede
/// suponer que siempre sea el centro.
fn sprite_screen_bounds(
    sprite: &Sprite,
    anchor: &Anchor,
    transform: &Transform,
    images: Option<&Assets<Image>>,
    texture_atlases: Option<&Assets<TextureAtlasLayout>>,
) -> Option<SpriteScreenBounds> {
    let size = sprite_render_size(sprite, images, texture_atlases)?;
    if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }

    let center = -anchor.as_vec() * size;
    let half = size * 0.5;
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut bottom = f32::INFINITY;
    let mut top = f32::NEG_INFINITY;
    for local in [
        Vec2::new(center.x - half.x, center.y - half.y),
        Vec2::new(center.x - half.x, center.y + half.y),
        Vec2::new(center.x + half.x, center.y - half.y),
        Vec2::new(center.x + half.x, center.y + half.y),
    ] {
        let point = transform.transform_point(local.extend(0.0));
        left = left.min(point.x);
        right = right.max(point.x);
        bottom = bottom.min(point.y);
        top = top.max(point.y);
    }
    Some(SpriteScreenBounds {
        left,
        right,
        bottom,
        top,
    })
}

impl DiagonalViewportSortScope {
    fn from_camera(
        camera_world: Vec2,
        ortho_scale: f32,
        window_width: f32,
        window_height: f32,
    ) -> Self {
        let half_width = window_width * 0.5 * ortho_scale;
        let half_height = window_height * 0.5 * ortho_scale;
        let left = camera_world.x - half_width;
        let right = camera_world.x + half_width;
        // Bevy crece hacia arriba; `ViewportAddLandscape` crece hacia abajo.
        let top = camera_world.y + half_height;
        let bottom = camera_world.y - half_height;

        // C++ divide enteros con truncamiento hacia cero. Usar `floor` aquí
        // ampliaba silenciosamente la banda negativa de columnas y volvía a
        // introducir producers de los triángulos externos del AABB.
        let column_min = (left / ISO_HW).trunc() as i64 - VIEWPORT_SORT_EDGE_TILES;
        let column_max = (right / ISO_HW).trunc() as i64 + VIEWPORT_SORT_EDGE_TILES;
        let row_min = (-top / ISO_QH).trunc() as i64 - VIEWPORT_SORT_EDGE_TILES;

        // `min_visible_height < MAX_TILE_EXTENT_TOP + bridge_height`: una
        // tesela al sur de la pantalla puede aportar un edificio alto o la
        // cubierta de un puente. El margen sólo crece en esa dirección; no
        // debe ensanchar ambas coordenadas como hacía el antiguo AABB.
        let lower_overhang = VIEWPORT_SORT_MAX_BUILDING_HEIGHT_PX
            + VIEWPORT_SORT_MAX_BRIDGE_HEIGHT_LEVELS * HEIGHT_PX;
        let row_max = ((-bottom + lower_overhang) / ISO_QH).ceil() as i64 - 1;

        Self {
            row_min,
            row_max,
            column_min,
            column_max,
            // Las cajas del sorter se expresan en píxeles enteros de mundo.
            // Redondear hacia fuera evita descartar un parent por una fracción
            // de pixel mientras la cámara se mueve.
            screen_left: left.floor() as i64,
            screen_right: right.ceil() as i64,
            screen_bottom: bottom.floor() as i64,
            screen_top: top.ceil() as i64,
        }
    }

    #[must_use]
    fn contains_source_tile(self, tx: u32, ty: u32) -> bool {
        let tx = i64::from(tx);
        let ty = i64::from(ty);
        let row = tx + ty;
        let column = ty - tx;
        self.row_min <= row
            && row <= self.row_max
            && self.column_min <= column
            && column <= self.column_max
    }

    /// El sort nativo sólo recibe un parent si el draw-proc de su tesela lo
    /// entrega al viewport. La banda diagonal recupera ese recorrido, pero el
    /// renderer retiene chunks de prefetch con producers cuya caja 3D ya no
    /// puede alcanzar el framebuffer. Dejarlos en la lista reasigna slots de
    /// profundidad a sprites visibles aunque OpenTTD nunca los rasterice.
    ///
    /// Las fórmulas son `RemapCoords` en la escala del cliente Rust:
    /// `x = 2 * (y - x)`, `y = -x - y + z`. Los máximos de
    /// [`ParentSpriteBounds`] son inclusivos, mientras que el clipping nativo
    /// proyecta el extremo `origin + extent` y deja un píxel de margen en los
    /// lados derecho/inferior. Los extremos se reconstruyen aquí para no
    /// perder un `SPR_EMPTY_BOUNDING_BOX` justo en el borde. OpenTTD admite
    /// extents cero (`max < min`), que conservamos como un extremo igual al
    /// origen.
    #[must_use]
    fn parent_bounds_reach_viewport(self, bounds: ParentSpriteBounds) -> bool {
        let xmin = i64::from(bounds.xmin.min(bounds.xmax));
        let xmax = i64::from(bounds.xmin.max(bounds.xmax));
        let ymin = i64::from(bounds.ymin.min(bounds.ymax));
        let ymax = i64::from(bounds.ymin.max(bounds.ymax));
        let zmin = i64::from(bounds.zmin.min(bounds.zmax));
        let zmax = i64::from(bounds.zmin.max(bounds.zmax));

        let x_end = if bounds.xmax < bounds.xmin {
            xmin
        } else {
            xmax + 1
        };
        let y_end = if bounds.ymax < bounds.ymin {
            ymin
        } else {
            ymax + 1
        };
        let z_end = if bounds.zmax < bounds.zmin {
            zmin
        } else {
            zmax + 1
        };

        let projected_left = 2 * (ymin - x_end);
        let projected_right = 2 * (y_end - xmin) + 1;
        let projected_bottom = -x_end - y_end + zmin - 1;
        let projected_top = -xmin - ymin + z_end;
        projected_right > self.screen_left
            && projected_left < self.screen_right
            && projected_top > self.screen_bottom
            && projected_bottom < self.screen_top
    }

    /// Reproduce el test de clipping de `AddSortableSpriteToDraw` para un PNG
    /// real. Sus rectángulos son semiabiertos: si sólo tocan el borde no se
    /// agrega ningún parent al sorter nativo.
    #[must_use]
    fn sprite_reaches_viewport(self, sprite: SpriteScreenBounds) -> bool {
        sprite.left < self.screen_right as f32
            && sprite.right > self.screen_left as f32
            && sprite.bottom < self.screen_top as f32
            && sprite.top > self.screen_bottom as f32
    }
}

/// Productores que OpenTTD puede entregar al sorter para la vista actual.
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
    // Sin mundo simulado el sorter puro conserva el stream completo. La
    // existencia del recurso, no sus dimensiones, es el contrato del pase de
    // mundo activo.
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

/// Refina el AABB histórico sólo en los zooms donde el raster verificó el
/// alcance diagonal de `ViewportAddLandscape`.
fn viewport_precise_sort_scope(
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
) -> Option<DiagonalViewportSortScope> {
    sim?;
    let Ok((transform, projection)) = cameras.single() else {
        return None;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return None;
    };
    if !precise_sort_scope_enabled(orthographic.scale) {
        return None;
    }
    let (width, height) = windows
        .iter()
        .next()
        .map(|window| (window.width(), window.height()))
        .unwrap_or((1280.0, 720.0));
    Some(DiagonalViewportSortScope::from_camera(
        transform.translation.truncate(),
        orthographic.scale,
        width,
        height,
    ))
}

fn parent_is_in_viewport_sort_scope(
    parent: &ViewportSortableParent,
    sprite_bounds: Option<SpriteScreenBounds>,
    scope: Option<TileViewportBounds>,
    precise_scope: Option<DiagonalViewportSortScope>,
) -> bool {
    let Some(scope) = scope else {
        // Las pruebas del sorter puro y los contextos sin cámara continúan
        // representando el stream completo, como el C++ cuando el draw-proc
        // recibe explícitamente todos los producers.
        return true;
    };
    let Some((tx, ty)) = viewport_parent_source_tile(parent.insertion_key) else {
        return false;
    };
    if !(scope.tx0 <= tx && tx < scope.tx1 && scope.ty0 <= ty && ty < scope.ty1) {
        return false;
    }
    let Some(precise_scope) = precise_scope else {
        return true;
    };
    precise_scope.contains_source_tile(tx, ty)
        && if parent.sprite_id == EMPTY_BOUNDING_BOX_SPRITE_ID {
            precise_scope.parent_bounds_reach_viewport(parent.bounds)
        } else {
            sprite_bounds.map_or_else(
                || precise_scope.parent_bounds_reach_viewport(parent.bounds),
                |sprite| precise_scope.sprite_reaches_viewport(sprite),
            )
        }
}

/// IDs de assets cuyo tamaño o contenido cambió desde el último pase.
///
/// `Assets<T>` se marca como cambiado también cuando otro sistema toma un
/// `ResMut` para actualizar su bookkeeping. El sorter sólo necesita
/// invalidarse cuando el evento afecta a una textura o layout que realmente
/// usa uno de sus parents.
fn changed_asset_ids<A: bevy::asset::Asset>(
    messages: Option<&Messages<AssetEvent<A>>>,
    cursor: &mut MessageCursor<AssetEvent<A>>,
) -> std::collections::HashSet<AssetId<A>> {
    let Some(messages) = messages else {
        return std::collections::HashSet::new();
    };
    cursor
        .read(messages)
        .map(|event| match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::Removed { id }
            | AssetEvent::Unused { id }
            | AssetEvent::LoadedWithDependencies { id } => *id,
        })
        .collect()
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
    precise_scope: Option<DiagonalViewportSortScope>,
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
        "precise_scope": precise_scope.map(|scope| json!({
            "row_min": scope.row_min,
            "row_max": scope.row_max,
            "column_min": scope.column_min,
            "column_max": scope.column_max,
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn sort_viewport_sortable_parents(
    mut parents: Query<(
        Entity,
        Ref<ViewportSortableParent>,
        Option<Ref<Visibility>>,
        &mut Transform,
        Option<(Ref<Sprite>, Ref<Anchor>)>,
    )>,
    mut removed: RemovedComponents<ViewportSortableParent>,
    mut child_depth_windows: ResMut<ViewportSortableChildDepthWindows>,
    viewport: ViewportSortScopeInputs,
    image_events: Option<Res<Messages<AssetEvent<Image>>>>,
    atlas_events: Option<Res<Messages<AssetEvent<TextureAtlasLayout>>>>,
    mut image_event_cursor: Local<MessageCursor<AssetEvent<Image>>>,
    mut atlas_event_cursor: Local<MessageCursor<AssetEvent<TextureAtlasLayout>>>,
    images: Option<Res<Assets<Image>>>,
    texture_atlases: Option<Res<Assets<TextureAtlasLayout>>>,
    mut previous_scope: Local<Option<ViewportSortScopeState>>,
) {
    let scope = viewport_sort_scope(
        viewport.sim.as_deref(),
        &viewport.windows,
        &viewport.cameras,
    );
    let precise_scope = viewport_precise_sort_scope(
        viewport.sim.as_deref(),
        &viewport.windows,
        &viewport.cameras,
    );
    let scope_state = ViewportSortScopeState {
        scope,
        precise_scope,
    };
    let scope_changed = previous_scope.as_ref() != Some(&scope_state);
    *previous_scope = Some(scope_state);

    // La geometría precisa depende de que Bevy haya materializado la imagen o
    // el layout del atlas. Esa carga no modifica `Sprite` ni `Anchor`, pero sí
    // puede convertir un fallback de bounds 3D en el rectángulo real del PNG;
    // sin esta invalidación un parent grande podía quedarse fuera del viewport
    // en el borde durante toda su vida. No usamos `Res::is_changed` aquí:
    // otros sistemas toman `ResMut<Assets<Image>>` para actualizar caches sin
    // cambiar el rectángulo que el sorter debe considerar.
    let changed_image_ids = changed_asset_ids(image_events.as_deref(), &mut image_event_cursor);
    let changed_atlas_ids = changed_asset_ids(atlas_events.as_deref(), &mut atlas_event_cursor);
    let asset_geometry_changed = (!changed_image_ids.is_empty() || !changed_atlas_ids.is_empty())
        && parents.iter_mut().any(|(_, _, _, _, sprite)| {
            let Some((sprite, _)) = sprite else {
                return false;
            };
            changed_image_ids.contains(&sprite.image.id())
                || sprite
                    .texture_atlas
                    .as_ref()
                    .is_some_and(|atlas| changed_atlas_ids.contains(&atlas.layout.id()))
        });
    let mut needs_sort = scope_changed || asset_geometry_changed || removed.read().next().is_some();
    for (_, parent, visibility, _, sprite) in &mut parents {
        needs_sort |= parent.is_added()
            || parent.is_changed()
            || visibility.as_ref().is_some_and(DetectChanges::is_changed)
            || sprite
                .as_ref()
                .is_some_and(|(sprite, anchor)| sprite.is_changed() || anchor.is_changed());
    }
    if !needs_sort {
        return;
    }

    let mut input = Vec::new();
    for (entity, parent, visibility, transform, sprite) in &mut parents {
        let sprite_bounds = precise_scope.and_then(|_| {
            sprite.and_then(|(sprite, anchor)| {
                sprite_screen_bounds(
                    &sprite,
                    &anchor,
                    &transform,
                    images.as_deref(),
                    texture_atlases.as_deref(),
                )
            })
        });
        if visibility.is_some_and(|visibility| *visibility == Visibility::Hidden)
            || !parent_is_in_viewport_sort_scope(&parent, sprite_bounds, scope, precise_scope)
        {
            continue;
        }
        input.push((entity, *parent, transform.translation.z));
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
    export_viewport_sort_trace(&input, &order, &sorted_depths, scope, precise_scope);

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
            && let Ok((_, _, _, mut transform, _)) = parents.get_mut(entity)
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
    fn viewport_scope_follows_the_native_diagonal_draw_band() {
        assert!(precise_sort_scope_enabled(0.25));
        assert!(precise_sort_scope_enabled(1.0));
        assert!(
            !precise_sort_scope_enabled(2.0),
            "Out2x conserva el conjunto AABB validado"
        );

        // Centro de la captura Kale (189,126) sobre terreno plano, a 384×320
        // y escala 1. El alcance no es el cuadrado x=181..197,
        // y=118..134: OpenTTD recorre el rectángulo equivalente en
        // row/column, dejando fuera sus dos triángulos.
        let scope = DiagonalViewportSortScope::from_camera(
            Vec2::new(-2_016.0, -5_040.0),
            1.0,
            384.0,
            320.0,
        );
        assert_eq!(
            scope,
            DiagonalViewportSortScope {
                row_min: 303,
                row_max: 343,
                column_min: -71,
                column_max: -55,
                screen_left: -2_208,
                screen_right: -1_824,
                screen_bottom: -5_200,
                screen_top: -4_880,
            }
        );

        assert!(scope.contains_source_tile(189, 126));
        assert!(scope.contains_source_tile(200, 143)); // edificio/puente alto al borde sur.
        assert!(
            !scope.contains_source_tile(181, 134),
            "la esquina del AABB no llega al framebuffer"
        );
        assert!(
            !scope.contains_source_tile(201, 143),
            "no ampliar indefinidamente la banda sur"
        );

        let visible = ParentSpriteBounds::new(3_024, 2_016, 8, 3_039, 2_031, 47);
        assert!(scope.parent_bounds_reach_viewport(visible));
        let distant_south = ParentSpriteBounds::new(3_360, 2_128, 8, 3_375, 2_143, 23);
        assert!(
            !scope.parent_bounds_reach_viewport(distant_south),
            "un producer retenido bajo el viewport no debe alterar slots visibles"
        );

        // `SPR_EMPTY_BOUNDING_BOX` proyecta el extremo exclusivo de su caja
        // (`origin + extent`), no sólo el máximo inclusivo almacenado en el
        // parent. Con una caja 1×1×1, la cara derecha puede entrar en la
        // primera columna visible aunque `2 * (ymax - xmin)` todavía quede
        // fuera. El caso también cubre el fallback 3D antes de materializar
        // un PNG de un parent.
        let edge_scope = DiagonalViewportSortScope {
            row_min: i64::MIN,
            row_max: i64::MAX,
            column_min: i64::MIN,
            column_max: i64::MAX,
            screen_left: 1,
            screen_right: 3,
            screen_bottom: 0,
            screen_top: 3,
        };
        assert!(edge_scope.parent_bounds_reach_viewport(ParentSpriteBounds::new(0, 0, 0, 0, 0, 0)));

        let touching_scope = DiagonalViewportSortScope {
            screen_left: 3,
            ..edge_scope
        };
        assert!(
            !touching_scope.parent_bounds_reach_viewport(ParentSpriteBounds::new(0, 0, 0, 0, 0, 0))
        );

        // `AddSortableSpriteToDraw` recorta contra el rectángulo del PNG, no
        // contra el prisma del parent. Un píxel dentro del borde izquierdo se
        // conserva aunque su caja 3D ya no alcance la vista; el ejemplo es la
        // capa Maglev 1241 de Kale (204,120).
        let native_left_edge = SpriteScreenBounds {
            left: -2_211.0,
            right: -2_207.0,
            bottom: -5_000.0,
            top: -4_984.0,
        };
        assert!(
            scope.sprite_reaches_viewport(native_left_edge),
            "un PNG que entra un píxel debe reservar un slot nativo"
        );
        let outside_native_edge = SpriteScreenBounds {
            left: -2_212.0,
            right: -2_208.0,
            bottom: -5_000.0,
            top: -4_984.0,
        };
        assert!(
            !scope.sprite_reaches_viewport(outside_native_edge),
            "un PNG que sólo toca el borde no debe reservar un slot"
        );
    }

    #[test]
    fn sprite_screen_bounds_follow_the_bevy_anchor_and_scale() {
        let sprite = Sprite::sized(Vec2::new(8.0, 4.0));
        let mut transform = Transform::from_xyz(10.0, 20.0, 0.0);
        transform.scale = Vec3::new(2.0, 3.0, 1.0);

        let bounds = sprite_screen_bounds(&sprite, &Anchor::TOP_LEFT, &transform, None, None)
            .expect("un Sprite con custom_size siempre tiene un rectángulo de pantalla");
        assert_eq!(
            bounds,
            SpriteScreenBounds {
                left: 10.0,
                right: 26.0,
                bottom: 8.0,
                top: 20.0,
            }
        );
    }

    #[test]
    fn sprite_screen_bounds_follow_rect_anchor_and_rotation() {
        let sprite = Sprite {
            rect: Some(Rect::new(0.0, 0.0, 8.0, 4.0)),
            ..default()
        };
        let mut transform = Transform::from_xyz(10.0, 20.0, 0.0);
        transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);

        let bounds = sprite_screen_bounds(&sprite, &Anchor::CENTER, &transform, None, None)
            .expect("un rect custom aporta bounds aunque no haya Assets<Image>");
        assert_eq!(
            bounds,
            SpriteScreenBounds {
                left: 8.0,
                right: 12.0,
                bottom: 16.0,
                top: 24.0,
            }
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixture creada dentro del mismo World.
    fn changing_parent_sprite_geometry_requests_a_new_sort() {
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let parent = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 1422,
                    bounds: ParentSpriteBounds::new(0, 0, 0, 15, 15, 15),
                    insertion_key: 0,
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
                Sprite::sized(Vec2::new(16.0, 16.0)),
                Anchor::CENTER,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<ViewportSortableChildDepthWindows>()
                .sort_runs,
            1,
            "un parent sin cambios visuales no debe reordenarse"
        );

        world
            .entity_mut(parent)
            .get_mut::<Sprite>()
            .unwrap()
            .custom_size = Some(Vec2::new(32.0, 16.0));
        schedule.run(&mut world);

        assert_eq!(
            world
                .resource::<ViewportSortableChildDepthWindows>()
                .sort_runs,
            2,
            "un cambio en la geometría pintada debe reevaluar el scope"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixture creada dentro del mismo World.
    fn loading_parent_texture_invalidates_precise_viewport_sort() {
        let mut world = viewport_scope_test_world(128, 128);
        world.insert_resource(Assets::<Image>::default());
        world.init_resource::<Messages<AssetEvent<Image>>>();
        let image_handle = world.resource::<Assets<Image>>().reserve_handle();

        // El prisma TILE_SEQ está deliberadamente lejos de la cámara, pero el
        // PNG se ubica dentro de ella. Antes de que llegue la textura, el
        // parent debe usar el fallback 3D y quedar fuera; el parent estático
        // mantiene una segunda entrada para observar que el primero vuelve al
        // stream cuando el asset pasa a estar disponible.
        let late_parent = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 4072,
                    bounds: ParentSpriteBounds::new(10_000, 10_000, 0, 10_000, 10_000, 19),
                    insertion_key: viewport_insertion_key(5, 5, 0),
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
                Sprite::from_image(image_handle.clone()),
                Anchor::CENTER,
            ))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 4073,
                bounds: ParentSpriteBounds::new(10_000, 10_000, 0, 10_000, 10_000, 19),
                insertion_key: viewport_insertion_key(5, 5, 1),
                source_depth: 1.000_5,
            },
            Transform::from_xyz(0.0, 0.0, 1.000_5),
            Sprite::sized(Vec2::splat(1.0)),
            Anchor::CENTER,
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<ViewportSortableChildDepthWindows>()
                .sort_runs,
            1,
            "un asset sin cambios no debe repetir el sort"
        );

        world
            .resource_mut::<Assets<Image>>()
            .insert(image_handle.id(), Image::default_uninit())
            .unwrap();
        world.write_message(AssetEvent::Added {
            id: image_handle.id(),
        });
        schedule.run(&mut world);

        let windows = world.resource::<ViewportSortableChildDepthWindows>();
        assert_eq!(
            windows.sort_runs, 2,
            "la carga del PNG debe invalidar el sort"
        );
        assert!(
            windows.next_parent_depth.contains_key(&late_parent),
            "el parent antes omitido debe entrar al stream cuando su textura ya tiene tamaño"
        );

        world
            .resource_mut::<Assets<Image>>()
            .remove(image_handle.id());
        world.write_message(AssetEvent::Removed {
            id: image_handle.id(),
        });
        schedule.run(&mut world);

        let windows = world.resource::<ViewportSortableChildDepthWindows>();
        assert_eq!(
            windows.sort_runs, 3,
            "la descarga del PNG debe invalidar el sort"
        );
        assert!(
            !windows.next_parent_depth.contains_key(&late_parent),
            "el parent debe volver al fallback cuando su textura fue descargada"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixture creada dentro del mismo World.
    fn loading_parent_atlas_layout_invalidates_precise_viewport_sort() {
        let mut world = viewport_scope_test_world(128, 128);
        world.insert_resource(Assets::<Image>::default());
        world.insert_resource(Assets::<TextureAtlasLayout>::default());
        world.init_resource::<Messages<AssetEvent<TextureAtlasLayout>>>();
        let image_handle = world.resource::<Assets<Image>>().reserve_handle();
        let layout_handle = world
            .resource::<Assets<TextureAtlasLayout>>()
            .reserve_handle();

        // La entrada atlas todavía no tiene layout materializado. Su bounds
        // 3D queda fuera de la banda visible y sólo debe entrar cuando Bevy
        // conoce el rectángulo que realmente va a rasterizar.
        let late_parent = world
            .spawn((
                ViewportSortableParent {
                    sprite_id: 4072,
                    bounds: ParentSpriteBounds::new(10_000, 10_000, 0, 10_000, 10_000, 19),
                    insertion_key: viewport_insertion_key(5, 5, 0),
                    source_depth: 1.0,
                },
                Transform::from_xyz(0.0, 0.0, 1.0),
                Sprite {
                    image: image_handle,
                    texture_atlas: Some(TextureAtlas {
                        layout: layout_handle.clone(),
                        index: 0,
                    }),
                    ..default()
                },
                Anchor::CENTER,
            ))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 4073,
                bounds: ParentSpriteBounds::new(10_000, 10_000, 0, 10_000, 10_000, 19),
                insertion_key: viewport_insertion_key(5, 5, 1),
                source_depth: 1.000_5,
            },
            Transform::from_xyz(0.0, 0.0, 1.000_5),
            Sprite::sized(Vec2::splat(1.0)),
            Anchor::CENTER,
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<ViewportSortableChildDepthWindows>()
                .sort_runs,
            1,
            "un layout ausente no debe repetir el sort"
        );

        let mut layout = TextureAtlasLayout::new_empty(UVec2::new(64, 32));
        layout.add_texture(URect::new(0, 0, 32, 16));
        world
            .resource_mut::<Assets<TextureAtlasLayout>>()
            .insert(layout_handle.id(), layout)
            .unwrap();
        world.write_message(AssetEvent::Added {
            id: layout_handle.id(),
        });
        schedule.run(&mut world);

        let windows = world.resource::<ViewportSortableChildDepthWindows>();
        assert_eq!(
            windows.sort_runs, 2,
            "la carga del layout debe invalidar el sort"
        );
        assert!(
            windows.next_parent_depth.contains_key(&late_parent),
            "el parent atlas antes omitido debe entrar al stream al materializarse su layout"
        );
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
                    bounds: ParentSpriteBounds::new(1_616, 1_616, 0, 1_630, 1_630, 60),
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
                    bounds: ParentSpriteBounds::new(1_616, 1_600, 0, 1_636, 1_620, 60),
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
