//! API común de resolución de callbacks `NewGRF` (#228 / #266).
//!
//! - Fallo observable: [`CALLBACK_FAILED`] (nunca se acepta un resultado “silencioso”).
//! - Storage: tras eval, writeback de `7C`/`\2psto` a vehículo o estación;
//!   los registros temporales (`7D`/`\2sto`) viven solo en el ctx y se descartan.
//! - Call sites #266: industry location, house/object construction, station availability,
//!   industry-tile trigger → Action2 random.

use crate::engine::EngineDef;
use crate::house_spec::HouseSpecDef;
use crate::industry_spec::IndustrySpecDef;
use crate::industry_tile::IndustryTileSpecDef;
use crate::map::TileCoord;
use crate::newgrf_sprites::{
    Action2EvalCtx, Action2RandomEntry, CALLBACK_FAILED, CBID_HOUSE_ALLOW_CONSTRUCTION,
    CBID_INDUSTRY_LOCATION, CBID_OBJECT_LAND_SLOPE_CHECK, CBID_STATION_ANIMATION_NEXT_FRAME,
    CBID_STATION_ANIMATION_SPEED, CBID_STATION_ANIMATION_TRIGGER, CBID_STATION_AVAILABILITY,
    CBID_VEHICLE_START_STOP_CHECK, TrainSpriteGraphics,
};
use crate::object_spec::ObjectSpecDef;
use crate::station::Station;
use crate::vehicle::Vehicle;
use crate::{RoadType, StopKind};

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

/// Resultado de un callback de ubicación/slope (`CB28` y afines).
///
/// `CALLBACK_FAILED` conserva el fallback de `OpenTTD`; sólo `0x400` significa
/// explícitamente “sin error”. Los demás resultados representan un motivo de
/// rechazo (texto `NewGRF` o error estándar).
#[must_use]
pub const fn callback_allows_location(result: u16) -> bool {
    result == CALLBACK_FAILED || result == 0x400
}

/// Resultado de un callback booleano de ocho bits (CB13 de station/RoadStop,
/// CB17 de house). `CALLBACK_FAILED` permite el fallback y cualquier byte bajo
/// no nulo permite la operación, como `Convert8bitBooleanCallback` upstream.
#[must_use]
pub const fn callback_allows_8bit_boolean(result: u16) -> bool {
    result == CALLBACK_FAILED || (result & 0xFF) != 0
}

/// Alias de compatibilidad para usuarios de la API que consultaban resultados
/// de ubicación. Los callbacks booleanos deben usar
/// [`callback_allows_8bit_boolean`] explícitamente.
#[must_use]
pub const fn callback_allows_placement(result: u16) -> bool {
    callback_allows_location(result)
}

/// Call site industria: CB `0x28` location al colocar (#266).
///
/// Sin runtime → permitir (vanilla). Deny observable si el CB no permite.
#[must_use]
pub fn apply_industry_location_callback(def: &IndustrySpecDef) -> bool {
    if !def.has_location_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result = runtime.resolve_callback(def.newgrf_local_id, CBID_INDUSTRY_LOCATION, 0, 0);
    callback_allows_location(result)
}

/// Call site house: CB `0x17` allow construction (#266).
#[must_use]
pub fn apply_house_construction_callback(def: &HouseSpecDef) -> bool {
    if !def.has_construction_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result = runtime.resolve_callback(def.newgrf_local_id, CBID_HOUSE_ALLOW_CONSTRUCTION, 0, 0);
    callback_allows_8bit_boolean(result)
}

/// Call site objeto: CB `0x157` de pendiente al construir cada tesela.
///
/// `param1` es el slope de la tesela y `param2` codifica el offset `(dx, dy)`
/// del footprint (`dy << 4 | dx`), igual que `object_cmd.cpp`. Durante la
/// construcción no existe una instancia de objeto persistente; por ahora el
/// resolver aporta esos parámetros genéricos, no los scopes completos de
/// objeto/vecinos de `OpenTTD`.
#[must_use]
pub fn apply_object_slope_callback(def: &ObjectSpecDef, slope: u8, footprint_offset: u8) -> bool {
    if !def.has_slope_check_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result = runtime.resolve_callback(
        def.local_id,
        CBID_OBJECT_LAND_SLOPE_CHECK,
        u32::from(slope),
        u32::from(footprint_offset),
    );
    callback_allows_location(result)
}

/// Call site de construcción de estación ferroviaria: CB `0x13` availability.
///
/// `OpenTTD` invoca este callback sin `Station` ni tesela creada. Por eso no hay
/// writeback de `7C`: ese scope no existe todavía. `CALLBACK_FAILED` conserva
/// el fallback y el resultado usa la semántica booleana de ocho bits.
#[must_use]
pub fn apply_station_availability_callback_for_build(
    def: &crate::station_class::StationSpecDef,
) -> bool {
    if !def.has_availability_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let result = runtime.resolve_callback(def.newgrf_local_id, CBID_STATION_AVAILABILITY, 0, 0);
    callback_allows_8bit_boolean(result)
}

/// Resolver stateful de estación para scopes que sí tienen una estación.
///
/// La construcción ferroviaria normal usa
/// [`apply_station_availability_callback_for_build`], igual que `OpenTTD`, y no
/// puede persistir registros porque aún no existe una estación.
#[must_use]
pub fn apply_station_availability_callback(
    gfx: &TrainSpriteGraphics,
    local_id: u8,
    station: &mut Station,
) -> bool {
    let mut ctx = action2_eval_ctx_from_station(station);
    let result = gfx.resolve_callback_ctx(local_id, CBID_STATION_AVAILABILITY, 0, 0, &mut ctx);
    writeback_station_persistent_registers(station, &ctx);
    callback_allows_8bit_boolean(result)
}

/// Ejecuta la disponibilidad de un `RoadStop` `NewGRF` (`CBID 0x13`) antes de
/// mostrar/aceptar la construcción.
///
/// `OpenTTD` construye este resolver sin estación ni tesela (`st = nullptr`,
/// `tile = INVALID_TILE`) y suministra tipo de parada y carretera actuales.
/// Este call site cubre esas variables estables (`0x40`, `0x41`, `0x43`,
/// `0x44`) y conserva el comportamiento seguro de `CALLBACK_FAILED` = permitir.
/// Los callbacks de animación y scopes vecinos siguen fuera de alcance.
#[must_use]
pub fn apply_road_stop_availability_callback(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    stop_kind: StopKind,
    road_type: RoadType,
    road_type_catalog: &[crate::road_type::RoadTypeDef],
) -> bool {
    if !def.has_availability_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };

    let mut ctx = Action2EvalCtx::default();
    // `RoadStopScopeResolver::GetVariable` upstream:
    // 0x40 view = 0 in construcción; 0x41 bus=0/truck=1/waypoint=2.
    ctx.vars.insert(0x40, 0);
    let stop_type = match stop_kind {
        StopKind::BusStop => 0,
        StopKind::TruckStop => 1,
        _ => 2,
    };
    ctx.vars.insert(0x41, stop_type);
    // En el picker no hay tesela: terreno es plano/default. La traducción de
    // road/tram usa la tabla GlobalVar del mismo GRF, igual que
    // `GetReverseRoadTypeTranslation` upstream.
    ctx.vars.insert(0x42, 0);
    let local_type = crate::newgrf_type_tables::reverse_road_type(
        def.newgrf_type_tables.as_ref(),
        road_type_catalog,
        road_type,
    );
    match road_type.road_tram_type() {
        crate::RoadTramType::Road => {
            ctx.vars.insert(0x43, u32::from(local_type));
            ctx.vars.insert(0x44, u32::MAX);
        }
        crate::RoadTramType::Tram => {
            ctx.vars.insert(0x43, u32::MAX);
            ctx.vars.insert(0x44, u32::from(local_type));
        }
    }
    let result = runtime.resolve_callback_ctx(
        def.newgrf_local_id,
        CBID_STATION_AVAILABILITY,
        0,
        0,
        &mut ctx,
    );
    callback_allows_8bit_boolean(result)
}

/// Resuelve un callback de animación de `RoadStop` con el scope estable que
/// necesita este corte de runtime.
///
/// La parada actual aporta vista, tipo, frame y storage persistente. Los
/// scopes vecinos, road/tram por tesela y textos/sounds del resultado siguen
/// siendo una extensión separada; `0x43`/`0x44` se marcan inválidos en este
/// scheduler porque no participa de una query de construcción.
fn resolve_road_stop_animation_callback(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    view: u8,
    callback: u16,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };

    let mut ctx = action2_eval_ctx_from_station(station);
    ctx.random_bits = param1;
    ctx.vars.insert(0x40, u32::from(view));
    ctx.vars.insert(
        0x41,
        match station.stop_kind {
            StopKind::BusStop => 0,
            StopKind::TruckStop => 1,
            _ => 2,
        },
    );
    // Las animaciones se ejecutan sobre una tesela ya construida. La capa de
    // mapa no guarda aún la identidad road/tram necesaria para el scope 0x43/
    // 0x44, por lo que usamos el valor inválido contractual de OpenTTD.
    ctx.vars.insert(0x42, 0);
    ctx.vars.insert(0x43, u32::MAX);
    ctx.vars.insert(0x44, u32::MAX);
    ctx.vars
        .insert(0x49, u32::from(station.road_stop_animation_frame));
    let result =
        runtime.resolve_callback_ctx(def.newgrf_local_id, callback, param1, param2, &mut ctx);
    writeback_station_persistent_registers(station, &ctx);
    result
}

fn road_stop_animation_random_bits(station: &Station, tick: u64) -> u32 {
    let x = station.pos.x.cast_unsigned();
    let y = station.pos.y.cast_unsigned();
    let tick = u32::try_from(tick).unwrap_or(u32::MAX);
    x.wrapping_mul(0x9E37_79B9)
        ^ y.wrapping_mul(0x85EB_CA6B)
        ^ tick.rotate_left(11)
        ^ u32::from(station.newgrf_random_bits)
}

/// Ejecuta `CBID_STATION_ANIMATION_TRIGGER` (`0x140`) para una parada vial.
///
/// `0xFD` no cambia nada, `0xFE` registra la parada para animación, `0xFF`
/// la quita y cualquier otro byte fija el frame y la activa, igual que
/// `AnimationBase::ChangeAnimationFrame` de `OpenTTD`. Devuelve si cambió el
/// estado persistente de la parada.
pub fn trigger_road_stop_animation(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    view: u8,
    trigger: u16,
    tick: u64,
) -> bool {
    if def.animation_triggers & trigger == 0 {
        return false;
    }
    let before = (
        station.road_stop_animation_frame,
        station.road_stop_animation_active,
    );
    let result = resolve_road_stop_animation_callback(
        def,
        station,
        view,
        CBID_STATION_ANIMATION_TRIGGER,
        road_stop_animation_random_bits(station, tick),
        u32::from(trigger),
    );
    if result == CALLBACK_FAILED {
        return false;
    }
    match (result & 0xFF) as u8 {
        0xFD => {}
        0xFE => station.road_stop_animation_active = true,
        0xFF => station.road_stop_animation_active = false,
        frame => {
            station.road_stop_animation_frame = frame;
            station.road_stop_animation_active = true;
        }
    }
    before
        != (
            station.road_stop_animation_frame,
            station.road_stop_animation_active,
        )
}

/// Avanza el scheduler de una parada vial `NewGRF` (`CB141`/`CB142`).
///
/// El frame y el bit activo pertenecen a la instancia de estación, no al
/// catálogo del GRF; por eso sobreviven al JSON save/load y a la rehidratación
/// de Action2. `CALLBACK_FAILED` conserva la secuencia declarada en Action0.
pub fn advance_road_stop_animation(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    view: u8,
    tick: u64,
) -> bool {
    if !station.road_stop_animation_active {
        return false;
    }
    let before = (
        station.road_stop_animation_frame,
        station.road_stop_animation_active,
    );
    let mut speed = def.animation_speed.min(16);
    if def.has_animation_speed_callback() {
        let result = resolve_road_stop_animation_callback(
            def,
            station,
            view,
            CBID_STATION_ANIMATION_SPEED,
            0,
            0,
        );
        if result != CALLBACK_FAILED {
            speed = u8::try_from(result & 0xFF).unwrap_or(16).min(16);
        }
    }
    if !tick.is_multiple_of(1_u64 << u32::from(speed)) {
        return false;
    }

    let mut frame_set_by_callback = false;
    if def.has_animation_next_frame_callback() {
        let random_bits = if def.animation_next_frame_uses_random_bits() {
            road_stop_animation_random_bits(station, tick)
        } else {
            0
        };
        let result = resolve_road_stop_animation_callback(
            def,
            station,
            view,
            CBID_STATION_ANIMATION_NEXT_FRAME,
            random_bits,
            0,
        );
        if result != CALLBACK_FAILED {
            match (result & 0xFF) as u8 {
                0xFF => station.road_stop_animation_active = false,
                0xFE => {}
                frame => {
                    station.road_stop_animation_frame = frame;
                    frame_set_by_callback = true;
                }
            }
        }
    }

    if station.road_stop_animation_active && !frame_set_by_callback {
        if station.road_stop_animation_frame < def.animation_frames {
            station.road_stop_animation_frame = station.road_stop_animation_frame.saturating_add(1);
        } else if station.road_stop_animation_frame == def.animation_frames && def.animation_loops()
        {
            station.road_stop_animation_frame = 0;
        } else {
            station.road_stop_animation_active = false;
        }
    }
    before
        != (
            station.road_stop_animation_frame,
            station.road_stop_animation_active,
        )
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

/// Resuelve un callback de animación de tesela de industria con contexto estable.
///
/// Los parámetros siguen el contrato de `OpenTTD`: `param1` son random bits y
/// `param2` el trigger. Además se exponen las coordenadas reales en `0x40`/`0x41`
/// para que Action2 no dependa de una coordenada ficticia del helper.
#[must_use]
pub fn resolve_industry_tile_animation_callback(
    def: &IndustryTileSpecDef,
    callback: u16,
    coord: TileCoord,
    param1: u32,
    param2: u32,
) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };
    let mut ctx = Action2EvalCtx {
        random_bits: param1,
        ..Action2EvalCtx::default()
    };
    ctx.vars.insert(0x40, coord.x.cast_unsigned());
    ctx.vars.insert(0x41, coord.y.cast_unsigned());
    runtime.resolve_callback_ctx(def.newgrf_local_id, callback, param1, param2, &mut ctx)
}

/// Compatibilidad con el helper anterior: consulta next-frame sin contexto.
#[must_use]
pub fn apply_industry_tile_anim_callback(def: &IndustryTileSpecDef) -> u16 {
    resolve_industry_tile_animation_callback(
        def,
        crate::newgrf_sprites::CBID_INDTILE_ANIMATION_NEXT_FRAME,
        TileCoord::new(0, 0),
        0,
        0,
    )
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
    fn callbacks_ac_location_and_8bit_boolean_semantics_match_upstream() {
        assert!(callback_allows_location(CALLBACK_FAILED));
        assert!(callback_allows_location(0x400));
        assert!(!callback_allows_location(0xFF));
        assert!(!callback_allows_location(0x401));

        assert!(callback_allows_8bit_boolean(CALLBACK_FAILED));
        assert!(!callback_allows_8bit_boolean(0));
        assert!(callback_allows_8bit_boolean(1));
        assert!(callback_allows_8bit_boolean(0xFF));
        assert!(!callback_allows_8bit_boolean(0x100));
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
            callback_mask: crate::industry_spec::INDUSTRY_CALLBACK_LOCATION_MASK,
            cost_multiplier: 0,
            name: "test".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_literal(0x10))),
        };
        assert!(!apply_industry_location_callback(&def));
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_400()));
        assert!(apply_industry_location_callback(&def));
        def.newgrf_runtime = None;
        assert!(apply_industry_location_callback(&def));
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x10)));
        def.callback_mask = 0;
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
            callback_mask: crate::house_spec::HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK,
            name: "cb-house".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_literal(0x01))),
        };
        assert!(apply_house_construction_callback(&house));
        house.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert!(!apply_house_construction_callback(&house));
        house.newgrf_runtime = Some(Box::new(gfx_callback_literal(0xFF)));
        assert!(apply_house_construction_callback(&house));
        house.callback_mask = 0;
        house.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
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
        assert_eq!(loaded.stations[0].newgrf_persistent_regs.get(&2), Some(&99));
    }

    #[test]
    fn callbacks_ac_road_stop_animation_writes_back_station_storage() {
        let def = crate::RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "anim".into(),
            short_label: "ANIM".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            flags: 0,
            callback_mask: 0,
            animation_status: 1,
            animation_frames: 1,
            animation_speed: 0,
            animation_triggers: crate::ROADSTOP_ANIMATION_TRIGGER_BUILT,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(gfx_callback_psto(4, 12, 0xFE))),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        };
        let mut station = Station::new_with_kind(TileCoord::new(1, 1), StopKind::BusStop);
        assert!(trigger_road_stop_animation(
            &def,
            &mut station,
            crate::RSV_BAY_NE,
            crate::ROADSTOP_ANIMATION_TRIGGER_BUILT,
            1,
        ));
        assert!(station.road_stop_animation_active);
        assert_eq!(station.newgrf_persistent_regs.get(&4), Some(&12));
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
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
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
        tile.newgrf_runtime
            .as_mut()
            .unwrap()
            .action2_random
            .get_mut(&3)
            .unwrap()
            .triggers = 0x04;
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
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 0,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        assert_eq!(apply_industry_tile_anim_callback(&tile), CALLBACK_FAILED);
    }
}
