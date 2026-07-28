//! Colocar objetos vanilla jugables y `NewGRF` 1×1 (`object_cmd.cpp` simplificado).
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]

use crate::economy::build_object_cost;
use crate::game_state::GameState;
use crate::map::{
    MP_OBJECT_MAPT, Map, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_TRANSMITTER, TileCoord, TileKind,
    is_map_object_tile, is_newgrf_object_type, object_type_from_tile,
};
use crate::object_spec::{ObjectSpecDef, object_spec_def};

use super::error::CommandError;
use super::util::in_bounds;

/// Tipos de objeto vanilla que el jugador puede construir (no `OWNED_LAND`).
#[must_use]
pub const fn is_buildable_object_type(object_type: u8) -> bool {
    matches!(
        object_type,
        OBJECT_TYPE_TRANSMITTER | OBJECT_TYPE_LIGHTHOUSE
    )
}

/// Vanilla 0/1, o id `NewGRF` presente en el catálogo con tamaño 1×1.
#[must_use]
pub fn is_allowed_build_object_type(object_type: u8, catalog: &[ObjectSpecDef]) -> bool {
    if is_buildable_object_type(object_type) {
        return true;
    }
    if !is_newgrf_object_type(object_type) {
        return false;
    }
    object_spec_def(catalog, u16::from(object_type)).is_some_and(ObjectSpecDef::is_1x1)
}

pub(crate) fn check_build_object(
    map: &Map,
    c: TileCoord,
    object_type: u8,
    catalog: &[ObjectSpecDef],
) -> Result<(), CommandError> {
    in_bounds(map, c)?;
    if !is_allowed_build_object_type(object_type, catalog) {
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

/// Comprueba colocación; límite 1 faro / 1 transmisor (no aplica a ids `NewGRF` ≥5).
pub(crate) fn check_build_object_placement(
    map: &Map,
    c: TileCoord,
    object_type: u8,
    catalog: &[ObjectSpecDef],
) -> Result<(), CommandError> {
    check_build_object(map, c, object_type, catalog)?;
    if !is_newgrf_object_type(object_type) && count_objects_of_type(map, object_type) >= 1 {
        return Err(CommandError::ObjectLimitReached);
    }
    Ok(())
}

pub(crate) fn build_object(
    state: &mut GameState,
    c: TileCoord,
    object_type: u8,
) -> Result<(), CommandError> {
    check_build_object_placement(&state.map, c, object_type, &state.object_spec_catalog)?;
    let cost = build_object_cost(&state.global_economy);
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
            if check_build_object_placement(
                &state.map,
                *pos,
                *object_type,
                &state.object_spec_catalog,
            )
            .is_ok() =>
        {
            build_object_cost(&state.global_economy)
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
    use crate::newgrf_actions::{
        apply_newgrf_objects, build_action0_object_payload, build_grf_v2_with_action0_and_action8,
    };
    use crate::object_spec::{NEW_OBJECT_OFFSET, OBJECT_SIZE_1X1, ObjectSpecDef};

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

    #[test]
    fn build_newgrf_object_1x1_sets_m5_from_catalog() {
        let a0 = build_action0_object_payload(0, b"LIGT", OBJECT_SIZE_1X1, "Faro", &[]);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'O', b'B', 0, 1], "obj", "");
        let dir = std::env::temp_dir().join(format!("openttdrs_obj_build_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("obj.grf"), &bytes).expect("write");
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("obj.grf", 15));
        apply_newgrf_objects(&mut state, &[&dir]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(state.object_spec_catalog.len(), 1);
        let id = state.object_spec_catalog[0].id;
        assert!(id >= NEW_OBJECT_OFFSET);
        let object_type = u8::try_from(id).expect("id fits m5");
        let c = TileCoord::new(2, 2);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: c,
                object_type,
            },
        )
        .expect("build newgrf object");
        let tile = state.map.get(c).expect("tile");
        assert_eq!(object_type_from_tile(&tile), Some(object_type));
        assert_eq!(crate::object_spec_id_from_tile(&tile), Some(id));
    }

    #[test]
    fn build_newgrf_object_rejects_non_1x1() {
        let mut state = GameState::new(8, 8);
        state.object_spec_catalog.push(ObjectSpecDef {
            id: NEW_OBJECT_OFFSET,
            class_label: "BIG ".into(),
            name: "Big".into(),
            size: 0x22,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            views: Vec::new(),
            associated_badges: Vec::new(),
        });
        assert_eq!(
            apply_command(
                &mut state,
                &Command::BuildObject {
                    pos: TileCoord::new(1, 1),
                    object_type: NEW_OBJECT_OFFSET as u8,
                },
            ),
            Err(CommandError::CannotBuildObjectHere)
        );
    }

    #[test]
    fn build_newgrf_object_skips_one_per_type_limit() {
        let mut state = GameState::new(8, 8);
        state.object_spec_catalog.push(ObjectSpecDef {
            id: NEW_OBJECT_OFFSET,
            class_label: "OBJ ".into(),
            name: "Obj".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            views: Vec::new(),
            associated_badges: Vec::new(),
        });
        let ot = NEW_OBJECT_OFFSET as u8;
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(1, 1),
                object_type: ot,
            },
        )
        .unwrap();
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(3, 3),
                object_type: ot,
            },
        )
        .expect("second newgrf object allowed");
    }

    #[test]
    fn set_current_object_spec_accepts_vanilla_and_catalog_1x1() {
        let mut state = GameState::new(4, 4);
        assert_eq!(state.current_object_spec, 0);
        apply_command(&mut state, &Command::SetCurrentObjectSpec(1)).unwrap();
        assert_eq!(state.current_object_spec, 1);
        apply_command(&mut state, &Command::SetCurrentObjectSpec(0)).unwrap();
        assert_eq!(state.current_object_spec, 0);

        state.object_spec_catalog.push(ObjectSpecDef {
            id: NEW_OBJECT_OFFSET,
            class_label: "OBJ ".into(),
            name: "Obj".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            views: Vec::new(),
            associated_badges: Vec::new(),
        });
        apply_command(
            &mut state,
            &Command::SetCurrentObjectSpec(NEW_OBJECT_OFFSET),
        )
        .unwrap();
        assert_eq!(state.current_object_spec, NEW_OBJECT_OFFSET);

        // Id desconocido o no 1×1: no cambia.
        apply_command(&mut state, &Command::SetCurrentObjectSpec(99)).unwrap();
        assert_eq!(state.current_object_spec, NEW_OBJECT_OFFSET);
        state.object_spec_catalog.push(ObjectSpecDef {
            id: NEW_OBJECT_OFFSET + 1,
            class_label: "BIG ".into(),
            name: "Big".into(),
            size: 0x22,
            from_newgrf: true,
            local_id: 1,
            grfid: 0,
            views: Vec::new(),
            associated_badges: Vec::new(),
        });
        apply_command(
            &mut state,
            &Command::SetCurrentObjectSpec(NEW_OBJECT_OFFSET + 1),
        )
        .unwrap();
        assert_eq!(state.current_object_spec, NEW_OBJECT_OFFSET);
    }
}
