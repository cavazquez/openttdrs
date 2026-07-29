//! API común de resolución de callbacks `NewGRF` (#228 / #266).
//!
//! - Fallo observable: [`CALLBACK_FAILED`] (nunca se acepta un resultado “silencioso”).
//! - Storage: tras eval, writeback de `7C`/`\2psto` a vehículo o estación;
//!   los registros temporales (`7D`/`\2sto`) viven solo en el ctx y se descartan.
//! - Call sites #266: industry location, house construction, station availability,
//!   industry-tile trigger → Action2 random.

use crate::engine::EngineDef;
use crate::house_spec::HouseSpecDef;
use crate::industry_spec::IndustrySpecDef;
use crate::industry_tile::IndustryTileSpecDef;
use crate::newgrf_sprites::{
    Action2EvalCtx, Action2RandomEntry, CALLBACK_FAILED, CBID_HOUSE_ALLOW_CONSTRUCTION,
    CBID_INDUSTRY_LOCATION, CBID_INDTILE_ANIM_NEXT_FRAME, CBID_STATION_AVAILABILITY,
    CBID_VEHICLE_START_STOP_CHECK, TrainSpriteGraphics,
};
use crate::station::Station;
use crate::vehicle::Vehicle;

/// Escribe `persistent_registers` del ctx al vehículo.
///
/// Los `temp_registers` no se persisten (ciclo de vida = evaluación).
pub fn writeback_vehicle_persistent_registers(vehicle: &mut Vehicle, ctx: &Action2EvalCtx) {
    vehicle
        .newgrf_persistent_regs
        .clone_from(&ctx.persistent_registers);
}

/// Siembra un ctx desde el vehículo (regs persistentes + random bits).
#[must_use]
pub fn action2_eval_ctx_from_vehicle(vehicle: &Vehicle) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    ctx.persistent_registers
        .clone_from(&vehicle.newgrf_persistent_regs);
    ctx.random_bits = u32::from(vehicle.newgrf_random_bits);
    ctx
}

/// Writeback de regs persistentes a estación (#266).
pub fn writeback_station_persistent_registers(station: &mut Station, ctx: &Action2EvalCtx) {
    station
        .newgrf_persistent_regs
        .clone_from(&ctx.persistent_registers);
}

/// Ctx Action2 desde estación (storage no-vehículo).
#[must_use]
pub fn action2_eval_ctx_from_station(station: &Station) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    ctx.persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    ctx.random_bits = u32::from(station.newgrf_random_bits);
    ctx
}

/// Resuelve un callback sobre el runtime Action2 del motor, con writeback de regs.
///
/// Sin runtime / sin asignación Action3 → [`CALLBACK_FAILED`] (observable).
#[must_use]
pub fn resolve_vehicle_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(runtime) = engine.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    let mut ctx = action2_eval_ctx_from_vehicle(vehicle);
    let result =
        runtime.resolve_callback_ctx(engine.newgrf_local_id, callback, param1, param2, &mut ctx);
    writeback_vehicle_persistent_registers(vehicle, &ctx);
    result
}

/// Semántica `OpenTTD` 15.3 para `CBID_VEHICLE_START_STOP_CHECK` (MVP):
/// - [`CALLBACK_FAILED`] → permitir
/// - `0x400` (GRF ≥ 8) → permitir
/// - byte bajo `0xFF` (GRF < 8) → permitir
/// - cualquier otro resultado → denegar (observable; no silencioso)
#[must_use]
pub fn vehicle_start_stop_callback_allows(result: u16) -> bool {
    result == CALLBACK_FAILED || result == 0x400 || (result & 0xFF) == 0xFF
}

/// Ejecuta CB 0x31 y aplica writeback. `true` = permitir start/stop.
pub fn apply_vehicle_start_stop_callback(engine: &EngineDef, vehicle: &mut Vehicle) -> bool {
    if engine.newgrf_runtime.is_none() {
        return true;
    }
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_START_STOP_CHECK, 0, 0);
    vehicle_start_stop_callback_allows(result)
}

/// Resuelve un callback genérico sobre graphics (sin vehículo), fallando de forma observable.
#[must_use]
pub fn resolve_callback_or_failed(
    gfx: &TrainSpriteGraphics,
    local_id: u8,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    gfx.resolve_callback(local_id, callback, param1, param2)
}

/// ¿El resultado permite construcción / ubicación? (FAILED / 0x400 / 0xFF → sí).
#[must_use]
pub fn callback_allows_placement(result: u16) -> bool {
    vehicle_start_stop_callback_allows(result)
}

/// Call site industria: CB `0x28` location al colocar (#266).
///
/// Sin runtime → permitir (vanilla). Deny observable si el CB no permite.
#[must_use]
pub fn apply_industry_location_callback(def: &IndustrySpecDef) -> bool {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result = runtime.resolve_callback(def.newgrf_local_id, CBID_INDUSTRY_LOCATION, 0, 0);
    callback_allows_placement(result)
}

/// Call site house: CB `0x17` allow construction (#266).
#[must_use]
pub fn apply_house_construction_callback(def: &HouseSpecDef) -> bool {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result =
        runtime.resolve_callback(def.newgrf_local_id, CBID_HOUSE_ALLOW_CONSTRUCTION, 0, 0);
    callback_allows_placement(result)
}

/// Call site estación: CB `0x13` availability + writeback storage (#266).
#[must_use]
pub fn apply_station_availability_callback(
    gfx: &TrainSpriteGraphics,
    local_id: u8,
    station: &mut Station,
) -> bool {
    let mut ctx = action2_eval_ctx_from_station(station);
    let result =
        gfx.resolve_callback_ctx(local_id, CBID_STATION_AVAILABILITY, 0, 0, &mut ctx);
    writeback_station_persistent_registers(station, &ctx);
    callback_allows_placement(result)
}

/// Trigger path: reseed `random_bits` y resolver un Action2 random group si el
/// trigger matchea `entry.triggers` (#266 / esqueleto `ResolveRerandomisation`).
///
/// Devuelve el set-id elegido, o `None` si no hay match / sets vacíos.
pub fn resolve_industry_tile_random_trigger(
    tile_spec: &IndustryTileSpecDef,
    waiting_triggers: u8,
    random_bits: &mut u8,
    world_seed: u64,
    tick: u64,
    salt: u64,
) -> Option<u16> {
    let runtime = tile_spec.newgrf_runtime.as_ref()?;
    let (set_id, entry) = runtime
        .action2_random
        .iter()
        .find(|(_, e)| e.triggers == 0 || (e.triggers & waiting_triggers) != 0)?;
    let _ = set_id;
    // Reseed bits (como industry_tile_rng) antes de evaluar el grupo.
    *random_bits = crate::map::industry_tile_rng(
        world_seed,
        tick,
        crate::map::TileCoord::new(0, 0),
        salt ^ u64::from(waiting_triggers),
    );
    let mut ctx = Action2EvalCtx {
        random_bits: u32::from(*random_bits),
        ..Action2EvalCtx::default()
    };
    Some(eval_random_entry(entry, &mut ctx))
}

fn eval_random_entry(entry: &Action2RandomEntry, ctx: &mut Action2EvalCtx) -> u16 {
    // Reutiliza la semántica de `eval_action2_random` vía resolve de un set dummy:
    // calculamos el índice aquí (misma fórmula).
    let n = entry.sets.len();
    if n == 0 {
        return 0;
    }
    let bits = ctx.random_bits;
    let mask = n.next_power_of_two().saturating_sub(1);
    let idx = (usize::try_from(bits >> entry.randbit).unwrap_or(0) & mask) % n;
    entry.sets[idx]
}

/// Call site industry tile anim (CB `0x25`); FAILED observable sin runtime.
#[must_use]
pub fn apply_industry_tile_anim_callback(def: &IndustryTileSpecDef) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    runtime.resolve_callback(def.newgrf_local_id, CBID_INDTILE_ANIM_NEXT_FRAME, 0, 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::engines_table;
    use crate::newgrf_sprites::{
        Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
        CBID_STATION_BUILD_TILE_LAYOUT, TrainSpriteAssign,
    };
    use crate::{TileCoord, VehicleKind};

    fn gfx_callback_literal(value: u8) -> TrainSpriteGraphics {
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
                        shift: 0,
                        and_mask: value,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    /// `16 * 64 = 0x400` vía operador mul (`and_mask` es BYTE).
    fn gfx_callback_allow_400() -> TrainSpriteGraphics {
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
                        shift: 0,
                        and_mask: 0x10,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: vec![Action2VarOp {
                    operator: 0x0A, // mul
                    rhs: Action2VarTerm {
                        variable: 0x1A,
                        param: None,
                        adjust: Action2VarAdjust {
                            shift: 0,
                            and_mask: 0x40,
                            ..Action2VarAdjust::default()
                        },
                    },
                }],
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn gfx_callback_psto(reg: u8, value: u8, result: u8) -> TrainSpriteGraphics {
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
                        shift: 0,
                        and_mask: value,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: vec![
                    Action2VarOp {
                        operator: 0x10, // psto
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: reg,
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                    Action2VarOp {
                        operator: 0x0F, // rst → result literal
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: result,
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
    fn callbacks_ac_resolve_failed_observable() {
        let empty = TrainSpriteGraphics::default();
        assert_eq!(
            resolve_callback_or_failed(&empty, 0, CBID_STATION_BUILD_TILE_LAYOUT, 0, 0),
            CALLBACK_FAILED
        );
        let gfx = gfx_callback_literal(0x0A);
        assert_eq!(
            resolve_callback_or_failed(&gfx, 0, CBID_STATION_BUILD_TILE_LAYOUT, 0, 0),
            0x0A
        );
        // local_id sin assign → FAILED
        assert_eq!(
            resolve_callback_or_failed(&gfx, 9, CBID_STATION_BUILD_TILE_LAYOUT, 0, 0),
            CALLBACK_FAILED
        );
    }

    #[test]
    fn callbacks_ac_vehicle_start_stop_denies() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x10))); // deny
        let mut v = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert!(!apply_vehicle_start_stop_callback(&engine, &mut v));

        engine.newgrf_runtime = Some(Box::new(gfx_callback_allow_400()));
        assert!(apply_vehicle_start_stop_callback(&engine, &mut v));

        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0xFF))); // GRF <8 allow
        assert!(apply_vehicle_start_stop_callback(&engine, &mut v));

        engine.newgrf_runtime = None;
        assert!(apply_vehicle_start_stop_callback(&engine, &mut v));
    }

    #[test]
    fn callbacks_ac_persistent_writeback_and_json_roundtrip() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_local_id = 0;
        // psto reg 3 = 42, resultado 0xFF (allow GRF <8)
        engine.newgrf_runtime = Some(Box::new(gfx_callback_psto(3, 42, 0xFF)));

        let mut state = crate::GameState::new(4, 4);
        let mut v = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        v.engine_id = Some(engine.id);
        assert!(apply_vehicle_start_stop_callback(&engine, &mut v));
        assert_eq!(v.newgrf_persistent_regs.get(&3), Some(&42));

        state.vehicles.push(v);
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(loaded.vehicles[0].newgrf_persistent_regs.get(&3), Some(&42));
    }

    #[test]
    fn callbacks_ac_start_stop_allows_semantics() {
        assert!(vehicle_start_stop_callback_allows(CALLBACK_FAILED));
        assert!(vehicle_start_stop_callback_allows(0x400));
        assert!(vehicle_start_stop_callback_allows(0xFF));
        assert!(!vehicle_start_stop_callback_allows(0));
        assert!(!vehicle_start_stop_callback_allows(0x10));
        assert!(!vehicle_start_stop_callback_allows(0x40F));
    }

    #[test]
    fn callbacks_ac_industry_location_denies_when_cb_says_so() {
        let mut def = IndustrySpecDef {
            id: 37,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: Vec::new(),
            produced_cargo_labels: Vec::new(),
            accepted_cargo_indices: Vec::new(),
            accepted_cargo_labels: Vec::new(),
            production_rates: Vec::new(),
            input_multipliers: Vec::new(),
            callback_mask: 0x0100,
            cost_multiplier: 0,
            name: "test".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_literal(0x10))),
        };
        assert!(!apply_industry_location_callback(&def));
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(0xFF)));
        assert!(apply_industry_location_callback(&def));
        def.newgrf_runtime = None;
        assert!(apply_industry_location_callback(&def));
    }

    #[test]
    fn callbacks_ac_house_construction_and_station_availability() {
        let mut house = HouseSpecDef {
            id: 200,
            local_id: 0,
            subst_id: 0,
            building_flags: 0,
            min_year: 0,
            max_year: 5000,
            population: 10,
            mail_generation: 0,
            availability: 0xFFFF,
            probability: 1,
            override_id: None,
            callback_mask: 0x0001,
            name: "cb-house".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_literal(0x01))),
        };
        assert!(!apply_house_construction_callback(&house));
        house.newgrf_runtime = Some(Box::new(gfx_callback_literal(0xFF)));
        assert!(apply_house_construction_callback(&house));

        let gfx = gfx_callback_psto(5, 7, 0xFF);
        let mut st = Station::new(TileCoord::new(2, 2));
        assert!(apply_station_availability_callback(&gfx, 0, &mut st));
        assert_eq!(st.newgrf_persistent_regs.get(&5), Some(&7));
    }

    #[test]
    fn callbacks_ac_station_persistent_json_roundtrip() {
        let mut state = crate::GameState::new(4, 4);
        let mut st = Station::new(TileCoord::new(1, 1));
        st.newgrf_persistent_regs.insert(2, 99);
        state.stations.push(st);
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(
            loaded.stations[0].newgrf_persistent_regs.get(&2),
            Some(&99)
        );
    }

    #[test]
    fn callbacks_ac_industry_tile_trigger_reseed_resolves_random() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_random.insert(
            3,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 0x01, // TileLoop
                randbit: 0,
                sets: vec![10, 11, 12, 13],
            },
        );
        let mut tile = IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(gfx)),
        };
        let mut bits = 0u8;
        let set = resolve_industry_tile_random_trigger(&tile, 0x01, &mut bits, 42, 7, 3);
        assert!(set.is_some());
        assert!(matches!(set, Some(10..=13)));
        // Sin match de trigger → None
        tile.newgrf_runtime.as_mut().unwrap().action2_random.get_mut(&3).unwrap().triggers = 0x04;
        let mut bits2 = 0u8;
        assert!(resolve_industry_tile_random_trigger(&tile, 0x01, &mut bits2, 1, 1, 1).is_none());
    }

    #[test]
    fn callbacks_ac_industry_tile_anim_failed_without_runtime() {
        let tile = IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(0),
            subst_id: 0,
            from_newgrf: false,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 0,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        assert_eq!(apply_industry_tile_anim_callback(&tile), CALLBACK_FAILED);
    }
}
