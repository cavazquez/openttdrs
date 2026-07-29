//! Pila modal MVP (#272): ownership, z-order y restore de foco.
//!
//! Un modal bloquea interacción del owner (flag), se apila sobre él (z ≥
//! [`MODAL_BASE_Z`]) y al cerrarse restaura el foco al owner. Escape cierra el
//! tope; Enter confirma query/error mínimos.

use bevy::prelude::*;

use crate::ui::floating_window::{FloatingWindowId, WindowKey};

/// Z base reservada para modales (sobre flotantes 2400, bajo overlays 3100).
pub(crate) const MODAL_BASE_Z: i32 = 2900;

/// Tipo de diálogo modal inventariado en `DIALOGS_FAMILY`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ModalKind {
    QueryString,
    ErrorDialog,
    OnScreenKeyboard,
}

impl ModalKind {
    #[must_use]
    pub(crate) const fn window_id(self) -> FloatingWindowId {
        match self {
            Self::QueryString => FloatingWindowId::QueryString,
            Self::ErrorDialog => FloatingWindowId::ErrorDialog,
            Self::OnScreenKeyboard => FloatingWindowId::OnScreenKeyboard,
        }
    }
}

/// Entrada de la pila: modal + owner bloqueado + foco a restaurar.
#[derive(Clone, Debug)]
pub(crate) struct ModalEntry {
    pub(crate) kind: ModalKind,
    pub(crate) key: WindowKey,
    /// Ventana dueña bloqueada mientras el modal está abierto.
    pub(crate) owner: Option<WindowKey>,
    /// Foco previo a restaurar al pop.
    pub(crate) restore_focus: Option<WindowKey>,
    /// Texto editable (query / OSK) o mensaje (error).
    pub(crate) text: String,
    /// `true` si Enter confirma (query/error); Escape siempre cancela.
    pub(crate) confirm_on_enter: bool,
}

/// Resource: pila LIFO de modales activos.
#[derive(Resource, Default, Debug)]
pub(crate) struct ModalStack {
    entries: Vec<ModalEntry>,
    /// Owners bloqueados (unión de todos los owners en la pila).
    blocked_owners: Vec<WindowKey>,
    last_restored_focus: Option<WindowKey>,
    /// Contador de z local sobre [`MODAL_BASE_Z`].
    z_bump: i32,
}

impl ModalStack {
    #[must_use]
    #[allow(dead_code)] // API pila; tests + hotkeys futuros
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    #[allow(dead_code)] // API pila; tests
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub(crate) fn top(&self) -> Option<&ModalEntry> {
        self.entries.last()
    }

    /// ¿Hay alguna entrada del kind dado en la pila?
    #[must_use]
    pub(crate) fn contains_kind(&self, kind: ModalKind) -> bool {
        self.entries.iter().any(|e| e.kind == kind)
    }

    /// ¿El owner está bloqueado por algún modal de la pila?
    #[must_use]
    #[allow(dead_code)] // API ownership; tests + bloqueo hotkeys futuros
    pub(crate) fn owner_is_blocked(&self, owner: WindowKey) -> bool {
        self.blocked_owners.contains(&owner)
    }

    /// Empuja un modal; bloquea owner y guarda foco a restaurar.
    pub(crate) fn push(&mut self, mut entry: ModalEntry) -> i32 {
        if let Some(owner) = entry.owner
            && !self.blocked_owners.contains(&owner)
        {
            self.blocked_owners.push(owner);
        }
        if entry.restore_focus.is_none() {
            entry.restore_focus = entry.owner;
        }
        self.z_bump += 1;
        let z = MODAL_BASE_Z + self.z_bump;
        self.entries.push(entry);
        z
    }

    /// Cierra el modal tope (Escape / cancel). Restaura foco del entry.
    pub(crate) fn pop_cancel(&mut self) -> Option<ModalEntry> {
        let entry = self.entries.pop()?;
        self.rebuild_blocked();
        self.last_restored_focus = entry.restore_focus;
        Some(entry)
    }

    /// Confirma el modal tope (Enter en query/error).
    pub(crate) fn pop_confirm(&mut self) -> Option<ModalEntry> {
        let top = self.entries.last()?;
        if !top.confirm_on_enter {
            return None;
        }
        self.pop_cancel()
    }

    /// Escape: cierra el tope si hay pila.
    pub(crate) fn handle_escape(&mut self) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let _ = self.pop_cancel();
        true
    }

    /// Enter: confirma query/error del tope.
    pub(crate) fn handle_enter(&mut self) -> bool {
        self.pop_confirm().is_some()
    }

    #[must_use]
    #[allow(dead_code)] // restore focus al cerrar; tests
    pub(crate) fn last_restored_focus(&self) -> Option<WindowKey> {
        self.last_restored_focus
    }

    fn rebuild_blocked(&mut self) {
        self.blocked_owners.clear();
        for entry in &self.entries {
            if let Some(owner) = entry.owner
                && !self.blocked_owners.contains(&owner)
            {
                self.blocked_owners.push(owner);
            }
        }
        if self.entries.is_empty() {
            self.z_bump = 0;
        }
    }
}

/// Abre query string modal (NewGRF rename / path genérico).
pub(crate) fn push_query_string(
    stack: &mut ModalStack,
    owner: Option<WindowKey>,
    initial: impl Into<String>,
) -> i32 {
    stack.push(ModalEntry {
        kind: ModalKind::QueryString,
        key: WindowKey::singleton(FloatingWindowId::QueryString),
        owner,
        restore_focus: owner,
        text: initial.into(),
        confirm_on_enter: true,
    })
}

/// Abre diálogo de error mínimo.
pub(crate) fn push_error_dialog(
    stack: &mut ModalStack,
    owner: Option<WindowKey>,
    message: impl Into<String>,
) -> i32 {
    stack.push(ModalEntry {
        kind: ModalKind::ErrorDialog,
        key: WindowKey::singleton(FloatingWindowId::ErrorDialog),
        owner,
        restore_focus: owner,
        text: message.into(),
        confirm_on_enter: true,
    })
}

/// Abre OSK stub apuntando a edición de texto (query path / rename).
pub(crate) fn push_osk(
    stack: &mut ModalStack,
    owner: Option<WindowKey>,
    initial: impl Into<String>,
) -> i32 {
    stack.push(ModalEntry {
        kind: ModalKind::OnScreenKeyboard,
        key: WindowKey::singleton(FloatingWindowId::OnScreenKeyboard),
        owner,
        restore_focus: owner,
        text: initial.into(),
        confirm_on_enter: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_stack_blocks_owner_and_restores_focus_on_escape() {
        let mut stack = ModalStack::default();
        let owner = WindowKey::singleton(FloatingWindowId::NewGrf);
        let z = push_query_string(&mut stack, Some(owner), "rename");
        assert!(z >= MODAL_BASE_Z);
        assert!(stack.owner_is_blocked(owner));
        assert_eq!(stack.top().map(|e| e.kind), Some(ModalKind::QueryString));
        assert!(stack.handle_escape());
        assert!(stack.is_empty());
        assert!(!stack.owner_is_blocked(owner));
        assert_eq!(stack.last_restored_focus(), Some(owner));
    }

    #[test]
    fn modal_enter_confirms_query_and_error_only() {
        let mut stack = ModalStack::default();
        push_osk(&mut stack, None, "");
        assert!(!stack.handle_enter(), "OSK no confirma con Enter");
        assert_eq!(stack.len(), 1);
        assert!(stack.handle_escape());

        push_error_dialog(&mut stack, None, "boom");
        assert!(stack.handle_enter());
        assert!(stack.is_empty());

        push_query_string(&mut stack, None, "path");
        assert!(stack.handle_enter());
        assert!(stack.is_empty());
    }

    #[test]
    fn modal_stack_z_order_increases_over_parent() {
        let mut stack = ModalStack::default();
        let owner = WindowKey::singleton(FloatingWindowId::NewGrf);
        let z1 = push_query_string(&mut stack, Some(owner), "a");
        let z2 = push_osk(&mut stack, Some(owner), "b");
        assert!(z2 > z1);
        assert!(z1 >= MODAL_BASE_Z);
        assert_eq!(stack.top().map(|e| e.kind), Some(ModalKind::OnScreenKeyboard));
        assert!(stack.handle_escape());
        assert_eq!(stack.top().map(|e| e.kind), Some(ModalKind::QueryString));
    }
}
