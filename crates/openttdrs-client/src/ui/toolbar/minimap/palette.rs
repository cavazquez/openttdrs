use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::sprites::company_colour_swatch_color;

use super::MinimapLayerState;

const INDUSTRY_DIM: Color = Color::srgb(0.22, 0.28, 0.14);
const OWNER_NEUTRAL: Color = Color::srgb(0.35, 0.35, 0.32);

pub(super) fn minimap_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Water => Color::srgb(0.08, 0.25, 0.55),
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel => {
            Color::srgb(0.48, 0.42, 0.32)
        }
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel => {
            Color::srgb(0.68, 0.68, 0.62)
        }
        TileKind::House => Color::srgb(0.72, 0.28, 0.2),
        TileKind::Industry | TileKind::CoalField => Color::srgb(0.78, 0.64, 0.2),
        TileKind::Station => Color::srgb(0.95, 0.95, 0.86),
        TileKind::Forest => Color::srgb(0.05, 0.34, 0.1),
        TileKind::Grass => Color::srgb(0.16, 0.48, 0.12),
        TileKind::ShipDepot => Color::srgb(0.06, 0.22, 0.48),
        TileKind::Airport => Color::srgb(0.55, 0.55, 0.5),
        TileKind::Void => Color::srgb(0.02, 0.02, 0.02),
        TileKind::Unknown(_) => Color::srgb(0.38, 0.12, 0.45),
    }
}

pub(super) fn minimap_cell_color(
    state: &GameState,
    layers: &MinimapLayerState,
    coord: TileCoord,
    kind: TileKind,
) -> Color {
    let mut color = minimap_color(kind);

    if matches!(kind, TileKind::Industry | TileKind::CoalField) {
        color = if layers.industries {
            Color::srgb(0.92, 0.72, 0.18)
        } else {
            INDUSTRY_DIM
        };
    }

    if layers.owners {
        if let Some(owner) = owner_for_tile(state, coord, kind) {
            color = company_color(state, owner);
        } else if is_owned_infra(kind) {
            color = OWNER_NEUTRAL;
        }
    }

    if layers.vehicles
        && state
            .vehicles
            .iter()
            .any(|v| v.is_consist_head() && v.pos == coord)
    {
        let owner = state
            .vehicles
            .iter()
            .find(|v| v.is_consist_head() && v.pos == coord)
            .map(|v| v.owner)
            .unwrap_or(CompanyId::PLAYER);
        color = company_color(state, owner);
    }

    color
}

fn is_owned_infra(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail
            | TileKind::RailDepot
            | TileKind::RailBridge
            | TileKind::RailTunnel
            | TileKind::Road
            | TileKind::RoadDepot
            | TileKind::RoadBridge
            | TileKind::RoadTunnel
            | TileKind::Station
            | TileKind::Airport
            | TileKind::ShipDepot
    )
}

fn owner_for_tile(state: &GameState, coord: TileCoord, kind: TileKind) -> Option<CompanyId> {
    if let Some(station) = state.stations.iter().find(|s| s.covers_tile(coord)) {
        return Some(station.owner);
    }
    if matches!(kind, TileKind::Station | TileKind::Airport) {
        // Ancla o tesela de estación sin covers_tile (p. ej. andén rail).
        if let Some(station) = state.stations.iter().min_by_key(|s| {
            let dx = (s.pos.x - coord.x).unsigned_abs();
            let dy = (s.pos.y - coord.y).unsigned_abs();
            dx.saturating_add(dy)
        }) && (station.pos.x - coord.x).unsigned_abs() <= 4
            && (station.pos.y - coord.y).unsigned_abs() <= 4
        {
            return Some(station.owner);
        }
    }
    if is_owned_infra(kind) {
        let m1 = state.map.get(coord).map(|t| t.m1).unwrap_or(0);
        return Some(CompanyId::from_tile_m1(m1, state.companies.len()));
    }
    None
}

fn company_color(state: &GameState, owner: CompanyId) -> Color {
    let colour = state
        .companies
        .get(owner.index())
        .map(|c| c.colour)
        .unwrap_or(state.company_colour);
    company_colour_swatch_color(colour)
}
