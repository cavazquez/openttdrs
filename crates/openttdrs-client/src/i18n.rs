//! Catálogo mínimo de textos del cliente.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Locale {
    #[default]
    Es,
    En,
}

impl Locale {
    pub(crate) const ALL: [Self; 2] = [Self::Es, Self::En];

    #[must_use]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
        }
    }

    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Es => "Español",
            Self::En => "English",
        }
    }

    #[must_use]
    pub(crate) fn from_code(code: &str) -> Self {
        let normalized = code.trim().to_ascii_lowercase();
        if normalized == "en" || normalized.starts_with("en-") || normalized.starts_with("en_") {
            Self::En
        } else {
            Self::Es
        }
    }
}

/// Traduce las claves del catálogo que ya migraron a locale.
/// Las superficies aún no migradas conservan su texto fuente.
#[must_use]
pub(crate) fn text(locale: Locale, source: &'static str) -> &'static str {
    if locale == Locale::Es {
        return source;
    }
    match source {
        "Archivo" => "File",
        "Mapa" => "Map",
        "Mundo" => "World",
        "Industrias" => "Industries",
        "Flota" => "Fleet",
        "Economía" => "Economy",
        "Ajustes" => "Settings",
        "Mensajes" => "Messages",
        "Ayuda" => "Help",
        "Idioma" => "Language",
        "Multijugador" => "Multiplayer",
        "Continuar partida" => "Continue game",
        "Nueva partida" => "New game",
        "Escenarios / heightmap" => "Scenarios / heightmap",
        "Editor de escenarios" => "Scenario editor",
        "Demo completa (mapa plano)" => "Full demo (flat map)",
        "Mejores puntuaciones" => "High scores",
        "Preferencias" => "Preferences",
        "Sonido / musica" => "Sound / music",
        "Salir" => "Exit",
        "Iniciar partida" => "Start game",
        "Volver" => "Back",
        "Si, salir" => "Yes, exit",
        "Cancelar" => "Cancel",
        "Minimapa" => "Minimap",
        "Mapa ampliado" => "Expanded map",
        "Opciones de visualización" => "Display options",
        "Vista extra" => "Extra viewport",
        "Carteles" => "Signs",
        "Guardar partida" => "Save game",
        "Cargar partida" => "Load game",
        "Volver al menú principal" => "Return to main menu",
        "Salir del juego" => "Exit game",
        "Guardar escenario" => "Save scenario",
        "Cargar escenario" => "Load scenario",
        "Guardar heightmap" => "Save heightmap",
        "Cargar heightmap" => "Load heightmap",
        "Salir del editor" => "Exit editor",
        "Directorio de pueblos" => "Town directory",
        "Lista de estaciones" => "Station list",
        "Lista de subvenciones" => "Subsidy list",
        "Historia" => "Story",
        "Directorio de industrias" => "Industry directory",
        "Trenes" => "Trains",
        "Vehículos de carretera" => "Road vehicles",
        "Barcos" => "Ships",
        "Aviones" => "Aircraft",
        "Finanzas" => "Finances",
        "Compañía" => "Company",
        "Ingresos" => "Income",
        "Beneficio operativo" => "Operating profit",
        "Valor de compañía" => "Company value",
        "Rendimiento" => "Performance",
        "Tarifas de carga" => "Cargo payment rates",
        "Objetivos" => "Goals",
        "Liga" => "League",
        "Sonido y música" => "Sound and music",
        "Distribución de carga" => "Cargo distribution",
        "IA / TransCargo" => "AI / TransCargo",
        "Noticias" => "News",
        "Ayuda y atajos" => "Help and shortcuts",
        "Consola" => "Console",
        "Inspector de tile" => "Tile inspector",
        "Historial de noticias" => "News history",
        "Preferencias de noticias" => "News preferences",
        "Clima" => "Climate",
        "Tamano del mapa (demo)" => "Map size (demo)",
        "Ancho (teselas)" => "Width (tiles)",
        "Alto (teselas)" => "Height (tiles)",
        "Ano de inicio" => "Start year",
        "Densidad de pueblos" => "Town density",
        "Densidad de industrias" => "Industry density",
        "Dinero inicial" => "Starting money",
        "Relieve" => "Terrain relief",
        "Terreno" => "Terrain",
        "Semilla" => "Seed",
        "Resolucion (reinicio al cambiar)" => "Resolution (restart to change)",
        "Escenarios: save/scenarios/ · Heightmaps: save/heightmaps/*.hmap" => {
            "Scenarios: save/scenarios/ · Heightmaps: save/heightmaps/*.hmap"
        }
        "Abrir escenarios (.json/.sav)" => "Open scenarios (.json/.sav)",
        "Abrir carpeta heightmaps" => "Open heightmaps folder",
        "Heightmaps detectados (clic para jugar)" => "Detected heightmaps (click to play)",
        "Abrir partida" => "Open game",
        "Abrir escenario" => "Open scenario",
        "Esc salir · raton para elegir" => "Esc quit · mouse to choose",
        "Enter iniciar · Esc volver · 1-4 clima · [ ] semilla · z/x densidad" => {
            "Enter start · Esc back · 1-4 climate · [ ] seed · z/x density"
        }
        "Esc volver" => "Esc back",
        "Esc cancelar" => "Esc cancel",
        _ => source,
    }
}

#[cfg(test)]
mod tests {
    use super::{Locale, text};

    #[test]
    fn locale_codes_accept_region_suffixes_and_fallback_to_spanish() {
        assert_eq!(Locale::from_code("en"), Locale::En);
        assert_eq!(Locale::from_code("EN-us"), Locale::En);
        assert_eq!(Locale::from_code("es-AR"), Locale::Es);
        assert_eq!(Locale::from_code("unknown"), Locale::Es);
    }

    #[test]
    fn catalog_translates_toolbar_text_without_changing_spanish() {
        assert_eq!(text(Locale::Es, "Guardar partida"), "Guardar partida");
        assert_eq!(text(Locale::En, "Guardar partida"), "Save game");
        assert_eq!(text(Locale::En, "Nueva partida"), "New game");
        assert_eq!(text(Locale::En, "Idioma"), "Language");
        assert_eq!(text(Locale::En, "Densidad de pueblos"), "Town density");
        assert_eq!(text(Locale::En, "Esc cancelar"), "Esc cancel");
        assert_eq!(text(Locale::En, "untranslated"), "untranslated");
    }
}
