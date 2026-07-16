use openttdrs_core::prelude::*;
use openttdrs_core::{PathNetwork, find_path};

pub(crate) fn place_stations(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    let positions: Vec<TileCoord> = state
        .industries
        .iter()
        .enumerate()
        .map(|(i, ind)| {
            let dy = if i % 2 == 0 { 3i32 } else { -3i32 };
            TileCoord::new(
                (ind.pos.x + 3).clamp(0, mw as i32 - 1),
                (ind.pos.y + dy).clamp(0, mh as i32 - 1),
            )
        })
        .collect();
    for pos in positions {
        let kind = state.map.get_kind(pos).unwrap_or(TileKind::Grass);
        if kind != TileKind::Water && kind != TileKind::Void {
            let _ = state.map.set_mapt_m5(pos, 0x50, 0);
            let _ = state.map.set_kind(pos, TileKind::Station);
            if let Some(mut t) = state.map.get(pos) {
                t.m6 = (t.m6 & !0x78) | (2 << 3); // StationType::Truck
                let _ = state.map.set_tile(pos, t);
            }
        }
        state.stations.push(Station::new_with_kind(
            pos,
            openttdrs_core::StopKind::TruckStop,
        ));
    }
}

#[allow(dead_code)] // reservado para bootstrap alternativo / tests
pub(crate) fn place_roads(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (from, to) in routes {
        let mut cur = from;
        while cur.x != to.x {
            cur.x += (to.x - cur.x).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
        while cur.y != to.y {
            cur.y += (to.y - cur.y).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn place_vehicles(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (i, (a, b)) in routes.into_iter().enumerate() {
        let kind = if i.is_multiple_of(2) {
            VehicleKind::Bus
        } else {
            VehicleKind::Truck
        };
        let mut v = Vehicle::new(i as u32, kind, a, b);
        if let Some(path) = find_path(&state.map, a, b, PathNetwork::Road) {
            v.path = path.into_iter().collect();
        }
        state.vehicles.push(v);
    }
}

#[cfg(test)]
mod tests {
    use super::{place_roads, place_stations, place_vehicles};
    use openttdrs_core::prelude::*;
    use openttdrs_core::{Industry, IndustryKind};

    #[test]
    fn place_stations_creates_one_station_per_industry() {
        let mut state = GameState::new(16, 16);
        state
            .industries
            .push(Industry::new(TileCoord::new(2, 2), IndustryKind::Forest));
        state.industries.push(Industry::new(
            TileCoord::new(10, 10),
            IndustryKind::CoalMine,
        ));

        place_stations(&mut state);
        assert_eq!(state.stations.len(), 2);
        for st in &state.stations {
            assert_eq!(
                state.map.get_kind(st.pos),
                Some(TileKind::Station),
                "demo: estación en mapa coincide con state.stations"
            );
        }
    }

    #[test]
    fn place_roads_marks_intermediate_tiles_as_road() {
        let mut state = GameState::new(16, 16);
        state
            .industries
            .push(Industry::new(TileCoord::new(1, 1), IndustryKind::Forest));
        state
            .stations
            .push(openttdrs_core::Station::new(TileCoord::new(4, 1)));

        place_roads(&mut state);

        assert_eq!(
            state.map.get_kind(TileCoord::new(2, 1)),
            Some(TileKind::Road)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(3, 1)),
            Some(TileKind::Road)
        );
        // Endpoint industry/station no se fuerzan a Road.
        assert_ne!(
            state.map.get_kind(TileCoord::new(1, 1)),
            Some(TileKind::Road)
        );
        assert_ne!(
            state.map.get_kind(TileCoord::new(4, 1)),
            Some(TileKind::Road)
        );
    }

    #[test]
    fn place_vehicles_assigns_alternating_kinds() {
        let mut state = GameState::new(16, 16);
        state
            .industries
            .push(Industry::new(TileCoord::new(1, 1), IndustryKind::Forest));
        state
            .industries
            .push(Industry::new(TileCoord::new(8, 8), IndustryKind::CoalMine));
        state
            .stations
            .push(openttdrs_core::Station::new(TileCoord::new(4, 1)));
        state
            .stations
            .push(openttdrs_core::Station::new(TileCoord::new(10, 8)));

        place_vehicles(&mut state);
        assert_eq!(state.vehicles.len(), 2);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Bus);
        assert_eq!(state.vehicles[1].kind, VehicleKind::Truck);
    }
}
