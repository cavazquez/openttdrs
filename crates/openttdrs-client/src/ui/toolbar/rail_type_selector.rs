//! Selector de tipo de vía en el panel Rail (`GameState.current_rail_type`).

use bevy::prelude::*;
use openttdrs_core::RailType;

use crate::state::SimWorld;
use crate::ui::font::{UiFontRole, ui_text_font_loaded};
use crate::ui::toolbar::{BuildMenuUi, ToolbarTooltipTarget};

const BTN_BG: Color = Color::srgb(0.36, 0.47, 0.26);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);
const BTN_BORDER: Color = Color::srgb(0.55, 0.68, 0.4);
const BTN_TEXT: Color = Color::srgb(0.95, 0.96, 0.82);

const RAIL_TYPES: [RailType; 4] = [
    RailType::Rail,
    RailType::Electric,
    RailType::Monorail,
    RailType::Maglev,
];

/// Botón que fija el tipo de vía para construcción nueva.
#[derive(Component, Clone, Copy)]
pub(crate) struct RailTypeSelectButton(pub RailType);

pub(crate) fn spawn_rail_type_selector(
    buttons: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    for rt in RAIL_TYPES {
        let tip = match rt {
            RailType::Rail => "Tipo de vía: normal",
            RailType::Electric => "Tipo de vía: eléctrica (catenaria)",
            RailType::Monorail => "Tipo de vía: monorail",
            RailType::Maglev => "Tipo de vía: maglev",
        };
        buttons.spawn((
            Button,
            RailTypeSelectButton(rt),
            ToolbarTooltipTarget { text: tip },
            BuildMenuUi,
            Node {
                min_width: Val::Px(44.0),
                height: Val::Px(48.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            children![(
                Text::new(short_label(rt)),
                ui_text_font_loaded(asset_server, UiFontRole::Caption),
                TextColor(BTN_TEXT),
            )],
        ));
    }
}

fn short_label(rt: RailType) -> &'static str {
    match rt {
        RailType::Rail => "Norm",
        RailType::Electric => "Eléc",
        RailType::Monorail => "Mono",
        RailType::Maglev => "Mag",
    }
}

pub(crate) fn handle_rail_type_select_buttons(
    buttons: Query<(&Interaction, &RailTypeSelectButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        sim.state.current_rail_type = button.0;
    }
}

pub(crate) fn sync_rail_type_select_visuals(
    sim: Res<SimWorld>,
    mut buttons: Query<(&RailTypeSelectButton, &mut BackgroundColor), With<Button>>,
) {
    let current = sim.state.current_rail_type;
    for (button, mut bg) in &mut buttons {
        *bg = BackgroundColor(if button.0 == current {
            BTN_ACTIVE
        } else {
            BTN_BG
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn selecting_rail_type_updates_game_state() {
        let mut world = World::new();
        let mut sim = SimWorld::default();
        sim.state.current_rail_type = RailType::Rail;
        world.insert_resource(sim);
        world.spawn((
            Button,
            RailTypeSelectButton(RailType::Electric),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_rail_type_select_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<SimWorld>().state.current_rail_type,
            RailType::Electric
        );
    }
}
