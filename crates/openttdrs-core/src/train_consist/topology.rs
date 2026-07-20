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
    let mut tilt = true;
    let mut curve_mod = i16::MAX;
    let mut saw_engine = false;
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
        if eng.is_train_engine() || eng.is_wagon() {
            saw_engine = true;
            tilt = tilt && eng.rail_tilts;
            curve_mod = curve_mod.min(eng.curve_speed_mod);
        }
    }
    if !saw_engine {
        tilt = false;
        curve_mod = 0;
    } else if curve_mod == i16::MAX {
        curve_mod = 0;
    }
    // Los trenes genéricos sin definición de motor se usan en escenarios y
    // saves antiguos; las locomotoras reales se filtran al cargar en estación.
    if total_cap == 0 {
        total_cap = crate::vehicle::VEHICLE_CAPACITY;
    }
    if let Some(head) = vehicles.iter_mut().find(|v| v.id == head_id) {
        head.cached_total_length = total_len.max(u16::from(super::VEHICLE_LENGTH));
        head.capacity = total_cap;
        head.cached_power_hp = total_power;
        head.cached_weight_t = total_weight.max(1);
        let te_coeff = head
            .engine_id
            .map_or(75, crate::engine::vanilla_train_tractive_effort);
        head.cached_max_te_n = crate::engine::train_max_te_n(head.cached_weight_t, te_coeff);
        let display_max = head
            .engine_id
            .and_then(crate::engine::engine_by_id)
            .map_or(128, |e| e.max_speed);
        let parts = u32::try_from(ids.len()).unwrap_or(1);
        head.cached_air_drag = crate::engine::train_default_air_drag(display_max, parts);
        head.cached_tilt = tilt;
        head.cached_curve_speed_mod = curve_mod;
        if head.cargo_type.is_none() {
            head.cargo_type = cargo_type;
        }
    }
    sync_consist_followers_and_curve_cache(vehicles, head_id, &ids);
}

/// Sincroniza poses derivadas de las unidades y `cached_max_curve_speed`.
fn sync_consist_followers_and_curve_cache(vehicles: &mut [Vehicle], head_id: u32, ids: &[u32]) {
    let head_snap = vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map(|v| v.direction);
    let Some(head_dir) = head_snap else {
        return;
    };
    super::controller::propagate_consist_unit_poses(vehicles, head_id);
    let mut units = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(v) = vehicles.iter().find(|v| v.id == id) {
            units.push((v.direction, v.unit_length.max(1)));
        }
    }
    if let Some(head) = vehicles.iter_mut().find(|v| v.id == head_id) {
        head.curve_prev_direction = head_dir;
        head.cached_max_curve_speed = crate::engine::get_curve_speed_limit(
            crate::engine::TrainAccelerationModel::Realistic,
            &units,
            0,
            head.cached_tilt,
            head.cached_curve_speed_mod,
        );
    }
}
