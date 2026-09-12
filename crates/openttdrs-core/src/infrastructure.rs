//! Contadores efímeros de infraestructura derivados del mapa.
//!
//! `OpenTTD` no serializa `CompanyInfrastructure`: lo reconstruye al cargar un
//! save y lo mantiene actualizado mientras ejecuta comandos. El runtime
//! propio todavía no tiene el contador incremental en cada Company, por lo
//! que esta primera superficie calcula el contrato ferroviario desde el mapa.

use crate::bridge_spec::{bridge_line_tiles, rail_bridge_other_end};
use crate::company::CompanyId;
use crate::map::{
    Map, Tile, TileCoord, TileKind, diag_dir_offset, is_road_level_crossing,
    resolve_existing_tunnel_end,
};
use crate::rail_signals::{rail_signal_present_mask, rail_tile_is_signals, tracks_overlap};
use crate::rail_type::{RailType, rail_type_from_tile};
use crate::station::{is_rail_station_type, station_type_from_m6};

/// Cantidad de railtypes vanilla que puede tener una compañía.
pub const RAIL_INFRASTRUCTURE_RAILTYPE_COUNT: usize = 4;

/// Parte ferroviaria de `CompanyInfrastructure`.
///
/// Los valores son piezas de infraestructura, no cantidad de teselas. Una
/// tesela plana con cruce puede aportar varias piezas; un túnel o puente se
/// pondera cuatro veces por su longitud, igual que el contador nativo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RailInfrastructureSummary {
    /// Piezas por `RailType`: normal, eléctrica, monorail y maglev.
    pub rail: [u32; RAIL_INFRASTRUCTURE_RAILTYPE_COUNT],
    /// Señales presentes, no cantidad de teselas señalizadas.
    pub signals: u32,
}

impl RailInfrastructureSummary {
    /// Suma de piezas ferroviarias en todos los railtypes.
    #[must_use]
    pub fn rail_total(self) -> u32 {
        self.rail.iter().copied().sum()
    }

    /// Piezas de un railtype concreto.
    #[must_use]
    pub fn rail_type_count(self, rail_type: RailType) -> u32 {
        self.rail[usize::from(rail_type.as_u8())]
    }
}

const TUNNELBRIDGE_TRACKBIT_FACTOR: u32 = 4;
const LEVELCROSSING_TRACKBIT_FACTOR: u32 = 2;

fn tile_owned_by(tile: Tile, owner: CompanyId) -> bool {
    tile.m1 & 0x1F == owner.0 & 0x1F
}

fn add_rail_pieces(summary: &mut RailInfrastructureSummary, tile: Tile, pieces: u32) {
    let slot = &mut summary.rail[usize::from(rail_type_from_tile(tile).as_u8())];
    *slot = slot.saturating_add(pieces);
}

fn plain_rail_pieces(tile: Tile) -> u32 {
    // RailTileType::Normal es el único subtipo que cuenta sus TrackBits. Los
    // subtipos señales y depot aportan una pieza, aunque la máscara tenga más
    // de un bit.
    if (tile.m5 >> 6) & 0x03 != 0 {
        return 1;
    }
    let bits = tile.m5 & 0x3F;
    let pieces = bits.count_ones();
    if tracks_overlap(bits) {
        pieces.saturating_mul(pieces)
    } else {
        pieces
    }
}

fn same_tunnel_kind(map: &Map, coord: TileCoord, kind: TileKind) -> bool {
    map.get(coord)
        .is_some_and(|tile| tile.kind == kind && tile.is_tunnel_bridge_tile())
}

fn is_tunnel_entry(map: &Map, coord: TileCoord, tile: Tile) -> bool {
    let (dx, dy) = diag_dir_offset(tile.m5 & 0x03);
    let behind = TileCoord::new(coord.x - dx, coord.y - dy);
    !same_tunnel_kind(map, behind, TileKind::RailTunnel)
}

fn tunnel_end_for_count(map: &Map, start: TileCoord, tile: Tile) -> Option<TileCoord> {
    let (dx, dy) = diag_dir_offset(tile.m5 & 0x03);
    let (width, height) = map.dimensions();
    let mut contiguous_end = start;
    for _ in 0..width.max(height) {
        let next = TileCoord::new(contiguous_end.x + dx, contiguous_end.y + dy);
        if !same_tunnel_kind(map, next, TileKind::RailTunnel) {
            break;
        }
        contiguous_end = next;
    }
    if contiguous_end == start {
        // Los túneles importados normalmente sólo exponen las dos bocas en
        // MP_TUNNELBRIDGE; en ese formato no hay tramo contiguo que recorrer.
        resolve_existing_tunnel_end(map, start)
    } else {
        Some(contiguous_end)
    }
}

fn tunnel_span_len(start: TileCoord, end: TileCoord, direction: u8) -> Option<u32> {
    let (dx, dy) = diag_dir_offset(direction);
    let delta_x = end.x - start.x;
    let delta_y = end.y - start.y;
    let steps = if dx != 0 {
        if delta_y != 0 || delta_x.signum() != dx {
            return None;
        }
        delta_x.unsigned_abs()
    } else {
        if delta_x != 0 || delta_y.signum() != dy {
            return None;
        }
        delta_y.unsigned_abs()
    };
    (steps > 0).then(|| steps.saturating_add(1))
}

fn is_before_in_map_order(a: TileCoord, b: TileCoord) -> bool {
    // Map::tiles() y-major order es el mismo orden lineal que usa TileIndex
    // para elegir el extremo norte de un túnel/puente.
    (a.y, a.x) < (b.y, b.x)
}

fn add_rail_tunnel(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    summary: &mut RailInfrastructureSummary,
) {
    let Some(end) = tunnel_end_for_count(map, coord, tile) else {
        return;
    };
    if !is_before_in_map_order(coord, end) || !is_tunnel_entry(map, coord, tile) {
        return;
    }
    let Some(length) = tunnel_span_len(coord, end, tile.m5 & 0x03) else {
        return;
    };
    add_rail_pieces(
        summary,
        tile,
        length.saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR),
    );
}

fn add_rail_bridge(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    summary: &mut RailInfrastructureSummary,
) {
    let Some(end) = rail_bridge_other_end(map, coord) else {
        return;
    };
    if !is_before_in_map_order(coord, end) {
        return;
    }
    let length = u32::try_from(bridge_line_tiles(coord, end).len()).unwrap_or(u32::MAX);
    add_rail_pieces(
        summary,
        tile,
        length.saturating_mul(TUNNELBRIDGE_TRACKBIT_FACTOR),
    );
}

/// Reconstruye la infraestructura ferroviaria de una compañía a partir del
/// mapa actual.
///
/// El recorrido cubre vía plana, señales, depósitos, estaciones/waypoints,
/// cruces a nivel y túneles/puentes. La parte de carretera, agua y aeropuertos
/// permanece fuera de esta primera superficie y se incorporará con sus
/// contratos de ownership propios.
#[must_use]
pub fn rail_infrastructure_for_company(map: &Map, owner: CompanyId) -> RailInfrastructureSummary {
    let (width, height) = map.dimensions();
    let mut summary = RailInfrastructureSummary::default();
    for y in 0..height {
        let Ok(y) = i32::try_from(y) else {
            continue;
        };
        for x in 0..width {
            let Ok(x) = i32::try_from(x) else {
                continue;
            };
            let coord = TileCoord::new(x, y);
            let Some(tile) = map.get(coord) else {
                continue;
            };
            if !tile_owned_by(tile, owner) {
                continue;
            }
            match tile.kind {
                TileKind::Rail => {
                    add_rail_pieces(&mut summary, tile, plain_rail_pieces(tile));
                    if rail_tile_is_signals(tile.m5) {
                        summary.signals = summary
                            .signals
                            .saturating_add(rail_signal_present_mask(tile.m3).count_ones());
                    }
                }
                TileKind::RailDepot => add_rail_pieces(&mut summary, tile, 1),
                TileKind::RailTunnel => add_rail_tunnel(map, coord, tile, &mut summary),
                TileKind::RailBridge => add_rail_bridge(map, coord, tile, &mut summary),
                TileKind::Station
                    if is_rail_station_type(station_type_from_m6(tile.m6)) && tile.m3 & 1 == 0 =>
                {
                    add_rail_pieces(&mut summary, tile, 1);
                }
                TileKind::Road if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) => {
                    add_rail_pieces(&mut summary, tile, LEVELCROSSING_TRACKBIT_FACTOR);
                }
                _ => {}
            }
        }
    }
    summary
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn tile(kind: TileKind, mapt: u8, m5: u8, owner: CompanyId, m3: u8, m6: u8, m8: u16) -> Tile {
        Tile {
            height: 1,
            kind,
            mapt,
            m5,
            m1: owner.0,
            m6,
            m8,
            m3,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn counts_owned_rail_pieces_by_type_and_signals() {
        let mut map = Map::new_flat(8, 4, 1);
        map.set_tile(
            TileCoord::new(1, 1),
            tile(TileKind::Rail, 0x10, 0x03, CompanyId::PLAYER, 0, 0, 0),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(2, 1),
            tile(
                TileKind::Rail,
                0x10,
                (1 << 6) | 0x03,
                CompanyId::PLAYER,
                0x30,
                0,
                0,
            ),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(3, 1),
            tile(TileKind::RailDepot, 0x10, 0, CompanyId::PLAYER, 0, 0, 2),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(4, 1),
            tile(TileKind::Station, 0x50, 0, CompanyId::PLAYER, 0, 0, 3),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(5, 1),
            tile(TileKind::Station, 0x50, 0, CompanyId::PLAYER, 1, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(6, 1),
            tile(TileKind::Road, 0x20, 1 << 6, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(1, 2),
            tile(TileKind::Rail, 0x10, 0x01, CompanyId(1), 0, 0, 0),
        )
        .unwrap();

        let summary = rail_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.rail_type_count(RailType::Rail), 5);
        assert_eq!(summary.rail_type_count(RailType::Electric), 2);
        assert_eq!(summary.rail_type_count(RailType::Monorail), 1);
        assert_eq!(summary.rail_type_count(RailType::Maglev), 1);
        assert_eq!(summary.rail_total(), 9);
        assert_eq!(summary.signals, 2);
    }

    #[test]
    fn counts_tunnel_and_bridge_once_with_native_factor() {
        let mut map = Map::new_flat(8, 4, 1);
        for x in 1..=5 {
            map.set_tile(
                TileCoord::new(x, 1),
                tile(
                    TileKind::RailTunnel,
                    0x90,
                    if x == 1 {
                        2
                    } else if x == 5 {
                        0
                    } else {
                        0
                    },
                    CompanyId::PLAYER,
                    0,
                    0,
                    1,
                ),
            )
            .unwrap();
        }
        map.set_tile(
            TileCoord::new(1, 2),
            tile(TileKind::RailBridge, 0x90, 0x82, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();
        map.set_tile(
            TileCoord::new(5, 2),
            tile(TileKind::RailBridge, 0x90, 0x80, CompanyId::PLAYER, 0, 0, 1),
        )
        .unwrap();

        let summary = rail_infrastructure_for_company(&map, CompanyId::PLAYER);
        assert_eq!(summary.rail_type_count(RailType::Electric), 40);
    }
}
