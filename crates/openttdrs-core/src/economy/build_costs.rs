//! Costos de construcción y modificación de terreno (`GetPrice`).

use super::global::GlobalEconomy;
use super::pricebase::{PriceIndex, get_price};
use crate::StopKind;
use crate::object_spec::OWNED_LAND_COST_FACTOR;

/// Coste de terraform por esquina modificada (`Price::Terraform`).
#[must_use]
pub fn terraform_cost_per_corner(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::Terraform, 1, 0)
}

/// Coste por tesela de terreno comprado (`Price::BuildObject`, factor vanilla 10).
#[must_use]
pub fn buy_land_cost(ge: &GlobalEconomy) -> i64 {
    build_object_cost_factored(ge, OWNED_LAND_COST_FACTOR, 1)
}

/// Coste de colocar faro o transmisor (`Price::BuildObject`, factor 1, 1 tesela).
#[must_use]
pub fn build_object_cost(ge: &GlobalEconomy) -> i64 {
    build_object_cost_factored(ge, 1, 1)
}

/// Coste de objeto con factor Action0 `0x0D` y número de teselas del footprint.
#[must_use]
pub fn build_object_cost_factored(ge: &GlobalEconomy, cost_factor: u8, tile_count: u32) -> i64 {
    let per_tile = get_price(ge, PriceIndex::BuildObject, i64::from(cost_factor), 0);
    per_tile.saturating_mul(i64::from(tile_count.max(1)))
}

/// Coste de retirar un objeto (`ObjectSpec::GetClearCost`).
///
/// `ClearTile_Object` aplica el precio `PR_CLEAR_OBJECT` a la huella completa
/// y luego divide el total entre cinco. El signo de un objeto con
/// `ObjectFlag::ClearIncome` se gestiona en el comando, no aquí.
#[must_use]
pub fn object_clear_cost_factored(ge: &GlobalEconomy, cost_factor: u8, tile_count: u32) -> i64 {
    let total = get_price(ge, PriceIndex::ClearObject, i64::from(cost_factor), 0)
        .saturating_mul(i64::from(tile_count.max(1)));
    total / 5
}

/// Coste por tesela de vía (`Price::BuildRail`).
#[must_use]
pub fn rail_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildRail, 1, 0)
}

/// Coste de vía con factor Action0 `0x13` (`8` = ×1).
#[must_use]
pub fn rail_build_cost_factored(ge: &GlobalEconomy, cost_multiplier: u16) -> i64 {
    let factor = if cost_multiplier == 0 {
        8
    } else {
        cost_multiplier
    };
    // Escala relativa a default 8: price * factor / 8.
    let base = get_price(ge, PriceIndex::BuildRail, 1, 0);
    base.saturating_mul(i64::from(factor)) / 8
}

/// Ingreso por retirar una pieza de vía (`RailClearCost`).
///
/// `OpenTTD` limita el reembolso configurado por `PR_CLEAR_RAIL` a tres
/// cuartos del coste de construcción cuando el tipo de vía es muy barato.
#[must_use]
pub fn rail_clear_cost(ge: &GlobalEconomy, cost_multiplier: u16) -> i64 {
    let configured = get_price(ge, PriceIndex::ClearRail, 1, 0);
    let build_cost = rail_build_cost_factored(ge, cost_multiplier);
    configured.max(-(build_cost.saturating_mul(3) / 4))
}

/// Coste por tesela de carretera (`Price::BuildRoad`).
#[must_use]
pub fn road_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildRoad, 1, 0)
}

/// Coste de carretera con factor Action0 `0x13` (`8` = ×1).
#[must_use]
pub fn road_build_cost_factored(ge: &GlobalEconomy, cost_multiplier: u16) -> i64 {
    let factor = if cost_multiplier == 0 {
        8
    } else {
        cost_multiplier
    };
    let base = get_price(ge, PriceIndex::BuildRoad, 1, 0);
    base.saturating_mul(i64::from(factor)) / 8
}

/// Coste plano por pieza de carretera (`PR_CLEAR_ROAD`).
#[must_use]
pub fn road_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearRoad, 1, 0)
}

/// Coste por pieza de carretera o tranvía (`RoadClearCost`).
///
/// `OpenTTD` cobra una tarifa plana al retirar carretera. El tranvía conserva
/// esa tarifa y descuenta tres cuartos del coste de construcción del tipo
/// retirado, por lo que puede producir un reembolso.
#[must_use]
pub fn road_clear_cost_factored(ge: &GlobalEconomy, is_tram: bool, cost_multiplier: u16) -> i64 {
    let clear = road_clear_cost(ge);
    if is_tram {
        clear.saturating_sub(road_build_cost_factored(ge, cost_multiplier).saturating_mul(3) / 4)
    } else {
        clear
    }
}

/// Coste de construir una señal ferroviaria (`PR_BUILD_SIGNALS`).
#[must_use]
pub fn signal_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildSignals, 1, 0)
}

/// Coste de retirar una señal ferroviaria (`PR_CLEAR_SIGNALS`).
#[must_use]
pub fn signal_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearSignals, 1, 0)
}

/// Coste de limpiar agua de mar o río (`PR_CLEAR_WATER`).
#[must_use]
pub fn water_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearWater, 1, 0)
}

/// Coste base de limpiar hierba con densidad (`PR_CLEAR_GRASS`).
#[must_use]
pub fn grass_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearGrass, 1, 0)
}

/// Coste de limpiar un canal (`PR_CLEAR_CANAL`).
#[must_use]
pub fn canal_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearCanal, 1, 0)
}

/// Coste de limpiar costa sin una sola esquina elevada (`PR_CLEAR_ROUGH`).
#[must_use]
pub fn rough_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearRough, 1, 0)
}

/// Coste de limpiar roca (`PR_CLEAR_ROCKS`).
#[must_use]
pub fn rocks_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearRocks, 1, 0)
}

/// Coste de limpiar un campo (`PR_CLEAR_FIELDS`).
#[must_use]
pub fn fields_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearFields, 1, 0)
}

/// Coste base de limpiar una tesela de árboles (`PR_CLEAR_TREES`).
#[must_use]
pub fn trees_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearTrees, 1, 0)
}

/// Coste de construir una tesela adicional de canal (`PR_BUILD_CANAL`).
#[must_use]
pub fn canal_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildCanal, 1, 0)
}

/// Coste de construir una esclusa (`PR_BUILD_LOCK`).
#[must_use]
pub fn lock_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildLock, 1, 0)
}

/// Coste de retirar una esclusa (`PR_CLEAR_LOCK`).
#[must_use]
pub fn lock_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearLock, 1, 0)
}

/// Coste del depósito ferroviario (`PR_BUILD_DEPOT_TRAIN`) y su tramo de vía.
///
/// `CmdBuildTrainDepot` suma ambos conceptos incluso cuando la boca ya toca una
/// vía existente. El factor Action0 de rail sólo afecta al segundo término.
#[must_use]
pub fn train_depot_build_cost(ge: &GlobalEconomy, rail_cost_multiplier: u16) -> i64 {
    get_price(ge, PriceIndex::BuildDepotTrain, 1, 0)
        .saturating_add(rail_build_cost_factored(ge, rail_cost_multiplier))
}

/// Coste de retirar un depósito ferroviario (`PR_CLEAR_DEPOT_TRAIN`).
#[must_use]
pub fn train_depot_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearDepotTrain, 1, 0)
}

/// Coste del depósito de carretera (`PR_BUILD_DEPOT_ROAD`).
#[must_use]
pub fn road_depot_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildDepotRoad, 1, 0)
}

/// Coste de retirar un depósito de carretera (`PR_CLEAR_DEPOT_ROAD`).
#[must_use]
pub fn road_depot_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearDepotRoad, 1, 0)
}

/// Coste del depósito naval (`PR_BUILD_DEPOT_SHIP`).
#[must_use]
pub fn ship_depot_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildDepotShip, 1, 0)
}

/// Coste de retirar un depósito naval (`PR_CLEAR_DEPOT_SHIP`).
#[must_use]
pub fn ship_depot_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearDepotShip, 1, 0)
}

/// Coste base de estación jugable (`Price::BuildStationRail` y equivalentes road).
#[must_use]
pub fn station_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildStationRail, 1, 0)
}

/// Coste de retirar una tesela de estación ferroviaria (`PR_CLEAR_STATION_RAIL`).
#[must_use]
pub fn rail_station_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearStationRail, 1, 0)
}

/// Coste de retirar una tesela de aeropuerto (`PR_CLEAR_STATION_AIRPORT`).
#[must_use]
pub fn airport_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearStationAirport, 1, 0)
}

/// Coste de construir un muelle (`PR_BUILD_STATION_DOCK`).
#[must_use]
pub fn dock_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildStationDock, 1, 0)
}

/// Coste de retirar un muelle (`PR_CLEAR_STATION_DOCK`).
#[must_use]
pub fn dock_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearStationDock, 1, 0)
}

/// Coste por tesela de una parada vial `NewGRF`.
///
/// `RoadStopSpec::GetBuildCost` usa la categoría de bus/camión y el
/// multiplicador Action0 `0x15` con un desplazamiento `-4`; `16` conserva el
/// precio vanilla.
#[must_use]
pub fn road_stop_build_cost_factored(
    ge: &GlobalEconomy,
    stop_kind: StopKind,
    cost_multiplier: u8,
) -> i64 {
    let index = if stop_kind == StopKind::TruckStop {
        PriceIndex::BuildStationTruck
    } else {
        PriceIndex::BuildStationBus
    };
    get_price(ge, index, i64::from(cost_multiplier), -4)
}

/// Coste por tesela de retirar una parada vial `NewGRF`.
///
/// El precio base de limpieza es independiente del de construcción, pero
/// comparte el multiplicador de `RoadStopSpec` y el desplazamiento `-4`.
#[must_use]
pub fn road_stop_clear_cost_factored(
    ge: &GlobalEconomy,
    stop_kind: StopKind,
    cost_multiplier: u8,
) -> i64 {
    let index = if stop_kind == StopKind::TruckStop {
        PriceIndex::ClearStationTruck
    } else {
        PriceIndex::ClearStationBus
    };
    get_price(ge, index, i64::from(cost_multiplier), -4)
}

/// Coste de waypoint ferroviario (`Price::BuildWaypointRail`).
#[must_use]
pub fn waypoint_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildWaypointRail, 1, 0)
}

/// Coste de retirar un waypoint ferroviario (`PR_CLEAR_WAYPOINT_RAIL`).
#[must_use]
pub fn rail_waypoint_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearWaypointRail, 1, 0)
}

/// Coste de construir una boya (`PR_BUILD_WAYPOINT_BUOY`).
#[must_use]
pub fn buoy_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildWaypointBuoy, 1, 0)
}

/// Coste de retirar una boya (`PR_CLEAR_WAYPOINT_BUOY`).
#[must_use]
pub fn buoy_clear_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::ClearWaypointBuoy, 1, 0)
}

/// Compatibilidad con API que solo recibía el acumulador de inflación.
#[must_use]
pub fn terraform_cost_per_corner_inflated(inflation_prices: u64) -> i64 {
    let ge = GlobalEconomy {
        inflation_prices,
        ..GlobalEconomy::new()
    };
    terraform_cost_per_corner(&ge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::pricebase::PriceIndex;
    use crate::economy::pricebase::medium_default_price;

    #[test]
    fn build_costs_track_price_base_at_default_difficulty() {
        let ge = GlobalEconomy::new();
        assert_eq!(
            terraform_cost_per_corner(&ge),
            medium_default_price(PriceIndex::Terraform)
        );
        assert_eq!(
            rail_build_cost(&ge),
            medium_default_price(PriceIndex::BuildRail)
        );
        assert_eq!(
            rail_clear_cost(&ge, 0),
            medium_default_price(PriceIndex::ClearRail)
        );
        assert_eq!(
            road_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearRoad)
        );
        assert_eq!(
            road_clear_cost_factored(&ge, false, 16),
            road_clear_cost(&ge)
        );
        assert_eq!(
            road_clear_cost_factored(&ge, true, 16),
            road_clear_cost(&ge) - road_build_cost_factored(&ge, 16) * 3 / 4
        );
        assert_eq!(
            road_depot_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearDepotRoad)
        );
        assert_eq!(
            train_depot_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearDepotTrain)
        );
        assert_eq!(
            rail_waypoint_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearWaypointRail)
        );
        assert_eq!(
            buoy_build_cost(&ge),
            medium_default_price(PriceIndex::BuildWaypointBuoy)
        );
        assert_eq!(
            buoy_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearWaypointBuoy)
        );
        assert_eq!(
            station_build_cost(&ge),
            medium_default_price(PriceIndex::BuildStationRail)
        );
        assert_eq!(
            rail_station_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearStationRail)
        );
        assert_eq!(
            airport_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearStationAirport)
        );
        assert_eq!(
            dock_build_cost(&ge),
            medium_default_price(PriceIndex::BuildStationDock)
        );
        assert_eq!(
            dock_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearStationDock)
        );
        assert_eq!(
            train_depot_build_cost(&ge, 0),
            medium_default_price(PriceIndex::BuildDepotTrain)
                + medium_default_price(PriceIndex::BuildRail)
        );
        assert_eq!(
            road_depot_build_cost(&ge),
            medium_default_price(PriceIndex::BuildDepotRoad)
        );
        assert_eq!(
            ship_depot_build_cost(&ge),
            medium_default_price(PriceIndex::BuildDepotShip)
        );
        assert_eq!(
            signal_build_cost(&ge),
            medium_default_price(PriceIndex::BuildSignals)
        );
        assert_eq!(
            signal_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearSignals)
        );
        assert_eq!(
            lock_build_cost(&ge),
            medium_default_price(PriceIndex::BuildLock)
        );
        assert_eq!(
            lock_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearLock)
        );
        assert_eq!(
            canal_build_cost(&ge),
            medium_default_price(PriceIndex::BuildCanal)
        );
        assert_eq!(
            grass_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearGrass)
        );
        assert_eq!(
            rough_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearRough)
        );
        assert_eq!(
            rocks_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearRocks)
        );
        assert_eq!(
            fields_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearFields)
        );
        assert_eq!(
            trees_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearTrees)
        );
    }

    #[test]
    fn road_stop_cost_multiplier_uses_build_and_clear_categories() {
        let ge = GlobalEconomy::new();
        assert_eq!(
            road_stop_build_cost_factored(&ge, StopKind::BusStop, 16),
            medium_default_price(PriceIndex::BuildStationBus)
        );
        assert_eq!(
            road_stop_build_cost_factored(&ge, StopKind::TruckStop, 8),
            medium_default_price(PriceIndex::BuildStationTruck) / 2
        );
        assert_eq!(
            road_stop_clear_cost_factored(&ge, StopKind::BusStop, 24),
            medium_default_price(PriceIndex::ClearStationBus) * 3 / 2
        );
        assert_eq!(
            road_stop_clear_cost_factored(&ge, StopKind::TruckStop, 0),
            0
        );
    }

    #[test]
    fn ship_depot_clear_cost_matches_native_price_base() {
        let ge = GlobalEconomy::new();
        assert_eq!(
            ship_depot_clear_cost(&ge),
            medium_default_price(PriceIndex::ClearDepotShip)
        );
    }

    #[test]
    fn object_clear_cost_uses_clear_price_and_footprint_divisor() {
        let ge = GlobalEconomy::new();
        assert_eq!(
            object_clear_cost_factored(&ge, 10, 1),
            medium_default_price(PriceIndex::ClearObject) * 10 / 5
        );
        assert_eq!(
            object_clear_cost_factored(&ge, 7, 2),
            medium_default_price(PriceIndex::ClearObject) * 7 * 2 / 5
        );
    }
}
