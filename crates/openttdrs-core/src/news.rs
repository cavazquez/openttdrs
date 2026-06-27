//! Cola de noticias al estilo `OpenTTD` (`AddNewsItem`, ticker / periódico).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::map::TileCoord;
use crate::tick::GameTick;

/// Año base del calendario mostrado en la barra (Y1 del sim = 1950).
pub const CALENDAR_BASE_YEAR: u32 = 1950;
pub const CALENDAR_DAYS_PER_YEAR: u64 = 365;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NewsType {
    CargoDelivered,
    FirstCargoDelivered,
    VehicleAdvice,
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

impl NewsQueue {
    pub const MAX_ITEMS: usize = 256;

    #[must_use]
    pub fn get(&self, id: u64) -> Option<&NewsItem> {
        self.items.iter().find(|item| item.id == id)
    }
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
    match news_type {
        NewsType::CargoDelivered | NewsType::FirstCargoDelivered => NewsDisplayMode::Full,
        NewsType::VehicleAdvice => NewsDisplayMode::Summary,
    }
}

#[must_use]
pub fn cargo_display_name(cargo: CargoType) -> &'static str {
    match cargo {
        CargoType::Passengers => "pasajeros",
        CargoType::Mail => "correo",
        CargoType::Goods => "mercancías",
        CargoType::Coal => "carbón",
        CargoType::Wood => "madera",
        CargoType::Oil => "petróleo",
    }
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
    add_news_item(state, item);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;

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
}
