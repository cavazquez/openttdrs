//! Inundación desde agua (`water_cmd.cpp`: `TileLoop_Water`, `DoFloodTile`, `FloodVehicles`).

use crate::GameState;
use crate::map::slope::{SLOPE_STEEP, tile_slope_and_z};
use crate::map::water_class::{
    WaterClass, make_water_tile, set_water_class_m1, water_class_from_m1,
};
use crate::map::{Map, Tile, TileCoord, TileKind};
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};
use crate::sim_events::SimEvent;
use crate::vehicle::{Vehicle, VehicleKind};
use crate::world_gen::{CLEAR_GROUND_GRASS, clear_ground_m5};

/// `WaterTileType::Coast` en bits 4–7 de `m5`.
const WATER_TYPE_COAST: u8 = 1;
/// `SLOPE_HALFTILE_MASK` — se ignora al indexar `_flood_from_dirs`.
const SLOPE_HALFTILE_MASK: u8 = 0xE0;

/// Comportamiento de inundación (`FloodingBehaviour`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloodingBehaviour {
    Active,
    DryOut,
    None,
}

/// Offsets `TileIndexDiffCByDir` (Direction N…NW).
const DIR_OFFSETS: [(i32, i32); 8] = [
    (-1, -1), // N
    (-1, 0),  // NE
    (-1, 1),  // E
    (0, 1),   // SE
    (1, 1),   // S
    (1, 0),   // SW
    (1, -1),  // W
    (0, -1),  // NW
];

/// `_flood_from_dirs[slope & ~HALFTILE & ~STEEP]` — bitmask de `Direction`.
const FLOOD_FROM_DIRS: [u8; 15] = [
    (1 << 7) | (1 << 5) | (1 << 3) | (1 << 1), // FLAT: NW SW SE NE
    (1 << 1) | (1 << 3),                       // W
    (1 << 7) | (1 << 1),                       // S
    1 << 1,                                    // SW
    (1 << 7) | (1 << 5),                       // E
    0,                                         // EW
    1 << 7,                                    // SE
    (1 << 0) | (1 << 7) | (1 << 1),            // WSE / steep S
    (1 << 5) | (1 << 3),                       // N
    1 << 3,                                    // NW
    0,                                         // NS
    (1 << 2) | (1 << 1) | (1 << 3),            // NWS / steep W
    1 << 5,                                    // NE
    (1 << 4) | (1 << 5) | (1 << 3),            // ENW / steep N
    (1 << 6) | (1 << 5) | (1 << 7),            // SEN / steep E
];

#[must_use]
const fn reverse_dir(dir: u8) -> u8 {
    dir ^ 4
}

#[must_use]
const fn is_slope_one_corner_raised(slope: u8) -> bool {
    matches!(slope & 0x0F, 1 | 2 | 4 | 8)
}

#[must_use]
fn water_tile_type(tile: Tile) -> u8 {
    (tile.m5 >> 4) & 0x0F
}

#[must_use]
fn is_coast_water(tile: Tile) -> bool {
    tile.kind == TileKind::Water && water_tile_type(tile) == WATER_TYPE_COAST
}

#[must_use]
fn is_non_flooding_water(tile: Tile) -> bool {
    tile.kind == TileKind::Water && (tile.m3 & 1) != 0
}

fn set_non_flooding_water(map: &mut Map, c: TileCoord, non_flooding: bool) {
    let Some(mut tile) = map.get(c) else {
        return;
    };
    if tile.kind != TileKind::Water {
        return;
    }
    if non_flooding {
        tile.m3 |= 1;
    } else {
        tile.m3 &= !1;
    }
    let _ = map.set_tile(c, tile);
}

/// `TreeGround::Shore` en bits 6–8 de `m2`.
#[must_use]
const fn tree_ground(m2: u8) -> u8 {
    (m2 >> 6) & 0x07
}

#[must_use]
fn make_tree_m2(ground: u8, density: u8) -> u8 {
    ((ground & 0x07) << 6) | ((density & 0x03) << 4)
}

/// Convierte la tesela en costa (`MakeShore`).
pub fn make_shore_tile(map: &mut Map, c: TileCoord) -> Result<(), super::MapError> {
    let mut tile = map.get(c).ok_or(super::MapError::OutOfBounds)?;
    tile.kind = TileKind::Water;
    tile.mapt = 0x60;
    tile.m5 = WATER_TYPE_COAST << 4;
    tile.m1 = set_water_class_m1(tile.m1, WaterClass::Sea);
    tile.m2 = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;
    map.set_tile(c, tile)
}

/// `GetFloodingBehaviour`.
#[must_use]
pub fn get_flooding_behaviour(tile: Tile) -> FloodingBehaviour {
    match tile.kind {
        TileKind::Water => {
            if is_coast_water(tile) {
                // La pendiente se consulta aparte en el bucle; aquí usamos m5+heurística
                // mínima: la decisión fina está en `tile_loop_water_at` con el mapa.
                FloodingBehaviour::Active
            } else if water_class_from_m1(tile.m1) == WaterClass::Sea {
                FloodingBehaviour::Active
            } else {
                FloodingBehaviour::None
            }
        }
        TileKind::Station | TileKind::Industry => {
            if water_class_from_m1(tile.m1) == WaterClass::Sea {
                FloodingBehaviour::Active
            } else {
                FloodingBehaviour::None
            }
        }
        TileKind::Forest => {
            if tree_ground(tile.m2) == 3 {
                FloodingBehaviour::DryOut
            } else {
                FloodingBehaviour::None
            }
        }
        TileKind::Void => FloodingBehaviour::Active,
        _ => FloodingBehaviour::None,
    }
}

fn flooding_behaviour_at(map: &Map, c: TileCoord, tile: Tile) -> FloodingBehaviour {
    if tile.kind == TileKind::Water && is_coast_water(tile) {
        let Some((slope, _)) = tile_slope_and_z(map, c) else {
            return FloodingBehaviour::None;
        };
        return if is_slope_one_corner_raised(slope & !SLOPE_STEEP) {
            FloodingBehaviour::Active
        } else {
            FloodingBehaviour::DryOut
        };
    }
    get_flooding_behaviour(tile)
}

/// ¿La tesela se puede arrasar por inundación (equivalente a `LandscapeClear` OK)?
#[must_use]
fn is_flood_clearable(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Grass | TileKind::Forest | TileKind::CoalField | TileKind::House
    )
}

/// `FloodVehicles`: ahoga tren/carretera en la tesela (y consist completo).
pub fn flood_vehicles(state: &mut GameState, tile: TileCoord) {
    let flood_z = 0_i16;
    let victims: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| vehicle_floodable_on_tile(v, tile, flood_z))
        .map(|v| consist_head_id(state, v.id))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    for head_id in victims {
        flood_vehicle_consist(state, head_id, tile);
    }
}

fn consist_head_id(state: &GameState, id: u32) -> u32 {
    let mut cur = id;
    for _ in 0..64 {
        let Some(v) = state.vehicles.iter().find(|v| v.id == cur) else {
            return id;
        };
        match v.prev_unit {
            Some(prev) => cur = prev,
            None => return cur,
        }
    }
    id
}

fn vehicle_floodable_on_tile(v: &Vehicle, tile: TileCoord, flood_z: i16) -> bool {
    if v.pos != tile {
        return false;
    }
    match v.kind {
        VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            v.z_pos.unwrap_or(0) <= flood_z
        }
        VehicleKind::Aircraft => {
            // Solo aviones en tierra (altitud 0) sobre la tesela.
            v.altitude == 0 && v.z_pos.unwrap_or(0) <= flood_z
        }
        VehicleKind::Ship => false,
    }
}

fn flood_vehicle_consist(state: &mut GameState, head_id: u32, at: TileCoord) {
    let Some(v) = state.vehicles.iter().find(|v| v.id == head_id) else {
        return;
    };
    let name = v.display_name();
    let kind = v.kind;
    let mut remove = vec![head_id];
    let mut next = v.next_unit;
    while let Some(id) = next {
        remove.push(id);
        next = state
            .vehicles
            .iter()
            .find(|u| u.id == id)
            .and_then(|u| u.next_unit);
    }
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::VehicleFlooded {
            vehicle_id: head_id,
            at,
            kind,
        });
    let news_id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = crate::news::NewsItem::new(
        news_id,
        format!("{name} inundado"),
        Some(format!(
            "Un vehículo quedó bajo el agua en ({}, {}).",
            at.x, at.y
        )),
        NewsType::Accident,
        default_display_for_type(NewsType::Accident),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
    state.vehicles.retain(|v| !remove.contains(&v.id));
}

/// `DoFloodTile`: convierte tierra inundable en mar o costa.
pub fn do_flood_tile(state: &mut GameState, target: TileCoord) -> bool {
    let Some(tile) = state.map.get(target) else {
        return false;
    };
    if tile.kind == TileKind::Water {
        return false;
    }

    let Some((tileh, _z)) = tile_slope_and_z(&state.map, target) else {
        return false;
    };
    let slope = tileh & !SLOPE_HALFTILE_MASK & !SLOPE_STEEP;

    if slope != 0 {
        match tile.kind {
            TileKind::Forest if !is_slope_one_corner_raised(slope) => {
                let density = (tile.m2 >> 4) & 0x03;
                let m2 = make_tree_m2(3, density.max(3));
                let mut t = tile;
                t.m2 = m2;
                t.m1 = set_water_class_m1(t.m1, WaterClass::Sea);
                let _ = state.map.set_tile(target, t);
                return true;
            }
            kind if is_flood_clearable(kind) => {
                flood_vehicles(state, target);
                if make_shore_tile(&mut state.map, target).is_ok() {
                    return true;
                }
            }
            _ => {}
        }
        return false;
    }

    // Plano: solo inundar teselas que el port puede arrasar (clear/trees/house).
    // OpenTTD también limpia rail con OWNER_WATER; aquí se deja para un corte posterior
    // (FloodHalftile / LandscapeClear completo) para no destruir redes a cota 0.
    if !is_flood_clearable(tile.kind) {
        return false;
    }
    flood_vehicles(state, target);
    let _ = state.map.set_kind(target, TileKind::Grass);
    let _ = state
        .map
        .set_mapt_m5(target, 0x00, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
    state.stations.retain(|s| s.pos != target);
    make_water_tile(&mut state.map, target, WaterClass::Sea).is_ok()
}

/// Intenta inundar el vecino en `dir`. Devuelve si sigue habiendo tierra inundable
/// alrededor (`continue_flooding` en OpenTTD).
fn try_flood_neighbor(state: &mut GameState, from: TileCoord, dir: u8) -> bool {
    let (dx, dy) = DIR_OFFSETS[usize::from(dir & 7)];
    let dest = TileCoord::new(from.x + dx, from.y + dy);
    let Some(dest_tile) = state.map.get(dest) else {
        return false;
    };
    if dest_tile.kind == TileKind::Water {
        return false;
    }
    if dest_tile.kind == TileKind::Station {
        return true;
    }
    // Vecino no-agua: podría inundarse más tarde si se despeja.
    let continue_flooding = true;
    if dest_tile.kind == TileKind::Forest && tree_ground(dest_tile.m2) == 3 {
        return continue_flooding;
    }
    let Some((slope_dest, z_dest)) = tile_slope_and_z(&state.map, dest) else {
        return continue_flooding;
    };
    if z_dest > 0 {
        return continue_flooding;
    }
    let slope_idx = slope_dest & !SLOPE_HALFTILE_MASK & !SLOPE_STEEP;
    let dirs = FLOOD_FROM_DIRS
        .get(usize::from(slope_idx))
        .copied()
        .unwrap_or(0);
    if dirs & (1 << reverse_dir(dir)) == 0 {
        return continue_flooding;
    }
    let _ = do_flood_tile(state, dest);
    continue_flooding
}

/// `TileLoop_Water` sobre una tesela visitada.
pub fn tile_loop_water_at(state: &mut GameState, c: TileCoord, tile: Tile) {
    if tile.kind == TileKind::Water && is_non_flooding_water(tile) {
        return;
    }
    match flooding_behaviour_at(&state.map, c, tile) {
        FloodingBehaviour::Active => {
            let mut continue_flooding = false;
            for dir in 0..8u8 {
                if try_flood_neighbor(state, c, dir) {
                    continue_flooding = true;
                }
            }
            if !continue_flooding && state.map.get_kind(c) == Some(TileKind::Water) {
                set_non_flooding_water(&mut state.map, c, true);
            }
        }
        FloodingBehaviour::DryOut => {
            dry_up_tile(state, c);
        }
        FloodingBehaviour::None => {}
    }
}

fn dry_up_tile(state: &mut GameState, c: TileCoord) {
    let Some(tile) = state.map.get(c) else {
        return;
    };
    match tile.kind {
        TileKind::Forest if tree_ground(tile.m2) == 3 => {
            let mut t = tile;
            t.m2 = make_tree_m2(0, 3);
            t.m1 = set_water_class_m1(t.m1, WaterClass::Invalid);
            let _ = state.map.set_tile(c, t);
        }
        TileKind::Water if is_coast_water(tile) => {
            let _ = state.map.set_kind(c, TileKind::Grass);
            let _ = state
                .map
                .set_mapt_m5(c, 0x00, clear_ground_m5(CLEAR_GROUND_GRASS, 3));
            let _ = state.map.set_m1(c, 0);
            let _ = state.map.set_m2(c, 0);
            let _ = state.map.set_m3(c, 0);
        }
        _ => {}
    }
}

/// Procesa inundación sobre las visitas del `RunTileLoop` del tick.
pub fn process_water_flood_from_visits(
    state: &mut GameState,
    visits: &[(TileCoord, Tile)],
) -> Vec<TileCoord> {
    let mut snapshot = std::collections::HashMap::new();
    for &(c, _) in visits {
        for dir in 0..8u8 {
            let (dx, dy) = DIR_OFFSETS[usize::from(dir)];
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if let Some(t) = state.map.get(n) {
                snapshot.entry(n).or_insert(t.kind);
            }
        }
        if let Some(t) = state.map.get(c) {
            snapshot.entry(c).or_insert(t.kind);
        }
    }
    for &(c, tile) in visits {
        tile_loop_water_at(state, c, tile);
    }
    let mut dirty = Vec::new();
    for (c, before_kind) in snapshot {
        if state.map.get_kind(c) != Some(before_kind) {
            dirty.push(c);
        }
    }
    dirty.sort_by_key(|t| (t.y, t.x));
    dirty
}

/// Hook de landscape: inundación sobre `runtime.tile_loop_visited`.
pub fn tick_water_flood(state: &mut GameState) {
    let visits = state.runtime.tile_loop_visited.clone();
    let dirty = process_water_flood_from_visits(state, &visits);
    state.runtime.landscape_tile_dirty.extend(dirty);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileLoopState;
    use crate::map::tile_loop::collect_tile_loop_visits;
    use crate::{Vehicle, VehicleKind};

    fn sea_at(map: &mut Map, c: TileCoord) {
        make_water_tile(map, c, WaterClass::Sea).unwrap();
    }

    /// `GameState::new` usa altura 1; la inundación solo actúa a `GetTileZ == 0`.
    fn flatten_sea_level(map: &mut Map) {
        let (w, h) = map.dimensions();
        for y in 0..h {
            for x in 0..w {
                map.set_height(TileCoord::new(x.cast_signed(), y.cast_signed()), 0)
                    .unwrap();
            }
        }
    }

    #[test]
    fn sea_floods_flat_neighbor_grass_at_z0() {
        let mut state = GameState::new(8, 8);
        flatten_sea_level(&mut state.map);
        let water = TileCoord::new(2, 2);
        let land = TileCoord::new(1, 2); // NE de water (dir NE = -1,0)
        sea_at(&mut state.map, water);
        state.map.set_kind(land, TileKind::Grass).unwrap();
        state
            .map
            .set_mapt_m5(land, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        let tile = state.map.get(water).unwrap();
        tile_loop_water_at(&mut state, water, tile);
        assert_eq!(state.map.get_kind(land), Some(TileKind::Water));
        assert_eq!(
            water_class_from_m1(state.map.get(land).unwrap().m1),
            WaterClass::Sea
        );
    }

    #[test]
    fn elevated_grass_is_not_flooded() {
        let mut state = GameState::new(8, 8);
        flatten_sea_level(&mut state.map);
        let water = TileCoord::new(2, 2);
        let land = TileCoord::new(1, 2);
        sea_at(&mut state.map, water);
        state.map.set_kind(land, TileKind::Grass).unwrap();
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            state
                .map
                .set_height(TileCoord::new(land.x + dx, land.y + dy), 4)
                .unwrap();
        }
        let tile = state.map.get(water).unwrap();
        tile_loop_water_at(&mut state, water, tile);
        assert_eq!(state.map.get_kind(land), Some(TileKind::Grass));
    }

    #[test]
    fn river_does_not_flood_neighbors() {
        let mut state = GameState::new(8, 8);
        flatten_sea_level(&mut state.map);
        let water = TileCoord::new(2, 2);
        let land = TileCoord::new(1, 2);
        make_water_tile(&mut state.map, water, WaterClass::River).unwrap();
        state.map.set_kind(land, TileKind::Grass).unwrap();
        let tile = state.map.get(water).unwrap();
        tile_loop_water_at(&mut state, water, tile);
        assert_eq!(state.map.get_kind(land), Some(TileKind::Grass));
    }

    #[test]
    fn flood_vehicles_removes_train_on_tile() {
        let mut state = GameState::new(8, 8);
        flatten_sea_level(&mut state.map);
        let land = TileCoord::new(3, 3);
        state.map.set_kind(land, TileKind::Grass).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, land, land);
        train.z_pos = Some(0);
        state.vehicles.push(train);
        assert!(do_flood_tile(&mut state, land));
        assert!(state.vehicles.is_empty());
        assert!(
            state
                .runtime
                .pending_sim_events
                .iter()
                .any(|e| matches!(e, SimEvent::VehicleFlooded { .. }))
        );
    }

    #[test]
    fn tile_loop_visits_propagate_flood() {
        let mut state = GameState::new(16, 16);
        flatten_sea_level(&mut state.map);
        let water = TileCoord::new(4, 4);
        let land = TileCoord::new(3, 4);
        sea_at(&mut state.map, water);
        state.map.set_kind(land, TileKind::Grass).unwrap();
        state
            .map
            .set_mapt_m5(land, 0, clear_ground_m5(CLEAR_GROUND_GRASS, 3))
            .unwrap();
        let mut loop_state = TileLoopState::default();
        let mut flooded = false;
        for tick in 0..512u64 {
            let visits =
                collect_tile_loop_visits(&state.map, tick, &mut loop_state.cur_tileloop_tile);
            let _ = process_water_flood_from_visits(&mut state, &visits);
            if state.map.get_kind(land) == Some(TileKind::Water) {
                flooded = true;
                break;
            }
        }
        assert!(flooded, "el mar debe inundar hierba vecina vía tile loop");
    }
}
