use crate::company::OWNER_NONE_M1;
use crate::house_spec::{
    BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1, BUILDING_FLAG_SIZE_2X2,
    house_footprint_offsets, vanilla_or_newgrf_house,
};
use crate::industry_spec::{
    INDUSTRY_BEHAVIOUR_PLANT_ON_BUILD_MASK, IndustrySpecDef, industry_spec_def,
};
use crate::map::{
    SLOPE_STEEP, Tile, TileCoord, TileKind, clear_neighbour_non_flooding_states,
    tile_has_water_class, tile_slope_and_z, water_class_from_m1,
};
use crate::town::{nearest_town_index, update_town_radius};
use crate::world_gen::{CLEAR_GROUND_GRASS, clear_ground_m5, plant_random_farm_fields_runtime};
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
fn industry_counter_seed(state: &GameState, c: TileCoord, industry_id: u16) -> u16 {
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
fn next_industry_instance_id(state: &GameState) -> u16 {
    (0..=u16::MAX)
        .find(|candidate| {
            !state
                .industries
                .iter()
                .any(|industry| industry.instance_id == *candidate)
        })
        .unwrap_or(u16::MAX)
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
        // `CheckIfIndustryTilesAreFree` treats `OnlyInTown` industries
        // specially: every footprint tile must already be a town building,
        // and the clear command runs as OWNER_TOWN. In particular this is
        // what rejects the Arctic/Tropic bank attempt at (26,40) in the
        // native 64x64 seed while admitting the later house-backed attempt.
        let requires_house = industry_requires_house_tiles(spec);
        let allows_house = industry_allows_house_tiles(spec);
        if requires_house {
            if existing_kind != TileKind::House {
                return Err(CommandError::IndustryMustBeBuiltInTown);
            }
        } else if !(allows_house && existing_kind == TileKind::House)
            && let Some(error) = map.get(*tile).and_then(industry_auto_clear_error_for_tile)
        {
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
        if !requires_house && !transport_tile_is_buildable(existing_kind) {
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

/// Vanilla `IndustryBehaviour::OnlyInTown` species use the house-only branch
/// of `CheckIfIndustryTilesAreFree`. The rest of the town association check
/// still happens in the procedural placement phase; this helper models the
/// per-tile requirement without coupling the command module to `GameState`.
const fn industry_requires_house_tiles(spec: IndustrySpec) -> bool {
    matches!(
        spec,
        IndustrySpec::BankArcticTropic | IndustrySpec::WaterTower
    )
}

/// `OnlyNearTown` differs from `OnlyInTown`: the Toy Shop may replace an
/// existing house, while ordinary clear tiles still use the automatic clear
/// contract. `OpenTTD` performs the house branch as `OWNER_TOWN`.
const fn industry_allows_house_tiles(spec: IndustrySpec) -> bool {
    matches!(spec, IndustrySpec::ToyShop)
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

/// Contrato de `ClearTile_Road` cuando la industria se prueba con `Auto`.
///
/// Una calle normal sólo puede desaparecer automáticamente si contiene una
/// única pieza de carretera y no tiene vía de tranvía. Cruces, depósitos,
/// puentes y túneles requieren una orden explícita de demolición. Durante la
/// generación esto importa aunque la altura sea plana: `CheckIfCanLevelIndustryPlatform`
/// no inspecciona el tipo cuando no hay que nivelar, pero
/// `CheckIfIndustryTilesAreFree` sí ejecuta el clear automático primero.
fn industry_auto_clear_error_for_tile(tile: Tile) -> Option<CommandError> {
    if let Some(error) = industry_auto_clear_error(tile.kind) {
        return Some(error);
    }
    if tile.kind != TileKind::Road {
        return None;
    }
    let road_tile_type = (tile.m5 >> 6) & 0x03;
    let road_bits = tile.m5 & 0x0F;
    let tram_bits = tile.m3 & 0x0F;
    if road_tile_type != 0 || road_bits.count_ones() != 1 || tram_bits != 0 {
        Some(CommandError::IndustryTileCannotBeCleared)
    } else {
        None
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
    place_industry_spec_template_sandbox(state, c, spec, 0, &template)?;
    // Vanilla farms carry `PlantOnBuild` in the built-in industry table. The
    // generation path uses the layout command plus its own shared RNG pass,
    // so this branch is intentionally limited to the direct player command.
    if matches!(spec, IndustrySpec::Farm | IndustrySpec::FarmTropic) {
        let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
        plant_fields_on_build(state, c, &footprint);
    }
    Ok(())
}

/// Ejecuta las 50 llamadas de `PlantRandomFarmField` posteriores a la creación
/// de una granja. `OpenTTD` consume el RNG global incluso cuando un intento no
/// encuentra un rectángulo apto; el helper conserva ese detalle.
fn plant_fields_on_build(state: &mut GameState, origin: TileCoord, footprint: &[TileCoord]) {
    let (width, height) = industry_footprint_dimensions(footprint, origin);
    let industry_id = state
        .industries
        .iter()
        .rev()
        .find(|industry| industry.pos == origin)
        .map_or(0, |industry| industry.instance_id);
    let mut effect_rng = state.random;
    plant_random_farm_fields_runtime(state, origin, width, height, industry_id, &mut effect_rng);
    state.random = effect_rng;
}

fn industry_footprint_dimensions(footprint: &[TileCoord], origin: TileCoord) -> (i32, i32) {
    let max_x = footprint
        .iter()
        .map(|coord| coord.x.saturating_sub(origin.x))
        .max()
        .unwrap_or(0);
    let max_y = footprint
        .iter()
        .map(|coord| coord.y.saturating_sub(origin.y))
        .max()
        .unwrap_or(0);
    (
        max_x.saturating_add(1).max(1),
        max_y.saturating_add(1).max(1),
    )
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
    // `Industry::selected_layout` is one-based in OpenTTD; zero means an
    // industry created before NewGRF layouts existed.  CB28 itself still
    // receives the zero-based `layout_index` below.
    let selected_layout = layout_index
        .checked_add(1)
        .and_then(|index| u8::try_from(index).ok())
        .ok_or(CommandError::OutOfBounds)?;
    place_industry_spec_template_sandbox(state, c, spec, selected_layout, &template)
}

fn place_industry_spec_template_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
    selected_layout: u8,
    template: &[(TileCoord, u8)],
) -> Result<(), CommandError> {
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    let industry_id = next_industry_instance_id(state);
    let random_colour = u8::try_from(industry_id.wrapping_mul(5) % 16).unwrap_or(0);
    let mut cleared_house_tiles = Vec::new();
    for (tile, m5) in template {
        // `OnlyNearTown` allows Toy Shops to replace houses. The native clear
        // command receives the sub-tile, resolves the northern/base tile of a
        // multi-tile house, and clears every part before `MakeIndustry`
        // writes the selected layout. Without this collateral clear a house
        // part outside the industry footprint survives and shifts the raw
        // MAPT/MAP8 bytes (for example Toyland seed 1330935380 at 426,140).
        if spec == IndustrySpec::ToyShop {
            clear_town_house_for_industry(state, *tile, &mut cleared_house_tiles);
        }
        let low_mapt = state
            .map
            .get(*tile)
            .map_or(0, |current| current.mapt & 0x0F);
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(*tile, 0x80 | low_mapt, *m5)
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
        // `MakeIndustry` clears MAP8 independently of the industry random
        // byte. This matters when a town bank replaces a house carrying
        // animation metadata.
        let mut map_tile = state.map.get(*tile).ok_or(CommandError::OutOfBounds)?;
        // `MakeIndustry` writes both halves of the 9-bit gfx ID. The low
        // byte is already part of the template, but bit 8 lives in `m6` and
        // must not survive from the clear/platform source tile.
        crate::map::set_industry_gfx(&mut map_tile, u16::from(*m5));
        map_tile.m8 = 0;
        state
            .map
            .set_tile(*tile, map_tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_m2_u16(*tile, industry_id)
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
        .extend(cleared_house_tiles.iter().copied());
    state
        .runtime
        .industry_tile_dirty
        .extend(footprint.iter().copied());
    // `CheckIfIndustryTilesAreFree` already rejects an existing industry on
    // every materialized footprint tile.  Do not remove an entity merely
    // because the north/origin tile happens to lie inside another layout:
    // native layouts may start at a positive offset (for example a coal mine
    // at the last tile of a gold mine).  Retaining by `c` reused the old ID
    // and made later MAP2 links diverge from the IndustryPool.
    let counter = industry_counter_seed(state, c, industry_id);
    state.industries.push(
        Industry::with_tiles_spec(c, spec.kind(), spec, footprint, random_colour)
            .with_instance_id(industry_id)
            .with_selected_layout(selected_layout)
            .with_founder(Some(state.active_company))
            .with_construction_date(
                state
                    .calendar
                    .date
                    .saturating_add(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR),
            )
            .with_construction_type(crate::industry::INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY)
            .with_last_prod_year(state.economy_timer.year)
            .with_counter(counter),
    );
    state.economy.money -= 250;
    Ok(())
}

/// Despeja la casa municipal completa que `CMD_LANDSCAPE_CLEAR` encuentra al
/// crear una Toy Shop sobre una subtesela de `OnlyNearTown`.
///
/// `GetHouseNorthPart` no guarda un puntero al origen en el mapa: deduce la
/// tesela norte mirando los tres IDs consecutivos anteriores y sus flags de
/// tamaño. Repetimos ese algoritmo para que una casa 1×2/2×1/2×2 se retire en
/// el mismo orden que `ClearTownHouse`, incluidos los efectos de población y
/// radios del pueblo. Las casas desconocidas se tratan como 1×1 segura, igual
/// que el fallback del cargador cuando falta una definición `NewGRF`.
fn clear_town_house_for_industry(
    state: &mut GameState,
    tile_coord: TileCoord,
    cleared: &mut Vec<TileCoord>,
) {
    let Some(tile) = state.map.get(tile_coord) else {
        return;
    };
    if tile.kind != TileKind::House {
        return;
    }

    let current_id = tile.m8 & 0x0FFF;
    let (base, base_id) = house_north_part(
        &state.map,
        tile_coord,
        current_id,
        &state.house_spec_catalog,
    );
    let Some(base_tile) = state
        .map
        .get(base)
        .filter(|candidate| candidate.kind == TileKind::House)
    else {
        return;
    };
    let Some(house) = vanilla_or_newgrf_house(&state.house_spec_catalog, base_id) else {
        // An invalid HouseID cannot describe a valid multi-tile footprint. The
        // explicit tile is still cleared below, matching the command's
        // best-effort behavior for legacy maps with incomplete catalogs.
        clear_town_house_tile(&mut state.map, tile_coord, cleared);
        return;
    };
    let flags = house.building_flags();
    let offsets = house_footprint_offsets(flags);
    let completed = base_tile.m3 & 0x80 != 0;
    let town_id = u32::from(base_tile.m2) | (u32::from(base_tile.m2_hi) << 8);

    for (dx, dy) in offsets {
        let part = TileCoord::new(base.x + dx, base.y + dy);
        if state.map.get_kind(part) == Some(TileKind::House) {
            clear_town_house_tile(&mut state.map, part, cleared);
        }
    }

    // `ClearTownHouse` updates the owning town once per house, not once per
    // sub-tile. Procedural maps normally carry MAP2, while imported/legacy
    // maps may only have a nearest-town association.
    let town_index = state
        .towns
        .iter()
        .position(|town| town.id == town_id)
        .or_else(|| nearest_town_index(&state.towns, base).map(|(index, _)| index));
    if let Some(index) = town_index {
        let town = &mut state.towns[index];
        if completed {
            town.population = town
                .population
                .saturating_sub(u32::from(house.population()));
        }
        town.num_houses = town.num_houses.saturating_sub(1);
        if house.is_church() {
            town.has_church = false;
        }
        if house.is_stadium() {
            town.has_stadium = false;
        }
        update_town_radius(town);
    }
}

/// Devuelve la tesela norte/base y el `HouseID` de una casa multitile.
fn house_north_part(
    map: &crate::map::Map,
    tile: TileCoord,
    house_id: u16,
    catalog: &[crate::house_spec::HouseSpecDef],
) -> (TileCoord, u16) {
    if house_id >= 3 {
        if vanilla_or_newgrf_house(catalog, house_id - 1)
            .is_some_and(|house| house.building_flags() & BUILDING_FLAG_SIZE_2X1 != 0)
        {
            let base = TileCoord::new(tile.x - 1, tile.y);
            if map.get_kind(base) == Some(TileKind::House) {
                return (base, house_id - 1);
            }
        }
        if vanilla_or_newgrf_house(catalog, house_id - 1).is_some_and(|house| {
            house.building_flags() & (BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2) != 0
        }) {
            let base = TileCoord::new(tile.x, tile.y - 1);
            if map.get_kind(base) == Some(TileKind::House) {
                return (base, house_id - 1);
            }
        }
        if house_id >= 2
            && vanilla_or_newgrf_house(catalog, house_id - 2)
                .is_some_and(|house| house.building_flags() & BUILDING_FLAG_SIZE_2X2 != 0)
        {
            let base = TileCoord::new(tile.x - 1, tile.y);
            if map.get_kind(base) == Some(TileKind::House) {
                return (base, house_id - 2);
            }
        }
        if house_id >= 3
            && vanilla_or_newgrf_house(catalog, house_id - 3)
                .is_some_and(|house| house.building_flags() & BUILDING_FLAG_SIZE_2X2 != 0)
        {
            let base = TileCoord::new(tile.x - 1, tile.y - 1);
            if map.get_kind(base) == Some(TileKind::House) {
                return (base, house_id - 3);
            }
        }
    }
    // The current tile is the north/base part for a 1×1 house (or when a
    // malformed legacy map has an orphaned sub-ID).
    (tile, house_id)
}

fn clear_town_house_tile(
    map: &mut crate::map::Map,
    coord: TileCoord,
    cleared: &mut Vec<TileCoord>,
) {
    let Some(mut tile) = map.get(coord) else {
        return;
    };
    clear_neighbour_non_flooding_states(map, coord);
    tile.kind = TileKind::Grass;
    tile.mapt &= 0x0F;
    tile.m1 = OWNER_NONE_M1;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m5 = clear_ground_m5(CLEAR_GROUND_GRASS, 3);
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    if map.set_tile(coord, tile).is_ok() {
        cleared.push(coord);
    }
}

/// Coloca industria desde [`IndustrySpecDef`] (layout `NewGRF` → tiles con gfx ≥175).
pub fn check_place_industry_spec_def(
    map: &crate::map::Map,
    c: TileCoord,
    def: &IndustrySpecDef,
) -> Result<(), CommandError> {
    check_place_industry_spec_def_layout(map, c, def, 0)
}

/// Valida el footprint del layout `NewGRF` que se va a materializar.
pub fn check_place_industry_spec_def_layout(
    map: &crate::map::Map,
    c: TileCoord,
    def: &IndustrySpecDef,
    layout_index: usize,
) -> Result<(), CommandError> {
    let footprint = def.footprint_at(c, layout_index);
    if footprint.is_empty() {
        return Err(CommandError::OutOfBounds);
    }
    for (tile, _) in &footprint {
        super::transport::check_in_bounds(map, *tile)?;
        let existing_kind = map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if let Some(error) = map.get(*tile).and_then(industry_auto_clear_error_for_tile) {
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
    place_industry_spec_def_layout_sandbox(state, c, type_id, 0, 0)
}

/// Coloca una industria `NewGRF` usando el layout elegido por el caller.
///
/// `OpenTTD` sortea el layout antes de consultar CB28 y conserva el ordinal
/// (uno-based; cero es legacy) en `Industry::selected_layout`. La entrada histórica
/// mantiene layout cero;
/// esta variante permite a la generación, SAV y UI pasar el valor real sin
/// volver a inferirlo desde la geometría de la huella.
#[allow(clippy::too_many_lines)]
pub fn place_industry_spec_def_layout_sandbox(
    state: &mut GameState,
    c: TileCoord,
    type_id: u16,
    layout_index: usize,
    random_bits: u16,
) -> Result<(), CommandError> {
    let Some(def) = industry_spec_def(&state.industry_spec_catalog, type_id).cloned() else {
        return Err(CommandError::OutOfBounds);
    };
    // CB28 recibe el índice cero-based, mientras que la instancia y el SAV
    // conservan `layout_index + 1` (`selected_layout` upstream). Mantener dos
    // valores evita que el ordinal persistido se filtre accidentalmente al
    // scope temporal del callback.
    let callback_layout = u8::try_from(layout_index).map_err(|_| CommandError::OutOfBounds)?;
    let selected_layout = callback_layout
        .checked_add(1)
        .ok_or(CommandError::OutOfBounds)?;
    // #266: CB 0x28 location — deny observable (no silencioso). El comando de
    // usuario debe exponer el scope temporal y `IACT_USERCREATION`.
    if !crate::newgrf_callback::apply_industry_location_callback_for_build(
        &def,
        state,
        c,
        callback_layout,
        u32::from(random_bits),
    ) {
        return Err(CommandError::NewGrfCallbackDenied);
    }
    check_place_industry_spec_def_layout(&state.map, c, &def, layout_index)?;
    let footprint = def.footprint_at(c, layout_index);
    // `CheckIfIndustryTileSlopes` ejecuta el callback 0x2F por cada tesela
    // después de las validaciones de ocupación y antes de escribir el mapa.
    // El fallback de cada tile usa `slopes_refused` (prop 0x0D), de modo que
    // una tesela inclinada no se admite sólo porque el runtime no tenga un
    // Action2 resoluble.
    for (tile, gfx) in &footprint {
        if let Some(tile_def) = state
            .industry_tile_spec_catalog
            .iter()
            .find(|candidate| candidate.gfx.as_u16() == *gfx)
            && !crate::newgrf_callback::apply_industry_tile_shape_callback_for_build(
                tile_def,
                &def,
                state,
                c,
                *tile,
                layout_index,
                random_bits,
                Some(state.active_company),
                2, // IACT_USERCREATION
            )
        {
            return Err(CommandError::NewGrfCallbackDenied);
        }
    }
    let tiles: Vec<TileCoord> = footprint.iter().map(|(t, _)| *t).collect();
    let industry_id = next_industry_instance_id(state);
    let random_colour = u8::try_from(industry_id.wrapping_mul(5) % 16).unwrap_or(0);
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
        let [industry_id_low, industry_id_high] = industry_id.to_le_bytes();
        map_tile.m2 = industry_id_low;
        map_tile.m2_hi = industry_id_high;
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
    // The footprint checks above make overlap impossible.  In particular,
    // never remove an unrelated industry whose layout contains only the
    // requested origin; `IndustryPool` keeps that entity and its ID alive.
    let kind = if def.is_processor() {
        IndustryKind::Factory
    } else {
        IndustryKind::CoalMine
    };
    let footprint_for_fields = tiles.clone();
    let counter = industry_counter_seed(state, c, industry_id);
    let mut industry = Industry::with_tiles(c, kind, tiles)
        .with_instance_id(industry_id)
        .with_random_colour(random_colour)
        .with_selected_layout(selected_layout)
        .with_newgrf_random(random_bits)
        .with_founder(Some(state.active_company))
        .with_construction_date(
            state
                .calendar
                .date
                .saturating_add(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR),
        )
        .with_construction_type(crate::industry::INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY)
        .with_last_prod_year(state.economy_timer.year)
        .with_counter(counter)
        .with_newgrf_spec(def.id, &def);
    if let Some(initial_level) =
        crate::newgrf_callback::resolve_industry_production_change_build_callback(
            &def,
            &mut industry,
            &mut state.random,
        )
    {
        industry.prod_level = initial_level;
    }
    if let Some(random_colour) =
        crate::newgrf_callback::resolve_industry_decide_colour_callback(&def, &mut industry)
    {
        industry.random_colour = random_colour;
    }
    // OpenTTD evalúa `CBID_INDUSTRY_INPUT/OUTPUT_CARGO_TYPES` después de
    // inicializar color y nivel, reemplazando las listas estáticas de la
    // instancia. Sin runtime se conserva el fallback de `with_newgrf_spec`.
    let _ = crate::newgrf_callback::apply_industry_dynamic_cargo_callbacks(&def, &mut industry);
    state.industries.push(industry);
    if def.behaviour & INDUSTRY_BEHAVIOUR_PLANT_ON_BUILD_MASK != 0 {
        plant_fields_on_build(state, c, &footprint_for_fields);
    }
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
            behaviour: 0,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
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
        assert_eq!(state.industries[0].selected_layout, 3);
        assert_eq!(
            state.industries[0].founder,
            Some(crate::company::CompanyId::PLAYER)
        );
        assert_eq!(
            state.industries[0].construction_date,
            crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR
        );
        assert_eq!(
            state.industries[0].construction_type,
            crate::industry::INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY
        );
        assert_eq!(state.industries[0].last_prod_year, state.economy_timer.year);
    }

    #[test]
    fn manual_farm_plants_fields_after_the_industry_is_created() {
        let mut state = GameState::new(64, 64);
        state.random = crate::cargodist::parity::Randomizer::new(0xCAFE);
        let origin = TileCoord::new(32, 32);

        assert!(
            apply_command(
                &mut state,
                &Command::PlaceIndustrySpec(origin, IndustrySpec::Farm),
            )
            .is_ok()
        );

        let industry_id = state.industries[0].instance_id;
        let fields = state
            .map
            .tiles()
            .iter()
            .filter(|tile| {
                tile.kind == TileKind::Grass
                    && crate::map::tree_tile_loop::clear_ground_type(tile.m5)
                        == crate::world_gen::CLEAR_GROUND_FIELDS
                    && (u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8)) == industry_id
            })
            .count();
        assert!(fields > 0, "PlantOnBuild must create at least one field");
    }

    #[test]
    fn newgrf_industry_layout_and_random_are_preserved() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);
        let mut def = test_newgrf_industry_spec();
        def.layouts = vec![
            vec![crate::industry_spec::IndustryLayoutTile {
                x: 0,
                y: 0,
                gfx: 175,
            }],
            vec![
                crate::industry_spec::IndustryLayoutTile {
                    x: 0,
                    y: 0,
                    gfx: 176,
                },
                crate::industry_spec::IndustryLayoutTile {
                    x: 1,
                    y: 0,
                    gfx: 177,
                },
            ],
        ];
        state.industry_spec_catalog.push(def);

        assert!(place_industry_spec_def_layout_sandbox(&mut state, origin, 37, 1, 0xBEEF).is_ok());

        assert_eq!(state.industries.len(), 1);
        let industry = &state.industries[0];
        assert_eq!(industry.selected_layout, 2);
        assert_eq!(industry.newgrf_random, 0xBEEF);
        assert_eq!(industry.founder, Some(crate::company::CompanyId::PLAYER));
        assert_eq!(
            industry.construction_date,
            crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR
        );
        assert_eq!(
            industry.construction_type,
            crate::industry::INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY
        );
        assert_eq!(industry.tiles.len(), 2);
        assert_eq!(
            state
                .map
                .get(origin)
                .map(|tile| crate::map::industry_gfx(&tile)),
            Some(176)
        );
        assert_eq!(
            state
                .map
                .get(TileCoord::new(5, 4))
                .map(|tile| crate::map::industry_gfx(&tile)),
            Some(177)
        );
    }

    #[test]
    fn newgrf_plant_on_build_uses_the_declared_behaviour() {
        let origin = TileCoord::new(32, 32);
        let mut state = GameState::new(64, 64);
        let mut def = test_newgrf_industry_spec();
        def.behaviour = INDUSTRY_BEHAVIOUR_PLANT_ON_BUILD_MASK;
        state.industry_spec_catalog.push(def);

        assert!(place_industry_spec_def_layout_sandbox(&mut state, origin, 37, 0, 0).is_ok());

        let industry_id = state.industries[0].instance_id;
        let fields = state
            .map
            .tiles()
            .iter()
            .filter(|tile| {
                tile.kind == TileKind::Grass
                    && crate::map::tree_tile_loop::clear_ground_type(tile.m5)
                        == crate::world_gen::CLEAR_GROUND_FIELDS
                    && (u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8)) == industry_id
            })
            .count();
        assert!(
            fields > 0,
            "PlantOnBuild must create fields around NewGRF industry"
        );
    }

    #[test]
    fn newgrf_industry_layout_out_of_range_is_rejected_atomically() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);
        state
            .industry_spec_catalog
            .push(test_newgrf_industry_spec());

        assert_eq!(
            place_industry_spec_def_layout_sandbox(&mut state, origin, 37, 1, 0),
            Err(CommandError::OutOfBounds)
        );
        assert!(state.industries.is_empty());
        assert_eq!(state.map.get_kind(origin), Some(TileKind::Grass));
    }

    #[test]
    fn industry_materialization_preserves_tropic_zone_nibble() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);
        assert!(state.map.set_mapt_m5(origin, 0x22, 0).is_ok());

        assert!(
            apply_command(
                &mut state,
                &Command::PlaceIndustrySpecLayout(origin, IndustrySpec::CoalMine, 0),
            )
            .is_ok()
        );

        assert_eq!(state.map.get(origin).map_or(0, |tile| tile.mapt & 0x0F), 2);
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
    fn industry_layout_origin_does_not_remove_containing_pool_item() {
        let first_origin = TileCoord::new(4, 4);
        let second_origin = TileCoord::new(7, 7);
        let mut state = GameState::new(32, 32);
        state.climate = crate::Climate::SubArctic;

        let Some(first_layout) =
            industry_template_with_layout(first_origin, IndustrySpec::GoldMine, 0)
        else {
            panic!("gold mine layout");
        };
        let Some(second_layout) =
            industry_template_with_layout(second_origin, IndustrySpec::CoalMine, 3)
        else {
            panic!("coal mine layout");
        };
        assert!(first_layout.iter().any(|(tile, _)| *tile == second_origin));
        assert!(
            second_layout
                .iter()
                .all(|(tile, _)| !first_layout.iter().any(|(old, _)| old == tile))
        );

        assert!(
            apply_command(
                &mut state,
                &Command::PlaceIndustrySpecLayout(first_origin, IndustrySpec::GoldMine, 0),
            )
            .is_ok()
        );
        assert!(
            apply_command(
                &mut state,
                &Command::PlaceIndustrySpecLayout(second_origin, IndustrySpec::CoalMine, 3),
            )
            .is_ok()
        );

        assert_eq!(state.industries.len(), 2);
        assert_eq!(state.industries[0].instance_id, 0);
        assert_eq!(state.industries[1].instance_id, 1);
        assert!(state.industries[0].tiles.contains(&second_origin));
        assert_eq!(
            state
                .map
                .get(TileCoord::new(second_origin.x, second_origin.y + 1))
                .map(|tile| tile.m2),
            Some(1)
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
    fn arctic_bank_requires_a_house_on_every_layout_tile() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::BankArcticTropic, 0,),
            Err(CommandError::IndustryMustBeBuiltInTown)
        );

        assert!(map.set_kind(origin, TileKind::House).is_ok());
        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::BankArcticTropic, 0,),
            Err(CommandError::IndustryMustBeBuiltInTown)
        );
        assert!(map.set_kind(TileCoord::new(5, 4), TileKind::House).is_ok());
        assert!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::BankArcticTropic, 0,)
                .is_ok()
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn industry_make_resets_map8_from_the_source_tile() {
        let origin = TileCoord::new(4, 4);
        let mut state = GameState::new(16, 16);
        state.climate = crate::Climate::SubArctic;
        for tile in [origin, TileCoord::new(5, 4)] {
            let mut source = state.map.get(tile).unwrap();
            source.kind = TileKind::House;
            source.m8 = 0x7F;
            source.m6 = 0x04;
            state.map.set_tile(tile, source).unwrap();
        }

        apply_command(
            &mut state,
            &Command::PlaceIndustrySpecLayout(origin, IndustrySpec::BankArcticTropic, 0),
        )
        .unwrap();

        assert_eq!(state.map.get(origin).unwrap().m8, 0);
        assert_eq!(state.map.get(TileCoord::new(5, 4)).unwrap().m8, 0);
        assert_eq!(state.map.get(origin).unwrap().m6 & 0x04, 0);
        assert_eq!(state.map.get(TileCoord::new(5, 4)).unwrap().m6 & 0x04, 0);
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
    fn industry_layout_rejects_a_multi_piece_road_under_auto_clear() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        let Some(mut road) = map.get(origin) else {
            panic!("flat tile missing");
        };
        road.kind = TileKind::Road;
        road.mapt = 0x20;
        road.m5 = 0x0A; // ROAD_X: dos piezas, por lo que Auto debe fallar.
        road.m3 = 0;
        assert!(map.set_tile(origin, road).is_ok());

        assert_eq!(
            check_place_industry_spec_layout(&map, origin, IndustrySpec::PowerStation, 2),
            Err(CommandError::IndustryTileCannotBeCleared)
        );
    }

    #[test]
    fn toy_shop_layout_allows_a_town_house_tile() {
        let origin = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 0);
        assert!(map.set_kind(origin, TileKind::House).is_ok());
        assert!(map.set_kind(TileCoord::new(5, 4), TileKind::House).is_ok());
        assert!(map.set_kind(TileCoord::new(4, 5), TileKind::House).is_ok());
        assert!(map.set_kind(TileCoord::new(5, 5), TileKind::House).is_ok());

        assert!(check_place_industry_spec_layout(&map, origin, IndustrySpec::ToyShop, 0).is_ok());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn toy_shop_clears_the_complete_multitile_house() {
        let origin = TileCoord::new(6, 6);
        let mut state = GameState::new(16, 16);
        state.climate = crate::Climate::Toyland;
        state.towns.push(crate::town::Town {
            id: 0,
            pos: TileCoord::new(6, 6),
            population: 1_000,
            num_houses: 1,
            ..Default::default()
        });
        state
            .map
            .make_town_house_footprint(
                TileCoord::new(6, 5),
                crate::map::TownHouseSpec {
                    house_id: 99,
                    town_id: 0,
                    random_bits: 7,
                    construction_counter: 0,
                    construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                    is_protected: false,
                    processing_time: 0,
                },
                crate::map::TownHouseFootprint::OneByTwo,
            )
            .unwrap();

        apply_command(
            &mut state,
            &Command::PlaceIndustrySpecLayout(origin, IndustrySpec::ToyShop, 0),
        )
        .unwrap();

        // The selected layout starts on the southern sub-tile (HouseID 100),
        // so native ClearTownHouse must also remove the northern HouseID 99.
        assert_eq!(
            state.map.get(TileCoord::new(6, 5)).unwrap().kind,
            TileKind::Grass
        );
        assert_eq!(state.map.get(origin).unwrap().kind, TileKind::Industry);
        let industry = state.map.get(origin).unwrap();
        assert_eq!(industry.mapt, 0x80);
        assert_eq!(
            industry.m1,
            crate::map::set_water_class_m1(0, crate::map::WaterClass::Invalid)
        );
        assert_eq!(state.towns[0].num_houses, 0);
        assert_eq!(state.towns[0].population, 965);
        assert_eq!(
            state.runtime.industry_tile_dirty,
            vec![
                TileCoord::new(6, 5),
                TileCoord::new(6, 6),
                TileCoord::new(6, 6),
                TileCoord::new(6, 7),
                TileCoord::new(7, 6),
                TileCoord::new(7, 7),
            ]
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
