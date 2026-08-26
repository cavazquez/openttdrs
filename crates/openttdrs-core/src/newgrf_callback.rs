//! API común de resolución de callbacks `NewGRF` (#228 / #266).
//!
//! - Fallo observable: [`CALLBACK_FAILED`] (nunca se acepta un resultado “silencioso”).
//! - Storage: tras eval, writeback de `7C`/`\2psto` a vehículo o estación;
//!   los registros temporales (`7D`/`\2sto`) viven solo en el ctx y se descartan.
//! - Call sites #266: industry location, house/object construction, station availability,
//!   industry-tile trigger → Action2 random.

use crate::cargo_spec::CargoSpecDef;
use crate::engine::EngineDef;
use crate::house_spec::HouseSpecDef;
use crate::industry_spec::IndustrySpecDef;
use crate::industry_tile::IndustryTileSpecDef;
use crate::map::{Map, TileCoord};
use crate::newgrf_sprites::{
    Action2EvalCtx, Action2RandomEntry, CALLBACK_FAILED, CBID_CARGO_PROFIT_CALC,
    CBID_CARGO_STATION_RATING_CALC, CBID_HOUSE_ALLOW_CONSTRUCTION, CBID_INDUSTRY_LOCATION,
    CBID_OBJECT_LAND_SLOPE_CHECK, CBID_STATION_ANIMATION_NEXT_FRAME, CBID_STATION_ANIMATION_SPEED,
    CBID_STATION_ANIMATION_TRIGGER, CBID_STATION_AVAILABILITY, CBID_STATION_LAND_SLOPE_CHECK,
    CBID_VEHICLE_32DAY_CALLBACK, CBID_VEHICLE_ARTIC_ENGINE, CBID_VEHICLE_LENGTH,
    CBID_VEHICLE_LOAD_AMOUNT, CBID_VEHICLE_REFIT_CAPACITY, CBID_VEHICLE_SOUND_EFFECT,
    CBID_VEHICLE_START_STOP_CHECK, CBID_VEHICLE_VISUAL_EFFECT, TrainSpriteGraphics,
};
use crate::object_spec::ObjectSpecDef;
use crate::road_stop_action2::{
    RoadStopWorldContext, action2_eval_ctx_for_road_stop_tile_with_catalog_and_world,
};
use crate::road_stop_spec::RoadStopSpecDef;
use crate::station::Station;
use crate::station_class::{StationAnimationTrigger, StationRandomTrigger};
use crate::town::Town;
use crate::vehicle::{Vehicle, VehicleKind, VehicleRandomTrigger};
use crate::{CargoType, GameState, RoadType, SoundId, StopKind, VehicleSoundEvent};

/// Contexto que el scheduler de callbacks necesita para reproducir los scopes
/// de una parada vial ya colocada. El renderer ya usaba estos pools; ahora las
/// rutas CB140–CB142 reciben el mismo mapa, catálogo y contexto de mundo.
#[derive(Clone, Copy)]
pub struct RoadStopCallbackWorld<'a> {
    pub map: &'a Map,
    pub road_stop_catalog: &'a [RoadStopSpecDef],
    pub towns: &'a [Town],
    pub companies: &'a [crate::company::Company],
    pub industries: &'a [crate::industry::Industry],
    pub road_type_catalog: &'a [crate::road_type::RoadTypeDef],
    pub climate: crate::Climate,
}

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
    ctx.vehicle_loading = vehicle.cargo_loading || vehicle.cargo_unloading;
    ctx.vehicle_cargo = vehicle.cargo;
    ctx.vehicle_capacity = vehicle.capacity;
    // `5F` combines the random bits (bits 8..15 in the vehicle scope) with
    // triggers still waiting to be consumed (bits 0..7).  Keeping this value
    // in the callback context is important for `CBID_RANDOM_TRIGGER` paths:
    // a GRF can deliberately defer a trigger until another one arrives.
    ctx.vars.insert(
        0x5F,
        u32::from(vehicle.newgrf_random_bits) << 8
            | u32::from(vehicle.newgrf_waiting_random_triggers),
    );
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
    let result = runtime.resolve_callback_ctx_u16(
        engine.newgrf_local_id,
        callback,
        param1,
        param2,
        &mut ctx,
    );
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

/// Resultado normalizado de `CBID_VEHICLE_32DAY_CALLBACK` (`0x32`).
///
/// Sólo los bits 0 y 1 tienen significado en `OpenTTD`: el primero solicita el
/// trigger de randomización 32-day y el segundo invalida la paleta/cache del
/// vehículo. Los bits restantes se devuelven en `unknown_bits` para que el
/// caller pueda diagnosticarlos sin convertir un resultado inválido en una
/// decisión silenciosa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vehicle32DayCallback {
    pub trigger_randomisation: bool,
    pub invalidate_palette: bool,
    pub unknown_bits: u16,
}

/// Ejecuta `CBID_VEHICLE_32DAY_CALLBACK` (`0x32`).
///
/// Este callback no tiene bit en la máscara Action0: la presencia de una
/// asignación Action3/runtime es la condición de `OpenTTD`. `CALLBACK_FAILED`
/// conserva la semántica de «sin callback» y devuelve `None`.
#[must_use]
pub fn resolve_vehicle_32day_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
) -> Option<Vehicle32DayCallback> {
    if engine.newgrf_grfid == 0 || engine.newgrf_runtime.is_none() {
        return None;
    }
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_32DAY_CALLBACK, 0, 0);
    if result == CALLBACK_FAILED {
        return None;
    }
    Some(Vehicle32DayCallback {
        trigger_randomisation: result & 1 != 0,
        invalidate_palette: result & 2 != 0,
        unknown_bits: result & !0x0003,
    })
}

/// Reevalúa el grupo Action2 activo tras un trigger de vehículo y reseedea
/// únicamente los bits que el grupo declara. La operación es determinista
/// por mundo/tick/vehículo para que replay y tests no dependan del orden de
/// iteración del RNG global.
///
/// `OpenTTD` mantiene `waiting_random_triggers` hasta que el grupo activo los
/// consume; esta implementación conserva exactamente ese comportamiento para
/// el scope propio. La máscara de random bits del port sigue siendo de ocho
/// bits, por lo que un `randbit` fuera de ese rango queda pendiente para la
/// ampliación del almacenamiento a 16 bits.
pub fn trigger_vehicle_randomisation(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
) -> bool {
    let Some(runtime) = engine.newgrf_runtime.as_ref() else {
        return false;
    };
    let before_random = vehicle.newgrf_random_bits;
    let before_waiting = vehicle.newgrf_waiting_random_triggers;
    vehicle.newgrf_waiting_random_triggers |= trigger.mask();
    let waiting = vehicle.newgrf_waiting_random_triggers;
    let mut ctx = action2_eval_ctx_from_vehicle(vehicle);
    ctx.vars.insert(
        0x5F,
        u32::from(vehicle.newgrf_random_bits) << 8 | u32::from(waiting),
    );
    let (reseed, used) =
        runtime.rerandomisation_for_local_id_u16(engine.newgrf_local_id, &mut ctx, waiting);
    writeback_vehicle_persistent_registers(vehicle, &ctx);
    vehicle.newgrf_waiting_random_triggers &= !used;

    let reseed_mask = u8::try_from(reseed & 0xFF).unwrap_or(0);
    if reseed_mask != 0 {
        let random = crate::map::industry_tile_rng(
            world_seed,
            tick,
            vehicle.pos,
            u64::from(vehicle.id) ^ (u64::from(trigger as u8) << 32),
        );
        vehicle.newgrf_random_bits = (before_random & !reseed_mask) | (random & reseed_mask);
    }
    before_random != vehicle.newgrf_random_bits
        || before_waiting != vehicle.newgrf_waiting_random_triggers
}

/// Resultado normalizado de `CBID_VEHICLE_VISUAL_EFFECT` (`0x10`).
///
/// El callback devuelve el byte bit-stuffed de `VisualEffect`: los bits 4–5
/// seleccionan vapor/diésel/chispa y el bit 6 desactiva el efecto. El valor
/// cero solicita la clase por defecto del motor; `OpenTTD` también conserva el
/// bit 7 (potencia de vagón), pero ese detalle no altera el renderer de humo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleVisualEffectKind {
    /// Resolver según la clase del motor (la ruta vanilla).
    Default,
    /// El vehículo no debe emitir humo/chispas.
    Disabled,
    Steam,
    Diesel,
    Electric,
}

/// Resuelve `CBID_VEHICLE_VISUAL_EFFECT` (`0x10`) con la máscara Action0.
///
/// Un resultado fuera del byte permitido o la ausencia de runtime se trata
/// como `None`, dejando que el caller aplique la propiedad/default vanilla.
#[must_use]
pub fn resolve_vehicle_visual_effect_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
) -> Option<VehicleVisualEffectKind> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 0) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_VISUAL_EFFECT, 0, 0);
    if result == CALLBACK_FAILED || result >= 0x100 {
        return None;
    }
    Some(decode_vehicle_visual_effect(u8::try_from(result).ok()?))
}

fn decode_vehicle_visual_effect(value: u8) -> VehicleVisualEffectKind {
    if value == crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT {
        return VehicleVisualEffectKind::Default;
    }
    if value & (1 << 6) != 0 {
        return VehicleVisualEffectKind::Disabled;
    }
    match (value >> 4) & 0x03 {
        1 => VehicleVisualEffectKind::Steam,
        2 => VehicleVisualEffectKind::Diesel,
        3 => VehicleVisualEffectKind::Electric,
        _ => VehicleVisualEffectKind::Default,
    }
}

/// Obtiene el efecto efectivo de un vehículo, aplicando CB10 cuando procede.
///
/// Si el callback no aplica, decodifica la propiedad Action0 conservada en el
/// catálogo. `0xFF` delega en la selección vanilla por clase; el callback sí es
/// ejecutado y sus registros persistentes se escriben de vuelta al vehículo.
#[must_use]
pub fn vehicle_visual_effect_kind(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
) -> VehicleVisualEffectKind {
    resolve_vehicle_visual_effect_callback(engine, vehicle)
        .unwrap_or_else(|| decode_vehicle_visual_effect(engine.visual_effect))
}

/// Resuelve `CBID_VEHICLE_LOAD_AMOUNT` (`0x12`) para carga gradual.
///
/// El callback devuelve un `BYTE`; `CALLBACK_FAILED` o cero conservan la
/// propiedad `load_amount` del motor. Un resultado fuera de ocho bits se trata
/// como inválido y también cae al valor de la propiedad, evitando que un GRF
/// mal formado detenga el pipeline de carga.
#[must_use]
pub fn resolve_vehicle_load_amount_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
) -> Option<u8> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 2) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_LOAD_AMOUNT, 0, 0);
    if result == CALLBACK_FAILED || result == 0 || result >= 0x100 {
        return None;
    }
    u8::try_from(result).ok()
}

/// Resuelve `CBID_VEHICLE_LENGTH` (`0x11`) y devuelve la longitud efectiva.
///
/// El callback de `OpenTTD` devuelve cuánto se acorta una unidad de ocho
/// fracciones de tesela (`0..=7`), no la longitud final. Un resultado fuera de
/// rango o `CALLBACK_FAILED` conserva la propiedad `shorten_factor` del motor.
#[must_use]
pub fn resolve_vehicle_length_callback(engine: &EngineDef, vehicle: &mut Vehicle) -> Option<u8> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 1) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_LENGTH, 0, 0);
    (result < 8).then(|| 8_u8.saturating_sub(u8::try_from(result).unwrap_or(7)))
}

/// Longitud de una unidad de vehículo al crear/refrescar su caché.
#[must_use]
pub fn vehicle_unit_length(engine: &EngineDef, vehicle: &mut Vehicle) -> u8 {
    resolve_vehicle_length_callback(engine, vehicle)
        .unwrap_or_else(|| 8_u8.saturating_sub(engine.shorten_factor.min(7)))
        .max(1)
}

/// Parte articulada devuelta por `CBID_VEHICLE_ARTIC_ENGINE` (`0x16`).
///
/// `local_id` es el id del motor dentro del mismo GRF y `mirrored` conserva el
/// bit de orientación que `OpenTTD` aplica al sprite de la pieza. El catálogo
/// actual usa ids locales de 8 bits, pero se conserva `u16` para no truncar la
/// codificación de GRF v8 (14 bits) antes de buscar el motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleArticulatedPart {
    pub local_id: u16,
    pub mirrored: bool,
}

/// Decodifica el resultado bruto de `CBID_VEHICLE_ARTIC_ENGINE`.
///
/// GRF anteriores a la versión 8 usan un callback de 8 bits (`0xFF` termina y
/// el bit 7 indica espejo); GRF v8+ usan 15 bits (`0x7FFF` termina y el bit 14
/// indica espejo). Una versión cero significa que el encabezado no fue
/// escaneado y se trata como GRF moderno, que es el fallback seguro para
/// entradas creadas por la API.
#[must_use]
pub fn decode_vehicle_articulated_part(
    result: u16,
    grf_version: u8,
) -> Option<VehicleArticulatedPart> {
    if result == CALLBACK_FAILED {
        return None;
    }
    if grf_version != 0 && grf_version < 8 {
        let value = result & 0x00FF;
        if value == 0x00FF {
            return None;
        }
        return Some(VehicleArticulatedPart {
            local_id: value & 0x007F,
            mirrored: value & 0x0080 != 0,
        });
    }
    if result == 0x7FFF {
        return None;
    }
    Some(VehicleArticulatedPart {
        local_id: result & 0x3FFF,
        mirrored: result & 0x4000 != 0,
    })
}

/// Ejecuta `CBID_VEHICLE_ARTIC_ENGINE` para la posición `index` (1..=100).
///
/// `None` significa callback ausente/fallido o terminador; cualquier registro
/// persistente escrito por Action2 se conserva en `vehicle`, igual que en los
/// demás callbacks de vehículo.
#[must_use]
pub fn resolve_vehicle_articulated_part_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    index: u8,
    grf_version: u8,
) -> Option<VehicleArticulatedPart> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 4) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let result = resolve_vehicle_callback(
        engine,
        vehicle,
        CBID_VEHICLE_ARTIC_ENGINE,
        u32::from(index),
        0,
    );
    decode_vehicle_articulated_part(result, grf_version)
}

/// Resuelve `CBID_VEHICLE_REFIT_CAPACITY` (`0x15`) para un cargo objetivo.
///
/// `OpenTTD` evalúa el callback con `Vehicle::cargo_type` ya cambiado al cargo
/// solicitado y devuelve la capacidad final (15 bits). El tipo original se
/// restaura antes de volver al caller; los registros `7C` sí se conservan en
/// el vehículo, como en cualquier callback con scope de vehículo. Un callback
/// fallido deja que el motor aplique su propiedad y multiplicador normales.
#[must_use]
pub fn resolve_vehicle_refit_capacity_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    cargo: CargoType,
) -> Option<u32> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 3) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let previous_cargo = vehicle.cargo_type;
    vehicle.cargo_type = Some(cargo);
    let result = resolve_vehicle_callback(engine, vehicle, CBID_VEHICLE_REFIT_CAPACITY, 0, 0);
    vehicle.cargo_type = previous_cargo;
    (result != CALLBACK_FAILED).then_some(u32::from(result))
}

/// Resultado del callback de sonido de un vehículo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleSoundOverride {
    /// El callback no aplica y debe sonar el efecto vanilla del evento.
    Default,
    /// El callback devolvió un sample global del baseset (`0..72`).
    Base(SoundId),
    /// El callback devolvió un sample local del GRF (`73 + local_id`).
    Newgrf { grfid: u32, local_id: u8 },
    /// El callback devolvió un id válido pero sin sample reproducible.
    Suppressed,
}

/// Resuelve `CBID_VEHICLE_SOUND_EFFECT` (`0x33`) para un vehículo real.
///
/// `OpenTTD` pasa el evento en `param1` y `0` en `param2`. Un callback fallido
/// conserva el SFX vanilla; un id global selecciona un sample del baseset; un
/// id desde `SOUND_COUNT` se traduce al espacio local del GRF. Un id custom
/// ausente o sin PCM suprime el sonido, igual que `INVALID_SOUND` upstream.
#[must_use]
pub fn resolve_vehicle_sound_callback(
    state: &mut GameState,
    vehicle_id: u32,
    event: VehicleSoundEvent,
) -> VehicleSoundOverride {
    let Some(vehicle_index) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return VehicleSoundOverride::Default;
    };
    let Some(engine_id) = state.vehicles[vehicle_index].engine_id else {
        return VehicleSoundOverride::Default;
    };
    let Some(engine) = state
        .engine_catalog
        .iter()
        .find(|e| e.id == engine_id)
        .cloned()
    else {
        return VehicleSoundOverride::Default;
    };
    if engine.newgrf_grfid == 0 || engine.vehicle_callback_mask & (1 << 7) == 0 {
        return VehicleSoundOverride::Default;
    }
    let result = resolve_vehicle_callback(
        &engine,
        &mut state.vehicles[vehicle_index],
        CBID_VEHICLE_SOUND_EFFECT,
        event as u32,
        0,
    );
    if result == CALLBACK_FAILED {
        return VehicleSoundOverride::Default;
    }
    let sound_count = u16::try_from(crate::sound_id::SOUND_COUNT).unwrap_or(u16::MAX);
    if result < sound_count {
        return u8::try_from(result)
            .ok()
            .and_then(SoundId::from_u8)
            .map_or(VehicleSoundOverride::Suppressed, VehicleSoundOverride::Base);
    }
    let Some(local_id) = u8::try_from(result - sound_count).ok() else {
        return VehicleSoundOverride::Suppressed;
    };
    let Some(def) =
        crate::sound_effect_def(&state.sound_effect_catalog, engine.newgrf_grfid, local_id)
    else {
        return VehicleSoundOverride::Suppressed;
    };
    if !def.has_sample || def.sample_pcm.is_empty() {
        return VehicleSoundOverride::Suppressed;
    }
    VehicleSoundOverride::Newgrf {
        grfid: engine.newgrf_grfid,
        local_id,
    }
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

/// Convierte un resultado callback de 15 bits al entero con signo de `OpenTTD`.
fn signed_15_bit_callback_result(result: u16) -> i64 {
    let low_14 = i64::from(result & 0x3FFF);
    if result & 0x4000 != 0 {
        low_14 - 0x4000
    } else {
        low_14
    }
}

/// Resuelve CB `0x39` de cargo y devuelve el ingreso que reemplaza al cálculo base.
///
/// `OpenTTD` entrega `param1=0` y empaqueta en `param2` la distancia (`u16`),
/// cantidad (`u8`) y períodos de tránsito (`u8`). El resultado es un
/// multiplicador firmado de 15 bits aplicado a `count * current_payment / 8192`.
/// Si el callback falla o no está declarado, `None` conserva la fórmula base.
/// El resolver cubre esos parámetros genéricos; los scopes avanzados de cargo
/// todavía no están implementados.
#[must_use]
pub fn resolve_cargo_profit_callback(
    def: &CargoSpecDef,
    count: u32,
    distance: u32,
    transit_periods: u16,
    current_payment: i64,
) -> Option<i64> {
    if !def.has_profit_calc_callback() {
        return None;
    }
    let runtime = def.newgrf_runtime.as_ref()?;
    let param2 = distance.min(u32::from(u16::MAX))
        | (count.min(u32::from(u8::MAX)) << 16)
        | (u32::from(transit_periods.min(u16::from(u8::MAX))) << 24);
    let result = runtime.resolve_callback(def.id, CBID_CARGO_PROFIT_CALC, 0, param2);
    if result == CALLBACK_FAILED {
        return None;
    }
    let multiplier = signed_15_bit_callback_result(result);
    Some(
        multiplier
            .saturating_mul(i64::from(count))
            .saturating_mul(current_payment)
            / 8192,
    )
}

/// Codificación histórica de tipo de vehículo que recibe CB145 en `var 10`.
const fn cargo_station_rating_vehicle_param(last_vehicle_kind: Option<VehicleKind>) -> u32 {
    match last_vehicle_kind {
        None => 0,
        Some(VehicleKind::Train) => 0x10,
        Some(VehicleKind::Truck | VehicleKind::Bus | VehicleKind::Tram) => 0x11,
        Some(VehicleKind::Ship) => 0x12,
        Some(VehicleKind::Aircraft) => 0x13,
    }
}

/// Resuelve CB `0x145` de cargo para el target de rating de una estación.
///
/// `param1` conserva el tipo histórico del último vehículo y `param2` empaqueta
/// días sin recogida (`u8`), máximo de carga esperando (`u16`) y última velocidad
/// (`u8`; `0xFF` si nunca llegó un vehículo). Si el callback no está disponible,
/// devuelve `None` para conservar el algoritmo de rating estándar.
#[must_use]
pub fn resolve_cargo_station_rating_callback(
    def: &CargoSpecDef,
    time_since_pickup: u8,
    max_waiting_cargo: u32,
    has_vehicle_ever_tried_loading: bool,
    last_speed: u8,
    last_vehicle_kind: Option<VehicleKind>,
) -> Option<i16> {
    if !def.has_station_rating_callback() {
        return None;
    }
    let runtime = def.newgrf_runtime.as_ref()?;
    let speed = if has_vehicle_ever_tried_loading {
        last_speed
    } else {
        u8::MAX
    };
    let param2 = u32::from(time_since_pickup)
        | (max_waiting_cargo.min(u32::from(u16::MAX)) << 8)
        | (u32::from(speed) << 24);
    let result = runtime.resolve_callback(
        def.id,
        CBID_CARGO_STATION_RATING_CALC,
        cargo_station_rating_vehicle_param(last_vehicle_kind),
        param2,
    );
    if result == CALLBACK_FAILED {
        return None;
    }
    i16::try_from(signed_15_bit_callback_result(result)).ok()
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

/// Call site de construcción ferroviaria: CB `0x149` de pendiente por tesela.
///
/// `OpenTTD` entrega el slope original y, para eje Y con esquinas W/E a distinta
/// altura, una variante orientada en los nibbles alto/bajo de `param1`.
/// `param2` empaqueta cantidad de andenes, longitud y los offsets de andén y
/// posición. Se resuelve antes de que exista una estación; por eso este corte no
/// aporta scope/registro persistente de estación ni la inversión de bit 10 de
/// GRFs anteriores a versión 8.
#[must_use]
pub fn apply_station_slope_callback_for_build(
    def: &crate::station_class::StationSpecDef,
    slope: u8,
    axis_y: bool,
    platforms: u8,
    length: u8,
    platform: u8,
    position: u8,
) -> bool {
    if !def.has_slope_check_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };

    // `PerformStationTileSlopeCheck`: SLOPE_W=1, SLOPE_E=4, SLOPE_EW=5.
    let axis_adjusted_slope = if axis_y && ((slope & 1 != 0) != (slope & 4 != 0)) {
        slope ^ 5
    } else {
        slope
    };
    let param1 = (u32::from(slope) << 4) | u32::from(axis_adjusted_slope);
    let param2 = (u32::from(platforms) << 24)
        | (u32::from(length) << 16)
        | (u32::from(platform) << 8)
        | u32::from(position);
    let result = runtime.resolve_callback(
        def.newgrf_local_id,
        CBID_STATION_LAND_SLOPE_CHECK,
        param1,
        param2,
    );
    callback_allows_location(result)
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
/// El picker no tiene una tesela base; las consultas de vecindad sólo se
/// resuelven al renderizar una parada ya colocada. Los callbacks de animación
/// siguen usando el scope local del scheduler.
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
#[allow(clippy::too_many_arguments)]
fn resolve_road_stop_animation_callback(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    view: u8,
    callback: u16,
    param1: u32,
    param2: u32,
    world: Option<RoadStopCallbackWorld<'_>>,
) -> u16 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return CALLBACK_FAILED;
    };

    let mut ctx = world.map_or_else(
        || action2_eval_ctx_from_road_stop(station, tile),
        |world| action2_eval_ctx_from_road_stop_with_world(station, tile, view, world),
    );
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
    // Las APIs legacy no tienen mapa y conservan los sentinelas de compra.
    // Cuando el scheduler entrega mundo, la ruta anterior ya materializó
    // terreno, road/tram, pueblo, compañía y vecindad reales.
    if world.is_none() {
        ctx.vars.insert(0x42, 0);
        ctx.vars.insert(0x43, u32::MAX);
        ctx.vars.insert(0x44, u32::MAX);
    }
    ctx.vars
        .insert(0x49, u32::from(station.road_stop_animation_frame_at(tile)));
    let result =
        runtime.resolve_callback_ctx(def.newgrf_local_id, callback, param1, param2, &mut ctx);
    writeback_station_persistent_registers(station, &ctx);
    result
}

fn road_stop_animation_random_bits(station: &Station, tile: TileCoord, tick: u64) -> u32 {
    let x = tile.x.cast_unsigned();
    let y = tile.y.cast_unsigned();
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
    trigger: StationAnimationTrigger,
    cargo_local_id: Option<u8>,
    tick: u64,
) -> bool {
    trigger_road_stop_animation_at(
        def,
        station,
        station.pos,
        view,
        trigger,
        cargo_local_id,
        tick,
    )
}

/// Variante por tesela de [`trigger_road_stop_animation`].
///
/// Las paradas unidas pueden usar specs y frames distintos. `tile` es por lo
/// tanto obligatorio en los call sites runtime; el wrapper legacy sólo queda
/// para la parada 1×1 y fixtures anteriores.
pub fn trigger_road_stop_animation_at(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    view: u8,
    trigger: StationAnimationTrigger,
    cargo_local_id: Option<u8>,
    tick: u64,
) -> bool {
    trigger_road_stop_animation_at_with_world(
        def,
        station,
        tile,
        view,
        trigger,
        cargo_local_id,
        tick,
        None,
    )
}

/// Variante de [`trigger_road_stop_animation_at`] con el contexto completo
/// del mundo para los scopes `RoadStop` de los callbacks.
#[allow(clippy::too_many_arguments)]
pub fn trigger_road_stop_animation_at_with_world(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    view: u8,
    trigger: StationAnimationTrigger,
    cargo_local_id: Option<u8>,
    tick: u64,
    world: Option<RoadStopCallbackWorld<'_>>,
) -> bool {
    if def.animation_triggers & trigger.mask() == 0 {
        return false;
    }
    let before = (
        station.road_stop_animation_frame_at(tile),
        station
            .road_stop_tile_state(tile)
            .map_or(station.road_stop_animation_active, |state| {
                state.animation_active
            }),
    );
    let result = resolve_road_stop_animation_callback(
        def,
        station,
        tile,
        view,
        CBID_STATION_ANIMATION_TRIGGER,
        road_stop_animation_random_bits(station, tile, tick),
        trigger.callback_param(cargo_local_id),
        world,
    );
    if result == CALLBACK_FAILED {
        return false;
    }
    {
        let state = station.ensure_road_stop_tile_state(tile);
        match (result & 0xFF) as u8 {
            0xFD => {}
            0xFE => state.animation_active = true,
            0xFF => state.animation_active = false,
            frame => {
                state.animation_frame = frame;
                state.animation_active = true;
            }
        }
    }
    station.sync_legacy_road_stop_anchor();
    before
        != (
            station.road_stop_animation_frame_at(tile),
            station
                .road_stop_tile_state(tile)
                .map_or(station.road_stop_animation_active, |state| {
                    state.animation_active
                }),
        )
}

fn action2_eval_ctx_from_road_stop(station: &Station, tile: TileCoord) -> Action2EvalCtx {
    let mut ctx = action2_eval_ctx_from_station(station);
    ctx.random_bits = u32::from(station.newgrf_random_bits)
        | (u32::from(station.road_stop_random_bits_at(tile)) << 16);
    ctx
}

fn action2_eval_ctx_from_road_stop_with_world(
    station: &Station,
    tile: TileCoord,
    view: u8,
    world: RoadStopCallbackWorld<'_>,
) -> Action2EvalCtx {
    action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
        world.map,
        std::slice::from_ref(station),
        world.road_stop_catalog,
        RoadStopWorldContext {
            towns: world.towns,
            companies: world.companies,
            industries: world.industries,
            road_type_catalog: world.road_type_catalog,
        },
        tile,
        view,
        world.climate,
    )
}

fn road_stop_random_u16(world_seed: u64, tick: u64, pos: TileCoord, salt: u64) -> u16 {
    let low = crate::map::industry_tile_rng(world_seed, tick, pos, salt);
    let high = crate::map::industry_tile_rng(world_seed, tick, pos, salt ^ 0xA5A5_5A5A);
    u16::from(low) | (u16::from(high) << 8)
}

/// Datos de tiempo y mundo que necesita la randomización de una parada vial.
///
/// Se agrupan para que las rutas por tesela y legacy compartan exactamente la
/// misma fuente determinista sin añadir parámetros sueltos a cada call site.
#[derive(Debug, Clone, Copy)]
pub struct RoadStopRandomisationContext {
    pub climate: crate::Climate,
    pub world_seed: u64,
    pub tick: u64,
}

/// Aplica la randomización Action2 de una tesela `RoadStop`.
///
/// Replica la ruta de `TriggerRoadStopRandomisation`: la propiedad Action0
/// `0x0D` filtra cargos, los eventos se acumulan para grupos `all`, sólo se
/// reseedean los bits de grupos Action2 alcanzables y los triggers consumidos
/// se limpian del estado persistente. La fuente pseudoaleatoria es
/// determinista por mundo/tick/tesela para preservar replay y tests.
///
/// El wrapper legacy conserva la ancla de una parada 1×1. Los call sites de
/// simulación usan [`trigger_road_stop_randomisation_at`] para mantener los
/// datos random independientes de cada tesela compuesta.
pub fn trigger_road_stop_randomisation(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    trigger: StationRandomTrigger,
    cargo: Option<CargoType>,
    climate: crate::Climate,
    world_seed: u64,
    tick: u64,
) -> bool {
    trigger_road_stop_randomisation_at(
        def,
        station,
        station.pos,
        trigger,
        cargo,
        RoadStopRandomisationContext {
            climate,
            world_seed,
            tick,
        },
    )
}

/// Variante por tesela de [`trigger_road_stop_randomisation`].
pub fn trigger_road_stop_randomisation_at(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    trigger: StationRandomTrigger,
    cargo: Option<CargoType>,
    context: RoadStopRandomisationContext,
) -> bool {
    trigger_road_stop_randomisation_at_with_world(def, station, tile, trigger, cargo, context, None)
}

/// Variante de [`trigger_road_stop_randomisation_at`] con los scopes de mundo
/// disponibles para la evaluación Action2 que decide los bits a resembrar.
pub fn trigger_road_stop_randomisation_at_with_world(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    trigger: StationRandomTrigger,
    cargo: Option<CargoType>,
    context: RoadStopRandomisationContext,
    world: Option<RoadStopCallbackWorld<'_>>,
) -> bool {
    if !def.has_random_cargo_triggers() {
        return false;
    }
    if cargo.is_some_and(|cargo| !def.cargo_triggers_randomisation(cargo, context.climate)) {
        return false;
    }
    // `CargoTaken` sólo ocurre cuando *todos* los cargos que declaró este
    // spec ya se vaciaron; no basta con que se haya retirado el cargo recibido.
    if trigger == StationRandomTrigger::CargoTaken
        && crate::ALL_CARGO_TYPES.iter().copied().any(|candidate| {
            def.cargo_triggers_randomisation(candidate, context.climate)
                && station.cargo_stock.get(candidate) != 0
        })
    {
        return false;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return false;
    };

    station.newgrf_waiting_random_triggers |= trigger.mask();
    let waiting = station.newgrf_waiting_random_triggers;
    let mut ctx = world.map_or_else(
        || action2_eval_ctx_from_road_stop(station, tile),
        |world| {
            let view = world.map.get(tile).map_or(0, |tile| tile.m5);
            action2_eval_ctx_from_road_stop_with_world(station, tile, view, world)
        },
    );
    let random_bits = ctx.random_bits;
    ctx.vars
        .insert(0x5F, random_bits.wrapping_shl(8) | u32::from(waiting));
    let (reseed, used) =
        runtime.rerandomisation_for_local_id(def.newgrf_local_id, &mut ctx, waiting);
    writeback_station_persistent_registers(station, &ctx);
    station.newgrf_waiting_random_triggers &= !used;

    let base_mask = u16::try_from(reseed & 0xFFFF).unwrap_or(0);
    let tile_mask = u8::try_from((reseed >> 16) & 0xFF).unwrap_or(0);
    let mut changed = false;
    if base_mask != 0 {
        let random = road_stop_random_u16(
            context.world_seed,
            context.tick,
            tile,
            u64::from(trigger as u8) | (u64::from(waiting) << 8),
        );
        let next = (station.newgrf_random_bits & !base_mask) | (random & base_mask);
        changed |= next != station.newgrf_random_bits;
        station.newgrf_random_bits = next;
    }
    if tile_mask != 0 {
        let random = crate::map::industry_tile_rng(
            context.world_seed,
            context.tick,
            tile,
            0x524F_4144_u64 | (u64::from(trigger as u8) << 16) | u64::from(waiting),
        );
        let state = station.ensure_road_stop_tile_state(tile);
        let next = (state.random_bits & !tile_mask) | (random & tile_mask);
        changed |= next != state.random_bits;
        state.random_bits = next;
    }
    station.sync_legacy_road_stop_anchor();
    changed
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
    advance_road_stop_animation_at(def, station, station.pos, view, tick)
}

/// Variante por tesela de [`advance_road_stop_animation`].
pub fn advance_road_stop_animation_at(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    view: u8,
    tick: u64,
) -> bool {
    advance_road_stop_animation_at_with_world(def, station, tile, view, tick, None)
}

/// Variante de [`advance_road_stop_animation_at`] con el contexto completo
/// del mundo para CB141/CB142.
pub fn advance_road_stop_animation_at_with_world(
    def: &crate::road_stop_spec::RoadStopSpecDef,
    station: &mut Station,
    tile: TileCoord,
    view: u8,
    tick: u64,
    world: Option<RoadStopCallbackWorld<'_>>,
) -> bool {
    let active = station
        .road_stop_tile_state(tile)
        .map_or(station.road_stop_animation_active, |state| {
            state.animation_active
        });
    if !active {
        return false;
    }
    let before = (station.road_stop_animation_frame_at(tile), active);
    let mut speed = def.animation_speed.min(16);
    if def.has_animation_speed_callback() {
        let result = resolve_road_stop_animation_callback(
            def,
            station,
            tile,
            view,
            CBID_STATION_ANIMATION_SPEED,
            0,
            0,
            world,
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
            road_stop_animation_random_bits(station, tile, tick)
        } else {
            0
        };
        let result = resolve_road_stop_animation_callback(
            def,
            station,
            tile,
            view,
            CBID_STATION_ANIMATION_NEXT_FRAME,
            random_bits,
            0,
            world,
        );
        if result != CALLBACK_FAILED {
            let state = station.ensure_road_stop_tile_state(tile);
            match (result & 0xFF) as u8 {
                0xFF => state.animation_active = false,
                0xFE => {}
                frame => {
                    state.animation_frame = frame;
                    frame_set_by_callback = true;
                }
            }
        }
    }

    {
        let state = station.ensure_road_stop_tile_state(tile);
        if state.animation_active && !frame_set_by_callback {
            if state.animation_frame < def.animation_frames {
                state.animation_frame = state.animation_frame.saturating_add(1);
            } else if state.animation_frame == def.animation_frames && def.animation_loops() {
                state.animation_frame = 0;
            } else {
                state.animation_active = false;
            }
        }
    }
    station.sync_legacy_road_stop_anchor();
    before
        != (
            station.road_stop_animation_frame_at(tile),
            station
                .road_stop_tile_state(tile)
                .map_or(station.road_stop_animation_active, |state| {
                    state.animation_active
                }),
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
    let (set_id, entry) = runtime.action2_random.iter().find(|(_, entry)| {
        entry.trigger_mask() == 0
            || entry
                .matched_rerandomisation_triggers(waiting_triggers)
                .is_some()
    })?;
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
                        and_mask: u32::from(value),
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

    fn gfx_callback_variable_byte(variable: u8, shift: u8) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift,
                        and_mask: u32::from(u8::MAX),
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

    /// Devuelve `0x400` sólo si un byte de variable coincide con `expected`.
    /// Así los tests prueban que el call site empaqueta el parámetro correcto,
    /// no sólo que invocó alguna cadena Action2.
    fn gfx_callback_allow_if_byte(variable: u8, shift: u8, expected: u8) -> TrainSpriteGraphics {
        let literal = |value: u8| Action2VarTerm {
            variable: 0x1A,
            param: None,
            adjust: Action2VarAdjust {
                shift: 0,
                and_mask: u32::from(value),
                ..Action2VarAdjust::default()
            },
        };
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift,
                        and_mask: u32::from(u8::MAX),
                        ..Action2VarAdjust::default()
                    },
                },
                // Comparación exacta -> 1; 1 * 16 * 64 = 0x400 (allow).
                ops: vec![
                    Action2VarOp {
                        operator: 0x12,
                        rhs: literal(expected),
                    },
                    Action2VarOp {
                        operator: 0x0A,
                        rhs: literal(0x10),
                    },
                    Action2VarOp {
                        operator: 0x0A,
                        rhs: literal(0x40),
                    },
                ],
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
                        and_mask: u32::from(value),
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
                                and_mask: u32::from(reg),
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
    fn callbacks_ac_vehicle_32day_decodes_two_bits_and_reports_unknown() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x3332_4441;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x87)));
        let mut vehicle = Vehicle::new(
            32,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert_eq!(
            resolve_vehicle_32day_callback(&engine, &mut vehicle),
            Some(Vehicle32DayCallback {
                trigger_randomisation: true,
                invalidate_palette: true,
                unknown_bits: 0x84,
            })
        );

        engine.newgrf_runtime = None;
        assert_eq!(resolve_vehicle_32day_callback(&engine, &mut vehicle), None);
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x01)));
        engine.newgrf_grfid = 0;
        assert_eq!(resolve_vehicle_32day_callback(&engine, &mut vehicle), None);
    }

    #[test]
    fn callbacks_ac_vehicle_random_trigger_consumes_waiting_and_reseeds_mask() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x5241_4E44;
        engine.newgrf_local_id = 0;
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_random.insert(
            2,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: VehicleRandomTrigger::Callback32.mask(),
                randbit: 0,
                sets: vec![0x8000, 0x8001],
            },
        );
        engine.newgrf_runtime = Some(Box::new(gfx));
        let mut vehicle = Vehicle::new(
            44,
            VehicleKind::Train,
            TileCoord::new(2, 3),
            TileCoord::new(2, 3),
        );
        vehicle.newgrf_random_bits = 0;
        assert!(trigger_vehicle_randomisation(
            &engine,
            &mut vehicle,
            VehicleRandomTrigger::Callback32,
            9,
            17,
        ));
        assert_eq!(vehicle.newgrf_waiting_random_triggers, 0);
        // The one-bit random group is applied to the deterministic vehicle
        // seed; the exact value is stable even when the new bit equals zero.
        let expected = crate::map::industry_tile_rng(
            9,
            17,
            vehicle.pos,
            u64::from(vehicle.id) ^ (u64::from(VehicleRandomTrigger::Callback32 as u8) << 32),
        );
        assert_eq!(vehicle.newgrf_random_bits & 1, expected & 1);
    }

    #[test]
    fn callbacks_ac_vehicle_32day_runs_in_staggered_economy_slot() {
        let mut state = crate::GameState::new(8, 8);
        let engine_id = state
            .engine_catalog
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .map(|e| e.id)
            .unwrap();
        let engine_index = state
            .engine_catalog
            .iter()
            .position(|e| e.id == engine_id)
            .unwrap();
        state.engine_catalog[engine_index].newgrf_grfid = 0x3332_534C;
        state.engine_catalog[engine_index].newgrf_local_id = 0;
        state.engine_catalog[engine_index].newgrf_runtime = Some(Box::new(gfx_callback_literal(1)));
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        vehicle.engine_id = Some(engine_id);
        state.vehicles.push(vehicle);
        state.economy_timer.date_fract = 0;
        crate::vehicle::process_vehicle_economy_day(&mut state);
        assert_eq!(state.vehicles[0].newgrf_day_counter, 1);
        assert_eq!(
            state.vehicles[0].newgrf_waiting_random_triggers,
            VehicleRandomTrigger::Callback32.mask()
        );
    }

    #[test]
    fn callbacks_ac_vehicle_visual_effect_decodes_types_and_disable_bit() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x5649_5355;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 0;
        let mut vehicle = Vehicle::new(
            5,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );

        // Bits 4–5 = 2 → diésel.
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x20)));
        assert_eq!(
            resolve_vehicle_visual_effect_callback(&engine, &mut vehicle),
            Some(VehicleVisualEffectKind::Diesel)
        );
        assert_eq!(
            vehicle_visual_effect_kind(&engine, &mut vehicle),
            VehicleVisualEffectKind::Diesel
        );

        // El bit 6 desactiva humo/chispas, incluso si los bits de tipo están puestos.
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0x70)));
        assert_eq!(
            resolve_vehicle_visual_effect_callback(&engine, &mut vehicle),
            Some(VehicleVisualEffectKind::Disabled)
        );

        // Cero pide la clase por defecto y un resultado ancho cae al fallback.
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert_eq!(
            resolve_vehicle_visual_effect_callback(&engine, &mut vehicle),
            Some(VehicleVisualEffectKind::Default)
        );
        engine.newgrf_runtime = Some(Box::new(gfx_callback_allow_400()));
        assert_eq!(
            resolve_vehicle_visual_effect_callback(&engine, &mut vehicle),
            None
        );

        // Sin callback, la propiedad Action0 del motor sigue siendo la fuente
        // de verdad y `0xFF` conserva el fallback por clase.
        engine.vehicle_callback_mask = 0;
        engine.visual_effect = 0x20;
        assert_eq!(
            vehicle_visual_effect_kind(&engine, &mut vehicle),
            VehicleVisualEffectKind::Diesel
        );
        engine.visual_effect = 0xCF;
        assert_eq!(
            vehicle_visual_effect_kind(&engine, &mut vehicle),
            VehicleVisualEffectKind::Disabled
        );
        engine.visual_effect = crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT;
        assert_eq!(
            vehicle_visual_effect_kind(&engine, &mut vehicle),
            VehicleVisualEffectKind::Default
        );
    }

    #[test]
    fn callbacks_ac_vehicle_load_amount_uses_nonzero_byte_and_falls_back() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x4C_4F_41_44;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 2;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(7)));
        let mut vehicle = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert_eq!(
            resolve_vehicle_load_amount_callback(&engine, &mut vehicle),
            Some(7)
        );

        // Cero significa «usa la propiedad load_amount», no una carga nula.
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert_eq!(
            resolve_vehicle_load_amount_callback(&engine, &mut vehicle),
            None
        );
        // Sin la máscara o sin GRF el callback no se consulta.
        engine.vehicle_callback_mask = 0;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(9)));
        assert_eq!(
            resolve_vehicle_load_amount_callback(&engine, &mut vehicle),
            None
        );
    }

    #[test]
    fn callbacks_ac_vehicle_length_converts_shorten_amount_and_falls_back() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x4C45_4E47;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 1;
        engine.shorten_factor = 6;
        let mut vehicle = Vehicle::new(
            4,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(3)));
        assert_eq!(
            resolve_vehicle_length_callback(&engine, &mut vehicle),
            Some(5)
        );
        assert_eq!(vehicle_unit_length(&engine, &mut vehicle), 5);

        // CALLBACK_FAILED o runtime ausente usa la propiedad shorten_factor.
        engine.newgrf_runtime = None;
        assert_eq!(resolve_vehicle_length_callback(&engine, &mut vehicle), None);
        assert_eq!(vehicle_unit_length(&engine, &mut vehicle), 2);
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(8)));
        assert_eq!(resolve_vehicle_length_callback(&engine, &mut vehicle), None);
        assert_eq!(vehicle_unit_length(&engine, &mut vehicle), 2);
    }

    #[test]
    fn callbacks_ac_vehicle_articulated_part_decodes_grf_width_and_mirror() {
        assert_eq!(
            decode_vehicle_articulated_part(0x80 | 7, 7),
            Some(VehicleArticulatedPart {
                local_id: 7,
                mirrored: true,
            })
        );
        assert_eq!(decode_vehicle_articulated_part(0xFF, 7), None);
        assert_eq!(
            decode_vehicle_articulated_part(0x4000 | 0x04D2, 8),
            Some(VehicleArticulatedPart {
                local_id: 1234,
                mirrored: true,
            })
        );
        assert_eq!(decode_vehicle_articulated_part(0x7FFF, 8), None);
        assert_eq!(decode_vehicle_articulated_part(CALLBACK_FAILED, 8), None);
        // La versión cero es el fallback moderno para entradas sin Action8.
        assert_eq!(
            decode_vehicle_articulated_part(0x4001, 0),
            Some(VehicleArticulatedPart {
                local_id: 1,
                mirrored: true,
            })
        );
    }

    #[test]
    fn callbacks_ac_vehicle_articulated_part_requires_mask_and_writes_callback() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x4152_5449;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 4;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(2)));
        let mut vehicle = Vehicle::new(
            6,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert_eq!(
            resolve_vehicle_articulated_part_callback(&engine, &mut vehicle, 1, 8),
            Some(VehicleArticulatedPart {
                local_id: 2,
                mirrored: false,
            })
        );
        let mut extended_runtime = gfx_callback_literal(2);
        extended_runtime.assigns.clear();
        extended_runtime.extended_assigns.push((1234, 2));
        engine.newgrf_local_id = 1234;
        engine.newgrf_runtime = Some(Box::new(extended_runtime));
        assert_eq!(
            resolve_vehicle_articulated_part_callback(&engine, &mut vehicle, 1, 8),
            Some(VehicleArticulatedPart {
                local_id: 2,
                mirrored: false,
            })
        );
        engine.vehicle_callback_mask = 0;
        assert_eq!(
            resolve_vehicle_articulated_part_callback(&engine, &mut vehicle, 1, 8),
            None
        );
    }

    #[test]
    fn callbacks_ac_vehicle_refit_capacity_uses_target_cargo_and_restores_type() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x5245_4649;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 3;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(42)));
        let mut vehicle = Vehicle::new(
            3,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.cargo_type = Some(CargoType::Passengers);
        assert_eq!(
            resolve_vehicle_refit_capacity_callback(&engine, &mut vehicle, CargoType::Coal),
            Some(42)
        );
        assert_eq!(
            vehicle.cargo_type,
            Some(CargoType::Passengers),
            "el callback no debe cambiar el refit solicitado"
        );

        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert_eq!(
            resolve_vehicle_refit_capacity_callback(&engine, &mut vehicle, CargoType::Coal),
            Some(0),
            "cero es una capacidad válida devuelta por CB15"
        );
        engine.vehicle_callback_mask = 0;
        assert_eq!(
            resolve_vehicle_refit_capacity_callback(&engine, &mut vehicle, CargoType::Coal),
            None
        );
    }

    #[test]
    fn callbacks_ac_vehicle_sound_translates_base_and_grf_local_ids() {
        let mut state = crate::GameState::new(4, 4);
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.id = 4_000;
        engine.newgrf_grfid = 0x534F_554E;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 7;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(SoundId::CashTill.as_u8())));
        let mut vehicle = Vehicle::new(
            77,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.engine_id = Some(engine.id);
        state.engine_catalog.push(engine);
        state.vehicles.push(vehicle);

        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 77, VehicleSoundEvent::Start),
            VehicleSoundOverride::Base(SoundId::CashTill)
        );

        let engine_index = state
            .engine_catalog
            .iter()
            .position(|candidate| candidate.id == 4_000)
            .unwrap();
        state.engine_catalog[engine_index].newgrf_runtime =
            Some(Box::new(gfx_callback_literal(74)));
        state.sound_effect_catalog.push(crate::SoundEffectDef {
            local_id: 1,
            grfid: 0x534F_554E,
            volume: 128,
            priority: 7,
            override_old: None,
            has_sample: true,
            sample_pcm: vec![0x80, 0x90],
            from_newgrf: true,
        });
        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 77, VehicleSoundEvent::Running),
            VehicleSoundOverride::Newgrf {
                grfid: 0x534F_554E,
                local_id: 1,
            }
        );
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
    fn callback_result_uses_upstream_signed_15_bit_encoding() {
        assert_eq!(signed_15_bit_callback_result(0), 0);
        assert_eq!(signed_15_bit_callback_result(0x3FFF), 16_383);
        assert_eq!(signed_15_bit_callback_result(0x4000), -16_384);
        assert_eq!(signed_15_bit_callback_result(0x7FFF), -1);
    }

    #[test]
    fn cargo_station_rating_uses_legacy_vehicle_type_codes() {
        assert_eq!(cargo_station_rating_vehicle_param(None), 0);
        assert_eq!(
            cargo_station_rating_vehicle_param(Some(VehicleKind::Train)),
            0x10
        );
        assert_eq!(
            cargo_station_rating_vehicle_param(Some(VehicleKind::Bus)),
            0x11
        );
        assert_eq!(
            cargo_station_rating_vehicle_param(Some(VehicleKind::Tram)),
            0x11
        );
        assert_eq!(
            cargo_station_rating_vehicle_param(Some(VehicleKind::Ship)),
            0x12
        );
        assert_eq!(
            cargo_station_rating_vehicle_param(Some(VehicleKind::Aircraft)),
            0x13
        );
    }

    #[test]
    fn cargo_station_rating_packs_upstream_callback_parameters_and_falls_back() {
        let mut def = CargoSpecDef {
            callback_mask: crate::CARGO_CALLBACK_STATION_RATING_CALC_MASK,
            ..CargoSpecDef::default()
        };

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x0C, 0)));
        assert_eq!(
            resolve_cargo_station_rating_callback(
                &def,
                0xAB,
                0x1234,
                true,
                0x44,
                Some(VehicleKind::Bus),
            ),
            Some(0x45),
            "var0C debe recibir CB145"
        );

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x10, 0)));
        assert_eq!(
            resolve_cargo_station_rating_callback(
                &def,
                0xAB,
                0x1234,
                true,
                0x44,
                Some(VehicleKind::Bus),
            ),
            Some(0x11),
            "var10 debe recibir el tipo road histórico"
        );

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x18, 0)));
        assert_eq!(
            resolve_cargo_station_rating_callback(
                &def,
                0xAB,
                0x1234,
                true,
                0x44,
                Some(VehicleKind::Bus),
            ),
            Some(0xAB),
            "var18 byte bajo debe ser días sin recogida"
        );

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x18, 8)));
        assert_eq!(
            resolve_cargo_station_rating_callback(
                &def,
                0xAB,
                0x1234,
                true,
                0x44,
                Some(VehicleKind::Bus),
            ),
            Some(0x34),
            "var18 bits 8..15 deben contener espera"
        );

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x18, 24)));
        assert_eq!(
            resolve_cargo_station_rating_callback(&def, 0, 0, false, 0x44, None),
            Some(0xFF),
            "sin visita previa OpenTTD entrega velocidad 0xFF"
        );
        assert_eq!(
            resolve_cargo_station_rating_callback(&def, 0, 0, true, 0x44, None),
            Some(0x44),
            "con visita previa conserva la última velocidad"
        );

        def.newgrf_runtime = Some(Box::default());
        assert_eq!(
            resolve_cargo_station_rating_callback(&def, 0, 0, false, 0, None),
            None,
            "CALLBACK_FAILED debe dejar el rating estándar"
        );
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
    fn callbacks_ac_station_slope_packs_upstream_parameters() {
        let mut def = crate::station_class::vanilla_station_spec_catalog()
            .pop()
            .unwrap();
        def.callback_mask = crate::station_class::STATION_CALLBACK_SLOPE_CHECK_MASK;

        // slope=West, eje Y: param1 = (1 << 4) | (1 ^ SLOPE_EW) = 0x14.
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x10, 0, 0x14)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        assert!(!apply_station_slope_callback_for_build(
            &def, 1, false, 3, 5, 2, 4,
        ));

        // param2 = tracks<<24 | length<<16 | platform<<8 | position.
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x18, 0, 4)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        assert!(!apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 3,
        ));

        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x18, 8, 2)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        assert!(!apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 1, 4,
        ));

        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x18, 16, 5)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        assert!(!apply_station_slope_callback_for_build(
            &def, 1, true, 3, 4, 2, 4,
        ));

        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x18, 24, 3)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        assert!(!apply_station_slope_callback_for_build(
            &def, 1, true, 2, 5, 2, 4,
        ));

        // Sin runtime, callback fallido o máscara inactiva conserva el fallback.
        def.newgrf_runtime = None;
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        def.newgrf_runtime = Some(Box::new(TrainSpriteGraphics::default()));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        def.callback_mask = 0;
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
    }

    #[test]
    fn callbacks_ac_station_and_road_stop_persistent_json_roundtrip() {
        let mut state = crate::GameState::new(4, 4);
        let mut st = Station::new(TileCoord::new(1, 1));
        st.newgrf_persistent_regs.insert(2, 99);
        st.newgrf_random_bits = 0xCAFE;
        st.road_stop_newgrf_random_bits = 0x55;
        st.newgrf_waiting_random_triggers = StationRandomTrigger::NewCargo.mask();
        state.stations.push(st);
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        let station = &loaded.stations[0];
        assert_eq!(station.newgrf_persistent_regs.get(&2), Some(&99));
        assert_eq!(station.newgrf_random_bits, 0xCAFE);
        assert_eq!(station.road_stop_newgrf_random_bits, 0x55);
        assert_eq!(
            station.newgrf_waiting_random_triggers,
            StationRandomTrigger::NewCargo.mask()
        );
    }

    #[test]
    fn callbacks_ac_road_stop_animation_writes_back_station_storage() {
        let mut def = crate::RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "anim".into(),
            short_label: "ANIM".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
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
            StationAnimationTrigger::Built,
            None,
            1,
        ));
        assert!(station.road_stop_animation_active);
        assert_eq!(station.newgrf_persistent_regs.get(&4), Some(&12));

        // Action0 usa la máscara, pero CB140 recibe el ordinal del trigger.
        def.animation_triggers = crate::ROADSTOP_ANIMATION_TRIGGER_VEHICLE_DEPARTS
            | crate::ROADSTOP_ANIMATION_TRIGGER_NEW_CARGO;
        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x18, 0)));
        assert!(trigger_road_stop_animation(
            &def,
            &mut station,
            crate::RSV_BAY_NE,
            StationAnimationTrigger::VehicleDeparts,
            None,
            2,
        ));
        assert_eq!(station.road_stop_animation_frame, 4);

        def.newgrf_runtime = Some(Box::new(gfx_callback_variable_byte(0x18, 8)));
        assert!(trigger_road_stop_animation(
            &def,
            &mut station,
            crate::RSV_BAY_NE,
            StationAnimationTrigger::NewCargo,
            Some(5),
            3,
        ));
        assert_eq!(station.road_stop_animation_frame, 5);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn road_stop_animation_callback_receives_world_scopes() {
        let def = crate::RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "world".into(),
            short_label: "WORLD".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_grf_version: 8,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: 0,
            animation_status: 1,
            animation_frames: 8,
            animation_speed: 0,
            animation_triggers: crate::ROADSTOP_ANIMATION_TRIGGER_BUILT,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(gfx_callback_variable_byte(0x46, 0))),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        };
        let catalog = vec![def.clone()];
        let coord = TileCoord::new(1, 2);
        let mut map = crate::Map::new_flat(8, 8, 0);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = crate::TileKind::Station;
        tile.m5 = crate::RSV_BAY_NE;
        map.set_tile(coord, tile).expect("station tile");
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(def.id);
        let towns = vec![crate::Town {
            pos: TileCoord::new(0, 0),
            ..crate::Town::default()
        }];
        let companies = Vec::new();

        assert!(trigger_road_stop_animation_at_with_world(
            &def,
            &mut station,
            coord,
            crate::RSV_BAY_NE,
            StationAnimationTrigger::Built,
            None,
            1,
            Some(RoadStopCallbackWorld {
                map: &map,
                road_stop_catalog: &catalog,
                towns: &towns,
                companies: &companies,
                industries: &[],
                road_type_catalog: &[],
                climate: crate::Climate::Temperate,
            }),
        ));
        assert_eq!(
            station.road_stop_animation_frame_at(coord),
            5,
            "CB140 debe ver var 46 = distancia cuadrática 5"
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    #[allow(clippy::too_many_lines)]
    fn road_stop_randomisation_uses_world_scopes_before_reseeding() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x46,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(u8::MAX),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(5, 5, 5)],
                default: 0,
            },
        );
        gfx.action2_random.insert(
            5,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: crate::StationRandomTrigger::NewCargo.mask(),
                randbit: 16,
                sets: vec![1, 2],
            },
        );
        let def = crate::RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "random-world".into(),
            short_label: "RWORLD".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_grf_version: 8,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 1 << crate::CargoType::Passengers.bitnum(),
            flags: 0,
            callback_mask: 0,
            animation_status: 0,
            animation_frames: 0,
            animation_speed: 0,
            animation_triggers: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(gfx)),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        };
        let catalog = vec![def.clone()];
        let coord = TileCoord::new(1, 2);
        let mut map = crate::Map::new_flat(8, 8, 0);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = crate::TileKind::Station;
        tile.m5 = crate::RSV_BAY_NE;
        map.set_tile(coord, tile).expect("station tile");
        let towns = vec![crate::Town {
            pos: TileCoord::new(0, 0),
            ..crate::Town::default()
        }];
        let companies = Vec::new();
        let world = Some(RoadStopCallbackWorld {
            map: &map,
            road_stop_catalog: &catalog,
            towns: &towns,
            companies: &companies,
            industries: &[],
            road_type_catalog: &[],
            climate: crate::Climate::Temperate,
        });

        let mut legacy = Station::new_with_kind(coord, StopKind::BusStop);
        legacy.road_stop_spec = Some(def.id);
        assert!(!trigger_road_stop_randomisation_at(
            &def,
            &mut legacy,
            coord,
            StationRandomTrigger::NewCargo,
            Some(crate::CargoType::Passengers),
            RoadStopRandomisationContext {
                climate: crate::Climate::Temperate,
                world_seed: 9,
                tick: 2,
            },
        ));
        assert_eq!(
            legacy.newgrf_waiting_random_triggers,
            StationRandomTrigger::NewCargo.mask(),
            "sin world var 46 no se alcanza la rama random"
        );

        let mut world_station = Station::new_with_kind(coord, StopKind::BusStop);
        world_station.road_stop_spec = Some(def.id);
        let _ = trigger_road_stop_randomisation_at_with_world(
            &def,
            &mut world_station,
            coord,
            StationRandomTrigger::NewCargo,
            Some(crate::CargoType::Passengers),
            RoadStopRandomisationContext {
                climate: crate::Climate::Temperate,
                world_seed: 9,
                tick: 2,
            },
            world,
        );
        assert_eq!(world_station.newgrf_waiting_random_triggers, 0);
        let expected = crate::map::industry_tile_rng(
            9,
            2,
            coord,
            0x524F_4144_u64
                | (u64::from(StationRandomTrigger::NewCargo as u8) << 16)
                | u64::from(StationRandomTrigger::NewCargo.mask()),
        );
        assert_eq!(world_station.road_stop_newgrf_random_bits & 1, expected & 1);
    }

    #[test]
    fn callbacks_ac_road_stop_randomisation_filters_cargo_and_keeps_all_waiting() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 7,
        });
        gfx.action2_random.insert(
            7,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                // NewCargo + VehicleArrives, bit 7 = ambos deben llegar.
                triggers: 0x80
                    | crate::StationRandomTrigger::NewCargo.mask()
                    | crate::StationRandomTrigger::VehicleArrives.mask(),
                randbit: 16,
                sets: vec![1, 2],
            },
        );
        let def = crate::RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "random".into(),
            short_label: "RAND".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_grf_version: 8,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 1 << crate::CargoType::Passengers.bitnum(),
            flags: 0,
            callback_mask: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(gfx)),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        };
        let mut station = Station::new_with_kind(TileCoord::new(6, 4), StopKind::BusStop);
        station.road_stop_newgrf_random_bits = 0;

        // Un cargo que no está en 0x0D no abre ni consume triggers.
        assert!(!trigger_road_stop_randomisation(
            &def,
            &mut station,
            StationRandomTrigger::NewCargo,
            Some(crate::CargoType::Coal),
            crate::Climate::Temperate,
            91,
            12,
        ));
        assert_eq!(station.newgrf_waiting_random_triggers, 0);

        // NewCargo queda pendiente porque el grupo es `all`.
        assert!(!trigger_road_stop_randomisation(
            &def,
            &mut station,
            StationRandomTrigger::NewCargo,
            Some(crate::CargoType::Passengers),
            crate::Climate::Temperate,
            91,
            12,
        ));
        assert_eq!(
            station.newgrf_waiting_random_triggers,
            StationRandomTrigger::NewCargo.mask()
        );

        // La llegada consume ambos eventos y reseedea sólo el byte de tesela
        // (randbit 16); los 16 bits base de estación no se tocan.
        let base_before = station.newgrf_random_bits;
        let waiting =
            StationRandomTrigger::NewCargo.mask() | StationRandomTrigger::VehicleArrives.mask();
        let expected = crate::map::industry_tile_rng(
            91,
            12,
            station.pos,
            0x524F_4144_u64
                | (u64::from(StationRandomTrigger::VehicleArrives as u8) << 16)
                | u64::from(waiting),
        );
        let _ = trigger_road_stop_randomisation(
            &def,
            &mut station,
            StationRandomTrigger::VehicleArrives,
            None,
            crate::Climate::Temperate,
            91,
            12,
        );
        assert_eq!(station.newgrf_waiting_random_triggers, 0);
        assert_eq!(station.newgrf_random_bits, base_before);
        assert_eq!(station.road_stop_newgrf_random_bits & 1, expected & 1);
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
