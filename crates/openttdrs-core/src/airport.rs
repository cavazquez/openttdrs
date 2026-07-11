//! Tipos y helpers de aeropuerto (piezas, footprint).

use crate::airport_class::{AirportSpecId, airport_spec_def};
use crate::map::{Map, TileCoord};
use crate::station::Station;

/// Pieza de aeropuerto en `m5` (helipuerto = 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AirportPiece {
    Heliport = 0,
    Hangar = 1,
    Apron = 2,
    Terminal = 3,
    Runway = 4,
    Taxiway = 5,
    Tower = 6,
    Stand = 7,
}

impl AirportPiece {
    #[must_use]
    pub fn from_m5(m5: u8) -> Self {
        match m5 {
            1 => Self::Hangar,
            2 => Self::Apron,
            3 => Self::Terminal,
            4 => Self::Runway,
            5 => Self::Taxiway,
            6 => Self::Tower,
            7 => Self::Stand,
            _ => Self::Heliport,
        }
    }

    #[must_use]
    pub const fn is_hangar(self) -> bool {
        matches!(self, Self::Hangar | Self::Heliport)
    }

    #[must_use]
    pub const fn is_loading(self) -> bool {
        matches!(
            self,
            Self::Apron | Self::Terminal | Self::Stand | Self::Heliport
        )
    }

    #[must_use]
    pub const fn is_runway(self) -> bool {
        matches!(self, Self::Runway)
    }
}

/// Small airport: 4×3 (eje X) o 3×4 (eje Y).
pub const AIRPORT_SMALL_W: i32 = 4;
pub const AIRPORT_SMALL_H: i32 = 3;

const HELIPORT_LAYOUT: &[AirportPiece] = &[AirportPiece::Heliport];

const HELIDEPOT_LAYOUT: &[AirportPiece] = &[
    AirportPiece::Hangar,
    AirportPiece::Apron,
    AirportPiece::Taxiway,
    AirportPiece::Stand,
];

const SMALL_LAYOUT: &[AirportPiece] = &[
    AirportPiece::Hangar,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    AirportPiece::Tower,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Taxiway,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Taxiway,
];

const COMMUTER_LAYOUT: &[AirportPiece] = &[
    AirportPiece::Hangar,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    AirportPiece::Tower,
    AirportPiece::Apron,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Taxiway,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Taxiway,
];

/// City 6×6 (inspirado en `_tile_table_city_0`).
const CITY_LAYOUT: &[AirportPiece] = &[
    // y=0
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Hangar,
    // y=1
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=2
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=3
    AirportPiece::Tower,
    AirportPiece::Apron,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Apron,
    AirportPiece::Tower,
    // y=4
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=5
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
];

/// Metropolitan 6×6 con doble pista.
const METROPOLITAN_LAYOUT: &[AirportPiece] = &[
    // y=0
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Hangar,
    // y=1
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=2
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=3
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Tower,
    // y=4
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    // y=5
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
];

/// International 7×7 con pistas N/S y helipads.
const INTERNATIONAL_LAYOUT: &[AirportPiece] = &[
    // y=0 pista norte
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    // y=1
    AirportPiece::Tower,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Hangar,
    // y=2
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=3
    AirportPiece::Hangar,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Heliport,
    // y=4
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Tower,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Heliport,
    // y=5
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=6 pista sur
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
];

/// Intercontinental 9×11 (doble pista + terminal central).
const INTERCONTINENTAL_LAYOUT: &[AirportPiece] = &[
    // y=0 pista norte
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    // y=1
    AirportPiece::Tower,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Hangar,
    // y=2
    AirportPiece::Hangar,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Terminal,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Hangar,
    // y=3
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    // y=4
    AirportPiece::Terminal,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Tower,
    AirportPiece::Stand,
    AirportPiece::Tower,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Terminal,
    // y=5
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=6
    AirportPiece::Heliport,
    AirportPiece::Apron,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Taxiway,
    AirportPiece::Apron,
    AirportPiece::Heliport,
    // y=7
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=8
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=9 pista sur A
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    // y=10 pista sur B
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
];

fn layout_for_spec(spec: AirportSpecId) -> &'static [AirportPiece] {
    match spec {
        AirportSpecId::Heliport => HELIPORT_LAYOUT,
        AirportSpecId::Helidepot => HELIDEPOT_LAYOUT,
        AirportSpecId::Small => SMALL_LAYOUT,
        AirportSpecId::Commuter => COMMUTER_LAYOUT,
        AirportSpecId::City => CITY_LAYOUT,
        AirportSpecId::Metropolitan => METROPOLITAN_LAYOUT,
        AirportSpecId::International => INTERNATIONAL_LAYOUT,
        AirportSpecId::Intercontinental => INTERCONTINENTAL_LAYOUT,
    }
}

/// Footprint en teselas según orientación (`axis_y` intercambia X/Y).
#[must_use]
pub fn airport_spec_footprint(spec: AirportSpecId, axis_y: bool) -> (i32, i32) {
    let Some(def) = airport_spec_def(spec) else {
        return (1, 1);
    };
    if axis_y {
        (def.size_y, def.size_x)
    } else {
        (def.size_x, def.size_y)
    }
}

/// Itera (coord, pieza) del footprint del spec.
pub fn airport_spec_tiles(
    origin: TileCoord,
    spec: AirportSpecId,
    axis_y: bool,
) -> impl Iterator<Item = (TileCoord, AirportPiece)> {
    let def = airport_spec_def(spec);
    let (base_w, base_h) = def.map_or((1, 1), |d| (d.size_x, d.size_y));
    let layout = layout_for_spec(spec);
    let (w, h) = if axis_y {
        (base_h, base_w)
    } else {
        (base_w, base_h)
    };
    (0..h).flat_map(move |row| {
        (0..w).map(move |col| {
            let (piece_row, piece_col) = if axis_y {
                (
                    usize::try_from(col).unwrap_or(0),
                    usize::try_from(row).unwrap_or(0),
                )
            } else {
                (
                    usize::try_from(row).unwrap_or(0),
                    usize::try_from(col).unwrap_or(0),
                )
            };
            let idx = piece_row * usize::try_from(base_w).unwrap_or(1) + piece_col;
            let piece = layout.get(idx).copied().unwrap_or(AirportPiece::Heliport);
            (TileCoord::new(origin.x + col, origin.y + row), piece)
        })
    })
}

#[must_use]
pub fn airport_small_footprint(axis_y: bool) -> (i32, i32) {
    airport_spec_footprint(AirportSpecId::Small, axis_y)
}

/// Itera (coord, pieza) del footprint small.
pub fn airport_small_tiles(
    origin: TileCoord,
    axis_y: bool,
) -> impl Iterator<Item = (TileCoord, AirportPiece)> {
    airport_spec_tiles(origin, AirportSpecId::Small, axis_y)
}

#[must_use]
pub fn airport_m6_airport(m6: u8) -> u8 {
    (m6 & !0x78) | (1 << 3)
}

/// ¿La tesela es hangar (compra de aviones)?
#[must_use]
pub fn airport_tile_is_hangar(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .filter(|t| t.kind == crate::map::TileKind::Airport)
        .is_some_and(|t| AirportPiece::from_m5(t.m5).is_hangar())
}

/// ¿La tesela es helipuerto 1×1 (compra de helicópteros)?
#[must_use]
pub fn airport_tile_is_heliport(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .filter(|t| t.kind == crate::map::TileKind::Airport)
        .is_some_and(|t| AirportPiece::from_m5(t.m5) == AirportPiece::Heliport)
}

/// Primera pista del footprint (para despegue/aterrizaje).
#[must_use]
pub fn airport_runway_tile(station: &Station, map: &Map) -> Option<TileCoord> {
    station
        .airport_tiles
        .iter()
        .copied()
        .find(|&c| {
            map.get(c)
                .is_some_and(|t| AirportPiece::from_m5(t.m5).is_runway())
        })
        .or(if station.airport_tiles.len() <= 1 {
            Some(station.pos)
        } else {
            None
        })
}

/// Tesela de carga (apron/terminal/stand) o hangar/helipuerto.
#[must_use]
pub fn airport_loading_tile(station: &Station, map: &Map) -> TileCoord {
    station
        .airport_tiles
        .iter()
        .copied()
        .find(|&c| {
            map.get(c)
                .is_some_and(|t| AirportPiece::from_m5(t.m5).is_loading())
        })
        .unwrap_or(station.pos)
}

/// Busca apron/loading en el blob de aeropuerto alrededor de `anchor` (sin lista de estaciones).
#[must_use]
pub fn airport_loading_tile_at(map: &Map, anchor: TileCoord) -> TileCoord {
    let Some(tile) = map.get(anchor) else {
        return anchor;
    };
    if tile.kind != crate::map::TileKind::Airport {
        return anchor;
    }
    if AirportPiece::from_m5(tile.m5) == AirportPiece::Heliport {
        return anchor;
    }
    // Buscar loading en un radio amplio (cubre hubs 9×11).
    for dy in -5i32..=10 {
        for dx in -5i32..=10 {
            let c = TileCoord::new(anchor.x + dx, anchor.y + dy);
            if map.get(c).is_some_and(|t| {
                t.kind == crate::map::TileKind::Airport
                    && AirportPiece::from_m5(t.m5).is_loading()
                    && !AirportPiece::from_m5(t.m5).is_hangar()
            }) {
                return c;
            }
        }
    }
    anchor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_footprint_matches_legacy() {
        assert_eq!(airport_spec_footprint(AirportSpecId::Small, false), (4, 3));
        assert_eq!(airport_spec_footprint(AirportSpecId::Small, true), (3, 4));
    }

    #[test]
    fn heliport_is_single_tile() {
        let tiles: Vec<_> =
            airport_spec_tiles(TileCoord::new(1, 1), AirportSpecId::Heliport, false).collect();
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].1, AirportPiece::Heliport);
    }

    #[test]
    fn commuter_tile_count() {
        let n = airport_spec_tiles(TileCoord::new(0, 0), AirportSpecId::Commuter, false).count();
        assert_eq!(n, 20);
    }

    #[test]
    fn city_and_metropolitan_are_6x6() {
        assert_eq!(
            airport_spec_tiles(TileCoord::new(0, 0), AirportSpecId::City, false).count(),
            36
        );
        assert_eq!(
            airport_spec_tiles(TileCoord::new(0, 0), AirportSpecId::Metropolitan, false).count(),
            36
        );
        assert!(
            airport_spec_tiles(TileCoord::new(0, 0), AirportSpecId::Metropolitan, false)
                .filter(|(_, p)| *p == AirportPiece::Runway)
                .count()
                >= 12
        );
    }

    #[test]
    fn international_is_7x7_with_hangar() {
        let tiles: Vec<_> =
            airport_spec_tiles(TileCoord::new(2, 2), AirportSpecId::International, false).collect();
        assert_eq!(tiles.len(), 49);
        assert!(tiles.iter().any(|(_, p)| *p == AirportPiece::Hangar));
        assert_eq!(
            airport_spec_footprint(AirportSpecId::International, true),
            (7, 7)
        );
    }

    #[test]
    fn intercontinental_is_9x11() {
        assert_eq!(
            airport_spec_footprint(AirportSpecId::Intercontinental, false),
            (9, 11)
        );
        assert_eq!(
            airport_spec_tiles(TileCoord::new(0, 0), AirportSpecId::Intercontinental, false)
                .count(),
            99
        );
    }
}
