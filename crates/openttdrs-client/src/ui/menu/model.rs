//! Modelo declarativo de menús de toolbar.

use bevy::prelude::Resource;

use crate::ui::graph_window::GraphKind;
use crate::ui::navigation::UiRoute;
use crate::ui::vehicle_list::VehicleListKind;

/// Identificador de un menú de navegación de la toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MenuId {
    File,
    EditorFile,
    Map,
    EditorMap,
    World,
    Industries,
    Fleet,
    Economy,
    Settings,
    Messages,
    Help,
}

impl MenuId {
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::File => "Archivo",
            Self::EditorFile => "Archivo",
            Self::Map => "Mapa",
            Self::EditorMap => "Mapa",
            Self::World => "Mundo",
            Self::Industries => "Industrias",
            Self::Fleet => "Flota",
            Self::Economy => "Economía",
            Self::Settings => "Ajustes",
            Self::Messages => "Mensajes",
            Self::Help => "Ayuda",
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

/// Condición dinámica que debe cumplir una entrada además de estar implementada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuAvailability {
    Always,
    HasCompanies,
    HasGoals,
    HasStory,
    /// Para pausa/avance cuando el catálogo lo declare (clientes MP no controlan sim).
    #[allow(dead_code)]
    CanControlSimulation,
}

/// Estado de disponibilidad recalculado desde la partida, no almacenado en el catálogo.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolbarContext {
    pub(crate) has_companies: bool,
    pub(crate) has_goals: bool,
    pub(crate) has_story: bool,
    /// Los clientes reciben pausa/avance del servidor y no pueden imponerlos localmente.
    pub(crate) can_control_simulation: bool,
}

impl Default for ToolbarContext {
    fn default() -> Self {
        Self {
            has_companies: true,
            has_goals: true,
            has_story: true,
            can_control_simulation: true,
        }
    }
}

impl ToolbarContext {
    #[must_use]
    pub(crate) const fn allows(self, availability: MenuAvailability) -> bool {
        match availability {
            MenuAvailability::Always => true,
            MenuAvailability::HasCompanies => self.has_companies,
            MenuAvailability::HasGoals => self.has_goals,
            MenuAvailability::HasStory => self.has_story,
            MenuAvailability::CanControlSimulation => self.can_control_simulation,
        }
    }
}

/// Entrada declarativa.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MenuEntrySpec {
    pub kind: MenuEntryKind,
    pub label: &'static str,
    pub action: Option<MenuAction>,
    /// Si `false`, la entrada no responde a clic y se dibuja atenuada.
    pub enabled: bool,
    pub availability: MenuAvailability,
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
            availability: MenuAvailability::Always,
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
            availability: MenuAvailability::Always,
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
            availability: MenuAvailability::Always,
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
    pub(crate) const fn when(mut self, availability: MenuAvailability) -> Self {
        self.availability = availability;
        self
    }

    #[must_use]
    pub(crate) const fn divider() -> Self {
        Self {
            kind: MenuEntryKind::Divider,
            label: "",
            action: None,
            enabled: false,
            availability: MenuAvailability::Always,
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

pub(crate) const FILE_MENU: MenuSpec = MenuSpec {
    id: MenuId::File,
    entries: &[
        MenuEntrySpec::item("Guardar partida", MenuAction::Route(UiRoute::SaveGame)),
        MenuEntrySpec::item("Cargar partida", MenuAction::Route(UiRoute::LoadGame)),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item(
            "Volver al menú principal",
            MenuAction::Route(UiRoute::ReturnMainMenu),
        ),
        MenuEntrySpec::item("Salir del juego", MenuAction::Route(UiRoute::ExitGame)),
    ],
};

pub(crate) const EDITOR_FILE_MENU: MenuSpec = MenuSpec {
    id: MenuId::EditorFile,
    entries: &[
        MenuEntrySpec::item(
            "Guardar escenario",
            MenuAction::Route(UiRoute::EditorSaveScenario),
        ),
        MenuEntrySpec::item(
            "Cargar escenario",
            MenuAction::Route(UiRoute::EditorLoadScenario),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item(
            "Guardar heightmap",
            MenuAction::Route(UiRoute::EditorSaveHeightmap),
        ),
        MenuEntrySpec::item(
            "Cargar heightmap",
            MenuAction::Route(UiRoute::EditorLoadHeightmap),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item("Salir del editor", MenuAction::Route(UiRoute::EditorExit)),
        MenuEntrySpec::item("Salir del juego", MenuAction::Route(UiRoute::ExitGame)),
    ],
};

pub(crate) const EDITOR_MAP_MENU: MenuSpec = MenuSpec {
    id: MenuId::EditorMap,
    entries: &[
        MenuEntrySpec::checkable(
            "Minimapa",
            MenuAction::Client(MenuClientAction::ToggleMinimap),
        ),
        MenuEntrySpec::item("Directorio de pueblos", MenuAction::Route(UiRoute::Towns)),
        MenuEntrySpec::item(
            "Vista extra",
            MenuAction::Client(MenuClientAction::OpenExtraViewport),
        ),
    ],
};

pub(crate) const WORLD_MENU: MenuSpec = MenuSpec {
    id: MenuId::World,
    entries: &[
        MenuEntrySpec::item("Directorio de pueblos", MenuAction::Route(UiRoute::Towns)),
        MenuEntrySpec::item("Lista de estaciones", MenuAction::Route(UiRoute::Stations))
            .when(MenuAvailability::HasCompanies),
        MenuEntrySpec::item(
            "Lista de subvenciones",
            MenuAction::Route(UiRoute::Subsidies),
        ),
        MenuEntrySpec::item("Historia", MenuAction::Route(UiRoute::Story))
            .when(MenuAvailability::HasStory),
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
        )
        .when(MenuAvailability::HasCompanies),
        MenuEntrySpec::item(
            "Vehículos de carretera",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Road)),
        )
        .when(MenuAvailability::HasCompanies),
        MenuEntrySpec::item(
            "Barcos",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Ship)),
        )
        .when(MenuAvailability::HasCompanies),
        MenuEntrySpec::item(
            "Aviones",
            MenuAction::Route(UiRoute::Vehicles(VehicleListKind::Aircraft)),
        )
        .when(MenuAvailability::HasCompanies),
    ],
};

pub(crate) const ECONOMY_MENU: MenuSpec = MenuSpec {
    id: MenuId::Economy,
    entries: &[
        MenuEntrySpec::item("Finanzas", MenuAction::Route(UiRoute::Finances))
            .when(MenuAvailability::HasCompanies),
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
        MenuEntrySpec::item("Objetivos", MenuAction::Route(UiRoute::Goals))
            .when(MenuAvailability::HasGoals),
        MenuEntrySpec::item("Liga", MenuAction::Route(UiRoute::League))
            .when(MenuAvailability::HasCompanies),
    ],
};

pub(crate) const SETTINGS_MENU: MenuSpec = MenuSpec {
    id: MenuId::Settings,
    entries: &[
        MenuEntrySpec::item(
            "Opciones de visualización",
            MenuAction::Route(UiRoute::DisplayOptions),
        ),
        MenuEntrySpec::item("Sonido y música", MenuAction::Route(UiRoute::SoundMusic)),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item(
            "Pathfinding / PBS",
            MenuAction::Route(UiRoute::PathfindingSettings),
        ),
        MenuEntrySpec::item(
            "Distribución de carga",
            MenuAction::Route(UiRoute::CargoDistSettings),
        ),
        MenuEntrySpec::item("IA / TransCargo", MenuAction::Route(UiRoute::AiSettings)),
        MenuEntrySpec::item("NewGRF", MenuAction::Route(UiRoute::NewGrf)),
        MenuEntrySpec::item("Noticias", MenuAction::Route(UiRoute::NewsSettings)),
    ],
};

pub(crate) const HELP_MENU: MenuSpec = MenuSpec {
    id: MenuId::Help,
    entries: &[
        MenuEntrySpec::item("Ayuda y atajos", MenuAction::Route(UiRoute::Help)),
        MenuEntrySpec::item("Consola", MenuAction::Route(UiRoute::DevConsole)),
        MenuEntrySpec::item(
            "Inspector de tile",
            MenuAction::Route(UiRoute::TileInspector),
        ),
        MenuEntrySpec::divider(),
        MenuEntrySpec::item("Cheats", MenuAction::Route(UiRoute::Cheats)),
    ],
};

pub(crate) const MESSAGES_MENU: MenuSpec = MenuSpec {
    id: MenuId::Messages,
    entries: &[
        MenuEntrySpec::item(
            "Historial de noticias",
            MenuAction::Route(UiRoute::NewsHistory),
        ),
        MenuEntrySpec::item(
            "Preferencias de noticias",
            MenuAction::Route(UiRoute::NewsSettings),
        ),
    ],
};

#[must_use]
pub(crate) fn all_toolbar_menu_specs() -> &'static [MenuSpec] {
    &[
        FILE_MENU,
        EDITOR_FILE_MENU,
        MAP_MENU,
        EDITOR_MAP_MENU,
        WORLD_MENU,
        INDUSTRIES_MENU,
        FLEET_MENU,
        ECONOMY_MENU,
        SETTINGS_MENU,
        MESSAGES_MENU,
        HELP_MENU,
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn pilot_menus_are_present() {
        let ids: Vec<_> = all_toolbar_menu_specs().iter().map(|s| s.id).collect();
        assert!(ids.contains(&MenuId::Map));
        assert!(ids.contains(&MenuId::File));
        assert!(ids.contains(&MenuId::EditorFile));
        assert!(ids.contains(&MenuId::EditorMap));
        assert!(ids.contains(&MenuId::World));
        assert!(ids.contains(&MenuId::Industries));
        assert!(ids.contains(&MenuId::Settings));
        assert!(ids.contains(&MenuId::Help));
        assert!(ids.contains(&MenuId::Messages));
    }

    #[test]
    fn industries_link_graph_is_enabled() {
        let entry = INDUSTRIES_MENU
            .entries
            .iter()
            .find(|e| e.label.contains("Link Graph"));
        assert!(entry.is_some_and(|e| e.enabled));
    }

    #[test]
    fn dynamic_requirements_are_declared_in_catalog() {
        let story = WORLD_MENU
            .entries
            .iter()
            .find(|entry| entry.label == "Historia")
            .expect("entrada Historia");
        let goals = ECONOMY_MENU
            .entries
            .iter()
            .find(|entry| entry.label == "Objetivos")
            .expect("entrada Objetivos");
        assert_eq!(story.availability, MenuAvailability::HasStory);
        assert_eq!(goals.availability, MenuAvailability::HasGoals);
    }

    #[test]
    fn toolbar_context_rejects_missing_content() {
        let context = ToolbarContext {
            has_companies: false,
            has_goals: false,
            has_story: false,
            can_control_simulation: false,
        };
        assert!(context.allows(MenuAvailability::Always));
        assert!(!context.allows(MenuAvailability::HasCompanies));
        assert!(!context.allows(MenuAvailability::HasGoals));
        assert!(!context.allows(MenuAvailability::HasStory));
        assert!(!context.allows(MenuAvailability::CanControlSimulation));
    }
}
