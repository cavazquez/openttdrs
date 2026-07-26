//! Cascada de poses de unidades después del avance autoritativo de la cabeza.
//!
//! Cada follower se coloca con `CalcNextVehicleOffset` respecto a la unidad
//! precedente ([`super::pose::consist_unit_poses`]).

use crate::map::{Map, TileKind};
use crate::vehicle::{Vehicle, VehicleKind, reverse_direction};

use super::pose::consist_unit_poses;
use super::topology::consist_unit_ids;

/// Persiste las poses unidad a unidad tras avanzar la cabeza.
///
/// La cabeza conserva velocidad, `progress`, path y reservas; esta fase solo
/// propaga la geometría física a las unidades siguientes vía offsets
/// `CalcNextVehicleOffset`.
pub fn propagate_consist_unit_poses(vehicles: &mut [Vehicle], head_id: u32) {
    propagate_consist_unit_poses_with_map(vehicles, head_id, None);
}

/// Como [`propagate_consist_unit_poses`], reteniendo en depósito unidades con
/// `Track::Depot` (`!depot_leave_cleared` sobre `RailDepot`).
pub fn propagate_consist_unit_poses_with_map(
    vehicles: &mut [Vehicle],
    head_id: u32,
    map: Option<&Map>,
) {
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
            let hold_in_depot = !unit.depot_leave_cleared
                && map.is_some_and(|m| m.get_kind(unit.pos) == Some(TileKind::RailDepot));
            if hold_in_depot {
                unit.direction = head_dir;
                unit.curve_prev_direction = head_dir;
                unit.running = running;
                unit.path.clear();
                unit.orders.clear();
                unit.current_order = 0;
                continue;
            }
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

/// Invierte en reposo la orientación de todo el consist de forma atómica.
///
/// Las unidades intercambian sus poses de extremo a extremo y todas invierten
/// su rumbo en el mismo tick. Así la locomotora queda en el nuevo frente antes
/// de volver a moverse y nunca avanza a través de sus propios vagones.
pub fn reverse_consist_at_stop(vehicles: &mut [Vehicle], head_id: u32, _map: &Map) -> bool {
    let ids = consist_unit_ids(vehicles, head_id);
    let Some(head_index) = vehicles.iter().position(|v| v.id == head_id) else {
        return false;
    };
    let head = &vehicles[head_index];
    if head.kind != VehicleKind::Train || !head.is_consist_head() || head.cur_speed != 0 {
        return false;
    }
    let Some(next) = head.movement_target() else {
        return false;
    };
    let outbound = crate::vehicle::direction_from_tile_step(head.pos, next);
    if outbound != reverse_direction(head.direction) {
        return false;
    }
    let original_path = head.path.clone();

    let poses: Vec<_> = ids
        .iter()
        .filter_map(|&id| {
            vehicles.iter().find(|v| v.id == id).map(|unit| {
                (
                    unit.pos,
                    unit.rail_pixel,
                    unit.direction,
                    unit.curve_prev_direction,
                    unit.z_pos,
                )
            })
        })
        .collect();
    if poses.len() != ids.len() {
        return false;
    }

    for (index, id) in ids.iter().copied().enumerate() {
        let Some(unit) = vehicles.iter_mut().find(|v| v.id == id) else {
            continue;
        };
        let (pos, rail_pixel, old_enter, old_exit, z_pos) = poses[poses.len() - 1 - index];
        // Recorrer una pieza al revés intercambia entrada/salida además de
        // invertir ambos rumbos.
        unit.pos = pos;
        unit.z_pos = z_pos;
        unit.direction = reverse_direction(old_exit);
        unit.curve_prev_direction = reverse_direction(old_enter);
        unit.rail_pixel = 15_u8.saturating_sub(rail_pixel.min(15));
        unit.progress = 0;
        unit.depart_turn = 0;
        unit.cur_speed = 0;
        unit.subspeed = 0;
        unit.path.clear();
        unit.rail_tile_history.clear();
        unit.reserved_steps.clear();
    }

    let head = &mut vehicles[head_index];
    head.direction = outbound;
    head.curve_prev_direction = outbound;
    // Un tren unitario (o una formación muy corta dentro de la misma tesela)
    // puede seguir usando el path original. Tras intercambiar extremos en un
    // consist largo, el primer paso ya no es adyacente y se recalculará en el
    // tick siguiente.
    if original_path
        .front()
        .is_some_and(|next| head.pos.x.abs_diff(next.x) + head.pos.y.abs_diff(next.y) == 1)
    {
        head.path = original_path;
    }
    head.wait_counter = 0;
    head.pbs_stuck = false;
    head.no_network_route_to_order = false;
    true
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::{DIR_NE, DIR_SW};

    fn train_unit(id: u32, x: i32, rail_pixel: u8) -> Vehicle {
        let pos = TileCoord::new(x, 2);
        let mut unit = Vehicle::new(id, VehicleKind::Train, pos, pos);
        unit.direction = DIR_SW;
        unit.curve_prev_direction = DIR_SW;
        unit.rail_pixel = rail_pixel;
        unit
    }

    #[test]
    fn reversal_waits_for_stop_and_flips_the_whole_consist_atomically() {
        let map = Map::new_flat(12, 5, 0);
        let mut head = train_unit(1, 6, 3);
        head.next_unit = Some(2);
        head.path = VecDeque::from([TileCoord::new(5, 2)]);
        head.cur_speed = 1;
        let mut middle = train_unit(2, 7, 7);
        middle.prev_unit = Some(1);
        middle.next_unit = Some(3);
        let mut tail = train_unit(3, 8, 12);
        tail.prev_unit = Some(2);
        let mut vehicles = vec![head, middle, tail];

        assert!(!reverse_consist_at_stop(&mut vehicles, 1, &map));
        assert_eq!(vehicles[0].pos, TileCoord::new(6, 2));

        vehicles[0].cur_speed = 0;
        assert!(reverse_consist_at_stop(&mut vehicles, 1, &map));
        assert_eq!(vehicles[0].pos, TileCoord::new(8, 2));
        assert_eq!(vehicles[1].pos, TileCoord::new(7, 2));
        assert_eq!(vehicles[2].pos, TileCoord::new(6, 2));
        assert_eq!(vehicles[0].rail_pixel, 3);
        assert_eq!(vehicles[1].rail_pixel, 8);
        assert_eq!(vehicles[2].rail_pixel, 12);
        assert!(vehicles.iter().all(|unit| unit.direction == DIR_NE));
        assert!(vehicles.iter().all(|unit| unit.cur_speed == 0));
        assert!(vehicles.iter().all(|unit| unit.path.is_empty()));
    }
}
