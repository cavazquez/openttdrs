use crate::map::{TileCoord, TileKind};
use crate::{GameState, Industry, IndustryKind, IndustrySpec};

use super::CommandError;
use super::transport::{build_error_for_kind, transport_tile_is_buildable};

mod industry_template;
mod layout_tables;
pub use industry_template::industry_template;

pub(super) fn place_industry_sandbox(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    place_industry_spec_sandbox(state, c, IndustrySpec::Factory)
}

pub(super) fn place_industry_kind_sandbox(
    state: &mut GameState,
    c: TileCoord,
    kind: IndustryKind,
) -> Result<(), CommandError> {
    let spec = match kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    };
    place_industry_spec_sandbox(state, c, spec)
}

pub(crate) fn check_place_industry_spec(
    map: &crate::map::Map,
    c: TileCoord,
    spec: IndustrySpec,
) -> Result<(), CommandError> {
    let template = industry_template(c, spec);
    for (tile, _) in &template {
        super::transport::check_in_bounds(map, *tile)?;
        let existing_kind = map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(existing_kind) {
            return Err(build_error_for_kind(existing_kind));
        }
    }
    Ok(())
}

pub(super) fn place_industry_spec_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
) -> Result<(), CommandError> {
    check_place_industry_spec(&state.map, c, spec)?;
    let template = industry_template(c, spec);
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    for (tile, m5) in &template {
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(*tile, 0x80, *m5)
            .map_err(|_| CommandError::OutOfBounds)?;
        // Sandbox: construcción instantánea → `IsIndustryCompleted` (bit 7 de m1).
        state
            .map
            .set_m1(*tile, 0x80)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    state
        .industries
        .push(Industry::with_tiles_spec(c, spec.kind(), spec, footprint));
    state.economy.money -= 250;
    Ok(())
}
