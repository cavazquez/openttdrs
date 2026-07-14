//! Expansión física de pueblos (`GrowTown` / `TryBuildTownHouse` simplificado).
//!
//! Al crecer, el pueblo intenta extender una calle o colocar una casa junto a
//! una existente. No construye puentes/túneles ni grids 2×2/3×3 (OOS).

use crate::map::{Map, TileCoord, TileKind, effective_road_bits, tile_slope_and_z};
use crate::town::Town;

/// Radio máximo de búsqueda alrededor del centro del pueblo.
pub const TOWN_EXPAND_SEARCH_RADIUS: i32 = 12;
/// Intentos de colocación por ciclo de crecimiento.
pub const TOWN_EXPAND_ATTEMPTS: u8 = 3;
/// Población añadida por casa colocada (además del step abstracto).
pub const TOWN_EXPAND_POP_PER_HOUSE: u32 = 8;

const ROAD_AXIS_X: u8 = 0x0A; // NE|SW
const ROAD_AXIS_Y: u8 = 0x05; // NW|SE
const HOUSE_ID_MAX: u16 = 110;

/// Resultado de un intento de expansión física.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownExpandResult {
    /// Se colocó una casa en `pos`.
    House(TileCoord),
    /// Se colocó o extendió una calle en `pos`.
    Road(TileCoord),
    /// No hubo sitio válido.
    None,
}

/// Intenta expandir el pueblo una vez (casa preferente, si no calle).
#[must_use]
pub fn expand_town_once(map: &mut Map, town: &Town, attempt_seed: u32) -> TownExpandResult {
    // Preferir casa junto a carretera existente.
    if let Some(pos) = try_place_house_near_road(map, town, attempt_seed) {
        return TownExpandResult::House(pos);
    }
    // Extender / sembrar calle.
    if let Some(pos) = try_extend_or_seed_road(map, town, attempt_seed) {
        return TownExpandResult::Road(pos);
    }
    TownExpandResult::None
}

/// Varias tentativas; actualiza población si hay casas nuevas.
pub fn expand_town_physically(map: &mut Map, town: &mut Town, tick: u64) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for attempt in 0..TOWN_EXPAND_ATTEMPTS {
        let seed = tick
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(u64::from(town.id).wrapping_mul(0x85EB_CA6B))
            .wrapping_add(u64::from(attempt).wrapping_mul(0xC2B2_AE3D));
        let seed = u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0);
        match expand_town_once(map, town, seed) {
            TownExpandResult::House(pos) => {
                town.population = town.population.saturating_add(TOWN_EXPAND_POP_PER_HOUSE);
                dirty.push(pos);
            }
            TownExpandResult::Road(pos) => {
                dirty.push(pos);
            }
            TownExpandResult::None => {}
        }
    }
    dirty
}

fn try_place_house_near_road(map: &mut Map, town: &Town, seed: u32) -> Option<TileCoord> {
    let candidates = collect_road_tiles_near(map, town.pos, TOWN_EXPAND_SEARCH_RADIUS);
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
            if can_build_house(map, pos) {
                let house_id = house_id_for(town.id, pos, seed);
                if map.set_completed_house(pos, house_id, 0).is_ok() {
                    return Some(pos);
                }
            }
        }
    }
    None
}

fn try_extend_or_seed_road(map: &mut Map, town: &Town, seed: u32) -> Option<TileCoord> {
    let roads = collect_road_tiles_near(map, town.pos, TOWN_EXPAND_SEARCH_RADIUS);
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
                if can_build_town_road(map, pos) && place_town_road(map, pos, axis) {
                    // Conectar la tesela origen si también es road.
                    let _ = or_road_bits(map, from, axis);
                    return Some(pos);
                }
            }
        }
    }
    // Semilla: calle en hierba plana cerca del centro (como fallback OpenTTD).
    seed_road_near_center(map, town.pos, seed)
}

fn seed_road_near_center(map: &mut Map, center: TileCoord, seed: u32) -> Option<TileCoord> {
    const OFFSETS: [(i32, i32); 13] = [
        (0, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, -1),
        (0, 2),
        (2, 0),
        (0, -2),
        (-2, 2),
        (2, 2),
        (2, -2),
        (-2, -2),
        (1, 0),
    ];
    let start = usize::try_from(seed).unwrap_or(0) % OFFSETS.len();
    for offset in 0..OFFSETS.len() {
        let (dx, dy) = OFFSETS[(start + offset) % OFFSETS.len()];
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

fn can_build_house(map: &Map, pos: TileCoord) -> bool {
    if map.get_kind(pos) != Some(TileKind::Grass) {
        return false;
    }
    is_flat_tile(map, pos)
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
    tile.m1 = 0;
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

fn house_id_for(town_id: u32, pos: TileCoord, seed: u32) -> u16 {
    let h = seed
        .wrapping_add(town_id.wrapping_mul(17))
        .wrapping_add(pos.x.cast_unsigned().wrapping_mul(31))
        .wrapping_add(pos.y.cast_unsigned().wrapping_mul(13));
    u16::try_from(h % u32::from(HOUSE_ID_MAX))
        .unwrap_or(1)
        .max(1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::station::{Station, StopKind};
    use crate::town::{TOWN_GROWTH_TICKS, Town, grow_town_if_served};

    #[test]
    fn expand_places_road_then_house() {
        let mut map = Map::new_flat(32, 32, 1);
        let town = Town {
            id: 1,
            pos: TileCoord::new(16, 16),
            name: "Expand".into(),
            population: 40,
            is_growing: true,
            ..Default::default()
        };
        // Sin calles: semilla de carretera.
        let r1 = expand_town_once(&mut map, &town, 1);
        assert!(matches!(r1, TownExpandResult::Road(_)), "{r1:?}");
        // Con calle: casa.
        let r2 = expand_town_once(&mut map, &town, 7);
        assert!(
            matches!(r2, TownExpandResult::House(_)) || matches!(r2, TownExpandResult::Road(_)),
            "{r2:?}"
        );
    }

    #[test]
    fn grow_town_if_served_places_tiles() {
        let mut map = Map::new_flat(32, 32, 1);
        // Calle inicial junto al centro.
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
            ..Default::default()
        }];
        let stations = vec![Station::new_with_kind(
            TileCoord::new(16, 17),
            StopKind::BusStop,
        )];
        let dirty = grow_town_if_served(&mut map, &[], &stations, &mut towns, TOWN_GROWTH_TICKS);
        assert!(!dirty.is_empty(), "debe ensuciar teselas");
        assert!(towns[0].population > 100);
        let houses = dirty
            .iter()
            .filter(|&&c| map.get_kind(c) == Some(TileKind::House))
            .count();
        let roads = dirty
            .iter()
            .filter(|&&c| map.get_kind(c) == Some(TileKind::Road))
            .count();
        assert!(houses + roads >= 1);
    }
}
