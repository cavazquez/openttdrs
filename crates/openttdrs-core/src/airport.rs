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

    /// Clasifica el `StationGfx` vanilla que `OpenTTD` persiste en `m5` para
    /// tiles `MP_STATION/Airport`.
    ///
    /// A diferencia de [`Self::from_m5`], esta tabla cubre los índices reales
    /// de `_station_display_datas_airport` (0..=73), incluidos los helipads
    /// embebidos de commuter/metropolitan/international.
    #[must_use]
    pub const fn from_station_gfx(gfx: u8) -> Self {
        match gfx {
            // Stands y jetways.
            3 | 25 | 26 => Self::Stand,
            // Taxiways / cruces de apron.
            4..=13 => Self::Taxiway,
            // Pistas grandes y chicas, con sus extremos/cercas.
            14..=18 | 40..=42 | 45..=46 | 49..=50 | 57..=60 => Self::Runway,
            // Terminales, concourse, pier y edificios bajos.
            19 | 21..=23 | 27..=28 | 33..=35 | 63..=64 | 69 => Self::Terminal,
            // Hangares grande/chico.
            24 | 43 => Self::Hangar,
            // Torre, radar y radio tower. Las banderas y las mitades de
            // apron son StationGfx distintos: no deben disparar la animación
            // genérica de una torre procedimental.
            20 | 31..=32 | 47 | 51..=52 => Self::Tower,
            // Helipuerto simple y helipads embebidos.
            44 | 53..=55 | 61 | 66..=68 => Self::Heliport,
            // Apron, grass y cercas restantes.
            _ => Self::Apron,
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

/// `StationGfx` que `OpenTTD` anima como radar de aeropuerto.
#[must_use]
pub const fn is_airport_radar_station_gfx(gfx: u8) -> bool {
    matches!(gfx, 31 | 51 | 52)
}

/// `StationGfx` que `OpenTTD` anima como manga de viento.
#[must_use]
pub const fn is_airport_flag_station_gfx(gfx: u8) -> bool {
    matches!(gfx, 39 | 73)
}

/// Cantidad de frames de la animación vanilla de un `StationGfx` airport.
///
/// Derivado de `_origin_airporttile_specs` (`airporttiles.h`): los radares
/// usan doce frames y las mangas de viento cuatro.
#[must_use]
pub const fn airport_station_gfx_animation_frames(gfx: u8) -> Option<u8> {
    if is_airport_radar_station_gfx(gfx) {
        Some(12)
    } else if is_airport_flag_station_gfx(gfx) {
        Some(4)
    } else {
        None
    }
}

#[cfg(test)]
mod station_gfx_tests {
    use super::{
        AirportPiece, airport_station_gfx_animation_frames, is_airport_flag_station_gfx,
        is_airport_radar_station_gfx,
    };

    #[test]
    fn classifies_all_vanilla_station_gfx_without_falling_back_to_heliport() {
        for gfx in 0..=73 {
            let piece = AirportPiece::from_station_gfx(gfx);
            if matches!(gfx, 44 | 53..=55 | 61 | 66..=68) {
                assert_eq!(piece, AirportPiece::Heliport, "gfx={gfx}");
            } else {
                assert_ne!(piece, AirportPiece::Heliport, "gfx={gfx}");
            }
        }
    }

    #[test]
    fn station_gfx_covers_runways_hangars_terminals_and_towers() {
        assert_eq!(AirportPiece::from_station_gfx(14), AirportPiece::Runway);
        assert_eq!(AirportPiece::from_station_gfx(24), AirportPiece::Hangar);
        assert_eq!(AirportPiece::from_station_gfx(33), AirportPiece::Terminal);
        assert_eq!(AirportPiece::from_station_gfx(51), AirportPiece::Tower);
        assert_eq!(AirportPiece::from_station_gfx(66), AirportPiece::Heliport);
        assert_eq!(AirportPiece::from_station_gfx(39), AirportPiece::Apron);
        assert_eq!(AirportPiece::from_station_gfx(72), AirportPiece::Apron);
    }

    #[test]
    fn station_gfx_animation_contract_matches_openttd_airporttiles_table() {
        for gfx in 0..=73 {
            let frames = airport_station_gfx_animation_frames(gfx);
            match gfx {
                31 | 51 | 52 => assert_eq!(frames, Some(12), "gfx={gfx}"),
                39 | 73 => assert_eq!(frames, Some(4), "gfx={gfx}"),
                _ => assert_eq!(frames, None, "gfx={gfx}"),
            }
        }
        assert!(is_airport_radar_station_gfx(31));
        assert!(is_airport_flag_station_gfx(39));
    }
}

/// Small airport: 4×3 (eje X) o 3×4 (eje Y).
pub const AIRPORT_SMALL_W: i32 = 4;
pub const AIRPORT_SMALL_H: i32 = 3;

const HELIPORT_LAYOUT: &[AirportPiece] = &[AirportPiece::Heliport];

/// Helidepot 2×2 (`_tile_table_helidepot_0`): edificio + hangar + helipad + apron.
const HELIDEPOT_LAYOUT: &[AirportPiece] = &[
    AirportPiece::Apron,  // (0,0) APT_LOW_BUILDING
    AirportPiece::Hangar, // (1,0) APT_DEPOT_SE
    AirportPiece::Stand,  // (0,1) APT_HELIPAD_*
    AirportPiece::Apron,  // (1,1) APT_APRON
];

/// Helistation 4×2 (`_tile_table_helistation_0`): hangar + edificio + 3 helipads + apron.
const HELISTATION_LAYOUT: &[AirportPiece] = &[
    // y=0
    AirportPiece::Hangar, // (0,0) APT_DEPOT_SE
    AirportPiece::Apron,  // (1,0) APT_LOW_BUILDING
    AirportPiece::Stand,  // (2,0) APT_HELIPAD_3
    AirportPiece::Stand,  // (3,0) APT_HELIPAD_3
    // y=1
    AirportPiece::Apron, // (0,1)
    AirportPiece::Apron, // (1,1)
    AirportPiece::Apron, // (2,1)
    AirportPiece::Stand, // (3,1) APT_HELIPAD_3
];

/// Country / Small (`_tile_table_country_0`): edificios + césped + pista.
const SMALL_LAYOUT: &[AirportPiece] = &[
    // y=0
    AirportPiece::Terminal, // APT_SMALL_BUILDING_1
    AirportPiece::Terminal, // APT_SMALL_BUILDING_2
    AirportPiece::Tower,    // APT_SMALL_BUILDING_3
    AirportPiece::Hangar,   // APT_SMALL_DEPOT_SE
    // y=1
    AirportPiece::Apron, // grass / fence
    AirportPiece::Stand, // grass (terminal stands area)
    AirportPiece::Stand,
    AirportPiece::Apron,
    // y=2 runway
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
];

/// Commuter 5×4 (`_tile_table_commuter_0`): torre/edificio, 2 helipads, hangar, stands y pista.
const COMMUTER_LAYOUT: &[AirportPiece] = &[
    // y=0
    AirportPiece::Tower,
    AirportPiece::Terminal,
    AirportPiece::Stand, // helipad
    AirportPiece::Stand, // helipad
    AirportPiece::Hangar,
    // y=1 apron
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    AirportPiece::Apron,
    // y=2 stands
    AirportPiece::Apron,
    AirportPiece::Stand,
    AirportPiece::Stand,
    AirportPiece::Stand,
    AirportPiece::Apron,
    // y=3 runway
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
    AirportPiece::Runway,
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
        AirportSpecId::Heliport | AirportSpecId::Oilrig => HELIPORT_LAYOUT,
        AirportSpecId::Helidepot => HELIDEPOT_LAYOUT,
        AirportSpecId::Helistation => HELISTATION_LAYOUT,
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

/// Footprint `NewGRF` (`size` del layout; `axis_y` intercambia ejes).
#[must_use]
pub fn newgrf_airport_footprint(
    def: &crate::airport_class::NewgrfAirportSpecDef,
    axis_y: bool,
) -> (i32, i32) {
    if axis_y {
        (def.size_y, def.size_x)
    } else {
        (def.size_x, def.size_y)
    }
}

/// Itera (coord, pieza) del primer layout `NewGRF` usable.
///
/// Piezas se derivan del `subst` gfx de cada tile (`AirportPiece::from_station_gfx`).
/// FTA `NewGRF` queda fuera de alcance (#260): construcción usa subst visual.
#[must_use]
pub fn newgrf_airport_tiles(
    origin: TileCoord,
    def: &crate::airport_class::NewgrfAirportSpecDef,
    tile_catalog: &[crate::airport_tile_spec::AirportTileSpecDef],
    axis_y: bool,
) -> Vec<(TileCoord, AirportPiece)> {
    newgrf_airport_tile_gfx(origin, def, tile_catalog, axis_y)
        .into_iter()
        .map(|(coord, gfx)| {
            let piece = AirportPiece::from_station_gfx(
                crate::airport_tile_spec::resolve_airport_tile_piece_gfx(gfx, tile_catalog),
            );
            (coord, piece)
        })
        .collect()
}

/// Itera la huella de un aeropuerto `NewGRF` conservando el gfx custom de
/// `AirportTile` por tesela.
///
/// El mapa necesita el `subst` vanilla en `m5` para FTA y compatibilidad con
/// saves, pero el compositor debe poder recuperar el id global asignado por
/// `AirportTiles` para dibujar Action1/2. Mantener ambas representaciones
/// evita inferir el origen a partir del hangar (que puede estar en cualquier
/// coordenada del layout).
#[must_use]
pub fn newgrf_airport_tile_gfx(
    origin: TileCoord,
    def: &crate::airport_class::NewgrfAirportSpecDef,
    tile_catalog: &[crate::airport_tile_spec::AirportTileSpecDef],
    axis_y: bool,
) -> Vec<(TileCoord, u16)> {
    let layout = def
        .layouts
        .iter()
        .find(|l| {
            // Prefer north (0) or south (4); else first.
            l.rotation == 0 || l.rotation == 4
        })
        .or_else(|| def.layouts.first());
    let Some(layout) = layout else {
        return airport_spec_tiles(origin, def.subst_id, axis_y)
            .map(|(coord, piece)| (coord, u16::from(piece as u8)))
            .collect();
    };
    layout
        .tiles
        .iter()
        .map(|t| {
            let (dx, dy) = if axis_y {
                (i32::from(t.y), i32::from(t.x))
            } else {
                (i32::from(t.x), i32::from(t.y))
            };
            let gfx = if t.gfx < crate::airport_tile_spec::NEW_AIRPORT_TILE_OFFSET {
                t.gfx
            } else {
                // Keep the global id even when the catalog entry is not
                // currently installed; the renderer will use the vanilla
                // fallback instead of silently shifting the footprint.
                tile_catalog
                    .iter()
                    .find(|candidate| candidate.gfx.as_u16() == t.gfx)
                    .map_or(t.gfx, |candidate| candidate.gfx.as_u16())
            };
            (TileCoord::new(origin.x + dx, origin.y + dy), gfx)
        })
        .collect()
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
