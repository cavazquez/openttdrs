//! Packets de carga al estilo `OpenTTD` (`cargopacket.h`).
//!
//! Cada lote lleva origen y edad de tránsito; la estación y el vehículo
//! mantienen colas FIFO. Los balances agregados (`CargoStock` / `Vehicle.cargo`)
//! se sincronizan desde estas listas.
//!
//! `next_hop` es el slice de `CargoDist` (#49): Manual usa órdenes;
//! Asymmetric/Symmetric usan `FlowStat` (con fallback a órdenes).

mod operations;
mod types;

// Re-exports públicos para mantener API estable
pub use operations::{
    choose_cargo_action, decide_cargo_unload_action, load_unload_speed, prepare_unload,
};
pub use types::{
    CargoPacket, CargoUnloadAction, StationCargoList, StationHopKey, VehicleCargoList,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::CargoType;
    use crate::map::TileCoord;

    #[test]
    fn station_take_splits_packet_fifo() {
        let mut list = StationCargoList::default();
        let src = TileCoord::new(1, 2);
        list.add_amount(CargoType::Coal, 10, src);
        let taken = list.take(CargoType::Coal, 4);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].count, 4);
        assert_eq!(list.total_of(CargoType::Coal), 6);
    }

    #[test]
    fn vehicle_take_amount_preserves_source() {
        let mut list = VehicleCargoList::default();
        list.push(CargoPacket::new(CargoType::Goods, 5, TileCoord::new(0, 0)));
        list.push(CargoPacket::new(CargoType::Goods, 5, TileCoord::new(3, 3)));
        let taken = list.take_amount(7);
        assert_eq!(taken.iter().map(|p| p.count).sum::<u16>(), 7);
        assert_eq!(list.total(), 3);
        assert_eq!(taken[0].source, TileCoord::new(0, 0));
    }

    #[test]
    fn payment_days_follow_packet_age() {
        let mut p = CargoPacket::new(CargoType::Coal, 1, TileCoord::new(0, 0));
        p.periods_in_transit = 10;
        assert_eq!(p.periods_in_transit, 10);
        assert_eq!(load_unload_speed(CargoType::Coal), 4);
        assert_eq!(load_unload_speed(CargoType::Passengers), 8);
    }

    #[test]
    fn decide_unload_keeps_when_next_hop_elsewhere() {
        let at = TileCoord::new(5, 5);
        let elsewhere = TileCoord::new(9, 9);
        let p = CargoPacket::new(CargoType::Goods, 1, at).with_next_hop(Some(elsewhere));
        assert_eq!(
            decide_cargo_unload_action(&p, at, false),
            CargoUnloadAction::Keep
        );
        let p2 = p.clone().with_next_hop(Some(at));
        assert_eq!(
            decide_cargo_unload_action(&p2, at, false),
            CargoUnloadAction::Deliver
        );
        assert_eq!(
            decide_cargo_unload_action(&p2, at, true),
            CargoUnloadAction::Transfer
        );
    }

    #[test]
    fn station_cargo_list_indexes_by_next_hop_and_reserves() {
        let mut list = StationCargoList::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        list.push(CargoPacket::new(CargoType::Coal, 5, a).with_next_hop(Some(b)));
        list.push(CargoPacket::new(CargoType::Coal, 3, a).with_next_hop(None));
        assert_eq!(list.by_next_hop.len(), 2);
        assert_eq!(list.reserve(4), 4);
        assert_eq!(list.reserved, 4);
        list.consume_reserved(4);
        assert_eq!(list.reserved, 0);
    }

    #[test]
    fn stage_classifies_transfer_deliver_keep() {
        let at = TileCoord::new(5, 5);
        let next = TileCoord::new(9, 9);
        let elsewhere = TileCoord::new(12, 12);
        let mut list = VehicleCargoList::default();
        list.push(CargoPacket::new(CargoType::Goods, 2, at).with_next_hop(Some(at)));
        list.push(
            CargoPacket::new(CargoType::Goods, 3, at)
                .with_first_station(TileCoord::new(0, 0))
                .with_next_hop(Some(elsewhere)),
        );
        list.push(CargoPacket::new(CargoType::Goods, 4, at).with_next_hop(Some(next)));
        assert!(list.stage(true, at, &[next], false, false));
        assert_eq!(list.staged_deliver, 2);
        assert_eq!(list.staged_transfer, 3);
        assert_eq!(list.staged_keep, 4);
    }

    #[test]
    fn decide_unload_keeps_at_boarding_station_even_without_next_hop() {
        let stop = TileCoord::new(3, 3);
        let house = TileCoord::new(4, 3);
        let mut p = CargoPacket::new(CargoType::Passengers, 5, house);
        p.first_station = Some(stop);
        assert_eq!(
            decide_cargo_unload_action(&p, stop, false),
            CargoUnloadAction::Keep
        );
        let dest = TileCoord::new(12, 8);
        assert_eq!(
            decide_cargo_unload_action(&p, dest, false),
            CargoUnloadAction::Deliver
        );
    }
}
