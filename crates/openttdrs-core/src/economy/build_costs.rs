//! Costos de construcción y modificación de terreno.

use super::payments::inflation_prices_factor;

/// Coste de terraform por esquina modificada (`Price::Terraform` normalizado × inflación).
#[must_use]
pub fn terraform_cost_per_corner(inflation_prices: u64) -> i64 {
    use crate::game_state::TERRAFORM_BASE_PRICE;
    TERRAFORM_BASE_PRICE.saturating_mul(i64::from(inflation_prices_factor(inflation_prices))) / 1024
}

/// Coste por tesela de terreno comprado.
#[must_use]
pub fn buy_land_cost(inflation_prices: u64) -> i64 {
    use crate::game_state::BUY_LAND_BASE_PRICE;
    BUY_LAND_BASE_PRICE.saturating_mul(i64::from(inflation_prices_factor(inflation_prices))) / 1024
}

/// Coste de colocar faro o transmisor.
#[must_use]
pub fn build_object_cost(inflation_prices: u64) -> i64 {
    use crate::game_state::BUILD_OBJECT_BASE_PRICE;
    BUILD_OBJECT_BASE_PRICE.saturating_mul(i64::from(inflation_prices_factor(inflation_prices)))
        / 1024
}
