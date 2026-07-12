//! Link graph observacional: flujos estación→estación por cargo (sin routing `CargoDist`).

use std::collections::HashMap;

use crate::cargo::CargoType;
use crate::map::TileCoord;

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
}

/// Estadísticas de flujos observados.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkGraphStats {
    #[serde(default)]
    pub edges: HashMap<LinkEdgeKey, LinkFlowSample>,
}

impl LinkGraphStats {
    /// Registra un flujo `from → to` con `units` del `cargo` dado.
    pub fn record_flow(&mut self, from: TileCoord, to: TileCoord, cargo: CargoType, units: u32) {
        if from == to || units == 0 {
            return;
        }
        let key = LinkEdgeKey { from, to, cargo };
        let sample = self.edges.entry(key).or_default();
        let u = u64::from(units);
        sample.units_month = sample.units_month.saturating_add(u);
        sample.units_total = sample.units_total.saturating_add(u);
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
