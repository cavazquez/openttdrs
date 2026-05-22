//! Sprites y clasificación de teselas `MP_STATION` (OpenTTD `station_map.h`).

use openttdrs_core::StopKind;

/// `StationType` en bits 3–6 de `m6` (`GetStationType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationTileClass {
    Rail,
    Airport,
    Truck,
    Bus,
    Other(u8),
}

/// Capa de sprite de estación de tren (offsets de `station_land.h`, 1 u OTTD ≈ 2 px).
#[derive(Debug, Clone, Copy)]
pub struct RailStationLayer {
    pub sprite_id: u32,
    pub dx: f32,
    pub dy: f32,
    pub w: f32,
    pub h: f32,
    pub z: f32,
}

/// Escala pantalla para `TILE_SEQ_*` (tesela OTTD 16×16 → rombo ~64×31).
const STATION_SEQ_UNIT: f32 = 2.0;

#[must_use]
pub fn station_type_from_m6(m6: u8) -> StationTileClass {
    match (m6 >> 3) & 0x0F {
        0 => StationTileClass::Rail,
        1 => StationTileClass::Airport,
        2 => StationTileClass::Truck,
        3 => StationTileClass::Bus,
        v => StationTileClass::Other(v),
    }
}

/// `StopKind` del simulador a partir de `m6` (`GetStationType`).
#[must_use]
pub fn stop_kind_from_m6(m6: u8) -> StopKind {
    match station_type_from_m6(m6) {
        StationTileClass::Bus => StopKind::BusStop,
        StationTileClass::Truck => StopKind::TruckStop,
        StationTileClass::Rail | StationTileClass::Airport | StationTileClass::Other(_) => {
            StopKind::RailStation
        }
    }
}

/// Clase visual: prioriza tipo en `m6` del tile (fixture/save); si no, `StopKind` del simulador.
#[must_use]
pub fn station_tile_class(m6: u8, stop_kind: Option<StopKind>) -> StationTileClass {
    match station_type_from_m6(m6) {
        StationTileClass::Rail | StationTileClass::Bus | StationTileClass::Truck => {
            station_type_from_m6(m6)
        }
        StationTileClass::Airport | StationTileClass::Other(_) => {
            if let Some(sk) = stop_kind {
                match sk {
                    StopKind::BusStop => StationTileClass::Bus,
                    StopKind::TruckStop => StationTileClass::Truck,
                    StopKind::RailStation => StationTileClass::Rail,
                }
            } else {
                StationTileClass::Truck
            }
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
#[must_use]
pub fn rail_station_ground_track_sprite(m5: u8) -> u32 {
    if rail_station_axis_y(m5) { 1011 } else { 1012 }
}

#[inline]
const fn layer(sprite_id: u32, dx: f32, dy: f32, w: f32, h: f32, z: f32) -> RailStationLayer {
    RailStationLayer {
        sprite_id,
        dx,
        dy,
        w,
        h,
        z,
    }
}

/// Convierte offsets `TILE_SEQ_LINE` de plataformas de tren a `xrel`/`yrel` para [`overlay_pos`].
#[must_use]
pub fn rail_station_overlay_rel(dx: f32, dy: f32) -> (f32, f32) {
    let xrel = 2.0 * (dy - dx) * STATION_SEQ_UNIT;
    let yrel = (dx + dy) * STATION_SEQ_UNIT;
    (xrel, yrel)
}

static RAIL_STATION_LAYERS_X: [RailStationLayer; 2] = [
    layer(1070, 0.0, 0.0, 42.0, 23.0, 0.03),
    layer(1072, 0.0, 11.0, 42.0, 23.0, 0.04),
];
static RAIL_STATION_LAYERS_Y: [RailStationLayer; 2] = [
    layer(1071, 0.0, 0.0, 42.0, 23.0, 0.03),
    layer(1069, 11.0, 0.0, 42.0, 23.0, 0.04),
];
static RAIL_STATION_LAYERS_X_BUILD: [RailStationLayer; 2] = [
    layer(1073, 0.0, 0.0, 42.0, 29.0, 0.03),
    layer(1072, 0.0, 11.0, 42.0, 23.0, 0.05),
];
static RAIL_STATION_LAYERS_Y_BUILD: [RailStationLayer; 2] = [
    layer(1074, 0.0, 0.0, 42.0, 29.0, 0.03),
    layer(1069, 11.0, 0.0, 42.0, 23.0, 0.05),
];

/// Capas en orden de pintado (tras la vía de fondo), según `station_land.h`.
#[must_use]
pub fn rail_station_draw_layers(m5: u8) -> &'static [RailStationLayer] {
    match rail_station_gfx(m5) {
        0 => &RAIL_STATION_LAYERS_X,
        1 => &RAIL_STATION_LAYERS_Y,
        2 => &RAIL_STATION_LAYERS_X_BUILD,
        3 => &RAIL_STATION_LAYERS_Y_BUILD,
        gfx if gfx & 1 != 0 => &RAIL_STATION_LAYERS_Y,
        _ => &RAIL_STATION_LAYERS_X,
    }
}

/// Sprites OpenGFX para estación de tren 1×1 (plataforma + edificio pequeño).
#[must_use]
#[allow(dead_code)]
pub fn rail_station_sprite_layers(axis_y: bool) -> (u32, u32) {
    if axis_y { (1069, 1074) } else { (1072, 1073) }
}

/// Índice 0..3 para `bus_stop_*` / `truck_stop_*` desde `m5` bajo (DiagDirection).
#[must_use]
pub fn road_stop_ground_index(m5: u8) -> usize {
    (m5 & 0x03) as usize
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/road_stop_gfx_data_generated.rs"
));

/// Convierte metadatos NFO de una capa BUILD a [`RoadStopSeqGfx`].
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
        StationTileClass::Rail | StationTileClass::Airport | StationTileClass::Other(_) => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m6_decodes_bus_truck_and_rail() {
        assert_eq!(station_type_from_m6(3 << 3), StationTileClass::Bus);
        assert_eq!(station_type_from_m6(2 << 3), StationTileClass::Truck);
        assert_eq!(station_type_from_m6(0), StationTileClass::Rail);
    }

    #[test]
    fn stop_kind_from_m6_matches_tile_class() {
        assert_eq!(stop_kind_from_m6(2 << 3), StopKind::TruckStop);
        assert_eq!(stop_kind_from_m6(3 << 3), StopKind::BusStop);
        assert_eq!(stop_kind_from_m6(0), StopKind::RailStation);
    }

    #[test]
    fn render_prefers_m6_rail_over_wrong_stop_kind() {
        assert_eq!(
            station_tile_class(0, Some(StopKind::TruckStop)),
            StationTileClass::Rail
        );
    }

    #[test]
    fn rail_layers_follow_gfx_and_include_building_variant() {
        assert_eq!(rail_station_draw_layers(0)[0].sprite_id, 1070);
        assert_eq!(rail_station_draw_layers(2)[0].sprite_id, 1073);
        assert_eq!(rail_station_ground_track_sprite(2), 1012);
        assert_eq!(rail_station_ground_track_sprite(3), 1011);
    }

    #[test]
    fn rail_front_platform_uses_ottd_dy_offset() {
        let (xrel, yrel) = rail_station_overlay_rel(0.0, 11.0);
        assert_eq!(xrel, 44.0);
        assert_eq!(yrel, 22.0);
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
}
