//! Estado del mundo de simulación y generación procedural.

use bevy::prelude::*;
use openttdrs_core::{
    GameState, Industry, IndustryKind, Map, Station, TileCoord, TileKind, Vehicle, VehicleKind,
    find_path,
};
use std::collections::BTreeMap;

/// Dimensiones del mapa generado proceduralmente (sin `OTTDMAP_FILE`).
pub const MAP_W: u32 = 24;
pub const MAP_H: u32 = 18;

/// Estado del mundo de simulación.
#[derive(Resource)]
pub struct SimWorld {
    pub state: GameState,
    /// Indica que el mapa se cargó desde un archivo .ottdmap externo.
    pub loaded_file: bool,
}

impl Default for SimWorld {
    fn default() -> Self {
        if let Ok(path) = std::env::var("OTTDMAP_FILE") {
            match std::fs::read(&path) {
                Ok(data) => match Map::from_ottd_binary(&data) {
                    Ok(map) => {
                        info!("Mapa cargado desde {path}");
                        let mut state = GameState::from_map(map);
                        place_industries(&mut state, true);
                        log_detection_summary(&state, true);
                        return Self {
                            state,
                            loaded_file: true,
                        };
                    }
                    Err(e) => error!("Error al parsear {path}: {e:?}"),
                },
                Err(e) => error!("No se pudo leer {path}: {e}"),
            }
        }
        let mut state = GameState::new(MAP_W, MAP_H);
        distribute_tile_kinds(&mut state, 0xDEAD_BEEF_CAFE_1234);
        place_industries(&mut state, false);
        place_stations(&mut state);
        place_roads(&mut state);
        place_vehicles(&mut state);
        log_detection_summary(&state, false);
        Self {
            state,
            loaded_file: false,
        }
    }
}

fn log_detection_summary(state: &GameState, loaded_from_file: bool) {
    let (mw, mh) = state.map.dimensions();
    info!("Resumen detección: mapa {mw}x{mh} ({} teselas)", mw * mh);

    let mut tiles: BTreeMap<String, u32> = BTreeMap::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            let Some(kind) = state.map.get_kind(c) else {
                continue;
            };
            let key = match kind {
                TileKind::Grass => "Grass".to_string(),
                TileKind::Water => "Water".to_string(),
                TileKind::Forest => "Forest".to_string(),
                TileKind::CoalField => "CoalField".to_string(),
                TileKind::Road => "Road".to_string(),
                TileKind::Rail => "Rail".to_string(),
                TileKind::House => "House".to_string(),
                TileKind::Station => "Station".to_string(),
                TileKind::Industry => "Industry".to_string(),
                TileKind::Void => "Void".to_string(),
                TileKind::Unknown(v) => format!("Unknown({v})"),
            };
            *tiles.entry(key).or_insert(0) += 1;
        }
    }

    info!("Teselas detectadas por tipo:");
    for (kind, count) in tiles {
        info!("  - {kind}: {count}");
    }

    if loaded_from_file {
        let mut industry_groups: BTreeMap<&'static str, u32> = BTreeMap::new();
        for y in 0..mh {
            for x in 0..mw {
                let c = TileCoord::new(x as i32, y as i32);
                let Some(tile) = state.map.get(c) else {
                    continue;
                };
                if tile.kind != TileKind::Industry {
                    continue;
                }
                let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                let group = industry_group_from_gfx(gfx);
                *industry_groups.entry(group).or_insert(0) += 1;
            }
        }
        info!("Teselas de industria por grupo OpenTTD (gfx):");
        for (group, count) in industry_groups {
            info!("  - {group}: {count}");
        }
    }

    let mut industries: BTreeMap<&'static str, u32> = BTreeMap::new();
    for ind in &state.industries {
        let key = match ind.kind {
            IndustryKind::CoalMine => "CoalMine",
            IndustryKind::Forest => "Forest",
        };
        *industries.entry(key).or_insert(0) += 1;
    }
    info!("Industrias detectadas: {}", state.industries.len());
    for (kind, count) in industries {
        info!("  - Industria {kind}: {count}");
    }

    info!("Estaciones detectadas: {}", state.stations.len());
    if loaded_from_file && state.stations.is_empty() {
        info!("  - Nota: en mapas .ottdmap todavía no se sintetizan estaciones para la simulación.");
    }

    let mut vehicles: BTreeMap<&'static str, u32> = BTreeMap::new();
    for v in &state.vehicles {
        let key = match v.kind {
            VehicleKind::Truck => "Truck",
        };
        *vehicles.entry(key).or_insert(0) += 1;
    }
    info!("Vehículos detectados: {}", state.vehicles.len());
    if loaded_from_file && state.vehicles.is_empty() {
        info!("  - Nota: en mapas .ottdmap todavía no se sintetizan vehículos para la simulación.");
    }
    for (kind, count) in vehicles {
        info!("  - Vehículo {kind}: {count}");
    }
}

fn distribute_tile_kinds(state: &mut GameState, seed: u64) {
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let kind = tile_kind_hash(x, y, seed);
            let c = TileCoord::new(x as i32, y as i32);
            let _ = state.map.set_kind(c, kind);
        }
    }
}

fn tile_kind_hash(x: u32, y: u32, seed: u64) -> TileKind {
    let mut h = seed
        .wrapping_add(u64::from(x).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(u64::from(y).wrapping_mul(0x6C62_272E_07BB_0142));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    match h % 10 {
        0 | 1 => TileKind::Water,
        2 | 3 => TileKind::Forest,
        4 => TileKind::CoalField,
        _ => TileKind::Grass,
    }
}

pub fn place_industries(state: &mut GameState, from_ottd_file: bool) {
    let (mw, mh) = state.map.dimensions();
    let mut coal_n = 0u32;
    let mut forest_n = 0u32;
    let mut industry_n = 0u32;

    let stride_proc = 4u32;
    let stride_ottd = 16u32;

    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            let Some(tile) = state.map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::CoalField if !from_ottd_file => {
                    if coal_n.is_multiple_of(stride_proc) {
                        state
                            .industries
                            .push(Industry::new(c, IndustryKind::CoalMine));
                    }
                    coal_n += 1;
                }
                TileKind::Forest if !from_ottd_file => {
                    if forest_n.is_multiple_of(stride_proc) {
                        state
                            .industries
                            .push(Industry::new(c, IndustryKind::Forest));
                    }
                    forest_n += 1;
                }
                TileKind::Industry => {
                    if industry_n.is_multiple_of(stride_ottd) {
                        let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                        let kind = classify_industry_kind_from_gfx(gfx);
                        state.industries.push(Industry::new(c, kind));
                    }
                    industry_n += 1;
                }
                _ => {}
            }
        }
    }
}

fn classify_industry_kind_from_gfx(gfx: u16) -> IndustryKind {
    // El core todavía modela dos tipos de industria (CoalMine / Forest).
    // Mapeamos grupos OpenTTD reales a uno de esos dos para simulación.
    match gfx {
        // Industrias de extracción/mina → CoalMine
        0..=6 | 47..=51 | 60..=71 | 89..=90 | 91..=99 => IndustryKind::CoalMine,
        // Industrias de bosque/campo/plantación → Forest
        24..=28 | 52..=57 | 72..=88 => IndustryKind::Forest,
        // Industrias de procesamiento/servicios: fallback estable por gfx.
        _ => {
            if gfx.is_multiple_of(2) {
                IndustryKind::CoalMine
            } else {
                IndustryKind::Forest
            }
        }
    }
}

fn industry_group_from_gfx(gfx: u16) -> &'static str {
    match gfx {
        0..=6 => "Coal Mine",
        7..=10 => "Power Station",
        11..=15 => "Sawmill",
        16..=23 => "Oil Refinery",
        24..=28 => "Forest",
        29..=32 => "Printing Works",
        33..=38 => "Oil Rig",
        39..=42 => "Steel Mill",
        43..=46 => "Factory",
        47..=51 => "Oil Wells",
        52..=57 => "Farm",
        58..=59 => "Bank",
        60..=71 => "Copper Ore Mine",
        72..=88 => "Plantations/Others",
        89..=90 => "Gold Mine",
        91..=99 => "Iron Ore Mine",
        100..=119 => "Other climates",
        _ => "Unknown gfx",
    }
}

fn place_stations(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    let positions: Vec<TileCoord> = state
        .industries
        .iter()
        .enumerate()
        .map(|(i, ind)| {
            let dy = if i % 2 == 0 { 3i32 } else { -3i32 };
            TileCoord::new(
                (ind.pos.x + 3).clamp(0, mw as i32 - 1),
                (ind.pos.y + dy).clamp(0, mh as i32 - 1),
            )
        })
        .collect();
    for pos in positions {
        state.stations.push(Station::new(pos));
    }
}

fn place_roads(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (from, to) in routes {
        let mut cur = from;
        while cur.x != to.x {
            cur.x += (to.x - cur.x).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
        while cur.y != to.y {
            cur.y += (to.y - cur.y).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
    }
}

fn place_vehicles(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (i, (a, b)) in routes.into_iter().enumerate() {
        let mut v = Vehicle::new(i as u32, VehicleKind::Truck, a, b);
        if let Some(path) = find_path(&state.map, a, b) {
            v.path = path.into_iter().collect();
        }
        state.vehicles.push(v);
    }
}
