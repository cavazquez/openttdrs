//! Manejo de renombrado de vehículos (botones, teclado, editable).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::{Command, vehicle::MAX_VEHICLE_NAME_CHARS};

use crate::state::SimWorld;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::vehicle_chain::{VehicleChainRegistry, VehicleChainSlot};

use super::{VehicleWindowRenameButton, VehicleWindowRenameInput, VehicleWindowState};

fn focused_rename_name(
    window_state: &VehicleWindowState,
    chain: &VehicleChainRegistry,
    rename_input_q: &Query<(&VehicleChainSlot, &EditableText), With<VehicleWindowRenameInput>>,
) -> Option<String> {
    let vehicle_id = window_state.vehicle_id?;
    let slot = chain.slot_of(vehicle_id)?;
    rename_input_q
        .iter()
        .find(|(s, _)| s.0 == slot)
        .map(|(_, e)| e.value().to_string())
}

fn apply_vehicle_rename(
    window_state: &mut VehicleWindowState,
    chain: &VehicleChainRegistry,
    sim: &mut SimWorld,
    hud_feedback: &mut HudBuildFeedback,
    rename_input_q: &Query<(&VehicleChainSlot, &EditableText), With<VehicleWindowRenameInput>>,
    elapsed_secs: f32,
) {
    let Some(vehicle_id) = window_state.vehicle_id else {
        return;
    };
    let name = focused_rename_name(window_state, chain, rename_input_q);
    match crate::network::apply_player_command(
        &mut sim.state,
        &Command::RenameVehicle { vehicle_id, name },
    ) {
        Ok(()) => window_state.rename_editing = false,
        Err(e) => push_build_command_error(hud_feedback, e, elapsed_secs),
    }
}

pub(crate) fn handle_vehicle_rename_buttons(
    mut buttons: Query<
        (&Interaction, &VehicleWindowRenameButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut window_state: ResMut<VehicleWindowState>,
    chain: Res<VehicleChainRegistry>,
    rename_input_q: Query<(&VehicleChainSlot, &EditableText), With<VehicleWindowRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            VehicleWindowRenameButton::Cancel => {
                window_state.rename_editing = false;
            }
            VehicleWindowRenameButton::Apply => {
                apply_vehicle_rename(
                    &mut window_state,
                    &chain,
                    &mut sim,
                    &mut hud_feedback,
                    &rename_input_q,
                    time.elapsed_secs(),
                );
            }
        }
    }
}

pub(crate) fn vehicle_window_rename_keyboard(
    mut window_state: ResMut<VehicleWindowState>,
    chain: Res<VehicleChainRegistry>,
    keys: Res<ButtonInput<KeyCode>>,
    rename_input_q: Query<(&VehicleChainSlot, &EditableText), With<VehicleWindowRenameInput>>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !window_state.rename_editing {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        window_state.rename_editing = false;
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        apply_vehicle_rename(
            &mut window_state,
            &chain,
            &mut sim,
            &mut hud_feedback,
            &rename_input_q,
            time.elapsed_secs(),
        );
    }
}

pub(crate) fn vehicle_window_rename_editable_keyboard(
    window_state: Res<VehicleWindowState>,
    chain: Res<VehicleChainRegistry>,
    mut key_events: MessageReader<KeyboardInput>,
    mut rename_input_q: Query<
        (&VehicleChainSlot, &mut EditableText),
        With<VehicleWindowRenameInput>,
    >,
) {
    if !window_state.rename_editing {
        return;
    }
    let Some(vehicle_id) = window_state.vehicle_id else {
        return;
    };
    let Some(slot) = chain.slot_of(vehicle_id) else {
        return;
    };
    let Some((_, mut editable)) = rename_input_q.iter_mut().find(|(s, _)| s.0 == slot) else {
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(text) = &ev.text else {
            continue;
        };
        for c in text.chars() {
            if !c.is_control() && editable.value().chars().count() < MAX_VEHICLE_NAME_CHARS {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
                ));
            }
        }
    }
}
