use crate::industry_spec::{IndustrySpecDef, industry_spec_def};
use crate::map::{TileCoord, TileKind};
use crate::{GameState, Industry, IndustryKind, IndustrySpec};

use super::CommandError;
use super::transport::{build_error_for_kind, transport_tile_is_buildable};

mod industry_template;
mod layout_tables;
mod toyland_layout_tables;
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

/// 12 bits deterministas para la fase de producción (`i->counter = GB(r, 4, 12)`).
fn industry_counter_seed(state: &GameState, c: TileCoord, industry_id: u8) -> u16 {
    let salt = u64::from(industry_id);
    let lo = crate::map::industry_tile_rng(state.world_seed, state.tick.get(), c, salt);
    let hi = crate::map::industry_tile_rng(state.world_seed, state.tick.get(), c, salt + 0x100);
    ((u16::from(hi) << 8) | u16::from(lo)) & crate::industry::INDUSTRY_COUNTER_MASK
}

pub fn check_place_industry_spec(
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
    if !spec.available_in(state.climate) {
        return Err(CommandError::IndustryNotAvailableInClimate);
    }
    check_place_industry_spec(&state.map, c, spec)?;
    let template = industry_template(c, spec);
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    let industry_id = u8::try_from(state.industries.len().saturating_add(1)).unwrap_or(255);
    let random_colour = industry_id.wrapping_mul(5) % 16;
    for (tile, m5) in &template {
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(*tile, 0x80, *m5)
            .map_err(|_| CommandError::OutOfBounds)?;
        // Obra desde etapa 0; el tile loop (P6) avanza `m1` hasta `IsIndustryCompleted`.
        // OpenTTD `MakeIndustry`: tierra → WaterClass::Invalid; oil rig → Sea.
        // `m1 = 0` sería Sea y el cliente pintaría agua bajo la fábrica.
        let m1 = if crate::map::industry_gfx_is_oil_rig(u16::from(*m5)) {
            crate::map::set_water_class_m1(0, crate::map::WaterClass::Sea)
        } else {
            crate::map::set_water_class_m1(0, crate::map::WaterClass::Invalid)
        };
        state
            .map
            .set_m1(*tile, m1)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_m2(*tile, industry_id)
            .map_err(|_| CommandError::OutOfBounds)?;
        // P7: `MakeIndustry` — random bits en m3, triggers limpios en m6.
        let bits = crate::map::industry_tile_rng(
            state.world_seed,
            state.tick.get(),
            *tile,
            u64::from(*m5),
        );
        let mut map_tile = state.map.get(*tile).ok_or(CommandError::OutOfBounds)?;
        crate::map::init_industry_tile_random(&mut map_tile, bits);
        state
            .map
            .set_tile(*tile, map_tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state
        .runtime
        .industry_tile_dirty
        .extend(footprint.iter().copied());
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    let counter = industry_counter_seed(state, c, industry_id);
    state.industries.push(
        Industry::with_tiles_spec(c, spec.kind(), spec, footprint, random_colour)
            .with_instance_id(industry_id)
            .with_counter(counter),
    );
    state.economy.money -= 250;
    Ok(())
}

/// Coloca industria desde [`IndustrySpecDef`] (layout `NewGRF` → tiles con gfx ≥175).
pub fn check_place_industry_spec_def(
    map: &crate::map::Map,
    c: TileCoord,
    def: &IndustrySpecDef,
) -> Result<(), CommandError> {
    let footprint = def.footprint_at(c, 0);
    if footprint.is_empty() {
        return Err(CommandError::OutOfBounds);
    }
    for (tile, _) in &footprint {
        super::transport::check_in_bounds(map, *tile)?;
        let existing_kind = map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(existing_kind) {
            return Err(build_error_for_kind(existing_kind));
        }
    }
    Ok(())
}

/// Coloca industria `NewGRF` por id global del catálogo.
pub fn place_industry_spec_def_sandbox(
    state: &mut GameState,
    c: TileCoord,
    type_id: u16,
) -> Result<(), CommandError> {
    let Some(def) = industry_spec_def(&state.industry_spec_catalog, type_id).cloned() else {
        return Err(CommandError::OutOfBounds);
    };
    // #266: CB 0x28 location — deny observable (no silencioso).
    if !crate::newgrf_callback::apply_industry_location_callback(&def) {
        return Err(CommandError::NewGrfCallbackDenied);
    }
    check_place_industry_spec_def(&state.map, c, &def)?;
    let footprint = def.footprint_at(c, 0);
    let tiles: Vec<TileCoord> = footprint.iter().map(|(t, _)| *t).collect();
    let industry_id = u8::try_from(state.industries.len().saturating_add(1)).unwrap_or(255);
    let random_colour = industry_id.wrapping_mul(5) % 16;
    for (tile, gfx) in &footprint {
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        let mut map_tile = state.map.get(*tile).ok_or(CommandError::OutOfBounds)?;
        crate::map::set_industry_gfx(&mut map_tile, *gfx);
        // Obra desde etapa 0; agua inválida salvo oil-rig gfx.
        let m1 = if crate::map::industry_gfx_is_oil_rig(*gfx) {
            crate::map::set_water_class_m1(0, crate::map::WaterClass::Sea)
        } else {
            crate::map::set_water_class_m1(0, crate::map::WaterClass::Invalid)
        };
        map_tile.m1 = m1;
        map_tile.m2 = industry_id;
        let bits = crate::map::industry_tile_rng(
            state.world_seed,
            state.tick.get(),
            *tile,
            u64::from(*gfx),
        );
        crate::map::init_industry_tile_random(&mut map_tile, bits);
        state
            .map
            .set_tile(*tile, map_tile)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state
        .runtime
        .industry_tile_dirty
        .extend(tiles.iter().copied());
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    let kind = if def.is_processor() {
        IndustryKind::Factory
    } else {
        IndustryKind::CoalMine
    };
    let counter = industry_counter_seed(state, c, industry_id);
    state.industries.push(
        Industry::with_tiles(c, kind, tiles)
            .with_instance_id(industry_id)
            .with_random_colour(random_colour)
            .with_counter(counter)
            .with_newgrf_spec(def.id, &def),
    );
    state.economy.money -= 250;
    Ok(())
}
