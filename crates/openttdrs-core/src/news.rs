//! Cola de noticias al estilo `OpenTTD` (`AddNewsItem`, ticker / periódico).

use std::collections::{HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::map::TileCoord;
use crate::station::{self, STATION_COVERAGE_RADIUS, station_covers_tile};
use crate::tick::GameTick;
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

/// Año base del calendario mostrado en la barra (Y1 del sim = 1950).
pub const CALENDAR_BASE_YEAR: u32 = 1950;
pub const CALENDAR_DAYS_PER_YEAR: u64 = 365;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NewsType {
    CargoDelivered,
    FirstCargoDelivered,
    FirstVehicleRunning,
    VehicleAdvice,
}

/// Variante de aviso operativo de vehículo (deduplicación en sim).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VehicleAdviceKind {
    NoNetworkRoute,
    NoOrders,
    IncompatibleStop,
    WaitingForCargo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewsDisplayMode {
    Off,
    Summary,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NewsReference {
    None,
    Tile(TileCoord),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: u64,
    pub headline: String,
    pub body: Option<String>,
    pub news_type: NewsType,
    pub display: NewsDisplayMode,
    pub economy_tick: u64,
    pub calendar_day: u64,
    pub reference: NewsReference,
}

impl NewsItem {
    #[must_use]
    pub fn new(
        id: u64,
        headline: impl Into<String>,
        body: Option<String>,
        news_type: NewsType,
        display: NewsDisplayMode,
        tick: GameTick,
        reference: NewsReference,
    ) -> Self {
        Self {
            id,
            headline: headline.into(),
            body,
            news_type,
            display,
            economy_tick: tick.get(),
            calendar_day: calendar_day_index(tick),
            reference,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingNewsEvent {
    ItemAdded { id: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsQueue {
    pub items: VecDeque<NewsItem>,
    pub next_id: u64,
}

/// Días de calendario que conserva el historial antes de purgar (mensual).
pub const NEWS_MAX_AGE_DAYS: u64 = 730;

impl NewsQueue {
    pub const MAX_ITEMS: usize = 256;

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&NewsItem> {
        self.items.iter().find(|item| item.id == id)
    }
}

#[must_use]
pub fn tick_for_calendar_year(year: u32) -> GameTick {
    let years = u64::from(year.saturating_sub(CALENDAR_BASE_YEAR));
    GameTick::new(years * crate::economy::TICKS_PER_YEAR)
}

#[must_use]
pub fn calendar_day_index(tick: GameTick) -> u64 {
    tick.get() / u64::from(TICKS_PER_TRANSIT_DAY)
}

#[must_use]
pub fn calendar_year_day(day_index: u64) -> (u32, u64) {
    let years = day_index / CALENDAR_DAYS_PER_YEAR;
    let year = CALENDAR_BASE_YEAR.saturating_add(u32::try_from(years).unwrap_or(u32::MAX));
    let doy = day_index % CALENDAR_DAYS_PER_YEAR + 1;
    (year, doy)
}

#[must_use]
pub fn format_calendar_date(tick: GameTick) -> String {
    let (year, doy) = calendar_year_day(calendar_day_index(tick));
    let (day, month) = doy_to_month_day(doy);
    format!("{day} {month} {year}")
}

fn doy_to_month_day(doy: u64) -> (u64, &'static str) {
    const MONTHS: [(&str, u64); 12] = [
        ("ene", 31),
        ("feb", 28),
        ("mar", 31),
        ("abr", 30),
        ("may", 31),
        ("jun", 30),
        ("jul", 31),
        ("ago", 31),
        ("sep", 30),
        ("oct", 31),
        ("nov", 30),
        ("dic", 31),
    ];
    let mut remaining = doy;
    for (name, len) in MONTHS {
        if remaining <= len {
            return (remaining, name);
        }
        remaining -= len;
    }
    (31, "dic")
}

#[must_use]
pub fn default_display_for_type(news_type: NewsType) -> NewsDisplayMode {
    NewsDisplaySettings::openttd_defaults().display_for(news_type)
}

/// Preferencias Off / Summary / Full por categoría (equivalente a `news_display_settings.ini`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsDisplaySettings {
    pub cargo_delivered: NewsDisplayMode,
    pub first_cargo_delivered: NewsDisplayMode,
    pub first_vehicle_running: NewsDisplayMode,
    pub vehicle_advice: NewsDisplayMode,
}

impl Default for NewsDisplaySettings {
    fn default() -> Self {
        Self::openttd_defaults()
    }
}

impl NewsDisplaySettings {
    #[must_use]
    pub const fn openttd_defaults() -> Self {
        Self {
            cargo_delivered: NewsDisplayMode::Full,
            first_cargo_delivered: NewsDisplayMode::Full,
            first_vehicle_running: NewsDisplayMode::Full,
            vehicle_advice: NewsDisplayMode::Summary,
        }
    }

    #[must_use]
    pub const fn display_for(self, news_type: NewsType) -> NewsDisplayMode {
        match news_type {
            NewsType::CargoDelivered => self.cargo_delivered,
            NewsType::FirstCargoDelivered => self.first_cargo_delivered,
            NewsType::FirstVehicleRunning => self.first_vehicle_running,
            NewsType::VehicleAdvice => self.vehicle_advice,
        }
    }

    pub fn set_display(&mut self, news_type: NewsType, mode: NewsDisplayMode) {
        match news_type {
            NewsType::CargoDelivered => self.cargo_delivered = mode,
            NewsType::FirstCargoDelivered => self.first_cargo_delivered = mode,
            NewsType::FirstVehicleRunning => self.first_vehicle_running = mode,
            NewsType::VehicleAdvice => self.vehicle_advice = mode,
        }
    }
}

#[must_use]
pub fn news_type_label(news_type: NewsType) -> &'static str {
    match news_type {
        NewsType::CargoDelivered => "Entrega de carga",
        NewsType::FirstCargoDelivered => "Primera entrega",
        NewsType::FirstVehicleRunning => "Primer vehículo en marcha",
        NewsType::VehicleAdvice => "Avisos de vehículo",
    }
}

#[must_use]
pub fn news_display_mode_label(mode: NewsDisplayMode) -> &'static str {
    match mode {
        NewsDisplayMode::Off => "Off",
        NewsDisplayMode::Summary => "Summary",
        NewsDisplayMode::Full => "Full",
    }
}

#[must_use]
pub fn vehicle_kind_label(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::Bus => "autobús",
        VehicleKind::Truck => "camión",
        VehicleKind::Tram => "tranvía",
        VehicleKind::Train => "tren",
        VehicleKind::Ship => "barco",
        VehicleKind::Aircraft => "avión",
    }
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

fn vehicle_advice_kind(state: &crate::GameState, v: &Vehicle) -> Option<VehicleAdviceKind> {
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
    }
}

#[must_use]
pub fn cargo_display_name(cargo: CargoType) -> &'static str {
    cargo.display_name()
}

#[must_use]
pub fn format_money(amount: i64) -> String {
    let sign = if amount < 0 { "-" } else { "" };
    let abs = amount.unsigned_abs();
    if abs >= 1_000_000 {
        let whole = abs / 1_000_000;
        let frac = (abs % 1_000_000) / 100_000;
        format!("{sign}${whole}.{frac}M")
    } else if abs >= 10_000 {
        let whole = abs / 1_000;
        let frac = (abs % 1_000) / 100;
        format!("{sign}${whole}.{frac}K")
    } else {
        format!("{sign}${abs}")
    }
}

pub fn add_news_item(state: &mut crate::GameState, item: NewsItem) {
    let id = item.id;
    state.news.items.push_front(item);
    while state.news.items.len() > NewsQueue::MAX_ITEMS {
        state.news.items.pop_back();
    }
    state
        .pending_news_events
        .push(PendingNewsEvent::ItemAdded { id });
}

pub fn push_cargo_delivery_news(
    state: &mut crate::GameState,
    units: u32,
    cargo: CargoType,
    payment: i64,
    at: TileCoord,
    first_delivery: bool,
) {
    let cargo_name = cargo_display_name(cargo);
    let news_type = if first_delivery {
        NewsType::FirstCargoDelivered
    } else {
        NewsType::CargoDelivered
    };
    let headline = if first_delivery {
        format!("¡Primera entrega! {units} u. de {cargo_name}")
    } else {
        format!("Entrega de {units} u. de {cargo_name}")
    };
    let body = Some(format!(
        "Tu compañía ha cobrado {} por transportar {cargo_name}.",
        format_money(payment)
    ));
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        headline,
        body,
        news_type,
        default_display_for_type(news_type),
        state.tick,
        NewsReference::Tile(at),
    );
    if first_delivery {
        state
            .pending_sim_events
            .push(crate::sim_events::SimEvent::NewsApplause);
    }
    add_news_item(state, item);
}

pub fn push_first_vehicle_running_news(
    state: &mut crate::GameState,
    vehicle_id: u32,
    at: TileCoord,
    kind: VehicleKind,
) {
    let kind_label = vehicle_kind_label(kind);
    let headline = format!("¡Tu primer {kind_label} está en marcha!");
    let body = Some(format!("El vehículo {vehicle_id} ha salido a operar."));
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        headline,
        body,
        NewsType::FirstVehicleRunning,
        default_display_for_type(NewsType::FirstVehicleRunning),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
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

pub fn push_autoreplace_failed_news(
    state: &mut crate::GameState,
    vehicle_id: u32,
    err: crate::CommandError,
) {
    let headline = format!("Autoreemplazo falló (vehículo {vehicle_id})");
    let body = Some(crate::command_error_message(err).to_string());
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let at = state
        .vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .map_or(TileCoord::new(0, 0), |v| v.pos);
    let item = NewsItem::new(
        id,
        headline,
        body,
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

/// Elimina noticias más antiguas que [`NEWS_MAX_AGE_DAYS`].
pub fn purge_old_news_items(state: &mut crate::GameState) {
    let current_day = calendar_day_index(state.tick);
    state
        .news
        .items
        .retain(|item| current_day.saturating_sub(item.calendar_day) <= NEWS_MAX_AGE_DAYS);
}

/// Purga mensual (cada 30 días de calendario) al estilo `RemoveOldNewsItems`.
pub fn maybe_purge_old_news(state: &mut crate::GameState) {
    let day = calendar_day_index(state.tick);
    if day == state.news_last_purge_day {
        return;
    }
    if !day.is_multiple_of(30) {
        return;
    }
    state.news_last_purge_day = day;
    purge_old_news_items(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::vehicle::{Vehicle, VehicleKind};

    #[test]
    fn tick_for_calendar_year_offsets_from_base() {
        use crate::economy::TICKS_PER_YEAR;
        assert_eq!(tick_for_calendar_year(1950), GameTick::new(0));
        assert_eq!(tick_for_calendar_year(1960).get(), 10 * TICKS_PER_YEAR);
        assert_eq!(
            format_calendar_date(tick_for_calendar_year(1980)),
            "1 ene 1980"
        );
    }

    #[test]
    fn calendar_day_zero_is_first_jan_1950() {
        assert_eq!(format_calendar_date(GameTick::new(0)), "1 ene 1950");
    }

    #[test]
    fn add_news_queues_front_and_emits_event() {
        let mut state = GameState::new(4, 4);
        let item = NewsItem::new(
            1,
            "Test",
            None,
            NewsType::CargoDelivered,
            NewsDisplayMode::Full,
            state.tick,
            NewsReference::None,
        );
        add_news_item(&mut state, item);
        assert_eq!(state.news.items.len(), 1);
        assert_eq!(state.news.items[0].headline, "Test");
        assert_eq!(
            state.pending_news_events,
            vec![PendingNewsEvent::ItemAdded { id: 1 }]
        );
    }

    #[test]
    fn push_cargo_delivery_uses_full_display() {
        let mut state = GameState::new(4, 4);
        push_cargo_delivery_news(
            &mut state,
            12,
            CargoType::Coal,
            450,
            TileCoord::new(2, 2),
            true,
        );
        assert_eq!(state.news.items.len(), 1);
        let item = &state.news.items[0];
        assert_eq!(item.news_type, NewsType::FirstCargoDelivered);
        assert_eq!(item.display, NewsDisplayMode::Full);
        assert!(item.headline.contains("Primera entrega"));
    }

    #[test]
    fn push_first_vehicle_running_uses_full_display() {
        let mut state = GameState::new(4, 4);
        push_first_vehicle_running_news(&mut state, 1, TileCoord::new(1, 1), VehicleKind::Bus);
        assert_eq!(state.news.items.len(), 1);
        let item = &state.news.items[0];
        assert_eq!(item.news_type, NewsType::FirstVehicleRunning);
        assert_eq!(item.display, NewsDisplayMode::Full);
        assert!(item.headline.contains("autobús"));
    }

    #[test]
    fn news_display_settings_override_per_type() {
        let mut settings = NewsDisplaySettings::openttd_defaults();
        settings.vehicle_advice = NewsDisplayMode::Off;
        assert_eq!(
            settings.display_for(NewsType::VehicleAdvice),
            NewsDisplayMode::Off
        );
        assert_eq!(
            settings.display_for(NewsType::CargoDelivered),
            NewsDisplayMode::Full
        );
    }

    #[test]
    fn poll_vehicle_advice_fires_once_until_cleared() {
        let mut state = GameState::new(8, 8);
        state.vehicles.push(Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(2, 2),
            TileCoord::new(2, 2),
        ));
        state.vehicles[0].running = true;
        poll_vehicle_advice_news(&mut state);
        assert_eq!(state.news.items.len(), 1);
        assert_eq!(state.news.items[0].news_type, NewsType::VehicleAdvice);
        poll_vehicle_advice_news(&mut state);
        assert_eq!(state.news.items.len(), 1);
        state.vehicles[0].running = false;
        poll_vehicle_advice_news(&mut state);
        assert!(state.news_advice_sent.is_empty());
        state.vehicles[0].running = true;
        poll_vehicle_advice_news(&mut state);
        assert_eq!(state.news.items.len(), 2);
    }

    #[test]
    fn purge_old_news_items_drops_ancient_entries() {
        let mut state = GameState::new(4, 4);
        let day_ticks =
            |days: u64| GameTick::new(u64::from(crate::economy::TICKS_PER_TRANSIT_DAY) * days);
        state.tick = day_ticks(900);
        add_news_item(
            &mut state,
            NewsItem::new(
                1,
                "Antigua",
                None,
                NewsType::VehicleAdvice,
                NewsDisplayMode::Summary,
                day_ticks(100),
                NewsReference::None,
            ),
        );
        add_news_item(
            &mut state,
            NewsItem::new(
                2,
                "Reciente",
                None,
                NewsType::VehicleAdvice,
                NewsDisplayMode::Summary,
                day_ticks(880),
                NewsReference::None,
            ),
        );
        purge_old_news_items(&mut state);
        assert_eq!(state.news.items.len(), 1);
        assert_eq!(state.news.items[0].headline, "Reciente");
    }
}
