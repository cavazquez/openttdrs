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
        Some(CargoType::Goods | CargoType::Valuables | CargoType::Candy | CargoType::Food) => 0x0020,
        Some(CargoType::Oil | CargoType::Water | CargoType::Rubber | CargoType::Cola | CargoType::Plastic) => 0x0040,
        Some(_) => 0x0010,
        None => 0,
    }
}

/// Contexto Action2 para dibujar/resolver sprites de una unidad del consist.
///
/// Rellena `random_bits` / `consist_random_bits` y variables de vehículo MVP
/// (`40`, `47`, `48`, `49`, `43`, `5F`, `B2`, `B4`, `B9`, `C0`, `C4`, `C6`, `C8`).
#[must_use]
pub fn action2_eval_ctx_for_unit(
    vehicles: &[Vehicle],
    unit_id: u32,
    tick: GameTick,
    engine_catalog: &[EngineDef],
    owner_colour: u8,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
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
    fill_vehicle_action2_vars(
        &mut ctx,
        vehicles,
        unit_id,
        tick,
        engine_catalog,
        owner_colour,
    );
    ctx
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

    // 5F: triggers low byte + random bits in other bytes
    let random_data = u32::from(unit.newgrf_random_bits) << 8;
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
