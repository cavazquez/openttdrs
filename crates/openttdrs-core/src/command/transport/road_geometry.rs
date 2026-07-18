//! Merge y propagación cardinal compartidos entre road (`m5`) y tram (`m3`).

use crate::GameState;
use crate::map::{Map, Tile, TileCoord, TileKind};

use super::super::{CommandError, tile_owner};

/// Enlaces cardinales: bit local → (dx, dy) → bit recíproco en el vecino.
pub(in crate::command::transport) const CARDINAL_LINK_TO_NEIGHBOR: [(u8, i32, i32, u8); 4] = [
    (8, -1, 0, 2), // NE → oeste recibe SW
    (1, 0, -1, 4), // NW → norte recibe SE
    (2, 1, 0, 8),  // SW → este recibe NE
    (4, 0, 1, 1),  // SE → sur recibe NW
];

/// Política de lectura/conexión sobre bits cardinales (road vs tram).
pub(in crate::command::transport) struct CardinalBitOverlay {
    pub neighbor_active: fn(&Map, TileCoord) -> bool,
    pub connect: fn(&Map, TileCoord) -> u8,
    pub read_bits: fn(&Tile) -> u8,
    /// Si es `true`, el merge aplica `& 0x0F` antes de `.max(1)` (tram).
    pub mask_nibble: bool,
}

fn road_stop_neighbor(map: &Map, n: TileCoord) -> bool {
    map.get(n)
        .is_some_and(|t| t.kind == TileKind::Station && (t.m3 & 0x0F) != 0)
}

fn road_neighbor_active(map: &Map, n: TileCoord) -> bool {
    matches!(
        map.get_kind(n),
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadDepot | TileKind::RoadTunnel)
    ) || road_stop_neighbor(map, n)
}

fn tram_neighbor_active(map: &Map, n: TileCoord) -> bool {
    map.get(n)
        .is_some_and(|t| t.kind == TileKind::Road && crate::road_type::tram_track_bits(&t) != 0)
}

fn road_tile_connects(map: &Map, n: TileCoord) -> bool {
    matches!(
        map.get_kind(n),
        Some(TileKind::Road | TileKind::RoadBridge | TileKind::RoadDepot | TileKind::RoadTunnel)
    ) || road_stop_neighbor(map, n)
}

fn road_connect(map: &Map, c: TileCoord) -> u8 {
    let mut bits = 0u8;
    let west = TileCoord::new(c.x - 1, c.y);
    let north = TileCoord::new(c.x, c.y - 1);
    let east = TileCoord::new(c.x + 1, c.y);
    let south = TileCoord::new(c.x, c.y + 1);
    if road_tile_connects(map, west) {
        bits |= 8;
    }
    if road_tile_connects(map, north) {
        bits |= 1;
    }
    if road_tile_connects(map, east) {
        bits |= 2;
    }
    if road_tile_connects(map, south) {
        bits |= 4;
    }
    bits
}

fn tram_connect(map: &Map, c: TileCoord) -> u8 {
    use crate::road_type::tram_track_bits;
    let mut bits = 0u8;
    let west = TileCoord::new(c.x - 1, c.y);
    let north = TileCoord::new(c.x, c.y - 1);
    let east = TileCoord::new(c.x + 1, c.y);
    let south = TileCoord::new(c.x, c.y + 1);
    if map.get_kind(west) == Some(TileKind::Road)
        && map.get(west).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 8;
    }
    if map.get_kind(north) == Some(TileKind::Road)
        && map.get(north).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 1;
    }
    if map.get_kind(east) == Some(TileKind::Road)
        && map.get(east).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 2;
    }
    if map.get_kind(south) == Some(TileKind::Road)
        && map.get(south).is_some_and(|t| tram_track_bits(&t) != 0)
    {
        bits |= 4;
    }
    bits
}

fn read_road_bits(t: &Tile) -> u8 {
    t.m5 & 0x0F
}

fn read_tram_bits(t: &Tile) -> u8 {
    t.m3 & 0x0F
}

pub(in crate::command::transport) const ROAD_OVERLAY: CardinalBitOverlay = CardinalBitOverlay {
    neighbor_active: road_neighbor_active,
    connect: road_connect,
    read_bits: read_road_bits,
    mask_nibble: false,
};

pub(in crate::command::transport) const TRAM_OVERLAY: CardinalBitOverlay = CardinalBitOverlay {
    neighbor_active: tram_neighbor_active,
    connect: tram_connect,
    read_bits: read_tram_bits,
    mask_nibble: true,
};

pub(in crate::command::transport) fn merge_cardinal_bits_with_neighbors(
    map: &Map,
    c: TileCoord,
    requested: u8,
    existing: u8,
    force_axis: bool,
    overlay: &CardinalBitOverlay,
) -> u8 {
    let has_w = (overlay.neighbor_active)(map, TileCoord::new(c.x - 1, c.y));
    let has_e = (overlay.neighbor_active)(map, TileCoord::new(c.x + 1, c.y));
    let has_n = (overlay.neighbor_active)(map, TileCoord::new(c.x, c.y - 1));
    let has_s = (overlay.neighbor_active)(map, TileCoord::new(c.x, c.y + 1));
    let connect = (overlay.connect)(map, c);
    let axis_h = has_w || has_e;
    let axis_v = has_n || has_s;
    let straight = requested == 0x0A || requested == 0x05;
    // Solo bits del eje pedido (0x0A = E–O, 0x05 = N–S).
    let same_axis_connect = connect & requested;

    // Reforzar/arrastrar un eje recto no debe inventar el perpendicular (#181).
    let bits = if straight && (force_axis || (axis_h && axis_v)) {
        existing | requested | same_axis_connect
    } else if axis_h && !axis_v {
        if existing & 0x05 == 0x05 && existing & 0x0A == 0 {
            connect | 0x05
        } else {
            connect | 0x0A
        }
    } else if axis_v && !axis_h {
        if existing & 0x0A == 0x0A && existing & 0x05 == 0 {
            connect | 0x0A
        } else {
            connect | 0x05
        }
    } else if requested.is_power_of_two() {
        // Propagate cardinal: añadir el stub sin OR-ear el eje cruzado (#181).
        let axis_mask = if requested & 0x0A != 0 { 0x0A } else { 0x05 };
        existing | requested | (connect & axis_mask)
    } else {
        existing | requested | connect
    };
    if overlay.mask_nibble {
        (bits & 0x0F).max(1)
    } else {
        bits.max(1)
    }
}

pub(in crate::command::transport) fn propagate_cardinal_bits_to_neighbors(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
    overlay: &CardinalBitOverlay,
    write: fn(&mut GameState, TileCoord, u8) -> Result<(), CommandError>,
    check_owner: bool,
) -> Result<(), CommandError> {
    for &(bit, dx, dy, reciproc) in &CARDINAL_LINK_TO_NEIGHBOR {
        if bits & bit == 0 {
            continue;
        }
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if state.map.get_kind(n) != Some(TileKind::Road) {
            continue;
        }
        if check_owner && tile_owner(state, n).is_some_and(|o| o != state.active_company) {
            continue;
        }
        let existing = state.map.get(n).map_or(0, |t| (overlay.read_bits)(&t));
        let merged =
            merge_cardinal_bits_with_neighbors(&state.map, n, reciproc, existing, false, overlay);
        if merged != existing {
            write(state, n, merged)?;
        }
    }
    Ok(())
}
