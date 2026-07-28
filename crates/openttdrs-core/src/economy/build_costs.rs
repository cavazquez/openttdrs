//! Costos de construcción y modificación de terreno (`GetPrice`).

use super::global::GlobalEconomy;
use super::pricebase::{PriceIndex, get_price};

/// Coste de terraform por esquina modificada (`Price::Terraform`).
#[must_use]
pub fn terraform_cost_per_corner(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::Terraform, 1, 0)
}

/// Coste por tesela de terreno comprado (`Price::BuildObject`).
#[must_use]
pub fn buy_land_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildObject, 1, 0)
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

/// Coste por tesela de vía (`Price::BuildRail`).
#[must_use]
pub fn rail_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildRail, 1, 0)
}

/// Coste por tesela de carretera (`Price::BuildRoad`).
#[must_use]
pub fn road_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildRoad, 1, 0)
}

/// Coste base de estación jugable (`Price::BuildStationRail` y equivalentes road).
#[must_use]
pub fn station_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildStationRail, 1, 0)
}

/// Coste de waypoint ferroviario (`Price::BuildWaypointRail`).
#[must_use]
pub fn waypoint_build_cost(ge: &GlobalEconomy) -> i64 {
    get_price(ge, PriceIndex::BuildWaypointRail, 1, 0)
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
            station_build_cost(&ge),
            medium_default_price(PriceIndex::BuildStationRail)
        );
    }
}
