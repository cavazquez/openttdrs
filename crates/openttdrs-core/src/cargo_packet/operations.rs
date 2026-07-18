//! Operaciones de carga/descarga y políticas de cargo packets.

use crate::cargo::CargoType;
use crate::map::TileCoord;

use super::types::{CargoPacket, CargoUnloadAction};

/// Unidades transferidas por tick en carga/descarga gradual (MVP).
///
/// Valores altos para pax/mail (rápido) y más bajos para bulk, alineados a la
/// idea de `LoadUnloadVehicle` sin copiar tablas `NewGRF`.
#[must_use]
pub const fn load_unload_speed(cargo: CargoType) -> u32 {
    match cargo {
        CargoType::Passengers => 8,
        CargoType::Mail => 6,
        CargoType::Goods | CargoType::Valuables | CargoType::Livestock => 5,
        CargoType::Coal
        | CargoType::Wood
        | CargoType::Oil
        | CargoType::Grain
        | CargoType::IronOre
        | CargoType::Steel => 4,
    }
}

/// Decide si el packet debe bajarse en `at` según `next_hop`.
///
/// `reinsert_freight`: freight que queda en cola de estación (hub), no sink final.
#[must_use]
pub fn decide_cargo_unload_action(
    packet: &CargoPacket,
    at: TileCoord,
    reinsert_freight: bool,
) -> CargoUnloadAction {
    // Pax/mail: nunca entregar en la estación de embarque (next_hop None +
    // cargo_source=casa generaban ingreso fantasma en el origen).
    if packet.cargo.is_town_cargo() && packet.first_station == Some(at) {
        return CargoUnloadAction::Keep;
    }
    if packet.next_hop.is_some_and(|hop| hop != at) {
        return CargoUnloadAction::Keep;
    }
    if reinsert_freight {
        CargoUnloadAction::Transfer
    } else {
        CargoUnloadAction::Deliver
    }
}
