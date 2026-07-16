//! Costos de compra, venta y explotación de vehículos.

use crate::vehicle::{Vehicle, VehicleKind};

/// Precio de compra del motor por defecto del tipo (catálogo del original).
#[must_use]
pub fn vehicle_purchase_cost(kind: VehicleKind) -> i64 {
    crate::engine::engine_for_vehicle(kind, crate::engine::default_engine_id(kind)).price
}

/// Reembolso al vender en depósito (~50 % del precio del modelo del vehículo).
#[must_use]
pub fn vehicle_sell_refund(vehicle: &Vehicle) -> i64 {
    let base = vehicle.effective_engine().price;
    (base * 50) / 100
}

/// Coste de explotación por tick (solo en movimiento, como corrida en `OpenTTD`).
#[must_use]
pub const fn vehicle_running_cost_per_tick(kind: VehicleKind, running: bool, moving: bool) -> i64 {
    if !running || !moving {
        return 0;
    }
    match kind {
        VehicleKind::Truck => 3,
        VehicleKind::Bus | VehicleKind::Tram => 2,
        VehicleKind::Train => 8,
        VehicleKind::Ship => 5,
        VehicleKind::Aircraft => 10,
    }
}
