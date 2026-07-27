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
    CargoPacket, CargoUnloadAction, StationCargoList, StationHopKey, TravelledVector,
    VehicleCargoList,
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
        assert!(list.stage(
            true,
            at,
            &[next],
            crate::vehicle::OrderUnloadType::UnloadIfPossible
        ));
        assert_eq!(list.staged_deliver, 2);
        assert_eq!(list.staged_transfer, 3);
        assert_eq!(list.staged_keep, 4);
    }

    #[test]
    fn forced_unload_delivers_when_accepted_and_transfers_otherwise() {
        use crate::vehicle::OrderUnloadType;

        let source = TileCoord::new(1, 1);
        let at = TileCoord::new(5, 5);
        let packet = CargoPacket::new(CargoType::Goods, 3, source)
            .with_first_station(source)
            .with_next_hop(Some(TileCoord::new(9, 9)));

        assert_eq!(
            choose_cargo_action(&packet, at, &[], OrderUnloadType::Unload, true),
            CargoUnloadAction::Deliver
        );
        assert_eq!(
            choose_cargo_action(&packet, at, &[], OrderUnloadType::Unload, false),
            CargoUnloadAction::Transfer
        );
        assert_eq!(
            choose_cargo_action(&packet, at, &[], OrderUnloadType::Transfer, true),
            CargoUnloadAction::Transfer
        );
        assert_eq!(
            choose_cargo_action(&packet, at, &[], OrderUnloadType::NoUnload, true),
            CargoUnloadAction::Keep
        );
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

    #[test]
    fn split_prorates_feeder_share() {
        let mut p = CargoPacket::new(CargoType::Goods, 10, TileCoord::new(0, 0));
        p.feeder_share = 100;
        let taken = p.split(4).unwrap();
        assert_eq!(taken.count, 4);
        assert_eq!(taken.feeder_share, 40);
        assert_eq!(p.count, 6);
        assert_eq!(p.feeder_share, 60);
    }

    #[test]
    fn get_distance_uses_travelled_capped_by_source_xy() {
        let mut p = CargoPacket::new(CargoType::Coal, 1, TileCoord::new(0, 0));
        p.update_loading_tile(TileCoord::new(10, 10));
        // En vehículo en (15,10): distancia Manhattan al load = 5.
        assert_eq!(p.get_distance(TileCoord::new(15, 10)), 5);
        // Cap por source_xy→destino: si viajamos en zigzag, no se paga de más.
        p.travelled.x = 10 + 100;
        p.travelled.y = 10;
        assert_eq!(p.get_distance(TileCoord::new(15, 10)), 5);
    }

    #[test]
    fn truncate_random_by_destination_preserves_other_cargo() {
        let mut list = StationCargoList::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        list.push(CargoPacket::new(CargoType::Coal, 20, a).with_next_hop(Some(b)));
        list.push(CargoPacket::new(CargoType::Goods, 5, a).with_next_hop(Some(b)));
        let mut rng = crate::cargodist::parity::Randomizer::new(42);
        let (moved, _) = list.truncate_cargo_amount(CargoType::Coal, 10, &mut rng);
        assert_eq!(moved, 10);
        assert_eq!(list.total_of(CargoType::Coal), 10);
        assert_eq!(list.total_of(CargoType::Goods), 5);
    }

    #[test]
    fn reroute_rewrites_next_hop() {
        let mut list = StationCargoList::default();
        let a = TileCoord::new(1, 1);
        let avoid = TileCoord::new(2, 2);
        let alt = TileCoord::new(3, 3);
        list.push(CargoPacket::new(CargoType::Coal, 7, a).with_next_hop(Some(avoid)));
        let moved = list.reroute(u32::MAX, avoid, Some(a), |_| Some(alt));
        assert_eq!(moved, 7);
        assert!(list.by_next_hop.contains_key(&StationHopKey(Some(alt))));
        assert!(!list.by_next_hop.contains_key(&StationHopKey(Some(avoid))));
    }
}
