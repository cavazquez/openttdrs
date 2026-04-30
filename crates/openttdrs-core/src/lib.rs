//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod command;
pub mod industry;
pub mod map;
pub mod ottdmap_extras;
pub mod pathfinder;
pub mod save;
pub mod station;
pub mod tick;
pub mod tnbp_decode;
pub mod vehicle;

pub use command::{Command, CommandError, apply_command};
pub use industry::{INDUSTRY_PRODUCE_TICKS, Industry, IndustryKind, industry_produce_period_ticks};
pub use map::{
    Map, MapError, OTTD_TILETYPE_TUNNELBRIDGE, Tile, TileCoord, TileKind,
    openttd_tile_index_to_coord,
};
pub use ottdmap_extras::{OttdmapExtras, dense_payload_end};
pub use pathfinder::find_path;
pub use save::SaveError;
pub use save::load_from_str;
pub use station::{STATION_COVERAGE_RADIUS, Station, StationCoverage, station_coverage_at};
pub use tick::GameTick;
pub use tnbp_decode::{
    JgrTunnelRecord, SlPrimitive, SlTableField, TnbpDecodeError, TnbpDecoded, decode_tnbp_blob,
    jgr_tunnels_from_decoded, read_sl_gamma, split_sl_gamma_segments, tnbp_blob_to_json_value,
};
pub use vehicle::{Vehicle, VehicleKind};

/// Contadores acumulativos de la simulación (carga/descarga, producción).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimStats {
    /// Eventos de carga (vehículo tomó cargo en una industria).
    pub cargo_pickups: u64,
    /// Eventos de descarga (vehículo entregó en una estación).
    pub cargo_deliveries: u64,
    /// Unidades de cargo cargadas (suma de `load`).
    pub cargo_units_loaded: u64,
    /// Unidades de cargo entregadas en estación.
    pub cargo_units_delivered: u64,
    /// Unidades añadidas al stock de industrias por `Industry::produce`.
    pub industry_cargo_units_produced: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompanyEconomy {
    pub money: i64,
    pub loan: i64,
}

impl Default for CompanyEconomy {
    fn default() -> Self {
        Self {
            money: 100_000,
            loan: 0,
        }
    }
}

pub const ROAD_BUILD_COST: i64 = 10;
pub const RAIL_BUILD_COST: i64 = 25;
pub const STATION_BUILD_COST: i64 = 200;
pub const DEPOT_BUILD_COST: i64 = 150;
pub const TUNNEL_BUILD_COST_PER_TILE: i64 = 90;
pub const BRIDGE_BUILD_COST_PER_TILE: i64 = 70;
pub const CLEAR_TILE_COST: i64 = 5;
pub const CARGO_DELIVERY_PAYMENT: i64 = 12;

/// Estado global mínimo del mundo simulado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub map: Map,
    pub tick: GameTick,
    pub industries: Vec<Industry>,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<Station>,
    pub stats: SimStats,
    #[serde(default)]
    pub economy: CompanyEconomy,
    /// Túneles JGR decodificados desde footer `TNBP` del `.ottdmap` (vacío si no hay o no aplica).
    #[serde(default)]
    pub jgr_tunnels_from_footer: Vec<JgrTunnelRecord>,
}

impl GameState {
    #[must_use]
    pub fn new(map_width: u32, map_height: u32) -> Self {
        Self {
            map: Map::new_flat(map_width, map_height, 1),
            tick: GameTick::default(),
            industries: Vec::new(),
            vehicles: Vec::new(),
            stations: Vec::new(),
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            jgr_tunnels_from_footer: Vec::new(),
        }
    }

    /// Crea un estado a partir de un mapa ya construido (sin industrias ni vehículos).
    #[must_use]
    pub fn from_map(map: Map) -> Self {
        Self {
            map,
            tick: GameTick::default(),
            industries: Vec::new(),
            vehicles: Vec::new(),
            stations: Vec::new(),
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            jgr_tunnels_from_footer: Vec::new(),
        }
    }

    /// Avanza un tick de simulación (equivalente conceptual a un frame lógico del juego).
    ///
    /// Orden dentro del tick:
    /// 1. Producción de industrias.
    /// 2. Carga/descarga según posición actual del vehículo.
    /// 3. Movimiento del vehículo (vehicle.step).
    pub fn step(&mut self) {
        self.tick.advance();
        let t = self.tick.get();

        for industry in &mut self.industries {
            let before = industry.stock;
            industry.produce(t);
            self.stats.industry_cargo_units_produced +=
                u64::from(industry.stock.saturating_sub(before));
        }

        // Carga: vehículo en posición de industria sin cargo → toma lo disponible.
        for i in 0..self.vehicles.len() {
            let vpos = self.vehicles[i].pos;
            let vcap = self.vehicles[i].capacity;
            if self.vehicles[i].cargo == 0
                && let Some(ind) = self.industries.iter_mut().find(|ind| ind.pos == vpos)
            {
                let load = ind.stock.min(vcap);
                self.vehicles[i].cargo = load;
                ind.stock -= load;
                if load > 0 {
                    self.stats.cargo_pickups += 1;
                    self.stats.cargo_units_loaded += u64::from(load);
                }
            }
        }

        // Descarga: vehículo en posición de estación con cargo → entrega.
        for i in 0..self.vehicles.len() {
            let vpos = self.vehicles[i].pos;
            let vcargo = self.vehicles[i].cargo;
            if vcargo > 0
                && let Some(st) = self.stations.iter_mut().find(|st| st.pos == vpos)
            {
                st.stock += vcargo;
                st.income += u64::from(vcargo);
                self.economy.money += i64::from(vcargo) * CARGO_DELIVERY_PAYMENT;
                self.stats.cargo_deliveries += 1;
                self.stats.cargo_units_delivered += u64::from(vcargo);
                self.vehicles[i].cargo = 0;
            }
        }

        // Recomputa el path BFS para vehículos que lo necesiten (path vacío y no en destino).
        for i in 0..self.vehicles.len() {
            if self.vehicles[i].path.is_empty()
                && self.vehicles[i].pos != self.vehicles[i].dest
                && let Some(path) =
                    pathfinder::find_path(&self.map, self.vehicles[i].pos, self.vehicles[i].dest)
            {
                self.vehicles[i].path = path.into_iter().collect();
            }
        }

        // Movimiento: sigue el path BFS o Manhattan fallback.
        for vehicle in &mut self.vehicles {
            vehicle.step();
        }
    }

    /// Serializa el estado a JSON (UTF-8) para guardado o depuración.
    ///
    /// # Errors
    ///
    /// Falla si algún campo no es serializable (no debería ocurrir en tipos propios).
    pub fn save_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Restaura un estado desde JSON producido por [`Self::save_json`].
    ///
    /// # Errors
    ///
    /// Devuelve error si el texto no es JSON válido o no coincide el esquema.
    pub fn load_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use industry::{INDUSTRY_PRODUCE_AMOUNT, industry_produce_period_ticks};
    use vehicle::VEHICLE_CAPACITY;

    use super::*;

    #[test]
    fn new_map_has_expected_dimensions() {
        let s = GameState::new(8, 8);
        assert_eq!(s.map.dimensions(), (8, 8));
    }

    #[test]
    fn step_increments_tick() {
        let mut s = GameState::new(4, 4);
        assert_eq!(s.tick.get(), 0);
        s.step();
        assert_eq!(s.tick.get(), 1);
        s.step();
        assert_eq!(s.tick.get(), 2);
    }

    #[test]
    fn tile_height_roundtrip() {
        let mut s = GameState::new(3, 3);
        let c = TileCoord::new(1, 1);
        s.map.set_height(c, 5).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 5);
    }

    #[test]
    fn tile_kind_default_is_grass() {
        let s = GameState::new(4, 4);
        for y in 0..4_i32 {
            for x in 0..4_i32 {
                let c = TileCoord::new(x, y);
                assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
            }
        }
    }

    #[test]
    fn tile_kind_roundtrip() {
        let mut s = GameState::new(4, 4);
        let c = TileCoord::new(2, 1);
        s.map.set_kind(c, TileKind::Water).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
        s.map.set_kind(c, TileKind::Forest).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        s.map.set_kind(c, TileKind::CoalField).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::CoalField));
    }

    #[test]
    fn bfs_finds_path_on_straight_road() {
        let mut m = Map::new_flat(8, 8, 0);
        for x in 0..=4_i32 {
            m.set_kind(TileCoord::new(x, 0), TileKind::Road).unwrap();
        }
        let path = pathfinder::find_path(&m, TileCoord::new(0, 0), TileCoord::new(4, 0));
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(*path.last().unwrap(), TileCoord::new(4, 0));
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn bfs_returns_none_when_blocked() {
        let m = Map::new_flat(8, 8, 0); // todo Grass, sin carreteras
        let path = pathfinder::find_path(&m, TileCoord::new(0, 0), TileCoord::new(4, 0));
        assert!(path.is_none());
    }

    #[test]
    fn vehicle_follows_path() {
        let mut s = GameState::new(8, 8);
        for x in 0..=4_i32 {
            s.map
                .set_kind(TileCoord::new(x, 0), TileKind::Road)
                .unwrap();
        }
        let start = TileCoord::new(0, 0);
        let dest = TileCoord::new(4, 0);
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

        let expected = pathfinder::find_path(&s.map, start, dest).expect("hay carretera");

        for (i, &tile) in expected.iter().enumerate() {
            s.step();
            assert_eq!(
                s.vehicles[0].pos,
                tile,
                "tick {} posición incorrecta",
                i + 1
            );
        }
    }

    #[test]
    fn vehicle_loads_from_industry() {
        let mut s = GameState::new(8, 8);
        let ipos = TileCoord::new(0, 0);
        let spos = TileCoord::new(4, 0);
        let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
        ind.stock = 50;
        s.industries.push(ind);
        s.stations.push(Station::new(spos));
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

        // Primer step: vehicle en ipos, cargo == 0 → carga.
        s.step();
        assert_eq!(s.vehicles[0].cargo, VEHICLE_CAPACITY.min(50));
        assert_eq!(s.industries[0].stock, 50 - VEHICLE_CAPACITY.min(50));
    }

    #[test]
    fn vehicle_delivers_to_station() {
        let mut s = GameState::new(8, 8);
        let ipos = TileCoord::new(0, 0);
        let spos = TileCoord::new(1, 0);
        let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
        ind.stock = 20;
        s.industries.push(ind);
        s.stations.push(Station::new(spos));
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

        // Tick 1: carga en industria.
        s.step();
        assert!(s.vehicles[0].cargo > 0);

        // Tick 2: vehicle.step() lo lleva a spos (dest a 1 tile); luego descarga.
        s.step();
        assert_eq!(s.vehicles[0].pos, spos);
        assert_eq!(s.vehicles[0].cargo, 0);
        assert!(s.stations[0].income > 0);
        assert!(s.economy.money > CompanyEconomy::default().money);
    }

    #[test]
    fn sim_stats_count_pickup_and_delivery() {
        let mut s = GameState::new(8, 8);
        let ipos = TileCoord::new(0, 0);
        let spos = TileCoord::new(1, 0);
        let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
        ind.stock = 20;
        s.industries.push(ind);
        s.stations.push(Station::new(spos));
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));
        assert_eq!(s.stats.cargo_pickups, 0);
        assert_eq!(s.stats.cargo_deliveries, 0);
        s.step();
        assert_eq!(s.stats.cargo_pickups, 1);
        assert!(s.stats.cargo_units_loaded > 0);
        s.step();
        assert_eq!(s.stats.cargo_deliveries, 1);
        assert!(s.stats.cargo_units_delivered > 0);
    }

    #[test]
    fn game_state_json_roundtrip() {
        let mut s = GameState::new(4, 4);
        s.industries
            .push(Industry::new(TileCoord::new(0, 0), IndustryKind::Forest));
        s.industries
            .push(Industry::new(TileCoord::new(1, 0), IndustryKind::Factory));
        s.vehicles.push(Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(0, 1),
            TileCoord::new(2, 1),
        ));
        s.jgr_tunnels_from_footer.push(JgrTunnelRecord {
            tile_n: 0,
            tile_s: 1,
            height: 2,
            is_chunnel: false,
            style_n: None,
            style_s: None,
        });
        let j = s.save_json().expect("json");
        assert!(j.contains("jgr_tunnels_from_footer"));
        let s2 = GameState::load_json(&j).expect("parse");
        assert_eq!(s2.map.dimensions(), (4, 4));
        assert_eq!(s2.industries.len(), 2);
        assert_eq!(s2.industries[0].kind, IndustryKind::Forest);
        assert_eq!(s2.industries[1].kind, IndustryKind::Factory);
        assert_eq!(s2.vehicles[0].kind, VehicleKind::Train);
        assert_eq!(s2.jgr_tunnels_from_footer.len(), 1);
        assert_eq!(s2.jgr_tunnels_from_footer[0].tile_n, 0);
    }

    #[test]
    fn factory_produces_half_as_often_as_mine() {
        assert_eq!(
            industry_produce_period_ticks(IndustryKind::Factory),
            industry_produce_period_ticks(IndustryKind::CoalMine) * 2
        );
        let mut coal = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        let mut fact = Industry::new(TileCoord::new(1, 0), IndustryKind::Factory);
        coal.produce(256);
        fact.produce(256);
        assert_eq!(coal.stock, INDUSTRY_PRODUCE_AMOUNT);
        assert_eq!(fact.stock, 0);
        fact.produce(512);
        assert_eq!(fact.stock, INDUSTRY_PRODUCE_AMOUNT);
    }

    #[test]
    fn economic_cycle_roundtrip() {
        let mut s = GameState::new(16, 16);
        let ipos = TileCoord::new(0, 0);
        let spos = TileCoord::new(2, 0); // 2 tiles de distancia

        // Industria con stock suficiente para varios ciclos.
        let mut ind = Industry::new(ipos, IndustryKind::CoalMine);
        ind.stock = 1000;
        s.industries.push(ind);
        s.stations.push(Station::new(spos));
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, ipos, spos));

        // Un ciclo completo: carga (tick 1) + viaje 2 tiles (tick 2-3) +
        // llegada/descarga (tick 3) + inversión (tick 3) + regreso 2 tiles (tick 4-5)
        // + llegada (tick 5) + inversión (tick 5) → income > 0 después de pocos ticks.
        for _ in 0..10 {
            s.step();
        }
        assert!(
            s.stations[0].income > 0,
            "debe haber income tras varios ticks"
        );
    }

    #[test]
    fn station_coverage_counts_nearby_cargo_sources_and_acceptors() {
        let mut s = GameState::new(16, 16);
        let station_pos = TileCoord::new(8, 8);
        let coal_pos = TileCoord::new(10, 8);
        let house_pos = TileCoord::new(7, 7);
        let far_forest_pos = TileCoord::new(14, 8);

        s.map.set_kind(coal_pos, TileKind::Industry).unwrap();
        s.map.set_kind(house_pos, TileKind::House).unwrap();
        s.map.set_kind(far_forest_pos, TileKind::Industry).unwrap();

        let mut coal = Industry::new(coal_pos, IndustryKind::CoalMine);
        coal.stock = 42;
        s.industries.push(coal);
        s.industries
            .push(Industry::new(far_forest_pos, IndustryKind::Forest));

        let coverage =
            station_coverage_at(&s.map, &s.industries, station_pos, STATION_COVERAGE_RADIUS);
        assert_eq!(coverage.accepts_mail, 1);
        assert_eq!(coverage.accepts_goods, 1);
        assert_eq!(coverage.supplies_coal, 1);
        assert_eq!(coverage.supplies_wood, 0);
        assert_eq!(coverage.supplied_stock, 42);
        assert!(coverage.accepts_anything());
        assert!(coverage.supplies_anything());
    }

    #[test]
    fn vehicle_moves_toward_dest() {
        let mut s = GameState::new(8, 8);
        let start = TileCoord::new(0, 0);
        let dest = TileCoord::new(5, 0);
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

        let dist_before = s.vehicles[0].manhattan_to_dest();
        s.step();
        let dist_after = s.vehicles[0].manhattan_to_dest();
        assert!(dist_after < dist_before, "debe acercarse al destino");
    }

    #[test]
    fn vehicle_inverts_on_arrival() {
        let mut s = GameState::new(8, 8);
        let start = TileCoord::new(0, 0);
        let dest = TileCoord::new(3, 0);
        s.vehicles
            .push(Vehicle::new(0, VehicleKind::Truck, start, dest));

        // Avanzar hasta llegar al destino (3 pasos + 1 de inversión).
        for _ in 0..=3 {
            s.step();
        }
        assert_eq!(s.vehicles[0].pos, dest);
        // Ahora el destino debe ser el origen original.
        assert_eq!(s.vehicles[0].dest, start);

        // Avanzar de vuelta hasta el origen.
        for _ in 0..=3 {
            s.step();
        }
        assert_eq!(s.vehicles[0].pos, start);
        assert_eq!(s.vehicles[0].dest, dest);
    }

    #[test]
    fn vehicle_with_orders_cycles_destinations() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.set_orders(vec![TileCoord::new(1, 0), TileCoord::new(1, 1)]);
        v.step();
        assert_eq!(v.pos, TileCoord::new(1, 0));
        assert_eq!(v.dest, TileCoord::new(1, 1));
        v.step();
        assert_eq!(v.pos, TileCoord::new(1, 1));
        assert_eq!(v.dest, TileCoord::new(1, 0));
    }

    #[test]
    fn two_worlds_same_vehicles_same_position() {
        let start = TileCoord::new(0, 0);
        let dest = TileCoord::new(4, 3);
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for s in [&mut a, &mut b] {
            s.vehicles
                .push(Vehicle::new(0, VehicleKind::Truck, start, dest));
        }
        for _ in 0..50 {
            a.step();
            b.step();
        }
        assert_eq!(a.vehicles[0].pos, b.vehicles[0].pos);
    }

    #[test]
    fn industry_produces_on_schedule() {
        let mut s = GameState::new(8, 8);
        s.industries
            .push(Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine));

        // Sin ticks no hay producción.
        assert_eq!(s.industries[0].stock, 0);

        // Avanzar exactamente INDUSTRY_PRODUCE_TICKS ticks.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);

        // Un segundo ciclo completo.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT * 2);
    }

    #[test]
    fn industry_does_not_exceed_capacity() {
        let mut s = GameState::new(8, 8);
        let mut ind = Industry::new(TileCoord::new(0, 0), IndustryKind::Forest);
        ind.capacity = INDUSTRY_PRODUCE_AMOUNT; // capacidad mínima: un ciclo
        s.industries.push(ind);

        // Primer ciclo llena hasta capacity.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);

        // Segundo ciclo: stock saturado, no supera capacity.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);
    }

    #[test]
    fn two_worlds_same_industries_same_stock() {
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for state in [&mut a, &mut b] {
            state
                .industries
                .push(Industry::new(TileCoord::new(1, 2), IndustryKind::CoalMine));
            state
                .industries
                .push(Industry::new(TileCoord::new(3, 4), IndustryKind::Forest));
        }
        for _ in 0..INDUSTRY_PRODUCE_TICKS * 3 {
            a.step();
            b.step();
        }
        assert_eq!(a.industries[0].stock, b.industries[0].stock);
        assert_eq!(a.industries[1].stock, b.industries[1].stock);
    }

    #[test]
    fn tile_height_and_kind_are_independent() {
        let mut s = GameState::new(4, 4);
        let c = TileCoord::new(1, 2);
        s.map.set_height(c, 7).unwrap();
        s.map.set_kind(c, TileKind::Forest).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 7);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        // Cambiar altura no afecta el tipo.
        s.map.set_height(c, 3).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        // Cambiar tipo no afecta la altura.
        s.map.set_kind(c, TileKind::Water).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 3);
    }
}
