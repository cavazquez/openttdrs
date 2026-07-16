use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::cargo_packet::StationCargoList;
use crate::company::CompanyId;
use crate::map::TileCoord;
use crate::vehicle::VehicleKind;

/// Días desde la última recogida por tipo de carga (0 = reciente).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoTimeSincePickup {
    pub passengers: u8,
    pub coal: u8,
    pub mail: u8,
    pub oil: u8,
    pub livestock: u8,
    pub goods: u8,
    pub grain: u8,
    pub wood: u8,
    pub iron_ore: u8,
    pub steel: u8,
    pub valuables: u8,
}

impl CargoTimeSincePickup {
    #[must_use]
    pub const fn get(self, cargo: CargoType) -> u8 {
        match cargo {
            CargoType::Passengers => self.passengers,
            CargoType::Coal => self.coal,
            CargoType::Mail => self.mail,
            CargoType::Oil => self.oil,
            CargoType::Livestock => self.livestock,
            CargoType::Goods => self.goods,
            CargoType::Grain => self.grain,
            CargoType::Wood => self.wood,
            CargoType::IronOre => self.iron_ore,
            CargoType::Steel => self.steel,
            CargoType::Valuables => self.valuables,
        }
    }

    pub fn set(&mut self, cargo: CargoType, days: u8) {
        *self.slot_mut(cargo) = days;
    }

    pub fn increment_waiting(&mut self, cargo: CargoType) {
        let slot = self.slot_mut(cargo);
        *slot = slot.saturating_add(1);
    }

    fn slot_mut(&mut self, cargo: CargoType) -> &mut u8 {
        match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Coal => &mut self.coal,
            CargoType::Mail => &mut self.mail,
            CargoType::Oil => &mut self.oil,
            CargoType::Livestock => &mut self.livestock,
            CargoType::Goods => &mut self.goods,
            CargoType::Grain => &mut self.grain,
            CargoType::Wood => &mut self.wood,
            CargoType::IronOre => &mut self.iron_ore,
            CargoType::Steel => &mut self.steel,
            CargoType::Valuables => &mut self.valuables,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    #[serde(default)]
    pub stop_kind: StopKind,
    /// Compañía propietaria (Fase 4; default jugador).
    #[serde(default)]
    pub owner: CompanyId,
    /// Nombre de la estación (saves de `OpenTTD` con nombre custom).
    #[serde(default)]
    pub name: Option<String>,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    #[serde(default)]
    pub cargo_stock: CargoStock,
    /// Cola de packets en espera (`StationCargoList`); fuente de verdad Fase 2.
    #[serde(default)]
    pub cargo_packets: StationCargoList,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
    /// Días sin recogida por tipo de carga en espera.
    #[serde(default)]
    pub time_since_pickup: CargoTimeSincePickup,
    /// Rating global simplificado (0–255; mayor = mejor servicio).
    #[serde(default = "default_station_rating")]
    pub rating: u8,
    /// Días sin recogida por compañía (rating competitivo; default vacío).
    #[serde(default)]
    pub company_time_since_pickup: Vec<(CompanyId, CargoTimeSincePickup)>,
    /// Teselas del aeropuerto (helipuerto = `[pos]`; small = footprint completo).
    #[serde(default)]
    pub airport_tiles: Vec<TileCoord>,
    /// Teselas adicionales unidas con `JoinStation` (paradas road 1×1).
    #[serde(default)]
    pub joined_tiles: Vec<TileCoord>,
    /// Spec NewGRF/vanilla usado al construir (`StationSpecId`; 0 = default).
    #[serde(default)]
    pub station_spec: crate::station_class::StationSpecId,
    /// Bits aleatorios `NewGRF` de la estación (var `5F` / random Action2).
    #[serde(default)]
    pub newgrf_random_bits: u8,
}

const fn default_station_rating() -> u8 {
    255
}

fn seed_station_newgrf_random_bits(pos: TileCoord) -> u8 {
    let x = pos.x.cast_unsigned();
    let y = pos.y.cast_unsigned();
    ((x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) >> 24) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum StopKind {
    #[default]
    TruckStop,
    BusStop,
    RailStation,
    /// Muelle (`StationType::Dock`); carga de mercancía para barcos.
    Dock,
    /// Helipuerto / aeropuerto 1×1 (`StationType::Airport`).
    Airport,
    /// Boya (`StationType::Buoy`); waypoint acuático sin carga.
    Buoy,
    /// Punto de paso ferroviario (`StationType::RailWaypoint`); sin carga ni parada.
    RailWaypoint,
    /// Punto de paso road (`StationType::RoadWaypoint`); sin carga ni parada.
    RoadWaypoint,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self::new_with_kind(pos, StopKind::TruckStop)
    }

    #[must_use]
    pub fn new_with_kind(pos: TileCoord, stop_kind: StopKind) -> Self {
        Self {
            pos,
            stop_kind,
            owner: CompanyId::PLAYER,
            name: None,
            stock: 0,
            cargo_stock: CargoStock::default(),
            cargo_packets: StationCargoList::default(),
            income: 0,
            time_since_pickup: CargoTimeSincePickup::default(),
            rating: default_station_rating(),
            company_time_since_pickup: vec![(CompanyId::PLAYER, CargoTimeSincePickup::default())],
            airport_tiles: Vec::new(),
            joined_tiles: Vec::new(),
            station_spec: crate::station_class::StationSpecId::DefaultRail,
            newgrf_random_bits: seed_station_newgrf_random_bits(pos),
        }
    }

    pub(super) fn company_pickup_slot_mut(
        &mut self,
        company: CompanyId,
    ) -> &mut CargoTimeSincePickup {
        if let Some(idx) = self
            .company_time_since_pickup
            .iter()
            .position(|(id, _)| *id == company)
        {
            return &mut self.company_time_since_pickup[idx].1;
        }
        self.company_time_since_pickup
            .push((company, CargoTimeSincePickup::default()));
        let idx = self.company_time_since_pickup.len() - 1;
        &mut self.company_time_since_pickup[idx].1
    }

    #[must_use]
    pub fn company_pickup_days(&self, company: CompanyId, cargo: CargoType) -> u8 {
        self.company_time_since_pickup
            .iter()
            .find(|(id, _)| *id == company)
            .map_or_else(|| self.time_since_pickup.get(cargo), |(_, t)| t.get(cargo))
    }

    /// Si hay balance legado sin packets, hidrata la cola (tests / saves v12).
    pub fn ensure_packets_from_stock(&mut self) {
        if self.cargo_packets.is_empty() {
            let stock = self.cargo_stock;
            if stock != CargoStock::default() {
                self.cargo_packets = StationCargoList::from_stock(stock, self.pos);
            }
        }
        self.sync_stock_from_packets();
    }

    /// Sincroniza `cargo_stock` / `stock` desde la cola de packets.
    pub fn sync_stock_from_packets(&mut self) {
        self.cargo_stock = self.cargo_packets.as_stock();
        self.stock = ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|c| c.is_freight())
            .map(|c| self.cargo_stock.get(c))
            .fold(0_u32, u32::saturating_add);
    }

    /// Añade carga en espera (producción pueblo / descarga freight).
    pub fn add_waiting_cargo(&mut self, cargo: CargoType, amount: u32) {
        if amount == 0 {
            return;
        }
        let was_empty = self.cargo_stock.get(cargo) == 0;
        self.ensure_packets_from_stock();
        self.cargo_packets.add_amount(cargo, amount, self.pos);
        if was_empty {
            // Tras truncate a 255, nueva carga empieza el ciclo de antigüedad.
            self.time_since_pickup.set(cargo, 0);
        }
        self.sync_stock_from_packets();
    }

    /// Reinserta packets en espera preservando `first_station` / `feeder_paid`.
    pub fn push_waiting_packets(
        &mut self,
        packets: impl IntoIterator<Item = crate::cargo_packet::CargoPacket>,
    ) {
        self.ensure_packets_from_stock();
        for p in packets {
            if p.count == 0 {
                continue;
            }
            let cargo = p.cargo;
            let was_empty = self.cargo_stock.get(cargo) == 0;
            self.cargo_packets.push(p);
            if was_empty {
                self.time_since_pickup.set(cargo, 0);
            }
        }
        self.sync_stock_from_packets();
    }

    /// Extrae packets en espera (carga a vehículo / consumo industria).
    pub fn take_waiting_cargo(
        &mut self,
        cargo: CargoType,
        amount: u32,
    ) -> Vec<crate::cargo_packet::CargoPacket> {
        self.ensure_packets_from_stock();
        let taken = self.cargo_packets.take(cargo, amount);
        self.sync_stock_from_packets();
        taken
    }

    /// ¿La estación cubre esta tesela (ancla, aeropuerto o unidas)?
    #[must_use]
    pub fn covers_tile(&self, c: TileCoord) -> bool {
        self.pos == c || self.airport_tiles.contains(&c) || self.joined_tiles.contains(&c)
    }

    #[must_use]
    pub fn can_service_vehicle(&self, vehicle_kind: VehicleKind) -> bool {
        matches!(
            (vehicle_kind, self.stop_kind),
            (
                VehicleKind::Train,
                StopKind::RailStation | StopKind::RailWaypoint
            ) | (VehicleKind::Bus | VehicleKind::Tram, StopKind::BusStop)
                | (VehicleKind::Truck, StopKind::TruckStop)
                | (
                    VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram,
                    StopKind::RoadWaypoint,
                )
                | (VehicleKind::Ship, StopKind::Dock | StopKind::Buoy)
                | (VehicleKind::Aircraft, StopKind::Airport)
        )
    }

    #[must_use]
    pub fn is_waypoint(&self) -> bool {
        matches!(
            self.stop_kind,
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint
        )
    }

    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        if matches!(
            self.stop_kind,
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint
        ) {
            return false;
        }
        match self.stop_kind {
            StopKind::BusStop => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::TruckStop | StopKind::RailStation => {
                !matches!(cargo, CargoType::Passengers | CargoType::Mail)
            }
            // Muelle: mercancía + pasajeros (ferry).
            StopKind::Dock => true,
            StopKind::Airport => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint => false,
        }
    }
}
