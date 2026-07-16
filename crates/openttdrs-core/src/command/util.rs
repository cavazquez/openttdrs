use crate::GameState;
use crate::company::CompanyId;
use crate::map::{TileCoord, TileKind};

use super::error::CommandError;

pub(crate) fn in_bounds(map: &crate::map::Map, c: TileCoord) -> Result<(), CommandError> {
    if map.get(c).is_none() {
        Err(CommandError::OutOfBounds)
    } else {
        Ok(())
    }
}

/// Exige que el vehículo exista y pertenezca a la compañía activa.
pub(crate) fn require_vehicle_owned_by_active(
    state: &GameState,
    vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    if vehicle.owner != state.active_company {
        return Err(CommandError::VehicleNotOwned);
    }
    Ok(())
}

/// Infraestructura con owner en `m1` (vía / carretera / depósitos).
#[must_use]
pub(crate) fn is_owned_infra_tile_kind(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail
            | TileKind::Road
            | TileKind::RailDepot
            | TileKind::RoadDepot
            | TileKind::ShipDepot
            | TileKind::RailBridge
            | TileKind::RoadBridge
            | TileKind::RailTunnel
            | TileKind::RoadTunnel
    )
}

/// Owner de la tesela: estación si aplica, si no `m1` en infra.
#[must_use]
pub(crate) fn tile_owner(state: &GameState, c: TileCoord) -> Option<CompanyId> {
    if let Some(station) = state
        .stations
        .iter()
        .find(|s| s.covers_tile(c) || s.pos == c)
    {
        return Some(station.owner);
    }
    let tile = state.map.get(c)?;
    if !is_owned_infra_tile_kind(tile.kind) {
        return None;
    }
    Some(CompanyId::from_tile_m1(tile.m1, state.companies.len()))
}

/// Exige que la infra en `c` (si tiene owner) sea de la compañía activa.
pub(crate) fn require_tile_owned_by_active(
    state: &GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    let Some(owner) = tile_owner(state, c) else {
        return Ok(());
    };
    if owner != state.active_company {
        return Err(CommandError::TileNotOwned);
    }
    Ok(())
}
