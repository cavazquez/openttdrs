//! Colocar objetos vanilla jugables: faro y transmisor (`object_cmd.cpp` simplificado).
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]

use crate::economy::build_object_cost;
use crate::game_state::GameState;
use crate::map::{
    MP_OBJECT_MAPT, Map, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_TRANSMITTER, TileCoord, TileKind,
    is_map_object_tile, object_type_from_tile,
};

use super::error::CommandError;
use super::util::in_bounds;

/// Tipos de objeto que el jugador puede construir (no `OWNED_LAND`).
#[must_use]
pub const fn is_buildable_object_type(object_type: u8) -> bool {
    matches!(
        object_type,
        OBJECT_TYPE_TRANSMITTER | OBJECT_TYPE_LIGHTHOUSE
    )
}

pub(crate) fn check_build_object(
    map: &Map,
    c: TileCoord,
    object_type: u8,
) -> Result<(), CommandError> {
    in_bounds(map, c)?;
    if !is_buildable_object_type(object_type) {
        return Err(CommandError::CannotBuildObjectHere);
    }
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if is_map_object_tile(tile.mapt) {
        return Err(CommandError::CannotBuildObjectHere);
    }
    match tile.kind {
        TileKind::Grass | TileKind::Forest => Ok(()),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => Err(CommandError::CannotBuildObjectHere),
    }
}

fn count_objects_of_type(map: &Map, object_type: u8) -> usize {
    let (w, h) = map.dimensions();
    let mut n = 0usize;
    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x as i32, y as i32);
            if let Some(tile) = map.get(c)
                && object_type_from_tile(&tile) == Some(object_type)
            {
                n += 1;
            }
        }
    }
    n
}

/// Comprueba colocación + límite de 1 faro / 1 transmisor por mapa.
pub(crate) fn check_build_object_placement(
    map: &Map,
    c: TileCoord,
    object_type: u8,
) -> Result<(), CommandError> {
    check_build_object(map, c, object_type)?;
    if count_objects_of_type(map, object_type) >= 1 {
        return Err(CommandError::ObjectLimitReached);
    }
    Ok(())
}

pub(crate) fn build_object(
    state: &mut GameState,
    c: TileCoord,
    object_type: u8,
) -> Result<(), CommandError> {
    check_build_object_placement(&state.map, c, object_type)?;
    let cost = build_object_cost(state.tick.get());
    if state.economy.money < cost {
        return Err(CommandError::InsufficientFunds);
    }
    state
        .map
        .set_mapt_m5(c, MP_OBJECT_MAPT, object_type)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_m1(c, state.active_company.0)
        .map_err(|_| CommandError::OutOfBounds)?;
    // Mantener hierba/bosque como base visual; el render usa `mapt`/`m5`.
    if state.map.get_kind(c) == Some(TileKind::Forest) {
        state
            .map
            .set_kind(c, TileKind::Grass)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
    Ok(())
}

pub(crate) fn build_object_quote(state: &GameState, cmd: &super::types::Command) -> i64 {
    use super::types::Command;
    match cmd {
        Command::BuildObject { pos, object_type }
            if check_build_object_placement(&state.map, *pos, *object_type).is_ok() =>
        {
            build_object_cost(state.tick.get())
        }
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::map::object_type_from_tile;

    #[test]
    fn build_lighthouse_marks_object_and_charges() {
        let mut state = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        let before = state.economy.money;
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: c,
                object_type: OBJECT_TYPE_LIGHTHOUSE,
            },
        )
        .expect("build");
        let tile = state.map.get(c).expect("tile");
        assert_eq!(object_type_from_tile(&tile), Some(OBJECT_TYPE_LIGHTHOUSE));
        assert!(state.economy.money < before);
    }

    #[test]
    fn build_object_rejects_second_of_same_type() {
        let mut state = GameState::new(8, 8);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(1, 1),
                object_type: OBJECT_TYPE_TRANSMITTER,
            },
        )
        .unwrap();
        assert_eq!(
            apply_command(
                &mut state,
                &Command::BuildObject {
                    pos: TileCoord::new(3, 3),
                    object_type: OBJECT_TYPE_TRANSMITTER,
                },
            ),
            Err(CommandError::ObjectLimitReached)
        );
        // Faro distinto: permitido.
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(3, 3),
                object_type: OBJECT_TYPE_LIGHTHOUSE,
            },
        )
        .unwrap();
    }

    #[test]
    fn clear_tile_removes_built_object() {
        let mut state = GameState::new(6, 6);
        let c = TileCoord::new(2, 2);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: c,
                object_type: OBJECT_TYPE_LIGHTHOUSE,
            },
        )
        .unwrap();
        apply_command(&mut state, &Command::ClearTile(c)).unwrap();
        let tile = state.map.get(c).unwrap();
        assert!(!is_map_object_tile(tile.mapt));
        assert_eq!(tile.kind, TileKind::Grass);
    }

    #[test]
    fn build_object_roundtrips_in_save() {
        let mut state = GameState::new(6, 6);
        let c = TileCoord::new(1, 1);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: c,
                object_type: OBJECT_TYPE_TRANSMITTER,
            },
        )
        .unwrap();
        let path = std::env::temp_dir().join(format!(
            "openttdrs_build_object_{}.json",
            std::process::id()
        ));
        crate::save::save(&state, &path).unwrap();
        let loaded = crate::save::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let tile = loaded.map.get(c).unwrap();
        assert_eq!(object_type_from_tile(&tile), Some(OBJECT_TYPE_TRANSMITTER));
    }
}
