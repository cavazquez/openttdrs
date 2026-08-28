//! Generación de objetos vanilla durante una partida nueva.
//!
//! `OpenTTD` ejecuta `GenerateObjects` después de pueblos/industrias y antes
//! de `GenerateTrees`.  Los dos objetos que pueden aparecer de forma
//! procedural son el transmisor (tipo `0`) y el faro (tipo `1`).  Esta etapa
//! mantiene el `ObjectID` nativo en `m2/m5`, guarda la instancia en `OBJS` y
//! continúa el mismo stream de [`WorldGenRng`] que usa el resto del mapa.

use crate::company::OWNER_NONE_M1;
use crate::game_state::GameState;
use crate::map::{
    MP_OBJECT_MAPT, Map, TileCoord, TileKind, WaterClass, set_water_class_m1, tile_slope_and_z,
    water_class_from_m1,
};
use crate::sav::SavObject;

use super::{Climate, PreserveRect, WorldGenRng, ceil_div, scale_by_size};

const OBJECT_TRANSMITTER_GENERATE_AMOUNT: u32 = 15;
const OBJECT_LIGHTHOUSE_GENERATE_AMOUNT: u32 = 8;
const OBJECT_GENERATION_ATTEMPTS: u32 = 1_000;
const OBJECT_CLEAR_DISTANCE_DIAMETER: i32 = 9;
const OBJECT_CLEAR_DISTANCE_RADIUS: i32 = OBJECT_CLEAR_DISTANCE_DIAMETER / 2;

/// Genera los objetos vanilla de una partida nueva y devuelve cuántas
/// instancias se pudieron colocar.
pub fn generate_objects_with_rng(
    state: &mut GameState,
    climate: Climate,
    rng: &mut WorldGenRng,
    preserve: &[PreserveRect],
) -> usize {
    let (width, height) = state.map.dimensions();
    if width < 4 || height < 4 {
        return 0;
    }

    let mut generated = 0usize;
    if matches!(
        climate,
        Climate::Temperate | Climate::SubArctic | Climate::SubTropical
    ) {
        let amount = scale_by_size(OBJECT_TRANSMITTER_GENERATE_AMOUNT, width, height);
        generated += generate_transmitters(state, amount, rng, preserve);
    }
    if matches!(climate, Climate::Temperate | Climate::SubArctic) {
        let amount = lighthouse_amount(&state.map);
        generated += generate_lighthouses(state, amount, rng, preserve);
    }
    generated
}

fn generate_transmitters(
    state: &mut GameState,
    mut amount: u32,
    rng: &mut WorldGenRng,
    preserve: &[PreserveRect],
) -> usize {
    let attempts = scale_by_size(
        OBJECT_GENERATION_ATTEMPTS,
        state.map.dimensions().0,
        state.map.dimensions().1,
    );
    let mut generated = 0usize;
    for _ in 0..attempts {
        if amount == 0 {
            break;
        }
        let random_tile = random_tile(&state.map, rng.next());
        let Some(random_tile) = random_tile else {
            continue;
        };
        if !transmitter_candidate(&state.map, Some(random_tile), preserve) {
            continue;
        }
        if object_nearby(&state.objects, random_tile, 0) {
            continue;
        }
        build_generated_object(state, random_tile, 0, 1, 1, rng);
        amount -= 1;
        generated += 1;
    }
    generated
}

fn generate_lighthouses(
    state: &mut GameState,
    mut amount: u32,
    rng: &mut WorldGenRng,
    preserve: &[PreserveRect],
) -> usize {
    let attempts = scale_by_size(
        OBJECT_GENERATION_ATTEMPTS,
        state.map.dimensions().0,
        state.map.dimensions().1,
    );
    let mut generated = 0usize;
    for _ in 0..attempts {
        if amount == 0 {
            break;
        }
        let Some(candidate) = lighthouse_candidate(&state.map, &state.objects, rng.next()) else {
            continue;
        };
        if preserve
            .iter()
            .any(|rect| rect.contains(candidate.x, candidate.y))
        {
            continue;
        }
        build_generated_object(state, candidate, 1, 1, 1, rng);
        amount -= 1;
        generated += 1;
    }
    generated
}

fn lighthouse_amount(map: &Map) -> u32 {
    let (width, height) = map.dimensions();
    let max_x = width.saturating_sub(1);
    let max_y = height.saturating_sub(1);
    let denominator = 2u32
        .saturating_mul(max_y)
        .saturating_add(2u32.saturating_mul(max_x))
        .saturating_sub(6);
    if denominator == 0 {
        return 0;
    }

    // `GenerateObjects` counts only the water one tile inside each freeform
    // edge.  The corners are deliberately not counted twice.
    let mut water_tiles = 0u32;
    for x in 0..max_x {
        if is_sea(map, TileCoord::new(x as i32, 1)) {
            water_tiles = water_tiles.saturating_add(1);
        }
        if is_sea(
            map,
            TileCoord::new(x as i32, max_y.saturating_sub(1) as i32),
        ) {
            water_tiles = water_tiles.saturating_add(1);
        }
    }
    for y in 1..max_y {
        if is_sea(map, TileCoord::new(1, y as i32)) {
            water_tiles = water_tiles.saturating_add(1);
        }
        if is_sea(
            map,
            TileCoord::new(max_x.saturating_sub(1) as i32, y as i32),
        ) {
            water_tiles = water_tiles.saturating_add(1);
        }
    }

    scale_by_size_1d(
        OBJECT_LIGHTHOUSE_GENERATE_AMOUNT.saturating_mul(water_tiles),
        width,
        height,
    ) / denominator
}

fn scale_by_size_1d(value: u32, width: u32, height: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    let log_x = width.max(1).ilog2();
    let log_y = height.max(1).ilog2();
    ceil_div((value << log_x).saturating_add(value << log_y), 1 << 9)
}

fn transmitter_candidate(map: &Map, coord: Option<TileCoord>, preserve: &[PreserveRect]) -> bool {
    let Some(coord) = coord else {
        return false;
    };
    if preserve.iter().any(|rect| rect.contains(coord.x, coord.y)) {
        return false;
    }
    let Some(tile) = map.get(coord) else {
        return false;
    };
    tile.kind == TileKind::Grass
        && tile.mapt >> 4 == 0
        && tile_slope_and_z(map, coord).is_some_and(|(slope, height)| slope == 0 && height >= 4)
}

fn lighthouse_candidate(map: &Map, objects: &[SavObject], random: u32) -> Option<TileCoord> {
    let (width, height) = map.dimensions();
    let max_x = width.checked_sub(1)?;
    let max_y = height.checked_sub(1)?;
    let perimeter_span = 2u32.checked_mul(max_x.checked_add(max_y)?)?;
    if perimeter_span == 0 || max_x < 2 || max_y < 2 {
        return None;
    }

    let mut perimeter = i32::from((random >> 16) as u16) % i32::try_from(perimeter_span).ok()?
        - i32::try_from(max_y).ok()?;
    let mut direction = 0u8;
    while perimeter > 0 {
        perimeter -= if direction & 1 == 0 {
            max_x as i32
        } else {
            max_y as i32
        };
        direction = direction.saturating_add(1);
    }
    if direction > 3 {
        return None;
    }

    let coord = match direction {
        0 => TileCoord::new((max_x - 1) as i32, (random % max_y) as i32),
        1 => TileCoord::new((random % max_x) as i32, 1),
        2 => TileCoord::new(1, (random % max_y) as i32),
        _ => TileCoord::new((random % max_x) as i32, (max_y - 1) as i32),
    };
    if !is_sea(map, coord) {
        return None;
    }

    let (dx, dy) = match direction {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        _ => (0, -1),
    };
    let mut current = coord;
    for _ in 0..19 {
        if !in_bounds(map, current) {
            return None;
        }
        if let Some(tile) = map.get(current)
            && tile.kind == TileKind::Grass
            && tile.mapt >> 4 == 0
            && tile_slope_and_z(map, current)
                .is_some_and(|(slope, height)| slope == 0 && height <= 2)
        {
            if object_nearby(objects, current, 1) {
                return None;
            }
            return Some(current);
        }
        current = TileCoord::new(current.x + dx, current.y + dy);
    }
    None
}

fn build_generated_object(
    state: &mut GameState,
    coord: TileCoord,
    object_type: u16,
    width: u16,
    height: u16,
    rng: &mut WorldGenRng,
) {
    let colour = (rng.next() & 0x0F) as u8;
    let object_id = state
        .objects
        .iter()
        .map(|object| object.object_id)
        .max()
        .unwrap_or(u32::MAX)
        .wrapping_add(1);
    let tile_random = rng.next() as u8;
    let Some(mut tile) = state.map.get(coord) else {
        return;
    };
    let water_class = if tile.kind == TileKind::Water {
        water_class_from_m1(tile.m1)
    } else {
        WaterClass::Invalid
    };
    tile.kind = TileKind::Unknown(10);
    tile.mapt = MP_OBJECT_MAPT;
    tile.m1 = set_water_class_m1(OWNER_NONE_M1, water_class);
    tile.m2 = object_id as u8;
    tile.m2_hi = (object_id >> 8) as u8;
    tile.m3 = tile_random;
    tile.m3hi = 0;
    tile.m5 = (object_id >> 16) as u8;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    if state.map.set_tile(coord, tile).is_err() {
        return;
    }

    let town = state
        .towns
        .iter()
        .min_by_key(|town| crate::house_spec::distance_square(town.pos, coord))
        .map_or(0, |town| town.id);
    state.objects.push(SavObject {
        object_id,
        tile: coord,
        width,
        height,
        town,
        build_date: state.calendar.date,
        colour,
        view: 0,
        object_type,
    });
    state.sav_objects_dirty = true;
}

fn object_nearby(objects: &[SavObject], coord: TileCoord, object_type: u16) -> bool {
    objects.iter().any(|object| {
        object.object_type == object_type
            && object.tile.x.abs_diff(coord.x) <= OBJECT_CLEAR_DISTANCE_RADIUS as u32
            && object.tile.y.abs_diff(coord.y) <= OBJECT_CLEAR_DISTANCE_RADIUS as u32
    })
}

fn random_tile(map: &Map, random: u32) -> Option<TileCoord> {
    let (width, height) = map.dimensions();
    let size = width.checked_mul(height)?;
    let index = u64::from(random & size.saturating_sub(1));
    crate::map::coord_from_linear_index(index, width).filter(|coord| {
        coord.x >= 0 && coord.y >= 0 && coord.x < width as i32 && coord.y < height as i32
    })
}

fn in_bounds(map: &Map, coord: TileCoord) -> bool {
    let (width, height) = map.dimensions();
    coord.x >= 0 && coord.y >= 0 && coord.x < width as i32 && coord.y < height as i32
}

fn is_sea(map: &Map, coord: TileCoord) -> bool {
    map.get(coord).is_some_and(|tile| {
        tile.kind == TileKind::Water && water_class_from_m1(tile.m1) == WaterClass::Sea
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::world_gen::{Climate, WorldGenConfig, apply_world_gen_with_rng};

    #[test]
    fn lighthouse_amount_scales_water_edges() {
        let mut map = Map::new_flat(64, 64, 0);
        for y in 1..63 {
            for x in 1..63 {
                if x == 1 || y == 1 || x == 62 || y == 62 {
                    map.set_tile(
                        TileCoord::new(x, y),
                        crate::map::Tile {
                            kind: TileKind::Water,
                            mapt: 0x60,
                            m1: set_water_class_m1(crate::company::OWNER_WATER_M1, WaterClass::Sea),
                            ..map.get(TileCoord::new(x, y)).unwrap()
                        },
                    )
                    .unwrap();
                }
            }
        }
        assert!(lighthouse_amount(&map) > 0);
    }

    #[test]
    fn generated_objects_use_native_id_and_pool_type() {
        let mut map = Map::new_flat(64, 64, 5);
        let mut cfg = WorldGenConfig {
            seed: 7,
            ..WorldGenConfig::default()
        };
        cfg.water_borders = Some(0);
        let mut rng = apply_world_gen_with_rng(&mut map, &cfg, &[]).unwrap();
        let mut state = GameState::from_map(map);
        state.climate = Climate::Temperate;
        let count = generate_objects_with_rng(&mut state, Climate::Temperate, &mut rng, &[]);
        assert!(count <= 2);
        for object in &state.objects {
            let tile = state.map.get(object.tile).unwrap();
            assert_eq!(tile.mapt >> 4, 10);
            assert_eq!(
                crate::map::object_id_from_tile(&tile),
                Some(object.object_id)
            );
            assert!(object.object_type <= 1);
            assert_eq!(
                tile.m1,
                set_water_class_m1(OWNER_NONE_M1, WaterClass::Invalid)
            );
        }
    }
}
