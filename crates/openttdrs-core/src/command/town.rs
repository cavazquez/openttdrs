//! Acciones de ciudad (`town_cmd.cpp` simplificado).

use crate::GameState;
use crate::map::{TileCoord, TileKind, tile_slope_and_z};
use crate::station::{
    TOWN_ADVERTISE_MEDIUM_RADIUS, TOWN_ADVERTISE_MEDIUM_RATING_BOOST, modify_station_rating_around,
};
use crate::town::{
    FUND_BUILDINGS_COST, FUND_BUILDINGS_RATING_BOOST, TOWN_ADVERTISE_COST, Town, update_town_radius,
};
use crate::townname::generate_town_name;

use super::error::CommandError;

/// Coste de fundar un pueblo (`CmdBuildTown` simplificado).
pub const FOUND_TOWN_COST: i64 = 12_500;
/// Distancia mínima (manhattan) a otro pueblo.
pub const FOUND_TOWN_MIN_DISTANCE: i32 = 14;
/// Casas iniciales al fundar.
const FOUND_TOWN_HOUSE_COUNT: usize = 5;

pub(crate) fn town_advertise(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    let idx = town_index(state, town_id)?;
    if state.economy.money < TOWN_ADVERTISE_COST {
        return Err(CommandError::InsufficientFunds);
    }
    state.economy.money -= TOWN_ADVERTISE_COST;
    let center = state.towns[idx].pos;
    let owner = state.active_company;
    let _ = modify_station_rating_around(
        &mut state.stations,
        center,
        owner,
        TOWN_ADVERTISE_MEDIUM_RADIUS,
        TOWN_ADVERTISE_MEDIUM_RATING_BOOST,
    );
    Ok(())
}

pub(crate) fn town_fund_buildings(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    let idx = town_index(state, town_id)?;
    if state.economy.money < FUND_BUILDINGS_COST {
        return Err(CommandError::InsufficientFunds);
    }
    state.economy.money -= FUND_BUILDINGS_COST;
    let owner = state.active_company;
    let delta = state.towns[idx].adjust_rating(owner, FUND_BUILDINGS_RATING_BOOST);
    crate::town::apply_fund_buildings_boost(&mut state.towns[idx]);
    state
        .runtime
        .pending_sim_events
        .push(crate::sim_events::SimEvent::TownRatingChanged { town_id, delta });
    Ok(())
}

/// Funda un pueblo en hierba plana (`CmdBuildTown` MVP).
pub(crate) fn found_town(state: &mut GameState, center: TileCoord) -> Result<(), CommandError> {
    check_found_town(state, center)?;
    if state.economy.money < FOUND_TOWN_COST {
        return Err(CommandError::InsufficientFunds);
    }

    let road_bits: u8 = 0x0A; // eje X
    let mut roads = Vec::new();
    let mut houses = Vec::new();
    for dx in -2..=2 {
        roads.push(TileCoord::new(center.x + dx, center.y));
        for row in [-1_i32, 1] {
            houses.push(TileCoord::new(center.x + dx, center.y + row));
        }
    }

    for &c in &roads {
        if state.map.get_kind(c) != Some(TileKind::Grass) {
            return Err(CommandError::CannotFoundTownHere);
        }
        if tile_slope_and_z(&state.map, c).is_none_or(|(h, _)| h != 0) {
            return Err(CommandError::CannotFoundTownHere);
        }
    }

    state.economy.money -= FOUND_TOWN_COST;

    for &c in &roads {
        if let Err(e) = super::transport::write_normal_road_tile(state, c, road_bits) {
            state.economy.money += FOUND_TOWN_COST;
            return Err(e);
        }
    }

    let mut placed = 0usize;
    for &c in &houses {
        if placed >= FOUND_TOWN_HOUSE_COUNT {
            break;
        }
        if state.map.get_kind(c) != Some(TileKind::Grass) {
            continue;
        }
        if state.map.set_completed_house(c, 1, 20).is_ok() {
            placed += 1;
        }
    }

    let town_id = state
        .towns
        .iter()
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let seed = state
        .world_seed
        .wrapping_add(u64::from(town_id).wrapping_mul(0x9E37_79B9))
        .wrapping_add(u64::from(center.x.cast_unsigned()) << 16)
        .wrapping_add(u64::from(center.y.cast_unsigned()));
    let name_seed = u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0);
    let name = generate_town_name(4, name_seed)
        .unwrap_or_else(|| format!("Pueblo {},{}", center.x, center.y));
    let mut town = Town {
        id: town_id,
        pos: TileCoord::new(center.x, center.y.saturating_sub(1)),
        name,
        population: u32::try_from(placed.saturating_mul(8)).unwrap_or(8),
        num_houses: u16::try_from(placed).unwrap_or(0),
        ..Default::default()
    };
    town.initialize_layout(None);
    town.init_growth_goals(state.climate);
    town.init_grow_counter();
    update_town_radius(&mut town);
    state.towns.push(town);
    Ok(())
}

/// Validación de fundación (preview / comando).
pub(crate) fn check_found_town(state: &GameState, center: TileCoord) -> Result<(), CommandError> {
    if state.map.get_kind(center) != Some(TileKind::Grass) {
        return Err(CommandError::CannotFoundTownHere);
    }
    if tile_slope_and_z(&state.map, center).is_none_or(|(h, _)| h != 0) {
        return Err(CommandError::CannotFoundTownHere);
    }
    for t in &state.towns {
        let dx = (t.pos.x - center.x).abs();
        let dy = (t.pos.y - center.y).abs();
        if dx + dy < FOUND_TOWN_MIN_DISTANCE {
            return Err(CommandError::TownTooClose);
        }
    }
    Ok(())
}

fn town_index(state: &GameState, town_id: u32) -> Result<usize, CommandError> {
    state
        .towns
        .iter()
        .position(|t| t.id == town_id)
        .ok_or(CommandError::TownNotFound)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::station::{StopKind, station_rating_for_cargo};
    use crate::{Command, GameState, apply_command, map::TileCoord};

    #[test]
    fn found_town_places_roads_houses_and_entity() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 100_000;
        apply_command(&mut s, &Command::FoundTown(TileCoord::new(16, 16))).unwrap();
        assert_eq!(s.towns.len(), 1);
        assert_eq!(s.map.get_kind(TileCoord::new(16, 16)), Some(TileKind::Road));
        assert!(
            s.map.get_kind(TileCoord::new(16, 15)) == Some(TileKind::House)
                || s.map.get_kind(TileCoord::new(16, 17)) == Some(TileKind::House)
        );
        assert!(s.economy.money < 100_000);
    }

    #[test]
    fn found_town_rejects_nearby() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 100_000;
        apply_command(&mut s, &Command::FoundTown(TileCoord::new(10, 10))).unwrap();
        let err = apply_command(&mut s, &Command::FoundTown(TileCoord::new(16, 10))).unwrap_err();
        assert_eq!(err, CommandError::TownTooClose);
    }

    #[test]
    fn town_advertise_boosts_nearby_station_rating_not_authority() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 10_000;
        let town_pos = TileCoord::new(10, 10);
        s.towns.push(Town {
            id: 1,
            pos: town_pos,
            name: "Testville".to_string(),
            ..Default::default()
        });
        let stop = TileCoord::new(12, 10);
        let mut st = crate::Station::new_with_kind(stop, StopKind::BusStop);
        st.goods.get_mut(CargoType::Passengers).has_rating = true;
        st.goods.get_mut(CargoType::Passengers).rating = 100;
        s.stations.push(st);

        let authority_before = s.towns[0].authority_rating(s.active_company);
        let station_before = station_rating_for_cargo(&s.stations[0], CargoType::Passengers);

        apply_command(&mut s, &Command::TownAdvertise(1)).unwrap();

        assert_eq!(
            s.towns[0].authority_rating(s.active_company),
            authority_before
        );
        assert_eq!(
            station_rating_for_cargo(&s.stations[0], CargoType::Passengers),
            station_before.saturating_add(crate::station::TOWN_ADVERTISE_MEDIUM_RATING_BOOST)
        );
    }
}
