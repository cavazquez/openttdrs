//! Gestión de carga del vehículo: packets, sincronización, carga/descarga.

use crate::cargo::CargoType;
use crate::map::TileCoord;

impl super::model::Vehicle {
    pub(crate) fn mark_cargo_loaded(&mut self, at: TileCoord) {
        if self.cargo_source.is_none() {
            self.cargo_source = Some(at);
        }
        // No resetear transit si ya hay packets a bordo (carga gradual / top-up).
        if self.cargo_packets.is_empty() {
            self.cargo_transit_ticks = 0;
        }
    }

    /// Sincroniza campos agregados desde `cargo_packets`.
    pub fn sync_cargo_from_packets(&mut self) {
        self.cargo = self.cargo_packets.total();
        if self.cargo == 0 {
            self.cargo_type = match self.kind {
                super::model::VehicleKind::Bus
                | super::model::VehicleKind::Tram
                | super::model::VehicleKind::Aircraft => Some(CargoType::Passengers),
                super::model::VehicleKind::Truck
                | super::model::VehicleKind::Train
                | super::model::VehicleKind::Ship => None,
            };
            self.cargo_source = None;
            self.cargo_transit_ticks = 0;
            return;
        }
        if let Some(ct) = self.cargo_packets.primary_type() {
            self.cargo_type = Some(ct);
        }
        self.cargo_source = self.cargo_packets.primary_source();
        let days = self.cargo_packets.max_periods_in_transit();
        self.cargo_transit_ticks =
            u32::from(days).saturating_mul(crate::economy::TICKS_PER_TRANSIT_DAY);
    }

    /// Hidrata packets desde campos legacy si la lista está vacía.
    pub fn ensure_packets_from_legacy(&mut self) {
        if self.cargo_packets.is_empty() && self.cargo > 0 {
            let days = crate::economy::ticks_to_transit_days(self.cargo_transit_ticks);
            let cargo_type = self.cargo_type.or(match self.kind {
                super::model::VehicleKind::Bus
                | super::model::VehicleKind::Tram
                | super::model::VehicleKind::Aircraft => Some(CargoType::Passengers),
                super::model::VehicleKind::Truck
                | super::model::VehicleKind::Train
                | super::model::VehicleKind::Ship => Some(CargoType::Goods),
            });
            self.cargo_packets = crate::cargo_packet::VehicleCargoList::from_legacy(
                self.cargo,
                cargo_type,
                self.cargo_source,
                days,
                self.pos,
            );
        }
        self.sync_cargo_from_packets();
    }

    /// ¿Hay transferencia gradual (carga o descarga) en curso?
    #[must_use]
    pub fn cargo_transfer_active(&self) -> bool {
        self.cargo_loading || self.cargo_unloading
    }

    pub(crate) fn clear_cargo(&mut self) {
        self.cargo_packets.clear();
        self.cargo_loading = false;
        self.cargo_unloading = false;
        self.cargo = 0;
        self.cargo_type = match self.kind {
            super::model::VehicleKind::Bus
            | super::model::VehicleKind::Tram
            | super::model::VehicleKind::Aircraft => Some(CargoType::Passengers),
            super::model::VehicleKind::Truck
            | super::model::VehicleKind::Train
            | super::model::VehicleKind::Ship => None,
        };
        self.cargo_source = None;
        self.cargo_transit_ticks = 0;
    }
}
