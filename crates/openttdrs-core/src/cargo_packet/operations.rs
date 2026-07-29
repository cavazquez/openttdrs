//! Operaciones de carga/descarga y políticas de cargo packets.

use crate::cargo::CargoType;
use crate::map::TileCoord;
use crate::vehicle::OrderUnloadType;

use super::types::{CargoPacket, CargoUnloadAction, VehicleCargoList};

/// Unidades transferidas por tick en carga/descarga gradual (MVP).
///
/// Valores altos para pax/mail (rápido) y más bajos para bulk, alineados a la
/// idea de `LoadUnloadVehicle` sin copiar tablas `NewGRF`.
#[must_use]
pub const fn load_unload_speed(cargo: CargoType) -> u32 {
    match cargo {
        CargoType::Passengers => 8,
        CargoType::Mail => 6,
        CargoType::Goods
        | CargoType::Valuables
        | CargoType::Livestock
        | CargoType::Candy
        | CargoType::Toys
        | CargoType::FizzyDrinks
        | CargoType::Food
        | CargoType::Gold
        | CargoType::Diamonds => 5,
        _ => 4,
    }
}

/// Decide si el packet debe bajarse en `at` según `next_hop` y flags de orden.
///
/// `unload_type`: política completa `OrderUnloadType` de la orden. El reinsert
/// físico de freight en cola **no** implica trasbordo económico: sin este flag la
/// bajada cobra como entrega final (`PayFinalDelivery`). Con él solo se acumula
/// `feeder_share` (`PayTransfer`).
#[must_use]
pub fn decide_cargo_unload_action(
    packet: &CargoPacket,
    at: TileCoord,
    force_transfer: bool,
) -> CargoUnloadAction {
    // Pax/mail: nunca entregar en la estación de embarque (next_hop None +
    // cargo_source=casa generaban ingreso fantasma en el origen).
    if packet.cargo.is_town_cargo() && packet.first_station == Some(at) {
        return CargoUnloadAction::Keep;
    }
    if packet.next_hop.is_some_and(|hop| hop != at) {
        return CargoUnloadAction::Keep;
    }
    if force_transfer {
        CargoUnloadAction::Transfer
    } else {
        CargoUnloadAction::Deliver
    }
}

/// `ChooseAction` / `Stage` — clasificación de un packet (P2.19).
#[must_use]
pub fn choose_cargo_action(
    packet: &CargoPacket,
    at: TileCoord,
    next_stations: &[TileCoord],
    unload_type: OrderUnloadType,
    accepted: bool,
) -> CargoUnloadAction {
    if unload_type == OrderUnloadType::NoUnload {
        return CargoUnloadAction::Keep;
    }
    if unload_type == OrderUnloadType::Unload && accepted && packet.first_station != Some(at) {
        return CargoUnloadAction::Deliver;
    }
    if matches!(
        unload_type,
        OrderUnloadType::Unload | OrderUnloadType::Transfer
    ) {
        return CargoUnloadAction::Transfer;
    }
    if packet.cargo.is_town_cargo() && packet.first_station == Some(at) {
        return CargoUnloadAction::Keep;
    }
    match packet.next_hop {
        None => {
            // OpenTTD: Deliver solo si accepted && first != current.
            // Freight sin hop (ruta Manual / industria→estación) se entrega o
            // reinserta aunque `first_station` sea esta misma estación.
            if !accepted || (packet.cargo.is_town_cargo() && packet.first_station == Some(at)) {
                CargoUnloadAction::Keep
            } else {
                CargoUnloadAction::Deliver
            }
        }
        Some(hop) if hop == at => CargoUnloadAction::Deliver,
        Some(hop) if next_stations.contains(&hop) => CargoUnloadAction::Keep,
        Some(_) => CargoUnloadAction::Transfer,
    }
}

/// `PrepareUnload` — clasifica la carga a bordo antes de la descarga gradual.
///
/// Usa `GetNextStoppingStation` + `Stage` (P2.19 / P2.22).
pub fn prepare_unload(
    cargo: &mut VehicleCargoList,
    accepted: bool,
    current_station: TileCoord,
    next_stations: &[TileCoord],
    unload_type: OrderUnloadType,
) -> bool {
    cargo.stage(accepted, current_station, next_stations, unload_type)
}
