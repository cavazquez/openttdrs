//! Enganche, desenganche y venta de unidades.

use crate::vehicle::Vehicle;

use super::topology::{consist_changed, consist_head_id, consist_unit_ids};

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
    // No partir un par dual-headed.
    if v.other_multiheaded_part.is_some() {
        return Err(());
    }
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
