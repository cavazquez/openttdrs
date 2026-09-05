//! Ventana formal de cheats (#45), paridad ligera con `CheatWindow` de OpenTTD.
//! Mutaciones solo vía `Command::Cheat*`. Abrir: Ctrl+Alt+C / Ajustes → Trucos…

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{calendar_day_index, calendar_year_day};

use crate::i18n::{Locale, localized_calendar_date, localized_text};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ON: Color = Color::srgb(0.28, 0.42, 0.28);

#[derive(Resource, Default)]
pub(crate) struct CheatWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct CheatWindowStatusText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheatWindowAction {
    ToggleEnabled,
    AddMoney,
    ToggleInfinite,
    ToggleBulldozer,
    YearMinus,
    YearPlus,
    CycleCompany,
}

pub(crate) fn setup_cheat_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::CheatWindow,
        "Trucos",
        TITLE_BROWN,
        Vec2::new(280.0, 140.0),
        360.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Singleplayer · Ctrl+Alt+C · consola: cheat …"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            CheatWindowStatusText,
            Text::new("—"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_cheat_btn(
                row,
                asset_server,
                CheatWindowAction::ToggleEnabled,
                "ON/OFF",
            );
            spawn_cheat_btn(row, asset_server, CheatWindowAction::AddMoney, "+$1M");
            spawn_cheat_btn(row, asset_server, CheatWindowAction::ToggleInfinite, "∞$");
            spawn_cheat_btn(
                row,
                asset_server,
                CheatWindowAction::ToggleBulldozer,
                "Bulldozer",
            );
            spawn_cheat_btn(row, asset_server, CheatWindowAction::YearMinus, "Año−");
            spawn_cheat_btn(row, asset_server, CheatWindowAction::YearPlus, "Año+");
            spawn_cheat_btn(
                row,
                asset_server,
                CheatWindowAction::CycleCompany,
                "Compañía",
            );
        });
    });
}

fn spawn_cheat_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: CheatWindowAction,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(72.0),
                height: Val::Px(26.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

pub(crate) fn sync_cheat_window(
    state: Res<CheatWindowState>,
    sim: Option<Res<SimWorld>>,
    prefs: Res<ClientPreferences>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut status_q: Query<&mut Text, With<CheatWindowStatusText>>,
    mut buttons: Query<(&CheatWindowAction, &mut BackgroundColor), With<Button>>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::CheatWindow {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    let status = format_cheat_status(&sim.state, prefs.locale());
    for mut text in &mut status_q {
        **text = status.clone();
    }
    let c = &sim.state.cheats;
    for (action, mut bg) in &mut buttons {
        *bg = BackgroundColor(match *action {
            CheatWindowAction::ToggleEnabled if c.enabled => BTN_ON,
            CheatWindowAction::ToggleInfinite if c.infinite_money_active() => BTN_ON,
            CheatWindowAction::ToggleBulldozer if c.magic_bulldozer_active() => BTN_ON,
            _ => BTN_BG,
        });
    }
}

pub(crate) fn handle_cheat_window_buttons(
    state: Res<CheatWindowState>,
    mut sim: Option<ResMut<SimWorld>>,
    buttons: Query<
        (&Interaction, &CheatWindowAction),
        (Changed<Interaction>, With<Button>, With<CheatWindowAction>),
    >,
) {
    if !state.open {
        return;
    }
    let Some(sim) = sim.as_deref_mut() else {
        return;
    };
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let cmd = match *action {
            CheatWindowAction::ToggleEnabled => Command::CheatSetEnabled(!sim.state.cheats.enabled),
            CheatWindowAction::AddMoney => Command::CheatAddMoney(1_000_000),
            CheatWindowAction::ToggleInfinite => Command::CheatToggleInfiniteMoney,
            CheatWindowAction::ToggleBulldozer => Command::CheatToggleMagicBulldozer,
            CheatWindowAction::YearMinus | CheatWindowAction::YearPlus => {
                let (year, _) = calendar_year_day(calendar_day_index(sim.state.tick));
                let next = if *action == CheatWindowAction::YearMinus {
                    year.saturating_sub(1)
                } else {
                    year.saturating_add(1)
                };
                Command::CheatSetYear(next)
            }
            CheatWindowAction::CycleCompany => {
                let n = sim.state.companies.len().max(1);
                let cur = usize::from(sim.state.active_company.0);
                let next = CompanyId(((cur + 1) % n) as u8);
                Command::CheatSwitchCompany(next)
            }
        };
        let _ = crate::network::apply_player_command(&mut sim.state, &cmd);
    }
}

pub(crate) fn handle_cheat_window_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CheatWindowState>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
) {
    if console.as_deref().is_some_and(|c| c.open) {
        return;
    }
    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if ctrl && alt && keyboard.just_pressed(KeyCode::KeyC) {
        state.open = !state.open;
    }
}

pub(crate) fn cheat_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<CheatWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::CheatWindow {
            state.open = false;
        }
    }
}

fn format_cheat_status(state: &openttdrs_core::GameState, locale: Locale) -> String {
    let c = &state.cheats;
    let date = localized_calendar_date(locale, state.tick);
    let company = state
        .companies
        .iter()
        .find(|co| co.id == state.active_company)
        .map(|co| co.name.as_str())
        .unwrap_or("?");
    let bool_label = |value| localized_text(locale, if value { "sí" } else { "no" });
    format!(
        "{}={} ∞$={} {}={}\n{}={} · {} · {} {} ({})",
        localized_text(locale, "activado"),
        bool_label(c.enabled),
        bool_label(c.infinite_money),
        localized_text(locale, "bulldozer"),
        bool_label(c.magic_bulldozer),
        localized_text(locale, "dinero"),
        state.economy.money,
        date,
        localized_text(locale, "compañía"),
        state.active_company.0,
        company
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::prelude::*;

    use super::{CheatWindowState, CheatWindowStatusText, sync_cheat_window};
    use crate::settings::ClientPreferences;
    use crate::state::SimWorld;

    #[test]
    fn cheat_status_follows_the_live_locale() {
        let mut world = World::new();
        world.insert_resource(CheatWindowState { open: true });
        world.insert_resource(SimWorld::default());
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        let status = world.spawn((CheatWindowStatusText, Text::new("—"))).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(sync_cheat_window);
        schedule.run(&mut world);
        let english = world.entity(status).get::<Text>().unwrap().as_str();
        assert!(english.starts_with("enabled="));
        assert!(english.contains(" Jan "));

        world.resource_mut::<ClientPreferences>().language = "es-AR".into();
        schedule.run(&mut world);
        let spanish = world.entity(status).get::<Text>().unwrap().as_str();
        assert!(spanish.starts_with("activado="));
        assert!(spanish.contains(" ene "));
    }
}
