//! Expansión física de pueblos (`GrowTown` / `GrowTownAtRoad` / `TryBuildTownHouse`).

use crate::house_spec::{
    BUILDING_FLAG_NOT_SLOPED, BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1,
    BUILDING_FLAG_SIZE_2X2, HouseSpec, HouseSpecDef, get_town_radius_group,
    grow_town_at_road_iterations, house_footprint_offsets, house_spec_def,
    pick_town_house_id_with_catalog, vanilla_or_newgrf_house,
};
use crate::map::{
    Map, SLOPE_STEEP, TileCoord, TileKind, diag_dir_offset, effective_road_bits,
    has_tile_water_ground, tile_slope_and_z,
};
use crate::newgrf_callback::apply_house_construction_callback;
use crate::town::{Town, TownLayout, update_town_radius};
use crate::world_gen::Climate;

/// Radio de búsqueda legado (edge zone / fallback).
pub const TOWN_EXPAND_SEARCH_RADIUS: i32 = 12;
/// Intentos de colocación por ciclo de crecimiento (fallback si el grafo no avanza).
pub const TOWN_EXPAND_ATTEMPTS: u8 = 3;
/// Población añadida por casa colocada (además del step abstracto).
pub const TOWN_EXPAND_POP_PER_HOUSE: u32 = 8;

const ROAD_NW: u8 = 0x01;
const ROAD_SW: u8 = 0x02;
const ROAD_SE: u8 = 0x04;
const ROAD_NE: u8 = 0x08;
const ROAD_AXIS_X: u8 = ROAD_NE | ROAD_SW; // 0x0A
const ROAD_AXIS_Y: u8 = ROAD_NW | ROAD_SE; // 0x05
/// LCG clásico para caminar el grafo de calles (mismo estilo que `rand()`).
const LCG_MUL: u32 = 1_103_515_245;
const LCG_ADD: u32 = 12_345;

/// Offsets de búsqueda de carretera desde el centro (`_town_coord_mod`).
const TOWN_COORD_MOD: [(i32, i32); 13] = [
    (-1, 0),
    (1, 1),
    (1, -1),
    (-1, -1),
    (-1, 0),
    (0, 2),
    (2, 0),
    (0, -2),
    (-1, -1),
    (-2, 2),
    (2, 2),
    (2, -2),
    (0, 0),
];

/// Resultado de un intento de expansión física.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownExpandResult {
    House(TileCoord),
    Road(TileCoord),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrowthResult {
    Succeed,
    SearchStopped,
    Continue,
}

/// Contexto de año/clima/catálogo `NewGRF` para elegir casas.
#[derive(Debug, Clone, Copy)]
pub struct TownExpandContext<'a> {
    pub climate: Climate,
    pub calendar_year: u32,
    pub house_catalog: &'a [HouseSpecDef],
    pub house_overrides: &'a [u16],
}

impl Default for TownExpandContext<'static> {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            calendar_year: 1960,
            house_catalog: &[],
            house_overrides: &[],
        }
    }
}

/// Varias tentativas; actualiza población si hay casas nuevas.
pub fn expand_town_physically(map: &mut Map, town: &mut Town, tick: u64) -> Vec<TileCoord> {
    expand_town_physically_with_ctx(map, town, tick, TownExpandContext::default())
}

/// Expansión con clima/año/catálogo explícitos.
pub fn expand_town_physically_with_ctx(
    map: &mut Map,
    town: &mut Town,
    tick: u64,
    ctx: TownExpandContext<'_>,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    let seed = tick
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(u64::from(town.id).wrapping_mul(0x85EB_CA6B));
    let seed = u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0);

    if grow_town(map, town, seed, ctx, &mut dirty) {
        return dirty;
    }
    // Fallback: intentos cortos si el grafo no encontró sitio.
    for attempt in 0..TOWN_EXPAND_ATTEMPTS {
        let s = seed.wrapping_add(u32::from(attempt).wrapping_mul(0xC2B2_AE3D));
        match expand_town_once_with_ctx(map, town, s, ctx) {
            TownExpandResult::House(pos) | TownExpandResult::Road(pos) => dirty.push(pos),
            TownExpandResult::None => {}
        }
    }
    dirty
}

/// Intenta expandir el pueblo una vez (casa preferente, si no calle).
pub fn expand_town_once(map: &mut Map, town: &mut Town, attempt_seed: u32) -> TownExpandResult {
    expand_town_once_with_ctx(map, town, attempt_seed, TownExpandContext::default())
}

/// Coloca un tramo de la obra vial financiada por la autoridad local.
///
/// Se reutiliza exactamente la búsqueda municipal normal: primero prolonga una
/// calle existente y, si el pueblo todavía no tiene red, siembra una cerca del
/// centro. El caller decide la cadencia (al aprobar la acción y una vez por
/// mes mientras dure la financiación).
#[must_use]
pub fn fund_town_road_once(map: &mut Map, town: &Town, seed: u32) -> Option<TileCoord> {
    try_extend_or_seed_road(map, town, seed)
}

fn expand_town_once_with_ctx(
    map: &mut Map,
    town: &mut Town,
    attempt_seed: u32,
    ctx: TownExpandContext<'_>,
) -> TownExpandResult {
    if let Some(pos) = try_place_house_near_road(map, town, attempt_seed, ctx) {
        return TownExpandResult::House(pos);
    }
    if let Some(pos) = try_extend_or_seed_road(map, town, attempt_seed) {
        return TownExpandResult::Road(pos);
    }
    TownExpandResult::None
}

/// `GrowTown`: busca carretera cerca del centro y camina el grafo.
fn grow_town(
    map: &mut Map,
    town: &mut Town,
    seed: u32,
    ctx: TownExpandContext<'_>,
    dirty: &mut Vec<TileCoord>,
) -> bool {
    let mut tile = town.pos;
    for &(dx, dy) in &TOWN_COORD_MOD {
        if town_road_bits(map, tile) != 0 {
            return grow_town_at_road(map, town, tile, seed, ctx, dirty);
        }
        tile = TileCoord::new(tile.x + dx, tile.y + dy);
    }
    // Sin carretera: sembrar bloque aleatorio en hierba plana.
    seed_road_near_center(map, town.pos, seed).is_some_and(|pos| {
        dirty.push(pos);
        true
    })
}

/// `GrowTownAtRoad`: recorre el grafo con iteraciones según `TownLayout`.
fn grow_town_at_road(
    map: &mut Map,
    town: &mut Town,
    mut tile: TileCoord,
    seed: u32,
    ctx: TownExpandContext<'_>,
    dirty: &mut Vec<TileCoord>,
) -> bool {
    let mut iterations = grow_town_at_road_iterations(town.layout, town.num_houses);
    let mut rng = seed;
    let mut target_dir: Option<u8> = None; // DiagDirection 0..3

    loop {
        let cur_rb = town_road_bits(map, tile);
        match grow_town_in_tile(
            map, town, &mut tile, cur_rb, target_dir, &mut rng, ctx, dirty,
        ) {
            GrowthResult::Succeed => return true,
            GrowthResult::SearchStopped => iterations = 0,
            GrowthResult::Continue => {}
        }

        let mut rb = cur_rb;
        if let Some(dir) = target_dir {
            rb &= !diag_dir_to_road_bits(reverse_diag(dir));
        }
        if rb == 0 {
            return false;
        }

        // Elegir dirección aleatoria aún conectada.
        let mut chosen = None;
        for _ in 0..8 {
            rng = rng.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
            let dir = u8::try_from(rng >> 16).unwrap_or(0) & 3;
            let bits = diag_dir_to_road_bits(dir);
            if rb & bits != 0 && can_follow_road(map, tile, dir) {
                chosen = Some(dir);
                break;
            }
            rb &= !bits;
            if rb == 0 {
                break;
            }
        }
        let Some(dir) = chosen else {
            return false;
        };
        target_dir = Some(dir);
        tile = tile_add_diag(tile, dir);

        // No crecer sobre carreteras de otro pueblo (MVP: owner town o none).
        if map.get_kind(tile) == Some(TileKind::Road) {
            // ok
        }

        iterations -= 1;
        if iterations < 0 {
            return false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn grow_town_in_tile(
    map: &mut Map,
    town: &mut Town,
    tile_ptr: &mut TileCoord,
    cur_rb: u8,
    target_dir: Option<u8>,
    rng: &mut u32,
    ctx: TownExpandContext<'_>,
    dirty: &mut Vec<TileCoord>,
) -> GrowthResult {
    let tile = *tile_ptr;

    if cur_rb == 0 {
        // Tesela sin carretera: intentar construir según layout.
        let Some(dir) = target_dir else {
            return GrowthResult::SearchStopped;
        };
        if !can_build_town_road(map, tile) {
            return GrowthResult::SearchStopped;
        }
        if !road_allowed_here(town, tile, dir) {
            return GrowthResult::SearchStopped;
        }
        let source = reverse_diag(dir);
        let rcmd = diag_dir_to_road_bits(dir) | diag_dir_to_road_bits(source);
        if place_town_road(map, tile, rcmd) {
            dirty.push(tile);
            return GrowthResult::Succeed;
        }
        return GrowthResult::SearchStopped;
    }

    // Hay carretera: intentar casa en dirección aleatoria o extender.
    *rng = rng.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
    let dir = u8::try_from(*rng >> 16).unwrap_or(0) & 3;
    let target_rb = diag_dir_to_road_bits(dir);
    let house_tile = tile_add_diag(tile, dir);

    if cur_rb & target_rb != 0 {
        // Ya hay carretera en esa dirección: no extender; probar otra.
        return GrowthResult::Continue;
    }

    // ¿Permitir casa según layout?
    let allow_house = match town.layout {
        TownLayout::Grid2x2 | TownLayout::Grid3x3 => {
            let grid = town_layout_allows_house_here(town, house_tile);
            let rcmd = grid_road_bits(town, tile, dir);
            (rcmd & target_rb) == 0 && grid
        }
        TownLayout::BetterRoads | TownLayout::Original | TownLayout::Random => {
            let road_ok = road_allowed_here(town, house_tile, dir);
            !road_ok || chance16(rng, 6, 10)
        }
    };

    if allow_house && let Some(house_base) = try_build_town_house(map, town, house_tile, *rng, ctx)
    {
        dirty.push(house_base);
        return GrowthResult::Succeed;
    }

    // Extender carretera.
    if can_build_town_road(map, house_tile) && road_allowed_here(town, house_tile, dir) {
        let rcmd = match town.layout {
            TownLayout::Grid2x2 | TownLayout::Grid3x3 => grid_road_bits(town, tile, dir),
            _ => target_rb | diag_dir_to_road_bits(reverse_diag(dir)),
        };
        if rcmd != 0 && place_town_road(map, house_tile, rcmd) {
            let _ = or_road_bits(map, tile, diag_dir_to_road_bits(dir));
            dirty.push(house_tile);
            return GrowthResult::Succeed;
        }
    }

    GrowthResult::Continue
}

fn try_build_town_house(
    map: &mut Map,
    town: &mut Town,
    pos: TileCoord,
    seed: u32,
    ctx: TownExpandContext<'_>,
) -> Option<TileCoord> {
    if !town_layout_allows_house_here(town, pos) {
        return None;
    }
    // `TryBuildTownHouse` prueba la tesela inicial sin prohibir pendientes.
    // El flag `NotSloped` se aplica sólo después de extraer el spec elegido.
    if !can_build_house(map, pos, false) {
        return None;
    }
    let height = map.get(pos).map_or(0, |t| t.height);
    let zone = get_town_radius_group(town, pos);
    let house_id = pick_town_house_id_with_catalog(
        town,
        zone,
        ctx.climate,
        height,
        ctx.calendar_year,
        seed,
        ctx.house_catalog,
        ctx.house_overrides,
    )?;
    let flags = house_spec_def(ctx.house_catalog, house_id)
        .map(|d| d.building_flags)
        .or_else(|| HouseSpec::get(house_id).map(|hs| hs.building_flags))
        .unwrap_or(crate::house_spec::BUILDING_FLAG_SIZE_1X1);
    // El candidato puede ser una subtesela de una casa grande. Antes de
    // obtener los bits aleatorios/callback, OpenTTD resuelve cuál de las
    // bases que la contienen cumple relieve y layout.
    let base = resolve_town_house_footprint(map, town, pos, flags)?;
    // `OpenTTD` evalúa CB 0x17 después de elegir el spec y antes de reservar
    // el footprint. Es un booleano de ocho bits: `CALLBACK_FAILED` o byte bajo
    // no nulo permite; cero rechaza sin dejar teselas parcialmente colocadas.
    if let Some(def) = house_spec_def(ctx.house_catalog, house_id)
        && !apply_house_construction_callback(def)
    {
        return None;
    }
    if !place_house_footprint_for_town(map, base, house_id, flags, Some(town.id)) {
        return None;
    }
    let tiles = u16::try_from(house_footprint_offsets(flags).len()).unwrap_or(1);
    if let Some(lookup) = vanilla_or_newgrf_house(ctx.house_catalog, house_id) {
        if lookup.is_church() {
            town.has_church = true;
        }
        if lookup.is_stadium() {
            town.has_stadium = true;
        }
        town.population = town
            .population
            .saturating_add(u32::from(lookup.population()).max(TOWN_EXPAND_POP_PER_HOUSE));
    } else {
        town.population = town.population.saturating_add(TOWN_EXPAND_POP_PER_HOUSE);
    }
    town.num_houses = town.num_houses.saturating_add(tiles);
    update_town_radius(town);
    Some(base)
}

/// Coloca el footprint de una casa (1×1 / 2×1 / 1×2 / 2×2) con ids consecutivos.
pub fn place_house_footprint(
    map: &mut Map,
    north: TileCoord,
    base_id: u16,
    building_flags: u8,
) -> bool {
    place_house_footprint_for_town(map, north, base_id, building_flags, None)
}

fn place_house_footprint_for_town(
    map: &mut Map,
    north: TileCoord,
    base_id: u16,
    building_flags: u8,
    town_id: Option<u32>,
) -> bool {
    let offsets = house_footprint_offsets(building_flags);
    let noslope = building_flags & BUILDING_FLAG_NOT_SLOPED != 0;
    for &(dx, dy) in &offsets {
        let pos = TileCoord::new(north.x + dx, north.y + dy);
        if !can_build_house(map, pos, noslope) {
            return false;
        }
    }
    for (i, &(dx, dy)) in offsets.iter().enumerate() {
        let pos = TileCoord::new(north.x + dx, north.y + dy);
        let id = base_id.saturating_add(u16::try_from(i).unwrap_or(0));
        if map.set_completed_house(pos, id, 0).is_err()
            || town_id.is_some_and(|town_id| map.set_house_town_id(pos, town_id).is_err())
        {
            return false;
        }
    }
    true
}

fn try_place_house_near_road(
    map: &mut Map,
    town: &mut Town,
    seed: u32,
    ctx: TownExpandContext<'_>,
) -> Option<TileCoord> {
    let radius = i32::try_from(town.squared_town_zone_radius[0].max(36)).unwrap_or(36);
    let candidates = collect_road_tiles_near(map, town.pos, radius.min(24));
    if candidates.is_empty() {
        return None;
    }
    let n = candidates.len();
    let start = usize::try_from(seed).unwrap_or(0) % n;
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for offset in 0..n {
        let road = candidates[(start + offset) % n];
        let dir_rot = usize::try_from(seed.wrapping_add(u32::try_from(offset).unwrap_or(0)))
            .unwrap_or(0)
            % dirs.len();
        for d in 0..dirs.len() {
            let (dx, dy) = dirs[(dir_rot + d) % dirs.len()];
            let pos = TileCoord::new(road.x + dx, road.y + dy);
            if let Some(base) = try_build_town_house(
                map,
                town,
                pos,
                seed.wrapping_add(u32::try_from(offset).unwrap_or(0)),
                ctx,
            ) {
                return Some(base);
            }
        }
    }
    None
}

fn try_extend_or_seed_road(map: &mut Map, town: &Town, seed: u32) -> Option<TileCoord> {
    let radius = i32::try_from(town.squared_town_zone_radius[0].max(36)).unwrap_or(36);
    let roads = collect_road_tiles_near(map, town.pos, radius.min(24));
    if !roads.is_empty() {
        let n = roads.len();
        let start = usize::try_from(seed).unwrap_or(0) % n;
        let dirs = [
            (1, 0, ROAD_AXIS_X),
            (-1, 0, ROAD_AXIS_X),
            (0, 1, ROAD_AXIS_Y),
            (0, -1, ROAD_AXIS_Y),
        ];
        for offset in 0..n {
            let from = roads[(start + offset) % n];
            let dir_rot = usize::try_from(
                seed.wrapping_add(u32::try_from(offset).unwrap_or(0).wrapping_mul(3)),
            )
            .unwrap_or(0)
                % dirs.len();
            for d in 0..dirs.len() {
                let (dx, dy, axis) = dirs[(dir_rot + d) % dirs.len()];
                let pos = TileCoord::new(from.x + dx, from.y + dy);
                if can_build_town_road(map, pos)
                    && road_allowed_here(town, pos, diag_from_delta(dx, dy))
                    && place_town_road(map, pos, axis)
                {
                    let _ = or_road_bits(map, from, axis);
                    return Some(pos);
                }
            }
        }
    }
    seed_road_near_center(map, town.pos, seed)
}

/// Predicado compartido de `TownLayoutAllowsHouseHere` para los dos caminos de
/// crecimiento: el runtime y la generación inicial.
pub(crate) fn town_layout_allows_house_here(town: &Town, tile: TileCoord) -> bool {
    let gx = town.pos.x - tile.x;
    let gy = town.pos.y - tile.y;
    match town.layout {
        TownLayout::Grid2x2 => gx.rem_euclid(3) != 0 && gy.rem_euclid(3) != 0,
        TownLayout::Grid3x3 => gx.rem_euclid(4) != 0 && gy.rem_euclid(4) != 0,
        _ => true,
    }
}

/// `TownLayoutAllows2x2HouseHere`: una casa 2×2 no puede ocupar una línea de
/// cuadrícula y, a diferencia de una 1×1, debe caber entera entre calles.
fn town_layout_allows_2x2_house_here(town: &Town, tile: TileCoord) -> bool {
    let gx = town.pos.x - tile.x;
    let gy = town.pos.y - tile.y;
    match town.layout {
        TownLayout::Grid2x2 => gx.rem_euclid(3) == 2 && gy.rem_euclid(3) == 2,
        TownLayout::Grid3x3 => (gx & 3) >= 2 && (gy & 3) >= 2,
        _ => true,
    }
}

/// `GetTileMaxZ`: una fundación sobre pendiente alcanza el máximo de la
/// tesela; las pendientes empinadas suben dos niveles.
pub(crate) fn town_house_tile_max_z(map: &Map, tile: TileCoord) -> Option<u8> {
    tile_slope_and_z(map, tile).map(|(slope, z)| {
        z.saturating_add(if slope == 0 {
            0
        } else if slope & SLOPE_STEEP != 0 {
            2
        } else {
            1
        })
    })
}

/// `CheckBuildHouseSameZ`: toda subtesela de una casa grande debe poder
/// limpiarse y terminar a la misma altura máxima que la tesela elegida.
fn check_build_house_same_z(map: &Map, tile: TileCoord, max_z: u8, noslope: bool) -> bool {
    can_build_house(map, tile, noslope) && town_house_tile_max_z(map, tile) == Some(max_z)
}

/// `CheckFree2x2Area`, en el recorrido C++ base, `+Y`, `+X+Y`, `+X`.
fn check_free_2x2_house_area(map: &Map, base: TileCoord, max_z: u8, noslope: bool) -> bool {
    [(0, 0), (0, 1), (1, 1), (1, 0)]
        .into_iter()
        .all(|(dx, dy)| {
            check_build_house_same_z(
                map,
                TileCoord::new(base.x + dx, base.y + dy),
                max_z,
                noslope,
            )
        })
}

/// Resuelve la tesela norte/base para una casa elegida por `TryBuildTownHouse`.
///
/// Para 1×2 / 2×1 prueba primero que la tesela inicial sea la base y luego la
/// única orientación inversa que aún la contiene. Para 2×2 prueba las cuatro
/// bases posibles en el orden exacto de `CheckTownBuild2x2House`.
pub(crate) fn resolve_town_house_footprint(
    map: &Map,
    town: &Town,
    tile: TileCoord,
    building_flags: u8,
) -> Option<TileCoord> {
    if !town_layout_allows_house_here(town, tile) || !can_build_house(map, tile, false) {
        return None;
    }
    let max_z = town_house_tile_max_z(map, tile)?;
    let noslope = building_flags & BUILDING_FLAG_NOT_SLOPED != 0;
    if noslope && !is_flat_tile(map, tile) {
        return None;
    }

    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        let mut base = tile;
        // `DIAGDIR_SE`, `SW`, `NW`; después de cada fallo se avanza en su
        // dirección opuesta: base, +NW, +NW+NE, +NW+NE+SE.
        for step in [Some(1_u8), Some(2_u8), Some(3_u8), None] {
            if town_layout_allows_2x2_house_here(town, base)
                && check_free_2x2_house_area(map, base, max_z, noslope)
            {
                return Some(base);
            }
            let Some(dir) = step else {
                break;
            };
            base = tile_add_diag(base, reverse_diag(dir));
        }
        return None;
    }

    let second = if building_flags & BUILDING_FLAG_SIZE_2X1 != 0 {
        // `DIAGDIR_SW` = +X.
        Some(2_u8)
    } else if building_flags & BUILDING_FLAG_SIZE_1X2 != 0 {
        // `DIAGDIR_SE` = +Y.
        Some(1_u8)
    } else {
        None
    };
    let Some(second) = second else {
        return Some(tile);
    };

    let forward = tile_add_diag(tile, second);
    if town_layout_allows_house_here(town, forward)
        && check_build_house_same_z(map, forward, max_z, noslope)
    {
        return Some(tile);
    }

    let base = tile_add_diag(tile, reverse_diag(second));
    if town_layout_allows_house_here(town, base)
        && check_build_house_same_z(map, base, max_z, noslope)
    {
        return Some(base);
    }
    None
}

fn road_allowed_here(town: &Town, tile: TileCoord, _dir: u8) -> bool {
    match town.layout {
        TownLayout::Grid2x2 => {
            let gx = town.pos.x - tile.x;
            let gy = town.pos.y - tile.y;
            gx.rem_euclid(3) == 0 || gy.rem_euclid(3) == 0
        }
        TownLayout::Grid3x3 => {
            let gx = town.pos.x - tile.x;
            let gy = town.pos.y - tile.y;
            gx.rem_euclid(4) == 0 || gy.rem_euclid(4) == 0
        }
        // BetterRoads: distancia mín. 2 entre calles (MVP: aceptar siempre).
        TownLayout::BetterRoads | TownLayout::Original | TownLayout::Random => true,
    }
}

fn grid_road_bits(town: &Town, tile: TileCoord, dir: u8) -> u8 {
    if !road_allowed_here(town, tile_add_diag(tile, dir), dir) {
        return 0;
    }
    diag_dir_to_road_bits(dir) | diag_dir_to_road_bits(reverse_diag(dir))
}

fn seed_road_near_center(map: &mut Map, center: TileCoord, seed: u32) -> Option<TileCoord> {
    let start = usize::try_from(seed).unwrap_or(0) % TOWN_COORD_MOD.len();
    for offset in 0..TOWN_COORD_MOD.len() {
        let (dx, dy) = TOWN_COORD_MOD[(start + offset) % TOWN_COORD_MOD.len()];
        let pos = TileCoord::new(center.x + dx, center.y + dy);
        if can_build_town_road(map, pos) {
            let axis = if seed.wrapping_add(u32::try_from(offset).unwrap_or(0)) & 1 == 0 {
                ROAD_AXIS_X
            } else {
                ROAD_AXIS_Y
            };
            if place_town_road(map, pos, axis) {
                return Some(pos);
            }
        }
    }
    None
}

fn collect_road_tiles_near(map: &Map, center: TileCoord, radius: i32) -> Vec<TileCoord> {
    let mut out = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let pos = TileCoord::new(center.x + dx, center.y + dy);
            if map.get_kind(pos) == Some(TileKind::Road) {
                out.push(pos);
            }
        }
    }
    out
}

/// Equivalente acotado de `CanBuildHouseHere` para un mapa materializado.
///
/// No incluye puentes por encima ni comandos de limpieza con coste, que aún
/// no existen en el mapa procedural; sí conserva los tipos de suelo y la
/// restricción de pendiente que consumen `TryBuildTownHouse`.
pub(crate) fn can_build_house(map: &Map, pos: TileCoord, noslope: bool) -> bool {
    let clearable = map.get(pos).is_some_and(|tile| {
        matches!(tile.kind, TileKind::Grass | TileKind::Forest)
            // `CMD_LANDSCAPE_CLEAR(NoWater)` despeja una costa: aunque sus
            // bytes sean MP_WATER, `HasTileWaterGround` la excluye.
            || (tile.kind == TileKind::Water && !has_tile_water_ground(tile))
    });
    if !clearable {
        return false;
    }
    tile_slope_and_z(map, pos)
        .is_some_and(|(slope, _)| slope & SLOPE_STEEP == 0 && (!noslope || slope == 0))
}

fn can_build_town_road(map: &Map, pos: TileCoord) -> bool {
    if map.get_kind(pos) != Some(TileKind::Grass) {
        return false;
    }
    is_flat_tile(map, pos)
}

fn is_flat_tile(map: &Map, pos: TileCoord) -> bool {
    tile_slope_and_z(map, pos).is_some_and(|(h, _)| h == 0)
}

fn place_town_road(map: &mut Map, pos: TileCoord, road_bits: u8) -> bool {
    let Some(mut tile) = map.get(pos) else {
        return false;
    };
    let bits = (road_bits & 0x0F).max(0x01);
    tile.kind = TileKind::Road;
    tile.mapt = 0x20;
    tile.m5 = bits;
    tile.m1 = crate::company::OWNER_TOWN_M1;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    map.set_tile(pos, tile).is_ok()
}

fn or_road_bits(map: &mut Map, pos: TileCoord, add_bits: u8) -> bool {
    let Some(mut tile) = map.get(pos) else {
        return false;
    };
    if tile.kind != TileKind::Road {
        return false;
    }
    let current =
        effective_road_bits(tile.mapt, tile.m5, tile.kind, 2, 9).unwrap_or(tile.m5 & 0x0F);
    tile.m5 = (current | (add_bits & 0x0F)) & 0x0F;
    if tile.m5 == 0 {
        tile.m5 = add_bits & 0x0F;
    }
    map.set_tile(pos, tile).is_ok()
}

fn town_road_bits(map: &Map, pos: TileCoord) -> u8 {
    let Some(tile) = map.get(pos) else {
        return 0;
    };
    if tile.kind != TileKind::Road {
        return 0;
    }
    effective_road_bits(tile.mapt, tile.m5, tile.kind, 2, 9).unwrap_or(tile.m5 & 0x0F)
}

fn can_follow_road(map: &Map, tile: TileCoord, dir: u8) -> bool {
    let target = tile_add_diag(tile, dir);
    match map.get_kind(target) {
        Some(TileKind::Road) => town_road_bits(map, target) != 0,
        Some(TileKind::Grass) => can_build_town_road(map, target),
        _ => false,
    }
}

const fn diag_dir_to_road_bits(dir: u8) -> u8 {
    match dir & 3 {
        0 => ROAD_NE, // NE
        1 => ROAD_SE, // SE
        2 => ROAD_SW, // SW
        _ => ROAD_NW, // NW
    }
}

const fn reverse_diag(dir: u8) -> u8 {
    dir.wrapping_add(2) & 3
}

fn tile_add_diag(tile: TileCoord, dir: u8) -> TileCoord {
    let (dx, dy) = diag_dir_offset(dir);
    TileCoord::new(tile.x + dx, tile.y + dy)
}

fn diag_from_delta(dx: i32, dy: i32) -> u8 {
    match (dx.signum(), dy.signum()) {
        (-1, 0) => 0, // NE
        (0, 1) => 1,  // SE
        (1, 0) => 2,  // SW
        _ => 3,       // NW
    }
}

fn chance16(rng: &mut u32, a: u32, b: u32) -> bool {
    if b == 0 {
        return false;
    }
    *rng = rng.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
    (*rng >> 16) % b < a
}

/// Coloca una casa elegida por spec (API de tests / fundación).
pub fn place_house_with_spec(
    map: &mut Map,
    town: &mut Town,
    pos: TileCoord,
    ctx: TownExpandContext<'_>,
    seed: u32,
) -> Option<TileCoord> {
    try_build_town_house(map, town, pos, seed, ctx)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::station::{Station, StopKind};
    use crate::town::{TOWN_GROWTH_TICKS, Town, grow_town_if_served, update_town_radius};

    #[test]
    fn expand_places_road_then_house() {
        let mut map = Map::new_flat(32, 32, 1);
        let mut town = Town {
            id: 1,
            pos: TileCoord::new(16, 16),
            name: "Expand".into(),
            population: 40,
            is_growing: true,
            num_houses: 4,
            ..Default::default()
        };
        update_town_radius(&mut town);
        let ctx = TownExpandContext {
            climate: Climate::Temperate,
            calendar_year: 1980,
            house_catalog: &[],
            house_overrides: &[],
        };
        let mut dirty = Vec::new();
        assert!(grow_town(&mut map, &mut town, 1, ctx, &mut dirty));
        assert!(!dirty.is_empty());
    }

    #[test]
    fn coast_is_clearable_for_a_town_house_but_plain_water_is_not() {
        let mut map = Map::new_flat(3, 3, 0);
        let coast = TileCoord::new(1, 1);
        let water = TileCoord::new(2, 1);
        assert!(crate::map::make_shore_tile(&mut map, coast).is_ok());
        assert!(crate::map::make_water_tile(&mut map, water, crate::map::WaterClass::Sea).is_ok());

        assert!(can_build_house(&map, coast, false));
        assert!(!can_build_house(&map, water, false));
    }

    #[test]
    fn grow_town_if_served_places_tiles() {
        let mut map = Map::new_flat(32, 32, 1);
        assert!(place_town_road(
            &mut map,
            TileCoord::new(16, 16),
            ROAD_AXIS_X
        ));
        let mut towns = vec![Town {
            id: 1,
            pos: TileCoord::new(16, 16),
            name: "Grow".into(),
            population: 100,
            passengers_served: 20,
            is_growing: true,
            num_houses: 4,
            ..Default::default()
        }];
        update_town_radius(&mut towns[0]);
        let stations = vec![Station::new_with_kind(
            TileCoord::new(16, 17),
            StopKind::BusStop,
        )];
        let dirty = grow_town_if_served(&mut map, &[], &stations, &mut towns, TOWN_GROWTH_TICKS);
        assert!(!dirty.is_empty(), "debe ensuciar teselas");
        assert!(
            towns[0].population > 100
                || dirty
                    .iter()
                    .any(|&c| map.get_kind(c) == Some(TileKind::Road)
                        || map.get_kind(c) == Some(TileKind::House)),
            "crecimiento físico o población"
        );
    }

    #[test]
    fn grid_layout_rejects_house_on_road_line() {
        let town = Town {
            pos: TileCoord::new(10, 10),
            layout: TownLayout::Grid2x2,
            ..Default::default()
        };
        // gx % 3 == 0 → línea de carretera
        assert!(!town_layout_allows_house_here(
            &town,
            TileCoord::new(10, 11)
        ));
        assert!(town_layout_allows_house_here(&town, TileCoord::new(11, 11)));
    }

    #[test]
    fn house_choice_uses_zone_not_mod_110() {
        let mut map = Map::new_flat(24, 24, 1);
        let mut town = Town {
            id: 2,
            pos: TileCoord::new(12, 12),
            name: "Zone".into(),
            num_houses: 40,
            ..Default::default()
        };
        update_town_radius(&mut town);
        assert!(place_town_road(
            &mut map,
            TileCoord::new(12, 12),
            ROAD_AXIS_X
        ));
        let pos = TileCoord::new(12, 11);
        let ctx = TownExpandContext {
            climate: Climate::Temperate,
            calendar_year: 1980,
            house_catalog: &[],
            house_overrides: &[],
        };
        assert!(try_build_town_house(&mut map, &mut town, pos, 99, ctx).is_some());
        let tile = map.get(pos).unwrap();
        assert_eq!(u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8), town.id);
        let house_id = tile.m8 & 0x0FFF;
        let hs = HouseSpec::get(house_id).unwrap();
        assert!(hs.is_size_1x1());
        assert!(hs.min_year <= 1980);
    }

    #[test]
    fn town_growth_uses_canonical_diagonal_tile_offsets() {
        let origin = TileCoord::new(10, 10);
        assert_eq!(tile_add_diag(origin, 0), TileCoord::new(9, 10)); // NE
        assert_eq!(tile_add_diag(origin, 1), TileCoord::new(10, 11)); // SE
        assert_eq!(tile_add_diag(origin, 2), TileCoord::new(11, 10)); // SW
        assert_eq!(tile_add_diag(origin, 3), TileCoord::new(10, 9)); // NW

        assert_eq!(diag_from_delta(-1, 0), 0);
        assert_eq!(diag_from_delta(0, 1), 1);
        assert_eq!(diag_from_delta(1, 0), 2);
        assert_eq!(diag_from_delta(0, -1), 3);
    }

    #[test]
    fn completed_multitile_house_uses_make_town_house_id_order() {
        let mut map = Map::new_flat(8, 8, 1);
        let base = TileCoord::new(3, 3);
        assert!(place_house_footprint(
            &mut map,
            base,
            80,
            crate::house_spec::BUILDING_FLAG_SIZE_2X2,
        ));

        assert_eq!(map.get(base).unwrap().m8 & 0x0FFF, 80);
        assert_eq!(map.get(TileCoord::new(3, 4)).unwrap().m8 & 0x0FFF, 81);
        assert_eq!(map.get(TileCoord::new(4, 3)).unwrap().m8 & 0x0FFF, 82);
        assert_eq!(map.get(TileCoord::new(4, 4)).unwrap().m8 & 0x0FFF, 83);
    }

    #[test]
    fn two_tile_footprints_reposition_to_keep_the_selected_tile() {
        let town = Town {
            pos: TileCoord::new(6, 6),
            ..Default::default()
        };

        let mut across_x = Map::new_flat(12, 12, 1);
        across_x
            .set_kind(TileCoord::new(7, 6), TileKind::Water)
            .unwrap();
        assert_eq!(
            resolve_town_house_footprint(
                &across_x,
                &town,
                TileCoord::new(6, 6),
                crate::house_spec::BUILDING_FLAG_SIZE_2X1,
            ),
            Some(TileCoord::new(5, 6)),
        );

        let mut across_y = Map::new_flat(12, 12, 1);
        across_y
            .set_kind(TileCoord::new(6, 7), TileKind::Water)
            .unwrap();
        assert_eq!(
            resolve_town_house_footprint(
                &across_y,
                &town,
                TileCoord::new(6, 6),
                crate::house_spec::BUILDING_FLAG_SIZE_1X2,
            ),
            Some(TileCoord::new(6, 5)),
        );
    }

    #[test]
    fn two_by_two_footprint_checks_the_four_native_base_positions() {
        let town = Town {
            pos: TileCoord::new(4, 4),
            ..Default::default()
        };
        let mut map = Map::new_flat(12, 12, 1);
        // Rechazar, en este orden, base, +NW y +NW+NE. La cuarta posición
        // (base +NW+NE+SE) sigue libre y contiene la tesela original.
        for blocked in [
            TileCoord::new(5, 5),
            TileCoord::new(5, 3),
            TileCoord::new(3, 3),
        ] {
            map.set_kind(blocked, TileKind::Water).unwrap();
        }

        assert_eq!(
            resolve_town_house_footprint(
                &map,
                &town,
                TileCoord::new(4, 4),
                crate::house_spec::BUILDING_FLAG_SIZE_2X2,
            ),
            Some(TileCoord::new(3, 4)),
        );
    }

    #[test]
    fn multitile_footprints_require_same_max_z_and_honour_not_sloped() {
        let town = Town {
            pos: TileCoord::new(3, 3),
            ..Default::default()
        };
        let mut unequal_z = Map::new_flat(10, 10, 1);
        unequal_z
            .set_kind(TileCoord::new(2, 3), TileKind::Water)
            .unwrap();
        // Sólo la segunda tesela del intento +X termina en `max Z = 2`.
        unequal_z.set_height(TileCoord::new(5, 3), 2).unwrap();
        assert_eq!(
            resolve_town_house_footprint(
                &unequal_z,
                &town,
                TileCoord::new(3, 3),
                crate::house_spec::BUILDING_FLAG_SIZE_2X1,
            ),
            None,
        );

        let mut sloped = Map::new_flat(10, 10, 1);
        sloped.set_height(TileCoord::new(4, 3), 2).unwrap();
        assert_eq!(
            resolve_town_house_footprint(
                &sloped,
                &town,
                TileCoord::new(3, 3),
                crate::house_spec::BUILDING_FLAG_SIZE_1X1,
            ),
            Some(TileCoord::new(3, 3)),
        );
        assert_eq!(
            resolve_town_house_footprint(
                &sloped,
                &town,
                TileCoord::new(3, 3),
                crate::house_spec::BUILDING_FLAG_SIZE_1X1
                    | crate::house_spec::BUILDING_FLAG_NOT_SLOPED,
            ),
            None,
        );
    }

    #[test]
    fn two_by_two_layout_requires_a_complete_grid_cell() {
        let grid_2 = Town {
            pos: TileCoord::new(10, 10),
            layout: TownLayout::Grid2x2,
            ..Default::default()
        };
        assert!(town_layout_allows_2x2_house_here(
            &grid_2,
            TileCoord::new(8, 8)
        ));
        assert!(!town_layout_allows_2x2_house_here(
            &grid_2,
            TileCoord::new(9, 8)
        ));

        let grid_3 = Town {
            layout: TownLayout::Grid3x3,
            ..grid_2
        };
        assert!(town_layout_allows_2x2_house_here(
            &grid_3,
            TileCoord::new(8, 8)
        ));
        assert!(!town_layout_allows_2x2_house_here(
            &grid_3,
            TileCoord::new(9, 8)
        ));
    }

    fn test_multitile_house(flags: u8) -> HouseSpecDef {
        HouseSpecDef {
            id: crate::house_spec::NEW_HOUSE_OFFSET,
            local_id: 0,
            subst_id: 0,
            building_flags: flags,
            min_year: 0,
            max_year: crate::house_spec::HOUSE_YEAR_MAX,
            population: 12,
            mail_generation: 0,
            availability: crate::house_spec::DEFAULT_HOUSE_AVAILABILITY,
            probability: 1,
            override_id: None,
            callback_mask: 0,
            name: "multitile-test".into(),
            from_newgrf: true,
            grfid: 0x4D54_4553,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: None,
        }
    }

    #[test]
    fn runtime_town_build_repositions_and_materializes_multitile_house() {
        let catalog = [test_multitile_house(
            crate::house_spec::BUILDING_FLAG_SIZE_2X1,
        )];
        // Suprime el pool vanilla para que la prueba llegue al spec multitile.
        let overrides = [0_u16; crate::house_spec::NUM_HOUSES_VANILLA];
        let mut map = Map::new_flat(12, 12, 1);
        map.set_kind(TileCoord::new(5, 4), TileKind::Water).unwrap();
        let mut town = Town {
            id: 7,
            pos: TileCoord::new(4, 4),
            ..Default::default()
        };
        let ctx = TownExpandContext {
            climate: Climate::Temperate,
            calendar_year: 1980,
            house_catalog: &catalog,
            house_overrides: &overrides,
        };

        assert_eq!(
            place_house_with_spec(&mut map, &mut town, TileCoord::new(4, 4), ctx, 0),
            Some(TileCoord::new(3, 4)),
        );
        assert_eq!(map.get(TileCoord::new(3, 4)).unwrap().m8 & 0x0FFF, 110);
        assert_eq!(map.get(TileCoord::new(4, 4)).unwrap().m8 & 0x0FFF, 111);
        assert_eq!(town.num_houses, 2);
    }
}
