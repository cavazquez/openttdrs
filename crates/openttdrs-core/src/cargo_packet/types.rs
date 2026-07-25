//! Estructuras de datos de cargo packets y métodos inherentes.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::map::TileCoord;

/// Acción al llegar a una estación (`MoveToAction` / `ChooseAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoUnloadAction {
    /// Destino de este hop: pagar y entregar.
    Deliver,
    /// Bajar para otro vehículo (trasbordo / feeder).
    Transfer,
    /// El `next_hop` apunta a otra estación: no descargar aquí.
    Keep,
    /// Reservado para carga (`MoveToAction::Load`); no se usa en descarga.
    Load,
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

/// Clave de hop en la cola de estación (`INVALID_STATION` → `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StationHopKey(pub Option<TileCoord>);

impl From<Option<TileCoord>> for StationHopKey {
    fn from(value: Option<TileCoord>) -> Self {
        Self(value)
    }
}

/// Cola de packets en estación indexada por `next_hop` ([`StationCargoList`] de `OpenTTD`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationCargoList {
    /// `MultiMap` `next_hop` → packets (FIFO por hop).
    #[serde(default)]
    pub by_next_hop: BTreeMap<StationHopKey, VecDeque<CargoPacket>>,
    /// Cantidad reservada para carga (`reserved_count`).
    #[serde(default)]
    pub reserved: u32,
    /// Campo legacy `packets` (saves / JSON antiguos); se migra a [`Self::by_next_hop`].
    #[serde(default, alias = "packets")]
    legacy_packets: VecDeque<CargoPacket>,
}

impl StationCargoList {
    fn migrate_legacy(&mut self) {
        if self.legacy_packets.is_empty() {
            return;
        }
        let pending: Vec<_> = self.legacy_packets.drain(..).collect();
        for p in pending {
            self.push(p);
        }
    }

    /// Vista plana FIFO (compat con UI / merge de estaciones).
    pub fn packets(&self) -> impl Iterator<Item = &CargoPacket> {
        self.by_next_hop.values().flat_map(|q| q.iter())
    }

    /// Drena todos los packets (merge de estaciones).
    pub fn drain_all(&mut self) -> Vec<CargoPacket> {
        self.migrate_legacy();
        let mut out = Vec::new();
        for q in self.by_next_hop.values_mut() {
            out.extend(q.drain(..));
        }
        self.by_next_hop.clear();
        out
    }

    /// Acceso mutable al campo plano legacy usado por UI/cliente.
    ///
    /// Mantiene sincronizada la vista indexada: al mutar vía este helper se
    /// reconstruye `by_next_hop` desde la cola plana.
    pub fn packets_mut_flat(&mut self) -> &mut VecDeque<CargoPacket> {
        self.migrate_legacy();
        // Materializar en legacy_packets como buffer editable.
        if self.legacy_packets.is_empty() && !self.by_next_hop.is_empty() {
            self.legacy_packets = self.drain_all().into();
        }
        &mut self.legacy_packets
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_next_hop.values().all(VecDeque::is_empty) && self.legacy_packets.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_next_hop.values().map(VecDeque::len).sum::<usize>() + self.legacy_packets.len()
    }

    #[must_use]
    pub fn total_of(&self, cargo: CargoType) -> u32 {
        self.packets()
            .chain(self.legacy_packets.iter())
            .filter(|p| p.cargo == cargo)
            .map(|p| u32::from(p.count))
            .fold(0, u32::saturating_add)
    }

    #[must_use]
    pub fn total_count(&self) -> u32 {
        self.packets()
            .chain(self.legacy_packets.iter())
            .map(|p| u32::from(p.count))
            .fold(0, u32::saturating_add)
    }

    #[must_use]
    pub fn available_of(&self, cargo: CargoType) -> u32 {
        let total = self.total_of(cargo);
        total.saturating_sub(self.reserved.min(total))
    }

    #[must_use]
    pub fn as_stock(&self) -> CargoStock {
        let mut stock = CargoStock::default();
        for p in self.packets().chain(self.legacy_packets.iter()) {
            stock.add(p.cargo, u32::from(p.count));
        }
        stock
    }

    /// Añade o fusiona con el último packet del mismo hop/tipo/origen/edad.
    pub fn push(&mut self, packet: CargoPacket) {
        self.migrate_legacy();
        if packet.count == 0 {
            return;
        }
        let key = StationHopKey(packet.next_hop);
        let queue = self.by_next_hop.entry(key).or_default();
        if let Some(last) = queue.back_mut()
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
        queue.push_back(packet);
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

    /// Extrae hasta `amount` unidades del tipo `cargo` (FIFO entre hops).
    pub fn take(&mut self, cargo: CargoType, amount: u32) -> Vec<CargoPacket> {
        self.migrate_legacy();
        if amount == 0 {
            return Vec::new();
        }
        let mut left = amount;
        let mut out = Vec::new();
        let keys: Vec<_> = self.by_next_hop.keys().copied().collect();
        for key in keys {
            if left == 0 {
                break;
            }
            let Some(queue) = self.by_next_hop.get_mut(&key) else {
                continue;
            };
            let mut kept = VecDeque::new();
            while let Some(mut p) = queue.pop_front() {
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
            *queue = kept;
        }
        self.by_next_hop.retain(|_, q| !q.is_empty());
        out
    }

    /// Reserva hasta `amount` unidades para carga (marca `reserved`).
    pub fn reserve(&mut self, amount: u32) -> u32 {
        let available = self.total_count().saturating_sub(self.reserved);
        let take = amount.min(available);
        self.reserved = self.reserved.saturating_add(take);
        take
    }

    /// Consume reserva al cargar.
    pub fn consume_reserved(&mut self, amount: u32) {
        self.reserved = self.reserved.saturating_sub(amount);
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
        self.packets()
            .chain(self.legacy_packets.iter())
            .filter(|p| p.cargo == cargo)
            .map(|p| u8::try_from(p.periods_in_transit.min(255)).unwrap_or(255))
            .max()
            .unwrap_or(0)
    }

    /// Envejece un periodo los packets en espera (rating / decay ligero).
    pub fn age_waiting_one_period(&mut self) {
        self.migrate_legacy();
        for q in self.by_next_hop.values_mut() {
            for p in q {
                p.periods_in_transit = p.periods_in_transit.saturating_add(1);
            }
        }
    }

    /// Elimina toda la carga en espera de un tipo (`TruncateCargo` en `OpenTTD`).
    pub fn truncate_cargo(&mut self, cargo: CargoType) {
        self.migrate_legacy();
        for q in self.by_next_hop.values_mut() {
            q.retain(|p| p.cargo != cargo);
        }
        self.by_next_hop.retain(|_, q| !q.is_empty());
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
    /// Conteos por acción tras `Stage` (P2.19).
    #[serde(skip)]
    pub staged_transfer: u32,
    #[serde(skip)]
    pub staged_deliver: u32,
    #[serde(skip)]
    pub staged_keep: u32,
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
        self.staged_transfer = 0;
        self.staged_deliver = 0;
        self.staged_keep = 0;
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
                    let share = (p.feeder_share * i64::from(take)) / i64::from(p.count);
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

    /// `VehicleCargoList::Stage` — clasifica packets en TRANSFER/DELIVER/KEEP.
    ///
    /// Reordena la lista: transfer al frente, deliver en medio, keep al final.
    /// Devuelve `true` si hay algo que descargar.
    pub fn stage(
        &mut self,
        accepted: bool,
        current_station: TileCoord,
        next_stations: &[TileCoord],
        force_transfer: bool,
        no_unload: bool,
    ) -> bool {
        self.staged_transfer = 0;
        self.staged_deliver = 0;
        self.staged_keep = 0;
        if self.packets.is_empty() {
            return false;
        }
        let mut transfer = Vec::new();
        let mut deliver = Vec::new();
        let mut keep = Vec::new();
        for mut cp in self.packets.drain(..) {
            let action = stage_packet(
                &cp,
                accepted,
                current_station,
                next_stations,
                force_transfer,
                no_unload,
            );
            match action {
                CargoUnloadAction::Transfer => {
                    // Trasbordo: next_hop hacia un destino distinto de las siguientes paradas.
                    if cp.next_hop.is_none()
                        || next_stations.iter().any(|s| Some(*s) == cp.next_hop)
                    {
                        // Elegir hop fuera de la ruta actual si hace falta.
                        cp.next_hop = None;
                    }
                    self.staged_transfer = self.staged_transfer.saturating_add(u32::from(cp.count));
                    transfer.push(cp);
                }
                CargoUnloadAction::Deliver => {
                    self.staged_deliver = self.staged_deliver.saturating_add(u32::from(cp.count));
                    deliver.push(cp);
                }
                CargoUnloadAction::Keep | CargoUnloadAction::Load => {
                    self.staged_keep = self.staged_keep.saturating_add(u32::from(cp.count));
                    keep.push(cp);
                }
            }
        }
        self.packets = transfer;
        self.packets.extend(deliver);
        self.packets.extend(keep);
        self.staged_transfer > 0 || self.staged_deliver > 0
    }
}

fn stage_packet(
    packet: &CargoPacket,
    accepted: bool,
    current_station: TileCoord,
    next_stations: &[TileCoord],
    force_transfer: bool,
    no_unload: bool,
) -> CargoUnloadAction {
    // Misma regla que `choose_cargo_action` (evita divergencia Stage/pago).
    super::operations::choose_cargo_action(
        packet,
        current_station,
        next_stations,
        force_transfer,
        no_unload,
        accepted,
    )
}
