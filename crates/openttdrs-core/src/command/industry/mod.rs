use crate::industry_spec::{IndustrySpecDef, industry_spec_def};
use crate::map::{
    SLOPE_STEEP, Tile, TileCoord, TileKind, tile_has_water_class, tile_slope_and_z,
    water_class_from_m1,
};
use crate::{GameState, Industry, IndustryKind, IndustrySpec};

use super::CommandError;
use super::transport::{build_error_for_kind, transport_tile_is_buildable};

mod industry_template;
mod layout_tables;
mod toyland_layout_tables;
pub use industry_template::{
    industry_template, industry_template_layout_count, industry_template_with_layout,
};

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

/// Primer slot libre de `IndustryPool` representable por el modelo actual.
///
/// `IndustryID(0)` es una instancia válida en `OpenTTD`, no un centinela. Usar
/// `industries.len() + 1` desplazaba todas las filas generadas y también el
/// `m2` de los campos que pertenecen a Farm. El pool real reutiliza huecos,
/// por lo que buscar el menor ID ausente es además estable tras demoliciones o
/// imports con IDs no densos.
fn next_industry_instance_id(state: &GameState) -> u8 {
    (0..=u8::MAX)
        .find(|candidate| {
            !state
                .industries
                .iter()
                .any(|industry| industry.instance_id == *candidate)
        })
        // El mapa crudo del modelo actual conserva un byte para el ID. Si se
        // agotaran los 256 slots, mantenemos el fallback histórico hasta que
        // RMAP-064 amplíe el campo a la representación completa del pool.
        .unwrap_or(u8::MAX)
}

pub fn check_place_industry_spec(
    map: &crate::map::Map,
    c: TileCoord,
    spec: IndustrySpec,
) -> Result<(), CommandError> {
    let template = industry_template(c, spec);
    check_industry_template(map, spec, &template)
}

/// Variante interna de `CreateNewIndustry` que conserva el layout ya elegido
/// por `RandomRange(indspec->layouts.size())`.
pub fn check_place_industry_spec_layout(
    map: &crate::map::Map,
    c: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
) -> Result<(), CommandError> {
    let template =
        industry_template_with_layout(c, spec, layout_index).ok_or(CommandError::OutOfBounds)?;
    check_industry_template(map, spec, &template)
}

fn check_industry_template(
    map: &crate::map::Map,
    spec: IndustrySpec,
    template: &[(TileCoord, u8)],
) -> Result<(), CommandError> {
    for (tile, _) in template {
        super::transport::check_in_bounds(map, *tile)?;
        let existing_kind = map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if let Some(error) = industry_auto_clear_error(existing_kind) {
            return Err(error);
        }
        // `CheckIfIndustryTilesAreFree` rejects a land industry on any tile
        // carrying a valid water class, including a coastal tree. Water-built
        // industries are not in the vanilla procedural catalog yet; keeping
        // this gate explicit prevents a later clear from silently drying a
        // coast while that model is added.
        if tile_has_water_class(existing_kind)
            && water_class_from_m1(map.get(*tile).map_or(0, |current| current.m1))
                != crate::map::WaterClass::Invalid
        {
            return Err(CommandError::CannotPlaceRoadOnWater);
        }
        if map.get(*tile).is_some_and(Tile::is_tunnel_bridge_tile) {
            // A bridge/tunnel tile is not a clear substrate for a generated
            // industry (`IsBridgeAbove`/`ClearTile_TunnelBridge` in C++).
            return Err(CommandError::IndustryTileCannotBeCleared);
        }
        if !transport_tile_is_buildable(existing_kind) {
            return Err(build_error_for_kind(existing_kind));
        }
        // Todas las teselas vanilla que forman las diez industrias force-one
        // Temperate usan `SLOPE_STEEP` como máscara rechazada. OpenTTD acepta
        // una pendiente simple y rechaza cualquiera de las cuatro empinadas.
        if force_one_temperate_rejects_steep_slope(spec)
            && tile_slope_and_z(map, *tile).is_some_and(|(slope, _)| slope & SLOPE_STEEP != 0)
        {
            return Err(CommandError::InvalidTerrainSlope);
        }
    }
    Ok(())
}

/// Errores no negociables de `CMD_LANDSCAPE_CLEAR` con `DoCommandFlag::Auto`
/// dentro de `CheckIfIndustryTilesAreFree`.
///
/// La lista se amplía por clase de tesela según se trace el clear nativo. Una
/// casa no se puede demoler en automático (`ClearTile_Town` lo rechaza), aunque
/// sí sea una superficie que otros comandos de transporte puedan reemplazar.
const fn industry_auto_clear_error(kind: TileKind) -> Option<CommandError> {
    match kind {
        TileKind::Industry => Some(CommandError::IndustryTileOccupied),
        TileKind::House => Some(CommandError::IndustryTileCannotBeCleared),
        _ => None,
    }
}

/// Las teselas vanilla de las industrias force-one Temperate tienen
/// `SLOPE_STEEP` como su máscara `slopes_refused`. Las reglas particulares de
/// las demás especies (por ejemplo bancos) se portan junto con sus specs.
const fn force_one_temperate_rejects_steep_slope(spec: IndustrySpec) -> bool {
    matches!(
        spec,
        IndustrySpec::CoalMine
            | IndustrySpec::PowerStation
            | IndustrySpec::Sawmill
            | IndustrySpec::Forest
            | IndustrySpec::OilRefinery
            | IndustrySpec::Factory
            | IndustrySpec::SteelMill
            | IndustrySpec::Farm
            | IndustrySpec::OilWells
            | IndustrySpec::IronOreMine
    )
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
    place_industry_spec_template_sandbox(state, c, spec, &template)
}

pub(super) fn place_industry_spec_layout_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
) -> Result<(), CommandError> {
    if !spec.available_in(state.climate) {
        return Err(CommandError::IndustryNotAvailableInClimate);
    }
    check_place_industry_spec_layout(&state.map, c, spec, layout_index)?;
    let template =
        industry_template_with_layout(c, spec, layout_index).ok_or(CommandError::OutOfBounds)?;
    place_industry_spec_template_sandbox(state, c, spec, &template)
}

fn place_industry_spec_template_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
    template: &[(TileCoord, u8)],
) -> Result<(), CommandError> {
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    let industry_id = next_industry_instance_id(state);
    let random_colour = industry_id.wrapping_mul(5) % 16;
    for (tile, m5) in template {
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
        if let Some(error) = industry_auto_clear_error(existing_kind) {
            return Err(error);
        }
        if tile_has_water_class(existing_kind)
            && water_class_from_m1(map.get(*tile).map_or(0, |current| current.m1))
                != crate::map::WaterClass::Invalid
        {
            return Err(CommandError::CannotPlaceRoadOnWater);
        }
        if map.get(*tile).is_some_and(Tile::is_tunnel_bridge_tile) {
            return Err(CommandError::IndustryTileCannotBeCleared);
        }
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
    let industry_id = next_industry_instance_id(state);
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
    let mut industry = Industry::with_tiles(c, kind, tiles)
        .with_instance_id(industry_id)
        .with_random_colour(random_colour)
        .with_counter(counter)
        .with_newgrf_spec(def.id, &def);
    if let Some(initial_level) =
        crate::newgrf_callback::resolve_industry_production_change_build_callback(
            &def,
            &industry,
            &mut state.random,
        )
    {
        industry.prod_level = initial_level;
    }
    state.industries.push(industry);
    state.economy.money -= 250;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::map::{Map, TileCoord, tile_slope_and_z};

    fn test_newgrf_industry_spec() -> IndustrySpecDef {
        IndustrySpecDef {
            id: 37,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: vec![vec![crate::industry_spec::IndustryLayoutTile {
                x: 0,
                y: 0,
                gfx: 175,
            }]],
            produced_cargo_indices: Vec::new(),
            produced_cargo_labels: Vec::new(),
            accepted_cargo_indices: Vec::new(),
            accepted_cargo_labels: Vec::new(),
            production_rates: Vec::new(),
            input_multipliers: Vec::new(),
            callback_mask: 0,
            cost_multiplier: 0,
            name: String::new(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: None,
        }
    }

    #[test]
    fn selected_layout_is_used_by_the_industry_command() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);

        assert!(
            apply_command(
                &mut state,
                &Command::PlaceIndustrySpecLayout(origin, IndustrySpec::PowerStation, 2),
            )
            .is_ok()
        );
        assert_eq!(state.industries.len(), 1);
        assert_eq!(state.industries[0].tiles.len(), 6);
        assert_eq!(
            state.map.get_kind(TileCoord::new(6, 4)),
            Some(TileKind::Industry)
        );
    }

    #[test]
    fn force_one_layout_rejects_a_steep_tile() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_height(origin, 2).is_ok());
        assert!(tile_slope_and_z(&map, origin).is_some_and(|(slope, _)| slope & SLOPE_STEEP != 0));
        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::CoalMine, 0),
            Err(CommandError::InvalidTerrainSlope)
        );
    }

    #[test]
    fn industry_layout_never_overwrites_an_existing_industry_tile() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_kind(origin, TileKind::Industry).is_ok());

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::CoalMine, 0),
            Err(CommandError::IndustryTileOccupied)
        );
    }

    #[test]
    fn industry_layout_rejects_a_house_that_auto_clear_cannot_demolish() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_kind(origin, TileKind::House).is_ok());

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::SteelMill, 0),
            Err(CommandError::IndustryTileCannotBeCleared)
        );
    }

    #[test]
    fn industry_layout_rejects_a_coastal_tree_with_water_class() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        let Some(mut tree) = map.get(origin) else {
            panic!("flat tile missing");
        };
        tree.kind = TileKind::Forest;
        tree.m1 = crate::map::set_water_class_m1(0, crate::map::WaterClass::Sea);
        assert!(map.set_tile(origin, tree).is_ok());

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::CoalMine, 0),
            Err(CommandError::CannotPlaceRoadOnWater)
        );
    }

    #[test]
    fn industry_layout_rejects_a_bridge_tile_before_clearing() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        let Some(mut bridge) = map.get(origin) else {
            panic!("flat tile missing");
        };
        bridge.mapt = crate::map::OTTD_TILETYPE_TUNNELBRIDGE << 4;
        bridge.kind = TileKind::RoadBridge;
        assert!(map.set_tile(origin, bridge).is_ok());

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::CoalMine, 0),
            Err(CommandError::IndustryTileCannotBeCleared)
        );
    }

    #[test]
    fn newgrf_industry_layout_never_overwrites_an_existing_industry_tile() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_kind(origin, TileKind::Industry).is_ok());
        let def = test_newgrf_industry_spec();

        assert_eq!(
            check_place_industry_spec_def(&map, origin, &def),
            Err(CommandError::IndustryTileOccupied)
        );
    }

    #[test]
    fn newgrf_industry_layout_rejects_a_house_that_auto_clear_cannot_demolish() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_kind(origin, TileKind::House).is_ok());
        let def = test_newgrf_industry_spec();

        assert_eq!(
            check_place_industry_spec_def(&map, origin, &def),
            Err(CommandError::IndustryTileCannotBeCleared)
        );
    }
}
