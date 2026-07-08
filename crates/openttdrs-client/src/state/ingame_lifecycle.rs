//! Marcadores compartidos del ciclo de vida en partida.

use bevy::prelude::*;

/// Marca la raíz del toolbar superior (solo partida).
#[derive(Component)]
pub(crate) struct InGameUi;
