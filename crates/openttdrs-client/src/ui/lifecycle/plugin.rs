//! Plugin y sistema `leave_ingame`.

use bevy::prelude::*;

use crate::state::ClientScreen;

use super::entity_cleanup::despawn_ingame_entities;
use super::resource_reset::apply_session_resource_teardown;

pub(crate) struct InGameLifecyclePlugin;

impl Plugin for InGameLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(ClientScreen::InGame), leave_ingame);
    }
}

/// Desmonta mundo, HUD y recursos de sesión al volver al menú u otra pantalla.
pub(crate) fn leave_ingame(world: &mut World) {
    despawn_ingame_entities(world);
    apply_session_resource_teardown(world);
}
