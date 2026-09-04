//! Gestión de carga del vehículo: packets, sincronización, carga/descarga.

use crate::cargo::CargoType;
use crate::map::TileCoord;

impl super::model::Vehicle {
    /// Datos que la estación recuerda de este vehículo para el rating (`economy.cpp:1745-1765`).
    ///
    /// Cada tipo usa su propia unidad de velocidad: trenes y barcos la máxima tal cual, los de
    /// carretera la mitad y las aeronaves convertidas a las unidades antiguas.
    #[must_use]
    pub fn station_visit(&self, tick: u64) -> crate::station::StationVisit {
        self.station_visit_with_speed(tick, u32::from(self.effective_engine().max_speed))
    }

    /// Variante runtime que aplica CB36 antes de registrar la visita.
    pub fn station_visit_with_callbacks(&mut self, tick: u64) -> crate::station::StationVisit {
        self.station_visit_with_callbacks_and_catalog(tick, &[])
    }

    /// Variante runtime que resuelve el motor `NewGRF` desde el catálogo activo.
    pub fn station_visit_with_callbacks_and_catalog(
        &mut self,
        tick: u64,
        engine_catalog: &[crate::engine::EngineDef],
    ) -> crate::station::StationVisit {
        let max_speed = u32::from(
            crate::newgrf_callback::effective_vehicle_max_speed_with_catalog(engine_catalog, self),
        );
        self.station_visit_with_speed(tick, max_speed)
    }

    fn station_visit_with_speed(&self, tick: u64, max_speed: u32) -> crate::station::StationVisit {
        use super::model::VehicleKind;

        let speed = match self.kind {
            VehicleKind::Train | VehicleKind::Ship => max_speed,
            VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Truck => max_speed / 2,
            VehicleKind::Aircraft => max_speed * 10 / 128,
        };
        let age_ticks = tick.saturating_sub(self.build_tick);
        let age_years = age_ticks / crate::economy::TICKS_PER_YEAR;

        crate::station::StationVisit {
            vehicle_kind: self.kind,
            last_speed: u8::try_from(speed.min(255)).unwrap_or(255),
            last_age: u8::try_from(age_years.min(255)).unwrap_or(255),
        }
    }

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
            // El tipo elegido por un refit permanece en la unidad aun cuando
            // todavía no haya packets. `refit_capacity` distingue ese estado
            // de los vehículos legacy que dejan que la próxima estación
            // seleccione cualquier carga compatible.
            let refitted_type = (self.refit_capacity > 0)
                .then_some(self.cargo_type)
                .flatten();
            self.cargo_type = match self.kind {
                super::model::VehicleKind::Bus
                | super::model::VehicleKind::Tram
                | super::model::VehicleKind::Aircraft => Some(CargoType::Passengers),
                super::model::VehicleKind::Truck
                | super::model::VehicleKind::Train
                | super::model::VehicleKind::Ship => refitted_type,
            };
            self.cargo_source = None;
            self.cargo_transit_ticks = 0;
            return;
        }
        if let Some(ct) = self.cargo_packets.primary_type() {
            self.cargo_type = Some(ct);
        }
        self.cargo_source = self.cargo_packets.primary_source();
        let periods = self.cargo_packets.max_periods_in_transit();
        self.cargo_transit_ticks =
            u32::from(periods).saturating_mul(crate::economy::CARGO_AGING_TICKS);
    }

    /// Hidrata packets desde campos legacy si la lista está vacía.
    pub fn ensure_packets_from_legacy(&mut self) {
        if self.cargo_packets.is_empty() && self.cargo > 0 {
            let periods = crate::economy::ticks_to_transit_periods(self.cargo_transit_ticks);
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
                periods,
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
        let refitted_type = (self.refit_capacity > 0)
            .then_some(self.cargo_type)
            .flatten();
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
            | super::model::VehicleKind::Ship => refitted_type,
        };
        self.cargo_source = None;
        self.cargo_transit_ticks = 0;
    }
}
