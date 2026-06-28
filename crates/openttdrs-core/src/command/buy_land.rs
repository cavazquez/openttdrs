//! Comprar terreno (`OBJECT_OWNED_LAND`), al estilo `object_cmd.cpp`.
#![allow(clippy::cast_possible_wrap)]

use crate::economy::buy_land_cost;
use crate::game_state::GameState;
use crate::map::{
    MP_OBJECT_MAPT, Map, OBJECT_TYPE_OWNED_LAND, TileCoord, TileKind, is_map_object_tile,
    is_owned_land_tile,
};

use super::types::CommandError;
use super::util::in_bounds;

pub(crate) fn check_buy_land(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if is_owned_land_tile(&tile) {
        return Err(CommandError::LandAlreadyOwned);
    }
    if is_map_object_tile(tile.mapt) {
        return Err(CommandError::CannotBuyLandHere);
    }
    match tile.kind {
        TileKind::Grass | TileKind::Forest => Ok(()),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => Err(CommandError::CannotBuyLandHere),
    }
}

pub(crate) fn tile_rect(from: TileCoord, to: TileCoord) -> impl Iterator<Item = TileCoord> {
    let x0 = from.x.min(to.x);
    let x1 = from.x.max(to.x);
    let y0 = from.y.min(to.y);
    let y1 = from.y.max(to.y);
    (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| TileCoord::new(x, y)))
}

pub(crate) fn buy_land(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    check_buy_land(&state.map, c)?;
    buy_land_area(state, c, c)
}

pub(crate) fn buy_land_area(
    state: &mut GameState,
    from: TileCoord,
    to: TileCoord,
) -> Result<(), CommandError> {
    let cost_per = buy_land_cost(state.tick.get());
    let candidates: Vec<TileCoord> = tile_rect(from, to)
        .filter(|c| check_buy_land(&state.map, *c).is_ok())
        .collect();
    if candidates.is_empty() {
        return Err(CommandError::CannotBuyLandHere);
    }
    let total = cost_per.saturating_mul(candidates.len() as i64);
    if state.economy.money < total {
        return Err(CommandError::InsufficientFunds);
    }
    for c in candidates {
        state
            .map
            .set_mapt_m5(c, MP_OBJECT_MAPT, OBJECT_TYPE_OWNED_LAND)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_m1(c, 0)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= total;
    Ok(())
}

pub(crate) fn buy_land_quote(state: &GameState, cmd: &super::types::Command) -> i64 {
    use super::types::Command;
    let cost_per = buy_land_cost(state.tick.get());
    match cmd {
        Command::BuyLand(c) if check_buy_land(&state.map, *c).is_ok() => cost_per,
        Command::BuyLandArea { from, to } => {
            let count = tile_rect(*from, *to)
                .filter(|c| check_buy_land(&state.map, *c).is_ok())
                .count();
            cost_per.saturating_mul(count as i64)
        }
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::command::{Command, apply_command};

    #[test]
    fn buy_land_marks_object_and_charges_money() {
        let mut state = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        let before = state.economy.money;
        buy_land(&mut state, c).expect("buy");
        let tile = state.map.get(c).expect("tile");
        assert!(is_owned_land_tile(&tile));
        assert!(state.economy.money < before);
    }

    #[test]
    fn buy_land_rejects_already_owned() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        apply_command(&mut state, &Command::BuyLand(c)).unwrap();
        assert_eq!(
            apply_command(&mut state, &Command::BuyLand(c)),
            Err(CommandError::LandAlreadyOwned)
        );
    }

    #[test]
    fn buy_land_area_covers_rectangle() {
        let mut state = GameState::new(6, 6);
        apply_command(
            &mut state,
            &Command::BuyLandArea {
                from: TileCoord::new(1, 1),
                to: TileCoord::new(3, 2),
            },
        )
        .unwrap();
        for y in 1..=2 {
            for x in 1..=3 {
                assert!(is_owned_land_tile(
                    &state.map.get(TileCoord::new(x, y)).expect("tile")
                ));
            }
        }
    }
}
