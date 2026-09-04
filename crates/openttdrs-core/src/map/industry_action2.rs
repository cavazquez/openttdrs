//! Contexto de variables Action2 para `IndustryTile` y su industria padre.
//!
//! `OpenTTD` resuelve una tesela de industria con dos scopes: la tesela
//! (`IndustryTileScopeResolver`) y la instancia que la contiene
//! (`IndustriesScopeResolver`). El renderer no debe inventar un random o una
//! posición fija para cada zoom, por eso este módulo construye ambos scopes a
//! partir de `MAP1/MAP2/MAP3/MAP4` y de los pools vivos de industrias.

use crate::cargo::CargoType;
use crate::cargo_spec::{CargoSpecDef, cargo_type_from_label_with_catalog};
use crate::house_spec::get_town_radius_group;
use crate::industry::{Industry, IndustrySpec};
use crate::industry_spec::{IndustrySpecDef, industry_spec_def};
use crate::industry_tile::{IndustryTileSpecDef, NEW_INDUSTRY_TILE_OFFSET, industry_tile_spec_def};
use crate::map::{
    Map, Tile, TileCoord, TileKind, industry_construction_stage, industry_gfx,
    industry_instance_id, industry_random_triggers, tile_slope_and_z, water_class,
};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::town::{HouseZone, Town};
use crate::world_gen::Climate;

/// Construye el contexto completo para una tesela de industria.
///
/// `neighbor_params` debe contener únicamente las parejas `(variable,
/// parámetro)` que el grafo Action2 del GRF consulta. Las variables de la
/// tesela se guardan en `vars`; las de la industria padre, en
/// `parent_vars`, que es la tabla que usa el parser cuando el ajuste marca
/// scope padre (0x82/0x86/0x8A).
#[must_use]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn action2_eval_ctx_for_industry_tile_with_world(
    map: &Map,
    coord: TileCoord,
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    current_spec: Option<&IndustryTileSpecDef>,
    neighbor_params: &[(u8, u8)],
) -> Action2EvalCtx {
    action2_eval_ctx_for_industry_tile_with_world_and_cargo_catalog(
        map,
        coord,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        current_spec,
        neighbor_params,
        &[],
    )
}

/// Variante catálogo-aware de [`action2_eval_ctx_for_industry_tile_with_world`].
#[must_use]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub fn action2_eval_ctx_for_industry_tile_with_world_and_cargo_catalog(
    map: &Map,
    coord: TileCoord,
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    current_spec: Option<&IndustryTileSpecDef>,
    neighbor_params: &[(u8, u8)],
    cargo_spec_catalog: &[CargoSpecDef],
) -> Action2EvalCtx {
    action2_eval_ctx_for_industry_tile_with_world_and_parent_and_cargo_catalog(
        map,
        coord,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        current_spec,
        neighbor_params,
        None,
        cargo_spec_catalog,
    )
}

/// Variante de [`action2_eval_ctx_for_industry_tile_with_world`] que permite
/// proporcionar una industria parent temporal durante la construcción.
///
/// `PerformIndustryTileSlopeCheck` upstream evalúa `IndustryTileResolverObject`
/// antes de materializar la tesela en el mapa; en ese momento no existe aún
/// un `IndustryID` que `find_industry` pueda encontrar. El parent explícito
/// conserva, sin mutar el mapa, el tipo, layout, random y fundador que ve el
/// callback `CBID_INDTILE_SHAPE_CHECK`.
#[must_use]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_industry_tile_with_world_and_parent(
    map: &Map,
    coord: TileCoord,
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    current_spec: Option<&IndustryTileSpecDef>,
    neighbor_params: &[(u8, u8)],
    parent: Option<&Industry>,
) -> Action2EvalCtx {
    action2_eval_ctx_for_industry_tile_with_world_and_parent_and_cargo_catalog(
        map,
        coord,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        current_spec,
        neighbor_params,
        parent,
        &[],
    )
}

/// Variante catálogo-aware de
/// [`action2_eval_ctx_for_industry_tile_with_world_and_parent`].
#[must_use]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_industry_tile_with_world_and_parent_and_cargo_catalog(
    map: &Map,
    coord: TileCoord,
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    current_spec: Option<&IndustryTileSpecDef>,
    neighbor_params: &[(u8, u8)],
    parent: Option<&Industry>,
    cargo_spec_catalog: &[CargoSpecDef],
) -> Action2EvalCtx {
    let tile = map.get(coord);
    let mut ctx = Action2EvalCtx::default();
    let Some(tile) = tile else {
        return ctx;
    };

    // IndustryTileScopeResolver::GetRandomBits/GetRandomTriggers.
    let random = u32::from(tile.m3);
    ctx.random_bits = random;
    ctx.vars.insert(
        0x5F,
        (random << 8) | u32::from(industry_random_triggers(&tile)),
    );

    // Variables 0x40..0x44 del scope de tesela.
    ctx.vars.insert(
        0x40,
        if tile.kind == TileKind::Industry {
            u32::from(industry_construction_stage(tile.m1))
        } else {
            0
        },
    );
    ctx.vars.insert(
        0x41,
        terrain_type_for_industry_tile(map, coord, tile, climate),
    );
    let town = closest_town(towns, coord);
    let town_zone = town.map_or(HouseZone::TownEdge, |town| {
        get_town_radius_group(town, coord)
    });
    ctx.vars.insert(0x42, u32::from(town_zone as u8));

    let current = parent.or_else(|| find_industry(industries, &tile, coord));
    let origin = current.map_or(coord, |industry| industry.pos);
    ctx.vars.insert(0x43, relative_position(coord, origin));
    // Unlike houses, an industry tile can expose the complete animation byte.
    ctx.vars.insert(
        0x44,
        if tile.kind == TileKind::Industry {
            u32::from(tile.m3hi)
        } else {
            0
        },
    );

    if let Some(industry) = current {
        populate_industry_parent_scope(
            &mut ctx,
            map,
            coord,
            industry,
            industry_catalog,
            cargo_spec_catalog,
        );
        ctx.parent_persistent_registers
            .clone_from(&industry.newgrf_persistent_regs);
    }

    let current_grfid = current_spec.map_or(0, |spec| spec.newgrf_grfid);
    for &(variable, parameter) in neighbor_params {
        let nearby = nearby_tile(map, coord, parameter);
        let child_value = match variable {
            // IndustryTileScopeResolver variables 0x60..0x62 use signed
            // offsets. The high bit of the land-info word marks the same
            // industry, as in GetNearbyIndustryTileInformation.
            0x60 => Some(nearby_industry_tile_information(
                map, nearby, current, climate,
            )),
            0x61 => {
                if tile_belongs_to_industry(map, nearby, current) {
                    Some(
                        map.get(nearby)
                            .map_or(0, |candidate| u32::from(candidate.m3hi)),
                    )
                } else {
                    Some(u32::MAX)
                }
            }
            0x62 => Some(industry_tile_id_at_offset(
                map,
                nearby,
                current,
                current_grfid,
                tile_spec_catalog,
            )),
            0x7A => Some(current_spec.map_or(u32::MAX, |spec| {
                spec.newgrf_badge_translation
                    .get(usize::from(parameter))
                    .map_or(u32::MAX, |&badge| {
                        if badge == u16::MAX {
                            u32::MAX
                        } else {
                            u32::from(spec.associated_badges.contains(&badge))
                        }
                    })
            })),
            // Parent scope variables that receive a parameter are stored in
            // the separate table so Action2 can distinguish each offset.
            _ => None,
        };
        if let Some(value) = child_value {
            ctx.parameterized_vars.insert((variable, parameter), value);
        }

        // The same Action2 variable number has a different meaning when the
        // parent-scope bit is set (types 0x82/0x86/0x8A). Populate that table
        // as well; a GRF may legitimately use both scopes in separate groups.
        let parent_value = match variable {
            0x60 => industry_tile_id_at_offset(
                map,
                nearby,
                current,
                current_spec.map_or(0, |spec| spec.newgrf_grfid),
                tile_spec_catalog,
            ),
            0x61 => {
                if tile_belongs_to_industry(map, nearby, current) {
                    map.get(nearby)
                        .map_or(0, |candidate| u32::from(candidate.m3))
                } else {
                    0
                }
            }
            0x62 => nearby_industry_tile_information(map, nearby, None, climate),
            0x63 => {
                if tile_belongs_to_industry(map, nearby, current) {
                    map.get(nearby)
                        .map_or(0, |candidate| u32::from(candidate.m3hi))
                } else {
                    u32::MAX
                }
            }
            0x64 => nearest_industry_distance(parameter, current, industries, industry_catalog),
            0x65 => industry_town_zone_distance(nearby, current, towns),
            0x66 => industry_town_distance_square(nearby, current, towns),
            0x67 | 0x68 => industry_count_and_distance(
                parameter,
                variable,
                current,
                industries,
                industry_catalog,
                &ctx,
            ),
            0x69..=0x71 => industry_cargo_variable(
                variable,
                parameter,
                current,
                industry_catalog,
                cargo_spec_catalog,
            ),
            0x7A => child_value.unwrap_or(u32::MAX),
            _ => continue,
        };
        ctx.parent_parameterized_vars
            .insert((variable, parameter), parent_value);
    }
    ctx
}

fn find_industry<'a>(
    industries: &'a [Industry],
    tile: &Tile,
    coord: TileCoord,
) -> Option<&'a Industry> {
    if tile.kind != TileKind::Industry {
        return None;
    }
    let id = industry_instance_id(tile);
    industries
        .iter()
        .find(|industry| industry.contains_tile(coord) && (id == 0 || industry.instance_id == id))
        .or_else(|| {
            industries
                .iter()
                .find(|industry| industry.instance_id == id)
        })
}

fn tile_belongs_to_industry(map: &Map, coord: TileCoord, current: Option<&Industry>) -> bool {
    let Some(tile) = map.get(coord) else {
        return false;
    };
    let Some(industry) = current else {
        return false;
    };
    if tile.kind != TileKind::Industry {
        return false;
    }
    (industry_instance_id(&tile) != 0 && industry_instance_id(&tile) == industry.instance_id)
        || industry.contains_tile(coord)
}

fn relative_position(coord: TileCoord, origin: TileCoord) -> u32 {
    let dx = coord.x.wrapping_sub(origin.x).to_le_bytes()[0];
    let dy = coord.y.wrapping_sub(origin.y).to_le_bytes()[0];
    (u32::from(dy & 0x0F) << 20)
        | (u32::from(dx & 0x0F) << 16)
        | (u32::from(dy) << 8)
        | u32::from(dx)
}

fn closest_town(towns: &[Town], coord: TileCoord) -> Option<&Town> {
    towns
        .iter()
        .min_by_key(|town| crate::house_spec::distance_square(town.pos, coord))
}

fn terrain_type_for_industry_tile(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    climate: Climate,
) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        if tile.m7 & 0x20 != 0 {
            return 1;
        }
        // An industry tile stores its gfx in m5, so inspect adjacent clear
        // tiles for the desert marker when the current tile hides it.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(nearby).is_some_and(|candidate| {
                candidate.kind == TileKind::Grass
                    && candidate.m5 & 0x07 == crate::world_gen::CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

fn nearby_tile(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let signed_nibble = |value: u8| {
        let value = i32::from(value & 0x0F);
        if value >= 8 { value - 16 } else { value }
    };
    TileCoord::new(
        base.x
            .saturating_add(signed_nibble(parameter))
            .rem_euclid(width),
        base.y
            .saturating_add(signed_nibble(parameter >> 4))
            .rem_euclid(height),
    )
}

fn nearby_industry_tile_information(
    map: &Map,
    coord: TileCoord,
    current: Option<&Industry>,
    climate: Climate,
) -> u32 {
    let Some(tile) = map.get(coord) else {
        return 0;
    };
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, tile.height));
    let terrain = terrain_type_for_industry_tile(map, coord, tile, climate);
    let water_bits = u32::from(water_class(tile).map_or(0, |class| (class.as_u8() + 1) & 3));
    let terrain_info = (water_bits << 5)
        | (u32::from(u8::from(tile.kind == TileKind::Water)) << 1)
        | (terrain << 2);
    let tile_type = tile_type_as_ottd(tile);
    let same = u8::from(tile_belongs_to_industry(map, coord, current));
    (u32::from(tile_type) << 24)
        | (u32::from(z) << 16)
        | (terrain_info << 8)
        | (u32::from(same) << 8)
        | u32::from(tileh)
}

fn tile_type_as_ottd(tile: Tile) -> u8 {
    if tile.ottd_type_nibble() != 0 || tile.kind == TileKind::Grass {
        return tile.ottd_type_nibble();
    }
    match tile.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
        TileKind::House => 3,
        TileKind::Forest => 4,
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        TileKind::CoalField | TileKind::Unknown(_) | TileKind::Grass => 0,
    }
}

fn industry_tile_id_at_offset(
    map: &Map,
    coord: TileCoord,
    current: Option<&Industry>,
    current_grfid: u32,
    tile_catalog: &[IndustryTileSpecDef],
) -> u32 {
    if !tile_belongs_to_industry(map, coord, current) {
        return u32::from(u16::MAX);
    }
    let Some(tile) = map.get(coord) else {
        return u32::from(u16::MAX);
    };
    let gfx = industry_gfx(&tile);
    if gfx < NEW_INDUSTRY_TILE_OFFSET {
        return 0xFF00 | u32::from(gfx);
    }
    let Some(def) = industry_tile_spec_def(tile_catalog, gfx) else {
        return 0xFFFE;
    };
    if !def.has_newgrf_sprites() {
        return 0xFF00 | u32::from(def.subst_id);
    }
    if def.newgrf_grfid == current_grfid {
        u32::from(def.newgrf_local_id)
    } else {
        0xFFFE
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn populate_industry_parent_scope(
    ctx: &mut Action2EvalCtx,
    map: &Map,
    coord: TileCoord,
    industry: &Industry,
    industry_catalog: &[IndustrySpecDef],
    cargo_spec_catalog: &[CargoSpecDef],
) {
    let spec = industry
        .newgrf_type_id
        .and_then(|id| industry_spec_def(industry_catalog, id));
    let inputs = industry_input_requirements(industry, spec, cargo_spec_catalog);
    for (slot, (cargo, _)) in inputs.iter().take(3).enumerate() {
        ctx.parent_vars.insert(
            0x40 + u8::try_from(slot).unwrap_or(0),
            industry
                .accepted_cargo_waiting(*cargo)
                .min(u32::from(u16::MAX)),
        );
    }
    ctx.parent_vars
        .insert(0x43, closest_water_distance(map, coord));
    // OpenTTD stores the selected layout on the Industry instance.  Keeping
    // the ordinal (rather than inferring it from the footprint) matters when
    // two layouts share the same geometry and when CB68 filters instances.
    ctx.parent_vars
        .insert(0x44, u32::from(industry.selected_layout));
    // `0x45` combines founder, AI bit and company recolour in OpenTTD.  The
    // reduced model has the stable founder id; the optional high bits remain
    // zero until company recolour metadata is modelled here.
    let founder = industry
        .founder
        .map_or(u32::from(crate::industry::INDUSTRY_FOUNDER_INVALID), |id| {
            u32::from(id.0)
        });
    ctx.parent_vars.insert(0x45, founder);
    ctx.parent_vars.insert(0x46, industry.construction_date);
    ctx.parent_vars
        .insert(0x47, u32::from(industry.control_flags));
    ctx.parent_vars.insert(0x64, 0);
    if let Some(spec) = spec {
        ctx.parent_vars.insert(0xA6, u32::from(spec.local_id));
    }
    let index = industry_tile_index(map, industry.pos);
    ctx.parent_vars.insert(0x80, index);
    ctx.parent_vars.insert(0x81, index >> 8);
    let (width, height) = industry_dimensions(industry);
    ctx.parent_vars.insert(0x86, u32::from(width));
    ctx.parent_vars.insert(0x87, u32::from(height));
    let outputs = industry_output_cargos(industry, spec, cargo_spec_catalog);
    let output_waiting = [industry.stock, industry.secondary_stock];
    let output_rates = [
        industry.production_rate(),
        industry
            .newgrf_secondary_production_rate
            .or_else(|| {
                industry
                    .spec
                    .and_then(IndustrySpec::production_rate_secondary)
            })
            .or_else(|| spec.and_then(IndustrySpecDef::secondary_production_rate))
            .unwrap_or(0),
    ];
    for slot in 0..2 {
        if let Some(cargo) = outputs.get(slot) {
            ctx.parent_vars.insert(
                0x88 + u8::try_from(slot).unwrap_or(0),
                u32::from(cargo.bitnum()),
            );
            let waiting = output_waiting[slot];
            let base = 0x8A + u8::try_from(slot * 2).unwrap_or(0);
            // 0x8A/0x8C return the complete WORD; the following variable is
            // only the high byte.  Truncating the low variable to eight bits
            // made Action2 groups disagree once a stock exceeded 255.
            ctx.parent_vars.insert(base, waiting & 0xFFFF);
            ctx.parent_vars.insert(base + 1, (waiting >> 8) & 0xFF);
            ctx.parent_vars.insert(
                0x8E + u8::try_from(slot).unwrap_or(0),
                u32::from(output_rates[slot]),
            );
        }
    }
    for (slot, (cargo, _)) in inputs.iter().take(3).enumerate() {
        ctx.parent_vars.insert(
            0x90 + u8::try_from(slot).unwrap_or(0),
            u32::from(cargo.bitnum()),
        );
    }
    ctx.parent_vars.insert(0x93, u32::from(industry.prod_level));
    let current = industry.history.samples.last();
    let previous = industry
        .history
        .samples
        .get(industry.history.samples.len().saturating_sub(2));
    let produced = current.map_or(0, |sample| sample.produced);
    let transported = current.map_or(0, |sample| sample.transported);
    let last_produced = previous.map_or(0, |sample| sample.produced);
    let last_transported = previous.map_or(0, |sample| sample.transported);
    put_low_high(ctx, 0x94, produced);
    put_low_high(ctx, 0x98, transported);
    put_low_high(ctx, 0x9E, last_produced);
    put_low_high(ctx, 0xA2, last_transported);
    ctx.parent_vars
        .insert(0x9C, u32::from(industry.last_month_pct_transported()));
    ctx.parent_vars
        .insert(0x9D, u32::from(industry.last_month_pct_transported()));
    ctx.parent_vars.insert(0xA7, founder);
    ctx.parent_vars
        .insert(0xA8, u32::from(industry.random_colour));
    ctx.parent_vars.insert(
        0xA9,
        industry
            .last_prod_year
            .saturating_sub(crate::news::CALENDAR_BASE_YEAR)
            .min(u32::from(u8::MAX)),
    );
    ctx.parent_vars.insert(0xAA, u32::from(industry.counter));
    ctx.parent_vars
        .insert(0xAB, u32::from(industry.counter >> 8));
    ctx.parent_vars
        .insert(0xAC, u32::from(industry.was_cargo_delivered));
    ctx.parent_vars.insert(
        0xB0,
        industry
            .construction_date
            .saturating_sub(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR)
            .min(u32::from(u16::MAX)),
    );
    ctx.parent_vars
        .insert(0xB3, u32::from(industry.construction_type));
    // `0xB4` is the most recent accepted-cargo date, rebased to the native
    // 1920 epoch and clamped to a WORD. Empty/legacy instances resolve zero.
    let last_accepted = crate::ALL_CARGO_TYPES
        .iter()
        .map(|&cargo| industry.last_accepted_date(cargo))
        .chain(
            (0..crate::CUSTOM_CARGO_COUNT)
                .map(|slot| industry.last_accepted_date(crate::cargo::custom_cargo(slot))),
        )
        .max()
        .unwrap_or(0);
    ctx.parent_vars.insert(
        0xB4,
        last_accepted
            .saturating_sub(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR)
            .min(u32::from(u16::MAX)),
    );
    // `IndustriesScopeResolver::GetRandomBits` reads Industry::random.  The
    // production phase counter is a separate variable (0xAA/0xAB) and must
    // not become the random source for Action2 groups.
    ctx.parent_random_bits = u32::from(industry.newgrf_random);
}

fn put_low_high(ctx: &mut Action2EvalCtx, low: u8, value: u32) {
    // OpenTTD's low variable is a WORD (the next variable exposes its high
    // byte), not an already-truncated byte.
    ctx.parent_vars.insert(low, value & 0xFFFF);
    ctx.parent_vars.insert(low + 1, (value >> 8) & 0xFF);
}

fn industry_tile_index(map: &Map, coord: TileCoord) -> u32 {
    let width = map.dimensions().0;
    let x = u32::try_from(coord.x.max(0)).unwrap_or(0);
    let y = u32::try_from(coord.y.max(0)).unwrap_or(0);
    y.saturating_mul(width).saturating_add(x)
}

fn industry_dimensions(industry: &Industry) -> (u8, u8) {
    let mut min_x = industry.pos.x;
    let mut max_x = industry.pos.x;
    let mut min_y = industry.pos.y;
    let mut max_y = industry.pos.y;
    for tile in &industry.tiles {
        min_x = min_x.min(tile.x);
        max_x = max_x.max(tile.x);
        min_y = min_y.min(tile.y);
        max_y = max_y.max(tile.y);
    }
    (
        u8::try_from(max_x.saturating_sub(min_x).saturating_add(1)).unwrap_or(u8::MAX),
        u8::try_from(max_y.saturating_sub(min_y).saturating_add(1)).unwrap_or(u8::MAX),
    )
}

fn closest_water_distance(map: &Map, coord: TileCoord) -> u32 {
    let (width, height) = map.dimensions();
    let mut best = u32::MAX;
    for y in 0..height {
        for x in 0..width {
            let candidate = TileCoord::new(
                i32::try_from(x).unwrap_or(i32::MAX),
                i32::try_from(y).unwrap_or(i32::MAX),
            );
            if map
                .get(candidate)
                .is_some_and(|tile| tile.kind == TileKind::Water)
            {
                let distance = coord
                    .x
                    .abs_diff(candidate.x)
                    .saturating_add(coord.y.abs_diff(candidate.y));
                best = best.min(distance);
            }
        }
    }
    best
}

fn industry_town_zone_distance(
    coord: TileCoord,
    current: Option<&Industry>,
    towns: &[Town],
) -> u32 {
    let Some(industry) = current else {
        return 0;
    };
    let town = closest_town(towns, industry.pos);
    let Some(town) = town else {
        return 0;
    };
    let zone = get_town_radius_group(town, coord) as u32;
    let distance = town
        .pos
        .x
        .abs_diff(coord.x)
        .saturating_add(town.pos.y.abs_diff(coord.y))
        .min(u32::from(u16::MAX));
    (zone << 16) | distance
}

fn industry_town_distance_square(
    coord: TileCoord,
    current: Option<&Industry>,
    towns: &[Town],
) -> u32 {
    let Some(industry) = current else {
        return 0;
    };
    closest_town(towns, industry.pos).map_or(0, |town| {
        crate::house_spec::distance_square(town.pos, coord)
    })
}

fn nearest_industry_distance(
    parameter: u8,
    current: Option<&Industry>,
    industries: &[Industry],
    catalog: &[IndustrySpecDef],
) -> u32 {
    let Some(current) = current else {
        return u32::MAX;
    };
    let target = catalog
        .iter()
        .find(|spec| spec.local_id == parameter)
        .and_then(|spec| {
            industries.iter().find(|industry| {
                industry.newgrf_type_id == Some(spec.id)
                    || (industry.newgrf_type_id.is_none()
                        && vanilla_kind_index(industry) == parameter)
            })
        })
        .or_else(|| {
            industries.iter().find(|industry| {
                industry.newgrf_type_id == Some(u16::from(parameter))
                    || (industry.newgrf_type_id.is_none()
                        && vanilla_kind_index(industry) == parameter)
            })
        });
    let Some(target) = target else {
        return u32::MAX;
    };
    industries
        .iter()
        .filter(|industry| {
            industry.instance_id != current.instance_id && same_industry_type(industry, target)
        })
        .map(|industry| {
            current
                .pos
                .x
                .abs_diff(industry.pos.x)
                .saturating_add(current.pos.y.abs_diff(industry.pos.y))
        })
        .min()
        .unwrap_or(u32::MAX)
}

#[allow(clippy::too_many_arguments)]
fn industry_count_and_distance(
    parameter: u8,
    variable: u8,
    current: Option<&Industry>,
    industries: &[Industry],
    catalog: &[IndustrySpecDef],
    ctx: &Action2EvalCtx,
) -> u32 {
    let Some(current) = current else {
        return u32::MAX;
    };
    let requested_grfid = ctx.registers_100.get(&0x100).copied().unwrap_or(0);
    let target = if requested_grfid == 0 {
        industries.iter().find(|industry| {
            industry.newgrf_type_id == Some(u16::from(parameter))
                || (industry.newgrf_type_id.is_none() && vanilla_kind_index(industry) == parameter)
        })
    } else {
        let wanted = catalog.iter().find(|spec| {
            spec.from_newgrf && spec.grfid == requested_grfid && spec.local_id == parameter
        });
        industries
            .iter()
            .find(|industry| wanted.is_some_and(|spec| industry.newgrf_type_id == Some(spec.id)))
    };
    let Some(target) = target else {
        return 0xFFFF;
    };
    let mut count = 0u32;
    let mut closest = u32::MAX;
    for industry in industries {
        if industry.instance_id == current.instance_id || !same_industry_type(industry, target) {
            continue;
        }
        if variable == 0x68 {
            let filter = ctx.registers_100.get(&0x101).copied().unwrap_or(0);
            if filter & 0xFF != 0 && filter & 0xFF != u32::from(industry.counter & 0xFF) {
                continue;
            }
        }
        count = count.saturating_add(1).min(u32::from(u8::MAX));
        let distance = current
            .pos
            .x
            .abs_diff(industry.pos.x)
            .saturating_add(current.pos.y.abs_diff(industry.pos.y));
        closest = closest.min(distance);
    }
    (count << 16) | closest.min(u32::from(u16::MAX))
}

fn same_industry_type(a: &Industry, b: &Industry) -> bool {
    match (a.newgrf_type_id, b.newgrf_type_id) {
        (Some(a), Some(b)) => a == b,
        (None, None) => a.kind == b.kind && a.spec == b.spec,
        _ => false,
    }
}

fn vanilla_kind_index(industry: &Industry) -> u8 {
    match industry.kind {
        crate::industry::IndustryKind::CoalMine => 0,
        crate::industry::IndustryKind::Forest => 1,
        crate::industry::IndustryKind::OilWell => 2,
        crate::industry::IndustryKind::Factory => 3,
    }
}

fn industry_cargo_variable(
    variable: u8,
    parameter: u8,
    current: Option<&Industry>,
    catalog: &[IndustrySpecDef],
    cargo_spec_catalog: &[CargoSpecDef],
) -> u32 {
    let Some(industry) = current else {
        return 0;
    };
    let spec = industry
        .newgrf_type_id
        .and_then(|id| industry_spec_def(catalog, id));
    let cargo_for_local =
        |indices: &[u8], labels: &[String], dynamic_slots: &[Option<CargoType>]| {
            let index = indices
                .iter()
                .position(|&candidate| candidate == parameter)?;
            if industry.newgrf_dynamic_cargo_types {
                // A dynamic callback may have deliberately blanked this slot;
                // never fall back to the static cargo in that case.
                return dynamic_slots.get(index).copied().flatten();
            }
            labels
                .get(index)
                .and_then(|label| cargo_type_from_label_with_catalog(label, cargo_spec_catalog))
                .or_else(|| dynamic_slots.get(index).copied().flatten())
                .or_else(|| CargoType::from_cargo_id(parameter))
        };
    let produced = spec
        .and_then(|spec| {
            cargo_for_local(
                &spec.produced_cargo_indices,
                &spec.produced_cargo_labels,
                &industry.newgrf_output_cargo_slots,
            )
        })
        .or_else(|| {
            industry_output_cargos(industry, spec, cargo_spec_catalog)
                .into_iter()
                .find(|cargo| cargo.bitnum() == parameter)
        });
    let accepted = spec
        .and_then(|spec| {
            cargo_for_local(
                &spec.accepted_cargo_indices,
                &spec.accepted_cargo_labels,
                &industry.newgrf_input_cargo_slots,
            )
        })
        .or_else(|| {
            industry_input_requirements(industry, spec, cargo_spec_catalog)
                .into_iter()
                .find(|(cargo, _)| cargo.bitnum() == parameter)
                .map(|(cargo, _)| cargo)
        });
    match variable {
        0x69 => produced.map_or(0, |cargo| {
            let outputs = industry_output_cargos(industry, spec, cargo_spec_catalog);
            industry_stock_for_cargo(industry, cargo, &outputs)
        }),
        0x6A => produced.map_or(0, |_| {
            industry
                .history
                .samples
                .last()
                .map_or(0, |sample| sample.produced)
        }),
        0x6B => produced.map_or(0, |_| {
            industry
                .history
                .samples
                .last()
                .map_or(0, |sample| sample.transported)
        }),
        // `AcceptedCargo::last_accepted` is an absolute economy date. Unlike
        // the waiting amount it remains meaningful when the queue is empty.
        0x6E => accepted.map_or(0, |cargo| industry.last_accepted_date(cargo)),
        0x6F => accepted.map_or(0, |cargo| industry.accepted_cargo_waiting(cargo)),
        0x70 => produced.map_or(0, |_| u32::from(industry.production_rate())),
        _ => 0,
    }
}

fn industry_input_requirements(
    industry: &Industry,
    spec: Option<&IndustrySpecDef>,
    cargo_spec_catalog: &[CargoSpecDef],
) -> Vec<(CargoType, u32)> {
    if !industry.newgrf_processing_inputs.is_empty() {
        return industry
            .newgrf_processing_inputs
            .iter()
            .map(|input| (input.cargo, input.batch))
            .collect();
    }
    if let Some(cargos) = spec
        .map(|def| def.accepted_cargo_types_with_catalog(cargo_spec_catalog))
        .filter(|cargos| !cargos.is_empty())
    {
        return cargos.into_iter().map(|cargo| (cargo, 8)).collect();
    }
    industry.station_input_requirements()
}

fn industry_output_cargos(
    industry: &Industry,
    spec: Option<&IndustrySpecDef>,
    cargo_spec_catalog: &[CargoSpecDef],
) -> Vec<CargoType> {
    if industry.newgrf_type_id.is_some()
        && industry.newgrf_output_cargo.is_none()
        && !industry.newgrf_dynamic_cargo_types
        && let Some(outputs) = spec
            .map(|def| def.produced_cargo_types_with_catalog(cargo_spec_catalog))
            .filter(|outputs| !outputs.is_empty())
    {
        return outputs;
    }
    let outputs = industry.produced_cargos();
    if !outputs.is_empty() {
        return outputs;
    }
    spec.map(|def| def.produced_cargo_types_with_catalog(cargo_spec_catalog))
        .filter(|outputs| !outputs.is_empty())
        .unwrap_or(outputs)
}

fn industry_stock_for_cargo(industry: &Industry, cargo: CargoType, outputs: &[CargoType]) -> u32 {
    match outputs.iter().position(|candidate| *candidate == cargo) {
        Some(0) => industry.stock,
        Some(1) => industry.secondary_stock,
        Some(_) => industry.newgrf_extra_produced_cargo.get(cargo),
        None => 0,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::industry::IndustryKind;
    use crate::map::set_industry_gfx;

    #[test]
    fn industry_tile_scope_uses_map_random_stage_and_relative_position() {
        let mut map = Map::new_flat(8, 8, 0);
        let origin = TileCoord::new(2, 2);
        let coord = TileCoord::new(3, 4);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Industry;
        tile.m1 = 0x80;
        tile.m2 = 7;
        tile.m3 = 0xA5;
        tile.m3hi = 0xD2;
        set_industry_gfx(&mut tile, 175);
        map.set_tile(coord, tile).unwrap();
        let industry = Industry::with_tiles(origin, IndustryKind::CoalMine, vec![origin, coord])
            .with_instance_id(7)
            .with_counter(0x1234);
        let ctx = action2_eval_ctx_for_industry_tile_with_world(
            &map,
            coord,
            &[industry],
            &[],
            &[],
            &[],
            Climate::Temperate,
            None,
            &[(0x60, 0), (0x61, 0)],
        );
        assert_eq!(ctx.random_bits, 0xA5);
        assert_eq!(ctx.vars.get(&0x40), Some(&3));
        assert_eq!(ctx.vars.get(&0x43), Some(&0x0021_0201));
        assert_eq!(ctx.vars.get(&0x44), Some(&0xD2));
        assert_eq!(ctx.vars.get(&0x5F), Some(&0xA500));
        assert_eq!(ctx.parameterized_vars.get(&(0x61, 0)), Some(&0xD2));
    }

    #[test]
    fn industry_parent_scope_preserves_layout_random_and_word_values() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Industry;
        tile.m2 = 9;
        set_industry_gfx(&mut tile, 7);
        map.set_tile(coord, tile).unwrap();

        let mut industry = Industry::new(coord, IndustryKind::Factory)
            .with_instance_id(9)
            .with_selected_layout(3)
            .with_newgrf_random(0xBEEF)
            .with_founder(Some(crate::company::CompanyId(2)))
            .with_construction_date(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17)
            .with_construction_type(crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION)
            .with_control_flags(5)
            .with_was_cargo_delivered(true)
            .with_last_prod_year(1972)
            .with_counter(0x0678);
        industry.set_last_accepted_date(
            crate::CargoType::Livestock,
            crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 23,
        );
        industry.stock = 0x1234;
        let ctx = action2_eval_ctx_for_industry_tile_with_world(
            &map,
            coord,
            &[industry],
            &[],
            &[],
            &[],
            Climate::Temperate,
            None,
            &[(0x6E, crate::CargoType::Livestock.bitnum())],
        );

        assert_eq!(ctx.parent_vars.get(&0x44), Some(&3));
        assert_eq!(ctx.parent_vars.get(&0x45), Some(&2));
        assert_eq!(
            ctx.parent_vars.get(&0x46),
            Some(&(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17))
        );
        assert_eq!(ctx.parent_vars.get(&0x47), Some(&5));
        assert_eq!(ctx.parent_vars.get(&0x8A), Some(&0x1234));
        assert_eq!(ctx.parent_vars.get(&0xA7), Some(&2));
        assert_eq!(ctx.parent_vars.get(&0xA9), Some(&22));
        assert_eq!(ctx.parent_vars.get(&0xAA), Some(&0x0678));
        assert_eq!(ctx.parent_vars.get(&0xAC), Some(&1));
        assert_eq!(ctx.parent_vars.get(&0xB0), Some(&17));
        assert_eq!(ctx.parent_vars.get(&0xB3), Some(&2));
        assert_eq!(ctx.parent_vars.get(&0xB4), Some(&23));
        assert_eq!(
            ctx.parent_parameterized_vars
                .get(&(0x6E, crate::CargoType::Livestock.bitnum())),
            Some(&(crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 23)),
        );
        assert_eq!(ctx.parent_random_bits, 0xBEEF);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn industry_parent_scope_resolves_custom_cargo_without_hydrated_slots() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Industry;
        tile.m2 = 7;
        set_industry_gfx(&mut tile, 175);
        map.set_tile(coord, tile).unwrap();

        let custom = CargoType::Custom(0);
        let cargo_catalog = vec![crate::CargoSpecDef {
            id: crate::cargo::CUSTOM_CARGO_OFFSET,
            local_id: 3,
            label: "TOFU".into(),
            name: "Tofu".into(),
            from_newgrf: true,
            grfid: 1,
            ..crate::CargoSpecDef::default()
        }];
        let industry_def = IndustrySpecDef {
            id: 7,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: vec![3],
            produced_cargo_labels: vec!["TOFU".into()],
            accepted_cargo_indices: vec![3],
            accepted_cargo_labels: vec!["TOFU".into()],
            production_rates: vec![4],
            input_multipliers: vec![256],
            callback_mask: 0,
            behaviour: 0,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "Tofu plant".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: None,
        };
        let mut industry = Industry::new(coord, IndustryKind::Factory)
            .with_instance_id(7)
            .with_newgrf_spec(7, &industry_def);
        industry.stock = 23;
        industry.add_accepted_cargo_waiting(custom, 11);

        let ctx = action2_eval_ctx_for_industry_tile_with_world_and_cargo_catalog(
            &map,
            coord,
            std::slice::from_ref(&industry),
            &[],
            &[],
            std::slice::from_ref(&industry_def),
            Climate::Temperate,
            None,
            &[(0x40, 0), (0x69, 3), (0x6F, 3), (0x90, 0)],
            &cargo_catalog,
        );

        assert_eq!(ctx.parent_vars.get(&0x40), Some(&11));
        assert_eq!(ctx.parent_vars.get(&0x90), Some(&31));
        assert_eq!(ctx.parent_parameterized_vars.get(&(0x69, 3)), Some(&23));
        assert_eq!(ctx.parent_parameterized_vars.get(&(0x6F, 3)), Some(&11));
    }

    #[test]
    fn industry_tile_id_at_offset_rejects_other_grfid() {
        let mut map = Map::new_flat(2, 1, 0);
        let mut tile = map.get(TileCoord::new(0, 0)).unwrap();
        tile.kind = TileKind::Industry;
        tile.m1 = 0x80;
        tile.m2 = 1;
        set_industry_gfx(&mut tile, 175);
        map.set_tile(TileCoord::new(0, 0), tile).unwrap();
        let mut other = tile;
        set_industry_gfx(&mut other, 176);
        map.set_tile(TileCoord::new(1, 0), other).unwrap();
        let industry =
            Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine).with_instance_id(1);
        let spec = IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 3,
            newgrf_grfid: 9,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        let ctx = action2_eval_ctx_for_industry_tile_with_world(
            &map,
            TileCoord::new(0, 0),
            &[industry],
            &[],
            &[spec],
            &[],
            Climate::Temperate,
            None,
            &[(0x62, 0x01)],
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x62, 1)), Some(&0xFFFE));
    }
}
