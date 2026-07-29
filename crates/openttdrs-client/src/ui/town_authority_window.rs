//! Stub Town Authority (`WC_TOWN_AUTHORITY`) — hija de Town (#269).
//!
//! Enlaza ratings de autoridad del dominio (`Town::authority_rating` / #230).

use bevy::prelude::*;
use openttdrs_core::TownAction;
use openttdrs_core::format_money;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CREAM,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::town_window::TownWindowState;

#[derive(Resource, Default)]
pub(crate) struct TownAuthorityWindowState {
    pub(crate) open: bool,
    pub(crate) town_id: Option<u32>,
}

#[derive(Component)]
pub(crate) struct TownAuthorityBodyText;

pub(crate) fn setup_town_authority_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::TownAuthority,
        "Autoridad local",
        TITLE_CREAM,
        Vec2::new(80.0, 140.0),
        220.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            TownAuthorityBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
    });
}

pub(crate) fn open_town_authority_for(town_id: u32, state: &mut TownAuthorityWindowState) {
    state.open = true;
    state.town_id = Some(town_id);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_town_authority_window(
    state: Res<TownAuthorityWindowState>,
    town: Res<TownWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<TownAuthorityBodyText>>,
    mut body_q: Query<
        &mut Text,
        (
            With<TownAuthorityBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::TownAuthority)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let town_id = state.town_id.or(town.town_id);
    let Some(t) = town_id.and_then(|id| sim.state.towns.iter().find(|x| x.id == id)) else {
        if let Ok(mut body) = body_q.single_mut() {
            **body = "Sin pueblo seleccionado.".into();
        }
        return;
    };

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(tt, _)| tt.0 == FloatingWindowId::TownAuthority)
    {
        **title = format!("Autoridad — {}", t.name);
    }

    let rating = t.authority_rating(sim.state.active_company);
    let bribe = format_money(TownAction::Bribe.cost());
    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "Pueblo: {}\nRating compañía activa: {rating}\n\
             Acciones (dominio #230): Publicidad / Fondos / Soborno ({bribe}).\n\
             Stub UI — lista de acciones 15.3 residual.",
            t.name
        );
    }
}

pub(crate) fn town_authority_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<TownAuthorityWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::TownAuthority {
            state.open = false;
            state.town_id = None;
        }
        // Cascada: cerrar Town cierra Authority (#269 / matriz padre→hija).
        if msg.0.class == FloatingWindowId::Town {
            state.open = false;
            state.town_id = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn on_closed_clears_authority_state() {
        let mut world = World::new();
        world.init_resource::<TownAuthorityWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        {
            let mut st = world.resource_mut::<TownAuthorityWindowState>();
            st.open = true;
            st.town_id = Some(3);
        }
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::TownAuthority),
        ));
        world
            .run_system_once(town_authority_window_on_closed)
            .unwrap();
        let st = world.resource::<TownAuthorityWindowState>();
        assert!(!st.open);
        assert!(st.town_id.is_none());
    }

    #[test]
    fn parent_town_closed_closes_authority() {
        let mut world = World::new();
        world.init_resource::<TownAuthorityWindowState>();
        world.init_resource::<Messages<FloatingWindowClosed>>();
        world.resource_mut::<TownAuthorityWindowState>().open = true;
        world.write_message(FloatingWindowClosed(
            crate::ui::floating_window::WindowKey::singleton(FloatingWindowId::Town),
        ));
        world
            .run_system_once(town_authority_window_on_closed)
            .unwrap();
        assert!(!world.resource::<TownAuthorityWindowState>().open);
    }
}
