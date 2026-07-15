//! GameScript-lite (#43): goals, story y league en Rust (sin VM Squirrel).

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::game_state::{GameState, company_net_value};
use crate::news::{
    NewsDisplayMode, NewsItem, NewsReference, NewsType, add_news_item, calendar_day_index,
    calendar_year_day,
};

/// Objetivo de escenario (paridad conceptual con `Goal` de `OpenTTD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GsGoal {
    pub id: u32,
    pub title: String,
    pub progress_num: u64,
    pub progress_den: u64,
    pub completed: bool,
    pub kind: GsGoalKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GsGoalKind {
    CompanyValue {
        min: i64,
    },
    /// Progreso = entregas totales de la compañía activa (el cargo es etiquetado).
    CargoDelivered {
        cargo: CargoType,
        min: u64,
    },
    ReachYear {
        year: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GsStoryPage {
    pub id: u32,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GsState {
    pub enabled: bool,
    pub goals: Vec<GsGoal>,
    pub story_pages: Vec<GsStoryPage>,
    pub story_index: usize,
    pub all_complete: bool,
    /// Evita repetir la noticia de victoria GS.
    #[serde(default)]
    pub victory_news_sent: bool,
}

/// Fila de liga (compañías ordenadas por valor neto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsLeagueRow {
    pub company_id: u8,
    pub name: String,
    pub is_ai: bool,
    pub net_value: i64,
    pub performance: i32,
}

/// Siembra un escenario demo (2 story + 3 goals).
pub fn seed_gs_demo(state: &mut GameState) {
    state.gs = GsState {
        enabled: true,
        goals: vec![
            GsGoal {
                id: 1,
                title: "Alcanzá un valor de compañía de 120.000".into(),
                progress_num: 0,
                progress_den: 120_000,
                completed: false,
                kind: GsGoalKind::CompanyValue { min: 120_000 },
            },
            GsGoal {
                id: 2,
                title: "Entregá 5 cargas (cualquier tipo)".into(),
                progress_num: 0,
                progress_den: 5,
                completed: false,
                kind: GsGoalKind::CargoDelivered {
                    cargo: CargoType::Coal,
                    min: 5,
                },
            },
            GsGoal {
                id: 3,
                title: "Llegá al año 1952".into(),
                progress_num: 0,
                progress_den: 1952,
                completed: false,
                kind: GsGoalKind::ReachYear { year: 1952 },
            },
        ],
        story_pages: vec![
            GsStoryPage {
                id: 1,
                title: "Bienvenido".into(),
                body: "Este es un escenario GameScript-lite (#43). Cumplí los objetivos \
                       de la lista Goals. No hay runtime Squirrel: la lógica corre en Rust."
                    .into(),
            },
            GsStoryPage {
                id: 2,
                title: "Consejo".into(),
                body: "Construí una línea de freight, entregá carga y mirá Finanzas / League \
                       para comparar compañías. Story, Goals y League están en los menús Mundo y Economía."
                    .into(),
            },
        ],
        story_index: 0,
        all_complete: false,
        victory_news_sent: false,
    };
    refresh_gs_progress(state);
}

/// Actualiza progreso de goals; emite noticia al completar todos (sin forzar endscreen).
pub fn tick_gs(state: &mut GameState) {
    if !state.gs.enabled {
        return;
    }
    refresh_gs_progress(state);
    let all = !state.gs.goals.is_empty() && state.gs.goals.iter().all(|g| g.completed);
    state.gs.all_complete = all;
    if all && !state.gs.victory_news_sent {
        state.gs.victory_news_sent = true;
        push_gs_victory_news(state);
    }
}

fn refresh_gs_progress(state: &mut GameState) {
    let company = state
        .companies
        .get(state.active_company.index())
        .or_else(|| state.companies.first());
    let net = company.map_or(0, |c| company_net_value(c.economy.money, c.economy.loan));
    let deliveries = company.map_or(0, |c| c.cargo_deliveries);
    let (year, _) = calendar_year_day(calendar_day_index(state.tick));

    for goal in &mut state.gs.goals {
        let (num, den, done) = match goal.kind {
            GsGoalKind::CompanyValue { min } => {
                let den = u64::try_from(min.max(1)).unwrap_or(1);
                let num = u64::try_from(net.max(0)).unwrap_or(0).min(den);
                (num, den, net >= min)
            }
            GsGoalKind::CargoDelivered { min, .. } => {
                let den = min.max(1);
                (deliveries.min(den), den, deliveries >= min)
            }
            GsGoalKind::ReachYear { year: target } => {
                let den = u64::from(target.max(1));
                let num = u64::from(year).min(den);
                (num, den, year >= target)
            }
        };
        goal.progress_num = num;
        goal.progress_den = den;
        if done {
            goal.completed = true;
        }
    }
}

fn push_gs_victory_news(state: &mut GameState) {
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = NewsItem::new(
        id,
        "Objetivos del escenario cumplidos",
        Some("Completaste todos los goals del GameScript demo.".into()),
        NewsType::CompanyInfo,
        NewsDisplayMode::Full,
        state.tick,
        NewsReference::None,
    );
    add_news_item(state, item);
}

/// Tabla de liga: valor neto descendente.
#[must_use]
pub fn league_rows(state: &GameState) -> Vec<GsLeagueRow> {
    let mut rows: Vec<GsLeagueRow> = state
        .companies
        .iter()
        .map(|c| {
            let performance = c
                .quarterly_economy
                .samples
                .last()
                .map_or(0, |q| q.performance_history);
            GsLeagueRow {
                company_id: c.id.0,
                name: c.name.clone(),
                is_ai: c.is_ai,
                net_value: company_net_value(c.economy.money, c.economy.loan),
                performance,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.net_value
            .cmp(&a.net_value)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::news::tick_for_calendar_year;

    #[test]
    fn seed_and_tick_completes_year_goal() {
        let mut state = GameState::new(16, 16);
        seed_gs_demo(&mut state);
        assert!(state.gs.enabled);
        assert_eq!(state.gs.goals.len(), 3);
        assert_eq!(state.gs.story_pages.len(), 2);

        state.tick = tick_for_calendar_year(1952);
        if let Some(c) = state.companies.first_mut() {
            c.economy.money = 200_000;
            c.economy.loan = 0;
            c.cargo_deliveries = 10;
        }
        tick_gs(&mut state);
        assert!(state.gs.goals.iter().all(|g| g.completed));
        assert!(state.gs.all_complete);
        assert!(state.gs.victory_news_sent);
        assert!(!state.news.items.is_empty());
    }

    #[test]
    fn disabled_gs_is_noop() {
        let mut state = GameState::new(8, 8);
        assert!(!state.gs.enabled);
        tick_gs(&mut state);
        assert!(!state.gs.all_complete);
    }

    #[test]
    fn league_orders_by_net_value() {
        let mut state = GameState::new(8, 8);
        state.ensure_rival_transcargo();
        if let Some(player) = state.companies.iter_mut().find(|c| !c.is_ai) {
            player.economy.money = 50_000;
            player.economy.loan = 0;
        }
        if let Some(ai) = state.companies.iter_mut().find(|c| c.is_ai) {
            ai.economy.money = 90_000;
            ai.economy.loan = 0;
        }
        let rows = league_rows(&state);
        assert!(rows.len() >= 2);
        assert!(rows[0].net_value >= rows[1].net_value);
    }
}
