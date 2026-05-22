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

/// Eje de plataforma (`HasBit(gfx,0) ? AXIS_Y : AXIS_X`, `GetRailStationAxis`).
#[must_use]
pub fn rail_station_axis_y(m5: u8) -> bool {
    m5 & 1 != 0
}

/// Sprites OpenGFX para estación de tren 1×1 (plataforma + edificio pequeño).
#[must_use]
#[allow(dead_code)] // usado en tests; el render usa `rail_station_draw_layers`
pub fn rail_station_sprite_layers(axis_y: bool) -> (u32, u32) {
    if axis_y {
        (1069, 1074) // platform_y_front, building_y
    } else {
        (1072, 1073) // platform_x_front, building_x
    }
}

/// Capas de dibujo en orden (tras hierba): trasera, frente, edificio.
/// El edificio tiene hueco transparente; sin plataforma trasera se ve el agua debajo.
#[must_use]
pub fn rail_station_draw_layers(axis_y: bool) -> &'static [(f32, u32)] {
    if axis_y {
        &[(0.01, 1071), (0.02, 1069), (0.06, 1074)]
    } else {
        &[(0.01, 1070), (0.02, 1072), (0.06, 1073)]
    }
}

/// Índice 0..3 para `bus_stop_*` / `truck_stop_*` desde `m5` bajo (DiagDirection).
#[must_use]
pub fn road_stop_ground_index(m5: u8) -> usize {
    (m5 & 0x03) as usize
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
    fn rail_layers_follow_axis_bit() {
        assert_eq!(rail_station_sprite_layers(true), (1069, 1074));
        assert_eq!(rail_station_sprite_layers(false), (1072, 1073));
    }

    #[test]
    fn rail_draw_layers_include_rear_platform() {
        assert_eq!(rail_station_draw_layers(true)[0].1, 1071);
        assert_eq!(rail_station_draw_layers(false)[0].1, 1070);
    }
}
