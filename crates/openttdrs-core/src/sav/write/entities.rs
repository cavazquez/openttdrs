//! Serialización de entidades: estaciones, ciudades, industrias.

use super::super::SavError;
use super::codec::write_str;
use crate::game_state::GameState;
use crate::industry::{Industry, IndustryKind, IndustrySpec};
use crate::map::coord_to_linear_index;
use crate::station::StopKind;

/// Bits `FACIL_*` al escribir `STNN` (alineados con el import).
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;
const FACIL_DOCK: u8 = 0x10;
const FACIL_WAYPOINT: u8 = 0x80;

fn facilities_for_stop(kind: StopKind) -> u8 {
    match kind {
        StopKind::RailStation => FACIL_TRAIN,
        StopKind::TruckStop => FACIL_TRUCK_STOP,
        StopKind::BusStop => FACIL_BUS_STOP,
        StopKind::Dock | StopKind::Buoy => FACIL_DOCK,
        StopKind::Airport => FACIL_AIRPORT,
        StopKind::RailWaypoint => FACIL_WAYPOINT | FACIL_TRAIN,
        StopKind::RoadWaypoint => FACIL_WAYPOINT | FACIL_BUS_STOP | FACIL_TRUCK_STOP,
    }
}

/// Construye records STNN desde estaciones del estado.
///
/// # Errors
///
/// Falla si algún nombre de estación es demasiado largo.
pub(super) fn stnn_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let mut out = Vec::with_capacity(state.stations.len());
    for st in &state.stations {
        if st.pos.x < 0 || st.pos.y < 0 {
            continue;
        }
        let ux = st.pos.x.cast_unsigned();
        let uy = st.pos.y.cast_unsigned();
        let tile_idx = uy.saturating_mul(map_w).saturating_add(ux);
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        let name = st.name.as_deref().unwrap_or("");
        write_str(name, &mut rec)?;
        rec.push(facilities_for_stop(st.stop_kind));
        out.push(rec);
    }
    Ok(out)
}

/// Construye records CITY desde ciudades del estado.
///
/// # Errors
///
/// Falla si algún nombre de ciudad es demasiado largo.
pub(super) fn city_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let mut out = Vec::with_capacity(state.towns.len());
    for town in &state.towns {
        let Some(tile_idx) = coord_to_linear_index(town.pos, map_w) else {
            continue;
        };
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        write_str(&town.name, &mut rec)?;
        // cache.population: el import la pone en 0 y rebuild_town_populations la recalcula;
        // igual la escribimos para roundtrip de lectura best-effort / fixtures.
        rec.extend_from_slice(&town.population.to_be_bytes());
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
        rec.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype (inglés)
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnameparts
        out.push(rec);
    }
    Ok(out)
}

fn industry_ottd_type(ind: &Industry) -> u8 {
    // Índices temperate OpenTTD (`table/industry.h`); best-effort.
    let spec = ind.spec.unwrap_or(match ind.kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    });
    match spec {
        IndustrySpec::CoalMine => 0,
        IndustrySpec::PowerStation => 1,
        IndustrySpec::Sawmill => 2,
        IndustrySpec::Forest => 3,
        IndustrySpec::OilRefinery => 4,
        IndustrySpec::OilWells => 5,
        IndustrySpec::Farm => 6,
        IndustrySpec::Factory => 7,
        IndustrySpec::IronOreMine => 8,
        IndustrySpec::GoldMine => 18,
        IndustrySpec::CopperOreMine => 24,
        other => {
            let _ = other;
            0
        }
    }
}

fn industry_footprint(ind: &Industry) -> (u8, u8) {
    if ind.tiles.is_empty() {
        return (1, 1);
    }
    let min_x = ind.tiles.iter().map(|t| t.x).min().unwrap_or(ind.pos.x);
    let max_x = ind.tiles.iter().map(|t| t.x).max().unwrap_or(ind.pos.x);
    let min_y = ind.tiles.iter().map(|t| t.y).min().unwrap_or(ind.pos.y);
    let max_y = ind.tiles.iter().map(|t| t.y).max().unwrap_or(ind.pos.y);
    let w = u8::try_from((max_x - min_x + 1).clamp(1, 255)).unwrap_or(1);
    let h = u8::try_from((max_y - min_y + 1).clamp(1, 255)).unwrap_or(1);
    (w, h)
}

pub(super) fn indy_records(state: &GameState, map_w: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(state.industries.len());
    for ind in &state.industries {
        let Some(tile_idx) = coord_to_linear_index(ind.pos, map_w) else {
            continue;
        };
        let (w, h) = industry_footprint(ind);
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        rec.push(w);
        rec.push(h);
        rec.push(industry_ottd_type(ind));
        out.push(rec);
    }
    out
}
