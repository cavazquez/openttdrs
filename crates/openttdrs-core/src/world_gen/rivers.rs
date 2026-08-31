//! Generación de ríos y costas.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::cargodist::parity::Randomizer;
use crate::company::OWNER_NONE_M1;
use crate::map::slope::{SLOPE_STEEP, complement_slope, inclined_slope_direction};
use crate::map::water_flood::{
    DIR_OFFSETS, FLOOD_FROM_DIRS, clear_neighbour_non_flooding_states, is_slope_one_corner_raised,
};
use crate::map::{
    Map, MapError, Tile, TileCoord, TileKind, WaterClass, is_river_tile, make_shore_tile,
    make_water_tile, set_water_class_m1, tile_slope_and_z, water_class_from_m1,
};

use super::config::{
    CLEAR_GROUND_GRASS, CLEAR_GROUND_ROUGH, PreserveRect, WorldGenConfig, clear_ground_m5,
};
use super::trees::place_tree_keep_density;

/// Flujos de río desde tierra alta hacia el mar / lagos (`WaterClass::River`).
pub(super) fn carve_rivers(
    map: &mut Map,
    config: &WorldGenConfig,
    map_w: i32,
    map_h: i32,
    preserve: &[PreserveRect],
    rng: &mut Randomizer,
) -> Result<(), MapError> {
    // `CreateRivers` calcula los pozos con `Map::ScaleBySize(4 << amount)`;
    // el conteo anterior dependía del área y producía cuatro veces más ríos
    // que OpenTTD en 64×64. Mantener la misma escala evita que el río altere
    // alturas que ya coinciden con TGP.
    let amount = u32::from(config.amount_of_rivers.min(3));
    let long_river_length = u32::from(config.min_river_length).saturating_mul(4);
    if amount == 0 {
        return Ok(());
    }
    let wells = super::population::scale_by_size(4u32 << amount, map_w as u32, map_h as u32);
    // C++ prueba primero `max(1, wells / 10)` ríos largos; el resto son
    // cortos. En 64² y cantidad predeterminada hay exactamente un pozo largo
    // (mínimo 16 × 4), no un río corto que pueda cruzar el mapa de cualquier
    // modo. Esta clasificación debe ocurrir antes de pintar una tesela.
    let long_wells = (wells / 10).max(1).min(wells);
    for well in 0..wells as usize {
        let is_long = well < long_wells as usize;
        let min_river_length =
            u32::from(config.min_river_length).saturating_mul(if is_long { 4 } else { 1 });
        // Cada intento C++ empieza con `RandomTile()` y busca el primer
        // manantial de su espiral 8×8. Esta fase debe usar el mapa y el
        // stream global reales: de ella depende `GenerateClearTile`.
        let tries = if is_long { 512 } else { 128 };
        for _ in 0..tries {
            let random = rng.next();
            let center = random_tile(random, map_w as u32, map_h as u32);
            let mut done = false;
            for spring in spiral_tiles(center, 8, map) {
                if !find_spring(map, spring, config, preserve) {
                    continue;
                }
                let (can_build, _) = flow_river(
                    map,
                    spring,
                    spring,
                    min_river_length,
                    long_river_length,
                    config.river_route_random,
                    config.climate,
                    config.seed,
                    rng,
                    preserve,
                    0,
                )?;
                done = can_build;
                // C++ rompe la espiral tras el primer `FindSpring`, aun si
                // ese flujo no termina construyéndose.
                break;
            }
            if done {
                break;
            }
        }
    }
    Ok(())
}

const RIVER_DIRS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];

/// `RandomTileSeed`: en los tamaños válidos de `OpenTTD` el índice se envuelve
/// con la máscara del mapa de potencia de dos.
fn random_tile(random: u32, map_w: u32, map_h: u32) -> TileCoord {
    let count = map_w.saturating_mul(map_h).max(1);
    let index = if map_w.is_power_of_two() && map_h.is_power_of_two() {
        random & count.saturating_sub(1)
    } else {
        random % count
    };
    TileCoord::new(
        i32::try_from(index % map_w.max(1)).unwrap_or(0),
        i32::try_from(index / map_w.max(1)).unwrap_or(0),
    )
}

/// Orden exacto de `SpiralTileSequence(center, diameter)` en el constructor
/// par usado por `CreateRivers`. El orden determina qué `FindSpring` gana.
fn spiral_tiles(center: TileCoord, diameter: u32, map: &Map) -> Vec<TileCoord> {
    let (width, height) = map.dimensions();
    if diameter == 0 || width == 0 || height == 0 {
        return Vec::new();
    }
    let max_radius = i32::try_from(diameter / 2).expect("river diameter fits i32");
    let odd_diameter = diameter % 2 == 1;
    let mut radius = 0i32;
    // `INVALID_DIAGDIR` es el estado especial que emite el centro de un
    // diámetro impar y, al avanzar, salta a la primera corona con
    // `TileIndexDiffCByDir(DIR_W) == (1, -1)`.
    let mut dir = if odd_diameter { usize::MAX } else { 0 };
    let mut position = i32::from(!odd_diameter);
    let mut x = if odd_diameter { center.x } else { center.x + 1 };
    let mut y = center.y;
    let mut tiles = Vec::with_capacity((diameter * diameter) as usize);

    while radius < max_radius || dir == usize::MAX {
        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
            tiles.push(TileCoord::new(x, y));
        }

        // `SpiralTileIterator::Increment`, incluido el caso central impar.
        if dir == usize::MAX {
            x += 1;
            y -= 1;
            dir = 0;
            position = 1 + radius * 2 + 1;
            continue;
        }

        let (dx, dy) = RIVER_DIRS[dir];
        x += dx;
        y += dy;
        position -= 1;
        if position > 0 {
            continue;
        }

        dir += 1;
        if dir == RIVER_DIRS.len() {
            // `TileIndexDiffCByDir(DIR_W)` avanza la corona siguiente.
            x += 1;
            y -= 1;
            radius += 1;
            dir = 0;
        }
        // `extent[dir]` vale 0 para el centro par y 1 para el hueco impar.
        position = i32::from(odd_diameter) + radius * 2 + 1;
    }
    tiles
}

/// `FindSpring` de `landscape.cpp` para los climas cuyo sustrato no necesita
/// consultar `TropicZone`.
fn find_spring(
    map: &Map,
    coord: TileCoord,
    config: &WorldGenConfig,
    preserve: &[PreserveRect],
) -> bool {
    if preserve.iter().any(|rect| rect.contains(coord.x, coord.y))
        || map.get(coord).is_some_and(is_plain_water_tile)
    {
        return false;
    }
    let Some((slope, reference_height)) = tile_slope_and_z(map, coord) else {
        return false;
    };
    if slope != 0 {
        return false;
    }
    // Tropical springs are restricted to the rainforest zone by
    // `FindSpring`; unlike the other climates this is not a blanket reject.
    // The zone is persisted in the low MAPT bits by `CreateDesertOrRainForest`.
    if config.climate == super::config::Climate::SubTropical
        && map.get(coord).is_none_or(|tile| tile.mapt & 0x03 != 2)
    {
        return false;
    }

    let mut higher_nearby = 0u32;
    for dx in -1..=1 {
        for dy in -1..=1 {
            if tile_add_wrap(map, coord, dx, dy)
                .and_then(|nearby| tile_max_z(map, nearby))
                .is_some_and(|height| height > reference_height)
            {
                higher_nearby += 1;
            }
        }
    }
    if higher_nearby < 4 {
        return false;
    }

    for dx in -16..=16 {
        for dy in -16..=16 {
            if tile_add_wrap(map, coord, dx, dy)
                .and_then(|nearby| tile_max_z(map, nearby))
                .is_some_and(|height| height > reference_height.saturating_add(2))
            {
                return false;
            }
        }
    }
    true
}

fn tile_max_z(map: &Map, coord: TileCoord) -> Option<u8> {
    let (slope, z) = tile_slope_and_z(map, coord)?;
    Some(z.saturating_add(if slope == 0 {
        0
    } else if slope & SLOPE_STEEP != 0 {
        2
    } else {
        1
    }))
}

fn is_inclined_slope(slope: u8) -> bool {
    matches!(slope, 3 | 6 | 9 | 12)
}

/// `IsWaterTile` de `OpenTTD` significa agua despejada (`WaterTileType::Clear`),
/// no cualquier tesela `MP_WATER`: una costa tiene el mismo tipo de mapa pero
/// debe poder convertirse en río durante `YapfBuildRiver`.
fn is_plain_water_tile(tile: Tile) -> bool {
    tile.kind == TileKind::Water && (tile.m5 >> 4) == 0
}

/// Traduce `TileAddWrap(tile, dx, dy)` para un mapa con `freeform_edges`
/// activo. `OpenTTD` no envuelve coordenadas: descarta el marco norte/oeste y
/// las teselas `MP_VOID` del borde sur/este. `FindSpring` usa esta variante
/// para no considerar alturas fuera del área interior.
fn tile_add_wrap(map: &Map, origin: TileCoord, dx: i32, dy: i32) -> Option<TileCoord> {
    let (width, height) = map.dimensions();
    let x = origin.x.saturating_add(dx);
    let y = origin.y.saturating_add(dy);
    let max_x = i32::try_from(width).ok()?.saturating_sub(1);
    let max_y = i32::try_from(height).ok()?.saturating_sub(1);
    if x <= 0 || y <= 0 || x >= max_x || y >= max_y {
        None
    } else {
        Some(TileCoord::new(x, y))
    }
}

fn river_flows_down(map: &Map, begin: TileCoord, end: TileCoord) -> bool {
    let Some((end_slope, end_height)) = tile_slope_and_z(map, end) else {
        return false;
    };
    if end_slope != 0 && !is_inclined_slope(end_slope) {
        return false;
    }
    let Some((begin_slope, begin_height)) = tile_slope_and_z(map, begin) else {
        return false;
    };
    if end_height > begin_height {
        return false;
    }
    (end_slope == begin_slope && end_height < begin_height) || end_slope == 0 || begin_slope == 0
}

/// Cuenta el parche de mar plano conectado a `start`, igual que
/// `CountConnectedSeaTiles`. Se conserva la lista porque `FlowRiver` aplana
/// cada tesela cuando el parche es un lago pequeño.
fn collect_connected_sea_tiles(
    map: &Map,
    start: TileCoord,
    limit: usize,
) -> (bool, Vec<TileCoord>) {
    fn visit(
        map: &Map,
        coord: TileCoord,
        width: u32,
        height: u32,
        limit: usize,
        seen: &mut HashSet<TileCoord>,
        sea: &mut Vec<TileCoord>,
    ) -> bool {
        let Some(tile) = map.get(coord) else {
            return false;
        };
        if !is_plain_water_tile(tile)
            || water_class_from_m1(tile.m1) != WaterClass::Sea
            || tile_slope_and_z(map, coord).is_none_or(|(slope, _)| slope != 0)
        {
            return false;
        }
        if coord.x <= 1
            || coord.y <= 1
            || coord.x >= width.saturating_sub(2) as i32
            || coord.y >= height.saturating_sub(2) as i32
        {
            return true;
        }
        if !seen.insert(coord) {
            return false;
        }
        sea.push(coord);
        // The native routine deliberately stops after counting limit + 1
        // tiles; the caller then accepts this as an ocean without retaining
        // or flattening the rest of the component.
        if sea.len() > limit {
            return false;
        }
        for (dx, dy) in RIVER_DIRS {
            let next = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(next).is_some()
                && !seen.contains(&next)
                && visit(map, next, width, height, limit, seen, sea)
            {
                return true;
            }
        }
        false
    }

    let (width, height) = map.dimensions();
    let mut seen = HashSet::new();
    let mut sea = Vec::new();
    let found_edge = visit(map, start, width, height, limit, &mut seen, &mut sea);
    if found_edge {
        return (true, sea);
    }
    (false, native_unordered_iteration(sea, width))
}

/// Orden de iteración de `std::unordered_set<TileIndex>` en la libstdc++ que
/// usa el oráculo `OpenTTD`. `CountConnectedSeaTiles` inserta por DFS y luego
/// `FlowRiver` recorre el contenedor; la lista enlazada global y los rehashes
/// de la política de primos son observables porque cada terraformación toca
/// esquinas compartidas. El modelo mantiene esos enlaces, incluido el nodo
/// centinela `before_begin`, para que el orden sea independiente de Rust.
fn native_unordered_iteration(coords: Vec<TileCoord>, map_width: u32) -> Vec<TileCoord> {
    const BEFORE_BEGIN: usize = usize::MAX;
    // Bucket counts observed from libstdc++'s `_Prime_rehash_policy` (load
    // factor 1.0), through every size reachable by a small-sea limit.
    const BUCKET_COUNTS: [usize; 10] = [1, 13, 29, 59, 127, 257, 541, 1109, 2357, 5087];

    #[derive(Clone, Copy)]
    struct Node {
        key: usize,
        next: Option<usize>,
    }

    fn next_bucket_count(minimum: usize) -> usize {
        BUCKET_COUNTS
            .iter()
            .copied()
            .find(|&count| count >= minimum)
            .unwrap_or_else(|| panic!("sea unordered_set exceeds native bucket model: {minimum}"))
    }

    let mut nodes: Vec<Node> = Vec::with_capacity(coords.len());
    let mut buckets: Vec<Option<usize>> = vec![None];
    let mut head: Option<usize> = None;
    let mut next_resize = 0usize;

    for coord in coords {
        let key = usize::try_from(coord.y)
            .expect("sea coordinate y is non-negative")
            .saturating_mul(map_width as usize)
            .saturating_add(usize::try_from(coord.x).expect("sea coordinate x is non-negative"));
        let element_count = nodes.len();
        if element_count + 1 > next_resize {
            let minimum_buckets =
                std::cmp::max(element_count + 1, if next_resize == 0 { 11 } else { 0 });
            if minimum_buckets >= buckets.len() {
                let desired = std::cmp::max(minimum_buckets + 1, buckets.len() * 2);
                let new_count = next_bucket_count(desired);
                let mut new_buckets = vec![None; new_count];
                let mut new_head = None;
                let mut current = head;
                while let Some(index) = current {
                    let old_next = nodes[index].next;
                    let bucket = nodes[index].key % new_count;
                    if new_buckets[bucket].is_none() {
                        nodes[index].next = new_head;
                        new_head = Some(index);
                        new_buckets[bucket] = Some(BEFORE_BEGIN);
                        if let Some(after) = nodes[index].next {
                            new_buckets[nodes[after].key % new_count] = Some(index);
                        }
                    } else {
                        let previous = new_buckets[bucket].expect("non-empty bucket has a link");
                        if previous == BEFORE_BEGIN {
                            nodes[index].next = new_head;
                            new_head = Some(index);
                        } else {
                            nodes[index].next = nodes[previous].next;
                            nodes[previous].next = Some(index);
                        }
                    }
                    current = old_next;
                }
                buckets = new_buckets;
                head = new_head;
                next_resize = new_count;
            } else {
                next_resize = buckets.len();
            }
        }

        let index = nodes.len();
        nodes.push(Node { key, next: head });
        let bucket = key % buckets.len();
        match buckets[bucket] {
            Some(BEFORE_BEGIN) => {
                head = Some(index);
            }
            Some(previous) => {
                nodes[index].next = nodes[previous].next;
                nodes[previous].next = Some(index);
            }
            None => {
                if let Some(old_head) = head {
                    let old_head_bucket = nodes[old_head].key % buckets.len();
                    buckets[old_head_bucket] = Some(index);
                }
                buckets[bucket] = Some(BEFORE_BEGIN);
                head = Some(index);
            }
        }
    }

    let mut ordered = Vec::with_capacity(nodes.len());
    let mut current = head;
    while let Some(index) = current {
        let x = nodes[index].key % map_width as usize;
        let y = nodes[index].key / map_width as usize;
        ordered.push(TileCoord::new(
            i32::try_from(x).expect("sea coordinate x fits i32"),
            i32::try_from(y).expect("sea coordinate y fits i32"),
        ));
        current = nodes[index].next;
    }
    ordered
}

/// `Chance16(a, b)` consume un único `Random()` y compara sus 16 bits bajos
/// con la fracción redondeada de `OpenTTD`. No es equivalente a
/// `RandomRange(b) < a`: ambas formas divergen en los bordes de la tabla.
fn chance16(rng: &mut Randomizer, numerator: u32, denominator: u32) -> bool {
    if denominator == 0 {
        return false;
    }
    let random_low = u64::from(rng.next() & 0xFFFF);
    let denominator = u64::from(denominator);
    ((random_low
        .saturating_mul(denominator)
        .saturating_add(denominator / 2))
        >> 16)
        < u64::from(numerator)
}

fn valid_river_terminus_tile(
    map: &Map,
    tile: TileCoord,
    height: u8,
    climate: super::config::Climate,
    _world_seed: u64,
) -> bool {
    let Some(entry) = map.get(tile) else {
        return false;
    };
    if entry.kind == TileKind::Void || entry.height != height {
        return false;
    }
    // `IsValidRiverTerminusTile` rejects tropical desert. The current
    // generator stores that zone in the low MAPT nibble; do not recompute a
    // coordinate hash here because the actual zone can have been changed by
    // `MakeRiverAndModifyDesertZoneAround` while a river is being built.
    if climate.uses_desert_patches() && entry.mapt & 0x03 == 1 {
        return false;
    }
    tile_slope_and_z(map, tile).is_some_and(|(slope, _)| slope == 0)
}

/// Port de `MakeLake`: el centro y cada expansión usan `MakeRiver`, por lo
/// que cada tesela nueva consume el byte aleatorio correspondiente.
fn make_lake(
    map: &mut Map,
    centre: TileCoord,
    height: u8,
    rng: &mut Randomizer,
    climate: super::config::Climate,
    world_seed: u64,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    if preserve
        .iter()
        .any(|rect| rect.contains(centre.x, centre.y))
    {
        return Ok(());
    }
    make_river_tile(map, centre, rng)?;
    let diameter = rng.random_range(8) + 3;
    for _ in 0..2 {
        for tile in spiral_tiles(centre, diameter, map) {
            if !valid_river_terminus_tile(map, tile, height, climate, world_seed) {
                continue;
            }
            let adjacent_water = RIVER_DIRS.iter().any(|&(dx, dy)| {
                map.get(TileCoord::new(tile.x + dx, tile.y + dy))
                    .is_some_and(is_plain_water_tile)
            });
            if adjacent_water {
                make_river_tile(map, tile, rng)?;
            }
        }
    }
    Ok(())
}

/// Port de `MakeWetlands`. En temperate conserva la misma escritura de suelo
/// rough y llama al constructor de árbol existente cuando el ajuste vanilla
/// `tree_placer=TP_IMPROVED` está activo en una partida nueva.
#[allow(clippy::too_many_arguments)]
fn make_wetlands(
    map: &mut Map,
    centre: TileCoord,
    height: u8,
    river_length: u32,
    rng: &mut Randomizer,
    climate: super::config::Climate,
    world_seed: u64,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    if preserve
        .iter()
        .any(|rect| rect.contains(centre.x, centre.y))
    {
        return Ok(());
    }
    make_river_tile(map, centre, rng)?;
    let diameter = river_length.max(16);
    let has_trees = chance16(rng, 1, 2);
    let radius = diameter / 2;
    let radius_squared = radius.saturating_mul(radius);
    for tile in spiral_tiles(centre, diameter, map) {
        if !valid_river_terminus_tile(map, tile, height, climate, world_seed) {
            continue;
        }
        let dx = tile.x.abs_diff(centre.x);
        let dy = tile.y.abs_diff(centre.y);
        if dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)) > radius_squared
            && chance16(rng, 3, 4)
        {
            continue;
        }
        if chance16(rng, 1, 3) {
            make_river_tile(map, tile, rng)?;
        } else if map.get_kind(tile) == Some(TileKind::Grass) {
            if let Some(mut entry) = map.get(tile) {
                entry.m5 = clear_ground_m5(CLEAR_GROUND_ROUGH, 3);
                map.set_tile(tile, entry)?;
            }
            if has_trees {
                let random = rng.next();
                let _ = place_tree_keep_density(map, tile, random, climate);
            }
        }
    }
    Ok(())
}

/// Selecciona y materializa un terminus no marino desde la cola BFS.
fn try_make_river_terminus(
    map: &mut Map,
    tile: TileCoord,
    begin: TileCoord,
    rng: &mut Randomizer,
    climate: super::config::Climate,
    world_seed: u64,
    preserve: &[PreserveRect],
) -> Result<bool, MapError> {
    let Some(height) = map.get(begin).map(|entry| entry.height) else {
        return Ok(false);
    };
    let valid = tile != begin && valid_river_terminus_tile(map, tile, height, climate, world_seed);
    if !valid {
        return Ok(false);
    }
    let lake = chance16(rng, 1, 3);
    if lake {
        make_lake(map, tile, height, rng, climate, world_seed, preserve)?;
    } else {
        let river_length = begin.x.abs_diff(tile.x) + begin.y.abs_diff(tile.y);
        make_wetlands(
            map,
            tile,
            height,
            river_length,
            rng,
            climate,
            world_seed,
            preserve,
        )?;
    }
    Ok(true)
}

/// Aplica la parte observable de los dos `CmdTerraformLand` que usa
/// `FlowRiver` para drenar un lago pequeño. Durante la generación mundial las
/// teselas de esos lagos están a cota cero, por lo que la orden de bajar falla
/// sin cambios y la orden complementaria eleva sus cuatro esquinas una unidad;
/// después `TerraformTile_Water` ejecuta `DoClearSquare`.
fn flatten_small_sea_tile(map: &mut Map, tile_coord: TileCoord) {
    if tile_slope_and_z(map, tile_coord).is_none() {
        return;
    }

    // `FlowRiver` first tries `SLOPE_ELEVATED` down. On a cota-zero lake the
    // command fails atomically, but once an earlier tile has raised the shared
    // corners, a flat cota-one tile can be lowered and raised again. Keeping
    // this transaction (rather than only raising) avoids creating cota two
    // plateaus and matches `CmdTerraformLand`'s command model.
    // Cada comando también visita todas las teselas que comparten una esquina
    // modificada. En particular, una costa vecina debe pasar por
    // `TerraformTile_Water`/`DoClearSquare`, no conservarse suspendida a una
    // nueva altura. `terraform_river_corners` conserva esa parte observable.
    let _ = terraform_river_corners(map, tile_coord, 0x0F, false);

    let Some((slope_after_lowering, _)) = tile_slope_and_z(map, tile_coord) else {
        return;
    };
    let raise_slope = complement_slope(slope_after_lowering);
    let _ = terraform_river_corners(map, tile_coord, raise_slope, true);
}

/// Explora y materializa `FlowRiver`, incluida la frontera de RNG, los
/// terminus y el trazado `YAPF`; RMAP-018 conserva la ampliación de matriz y
/// climas fuera de la cohorte validada.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn flow_river(
    map: &mut Map,
    spring: TileCoord,
    begin: TileCoord,
    min_river_length: u32,
    long_river_length: u32,
    route_random: u8,
    climate: super::config::Climate,
    world_seed: u64,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    depth: usize,
) -> Result<(bool, bool), MapError> {
    if depth > map.tiles().len() {
        return Ok((false, false));
    }
    if map.get(begin).is_some_and(is_plain_water_tile) {
        let result = (
            spring.x.abs_diff(begin.x) + spring.y.abs_diff(begin.y) > min_river_length,
            tile_slope_and_z(map, begin).is_some_and(|(_, z)| z == 0),
        );
        return Ok(result);
    }
    let Some((_, height_begin)) = tile_slope_and_z(map, begin) else {
        return Ok((false, false));
    };
    let mut marks = HashSet::from([begin]);
    let mut queue = vec![begin];
    let mut found = None;
    // El `for (size_t i = 0; i != queue.size(); i++)` nativo reevalúa
    // `queue.size()` después de cada expansión. Un rango Rust captura el
    // extremo al crearse y truncaba el BFS a sus primeros cuatro vecinos,
    // evitando alcanzar las cotas inferiores y desfasando el RNG.
    let mut index = 0;
    while index < queue.len() {
        let end = queue[index];
        index += 1;
        let Some((slope_end, height_end)) = tile_slope_and_z(map, end) else {
            continue;
        };
        let is_water = map.get(end).is_some_and(is_plain_water_tile);
        if slope_end == 0 && (height_end < height_begin || (height_end == height_begin && is_water))
        {
            if is_water
                && map.get(end).is_some_and(|tile| {
                    is_plain_water_tile(tile) && water_class_from_m1(tile.m1) == WaterClass::Sea
                })
            {
                let (width, height) = map.dimensions();
                let threshold =
                    ((2.0 * f64::from(width.saturating_mul(height)).sqrt()) as usize).min(1024);
                let (found_edge, sea_tiles) = collect_connected_sea_tiles(map, end, threshold);
                if found_edge || sea_tiles.len() > threshold {
                    found = Some(end);
                    break;
                }
                // A small inland sea is not a valid river terminus. The
                // native generator raises/clears it and keeps expanding
                // the same BFS, which is also important for the RNG and
                // for the eventual clear/town phases.
                for sea_tile in sea_tiles {
                    flatten_small_sea_tile(map, sea_tile);
                }
            } else {
                found = Some(end);
                break;
            }
        }
        for (dx, dy) in RIVER_DIRS {
            let next = TileCoord::new(end.x + dx, end.y + dy);
            if preserve.iter().any(|rect| rect.contains(next.x, next.y)) || marks.contains(&next) {
                continue;
            }
            if map
                .get_kind(next)
                .is_some_and(|kind| kind != TileKind::Void)
                && river_flows_down(map, end, next)
            {
                marks.insert(next);
                queue.push(next);
            }
        }
    }
    if let Some(end) = found {
        let (found, main_river) = flow_river(
            map,
            spring,
            end,
            min_river_length,
            long_river_length,
            route_random,
            climate,
            world_seed,
            rng,
            preserve,
            depth + 1,
        )?;
        if found {
            // OpenTTD builds the downstream segment first while unwinding
            // FlowRiver recursion. The route finder consumes the global
            // Randomizer once per candidate edge, so it must run before the
            // caller proceeds to the next river attempt.
            build_river_path(
                map,
                begin,
                end,
                spring,
                main_river,
                long_river_length,
                route_random,
                rng,
                preserve,
            )?;
        }
        return Ok((found, main_river));
    }
    if queue.len() > 32 {
        // `FlowRiver` intenta terminar en el N-ésimo tile considerado. La
        // selección y el `Chance16` se hacen aun cuando la candidata resulte
        // inválida, porque ambos sorteos pertenecen al stream compartido.
        let queue_index = rng.random_range(queue.len() as u32) as usize;
        let lake_centre = queue[queue_index];
        if spring.x.abs_diff(lake_centre.x) + spring.y.abs_diff(lake_centre.y) > min_river_length {
            let terminus = try_make_river_terminus(
                map,
                lake_centre,
                begin,
                rng,
                climate,
                world_seed,
                preserve,
            )?;
            if terminus {
                build_river_path(
                    map,
                    begin,
                    lake_centre,
                    spring,
                    false,
                    long_river_length,
                    route_random,
                    rng,
                    preserve,
                )?;
                return Ok((true, false));
            }
        }
    }
    Ok((false, false))
}

/// Port mínimo de `YapfBuildRiver` para el generador original.
///
/// El YAPF nativo usa A* con coste Manhattan más `RandomRange` por cada
/// arista candidata. Se conserva el orden de vecinos NE, SE, SW, NW y la
/// semántica de empate del heap binario para que la selección de ruta y el
/// stream RNG no dependan de `BinaryHeap` de la biblioteca estándar.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_river_path(
    map: &mut Map,
    start: TileCoord,
    end: TileCoord,
    spring: TileCoord,
    main_river: bool,
    long_river_length: u32,
    route_random: u8,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
) -> Result<bool, MapError> {
    // `CYapfBaseT` stops after the expert setting
    // `pf.yapf.max_search_nodes` (10000 by default).  The limit is part of
    // generation parity: without it a difficult downhill route can keep
    // consuming the global RNG and eventually paint a path that OpenTTD
    // abandons.
    const MAX_SEARCH_NODES: usize = 10_000;
    #[derive(Clone, Copy)]
    struct Node {
        coord: TileCoord,
        parent: Option<usize>,
        cost: u32,
        estimate: u32,
    }

    fn less(nodes: &[Node], left: usize, right: usize) -> bool {
        nodes[left].estimate < nodes[right].estimate
    }

    fn heapify_up(heap: &mut [usize], nodes: &[Node], mut gap: usize) {
        let item = heap[gap];
        while gap > 1 {
            let parent = gap / 2;
            if !less(nodes, item, heap[parent]) {
                break;
            }
            heap[gap] = heap[parent];
            gap = parent;
        }
        heap[gap] = item;
    }

    fn heap_position(heap: &[usize], item: usize) -> Option<usize> {
        heap.iter()
            .enumerate()
            .skip(1)
            .find_map(|(position, &index)| (index == item).then_some(position))
    }

    fn heap_remove(heap: &mut Vec<usize>, nodes: &[Node], position: usize) {
        let items = heap.len().saturating_sub(1);
        debug_assert!(position > 0 && position <= items);
        if position == items {
            heap.pop();
            return;
        }
        let last = heap.pop().expect("heap has an item");
        let mut gap = position;
        while gap > 1 {
            let parent = gap / 2;
            if !less(nodes, last, heap[parent]) {
                break;
            }
            heap[gap] = heap[parent];
            gap = parent;
        }
        let mut child = gap * 2;
        let remaining = heap.len().saturating_sub(1);
        while child <= remaining {
            if child < remaining && less(nodes, heap[child + 1], heap[child]) {
                child += 1;
            }
            if !less(nodes, heap[child], last) {
                break;
            }
            heap[gap] = heap[child];
            gap = child;
            child = gap * 2;
        }
        heap[gap] = last;
    }

    let mut nodes = vec![Node {
        coord: start,
        parent: None,
        cost: 0,
        estimate: 0,
    }];
    let mut heap = vec![0usize];
    heap.push(0);
    let mut open = std::collections::HashMap::from([(start, 0usize)]);
    let mut closed = HashSet::new();
    let mut destination = None;

    while !heap.is_empty() {
        // OpenTTD keeps the best node in the heap while PfFollowNode adds its
        // children, and only then moves it to the closed set. This detail is
        // observable because a back-edge is still looked up in the open set
        // and because insertion/removal changes the binary-heap tie order.
        let current_idx = heap[1];
        let current = nodes[current_idx];
        if current.coord == end {
            destination = Some(current_idx);
            break;
        }

        for (dx, dy) in RIVER_DIRS {
            let next = TileCoord::new(current.coord.x + dx, current.coord.y + dy);
            if preserve.iter().any(|rect| rect.contains(next.x, next.y))
                || map.get_kind(next).is_none_or(|kind| kind == TileKind::Void)
                || !river_flows_down(map, current.coord, next)
            {
                continue;
            }

            // `PfCalcCost` calls RandomRange even when the edge later turns
            // out to be a duplicate in the open/closed list.
            let edge_cost = rng.random_range(u32::from(route_random));
            let cost = current.cost.saturating_add(1).saturating_add(edge_cost);
            let estimate = cost.saturating_add(
                end.x
                    .abs_diff(next.x)
                    .saturating_add(end.y.abs_diff(next.y)),
            );
            let candidate = Node {
                coord: next,
                parent: Some(current_idx),
                cost,
                estimate,
            };

            if let Some(&open_idx) = open.get(&next) {
                if estimate < nodes[open_idx].estimate {
                    let position =
                        heap_position(&heap, open_idx).expect("open node is present in heap");
                    heap_remove(&mut heap, &nodes, position);
                    nodes[open_idx] = candidate;
                    heap.push(open_idx);
                    let position = heap.len() - 1;
                    heapify_up(&mut heap, &nodes, position);
                }
                continue;
            }
            if closed.contains(&next) {
                continue;
            }
            let node_idx = nodes.len();
            nodes.push(candidate);
            open.insert(next, node_idx);
            heap.push(node_idx);
            let position = heap.len() - 1;
            heapify_up(&mut heap, &nodes, position);
        }

        // OpenTTD checks the closed-node count immediately after following a
        // node and before moving that node from open to closed.  Preserve the
        // same off-by-one observable (the 10001st node may be expanded, but
        // is not closed) so both route failure and RNG consumption match.
        if closed.len() >= MAX_SEARCH_NODES {
            break;
        }

        let position = heap_position(&heap, current_idx).expect("expanded node is present in heap");
        heap_remove(&mut heap, &nodes, position);
        open.remove(&current.coord);
        closed.insert(current.coord);
    }

    let Some(destination) = destination else {
        return Ok(false);
    };

    let mut path = Vec::new();
    let mut cursor = Some(destination);
    while let Some(index) = cursor {
        let node = nodes[index];
        path.push(node.coord);
        cursor = node.parent;
    }
    path.reverse();
    // `YapfRiverBuilder` walks from the destination node back through its
    // parents, and `MakeRiver` consumes Random() in that order. Keep the
    // reverse traversal so MAP4 (`m4`) and the global stream stay native.
    for &tile in path.iter().rev() {
        if !map.get(tile).is_some_and(is_plain_water_tile) {
            make_river_tile(map, tile, rng)?;
        }
    }

    if main_river {
        widen_river_path(map, &path, spring, long_river_length, rng, preserve)?;
    }

    Ok(true)
}

/// `MakeRiverAndModifyDesertZoneAround`: `MakeWater` receives the low byte of
/// a fresh global `Random()` draw for every newly materialized river tile and
/// clears desert zones in the surrounding diameter-five spiral.
fn make_river_tile(map: &mut Map, coord: TileCoord, rng: &mut Randomizer) -> Result<(), MapError> {
    make_water_tile(map, coord, WaterClass::River)?;
    let mut tile = map.get(coord).ok_or(MapError::OutOfBounds)?;
    tile.m3hi = rng.next() as u8;
    map.set_tile(coord, tile)?;

    // `MakeRiverAndModifyDesertZoneAround` removes desert directly around
    // every river tile. This mutation is observable before the next spring
    // search, so it must happen in the same helper rather than only when a
    // lake or wetland is created.
    for nearby in spiral_tiles(coord, 5, map) {
        let Some(mut nearby_tile) = map.get(nearby) else {
            continue;
        };
        if nearby_tile.mapt & 0x03 == 1 {
            nearby_tile.mapt &= !0x03;
            map.set_tile(nearby, nearby_tile)?;
        }
    }
    Ok(())
}

fn is_slope_with_one_corner_raised(slope: u8) -> bool {
    slope & SLOPE_STEEP == 0 && matches!(slope & 0x0F, 1 | 2 | 4 | 8)
}

fn is_slope_with_three_corners_raised(slope: u8) -> bool {
    slope & SLOPE_STEEP == 0 && is_slope_with_one_corner_raised(complement_slope(slope))
}

/// Cambia las esquinas indicadas por una orden `CmdTerraformLand`. Las
/// validaciones se hacen antes de escribir para conservar la atomicidad que
/// necesita `RiverMakeWider` cuando una orilla está en cota cero.
fn terraform_river_corners(map: &mut Map, tile: TileCoord, corners: u8, raise: bool) -> bool {
    fn height_of(map: &Map, updates: &BTreeMap<TileCoord, i32>, coord: TileCoord) -> Option<i32> {
        updates
            .get(&coord)
            .copied()
            .or_else(|| map.get(coord).map(|tile| i32::from(tile.height)))
    }

    fn mark_dirty(map: &Map, dirty_tiles: &mut BTreeSet<TileCoord>, coord: TileCoord) {
        for (dx, dy) in [(0, 0), (-1, 0), (0, -1), (-1, -1)] {
            let dirty = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(dirty).is_some() {
                dirty_tiles.insert(dirty);
            }
        }
    }

    fn terraform_tile_height(
        map: &Map,
        updates: &mut BTreeMap<TileCoord, i32>,
        dirty_tiles: &mut BTreeSet<TileCoord>,
        coord: TileCoord,
        height: i32,
    ) -> bool {
        let Some(current) = height_of(map, updates, coord) else {
            return false;
        };
        // `TerraformTileHeight` rejects no-op and below-sea-level writes
        // before recording any part of the command.
        if height == current || !(0..=255).contains(&height) {
            return false;
        }
        mark_dirty(map, dirty_tiles, coord);
        updates.insert(coord, height);

        // OpenTTD recursively adjusts neighbouring corners until every
        // shared edge differs by at most one level. The adjustment is staged
        // and therefore remains atomic if any recursive write is invalid.
        for (dx, dy) in RIVER_DIRS {
            let neighbour = TileCoord::new(coord.x + dx, coord.y + dy);
            let Some(neighbour_height) = height_of(map, updates, neighbour) else {
                continue;
            };
            let difference = height - neighbour_height;
            if difference.abs() > 1 {
                let correction = difference + if difference < 0 { 1 } else { -1 };
                if !terraform_tile_height(
                    map,
                    updates,
                    dirty_tiles,
                    neighbour,
                    neighbour_height + correction,
                ) {
                    return false;
                }
            }
        }
        true
    }

    const OFFSETS: [(u8, i32, i32); 4] = [(1, 1, 0), (2, 1, 1), (4, 0, 1), (8, 0, 0)];
    let direction = if raise { 1_i32 } else { -1_i32 };
    let mut updates = BTreeMap::<TileCoord, i32>::new();
    let mut dirty_tiles = BTreeSet::new();
    for (bit, dx, dy) in OFFSETS {
        if corners & bit == 0 {
            continue;
        }
        let coord = TileCoord::new(tile.x + dx, tile.y + dy);
        let Some(height) = height_of(map, &updates, coord) else {
            // The native command skips slope bits whose corner is outside
            // the map edge (the corresponding `tile + offset < Map::Size()`
            // guard), which is also what freeform river widening observes.
            continue;
        };
        if !terraform_tile_height(
            map,
            &mut updates,
            &mut dirty_tiles,
            coord,
            height + direction,
        ) {
            return false;
        }
    }

    // `TerraformTile_Water`/`DoClearSquare` run during the command's model
    // pass, before staged heights are committed. Iterate the ordered set used
    // by OpenTTD's `std::set<TileIndex>` rather than a hash set.
    for dirty in dirty_tiles {
        if map
            .get(dirty)
            .is_some_and(|entry| matches!(entry.kind, TileKind::Grass | TileKind::Water))
        {
            clear_terraform_tile(map, dirty);
        }
    }
    for (coord, height) in updates {
        if map
            .set_height(coord, u8::try_from(height).unwrap_or(0))
            .is_err()
        {
            return false;
        }
    }
    true
}

/// `DoClearSquare` aplicado por `TerraformTile_Clear`/`TerraformTile_Water`
/// durante la ampliación de un río. La densidad de hierba inicial es tres y el
/// resto de planos se reinicia exactamente como `MakeClear(tile, CLEAR_GRASS,
/// 3)`. El callback de una tesela clear también se ejecuta al modificar una
/// esquina: limitarlo a agua dejaba rocas de `FixSlopes` que el original
/// limpia antes de `TileLoopClearAlps`.
fn clear_terraform_tile(map: &mut Map, coord: TileCoord) {
    let Some(mut tile) = map.get(coord) else {
        return;
    };
    tile.kind = TileKind::Grass;
    // `DoClearSquare` calls `MakeClear`, whose `SetTileType(MP_CLEAR)` only
    // replaces MAPT bits 4..7.  Tropical-zone/bridge-state bits therefore
    // survive when river widening terraforms a coast back into clear ground.
    tile.mapt &= 0x0F;
    tile.m5 = clear_ground_m5(CLEAR_GROUND_GRASS, 3);
    tile.m1 = OWNER_NONE_M1;
    tile.m2 = 0;
    tile.m2_hi = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    let _ = map.set_tile(coord, tile);
    clear_neighbour_non_flooding_states(map, coord);
}

/// Subconjunto ejecutable de `RiverMakeWider`. El caso plano (el dominante en
/// TGP) conserva exactamente la expansión espiral y los sorteos de `MakeRiver`;
/// las ramas de pendiente aplican las mismas salvaguardas de dirección y
/// terraformación antes de materializar agua.
#[allow(clippy::similar_names, clippy::too_many_lines)]
fn river_make_wider(
    map: &mut Map,
    tile: TileCoord,
    origin_tile: TileCoord,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let Some(entry) = map.get(tile) else {
        return Ok(());
    };
    // `RiverMakeWider` empieza con `IsValidTile`: el borde libre de OpenTTD
    // está representado en el port como `MP_VOID`, no como una coordenada
    // ausente. Nunca se puede convertir ese marco en río ni consumir su byte
    // aleatorio; hacerlo desalineaba todos los pozos posteriores.
    if preserve.iter().any(|rect| rect.contains(tile.x, tile.y))
        || entry.kind == TileKind::Void
        || is_plain_water_tile(entry)
    {
        return Ok(());
    }
    let Some((mut cur_slope, _)) = tile_slope_and_z(map, tile) else {
        return Ok(());
    };
    let Some((origin_slope, _)) = tile_slope_and_z(map, origin_tile) else {
        return Ok(());
    };
    let Some(tile_max) = tile_max_z(map, tile) else {
        return Ok(());
    };
    let Some(origin_max) = tile_max_z(map, origin_tile) else {
        return Ok(());
    };
    if tile_max == 0 || tile_max > origin_max {
        return Ok(());
    }

    let mut desired_slope = origin_slope;
    if cur_slope != 0 && !is_inclined_slope(cur_slope) {
        if cur_slope & SLOPE_STEEP != 0 {
            return Ok(());
        }
        let mut flat_river_found = false;
        let mut sloped_river_found = false;
        for direction in 0..4u8 {
            let (dx, dy) = crate::map::diag_dir_offset(direction);
            let other = TileCoord::new(tile.x + dx, tile.y + dy);
            let Some(other_tile) = map.get(other) else {
                continue;
            };
            if !is_river_tile(other_tile) {
                continue;
            }
            let Some((other_slope, _)) = tile_slope_and_z(map, other) else {
                continue;
            };
            if is_inclined_slope(other_slope)
                && tile_max_z(map, tile) == tile_max_z(map, other)
                && inclined_slope_direction(other_slope).is_some_and(|other_direction| {
                    other_direction == (direction + 1) % 4 || other_direction == (direction + 3) % 4
                })
            {
                desired_slope = other_slope;
                sloped_river_found = true;
                break;
            }
            if other_slope == 0 {
                flat_river_found = true;
            }
        }
        if !sloped_river_found && !flat_river_found {
            return Ok(());
        }
        if !sloped_river_found {
            desired_slope = 0;
        }

        if desired_slope == 0 && is_slope_with_three_corners_raised(cur_slope) {
            let _ = terraform_river_corners(map, tile, complement_slope(cur_slope), true);
        } else if is_inclined_slope(desired_slope) {
            let river_direction =
                inclined_slope_direction(desired_slope).map_or(0, |direction| (direction + 2) % 4);
            for diff in [1u8, 3u8] {
                let direction = (river_direction + diff) % 4;
                let (dx, dy) = crate::map::diag_dir_offset(direction);
                let other = TileCoord::new(tile.x + dx, tile.y + dy);
                if map.get(other).is_some_and(is_plain_water_tile)
                    && map.get(other).is_some_and(is_river_tile)
                    && tile_slope_and_z(map, other).is_some_and(|(slope, _)| slope == 0)
                {
                    return Ok(());
                }
            }
            let mut to_change = cur_slope ^ desired_slope;
            if !is_slope_with_one_corner_raised(cur_slope) {
                to_change &= complement_slope(desired_slope);
                let _ = terraform_river_corners(map, tile, to_change, false);
            }
            cur_slope = tile_slope_and_z(map, tile).map_or(cur_slope, |(slope, _)| slope);
            if cur_slope != desired_slope && is_slope_with_one_corner_raised(cur_slope) {
                let _ = terraform_river_corners(map, tile, cur_slope ^ desired_slope, true);
            }
        }
        cur_slope = tile_slope_and_z(map, tile).map_or(cur_slope, |(slope, _)| slope);
    }

    if is_inclined_slope(cur_slope) {
        let direction = inclined_slope_direction(cur_slope).unwrap_or(0);
        let (upstream_dx, upstream_dy) = crate::map::diag_dir_offset(direction);
        let (downstream_dx, downstream_dy) = crate::map::diag_dir_offset((direction + 2) % 4);
        let upstream = TileCoord::new(tile.x + upstream_dx, tile.y + upstream_dy);
        let downstream = TileCoord::new(tile.x + downstream_dx, tile.y + downstream_dy);
        // `IsValidTile` excludes the freeform-edge MP_VOID frame.  A void
        // neighbour is still present in the map vector, so checking only
        // `Option::is_none` would incorrectly create river tiles outside the
        // playable area and consume an extra global RNG draw.
        if map
            .get(upstream)
            .is_none_or(|tile| tile.kind == TileKind::Void)
            || map
                .get(downstream)
                .is_none_or(|tile| tile.kind == TileKind::Void)
        {
            return Ok(());
        }
        let downstream_is_ocean = tile_slope_and_z(map, downstream).is_some_and(|(slope, z)| {
            z == 0 && (slope == 0 || is_slope_with_one_corner_raised(slope))
        });
        if !map.get(downstream).is_some_and(is_plain_water_tile) && !downstream_is_ocean {
            if tile_slope_and_z(map, downstream).is_none_or(|(slope, _)| slope != 0) {
                return Ok(());
            }
            make_river_tile(map, downstream, rng)?;
        }
        if !map.get(upstream).is_some_and(is_plain_water_tile) {
            if tile_slope_and_z(map, upstream).is_none_or(|(slope, _)| slope != 0) {
                return Ok(());
            }
            make_river_tile(map, upstream, rng)?;
        }
    }
    if cur_slope == desired_slope && !map.get(tile).is_some_and(is_plain_water_tile) {
        make_river_tile(map, tile, rng)?;
    }
    Ok(())
}

fn widen_river_path(
    map: &mut Map,
    path: &[TileCoord],
    spring: TileCoord,
    long_river_length: u32,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let divisor = (long_river_length / 3).max(1);
    for &center in path.iter().rev() {
        let distance = spring.x.abs_diff(center.x) + spring.y.abs_diff(center.y);
        let diameter = 3u32.min(distance / divisor + 1);
        if diameter <= 1 {
            continue;
        }
        for tile in spiral_tiles(center, diameter, map) {
            river_make_wider(map, tile, center, rng, preserve)?;
        }
    }
    Ok(())
}

/// Port de `ConvertGroundTilesIntoWaterTiles` de `water_cmd.cpp`.
///
/// El generador original no transforma todo terreno a altura cero en agua:
/// las pendientes complejas se conservan como `MP_CLEAR` salvo que alguna de
/// sus direcciones de inundación llegue a una pendiente plana, de una esquina
/// elevada o a `MP_VOID`. Esta distinción determina tanto el tipo de tesela
/// como `MAP1`/`MAP5`, y por extensión el comportamiento posterior de
/// `TileLoop_Water`.
pub(super) fn convert_ground_tiles_into_water_tiles(
    map: &mut Map,
    preserve: &[PreserveRect],
) -> Result<(), MapError> {
    let (width, height) = map.dimensions();
    let width = i32::try_from(width).expect("map width fits i32");
    let height = i32::try_from(height).expect("map height fits i32");

    for y in 0..height {
        for x in 0..width {
            if preserve.iter().any(|rect| rect.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            if map.get_kind(c) != Some(TileKind::Grass) {
                continue;
            }
            let Some((slope, z)) = tile_slope_and_z(map, c) else {
                continue;
            };
            if z != 0 {
                continue;
            }

            match slope {
                0 => make_water_tile(map, c, WaterClass::Sea)?,
                1 | 2 | 4 | 8 => make_shore_tile(map, c)?,
                _ => {
                    let slope_index = usize::from((slope & !SLOPE_STEEP) & 0x0F);
                    let mut directions = FLOOD_FROM_DIRS[slope_index];
                    let mut make_shore = false;
                    while directions != 0 {
                        let direction = directions.trailing_zeros() as usize;
                        directions &= directions - 1;
                        let (dx, dy) = DIR_OFFSETS[direction];
                        let dest = TileCoord::new(c.x + dx, c.y + dy);
                        if map.get(dest).is_none() {
                            continue;
                        }
                        let Some((dest_slope, _)) = tile_slope_and_z(map, dest) else {
                            continue;
                        };
                        let dest_slope = dest_slope & !SLOPE_STEEP;
                        if dest_slope == 0
                            || is_slope_one_corner_raised(dest_slope)
                            || map.get_kind(dest) == Some(TileKind::Void)
                        {
                            make_shore = true;
                            break;
                        }
                    }
                    if make_shore {
                        make_shore_tile(map, c)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Heurística heredada para heightmaps externos.
///
/// El pipeline procedural usa [`convert_ground_tiles_into_water_tiles`], que
/// replica el contrato por pendiente del original. Los heightmaps ya llegan
/// con el agua materializada y conservan esta pasada visual independiente.
pub(super) fn mark_water_coasts(
    map: &mut Map,
    mw: i32,
    mh: i32,
    _sea_level: u8,
    preserve: &[PreserveRect],
) {
    const WATER_COAST_M5: u8 = 0x10;
    for y in 0..mh {
        for x in 0..mw {
            if preserve.iter().any(|r| r.contains(x, y)) {
                continue;
            }
            let c = TileCoord::new(x, y);
            if map.get_kind(c) != Some(TileKind::Water) {
                continue;
            }
            let adjacent_land = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .into_iter()
                .any(|(dx, dy)| {
                    let nc = TileCoord::new(x + dx, y + dy);
                    map.get_kind(nc)
                        .is_some_and(|k| k != TileKind::Water && k != TileKind::Void)
                });
            if adjacent_land {
                let _ = map.set_mapt_m5(c, 0x60, WATER_COAST_M5);
                if let Some(tile) = map.get(c) {
                    let _ = map.set_m1(c, set_water_class_m1(tile.m1, WaterClass::Sea));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargodist::parity::Randomizer;
    use crate::company::{OWNER_NONE_M1, OWNER_WATER_M1};
    use crate::map::is_river_tile;
    use crate::world_gen::{CLEAR_GROUND_DESERT, CLEAR_GROUND_ROCKY, Climate};

    #[test]
    fn converter_makes_flat_ground_sea_and_one_corner_slope_shore() {
        let mut map = Map::new_flat(8, 8, 0);
        let flat = TileCoord::new(2, 2);
        let shore = TileCoord::new(5, 2);
        // `hwest` de `shore`: slope W (una única esquina elevada).
        map.set_height(TileCoord::new(shore.x + 1, shore.y), 1)
            .expect("raise west corner");

        convert_ground_tiles_into_water_tiles(&mut map, &[]).expect("convert ground");

        let sea = map.get(flat).expect("flat sea");
        assert_eq!(sea.kind, TileKind::Water);
        assert_eq!(sea.m1, OWNER_WATER_M1);
        assert_eq!(sea.m5, 0);

        let coast = map.get(shore).expect("shore");
        assert_eq!(coast.kind, TileKind::Water);
        assert_eq!(coast.m1, OWNER_WATER_M1);
        assert_eq!(coast.m5, 0x10);
    }

    #[test]
    fn converter_keeps_slope_without_flood_directions_as_clear_ground() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        // `SLOPE_EW` (bits 1|4) no tiene entradas en `_flood_from_dirs`.
        map.set_height(TileCoord::new(c.x + 1, c.y), 1)
            .expect("raise west corner");
        map.set_height(TileCoord::new(c.x, c.y + 1), 1)
            .expect("raise east corner");
        map.set_m1(c, OWNER_NONE_M1).expect("clear owner");

        convert_ground_tiles_into_water_tiles(&mut map, &[]).expect("convert ground");

        let clear = map.get(c).expect("clear slope");
        assert_eq!(clear.kind, TileKind::Grass);
        assert_eq!(clear.m1, OWNER_NONE_M1);
    }

    #[test]
    fn tropical_springs_require_rainforest_but_are_not_globally_rejected() {
        let centre = TileCoord::new(32, 32);
        let mut map = Map::new_flat(64, 64, 1);
        // Keep the centre flat while putting four higher neighbours around
        // it, satisfying FindSpring's local hill test without introducing a
        // height more than two levels above the reference.
        for raised in [
            TileCoord::new(31, 32),
            TileCoord::new(34, 32),
            TileCoord::new(32, 31),
            TileCoord::new(32, 34),
        ] {
            map.set_height(raised, 2).expect("spring neighbour");
        }
        let config = WorldGenConfig {
            climate: Climate::SubTropical,
            ..WorldGenConfig::default()
        };

        assert!(!find_spring(&map, centre, &config, &[]));
        map.set_mapt_m5(centre, 0x02, 0).expect("rainforest zone");
        assert!(find_spring(&map, centre, &config, &[]));
    }

    #[test]
    fn tropical_river_terminus_uses_materialized_desert_zone() {
        let tile = TileCoord::new(4, 4);
        let mut map = Map::new_flat(16, 16, 1);
        map.set_mapt_m5(tile, 0x01, 0).expect("desert zone");
        assert!(!valid_river_terminus_tile(
            &map,
            tile,
            1,
            Climate::SubTropical,
            0
        ));
        map.set_mapt_m5(tile, 0x02, 0).expect("rainforest zone");
        assert!(valid_river_terminus_tile(
            &map,
            tile,
            1,
            Climate::SubTropical,
            0
        ));
    }

    #[test]
    fn river_tile_clears_desert_within_native_spiral_diameter() {
        let centre = TileCoord::new(8, 8);
        let nearby = TileCoord::new(10, 8);
        let outside = TileCoord::new(11, 8);
        let mut map = Map::new_flat(24, 24, 1);
        for tile in [centre, nearby, outside] {
            map.set_mapt_m5(tile, 0x01, clear_ground_m5(CLEAR_GROUND_DESERT, 3))
                .expect("desert zone");
        }

        let mut rng = Randomizer::new(7);
        make_river_tile(&mut map, centre, &mut rng).expect("river tile");

        assert_eq!(map.get(centre).expect("river tile").mapt & 0x03, 0);
        assert_eq!(map.get(nearby).expect("nearby tile").mapt & 0x03, 0);
        assert_eq!(map.get(outside).expect("outside tile").mapt & 0x03, 1);
    }

    #[test]
    fn small_sea_flattening_raises_and_clears_the_connected_patch() {
        let mut map = Map::new_flat(8, 8, 0);
        let patch = [TileCoord::new(3, 3), TileCoord::new(4, 3)];
        for tile in patch {
            make_water_tile(&mut map, tile, WaterClass::Sea).expect("make lake tile");
        }

        for tile in patch {
            flatten_small_sea_tile(&mut map, tile);
        }

        for tile in patch {
            let clear = map.get(tile).expect("cleared lake tile");
            assert_eq!(clear.kind, TileKind::Grass);
            assert_eq!(clear.height, 1);
            assert_eq!(clear.m1, OWNER_NONE_M1);
            assert_eq!(clear.m5, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
        }
        // Both tiles share the two raised corners. The second command must
        // lower/re-raise a flat cota-one tile instead of creating cota two.
        assert_eq!(
            map.get(TileCoord::new(4, 3)).expect("shared corner").height,
            1
        );
        assert_eq!(
            map.get(TileCoord::new(4, 4)).expect("shared corner").height,
            1
        );
    }

    #[test]
    fn small_sea_flattening_clears_coasts_sharing_raised_corners() {
        let mut map = Map::new_flat(8, 8, 0);
        let lake = TileCoord::new(3, 3);
        let adjacent_coast = TileCoord::new(2, 3);
        make_water_tile(&mut map, lake, WaterClass::Sea).expect("make inland sea tile");
        make_shore_tile(&mut map, adjacent_coast).expect("make neighbouring coast");

        flatten_small_sea_tile(&mut map, lake);

        for tile in [lake, adjacent_coast] {
            let clear = map.get(tile).expect("terraform callback clears water");
            assert_eq!(clear.kind, TileKind::Grass, "tile {tile:?}");
            assert_eq!(clear.m1, OWNER_NONE_M1, "tile {tile:?}");
            assert_eq!(clear.m5, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
        }
    }

    #[test]
    fn small_sea_flattening_clears_rocky_ground_sharing_raised_corners() {
        let mut map = Map::new_flat(8, 8, 0);
        let lake = TileCoord::new(3, 3);
        let adjacent_rock = TileCoord::new(2, 3);
        make_water_tile(&mut map, lake, WaterClass::Sea).expect("make inland sea tile");
        map.set_mapt_m5(adjacent_rock, 0, clear_ground_m5(CLEAR_GROUND_ROCKY, 3))
            .expect("make neighbouring ground rocky");

        flatten_small_sea_tile(&mut map, lake);

        let clear = map.get(adjacent_rock).expect("cleared neighbouring ground");
        assert_eq!(clear.kind, TileKind::Grass);
        assert_eq!(clear.height, 0);
        assert_eq!(clear.m5, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
    }

    fn descending_river_test_map(width: i32, sea_x: i32) -> Map {
        let mut map = Map::new_flat(width as u32, 8, 0);
        for x in 0..width {
            for y in 0..8 {
                map.set_height(TileCoord::new(x, y), (96 - x).try_into().expect("height"))
                    .expect("set height");
            }
        }
        // `FlowRiver` accepts only a flat water terminus. Flatten all four
        // corners of the synthetic sea tile after creating the descending
        // test slope; this mirrors a real sea tile without relying on the
        // old heuristic route builder.
        for corner in [
            TileCoord::new(sea_x, 3),
            TileCoord::new(sea_x + 1, 3),
            TileCoord::new(sea_x, 4),
            TileCoord::new(sea_x + 1, 4),
        ] {
            map.set_height(corner, 1).expect("flatten sea corner");
        }
        make_water_tile(&mut map, TileCoord::new(sea_x, 3), WaterClass::Sea)
            .expect("make terminal sea");
        map
    }

    #[test]
    fn long_river_rejects_a_route_shorter_than_its_manhattan_minimum() {
        let mut map = descending_river_test_map(16, 9);
        let start = TileCoord::new(1, 3);
        let mut rng = Randomizer::new(7);

        let built = flow_river(
            &mut map,
            start,
            start,
            16,
            64,
            5,
            Climate::Temperate,
            0,
            &mut rng,
            &[],
            0,
        )
        .expect("flow river")
        .0;

        assert!(!built);
        assert!(map.tiles().iter().all(|tile| !is_river_tile(*tile)));
    }

    #[test]
    fn accepted_route_is_painted_only_after_reaching_water_and_minimum_length() {
        let mut map = Map::new_flat(8, 8, 0);
        let start = TileCoord::new(3, 3);
        let end = TileCoord::new(4, 3);
        make_water_tile(&mut map, end, WaterClass::Sea).expect("make terminal sea");
        let mut rng = Randomizer::new(7);

        let built = build_river_path(&mut map, start, end, start, false, 64, 5, &mut rng, &[])
            .expect("build river path");

        assert!(built);
        assert!(map.tiles().iter().any(|tile| is_river_tile(*tile)));
    }

    #[test]
    fn inland_sea_iteration_matches_libstdcxx_rehash_order() {
        let coords = (0..30).map(|x| TileCoord::new(x, 0)).collect::<Vec<_>>();
        let actual = native_unordered_iteration(coords, 1024);
        let mut expected = vec![TileCoord::new(29, 0)];
        expected.extend((0..=12).rev().map(|x| TileCoord::new(x, 0)));
        expected.extend((13..=28).map(|x| TileCoord::new(x, 0)));
        assert_eq!(actual, expected);
    }

    #[test]
    fn terraform_propagates_corner_height_differences_recursively() {
        let mut map = Map::new_flat(8, 8, 0);
        let tile = TileCoord::new(3, 3);
        map.set_height(TileCoord::new(5, 3), 4)
            .expect("raise neighbouring corner");

        assert!(terraform_river_corners(&mut map, tile, 0x01, true));
        assert_eq!(
            map.get(TileCoord::new(4, 3)).expect("raised corner").height,
            1
        );
        assert_eq!(
            map.get(TileCoord::new(5, 3))
                .expect("recursively adjusted corner")
                .height,
            2
        );
        assert!(map.tiles().iter().all(|entry| entry.height <= 4));
    }

    #[test]
    fn river_widening_never_materializes_the_freeform_void_border() {
        let mut map = Map::new_flat(8, 8, 1);
        let border = TileCoord::new(0, 3);
        let origin = TileCoord::new(1, 3);
        let mut entry = map.get(border).expect("freeform border tile");
        entry.kind = TileKind::Void;
        map.set_tile(border, entry)
            .expect("mark freeform border void");
        let mut rng = Randomizer::new(0x1234_5678);
        let before = rng;

        river_make_wider(&mut map, border, origin, &mut rng, &[])
            .expect("skip freeform void border");

        assert_eq!(
            map.get(border).expect("border remains").kind,
            TileKind::Void
        );
        assert_eq!(rng, before, "void border must not consume Random()");
    }

    #[test]
    fn one_well_is_a_long_attempt_like_create_rivers() {
        let wells = 1u32;
        let long_wells = (wells / 10).max(1).min(wells);
        assert_eq!(long_wells, 1);
        assert_eq!(u32::from(16u8).saturating_mul(4), 64);
    }

    #[test]
    fn unbuilt_long_well_still_consumes_every_random_tile_attempt() {
        let mut map = Map::new_flat(64, 64, 0);
        let config = WorldGenConfig {
            amount_of_rivers: 2,
            ..WorldGenConfig::default()
        };
        let mut actual = Randomizer::new(0xC0FF_EE00);
        carve_rivers(&mut map, &config, 64, 64, &[], &mut actual).expect("carve rivers");

        let mut expected = Randomizer::new(0xC0FF_EE00);
        for _ in 0..512 {
            let _ = expected.next();
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn spiral_sequence_matches_openttd_odd_diameter() {
        let map = Map::new_flat(64, 64, 0);
        let actual = spiral_tiles(TileCoord::new(1, 1), 5, &map);
        let expected = [
            (1, 1),
            (2, 0),
            (1, 0),
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 2),
            (2, 2),
            (2, 1),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (3, 2),
            (3, 1),
            (3, 0),
        ];
        let expected = expected
            .into_iter()
            .map(|(x, y)| TileCoord::new(x, y))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn spiral_sequence_matches_openttd_even_diameter() {
        let map = Map::new_flat(64, 64, 0);
        let actual = spiral_tiles(TileCoord::new(1, 1), 6, &map);
        let expected = [
            (2, 1),
            (1, 1),
            (1, 2),
            (2, 2),
            (3, 0),
            (2, 0),
            (1, 0),
            (0, 0),
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 3),
            (2, 3),
            (3, 3),
            (3, 2),
            (3, 1),
            (0, 4),
            (1, 4),
            (2, 4),
            (3, 4),
            (4, 4),
            (4, 3),
            (4, 2),
            (4, 1),
            (4, 0),
        ];
        let expected = expected
            .into_iter()
            .map(|(x, y)| TileCoord::new(x, y))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
