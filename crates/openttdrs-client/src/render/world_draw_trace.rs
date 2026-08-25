//! Traza opt-in de decisiones de dibujo del renderer candidato (`world-draw`).
//!
//! A diferencia de inspeccionar entidades Bevy después del spawn, esta traza
//! conserva el ID lógico OpenGFX en el punto donde cada familia de tiles lo
//! selecciona, antes de convertirlo en un `Handle<Image>` o una región de atlas.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::log::{error, info};
use openttdrs_core::prelude::TileKind;
use serde_json::json;

use super::{TileRenderContext, TileViewportBounds};

const WORLD_DRAW_OUT_ENV: &str = "OPENTTDRS_WORLD_DRAW_OUT";
const WORLD_DRAW_REGION_ENV: &str = "OPENTTDRS_WORLD_DRAW_REGION";
const WORLD_DRAW_SOURCE_ENV: &str = "OPENTTDRS_WORLD_DRAW_SOURCE";
const WORLD_DRAW_SAVE_SHA256_ENV: &str = "OPENTTDRS_WORLD_DRAW_SAVE_SHA256";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TraceRegion {
    tx0: u32,
    ty0: u32,
    tx1: u32,
    ty1: u32,
}

impl TraceRegion {
    const fn full(width: u32, height: u32) -> Self {
        Self {
            tx0: 0,
            ty0: 0,
            tx1: width,
            ty1: height,
        }
    }

    fn json(self) -> serde_json::Value {
        json!({
            "min_x": self.tx0,
            "min_y": self.ty0,
            "max_x": self.tx1.saturating_sub(1),
            "max_y": self.ty1.saturating_sub(1),
        })
    }

    fn as_bounds(self) -> TileViewportBounds {
        TileViewportBounds {
            tx0: self.tx0,
            ty0: self.ty0,
            tx1: self.tx1,
            ty1: self.ty1,
        }
    }
}

/// Valor del contrato `world-draw` para la región exportada.
///
/// OpenTTD distingue una auditoría completa (`null`) de una región que fue
/// solicitada explícitamente, incluso si ésta cubre accidentalmente todo el
/// mapa. El candidato mantiene el rectángulo completo internamente para
/// recorrer las teselas, pero no debe serializarlo como si fuera un recorte.
fn metadata_region(region: Option<TraceRegion>) -> serde_json::Value {
    region
        .map(TraceRegion::json)
        .unwrap_or(serde_json::Value::Null)
}

#[derive(Debug)]
struct TraceDraw {
    role: &'static str,
    primitive: &'static str,
    sprite_id: u32,
    palette: u32,
    fallback: bool,
    /// Un hijo de fundación está anclado al padre en píxeles de pantalla, no
    /// a una coordenada de mundo propia. Conservamos el `null` del draw proc
    /// C++ para no fingir una posición absoluta.
    has_world: bool,
    /// Señala que `world`/`offset`/`bounds` son parte del contrato incluso
    /// cuando `bounds` es `null` (como un child de fundación).
    geometry_explicit: bool,
    offset: (i32, i32, i32),
    world_xy_delta: (i32, i32),
    world_z_delta: i32,
    bounds: Option<TraceSpriteBounds>,
    /// Último `sortable` de la tesela al que OpenTTD colgaría un `child`.
    /// Los `DrawGroundSprite` posteriores a `DrawFoundation` no conservan
    /// coordenadas de mundo propias: el vínculo es parte del contrato visual.
    parent_ordinal: Option<usize>,
}

/// Decisión previa a elegir un sprite de fundación. Es deliberadamente un
/// registro separado de `TraceDraw`: permite confrontar la geometría de
/// bordes con OpenTTD aun cuando todavía no cubrimos todos sus draw procs.
#[derive(Debug)]
struct TraceFoundation {
    source: &'static str,
    foundation: u8,
    foundation_tileh: u8,
    foundation_base_z: u8,
    sprite_block: u8,
    has_nw: bool,
    has_ne: bool,
    nw_w_here: i32,
    nw_n_here: i32,
    nw_w_neighbour: i32,
    nw_n_neighbour: i32,
    ne_e_here: i32,
    ne_n_here: i32,
    ne_e_neighbour: i32,
    ne_n_neighbour: i32,
}

/// Caja de ordenación en coordenadas OpenTTD. Mantenerla en la traza permite
/// comprobar que un sprite correcto no esté anclado en la posición equivocada.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TraceSpriteBounds {
    pub(crate) ox: i32,
    pub(crate) oy: i32,
    pub(crate) oz: i32,
    pub(crate) ex: i32,
    pub(crate) ey: i32,
    pub(crate) ez: i32,
}

impl TraceSpriteBounds {
    #[must_use]
    pub(crate) const fn new(ox: i32, oy: i32, oz: i32, ex: i32, ey: i32, ez: i32) -> Self {
        Self {
            ox,
            oy,
            oz,
            ex,
            ey,
            ez,
        }
    }
}

#[derive(Debug)]
struct TraceTile {
    tx: u32,
    ty: u32,
    tile_type: u8,
    tile_kind: &'static str,
    /// Bytes crudos del save que deciden subtipos de estaciones, vías y agua.
    /// No forman parte del contrato C++ de dibujo, pero permiten diagnosticar
    /// que la selección candidata use exactamente la tesela que se importó.
    m5: u8,
    m6: u8,
    tileh: u8,
    base_z: u8,
    foundations: Vec<TraceFoundation>,
    draws: Vec<TraceDraw>,
    last_parent: Option<usize>,
}

#[derive(Debug)]
struct TraceState {
    out: PathBuf,
    source: Option<String>,
    save_sha256: Option<String>,
    width: u32,
    height: u32,
    region: TraceRegion,
    requested_region: Option<TraceRegion>,
    current: Option<(u32, u32)>, // (y, x) para el orden estable del BTreeMap.
    tiles: BTreeMap<(u32, u32), TraceTile>,
}

thread_local! {
    static ACTIVE_TRACE: RefCell<Option<TraceState>> = const { RefCell::new(None) };
}

/// Sesión sin estado público: el render del mundo es síncrono y corre en el
/// mismo hilo, por lo que un `thread_local` evita propagar un parámetro de
/// diagnóstico por todos los spawners.
pub(crate) struct WorldDrawTrace;

impl WorldDrawTrace {
    /// Inicia la traza si existe `OPENTTDRS_WORLD_DRAW_OUT`.
    ///
    /// Si se indicó `OPENTTDRS_WORLD_DRAW_REGION=x0,y0,x1,y1`, la sesión
    /// renderiza exactamente ese rectángulo inclusivo. Es deliberadamente un
    /// modo de diagnóstico: evita depender del viewport/culling actual para
    /// poder compararlo con el oráculo C++.
    pub(crate) fn start(
        width: u32,
        height: u32,
        _spawn_bounds: TileViewportBounds,
    ) -> Option<Self> {
        let out = std::env::var_os(WORLD_DRAW_OUT_ENV)?;
        let requested_region = match std::env::var(WORLD_DRAW_REGION_ENV) {
            Ok(raw) => match parse_region(&raw, width, height) {
                Ok(region) => Some(region),
                Err(message) => {
                    error!("{WORLD_DRAW_REGION_ENV}={raw:?} inválida: {message}");
                    return None;
                }
            },
            Err(_) => None,
        };
        // Una traza sin región es una auditoría de mapa, no una foto del
        // viewport inicial. El culling normal puede abarcar sólo una parte de
        // un save grande (Kale: 29.929/65.536 teselas), y convertir esa
        // muestra en "full" ocultaría justamente las divergencias alejadas de
        // la cámara. La región explícita mantiene el flujo focalizado.
        let region = requested_region.unwrap_or_else(|| TraceRegion::full(width, height));
        let state = TraceState {
            out: PathBuf::from(out),
            source: std::env::var(WORLD_DRAW_SOURCE_ENV).ok(),
            save_sha256: std::env::var(WORLD_DRAW_SAVE_SHA256_ENV).ok(),
            width,
            height,
            region,
            requested_region,
            current: None,
            tiles: BTreeMap::new(),
        };
        ACTIVE_TRACE.with(|active| *active.borrow_mut() = Some(state));
        Some(Self)
    }

    /// Rectángulo que debe recorrer el spawner durante esta sesión.
    pub(crate) fn render_bounds(&self) -> TileViewportBounds {
        ACTIVE_TRACE.with(|active| {
            let active = active.borrow();
            if let Some(state) = active.as_ref() {
                state.region.as_bounds()
            } else {
                TileViewportBounds {
                    tx0: 0,
                    ty0: 0,
                    tx1: 0,
                    ty1: 0,
                }
            }
        })
    }

    /// Abre el contexto de una tesela; los spawners inferiores pueden añadir
    /// comandos sin conocer la sesión ni los detalles del archivo JSONL.
    pub(crate) fn begin_tile(&self, ctx: &TileRenderContext) {
        ACTIVE_TRACE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(state) = active.as_mut() else {
                return;
            };
            let key = (ctx.ty, ctx.tx);
            state.tiles.entry(key).or_insert_with(|| TraceTile {
                tx: ctx.tx,
                ty: ctx.ty,
                // Para el contrato con C++ importa el nibble MAPT original.
                // `MP_OBJECT` se representa visualmente como césped, pero no
                // debe aparecer en la traza como `MP_CLEAR` (0).
                tile_type: ctx.tile.map_or_else(
                    || openttd_tile_type(ctx.kind),
                    openttdrs_core::Tile::ottd_type_nibble,
                ),
                tile_kind: tile_kind_name(ctx.kind),
                m5: ctx.tile.map_or(0, |tile| tile.m5),
                m6: ctx.tile.map_or(0, |tile| tile.m6),
                tileh: ctx.info.tileh,
                base_z: ctx.info.base_z,
                foundations: Vec::new(),
                draws: Vec::new(),
                last_parent: None,
            });
            state.current = Some(key);
        });
    }

    pub(crate) fn end_tile(&self) {
        ACTIVE_TRACE.with(|active| {
            if let Some(state) = active.borrow_mut().as_mut() {
                state.current = None;
            }
        });
    }

    /// Guarda el mismo punto de decisión que `DrawFoundation` en OpenTTD.
    /// La pendiente y altura corresponden a la superficie después de aplicar
    /// la fundación, no al relieve crudo de la tesela.
    #[allow(clippy::too_many_arguments)] // Espeja los campos de la decisión C++.
    pub(crate) fn record_foundation(
        source: &'static str,
        foundation: u8,
        foundation_tileh: u8,
        foundation_base_z: u8,
        sprite_block: u8,
        has_nw: bool,
        has_ne: bool,
        nw_edge: (i32, i32, i32, i32),
        ne_edge: (i32, i32, i32, i32),
    ) {
        ACTIVE_TRACE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(state) = active.as_mut() else {
                return;
            };
            let Some(key) = state.current else {
                return;
            };
            let Some(tile) = state.tiles.get_mut(&key) else {
                return;
            };
            tile.foundations.push(TraceFoundation {
                source,
                foundation,
                foundation_tileh,
                foundation_base_z,
                sprite_block,
                has_nw,
                has_ne,
                nw_w_here: nw_edge.0,
                nw_n_here: nw_edge.1,
                nw_w_neighbour: nw_edge.2,
                nw_n_neighbour: nw_edge.3,
                ne_e_here: ne_edge.0,
                ne_n_here: ne_edge.1,
                ne_e_neighbour: ne_edge.2,
                ne_n_neighbour: ne_edge.3,
            });
        });
    }

    /// Registra una selección de sprite OpenGFX antes de crear el sprite Bevy.
    /// `fallback` significa que el renderer no encontró el asset esperado y
    /// tuvo que omitirlo o degradarlo; sirve para separar errores de selección
    /// de errores de atlas/paleta.
    pub(crate) fn record_sprite(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        fallback: bool,
    ) {
        Self::record_sprite_with_draw_state(
            role,
            primitive,
            sprite_id,
            0,
            fallback,
            (0, 0, 0),
            (0, 0),
            0,
            None,
            true,
            false,
        );
    }

    /// Igual que [`Self::record_sprite`], pero preserva la `PaletteID` que
    /// OpenTTD entregó al blitter. Se usa para capas sin geometría explícita
    /// (por ejemplo las casas): marcar una geometría inventada convertiría el
    /// audit de traza en un falso desvío, mientras que perder la paleta lo
    /// convertía en un falso positivo visual.
    pub(crate) fn record_sprite_with_palette(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
    ) {
        Self::record_sprite_with_draw_state(
            role,
            primitive,
            sprite_id,
            palette,
            fallback,
            (0, 0, 0),
            (0, 0),
            0,
            None,
            true,
            false,
        );
    }

    /// Variante de [`Self::record_sprite`] para sprites con posición interna o
    /// bounds propios, como árboles, señales y piezas de puentes.
    pub(crate) fn record_sprite_with_geometry(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        fallback: bool,
        offset: (i32, i32, i32),
        world_z_delta: i32,
        bounds: Option<TraceSpriteBounds>,
    ) {
        Self::record_sprite_with_draw_state(
            role,
            primitive,
            sprite_id,
            0,
            fallback,
            offset,
            (0, 0),
            world_z_delta,
            bounds,
            true,
            true,
        );
    }

    /// Registra un `AddChildSpriteScreen` producido por `DrawGroundSprite`
    /// después de `DrawFoundation`. Su offset ya está normalizado por
    /// `ZOOM_BASE`, como el stream del oráculo C++.
    pub(crate) fn record_foundation_child_sprite(
        role: &'static str,
        sprite_id: u32,
        fallback: bool,
        offset: (i32, i32, i32),
    ) {
        Self::record_child_sprite_screen(role, sprite_id, 0, fallback, offset);
    }

    /// Registra un `AddChildSpriteScreen` relativo al último sortable.
    ///
    /// Se usa tanto para el suelo después de una fundación como para overlays
    /// propios del draw proc, por ejemplo el ascensor de `TownDrawHouseLift`.
    /// El offset ya está en píxeles de pantalla normalizados por `ZOOM_BASE`.
    pub(crate) fn record_child_sprite_screen(
        role: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
        offset: (i32, i32, i32),
    ) {
        Self::record_sprite_with_draw_state(
            role,
            "child",
            sprite_id,
            palette,
            fallback,
            offset,
            (0, 0),
            0,
            None,
            false,
            true,
        );
    }

    /// Como [`Self::record_foundation_child_sprite`], preservando la paleta
    /// lógica del draw proc. Las reservas PBS usan `PALETTE_CRASH` aunque se
    /// cuelguen de una fundación y por ello no tengan coordenada de mundo.
    pub(crate) fn record_foundation_child_sprite_with_palette(
        role: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
        offset: (i32, i32, i32),
    ) {
        Self::record_child_sprite_screen(role, sprite_id, palette, fallback, offset);
    }

    /// Variante que conserva la paleta lógica de OpenTTD además de la
    /// geometría. El renderer Bevy puede aplicar el recolor con otra textura,
    /// pero la traza sigue pudiendo verificar que eligió la misma paleta que
    /// el `draw_tile_proc` de referencia.
    #[allow(clippy::too_many_arguments)] // Conserva la geometría completa del draw call.
    pub(crate) fn record_sprite_with_palette_and_geometry(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
        offset: (i32, i32, i32),
        world_z_delta: i32,
        bounds: Option<TraceSpriteBounds>,
    ) {
        Self::record_sprite_with_draw_state(
            role,
            primitive,
            sprite_id,
            palette,
            fallback,
            offset,
            (0, 0),
            world_z_delta,
            bounds,
            true,
            true,
        );
    }

    /// Variante para un sprite cuyo ancla de mundo no coincide con el origen
    /// de la tesela y que conserva una paleta explícita. Los puentes usan esta
    /// forma: sus PNG pueden ser compartidos entre rail/mono/maglev, mientras que la paleta
    /// (`PALETTE_TO_STRUCT_*`) determina su apariencia final.
    #[allow(clippy::too_many_arguments)] // Espeja el draw call completo.
    pub(crate) fn record_sprite_with_palette_and_world_geometry(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
        world_xy_delta: (i32, i32),
        world_z_delta: i32,
        offset: (i32, i32, i32),
        bounds: Option<TraceSpriteBounds>,
    ) {
        Self::record_sprite_with_draw_state(
            role,
            primitive,
            sprite_id,
            palette,
            fallback,
            offset,
            world_xy_delta,
            world_z_delta,
            bounds,
            true,
            true,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn record_sprite_with_draw_state(
        role: &'static str,
        primitive: &'static str,
        sprite_id: u32,
        palette: u32,
        fallback: bool,
        offset: (i32, i32, i32),
        world_xy_delta: (i32, i32),
        world_z_delta: i32,
        bounds: Option<TraceSpriteBounds>,
        has_world: bool,
        geometry_explicit: bool,
    ) {
        ACTIVE_TRACE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(state) = active.as_mut() else {
                return;
            };
            let Some(key) = state.current else {
                return;
            };
            let Some(tile) = state.tiles.get_mut(&key) else {
                return;
            };
            let ordinal = tile.draws.len();
            // `AddSortableSpriteToDraw` dentro de `StartSpriteCombine` es un
            // hijo del primer sortable del bloque, igual que un
            // `AddChildSpriteScreen` conserva el último parent activo. Esto
            // permite que la traza candidata exprese la relación de los
            // overlays de puentes, árboles y túneles sin inventar un parent
            // adicional en el JSONL.
            let parent_ordinal = matches!(primitive, "child" | "combined")
                .then_some(tile.last_parent)
                .flatten();
            tile.draws.push(TraceDraw {
                role,
                primitive,
                sprite_id,
                palette,
                fallback,
                has_world,
                geometry_explicit,
                offset,
                world_xy_delta,
                world_z_delta,
                bounds,
                parent_ordinal,
            });
            // `AddSortableSpriteToDraw` deja el padre activo para los
            // `AddChildSpriteScreen` que siguen. El stream candidato no
            // modela todavía SpriteCombine, pero sí todos los cimientos.
            if primitive == "sortable" {
                tile.last_parent = Some(ordinal);
            }
        });
    }

    /// Escribe JSONL ordenado por `(y, x)`, aun si una tesela tuvo overlays
    /// diferidos (estaciones, casas, industrias) en una segunda pasada.
    pub(crate) fn finish(self) {
        let Some(state) = ACTIVE_TRACE.with(|active| active.borrow_mut().take()) else {
            return;
        };

        let mut rows = Vec::new();
        rows.push(
            json!({
                "kind": "metadata",
                "schema_version": 1,
                "contract": "world-draw",
                "producer": "openttdrs",
                "stage": "candidate_selection_before_atlas",
                "width": state.width,
                "height": state.height,
                "source": state.source,
                "save_sha256": state.save_sha256,
                "region": metadata_region(state.requested_region),
                "requested_region": metadata_region(state.requested_region),
                "clipping": "trace-region",
                "coverage": [
                    "tile_context",
                    "trees",
                    "rail",
                    "track_fences",
                    "catenary",
                    "tunnel",
                    "bridge",
                    "road_ground",
                    "roadside_details",
                    "road_stops",
                    "ship_depot",
                    "industries",
                    "object_landmarks",
                    "clear_land",
                    "void",
                    "water"
                ],
            })
            .to_string(),
        );

        let mut draw_count = 0usize;
        for tile in state.tiles.values() {
            rows.push(
                json!({
                    "kind": "tile",
                    "index": u64::from(tile.ty) * u64::from(state.width) + u64::from(tile.tx),
                    "x": tile.tx,
                    "y": tile.ty,
                    "tile_type": tile.tile_type,
                    "tile_kind": tile.tile_kind,
                    "m5": tile.m5,
                    "m6": tile.m6,
                    "tileh": tile.tileh,
                    "base_z": tile.base_z,
                })
                .to_string(),
            );
            for foundation in &tile.foundations {
                rows.push(
                    json!({
                        "kind": "foundation",
                        "x": tile.tx,
                        "y": tile.ty,
                        "source": foundation.source,
                        "foundation": foundation.foundation,
                        "foundation_tileh": foundation.foundation_tileh,
                        "foundation_base_z": foundation.foundation_base_z,
                        "sprite_block": foundation.sprite_block,
                        "has_nw": foundation.has_nw,
                        "has_ne": foundation.has_ne,
                        "nw_w_here": foundation.nw_w_here,
                        "nw_n_here": foundation.nw_n_here,
                        "nw_w_neighbour": foundation.nw_w_neighbour,
                        "nw_n_neighbour": foundation.nw_n_neighbour,
                        "ne_e_here": foundation.ne_e_here,
                        "ne_n_here": foundation.ne_n_here,
                        "ne_e_neighbour": foundation.ne_e_neighbour,
                        "ne_n_neighbour": foundation.ne_n_neighbour,
                    })
                    .to_string(),
                );
            }
            for (ordinal, draw) in tile.draws.iter().enumerate() {
                draw_count += 1;
                rows.push(
                    json!({
                        "kind": "draw",
                        "x": tile.tx,
                        "y": tile.ty,
                        "ordinal": ordinal,
                        "role": draw.role,
                        "primitive": draw.primitive,
                        "sprite": {
                            "source": "opengfx",
                            "id": draw.sprite_id,
                            "raw_id": draw.sprite_id,
                        },
                        "palette": draw.palette,
                        "resolved_palette": draw.palette,
                        "world": draw.has_world.then(|| json!({
                            "x": i64::from(tile.tx) * 16 + i64::from(draw.world_xy_delta.0),
                            "y": i64::from(tile.ty) * 16 + i64::from(draw.world_xy_delta.1),
                            "z": i64::from(tile.base_z) * 8 + i64::from(draw.world_z_delta),
                        })),
                        "bounds": draw.bounds.map(|bounds| json!({
                            "ox": bounds.ox,
                            "oy": bounds.oy,
                            "oz": bounds.oz,
                            "ex": bounds.ex,
                            "ey": bounds.ey,
                            "ez": bounds.ez,
                        })),
                        "offset": {
                            "x": draw.offset.0,
                            "y": draw.offset.1,
                            "z": draw.offset.2,
                        },
                        "combine_group": serde_json::Value::Null,
                        "parent_ordinal": draw.parent_ordinal,
                        "transparent": false,
                        "geometry_explicit": draw.geometry_explicit,
                        "fallback": draw.fallback,
                    })
                    .to_string(),
                );
            }
        }
        rows.push(
            json!({"kind": "complete", "tiles": state.tiles.len(), "draws": draw_count})
                .to_string(),
        );

        let contents = rows.join("\n") + "\n";
        match std::fs::write(&state.out, contents) {
            Ok(()) => info!(
                "World draw trace candidata escrita en {} ({} teselas, {draw_count} comandos)",
                state.out.display(),
                state.tiles.len()
            ),
            Err(err) => error!(
                "No se pudo escribir {WORLD_DRAW_OUT_ENV}={}: {err}",
                state.out.display()
            ),
        }
    }
}

fn parse_region(raw: &str, width: u32, height: u32) -> Result<TraceRegion, &'static str> {
    let mut values = raw.split(',').map(str::trim).map(str::parse::<u32>);
    let (Some(Ok(tx0)), Some(Ok(ty0)), Some(Ok(tx1)), Some(Ok(ty1))) =
        (values.next(), values.next(), values.next(), values.next())
    else {
        return Err("usar x0,y0,x1,y1");
    };
    if values.next().is_some() || tx0 > tx1 || ty0 > ty1 {
        return Err("usar límites inclusivos ordenados x0,y0,x1,y1");
    }
    if tx0 >= width || ty0 >= height {
        return Err("el origen queda fuera del mapa");
    }
    Ok(TraceRegion {
        tx0,
        ty0,
        tx1: tx1.saturating_add(1).min(width),
        ty1: ty1.saturating_add(1).min(height),
    })
}

fn tile_kind_name(kind: TileKind) -> &'static str {
    match kind {
        TileKind::Void => "void",
        TileKind::Grass => "clear",
        TileKind::Water => "water",
        TileKind::Road | TileKind::RoadDepot => "road",
        TileKind::Rail | TileKind::RailDepot => "railway",
        TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge => "tunnel_bridge",
        TileKind::House => "house",
        TileKind::Industry => "industry",
        TileKind::Station | TileKind::Airport => "station",
        TileKind::Forest | TileKind::CoalField => "trees",
        TileKind::ShipDepot => "water",
        TileKind::Unknown(_) => "object",
    }
}

fn openttd_tile_type(kind: TileKind) -> u8 {
    match kind {
        TileKind::Grass => 0,
        TileKind::Rail | TileKind::RailDepot => 1,
        TileKind::Road | TileKind::RoadDepot => 2,
        TileKind::House => 3,
        TileKind::Forest | TileKind::CoalField => 4,
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge => 9,
        TileKind::Unknown(_) => 10,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{TraceRegion, metadata_region, parse_region};
    use serde_json::json;

    #[test]
    fn missing_region_means_the_entire_map() {
        assert_eq!(
            TraceRegion::full(256, 128).as_bounds(),
            crate::render::TileViewportBounds::full(256, 128)
        );
        // Es la representación contractual que emite el oráculo C++.
        assert_eq!(metadata_region(None), json!(null));
    }

    #[test]
    fn requested_region_is_serialized_as_an_inclusive_rect() {
        assert_eq!(
            metadata_region(Some(TraceRegion {
                tx0: 4,
                ty0: 5,
                tx1: 9,
                ty1: 11,
            })),
            json!({"min_x": 4, "min_y": 5, "max_x": 8, "max_y": 10})
        );
    }

    #[test]
    fn parses_inclusive_region_and_clamps_its_end() {
        let region = parse_region("252,253,999,999", 256, 256).expect("valid region");
        assert_eq!(
            (region.tx0, region.ty0, region.tx1, region.ty1),
            (252, 253, 256, 256)
        );
    }

    #[test]
    fn rejects_invalid_or_outside_regions() {
        assert!(parse_region("2,1,0,3", 64, 64).is_err());
        assert!(parse_region("64,0,64,0", 64, 64).is_err());
        assert!(parse_region("one,two,three,four", 64, 64).is_err());
    }
}
