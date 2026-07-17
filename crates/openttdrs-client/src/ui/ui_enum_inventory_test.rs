//! Inventario UI-0 (#29/#30): conteos estables de enums de rutas.

use super::floating_window::FloatingWindowId;
use super::toolbar::editor_toolbar::EditorToolbarAction;
use super::toolbar::{BuildMenuAction, SaveMenuAction, ToolbarGroup};

/// Debe coincidir con la tabla de `docs/parity/ui_route_inventory.md`.
const EXPECTED_FLOATING_WINDOWS: usize = 42;
const EXPECTED_BUILD_MENU_ACTIONS: usize = 66;
const EXPECTED_SAVE_MENU_ACTIONS: usize = 22;
const EXPECTED_TOOLBAR_GROUPS: usize = 8;
const EXPECTED_EDITOR_TOOLBAR_ACTIONS: usize = 19;

#[test]
fn ui_enum_inventory_counts() {
    assert_eq!(
        FloatingWindowId::ALL.len(),
        EXPECTED_FLOATING_WINDOWS,
        "actualizar FloatingWindowId::ALL y docs/parity/ui_route_inventory.md"
    );
    assert_eq!(
        BuildMenuAction::ALL.len(),
        EXPECTED_BUILD_MENU_ACTIONS,
        "actualizar BuildMenuAction::ALL"
    );
    assert_eq!(
        SaveMenuAction::ALL.len(),
        EXPECTED_SAVE_MENU_ACTIONS,
        "actualizar SaveMenuAction::ALL"
    );
    assert_eq!(
        ToolbarGroup::ALL.len(),
        EXPECTED_TOOLBAR_GROUPS,
        "actualizar ToolbarGroup::ALL"
    );
    assert_eq!(
        EditorToolbarAction::ALL.len(),
        EXPECTED_EDITOR_TOOLBAR_ACTIONS,
        "actualizar EditorToolbarAction::ALL (#42)"
    );
}

#[test]
fn floating_window_all_has_unique_storage_keys() {
    let mut keys: Vec<&str> = FloatingWindowId::ALL
        .iter()
        .map(|id| id.storage_key())
        .collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), FloatingWindowId::ALL.len());
}

#[test]
fn build_menu_all_has_no_duplicates() {
    let mut seen = Vec::new();
    for &a in BuildMenuAction::ALL {
        assert!(
            !seen.contains(&a),
            "BuildMenuAction duplicado en ALL: {a:?}"
        );
        seen.push(a);
    }
}
