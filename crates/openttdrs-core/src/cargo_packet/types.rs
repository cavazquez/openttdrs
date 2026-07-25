//! Estructuras de datos de cargo packets y métodos inherentes.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::cargodist::parity::Randomizer;
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

/// Vector acumulado de teselas recorridas en vehículo (`Coord2D` de `OpenTTD`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TravelledVector {
    pub x: i16,
    pub y: i16,
}

/// Lote de carga con origen y edad (`CargoPacket` de `OpenTTD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoPacket {
    pub cargo: CargoType,
    pub count: u16,
    pub source: TileCoord,
    /// Orígen geográfico de pago (`source_xy`); se fija en la primera carga.
    #[serde(default)]
    pub source_xy: Option<TileCoord>,
    /// Vector de distancia recorrida en vehículo (`travelled`).
    #[serde(default)]
    pub travelled: TravelledVector,
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
            source_xy: None,
            travelled: TravelledVector::default(),
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

    /// Parte proporcional de `feeder_share` para `part` unidades (`GetFeederShare`).
    #[must_use]
    pub fn feeder_share_of(&self, part: u16) -> i64 {
        if self.count == 0 || part == 0 {
            return 0;
        }
        self.feeder_share * i64::from(part) / i64::from(self.count)
    }

    /// `CargoPacket::Split` — divide el paquete prorrateando `feeder_share`.
    #[must_use]
    pub fn split(&mut self, new_size: u16) -> Option<Self> {
        if new_size == 0 || new_size >= self.count {
            return None;
        }
        let fs = self.feeder_share_of(new_size);
        let mut taken = self.clone();
        taken.count = new_size;
        taken.feeder_share = fs;
        self.feeder_share = self.feeder_share.saturating_sub(fs);
        self.count -= new_size;
        Some(taken)
    }

    /// `UpdateLoadingTile` — fija `source_xy` y acumula el tile de carga en `travelled`.
    pub fn update_loading_tile(&mut self, tile: TileCoord) {
        if self.source_xy.is_none() {
            self.source_xy = Some(tile);
        }
        self.travelled.x = self.travelled.x.saturating_add(clamp_coord(tile.x));
        self.travelled.y = self.travelled.y.saturating_add(clamp_coord(tile.y));
    }

    /// `UpdateUnloadingTile` — resta el tile de descarga del vector `travelled`.
    pub fn update_unloading_tile(&mut self, tile: TileCoord) {
        self.travelled.x = self.travelled.x.saturating_sub(clamp_coord(tile.x));
        self.travelled.y = self.travelled.y.saturating_sub(clamp_coord(tile.y));
    }

    /// Distancia de pago por tramos (`CargoPacket::GetDistance`).
    ///
    /// Usa el vector recorrido en vehículo, acotado por Manhattan `source_xy`→destino.
    #[must_use]
    pub fn get_distance(&self, current_tile: TileCoord) -> u32 {
        let source = self.source_xy.unwrap_or(self.source);
        let local_x = i32::from(self.travelled.x) - current_tile.x;
        let local_y = i32::from(self.travelled.y) - current_tile.y;
        let distance_travelled = local_x.unsigned_abs() + local_y.unsigned_abs();
        let distance_source_dest = crate::economy::manhattan_distance(source, current_tile);
        distance_travelled.min(distance_source_dest)
    }

    fn same_merge_key(&self, other: &Self) -> bool {
        self.cargo == other.cargo
            && self.source == other.source
            && self.source_xy == other.source_xy
            && self.travelled == other.travelled
            && self.periods_in_transit == other.periods_in_transit
            && self.first_station == other.first_station
            && self.feeder_paid == other.feeder_paid
            && self.feeder_share == other.feeder_share
            && self.next_hop == other.next_hop
    }
}

fn clamp_coord(v: i32) -> i16 {
    i16::try_from(v).unwrap_or(if v < 0 { i16::MIN } else { i16::MAX })
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
            && last.same_merge_key(&packet)
        {
            last.count = last.count.saturating_add(packet.count);
            last.feeder_share = last.feeder_share.saturating_add(packet.feeder_share);
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
                    if let Some(taken) = p.split(take) {
                        left = 0;
                        out.push(taken);
                        kept.push_back(p);
                    } else {
                        kept.push_back(p);
                    }
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

    /// Truncado aleatorio por destino (`StationCargoList::Truncate`).
    ///
    /// Cada hop pierde aproximadamente el mismo porcentaje; opcionalmente
    /// acumula descartes por `first_station` (castigo de rating en origen).
    pub fn truncate_cargo_amount(
        &mut self,
        cargo: CargoType,
        amount: u32,
        rng: &mut Randomizer,
    ) -> (u32, BTreeMap<Option<TileCoord>, u32>) {
        self.migrate_legacy();
        let mut cargo_per_source: BTreeMap<Option<TileCoord>, u32> = BTreeMap::new();
        let total = self.total_of(cargo);
        let max_move = amount.min(total);
        if max_move == 0 {
            return (0, cargo_per_source);
        }

        // Materializar solo los packets del tipo (preservando hops ajenos).
        let mut packets: Vec<CargoPacket> = Vec::new();
        for q in self.by_next_hop.values_mut() {
            let mut kept = VecDeque::new();
            for p in q.drain(..) {
                if p.cargo == cargo {
                    packets.push(p);
                } else {
                    kept.push_back(p);
                }
            }
            *q = kept;
        }
        self.by_next_hop.retain(|_, q| !q.is_empty());

        let mut prev_count = total;
        let mut moved = 0_u32;
        let mut loop_n = 0_u32;
        let mut remaining = packets;
        while max_move > moved && loop_n < 8 {
            let mut next_remaining = Vec::new();
            let mut early_done = false;
            for mut p in remaining {
                if early_done || max_move <= moved {
                    next_remaining.push(p);
                    continue;
                }
                if prev_count > max_move && rng.random_range(prev_count) < prev_count - max_move {
                    if loop_n == 0 {
                        *cargo_per_source.entry(p.first_station).or_default() += u32::from(p.count);
                    }
                    next_remaining.push(p);
                    continue;
                }
                let diff = max_move - moved;
                if u32::from(p.count) > diff {
                    if diff > 0 {
                        #[allow(clippy::cast_possible_truncation)]
                        let take = diff as u16;
                        let _ = p.split(take);
                        moved += diff;
                        if loop_n > 0 {
                            let entry = cargo_per_source.entry(p.first_station).or_default();
                            *entry = entry.saturating_sub(diff);
                            next_remaining.push(p);
                            early_done = true;
                            continue;
                        }
                        *cargo_per_source.entry(p.first_station).or_default() += u32::from(p.count);
                    }
                    next_remaining.push(p);
                } else {
                    let cnt = u32::from(p.count);
                    if loop_n > 0 {
                        let entry = cargo_per_source.entry(p.first_station).or_default();
                        *entry = entry.saturating_sub(cnt);
                    }
                    moved += cnt;
                }
            }
            remaining = next_remaining;
            if early_done || moved >= max_move {
                break;
            }
            loop_n = loop_n.saturating_add(1);
            prev_count = remaining
                .iter()
                .map(|p| u32::from(p.count))
                .fold(0, u32::saturating_add)
                .max(1);
        }
        for p in remaining {
            self.push(p);
        }
        (moved, cargo_per_source)
    }

    /// `StationCargoList::Reroute` — reasigna `next_hop == avoid` a otra vía.
    pub fn reroute(
        &mut self,
        max_move: u32,
        avoid: TileCoord,
        avoid2: Option<TileCoord>,
        mut pick_next: impl FnMut(Option<TileCoord>) -> Option<TileCoord>,
    ) -> u32 {
        self.migrate_legacy();
        let key = StationHopKey(Some(avoid));
        let Some(mut queue) = self.by_next_hop.remove(&key) else {
            return 0;
        };
        let mut moved = 0_u32;
        let mut kept = VecDeque::new();
        while let Some(mut p) = queue.pop_front() {
            if moved >= max_move {
                kept.push_back(p);
                continue;
            }
            let available = u32::from(p.count);
            let take = available.min(max_move - moved);
            if take == 0 {
                kept.push_back(p);
                continue;
            }
            let new_hop = pick_next(p.first_station)
                .filter(|h| *h != avoid && avoid2.is_none_or(|a2| *h != a2));
            if take < available {
                #[allow(clippy::cast_possible_truncation)]
                if let Some(mut taken) = p.split(take as u16) {
                    taken.next_hop = new_hop;
                    moved += take;
                    self.push(taken);
                    kept.push_back(p);
                } else {
                    kept.push_back(p);
                }
            } else {
                p.next_hop = new_hop;
                moved += available;
                self.push(p);
            }
        }
        if !kept.is_empty() {
            self.by_next_hop.insert(key, kept);
        }
        moved
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
            && last.same_merge_key(&packet)
        {
            last.count = last.count.saturating_add(packet.count);
            last.feeder_share = last.feeder_share.saturating_add(packet.feeder_share);
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
                if let Some(taken) = p.split(take) {
                    left = 0;
                    out.push(taken);
                    kept.push(p);
                } else {
                    kept.push(p);
                }
            }
        }
        self.packets = kept;
        out
    }

    /// `VehicleCargoList::Reroute` — reescribe `next_hop` de packets en TRANSFER.
    pub fn reroute(
        &mut self,
        max_move: u32,
        avoid: TileCoord,
        avoid2: Option<TileCoord>,
        mut pick_next: impl FnMut(Option<TileCoord>) -> Option<TileCoord>,
    ) -> u32 {
        let mut moved = 0_u32;
        for p in &mut self.packets {
            if moved >= max_move {
                break;
            }
            let hop = p.next_hop;
            if hop != Some(avoid) && avoid2.is_none_or(|a2| hop != Some(a2)) {
                continue;
            }
            let new_hop = pick_next(p.first_station)
                .filter(|h| *h != avoid && avoid2.is_none_or(|a2| *h != a2));
            p.next_hop = new_hop;
            moved = moved.saturating_add(u32::from(p.count));
        }
        moved
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
