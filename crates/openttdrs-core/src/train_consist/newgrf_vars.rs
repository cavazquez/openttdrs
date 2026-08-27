//! Variables `NewGRF` Action2 para unidades de tren.

use crate::cargo::CargoType;
use crate::economy::TICKS_PER_DAY;
use crate::engine::{EngineDef, engine_in_catalog};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::news::{calendar_day_index, calendar_year_day};
use crate::tick::GameTick;
use crate::vehicle::Vehicle;

use super::metrics::cargo_unit_weight_16ths;
use super::topology::{consist_head_id, consist_unit_ids};

/// Mapa fijo `CargoType` → cargo type A (clima templado TTD, sin tabla GRF).
#[must_use]
pub fn cargo_type_a_id(cargo: Option<CargoType>) -> u8 {
    match cargo {
        Some(c) => c.temperate_id(),
        None => 0xFF,
    }
}

/// Clase de carga (bits) aproximada para var `47`.
#[must_use]
pub fn cargo_class_bits(cargo: Option<CargoType>) -> u16 {
    match cargo {
        Some(CargoType::Passengers) => 0x0001,
        Some(CargoType::Mail) => 0x0002,
        Some(CargoType::Goods | CargoType::Valuables | CargoType::Candy | CargoType::Food) => {
            0x0020
        }
        Some(
            CargoType::Oil
            | CargoType::Water
            | CargoType::Rubber
            | CargoType::Cola
            | CargoType::Plastic,
        ) => 0x0040,
        Some(_) => 0x0010,
        None => 0,
    }
}

/// Contexto Action2 para dibujar/resolver sprites de una unidad del consist.
///
/// Rellena `random_bits` / `consist_random_bits` y variables de vehículo MVP
/// (`40`, `47`, `48`, `49`, `43`, `5F`, `B2`, `B4`, `B9`, `C0`, `C4`, `C6`, `C8`).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn action2_eval_ctx_for_unit(
    vehicles: &[Vehicle],
    unit_id: u32,
    tick: GameTick,
    engine_catalog: &[EngineDef],
    owner_colour: u8,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let current = vehicles.iter().find(|v| v.id == unit_id);
    let mut cur = Some(unit_id);
    for offset in 0u8..=15 {
        let Some(id) = cur else {
            break;
        };
        let Some(unit) = vehicles.iter().find(|v| v.id == id) else {
            break;
        };
        let bits = u32::from(unit.newgrf_random_bits);
        if offset == 0 {
            ctx.random_bits = bits;
        }
        ctx.consist_random_bits.insert(offset, bits);
        cur = unit.prev_unit;
    }
    // Keep both directions available for relative random groups (`0x84`).
    // The legacy `consist_random_bits` table above is retained because older
    // callers address it by the nibble encoded in the Action2 payload.
    if let Some(unit) = current {
        ctx.vehicle_palette_generation = unit.newgrf_palette_generation;
        ctx.relative_random_bits
            .insert(0, u32::from(unit.newgrf_random_bits));
        fill_relative_vehicle_vars(
            &mut ctx,
            vehicles,
            unit,
            0,
            unit,
            tick,
            engine_catalog,
            owner_colour,
        );
        let mut next = unit.next_unit;
        for distance in 1i16..=15 {
            let Some(id) = next else {
                break;
            };
            let Some(candidate) = vehicles.iter().find(|v| v.id == id) else {
                break;
            };
            ctx.relative_random_bits
                .insert(distance, u32::from(candidate.newgrf_random_bits));
            fill_relative_vehicle_vars(
                &mut ctx,
                vehicles,
                unit,
                distance,
                candidate,
                tick,
                engine_catalog,
                owner_colour,
            );
            next = candidate.next_unit;
        }
        let mut previous = unit.prev_unit;
        for distance in 1i16..=15 {
            let Some(id) = previous else {
                break;
            };
            let Some(candidate) = vehicles.iter().find(|v| v.id == id) else {
                break;
            };
            ctx.relative_random_bits
                .insert(-distance, u32::from(candidate.newgrf_random_bits));
            fill_relative_vehicle_vars(
                &mut ctx,
                vehicles,
                unit,
                -distance,
                candidate,
                tick,
                engine_catalog,
                owner_colour,
            );
            previous = candidate.prev_unit;
        }

        // Action2 random type 0x84 direction 3 starts at the first vehicle
        // in the contiguous run ending at the current vehicle whose engine
        // id matches the current one, then advances by the encoded count.
        // OpenTTD calls this the relative scope used by vehicle variable 41.
        // Keep the table keyed by the count (rather than by chain offset) so
        // the evaluator can apply a dynamic count from register 0x100.
        if let Some(engine_id) = vehicle_engine_identity(unit) {
            let mut first_same_offset = 0i16;
            let mut previous = unit.prev_unit;
            while let Some(id) = previous {
                let Some(candidate) = vehicles.iter().find(|v| v.id == id) else {
                    break;
                };
                if vehicle_engine_identity(candidate) != Some(engine_id) {
                    break;
                }
                first_same_offset -= 1;
                previous = candidate.prev_unit;
            }
            for count in 0i16..=15 {
                let target_offset = first_same_offset + count;
                if let Some(bits) = ctx.relative_random_bits.get(&target_offset) {
                    ctx.relative_same_engine_random_bits.insert(count, *bits);
                }
            }
        }
    }
    fill_vehicle_action2_vars(
        &mut ctx,
        vehicles,
        unit_id,
        tick,
        engine_catalog,
        owner_colour,
    );
    // For an articulated child/wagon, OpenTTD's parent scope is the vehicle
    // immediately toward the engine (`Previous()`).  Build that scope with
    // the same catalogue and tick so all vehicle variables (including cargo,
    // consist position and persistent storage) use one consistent snapshot.
    if let Some(parent_id) = current.and_then(|unit| unit.prev_unit) {
        let mut parent_ctx = Action2EvalCtx::default();
        fill_vehicle_action2_vars(
            &mut parent_ctx,
            vehicles,
            parent_id,
            tick,
            engine_catalog,
            owner_colour,
        );
        ctx.parent_vars = parent_ctx.vars;
        ctx.parent_parameterized_vars = parent_ctx.parameterized_vars;
        ctx.parent_persistent_registers = parent_ctx.persistent_registers;
        ctx.parent_random_bits = vehicles
            .iter()
            .find(|vehicle| vehicle.id == parent_id)
            .map_or(0, |vehicle| u32::from(vehicle.newgrf_random_bits));
        ctx.parent_vehicle_palette_generation = vehicles
            .iter()
            .find(|vehicle| vehicle.id == parent_id)
            .map_or(0, |vehicle| vehicle.newgrf_palette_generation);
    }
    ctx
}

/// Engine identity used by `NewGRF` scopes. Imported vehicles retain the
/// native `OpenTTD` engine type; newly-created vehicles only have the catalog
/// id, so the latter is the fallback for runtime-created consists.
fn vehicle_engine_identity(vehicle: &Vehicle) -> Option<u16> {
    vehicle.native_engine_type.or(vehicle.engine_id)
}

fn fill_vehicle_action2_vars(
    ctx: &mut Action2EvalCtx,
    vehicles: &[Vehicle],
    unit_id: u32,
    tick: GameTick,
    engine_catalog: &[EngineDef],
    owner_colour: u8,
) {
    let Some(unit) = vehicles.iter().find(|v| v.id == unit_id) else {
        return;
    };
    let head_id = consist_head_id(vehicles, unit_id).unwrap_or(unit_id);
    let ids = consist_unit_ids(vehicles, head_id);
    let n = ids.len();
    let ff = ids.iter().position(|&id| id == unit_id).unwrap_or(0);
    let bb = n.saturating_sub(1).saturating_sub(ff);
    let nn = n.saturating_sub(1); // var 40: zero-based count
    let var40 = u32::from(u8::try_from(ff).unwrap_or(0xFF))
        | (u32::from(u8::try_from(bb).unwrap_or(0xFF)) << 8)
        | (u32::from(u8::try_from(nn).unwrap_or(0xFF)) << 16);
    ctx.vars.insert(0x40, var40);

    let cargo = unit.cargo_type;
    // `RealSpriteGroup::Resolve` switches between its loaded and loading
    // lists while a vehicle is inside a load/unload window. Keep the state and
    // the proportional cargo stage in the shared Action2 context so render,
    // previews and callbacks make the same choice.
    ctx.vehicle_loading = unit.cargo_loading || unit.cargo_unloading;
    ctx.vehicle_cargo = unit.cargo;
    ctx.vehicle_capacity = unit.capacity;
    let tt = cargo_type_a_id(cargo);
    let ww = cargo_unit_weight_16ths(cargo);
    let cccc = u32::from(cargo_class_bits(cargo));
    let var47 = u32::from(tt) | (u32::from(ww) << 8) | (cccc << 16);
    ctx.vars.insert(0x47, var47);
    ctx.vars.insert(0xB9, u32::from(tt));

    // bit0 = available on market
    ctx.vars.insert(0x48, 1);

    let build_year = calendar_year_day(calendar_day_index(GameTick::new(unit.build_tick))).0;
    ctx.vars.insert(0x49, build_year);
    ctx.vars.insert(
        0xC4,
        u32::from(u8::try_from(build_year.saturating_sub(1920).min(255)).unwrap_or(255)),
    );

    let age_days = tick.get().saturating_sub(unit.build_tick) / u64::from(TICKS_PER_DAY);
    ctx.vars.insert(
        0xC0,
        u32::try_from(age_days.min(u64::from(u16::MAX))).unwrap_or(u32::from(u16::MAX)),
    );

    // 43: Ccttmmnn — colour primary/secondary + company id
    let nn_player = u32::from(unit.owner.0);
    let mm = 0u32; // single-player host
    let tt_player = 0u32; // human
    let c = u32::from(owner_colour & 0x0F);
    let var43 = nn_player | (mm << 8) | (tt_player << 16) | ((c | (c << 4)) << 24);
    ctx.vars.insert(0x43, var43);

    // 5F: triggers low byte + random bits in other bytes.  The waiting mask
    // must survive the Action2 snapshot so `CBID_RANDOM_TRIGGER` can consume
    // only the events matched by the active random group.
    let random_data =
        u32::from(unit.newgrf_random_bits) << 8 | u32::from(unit.newgrf_waiting_random_triggers);
    ctx.vars.insert(0x5F, random_data);

    let mut status = 0u32;
    if !unit.running {
        status |= 1 << 1;
    }
    ctx.vars.insert(0xB2, status);
    ctx.vars.insert(0xB4, u32::from(unit.cur_speed));

    let eng = unit
        .engine_id
        .and_then(|id| engine_in_catalog(engine_catalog, id));
    let local_id = eng.map_or(0, |e| e.newgrf_local_id);
    ctx.vars.insert(0xC6, u32::from(local_id));
    // FD = trains forward
    ctx.vars.insert(0xC8, 0xFD);

    ctx.persistent_registers
        .clone_from(&unit.newgrf_persistent_regs);
}

/// Populate the relative vehicle tables used by Action2 variables `61`/`62`.
///
/// `61` is evaluated through registers `10F` (signed vehicle offset) and
/// `10E` (secondary parameter), while `62` carries its signed offset in the
/// term itself.  The table is deliberately derived from the same
/// `fill_vehicle_action2_vars` snapshot as the self/parent scopes, avoiding a
/// second source of cargo/build-year/consist-position values.
#[allow(clippy::too_many_arguments)]
fn fill_relative_vehicle_vars(
    ctx: &mut Action2EvalCtx,
    vehicles: &[Vehicle],
    current: &Vehicle,
    offset: i16,
    candidate: &Vehicle,
    tick: GameTick,
    engine_catalog: &[EngineDef],
    owner_colour: u8,
) {
    let mut candidate_ctx = Action2EvalCtx::default();
    fill_vehicle_action2_vars(
        &mut candidate_ctx,
        vehicles,
        candidate.id,
        tick,
        engine_catalog,
        owner_colour,
    );
    for (&variable, &value) in &candidate_ctx.vars {
        ctx.relative_vars.insert((offset, variable), value);
    }
    // Upstream exposes random bits through var 5F as random<<8 | triggers.
    // Keep the low trigger byte as part of the relative scope too; variable
    // 61/5F is frequently used by vehicle sprite groups after a cargo/depot
    // event.
    ctx.relative_vars.insert(
        (offset, 0x5F),
        u32::from(candidate.newgrf_random_bits) << 8
            | u32::from(candidate.newgrf_waiting_random_triggers),
    );

    if is_ground_vehicle(current) && is_ground_vehicle(candidate) {
        // A var 61 lookup may ask the selected vehicle for var 62.  OpenTTD
        // evaluates that second lookup relative to the selected vehicle, not
        // relative to the original resolver.  Materialize the signed byte
        // offsets in the parameterized table so the Action2 evaluator can do
        // the same without retaining a live vehicle pointer.
        for nested_offset in i16::from(i8::MIN)..=i16::from(i8::MAX) {
            let Some(nested_candidate) = vehicle_at_relative(vehicles, candidate, nested_offset)
            else {
                continue;
            };
            let nested_curvature =
                vehicle_relative_curvature(candidate, nested_candidate, nested_offset);
            let encoded_offset = nested_offset.to_le_bytes()[0];
            ctx.relative_parameterized_vars
                .insert((offset, 0x62, encoded_offset), nested_curvature);
        }
    }

    ctx.relative_vars.insert(
        (offset, 0x62),
        vehicle_relative_curvature(current, candidate, offset),
    );
}

fn is_ground_vehicle(vehicle: &Vehicle) -> bool {
    matches!(
        vehicle.kind,
        crate::vehicle::VehicleKind::Train
            | crate::vehicle::VehicleKind::Bus
            | crate::vehicle::VehicleKind::Truck
            | crate::vehicle::VehicleKind::Tram
    )
}

fn vehicle_at_relative<'a>(
    vehicles: &'a [Vehicle],
    current: &Vehicle,
    offset: i16,
) -> Option<&'a Vehicle> {
    let mut id = Some(current.id);
    if offset < 0 {
        for _ in 0..offset.unsigned_abs() {
            let current_id = id?;
            id = vehicles
                .iter()
                .find(|vehicle| vehicle.id == current_id)?
                .prev_unit;
        }
    } else {
        for _ in 0..offset {
            let current_id = id?;
            id = vehicles
                .iter()
                .find(|vehicle| vehicle.id == current_id)?
                .next_unit;
        }
    }
    vehicles.iter().find(|vehicle| Some(vehicle.id) == id)
}

fn vehicle_relative_curvature(current: &Vehicle, candidate: &Vehicle, offset: i16) -> u32 {
    let previous = offset < 0;
    let direction = if previous {
        crate::train_movement::dir_difference(candidate.direction, current.direction)
    } else {
        crate::train_movement::dir_difference(current.direction, candidate.direction)
    };
    let mut curvature = u32::from(direction.min(7));
    if direction > 2 {
        curvature |= 0x08;
    }
    if candidate.crashed {
        curvature |= 0x80;
    }
    let (dx, dy) = if previous {
        (
            candidate.pos.x - current.pos.x,
            candidate.pos.y - current.pos.y,
        )
    } else {
        (
            current.pos.x - candidate.pos.x,
            current.pos.y - candidate.pos.y,
        )
    };
    let dz = if previous {
        i32::from(candidate.z_pos.unwrap_or(0)) - i32::from(current.z_pos.unwrap_or(0))
    } else {
        i32::from(current.z_pos.unwrap_or(0)) - i32::from(candidate.z_pos.unwrap_or(0))
    };
    curvature |= (dx.cast_unsigned() & 0xFF) << 8;
    curvature |= (dy.cast_unsigned() & 0xFF) << 16;
    curvature |= (dz.cast_unsigned() & 0xFF) << 24;
    curvature
}
