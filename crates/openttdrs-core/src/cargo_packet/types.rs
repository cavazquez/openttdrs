//! Estructuras de datos de cargo packets y métodos inherentes.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::map::TileCoord;

/// Acción al llegar a una estación (`CargoPaymentAction` / `ChooseAction` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoUnloadAction {
    /// Destino de este hop: pagar y entregar / trasbordar según tipo.
    Deliver,
    /// Bajar para otro vehículo (misma tesela que `next_hop`, no sink final).
    Transfer,
    /// El `next_hop` apunta a otra estación: no descargar aquí.
    Keep,
}

/// Lote de carga con origen y edad (`CargoPacket` de `OpenTTD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoPacket {
    pub cargo: CargoType,
    pub count: u16,
    pub source: TileCoord,
    /// Periodos de tránsito (incrementa cada `CARGO_AGING_TICKS` = 185 ticks, ~2,5 días).
    #[serde(default)]
    pub periods_in_transit: u16,
    /// Estación de primer embarque (feeder / `CargoDist`).
    #[serde(default)]
    pub first_station: Option<TileCoord>,
    /// Crédito feeder ya liquidado en este packet (evita doble pago).
    #[serde(default)]
    pub feeder_paid: bool,
    /// Acumulado de pagos feeder (`Money feeder_share` en `OpenTTD`).
    #[serde(default)]
    pub feeder_share: i64,
    /// Siguiente estación de la ruta (`CargoPacket::next_hop` / `FlowStat`).
    #[serde(default)]
    pub next_hop: Option<TileCoord>,
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
            feeder_share: 0,
            next_hop: None,
        }
    }

    #[must_use]
    pub fn with_first_station(mut self, station: TileCoord) -> Self {
        self.first_station = Some(station);
        self
    }

    #[must_use]
    pub fn with_next_hop(mut self, hop: Option<TileCoord>) -> Self {
        self.next_hop = hop;
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
            && last.feeder_share == packet.feeder_share
            && last.next_hop == packet.next_hop
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
        for cargo in ALL_CARGO_TYPES {
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

    /// Envejece un periodo los packets en espera (rating / decay ligero).
    pub fn age_waiting_one_period(&mut self) {
        for p in &mut self.packets {
            p.periods_in_transit = p.periods_in_transit.saturating_add(1);
        }
    }

    /// Elimina toda la carga en espera de un tipo (`TruncateCargo` en `OpenTTD`).
    pub fn truncate_cargo(&mut self, cargo: CargoType) {
        self.packets.retain(|p| p.cargo != cargo);
    }

    /// Descarta hasta `amount` unidades del tipo, empezando por lo más viejo
    /// (`TruncateCargo` con `max_move`). Devuelve lo realmente descartado.
    pub fn truncate_cargo_amount(&mut self, cargo: CargoType, amount: u32) -> u32 {
        let removed: u32 = self
            .take(cargo, amount)
            .iter()
            .map(|p| u32::from(p.count))
            .sum();
        removed
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
            && last.feeder_share == packet.feeder_share
            && last.next_hop == packet.next_hop
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
                if p.feeder_share > 0 {
                    let share = (i64::from(p.feeder_share) * i64::from(take))
                        / i64::from(p.count);
                    taken.feeder_share = share;
                    p.feeder_share = p.feeder_share.saturating_sub(share);
                }
                p.count -= take;
                left = 0;
                out.push(taken);
                kept.push(p);
            }
        }
        self.packets = kept;
        out
    }

    /// Envejece un periodo todos los packets a bordo.
    pub fn age_one_period(&mut self) {
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
