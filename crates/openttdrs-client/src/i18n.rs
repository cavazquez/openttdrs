//! Catálogo y sincronización de locale de las superficies UI del cliente.

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::text::EditableText;

use crate::bevy_app::UpdateSet;
use crate::settings::ClientPreferences;

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

/// Clave española conservada por un texto UI que participa del catálogo.
///
/// Sólo se inserta cuando la cadena coincide exactamente con una entrada
/// conocida. Así un nombre de pueblo, un texto NewGRF o un mensaje de partida
/// no se confunde con una etiqueta de interfaz al cambiar el idioma.
#[derive(Component, Debug, Clone)]
struct LocalizedUiText(String);

/// Aplica el locale también a ventanas creadas después de cambiar la
/// preferencia, sin obligar a cada constructor de UI a duplicar un marcador.
pub(crate) struct LocalizationPlugin;

impl Plugin for LocalizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                register_catalog_ui_texts,
                sync_new_catalog_ui_texts,
                sync_catalog_ui_texts_on_locale_change,
                sync_changed_catalog_ui_texts,
            )
                .chain()
                .in_set(UpdateSet::Ui),
        );
    }
}

fn register_catalog_ui_texts(
    mut commands: Commands,
    texts: Query<
        (Entity, &Text),
        (
            Or<(Added<Text>, Changed<Text>)>,
            Without<LocalizedUiText>,
            Without<EditableText>,
        ),
    >,
) {
    for (entity, value) in &texts {
        let source = value.as_str();
        if text(Locale::En, source) != source {
            commands
                .entity(entity)
                .insert(LocalizedUiText(source.to_owned()));
        }
    }
}

fn sync_new_catalog_ui_texts(
    prefs: Res<ClientPreferences>,
    mut texts: Query<(&LocalizedUiText, &mut Text), Added<LocalizedUiText>>,
) {
    sync_catalog_ui_texts(prefs.locale(), &mut texts);
}

fn sync_catalog_ui_texts_on_locale_change(
    prefs: Res<ClientPreferences>,
    mut texts: Query<(&LocalizedUiText, &mut Text)>,
) {
    if prefs.is_changed() {
        sync_catalog_ui_texts(prefs.locale(), &mut texts);
    }
}

/// Algunos títulos y resúmenes se escriben después de crear su entidad. Al
/// volver a recibir la clave española, se la vuelve a localizar sin tocar
/// entradas editables ni datos de la partida.
fn sync_changed_catalog_ui_texts(
    prefs: Res<ClientPreferences>,
    mut texts: Query<(&LocalizedUiText, &mut Text), (Changed<Text>, Without<EditableText>)>,
) {
    sync_catalog_ui_texts(prefs.locale(), &mut texts);
}

fn sync_catalog_ui_texts<F: QueryFilter>(
    locale: Locale,
    texts: &mut Query<(&LocalizedUiText, &mut Text), F>,
) {
    for (key, mut value) in texts.iter_mut() {
        let translated = text(locale, &key.0);
        if value.as_str() != translated {
            **value = translated.to_owned();
        }
    }
}

/// Traduce las claves del catálogo que ya migraron a locale.
/// Las superficies aún no migradas conservan su texto fuente.
#[must_use]
pub(crate) fn text(locale: Locale, source: &str) -> &str {
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
        "Templado" => "Temperate",
        "Artico" => "Arctic",
        "Baja" => "Sparse",
        "Media" => "Normal",
        "Alta" => "Dense",
        "Llano" => "Flat",
        "Montañoso" => "Hilly",
        "clima" => "climate",
        "inicio" => "start",
        "semilla" => "seed",
        "Pueblos" => "Towns",
        "industrias" => "industries",
        "capital" => "cash",
        "relieve" => "terrain",
        "rival" => "rival",
        "desastres" => "disasters",
        "sí" => "yes",
        "no" => "no",
        "auto" => "auto",
        "isla procedural + lagos" => "procedural island + lakes",
        "colinas procedural + lagos" => "procedural hills + lakes",
        "demo completa (plana)" => "full demo (flat)",
        "mapa plano" => "flat map",
        // Ventanas y controles reutilizables.
        "Singleplayer · Ctrl+Alt+C · consola: cheat …" => {
            "Singleplayer · Ctrl+Alt+C · console: cheat …"
        }
        "GameScript-lite · progreso de goals del escenario" => {
            "GameScript-lite · scenario goal progress"
        }
        "Editor · regenera el mapa (borra pueblos/industrias/infra)" => {
            "Editor · regenerates the map (clears towns/industries/infrastructure)"
        }
        "Semilla —" => "Seed —",
        "Stack + params (P◀/P▶, −/+) + Inspeccionar. Action2 lee param[] vía 0x7F." => {
            "Stack + params (P◀/P▶, −/+) + Inspect. Action2 reads param[] through 0x7F."
        }
        "Selecciona una entrada: Inspeccionar o edita params (P◀/P▶, −/+)." => {
            "Select an entry: Inspect it or edit params (P◀/P▶, −/+)."
        }
        "Pools de órdenes compartidas." => "Shared order pools.",
        "Vincular vehículo" => "Link vehicle",
        "Orientación del muelle" => "Dock orientation",
        "Tipo de depósito a construir" => "Depot type to build",
        "filtrar…" => "filter…",
        "El escenario tiene cambios sin guardar." => "The scenario has unsaved changes.",
        "Tipo (Ctrl+clic cicla; Ctrl+Shift cambia estilo)" => {
            "Type (Ctrl+click cycles; Ctrl+Shift changes style)"
        }
        "Estilo" => "Style",
        "Densidad al arrastrar (Shift+RMB cicla)" => "Density while dragging (Shift+RMB cycles)",
        "Acepta: Nada" => "Accepts: Nothing",
        "Suministra: Nada" => "Supplies: Nothing",
        "Sin paradas NewGRF para este tipo" => "No NewGRF stops for this type",
        "— Música —" => "— Music —",
        "Detenido · 0 / 0" => "Stopped · 0 / 0",
        "(sin pistas)" => "(no tracks)",
        "Espera ante path sin reserva (días). 255 = nunca girar." => {
            "Wait for path without reservation (days). 255 = never turn around."
        }
        "Intervalo de look-ahead (ticks). 255 = desactivar." => {
            "Look-ahead interval (ticks). 255 = disable."
        }
        "Girar en señales" => "Turn at signals",
        "Siempre reservar" => "Always reserve",
        "Por defecto" => "Default",
        "Selecciona un tile (clic) · F2 abre/cierra · gizmos marcan bounds" => {
            "Select a tile (click) · F2 opens/closes · gizmos mark bounds"
        }
        "(sin selección)" => "(no selection)",
        "Fundar pueblo" => "Found town",
        "Seleccionado: —" => "Selected: —",
        "Escribe help y Enter. F3 / ` abre o cierra." => {
            "Type help and Enter. F3 / ` opens or closes."
        }
        "Arrastra un tramo válido sobre agua o desnivel." => {
            "Drag a valid span over water or uneven land."
        }
        "Tamaño: —" => "Size: —",
        "Cobertura: —" => "Coverage: —",
        "Preferencias de cliente (se guardan al salir)" => "Client preferences (saved on exit)",
        "Presets de cliente" => "Client presets",
        "Transparencia / invisibilidad (TO_*)" => "Transparency / invisibility (TO_*)",
        "Elige un destino para añadirlo a la ruta." => "Choose a destination to add to the route.",
        "Elegir en el mapa" => "Choose on map",
        "Elige el tipo de carga." => "Choose the cargo type.",
        "Nombre:" => "Name:",
        "Sigue la cámara principal (zoom más alejado)." => {
            "Follows the main camera (more zoomed out)."
        }
        "Stock: --" => "Stock: --",
        "Off = silencio · Summary = ticker · Full = cartel" => {
            "Off = silence · Summary = ticker · Full = newspaper"
        }
        "Fin de partida" => "End of game",
        "Menú principal" => "Main menu",
        "capas" => "layers",
        "Clic en fila: seleccionar y centrar origen" => "Click a row: select and center its origin",
        "No hay subvenciones activas ni ofertas." => "There are no active subsidies or offers.",
        "Finanzas…" => "Finances…",
        "Reglas de autoreemplazo." => "Autoreplace rules.",
        "Sin páginas de historia." => "No story pages.",
        "Ajustes del rival TransCargo (construcción mensual)." => {
            "TransCargo rival settings (monthly construction)."
        }
        "IA activa" => "AI enabled",
        "Umbral de dinero para nueva ruta" => "Cash threshold for a new route",
        "Máximo de rutas (trenes)" => "Maximum routes (trains)",
        "Color compañía" => "Company colour",
        "Fundar (clic en el mapa):" => "Fund (click on the map):",
        "Compañías ordenadas por valor neto · performance trimestral" => {
            "Companies sorted by net worth · quarterly performance"
        }
        "Filtro: todos" => "Filter: all",
        "buscar…" => "search…",
        "Comprar vehículo" => "Buy vehicle",
        "(sin puntuaciones)" => "(no high scores)",
        "¿Salir de OpenTTDRS?" => "Exit OpenTTDRS?",
        "No hay noticias todavía." => "There is no news yet.",
        _ => source,
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use bevy::text::EditableText;

    use crate::settings::ClientPreferences;

    use super::{Locale, LocalizationPlugin, text};

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

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_updates_existing_and_late_window_text() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let initial = app.world_mut().spawn(Text::new("Noticias")).id();

        app.update();
        assert_eq!(
            app.world().get::<Text>(initial).unwrap().as_str(),
            "Noticias"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(app.world().get::<Text>(initial).unwrap().as_str(), "News");

        **app.world_mut().get_mut::<Text>(initial).unwrap() = "Noticias".into();
        app.update();
        assert_eq!(app.world().get::<Text>(initial).unwrap().as_str(), "News");

        let late = app
            .world_mut()
            .spawn(Text::new("No hay noticias todavía."))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "There is no news yet."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(initial).unwrap().as_str(),
            "Noticias"
        );
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "No hay noticias todavía."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_leaves_editable_player_text_alone() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        app.add_plugins(LocalizationPlugin);
        let input = app
            .world_mut()
            .spawn((Text::new("Noticias"), EditableText::new("Noticias")))
            .id();

        app.update();
        assert_eq!(app.world().get::<Text>(input).unwrap().as_str(), "Noticias");
    }
}
