//! Sincronización de la ventana de vista del vehículo.

use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::VehicleKind;

use crate::render::{PrimaryGameCamera, TruckHandles, vehicle_world_position};
use crate::state::SimWorld;
use crate::ui::floating_window::{FloatingWindow, FloatingWindowId, FloatingWindowTitleText};
use crate::ui::vehicle_chain::{VehicleChainRegistry, VehicleChainSlot};

use super::status::format_vehicle_status;
use super::{
    VehicleConsistUnitSprite, VehicleWindowPreviewCamera, VehicleWindowRefitOnly,
    VehicleWindowRenameInput, VehicleWindowRenameRow, VehicleWindowState, VehicleWindowStatusText,
    VehicleWindowToggleText, VehicleWindowTrainOnly, vehicle_side_sprite,
};

/// TitleText → contenedor → title bar → FloatingWindow root.
fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
    let center = child_of.parent();
    let bar = parents.get(center).ok()?.parent();
    parents.get(bar).ok().map(|c| c.parent())
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn sync_vehicle_window(
    window_state: Res<VehicleWindowState>,
    chain: Res<VehicleChainRegistry>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(Entity, &mut FloatingWindow, &VehicleChainSlot, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut status_q: Query<
        (&VehicleChainSlot, &mut Text, &mut TextColor),
        (
            With<VehicleWindowStatusText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut toggle_q: Query<
        (&VehicleChainSlot, &mut Text),
        (
            With<VehicleWindowToggleText>,
            Without<VehicleWindowStatusText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut rename_row_q: Query<
        (&VehicleChainSlot, &mut Node),
        (
            With<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
            Without<VehicleWindowRefitOnly>,
        ),
    >,
    mut train_row_q: Query<
        (&VehicleChainSlot, &mut Node),
        (
            With<VehicleWindowTrainOnly>,
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowRefitOnly>,
        ),
    >,
    mut refit_row_q: Query<
        (&VehicleChainSlot, &mut Node),
        (
            With<VehicleWindowRefitOnly>,
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
        ),
    >,
    _rename_input_q: Query<&mut EditableText, With<VehicleWindowRenameInput>>,
    mut consist_q: Query<
        (
            &VehicleChainSlot,
            &VehicleConsistUnitSprite,
            &mut ImageNode,
            &mut Node,
        ),
        (
            Without<VehicleWindowRenameRow>,
            Without<VehicleWindowTrainOnly>,
            Without<VehicleWindowRefitOnly>,
        ),
    >,
    mut preview: Query<
        (&mut Transform, &mut Camera),
        (With<VehicleWindowPreviewCamera>, Without<PrimaryGameCamera>),
    >,
) {
    let focused_slot = window_state.vehicle_id.and_then(|id| chain.slot_of(id));

    for (root_entity, mut win, slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::Vehicle {
            continue;
        }
        let slot_idx = slot.0;
        let vehicle_id = chain.vehicle_at(slot_idx);
        win.key.instance = vehicle_id.unwrap_or(0);
        let vehicle = vehicle_id.and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
        let Some(vehicle) = vehicle else {
            *vis = Visibility::Hidden;
            for (consist_slot, _, _, mut node) in &mut consist_q {
                if consist_slot.0 == slot_idx {
                    node.display = Display::None;
                }
            }
            for (row_slot, mut row) in &mut train_row_q {
                if row_slot.0 == slot_idx {
                    row.display = Display::None;
                }
            }
            for (row_slot, mut row) in &mut refit_row_q {
                if row_slot.0 == slot_idx {
                    row.display = Display::None;
                }
            }
            for (row_slot, mut row) in &mut rename_row_q {
                if row_slot.0 == slot_idx {
                    row.display = Display::None;
                }
            }
            continue;
        };
        *vis = Visibility::Visible;

        let unit_ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id);
        if let Some(trucks) = trucks.as_ref() {
            for (consist_slot, sprite, mut image, mut node) in &mut consist_q {
                if consist_slot.0 != slot_idx {
                    continue;
                }
                if let Some(&unit_id) = unit_ids.get(sprite.unit_idx)
                    && let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id)
                {
                    node.display = Display::Flex;
                    image.image = vehicle_side_sprite(trucks, unit);
                } else {
                    node.display = Display::None;
                }
            }
        } else {
            for (consist_slot, _, _, mut node) in &mut consist_q {
                if consist_slot.0 == slot_idx {
                    node.display = Display::None;
                }
            }
        }

        let show_rename = window_state.rename_editing && focused_slot == Some(slot_idx);
        for (row_slot, mut row) in &mut rename_row_q {
            if row_slot.0 == slot_idx {
                row.display = if show_rename {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }

        let train_display = if vehicle.kind == VehicleKind::Train {
            Display::Flex
        } else {
            Display::None
        };
        for (row_slot, mut row) in &mut train_row_q {
            if row_slot.0 == slot_idx {
                row.display = train_display;
            }
        }

        let refit_display = if openttdrs_core::refit_allowed(vehicle, &sim.state.map) {
            Display::Flex
        } else {
            Display::None
        };
        for (row_slot, mut row) in &mut refit_row_q {
            if row_slot.0 == slot_idx {
                row.display = refit_display;
            }
        }

        let title_name = vehicle.display_name();
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::Vehicle {
                continue;
            }
            if title_root_entity(child_of, &parents) == Some(root_entity) {
                **text = title_name.clone();
            }
        }
        let (status_text, status_color) = format_vehicle_status(vehicle, &sim);
        for (status_slot, mut status, mut color) in &mut status_q {
            if status_slot.0 != slot_idx {
                continue;
            }
            **status = status_text.clone();
            *color = TextColor(status_color);
        }
        let toggle_label = if vehicle.running {
            "■".to_string()
        } else {
            "▶".to_string()
        };
        for (toggle_slot, mut toggle) in &mut toggle_q {
            if toggle_slot.0 != slot_idx {
                continue;
            }
            // Iconos ▶ / ■ en la toolbar (#174).
            **toggle = toggle_label.clone();
        }
    }

    // Preview camera sigue solo al vehículo enfocado.
    let focused = window_state
        .vehicle_id
        .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id));
    if let Ok((mut tf, mut cam)) = preview.single_mut() {
        if let Some(vehicle) = focused {
            cam.is_active = true;
            let world_pos = vehicle_world_position(vehicle, &sim.state.map);
            tf.translation = Vec3::new(world_pos.x, world_pos.y, 999.0);
        } else {
            cam.is_active = false;
        }
    }
}

