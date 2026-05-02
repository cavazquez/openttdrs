use openttdrs_core::{STATION_COVERAGE_RADIUS, station_coverage_at};

use crate::state::SimWorld;

pub(crate) fn station_details_text(sim: &SimWorld, pos: openttdrs_core::TileCoord) -> String {
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
        .map(|station| format!("stock:{} income:{}", station.stock, station.income))
        .unwrap_or_else(|| "stock:n/d income:n/d".to_string());
    format!(
        "\nStation {station_line}\nCoverage r{} accepts mail:{} goods:{}\nSupplies coal:{} wood:{} oil:{} source stock:{}",
        STATION_COVERAGE_RADIUS,
        coverage.accepts_mail,
        coverage.accepts_goods,
        coverage.supplies_coal,
        coverage.supplies_wood,
        coverage.supplies_oil,
        coverage.supplied_stock
    )
}
