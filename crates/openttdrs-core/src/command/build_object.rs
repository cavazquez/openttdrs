//! Colocar objetos vanilla jugables y `NewGRF` multitile (`object_cmd.cpp` simplificado).
#![allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]

use crate::economy::build_object_cost_factored;
use crate::game_state::GameState;
use crate::map::{
    MP_OBJECT_MAPT, Map, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_TRANSMITTER, TileCoord, TileKind,
    is_map_object_tile, is_newgrf_object_type, object_footprint_tiles, object_tile_offset_byte,
    object_type_dims, object_type_from_tile, tile_slope_and_z,
};
use crate::object_spec::{DEFAULT_OBJECT_BUILD_COST_FACTOR, ObjectSpecDef, object_spec_def};
use crate::town::Town;
use crate::world_gen::Climate;

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

/// Vanilla 0/1, o id `NewGRF` presente en el catálogo con tamaño válido.
#[must_use]
pub fn is_allowed_build_object_type(object_type: u8, catalog: &[ObjectSpecDef]) -> bool {
    if is_buildable_object_type(object_type) {
        return true;
    }
    if !is_newgrf_object_type(object_type) {
        return false;
    }
    object_spec_def(catalog, u16::from(object_type))
        .is_some_and(|d| d.size_width() > 0 && d.size_height() > 0)
}

fn object_build_cost_params(object_type: u8, catalog: &[ObjectSpecDef]) -> (u8, u32) {
    if is_newgrf_object_type(object_type)
        && let Some(def) = object_spec_def(catalog, u16::from(object_type))
    {
        return (def.build_cost_factor, def.tile_count().max(1));
    }
    (DEFAULT_OBJECT_BUILD_COST_FACTOR, 1)
}

fn check_single_object_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(map, c)?;
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

pub(crate) fn check_build_object_with_towns(
    map: &Map,
    c: TileCoord,
    object_type: u8,
    catalog: &[ObjectSpecDef],
    climate: Climate,
    towns: &mut [Town],
) -> Result<(), CommandError> {
    in_bounds(map, c)?;
    if !is_allowed_build_object_type(object_type, catalog) {
        return Err(CommandError::CannotBuildObjectHere);
    }
    let newgrf_spec = if is_newgrf_object_type(object_type) {
        let Some(def) = object_spec_def(catalog, u16::from(object_type)) else {
            return Err(CommandError::CannotBuildObjectHere);
        };
        if !def.available_in_climate(climate.newgrf_landscape_bit()) {
            return Err(CommandError::CannotBuildObjectHere);
        }
        Some(def)
    } else {
        None
    };
    let (w, h) = object_type_dims(object_type, catalog);
    if w == 0 || h == 0 {
        return Err(CommandError::CannotBuildObjectHere);
    }
    // Validar footprint completo ANTES de mutar el mapa.
    for tile in object_footprint_tiles(c, w, h) {
        check_single_object_tile(map, tile)?;
    }
    // `CBID_OBJECT_LAND_SLOPE_CHECK` se evalúa una vez por tesela, después de
    // que el footprint entero superó las validaciones comunes y antes de mutar.
    if let Some(def) = newgrf_spec {
        for dy in 0..h {
            for dx in 0..w {
                let tile = TileCoord::new(c.x + i32::from(dx), c.y + i32::from(dy));
                let (slope, _) = tile_slope_and_z(map, tile).ok_or(CommandError::OutOfBounds)?;
                let offset = object_tile_offset_byte(dx, dy);
                let nearest_town = towns
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, town)| crate::house_spec::distance_square(town.pos, tile))
                    .map(|(index, _)| index);
                if let Some(index) = nearest_town {
                    if !crate::newgrf_callback::apply_object_slope_callback_for_build(
                        def,
                        map,
                        &mut towns[index],
                        tile,
                        slope,
                        offset,
                        climate,
                    ) {
                        return Err(CommandError::NewGrfCallbackDenied);
                    }
                } else if !crate::newgrf_callback::apply_object_slope_callback(def, slope, offset) {
                    return Err(CommandError::NewGrfCallbackDenied);
                }
            }
        }
    }
    Ok(())
}

fn count_objects_of_type(map: &Map, object_type: u8) -> usize {
    let (w, h) = map.dimensions();
    let mut n = 0usize;
    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x as i32, y as i32);
            if let Some(tile) = map.get(c)
                && object_type_from_tile(&tile) == Some(object_type)
                && tile.m2 == 0
            {
                // Contar solo orígenes (m2 == 0) para multitile.
                n += 1;
            }
        }
    }
    n
}

/// Comprueba colocación; límite 1 faro / 1 transmisor (no aplica a ids `NewGRF` ≥5).
pub(crate) fn check_build_object_placement_with_towns(
    map: &Map,
    c: TileCoord,
    object_type: u8,
    catalog: &[ObjectSpecDef],
    climate: Climate,
    towns: &mut [Town],
) -> Result<(), CommandError> {
    check_build_object_with_towns(map, c, object_type, catalog, climate, towns)?;
    if !is_newgrf_object_type(object_type) && count_objects_of_type(map, object_type) >= 1 {
        return Err(CommandError::ObjectLimitReached);
    }
    Ok(())
}

fn place_object_tile(
    state: &mut GameState,
    c: TileCoord,
    object_type: u8,
    offset: u8,
) -> Result<(), CommandError> {
    let low_mapt = state.map.get(c).map_or(0, |tile| tile.mapt & 0x0F);
    state
        .map
        .set_mapt_m5(c, MP_OBJECT_MAPT | low_mapt, object_type)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_m2(c, offset)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_m1(c, state.active_company.0)
        .map_err(|_| CommandError::OutOfBounds)?;
    if state.map.get_kind(c) == Some(TileKind::Forest) {
        state
            .map
            .set_kind(c, TileKind::Grass)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    Ok(())
}

pub(crate) fn build_object(
    state: &mut GameState,
    c: TileCoord,
    object_type: u8,
) -> Result<(), CommandError> {
    // El callback se evalúa contra una copia de los pueblos. Así el preflight
    // conserva sus efectos PSA para el execute, pero un fallo de fondos no
    // muta el estado persistente.
    let mut callback_towns = state.towns.clone();
    check_build_object_placement_with_towns(
        &state.map,
        c,
        object_type,
        &state.object_spec_catalog,
        state.climate,
        &mut callback_towns,
    )?;
    let (factor, tiles) = object_build_cost_params(object_type, &state.object_spec_catalog);
    let cost = build_object_cost_factored(&state.global_economy, factor, tiles);
    if state.economy.money < cost {
        return Err(CommandError::InsufficientFunds);
    }
    state.towns = callback_towns;
    let (w, h) = object_type_dims(object_type, &state.object_spec_catalog);
    for dy in 0..h {
        for dx in 0..w {
            let tile = TileCoord::new(c.x + i32::from(dx), c.y + i32::from(dy));
            place_object_tile(state, tile, object_type, object_tile_offset_byte(dx, dy))?;
        }
    }
    // El mapa local conserva el layout histórico del puerto (m2 = offset),
    // pero el pool `OBJS` necesita igualmente una instancia para que el save
    // pueda reconstruir metadata, huella y callbacks. El ObjectID del origen
    // es el que expone actualmente `GetObjectIndex` para este layout.
    if let Some(origin) = state.map.get(c)
        && let Some(object_id) = crate::map::object_id_from_tile(&origin)
        && !state
            .objects
            .iter()
            .any(|object| object.object_id == object_id)
    {
        state.objects.push(crate::sav::SavObject {
            object_id,
            tile: c,
            width: u16::from(w),
            height: u16::from(h),
            town: state
                .towns
                .iter()
                .min_by_key(|town| crate::house_spec::distance_square(town.pos, c))
                .map_or(0, |town| town.id),
            build_date: state.calendar.date,
            colour: state.company_colour,
            view: 0,
            object_type: u16::from(object_type),
        });
    }
    state.sav_objects_dirty = true;
    state.economy.money -= cost;
    Ok(())
}

pub(crate) fn build_object_quote(state: &GameState, cmd: &super::types::Command) -> i64 {
    use super::types::Command;
    match cmd {
        Command::BuildObject { pos, object_type } => {
            let mut callback_towns = state.towns.clone();
            if check_build_object_placement_with_towns(
                &state.map,
                *pos,
                *object_type,
                &state.object_spec_catalog,
                state.climate,
                &mut callback_towns,
            )
            .is_err()
            {
                return 0;
            }
            let (factor, tiles) =
                object_build_cost_params(*object_type, &state.object_spec_catalog);
            build_object_cost_factored(&state.global_economy, factor, tiles)
        }
        _ => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command, command_would_fail};
    use crate::economy::build_object_cost;
    use crate::map::object_type_from_tile;
    use crate::newgrf_actions::{
        ACTION0_FEATURE_OBJECTS, apply_newgrf_objects, build_action0_object_payload,
        build_action0_object_payload_full, build_action0_object_payload_with_callback_mask,
        build_grf_v2_with_action0_and_action8,
    };
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, TrainSpriteAssign,
        TrainSpriteGraphics, build_action2_callback_literal_payload,
        build_grf_v2_feature_with_action2_chain,
    };
    use crate::object_spec::{
        DEFAULT_OBJECT_CLIMATE_MASK, NEW_OBJECT_OFFSET, OBJECT_CALLBACK_SLOPE_CHECK_MASK,
        OBJECT_SIZE_1X1, ObjectSpecDef,
    };

    fn push_spec(state: &mut GameState, size: u8, cost_factor: u8, climate_mask: u8) -> u8 {
        let id = NEW_OBJECT_OFFSET + state.object_spec_catalog.len() as u16;
        state.object_spec_catalog.push(ObjectSpecDef {
            id,
            class_label: "OBJ ".into(),
            name: "Obj".into(),
            size,
            from_newgrf: true,
            local_id: u8::try_from(state.object_spec_catalog.len()).unwrap_or(0),
            grfid: 0x4F_42_00_01,
            newgrf_grf_version: 0,
            climate_mask,
            build_cost_factor: cost_factor,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        });
        u8::try_from(id).expect("id fits m5")
    }

    fn parent_psto_runtime(reg: u8, value: u8, result: u8) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0x80,
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: vec![
                    Action2VarOp {
                        operator: 0x10,
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0x80,
                                and_mask: u32::from(reg),
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                    Action2VarOp {
                        operator: 0x0F,
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0x80,
                                and_mask: u32::from(result),
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                ],
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

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
    fn built_object_preserves_tropic_zone_nibble() {
        let mut state = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        state
            .map
            .set_mapt_m5(c, 0x22, 0)
            .expect("tropical zone fixture");

        place_object_tile(&mut state, c, OBJECT_TYPE_LIGHTHOUSE, 0).expect("place object tile");

        assert_eq!(state.map.get(c).expect("object tile").mapt & 0x0F, 2);
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

    /// CB157 debe venir del Action2 del GRF cargado y bloquear tanto la query
    /// como el execute antes de cobrar o escribir alguna tesela del objeto.
    #[test]
    fn loaded_newgrf_object_slope_callback_blocks_construction() {
        let action0 = build_action0_object_payload_with_callback_mask(
            0,
            b"CBOS",
            OBJECT_SIZE_1X1,
            DEFAULT_OBJECT_CLIMATE_MASK,
            1,
            OBJECT_CALLBACK_SLOPE_CHECK_MASK,
            "Objeto con pendiente controlada",
            &[],
        );
        let action2 = build_action2_callback_literal_payload(
            ACTION0_FEATURE_OBJECTS,
            7,
            0, // CB de ubicación: cero no es FAILED ni 0x400, por lo que deniega.
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_OBJECTS,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            *b"CBOS",
            "object-slope-callback",
        );
        let dir = std::env::temp_dir().join(format!(
            "openttdrs_object_slope_callback_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("object-slope-callback.grf"), &bytes).expect("write");

        let mut state = GameState::new(8, 8);
        let mut entry = crate::NewGrfEntry::new(
            "object-slope-callback.grf",
            crate::newgrf_config::grfid_from_bytes(*b"CBOS"),
        );
        entry.grf_version = 8;
        state.newgrf_stack.push(entry);
        apply_newgrf_objects(&mut state, &[&dir]);
        let _ = std::fs::remove_dir_all(&dir);

        let def = state.object_spec_catalog.first().expect("object spec");
        assert!(def.has_slope_check_callback());
        assert!(def.newgrf_runtime.is_some());
        let command = Command::BuildObject {
            pos: TileCoord::new(3, 3),
            object_type: u8::try_from(def.id).expect("id fits m5"),
        };
        assert_eq!(
            command_would_fail(&state, &command),
            Some(CommandError::NewGrfCallbackDenied)
        );
        let money_before = state.economy.money;
        assert_eq!(
            apply_command(&mut state, &command),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(state.economy.money, money_before);
        assert_eq!(
            state.map.get_kind(TileCoord::new(3, 3)),
            Some(TileKind::Grass)
        );

        // En GRF 7, el mismo cero del callback se interpreta como éxito por
        // la inversión histórica del bit 10.
        state.object_spec_catalog[0].newgrf_grf_version = 7;
        assert_eq!(command_would_fail(&state, &command), None);
        apply_command(&mut state, &command).expect("GRF 7 permite la pendiente");
    }

    #[test]
    fn object_slope_callback_psa_commits_only_on_execute() {
        let mut state = GameState::new(8, 8);
        let object_type = push_spec(&mut state, OBJECT_SIZE_1X1, 1, DEFAULT_OBJECT_CLIMATE_MASK);
        state.towns.push(crate::town::Town {
            id: 12,
            pos: TileCoord::new(3, 3),
            ..Default::default()
        });
        let def = state.object_spec_catalog.first_mut().expect("object spec");
        def.callback_mask = OBJECT_CALLBACK_SLOPE_CHECK_MASK;
        def.grfid = 0x4F42_5053;
        def.newgrf_grf_version = 7; // callback 0 = éxito en GRF < 8
        def.newgrf_runtime = Some(Box::new(parent_psto_runtime(5, 42, 0)));
        let command = Command::BuildObject {
            pos: TileCoord::new(3, 3),
            object_type,
        };

        assert_eq!(command_would_fail(&state, &command), None);
        assert!(state.towns[0].newgrf_persistent_regs.is_empty());
        apply_command(&mut state, &command).expect("build object");
        assert_eq!(
            state.towns[0]
                .newgrf_persistent_regs
                .get(&0x4F42_5053)
                .and_then(|registers| registers.get(&5)),
            Some(&42)
        );

        let mut no_money = GameState::new(8, 8);
        let no_money_type = push_spec(
            &mut no_money,
            OBJECT_SIZE_1X1,
            1,
            DEFAULT_OBJECT_CLIMATE_MASK,
        );
        no_money.towns.push(crate::town::Town {
            id: 13,
            pos: TileCoord::new(4, 4),
            ..Default::default()
        });
        let no_money_def = no_money
            .object_spec_catalog
            .first_mut()
            .expect("object spec");
        no_money_def.callback_mask = OBJECT_CALLBACK_SLOPE_CHECK_MASK;
        no_money_def.grfid = 0x4F42_5054;
        no_money_def.newgrf_grf_version = 7;
        no_money_def.newgrf_runtime = Some(Box::new(parent_psto_runtime(5, 42, 0)));
        no_money.economy.money = 0;
        let no_money_command = Command::BuildObject {
            pos: TileCoord::new(4, 4),
            object_type: no_money_type,
        };
        assert_eq!(
            apply_command(&mut no_money, &no_money_command),
            Err(CommandError::InsufficientFunds)
        );
        assert!(no_money.towns[0].newgrf_persistent_regs.is_empty());
    }

    #[test]
    fn build_newgrf_object_2x1_writes_full_footprint() {
        let mut state = GameState::new(8, 8);
        let ot = push_spec(&mut state, 0x12, 1, DEFAULT_OBJECT_CLIMATE_MASK); // 2×1
        let origin = TileCoord::new(2, 2);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: origin,
                object_type: ot,
            },
        )
        .expect("build 2x1");
        let a = state.map.get(origin).unwrap();
        let b = state.map.get(TileCoord::new(3, 2)).unwrap();
        assert_eq!(object_type_from_tile(&a), Some(ot));
        assert_eq!(object_type_from_tile(&b), Some(ot));
        assert_eq!(a.m2, object_tile_offset_byte(0, 0));
        assert_eq!(b.m2, object_tile_offset_byte(1, 0));
    }

    #[test]
    fn build_newgrf_object_rejects_occupied_footprint() {
        let mut state = GameState::new(8, 8);
        let ot = push_spec(&mut state, 0x12, 1, DEFAULT_OBJECT_CLIMATE_MASK);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(2, 2),
                object_type: OBJECT_TYPE_LIGHTHOUSE,
            },
        )
        .unwrap();
        // Footprint (1,2)-(2,2) overlaps lighthouse at (2,2).
        assert_eq!(
            apply_command(
                &mut state,
                &Command::BuildObject {
                    pos: TileCoord::new(1, 2),
                    object_type: ot,
                },
            ),
            Err(CommandError::CannotBuildObjectHere)
        );
        // Origin tile must remain grass (no partial mutate).
        let origin = state.map.get(TileCoord::new(1, 2)).unwrap();
        assert!(!is_map_object_tile(origin.mapt));
    }

    #[test]
    fn clear_tile_demolishes_multitile_footprint() {
        let mut state = GameState::new(8, 8);
        let ot = push_spec(&mut state, 0x12, 1, DEFAULT_OBJECT_CLIMATE_MASK);
        let origin = TileCoord::new(2, 2);
        let other = TileCoord::new(3, 2);
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: origin,
                object_type: ot,
            },
        )
        .unwrap();
        apply_command(&mut state, &Command::ClearTile(other)).unwrap();
        assert!(!is_map_object_tile(state.map.get(origin).unwrap().mapt));
        assert!(!is_map_object_tile(state.map.get(other).unwrap().mapt));
    }

    #[test]
    fn build_newgrf_object_uses_cost_factor() {
        let mut state = GameState::new(8, 8);
        let factor = 4u8;
        let ot = push_spec(
            &mut state,
            OBJECT_SIZE_1X1,
            factor,
            DEFAULT_OBJECT_CLIMATE_MASK,
        );
        let before = state.economy.money;
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(1, 1),
                object_type: ot,
            },
        )
        .unwrap();
        let expected = build_object_cost_factored(&state.global_economy, factor, 1);
        assert_eq!(before - state.economy.money, expected);
        assert_ne!(expected, build_object_cost(&state.global_economy));
    }

    #[test]
    fn build_newgrf_object_rejects_wrong_climate() {
        let mut state = GameState::new(8, 8);
        state.climate = Climate::Temperate;
        let ot = push_spec(&mut state, OBJECT_SIZE_1X1, 1, 0x02); // solo ártico
        assert_eq!(
            apply_command(
                &mut state,
                &Command::BuildObject {
                    pos: TileCoord::new(1, 1),
                    object_type: ot,
                },
            ),
            Err(CommandError::CannotBuildObjectHere)
        );
    }

    #[test]
    fn build_newgrf_object_skips_one_per_type_limit() {
        let mut state = GameState::new(8, 8);
        let ot = push_spec(&mut state, OBJECT_SIZE_1X1, 1, DEFAULT_OBJECT_CLIMATE_MASK);
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
    fn set_current_object_spec_accepts_vanilla_and_catalog_sizes() {
        let mut state = GameState::new(4, 4);
        assert_eq!(state.current_object_spec, 0);
        apply_command(&mut state, &Command::SetCurrentObjectSpec(1)).unwrap();
        assert_eq!(state.current_object_spec, 1);
        apply_command(&mut state, &Command::SetCurrentObjectSpec(0)).unwrap();
        assert_eq!(state.current_object_spec, 0);

        let id_1x1 = NEW_OBJECT_OFFSET;
        state.object_spec_catalog.push(ObjectSpecDef {
            id: id_1x1,
            class_label: "OBJ ".into(),
            name: "Obj".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 1,
            newgrf_grf_version: 0,
            climate_mask: DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: 1,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        });
        apply_command(&mut state, &Command::SetCurrentObjectSpec(id_1x1)).unwrap();
        assert_eq!(state.current_object_spec, id_1x1);

        let id_2x1 = NEW_OBJECT_OFFSET + 1;
        state.object_spec_catalog.push(ObjectSpecDef {
            id: id_2x1,
            class_label: "BIG ".into(),
            name: "Big".into(),
            size: 0x12,
            from_newgrf: true,
            local_id: 1,
            grfid: 1,
            newgrf_grf_version: 0,
            climate_mask: DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: 1,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        });
        apply_command(&mut state, &Command::SetCurrentObjectSpec(id_2x1)).unwrap();
        assert_eq!(state.current_object_spec, id_2x1);

        apply_command(&mut state, &Command::SetCurrentObjectSpec(99)).unwrap();
        assert_eq!(state.current_object_spec, id_2x1);
    }

    #[test]
    fn object_spec_json_roundtrips_grfid_local_id_and_cost() {
        let def = ObjectSpecDef {
            id: NEW_OBJECT_OFFSET,
            class_label: "LIGT".into(),
            name: "Faro2".into(),
            size: 0x12,
            from_newgrf: true,
            local_id: 3,
            grfid: 0x4F_42_00_02,
            newgrf_grf_version: 0,
            climate_mask: 0x05,
            build_cost_factor: 5,
            flags: crate::object_spec::OBJECT_FLAG_ANIMATION,
            animation_frames: 3,
            animation_status: 1,
            animation_speed: 4,
            animation_triggers: 0x12,
            callback_mask: OBJECT_CALLBACK_SLOPE_CHECK_MASK,
            views: Vec::new(),
            newgrf_runtime: None,
            associated_badges: vec![1, 2],
        };
        let json = serde_json::to_string(&def).expect("ser");
        assert!(json.contains("\"local_id\":3"));
        assert!(json.contains("\"grfid\":"));
        assert!(json.contains("\"build_cost_factor\":5"));
        assert!(json.contains("\"callback_mask\":1"));
        let loaded: ObjectSpecDef = serde_json::from_str(&json).expect("de");
        assert_eq!(loaded.local_id, 3);
        assert_eq!(loaded.grfid, 0x4F_42_00_02);
        assert_eq!(loaded.size, 0x12);
        assert_eq!(loaded.build_cost_factor, 5);
        assert_eq!(loaded.climate_mask, 0x05);
        assert_eq!(loaded.callback_mask, OBJECT_CALLBACK_SLOPE_CHECK_MASK);
        assert_eq!(loaded.flags, crate::object_spec::OBJECT_FLAG_ANIMATION);
        assert_eq!(loaded.animation_frames, 3);
        assert_eq!(loaded.animation_status, 1);
        assert_eq!(loaded.animation_speed, 4);
        assert_eq!(loaded.animation_triggers, 0x12);
        assert!(loaded.views.is_empty());
        assert!(loaded.newgrf_runtime.is_none());

        let legacy: ObjectSpecDef = serde_json::from_value(serde_json::json!({
            "id": NEW_OBJECT_OFFSET,
            "class_label": "OLD ",
            "name": "Old",
            "size": 0x11,
            "from_newgrf": true
        }))
        .expect("legacy spec");
        assert_eq!(legacy.animation_status, 0xFF);
        assert_eq!(legacy.animation_speed, 2);
        assert_eq!(legacy.animation_frames, 0);
        assert_eq!(legacy.animation_triggers, 0);
    }

    #[test]
    fn newgrf_object_map_survives_save_load_and_reapply() {
        let a0 = build_action0_object_payload_full(3, b"LIGT", 0x12, 0x0F, 5, "Faro2", &[]);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'O', b'B', 0, 2], "obj2", "");
        let dir =
            std::env::temp_dir().join(format!("openttdrs_obj_saveload_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("obj2.grf"), &bytes).expect("write");
        let mut state = GameState::new(8, 8);
        let grfid = crate::newgrf_config::grfid_from_bytes([b'O', b'B', 0, 2]);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("obj2.grf", grfid));
        apply_newgrf_objects(&mut state, &[&dir]);
        let ot = u8::try_from(state.object_spec_catalog[0].id).unwrap();
        apply_command(
            &mut state,
            &Command::BuildObject {
                pos: TileCoord::new(2, 2),
                object_type: ot,
            },
        )
        .unwrap();

        let path = dir.join("game.json");
        crate::save::save(&state, &path).unwrap();
        let mut loaded = crate::save::load(&path).unwrap();
        // Re-aplicar NewGRF post-load (como hace migrate con search dirs).
        apply_newgrf_objects(&mut loaded, &[&dir]);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(loaded.object_spec_catalog.len(), 1);
        let loaded_def = &loaded.object_spec_catalog[0];
        assert_eq!(loaded_def.local_id, 3);
        assert_eq!(loaded_def.grfid, grfid);
        assert_eq!(loaded_def.size, 0x12);
        assert_eq!(loaded_def.build_cost_factor, 5);
        let a = loaded.map.get(TileCoord::new(2, 2)).unwrap();
        let b = loaded.map.get(TileCoord::new(3, 2)).unwrap();
        assert_eq!(object_type_from_tile(&a), Some(ot));
        assert_eq!(object_type_from_tile(&b), Some(ot));
        assert_eq!(a.m2, object_tile_offset_byte(0, 0));
        assert_eq!(b.m2, object_tile_offset_byte(1, 0));
    }
}
