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
    #[serde(default, with = "ordered_map_serde")]
    pub by_next_hop: BTreeMap<StationHopKey, VecDeque<CargoPacket>>,
    /// Cantidad reservada para carga (`reserved_count`).
    #[serde(default)]
    pub reserved: u32,
    /// Reservas nuevas separadas por cargo.
    ///
    /// `reserved` se conserva como total para compatibilidad con JSON/SAV
    /// antiguos que no podían expresar el cargo de la reserva. Cuando existe
    /// esta tabla, sus entradas representan la parte conocida del total y el
    /// remanente legacy se trata de forma conservadora para cualquier cargo.
    #[serde(default, with = "ordered_map_serde")]
    pub reserved_by_cargo: BTreeMap<CargoType, u32>,
    /// Parte de `reserved_by_cargo` que ya fue movida físicamente a un vehículo
    /// como `MTA_LOAD`. Se mantiene separada porque el contador legacy también
    /// representa reservas virtuales cuyos packets siguen en esta cola.
    #[serde(default, with = "ordered_map_serde")]
    pub reserved_physically_by_cargo: BTreeMap<CargoType, u32>,
    /// Campo legacy `packets` (saves / JSON antiguos); se migra a [`Self::by_next_hop`].
    #[serde(default, alias = "packets")]
    legacy_packets: VecDeque<CargoPacket>,
}

/// JSON no admite claves estructuradas de mapas. Las colas usan tanto
/// `StationHopKey` como `CargoType` (que puede ser un cargo `NewGRF`), así que
/// se guardan como secuencias de entradas ordenadas.
mod ordered_map_serde {
    use std::collections::BTreeMap;

    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OrderedMapWire<K, V> {
        Entries(Vec<(K, V)>),
        /// Compatibilidad con los mapas vacíos que el formato JSON anterior
        /// sí podía expresar como `{}`.
        LegacyEmpty(BTreeMap<String, V>),
    }

    pub fn serialize<S, K, V>(map: &BTreeMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize + Ord,
        V: Serialize,
    {
        map.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    {
        match OrderedMapWire::deserialize(deserializer)? {
            OrderedMapWire::Entries(entries) => Ok(entries.into_iter().collect()),
            OrderedMapWire::LegacyEmpty(entries) if entries.is_empty() => Ok(BTreeMap::new()),
            OrderedMapWire::LegacyEmpty(_) => Err(D::Error::custom(
                "mapa de cola de carga legado con claves no recuperable",
            )),
        }
    }
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
        let typed_physical = self
            .reserved_physically_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
            .min(self.reserved_by_cargo.get(&cargo).copied().unwrap_or(0));
        total.saturating_sub(self.reserved_for(cargo).saturating_sub(typed_physical))
    }

    fn physically_reserved_for(&self, cargo: CargoType) -> u32 {
        self.reserved_physically_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
    }

    fn physically_reserved_total(&self) -> u32 {
        self.reserved_physically_by_cargo
            .values()
            .copied()
            .fold(0, u32::saturating_add)
            .min(self.reserved)
    }

    fn available_count(&self) -> u32 {
        self.total_count().saturating_sub(
            self.reserved
                .saturating_sub(self.physically_reserved_total()),
        )
    }

    #[must_use]
    pub fn as_stock(&self) -> CargoStock {
        let mut stock = CargoStock::default();
        for p in self.packets().chain(self.legacy_packets.iter()) {
            stock.add(p.cargo, u32::from(p.count));
        }
        stock
    }

    /// Stock visible para una carga cuyo siguiente salto puede ser cualquiera
    /// de `next_stations` (más los packets sin destino explícito).
    #[must_use]
    pub fn stock_for_next_stations(&self, next_stations: &[TileCoord]) -> CargoStock {
        let mut stock = CargoStock::default();
        for packet in self.packets().chain(self.legacy_packets.iter()) {
            if packet
                .next_hop
                .is_none_or(|hop| next_stations.contains(&hop))
            {
                stock.add(packet.cargo, u32::from(packet.count));
            }
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
        self.take_matching(cargo, amount, |_| true)
    }

    /// Extrae hasta `amount` unidades sólo de los siguientes destinos válidos.
    ///
    /// Los packets con `next_hop == None` representan `StationID::Invalid` y
    /// pueden cargarse para cualquiera de las estaciones siguientes.
    pub fn take_for(
        &mut self,
        cargo: CargoType,
        amount: u32,
        next_stations: &[TileCoord],
    ) -> Vec<CargoPacket> {
        self.take_matching(cargo, amount, |hop| {
            hop.is_none_or(|station| next_stations.contains(&station))
        })
    }

    fn take_matching(
        &mut self,
        cargo: CargoType,
        amount: u32,
        mut matches_hop: impl FnMut(Option<TileCoord>) -> bool,
    ) -> Vec<CargoPacket> {
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
            if !matches_hop(key.0) {
                continue;
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

    /// Cantidad reservada que debe descontarse de un cargo concreto.
    ///
    /// El total legacy no asociado a un cargo se aplica a cada consulta: no
    /// sabemos qué tipo reservó un save antiguo y bloquear de más es más
    /// seguro que permitir que dos vehículos consuman la misma reserva.
    #[must_use]
    pub fn reserved_for(&self, cargo: CargoType) -> u32 {
        let known_total = self
            .reserved_by_cargo
            .values()
            .copied()
            .fold(0, u32::saturating_add)
            .min(self.reserved);
        let legacy_unassigned = self.reserved.saturating_sub(known_total);
        self.reserved_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
            .saturating_add(legacy_unassigned)
    }

    /// Reserva hasta `amount` unidades sin asociarlas a un cargo concreto.
    ///
    /// Se mantiene para callers legacy y pruebas de compatibilidad. Las
    /// nuevas rutas de carga deben usar [`Self::reserve_for`].
    pub fn reserve(&mut self, amount: u32) -> u32 {
        let available = self.available_count();
        let take = amount.min(available);
        self.reserved = self.reserved.saturating_add(take);
        take
    }

    /// Reserva hasta `amount` unidades del cargo indicado.
    pub fn reserve_for(&mut self, cargo: CargoType, amount: u32) -> u32 {
        let available = self.available_of(cargo);
        let take = amount.min(available);
        if take == 0 {
            return 0;
        }
        self.reserved = self.reserved.saturating_add(take);
        let entry = self.reserved_by_cargo.entry(cargo).or_default();
        *entry = entry.saturating_add(take);
        take
    }

    /// Consume la reserva nueva de un cargo, dejando intacta la parte legacy
    /// que no está asociada a ningún tipo.
    pub fn consume_reserved_for(&mut self, cargo: CargoType, amount: u32) {
        let known_total = self
            .reserved_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
            .min(self.reserved);
        let physical = self.physically_reserved_for(cargo).min(known_total);
        let known_virtual = known_total.saturating_sub(physical);
        let consumed_virtual = known_virtual.min(amount);
        if consumed_virtual > 0 {
            let remaining = self
                .reserved_by_cargo
                .get(&cargo)
                .copied()
                .unwrap_or(0)
                .saturating_sub(consumed_virtual);
            if remaining == 0 {
                self.reserved_by_cargo.remove(&cargo);
            } else if let Some(entry) = self.reserved_by_cargo.get_mut(&cargo) {
                *entry = remaining;
            }
            self.reserved = self.reserved.saturating_sub(consumed_virtual);
        }
        let remaining = amount.saturating_sub(consumed_virtual);
        let legacy = self.legacy_reserved_count().min(remaining);
        self.reserved = self.reserved.saturating_sub(legacy);
    }

    /// Consume reserva legacy al cargar.
    pub fn consume_reserved(&mut self, amount: u32) {
        let mut remaining = amount.min(self.reserved);
        let cargo_keys: Vec<_> = self.reserved_by_cargo.keys().copied().collect();
        for cargo in cargo_keys {
            if remaining == 0 {
                break;
            }
            let known_total = self.reserved_by_cargo.get(&cargo).copied().unwrap_or(0);
            let physical = self.physically_reserved_for(cargo).min(known_total);
            let known_virtual = known_total.saturating_sub(physical);
            let consumed = known_virtual.min(remaining);
            if consumed > 0 {
                self.consume_reserved_for(cargo, consumed);
                remaining = remaining.saturating_sub(consumed);
            }
        }
        let legacy = self.legacy_reserved_count().min(remaining);
        self.reserved = self.reserved.saturating_sub(legacy);
    }

    fn legacy_reserved_count(&self) -> u32 {
        let known_total = self
            .reserved_by_cargo
            .values()
            .copied()
            .fold(0, u32::saturating_add)
            .min(self.reserved);
        self.reserved.saturating_sub(known_total)
    }

    fn reserve_physically_for(&mut self, cargo: CargoType, amount: u32) {
        if amount == 0 {
            return;
        }
        self.reserved = self.reserved.saturating_add(amount);
        let known = self.reserved_by_cargo.entry(cargo).or_default();
        *known = known.saturating_add(amount);
        let physical = self.reserved_physically_by_cargo.entry(cargo).or_default();
        *physical = physical.saturating_add(amount);
    }

    fn consume_physical_reserved_for(&mut self, cargo: CargoType, amount: u32) -> u32 {
        let moved = amount.min(self.physically_reserved_for(cargo));
        if moved == 0 {
            return 0;
        }
        let remaining_physical = self
            .reserved_physically_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
            .saturating_sub(moved);
        if remaining_physical == 0 {
            self.reserved_physically_by_cargo.remove(&cargo);
        } else if let Some(entry) = self.reserved_physically_by_cargo.get_mut(&cargo) {
            *entry = remaining_physical;
        }

        let remaining_known = self
            .reserved_by_cargo
            .get(&cargo)
            .copied()
            .unwrap_or(0)
            .saturating_sub(moved);
        if remaining_known == 0 {
            self.reserved_by_cargo.remove(&cargo);
        } else if let Some(entry) = self.reserved_by_cargo.get_mut(&cargo) {
            *entry = remaining_known;
        }
        self.reserved = self.reserved.saturating_sub(moved);
        moved
    }

    /// Reserva packets físicamente para una visita de vehículo (`Reserve`).
    ///
    /// A diferencia de [`Self::reserve_for`], los packets dejan la cola de la
    /// estación y pasan al extremo `MTA_LOAD` del vehículo. El origen se
    /// guarda en el vehículo para que una cancelación pueda devolverlos a la
    /// estación correcta. Las reservas virtuales legacy del mismo cargo no se
    /// mezclan: no tienen identidad suficiente para devolverse sin riesgo.
    pub fn reserve_for_vehicle(
        &mut self,
        station: TileCoord,
        cargo: CargoType,
        amount: u32,
        next_stations: &[TileCoord],
        current_tile: TileCoord,
        destination: &mut VehicleCargoList,
    ) -> u32 {
        if amount == 0 || !destination.can_accept_reserved_from_station(station, cargo) {
            return 0;
        }
        let physical = self.physically_reserved_for(cargo);
        if self.reserved_for(cargo) > physical {
            return 0;
        }
        let take = amount.min(self.available_of(cargo));
        if take == 0 {
            return 0;
        }
        let mut packets = self.take_for(cargo, take, next_stations);
        if packets.is_empty() {
            return 0;
        }
        for packet in &mut packets {
            if packet.first_station.is_none() {
                packet.first_station = Some(station);
            }
            packet.update_loading_tile(current_tile);
        }
        let moved = packets
            .iter()
            .map(|packet| u32::from(packet.count))
            .fold(0, u32::saturating_add);
        self.reserve_physically_for(cargo, moved);
        destination.append_reserved_packets_from_station(station, cargo, packets);
        moved
    }

    /// Promueve la reserva física de una visita a carga efectiva (`Load`).
    pub fn load_reserved_from_vehicle(
        &mut self,
        station: TileCoord,
        destination: &mut VehicleCargoList,
        amount: u32,
    ) -> u32 {
        let Some(cargo) = destination.reservation_cargo else {
            return 0;
        };
        if destination.reservation_station != Some(station) {
            return 0;
        }
        let move_limit = amount
            .min(destination.reserved_count())
            .min(self.physically_reserved_for(cargo));
        let moved = destination.load_reserved(move_limit);
        self.consume_physical_reserved_for(cargo, moved)
    }

    /// Devuelve a esta estación la parte de `MTA_LOAD` que no llegó a
    /// cargarse. El extremo reservado se recorre hacia atrás, pero la salida
    /// se entrega en FIFO para que la cola no cambie de orden.
    pub fn return_reserved_from_vehicle(
        &mut self,
        station: TileCoord,
        source: &mut VehicleCargoList,
        amount: u32,
        next_hop: Option<TileCoord>,
        current_tile: TileCoord,
    ) -> u32 {
        let Some(cargo) = source.reservation_cargo else {
            return 0;
        };
        if source.reservation_station != Some(station) {
            return 0;
        }
        let target = amount
            .min(source.reserved_count())
            .min(self.physically_reserved_for(cargo));
        if target == 0 {
            return 0;
        }
        let packets = source.take_reserved_packets(target);
        let moved = packets
            .iter()
            .map(|packet| u32::from(packet.count))
            .fold(0, u32::saturating_add);
        for mut packet in packets {
            packet.update_unloading_tile(current_tile);
            packet.next_hop = next_hop;
            self.push(packet);
        }
        self.consume_physical_reserved_for(cargo, moved)
    }

    /// Migra un balance agregado a packets sintéticos.
    #[must_use]
    pub fn from_stock(stock: CargoStock, source: TileCoord) -> Self {
        let mut list = Self::default();
        for cargo in ALL_CARGO_TYPES {
            list.add_amount(cargo, stock.get(cargo), source);
        }
        for (cargo, amount) in stock.custom_entries() {
            list.add_amount(cargo, amount, source);
        }
        list
    }

    #[must_use]
    pub fn pick_freight_to_load(&self, preferred: Option<CargoType>) -> Option<CargoType> {
        self.as_stock().pick_freight_to_load(preferred)
    }

    /// Indica si hay carga de un tipo para alguno de los próximos destinos.
    ///
    /// Es el equivalente de `StationCargoList::HasCargoFor`: un paquete sin
    /// `next_hop` representa `StationID::Invalid()` y puede viajar a cualquier
    /// estación; los demás sólo cuentan si coinciden con una de las ramas
    /// posibles de la orden siguiente.
    #[must_use]
    pub fn has_cargo_for(&self, cargo: CargoType, next_stations: &[TileCoord]) -> bool {
        self.packets()
            .chain(self.legacy_packets.iter())
            .any(|packet| {
                packet.cargo == cargo
                    && packet
                        .next_hop
                        .is_none_or(|hop| next_stations.contains(&hop))
            })
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
    /// Conteos nativos `VehicleCargoList::action_counts`:
    /// transferir, entregar, conservar y cargar.
    #[serde(default)]
    pub action_counts: [u32; 4],
    /// Estación propietaria de la sección `MTA_LOAD` pendiente.
    #[serde(default)]
    pub reservation_station: Option<TileCoord>,
    /// Cargo de la sección `MTA_LOAD` pendiente.
    #[serde(default)]
    pub reservation_cargo: Option<CargoType>,
    /// Conteos por acción tras `Stage` (P2.19).
    #[serde(skip)]
    pub staged_transfer: u32,
    #[serde(skip)]
    pub staged_deliver: u32,
    #[serde(skip)]
    pub staged_keep: u32,
}

const VEHICLE_ACTION_KEEP: usize = 2;
const VEHICLE_ACTION_LOAD: usize = 3;

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

    /// Cantidad almacenada que ya no está pendiente de una reserva de carga.
    #[must_use]
    pub fn stored_count(&self) -> u32 {
        let total = self.total();
        total.saturating_sub(self.action_counts[VEHICLE_ACTION_LOAD].min(total))
    }

    /// Cantidad de packets asociados a una reserva que aún debe cargarse.
    #[must_use]
    pub fn reserved_count(&self) -> u32 {
        self.action_counts[VEHICLE_ACTION_LOAD].min(self.total())
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
        self.action_counts = [0; 4];
        self.reservation_station = None;
        self.reservation_cargo = None;
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

    /// Añade packets ya seleccionados por la estación como `MTA_LOAD`.
    ///
    /// Se preserva el límite de la sección reservada (sin fusionar con el
    /// último packet) porque `OpenTTD` mantiene los packets `MTA_LOAD` en el
    /// extremo que `CargoReturn` recorre al cancelar una visita.
    pub fn append_reserved_packets(
        &mut self,
        packets: impl IntoIterator<Item = CargoPacket>,
    ) -> u32 {
        let mut added = 0_u32;
        for packet in packets {
            if packet.count == 0 {
                continue;
            }
            added = added.saturating_add(u32::from(packet.count));
            self.packets.push(packet);
        }
        self.action_counts[VEHICLE_ACTION_LOAD] =
            self.action_counts[VEHICLE_ACTION_LOAD].saturating_add(added);
        added
    }

    fn can_accept_reserved_from_station(&self, station: TileCoord, cargo: CargoType) -> bool {
        self.reserved_count() == 0
            || (self.reservation_station == Some(station) && self.reservation_cargo == Some(cargo))
    }

    fn append_reserved_packets_from_station(
        &mut self,
        station: TileCoord,
        cargo: CargoType,
        packets: Vec<CargoPacket>,
    ) -> u32 {
        debug_assert!(self.can_accept_reserved_from_station(station, cargo));
        debug_assert!(packets.iter().all(|packet| packet.cargo == cargo));
        let added = self.append_reserved_packets(packets);
        if added > 0 {
            self.reservation_station = Some(station);
            self.reservation_cargo = Some(cargo);
        }
        added
    }

    fn clear_reservation_source_if_settled(&mut self) {
        if self.reserved_count() == 0 {
            self.reservation_station = None;
            self.reservation_cargo = None;
        }
    }

    /// Promueve una parte de la reserva a carga efectiva (`MTA_KEEP`).
    pub fn load_reserved(&mut self, amount: u32) -> u32 {
        let moved = amount.min(self.reserved_count());
        self.action_counts[VEHICLE_ACTION_LOAD] =
            self.action_counts[VEHICLE_ACTION_LOAD].saturating_sub(moved);
        self.action_counts[VEHICLE_ACTION_KEEP] =
            self.action_counts[VEHICLE_ACTION_KEEP].saturating_add(moved);
        self.clear_reservation_source_if_settled();
        moved
    }

    /// Extrae desde el extremo los packets aún reservados para devolverlos a
    /// la estación. El orden de salida se invierte para conservar FIFO.
    pub fn take_reserved_packets(&mut self, amount: u32) -> Vec<CargoPacket> {
        let target = amount.min(self.reserved_count());
        let mut left = target;
        let mut out = Vec::new();
        while left > 0 {
            let Some(mut packet) = self.packets.pop() else {
                break;
            };
            let available = u32::from(packet.count);
            if available <= left {
                left = left.saturating_sub(available);
                out.push(packet);
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let take = left as u16;
            let Some(taken) = packet.split(take) else {
                self.packets.push(packet);
                break;
            };
            self.packets.push(packet);
            out.push(taken);
            left = 0;
        }
        let moved = target.saturating_sub(left);
        self.action_counts[VEHICLE_ACTION_LOAD] =
            self.action_counts[VEHICLE_ACTION_LOAD].saturating_sub(moved);
        self.clear_reservation_source_if_settled();
        out.reverse();
        out
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
        unload_type: crate::vehicle::OrderUnloadType,
    ) -> bool {
        self.staged_transfer = 0;
        self.staged_deliver = 0;
        self.staged_keep = 0;
        self.reservation_station = None;
        self.reservation_cargo = None;
        self.action_counts = [0; 4];
        if self.packets.is_empty() {
            return false;
        }
        let mut transfer = Vec::new();
        let mut deliver = Vec::new();
        let mut keep = Vec::new();
        for mut cp in self.packets.drain(..) {
            let action = stage_packet(&cp, accepted, current_station, next_stations, unload_type);
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
        self.action_counts = [
            self.staged_transfer,
            self.staged_deliver,
            self.staged_keep,
            0,
        ];
        self.staged_transfer > 0 || self.staged_deliver > 0
    }
}

fn stage_packet(
    packet: &CargoPacket,
    accepted: bool,
    current_station: TileCoord,
    next_stations: &[TileCoord],
    unload_type: crate::vehicle::OrderUnloadType,
) -> CargoUnloadAction {
    // Misma regla que `choose_cargo_action` (evita divergencia Stage/pago).
    super::operations::choose_cargo_action(
        packet,
        current_station,
        next_stations,
        unload_type,
        accepted,
    )
}
