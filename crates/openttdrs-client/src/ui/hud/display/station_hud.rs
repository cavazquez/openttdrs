use openttdrs_core::prelude::*;
use openttdrs_core::{STATION_COVERAGE_RADIUS, station_coverage_at};

use crate::i18n::Locale;
use crate::sprites::{StationTileClass, station_type_from_m6};
use crate::state::SimWorld;

fn stop_kind_label(locale: Locale, class: StationTileClass) -> String {
    let es = locale == Locale::Es;
    match class {
        StationTileClass::Bus if es => "parada bus".into(),
        StationTileClass::Bus => "bus stop".into(),
        StationTileClass::Truck if es => "parada camión".into(),
        StationTileClass::Truck => "truck stop".into(),
        StationTileClass::Rail if es => "estación tren".into(),
        StationTileClass::Rail => "train station".into(),
        StationTileClass::RailWaypoint => "rail waypoint".into(),
        StationTileClass::RoadWaypoint => "road waypoint".into(),
        StationTileClass::Airport if es => "aeropuerto".into(),
        StationTileClass::Airport => "airport".into(),
        StationTileClass::Oilrig if es => "plataforma petrolera".into(),
        StationTileClass::Oilrig => "oil rig".into(),
        StationTileClass::Dock if es => "muelle".into(),
        StationTileClass::Dock => "dock".into(),
        StationTileClass::Buoy if es => "boya".into(),
        StationTileClass::Buoy => "buoy".into(),
        StationTileClass::Other(station_type) => {
            if es {
                format!("⚠ estación sin renderer (tipo m6 {station_type})")
            } else {
                format!("⚠ station without renderer (m6 type {station_type})")
            }
        }
    }
}

/// Texto dinámico de una estación en el HUD, localizado sin tocar nombres ni
/// datos que provengan de la partida.
pub(crate) fn localized_station_details_text(
    locale: Locale,
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
        .map(|station| {
            if locale == Locale::Es {
                format!("stock:{} ingresos:${}", station.stock, station.income)
            } else {
                format!("stock:{} income:${}", station.stock, station.income)
            }
        })
        .unwrap_or_else(|| {
            if locale == Locale::Es {
                "stock:n/d income:n/d".to_string()
            } else {
                "stock:n/a income:n/a".to_string()
            }
        });
    if locale == Locale::Es {
        format!(
            "\n{} · {station_line}\nCarga/descarga junto a la vía · cobertura r{}\nAcepta mail:{} goods:{} · suministra coal:{} wood:{} oil:{} source stock:{}",
            stop_kind_label(locale, class),
            STATION_COVERAGE_RADIUS,
            coverage.accepts_mail,
            coverage.accepts_goods,
            coverage.supplies_coal,
            coverage.supplies_wood,
            coverage.supplies_oil,
            coverage.supplied_stock
        )
    } else {
        format!(
            "\n{} · {station_line}\nLoad/unload beside rail · coverage r{}\nAccepts mail:{} goods:{} · supplies coal:{} wood:{} oil:{} source stock:{}",
            stop_kind_label(locale, class),
            STATION_COVERAGE_RADIUS,
            coverage.accepts_mail,
            coverage.accepts_goods,
            coverage.supplies_coal,
            coverage.supplies_wood,
            coverage.supplies_oil,
            coverage.supplied_stock
        )
    }
}

pub(crate) fn localized_road_depot_tile_details(locale: Locale, m5: u8) -> String {
    const DIR: [&str; 4] = ["NE", "SE", "SW", "NW"];
    let dir = (m5 & 0x03).min(3) as usize;
    if locale == Locale::Es {
        format!(
            "\nDepósito carretera orient. {} · panel: comprar bus/camión\nNo es parada de carga (usa parada camión en hierba junto a la vía)",
            DIR[dir]
        )
    } else {
        format!(
            "\nRoad depot facing {} · panel: buy bus/truck\nNot a cargo stop (use a truck stop on grass beside the road)",
            DIR[dir]
        )
    }
}

pub(crate) fn localized_rail_depot_tile_details(locale: Locale, m5: u8) -> String {
    const DIR: [&str; 4] = ["NE", "SE", "SW", "NW"];
    let dir = (m5 & 0x03).min(3) as usize;
    if locale == Locale::Es {
        format!(
            "\nDepósito vía orient. {} · panel: comprar tren\nNo es estación de carga",
            DIR[dir]
        )
    } else {
        format!(
            "\nRail depot facing {} · panel: buy train\nNot a cargo station",
            DIR[dir]
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        localized_road_depot_tile_details, localized_station_details_text, stop_kind_label,
    };
    use crate::i18n::Locale;
    use openttdrs_core::prelude::*;
    use openttdrs_core::{Industry, IndustryKind};

    use crate::sprites::StationTileClass;
    use crate::state::SimWorld;

    #[test]
    fn road_depot_details_mention_not_a_load_station() {
        let text = localized_road_depot_tile_details(Locale::Es, 2);
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
        let text = localized_station_details_text(Locale::Es, &sim, pos, &tile);
        assert!(text.contains("parada camión"));
        assert!(text.contains("Carga/descarga"));
    }

    #[test]
    fn unknown_station_type_is_labeled_as_a_warning() {
        assert!(stop_kind_label(Locale::Es, StationTileClass::Other(4)).contains("sin renderer"));
        assert!(stop_kind_label(Locale::Es, StationTileClass::Other(4)).contains("m6 4"));
    }

    #[test]
    fn station_and_depot_details_have_an_english_runtime_variant() {
        let sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let pos = TileCoord::new(2, 2);
        let tile = sim.state.map.get(pos).unwrap();
        let station = localized_station_details_text(Locale::En, &sim, pos, &tile);
        assert!(station.contains("Load/unload"));
        let depot = localized_road_depot_tile_details(Locale::En, 2);
        assert!(depot.contains("Road depot facing SW"));
    }
}
