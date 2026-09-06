//! `FlowStat` / `FlowStatMap` simplificados (#49).
//!
//! `FlowStat` de partida + `resolve_next_hop`.
//! Los shares los rellena [`crate::cargodist::parity`] (Demand + MCF1/2).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cargo::{CARGO_CLASS_ARMOURED, CARGO_CLASS_MAIL, CARGO_CLASS_PASSENGERS, CargoType};
use crate::cargo_spec::{CargoSpecDef, cargo_spec_for_type};
use crate::cargodist::legacy::link_graph::LinkGraphStats;
use crate::map::TileCoord;

/// Modo de distribución (`linkgraph.distribution_*` simplificado).
///
/// Los discriminantes son los bytes nativos de `OpenTTD`
/// `DistributionType`: no se debe depender del orden de declaración para
/// serializarlos a un `.sav`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionType {
    /// Sin auto-routing: `next_hop` solo desde órdenes del vehículo.
    #[default]
    Manual = 0,
    /// `FlowStat` vía pipeline `OpenTTD` (Demand Asymmetric + MCF).
    Asymmetric = 1,
    /// Demand Symmetric `OpenTTD` (geografía + supply) + MCF.
    Symmetric = 2,
}

impl DistributionType {
    /// Decodifica el byte de `OpenTTD::DistributionType`.
    #[must_use]
    pub const fn from_openttd(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Manual),
            1 => Some(Self::Asymmetric),
            2 => Some(Self::Symmetric),
            _ => None,
        }
    }

    /// Byte nativo de `OpenTTD::DistributionType`.
    #[must_use]
    pub const fn as_openttd(self) -> u8 {
        self as u8
    }
}

/// Dos segundos aproximados por día económico, igual que
/// `EconomyTime::SECONDS_PER_DAY` de `OpenTTD` 15.3.
pub const ECONOMY_SECONDS_PER_DAY: u16 = 2;

/// Perfil nativo completo de `linkgraph.*` que vive en `PATS`.
///
/// Se mantiene separado del modo global legacy para que los JSON propios
/// anteriores a la interoperabilidad por clase sigan ejecutando su
/// `distribution` único, mientras un `.sav` conserva exactamente sus
/// valores en segundos y sus cuatro selectores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoDistPerCargoSettings {
    /// Presupuesto de un job (`linkgraph.recalc_time`, segundos).
    pub recalc_time_seconds: u16,
    /// Cadencia entre jobs (`linkgraph.recalc_interval`, segundos).
    pub recalc_interval_seconds: u16,
    pub distribution_pax: DistributionType,
    pub distribution_mail: DistributionType,
    pub distribution_armoured: DistributionType,
    pub distribution_default: DistributionType,
    pub accuracy: u8,
    pub demand_size: u8,
    pub demand_distance: u8,
    pub short_path_saturation: u8,
}

impl Default for CargoDistPerCargoSettings {
    fn default() -> Self {
        Self {
            recalc_time_seconds: 32,
            recalc_interval_seconds: 8,
            distribution_pax: DistributionType::Manual,
            distribution_mail: DistributionType::Manual,
            distribution_armoured: DistributionType::Manual,
            distribution_default: DistributionType::Manual,
            accuracy: 16,
            demand_size: 100,
            demand_distance: 100,
            short_path_saturation: 80,
        }
    }
}

impl CargoDistPerCargoSettings {
    #[must_use]
    fn distribution_for(self, cargo: CargoType, catalog: &[CargoSpecDef]) -> DistributionType {
        // OpenTTD consulta las clases del CargoSpec activo. Para vanilla sin
        // spec registrada usamos las clases estáticas; para NewGRF la entrada
        // del catálogo es autoritativa.
        let classes = cargo_spec_for_type(catalog, cargo)
            .map_or_else(|| cargo.classes(), |spec| spec.classes);
        if classes & CARGO_CLASS_PASSENGERS != 0 {
            self.distribution_pax
        } else if classes & CARGO_CLASS_MAIL != 0 {
            self.distribution_mail
        } else if classes & CARGO_CLASS_ARMOURED != 0 {
            self.distribution_armoured
        } else {
            self.distribution_default
        }
    }

    #[must_use]
    const fn has_automatic_distribution(self) -> bool {
        !matches!(self.distribution_pax, DistributionType::Manual)
            || !matches!(self.distribution_mail, DistributionType::Manual)
            || !matches!(self.distribution_armoured, DistributionType::Manual)
            || !matches!(self.distribution_default, DistributionType::Manual)
    }
}

/// Ajustes de `CargoDist` (persistidos; flows se reconstruyen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoDistSettings {
    /// Modo global de los JSON propios anteriores al perfil nativo por clase.
    ///
    /// Cuando `per_cargo` es `None`, conserva el contrato legacy: el mismo
    /// modo se aplica a todas las cargas. Un comando UI simplificado también
    /// vuelve deliberadamente a este modo global.
    #[serde(default)]
    pub distribution: DistributionType,
    /// Intervalo legacy en días económicos.
    ///
    /// Un `.sav` moderno usa `per_cargo.recalc_interval_seconds`; este
    /// campo existe para no reinterpretar JSON propio de versiones anteriores.
    #[serde(default = "default_recalc_interval_days")]
    pub recalc_interval_days: u32,
    /// Perfil exacto de `PATS.linkgraph.*`; ausente para estados JSON legacy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_cargo: Option<CargoDistPerCargoSettings>,
}

const fn default_recalc_interval_days() -> u32 {
    4
}

impl Default for CargoDistSettings {
    fn default() -> Self {
        Self {
            distribution: DistributionType::default(),
            recalc_interval_days: default_recalc_interval_days(),
            per_cargo: None,
        }
    }
}

impl CargoDistSettings {
    /// Perfil que debe escribirse en `PATS`.
    ///
    /// `distribution_default` no admite `Symmetric` en `OpenTTD`. El modo
    /// global legacy sí lo permitía para el simulador propio; al exportarlo se
    /// proyecta al máximo nativo válido (`Asymmetric`) sólo para esa clase.
    #[must_use]
    pub fn openttd_settings(self) -> CargoDistPerCargoSettings {
        let mut settings = self.per_cargo.unwrap_or_else(|| {
            let legacy_default = if matches!(self.distribution, DistributionType::Symmetric) {
                DistributionType::Asymmetric
            } else {
                self.distribution
            };
            let days = self.recalc_interval_days.max(1);
            let seconds = u16::try_from(days.saturating_mul(u32::from(ECONOMY_SECONDS_PER_DAY)))
                .unwrap_or(u16::MAX)
                .clamp(4, 90);
            CargoDistPerCargoSettings {
                recalc_interval_seconds: seconds,
                distribution_pax: self.distribution,
                distribution_mail: self.distribution,
                distribution_armoured: self.distribution,
                distribution_default: legacy_default,
                ..CargoDistPerCargoSettings::default()
            }
        });
        // La UI nativa no permite estos valores fuera de rango. Proteger el
        // writer también cubre estados JSON editados manualmente, sin tocar
        // ninguna entrada válida que se haya importado desde PATS.
        settings.recalc_interval_seconds = settings.recalc_interval_seconds.clamp(4, 90);
        settings.recalc_time_seconds = settings.recalc_time_seconds.clamp(1, 9_000);
        settings.accuracy = settings.accuracy.clamp(2, 64);
        settings.demand_size = settings.demand_size.min(100);
        settings.short_path_saturation = settings.short_path_saturation.min(250);
        if matches!(settings.distribution_default, DistributionType::Symmetric) {
            settings.distribution_default = DistributionType::Asymmetric;
        }
        settings
    }

    /// Cadencia del scheduler síncrono en días económicos.
    ///
    /// `OpenTTD` guarda segundos y divide por
    /// `EconomyTime::SECONDS_PER_DAY` antes de hacer el módulo. La división
    /// truncada es intencional y conserva el comportamiento para valores
    /// impares válidos como 5 s.
    #[must_use]
    pub fn effective_recalc_interval_days(self) -> u32 {
        self.per_cargo.map_or_else(
            || self.recalc_interval_days.max(1),
            |settings| u32::from(settings.recalc_interval_seconds / ECONOMY_SECONDS_PER_DAY).max(1),
        )
    }

    /// Modo efectivo para una carga concreta, con la precedencia nativa de
    /// clases pasajeros → correo → blindado → resto.
    #[must_use]
    pub fn distribution_for(self, cargo: CargoType, catalog: &[CargoSpecDef]) -> DistributionType {
        self.per_cargo.map_or(self.distribution, |settings| {
            settings.distribution_for(cargo, catalog)
        })
    }

    /// Indica si una carga concreta usa distribución automática.
    #[must_use]
    pub fn is_automatically_distributed(self, cargo: CargoType, catalog: &[CargoSpecDef]) -> bool {
        !matches!(
            self.distribution_for(cargo, catalog),
            DistributionType::Manual
        )
    }

    /// Indica si algún grupo de carga tiene distribución automática.
    #[must_use]
    pub fn has_automatic_distribution(self) -> bool {
        self.per_cargo.map_or(
            !matches!(self.distribution, DistributionType::Manual),
            CargoDistPerCargoSettings::has_automatic_distribution,
        )
    }

    /// Restablece el modo global usado por el comando UI legacy.
    pub fn set_legacy_distribution(&mut self, distribution: DistributionType) {
        self.distribution = distribution;
        self.per_cargo = None;
    }
}

/// Shares `via → amount` para un origen (`FlowStat`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStat {
    /// Pares (via, amount); amount > 0.
    pub shares: Vec<(TileCoord, u32)>,
}

impl FlowStat {
    pub fn add_share(&mut self, via: TileCoord, amount: u32) {
        if amount == 0 {
            return;
        }
        if let Some((_, acc)) = self.shares.iter_mut().find(|(v, _)| *v == via) {
            *acc = acc.saturating_add(amount);
        } else {
            self.shares.push((via, amount));
        }
    }

    /// Siguiente hop: vía con mayor share (determinista; empate por coords).
    #[must_use]
    pub fn get_via(&self) -> Option<TileCoord> {
        self.shares
            .iter()
            .max_by(|a, b| {
                a.1.cmp(&b.1)
                    .then_with(|| a.0.x.cmp(&b.0.x))
                    .then_with(|| a.0.y.cmp(&b.0.y))
            })
            .map(|(via, _)| *via)
    }

    /// `GetVia` estilo `OpenTTD`: `RandomRange` ponderado por shares.
    pub fn get_via_random(
        &self,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        let total: u32 = self.shares.iter().map(|(_, a)| *a).sum();
        if total == 0 {
            return None;
        }
        let mut pick = rng.random_range(total);
        for (via, amount) in &self.shares {
            if pick < *amount {
                return Some(*via);
            }
            pick = pick.saturating_sub(*amount);
        }
        self.shares.last().map(|(via, _)| *via)
    }

    /// `FlowStat::GetVia(excluded, excluded2)` — evita hops obsoletos al reroute.
    pub fn get_via_excluding(
        &self,
        excluded: TileCoord,
        excluded2: Option<TileCoord>,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        let filtered: Vec<(TileCoord, u32)> = self
            .shares
            .iter()
            .copied()
            .filter(|(via, amount)| {
                *amount > 0 && *via != excluded && excluded2.is_none_or(|e2| *via != e2)
            })
            .collect();
        let total: u32 = filtered.iter().map(|(_, a)| *a).sum();
        if total == 0 {
            return None;
        }
        let mut pick = rng.random_range(total);
        for (via, amount) in &filtered {
            if pick < *amount {
                return Some(*via);
            }
            pick = pick.saturating_sub(*amount);
        }
        filtered.last().map(|(via, _)| *via)
    }

    #[must_use]
    pub fn get_share(&self, via: TileCoord) -> u32 {
        self.shares
            .iter()
            .find(|(v, _)| *v == via)
            .map_or(0, |(_, a)| *a)
    }
}

/// Flows por origen en una estación y un cargo (`FlowStatMap`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStatMap {
    /// `origin → FlowStat`.
    pub by_origin: HashMap<TileCoord, FlowStat>,
}

impl FlowStatMap {
    pub fn add_flow(&mut self, origin: TileCoord, via: TileCoord, amount: u32) {
        if amount == 0 || origin == via {
            return;
        }
        self.by_origin
            .entry(origin)
            .or_default()
            .add_share(via, amount);
    }

    #[must_use]
    pub fn get_via(&self, origin: TileCoord) -> Option<TileCoord> {
        self.by_origin.get(&origin).and_then(FlowStat::get_via)
    }

    pub fn get_via_random(
        &self,
        origin: TileCoord,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_origin
            .get(&origin)
            .and_then(|fs| fs.get_via_random(rng))
    }

    pub fn get_via_excluding(
        &self,
        origin: TileCoord,
        excluded: TileCoord,
        excluded2: Option<TileCoord>,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_origin
            .get(&origin)
            .and_then(|fs| fs.get_via_excluding(excluded, excluded2, rng))
    }
}

/// Tabla de flows de una estación: por cargo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationFlowTable {
    pub by_cargo: HashMap<CargoType, FlowStatMap>,
}

impl StationFlowTable {
    pub fn add_flow(&mut self, cargo: CargoType, origin: TileCoord, via: TileCoord, amount: u32) {
        self.by_cargo
            .entry(cargo)
            .or_default()
            .add_flow(origin, via, amount);
    }

    #[must_use]
    pub fn get_via(&self, cargo: CargoType, origin: TileCoord) -> Option<TileCoord> {
        self.by_cargo.get(&cargo).and_then(|m| m.get_via(origin))
    }

    pub fn get_via_random(
        &self,
        cargo: CargoType,
        origin: TileCoord,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_cargo
            .get(&cargo)
            .and_then(|m| m.get_via_random(origin, rng))
    }

    pub fn get_via_excluding(
        &self,
        cargo: CargoType,
        origin: TileCoord,
        excluded: TileCoord,
        excluded2: Option<TileCoord>,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        self.by_cargo
            .get(&cargo)
            .and_then(|m| m.get_via_excluding(origin, excluded, excluded2, rng))
    }
}

/// Arista planificada agregada desde shares (`estación → via`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedFlowEdge {
    pub from: TileCoord,
    pub to: TileCoord,
    pub cargo: CargoType,
    pub amount: u32,
}

/// Flows por tesela de estación (reconstruidos; no hace falta persistir).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationFlows {
    pub by_station: HashMap<TileCoord, StationFlowTable>,
}

impl StationFlows {
    /// Mapper ingenuo: cada arista observada `from→to` es un share en `from`
    /// con origen=`from` (y también origen genérico si hace falta).
    #[must_use]
    pub fn from_link_graph(graph: &LinkGraphStats) -> Self {
        let mut out = Self::default();
        for (key, sample) in &graph.edges {
            let amount = u32::try_from(sample.units_total.min(u64::from(u32::MAX))).unwrap_or(0);
            if amount == 0 {
                continue;
            }
            // En la estación origen del enlace: cargo con origin=from via=to.
            out.by_station
                .entry(key.from)
                .or_default()
                .add_flow(key.cargo, key.from, key.to, amount);
        }
        out
    }

    #[must_use]
    pub fn get_via(
        &self,
        at_station: TileCoord,
        cargo: CargoType,
        origin: TileCoord,
    ) -> Option<TileCoord> {
        let table = self.by_station.get(&at_station)?;
        table
            .get_via(cargo, origin)
            .or_else(|| table.get_via(cargo, at_station))
    }

    pub fn get_via_random(
        &self,
        at_station: TileCoord,
        cargo: CargoType,
        origin: TileCoord,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        let table = self.by_station.get(&at_station)?;
        table
            .get_via_random(cargo, origin, rng)
            .or_else(|| table.get_via_random(cargo, at_station, rng))
    }

    /// `GoodsEntry::GetVia(source, excluded, excluded2)` para reroute de carga.
    pub fn get_via_excluding(
        &self,
        at_station: TileCoord,
        cargo: CargoType,
        origin: TileCoord,
        excluded: TileCoord,
        excluded2: Option<TileCoord>,
        rng: &mut crate::cargodist::parity::Randomizer,
    ) -> Option<TileCoord> {
        let table = self.by_station.get(&at_station)?;
        table
            .get_via_excluding(cargo, origin, excluded, excluded2, rng)
            .or_else(|| table.get_via_excluding(cargo, at_station, excluded, excluded2, rng))
    }

    /// Agrega shares como aristas planificadas (orden: amount desc).
    #[must_use]
    pub fn planned_edges_filtered(
        &self,
        cargo: Option<CargoType>,
        limit: usize,
    ) -> Vec<PlannedFlowEdge> {
        let mut acc: HashMap<(TileCoord, TileCoord, CargoType), u32> = HashMap::new();
        for (station, table) in &self.by_station {
            for (cargo_ty, map) in &table.by_cargo {
                if cargo.is_some_and(|c| c != *cargo_ty) {
                    continue;
                }
                for flow in map.by_origin.values() {
                    for (via, amount) in &flow.shares {
                        if *amount == 0 {
                            continue;
                        }
                        let entry = acc.entry((*station, *via, *cargo_ty)).or_default();
                        *entry = entry.saturating_add(*amount);
                    }
                }
            }
        }
        let mut edges: Vec<PlannedFlowEdge> = acc
            .into_iter()
            .map(|((from, to, cargo), amount)| PlannedFlowEdge {
                from,
                to,
                cargo,
                amount,
            })
            .collect();
        edges.sort_by(|a, b| {
            b.amount
                .cmp(&a.amount)
                .then_with(|| a.from.x.cmp(&b.from.x))
                .then_with(|| a.from.y.cmp(&b.from.y))
                .then_with(|| a.to.x.cmp(&b.to.x))
                .then_with(|| a.to.y.cmp(&b.to.y))
        });
        edges.truncate(limit);
        edges
    }
}

/// Elige `next_hop` según modo de distribución.
pub fn resolve_next_hop(
    distribution: DistributionType,
    flows: &StationFlows,
    at_station: TileCoord,
    cargo: CargoType,
    origin: TileCoord,
    order_hop: Option<TileCoord>,
    rng: &mut crate::cargodist::parity::Randomizer,
) -> Option<TileCoord> {
    match distribution {
        DistributionType::Manual => order_hop,
        DistributionType::Asymmetric | DistributionType::Symmetric => flows
            .get_via_random(at_station, cargo, origin, rng)
            .or(order_hop),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn get_via_picks_largest_share() {
        let mut fs = FlowStat::default();
        fs.add_share(TileCoord::new(2, 2), 10);
        fs.add_share(TileCoord::new(3, 3), 30);
        fs.add_share(TileCoord::new(4, 4), 5);
        assert_eq!(fs.get_via(), Some(TileCoord::new(3, 3)));
    }

    #[test]
    fn from_link_graph_builds_station_flows() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(5, 5);
        g.record_flow(a, b, CargoType::Coal, 40);
        let flows = StationFlows::from_link_graph(&g);
        assert_eq!(
            flows.get_via(a, CargoType::Coal, a),
            Some(b),
            "desde A el hop de carbón es B"
        );
        let mut rng = crate::cargodist::parity::Randomizer::new(1);
        assert_eq!(
            resolve_next_hop(
                DistributionType::Manual,
                &flows,
                a,
                CargoType::Coal,
                a,
                Some(TileCoord::new(9, 9)),
                &mut rng,
            ),
            Some(TileCoord::new(9, 9))
        );
        assert_eq!(
            resolve_next_hop(
                DistributionType::Asymmetric,
                &flows,
                a,
                CargoType::Coal,
                a,
                Some(TileCoord::new(9, 9)),
                &mut rng,
            ),
            Some(b)
        );
    }

    #[test]
    fn planned_edges_two_hop() {
        let mut flows = StationFlows::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(3, 3);
        let c = TileCoord::new(5, 5);
        flows
            .by_station
            .entry(a)
            .or_default()
            .add_flow(CargoType::Goods, a, b, 30);
        flows
            .by_station
            .entry(b)
            .or_default()
            .add_flow(CargoType::Goods, a, c, 30);
        let edges = flows.planned_edges_filtered(Some(CargoType::Goods), 10);
        assert!(
            edges
                .iter()
                .any(|e| e.from == a && e.to == b && e.amount == 30)
        );
        assert!(
            edges
                .iter()
                .any(|e| e.from == b && e.to == c && e.amount == 30)
        );
    }

    #[test]
    fn openttd_distribution_bytes_and_class_precedence_are_preserved() {
        assert_eq!(
            DistributionType::from_openttd(0),
            Some(DistributionType::Manual)
        );
        assert_eq!(
            DistributionType::from_openttd(1),
            Some(DistributionType::Asymmetric)
        );
        assert_eq!(
            DistributionType::from_openttd(2),
            Some(DistributionType::Symmetric)
        );
        assert_eq!(DistributionType::from_openttd(3), None);
        assert_eq!(DistributionType::Symmetric.as_openttd(), 2);

        let mut profile = CargoDistPerCargoSettings {
            distribution_pax: DistributionType::Symmetric,
            distribution_mail: DistributionType::Asymmetric,
            distribution_armoured: DistributionType::Manual,
            distribution_default: DistributionType::Asymmetric,
            recalc_interval_seconds: 5,
            ..CargoDistPerCargoSettings::default()
        };
        let settings = CargoDistSettings {
            per_cargo: Some(profile),
            ..CargoDistSettings::default()
        };
        assert_eq!(
            settings.distribution_for(CargoType::Passengers, &[]),
            DistributionType::Symmetric
        );
        assert_eq!(
            settings.distribution_for(CargoType::Mail, &[]),
            DistributionType::Asymmetric
        );
        assert_eq!(
            settings.distribution_for(CargoType::Valuables, &[]),
            DistributionType::Manual
        );
        assert_eq!(
            settings.distribution_for(CargoType::Coal, &[]),
            DistributionType::Asymmetric
        );
        // OpenTTD hace división entera por EconomyTime::SECONDS_PER_DAY.
        assert_eq!(settings.effective_recalc_interval_days(), 2);

        profile.distribution_pax = DistributionType::Manual;
        profile.distribution_mail = DistributionType::Manual;
        profile.distribution_armoured = DistributionType::Manual;
        profile.distribution_default = DistributionType::Manual;
        let custom = CargoSpecDef {
            id: CargoType::Custom(0).cargo_id(),
            classes: CARGO_CLASS_PASSENGERS | CARGO_CLASS_MAIL,
            ..CargoSpecDef::default()
        };
        let custom_settings = CargoDistSettings {
            per_cargo: Some(CargoDistPerCargoSettings {
                distribution_pax: DistributionType::Symmetric,
                distribution_mail: DistributionType::Asymmetric,
                ..profile
            }),
            ..CargoDistSettings::default()
        };
        assert_eq!(
            custom_settings.distribution_for(CargoType::Custom(0), &[custom]),
            DistributionType::Symmetric,
            "pasajeros precede a correo, igual que GetDistributionType"
        );
    }
}
