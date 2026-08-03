//! Acciones de ciudad (`town_cmd.cpp` / `CmdDoTownAction`).

use crate::GameState;
use crate::map::{TileCoord, TileKind, tile_slope_and_z};
use crate::town::{Town, update_town_radius};
use crate::town_action::{
    TownAction, TownActionError, TownAuthoritySettings, execute_town_action, mask_of_town_actions,
};
use crate::townname::generate_town_name;

use super::error::CommandError;

/// Coste de fundar un pueblo (`CmdBuildTown` simplificado).
pub const FOUND_TOWN_COST: i64 = 12_500;
/// Distancia mínima (manhattan) a otro pueblo.
pub const FOUND_TOWN_MIN_DISTANCE: i32 = 14;
/// Casas iniciales al fundar.
const FOUND_TOWN_HOUSE_COUNT: usize = 5;

/// Compat: publicidad mediana.
pub(crate) fn town_advertise(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    do_town_action(state, town_id, TownAction::AdvertiseMedium)
}

/// Compat: financiar edificios.
pub(crate) fn town_fund_buildings(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    do_town_action(state, town_id, TownAction::FundBuildings)
}

/// `CmdDoTownAction`.
pub(crate) fn do_town_action(
    state: &mut GameState,
    town_id: u32,
    action: TownAction,
) -> Result<(), CommandError> {
    let idx = town_index(state, town_id)?;
    let company = state.active_company;
    let settings = TownAuthoritySettings::default();
    let mask = mask_of_town_actions(&state.towns[idx], company, state.economy.money, settings);
    if mask & (1 << (action as u8)) == 0 {
        return Err(CommandError::TownActionNotAvailable);
    }
    let cost = action.cost();
    if state.economy.money < cost {
        return Err(CommandError::InsufficientFunds);
    }

    let bribe_fails = if action == TownAction::Bribe {
        // Chance16(1, 14).
        state.interactive_random.next().is_multiple_of(14)
    } else {
        false
    };

    state.economy.money -= cost;
    let result = execute_town_action(
        &mut state.towns[idx],
        &mut state.stations,
        &mut state.map,
        company,
        action,
        bribe_fails,
    );
    match result {
        Ok(suggested) => {
            if matches!(action, TownAction::BuildStatue | TownAction::RoadRebuild) {
                if action == TownAction::BuildStatue && suggested.is_none() {
                    state.economy.money += cost;
                    return Err(CommandError::StatueNoPlace);
                }
                if let Some(pos) = suggested {
                    mark_town_action_tiles_dirty(state, pos);
                }
            }
            if action == TownAction::FundBuildings {
                state.runtime.pending_sim_events.push(
                    crate::sim_events::SimEvent::TownRatingChanged {
                        town_id,
                        delta: crate::town::FUND_BUILDINGS_RATING_BOOST,
                    },
                );
            }
            Ok(())
        }
        Err(TownActionError::NotAvailable | TownActionError::AlreadyHasStatue) => {
            state.economy.money += cost;
            Err(CommandError::TownActionNotAvailable)
        }
        Err(TownActionError::NoStatuePlace) => {
            state.economy.money += cost;
            Err(CommandError::StatueNoPlace)
        }
    }
}

/// Las obras de autoridad no tienen una coordenada en `Command`, por lo que
/// marcan explícitamente la tesela modificada y sus vecinas para el remapeo
/// visual y para las conexiones de carretera.
fn mark_town_action_tiles_dirty(state: &mut GameState, at: TileCoord) {
    let (width, height) = state.map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return;
    };
    for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
        let pos = TileCoord::new(at.x + dx, at.y + dy);
        if pos.x >= 0
            && pos.y >= 0
            && pos.x < width
            && pos.y < height
            && !state.runtime.landscape_tile_dirty.contains(&pos)
        {
            state.runtime.landscape_tile_dirty.push(pos);
        }
    }
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::company::{CompanyId, OWNER_TOWN_M1};
    use crate::map::object_type_from_tile;
    use crate::station::{StopKind, station_rating_for_cargo};
    use crate::town_action::{
        ADVERTISE_MEDIUM_BOOST, BUILD_STATUE_AUTHORITY_RATING_BOOST, EXCLUSIVE_RIGHTS_MONTHS,
        ROAD_REBUILD_MONTHS,
    };
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
            station_before.saturating_add(ADVERTISE_MEDIUM_BOOST)
        );
    }

    #[test]
    fn eight_town_actions_have_distinct_costs() {
        let costs: Vec<_> = TownAction::all().map(TownAction::cost).to_vec();
        assert_eq!(costs.len(), 8);
        assert_eq!(costs[1], 1_000); // medium
        assert!(costs[0] < costs[1] && costs[1] < costs[2]);
        assert!(costs[6] > costs[5]); // rights > fund
    }

    #[test]
    fn same_town_action_has_same_canonical_hash_on_two_replicas() {
        let mut first = GameState::new(32, 32);
        first.economy.money = 100_000;
        first.towns.push(Town {
            id: 1,
            pos: TileCoord::new(8, 8),
            name: "Hashville".into(),
            ..Default::default()
        });
        let mut second = first.clone();
        let command = Command::DoTownAction {
            town_id: 1,
            action: TownAction::RoadRebuild,
        };

        apply_command(&mut first, &command).unwrap();
        apply_command(&mut second, &command).unwrap();

        assert_eq!(first.canonical_hash(), second.canonical_hash());
    }

    #[test]
    fn buy_rights_grants_twelve_month_exclusivity() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 100_000;
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(8, 8),
            name: "A".into(),
            ..Default::default()
        });
        apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::BuyRights,
            },
        )
        .unwrap();
        assert_eq!(s.towns[0].exclusive_counter, EXCLUSIVE_RIGHTS_MONTHS);
        assert_eq!(s.towns[0].exclusivity, Some(CompanyId::PLAYER));
    }

    #[test]
    fn exclusivity_filters_cargo_for_rival_company() {
        use crate::town::TOWN_PRODUCE_TICKS;
        use crate::town::produce_town_cargo;

        let mut s = GameState::new(16, 16);
        let town_pos = TileCoord::new(4, 4);
        s.towns.push(Town {
            id: 1,
            pos: town_pos,
            name: "X".into(),
            exclusive_counter: 12,
            exclusivity: Some(CompanyId::PLAYER),
            ..Default::default()
        });
        s.map
            .set_kind(TileCoord::new(4, 5), TileKind::House)
            .unwrap();

        let mut player_stop =
            crate::Station::new_with_kind(TileCoord::new(5, 5), StopKind::BusStop);
        player_stop.owner = CompanyId::PLAYER;
        player_stop.goods.get_mut(CargoType::Passengers).has_rating = true;
        player_stop.goods.get_mut(CargoType::Passengers).rating = 200;
        player_stop.goods.get_mut(CargoType::Passengers).last_speed = 1;

        let mut rival_stop = crate::Station::new_with_kind(TileCoord::new(3, 5), StopKind::BusStop);
        rival_stop.owner = CompanyId(1);
        rival_stop.goods.get_mut(CargoType::Passengers).has_rating = true;
        rival_stop.goods.get_mut(CargoType::Passengers).rating = 255;
        rival_stop.goods.get_mut(CargoType::Passengers).last_speed = 1;

        s.stations = vec![player_stop, rival_stop];
        let _ = produce_town_cargo(
            &s.map,
            &[],
            &mut s.stations,
            &s.towns,
            TOWN_PRODUCE_TICKS,
            true,
        );
        let player_waiting = s.stations[0].cargo_stock.get(CargoType::Passengers);
        let rival_waiting = s.stations[1].cargo_stock.get(CargoType::Passengers);
        assert!(player_waiting > 0, "dueño exclusivo recibe carga");
        assert_eq!(rival_waiting, 0, "rival excluido");
    }

    #[test]
    fn build_statue_only_once_per_company() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 200_000;
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(10, 10),
            name: "S".into(),
            ..Default::default()
        });
        apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::BuildStatue,
            },
        )
        .unwrap();
        assert!(s.towns[0].has_statue(CompanyId::PLAYER));
        let statue_tile = s.map.get(TileCoord::new(10, 10)).expect("statue tile");
        assert_eq!(
            object_type_from_tile(&statue_tile),
            Some(crate::OBJECT_TYPE_STATUE_COMPANY)
        );
        assert!(
            s.runtime
                .landscape_tile_dirty
                .contains(&TileCoord::new(10, 10))
        );
        let err = apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::BuildStatue,
            },
        )
        .unwrap_err();
        assert_eq!(err, CommandError::TownActionNotAvailable);
    }

    #[test]
    fn build_statue_rejects_when_no_clear_tile_found() {
        let mut s = GameState::new(12, 12);
        s.economy.money = 50_000;
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(6, 6),
            name: "S".into(),
            ..Default::default()
        });
        for dx in -4..=4 {
            for dy in -4..=4 {
                let c = TileCoord::new(6 + dx, 6 + dy);
                s.map.set_kind(c, TileKind::Road).unwrap();
            }
        }
        let err = apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::BuildStatue,
            },
        )
        .unwrap_err();
        assert_eq!(err, CommandError::StatueNoPlace);
    }

    #[test]
    fn build_statue_increases_authority_by_26() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 50_000;
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(10, 10),
            name: "S".into(),
            ..Default::default()
        });
        let before = s.towns[0].authority_rating(CompanyId::PLAYER);

        apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::BuildStatue,
            },
        )
        .unwrap();

        assert_eq!(
            s.towns[0].authority_rating(CompanyId::PLAYER),
            before + i16::from(BUILD_STATUE_AUTHORITY_RATING_BOOST)
        );
    }

    #[test]
    fn road_rebuild_places_a_municipal_road_immediately() {
        let mut s = GameState::new(32, 32);
        s.economy.money = 50_000;
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(16, 16),
            name: "Roads".into(),
            ..Default::default()
        });

        apply_command(
            &mut s,
            &Command::DoTownAction {
                town_id: 1,
                action: TownAction::RoadRebuild,
            },
        )
        .unwrap();

        assert_eq!(s.towns[0].road_build_months, ROAD_REBUILD_MONTHS);
        assert!(s.runtime.landscape_tile_dirty.iter().any(|&c| {
            s.map.get_kind(c) == Some(TileKind::Road)
                && s.map.get(c).is_some_and(|tile| tile.m1 == OWNER_TOWN_M1)
        }));
    }
}
