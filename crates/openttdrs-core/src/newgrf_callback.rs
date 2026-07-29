//! API común de resolución de callbacks `NewGRF` (#228).
//!
//! - Fallo observable: [`CALLBACK_FAILED`] (nunca se acepta un resultado “silencioso”).
//! - Storage: tras eval, [`writeback_vehicle_persistent_registers`] persiste `7C`/`\2psto`;
//!   los registros temporales (`7D`/`\2sto`) viven solo en el ctx y se descartan.

use crate::engine::EngineDef;
use crate::newgrf_sprites::{
    Action2EvalCtx, CALLBACK_FAILED, CBID_VEHICLE_START_STOP_CHECK, TrainSpriteGraphics,
};
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::engines_table;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
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
}
