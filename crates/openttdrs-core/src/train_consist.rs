//! Consist ferroviario: locomotora (cabeza) + cadena de unidades (`Next()`).
//!
//! Longitud en unidades `OpenTTD` (`VEHICLE_LENGTH = 8` por unidad). La cabeza
//! lleva órdenes y pathfinding; los vagones siguen la posición de la cabeza
//! con offset por longitud acumulada.

use crate::cargo::CargoType;
use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::engine::{EngineDef, engine_by_id, engine_in_catalog};
use crate::map::TileCoord;
use crate::newgrf_sprites::Action2EvalCtx;
use crate::news::{calendar_day_index, calendar_year_day};
use crate::tick::GameTick;
use crate::vehicle::{Vehicle, VehicleKind};

/// Longitud de una unidad de tren en fracciones de tesela (`OpenTTD` `VEHICLE_LENGTH`).
pub const VEHICLE_LENGTH: u8 = 8;
/// Fracciones de tesela por tesela completa.
pub const TILE_FRACTIONS: u16 = 256;

/// Mapa fijo `CargoType` → cargo type A (clima templado TTD, sin tabla GRF).
#[must_use]
pub fn cargo_type_a_id(cargo: Option<CargoType>) -> u8 {
    match cargo {
        Some(CargoType::Passengers) => 0,
        Some(CargoType::Coal) => 1,
        Some(CargoType::Mail) => 2,
        Some(CargoType::Oil) => 3,
        Some(CargoType::Goods) => 5,
        Some(CargoType::Wood) => 7,
        None => 0xFF,
    }
}

/// Clase de carga (bits) aproximada para var `47`.
#[must_use]
pub fn cargo_class_bits(cargo: Option<CargoType>) -> u16 {
    match cargo {
        Some(CargoType::Passengers) => 0x0001,
        Some(CargoType::Mail) => 0x0002,
        Some(CargoType::Goods) => 0x0020,
        Some(CargoType::Coal | CargoType::Wood) => 0x0010,
        Some(CargoType::Oil) => 0x0040,
        None => 0,
    }
}

fn cargo_unit_weight_16ths(cargo: Option<CargoType>) -> u8 {
    match cargo {
        Some(CargoType::Passengers) => 1,
        Some(CargoType::Mail) => 4,
        Some(CargoType::Goods) => 8,
        Some(CargoType::Coal | CargoType::Wood | CargoType::Oil) => 16,
        None => 0,
    }
}

/// ¿El motor es un vagón (sin potencia, con capacidad de carga)?
#[must_use]
pub fn engine_is_wagon(engine: &EngineDef) -> bool {
    engine.kind == VehicleKind::Train && engine.power_hp == 0 && engine.capacity > 0
}

/// ¿El motor es locomotora o DMU (puede ser cabeza de consist)?
#[must_use]
pub fn engine_is_train_engine(engine: &EngineDef) -> bool {
    engine.kind == VehicleKind::Train && !engine_is_wagon(engine)
}

/// Recorre la cadena desde `head_id` hacia atrás (`next_unit`).
#[must_use]
pub fn consist_unit_ids(vehicles: &[Vehicle], head_id: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cur = Some(head_id);
    let mut guard = 0_u32;
    while let Some(id) = cur {
        if guard > 256 {
            break;
        }
        guard += 1;
        out.push(id);
        cur = vehicles
            .iter()
            .find(|v| v.id == id)
            .and_then(|v| v.next_unit);
    }
    out
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

    let age_days = tick.get().saturating_sub(unit.build_tick) / u64::from(TICKS_PER_TRANSIT_DAY);
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
}

/// ID de la cabeza del consist que contiene `vehicle_id`.
#[must_use]
pub fn consist_head_id(vehicles: &[Vehicle], vehicle_id: u32) -> Option<u32> {
    let mut cur = vehicle_id;
    let mut guard = 0_u32;
    loop {
        if guard > 256 {
            return None;
        }
        guard += 1;
        let v = vehicles.iter().find(|v| v.id == cur)?;
        match v.prev_unit {
            Some(prev) => cur = prev,
            None => return Some(cur),
        }
    }
}

/// ¿`other_id` pertenece al mismo consist que `vehicle_id`?
#[must_use]
pub fn same_consist(vehicles: &[Vehicle], vehicle_id: u32, other_id: u32) -> bool {
    match (
        consist_head_id(vehicles, vehicle_id),
        consist_head_id(vehicles, other_id),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => vehicle_id == other_id,
    }
}

/// Recalcula `cached_total_length` y capacidad agregada en la cabeza.
pub fn consist_changed(vehicles: &mut [Vehicle], head_id: u32) {
    let ids = consist_unit_ids(vehicles, head_id);
    if ids.is_empty() {
        return;
    }
    let mut total_len = 0_u16;
    let mut total_cap = 0_u32;
    let mut total_weight = 0_u16;
    let mut total_power = 0_u32;
    let mut cargo_type = None;
    for &id in &ids {
        let Some(v) = vehicles.iter().find(|v| v.id == id) else {
            continue;
        };
        total_len = total_len.saturating_add(u16::from(v.unit_length.max(1)));
        let eng = v
            .engine_id
            .and_then(engine_by_id)
            .unwrap_or_else(|| crate::engine::engine_for_vehicle(v.kind, 0));
        total_weight = total_weight.saturating_add(eng.weight_t);
        total_power = total_power.saturating_add(eng.power_hp);
        if eng.capacity > 0 {
            total_cap = total_cap.saturating_add(eng.capacity);
            if cargo_type.is_none() {
                cargo_type = eng.cargo;
            }
        }
    }
    // Capacidad mínima 1 para locos solas (compatibilidad con trenes puntuales previos).
    if total_cap == 0 {
        total_cap = crate::vehicle::VEHICLE_CAPACITY;
    }
    if let Some(head) = vehicles.iter_mut().find(|v| v.id == head_id) {
        head.cached_total_length = total_len.max(u16::from(VEHICLE_LENGTH));
        head.capacity = total_cap;
        head.cached_power_hp = total_power;
        head.cached_weight_t = total_weight.max(1);
        if head.cargo_type.is_none() {
            head.cargo_type = cargo_type;
        }
    }
    let head_pos = vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map(|v| (v.pos, v.direction, v.running, v.progress));
    let Some((pos, dir, running, progress)) = head_pos else {
        return;
    };
    for &id in ids.iter().skip(1) {
        if let Some(v) = vehicles.iter_mut().find(|v| v.id == id) {
            v.pos = pos;
            v.direction = dir;
            v.running = running;
            v.progress = progress;
            v.path.clear();
            v.orders.clear();
            v.current_order = 0;
        }
    }
}

/// Capacidad total del consist (cabeza).
#[must_use]
pub fn consist_capacity(vehicles: &[Vehicle], head_id: u32) -> u32 {
    vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(0, |v| v.capacity)
}

/// Peso total (t) del consist.
#[must_use]
pub fn consist_weight_t(vehicles: &[Vehicle], head_id: u32) -> u16 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.weight_t))
        .fold(0_u16, u16::saturating_add)
}

/// Potencia total (HP) del consist.
#[must_use]
pub fn consist_power_hp(vehicles: &[Vehicle], head_id: u32) -> u32 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.power_hp))
        .fold(0_u32, u32::saturating_add)
}

/// Número de teselas que ocupa el consist (redondeo hacia arriba).
#[must_use]
pub fn consist_tile_span(vehicles: &[Vehicle], head_id: u32) -> u32 {
    let len = vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(u16::from(VEHICLE_LENGTH), |v| v.cached_total_length);
    u32::from(len).div_ceil(u32::from(TILE_FRACTIONS)).max(1)
}

/// Teselas ocupadas por el consist: cabeza + cola.
///
/// Preferencia: historial real de la cabeza (`rail_tile_history`); si falta,
/// vecinos en sentido opuesto a la dirección (MVP).
#[must_use]
pub fn consist_occupied_tiles(vehicles: &[Vehicle], head_id: u32) -> Vec<TileCoord> {
    let Some(head) = vehicles.iter().find(|v| v.id == head_id) else {
        return Vec::new();
    };
    let span = consist_tile_span(vehicles, head_id) as usize;
    let mut tiles = vec![head.pos];
    if span <= 1 {
        return tiles;
    }
    // Historial: teselas que la cabeza acaba de abandonar (frente = más reciente).
    for &t in head.rail_tile_history.iter().take(span.saturating_sub(1)) {
        if tiles.last() != Some(&t) {
            tiles.push(t);
        }
        if tiles.len() >= span {
            return tiles;
        }
    }
    let back = opposite_diag(head.direction);
    let mut cur = *tiles.last().unwrap_or(&head.pos);
    while tiles.len() < span {
        let next = offset_tile(cur, back);
        tiles.push(next);
        cur = next;
    }
    tiles
}

fn opposite_diag(dir: u8) -> u8 {
    dir.wrapping_add(4) % 8
}

fn offset_tile(c: TileCoord, dir: u8) -> TileCoord {
    let (dx, dy) = match dir {
        0 => (0, -1),  // N
        1 => (1, -1),  // NE
        2 => (1, 0),   // E
        3 => (1, 1),   // SE
        4 => (0, 1),   // S
        5 => (-1, 1),  // SW
        6 => (-1, 0),  // W
        _ => (-1, -1), // NW
    };
    TileCoord::new(c.x + dx, c.y + dy)
}

/// Engancha `wagon_id` al final del consist de `head_id`.
#[allow(clippy::result_unit_err)]
pub fn attach_wagon(vehicles: &mut [Vehicle], head_id: u32, wagon_id: u32) -> Result<(), ()> {
    if head_id == wagon_id {
        return Err(());
    }
    if consist_head_id(vehicles, wagon_id) != Some(wagon_id) {
        // Solo cabezas de cadena (vagón suelto o loco) se pueden enganchar como unidad.
        // Si el vagón ya tiene prev, rechazar.
        if vehicles
            .iter()
            .find(|v| v.id == wagon_id)
            .is_some_and(|v| v.prev_unit.is_some())
        {
            return Err(());
        }
    }
    let ids = consist_unit_ids(vehicles, head_id);
    let Some(&tail_id) = ids.last() else {
        return Err(());
    };
    if ids.contains(&wagon_id) {
        return Err(());
    }
    // Desconectar wagon de su cadena previa (si era cabeza de otra).
    let wagon_next = vehicles
        .iter()
        .find(|v| v.id == wagon_id)
        .and_then(|v| v.next_unit);
    if let Some(n) = wagon_next
        && let Some(v) = vehicles.iter_mut().find(|v| v.id == n)
    {
        v.prev_unit = None;
    }
    if let Some(w) = vehicles.iter_mut().find(|v| v.id == wagon_id) {
        w.next_unit = None;
        w.prev_unit = Some(tail_id);
        w.orders.clear();
        w.current_order = 0;
        w.path.clear();
    }
    if let Some(t) = vehicles.iter_mut().find(|v| v.id == tail_id) {
        t.next_unit = Some(wagon_id);
    }
    consist_changed(vehicles, head_id);
    Ok(())
}

/// Desengancha `unit_id` del consist; queda como cabeza suelta.
#[allow(clippy::result_unit_err)]
pub fn detach_unit(vehicles: &mut [Vehicle], unit_id: u32) -> Result<(), ()> {
    let Some(v) = vehicles.iter().find(|x| x.id == unit_id) else {
        return Err(());
    };
    let prev = v.prev_unit;
    let next = v.next_unit;
    let old_head = consist_head_id(vehicles, unit_id).unwrap_or(unit_id);

    if let Some(p) = prev
        && let Some(pv) = vehicles.iter_mut().find(|x| x.id == p)
    {
        pv.next_unit = next;
    }
    if let Some(n) = next
        && let Some(nv) = vehicles.iter_mut().find(|x| x.id == n)
    {
        nv.prev_unit = prev;
    }
    if let Some(u) = vehicles.iter_mut().find(|x| x.id == unit_id) {
        u.prev_unit = None;
        u.next_unit = None;
        u.cached_total_length = u16::from(u.unit_length.max(1));
    }
    if old_head != unit_id {
        consist_changed(vehicles, old_head);
    }
    if next.is_some()
        && let Some(new_head) = next
    {
        // El resto de la cadena detrás del detach: su nueva cabeza es `next`.
        if let Some(nv) = vehicles.iter_mut().find(|x| x.id == new_head) {
            nv.prev_unit = None;
        }
        consist_changed(vehicles, new_head);
    }
    consist_changed(vehicles, unit_id);
    Ok(())
}

/// IDs a vender: si es cabeza, toda la cadena; si es vagón, solo esa unidad (tras detach implícito).
#[must_use]
pub fn sell_chain_ids(vehicles: &[Vehicle], vehicle_id: u32) -> Vec<u32> {
    let Some(v) = vehicles.iter().find(|x| x.id == vehicle_id) else {
        return Vec::new();
    };
    if v.prev_unit.is_none() {
        consist_unit_ids(vehicles, vehicle_id)
    } else {
        vec![vehicle_id]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::economy::TICKS_PER_TRANSIT_DAY;
    use crate::vehicle::VehicleKind;

    fn train(id: u32) -> Vehicle {
        let mut v = Vehicle::new(
            id,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        v.unit_length = VEHICLE_LENGTH;
        v.cached_total_length = u16::from(VEHICLE_LENGTH);
        v
    }

    #[test]
    fn attach_and_detach_wagon() {
        let mut vs = vec![train(1), train(2)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert_eq!(vs[0].next_unit, Some(2));
        assert_eq!(vs[1].prev_unit, Some(1));
        assert_eq!(consist_unit_ids(&vs, 1), vec![1, 2]);
        assert!(vs[0].cached_total_length >= 16);
        assert!(detach_unit(&mut vs, 2).is_ok());
        assert_eq!(vs[0].next_unit, None);
        assert_eq!(vs[1].prev_unit, None);
    }

    #[test]
    fn action2_ctx_counts_back_to_head() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[0].newgrf_random_bits = 0x11;
        vs[1].newgrf_random_bits = 0x22;
        vs[2].newgrf_random_bits = 0x33;
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        vs[2].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        let ctx = action2_eval_ctx_for_unit(&vs, 3, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(ctx.random_bits, 0x33);
        assert_eq!(ctx.consist_random_bits.get(&0), Some(&0x33));
        assert_eq!(ctx.consist_random_bits.get(&1), Some(&0x22));
        assert_eq!(ctx.consist_random_bits.get(&2), Some(&0x11));
    }

    #[test]
    fn action2_ctx_var40_consist_position() {
        let mut vs = vec![train(1), train(2), train(3)];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        vs[2].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        let ctx = action2_eval_ctx_for_unit(&vs, 2, crate::tick::GameTick::new(0), &[], 4);
        // head=1 ff=0; unit2 ff=1 bb=1; nn=2 (3 vehicles zero-based)
        let v40 = ctx.vars.get(&0x40).copied();
        assert_eq!(v40.map(|v| v & 0xFF), Some(1)); // ff
        assert_eq!(v40.map(|v| (v >> 8) & 0xFF), Some(1)); // bb
        assert_eq!(v40.map(|v| (v >> 16) & 0xFF), Some(2)); // nn
    }

    #[test]
    fn action2_ctx_cargo_vars() {
        let mut v = train(10);
        v.cargo_type = Some(CargoType::Coal);
        v.cur_speed = 40;
        v.running = false;
        let vs = vec![v];
        let ctx = action2_eval_ctx_for_unit(
            &vs,
            10,
            crate::tick::GameTick::new(u64::from(TICKS_PER_TRANSIT_DAY) * 10),
            &[],
            2,
        );
        assert_eq!(ctx.vars.get(&0xB9), Some(&1)); // coal type A
        assert_eq!(ctx.vars.get(&0x47).map(|v| v & 0xFF), Some(1));
        assert_eq!(ctx.vars.get(&0xB4), Some(&40));
        assert_eq!(ctx.vars.get(&0xB2), Some(&(1 << 1)));
        assert_eq!(ctx.vars.get(&0xC8), Some(&0xFD));
    }

    #[test]
    fn consist_tile_span_grows_with_units() {
        let mut vs = vec![train(1), train(2), train(3)];
        assert!(attach_wagon(&mut vs, 1, 2).is_ok());
        assert!(attach_wagon(&mut vs, 1, 3).is_ok());
        // 3 * 8 = 24 fracciones → 1 tesela (24 < 256)
        assert_eq!(consist_tile_span(&vs, 1), 1);
        // Forzar longitudes grandes
        vs[0].unit_length = 100;
        vs[1].unit_length = 100;
        vs[2].unit_length = 100;
        consist_changed(&mut vs, 1);
        assert!(consist_tile_span(&vs, 1) >= 2);
    }
}
