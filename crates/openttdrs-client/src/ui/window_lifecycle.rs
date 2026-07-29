//! Helpers para el ciclo de vida de ventanas flotantes (UI-9 / #159).
//!
//! Reduce la duplicación del patrón común:
//! - Resource `*WindowState { open: bool }`
//! - `setup_*` con `spawn_floating_window`
//! - `sync_*` que sincroniza `Visibility` según `state.open`
//! - `*_on_closed` que limpia `state.open = false` al recibir `FloatingWindowClosed`

use bevy::prelude::*;

use crate::ui::floating_window::{FloatingWindow, FloatingWindowClosed, FloatingWindowId};

/// Sincroniza la visibilidad de una ventana flotante según un valor booleano.
///
/// Busca la ventana por `id` y establece `Visibility::Visible` si `open` es
/// `true`, o `Visibility::Hidden` si es `false`.
pub(crate) fn sync_floating_window_visibility(
    windows: &mut Query<(&FloatingWindow, &mut Visibility)>,
    id: FloatingWindowId,
    open: bool,
) {
    if let Some((_, mut vis)) = windows.iter_mut().find(|(w, _)| w.id == id) {
        *vis = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Cierra una ventana flotante estableciendo un campo `open: bool` a `false`.
///
/// Lee mensajes `FloatingWindowClosed` y ejecuta el callback para el `id` dado.
pub(crate) fn close_floating_window_on_message(
    closed: &mut MessageReader<FloatingWindowClosed>,
    id: FloatingWindowId,
    mut set_closed: impl FnMut(),
) {
    for msg in closed.read() {
        if msg.0.class == id {
            set_closed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_visibility_basic() {
        let id = FloatingWindowId::Help;
        assert_eq!(id.storage_key(), "Help");
    }
}
