//! Catálogo y sincronización de locale de las superficies UI del cliente.

use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::{GameTick, format_calendar_date};

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
        // OpenTTD persiste en su configuración el nombre del archivo `.lng`,
        // no sólo el ISO del encabezado. Aceptamos sus tres variantes inglesas
        // disponibles en 15.3, además de las variantes españolas equivalentes,
        // sin afirmar que ya soportamos todos los packs upstream.
        let pack_name = normalized
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(normalized.as_str());
        let english_pack = matches!(
            pack_name,
            "english"
                | "english.txt"
                | "english.lng"
                | "english_us.txt"
                | "english_us.lng"
                | "english_au.txt"
                | "english_au.lng"
        );
        if english_pack
            || normalized == "en"
            || normalized.starts_with("en-")
            || normalized.starts_with("en_")
        {
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

/// El catálogo se sincroniza antes de los sistemas que materializan texto
/// dinámico. Así una clave estática que fue reemplazada por una fila/título
/// con datos de la partida no puede volver a sobrescribir ese texto al final
/// del frame.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) struct LocalizationSet;

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
                .in_set(UpdateSet::Ui)
                .in_set(LocalizationSet),
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
        // Panel de órdenes: los controles estáticos pasan por el plugin y las
        // filas dinámicas consultan estas mismas claves al refrescarse.
        "Órdenes" => "Orders",
        "Horario" => "Timetable",
        "Modo carga" => "Loading mode",
        "Modo descarga" => "Unloading mode",
        "Paradas intermedias" => "Intermediate stops",
        "Posición andén" => "Platform position",
        "Parar depósito" => "Stop at depot",
        "Refit orden" => "Order refit",
        "Saltarse" => "Skip",
        "Eliminar" => "Delete",
        "Ir a" => "Go to",
        "Compartir" => "Share",
        "Desvincular" => "Unlink",
        "Pools" => "Shared pools",
        "Cond. >50%" => "If >50%",
        "Cond. <50%" => "If <50%",
        "Ciclar cond." => "Cycle condition",
        " · clic en parada" => " · click a stop",
        " · pool #" => " · shared pool #",
        "Sin órdenes — «Ir a» y clic en una parada del mapa." => {
            "No orders — “Go to” and click a stop on the map."
        }
        "Parada bus" => "Bus stop",
        "Parada carga" => "Truck stop",
        "Estacion tren" => "Train station",
        "Waypoint road" => "Road waypoint",
        "Muelle" => "Dock",
        "Boya" => "Buoy",
        "Aeropuerto" => "Airport",
        "Estación" => "Station",
        "Depósito vía (parar)" => "Rail depot (stop)",
        "Depósito vía (serv. si hace falta)" => "Rail depot (service if needed)",
        "Depósito (parar)" => "Depot (stop)",
        "Depósito (serv. si hace falta)" => "Depot (service if needed)",
        "Depósito" => "Depot",
        "Depósito vía" => "Rail depot",
        "Casilla" => "Tile",
        "Cond." => "If",
        "ord." => "order",
        "carga>" => "load>",
        "carga<" => "load<",
        "carga%" => "load%",
        "fiab" => "reliability",
        "vmax" => "max speed",
        "edad" => "age",
        "serv" => "service",
        "siempre" => "always",
        "vida" => "lifetime",
        "fiabmáx" => "max reliability",
        "marcha atrás" => "driving backwards",
        "cargar si posible" => "load if available",
        "carga completa" => "full load",
        "completar una carga" => "full load any cargo",
        "no cargar" => "no loading",
        "descargar si posible" => "unload if accepted",
        "descarga forzada" => "unload all",
        "transferir" => "transfer",
        "no descargar" => "no unloading",
        "sin paradas intermedias" => "non-stop",
        "paradas intermedias" => "with intermediate stops",
        "andén cercano" => "near platform end",
        "andén central" => "middle of platform",
        "andén lejano" => "far platform end",
        "parar" => "stop",
        "servicio" => "service",
        " · sin ruta por red" => " · no network route",
        " — incompatible: solo buses" => " — incompatible: buses only",
        " — incompatible: solo camiones/carga" => " — incompatible: trucks only",
        " — incompatible: solo barcos" => " — incompatible: ships only",
        " — incompatible: solo aviones" => " — incompatible: aircraft only",
        " — incompatible: solo trenes" => " — incompatible: trains only",
        " — incompatible: solo vehículos de carretera" => " — incompatible: road vehicles only",
        // Ventanas y controles reutilizables.
        "Trucos" => "Cheats",
        "Trucos..." => "Cheats...",
        "Singleplayer · Ctrl+Alt+C · consola: cheat …" => {
            "Singleplayer · Ctrl+Alt+C · console: cheat …"
        }
        "Dinero, año, bulldozer, compañía (Ctrl+Alt+C)" => {
            "Money, year, bulldozer, company (Ctrl+Alt+C)"
        }
        "Año−" => "Year−",
        "Año+" => "Year+",
        "Sin escenario GS activo" => "No active GS scenario",
        // Estado de la ventana de trucos: los valores se materializan en cada
        // frame, por lo que sus etiquetas se traducen antes de interpolarlos.
        "activado" => "enabled",
        "bulldozer" => "bulldozer",
        "dinero" => "money",
        "compañía" => "company",
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
        // Errores de comandos: se generan durante la partida y por eso no
        // pasan por un constructor de ventana que pueda traducirlos al crear
        // el HUD. Mantener sus claves aquí permite que el feedback se
        // actualice también cuando el jugador cambia de idioma en vivo.
        "Fuera del mapa." => "Outside the map.",
        "No se puede construir carretera en agua." => "Cannot build a road on water.",
        "No se puede construir carretera aquí." => "Cannot build a road here.",
        "No se puede construir vía en agua." => "Cannot build rail on water.",
        "No se puede construir vía aquí." => "Cannot build rail here.",
        "No se puede construir estación en agua." => "Cannot build a station on water.",
        "No se puede construir estación aquí." => "Cannot build a station here.",
        "La parada debe ir en hierba o bosque limpiable, no sobre carretera ni vía." => {
            "The stop must be on clearable grass or forest, not on a road or rail."
        }
        "La entrada debe dar a la carretera o vía en esa dirección." => {
            "The entrance must face the road or rail in that direction."
        }
        "Ya hay una estación en esta tesela." => "There is already a station on this tile.",
        "Este tipo de estación no permite ese número de andenes o longitud." => {
            "This station type does not allow that platform count or length."
        }
        "No hay estación en esta tesela." => "There is no station on this tile.",
        "Vehículo no encontrado." => "Vehicle not found.",
        "Ese vehículo pertenece a otra compañía." => "That vehicle belongs to another company.",
        "Esta infraestructura pertenece a otra compañía." => {
            "This infrastructure belongs to another company."
        }
        "Solo se puede vender un vehículo dentro de un depósito." => {
            "A vehicle can only be sold inside a depot."
        }
        "Ubicación de depósito inválida." => "Invalid depot location.",
        "Tipo de vehículo no permitido aquí." => "Vehicle type is not allowed here.",
        "Modelo de vehículo desconocido." => "Unknown vehicle model.",
        "No hay dinero suficiente." => "Insufficient funds.",
        "Parada incompatible con este vehículo." => "Stop is incompatible with this vehicle.",
        "Índice de orden inválido." => "Invalid order index.",
        "Ese ajuste solo aplica a paradas de estación." => {
            "That setting only applies to station stops."
        }
        "No hay depósito compatible en el mapa." => "No compatible depot found on the map.",
        "El nombre del vehículo es demasiado largo." => "Vehicle name is too long.",
        "El nombre de la estación es demasiado largo." => "Station name is too long.",
        "Solo se puede refit en depósito, sin carga y con un tipo compatible." => {
            "Refit is only possible in a depot, with no cargo, and with a compatible type."
        }
        "Ese ajuste de horario no aplica a esta orden." => {
            "That timetable setting does not apply to this order."
        }
        "Autoreemplazo no permitido para este vehículo o motor." => {
            "Autoreplace is not allowed for this vehicle or engine."
        }
        "No hay regla de autoreemplazo para ese motor." => {
            "No autoreplace rule exists for that engine."
        }
        "Grupo de vehículos no encontrado." => "Vehicle group not found.",
        "Nombre de grupo inválido." => "Invalid group name.",
        "Pool de órdenes compartidas no encontrado." => "Shared orders pool not found.",
        "El vehículo aún espera según el horario antes de salir del depósito." => {
            "The vehicle is still waiting according to its timetable before leaving the depot."
        }
        "Túnel inválido: entrada en pendiente inclinada (NE/SE/SW/NW) y salida al mismo nivel." => {
            "Invalid tunnel: entrance on a sloped tile (NE/SE/SW/NW) and exit at the same level."
        }
        "Este tipo de puente no está disponible (año, longitud o presupuesto)." => {
            "This bridge type is unavailable (year, length, or budget)."
        }
        "Puente inválido: las orillas al mismo nivel y agua o terreno más bajo bajo el tramo." => {
            "Invalid bridge: banks must be level with water or lower ground beneath the span."
        }
        "La vía no puede construirse en esta pendiente con esa geometría." => {
            "Rail cannot be built on this slope with that geometry."
        }
        "El waypoint solo puede colocarse sobre vía recta (eje X o Y)." => {
            "A waypoint can only be placed on straight rail (X or Y axis)."
        }
        "No hay vía que quitar aquí." => "There is no rail to remove here.",
        "No hay tranvía que quitar aquí." => "There is no tram track to remove here.",
        "No hay vía que convertir aquí." => "There is no rail to convert here.",
        "Hay un tren incompatible con ese tipo de vía." => {
            "A train is incompatible with that rail type."
        }
        "Este motor requiere vía electrificada (convertí la vía o el depósito)." => {
            "This engine requires electrified rail (convert the rail or depot)."
        }
        "Este motor requiere vía monorail (convertí la vía adyacente)." => {
            "This engine requires monorail (convert the adjacent rail)."
        }
        "Este motor requiere vía maglev (convertí la vía adyacente)." => {
            "This engine requires maglev (convert the adjacent rail)."
        }
        "La señal solo puede colocarse sobre vía recta (eje X o Y)." => {
            "A signal can only be placed on straight rail (X or Y axis)."
        }
        "Ya hay una señal en esa dirección." => "There is already a signal in that direction.",
        "Solo se puede modificar el terreno en hierba o bosque libre." => {
            "Terrain can only be modified on clear grass or forest."
        }
        "Demasiado alto: no se puede elevar más." => "Too high: terrain cannot be raised further.",
        "Demasiado bajo: ya está al nivel del mar." => "Too low: it is already at sea level.",
        "Pendiente inválida en el vecindario." => "Invalid slope in the neighbourhood.",
        "Esta tesela ya es terreno comprado." => "This tile is already owned land.",
        "Solo se puede comprar hierba o bosque libre (sin objetos ni infra)." => {
            "Only clear grass or forest can be purchased (no objects or infrastructure)."
        }
        "Solo se puede colocar el faro/transmisor en hierba o bosque libre." => {
            "The lighthouse/transmitter can only be placed on clear grass or forest."
        }
        "Ya hay un faro o transmisor de ese tipo en el mapa." => {
            "There is already a lighthouse or transmitter of that type on the map."
        }
        "Esta industria no está disponible en el clima de este mapa." => {
            "This industry is unavailable in this map's climate."
        }
        "No se puede construir una industria sobre otra industria existente." => {
            "An industry cannot be built on top of an existing industry."
        }
        "Hay una construcción que debe demolerse antes de ubicar la industria." => {
            "A building must be demolished before placing the industry."
        }
        "Esta industria solo puede construirse sobre edificios de un pueblo." => {
            "This industry can only be built on town buildings."
        }
        "El préstamo ya está al máximo permitido." => {
            "The loan is already at the allowed maximum."
        }
        "No hay préstamo suficiente para devolver." => "There is not enough loan to repay.",
        "Ciudad no encontrada." => "Town not found.",
        "Esa acción de autoridad no está disponible ahora." => {
            "That town authority action is not available now."
        }
        "No hay sitio libre para la estatua." => "There is no free space for the statue.",
        "No se puede fundar un pueblo aquí." => "A town cannot be founded here.",
        "Hay otro pueblo demasiado cerca." => "Another town is too close.",
        "Cheats desactivados (consola: cheat on)." => "Cheats are disabled (console: cheat on).",
        "Año de cheat inválido (1950–2450)." => "Invalid cheat year (1950–2450).",
        "Compañía no encontrada." => "Company not found.",
        "Ese color ya lo usa otra compañía." => "That colour is already used by another company.",
        "No puedes comprar tu propia compañía." => "You cannot buy your own company.",
        "La compañía no está en quiebra." => "The company is not bankrupt.",
        "La autoridad local no permite construir una estación aquí." => {
            "The local authority does not allow a station here."
        }
        "La autoridad local rechaza el aeropuerto: demasiado ruido." => {
            "The local authority rejects the airport: too much noise."
        }
        "No se puede plantar un árbol aquí." => "A tree cannot be planted here.",
        "No hay árbol ni cultivo en esta tesela." => "There is no tree or crop on this tile.",
        "Cartel no encontrado." => "Sign not found.",
        "El nombre del cartel es demasiado largo." => "Sign name is too long.",
        "El cartel necesita un nombre." => "The sign needs a name.",
        "No se pueden unir: road 1×1 adyacentes o rail (huella/eje) del mismo tipo." => {
            "Cannot join: adjacent 1×1 roads or rail (footprint/axis) of the same type."
        }
        "Índice NewGRF inválido." => "Invalid NewGRF index.",
        "Ese NewGRF es base y no se puede desactivar ni quitar." => {
            "That NewGRF is a base set and cannot be disabled or removed."
        }
        "Ya hay un NewGRF con ese GRFID." => "A NewGRF with that GRFID already exists.",
        "Entrada NewGRF inválida." => "Invalid NewGRF entry.",
        "Índice de parámetro NewGRF inválido." => "Invalid NewGRF parameter index.",
        "Esta parada NewGRF no admite bus o camión en esta herramienta." => {
            "This NewGRF stop does not support buses or trucks in this tool."
        }
        "Esta parada NewGRF no admite el tipo de vía actual (carretera/tranvía)." => {
            "This NewGRF stop does not support the current road type (road/tram)."
        }
        "Esta parada NewGRF solo admite colocación drive-through." => {
            "This NewGRF stop only supports drive-through placement."
        }
        "Un NewGRF denegó esta acción (callback)." => "A NewGRF denied this action (callback).",
        _ => source,
    }
}

/// Traduce una cadena de UI ya materializada sin perder los valores dinámicos.
/// Las claves desconocidas se conservan tal cual hasta que entren al catálogo.
#[must_use]
pub(crate) fn localized_text(locale: Locale, source: &str) -> String {
    text(locale, source).to_owned()
}

/// Formatea una fecha del simulador para la superficie UI activa.
///
/// El core conserva el formato español como dato canónico de sus noticias;
/// esta conversión sólo afecta las fechas materializadas por el cliente. Así
/// el cambio de locale no modifica ticks, saves ni texto producido por un
/// GameScript.
#[must_use]
pub(crate) fn localized_calendar_date(locale: Locale, tick: GameTick) -> String {
    let source = format_calendar_date(tick);
    if locale == Locale::Es {
        return source;
    }
    let mut parts = source.split_whitespace();
    let (Some(day), Some(month), Some(year)) = (parts.next(), parts.next(), parts.next()) else {
        return source;
    };
    if parts.next().is_some() {
        return source;
    }
    let month = match month {
        "ene" => "Jan",
        "feb" => "Feb",
        "mar" => "Mar",
        "abr" => "Apr",
        "may" => "May",
        "jun" => "Jun",
        "jul" => "Jul",
        "ago" => "Aug",
        "sep" => "Sep",
        "oct" => "Oct",
        "nov" => "Nov",
        "dic" => "Dec",
        _ => month,
    };
    format!("{day} {month} {year}")
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use bevy::text::EditableText;
    use openttdrs_core::GameTick;

    use crate::settings::ClientPreferences;

    use super::{Locale, LocalizationPlugin, localized_calendar_date, localized_text, text};

    #[test]
    fn locale_codes_and_openttd_pack_filenames_resolve_to_supported_locales() {
        assert_eq!(Locale::from_code("en"), Locale::En);
        assert_eq!(Locale::from_code("EN-us"), Locale::En);
        assert_eq!(Locale::from_code("es-AR"), Locale::Es);
        assert_eq!(Locale::from_code(" english.lng "), Locale::En);
        assert_eq!(Locale::from_code("lang/english_US.lng"), Locale::En);
        assert_eq!(Locale::from_code("english_AU.txt"), Locale::En);
        assert_eq!(Locale::from_code("spanish.lng"), Locale::Es);
        assert_eq!(Locale::from_code("spanish_MX.txt"), Locale::Es);
        assert_eq!(Locale::from_code("unknown"), Locale::Es);

        let prefs = ClientPreferences {
            language: "english.lng".into(),
            ..ClientPreferences::default()
        };
        assert_eq!(prefs.locale(), Locale::En);
    }

    #[test]
    fn catalog_translates_toolbar_text_without_changing_spanish() {
        assert_eq!(text(Locale::Es, "Guardar partida"), "Guardar partida");
        assert_eq!(text(Locale::En, "Guardar partida"), "Save game");
        assert_eq!(text(Locale::En, "Nueva partida"), "New game");
        assert_eq!(text(Locale::En, "Idioma"), "Language");
        assert_eq!(text(Locale::En, "Densidad de pueblos"), "Town density");
        assert_eq!(text(Locale::En, "Esc cancelar"), "Esc cancel");
        assert_eq!(text(Locale::En, "Horario"), "Timetable");
        assert_eq!(text(Locale::En, "Modo carga"), "Loading mode");
        assert_eq!(text(Locale::En, "Compartir"), "Share");
        assert_eq!(text(Locale::En, "Trucos..."), "Cheats...");
        assert_eq!(
            text(Locale::En, "Sin escenario GS activo"),
            "No active GS scenario"
        );
        assert_eq!(text(Locale::En, "untranslated"), "untranslated");
    }

    #[test]
    fn catalog_translates_runtime_command_errors() {
        assert_eq!(
            localized_text(Locale::En, "No se puede construir carretera en agua."),
            "Cannot build a road on water."
        );
        assert_eq!(
            localized_text(Locale::En, "No hay depósito compatible en el mapa."),
            "No compatible depot found on the map."
        );
        assert_eq!(
            localized_text(Locale::En, "Un NewGRF denegó esta acción (callback)."),
            "A NewGRF denied this action (callback)."
        );
    }

    #[test]
    fn calendar_date_is_localized_only_at_the_client_boundary() {
        assert_eq!(
            localized_calendar_date(Locale::Es, GameTick::new(0)),
            "1 ene 1950"
        );
        assert_eq!(
            localized_calendar_date(Locale::En, GameTick::new(0)),
            "1 Jan 1950"
        );
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
    fn localization_plugin_applies_openttd_pack_filenames_live() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences {
            language: "spanish.lng".into(),
            ..ClientPreferences::default()
        });
        app.add_plugins(LocalizationPlugin);
        let label = app.world_mut().spawn(Text::new("Noticias")).id();

        app.update();
        assert_eq!(app.world().get::<Text>(label).unwrap().as_str(), "Noticias");

        app.world_mut().resource_mut::<ClientPreferences>().language = "english.lng".into();
        app.update();
        assert_eq!(app.world().get::<Text>(label).unwrap().as_str(), "News");

        app.world_mut().resource_mut::<ClientPreferences>().language = "spanish_MX.lng".into();
        app.update();
        assert_eq!(app.world().get::<Text>(label).unwrap().as_str(), "Noticias");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_updates_cheat_and_goal_labels_live_without_touching_game_data() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let cheat_title = app.world_mut().spawn(Text::new("Trucos")).id();
        // Los títulos del GameScript son datos de la partida, no claves UI.
        let game_script_goal = app
            .world_mut()
            .spawn(Text::new("Meta de jugador: 12/20"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(cheat_title).unwrap().as_str(),
            "Cheats"
        );
        // La lista puede materializar su estado vacío después del cambio de
        // idioma; el registro tardío debe usar el locale actual.
        let goal_empty = app
            .world_mut()
            .spawn(Text::new("Sin escenario GS activo"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(goal_empty).unwrap().as_str(),
            "No active GS scenario"
        );
        assert_eq!(
            app.world().get::<Text>(game_script_goal).unwrap().as_str(),
            "Meta de jugador: 12/20"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(cheat_title).unwrap().as_str(),
            "Trucos"
        );
        assert_eq!(
            app.world().get::<Text>(goal_empty).unwrap().as_str(),
            "Sin escenario GS activo"
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
