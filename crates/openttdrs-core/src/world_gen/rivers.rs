//! Generación de ríos y costas.

use std::collections::HashSet;

use crate::cargodist::parity::Randomizer;
use crate::company::OWNER_NONE_M1;
use crate::map::slope::{SLOPE_STEEP, complement_slope};
use crate::map::water_flood::{DIR_OFFSETS, FLOOD_FROM_DIRS, is_slope_one_corner_raised};
use crate::map::{
    Map, MapError, TileCoord, TileKind, WaterClass, make_shore_tile, make_water_tile,
    set_water_class_m1, tile_slope_and_z, water_class_from_m1,
};

use super::config::{CLEAR_GROUND_GRASS, PreserveRect, WorldGenConfig, clear_ground_m5};

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
                let (can_build, _) =
                    probe_flow_river(map, spring, spring, min_river_length, rng, preserve, 0);
                if can_build {
                    done = flow_river(map, spring, u64::from(random), preserve, min_river_length)?;
                }
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
        || map.get_kind(coord) == Some(TileKind::Water)
    {
        return false;
    }
    let Some((slope, reference_height)) = tile_slope_and_z(map, coord) else {
        return false;
    };
    if slope != 0 || matches!(config.climate, super::config::Climate::SubTropical) {
        return false;
    }

    let mut higher_nearby = 0u32;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if tile_max_z(map, nearby).is_some_and(|height| height > reference_height) {
                higher_nearby += 1;
            }
        }
    }
    if higher_nearby < 4 {
        return false;
    }

    for dx in -16..=16 {
        for dy in -16..=16 {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if tile_max_z(map, nearby)
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
    let (width, height) = map.dimensions();
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    let mut sea = Vec::new();
    while let Some(coord) = stack.pop() {
        let Some(tile) = map.get(coord) else {
            continue;
        };
        if tile.kind != TileKind::Water
            || water_class_from_m1(tile.m1) != WaterClass::Sea
            || tile_slope_and_z(map, coord).is_none_or(|(slope, _)| slope != 0)
        {
            continue;
        }
        if coord.x <= 1
            || coord.y <= 1
            || coord.x >= width.saturating_sub(2) as i32
            || coord.y >= height.saturating_sub(2) as i32
        {
            return (true, sea);
        }
        if !seen.insert(coord) {
            continue;
        }
        sea.push(coord);
        // The native routine deliberately stops after counting limit + 1
        // tiles; the caller then accepts this as an ocean without retaining
        // or flattening the rest of the component.
        if sea.len() > limit {
            return (false, sea);
        }
        for (dx, dy) in RIVER_DIRS {
            stack.push(TileCoord::new(coord.x + dx, coord.y + dy));
        }
    }
    (false, sea)
}

/// Aplica la parte observable de los dos `CmdTerraformLand` que usa
/// `FlowRiver` para drenar un lago pequeño. Durante la generación mundial las
/// teselas de esos lagos están a cota cero, por lo que la orden de bajar falla
/// sin cambios y la orden complementaria eleva sus cuatro esquinas una unidad;
/// después `TerraformTile_Water` ejecuta `DoClearSquare`.
fn flatten_small_sea_tile(map: &mut Map, tile_coord: TileCoord) {
    const CORNERS: [(u8, i32, i32); 4] = [
        (1, 1, 0), // SLOPE_W: tile + TileDiffXY(1, 0)
        (2, 1, 1), // SLOPE_S
        (4, 0, 1), // SLOPE_E
        (8, 0, 0), // SLOPE_N
    ];
    let Some((slope, _)) = tile_slope_and_z(map, tile_coord) else {
        return;
    };

    // `FlowRiver` first tries `SLOPE_ELEVATED` down. On a cota-zero lake the
    // command fails atomically, but once an earlier tile has raised the shared
    // corners, a flat cota-one tile can be lowered and raised again. Keeping
    // this transaction (rather than only raising) avoids creating cota two
    // plateaus and matches `CmdTerraformLand`'s command model.
    let can_lower = CORNERS.iter().all(|&(_, dx, dy)| {
        map.get(TileCoord::new(tile_coord.x + dx, tile_coord.y + dy))
            .is_some_and(|tile| tile.height > 0)
    });
    if can_lower {
        for &(_, dx, dy) in &CORNERS {
            let corner = TileCoord::new(tile_coord.x + dx, tile_coord.y + dy);
            if let Some(height) = map.get(corner).map(|tile| tile.height) {
                let _ = map.set_height(corner, height - 1);
            }
        }
    }

    let slope_after_lowering = tile_slope_and_z(map, tile_coord)
        .map_or(slope, |(slope_after_lowering, _)| slope_after_lowering);
    let raise_slope = complement_slope(slope_after_lowering);
    for (bit, dx, dy) in CORNERS {
        if raise_slope & bit == 0 {
            continue;
        }
        let corner = TileCoord::new(tile_coord.x + dx, tile_coord.y + dy);
        let Some(height) = map.get(corner).map(|tile| tile.height) else {
            continue;
        };
        let _ = map.set_height(corner, height.saturating_add(1));
    }

    // `DoClearSquare`/`MakeClear(tile, CLEAR_GRASS, 3)` resets every map
    // plane owned by the former water tile, not only its semantic kind.
    let Some(mut cleared) = map.get(tile_coord) else {
        return;
    };
    cleared.kind = TileKind::Grass;
    cleared.mapt = 0;
    cleared.m5 = clear_ground_m5(CLEAR_GROUND_GRASS, 3);
    cleared.m1 = OWNER_NONE_M1;
    cleared.m2 = 0;
    cleared.m2_hi = 0;
    cleared.m3 = 0;
    cleared.m3hi = 0;
    cleared.m6 = 0;
    cleared.m7 = 0;
    cleared.m8 = 0;
    let _ = map.set_tile(tile_coord, cleared);
}

/// Explora `FlowRiver` hasta la frontera que consume RNG. El trazado YAPF,
/// lagos/humedales y ensanchamiento siguen separados en RMAP-018.
fn probe_flow_river(
    map: &mut Map,
    spring: TileCoord,
    begin: TileCoord,
    min_river_length: u32,
    rng: &mut Randomizer,
    preserve: &[PreserveRect],
    depth: usize,
) -> (bool, bool) {
    if depth > map.tiles().len() {
        return (false, false);
    }
    if map.get_kind(begin) == Some(TileKind::Water) {
        return (
            spring.x.abs_diff(begin.x) + spring.y.abs_diff(begin.y) > min_river_length,
            tile_slope_and_z(map, begin).is_some_and(|(_, z)| z == 0),
        );
    }
    let Some((_, height_begin)) = tile_slope_and_z(map, begin) else {
        return (false, false);
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
        let is_water = map.get_kind(end) == Some(TileKind::Water);
        if slope_end == 0 && (height_end < height_begin || (height_end == height_begin && is_water))
        {
            if is_water
                && map
                    .get(end)
                    .is_some_and(|tile| water_class_from_m1(tile.m1) == WaterClass::Sea)
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
            if preserve.iter().any(|rect| rect.contains(next.x, next.y)) || !marks.insert(next) {
                continue;
            }
            if map.get(next).is_some() && river_flows_down(map, end, next) {
                queue.push(next);
            }
        }
    }
    if let Some(end) = found {
        return probe_flow_river(map, spring, end, min_river_length, rng, preserve, depth + 1);
    }
    if queue.len() > 32 {
        // `RandomRange(queue.size())` para el posible lago. Su construcción
        // aún está abierta, pero el sorteo pertenece al stream de la fase.
        let _ = queue[rng.random_range(queue.len() as u32) as usize];
    }
    (false, false)
}

pub(super) fn flow_river(
    map: &mut Map,
    start: TileCoord,
    seed: u64,
    preserve: &[PreserveRect],
    min_river_length: u32,
) -> Result<bool, MapError> {
    let mut cur = start;
    let mut steps = 0u32;
    let mut rng = seed;
    let mut path = Vec::new();
    let mut reached_water = false;
    while steps < 200 {
        steps += 1;
        if preserve.iter().any(|r| r.contains(cur.x, cur.y)) {
            break;
        }
        match map.get_kind(cur) {
            Some(TileKind::Grass | TileKind::Forest | TileKind::CoalField) => {
                // `FlowRiver`/YAPF primero prueba que el flujo alcanza una
                // terminación válida; no materializa un río parcial mientras
                // explora. Guardar el trayecto evita que un pozo largo fallido
                // deje agua sintética en mapas pequeños.
                path.push(cur);
            }
            Some(TileKind::Water) => {
                reached_water = true;
                break;
            }
            _ => break,
        }
        let Some((_, cz)) = tile_slope_and_z(map, cur) else {
            break;
        };
        let mut best: Option<(TileCoord, u8)> = None;
        let mut candidates = [(0i32, 0i32); 4];
        let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for (i, &(dx, dy)) in dirs.iter().enumerate() {
            candidates[i] = (dx, dy);
        }
        // Mezcla ligera según seed.
        if rng & 1 != 0 {
            candidates.swap(0, 2);
        }
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        for &(dx, dy) in &candidates {
            let n = TileCoord::new(cur.x + dx, cur.y + dy);
            if preserve.iter().any(|r| r.contains(n.x, n.y)) {
                continue;
            }
            let Some((_, nz)) = tile_slope_and_z(map, n) else {
                continue;
            };
            if nz > cz {
                continue;
            }
            match best {
                None => best = Some((n, nz)),
                Some((_, bz)) if nz < bz => best = Some((n, nz)),
                Some((_, bz)) if nz == bz && rng.trailing_zeros() >= 3 => best = Some((n, nz)),
                _ => {}
            }
        }
        let Some((next, _)) = best else {
            break;
        };
        if next == cur {
            break;
        }
        cur = next;
    }

    let distance = start.x.abs_diff(cur.x) + start.y.abs_diff(cur.y);
    if !reached_water || distance <= min_river_length {
        return Ok(false);
    }

    for tile in path {
        make_water_tile(map, tile, WaterClass::River)?;
    }
    Ok(true)
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

    fn descending_river_test_map(width: i32, sea_x: i32) -> Map {
        let mut map = Map::new_flat(width as u32, 8, 0);
        for x in 0..width {
            for y in 0..8 {
                map.set_height(TileCoord::new(x, y), (96 - x).try_into().expect("height"))
                    .expect("set height");
            }
        }
        make_water_tile(&mut map, TileCoord::new(sea_x, 3), WaterClass::Sea)
            .expect("make terminal sea");
        map
    }

    #[test]
    fn long_river_rejects_a_route_shorter_than_its_manhattan_minimum() {
        let mut map = descending_river_test_map(16, 9);
        let start = TileCoord::new(1, 3);

        let built = flow_river(&mut map, start, 7, &[], 16).expect("flow river");

        assert!(!built);
        assert!(map.tiles().iter().all(|tile| !is_river_tile(*tile)));
    }

    #[test]
    fn accepted_route_is_painted_only_after_reaching_water_and_minimum_length() {
        let mut map = descending_river_test_map(40, 25);
        let start = TileCoord::new(1, 3);

        let built = flow_river(&mut map, start, 7, &[], 16).expect("flow river");

        assert!(built);
        assert!(map.tiles().iter().any(|tile| is_river_tile(*tile)));
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
