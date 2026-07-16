//! Métricas del consist: capacidad, peso, potencia, longitud en teselas.

use crate::cargo::CargoType;
use crate::engine::engine_by_id;
use crate::map::TileCoord;
use crate::vehicle::Vehicle;

use super::topology::consist_unit_ids;

pub(crate) fn cargo_unit_weight_16ths(cargo: Option<CargoType>) -> u8 {
    match cargo {
        Some(CargoType::Passengers) => 1,
        Some(CargoType::Mail | CargoType::Valuables) => 4,
        Some(CargoType::Goods | CargoType::Livestock) => 8,
        Some(
            CargoType::Coal
            | CargoType::Wood
            | CargoType::Oil
            | CargoType::Grain
            | CargoType::IronOre
            | CargoType::Steel,
        ) => 16,
        None => 0,
    }
}

/// Capacidad total del consist (cabeza).
#[must_use]
pub fn consist_capacity(vehicles: &[Vehicle], head_id: u32) -> u32 {
    vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(0, |v| v.capacity)
}

/// Peso total (t) del consist.
#[must_use]
pub fn consist_weight_t(vehicles: &[Vehicle], head_id: u32) -> u16 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.weight_t))
        .fold(0_u16, u16::saturating_add)
}

/// Potencia total (HP) del consist.
#[must_use]
pub fn consist_power_hp(vehicles: &[Vehicle], head_id: u32) -> u32 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.power_hp))
        .fold(0_u32, u32::saturating_add)
}

/// Número de teselas que ocupa el consist (redondeo hacia arriba).
#[must_use]
pub fn consist_tile_span(vehicles: &[Vehicle], head_id: u32) -> u32 {
    let len = vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(u16::from(super::VEHICLE_LENGTH), |v| v.cached_total_length);
    u32::from(len)
        .div_ceil(u32::from(super::TILE_FRACTIONS))
        .max(1)
}

/// Teselas ocupadas por el consist: cabeza + cola.
///
/// Preferencia: historial real de la cabeza (`rail_tile_history`); si falta,
/// vecinos en sentido opuesto a la dirección (MVP).
#[must_use]
pub fn consist_occupied_tiles(vehicles: &[Vehicle], head_id: u32) -> Vec<TileCoord> {
    let Some(head) = vehicles.iter().find(|v| v.id == head_id) else {
        return Vec::new();
    };
    let span = consist_tile_span(vehicles, head_id) as usize;
    let mut tiles = vec![head.pos];
    if span <= 1 {
        return tiles;
    }
    // Historial: teselas que la cabeza acaba de abandonar (frente = más reciente).
    for &t in head.rail_tile_history.iter().take(span.saturating_sub(1)) {
        if tiles.last() != Some(&t) {
            tiles.push(t);
        }
        if tiles.len() >= span {
            return tiles;
        }
    }
    let back = opposite_diag(head.direction);
    let mut cur = *tiles.last().unwrap_or(&head.pos);
    while tiles.len() < span {
        let next = offset_tile(cur, back);
        tiles.push(next);
        cur = next;
    }
    tiles
}

fn opposite_diag(dir: u8) -> u8 {
    dir.wrapping_add(4) % 8
}

fn offset_tile(c: TileCoord, dir: u8) -> TileCoord {
    let (dx, dy) = match dir {
        0 => (0, -1),  // N
        1 => (1, -1),  // NE
        2 => (1, 0),   // E
        3 => (1, 1),   // SE
        4 => (0, 1),   // S
        5 => (-1, 1),  // SW
        6 => (-1, 0),  // W
        _ => (-1, -1), // NW
    };
    TileCoord::new(c.x + dx, c.y + dy)
}
