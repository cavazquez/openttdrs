use openttdrs_core::{GameState, Station, TileCoord, TileKind, Vehicle, VehicleKind, find_path};

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
        state.stations.push(Station::new(pos));
    }
}

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

pub(crate) fn place_vehicles(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (i, (a, b)) in routes.into_iter().enumerate() {
        let kind = if i.is_multiple_of(2) {
            VehicleKind::Train
        } else {
            VehicleKind::Truck
        };
        let mut v = Vehicle::new(i as u32, kind, a, b);
        if let Some(path) = find_path(&state.map, a, b) {
            v.path = path.into_iter().collect();
        }
        state.vehicles.push(v);
    }
}

#[cfg(test)]
mod tests {
    use super::{place_roads, place_stations, place_vehicles};
    use openttdrs_core::{GameState, Industry, IndustryKind, TileCoord, TileKind, VehicleKind};

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
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert_eq!(state.vehicles[1].kind, VehicleKind::Truck);
    }
}
