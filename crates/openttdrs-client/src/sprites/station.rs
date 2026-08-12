//! Sprites y clasificación de teselas `MP_STATION` (OpenTTD `station_map.h`).

use std::sync::{Mutex, OnceLock};

use openttdrs_core::{RailType, STATION_TYPE_DOCK, StopKind};

use super::rail::{
    MAGLEV_RAIL_SPRITE_OFFSET, MONO_RAIL_SPRITE_OFFSET, rail_sloped_track_sprite_id,
    remap_rail_sprite_id,
};

/// `StationType` en bits 3–6 de `m6` (`GetStationType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationTileClass {
    Rail,
    RailWaypoint,
    RoadWaypoint,
    Airport,
    Truck,
    Bus,
    Dock,
    Buoy,
    Other(u8),
}

/// Capa de sprite de estación de tren (`TILE_SEQ_LINE` de `station_land.h`).
///
/// `dx`/`dy`/`dz` son el origen del bounding box en unidades de mundo OTTD
/// (16 por tesela); la posición en pantalla sale de `remap_tile_offset` × 0.5
/// más los offsets NFO del sprite ([`rail_station_sprite_meta`]).
#[derive(Debug, Clone, Copy)]
pub struct RailStationLayer {
    pub sprite_id: u32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub z: f32,
}

/// Primer y último sprite de la disposición vanilla de estación ferroviaria.
///
/// OpenTTD coloca las tres familias contiguamente: rail/elrail en 1069–1086,
/// monorraíl en 1151–1168 (+82) y maglev en 1233–1250 (+164). No son
/// imágenes decorativas intercambiables: el desplazamiento se aplica a cada
/// capa de `DrawRailTileSeq`, además de la vía de suelo.
const RAIL_STATION_SPRITE_FIRST: u32 = 1069;
const RAIL_STATION_SPRITE_LAST: u32 = 1086;

/// Aplica `RailTypeInfo::GetRailtypeSpriteOffset()` a una capa vanilla de
/// estación ferroviaria.
#[must_use]
pub const fn rail_station_sprite_id_for_type(sprite_id: u32, rail_type: RailType) -> u32 {
    if sprite_id < RAIL_STATION_SPRITE_FIRST || sprite_id > RAIL_STATION_SPRITE_LAST {
        return sprite_id;
    }
    match rail_type {
        RailType::Monorail => sprite_id + MONO_RAIL_SPRITE_OFFSET,
        RailType::Maglev => sprite_id + MAGLEV_RAIL_SPRITE_OFFSET,
        RailType::Rail | RailType::Electric => sprite_id,
    }
}

/// ID rail/elrail que describe la geometría de una capa, incluso si el
/// sprite seleccionado es mono o maglev.
#[must_use]
pub const fn rail_station_sprite_base_id(sprite_id: u32) -> u32 {
    match sprite_id {
        1151..=1168 => sprite_id - MONO_RAIL_SPRITE_OFFSET,
        1233..=1250 => sprite_id - MAGLEV_RAIL_SPRITE_OFFSET,
        _ => sprite_id,
    }
}

/// Copia una capa de la tabla vanilla y selecciona su variante de red.
#[must_use]
pub const fn rail_station_layer_for_type(
    mut layer: RailStationLayer,
    rail_type: RailType,
) -> RailStationLayer {
    layer.sprite_id = rail_station_sprite_id_for_type(layer.sprite_id, rail_type);
    layer
}

/// Caja `TILE_SEQ` de cada capa vanilla de estación ferroviaria.
///
/// Es la geometría que OpenTTD entrega a `AddSortableSpriteToDraw` en
/// `station_land.h`; no deriva del tamaño del PNG.
#[must_use]
pub const fn rail_station_layer_bounds(sprite_id: u32) -> Option<(i32, i32, i32)> {
    match rail_station_sprite_base_id(sprite_id) {
        1069 | 1071 | 1075 | 1077 => Some((5, 16, 2)),
        1070 | 1072 | 1078 => Some((16, 5, 2)),
        1073 => Some((16, 5, 15)),
        1074 => Some((5, 16, 15)),
        1076 => Some((16, 5, 7)),
        1079..=1082 => Some((16, 16, 10)),
        _ => None,
    }
}

#[must_use]
pub fn station_type_from_m6(m6: u8) -> StationTileClass {
    match (m6 >> 3) & 0x0F {
        0 => StationTileClass::Rail,
        1 => StationTileClass::Airport,
        2 => StationTileClass::Truck,
        3 => StationTileClass::Bus,
        STATION_TYPE_DOCK => StationTileClass::Dock,
        6 => StationTileClass::Buoy,
        7 => StationTileClass::RailWaypoint,
        8 => StationTileClass::RoadWaypoint,
        v => StationTileClass::Other(v),
    }
}

/// Registra una vez por sesión cada tipo de estación sin renderer.
///
/// Es preferible mostrar una tesela de diagnóstico a degradarla a una parada
/// de bus/camión: esa sustitución disimula errores de importación y hace mucho
/// más difícil comparar una partida con OpenTTD.
pub fn log_unknown_station_type_once(m6: u8) {
    let StationTileClass::Other(station_type) = station_type_from_m6(m6) else {
        return;
    };
    static LOGGED_TYPES: OnceLock<Mutex<[bool; 16]>> = OnceLock::new();
    let Ok(mut logged) = LOGGED_TYPES.get_or_init(|| Mutex::new([false; 16])).lock() else {
        return;
    };
    let index = usize::from(station_type);
    if logged[index] {
        return;
    }
    logged[index] = true;
    bevy::log::warn!(
        "Estación OpenTTD sin renderer: StationType={station_type} (m6=0x{m6:02X}); se muestra marcador magenta"
    );
}

/// `StopKind` del simulador a partir de `m6` (`GetStationType`).
pub use openttdrs_core::stop_kind_from_m6;

/// Clase visual: prioriza `StopKind` del simulador y tipo en `m6` del tile.
#[must_use]
pub fn station_tile_class(m6: u8, stop_kind: Option<StopKind>) -> StationTileClass {
    let raw_class = station_type_from_m6(m6);
    if matches!(raw_class, StationTileClass::Other(_)) {
        return raw_class;
    }
    if stop_kind == Some(StopKind::RailWaypoint) {
        return StationTileClass::RailWaypoint;
    }
    if stop_kind == Some(StopKind::RoadWaypoint) {
        return StationTileClass::RoadWaypoint;
    }
    match raw_class {
        // `m6` es la fuente de verdad por tile. Una estación puede combinar
        // tren, buses y aeropuerto; su `StopKind` simplificado no debe
        // convertir los tiles de aeropuerto en una parada vial genérica.
        StationTileClass::Airport => StationTileClass::Airport,
        StationTileClass::Rail
        | StationTileClass::RailWaypoint
        | StationTileClass::RoadWaypoint
        | StationTileClass::Bus
        | StationTileClass::Truck
        | StationTileClass::Dock
        | StationTileClass::Buoy => station_type_from_m6(m6),
        StationTileClass::Other(_) => {
            unreachable!("se filtró antes el tipo de estación desconocido")
        }
    }
}

/// `StationGfx` en `m5` (bits bajos; bit 0 = eje Y, `GetRailStationAxis`).
#[must_use]
pub fn rail_station_gfx(m5: u8) -> u8 {
    m5 & 0x0F
}

/// Eje de plataforma (`HasBit(gfx,0) ? AXIS_Y : AXIS_X`, `GetRailStationAxis`).
#[must_use]
pub fn rail_station_axis_y(m5: u8) -> bool {
    rail_station_gfx(m5) & 1 != 0
}

/// Vía de fondo (`SPR_RAIL_TRACK_X` / `SPR_RAIL_TRACK_Y`) antes de las plataformas.
///
/// En pendiente OpenTTD usa el sprite inclinado (`DrawRailTile` / `_track_sloped_sprites`), sin
/// distinguir eje X/Y.
#[must_use]
pub fn rail_station_ground_track_sprite(m5: u8, tileh: u8) -> u32 {
    if tileh != 0
        && let Some(sid) = rail_sloped_track_sprite_id(tileh, false)
    {
        return sid;
    }
    if rail_station_axis_y(m5) { 1011 } else { 1012 }
}

/// Vía de fondo de estación para el tipo de red de la tesela.
///
/// La disposición vanilla entrega `SPR_RAIL_TRACK_*`; OpenTTD le suma el
/// mismo offset de railtype antes de dibujarla. En pendiente se conserva el
/// sprite de vía inclinada y se remapea de la misma forma.
#[must_use]
pub fn rail_station_ground_track_sprite_for_type(m5: u8, tileh: u8, rail_type: RailType) -> u32 {
    remap_rail_sprite_id(rail_station_ground_track_sprite(m5, tileh), rail_type)
}

#[inline]
const fn layer(sprite_id: u32, dx: f32, dy: f32, dz: f32, z: f32) -> RailStationLayer {
    RailStationLayer {
        sprite_id,
        dx,
        dy,
        dz,
        z,
    }
}

/// Metadata NFO (w, h, xrel, yrel) de un sprite de estación de tren.
#[must_use]
pub fn rail_station_sprite_meta(sprite_id: u32) -> Option<(f32, f32, f32, f32)> {
    RAIL_STATION_SPRITE_META
        .iter()
        .find(|(sid, ..)| *sid == sprite_id)
        .map(|&(_, w, h, xr, yr)| (w, h, xr, yr))
}

/// Metadata NFO para pintar un layer de waypoint.
///
/// Las mitades este reutilizan el xrel/yrel del ancla oeste (mismo parent Action1
/// con TILE_SEQ dy/dx); el tamaño `w`/`h` sigue siendo el del PNG este.
#[must_use]
pub fn rail_waypoint_layer_meta(sprite_id: u32) -> Option<(f32, f32, f32, f32)> {
    let (w, h, _, _) = rail_station_sprite_meta(sprite_id)?;
    let anchor = match sprite_id {
        4975 => 4974, // cuerpo X este ← ancla oeste
        4979 => 4978, // toldo X este ← toldo oeste
        4977 => 4976,
        4981 => 4980,
        other => other,
    };
    let (_, _, xr, yr) = rail_station_sprite_meta(anchor)?;
    Some((w, h, xr, yr))
}

/// `xrel`/`yrel` para `overlay_pos`: origen `TILE_SEQ` remapeado + offsets NFO.
#[must_use]
pub fn rail_station_overlay_rel(
    seq: &RailStationLayer,
    nfo_xrel: f32,
    nfo_yrel: f32,
) -> (f32, f32) {
    let off = crate::iso::remap_tile_offset(seq.dx, seq.dy, seq.dz) * 0.5;
    (off.x + nfo_xrel, nfo_yrel - off.y)
}

/// Centro Bevy de un poste (`overlay_pos` + `TILE_SEQ` de `station_land.h`).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn rail_waypoint_sprite_center(
    ref_pos: bevy::prelude::Vec2,
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
    seq: &RailStationLayer,
    nfo_xrel: f32,
    nfo_yrel: f32,
    w: f32,
    h: f32,
) -> bevy::prelude::Vec3 {
    let (xrel, yrel) = rail_station_overlay_rel(seq, nfo_xrel, nfo_yrel);
    crate::iso::overlay_pos(ref_pos, xrel, yrel, w, h, base_z, layer_z, tx, ty)
}

// Secuencias de `_station_display_datas_rail` (gfx 0..7). Cristal 1083–1086:
// alpha aproximado en el cliente (`PALETTE_TO_TRANSPARENT` → ~0.45).
static RAIL_STATION_SEQ_0: [RailStationLayer; 2] = [
    layer(1070, 0.0, 0.0, 0.0, 0.03),
    layer(1072, 0.0, 11.0, 0.0, 0.04),
];
static RAIL_STATION_SEQ_1: [RailStationLayer; 2] = [
    layer(1071, 0.0, 0.0, 0.0, 0.03),
    layer(1069, 11.0, 0.0, 0.0, 0.04),
];
static RAIL_STATION_SEQ_2: [RailStationLayer; 2] = [
    layer(1073, 0.0, 0.0, 0.0, 0.03),
    layer(1072, 0.0, 11.0, 0.0, 0.05),
];
static RAIL_STATION_SEQ_3: [RailStationLayer; 2] = [
    layer(1074, 0.0, 0.0, 0.0, 0.03),
    layer(1069, 11.0, 0.0, 0.0, 0.05),
];
static RAIL_STATION_SEQ_4: [RailStationLayer; 4] = [
    layer(1076, 0.0, 0.0, 0.0, 0.03),
    layer(1072, 0.0, 11.0, 0.0, 0.04),
    layer(1079, 0.0, 0.0, 16.0, 0.05),
    layer(1083, 0.0, 0.0, 16.0, 0.06),
];
static RAIL_STATION_SEQ_5: [RailStationLayer; 4] = [
    layer(1077, 0.0, 0.0, 0.0, 0.03),
    layer(1069, 11.0, 0.0, 0.0, 0.04),
    layer(1080, 0.0, 0.0, 16.0, 0.05),
    layer(1084, 0.0, 0.0, 16.0, 0.06),
];
static RAIL_STATION_SEQ_6: [RailStationLayer; 4] = [
    layer(1070, 0.0, 0.0, 0.0, 0.03),
    layer(1078, 0.0, 11.0, 0.0, 0.04),
    layer(1081, 0.0, 0.0, 16.0, 0.05),
    layer(1085, 0.0, 0.0, 16.0, 0.06),
];
static RAIL_STATION_SEQ_7: [RailStationLayer; 4] = [
    layer(1071, 0.0, 0.0, 0.0, 0.03),
    layer(1075, 11.0, 0.0, 0.0, 0.04),
    layer(1082, 0.0, 0.0, 16.0, 0.05),
    layer(1086, 0.0, 0.0, 16.0, 0.06),
];

/// Cristal de techo (`SPR_RAIL_ROOF_GLASS_*`): tint translúcido, sin company colour.
#[must_use]
pub const fn rail_station_roof_glass_sprite(sprite_id: u32) -> bool {
    matches!(rail_station_sprite_base_id(sprite_id), 1083..=1086)
}

/// Cuerpo ogfx2 19/20 + toldos CC 21/22 (eje X).
///
/// Prop 1A: parents en (0,0) y (0,13). Los PNG este (4975/4979) se dibujan con
/// el **mismo xrel/yrel** que el oeste (ver `rail_waypoint_sprite_meta`); el
/// desplazamiento lateral lo aporta solo TILE_SEQ dy=13. Usar xrel NFO −8 además
/// de dy=13 separaba las mitades; dy=0 + xrel distintos formaba una V (capturas).
static RAIL_WAYPOINT_SEQ_X: [RailStationLayer; 4] = [
    layer(4974, 0.0, 0.0, 0.0, 0.05),
    layer(4975, 0.0, 13.0, 0.0, 0.06),
    layer(4978, 0.0, 0.0, 0.0, 0.07),
    layer(4979, 0.0, 13.0, 0.0, 0.08),
];
/// Cuerpo 23/24 + toldos 25/26; parents (0,0) y (13,0).
static RAIL_WAYPOINT_SEQ_Y: [RailStationLayer; 4] = [
    layer(4976, 0.0, 0.0, 0.0, 0.05),
    layer(4977, 13.0, 0.0, 0.0, 0.06),
    layer(4980, 0.0, 0.0, 0.0, 0.07),
    layer(4981, 13.0, 0.0, 0.0, 0.08),
];

/// Capas de waypoint (ogfx2_stations: ground vía aparte + 4 child sprites).
#[must_use]
pub fn rail_waypoint_draw_layers(m5: u8) -> &'static [RailStationLayer] {
    if rail_station_axis_y(m5) {
        &RAIL_WAYPOINT_SEQ_Y
    } else {
        &RAIL_WAYPOINT_SEQ_X
    }
}

/// Capas en orden de pintado (tras la vía de fondo), según `station_land.h`.
#[must_use]
pub fn rail_station_draw_layers(m5: u8) -> &'static [RailStationLayer] {
    match rail_station_gfx(m5) {
        0 => &RAIL_STATION_SEQ_0,
        1 => &RAIL_STATION_SEQ_1,
        2 => &RAIL_STATION_SEQ_2,
        3 => &RAIL_STATION_SEQ_3,
        4 => &RAIL_STATION_SEQ_4,
        5 => &RAIL_STATION_SEQ_5,
        6 => &RAIL_STATION_SEQ_6,
        7 => &RAIL_STATION_SEQ_7,
        gfx if gfx & 1 != 0 => &RAIL_STATION_SEQ_1,
        _ => &RAIL_STATION_SEQ_0,
    }
}

/// Sprites OpenGFX para estación de tren 1×1 (plataforma + edificio pequeño).
#[must_use]
#[allow(dead_code)]
pub fn rail_station_sprite_layers(axis_y: bool) -> (u32, u32) {
    if axis_y { (1069, 1074) } else { (1072, 1073) }
}

/// Índice de suelo OpenGFX: bahía `m5` 0..3; drive-through 4/5 → eje como 0/1.
#[must_use]
pub fn road_stop_ground_index(m5: u8) -> usize {
    match m5 {
        4 => 0, // RSV_DRIVE_THROUGH_X
        5 => 1, // RSV_DRIVE_THROUGH_Y
        d => (d & 0x03) as usize,
    }
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/road_stop_gfx_data_generated.rs"
));

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/rail_station_draw_data_generated.rs"
));

/// Convierte metadatos NFO de una capa BUILD a `RoadStopSeqGfx`.
#[must_use]
pub fn road_stop_seq_gfx(layer: &RoadStopLayerGfx) -> crate::iso::RoadStopSeqGfx {
    crate::iso::RoadStopSeqGfx {
        dx: layer.dx,
        dy: layer.dy,
        dz: layer.dz,
        x_offs: layer.x_offs,
        y_offs: layer.y_offs,
        remap_x_adj: layer.remap_x_adj,
    }
}

/// Capas BUILD_A/B/C por dirección (0=NE … 3=NW), orden de pintado.
#[must_use]
pub fn road_stop_build_layers(class: StationTileClass, dir: usize) -> &'static [RoadStopLayerGfx] {
    let dir = dir.min(3);
    match class {
        StationTileClass::Bus => &BUS_STOP_BUILD_LAYERS[dir],
        StationTileClass::Truck => &TRUCK_STOP_BUILD_LAYERS[dir],
        StationTileClass::Rail
        | StationTileClass::RailWaypoint
        | StationTileClass::RoadWaypoint
        | StationTileClass::Airport
        | StationTileClass::Dock
        | StationTileClass::Buoy
        | StationTileClass::Other(_) => &[],
    }
}

/// Capas Action5 vanilla de una parada pasante (`m5` 4=X, 5=Y).
///
/// A diferencia de una bahía, la parada pasante se apoya en una carretera
/// pavimentada y solo superpone dos tiras angostas con la señalización. No se
/// debe degradar al suelo/edificio de las orientaciones 0/1.
#[must_use]
pub fn road_stop_drive_through_layers(
    class: StationTileClass,
    orientation: u8,
) -> &'static [RoadStopLayerGfx] {
    let axis = match orientation {
        openttdrs_core::RSV_DRIVE_THROUGH_X => 0,
        openttdrs_core::RSV_DRIVE_THROUGH_Y => 1,
        _ => return &[],
    };
    match class {
        StationTileClass::Bus => &BUS_STOP_DRIVE_THROUGH_LAYERS[axis],
        StationTileClass::Truck => &TRUCK_STOP_DRIVE_THROUGH_LAYERS[axis],
        StationTileClass::Rail
        | StationTileClass::RailWaypoint
        | StationTileClass::RoadWaypoint
        | StationTileClass::Airport
        | StationTileClass::Dock
        | StationTileClass::Buoy
        | StationTileClass::Other(_) => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m6_decodes_bus_truck_dock_rail_and_waypoint() {
        assert_eq!(station_type_from_m6(3 << 3), StationTileClass::Bus);
        assert_eq!(station_type_from_m6(2 << 3), StationTileClass::Truck);
        assert_eq!(
            station_type_from_m6(STATION_TYPE_DOCK << 3),
            StationTileClass::Dock
        );
        assert_eq!(station_type_from_m6(0), StationTileClass::Rail);
        assert_eq!(station_type_from_m6(7 << 3), StationTileClass::RailWaypoint);
    }

    #[test]
    fn stop_kind_from_m6_matches_tile_class() {
        assert_eq!(stop_kind_from_m6(2 << 3), StopKind::TruckStop);
        assert_eq!(stop_kind_from_m6(3 << 3), StopKind::BusStop);
        assert_eq!(stop_kind_from_m6(STATION_TYPE_DOCK << 3), StopKind::Dock);
        assert_eq!(stop_kind_from_m6(0), StopKind::RailStation);
        assert_eq!(stop_kind_from_m6(7 << 3), StopKind::RailWaypoint);
    }

    #[test]
    fn render_prefers_m6_rail_over_wrong_stop_kind() {
        assert_eq!(
            station_tile_class(0, Some(StopKind::TruckStop)),
            StationTileClass::Rail
        );
    }

    #[test]
    fn render_keeps_airport_m6_when_station_also_has_rail_or_bus() {
        assert_eq!(
            station_tile_class(1 << 3, Some(StopKind::RailStation)),
            StationTileClass::Airport
        );
        assert_eq!(station_tile_class(1 << 3, None), StationTileClass::Airport);
    }

    #[test]
    fn unsupported_m6_is_diagnostic_instead_of_bus_fallback() {
        assert_eq!(
            station_tile_class(4 << 3, Some(StopKind::BusStop)),
            StationTileClass::Other(4)
        );
    }

    #[test]
    fn rail_layers_follow_gfx_and_include_building_variant() {
        assert_eq!(rail_station_draw_layers(0)[0].sprite_id, 1070);
        assert_eq!(rail_station_draw_layers(2)[0].sprite_id, 1073);
        assert_eq!(rail_station_ground_track_sprite(2, 0), 1012);
        assert_eq!(rail_station_ground_track_sprite(3, 0), 1011);
    }

    #[test]
    fn rail_layers_gfx_4_to_7_include_roof_halves() {
        // `_station_display_datas_4..7`: pilares/plataforma + estructura + cristal.
        assert_eq!(rail_station_draw_layers(4)[0].sprite_id, 1076);
        assert_eq!(rail_station_draw_layers(4)[2].sprite_id, 1079);
        assert_eq!(rail_station_draw_layers(4)[3].sprite_id, 1083);
        assert_eq!(rail_station_draw_layers(5)[2].sprite_id, 1080);
        assert_eq!(rail_station_draw_layers(5)[3].sprite_id, 1084);
        assert_eq!(rail_station_draw_layers(6)[1].sprite_id, 1078);
        assert_eq!(rail_station_draw_layers(6)[2].sprite_id, 1081);
        assert_eq!(rail_station_draw_layers(6)[3].sprite_id, 1085);
        assert_eq!(rail_station_draw_layers(7)[2].sprite_id, 1082);
        assert_eq!(rail_station_draw_layers(7)[3].sprite_id, 1086);
        for gfx in 4..=7u8 {
            let layers = rail_station_draw_layers(gfx);
            assert_eq!(layers.len(), 4);
            assert_eq!(layers[2].dz, 16.0);
            assert_eq!(layers[3].dz, 16.0);
            assert!(rail_station_roof_glass_sprite(layers[3].sprite_id));
        }
    }

    #[test]
    fn rail_station_meta_covers_all_layer_sprites() {
        for gfx in 0..=7u8 {
            for l in rail_station_draw_layers(gfx) {
                assert!(
                    rail_station_sprite_meta(l.sprite_id).is_some(),
                    "sin meta NFO para sprite {}",
                    l.sprite_id
                );
            }
        }
    }

    #[test]
    fn rail_station_variants_follow_upstream_railtype_offsets() {
        // `DrawTile_Station` toma `GetRailtypeSpriteOffset()` (82 por
        // fallback railtype) y `DrawRailTileSeq` lo suma a cada parent/child.
        // El caso real Kale (226,42) es gfx=5 de monorail: 1077/1069/1080/
        // 1084 -> 1159/1151/1162/1166.
        assert_eq!(
            rail_station_draw_layers(5)
                .iter()
                .map(|layer| rail_station_layer_for_type(*layer, RailType::Monorail).sprite_id)
                .collect::<Vec<_>>(),
            vec![1159, 1151, 1162, 1166]
        );
        assert_eq!(
            rail_station_draw_layers(5)
                .iter()
                .map(|layer| rail_station_layer_for_type(*layer, RailType::Maglev).sprite_id)
                .collect::<Vec<_>>(),
            vec![1241, 1233, 1244, 1248]
        );
        assert_eq!(
            rail_station_ground_track_sprite_for_type(5, 0, RailType::Monorail),
            1093
        );
        assert_eq!(
            rail_station_ground_track_sprite_for_type(5, 0, RailType::Maglev),
            1175
        );
    }

    #[test]
    fn rail_station_typed_variants_keep_tile_seq_and_glass_contract() {
        for rail_type in [
            RailType::Rail,
            RailType::Electric,
            RailType::Monorail,
            RailType::Maglev,
        ] {
            for gfx in 0..=7u8 {
                for layer in rail_station_draw_layers(gfx) {
                    let typed = rail_station_layer_for_type(*layer, rail_type);
                    assert_eq!(
                        rail_station_sprite_base_id(typed.sprite_id),
                        layer.sprite_id,
                        "{rail_type:?}, gfx={gfx}"
                    );
                    assert_eq!(
                        rail_station_layer_bounds(typed.sprite_id),
                        rail_station_layer_bounds(layer.sprite_id),
                        "{rail_type:?}, gfx={gfx}, sprite={}",
                        typed.sprite_id
                    );
                    assert_eq!(
                        rail_station_roof_glass_sprite(typed.sprite_id),
                        rail_station_roof_glass_sprite(layer.sprite_id),
                        "{rail_type:?}, gfx={gfx}, sprite={}",
                        typed.sprite_id
                    );
                    assert!(
                        rail_station_sprite_meta(typed.sprite_id).is_some(),
                        "sin meta NFO para {rail_type:?}, gfx={gfx}, sprite={}",
                        typed.sprite_id
                    );
                }
            }
        }
    }

    #[test]
    fn rail_station_ground_track_uses_sloped_sprite_on_slope() {
        assert_eq!(rail_station_ground_track_sprite(0, 12), 1031);
        assert_eq!(rail_station_ground_track_sprite(1, 12), 1031);
        assert_eq!(rail_station_ground_track_sprite(3, 6), 1032);
    }

    #[test]
    fn vanilla_station_layer_bounds_match_tile_seq_contract() {
        assert_eq!(rail_station_layer_bounds(1070), Some((16, 5, 2)));
        assert_eq!(rail_station_layer_bounds(1076), Some((16, 5, 7)));
        assert_eq!(rail_station_layer_bounds(1077), Some((5, 16, 2)));
        assert_eq!(rail_station_layer_bounds(1073), Some((16, 5, 15)));
        assert_eq!(rail_station_layer_bounds(1079), Some((16, 16, 10)));
        assert_eq!(rail_station_layer_bounds(1083), None);
    }

    #[test]
    fn rail_front_platform_uses_ottd_dy_offset() {
        // dy = 11 → RemapCoords ×0.5: +22 px en x, +11 px hacia abajo.
        let seq = layer(1072, 0.0, 11.0, 0.0, 0.04);
        let (xrel, yrel) = rail_station_overlay_rel(&seq, -31.0, -3.0);
        assert_eq!(xrel, -9.0);
        assert_eq!(yrel, 8.0);
    }

    #[test]
    fn rail_roof_uses_dz_to_raise_sprite() {
        // dz = 16 levanta el techo 16 px (remap ×0.5).
        let seq = layer(1079, 0.0, 0.0, 16.0, 0.05);
        let (xrel, yrel) = rail_station_overlay_rel(&seq, -31.0, -5.0);
        assert_eq!(xrel, -31.0);
        assert_eq!(yrel, -21.0);
    }

    #[test]
    fn rail_waypoint_meta_covers_layer_sprites() {
        for axis_y in [false, true] {
            let m5 = u8::from(axis_y);
            for l in rail_waypoint_draw_layers(m5) {
                assert!(
                    rail_station_sprite_meta(l.sprite_id).is_some(),
                    "sin meta NFO para waypoint sprite {}",
                    l.sprite_id
                );
            }
        }
    }

    #[test]
    fn rail_waypoint_ogfx2_uses_tile_seq_with_shared_anchor() {
        let x = rail_waypoint_draw_layers(0);
        let y = rail_waypoint_draw_layers(1);
        assert_eq!(x.len(), 4, "cuerpo + toldos CC eje X");
        assert_eq!(y.len(), 4, "cuerpo + toldos CC eje Y");
        assert_eq!((x[0].dx, x[0].dy), (0.0, 0.0));
        assert_eq!((x[1].dx, x[1].dy), (0.0, 13.0));
        assert_eq!((y[1].dx, y[1].dy), (13.0, 0.0));
        let Some((_, _, xr_w, yr_w)) = rail_waypoint_layer_meta(4974) else {
            panic!("meta oeste 4974");
        };
        let Some((_, _, xr_e, yr_e)) = rail_waypoint_layer_meta(4975) else {
            panic!("meta este 4975");
        };
        assert_eq!((xr_w, yr_w), (xr_e, yr_e), "mitad este reusa ancla oeste");
        assert_eq!(
            x.iter().map(|l| l.sprite_id).collect::<Vec<_>>(),
            vec![4974, 4975, 4978, 4979]
        );
        assert_eq!(
            y.iter().map(|l| l.sprite_id).collect::<Vec<_>>(),
            vec![4976, 4977, 4980, 4981]
        );
    }

    #[test]
    fn rail_waypoint_halves_offset_by_tile_seq_dy13() {
        let origin = crate::iso::iso(3, 4);
        let Some((_, _, xr, yr)) = rail_waypoint_layer_meta(4974) else {
            panic!("meta 4974");
        };
        let p0 = rail_waypoint_sprite_center(
            origin,
            3,
            4,
            0,
            0.05,
            &layer(4974, 0.0, 0.0, 0.0, 0.05),
            xr,
            yr,
            40.0,
            29.0,
        );
        let p1 = rail_waypoint_sprite_center(
            origin,
            3,
            4,
            0,
            0.06,
            &layer(4975, 0.0, 13.0, 0.0, 0.06),
            xr,
            yr,
            40.0,
            29.0,
        );
        // remap×0.5(dy=13) → (+26, −13) en pantalla.
        assert!(p1.x > p0.x + 20.0, "este por dy=13 (Δx={})", p1.x - p0.x);
        assert!(
            p1.y < p0.y - 8.0,
            "este baja con dy=13 (Δy={})",
            p1.y - p0.y
        );
    }

    #[test]
    fn rail_waypoint_y_halves_offset_by_dx13() {
        let origin = crate::iso::iso(3, 4);
        let Some((_, _, xr, yr)) = rail_waypoint_layer_meta(4976) else {
            panic!("meta 4976");
        };
        let p0 = rail_waypoint_sprite_center(
            origin,
            3,
            4,
            0,
            0.05,
            &layer(4976, 0.0, 0.0, 0.0, 0.05),
            xr,
            yr,
            38.0,
            28.0,
        );
        let p1 = rail_waypoint_sprite_center(
            origin,
            3,
            4,
            0,
            0.06,
            &layer(4977, 13.0, 0.0, 0.0, 0.06),
            xr,
            yr,
            38.0,
            28.0,
        );
        assert!(
            p1.x < p0.x - 20.0,
            "eje Y: dx=13 a la izquierda (Δx={})",
            p1.x - p0.x
        );
        assert!(p1.y < p0.y - 8.0, "eje Y: dx=13 baja (Δy={})", p1.y - p0.y);
    }

    #[test]
    fn station_tile_class_prefers_stop_kind_for_waypoint() {
        assert_eq!(
            station_tile_class(0, Some(StopKind::RailWaypoint)),
            StationTileClass::RailWaypoint
        );
    }

    #[test]
    fn road_stop_build_layers_per_direction() {
        assert_eq!(road_stop_build_layers(StationTileClass::Bus, 0).len(), 3);
        assert_eq!(road_stop_build_layers(StationTileClass::Bus, 0)[0].dx, 2.0);
        assert_eq!(
            road_stop_build_layers(StationTileClass::Truck, 0)[0].dy,
            15.0
        );
    }

    #[test]
    fn drive_through_stops_have_their_own_two_layer_layout() {
        assert_eq!(
            road_stop_drive_through_layers(StationTileClass::Bus, 4).len(),
            2
        );
        assert_eq!(
            road_stop_drive_through_layers(StationTileClass::Truck, 5).len(),
            2
        );
        assert!(road_stop_drive_through_layers(StationTileClass::Bus, 0).is_empty());
    }

    #[test]
    fn bus_stop_build_layers_use_distinct_sprites_per_direction() {
        let mut paths = std::collections::HashSet::new();
        for dir in 0..4 {
            for layer in road_stop_build_layers(StationTileClass::Bus, dir) {
                assert!(paths.insert(layer.path), "duplicate bus BUILD path");
            }
        }
        assert_eq!(paths.len(), 12);
        for dir in 0..4 {
            assert!(
                road_stop_build_layers(StationTileClass::Bus, dir)[0]
                    .path
                    .contains("bus_stop")
            );
        }
    }
}
