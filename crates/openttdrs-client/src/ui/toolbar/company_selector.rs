//! Selector de compañía activa (toolbar).

use bevy::prelude::*;
use openttdrs_core::CompanyId;

use crate::state::SimWorld;
use crate::ui::toolbar::{BuildMenuUi, ToolbarTooltipTarget};

const BTN_BG: Color = Color::srgb(0.36, 0.47, 0.26);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);
const BTN_BORDER: Color = Color::srgb(0.55, 0.68, 0.4);
const BTN_TEXT: Color = Color::srgb(0.08, 0.07, 0.05);

/// Fila de chips de compañía (rebuild dinámico).
#[derive(Component)]
pub(crate) struct CompanySelectorRow;

/// Chip que activa una compañía.
#[derive(Component, Clone, Copy)]
pub(crate) struct CompanySelectorButton(pub CompanyId);

pub(crate) fn spawn_company_selector(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                align_items: AlignItems::FlexStart,
                align_self: AlignSelf::Center,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|col| {
            col.spawn((
                Text::new("Compañía"),
                TextFont {
                    font_size: FontSize::Rem(0.65),
                    ..default()
                },
                TextColor(BTN_TEXT),
                BuildMenuUi,
            ));
            col.spawn((
                CompanySelectorRow,
                BuildMenuUi,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    flex_wrap: FlexWrap::Wrap,
                    max_width: Val::Px(160.0),
                    ..default()
                },
            ));
        });
}

pub(crate) fn handle_company_selector_buttons(
    buttons: Query<(&Interaction, &CompanySelectorButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let _ = sim.state.set_active_company(button.0);
    }
}

pub(crate) fn sync_company_selector(
    sim: Res<SimWorld>,
    row: Query<Entity, With<CompanySelectorRow>>,
    existing: Query<(Entity, &CompanySelectorButton)>,
    mut styles: Query<(&CompanySelectorButton, &Interaction, &mut BackgroundColor), With<Button>>,
    mut commands: Commands,
) {
    let want: Vec<(CompanyId, String, bool)> = sim
        .state
        .companies
        .iter()
        .map(|c| {
            let mut label = c.name.clone();
            if c.id == sim.state.active_company {
                label.push('*');
            }
            if c.is_ai {
                label.push_str(" (IA)");
            }
            // Etiqueta corta para el chip.
            let short = if label.chars().count() > 10 {
                format!("{}…", label.chars().take(8).collect::<String>())
            } else {
                label
            };
            (c.id, short, c.id == sim.state.active_company)
        })
        .collect();
    let have: Vec<CompanyId> = existing.iter().map(|(_, b)| b.0).collect();
    let want_ids: Vec<CompanyId> = want.iter().map(|(id, _, _)| *id).collect();
    if want_ids != have {
        for (entity, _) in &existing {
            commands.entity(entity).despawn();
        }
        if let Ok(row_entity) = row.single() {
            commands.entity(row_entity).with_children(|row| {
                for (id, label, _) in &want {
                    row.spawn((
                        Button,
                        CompanySelectorButton(*id),
                        BuildMenuUi,
                        ToolbarTooltipTarget {
                            text: "Compañía activa (comandos / HUD)",
                        },
                        Node {
                            min_width: Val::Px(48.0),
                            height: Val::Px(22.0),
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
                            Text::new(label.clone()),
                            TextFont {
                                font_size: FontSize::Rem(0.6),
                                ..default()
                            },
                            TextColor(BTN_TEXT),
                        )],
                    ));
                }
            });
        }
    }
    for (button, interaction, mut bg) in &mut styles {
        let selected = button.0 == sim.state.active_company;
        *bg = BackgroundColor(if selected {
            BTN_ACTIVE
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.5, 0.62, 0.38)
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
    fn selecting_company_switches_active() {
        let mut world = World::new();
        let mut sim = SimWorld::default();
        sim.state.ensure_companies();
        sim.state.ensure_rival_transcargo();
        let rival = sim.state.companies.iter().find(|c| c.is_ai).unwrap().id;
        world.insert_resource(sim);
        world.spawn((Button, CompanySelectorButton(rival), Interaction::Pressed));
        world
            .run_system_once(handle_company_selector_buttons)
            .unwrap();
        assert_eq!(world.resource::<SimWorld>().state.active_company, rival);
    }
}
