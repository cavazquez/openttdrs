//! Modelo declarativo de menús de toolbar.

use crate::ui::graph_window::GraphKind;
use crate::ui::navigation::UiRoute;
use crate::ui::vehicle_list::VehicleListKind;

/// Identificador de un menú de navegación de la toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MenuId {
    Map,
    World,
    Industries,
    Fleet,
    Economy,
}

impl MenuId {
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Map => "Mapa",
            Self::World => "Mundo",
            Self::Industries => "Industrias",
            Self::Fleet => "Flota",
            Self::Economy => "Economía",
        }
    }
}

/// Acción cliente (no abre `UiRoute`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuClientAction {
    ToggleMinimap,
    ExpandMinimap,
    OpenDisplayOptions,
    OpenExtraViewport,
}

/// Destino de una entrada de menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Route(UiRoute),
    Client(MenuClientAction),
}

/// Tipo visual de fila.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuEntryKind {
    Action,
    Divider,
}

/// Entrada declarativa.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuEntrySpec {
    pub kind: MenuEntryKind,
    pub label: &'static str,
    pub action: Option<MenuAction>,
    /// Si `false`, la entrada no responde a clic y se dibuja atenuada.
    pub enabled: bool,
    /// Marca de verificación (p. ej. minimapa visible).
    pub checkable: bool,
    pub hotkey: Option<&'static str>,
}

impl MenuEntrySpec {
    #[must_use]
    pub(crate) const fn item(label: &'static str, action: MenuAction) -> Self {
        Self {
            kind: MenuEntryKind::Action,
            label,
            action: Some(action),
            enabled: true,
            checkable: false,
            hotkey: None,
        }
    }

    #[must_use]
    pub(crate) const fn checkable(label: &'static str, action: MenuAction) -> Self {
        Self {
            kind: MenuEntryKind::Action,
            label,
            action: Some(action),
            enabled: true,
            checkable: true,
            hotkey: None,
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn disabled(label: &'static str, action: MenuAction) -> Self {
        Self {
            kind: MenuEntryKind::Action,
            label,
            action: Some(action),
            enabled: false,
            checkable: false,
            hotkey: None,
        }
    }

    #[must_use]
    #[allow(dead_code)] // API lista; hotkeys se muestran en chrome cuando se asignen.
    pub(crate) const fn with_hotkey(mut self, hotkey: &'static str) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    #[must_use]
    pub(crate) const fn divider() -> Self {
        Self {
            kind: MenuEntryKind::Divider,
            label: "",
            action: None,
            enabled: false,
            checkable: false,
            hotkey: None,
        }
    }
}

/// Especificación completa de un menú.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuSpec {
    pub id: MenuId,
    pub entries: &'static [MenuEntrySpec],
}

pub(crate) const MAP_MENU: MenuSpec = MenuSpec {
    id: MenuId::Map,
    entries: &[
        MenuEntrySpec::checkable(
            "Minimapa",
            MenuAction::Client(MenuClientAction::ToggleMinimap),
        ),
        MenuEntrySpec::checkable(
            "Mapa ampliado",
            MenuAction::Client(MenuClientAction::ExpandMinimap),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item(
            "Opciones de visualización",
            MenuAction::Client(MenuClientAction::OpenDisplayOptions),
        ),
        MenuEntrySpec::item(
            "Vista extra",
            MenuAction::Client(MenuClientAction::OpenExtraViewport),
        ),
        MenuEntrySpec::item("Carteles", MenuAction::Route(UiRoute::SignList)),
    ],
};

pub(crate) const WORLD_MENU: MenuSpec = MenuSpec {
    id: MenuId::World,
    entries: &[
        MenuEntrySpec::item("Directorio de pueblos", MenuAction::Route(UiRoute::Towns)),
        MenuEntrySpec::item("Lista de estaciones", MenuAction::Route(UiRoute::Stations)),
        MenuEntrySpec::item(
            "Lista de subvenciones",
            MenuAction::Route(UiRoute::Subsidies),
        ),
        MenuEntrySpec::item("Historia", MenuAction::Route(UiRoute::Story)),
    ],
};

pub(crate) const INDUSTRIES_MENU: MenuSpec = MenuSpec {
    id: MenuId::Industries,
    entries: &[
        MenuEntrySpec::item(
            "Directorio de industrias",
            MenuAction::Route(UiRoute::Industries),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item("Link Graph", MenuAction::Route(UiRoute::LinkGraph)),
    ],
};

pub(crate) const FLEET_MENU: MenuSpec = MenuSpec {
    id: MenuId::Fleet,
    entries: &[
        MenuEntrySpec::item(
            "Trenes",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Train)),
        ),
        MenuEntrySpec::item(
            "Vehículos de carretera",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Road)),
        ),
        MenuEntrySpec::item(
            "Barcos",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Ship)),
        ),
        MenuEntrySpec::item(
            "Aviones",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Aircraft)),
        ),
    ],
};

pub(crate) const ECONOMY_MENU: MenuSpec = MenuSpec {
    id: MenuId::Economy,
    entries: &[
        MenuEntrySpec::item("Finanzas", MenuAction::Route(UiRoute::Finances)),
        MenuEntrySpec::item(
            "Ingresos",
            MenuAction::Route(UiRoute::Graph(GraphKind::Income)),
        ),
        MenuEntrySpec::item(
            "Beneficio operativo",
            MenuAction::Route(UiRoute::Graph(GraphKind::OperatingProfit)),
        ),
        MenuEntrySpec::item(
            "Valor de compañía",
            MenuAction::Route(UiRoute::Graph(GraphKind::CompanyValue)),
        ),
        MenuEntrySpec::item(
            "Rendimiento",
            MenuAction::Route(UiRoute::Graph(GraphKind::PerformanceHistory)),
        ),
        MenuEntrySpec::item(
            "Tarifas de carga",
            MenuAction::Route(UiRoute::CargoPaymentRates),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item("Objetivos", MenuAction::Route(UiRoute::Goals)),
        MenuEntrySpec::item("Liga", MenuAction::Route(UiRoute::League)),
    ],
};

#[must_use]
pub(crate) fn all_toolbar_menu_specs() -> &'static [MenuSpec] {
    &[
        MAP_MENU,
        WORLD_MENU,
        INDUSTRIES_MENU,
        FLEET_MENU,
        ECONOMY_MENU,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_menus_are_present() {
        let ids: Vec<_> = all_toolbar_menu_specs().iter().map(|s| s.id).collect();
        assert!(ids.contains(&MenuId::Map));
        assert!(ids.contains(&MenuId::World));
        assert!(ids.contains(&MenuId::Industries));
    }

    #[test]
    fn industries_link_graph_is_enabled() {
        let entry = INDUSTRIES_MENU
            .entries
            .iter()
            .find(|e| e.label.contains("Link Graph"));
        assert!(entry.is_some_and(|e| e.enabled));
    }
}
