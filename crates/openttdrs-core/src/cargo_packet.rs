//! Packets de carga al estilo `OpenTTD` (`cargopacket.h`).
//!
//! Cada lote lleva origen y edad de tránsito; la estación y el vehículo
//! mantienen colas FIFO. Los balances agregados (`CargoStock` / `Vehicle.cargo`)
//! se sincronizan desde estas listas.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::cargo::{CargoStock, CargoType};
use crate::map::TileCoord;

/// Unidades transferidas por tick en carga/descarga gradual (MVP).
///
/// Valores altos para pax/mail (rápido) y más bajos para bulk, alineados a la
/// idea de `LoadUnloadVehicle` sin copiar tablas `NewGRF`.
#[must_use]
pub const fn load_unload_speed(cargo: CargoType) -> u32 {
    match cargo {
        CargoType::Passengers => 8,
        CargoType::Mail => 6,
        CargoType::Goods => 5,
        CargoType::Coal | CargoType::Wood | CargoType::Oil => 4,
    }
}

/// Lote de carga con origen y edad (`CargoPacket` de `OpenTTD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoPacket {
    pub cargo: CargoType,
    pub count: u16,
    pub source: TileCoord,
    /// Días de tránsito (incrementa cada `TICKS_PER_TRANSIT_DAY` a bordo).
    #[serde(default)]
    pub periods_in_transit: u16,
    /// Estación de primer embarque (feeder / Cargo Dist).
    #[serde(default)]
    pub first_station: Option<TileCoord>,
    /// Crédito feeder ya liquidado en este packet (evita doble pago).
    #[serde(default)]
    pub feeder_paid: bool,
}

impl CargoPacket {
    #[must_use]
    pub fn new(cargo: CargoType, count: u16, source: TileCoord) -> Self {
        Self {
            cargo,
            count,
            source,
            periods_in_transit: 0,
            first_station: None,
            feeder_paid: false,
        }
    }

    #[must_use]
    pub fn with_first_station(mut self, station: TileCoord) -> Self {
        self.first_station = Some(station);
        self
    }
}

/// Cola de packets en estación (FIFO por tipo al extraer).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationCargoList {
    #[serde(default)]
    pub packets: VecDeque<CargoPacket>,
}

impl StationCargoList {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    #[must_use]
    pub fn total_of(&self, cargo: CargoType) -> u32 {
        self.packets
            .iter()
            .filter(|p| p.cargo == cargo)
            .map(|p| u32::from(p.count))
            .fold(0, u32::saturating_add)
    }

    #[must_use]
    pub fn as_stock(&self) -> CargoStock {
        let mut stock = CargoStock::default();
        for p in &self.packets {
            stock.add(p.cargo, u32::from(p.count));
        }
        stock
    }

    /// Añade o fusiona con el último packet del mismo tipo/origen/edad.
    pub fn push(&mut self, packet: CargoPacket) {
        if packet.count == 0 {
            return;
        }
        if let Some(last) = self.packets.back_mut()
            && last.cargo == packet.cargo
            && last.source == packet.source
            && last.periods_in_transit == packet.periods_in_transit
            && last.first_station == packet.first_station
            && last.feeder_paid == packet.feeder_paid
        {
            last.count = last.count.saturating_add(packet.count);
            return;
        }
        self.packets.push_back(packet);
    }

    pub fn add_amount(&mut self, cargo: CargoType, amount: u32, source: TileCoord) {
        if amount == 0 {
            return;
        }
        let mut left = amount;
        while left > 0 {
            let chunk = left.min(u32::from(u16::MAX));
            #[allow(clippy::cast_possible_truncation)]
            self.push(CargoPacket::new(cargo, chunk as u16, source).with_first_station(source));
            left -= chunk;
        }
    }

    /// Extrae hasta `amount` unidades del tipo `cargo` (FIFO).
    pub fn take(&mut self, cargo: CargoType, amount: u32) -> Vec<CargoPacket> {
        if amount == 0 {
            return Vec::new();
        }
        let mut left = amount;
        let mut out = Vec::new();
        let mut kept = VecDeque::new();
        for mut p in self.packets.drain(..) {
            if left == 0 || p.cargo != cargo {
                kept.push_back(p);
                continue;
            }
            let available = u32::from(p.count);
            if available <= left {
                left -= available;
                out.push(p);
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let take = left as u16;
                let mut taken = p.clone();
                taken.count = take;
                p.count -= take;
                left = 0;
                out.push(taken);
                kept.push_back(p);
            }
        }
        self.packets = kept;
        out
    }

    /// Migra un balance agregado a packets sintéticos.
    #[must_use]
    pub fn from_stock(stock: CargoStock, source: TileCoord) -> Self {
        let mut list = Self::default();
        for cargo in [
            CargoType::Passengers,
            CargoType::Mail,
            CargoType::Goods,
            CargoType::Coal,
            CargoType::Wood,
            CargoType::Oil,
        ] {
            list.add_amount(cargo, stock.get(cargo), source);
        }
        list
    }

    #[must_use]
    pub fn pick_freight_to_load(&self, preferred: Option<CargoType>) -> Option<CargoType> {
        self.as_stock().pick_freight_to_load(preferred)
    }

    /// Edad máxima (días) del packet más viejo de un tipo (para rating).
    #[must_use]
    pub fn oldest_waiting_days(&self, cargo: CargoType) -> u8 {
        self.packets
            .iter()
            .filter(|p| p.cargo == cargo)
            .map(|p| u8::try_from(p.periods_in_transit.min(255)).unwrap_or(255))
            .max()
            .unwrap_or(0)
    }

    /// Envejece packets en espera un día (rating / decay ligero).
    pub fn age_waiting_one_day(&mut self) {
        for p in &mut self.packets {
            p.periods_in_transit = p.periods_in_transit.saturating_add(1);
        }
    }

    /// Elimina toda la carga en espera de un tipo (`TruncateCargo` en `OpenTTD`).
    pub fn truncate_cargo(&mut self, cargo: CargoType) {
        self.packets.retain(|p| p.cargo != cargo);
    }
}

/// Carga a bordo del vehículo como lista de packets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleCargoList {
    #[serde(default)]
    pub packets: Vec<CargoPacket>,
}

impl VehicleCargoList {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.packets
            .iter()
            .map(|p| u32::from(p.count))
            .fold(0, u32::saturating_add)
    }

    #[must_use]
    pub fn primary_type(&self) -> Option<CargoType> {
        self.packets.first().map(|p| p.cargo)
    }

    #[must_use]
    pub fn primary_source(&self) -> Option<TileCoord> {
        self.packets.first().map(|p| p.source)
    }

    /// Máximo `periods_in_transit` a bordo (compat con reloj global).
    #[must_use]
    pub fn max_periods_in_transit(&self) -> u16 {
        self.packets
            .iter()
            .map(|p| p.periods_in_transit)
            .max()
            .unwrap_or(0)
    }

    pub fn clear(&mut self) {
        self.packets.clear();
    }

    pub fn push(&mut self, packet: CargoPacket) {
        if packet.count == 0 {
            return;
        }
        if let Some(last) = self.packets.last_mut()
            && last.cargo == packet.cargo
            && last.source == packet.source
            && last.periods_in_transit == packet.periods_in_transit
            && last.first_station == packet.first_station
            && last.feeder_paid == packet.feeder_paid
        {
            last.count = last.count.saturating_add(packet.count);
            return;
        }
        self.packets.push(packet);
    }

    pub fn append_packets(&mut self, packets: impl IntoIterator<Item = CargoPacket>) {
        for p in packets {
            self.push(p);
        }
    }

    /// Extrae hasta `amount` unidades (FIFO), mismo tipo que el primero si hay.
    pub fn take_amount(&mut self, amount: u32) -> Vec<CargoPacket> {
        if amount == 0 || self.packets.is_empty() {
            return Vec::new();
        }
        let cargo = self.packets[0].cargo;
        let mut left = amount;
        let mut out = Vec::new();
        let mut kept = Vec::new();
        for mut p in self.packets.drain(..) {
            if left == 0 || p.cargo != cargo {
                kept.push(p);
                continue;
            }
            let available = u32::from(p.count);
            if available <= left {
                left -= available;
                out.push(p);
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let take = left as u16;
                let mut taken = p.clone();
                taken.count = take;
                p.count -= take;
                left = 0;
                out.push(taken);
                kept.push(p);
            }
        }
        self.packets = kept;
        out
    }

    /// Envejece un día todos los packets a bordo.
    pub fn age_one_day(&mut self) {
        for p in &mut self.packets {
            p.periods_in_transit = p.periods_in_transit.saturating_add(1);
        }
    }

    /// Migra carga agregada legacy a un packet.
    #[must_use]
    pub fn from_legacy(
        cargo: u32,
        cargo_type: Option<CargoType>,
        source: Option<TileCoord>,
        transit_days: u16,
        fallback_pos: TileCoord,
    ) -> Self {
        let mut list = Self::default();
        if cargo == 0 {
            return list;
        }
        let Some(ct) = cargo_type else {
            return list;
        };
        let mut left = cargo;
        let src = source.unwrap_or(fallback_pos);
        while left > 0 {
            let chunk = left.min(u32::from(u16::MAX));
            #[allow(clippy::cast_possible_truncation)]
            let mut p = CargoPacket::new(ct, chunk as u16, src);
            p.periods_in_transit = transit_days;
            list.push(p);
            left -= chunk;
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn station_take_splits_packet_fifo() {
        let mut list = StationCargoList::default();
        let src = TileCoord::new(1, 2);
        list.add_amount(CargoType::Coal, 10, src);
        let taken = list.take(CargoType::Coal, 4);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].count, 4);
        assert_eq!(list.total_of(CargoType::Coal), 6);
    }

    #[test]
    fn vehicle_take_amount_preserves_source() {
        let mut list = VehicleCargoList::default();
        list.push(CargoPacket::new(CargoType::Goods, 5, TileCoord::new(0, 0)));
        list.push(CargoPacket::new(CargoType::Goods, 5, TileCoord::new(3, 3)));
        let taken = list.take_amount(7);
        assert_eq!(taken.iter().map(|p| p.count).sum::<u16>(), 7);
        assert_eq!(list.total(), 3);
        assert_eq!(taken[0].source, TileCoord::new(0, 0));
    }

    #[test]
    fn payment_days_follow_packet_age() {
        let mut p = CargoPacket::new(CargoType::Coal, 1, TileCoord::new(0, 0));
        p.periods_in_transit = 10;
        assert_eq!(p.periods_in_transit, 10);
        assert_eq!(load_unload_speed(CargoType::Coal), 4);
        assert_eq!(load_unload_speed(CargoType::Passengers), 8);
    }
}
