//! Motor FTA de aeropuerto (Country … International).
//!
//! Port parcial de `AirportMovingData` + `AirportFTA` (`table/airport_movement.h`).

mod city;
mod commuter;
mod country;
mod helidepot;
mod international;
mod metropolitan;
mod profile;
mod tick;
mod types;

pub use city::{CITY_ENTRIES, CITY_MOVING_DATA, CITY_NOF_ELEMENTS, city_fta_edges};
pub use commuter::{
    COMMUTER_ENTRIES, COMMUTER_MOVING_DATA, COMMUTER_NOF_ELEMENTS, commuter_fta_edges,
};
pub use country::{COUNTRY_ENTRIES, COUNTRY_MOVING_DATA, COUNTRY_NOF_ELEMENTS, country_fta_edges};
pub use helidepot::{
    HELIDEPOT_ENTRIES, HELIDEPOT_MOVING_DATA, HELIDEPOT_NOF_ELEMENTS, helidepot_fta_edges,
};
pub use international::{
    INTERNATIONAL_ENTRIES, INTERNATIONAL_MOVING_DATA, INTERNATIONAL_NOF_ELEMENTS,
    international_fta_edges,
};
pub use metropolitan::{
    METROPOLITAN_ENTRIES, METROPOLITAN_MOVING_DATA, METROPOLITAN_NOF_ELEMENTS,
    metropolitan_fta_edges,
};
pub use profile::fta_profile_for_spec;
pub use tick::{
    airport_nw_origin, init_airport_fta_on_purchase, init_country_fta_on_purchase,
    station_uses_airport_fta, station_uses_country_fta, tick_airport_fta, tick_country_airport_fta,
};
pub use types::{
    AirportBlockBits, AirportFtaEdge, AirportFtaKind, AirportFtaProfile, AirportHeading,
    AirportMovingData, AirportMovingDataFlags, BLOCK_HANGAR2_AREA, BLOCK_HELIPAD1, BLOCK_HELIPAD2,
    BLOCK_PRE_HELIPAD, BLOCK_TERM3, FLAG_BRAKE, FLAG_EXACT, FLAG_HELI_LOWER, FLAG_HELI_RAISE,
    FLAG_HOLD, FLAG_LAND, FLAG_NO_SPEED_CLAMP, FLAG_SLOW_TURN, FLAG_TAKEOFF,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::engine::{ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_TRICARIO};
    use crate::sim_events::SimEvent;
    use crate::vehicle::{AircraftPhase, VehicleOrder};
    use crate::{AirportSpecId, Command, GameState, TileCoord, apply_command};

    #[test]
    fn country_tables_have_expected_size() {
        assert_eq!(COUNTRY_MOVING_DATA.len(), 22);
        assert_eq!(COUNTRY_NOF_ELEMENTS, 22);
        assert_eq!(COUNTRY_ENTRIES.len(), 4);
        assert!(COUNTRY_MOVING_DATA[9].flags & FLAG_TAKEOFF != 0);
        assert!(COUNTRY_MOVING_DATA[11].flags & FLAG_LAND != 0);
        assert!(COUNTRY_MOVING_DATA[12].flags & FLAG_BRAKE != 0);
    }

    #[test]
    fn country_fta_hangar_leads_outside() {
        let edges = country_fta_edges(0);
        assert!(!edges.is_empty());
        assert_eq!(edges[0].next_position, 1);
        assert_eq!(edges[0].heading, AirportHeading::Hangar);
    }

    #[test]
    fn helidepot_tables_have_expected_size() {
        assert_eq!(HELIDEPOT_MOVING_DATA.len(), 18);
        assert_eq!(HELIDEPOT_NOF_ELEMENTS, 18);
        assert_eq!(HELIDEPOT_ENTRIES, [4, 4, 4, 4]);
        assert!(HELIDEPOT_MOVING_DATA[11].flags & FLAG_HELI_RAISE != 0);
        assert!(HELIDEPOT_MOVING_DATA[10].flags & FLAG_HELI_LOWER != 0);
        let from_pad = helidepot_fta_edges(14);
        assert!(
            from_pad
                .iter()
                .any(|e| e.heading == AirportHeading::HeliTakeoff && e.next_position == 17)
        );
    }

    #[test]
    fn country_airport_cycle_hangar_takeoff_fly_land_term() {
        let mut s = GameState::new(48, 48);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(20, 20),
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
        assert_eq!(s.stations.len(), 2);
        assert!(station_uses_country_fta(&s.stations[0]));
        assert!(station_uses_country_fta(&s.stations[1]));

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);
        assert_eq!(s.vehicles[0].airport_pos, 0);
        assert_eq!(s.vehicles[0].aircraft_phase, AircraftPhase::InHangar);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_term = false;

        for _ in 0..12_000 {
            s.step();
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing
                && (matches!(v.airport_pos, 2 | 3)
                    || (matches!(
                        v.aircraft_phase,
                        AircraftPhase::Taxi | AircraftPhase::InHangar
                    ) && s.stations[1].covers_tile(v.pos)))
            {
                saw_term = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_term {
                break;
            }
        }

        assert!(saw_takeoff, "debe emitir takeoff FTA");
        assert!(saw_flying, "debe entrar en crucero Flying");
        assert!(saw_landing, "debe emitir/aterrizar Landing");
        assert!(saw_term, "debe llegar a terminal/hangar del destino");
    }

    #[test]
    fn helidepot_cycle_hangar_takeoff_fly_land_pad() {
        let mut s = GameState::new(48, 48);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Helidepot,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(20, 20),
                axis_y: false,
                spec: AirportSpecId::Helidepot,
            },
        )
        .unwrap();
        assert!(station_uses_airport_fta(&s.stations[0]));
        assert_eq!(s.stations[0].airport_spec, AirportSpecId::Helidepot);
        assert_eq!(s.stations[0].airport_tiles.len(), 4);

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_TRICARIO),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_pad = false;

        for _ in 0..12_000 {
            s.step();
            if s.vehicles.is_empty() {
                break;
            }
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing && v.airport_pos == 14 && s.stations[1].covers_tile(v.pos) {
                saw_pad = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_pad {
                break;
            }
        }

        assert!(saw_takeoff, "Helidepot: takeoff heli");
        assert!(saw_flying, "Helidepot: crucero");
        assert!(saw_landing, "Helidepot: landing/lower");
        assert!(saw_pad, "Helidepot: llegar a helipad1 (pos 14)");
    }

    #[test]
    fn helidepot_pad_block_blocks_second_reservation() {
        let mut st =
            crate::Station::new_with_kind(TileCoord::new(0, 0), crate::station::StopKind::Airport);
        st.airport_spec = AirportSpecId::Helidepot;
        st.airport_tiles = vec![
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
            TileCoord::new(0, 1),
            TileCoord::new(1, 1),
        ];
        st.airport_blocks = BLOCK_HELIPAD1;
        let edges = helidepot_fta_edges(1);
        let pad_edge = edges
            .iter()
            .find(|e| e.heading == AirportHeading::Helipad1)
            .expect("edge HELIPAD1");
        assert_ne!(pad_edge.blocks & BLOCK_HELIPAD1, 0);
        assert_ne!(st.airport_blocks & pad_edge.blocks, 0);
    }

    #[test]
    fn commuter_tables_have_expected_size() {
        assert_eq!(COMMUTER_MOVING_DATA.len(), 38);
        assert_eq!(COMMUTER_NOF_ELEMENTS, 38);
        assert_eq!(COMMUTER_ENTRIES, [22, 21, 24, 23]);
        assert!(COMMUTER_MOVING_DATA[15].flags & FLAG_TAKEOFF != 0);
        assert!(COMMUTER_MOVING_DATA[17].flags & FLAG_LAND != 0);
        assert!(COMMUTER_MOVING_DATA[6].flags & FLAG_EXACT != 0);
        let from_hangar = commuter_fta_edges(1);
        assert!(
            from_hangar
                .iter()
                .any(|e| e.heading == AirportHeading::Takeoff && e.next_position == 11)
        );
        assert!(
            from_hangar
                .iter()
                .any(|e| e.heading == AirportHeading::Helipad1)
        );
    }

    #[test]
    fn commuter_cycle_hangar_takeoff_fly_land_term() {
        let mut s = GameState::new(56, 56);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Commuter,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(24, 24),
                axis_y: false,
                spec: AirportSpecId::Commuter,
            },
        )
        .unwrap();
        assert!(station_uses_airport_fta(&s.stations[0]));
        assert_eq!(s.stations[0].airport_tiles.len(), 20);

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_term = false;

        for _ in 0..15_000 {
            s.step();
            if s.vehicles.is_empty() {
                break;
            }
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing
                && (matches!(v.airport_pos, 3..=5)
                    || (matches!(
                        v.aircraft_phase,
                        AircraftPhase::Taxi | AircraftPhase::InHangar
                    ) && s.stations[1].covers_tile(v.pos)))
            {
                saw_term = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_term {
                break;
            }
        }

        assert!(saw_takeoff, "Commuter: takeoff");
        assert!(saw_flying, "Commuter: crucero");
        assert!(saw_landing, "Commuter: landing");
        assert!(saw_term, "Commuter: terminal/hangar destino");
    }

    #[test]
    fn city_tables_have_expected_size() {
        assert_eq!(CITY_MOVING_DATA.len(), 30);
        assert_eq!(CITY_NOF_ELEMENTS, 30);
        assert_eq!(CITY_ENTRIES, [26, 29, 27, 28]);
        assert!(CITY_MOVING_DATA[12].flags & FLAG_TAKEOFF != 0);
        assert!(CITY_MOVING_DATA[14].flags & FLAG_LAND != 0);
        let from_center = city_fta_edges(7);
        assert!(
            from_center
                .iter()
                .any(|e| e.heading == AirportHeading::Takeoff && e.next_position == 8)
        );
        assert!(
            from_center
                .iter()
                .any(|e| e.heading == AirportHeading::Term1 && e.next_position == 2)
        );
    }

    #[test]
    fn city_cycle_hangar_takeoff_fly_land_term() {
        let mut s = GameState::new(64, 64);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::City,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(28, 28),
                axis_y: false,
                spec: AirportSpecId::City,
            },
        )
        .unwrap();
        assert!(station_uses_airport_fta(&s.stations[0]));
        assert_eq!(s.stations[0].airport_tiles.len(), 36);

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_term = false;

        for _ in 0..18_000 {
            s.step();
            if s.vehicles.is_empty() {
                break;
            }
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing
                && (matches!(v.airport_pos, 2..=4)
                    || (matches!(
                        v.aircraft_phase,
                        AircraftPhase::Taxi | AircraftPhase::InHangar
                    ) && s.stations[1].covers_tile(v.pos)))
            {
                saw_term = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_term {
                break;
            }
        }

        assert!(saw_takeoff, "City: takeoff");
        assert!(saw_flying, "City: crucero");
        assert!(saw_landing, "City: landing");
        assert!(saw_term, "City: terminal/hangar destino");
    }

    #[test]
    fn metropolitan_tables_have_expected_size() {
        assert_eq!(METROPOLITAN_MOVING_DATA.len(), 28);
        assert_eq!(METROPOLITAN_NOF_ELEMENTS, 28);
        assert_eq!(METROPOLITAN_ENTRIES, [20, 19, 22, 21]);
        assert!(METROPOLITAN_MOVING_DATA[12].flags & FLAG_TAKEOFF != 0);
        assert!(METROPOLITAN_MOVING_DATA[14].flags & FLAG_LAND != 0);
        let from_center = metropolitan_fta_edges(7);
        assert!(
            from_center
                .iter()
                .any(|e| e.heading == AirportHeading::Takeoff && e.next_position == 8)
        );
        assert!(
            from_center
                .iter()
                .any(|e| e.heading == AirportHeading::Term1 && e.next_position == 2)
        );
        let from_approach = metropolitan_fta_edges(13);
        assert!(
            from_approach
                .iter()
                .any(|e| e.heading == AirportHeading::Landing && e.next_position == 14)
        );
    }

    #[test]
    fn metropolitan_cycle_hangar_takeoff_fly_land_term() {
        let mut s = GameState::new(64, 64);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Metropolitan,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(28, 28),
                axis_y: false,
                spec: AirportSpecId::Metropolitan,
            },
        )
        .unwrap();
        assert!(station_uses_airport_fta(&s.stations[0]));
        assert_eq!(s.stations[0].airport_tiles.len(), 36);

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_term = false;

        for _ in 0..18_000 {
            s.step();
            if s.vehicles.is_empty() {
                break;
            }
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing
                && (matches!(v.airport_pos, 2..=4)
                    || (matches!(
                        v.aircraft_phase,
                        AircraftPhase::Taxi | AircraftPhase::InHangar
                    ) && s.stations[1].covers_tile(v.pos)))
            {
                saw_term = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_term {
                break;
            }
        }

        assert!(saw_takeoff, "Metropolitan: takeoff");
        assert!(saw_flying, "Metropolitan: crucero");
        assert!(saw_landing, "Metropolitan: landing");
        assert!(saw_term, "Metropolitan: terminal/hangar destino");
    }

    #[test]
    fn international_tables_have_expected_size() {
        assert_eq!(INTERNATIONAL_MOVING_DATA.len(), 53);
        assert_eq!(INTERNATIONAL_NOF_ELEMENTS, 53);
        assert_eq!(INTERNATIONAL_ENTRIES, [38, 37, 40, 39]);
        assert!(INTERNATIONAL_MOVING_DATA[31].flags & FLAG_TAKEOFF != 0);
        assert!(INTERNATIONAL_MOVING_DATA[33].flags & FLAG_LAND != 0);
        let from_term_group = international_fta_edges(25);
        assert!(
            from_term_group
                .iter()
                .any(|e| e.heading == AirportHeading::Takeoff && e.next_position == 26)
        );
        assert!(
            from_term_group
                .iter()
                .any(|e| e.heading == AirportHeading::Term3 && e.next_position == 6)
        );
        let from_approach = international_fta_edges(32);
        assert!(
            from_approach
                .iter()
                .any(|e| e.heading == AirportHeading::Landing && e.next_position == 33)
        );
    }

    #[test]
    fn international_cycle_hangar_takeoff_fly_land_term() {
        let mut s = GameState::new(64, 64);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::International,
            },
        )
        .unwrap();
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(20, 20),
                axis_y: false,
                spec: AirportSpecId::International,
            },
        )
        .unwrap();
        assert!(station_uses_airport_fta(&s.stations[0]));
        assert_eq!(s.stations[0].airport_tiles.len(), 49);

        let hangar_a = s.stations[0].pos;
        let hangar_b = s.stations[1].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar_a, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        assert!(s.vehicles[0].airport_fta_active);

        apply_command(
            &mut s,
            &Command::SetVehicleOrderList(
                id,
                vec![
                    VehicleOrder::station(hangar_b),
                    VehicleOrder::station(hangar_a),
                ],
            ),
        )
        .unwrap();
        apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();

        let mut saw_takeoff = false;
        let mut saw_flying = false;
        let mut saw_landing = false;
        let mut saw_term = false;

        for _ in 0..24_000 {
            s.step();
            if s.vehicles.is_empty() {
                break;
            }
            let events = s.runtime.pending_sim_events.drain();
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftTakeoff { .. }))
            {
                saw_takeoff = true;
            }
            if events
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftLanding { .. }))
            {
                saw_landing = true;
            }
            let v = &s.vehicles[0];
            if v.aircraft_phase == AircraftPhase::Flying {
                saw_flying = true;
            }
            if saw_landing
                && (matches!(v.airport_pos, 4..=9)
                    || (matches!(
                        v.aircraft_phase,
                        AircraftPhase::Taxi | AircraftPhase::InHangar
                    ) && s.stations[1].covers_tile(v.pos)))
            {
                saw_term = true;
            }
            if saw_takeoff && saw_flying && saw_landing && saw_term {
                break;
            }
        }

        assert!(saw_takeoff, "International: takeoff");
        assert!(saw_flying, "International: crucero");
        assert!(saw_landing, "International: landing");
        assert!(saw_term, "International: terminal/hangar destino");
    }
}
