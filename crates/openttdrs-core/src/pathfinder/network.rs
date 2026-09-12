//! Topología multi-red y conectividad para pathfinding.

use std::collections::HashMap;

use crate::map::{
    Map, Tile, TileCoord, TileKind, diag_dir_offset, openttd_tile_index_to_coord,
    resolve_existing_tunnel_end,
};
use crate::ship_movement::{is_water_network_tile_at, water_tiles_connected};
use crate::tnbp_decode::JgrTunnelRecord;
use crate::vehicle::VehicleKind;

fn is_any_transport_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road
            | TileKind::Rail
            | TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge
    )
}

pub(super) fn is_road_stop_station(tile: &Tile) -> bool {
    tile.kind == TileKind::Station && matches!((tile.m6 >> 3) & 0x0F, 2 | 3 | 8)
}

pub(crate) fn is_rail_station_tile(tile: &Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

pub(super) fn is_network_tile(
    map: &Map,
    c: TileCoord,
    kind: TileKind,
    network: PathNetwork,
) -> bool {
    match network {
        PathNetwork::Road => is_road_network_tile(kind) || is_road_stop_station_tile(map, c),
        PathNetwork::Tram => is_tram_network_tile(map, c, kind),
        // Trenes no circulan por la tesela de plataforma; paran en la vía adyacente.
        PathNetwork::Rail => is_rail_network_tile(kind),
        PathNetwork::Water => is_water_network_tile_at(map, c),
        PathNetwork::Air => true,
    }
}

/// Red de transporte para pathfinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathNetwork {
    Road,
    /// Overlay de tranvía (`m3`); sin fallback a cruce completo.
    Tram,
    Rail,
    Water,
    Air,
}

#[must_use]
pub const fn path_network_for_vehicle(kind: VehicleKind) -> PathNetwork {
    match kind {
        VehicleKind::Train => PathNetwork::Rail,
        VehicleKind::Truck | VehicleKind::Bus => PathNetwork::Road,
        VehicleKind::Tram => PathNetwork::Tram,
        VehicleKind::Ship => PathNetwork::Water,
        VehicleKind::Aircraft => PathNetwork::Air,
    }
}

/// Misma condición que los tiles intermedios del pathfinder (carretera/vía/túnel/puente).
#[must_use]
pub fn tile_is_path_traversable(kind: TileKind) -> bool {
    is_any_transport_tile(kind)
}

#[must_use]
#[inline]
pub(crate) fn is_road_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
    )
}

/// Parada bus/camión con boca a carretera (`m3` = road bits de acceso).
#[must_use]
pub(super) fn is_road_stop_station_tile(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .is_some_and(|t| is_road_stop_station(&t) && (t.m3 & 0x0F) != 0)
}

/// Tesela de red de tranvía: overlay m3, depósito de carretera, o parada bus.
#[must_use]
fn is_tram_network_tile(map: &Map, c: TileCoord, kind: TileKind) -> bool {
    match kind {
        TileKind::RoadDepot => true,
        TileKind::Road | TileKind::RoadTunnel | TileKind::RoadBridge => map
            .get(c)
            .is_some_and(|t| crate::road_type::tram_track_bits(&t) != 0),
        TileKind::Station => is_road_stop_station_tile(map, c),
        _ => false,
    }
}

#[must_use]
#[inline]
pub(crate) fn is_rail_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    )
}

/// Portal opuesto de un túnel vanilla persistido en `MP_TUNNELBRIDGE`.
///
/// `OpenTTD` serializa sólo las dos bocas; el corredor intermedio conserva el
/// terreno que había debajo. Los mapas construidos localmente pueden, en
/// cambio, materializar cada tesela como `RoadTunnel`/`RailTunnel`. En ese
/// segundo formato el vecino inmediato en la dirección de `m5` identifica la
/// continuidad local y el A*/YAPF no debe saltar desde una tesela interior.
#[must_use]
pub(super) fn tunnel_other_end(map: &Map, c: TileCoord, kind: TileKind) -> Option<TileCoord> {
    let start = map.get(c)?;
    if start.kind != kind || !start.is_tunnel_bridge_tile() || start.m5 & 0x80 != 0 {
        return None;
    }
    let (dx, dy) = diag_dir_offset(start.m5 & 0x03);
    let next = TileCoord::new(c.x + dx, c.y + dy);
    if map.get_kind(next) == Some(kind) {
        return None;
    }
    let other = resolve_existing_tunnel_end(map, c)?;
    let end = map.get(other)?;
    (end.kind == kind
        && end.is_tunnel_bridge_tile()
        && end.m5 & 0x80 == 0
        && end.m5 & 0x0C == start.m5 & 0x0C)
        .then_some(other)
}

/// Enlaces «wormhole» entre entradas de túnel (p. ej. pool JGR `tile_n` ↔ `tile_s`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelWormholes {
    links: HashMap<TileCoord, TileCoord>,
}

impl TunnelWormholes {
    /// Construye enlaces bidireccionales desde registros JGR y dimensiones del mapa.
    #[must_use]
    pub fn from_jgr_records(map: &Map, records: &[JgrTunnelRecord]) -> Self {
        let (w, h) = map.dimensions();
        let mut links = HashMap::new();
        for r in records {
            if let (Some(a), Some(b)) = (
                openttd_tile_index_to_coord(r.tile_n, w, h),
                openttd_tile_index_to_coord(r.tile_s, w, h),
            ) {
                links.insert(a, b);
                links.insert(b, a);
            }
        }
        Self { links }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    #[must_use]
    pub fn other_end(&self, c: TileCoord) -> Option<TileCoord> {
        self.links.get(&c).copied()
    }
}

/// Bit de carretera que sale de `cur` hacia `next` (`road_bits_toward_neighbor` del cliente).
#[must_use]
const fn road_bits_toward_neighbor(dx: i32, dy: i32) -> u8 {
    match (dx, dy) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0x0F,
    }
}

#[must_use]
fn effective_road_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::RoadTunnel => {
            // El formato local conserva el corredor como una cadena de
            // teselas. Mantenerla recta evita que una boca vertical herede el
            // `m5` plano de una tesela interior.
            let horizontal = [(-1, 0), (1, 0)].into_iter().any(|(dx, dy)| {
                map.get_kind(TileCoord::new(c.x + dx, c.y + dy)) == Some(TileKind::RoadTunnel)
            });
            let vertical = [(0, -1), (0, 1)].into_iter().any(|(dx, dy)| {
                map.get_kind(TileCoord::new(c.x + dx, c.y + dy)) == Some(TileKind::RoadTunnel)
            });
            if horizontal && !vertical {
                0x0A
            } else if vertical && !horizontal {
                0x05
            } else {
                // En un mapa importado sólo existe la boca. `m5` apunta hacia
                // el interior; la carretera visible sale por la dirección
                // opuesta (GetAnyRoadBits/DiagDirToRoadBits).
                let (dx, dy) = diag_dir_offset(t.m5 & 0x03);
                road_bits_toward_neighbor(-dx, -dy)
            }
        }
        TileKind::RoadBridge => {
            // El vano de un puente no son teselas de carretera: sólo las dos
            // rampas son transitables y cada una admite su lado exterior.
            let (dx, dy) = diag_dir_offset(t.m5 & 0x03);
            road_bits_toward_neighbor(-dx, -dy)
        }
        TileKind::Road => {
            let bits = t.m5 & 0x0F;
            if bits == 0 { 0x0F } else { bits }
        }
        // m5 low bits = DiagDir de la boca (no road bits cardinales).
        TileKind::RoadDepot => match t.m5 & 0x03 {
            0 => 0x08, // boca oeste
            1 => 0x04, // boca sur
            2 => 0x02, // boca este
            _ => 0x01, // boca norte
        },
        TileKind::Station if is_road_stop_station(&t) => t.m3 & 0x0F,
        _ => 0,
    }
}

/// Bits de tranvía (`m3`); sin fallback `0x0F` — m3=0 no es transitable.
#[must_use]
fn effective_tram_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Road | TileKind::RoadTunnel | TileKind::RoadBridge => {
            crate::road_type::tram_track_bits(&t)
        }
        TileKind::RoadDepot => {
            let bits = crate::road_type::tram_track_bits(&t);
            if bits != 0 {
                bits
            } else {
                // Boca del depósito: si aún no hay overlay, usar bits de carretera (m5).
                let bits = t.m5 & 0x0F;
                if bits == 0 { 0x0F } else { bits }
            }
        }
        TileKind::Station if is_road_stop_station(&t) => t.m3 & 0x0F,
        _ => 0,
    }
}

#[must_use]
fn road_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    let exit = road_bits_toward_neighbor(dx, dy);
    let entry = road_bits_toward_neighbor(-dx, -dy);
    let cur_bits = effective_road_bits(map, cur);
    let next_bits = effective_road_bits(map, next);
    cur_bits & exit != 0 && next_bits & entry != 0
}

#[must_use]
fn tram_tiles_connected(map: &Map, cur: TileCoord, next: TileCoord) -> bool {
    let dx = next.x - cur.x;
    let dy = next.y - cur.y;
    if dx.abs() + dy.abs() != 1 {
        return false;
    }
    let exit = road_bits_toward_neighbor(dx, dy);
    let entry = road_bits_toward_neighbor(-dx, -dy);
    let cur_bits = effective_tram_bits(map, cur);
    let next_bits = effective_tram_bits(map, next);
    cur_bits & exit != 0 && next_bits & entry != 0
}

pub(super) fn tiles_connected(
    map: &Map,
    cur: TileCoord,
    next: TileCoord,
    network: PathNetwork,
) -> bool {
    match network {
        PathNetwork::Road => road_tiles_connected(map, cur, next),
        PathNetwork::Tram => tram_tiles_connected(map, cur, next),
        PathNetwork::Water => water_tiles_connected(map, cur, next),
        PathNetwork::Rail | PathNetwork::Air => {
            debug_assert!(false, "rail/air no usan tiles_connected genérico");
            false
        }
    }
}
