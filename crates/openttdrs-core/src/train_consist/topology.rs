//! Topología del consist: recorrido de cadena, identificación de cabeza/cola.

use crate::engine::{EngineDef, engine_by_id};
use crate::vehicle::{Vehicle, VehicleKind};

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
        // Multihead: cada cabina aporta la mitad de la potencia del motor.
        let unit_power = if eng.is_dual_headed() || v.other_multiheaded_part.is_some() {
            eng.power_hp / 2
        } else {
            eng.power_hp
        };
        total_power = total_power.saturating_add(unit_power);
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
        head.cached_total_length = total_len.max(u16::from(super::VEHICLE_LENGTH));
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
