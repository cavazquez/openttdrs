//! Cola de noticias: tipos, configuración y funciones de gestión de items.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::map::TileCoord;
use crate::tick::GameTick;
use crate::vehicle::VehicleKind;

use super::calendar::{calendar_day_index, format_calendar_day_index};
use super::formatting::{cargo_display_name, format_money, vehicle_kind_label};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NewsType {
    CargoDelivered,
    FirstCargoDelivered,
    FirstVehicleRunning,
    VehicleAdvice,
    /// Accidente (choque de trenes, etc.).
    Accident,
    /// Información de compañía (compra, quiebra rival).
    CompanyInfo,
    /// Industria que anuncia cierre o acaba de cerrar (`NewsType::IndustryClose`).
    IndustryClose,
    /// Recesión económica (`NewsType::Economy` en `OpenTTD`).
    Economy,
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

    /// Etiqueta de fecha del ítem (usa `calendar_day`, no el tick crudo).
    #[must_use]
    pub fn date_label(&self) -> String {
        format_calendar_day_index(self.calendar_day)
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
    #[serde(default = "default_accident_display")]
    pub accident: NewsDisplayMode,
    #[serde(default = "default_company_info_display")]
    pub company_info: NewsDisplayMode,
    #[serde(default = "default_industry_close_display")]
    pub industry_close: NewsDisplayMode,
    #[serde(default = "default_economy_display")]
    pub economy: NewsDisplayMode,
}

const fn default_accident_display() -> NewsDisplayMode {
    NewsDisplayMode::Full
}

const fn default_company_info_display() -> NewsDisplayMode {
    NewsDisplayMode::Summary
}

const fn default_industry_close_display() -> NewsDisplayMode {
    NewsDisplayMode::Summary
}

const fn default_economy_display() -> NewsDisplayMode {
    NewsDisplayMode::Summary
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
            accident: NewsDisplayMode::Full,
            company_info: NewsDisplayMode::Summary,
            industry_close: NewsDisplayMode::Summary,
            economy: NewsDisplayMode::Summary,
        }
    }

    #[must_use]
    pub const fn display_for(self, news_type: NewsType) -> NewsDisplayMode {
        match news_type {
            NewsType::CargoDelivered => self.cargo_delivered,
            NewsType::FirstCargoDelivered => self.first_cargo_delivered,
            NewsType::FirstVehicleRunning => self.first_vehicle_running,
            NewsType::VehicleAdvice => self.vehicle_advice,
            NewsType::Accident => self.accident,
            NewsType::CompanyInfo => self.company_info,
            NewsType::IndustryClose => self.industry_close,
            NewsType::Economy => self.economy,
        }
    }

    pub fn set_display(&mut self, news_type: NewsType, mode: NewsDisplayMode) {
        match news_type {
            NewsType::CargoDelivered => self.cargo_delivered = mode,
            NewsType::FirstCargoDelivered => self.first_cargo_delivered = mode,
            NewsType::FirstVehicleRunning => self.first_vehicle_running = mode,
            NewsType::VehicleAdvice => self.vehicle_advice = mode,
            NewsType::Accident => self.accident = mode,
            NewsType::CompanyInfo => self.company_info = mode,
            NewsType::IndustryClose => self.industry_close = mode,
            NewsType::Economy => self.economy = mode,
        }
    }
}

pub fn add_news_item(state: &mut crate::GameState, item: NewsItem) {
    let id = item.id;
    state.news.items.push_front(item);
    while state.news.items.len() > NewsQueue::MAX_ITEMS {
        state.news.items.pop_back();
    }
    state
        .runtime
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
            .runtime
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

/// La industria anuncia que cerrará el mes que viene.
pub fn report_industry_closing(state: &mut crate::GameState, at: TileCoord) {
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Industria en ({}, {}) anuncia su cierre", at.x, at.y),
        Some("Dejará de producir y desaparecerá el mes que viene.".into()),
        NewsType::IndustryClose,
        default_display_for_type(NewsType::IndustryClose),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
}

/// La industria ya ha sido retirada del mapa.
pub fn report_industry_closed(state: &mut crate::GameState, at: TileCoord) {
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Industria cerrada en ({}, {})", at.x, at.y),
        None,
        NewsType::IndustryClose,
        default_display_for_type(NewsType::IndustryClose),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
}

/// Noticias de recesión (`STR_NEWS_BEGIN_OF_RECESSION` / `STR_NEWS_END_OF_RECESSION`).
pub fn push_economy_fluctuation_news(
    state: &mut crate::GameState,
    event: crate::economy::FluctuationEvent,
) {
    let (headline, body) = match event {
        crate::economy::FluctuationEvent::RecessionStart => (
            "Comienza una recesión económica",
            Some("La demanda de carga y la producción industrial se reducirán.".into()),
        ),
        crate::economy::FluctuationEvent::RecessionEnd => (
            "La recesión ha terminado",
            Some("La economía vuelve a la normalidad.".into()),
        ),
    };
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        headline,
        body,
        NewsType::Economy,
        default_display_for_type(NewsType::Economy),
        state.tick,
        NewsReference::None,
    );
    add_news_item(state, item);
}

/// Oferta de subsidio publicada.
pub fn push_subsidy_offer_news(
    state: &mut crate::GameState,
    cargo: CargoType,
    industry_pos: TileCoord,
    station_pos: TileCoord,
) {
    let cargo_name = cargo_display_name(cargo);
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Subvención: {cargo_name}"),
        Some(format!(
            "Transportar {cargo_name} desde ({}, {}) hacia la estación ({}, {}).",
            industry_pos.x, industry_pos.y, station_pos.x, station_pos.y
        )),
        NewsType::CompanyInfo,
        default_display_for_type(NewsType::CompanyInfo),
        state.tick,
        NewsReference::Tile(station_pos),
    );
    add_news_item(state, item);
}

/// Subsidio adjudicado a una compañía.
pub fn push_subsidy_awarded_news(
    state: &mut crate::GameState,
    cargo: CargoType,
    company_name: &str,
    station_pos: TileCoord,
) {
    let cargo_name = cargo_display_name(cargo);
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Subvención adjudicada: {cargo_name}"),
        Some(format!(
            "«{company_name}» se adjudica el transporte de {cargo_name} (pago ×2)."
        )),
        NewsType::CompanyInfo,
        default_display_for_type(NewsType::CompanyInfo),
        state.tick,
        NewsReference::Tile(station_pos),
    );
    add_news_item(state, item);
}

/// Desastre ambiental / accidente.
pub fn push_disaster_news(
    state: &mut crate::GameState,
    kind: crate::sim_events::DisasterKind,
    at: TileCoord,
) {
    let (headline, body) = disaster_copy(kind, at);
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        headline,
        Some(body),
        NewsType::Accident,
        default_display_for_type(NewsType::Accident),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
}

fn disaster_copy(kind: crate::sim_events::DisasterKind, at: TileCoord) -> (String, String) {
    use crate::sim_events::DisasterKind;
    let where_ = format!("({}, {})", at.x, at.y);
    match kind {
        DisasterKind::SmallUfo => (
            "OVNI pequeño avistado".into(),
            format!("Un OVNI pequeño se aproxima a {where_}."),
        ),
        DisasterKind::BigUfo => (
            "OVNI enorme avistado".into(),
            format!("Un OVNI enorme se aproxima a {where_}."),
        ),
        DisasterKind::Airplane => (
            "Accidente aéreo".into(),
            format!("Un avión se estrella cerca de {where_}."),
        ),
        DisasterKind::Helicopter => (
            "Accidente de helicóptero".into(),
            format!("Un helicóptero se estrella en {where_}."),
        ),
        DisasterKind::Submarine => (
            "Submarino a la deriva".into(),
            format!("Un submarino provoca daños en {where_}."),
        ),
        DisasterKind::CoalMineSubsidence => (
            "Hundimiento minero".into(),
            format!("Un hundimiento en mina afecta {where_}."),
        ),
    }
}

/// Logro / goal de escenario cumplido por una compañía rival (#180).
pub fn push_rival_achievement_news(
    state: &mut crate::GameState,
    company_name: &str,
    goal_title: &str,
) {
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Logro rival: {company_name}"),
        Some(format!(
            "«{company_name}» cumplió el objetivo: {goal_title}"
        )),
        NewsType::CompanyInfo,
        default_display_for_type(NewsType::CompanyInfo),
        state.tick,
        NewsReference::None,
    );
    add_news_item(state, item);
}

/// Aviso de quiebra (jugador o rival).
pub fn push_bankruptcy_news(
    state: &mut crate::GameState,
    company_name: &str,
    month: u8,
    limit: u8,
) {
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        format!("Quiebra: {company_name}"),
        Some(format!(
            "La compañía «{company_name}» está en quiebra (mes {month}/{limit})."
        )),
        NewsType::CompanyInfo,
        default_display_for_type(NewsType::CompanyInfo),
        state.tick,
        NewsReference::None,
    );
    add_news_item(state, item);
}

pub fn push_autoreplace_failed_news(
    state: &mut crate::GameState,
    vehicle_id: u32,
    err: crate::CommandError,
) {
    let headline = format!("Autoreemplazo falló (vehículo {vehicle_id})");
    let body = Some(err.to_string());
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
    if day == state.runtime.news_last_purge_day {
        return;
    }
    if !day.is_multiple_of(30) {
        return;
    }
    state.runtime.news_last_purge_day = day;
    purge_old_news_items(state);
}
