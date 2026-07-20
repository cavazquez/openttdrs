//! Cascada de poses de unidades después del avance autoritativo de la cabeza.

use crate::vehicle::Vehicle;

use super::pose::consist_unit_poses;
use super::topology::consist_unit_ids;

/// Persiste las poses proyectadas de los followers tras avanzar la cabeza.
///
/// La cabeza conserva velocidad, `progress`, path y reservas; esta fase solo
/// propaga la geometría física a las unidades siguientes. El controlador de
/// track/PBS por unidad se añadirá sobre esta API.
pub fn propagate_consist_unit_poses(vehicles: &mut [Vehicle], head_id: u32) {
    let ids = consist_unit_ids(vehicles, head_id);
    let Some(head) = vehicles.iter().find(|v| v.id == head_id) else {
        return;
    };
    let head_pos = head.pos;
    let head_dir = head.direction;
    let head_history_empty = head.rail_tile_history.is_empty();
    let running = head.running;
    // Depósito / composición recién montada: todas las unidades comparten tesela
    // y aún no hay historial de vía. No proyectar fuera del depósito.
    let stacked_on_head = ids.iter().all(|&id| {
        vehicles
            .iter()
            .find(|v| v.id == id)
            .is_some_and(|v| v.pos == head_pos)
    });
    if head_history_empty && stacked_on_head {
        for id in ids.into_iter().skip(1) {
            if let Some(unit) = vehicles.iter_mut().find(|v| v.id == id) {
                unit.direction = head_dir;
                unit.curve_prev_direction = head_dir;
                unit.running = running;
                unit.path.clear();
                unit.orders.clear();
                unit.current_order = 0;
            }
        }
        return;
    }

    let poses = consist_unit_poses(vehicles, head_id);
    for (index, id) in ids.into_iter().enumerate().skip(1) {
        let Some(pose) = poses.get(index).copied() else {
            continue;
        };
        if let Some(unit) = vehicles.iter_mut().find(|v| v.id == id) {
            unit.pos = pose.tile;
            unit.rail_pixel = pose.rail_pixel;
            unit.direction = pose.direction;
            unit.curve_prev_direction = pose.curve_prev_direction;
            unit.running = running;
            unit.path.clear();
            unit.orders.clear();
            unit.current_order = 0;
        }
    }
}
