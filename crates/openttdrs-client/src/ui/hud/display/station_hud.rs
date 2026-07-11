use openttdrs_core::{STATION_COVERAGE_RADIUS, Tile, station_coverage_at};

use crate::sprites::{StationTileClass, station_type_from_m6};
use crate::state::SimWorld;

fn stop_kind_label(class: StationTileClass) -> &'static str {
    match class {
        StationTileClass::Bus => "parada bus",
        StationTileClass::Truck => "parada camión",
        StationTileClass::Rail => "estación tren",
        StationTileClass::RailWaypoint => "waypoint",
        StationTileClass::RoadWaypoint => "waypoint road",
        StationTileClass::Airport => "aeropuerto",
        StationTileClass::Dock => "muelle",
        StationTileClass::Buoy => "boya",
        StationTileClass::Other(_) => "estación",
    }
}

pub(crate) fn station_details_text(
    sim: &SimWorld,
    pos: openttdrs_core::TileCoord,
    tile: &Tile,
) -> String {
    let class = station_type_from_m6(tile.m6);
    let coverage = station_coverage_at(
        &sim.state.map,
        &sim.state.industries,
        pos,
        STATION_COVERAGE_RADIUS,
    );
    let station_line = sim
        .state
        .stations
        .iter()
        .find(|station| station.pos == pos)
        .map(|station| format!("stock:{} ingresos:${}", station.stock, station.income))
        .unwrap_or_else(|| "stock:n/d income:n/d".to_string());
    format!(
        "\n{} · {station_line}\nCarga/descarga junto a la vía · cobertura r{}\nAcepta mail:{} goods:{} · suministra coal:{} wood:{} oil:{} source stock:{}",
        stop_kind_label(class),
        STATION_COVERAGE_RADIUS,
        coverage.accepts_mail,
        coverage.accepts_goods,
        coverage.supplies_coal,
        coverage.supplies_wood,
        coverage.supplies_oil,
        coverage.supplied_stock
    )
}

pub(crate) fn road_depot_tile_details(m5: u8) -> String {
    const DIR: [&str; 4] = ["NE", "SE", "SW", "NW"];
    let dir = (m5 & 0x03).min(3) as usize;
    format!(
        "\nDepósito carretera orient. {} · panel: comprar bus/camión\nNo es parada de carga (usa parada camión en hierba junto a la vía)",
        DIR[dir]
    )
}

pub(crate) fn rail_depot_tile_details(m5: u8) -> String {
    const DIR: [&str; 4] = ["NE", "SE", "SW", "NW"];
    let dir = (m5 & 0x03).min(3) as usize;
    format!(
        "\nDepósito vía orient. {} · panel: comprar tren\nNo es estación de carga",
        DIR[dir]
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{road_depot_tile_details, station_details_text};
    use openttdrs_core::{GameState, Industry, IndustryKind, Station, TileCoord, TileKind};

    use crate::state::SimWorld;

    #[test]
    fn road_depot_details_mention_not_a_load_station() {
        let text = road_depot_tile_details(2);
        assert!(text.contains("Depósito carretera"));
        assert!(text.contains("SW"));
        assert!(text.contains("No es parada"));
    }

    #[test]
    fn station_details_text_includes_stop_kind() {
        let mut sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let pos = TileCoord::new(2, 2);
        sim.state.map.set_kind(pos, TileKind::Station).unwrap();
        let mut tile = sim.state.map.get(pos).unwrap();
        tile.m6 = 2 << 3; // truck stop
        sim.state.map.set_tile(pos, tile).unwrap();
        sim.state.stations.push(Station::new(pos));
        sim.state
            .industries
            .push(Industry::new(TileCoord::new(3, 2), IndustryKind::CoalMine));

        let tile = sim.state.map.get(pos).unwrap();
        let text = station_details_text(&sim, pos, &tile);
        assert!(text.contains("parada camión"));
        assert!(text.contains("Carga/descarga"));
    }
}
