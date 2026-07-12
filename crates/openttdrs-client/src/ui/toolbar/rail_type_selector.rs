//! Selector de tipo de vía en el panel Rail (`GameState.current_rail_type`).
//!
//! Al cambiar el tipo, actualiza los iconos del toolbar como OpenTTD
//! (`BuildRailToolbarWindow::OnInit` / `gui_sprites`).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::RailType;

use crate::state::SimWorld;
use crate::ui::font::{UiFontRole, ui_text_font_loaded};
use crate::ui::toolbar::{BuildMenuAction, BuildMenuUi, ToolSelectButton, ToolbarTooltipTarget};

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

/// Icono tipado del toolbar (paridad `RailTypeInfo::gui_sprites`).
#[must_use]
pub(crate) fn rail_toolbar_icon_path(
    rt: RailType,
    action: BuildMenuAction,
) -> Option<&'static str> {
    let slot = match action {
        BuildMenuAction::RailVert => "rail_ns",
        BuildMenuAction::RailX => "rail_x",
        BuildMenuAction::RailHorz => "rail_ew",
        BuildMenuAction::RailY => "rail_y",
        BuildMenuAction::Rail => "autorail",
        BuildMenuAction::RailDepot => "depot",
        BuildMenuAction::RailTunnel => "tunnel",
        BuildMenuAction::RailConvert => "convert",
        _ => return None,
    };
    Some(match (rt, slot) {
        (RailType::Rail, "rail_ns") => "assets/opengfx/tiles/toolbar_rail_rail_ns.png",
        (RailType::Rail, "rail_x") => "assets/opengfx/tiles/toolbar_rail_rail_x.png",
        (RailType::Rail, "rail_ew") => "assets/opengfx/tiles/toolbar_rail_rail_ew.png",
        (RailType::Rail, "rail_y") => "assets/opengfx/tiles/toolbar_rail_rail_y.png",
        (RailType::Rail, "autorail") => "assets/opengfx/tiles/toolbar_rail_autorail.png",
        (RailType::Rail, "depot") => "assets/opengfx/tiles/toolbar_rail_depot.png",
        (RailType::Rail, "tunnel") => "assets/opengfx/tiles/toolbar_rail_tunnel.png",
        (RailType::Rail, "convert") => "assets/opengfx/tiles/toolbar_rail_convert.png",

        (RailType::Electric, "rail_ns") => "assets/opengfx/tiles/toolbar_rail_electric_rail_ns.png",
        (RailType::Electric, "rail_x") => "assets/opengfx/tiles/toolbar_rail_electric_rail_x.png",
        (RailType::Electric, "rail_ew") => "assets/opengfx/tiles/toolbar_rail_electric_rail_ew.png",
        (RailType::Electric, "rail_y") => "assets/opengfx/tiles/toolbar_rail_electric_rail_y.png",
        (RailType::Electric, "autorail") => {
            "assets/opengfx/tiles/toolbar_rail_electric_autorail.png"
        }
        (RailType::Electric, "depot") => "assets/opengfx/tiles/toolbar_rail_electric_depot.png",
        (RailType::Electric, "tunnel") => "assets/opengfx/tiles/toolbar_rail_electric_tunnel.png",
        (RailType::Electric, "convert") => "assets/opengfx/tiles/toolbar_rail_electric_convert.png",

        (RailType::Monorail, "rail_ns") => "assets/opengfx/tiles/toolbar_rail_mono_rail_ns.png",
        (RailType::Monorail, "rail_x") => "assets/opengfx/tiles/toolbar_rail_mono_rail_x.png",
        (RailType::Monorail, "rail_ew") => "assets/opengfx/tiles/toolbar_rail_mono_rail_ew.png",
        (RailType::Monorail, "rail_y") => "assets/opengfx/tiles/toolbar_rail_mono_rail_y.png",
        (RailType::Monorail, "autorail") => "assets/opengfx/tiles/toolbar_rail_mono_autorail.png",
        (RailType::Monorail, "depot") => "assets/opengfx/tiles/toolbar_rail_mono_depot.png",
        (RailType::Monorail, "tunnel") => "assets/opengfx/tiles/toolbar_rail_mono_tunnel.png",
        (RailType::Monorail, "convert") => "assets/opengfx/tiles/toolbar_rail_mono_convert.png",

        (RailType::Maglev, "rail_ns") => "assets/opengfx/tiles/toolbar_rail_maglev_rail_ns.png",
        (RailType::Maglev, "rail_x") => "assets/opengfx/tiles/toolbar_rail_maglev_rail_x.png",
        (RailType::Maglev, "rail_ew") => "assets/opengfx/tiles/toolbar_rail_maglev_rail_ew.png",
        (RailType::Maglev, "rail_y") => "assets/opengfx/tiles/toolbar_rail_maglev_rail_y.png",
        (RailType::Maglev, "autorail") => "assets/opengfx/tiles/toolbar_rail_maglev_autorail.png",
        (RailType::Maglev, "depot") => "assets/opengfx/tiles/toolbar_rail_maglev_depot.png",
        (RailType::Maglev, "tunnel") => "assets/opengfx/tiles/toolbar_rail_maglev_tunnel.png",
        (RailType::Maglev, "convert") => "assets/opengfx/tiles/toolbar_rail_maglev_convert.png",

        _ => return None,
    })
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

/// Cambia los iconos de construcción según `current_rail_type` (como OpenTTD).
pub(crate) fn sync_rail_toolbar_icons(
    sim: Res<SimWorld>,
    asset_server: Res<AssetServer>,
    mut last: Local<Option<RailType>>,
    buttons: Query<(&BuildMenuAction, &Children), With<ToolSelectButton>>,
    mut icons: Query<&mut ImageNode>,
) {
    let rt = sim.state.current_rail_type;
    if *last == Some(rt) {
        return;
    }
    *last = Some(rt);
    for (action, children) in &buttons {
        let Some(path) = rail_toolbar_icon_path(rt, *action) else {
            continue;
        };
        for child in children.iter() {
            let Ok(mut node) = icons.get_mut(child) else {
                continue;
            };
            node.image = asset_server.load(path);
        }
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

    #[test]
    fn toolbar_icons_differ_by_rail_type() {
        let ns_rail = rail_toolbar_icon_path(RailType::Rail, BuildMenuAction::RailVert).unwrap();
        let ns_el = rail_toolbar_icon_path(RailType::Electric, BuildMenuAction::RailVert).unwrap();
        let ns_mono =
            rail_toolbar_icon_path(RailType::Monorail, BuildMenuAction::RailVert).unwrap();
        let ns_mag = rail_toolbar_icon_path(RailType::Maglev, BuildMenuAction::RailVert).unwrap();
        assert_ne!(ns_rail, ns_el);
        assert_ne!(ns_rail, ns_mono);
        assert_ne!(ns_rail, ns_mag);
        assert!(ns_el.contains("electric"));
        assert!(ns_mono.contains("mono"));
        assert!(ns_mag.contains("maglev"));
    }
}
