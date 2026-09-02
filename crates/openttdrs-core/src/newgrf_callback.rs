//! API común de resolución de callbacks `NewGRF` (#228 / #266).
//!
//! - Fallo observable: [`CALLBACK_FAILED`] (nunca se acepta un resultado “silencioso”).
//! - Storage: tras eval, writeback de `7C`/`\2psto` a vehículo o estación;
//!   los registros temporales (`7D`/`\2sto`) viven solo en el ctx y se descartan.
//! - Call sites #266: industry location, house/object construction, station availability,
//!   industry-tile trigger → Action2 random.

use crate::cargo_spec::CargoSpecDef;
use crate::cargodist::parity::Randomizer;
use crate::engine::EngineDef;
use crate::house_spec::HouseSpecDef;
use crate::industry::{Industry, IndustryProductionAction};
use crate::industry_spec::{IndustrySpecDef, cargo_type_from_label};
use crate::industry_tile::IndustryTileSpecDef;
use crate::map::{Map, TileCoord, has_tile_water_ground};
use crate::newgrf_sprites::{
    Action2EvalCtx, Action2RandomEntry, CALLBACK_FAILED, CBID_CARGO_PROFIT_CALC,
    CBID_CARGO_STATION_RATING_CALC, CBID_HOUSE_ALLOW_CONSTRUCTION, CBID_INDUSTRY_LOCATION,
    CBID_INDUSTRY_MONTHLY_PROD_CHANGE, CBID_INDUSTRY_PROD_CHANGE_BUILD,
    CBID_INDUSTRY_PRODUCTION_CHANGE, CBID_OBJECT_LAND_SLOPE_CHECK,
    CBID_STATION_ANIMATION_NEXT_FRAME, CBID_STATION_ANIMATION_SPEED,
    CBID_STATION_ANIMATION_TRIGGER, CBID_STATION_AVAILABILITY, CBID_STATION_LAND_SLOPE_CHECK,
    CBID_VEHICLE_32DAY_CALLBACK, CBID_VEHICLE_ARTIC_ENGINE, CBID_VEHICLE_COLOUR_MAPPING,
    CBID_VEHICLE_LENGTH, CBID_VEHICLE_LOAD_AMOUNT, CBID_VEHICLE_MODIFY_PROPERTY,
    CBID_VEHICLE_REFIT_CAPACITY, CBID_VEHICLE_SOUND_EFFECT, CBID_VEHICLE_START_STOP_CHECK,
    CBID_VEHICLE_VISUAL_EFFECT, IndustryProductionGroup, TrainSpriteGraphics,
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
use std::collections::HashSet;

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
    ctx.vehicle_palette_generation = vehicle.newgrf_palette_generation;
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
/// el scope propio. La máscara de random bits conserva los 16 bits nativos de
/// `OpenTTD`; los grupos que declaran bits fuera de esa palabra siguen siendo
/// ignorados por el evaluador Action2, no truncados al persistir el vehículo.
pub fn trigger_vehicle_randomisation(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
) -> bool {
    trigger_vehicle_randomisation_with_base(engine, vehicle, trigger, world_seed, tick, 0, true).0
}

fn vehicle_random_word(
    vehicle: &Vehicle,
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
) -> u16 {
    let salt = u64::from(vehicle.id) ^ (u64::from(trigger as u8) << 32);
    let low = crate::map::industry_tile_rng(world_seed, tick, vehicle.pos, salt);
    let high = crate::map::industry_tile_rng(world_seed, tick, vehicle.pos, salt ^ 0xA5A5_5A5A);
    u16::from(low) | (u16::from(high) << 8)
}

/// Ejecuta la randomización de un vehículo y devuelve `(cambió, palabra_random)`
/// para propagar la misma palabra a los vehículos que exige `OpenTTD`.
fn trigger_vehicle_randomisation_with_base(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
    base_random: u16,
    first: bool,
) -> (bool, u16) {
    let random = if first {
        vehicle_random_word(vehicle, trigger, world_seed, tick)
    } else {
        base_random
    };
    let before_random = vehicle.newgrf_random_bits;
    let before_waiting = vehicle.newgrf_waiting_random_triggers;
    vehicle.newgrf_waiting_random_triggers |= trigger.mask();
    let waiting = vehicle.newgrf_waiting_random_triggers;
    let Some(runtime) = engine.newgrf_runtime.as_ref() else {
        return (
            before_random != vehicle.newgrf_random_bits
                || before_waiting != vehicle.newgrf_waiting_random_triggers,
            random,
        );
    };
    let mut ctx = action2_eval_ctx_from_vehicle(vehicle);
    ctx.vars.insert(
        0x5F,
        u32::from(vehicle.newgrf_random_bits) << 8 | u32::from(waiting),
    );
    let (reseed, used) =
        runtime.rerandomisation_for_local_id_u16(engine.newgrf_local_id, &mut ctx, waiting);
    writeback_vehicle_persistent_registers(vehicle, &ctx);
    vehicle.newgrf_waiting_random_triggers &= !used;

    let reseed_mask = u16::try_from(reseed & 0xFFFF).unwrap_or(0);
    if reseed_mask != 0 {
        vehicle.newgrf_random_bits = (before_random & !reseed_mask) | (random & reseed_mask);
    }
    (
        before_random != vehicle.newgrf_random_bits
            || before_waiting != vehicle.newgrf_waiting_random_triggers,
        random,
    )
}

/// Busca el motor efectivo de una unidad dentro del catálogo de la partida.
///
/// `Vehicle::effective_engine` sólo puede consultar el catálogo vanilla
/// estático. Los motores asignados por Action0 viven en `GameState` y deben
/// resolverse contra ese vector antes de ejecutar callbacks o física; si el
/// id no existe se conserva el fallback vanilla para saves antiguos.
#[must_use]
pub fn engine_for_vehicle_catalog<'a>(
    catalog: &'a [EngineDef],
    vehicle: &Vehicle,
) -> &'a EngineDef {
    vehicle
        .engine_id
        .and_then(|id| catalog.iter().find(|candidate| candidate.id == id))
        .or_else(|| vehicle.engine_id.and_then(crate::engine::engine_by_id))
        .unwrap_or_else(|| {
            crate::engine::engine_for_vehicle(
                vehicle.kind,
                crate::engine::default_engine_id(vehicle.kind),
            )
        })
}

fn engine_for_vehicle_catalog_owned(catalog: &[EngineDef], vehicle: &Vehicle) -> Option<EngineDef> {
    vehicle
        .engine_id
        .and_then(|id| {
            catalog
                .iter()
                .find(|candidate| candidate.id == id)
                .cloned()
                .or_else(|| crate::engine::engine_by_id(id).cloned())
        })
        .or_else(|| Some(engine_for_vehicle_catalog(catalog, vehicle).clone()))
}

fn vehicle_previous_id(vehicles: &[Vehicle], id: u32) -> Option<u32> {
    vehicles
        .iter()
        .find(|vehicle| vehicle.id == id)
        .and_then(|vehicle| vehicle.prev_unit)
}

fn vehicle_next_id(vehicles: &[Vehicle], id: u32) -> Option<u32> {
    vehicles
        .iter()
        .find(|vehicle| vehicle.id == id)
        .and_then(|vehicle| vehicle.next_unit)
}

fn vehicle_chain_head_id(vehicles: &[Vehicle], id: u32) -> Option<u32> {
    let mut current = id;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let Some(previous) = vehicle_previous_id(vehicles, current) else {
            return Some(current);
        };
        if vehicles.iter().all(|vehicle| vehicle.id != previous) {
            return Some(current);
        }
        current = previous;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn trigger_vehicle_randomisation_chain_step(
    vehicles: &mut [Vehicle],
    catalog: &[EngineDef],
    id: u32,
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
    base_random: u16,
    first: bool,
    seen: &mut HashSet<u32>,
) -> (bool, u16) {
    if !seen.insert(id) {
        return (false, base_random);
    }
    let Some(index) = vehicles.iter().position(|vehicle| vehicle.id == id) else {
        return (false, base_random);
    };
    let engine = engine_for_vehicle_catalog_owned(catalog, &vehicles[index]);
    let Some(engine) = engine else {
        return (false, base_random);
    };
    let (mut changed, random) = trigger_vehicle_randomisation_with_base(
        &engine,
        &mut vehicles[index],
        trigger,
        world_seed,
        tick,
        base_random,
        first,
    );
    let next = vehicle_next_id(vehicles, id);
    match trigger {
        VehicleRandomTrigger::NewCargo => {
            if let Some(head) = vehicle_chain_head_id(vehicles, id) {
                // `NewCargo` first applies to the unit that picked up the
                // cargo and then raises `AnyNewCargo` from the front of the
                // consist.  The nested walk needs its own cycle guard: when
                // the picked-up unit is already the front unit, the outer
                // `seen` set already contains that id but the nested event
                // must still be evaluated for it.
                let mut any_new_cargo_seen = HashSet::new();
                let (next_changed, _) = trigger_vehicle_randomisation_chain_step(
                    vehicles,
                    catalog,
                    head,
                    VehicleRandomTrigger::AnyNewCargo,
                    world_seed,
                    tick,
                    random,
                    false,
                    &mut any_new_cargo_seen,
                );
                changed |= next_changed;
            }
        }
        VehicleRandomTrigger::Depot => {
            if let Some(next) = next {
                let (next_changed, _) = trigger_vehicle_randomisation_chain_step(
                    vehicles, catalog, next, trigger, world_seed, tick, 0, true, seen,
                );
                changed |= next_changed;
            }
        }
        VehicleRandomTrigger::Empty | VehicleRandomTrigger::AnyNewCargo => {
            if let Some(next) = next {
                let (next_changed, _) = trigger_vehicle_randomisation_chain_step(
                    vehicles, catalog, next, trigger, world_seed, tick, random, false, seen,
                );
                changed |= next_changed;
            }
        }
        VehicleRandomTrigger::Callback32 => {}
    }
    (changed, random)
}

/// Replica `TriggerVehicleRandomisation` para una cadena completa de
/// vehículos. `NewCargo` dispara `AnyNewCargo` desde la cabeza; `Depot`
/// reseedea cada unidad con su propio aleatorio; `Empty` y `AnyNewCargo`
/// comparten la palabra aleatoria de la primera unidad. Los enlaces inválidos
/// se cortan de forma determinista y nunca pueden crear una recursión infinita.
pub fn trigger_vehicle_randomisation_chain(
    vehicles: &mut [Vehicle],
    vehicle_id: u32,
    engine_catalog: &[EngineDef],
    trigger: VehicleRandomTrigger,
    world_seed: u64,
    tick: u64,
) -> bool {
    let mut seen = HashSet::new();
    trigger_vehicle_randomisation_chain_step(
        vehicles,
        engine_catalog,
        vehicle_id,
        trigger,
        world_seed,
        tick,
        0,
        true,
        &mut seen,
    )
    .0
}

/// Resultado normalizado de `CBID_VEHICLE_COLOUR_MAPPING` (`0x2D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleColourMapping {
    /// `PaletteID` devuelto por el callback (bits 0..13).
    pub palette_id: u16,
    /// Bit 14: aplicar los colores de la compañía sobre la paleta.
    pub apply_company_colour: bool,
}

impl VehicleColourMapping {
    /// Convierte el resultado a una paleta de compañía de un solo canal. Para
    /// motores 2CC usar [`Self::palette_for_companies`], que incorpora el
    /// segundo color y el rango `SPR_2CCMAP_BASE`.
    #[must_use]
    pub const fn palette_for_company(self, company_colour: u8) -> u16 {
        if self.apply_company_colour {
            // `PALETTE_RECOLOUR_START` de OpenTTD (775) es la tabla que el
            // cliente ya puede hornear sin depender de un atlas adicional.
            775 + (company_colour & 0x0F) as u16
        } else {
            self.palette_id
        }
    }

    /// Convierte el resultado del callback en la paleta efectiva de uno o dos
    /// colores de compañía.
    #[must_use]
    pub const fn palette_for_companies(self, primary: u8, secondary: u8, uses_2cc: bool) -> u16 {
        if !self.apply_company_colour {
            return self.palette_id;
        }
        if uses_2cc {
            crate::newgrf_sprites::TWOCC_PALETTE_BASE
                + (primary & 0x0F) as u16
                + ((secondary & 0x0F) as u16) * 16
        } else {
            775 + (primary & 0x0F) as u16
        }
    }
}

/// Ejecuta `CBID_VEHICLE_COLOUR_MAPPING` (`0x2D`) sin mutar el vehículo real.
///
/// El renderer consulta este callback por frame y no debe escribir registros
/// persistentes durante una fase visual; por eso se evalúa sobre una copia del
/// vehículo. `CALLBACK_FAILED`, GRF vanilla, máscara ausente o un resultado
/// fuera de los 15 bits dejan la paleta vanilla al caller.
#[must_use]
pub fn resolve_vehicle_colour_mapping_callback(
    engine: &EngineDef,
    vehicle: &Vehicle,
) -> Option<VehicleColourMapping> {
    if engine.newgrf_grfid == 0
        || engine.vehicle_callback_mask & (1 << 6) == 0
        || engine.newgrf_runtime.is_none()
    {
        return None;
    }
    let mut snapshot = vehicle.clone();
    let result = resolve_vehicle_callback(engine, &mut snapshot, CBID_VEHICLE_COLOUR_MAPPING, 0, 0);
    if result == CALLBACK_FAILED {
        return None;
    }
    Some(VehicleColourMapping {
        palette_id: result & 0x3FFF,
        apply_company_colour: result & 0x4000 != 0,
    })
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

/// Ejecuta `CBID_VEHICLE_MODIFY_PROPERTY` (`0x36`) para una propiedad Action0.
///
/// `param1` es el identificador de propiedad tal como aparece en
/// `newgrf_properties.h`. El resultado siempre es de 15 bits; cuando
/// `is_signed` se solicita, se aplica la extensión de signo de esos 15 bits
/// que usa `GetEngineProperty`. `None` significa `CALLBACK_FAILED`, ausencia
/// de runtime/GRF o una máscara de propiedad no aplicable al motor.
#[must_use]
pub fn resolve_vehicle_modify_property_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    property: u8,
    is_signed: bool,
) -> Option<i32> {
    if engine.newgrf_grfid == 0 || engine.newgrf_runtime.is_none() {
        return None;
    }
    let result = resolve_vehicle_callback(
        engine,
        vehicle,
        CBID_VEHICLE_MODIFY_PROPERTY,
        u32::from(property),
        0,
    );
    if result == CALLBACK_FAILED {
        return None;
    }
    let value = i32::from(result & 0x7FFF);
    Some(if is_signed && value & 0x4000 != 0 {
        value - 0x8000
    } else {
        value
    })
}

/// Devuelve la velocidad máxima efectiva de una unidad, consultando CB36
/// cuando el motor procede de un `NewGRF`.
///
/// Los valores de `max_speed` se expresan en las unidades nativas de la
/// propiedad Action0 de cada clase (`0x09` tren, `0x15` carretera, `0x0B`
/// barco y `0x0C` aeronave). `CALLBACK_FAILED` o un resultado que no quepa en
/// `u16` conservan la propiedad base del catálogo. El callback se ejecuta con
/// el vehículo mutable para que los registros persistentes (`7C`) sigan la
/// misma semántica que el resto de consultas runtime.
#[must_use]
pub fn vehicle_max_speed(engine: &EngineDef, vehicle: &mut Vehicle) -> u16 {
    let property = match engine.kind {
        VehicleKind::Train => 0x09,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 0x15,
        VehicleKind::Ship => 0x0B,
        VehicleKind::Aircraft => 0x0C,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u16::try_from(value).ok())
        .unwrap_or(engine.max_speed)
}

/// Atajo para consultar la velocidad efectiva desde el estado de una unidad.
#[must_use]
pub fn effective_vehicle_max_speed(vehicle: &mut Vehicle) -> u16 {
    let engine = vehicle.effective_engine();
    vehicle_max_speed(engine, vehicle)
}

/// Igual que [`effective_vehicle_max_speed`], resolviendo el motor contra el
/// catálogo de la partida para que los ratings y APIs que reciben `GameState`
/// no vuelvan accidentalmente al catálogo vanilla.
#[must_use]
pub fn effective_vehicle_max_speed_with_catalog(
    catalog: &[EngineDef],
    vehicle: &mut Vehicle,
) -> u16 {
    let engine = engine_for_vehicle_catalog(catalog, vehicle);
    vehicle_max_speed(engine, vehicle)
}

/// Resuelve la propiedad de capacidad de CB36 para la clase de una unidad.
///
/// El valor devuelto mantiene la unidad nativa de Action0 (capacidad de
/// pasajeros/carga; para aeronaves se usa `0x11` cuando el tipo actual es
/// correo). `None` conserva el valor ya calculado por el caller y distingue
/// un callback ausente/fallido de un resultado válido igual a cero.
#[must_use]
pub fn resolve_vehicle_capacity_property_callback(
    engine: &EngineDef,
    vehicle: &mut Vehicle,
) -> Option<u32> {
    let property = match engine.kind {
        VehicleKind::Train => 0x14,
        VehicleKind::Ship => 0x0D,
        VehicleKind::Aircraft if vehicle.cargo_type == Some(CargoType::Mail) => 0x11,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Aircraft => 0x0F,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u32::try_from(value).ok())
}

/// Resuelve el factor de compra o explotación de una unidad mediante CB36.
///
/// Los factores de coste son BYTE en las cuatro clases. Un resultado fuera de
/// ese rango no es una propiedad válida y se deja al caller para conservar el
/// factor Action0 ya calculado en el catálogo.
#[must_use]
pub fn vehicle_cost_factor(engine: &EngineDef, vehicle: &mut Vehicle, running: bool) -> Option<u8> {
    let property = match (engine.kind, running) {
        (VehicleKind::Train, false) => 0x17,
        (VehicleKind::Train, true) => 0x0D,
        (VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram, false) => 0x11,
        (VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram, true) => 0x09,
        (VehicleKind::Ship, false) => 0x0A,
        (VehicleKind::Ship, true) => 0x0F,
        (VehicleKind::Aircraft, false) => 0x0B,
        (VehicleKind::Aircraft, true) => 0x0E,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u8::try_from(value).ok())
}

/// Potencia efectiva de una unidad después de `CBID_VEHICLE_MODIFY_PROPERTY`.
///
/// La propiedad ferroviaria `0x0B` ya está expresada en HP; la vial `0x13`
/// está expresada en decenas de HP. Un resultado cero es válido y por eso no
/// se usa `unwrap_or` sobre la conversión antes de distinguir el fallo del
/// callback. Las piezas articuladas viales no aportan potencia en `OpenTTD`.
#[must_use]
pub fn vehicle_power_hp(engine: &EngineDef, vehicle: &mut Vehicle) -> u32 {
    if matches!(
        engine.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) && vehicle.is_articulated_unit()
    {
        return 0;
    }
    let property = match engine.kind {
        VehicleKind::Train => 0x0B,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 0x13,
        VehicleKind::Ship | VehicleKind::Aircraft => return engine.power_hp,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u32::try_from(value).ok())
        .map_or(engine.power_hp, |value| {
            if matches!(
                engine.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            ) {
                value.saturating_mul(10)
            } else {
                value
            }
        })
}

/// Peso de motor efectivo en toneladas después de CB36.
///
/// `0x16` ferroviario usa toneladas enteras; `0x14` vial usa cuartos de
/// tonelada y `OpenTTD` aplica división entera al convertirlo para la física.
/// Las piezas articuladas viales sólo pesan la carga que se les asigne, no
/// vuelven a sumar el peso de su motor.
#[must_use]
pub fn vehicle_weight_t(engine: &EngineDef, vehicle: &mut Vehicle) -> u16 {
    if matches!(
        engine.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) && vehicle.is_articulated_unit()
    {
        return 0;
    }
    let property = match engine.kind {
        VehicleKind::Train => 0x16,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 0x14,
        VehicleKind::Ship | VehicleKind::Aircraft => return engine.weight_t,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u16::try_from(value).ok())
        .map_or(engine.weight_t, |value| {
            if matches!(
                engine.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            ) {
                value / 4
            } else {
                value
            }
        })
}

/// Coeficiente de esfuerzo tractor efectivo (`1/256`) después de CB36.
///
/// El valor es un BYTE en las dos clases terrestres. Resultados fuera de ese
/// rango son inválidos y conservan la tabla/property vanilla; cero, en
/// cambio, desactiva deliberadamente el esfuerzo tractor de la unidad.
#[must_use]
pub fn vehicle_tractive_effort(engine: &EngineDef, vehicle: &mut Vehicle) -> u8 {
    if matches!(
        engine.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) && vehicle.is_articulated_unit()
    {
        return 0;
    }
    let (property, fallback) = match engine.kind {
        VehicleKind::Train => (0x1F, crate::engine::engine_tractive_effort(engine)),
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            (0x18, crate::engine::road_engine_tractive_effort(engine))
        }
        VehicleKind::Ship | VehicleKind::Aircraft => return 0,
    };
    resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(fallback)
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
    let callback_shorten = (!matches!(engine.kind, VehicleKind::Ship | VehicleKind::Aircraft))
        .then(|| {
            let property = match engine.kind {
                VehicleKind::Train => 0x21,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 0x23,
                VehicleKind::Ship | VehicleKind::Aircraft => return None,
            };
            resolve_vehicle_modify_property_callback(engine, vehicle, property, false)
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| *value < 8)
        })
        .flatten();
    resolve_vehicle_length_callback(engine, vehicle)
        .or(callback_shorten.map(|shorten| 8_u8.saturating_sub(shorten)))
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
    if engine.newgrf_grfid == 0 {
        return VehicleSoundOverride::Default;
    }
    if engine.vehicle_callback_mask & (1 << 7) == 0 {
        return vehicle_action0_sound_override(&state.sound_effect_catalog, &engine, event);
    }
    let result = resolve_vehicle_callback(
        &engine,
        &mut state.vehicles[vehicle_index],
        CBID_VEHICLE_SOUND_EFFECT,
        event as u32,
        0,
    );
    if result == CALLBACK_FAILED {
        return vehicle_action0_sound_override(&state.sound_effect_catalog, &engine, event);
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

/// Resuelve el sonido de salida declarado por Action0 cuando CB33 no está
/// disponible o devuelve `CALLBACK_FAILED`.
///
/// El campo `sound_effect` se guarda en el catálogo con el valor bruto de la
/// propiedad. `0` y `0xFF` son los sentinelas de `OpenTTD` para conservar el
/// sonido por defecto; los valores menores a `SOUND_COUNT` son muestras del
/// baseset y los restantes son IDs locales del GRF (`SOUND_COUNT + id`).
/// Sólo los eventos de salida (`VSE_START`) consultan esta propiedad: los
/// sonidos de marcha, avería y aterrizaje tienen sus propias selecciones
/// vanilla y no deben heredar el SFX de salida del motor.
#[must_use]
fn vehicle_action0_sound_override(
    catalog: &[crate::sound_effect::SoundEffectDef],
    engine: &EngineDef,
    event: VehicleSoundEvent,
) -> VehicleSoundOverride {
    if event != VehicleSoundEvent::Start
        || engine.sound_effect == 0
        || engine.sound_effect == u8::MAX
    {
        return VehicleSoundOverride::Default;
    }
    let sound_count = u8::try_from(crate::sound_id::SOUND_COUNT).unwrap_or(u8::MAX);
    if engine.sound_effect < sound_count {
        return SoundId::from_u8(engine.sound_effect)
            .map_or(VehicleSoundOverride::Suppressed, VehicleSoundOverride::Base);
    }
    let local_id = engine.sound_effect.saturating_sub(sound_count);
    let Some(def) = crate::sound_effect_def(catalog, engine.newgrf_grfid, local_id) else {
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

/// Aplica la compatibilidad de resultados de ubicación de `OpenTTD`.
///
/// En GRF anteriores a la versión 8 el bit 10 de los callbacks de ubicación
/// está invertido: `0` significa «sin error» y `0x400` es un error estándar.
/// Una versión `0` identifica specs vanilla/fixtures sin versión Action8 y no
/// debe activar la inversión; los GRF publicados usan las versiones 7/8.
#[must_use]
pub const fn callback_allows_location_for_grf(result: u16, grfid: u32, grf_version: u8) -> bool {
    if result == CALLBACK_FAILED {
        return true;
    }
    let normalized = if grfid != 0 && grf_version != 0 && grf_version < 8 {
        result ^ (1 << 10)
    } else {
        result
    };
    callback_allows_location(normalized)
}

/// Resultado de un callback booleano de ocho bits (CB13 de station/RoadStop,
/// CB17 de house). `CALLBACK_FAILED` permite el fallback y cualquier byte bajo
/// no nulo permite la operación, como `Convert8bitBooleanCallback` upstream.
#[must_use]
pub const fn callback_allows_8bit_boolean(result: u16) -> bool {
    result == CALLBACK_FAILED || (result & 0xFF) != 0
}

/// Convierte los callbacks `ConvertBooleanCallback` de `OpenTTD`.
///
/// Los callbacks de dibujo de fundaciones (casas, industrias y aeropuertos)
/// usan la conversión booleana de 15 bits, no la variante de ocho bits:
/// `CALLBACK_FAILED` conserva la fundación por defecto y cualquier resultado
/// no nulo la solicita. Mantener esta decisión en un helper evita que cada
/// renderer trate `0x100`/`0x400` de forma distinta.
#[must_use]
pub const fn callback_draws_default_foundation(result: u16) -> bool {
    if result == CALLBACK_FAILED {
        true
    } else {
        result != 0
    }
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

/// Call site de industria al fundar desde el mapa/juego.
///
/// El callback de `OpenTTD` recibe `IACT_USERCREATION` en `param2` y un scope
/// temporal con la tesela, el layout y el pueblo más cercano. La API histórica
/// [`apply_industry_location_callback`] se conserva para consumidores que no
/// tienen mundo disponible; los comandos de construcción deben usar esta
/// variante para que un Action2 pueda consultar esas variables.
#[must_use]
pub fn apply_industry_location_callback_for_build(
    def: &IndustrySpecDef,
    state: &GameState,
    pos: TileCoord,
    layout_index: u8,
    random_bits: u32,
) -> bool {
    if !def.has_location_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };

    // Reutilizar el contexto de tesela mantiene la codificación de terreno y
    // los fallbacks de mapas importados en un único sitio.
    let mut ctx = crate::map::action2_eval_ctx_for_industry_tile_with_world(
        &state.map,
        pos,
        &state.industries,
        &state.towns,
        &state.industry_tile_spec_catalog,
        &state.industry_spec_catalog,
        state.climate,
        None,
        &[],
    );
    ctx.random_bits = random_bits;
    let map_index = crate::map::coord_to_linear_index(pos, state.map.dimensions().0).unwrap_or(0);
    ctx.vars.insert(0x80, map_index);
    ctx.vars.insert(0x81, map_index >> 8);
    ctx.vars.insert(0x86, u32::from(layout_index));
    ctx.vars
        .insert(0x87, ctx.vars.get(&0x41).copied().unwrap_or(0));
    ctx.vars.insert(
        0x8A,
        state.map.get(pos).map_or(0, |tile| u32::from(tile.height)),
    );
    // `GetClosestWaterDistance(tile, true)` de OpenTTD. Las industrias
    // representadas actualmente son terrestres; cuando el catálogo conserve
    // `BuiltOnWater` este argumento deberá derivarse de esa propiedad.
    ctx.vars
        .insert(0x8B, closest_water_distance_for_location(&state.map, pos));
    ctx.vars.insert(0x8F, random_bits);
    for (parameter, &badge) in def.newgrf_badge_translation.iter().enumerate() {
        let value = if badge == u16::MAX {
            u32::MAX
        } else {
            u32::from(def.associated_badges.contains(&badge))
        };
        if let Ok(parameter) = u8::try_from(parameter) {
            ctx.parameterized_vars.insert((0x7A, parameter), value);
        }
    }
    if let Some((town_idx, distance)) = crate::town::nearest_town_index(&state.towns, pos) {
        let town = &state.towns[town_idx];
        ctx.vars.insert(0x82, town.id);
        ctx.vars.insert(
            0x88,
            u32::from(crate::house_spec::get_town_radius_group(town, pos) as u8),
        );
        ctx.vars.insert(0x89, distance.min(u32::from(u8::MAX)));
        ctx.vars.insert(
            0x8D,
            crate::house_spec::distance_square(town.pos, pos).min(u32::from(u16::MAX)),
        );
    } else {
        ctx.vars.insert(0x82, 0);
        ctx.vars.insert(0x88, 0);
        ctx.vars.insert(0x89, 0);
        ctx.vars.insert(0x8D, 0);
    }

    // `IACT_USERCREATION` (newgrf_industries.h) is the callback parameter
    // used by the user-facing Fund/Build command.
    let result =
        runtime.resolve_callback_ctx(def.newgrf_local_id, CBID_INDUSTRY_LOCATION, 0, 2, &mut ctx);
    callback_allows_location(result)
}

/// Réplica acotada de `GetClosestWaterDistance(tile, true)` para el scope de
/// construcción. El resultado está limitado a `0x7F`, como en `OpenTTD`.
fn closest_water_distance_for_location(map: &Map, center: TileCoord) -> u32 {
    let is_water = |coord: TileCoord| {
        map.get(coord).is_some_and(has_tile_water_ground)
    };
    if is_water(center) {
        return 0;
    }

    let (width, height) = map.dimensions();
    let max_x = i32::try_from(width).unwrap_or(i32::MAX);
    let max_y = i32::try_from(height).unwrap_or(i32::MAX);
    for distance in 1..0x7F_u32 {
        let d = i32::try_from(distance).unwrap_or(i32::MAX);
        let mut x = center.x;
        let mut y = center.y.saturating_sub(d);
        for (dx, dy) in [(-1, 1), (1, 1), (1, -1), (-1, -1)] {
            for _ in 0..distance {
                if x >= 0 && y >= 0 && x < max_x && y < max_y && is_water(TileCoord::new(x, y)) {
                    return distance;
                }
                x = x.saturating_add(dx);
                y = y.saturating_add(dy);
            }
        }
    }
    0x7F
}

/// Construye el contexto mínimo del scope `Industry` para callbacks de
/// producción. Las variables de vecinos/industria cercana que requieren
/// consultar el mapa se dejan fuera; las variables propias (stocks, nivel,
/// contador y posición) sí quedan disponibles para Action2.
fn action2_eval_ctx_from_industry(industry: &Industry, random: u32) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx {
        random_bits: random,
        ..Action2EvalCtx::default()
    };
    let accepted = industry.station_input_requirements();
    ctx.vars.insert(
        0x40,
        accepted
            .first()
            .map_or(0, |(cargo, _)| industry.accepted_cargo_waiting(*cargo)),
    );
    ctx.vars.insert(
        0x41,
        accepted
            .get(1)
            .map_or(0, |(cargo, _)| industry.accepted_cargo_waiting(*cargo)),
    );
    ctx.vars.insert(
        0x42,
        accepted
            .get(2)
            .map_or(0, |(cargo, _)| industry.accepted_cargo_waiting(*cargo)),
    );
    ctx.vars
        .insert(0x80, u32::from_ne_bytes(industry.pos.x.to_ne_bytes()));
    ctx.vars
        .insert(0x81, u32::from_ne_bytes(industry.pos.y.to_ne_bytes()));
    ctx.vars.insert(0x93, u32::from(industry.prod_level));
    ctx.vars.insert(0xAA, u32::from(industry.counter));
    ctx
}

/// Resultado observable de un callback de producción iterativo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IndustryProductionCallbackResult {
    /// Cantidad de grupos que se aplicaron antes de terminar (`again == 0`).
    pub iterations: u16,
    /// Unidades retiradas de las colas de cargos aceptados.
    pub inputs_consumed: u32,
    /// Unidades añadidas a las colas de cargos producidos.
    pub outputs_added: u32,
}

fn cargo_for_group_index(raw: u8, indices: &[u8], labels: &[String]) -> Option<CargoType> {
    indices
        .iter()
        .position(|&index| index == raw)
        .and_then(|idx| labels.get(idx))
        .and_then(|label| cargo_type_from_label(Some(label.as_str())))
        .or_else(|| CargoType::from_cargo_id(raw))
}

fn indirect_production_value(raw: i32, indirect: bool, ctx: &Action2EvalCtx) -> i32 {
    if !indirect {
        return raw;
    }
    let index = u8::try_from(raw).unwrap_or(0);
    i32::try_from(ctx.temp_registers.get(&index).copied().unwrap_or(0)).unwrap_or(i32::MAX)
}

fn apply_industry_group_input(industry: &mut Industry, cargo: CargoType, amount: i32) -> u32 {
    if amount >= 0 {
        let requested = u32::try_from(amount).unwrap_or(u32::MAX);
        industry.take_accepted_cargo_waiting(cargo, requested)
    } else {
        let added = amount.unsigned_abs();
        industry.add_accepted_cargo_waiting(cargo, added);
        0
    }
}

/// Ejecuta `IndustryProductionSpriteGroup` para CB1 (cargo recibido) o CB2
/// (ciclo de 256 ticks).
///
/// `None`/cero significa que el callback está declarado pero no resolvió un
/// grupo; en ese caso `OpenTTD` conserva las colas pendientes y no aplica el
/// algoritmo vanilla silenciosamente. Las dos salidas históricas de
/// [`Industry`] siguen usando `stock`/`secondary_stock`; cargos adicionales
/// se guardan en `newgrf_extra_produced_cargo`.
pub fn apply_industry_production_callback(
    def: &IndustrySpecDef,
    industry: &mut Industry,
    reason: u8,
    rng: &mut Randomizer,
) -> IndustryProductionCallbackResult {
    let declared = match reason {
        0 => def.has_production_cargo_arrival_callback(),
        1 => def.has_production_256_ticks_callback(),
        _ => false,
    };
    if !declared {
        return IndustryProductionCallbackResult::default();
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return IndustryProductionCallbackResult::default();
    };

    let random = rng.next();
    let mut ctx = action2_eval_ctx_from_industry(industry, random);
    let mut result = IndustryProductionCallbackResult::default();
    let accepted_slots: Vec<CargoType> = industry
        .station_input_requirements()
        .into_iter()
        .map(|(cargo, _)| cargo)
        .collect();
    let produced_slots = industry.produced_cargos();

    for loop_index in 0..=u16::MAX {
        ctx.vars
            .insert(0x18, u32::from(reason) | (u32::from(loop_index) << 8));
        let Some(group) =
            runtime.industry_production_group_u16(u16::from(def.newgrf_local_id), &mut ctx)
        else {
            break;
        };
        let group: IndustryProductionGroup = group.clone();
        let indirect = group.version >= 1;
        if group.version < 2 {
            for (idx, &raw) in group.subtract_input.iter().enumerate() {
                let Some(&cargo) = accepted_slots.get(idx) else {
                    continue;
                };
                result.inputs_consumed =
                    result
                        .inputs_consumed
                        .saturating_add(apply_industry_group_input(
                            industry,
                            cargo,
                            indirect_production_value(i32::from(raw), indirect, &ctx),
                        ));
            }
            for (idx, &raw) in group.add_output.iter().enumerate() {
                let Some(&cargo) = produced_slots.get(idx) else {
                    continue;
                };
                let amount = indirect_production_value(i32::from(raw), indirect, &ctx).max(0);
                let amount = u32::try_from(amount).unwrap_or(u32::MAX);
                industry.add_newgrf_produced_cargo(cargo, amount);
                result.outputs_added = result.outputs_added.saturating_add(amount);
            }
        } else {
            for (idx, &raw_cargo) in group.cargo_input.iter().enumerate() {
                let Some(cargo) = cargo_for_group_index(
                    raw_cargo,
                    &def.accepted_cargo_indices,
                    &def.accepted_cargo_labels,
                ) else {
                    continue;
                };
                let amount = group.subtract_input.get(idx).copied().map_or(0, |value| {
                    indirect_production_value(i32::from(value), indirect, &ctx)
                });
                result.inputs_consumed = result
                    .inputs_consumed
                    .saturating_add(apply_industry_group_input(industry, cargo, amount));
            }
            for (idx, &raw_cargo) in group.cargo_output.iter().enumerate() {
                let Some(cargo) = cargo_for_group_index(
                    raw_cargo,
                    &def.produced_cargo_indices,
                    &def.produced_cargo_labels,
                ) else {
                    continue;
                };
                let amount = group.add_output.get(idx).copied().map_or(0, |value| {
                    indirect_production_value(i32::from(value), indirect, &ctx).max(0)
                });
                let amount = u32::try_from(amount).unwrap_or(u32::MAX);
                industry.add_newgrf_produced_cargo(cargo, amount);
                result.outputs_added = result.outputs_added.saturating_add(amount);
            }
        }
        result.iterations = result.iterations.saturating_add(1);
        let again = indirect_production_value(i32::from(group.again), indirect, &ctx);
        if again == 0 {
            break;
        }
    }
    result
}

/// Decodificación del resultado de `CBID_INDUSTRY_PRODUCTION_CHANGE` y
/// `CBID_INDUSTRY_MONTHLYPROD_CHANGE` (`OpenTTD` `ChangeIndustryProduction`).
fn decode_industry_production_action(
    result: u16,
    ctx: &Action2EvalCtx,
) -> IndustryProductionAction {
    if result == CALLBACK_FAILED {
        return IndustryProductionAction::NoChange;
    }
    match result & 0x0F {
        0x01 => IndustryProductionAction::Halve,
        0x02 => IndustryProductionAction::Double,
        0x03 => IndustryProductionAction::Close,
        0x04 => IndustryProductionAction::Standard,
        0x05..=0x08 => IndustryProductionAction::Divide(1 << ((result & 0x0F) - 3)),
        0x09..=0x0C => IndustryProductionAction::Multiply(1 << ((result & 0x0F) - 7)),
        0x0D => IndustryProductionAction::Decrease,
        0x0E => IndustryProductionAction::Increase,
        0x0F => {
            // CB 0xF reads byte 2 of register 0x100 (`regs100[0]`).
            let level = ctx
                .registers_100
                .get(&0x100)
                .copied()
                .map_or(0, |value| ((value >> 16) & 0xFF) as u8);
            IndustryProductionAction::Set(level)
        }
        _ => IndustryProductionAction::NoChange,
    }
}

/// Ejecuta el callback de cambio de producción declarado por una industria.
///
/// `None` significa que el callback no está declarado; `Some(NoChange)` es
/// distinto: el callback está declarado pero devolvió `CALLBACK_FAILED`/0 y,
/// como en `OpenTTD`, no debe caer silenciosamente al algoritmo vanilla.
pub fn resolve_industry_production_change_callback(
    def: &IndustrySpecDef,
    industry: &Industry,
    monthly: bool,
    rng: &mut Randomizer,
) -> Option<IndustryProductionAction> {
    let declared = if monthly {
        def.has_monthly_production_change_callback()
    } else {
        def.has_production_change_callback()
    };
    if !declared {
        return None;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return Some(IndustryProductionAction::NoChange);
    };
    let random = rng.next();
    let callback = if monthly {
        CBID_INDUSTRY_MONTHLY_PROD_CHANGE
    } else {
        CBID_INDUSTRY_PRODUCTION_CHANGE
    };
    let mut ctx = action2_eval_ctx_from_industry(industry, random);
    let result = runtime.resolve_callback_ctx_u16(
        u16::from(def.newgrf_local_id),
        callback,
        0,
        random,
        &mut ctx,
    );
    Some(decode_industry_production_action(result, &ctx))
}

/// Ejecuta `CBID_INDUSTRY_PROD_CHANGE_BUILD` (`0x15F`) al fundar una industria.
/// Un resultado fuera de `PRODLEVEL_MINIMUM..=PRODLEVEL_MAXIMUM` conserva el
/// nivel inicial vanilla, igual que el chequeo de `OpenTTD`.
pub fn resolve_industry_production_change_build_callback(
    def: &IndustrySpecDef,
    industry: &Industry,
    rng: &mut Randomizer,
) -> Option<u8> {
    if !def.has_production_change_build_callback() {
        return None;
    }
    let runtime = def.newgrf_runtime.as_ref()?;
    let random = rng.next();
    let mut ctx = action2_eval_ctx_from_industry(industry, random);
    let result = runtime.resolve_callback_ctx_u16(
        u16::from(def.newgrf_local_id),
        CBID_INDUSTRY_PROD_CHANGE_BUILD,
        0,
        random,
        &mut ctx,
    );
    (result != CALLBACK_FAILED
        && (u16::from(crate::industry::PRODLEVEL_MINIMUM)
            ..=u16::from(crate::industry::PRODLEVEL_MAXIMUM))
            .contains(&result))
    .then(|| u8::try_from(result).unwrap_or(crate::industry::PRODLEVEL_DEFAULT))
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
/// construcción no existe una instancia de objeto persistente; el resolver
/// aporta los parámetros genéricos y aplica la inversión de bit 10 de GRF
/// anteriores a la versión 8. Los scopes completos de objeto/vecinos de
/// `OpenTTD` siguen fuera de este corte.
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
    callback_allows_location_for_grf(result, def.grfid, def.newgrf_grf_version)
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
/// aporta scope/registro persistente de estación, aunque sí aplica la inversión
/// de bit 10 de GRFs anteriores a versión 8.
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
    callback_allows_location_for_grf(result, def.newgrf_grfid, def.newgrf_grf_version)
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

    fn gfx_callback_literal_u16(value: u16) -> TrainSpriteGraphics {
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

    fn gfx_callback_allow_if_parameterized_byte(
        variable: u8,
        parameter: u8,
        shift: u8,
        expected: u8,
    ) -> TrainSpriteGraphics {
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
                    param: Some(parameter),
                    adjust: Action2VarAdjust {
                        shift,
                        and_mask: u32::from(u8::MAX),
                        ..Action2VarAdjust::default()
                    },
                },
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
        assert_eq!(vehicle.newgrf_random_bits & 1, u16::from(expected) & 1);
    }

    #[test]
    fn callbacks_ac_vehicle_random_trigger_reseeds_the_high_random_word() {
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
                randbit: 8,
                sets: vec![0x8000, 0x8001],
            },
        );
        engine.newgrf_runtime = Some(Box::new(gfx));
        let mut vehicle = Vehicle::new(
            45,
            VehicleKind::Train,
            TileCoord::new(2, 3),
            TileCoord::new(2, 3),
        );
        vehicle.newgrf_random_bits = 0x8000;
        assert!(trigger_vehicle_randomisation(
            &engine,
            &mut vehicle,
            VehicleRandomTrigger::Callback32,
            9,
            17,
        ));
        let salt =
            u64::from(vehicle.id) ^ (u64::from(VehicleRandomTrigger::Callback32 as u8) << 32);
        let high = crate::map::industry_tile_rng(9, 17, vehicle.pos, salt ^ 0xA5A5_5A5A);
        assert_eq!(
            vehicle.newgrf_random_bits & 0x0100,
            u16::from(high) << 8 & 0x0100
        );
        // A reseed of bit 8 must not erase an unrelated bit in the word.
        assert_eq!(vehicle.newgrf_random_bits & 0x8000, 0x8000);
    }

    #[test]
    fn callbacks_ac_vehicle_random_trigger_propagates_empty_to_the_chain() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x4348_4149;
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
                triggers: VehicleRandomTrigger::Empty.mask(),
                randbit: 8,
                sets: vec![0x8000, 0x8001],
            },
        );
        engine.newgrf_runtime = Some(Box::new(gfx));
        let mut vehicles = (0..3)
            .map(|id| {
                let mut vehicle = Vehicle::new(
                    100 + id,
                    VehicleKind::Train,
                    TileCoord::new(2, 3),
                    TileCoord::new(2, 3),
                );
                vehicle.engine_id = Some(engine.id);
                vehicle.newgrf_random_bits = 0x8000 | u16::try_from(id).unwrap();
                vehicle
            })
            .collect::<Vec<_>>();
        vehicles[0].next_unit = Some(101);
        vehicles[1].prev_unit = Some(100);
        vehicles[1].next_unit = Some(102);
        vehicles[2].prev_unit = Some(101);

        assert!(trigger_vehicle_randomisation_chain(
            &mut vehicles,
            100,
            std::slice::from_ref(&engine),
            VehicleRandomTrigger::Empty,
            99,
            7,
        ));
        assert!(
            vehicles
                .iter()
                .all(|vehicle| vehicle.newgrf_waiting_random_triggers == 0)
        );
        // Empty propagates the first unit's random word to every unit, while
        // preserving unrelated bits in each vehicle.
        assert_eq!(
            vehicles[0].newgrf_random_bits & 0x0100,
            vehicles[1].newgrf_random_bits & 0x0100
        );
        assert_eq!(
            vehicles[1].newgrf_random_bits & 0x0100,
            vehicles[2].newgrf_random_bits & 0x0100
        );
        assert_eq!(vehicles[0].newgrf_random_bits & 0x8000, 0x8000);
        assert_eq!(vehicles[1].newgrf_random_bits & 0x8000, 0x8000);
        assert_eq!(vehicles[2].newgrf_random_bits & 0x8000, 0x8000);
    }

    #[test]
    fn callbacks_ac_vehicle_new_cargo_runs_any_new_cargo_from_chain_head() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x4E45_5743;
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
                triggers: VehicleRandomTrigger::AnyNewCargo.mask(),
                randbit: 8,
                sets: vec![0x8000, 0x8001],
            },
        );
        engine.newgrf_runtime = Some(Box::new(gfx));
        let mut vehicle = Vehicle::new(
            46,
            VehicleKind::Train,
            TileCoord::new(2, 3),
            TileCoord::new(2, 3),
        );
        vehicle.engine_id = Some(engine.id);
        vehicle.newgrf_random_bits = 0x8000;
        let vehicle_id = vehicle.id;
        assert!(trigger_vehicle_randomisation_chain(
            std::slice::from_mut(&mut vehicle),
            vehicle_id,
            std::slice::from_ref(&engine),
            VehicleRandomTrigger::NewCargo,
            99,
            7,
        ));
        // No group in this fixture consumes the outer `NewCargo` bit; the
        // nested `AnyNewCargo` group must still run and consume only its bit.
        assert_eq!(
            vehicle.newgrf_waiting_random_triggers,
            VehicleRandomTrigger::NewCargo.mask()
        );
        let salt = u64::from(vehicle.id) ^ (u64::from(VehicleRandomTrigger::NewCargo as u8) << 32);
        let high = crate::map::industry_tile_rng(99, 7, vehicle.pos, salt ^ 0xA5A5_5A5A);
        assert_eq!(
            vehicle.newgrf_random_bits & 0x0100,
            u16::from(high) << 8 & 0x0100
        );
    }

    #[test]
    fn callbacks_ac_vehicle_colour_mapping_respects_mask_and_company_bit() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x434F_4C52;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1 << 6;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(0x4000 | 0x0310)));
        let vehicle = Vehicle::new(
            45,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        let mapping = resolve_vehicle_colour_mapping_callback(&engine, &vehicle).unwrap();
        assert_eq!(mapping.palette_id, 0x0310);
        assert!(mapping.apply_company_colour);
        assert_eq!(mapping.palette_for_company(4), 779);
        assert_eq!(
            mapping.palette_for_companies(4, 6, true),
            crate::newgrf_sprites::TWOCC_PALETTE_BASE + 4 + 6 * 16
        );
        assert_eq!(mapping.palette_for_companies(4, 6, false), 779);

        engine.vehicle_callback_mask = 0;
        assert_eq!(
            resolve_vehicle_colour_mapping_callback(&engine, &vehicle),
            None
        );
        engine.vehicle_callback_mask = 1 << 6;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(0x3FFF)));
        let mapping = resolve_vehicle_colour_mapping_callback(&engine, &vehicle).unwrap();
        assert_eq!(mapping.palette_id, 0x3FFF);
        assert!(!mapping.apply_company_colour);
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
    fn callbacks_ac_vehicle_modify_property_sign_extends_and_feeds_length_fallback() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        engine.newgrf_grfid = 0x5052_4F50;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 0;
        engine.shorten_factor = 7;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal(3)));
        let mut vehicle = Vehicle::new(
            46,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert_eq!(
            resolve_vehicle_modify_property_callback(&engine, &mut vehicle, 0x21, false),
            Some(3)
        );
        assert_eq!(vehicle_unit_length(&engine, &mut vehicle), 5);

        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(0x7FFF)));
        assert_eq!(
            resolve_vehicle_modify_property_callback(&engine, &mut vehicle, 0x2E, true),
            Some(-1)
        );
    }

    #[test]
    fn callbacks_ac_vehicle_modify_property_overrides_class_speed_and_falls_back() {
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        let base_speed = engine.max_speed;
        engine.newgrf_grfid = 0x5350_4545;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(77)));
        let mut vehicle = Vehicle::new(
            47,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.engine_id = Some(engine.id);

        assert_eq!(vehicle_max_speed(&engine, &mut vehicle), 77);

        // CALLBACK_FAILED conserva la propiedad Action0 del motor.
        engine.newgrf_runtime = None;
        assert_eq!(vehicle_max_speed(&engine, &mut vehicle), base_speed);
    }

    #[test]
    fn callbacks_ac_vehicle_modify_property_selects_capacity_property_per_class() {
        let mut train_engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        train_engine.newgrf_grfid = 0x4341_5041;
        train_engine.newgrf_local_id = 0;
        train_engine.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x1A, 0, 0x14)));
        let mut train = Vehicle::new(
            48,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        assert_eq!(
            resolve_vehicle_capacity_property_callback(&train_engine, &mut train),
            Some(0x800)
        );

        let mut aircraft_engine = train_engine.clone();
        aircraft_engine.kind = VehicleKind::Aircraft;
        aircraft_engine.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x1A, 0, 0x11)));
        let mut aircraft = Vehicle::new(
            49,
            VehicleKind::Aircraft,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        aircraft.cargo_type = Some(CargoType::Mail);
        assert_eq!(
            resolve_vehicle_capacity_property_callback(&aircraft_engine, &mut aircraft),
            Some(0x800)
        );
    }

    #[test]
    fn callbacks_ac_vehicle_modify_property_resolves_physical_values_per_class() {
        let mut train_engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Train && e.power_hp > 0)
            .cloned()
            .unwrap();
        train_engine.newgrf_grfid = 0x5048_5953;
        train_engine.newgrf_local_id = 0;
        let mut train = Vehicle::new(
            50,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );

        train_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(1234)));
        assert_eq!(vehicle_power_hp(&train_engine, &mut train), 1234);
        train_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(57)));
        assert_eq!(vehicle_weight_t(&train_engine, &mut train), 57);
        train_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(201)));
        assert_eq!(vehicle_tractive_effort(&train_engine, &mut train), 201);

        let mut road_engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Bus)
            .cloned()
            .unwrap();
        road_engine.newgrf_grfid = 0x5048_5953;
        road_engine.newgrf_local_id = 0;
        let mut bus = Vehicle::new(
            51,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        road_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(37)));
        assert_eq!(vehicle_power_hp(&road_engine, &mut bus), 370);
        road_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(13)));
        assert_eq!(vehicle_weight_t(&road_engine, &mut bus), 3);
        road_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(180)));
        assert_eq!(vehicle_tractive_effort(&road_engine, &mut bus), 180);

        // Un resultado cero no se confunde con callback ausente.
        train_engine.newgrf_runtime = Some(Box::new(gfx_callback_literal_u16(0)));
        assert_eq!(vehicle_power_hp(&train_engine, &mut train), 0);
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
    fn callbacks_ac_vehicle_sound_uses_action0_sfx_without_cb33() {
        let mut state = crate::GameState::new(4, 4);
        let mut engine = engines_table()
            .iter()
            .find(|e| e.kind == VehicleKind::Ship)
            .cloned()
            .unwrap();
        engine.id = 4_001;
        engine.newgrf_grfid = 0x5346_5830;
        engine.sound_effect = SoundId::LevelCrossing.as_u8();
        let mut vehicle = Vehicle::new(
            78,
            VehicleKind::Ship,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.engine_id = Some(engine.id);
        state.engine_catalog.push(engine);
        state.vehicles.push(vehicle);

        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 78, VehicleSoundEvent::Start),
            VehicleSoundOverride::Base(SoundId::LevelCrossing)
        );
        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 78, VehicleSoundEvent::Running),
            VehicleSoundOverride::Default
        );

        let engine_index = state
            .engine_catalog
            .iter()
            .position(|candidate| candidate.id == 4_001)
            .unwrap();
        state.engine_catalog[engine_index].sound_effect =
            u8::try_from(crate::sound_id::SOUND_COUNT).unwrap() + 2;
        state.sound_effect_catalog.push(crate::SoundEffectDef {
            local_id: 2,
            grfid: 0x5346_5830,
            volume: 128,
            priority: 9,
            override_old: None,
            has_sample: true,
            sample_pcm: vec![0x80],
            from_newgrf: true,
        });
        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 78, VehicleSoundEvent::Start),
            VehicleSoundOverride::Newgrf {
                grfid: 0x5346_5830,
                local_id: 2,
            }
        );

        state.engine_catalog[engine_index].sound_effect =
            u8::try_from(crate::sound_id::SOUND_COUNT).unwrap() + 3;
        assert_eq!(
            resolve_vehicle_sound_callback(&mut state, 78, VehicleSoundEvent::Start),
            VehicleSoundOverride::Suppressed
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

        // `GetErrorMessageFromLocationCallbackResult` invierte bit 10 para
        // GRF < 8: cero es éxito y 0x400 es el error estándar.
        assert!(callback_allows_location_for_grf(0, 1, 7));
        assert!(!callback_allows_location_for_grf(0x400, 1, 7));
        assert!(!callback_allows_location_for_grf(0, 1, 8));
        assert!(callback_allows_location_for_grf(0x400, 1, 8));
        assert!(callback_allows_location_for_grf(CALLBACK_FAILED, 1, 7));

        assert!(callback_allows_8bit_boolean(CALLBACK_FAILED));
        assert!(!callback_allows_8bit_boolean(0));
        assert!(callback_allows_8bit_boolean(1));
        assert!(callback_allows_8bit_boolean(0xFF));
        assert!(!callback_allows_8bit_boolean(0x100));
    }

    #[test]
    fn foundation_callback_uses_full_boolean_semantics() {
        assert!(callback_draws_default_foundation(CALLBACK_FAILED));
        assert!(!callback_draws_default_foundation(0));
        assert!(callback_draws_default_foundation(1));
        // `ConvertBooleanCallback` checks the complete callback result for
        // foundation callbacks; this differs from `Convert8bitBooleanCallback`.
        assert!(callback_draws_default_foundation(0x100));
        assert!(callback_draws_default_foundation(0x400));
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
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
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
    fn industry_location_build_exposes_creation_param_and_scope() {
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
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "location-scope".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_allow_if_byte(0x18, 0, 2))),
        };
        let state = crate::GameState::new(8, 8);
        let pos = TileCoord::new(2, 3);

        // La API legacy no conoce el tipo de creación y deja `param2 = 0`.
        assert!(!apply_industry_location_callback(&def));
        // El call site de construcción pasa IACT_USERCREATION (=2).
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0
        ));

        // El mismo contexto expone el índice de layout y TileIndex de OpenTTD.
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x86, 0, 4)));
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0
        ));
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x80, 0, 26)));
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0
        ));

        def.associated_badges = vec![7];
        def.newgrf_badge_translation = vec![u16::MAX, 7];
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_parameterized_byte(
            0x7A, 1, 0, 1,
        )));
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0
        ));

        // `0x8B` recorre el rombo Manhattan hasta agua y `0x8F` expone los
        // 32 bits aleatorios del intento de construcción.
        let mut state = state;
        state
            .map
            .set_kind(TileCoord::new(0, 0), crate::map::TileKind::Water)
            .unwrap();
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x8B, 0, 5)));
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0xAB
        ));
        def.newgrf_runtime = Some(Box::new(gfx_callback_allow_if_byte(0x8F, 0, 0xAB)));
        assert!(apply_industry_location_callback_for_build(
            &def, &state, pos, 4, 0xAB
        ));
    }

    #[test]
    fn callbacks_ac_industry_production_change_decodes_actions_and_build_level() {
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
            production_rates: vec![15],
            input_multipliers: Vec::new(),
            callback_mask: crate::industry_spec::INDUSTRY_CALLBACK_PRODUCTION_CHANGE_MASK,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "production-callback".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx_callback_literal(0x02))),
        };
        let industry = Industry::new(
            TileCoord::new(4, 5),
            crate::industry::IndustryKind::CoalMine,
        );
        let mut rng = Randomizer::new(42);
        assert_eq!(
            resolve_industry_production_change_callback(&def, &industry, false, &mut rng),
            Some(IndustryProductionAction::Double)
        );

        def.callback_mask = crate::industry_spec::INDUSTRY_CALLBACK_PROD_CHANGE_BUILD_MASK;
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(64)));
        assert_eq!(
            resolve_industry_production_change_build_callback(&def, &industry, &mut rng),
            Some(64)
        );
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(3)));
        assert_eq!(
            resolve_industry_production_change_build_callback(&def, &industry, &mut rng),
            None,
            "niveles fuera del rango válido conservan el valor vanilla"
        );
    }

    #[test]
    fn industry_production_group_consumes_inputs_and_adds_outputs() {
        let mut runtime = TrainSpriteGraphics::default();
        runtime
            .assigns
            .push(crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 0,
                set_id: 7,
            });
        runtime.industry_production.insert(
            7,
            IndustryProductionGroup {
                version: 0,
                subtract_input: vec![3, 0, 0],
                cargo_input: Vec::new(),
                add_output: vec![5, 0],
                cargo_output: Vec::new(),
                again: 0,
            },
        );
        let def = IndustrySpecDef {
            id: 37,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: vec![5],
            produced_cargo_labels: vec!["GOOD".into()],
            accepted_cargo_indices: vec![4, 6, 9],
            accepted_cargo_labels: vec!["LVST".into(), "GRNT".into(), "STEL".into()],
            production_rates: vec![0],
            input_multipliers: Vec::new(),
            callback_mask: crate::industry_spec::INDUSTRY_CALLBACK_PRODUCTION_CARGO_ARRIVAL_MASK,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "production-group".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut industry = Industry::with_tiles_spec(
            TileCoord::new(4, 5),
            crate::industry::IndustryKind::Factory,
            crate::industry::IndustrySpec::Factory,
            vec![TileCoord::new(4, 5)],
            0,
        );
        industry.add_accepted_cargo_waiting(CargoType::Livestock, 10);
        let mut rng = Randomizer::new(42);
        let applied = apply_industry_production_callback(&def, &mut industry, 0, &mut rng);
        assert_eq!(applied.iterations, 1);
        assert_eq!(applied.inputs_consumed, 3);
        assert_eq!(applied.outputs_added, 5);
        assert_eq!(industry.accepted_cargo_waiting(CargoType::Livestock), 7);
        assert_eq!(industry.stock, 5);
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

        // Los GRF antiguos expresan éxito con cero (bit 10 invertido).
        def.callback_mask = crate::station_class::STATION_CALLBACK_SLOPE_CHECK_MASK;
        def.newgrf_grfid = 0x1234;
        def.newgrf_grf_version = 7;
        def.newgrf_runtime = Some(Box::new(gfx_callback_literal(0)));
        assert!(apply_station_slope_callback_for_build(
            &def, 1, true, 3, 5, 2, 4,
        ));
        def.newgrf_grf_version = 8;
        assert!(!apply_station_slope_callback_for_build(
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
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
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
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 0,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        assert_eq!(apply_industry_tile_anim_callback(&tile), CALLBACK_FAILED);
    }
}
