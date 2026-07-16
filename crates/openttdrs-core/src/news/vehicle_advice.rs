//! Avisos operativos de vehículos: sin órdenes, sin ruta, esperando carga, etc.

use std::collections::HashSet;

use crate::map::TileCoord;
use crate::station::{self, STATION_COVERAGE_RADIUS, station_covers_tile};
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

use super::queue::{NewsItem, NewsReference, add_news_item};
use super::queue::{NewsType, default_display_for_type};

/// Variante de aviso operativo de vehículo (deduplicación en sim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VehicleAdviceKind {
    NoNetworkRoute,
    NoOrders,
    IncompatibleStop,
    WaitingForCargo,
    /// Sin path PBS tras reversa / timeout (`MarkTrainAsStuck`).
    PbsStuck,
}

fn advice_key(vehicle_id: u32, kind: VehicleAdviceKind) -> u64 {
    (u64::from(vehicle_id) << 8) | kind as u64
}

fn vehicle_has_incompatible_stop(state: &crate::GameState, v: &Vehicle) -> bool {
    if !v.running || v.orders.is_empty() {
        return false;
    }
    let Some(order) = v.orders.get(v.current_order) else {
        return false;
    };
    match order {
        VehicleOrder::Station { station, .. } => state
            .stations
            .iter()
            .find(|s| s.pos == *station)
            .is_some_and(|st| !st.can_service_vehicle(v.kind) || st.is_waypoint()),
        VehicleOrder::Waypoint { .. } => v.kind != VehicleKind::Train,
        VehicleOrder::Depot { .. } | VehicleOrder::Tile(_) | VehicleOrder::Conditional { .. } => {
            false
        }
    }
}

fn vehicle_waiting_for_cargo(state: &crate::GameState, v: &Vehicle) -> bool {
    if !v.running || v.cargo > 0 || v.no_network_route_to_order || v.orders.is_empty() {
        return false;
    }
    let Some(VehicleOrder::Station { station, .. }) = v.orders.get(v.current_order).copied() else {
        return false;
    };
    if !station_covers_tile(station, v.pos, 1) && v.pos != station {
        return false;
    }
    let Some(st) = state.stations.iter().find(|s| s.pos == station) else {
        return false;
    };
    if !st.can_service_vehicle(v.kind) {
        return false;
    }
    let industry_has = state.industries.iter().any(|ind| {
        ind.stock > 0
            && station::industry_in_station_coverage(ind, station, STATION_COVERAGE_RADIUS)
            && st.accepts_cargo(ind.output_cargo())
    });
    let station_has = match v.kind {
        VehicleKind::Bus | VehicleKind::Tram => {
            st.cargo_stock.passengers > 0 || st.cargo_stock.mail > 0
        }
        VehicleKind::Truck | VehicleKind::Train => {
            st.stock > 0 || st.cargo_stock.pick_freight_to_load(v.cargo_type).is_some()
        }
        VehicleKind::Ship | VehicleKind::Aircraft => false,
    };
    !industry_has && !station_has
}

pub fn vehicle_advice_kind(state: &crate::GameState, v: &Vehicle) -> Option<VehicleAdviceKind> {
    if !v.running {
        return None;
    }
    if v.no_network_route_to_order {
        return Some(VehicleAdviceKind::NoNetworkRoute);
    }
    if v.orders.is_empty() {
        return Some(VehicleAdviceKind::NoOrders);
    }
    if vehicle_has_incompatible_stop(state, v) {
        return Some(VehicleAdviceKind::IncompatibleStop);
    }
    if vehicle_waiting_for_cargo(state, v) {
        return Some(VehicleAdviceKind::WaitingForCargo);
    }
    None
}

fn vehicle_advice_headline(
    vehicle_id: u32,
    current_order: usize,
    kind: VehicleAdviceKind,
) -> String {
    match kind {
        VehicleAdviceKind::NoNetworkRoute => format!(
            "Sin ruta por red: vehículo {vehicle_id} (orden {})",
            current_order.saturating_add(1)
        ),
        VehicleAdviceKind::NoOrders => format!("Sin órdenes: vehículo {vehicle_id}"),
        VehicleAdviceKind::IncompatibleStop => {
            format!("Parada incompatible: vehículo {vehicle_id}")
        }
        VehicleAdviceKind::WaitingForCargo => {
            format!("Sin carga disponible: vehículo {vehicle_id}")
        }
        VehicleAdviceKind::PbsStuck => {
            format!("Sin camino reservado: vehículo {vehicle_id}")
        }
    }
}

pub fn push_vehicle_advice_news(
    state: &mut crate::GameState,
    vehicle_id: u32,
    current_order: usize,
    at: TileCoord,
    advice: VehicleAdviceKind,
) {
    let headline = vehicle_advice_headline(vehicle_id, current_order, advice);
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        headline,
        None,
        NewsType::VehicleAdvice,
        default_display_for_type(NewsType::VehicleAdvice),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
}

/// Emite ticker de aviso la primera vez que un vehículo entra en cada condición.
pub fn poll_vehicle_advice_news(state: &mut crate::GameState) {
    let mut active_keys = HashSet::new();
    let mut pending = Vec::new();
    for v in &state.vehicles {
        let Some(advice) = vehicle_advice_kind(state, v) else {
            continue;
        };
        let key = advice_key(v.id, advice);
        active_keys.insert(key);
        if state.news_advice_sent.contains(&key) {
            continue;
        }
        pending.push((v.id, v.current_order, v.pos, advice, key));
    }
    state
        .news_advice_sent
        .retain(|key| active_keys.contains(key));
    for (vehicle_id, current_order, pos, advice, key) in pending {
        push_vehicle_advice_news(state, vehicle_id, current_order, pos, advice);
        state.news_advice_sent.insert(key);
    }
}
