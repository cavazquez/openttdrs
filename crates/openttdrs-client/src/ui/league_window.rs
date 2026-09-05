//! Tabla de liga / ranking de compañías (#43).

use bevy::prelude::*;
use openttdrs_core::{GsLeagueRow, format_money, league_rows};

use crate::i18n::{Locale, localized_text};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, spawn_floating_window,
    window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::list_window::{
    LIST_DEFAULT_HEIGHT, clear_list_children, spawn_list_empty_label, spawn_list_row_button,
    spawn_list_scroll_area,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct LeagueWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct LeagueListRoot;

#[derive(Component)]
struct LeagueListRow;

#[derive(Default)]
pub(crate) struct LeagueCache {
    fingerprint: u64,
    locale: Option<Locale>,
}

impl LeagueCache {
    fn needs_refresh(&self, fingerprint: u64, locale: Locale) -> bool {
        self.fingerprint != fingerprint || self.locale != Some(locale)
    }

    fn record(&mut self, fingerprint: u64, locale: Locale) {
        self.fingerprint = fingerprint;
        self.locale = Some(locale);
    }

    fn reset(&mut self) {
        self.fingerprint = 0;
        self.locale = None;
    }
}

fn league_row_text(locale: Locale, rank: usize, row: &GsLeagueRow) -> String {
    let kind = if row.is_ai {
        "IA"
    } else {
        match locale {
            Locale::Es => "Humana",
            Locale::En => "Human",
        }
    };
    let performance = match locale {
        Locale::Es => "rend.",
        Locale::En => "perf",
    };
    format!(
        "{rank}. {} ({kind})  {}  {performance} {}",
        row.name,
        format_money(row.net_value),
        row.performance
    )
}

pub(crate) fn setup_league_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::League,
        "Liga",
        TITLE_CREAM,
        Vec2::new(440.0, 110.0),
        460.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Compañías ordenadas por valor neto · performance trimestral"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        spawn_list_scroll_area(body, asset_server, LeagueListRoot, LIST_DEFAULT_HEIGHT);
    });
}

pub(crate) fn open_league_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<LeagueWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::League {
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_league_window(
    state: Res<LeagueWindowState>,
    sim: Option<Res<SimWorld>>,
    prefs: Res<ClientPreferences>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<LeagueListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<LeagueCache>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::League {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        cache.reset();
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    let rows = league_rows(&sim.state);
    let fingerprint = rows.iter().fold(rows.len() as u64, |acc, r| {
        acc.wrapping_mul(31)
            .wrapping_add(u64::from(r.company_id))
            .wrapping_add(r.net_value as u64)
            .wrapping_add(r.performance as u64)
    });
    let locale = prefs.locale();
    if !cache.needs_refresh(fingerprint, locale) {
        return;
    }
    cache.record(fingerprint, locale);
    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            spawn_list_empty_label(
                list,
                &asset_server,
                &localized_text(locale, "Sin compañías"),
            );
            return;
        }
        for (rank, row) in rows.iter().enumerate() {
            spawn_list_row_button(
                list,
                &asset_server,
                league_row_text(locale, rank + 1, row),
                LeagueListRow,
                rank == 0,
            );
        }
    });
}

pub(crate) fn league_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<LeagueWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::League {
            state.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LeagueCache, league_row_text};
    use crate::i18n::Locale;
    use openttdrs_core::GsLeagueRow;

    #[test]
    fn league_rows_use_the_live_locale_without_translating_company_names() {
        let row = GsLeagueRow {
            company_id: 7,
            name: "Compañía Ñandú".into(),
            is_ai: false,
            net_value: 12_345,
            performance: 87,
        };
        assert_eq!(
            league_row_text(Locale::En, 1, &row),
            "1. Compañía Ñandú (Human)  $12.3K  perf 87"
        );
        assert_eq!(
            league_row_text(Locale::Es, 1, &row),
            "1. Compañía Ñandú (Humana)  $12.3K  rend. 87"
        );
    }

    #[test]
    fn league_cache_invalidates_when_only_the_locale_changes() {
        let mut cache = LeagueCache {
            fingerprint: 99,
            locale: Some(Locale::Es),
        };
        assert!(!cache.needs_refresh(99, Locale::Es));
        assert!(cache.needs_refresh(99, Locale::En));
        assert!(cache.needs_refresh(100, Locale::Es));
        cache.record(99, Locale::En);
        assert!(!cache.needs_refresh(99, Locale::En));
        cache.reset();
        assert!(cache.needs_refresh(0, Locale::Es));
    }
}
