//! Sub-tesela de vehículos en carretera/vía (`table/roadveh_movement.h`).

pub mod bay;
pub mod controller;
mod curves;
pub mod depot;
pub mod drive_data;
pub mod pose;
mod render_pose;
pub mod rvsb;
pub mod traffic;

// Re-exportar tipos y funciones públicas principales
pub use bay::{BayStationTable, bay_station_table, parked_inside_bay};
pub use controller::{
    individual_road_vehicle_controller, road_vehicle_step_solo, road_vehicle_tick,
};
pub use curves::{straight_subtile, train_straight_subtile, turn_curve_points};
pub use depot::{
    ROAD_DEPOT_ENTRY_STOP, ROAD_DEPOT_EXIT_START, ROAD_DEPOT_PROGRESS_STEP, road_depot_direction,
    road_depot_entry_direction, road_depot_exit_direction, road_depot_subtile,
};
pub use drive_data::{RDE_NEXT_TILE, RDE_TURNED, RoadDriveEntry, road_drive_entry};
pub use pose::{
    VehiclePose, extrapolate_vehicle_pose, retreat_vehicle_pose, vehicle_render_progress,
};
pub use render_pose::{
    road_turn_entry_exit, train_subtile_direction, vehicle_render_direction,
    vehicle_render_direction_at, vehicle_render_direction_at_with_map, vehicle_subtile,
    vehicle_subtile_at, vehicle_subtile_at_with_map, vehicle_subtile_with_progress,
};
pub use rvsb::{RVSB_IN_DEPOT, RVSB_TRACKDIR_MASK, trackdir_from_direction};
pub use traffic::{BLOCKED_CTR_LIMIT, apply_road_veh_close_to, road_veh_find_close_to};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::VecDeque;

    use crate::map::TileCoord;
    use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleKind};

    use super::*;

    fn ne_to_se_turn_vehicle() -> Vehicle {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(0, 2),
        );
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v
    }

    #[test]
    fn detects_ne_to_se_turn() {
        let v = ne_to_se_turn_vehicle();
        assert_eq!(road_turn_entry_exit(&v), Some((DIR_NE, DIR_SE)));
    }

    #[test]
    fn turn_curve_endpoints_match_openrtd_data() {
        let v = ne_to_se_turn_vehicle();
        let (entry, exit) = road_turn_entry_exit(&v).unwrap();
        let curve = turn_curve_points(entry, exit).unwrap();
        let start = curves::sample_curve(curve, 0.0);
        let end = curves::sample_curve(curve, 255.0);
        assert_eq!(start, curve[0]);
        assert_eq!(end, curve[curve.len() - 1]);
    }

    #[test]
    fn straight_tile_uses_movement_direction_not_cardinal_sprite() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        v.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ]);
        v.progress = 200;
        assert!(road_turn_entry_exit(&v).is_none());
        let (x, y) = vehicle_subtile(&v);
        let (sx, sy) = straight_subtile(DIR_SW, 200.0);
        assert_eq!((x, y), (sx, sy));
    }

    #[test]
    fn train_uses_center_track_not_road_lanes() {
        const TRAIN_TRACK_CENTER: f32 = 8.0;
        let (tx, ty) = train_straight_subtile(DIR_SW, 128.0);
        let (rx, ry) = straight_subtile(DIR_SW, 128.0);
        assert!(
            (ty - TRAIN_TRACK_CENTER).abs() < 0.1,
            "eje horizontal por el centro de la vía"
        );
        assert!((ty - ry).abs() > 0.5, "no usa carril de carretera (y={ry})");
        assert!(tx > 0.0 && tx < 15.0, "avance a lo largo de x (tx={tx})");
        let _ = rx;
    }

    #[test]
    fn parked_at_station_uses_inbound_lane_end() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(15, 3),
            TileCoord::new(15, 3),
        );
        v.direction = DIR_NW;
        v.progress = 255;
        let parked = vehicle_subtile_with_progress(&v, 255);
        let inbound_end = straight_subtile(DIR_NW, 255.0);
        assert_eq!(parked, inbound_end);
    }

    #[test]
    fn extrapolate_crosses_tile_between_sim_ticks() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(5, 6),
            TileCoord::new(6, 6),
        );
        v.path = VecDeque::from([TileCoord::new(6, 6)]);
        v.set_cruise_speed();
        v.progress = 230;
        let pose = extrapolate_vehicle_pose(&v, 1.0);
        assert_eq!(
            pose.pos,
            TileCoord::new(6, 6),
            "extrapolación debe cruzar la tesela como haría el siguiente tick"
        );
        assert!(pose.progress < v.progress_step());
    }

    /// Camión con orden en la bahía donde está parado (entró rumbo NW).
    fn parked_in_bay_vehicle() -> Vehicle {
        let bay = TileCoord::new(4, 5);
        let mut v = Vehicle::new(0, VehicleKind::Truck, bay, bay);
        v.direction = DIR_NW;
        v.progress = 255;
        v.set_station_orders(vec![bay, TileCoord::new(10, 5)]);
        v.progress = 255;
        v
    }

    #[test]
    fn parked_in_bay_sits_on_stop_frame_of_rv_station_table() {
        let v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "detenido en el stop frame de `_rv_station_left_se_far` (9,5)"
        );
    }

    #[test]
    fn bay_entry_follows_rv_station_table_from_mouth_to_stop() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        v.progress = 0;
        assert_eq!(vehicle_subtile(&v), table.points[0], "entra por la boca");
        v.progress = 255;
        assert_eq!(vehicle_subtile(&v), table.points[table.stop]);
    }

    #[test]
    fn bay_exit_retraces_loop_back_to_mouth() {
        let mut v = parked_in_bay_vehicle();
        let table = bay_station_table(DIR_NW, true).unwrap();
        // Tras el giro: rumbo de salida SE hacia la carretera de acceso.
        v.direction = DIR_SE;
        v.path = VecDeque::from([TileCoord::new(4, 6)]);
        v.progress = 0;
        assert_eq!(
            vehicle_subtile(&v),
            table.points[table.stop],
            "la salida arranca en el punto de parada"
        );
        v.progress = 255;
        assert_eq!(
            vehicle_subtile(&v),
            *table.points.last().unwrap(),
            "y termina en la boca (5,15)"
        );
    }

    #[test]
    fn bay_sprite_direction_follows_loop_not_logical_heading() {
        let mut v = parked_in_bay_vehicle();
        // Mitad de la entrada SE-far: tramo transversal del lazo (x decrece →
        // componente NE), distinto del rumbo lógico NW de entrada.
        v.progress = 40;
        let pose = VehiclePose::from_vehicle(&v);
        let dir = render_pose::vehicle_render_direction_at(&v, pose);
        assert_ne!(dir, DIR_SE, "no debe usar el rumbo de salida");
        let table = bay_station_table(DIR_NW, true).unwrap();
        let (x0, _) = table.points[0];
        let (x1, _) = table.points[7];
        assert!(x1 < x0, "el tramo inicial del lazo se mueve hacia -x (NE)");
    }

    #[test]
    fn turn_midpoint_differs_from_tile_center() {
        let mut v = ne_to_se_turn_vehicle();
        v.progress = 128;
        let (x, y) = vehicle_subtile(&v);
        // Punto medio de la curva NE→SE (~(7,8)), no centro del rombo.
        assert!(x > 5.0 && x < 10.0);
        assert!(y > 6.0 && y < 11.0);
    }
}
