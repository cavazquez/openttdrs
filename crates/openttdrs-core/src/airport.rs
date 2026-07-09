//! Tipos y helpers de aeropuerto (piezas, footprint small).

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

/// Layout row-major (fila 0 = norte del footprint en eje X).
const SMALL_LAYOUT: [[AirportPiece; 4]; 3] = [
    [
        AirportPiece::Hangar,
        AirportPiece::Apron,
        AirportPiece::Terminal,
        AirportPiece::Tower,
    ],
    [
        AirportPiece::Runway,
        AirportPiece::Runway,
        AirportPiece::Runway,
        AirportPiece::Runway,
    ],
    [
        AirportPiece::Taxiway,
        AirportPiece::Apron,
        AirportPiece::Stand,
        AirportPiece::Taxiway,
    ],
];

#[must_use]
pub fn airport_small_footprint(axis_y: bool) -> (i32, i32) {
    if axis_y {
        (AIRPORT_SMALL_H, AIRPORT_SMALL_W)
    } else {
        (AIRPORT_SMALL_W, AIRPORT_SMALL_H)
    }
}

/// Itera (coord, pieza) del footprint small.
pub fn airport_small_tiles(
    origin: TileCoord,
    axis_y: bool,
) -> impl Iterator<Item = (TileCoord, AirportPiece)> {
    let (w, h) = airport_small_footprint(axis_y);
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
            let piece = SMALL_LAYOUT[piece_row][piece_col];
            (TileCoord::new(origin.x + col, origin.y + row), piece)
        })
    })
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
    // Helipuerto.
    if AirportPiece::from_m5(tile.m5) == AirportPiece::Heliport {
        return anchor;
    }
    // Buscar loading en un radio 4×4 alrededor del ancla.
    for dy in -2i32..=3 {
        for dx in -2i32..=3 {
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
