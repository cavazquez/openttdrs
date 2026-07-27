//! Ascensor de Large Office (`AnimateTile_Town` / `town_map.h`).

use std::collections::HashSet;

use crate::cargodist::parity::Randomizer;
use crate::house_spec::{BUILDING_FLAG_IS_ANIMATED, HouseSpec};

use super::{Map, Tile, TileCoord, TileKind};

pub const LIFT_MAX_POSITION: u8 = 36;
const LIFT_DESTINATION_FLOORS: u8 = 7;
const LIFT_STEPS_PER_FLOOR: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiftStep {
    Idle,
    Moving,
    Arrived,
}

#[must_use]
pub const fn lift_has_destination(tile: Tile) -> bool {
    tile.m7 & 1 != 0
}

#[must_use]
pub const fn lift_destination(tile: Tile) -> u8 {
    (tile.m7 >> 1) & 0x07
}

#[must_use]
pub const fn lift_position(tile: Tile) -> u8 {
    (tile.m6 >> 2) & 0x3F
}

#[must_use]
pub const fn with_lift_destination(mut tile: Tile, destination: u8) -> Tile {
    tile.m7 = (tile.m7 & !0x0F) | 1 | ((destination & 0x07) << 1);
    tile
}

#[must_use]
pub const fn with_lift_position(mut tile: Tile, position: u8) -> Tile {
    let clamped = if position > LIFT_MAX_POSITION {
        LIFT_MAX_POSITION
    } else {
        position
    };
    tile.m6 = (tile.m6 & 0x03) | ((clamped & 0x3F) << 2);
    tile
}

#[must_use]
pub const fn halt_lift(mut tile: Tile) -> Tile {
    tile.m7 &= !0x0F;
    tile
}

#[must_use]
pub fn house_tile_has_lift(tile: Tile) -> bool {
    if tile.kind != TileKind::House || tile.m3 & 0x80 == 0 {
        return false;
    }
    HouseSpec::get(tile.m8 & 0x0FFF)
        .is_some_and(|spec| spec.building_flags & BUILDING_FLAG_IS_ANIMATED != 0)
}

/// Un paso de `AnimateTile_Town`; el destino ya debe estar asignado.
pub fn advance_house_lift(tile: &mut Tile) -> LiftStep {
    if !house_tile_has_lift(*tile) || !lift_has_destination(*tile) {
        return LiftStep::Idle;
    }
    let destination = lift_destination(*tile) * LIFT_STEPS_PER_FLOOR;
    let position = lift_position(*tile);
    let next = if position < destination {
        position + 1
    } else {
        position.saturating_sub(1)
    };
    *tile = with_lift_position(*tile, next);
    if next == destination {
        *tile = halt_lift(*tile);
        LiftStep::Arrived
    } else {
        LiftStep::Moving
    }
}

fn choose_lift_destination(position: u8, rng: &mut Randomizer) -> u8 {
    loop {
        let destination =
            u8::try_from(rng.random_range(u32::from(LIFT_DESTINATION_FLOORS))).unwrap_or(0);
        if destination != 1 && destination * LIFT_STEPS_PER_FLOOR != position {
            return destination;
        }
    }
}

/// Activa ascensores desde las visitas de `TileLoop_Town` y avanza solo los
/// activos, sin barrer todo el mapa cada cuatro ticks.
pub fn step_house_lifts<S: std::hash::BuildHasher>(
    map: &mut Map,
    tick: u64,
    visits: &[(TileCoord, Tile)],
    rng: &mut Randomizer,
    active: &mut HashSet<TileCoord, S>,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &(coord, _) in visits {
        let Some(mut tile) = map.get(coord) else {
            continue;
        };
        if !house_tile_has_lift(tile) {
            active.remove(&coord);
            continue;
        }
        if lift_has_destination(tile) {
            active.insert(coord);
        } else if rng.random_range(2) == 0 {
            tile = with_lift_destination(tile, choose_lift_destination(lift_position(tile), rng));
            let _ = map.set_tile(coord, tile);
            active.insert(coord);
            dirty.push(coord);
        }
    }

    if tick & 3 != 0 {
        return dirty;
    }
    let mut coords: Vec<_> = active.iter().copied().collect();
    coords.sort_by_key(|coord| (coord.y, coord.x));
    for coord in coords {
        let Some(mut tile) = map.get(coord) else {
            active.remove(&coord);
            continue;
        };
        let step = advance_house_lift(&mut tile);
        if step == LiftStep::Idle {
            active.remove(&coord);
            continue;
        }
        let _ = map.set_tile(coord, tile);
        dirty.push(coord);
        if step == LiftStep::Arrived {
            active.remove(&coord);
        }
    }
    dirty.sort_unstable_by_key(|coord| (coord.y, coord.x));
    dirty.dedup();
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn large_office() -> Tile {
        Tile::completed_house(4, 0, 0)
    }

    #[test]
    fn lift_moves_up_one_position_towards_destination() {
        let mut tile = with_lift_destination(large_office(), 2);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Moving);
        assert_eq!(lift_position(tile), 1);
        assert_eq!(lift_destination(tile), 2);
    }

    #[test]
    fn lift_moves_down_one_position_towards_destination() {
        let mut tile = with_lift_position(large_office(), 18);
        tile = with_lift_destination(tile, 0);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Moving);
        assert_eq!(lift_position(tile), 17);
    }

    #[test]
    fn lift_halts_exactly_at_destination() {
        let mut tile = with_lift_position(large_office(), 11);
        tile = with_lift_destination(tile, 2);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Arrived);
        assert_eq!(lift_position(tile), 12);
        assert!(!lift_has_destination(tile));
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Idle);
    }
}
