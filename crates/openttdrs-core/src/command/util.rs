use crate::GameState;
use crate::map::TileCoord;

use super::types::CommandError;

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
