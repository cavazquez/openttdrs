//! Cola de noticias al estilo `OpenTTD` (`AddNewsItem`, ticker / periódico).

mod calendar;
mod formatting;
mod queue;
mod vehicle_advice;

pub use calendar::{
    CALENDAR_BASE_YEAR, CALENDAR_DAYS_PER_YEAR, OPENTTD_CALENDAR_BASE_DATE, calendar_day_index,
    calendar_day_index_at_start_of_year, calendar_day_index_from_openttd_date,
    calendar_day_index_from_state, calendar_day_of_month_from_day_index, calendar_days_in_year,
    calendar_month_from_day_index, calendar_month_name, calendar_year_day,
    calendar_ymd_from_day_index, format_calendar_date, format_calendar_date_from_state,
    format_calendar_day_index, is_calendar_leap_year, openttd_date_at_start_of_year,
    openttd_date_from_calendar_day_index, tick_for_calendar_year,
};
pub use formatting::{
    cargo_display_name, format_money, news_display_mode_label, news_type_label, vehicle_kind_label,
};
pub use queue::{
    NEWS_MAX_AGE_DAYS, NewsDisplayMode, NewsDisplaySettings, NewsItem, NewsQueue, NewsReference,
    NewsType, PendingNewsEvent, add_news_item, default_display_for_type, maybe_purge_old_news,
    purge_old_news_items, push_autoreplace_failed_news, push_bankruptcy_news,
    push_cargo_delivery_news, push_disaster_news, push_economy_fluctuation_news,
    push_first_vehicle_running_news, push_new_vehicle_available_news, push_rival_achievement_news,
    push_subsidy_awarded_news, push_subsidy_offer_news, report_industry_closed,
    report_industry_closing, report_industry_opened,
};
pub use vehicle_advice::{VehicleAdviceKind, poll_vehicle_advice_news, push_vehicle_advice_news};

/// Detecta motores introducidos desde el último arranque y publica su noticia.
///
/// La marca es efímera a propósito: al rehidratar un save se consideran ya
/// anunciados todos los motores cuya fecha quedó atrás, evitando repetir en
/// bloque la historia de una partida cargada. Los motores que llegan después
/// por un stack `NewGRF` sí se anuncian una vez si todavía son contemporáneos.
pub fn poll_new_vehicle_news(state: &mut crate::GameState) {
    let year = state.calendar.year;
    if !state.runtime.engine_available_news_initialized {
        state.runtime.engine_available_news_sent.extend(
            state
                .engine_catalog
                .iter()
                .filter_map(|engine| (u32::from(engine.intro_year) <= year).then_some(engine.id)),
        );
        state.runtime.engine_available_news_initialized = true;
        return;
    }

    let mut pending = Vec::new();
    for engine in &state.engine_catalog {
        if u32::from(engine.intro_year) > year
            || state
                .runtime
                .engine_available_news_sent
                .contains(&engine.id)
        {
            continue;
        }
        state.runtime.engine_available_news_sent.insert(engine.id);
        if crate::engine::engine_available_in_year(engine, year) && !engine.no_news() {
            pending.push((engine.id, engine.kind, engine.name.clone()));
        }
    }

    for (engine_id, kind, name) in pending {
        push_new_vehicle_available_news(state, engine_id, kind, &name);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::cargo::CargoType;
    use crate::map::TileCoord;
    use crate::vehicle::{Vehicle, VehicleKind};

    #[test]
    fn tick_for_calendar_year_offsets_from_base() {
        use crate::tick::GameTick;
        assert_eq!(tick_for_calendar_year(1950), GameTick::new(0));
        assert_eq!(tick_for_calendar_year(1960).get(), 3_652 * 74);
        assert_eq!(
            format_calendar_date(tick_for_calendar_year(1980)),
            "1 ene 1980"
        );
    }

    #[test]
    fn calendar_day_zero_is_first_jan_1950() {
        use crate::tick::GameTick;
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
            state.runtime.pending_news_events,
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
    fn new_vehicle_news_respects_no_news_and_is_emitted_once() {
        let mut state = GameState::new(8, 8);
        let mut announced =
            crate::engine::engine_for_vehicle(VehicleKind::Bus, crate::engine::ENGINE_BUS_MPS)
                .clone();
        announced.id = 20_101;
        announced.name = "Bus anunciado".into();
        announced.intro_year = 1951;
        let mut silent = announced.clone();
        silent.id = 20_102;
        silent.name = "Bus silencioso".into();
        silent.extra_flags = crate::engine::EXTRA_ENGINE_FLAG_NO_NEWS;
        state.engine_catalog = vec![announced, silent];

        state.tick = tick_for_calendar_year(1950);
        state.sync_timers_from_tick();
        poll_new_vehicle_news(&mut state);
        state.tick = tick_for_calendar_year(1951);
        state.sync_timers_from_tick();
        poll_new_vehicle_news(&mut state);
        poll_new_vehicle_news(&mut state);

        assert_eq!(state.news.items.len(), 1);
        let item = &state.news.items[0];
        assert_eq!(item.news_type, NewsType::NewVehicles);
        assert!(item.headline.contains("Bus anunciado"));
        assert!(state.runtime.engine_available_news_sent.contains(&20_102));
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
        assert!(state.runtime.news_advice_sent.is_empty());
        state.vehicles[0].running = true;
        poll_vehicle_advice_news(&mut state);
        assert_eq!(state.news.items.len(), 2);
    }

    #[test]
    fn purge_old_news_items_drops_ancient_entries() {
        use crate::tick::GameTick;
        let mut state = GameState::new(4, 4);
        let day_ticks = |days: u64| GameTick::new(u64::from(crate::economy::TICKS_PER_DAY) * days);
        state.tick = day_ticks(900);
        state.sync_timers_from_tick();
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

    #[test]
    fn disaster_news_date_matches_state_tick_year() {
        use crate::disaster::force_disaster;
        use crate::map::TileCoord;
        use crate::sim_events::DisasterKind;
        use crate::tick::GameTick;
        let mut state = GameState::new(8, 8);
        state.tick = tick_for_calendar_year(1980);
        // Avanzar ~91 días → ~2 abr 1980.
        state.tick = GameTick::new(
            state
                .tick
                .get()
                .saturating_add(u64::from(crate::economy::TICKS_PER_DAY) * 91),
        );
        force_disaster(&mut state, DisasterKind::BigUfo, TileCoord::new(3, 3));
        let item = state.news.items.back().expect("noticia OVNI");
        assert_eq!(item.date_label(), format_calendar_date(state.tick));
        assert!(
            item.date_label().contains("1980"),
            "popup no debe quedar anclado a 1950: {}",
            item.date_label()
        );
    }
}
