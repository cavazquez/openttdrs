//! Link graph observacional: flujos estación→estación por cargo.
//! Alimenta el pipeline de paridad MCF (`linkgraph_parity`) y la UI Link Graph.

use std::collections::HashMap;

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Chunk de runtime `LGRJ`/`LGRS` conservado de forma opaca durante un
/// roundtrip `.sav`. No se interpreta ni se ejecuta en Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkGraphRuntimeChunk {
    pub(crate) name: [u8; 4],
    pub(crate) ch_type: u8,
    pub(crate) body: Vec<u8>,
}

/// Clave de arista observada (origen de carga → destino de descarga/transfer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LinkEdgeKey {
    pub from: TileCoord,
    pub to: TileCoord,
    pub cargo: CargoType,
}

/// Muestra de flujo en una arista.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkFlowSample {
    /// Unidades en el mes calendario actual.
    pub units_month: u64,
    /// Unidades acumuladas desde el inicio de la partida / carga.
    pub units_total: u64,
    /// Capacidad de vehículos acumulada (`IncreaseStats`-like).
    #[serde(default)]
    pub capacity_total: u64,
    /// Suma ponderada de tiempos de viaje (ticks × capacity), como `OpenTTD`.
    #[serde(default)]
    pub travel_time_sum: u64,
}

impl LinkFlowSample {
    /// Tiempo de viaje medio en ticks (`travel_time_sum / capacity`).
    #[must_use]
    pub fn travel_time(&self) -> u32 {
        if self.capacity_total == 0 {
            return 0;
        }
        u32::try_from(self.travel_time_sum / self.capacity_total).unwrap_or(u32::MAX)
    }
}

/// Estadísticas de flujos observados.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkGraphStats {
    #[serde(default)]
    pub edges: HashMap<LinkEdgeKey, LinkFlowSample>,
    /// Runtime `LGRJ`/`LGRS` original, solo válido hasta mutar el grafo.
    #[serde(skip)]
    pub(crate) runtime_chunks: Vec<LinkGraphRuntimeChunk>,
}

impl LinkGraphStats {
    /// Registra un flujo `from → to` con `units` del `cargo` dado.
    pub fn record_flow(&mut self, from: TileCoord, to: TileCoord, cargo: CargoType, units: u32) {
        self.record_trip(from, to, cargo, units, units, 0);
    }

    /// Registra viaje: usage (`units`) + capacidad + tiempo de viaje.
    pub fn record_trip(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        cargo: CargoType,
        units: u32,
        capacity: u32,
        travel_time: u32,
    ) {
        if from == to || (units == 0 && capacity == 0) {
            return;
        }
        // Los jobs y la cola de OpenTTD quedan obsoletos al cambiar el grafo.
        self.runtime_chunks.clear();
        let key = LinkEdgeKey { from, to, cargo };
        let sample = self.edges.entry(key).or_default();
        let u = u64::from(units);
        let cap = u64::from(capacity);
        sample.units_month = sample.units_month.saturating_add(u);
        sample.units_total = sample.units_total.saturating_add(u);
        sample.capacity_total = sample.capacity_total.saturating_add(cap);
        if travel_time > 0 && cap > 0 {
            sample.travel_time_sum = sample
                .travel_time_sum
                .saturating_add(u64::from(travel_time).saturating_mul(cap));
        } else if travel_time == 0
            && cap > 0
            && sample.travel_time_sum > 0
            && sample.capacity_total > cap
        {
            // Sin medición nueva: prorratea el promedio previo (OpenTTD Update Increase).
            let prev_cap = sample.capacity_total - cap;
            let avg = sample.travel_time_sum / prev_cap.max(1);
            sample.travel_time_sum = sample
                .travel_time_sum
                .saturating_add(avg.saturating_mul(cap));
        }
    }

    /// Al cierre de mes: pone a cero los contadores mensuales (conserva totales).
    pub fn rollover_month(&mut self) {
        for sample in self.edges.values_mut() {
            sample.units_month = 0;
        }
    }

    /// Aristas ordenadas por volumen del mes (desc); desempate por total.
    #[must_use]
    pub fn top_edges_by_month(&self, limit: usize) -> Vec<(LinkEdgeKey, LinkFlowSample)> {
        let mut entries: Vec<_> = self.edges.iter().map(|(k, v)| (*k, *v)).collect();
        entries.sort_by(|a, b| {
            b.1.units_month
                .cmp(&a.1.units_month)
                .then_with(|| b.1.units_total.cmp(&a.1.units_total))
                .then_with(|| a.0.from.x.cmp(&b.0.from.x))
                .then_with(|| a.0.from.y.cmp(&b.0.from.y))
                .then_with(|| a.0.to.x.cmp(&b.0.to.x))
                .then_with(|| a.0.to.y.cmp(&b.0.to.y))
        });
        entries.truncate(limit);
        entries
    }

    /// Filtra por cargo (si `None`, todas) y ordena por mes.
    #[must_use]
    pub fn top_edges_filtered(
        &self,
        cargo: Option<CargoType>,
        limit: usize,
    ) -> Vec<(LinkEdgeKey, LinkFlowSample)> {
        let mut entries: Vec<_> = self
            .edges
            .iter()
            .filter(|(k, _)| cargo.is_none_or(|c| k.cargo == c))
            .map(|(k, v)| (*k, *v))
            .collect();
        entries.sort_by(|a, b| {
            b.1.units_month
                .cmp(&a.1.units_month)
                .then_with(|| b.1.units_total.cmp(&a.1.units_total))
        });
        entries.truncate(limit);
        entries
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn record_trip_tracks_capacity_and_travel_time() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        g.record_trip(a, b, CargoType::Coal, 10, 40, 100);
        let sample = g.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Coal,
        }];
        assert_eq!(sample.capacity_total, 40);
        assert_eq!(sample.travel_time(), 100);
    }

    #[test]
    fn record_and_rollover() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(1, 1);
        let b = TileCoord::new(2, 2);
        g.record_flow(a, b, CargoType::Coal, 10);
        g.record_flow(a, b, CargoType::Coal, 5);
        let sample = g.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Coal,
        }];
        assert_eq!(sample.units_month, 15);
        assert_eq!(sample.units_total, 15);
        g.rollover_month();
        let sample = g.edges[&LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Coal,
        }];
        assert_eq!(sample.units_month, 0);
        assert_eq!(sample.units_total, 15);
    }

    #[test]
    fn ignores_zero_and_self() {
        let mut g = LinkGraphStats::default();
        let a = TileCoord::new(0, 0);
        g.record_flow(a, a, CargoType::Goods, 9);
        g.record_flow(a, TileCoord::new(1, 0), CargoType::Goods, 0);
        assert!(g.edges.is_empty());
    }
}
