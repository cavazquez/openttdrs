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
        if localized_text(Locale::En, source) != source {
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
        let translated = localized_text(locale, &key.0);
        if value.as_str() != translated {
            **value = translated;
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
        "Nombres de pueblos" => "Town names",
        "Nombres de estaciones" => "Station names",
        "Nombres de puntos de paso" => "Waypoint names",
        "Nombres de competidores" => "Competitor names",
        "Animación completa" => "Full animation",
        "Detalle completo" => "Full detail",
        "Reservas PBS" => "PBS reservations",
        "Overlay Link Graph" => "Link graph overlay",
        "Gizmos de depuración" => "Debug gizmos",
        "Overlay de diagnóstico" => "Diagnostics overlay",
        "Clásico" => "Classic",
        "Árboles" => "Trees",
        "Casas" => "Houses",
        "Edificios" => "Buildings",
        "Puentes" => "Bridges",
        "Estructuras" => "Structures",
        "Catenaria" => "Catenary",
        "Textos" => "Text",
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
        "Autoridad local" => "Local authority",
        "Autoridad" => "Authority",
        "Sin pueblo seleccionado." => "No town selected.",
        "Pueblo" => "Town",
        "Dinero" => "Money",
        "Rating compañía activa" => "Active company rating",
        "Ratings por compañía" => "Ratings by company",
        "Acciones de autoridad" => "Authority actions",
        "Publicidad pequeña" => "Small advertising campaign",
        "Publicidad mediana" => "Medium advertising campaign",
        "Publicidad grande" => "Large advertising campaign",
        "Reconstruir carreteras" => "Rebuild roads",
        "Construir estatua" => "Build statue",
        "Financiar edificios" => "Fund buildings",
        "Comprar derechos exclusivos" => "Buy exclusive rights",
        "Sobornar autoridad" => "Bribe authority",
        "Disponible" => "Available",
        "Sin fondos" => "Insufficient funds",
        "No disponible" => "Unavailable",
        "buscar pueblo…" => "search town…",
        "Población" => "Population",
        "No hay pueblos." => "No towns.",
        "Ningún pueblo coincide con el filtro." => "No towns match the filter.",
        "población" => "population",
        "autoridad" => "authority",
        "Loc" => "Center",
        "Pub" => "Advertise",
        "Fondos" => "Fund",
        "Aut." => "Auth.",
        "invierno" => "winter",
        "desierto" => "desert",
        "meta" => "goal",
        "sin metas activas en este clima" => "no active goals in this climate",
        "buena" => "good",
        "neutral" => "neutral",
        "mala" => "poor",
        "avanza el tiempo para ver series" => "time advances to show series",
        "Habitantes" => "Population",
        "Pasajeros" => "Passengers",
        "Correo" => "Mail",
        "Bienes" => "Goods",
        "Comida" => "Food",
        "Creciendo" => "Growing",
        "Financiación edificios" => "Building funding",
        "mes(es)" => "month(s)",
        "Servicio acumulado" => "Accumulated service",
        "partida" => "game",
        "Crecimiento financiado" => "Funded growth",
        "Metas de carga (mes anterior)" => "Cargo goals (previous month)",
        "Demanda teórica por ciclo (casas × tasa)" => {
            "Theoretical demand per cycle (houses × rate)"
        }
        "Pasajeros máx." => "Max passengers",
        "Correo máx." => "Max mail",
        "Lista de estaciones" => "Station list",
        "Todas" => "All",
        "Mía" => "Mine",
        "Tipo*" => "Type*",
        "Camión" => "Truck",
        "Aero" => "Air",
        "WP" => "WP",
        "Carga*" => "Cargo*",
        "Carbón" => "Coal",
        "Madera" => "Wood",
        "Petróleo" => "Oil",
        "Grano" => "Grain",
        "Acero" => "Steel",
        "Ganado" => "Livestock",
        "Valor" => "Valuables",
        "WP road" => "Road WP",
        "No hay estaciones con estos filtros." => "No stations match these filters.",
        "waiting" => "waiting",
        "Lista de subvenciones" => "Subsidy list",
        "Subvenciones" => "Subsidies",
        "Centrar origen" => "Center source",
        "Centrar destino" => "Center destination",
        "Abrir entidad" => "Open entity",
        "Historia" => "Story",
        "Directorio de industrias" => "Industry directory",
        "Producción industria" => "Industry production",
        "Producción" => "Production",
        "Nivel prod" => "Production level",
        "Rate ciclo" => "Cycle rate",
        "Producido total" => "Total produced",
        "Cargas" => "Cargo",
        "(sin cargo producido)" => "(no produced cargo)",
        "Sin industria seleccionada." => "No industry selected.",
        "Industria no encontrada." => "Industry not found.",
        "Posición:" => "Position:",
        "tiles" => "tiles",
        "Producción:" => "Production:",
        "cada" => "every",
        "Cadena:" => "Chain:",
        "Historial" => "History",
        "Historial mensual" => "Monthly history",
        "avanza el tiempo" => "time advances",
        "Producido" => "Produced",
        "Transport." => "Transported",
        "Industria sin datos de simulación" => "Industry without simulation data",
        "Tiles conectadas:" => "Connected tiles:",
        "sin sprite" => "without sprite",
        "Stub — gráfico mensual 15.3 residual (#269)." => {
            "Stub — residual 15.3 monthly graph (#269)."
        }
        "buscar industria…" => "search industry…",
        "Stock" => "Stock",
        "Carbon" => "Coal",
        "Mina de carbón" => "Coal mine",
        "Central eléctrica" => "Power station",
        "Mina de hierro" => "Iron ore mine",
        "Mina de cobre" => "Copper ore mine",
        "Mina de oro" => "Gold mine",
        "Mina de diamantes" => "Diamond mine",
        "Bosque" => "Forest",
        "Granja" => "Farm",
        "Granja tropical" => "Tropical farm",
        "Pozos petroleros" => "Oil wells",
        "Refinería" => "Oil refinery",
        "Fábrica" => "Factory",
        "Fábrica tropical" => "Tropical factory",
        "Aserradero" => "Sawmill",
        "Papelera" => "Paper mill",
        "Imprenta" => "Printing works",
        "Planta de alimentos" => "Food processing plant",
        "Plantación de fruta" => "Fruit plantation",
        "Plantación de caucho" => "Rubber plantation",
        "Suministro de agua" => "Water supply",
        "Torre de agua" => "Water tower",
        "Aserradero tropical" => "Tropical lumber mill",
        "Acería" => "Steel mill",
        "Banco" => "Bank",
        "Banco (ártico/trópico)" => "Bank (arctic/tropic)",
        "Algodón de azúcar" => "Cotton candy",
        "Fábrica de caramelos" => "Candy factory",
        "Granja de baterías" => "Battery farm",
        "Pozo de cola" => "Cola well",
        "Tienda de juguetes" => "Toy shop",
        "Fábrica de juguetes" => "Toy factory",
        "Fuente de plástico" => "Plastic fountain",
        "Fábrica de bebidas gaseosas" => "Fizzy drink factory",
        "Generador de burbujas" => "Bubble generator",
        "Cantera de toffee" => "Toffee quarry",
        "Mina de azúcar" => "Sugar mine",
        "No hay industrias." => "No industries.",
        "Ninguna industria coincide con el filtro." => "No industries match the filter.",
        "pasajeros" => "passengers",
        "carbón" => "coal",
        "correo" => "mail",
        "petróleo" => "oil",
        "ganado" => "livestock",
        "mercancías" => "goods",
        "grano" => "grain",
        "madera" => "wood",
        "mineral de hierro" => "iron ore",
        "acero" => "steel",
        "objetos de valor" => "valuables",
        "trigo" => "wheat",
        "papel" => "paper",
        "oro" => "gold",
        "comida" => "food",
        "caucho" => "rubber",
        "fruta" => "fruit",
        "maíz" => "maize",
        "mineral de cobre" => "copper ore",
        "agua" => "water",
        "diamantes" => "diamonds",
        "azúcar" => "sugar",
        "juguetes" => "toys",
        "baterías" => "batteries",
        "caramelos" => "candy",
        "cola" => "cola",
        "algodón de azúcar" => "cotton candy",
        "burbujas" => "bubbles",
        "plástico" => "plastic",
        "refrescos" => "fizzy drinks",
        "carga personalizada" => "custom cargo",
        "Hierro" => "Iron",
        "Oro" => "Gold",
        "Pozos" => "Oil wells",
        "Granja tropic" => "Tropical farm",
        "Cobre" => "Copper",
        "Fruta" => "Fruit",
        "Caucho" => "Rubber",
        "Alimentos" => "Food",
        "Diamantes" => "Diamonds",
        "Agua" => "Water",
        "Aserradero tropic" => "Tropical lumber mill",
        "Fábrica tropic" => "Tropical factory",
        "Algodón" => "Cotton candy",
        "Caramelos" => "Candy",
        "Baterías" => "Batteries",
        "Cola" => "Cola",
        "Juguetes" => "Toys",
        "Plástico" => "Plastic",
        "Gaseosa" => "Fizzy drinks",
        "Burbujas" => "Bubbles",
        "Toffee" => "Toffee",
        "Azúcar" => "Sugar",
        "Trenes" => "Trains",
        "Vehículos de carretera" => "Road vehicles",
        "Barcos" => "Ships",
        "Aviones" => "Aircraft",
        "Lista de trenes" => "Train list",
        "Lista de vehículos de carretera" => "Road vehicle list",
        "Lista de barcos" => "Ship list",
        "Lista de aviones" => "Aircraft list",
        "No hay trenes." => "No trains.",
        "No hay vehículos de carretera." => "No road vehicles.",
        "No hay barcos." => "No ships.",
        "No hay aviones." => "No aircraft.",
        "Ningún vehículo visita esta estación." => "No vehicle visits this station.",
        "estación" => "station",
        "Iniciar" => "Start",
        "Grupos" => "Groups",
        "Crear grupo" => "Create group",
        "Asignar" => "Assign",
        "Iniciar grupo" => "Start group",
        "Quitar filtro" => "Clear filter",
        "Grupo" => "Group",
        "Edad" => "Age",
        "Guardar" => "Save",
        "Finanzas" => "Finances",
        "Compañía" => "Company",
        "Pedir préstamo" => "Take loan",
        "Devolver préstamo" => "Repay loan",
        "Comprar rival (quiebra)" => "Buy rival (bankruptcy)",
        "IA…" => "AI…",
        "Préstamo" => "Loan",
        "Efectivo" => "Cash",
        "Patrimonio neto" => "Net worth",
        "cada operación" => "per operation",
        "Ingresos por transporte" => "Transport income",
        "Costes de explotación" => "Running costs",
        "Entregas" => "Deliveries",
        "unidades" => "units",
        "Infraestructura" => "Infrastructure",
        "Estaciones" => "Stations",
        "Vía" => "Rail",
        "teselas" => "tiles",
        "color" => "colour",
        "IA" => "AI",
        "vehículos" => "vehicles",
        "Averiado" => "Broken down",
        "Detenido" => "Stopped",
        "Sin ruta" => "No route",
        "Esperando señal" => "Waiting for signal",
        "Cargando" => "Loading",
        "Descargando" => "Unloading",
        "En marcha" => "Running",
        "sin órdenes" => "no orders",
        "Cond. → ord." => "Cond. → order",
        "Stub — Livery/ManagerFace/Infrastructure residual (#271)." => {
            "Stub — residual Livery/ManagerFace/Infrastructure (#271)."
        }
        "Ingresos" => "Income",
        "Beneficio operativo" => "Operating profit",
        "Valor de compañía" => "Company value",
        "Valor compañía" => "Company value",
        "Rendimiento" => "Performance",
        "Sin datos de" => "No data for",
        "Último" => "Last",
        "mensuales" => "monthly",
        "trimestrales" => "quarterly",
        "Tarifas de carga" => "Cargo payment rates",
        "Objetivos" => "Goals",
        "Liga" => "League",
        "Sonido y música" => "Sound and music",
        "Distribución de carga" => "Cargo distribution",
        "Manual: hop desde órdenes del vehículo.\nAsimétrica: Demand + MCF OpenTTD (Dijkstra distancia/capacidad).\nSimétrica: Demand Symmetric OpenTTD (geografía + supply) + MCF." => {
            "Manual: hop from vehicle orders.\nAsymmetric: OpenTTD Demand + MCF (distance/capacity Dijkstra).\nSymmetric: OpenTTD Demand Symmetric (geography + supply) + MCF."
        }
        "Asimétrica" => "Asymmetric",
        "Simétrica" => "Symmetric",
        "IA / TransCargo" => "AI / TransCargo",
        "Noticias" => "News",
        "Entrega de carga" => "Cargo delivery",
        "Primera entrega" => "First delivery",
        "Primer vehículo en marcha" => "First vehicle running",
        "Avisos de vehículo" => "Vehicle advice",
        "Accidentes" => "Accidents",
        "Compañías" => "Companies",
        "Cierre de industria" => "Industry closure",
        "Cartel" => "Newspaper",
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
        "Horario ON/OFF" => "Timetable ON/OFF",
        "Autorrelleno" => "Autofill",
        "Poner en hora" => "Reset lateness",
        "Ticks/Seg" => "Ticks/Sec",
        "Viaje" => "Travel",
        "esp." => "wait ",
        "viaje" => "travel",
        "condicional" => "conditional",
        "tarde" => "late",
        "adelantado" => "early",
        "en hora" => "on time",
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
        "Selección de parada" => "Road stop selection",
        "Clase" => "Class",
        "Vista previa" => "Preview",
        "Tipo" => "Type",
        "Parada" => "Stop",
        "Parada de autobús" => "Bus stop",
        "Parada de bus" => "Bus stop",
        "Parada de camión" => "Truck stop",
        "Estación de tren" => "Train station",
        "Selección de aeropuerto" => "Airport selection",
        "Destinos" => "Destinations",
        "Selección de objeto" => "Object selection",
        "Selección de puente" => "Bridge selection",
        "Objeto" => "Object",
        "Transmisor" => "Transmitter",
        "Faro" => "Lighthouse",
        "Seleccionado:" => "Selected:",
        "Carretera" => "Road",
        "Tren" => "Rail",
        "Barco" => "Ship",
        "Estación tren" => "Train station",
        "Waypoint" => "Waypoint",
        "Selección de estación" => "Station selection",
        "Clase / tipo (NewGRF)" => "Class / type (NewGRF)",
        "Orientación" => "Orientation",
        "Eje X" => "Axis X",
        "Eje Y" => "Axis Y",
        "Cartel de texto" => "Text sign",
        "Centrar" => "Center",
        "Renombrar" => "Rename",
        "Eliminar cartel" => "Delete sign",
        "Aplicar" => "Apply",
        "Clic en una fila para seleccionar:" => "Click a row to select:",
        "Eléc" => "Elec",
        "Tipo de vía: normal (construir / convertir)" => "Rail type: normal (build / convert)",
        "Tipo de vía: eléctrica (construir / convertir)" => {
            "Rail type: electric (build / convert)"
        }
        "Tipo de vía: monorail (construir / convertir)" => "Rail type: monorail (build / convert)",
        "Tipo de vía: maglev (construir / convertir)" => "Rail type: maglev (build / convert)",
        "Tipo de carretera (vanilla + NewGRF Action0/1/3)" => {
            "Road type (vanilla + NewGRF Action0/1/3)"
        }
        "Tipo de tranvía (vanilla + NewGRF Action0/1/3)" => {
            "Tram type (vanilla + NewGRF Action0/1/3)"
        }
        "Coloca la boya en agua navegable para abrir rutas." => {
            "Place the buoy in navigable water to open routes."
        }
        "Waypoint ferroviario" => "Rail waypoint",
        "Coloca el waypoint sobre vía férrea recta." => {
            "Place the waypoint on straight rail track."
        }
        "Waypoint de carretera" => "Road waypoint",
        "Coloca el waypoint sobre carretera recta." => "Place the waypoint on straight road.",
        "Arbolado" => "Trees",
        "Planta o hace crecer árboles en hierba / bosque." => {
            "Plant or grow trees on grass / forest."
        }
        "Elevar, bajar, nivelar o comprar terreno con la herramienta activa." => {
            "Raise, lower, level or buy land with the active tool."
        }
        "Coloca un cartel de texto en el mapa." => "Place a text sign on the map.",
        "Número de andenes" => "Number of platforms",
        "Longitud de andén" => "Platform length",
        "Mostrar área de cobertura" => "Show coverage area",
        "Desactivado" => "Off",
        "Activado" => "On",
        "Cobertura" => "Coverage",
        "Cobertura:" => "Coverage:",
        "oculta" => "hidden",
        "casas" => "houses",
        "stock ind." => "industry stock",
        "apunta al mapa" => "point to the map",
        "Acepta:" => "Accepts:",
        "Suministra:" => "Supplies:",
        "Nada" => "Nothing",
        "Plataforma petrolera" => "Oil rig",
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
        "Depósito de Trenes" => "Train depot",
        "Depósito de Barcos" => "Ship depot",
        "Depósito de Carretera" => "Road depot",
        "Hangar de Aviones" => "Aircraft hangar",
        "Detalles" => "Details",
        "Capacidad" => "Capacity",
        "Totales" => "Totals",
        "Cualquiera" => "Any",
        "Peso" => "Weight",
        "Potencia" => "Power",
        "Beneficio este año" => "Profit this year",
        "renovar" => "renew",
        "depósito" => "depot",
        "fiab." => "rel.",
        "año" => "year",
        "Velocidad" => "Speed",
        "máx." => "max.",
        "Activa" => "Active",
        "Vender" => "Sell",
        "Cadena" => "Chain",
        "Nuevos" => "New",
        "Clonar" => "Clone",
        "Deseng." => "Detach",
        "Ruta" => "Route",
        "Órd." => "Ord.",
        "Nom." => "Name",
        "Carga" => "Cargo",
        "Unir" => "Join",
        "Cerrar" => "Close",
        "Añadir a ruta del vehículo" => "Add to vehicle route",
        "Editar órdenes" => "Edit orders",
        "Centrar cámara en la estación" => "Center camera on station",
        "Renombrar estación" => "Rename station",
        "Ver vehículos que visitan esta estación" => "View vehicles visiting this station",
        "Filtrar carga: todas / con espera / aceptadas" => "Filter cargo: all / waiting / accepted",
        "Unir con otra estación" => "Join another station",
        "todas" => "all",
        "con espera" => "waiting",
        "aceptadas" => "accepted",
        "ninguna" => "none",
        "ingresos" => "income",
        "activo" => "active",
        "Cargas en espera" => "Cargo waiting",
        "Vehículos en ruta" => "Vehicles en route",
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
        "Selecciona una entrada del stack." => "Select an entry in the stack.",
        "Quitar" => "Remove",
        "Añadir…" => "Add…",
        "Inspeccionar" => "Inspect",
        "Pools de órdenes compartidas." => "Shared order pools.",
        "Órdenes compartidas" => "Shared orders",
        "Pools existentes. Abre desde Órdenes → Pools para vincular." => {
            "Existing pools. Open from Orders → Pools to link."
        }
        "Vincular vehículo" => "Link vehicle",
        "Señales" => "Signals",
        "Bloque" => "Block",
        "Entrada" => "Entry",
        "Salida" => "Exit",
        "Combinada" => "Combo",
        "Ruta PBS" => "Path",
        "Ruta 1vía" => "One-way path",
        "Eléctrica" => "Electric",
        "Semáforo" => "Semaphore",
        "densidad" => "density",
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
        "Vehículos" => "Vehicles",
        "Ambiente" => "Ambient",
        "Desastres" => "Disasters",
        "Confirmación" => "Confirmation",
        "Clic toolbar" => "Toolbar click",
        "◀ Ant." => "◀ Prev.",
        "Sig. ▶" => "Next ▶",
        "Reproducir" => "Play",
        "Detener" => "Stop",
        "Reproduciendo" => "Playing",
        "Espera ante path sin reserva (días). 255 = nunca girar." => {
            "Wait for path without reservation (days). 255 = never turn around."
        }
        "Señales PBS" => "PBS signals",
        "Espera" => "Wait",
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
        "Tamaño:" => "Size:",
        "Tamaño: —" => "Size: —",
        "Cobertura: —" => "Coverage: —",
        "Preferencias de cliente (se guardan al salir)" => "Client preferences (saved on exit)",
        "Presets de cliente" => "Client presets",
        "Transparencia / invisibilidad (TO_*)" => "Transparency / invisibility (TO_*)",
        "Elige un destino para añadirlo a la ruta." => "Choose a destination to add to the route.",
        "Elegir en el mapa" => "Choose on map",
        "Elige el tipo de carga." => "Choose the cargo type.",
        "Refit solo en depósito, sin carga y con tipos alternativos." => {
            "Refit requires a depot, no cargo and alternative types."
        }
        "Unidades" => "Units",
        "Cap. resultante" => "Resulting capacity",
        "Clic en unidad para seleccionar; clic en carga para aplicar." => {
            "Click a unit to select; click cargo to apply."
        }
        "Coste" => "Cost",
        "gratis" => "free",
        "Clic en una carga de la lista para aplicar." => "Click a cargo in the list to apply.",
        "cap." => "cap.",
        "Nombre:" => "Name:",
        "Sigue la cámara principal (zoom más alejado)." => {
            "Follows the main camera (more zoomed out)."
        }
        "Stock: --" => "Stock: --",
        "Silencio" => "Off",
        "Resumen" => "Summary",
        "Completo" => "Full",
        "Silencio = sin noticias · Resumen = ticker · Completo = periódico" => {
            "Off = silence · Summary = ticker · Full = newspaper"
        }
        "Fin de partida" => "End of game",
        "Menú principal" => "Main menu",
        "capas" => "layers",
        "Clic en fila: seleccionar y centrar origen" => "Click a row: select and center its origin",
        "No hay subvenciones activas ni ofertas." => "There are no active subsidies or offers.",
        "Industria" => "Industry",
        "Finanzas…" => "Finances…",
        "Reglas de autoreemplazo." => "Autoreplace rules.",
        "Reglas" => "Rules",
        "Desde" => "From",
        "Hacia" => "To",
        "Añadir" => "Add",
        "Solo viejos" => "Only old",
        "Borrar" => "Clear",
        "Aplicar depósito" => "Apply depot",
        "Sin páginas de historia." => "No story pages.",
        "Sin historia" => "No story",
        "Este escenario no tiene páginas Story (GS demo desactivado)." => {
            "This scenario has no Story pages (GS demo disabled)."
        }
        "Anterior" => "Previous",
        "Siguiente" => "Next",
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
        "Sin compañías" => "No companies",
        "Filtro: todos" => "Filter: all",
        "buscar…" => "search…",
        "Nuevos vehículos" => "New vehicles",
        "Nuevos vehículos ferroviarios" => "New rail vehicles",
        "Nuevos barcos" => "New ships",
        "Nuevos helicópteros" => "New helicopters",
        "Nuevos aviones" => "New aircraft",
        "Nuevos vehículos de carretera" => "New road vehicles",
        "Nombre" => "Name",
        "Precio" => "Price",
        "vagón" => "wagon",
        "enganchar a locomotora" => "attach to locomotive",
        "locomotora" => "locomotive",
        "metadatos" => "metadata",
        "sin sprites" => "no sprites",
        "Coste de operación" => "Running cost",
        "Diseñado" => "Introduced",
        "Fiabilidad" => "Reliability",
        "nada (solo locomotora)" => "none (locomotive only)",
        "Vel." => "Speed",
        "Año" => "Year",
        "Todos" => "All",
        "Buses" => "Buses",
        "Camiones" => "Trucks",
        "Tranvías" => "Trams",
        "Locomotoras" => "Locomotives",
        "Vagones" => "Wagons",
        "Comprar vehículo" => "Buy vehicle",
        "(sin puntuaciones)" => "(no high scores)",
        "¿Salir de OpenTTDRS?" => "Exit OpenTTDRS?",
        "No hay noticias todavía." => "There is no news yet.",
        "Comienza una recesión económica" => "An economic recession begins",
        "La demanda de carga y la producción industrial se reducirán." => {
            "Cargo demand and industrial production will decrease."
        }
        "La recesión ha terminado" => "The recession has ended",
        "La economía vuelve a la normalidad." => "The economy returns to normal.",
        "OVNI pequeño avistado" => "Small UFO sighted",
        "OVNI enorme avistado" => "Large UFO sighted",
        "Accidente aéreo" => "Aircraft accident",
        "Accidente de helicóptero" => "Helicopter accident",
        "Submarino a la deriva" => "Submarine adrift",
        "Hundimiento minero" => "Mine subsidence",
        "Dejará de producir y desaparecerá el mes que viene." => {
            "It will stop producing and disappear next month."
        }
        "¡Tu primer autobús está en marcha!" => "Your first bus is running!",
        "¡Tu primer camión está en marcha!" => "Your first truck is running!",
        "¡Tu primer tranvía está en marcha!" => "Your first tram is running!",
        "¡Tu primer tren está en marcha!" => "Your first train is running!",
        "¡Tu primer barco está en marcha!" => "Your first ship is underway!",
        "¡Tu primer avión está en marcha!" => "Your first aircraft is in the air!",
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
        "No se puede demoler: hay un vehículo ocupando la tesela." => {
            "Cannot demolish: a vehicle is occupying the tile."
        }
        "Ubicación de depósito inválida." => "Invalid depot location.",
        "No quedan identificadores de depósito disponibles." => {
            "No depot identifiers are available."
        }
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
        "Autoreemplazo" => "Autoreplace",
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
        "El puente queda demasiado bajo para esta parada NewGRF." => {
            "The bridge is too low for this NewGRF stop."
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
        "Este objeto no se puede demoler sin el bulldozer mágico." => {
            "This object cannot be demolished without the magic bulldozer."
        }
        "Hay un objeto que debe demolerse antes de construir aquí." => {
            "An object must be demolished before building here."
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
        "Esta industria marítima solo puede construirse sobre agua abierta." => {
            "This maritime industry can only be built on open water."
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
        "El layout seleccionado no existe para este aeropuerto NewGRF." => {
            "The selected layout does not exist for this NewGRF airport."
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
    let translated = text(locale, source);
    if translated != source {
        return translated.to_owned();
    }
    if locale == Locale::En {
        return translate_dynamic_news_text(source).unwrap_or_else(|| source.to_owned());
    }
    source.to_owned()
}

/// Traduce sólo plantillas de noticias emitidas por el core cuya forma y
/// valores dinámicos son identificables sin interpretar texto arbitrario.
/// Nombres de cargo, importes e IDs se conservan byte a byte.
fn translate_dynamic_news_text(source: &str) -> Option<String> {
    if let Some(rest) = source.strip_prefix("Entrega de ")
        && let Some((units, cargo)) = rest.split_once(" u. de ")
        && is_ascii_digits(units)
        && is_news_fragment(cargo)
    {
        return Some(format!("Delivery of {units} units of {cargo}"));
    }
    if let Some(rest) = source.strip_prefix("¡Primera entrega! ")
        && let Some((units, cargo)) = rest.split_once(" u. de ")
        && is_ascii_digits(units)
        && is_news_fragment(cargo)
    {
        return Some(format!("First delivery! {units} units of {cargo}"));
    }
    if let Some(rest) = source.strip_prefix("Tu compañía ha cobrado ")
        && let Some((money, cargo)) = rest.split_once(" por transportar ")
        && let Some(cargo) = cargo.strip_suffix('.')
        && is_money_token(money)
        && is_news_fragment(cargo)
    {
        return Some(format!(
            "Your company earned {money} for transporting {cargo}."
        ));
    }
    if let Some(rest) = source.strip_prefix("El vehículo ")
        && let Some(vehicle_id) = rest.strip_suffix(" ha salido a operar.")
        && is_ascii_digits(vehicle_id)
    {
        return Some(format!("Vehicle {vehicle_id} has started operating."));
    }
    if let Some(cargo) = source.strip_prefix("Subvención: ")
        && is_news_fragment(cargo)
    {
        return Some(format!("Subsidy: {cargo}"));
    }
    if let Some(cargo) = source.strip_prefix("Subvención adjudicada: ")
        && is_news_fragment(cargo)
    {
        return Some(format!("Subsidy awarded: {cargo}"));
    }
    if let Some(rest) = source.strip_prefix("Transportar ")
        && let Some((cargo, rest)) = rest.split_once(" desde (")
        && let Some((source_coord, destination_coord)) = rest
            .strip_suffix(").")
            .and_then(|value| value.split_once(") hacia la estación ("))
        && is_news_fragment(cargo)
        && is_coordinate_pair(source_coord)
        && is_coordinate_pair(destination_coord)
    {
        return Some(format!(
            "Transport {cargo} from ({source_coord}) to station ({destination_coord})."
        ));
    }
    if let Some(rest) = source.strip_prefix("«")
        && let Some((company, rest)) = rest.split_once("» se adjudica el transporte de ")
        && let Some(cargo) = rest.strip_suffix(" (pago ×2).")
        && is_news_fragment(company)
        && is_news_fragment(cargo)
    {
        return Some(format!(
            "«{company}» wins the {cargo} transport contract (payment ×2)."
        ));
    }
    if let Some(company) = source.strip_prefix("Logro rival: ")
        && is_news_fragment(company)
    {
        return Some(format!("Rival achievement: {company}"));
    }
    if let Some(rest) = source.strip_prefix("«")
        && let Some((company, goal)) = rest.split_once("» cumplió el objetivo: ")
        && is_news_fragment(company)
        && is_news_fragment(goal)
    {
        return Some(format!("«{company}» completed the goal: {goal}"));
    }
    if let Some(company) = source.strip_prefix("Quiebra: ")
        && is_news_fragment(company)
    {
        return Some(format!("Bankruptcy: {company}"));
    }
    if let Some(rest) = source.strip_prefix("La compañía «")
        && let Some((company, rest)) = rest.split_once("» está en quiebra (mes ")
        && let Some((month, limit)) = rest
            .strip_suffix(").")
            .and_then(|value| value.split_once('/'))
        && is_news_fragment(company)
        && is_ascii_digits(month)
        && is_ascii_digits(limit)
    {
        return Some(format!(
            "Company «{company}» is bankrupt (month {month}/{limit})."
        ));
    }
    if let Some(company) = source.strip_prefix("Comprada ")
        && is_news_fragment(company)
    {
        return Some(format!("Company bought {company}"));
    }
    if let Some(rest) = source.strip_prefix("La compañía «")
        && let Some((company, price)) = rest.split_once("» fue adquirida por £")
        && let Some(price) = price.strip_suffix('.')
        && is_news_fragment(company)
        && is_ascii_digits(price)
    {
        return Some(format!("Company «{company}» was acquired for £{price}."));
    }
    if let Some(vehicle_id) = source
        .strip_prefix("Autoreemplazo falló (vehículo ")
        .and_then(|value| value.strip_suffix(')'))
        && is_ascii_digits(vehicle_id)
    {
        return Some(format!("Autoreplace failed (vehicle {vehicle_id})"));
    }
    if let Some(coordinates) = source
        .strip_prefix("Industria en (")
        .and_then(|value| value.strip_suffix(") anuncia su cierre"))
        && is_coordinate_pair(coordinates)
    {
        return Some(format!("Industry at ({coordinates}) announces its closure"));
    }
    if let Some(coordinates) = source
        .strip_prefix("Industria cerrada en (")
        .and_then(|value| value.strip_suffix(')'))
        && is_coordinate_pair(coordinates)
    {
        return Some(format!("Industry closed at ({coordinates})"));
    }
    for (spanish_prefix, spanish_suffix, english_prefix) in [
        (
            "Un OVNI pequeño se aproxima a (",
            ").",
            "A small UFO is approaching (",
        ),
        (
            "Un OVNI enorme se aproxima a (",
            ").",
            "A large UFO is approaching (",
        ),
        (
            "Un avión se estrella cerca de (",
            ").",
            "An aircraft crashes near (",
        ),
        (
            "Un helicóptero se estrella en (",
            ").",
            "A helicopter crashes at (",
        ),
        (
            "Un submarino provoca daños en (",
            ").",
            "A submarine causes damage at (",
        ),
        (
            "Un hundimiento en mina afecta (",
            ").",
            "A mine subsidence affects (",
        ),
    ] {
        if let Some(coordinates) = source
            .strip_prefix(spanish_prefix)
            .and_then(|value| value.strip_suffix(spanish_suffix))
            && is_coordinate_pair(coordinates)
        {
            return Some(format!("{english_prefix}{coordinates})."));
        }
    }
    if let Some(rest) = source.strip_prefix("Sin ruta por red: vehículo ")
        && let Some((vehicle_id, order)) = rest
            .strip_suffix(')')
            .and_then(|value| value.split_once(" (orden "))
        && is_ascii_digits(vehicle_id)
        && is_ascii_digits(order)
    {
        return Some(format!(
            "No network route: vehicle {vehicle_id} (order {order})"
        ));
    }
    for (spanish_prefix, english_prefix) in [
        ("Sin órdenes: vehículo ", "No orders: vehicle "),
        (
            "Parada incompatible: vehículo ",
            "Incompatible stop: vehicle ",
        ),
        (
            "Sin carga disponible: vehículo ",
            "No cargo available: vehicle ",
        ),
        (
            "Sin camino reservado: vehículo ",
            "No reserved path: vehicle ",
        ),
    ] {
        if let Some(vehicle_id) = source.strip_prefix(spanish_prefix)
            && is_ascii_digits(vehicle_id)
        {
            return Some(format!("{english_prefix}{vehicle_id}"));
        }
    }
    if let Some(victims) = source
        .strip_prefix("Choque de trenes (")
        .and_then(|value| value.strip_suffix(" víctimas)"))
        && is_ascii_digits(victims)
    {
        return Some(format!("Train collision ({victims} victims)"));
    }
    if let Some(rest) = source.strip_prefix("Los trenes #")
        && let Some((first, rest)) = rest.split_once(" y #")
        && let Some((second, coordinates)) = rest
            .strip_suffix(").")
            .and_then(|value| value.split_once(" colisionaron en ("))
        && is_ascii_digits(first)
        && is_ascii_digits(second)
        && is_coordinate_pair(coordinates)
    {
        return Some(format!(
            "Trains #{first} and #{second} collided at ({coordinates})."
        ));
    }
    if let Some(coordinates) = source
        .strip_prefix("Un jet intentó aterrizar en pista corta en (")
        .and_then(|value| value.strip_suffix(")."))
        && is_coordinate_pair(coordinates)
    {
        return Some(format!(
            "A jet attempted to land on a short runway at ({coordinates})."
        ));
    }
    if let Some(vehicle_id) = source
        .strip_prefix("Choque en paso a nivel (vehículo #")
        .and_then(|value| value.strip_suffix(')'))
        && is_ascii_digits(vehicle_id)
    {
        return Some(format!("Level crossing crash (vehicle #{vehicle_id})"));
    }
    if let Some(coordinates) = source
        .strip_prefix("Un vehículo de carretera chocó con un tren en (")
        .and_then(|value| value.strip_suffix(")."))
        && is_coordinate_pair(coordinates)
    {
        return Some(format!(
            "A road vehicle collided with a train at ({coordinates})."
        ));
    }
    if let Some(coordinates) = source
        .strip_prefix("Un vehículo quedó bajo el agua en (")
        .and_then(|value| value.strip_suffix(")."))
        && is_coordinate_pair(coordinates)
    {
        return Some(format!("A vehicle was flooded at ({coordinates})."));
    }
    if let Some(name) = source.strip_suffix(" se estrelló al aterrizar")
        && is_news_fragment(name)
    {
        return Some(format!("{name} crashed while landing"));
    }
    if let Some(name) = source.strip_suffix(" inundado")
        && is_news_fragment(name)
    {
        return Some(format!("{name} flooded"));
    }
    None
}

fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_money_token(value: &str) -> bool {
    let digits = value.strip_prefix("$").or_else(|| value.strip_prefix("-$"));
    digits.is_some_and(|digits| {
        !digits.is_empty()
            && digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.' || byte == b'M' || byte == b'K')
    })
}

fn is_coordinate_pair(value: &str) -> bool {
    let Some((x, y)) = value.split_once(", ") else {
        return false;
    };
    x.parse::<i32>().is_ok() && y.parse::<i32>().is_ok()
}

fn is_news_fragment(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|character| matches!(character, '\n' | '\r' | '⟦' | '⟧' | '(' | ')'))
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
        assert_eq!(
            localized_text(
                Locale::En,
                "Este objeto no se puede demoler sin el bulldozer mágico.",
            ),
            "This object cannot be demolished without the magic bulldozer."
        );
        assert_eq!(
            localized_text(
                Locale::En,
                "Hay un objeto que debe demolerse antes de construir aquí."
            ),
            "An object must be demolished before building here."
        );
    }

    #[test]
    fn catalog_translates_all_news_settings_categories() {
        for (spanish, english) in [
            ("Entrega de carga", "Cargo delivery"),
            ("Primera entrega", "First delivery"),
            ("Primer vehículo en marcha", "First vehicle running"),
            ("Avisos de vehículo", "Vehicle advice"),
            ("Accidentes", "Accidents"),
            ("Compañías", "Companies"),
            ("Pedir préstamo", "Take loan"),
            ("Devolver préstamo", "Repay loan"),
            ("Comprar rival (quiebra)", "Buy rival (bankruptcy)"),
            ("IA…", "AI…"),
            ("Efectivo", "Cash"),
            ("Patrimonio neto", "Net worth"),
            ("cada operación", "per operation"),
            ("Ingresos por transporte", "Transport income"),
            ("Costes de explotación", "Running costs"),
            ("Entregas", "Deliveries"),
            ("unidades", "units"),
            ("Infraestructura", "Infrastructure"),
            ("Estaciones", "Stations"),
            ("Vía", "Rail"),
            ("teselas", "tiles"),
            ("color", "colour"),
            ("IA", "AI"),
            ("Ingresos", "Income"),
            ("Beneficio operativo", "Operating profit"),
            ("Valor compañía", "Company value"),
            ("Rendimiento", "Performance"),
            ("Sin datos de", "No data for"),
            ("Último", "Last"),
            ("mensuales", "monthly"),
            ("trimestrales", "quarterly"),
            ("Cierre de industria", "Industry closure"),
            ("Economía", "Economy"),
            ("Cartel", "Newspaper"),
            ("Lista de trenes", "Train list"),
            ("Lista de vehículos de carretera", "Road vehicle list"),
            ("Lista de barcos", "Ship list"),
            ("Lista de aviones", "Aircraft list"),
            ("No hay trenes.", "No trains."),
            ("No hay vehículos de carretera.", "No road vehicles."),
            ("No hay barcos.", "No ships."),
            ("No hay aviones.", "No aircraft."),
            (
                "Ningún vehículo visita esta estación.",
                "No vehicle visits this station.",
            ),
            ("estación", "station"),
            ("Iniciar", "Start"),
            ("Grupos", "Groups"),
            ("Crear grupo", "Create group"),
            ("Asignar", "Assign"),
            ("Iniciar grupo", "Start group"),
            ("Quitar filtro", "Clear filter"),
            ("Grupo", "Group"),
            ("Edad", "Age"),
            ("Guardar", "Save"),
            ("Silencio", "Off"),
            ("Resumen", "Summary"),
            ("Completo", "Full"),
            ("Nuevos vehículos", "New vehicles"),
            ("Nuevos vehículos ferroviarios", "New rail vehicles"),
            ("Nuevos barcos", "New ships"),
            ("Nuevos helicópteros", "New helicopters"),
            ("Nuevos aviones", "New aircraft"),
            ("Nuevos vehículos de carretera", "New road vehicles"),
            ("Averiado", "Broken down"),
            ("Detenido", "Stopped"),
            ("Sin ruta", "No route"),
            ("Esperando señal", "Waiting for signal"),
            ("Cargando", "Loading"),
            ("Descargando", "Unloading"),
            ("En marcha", "Running"),
            ("sin órdenes", "no orders"),
            ("Cond. → ord.", "Cond. → order"),
            ("Nombre", "Name"),
            ("Precio", "Price"),
            ("vagón", "wagon"),
            ("enganchar a locomotora", "attach to locomotive"),
            ("locomotora", "locomotive"),
            ("metadatos", "metadata"),
            ("sin sprites", "no sprites"),
            ("Coste de operación", "Running cost"),
            ("Diseñado", "Introduced"),
            ("Fiabilidad", "Reliability"),
            ("nada (solo locomotora)", "none (locomotive only)"),
            ("Vel.", "Speed"),
            ("Año", "Year"),
            ("Todos", "All"),
            ("Buses", "Buses"),
            ("Camiones", "Trucks"),
            ("Tranvías", "Trams"),
            ("Locomotoras", "Locomotives"),
            ("Vagones", "Wagons"),
            ("Selección de parada", "Road stop selection"),
            ("Clase", "Class"),
            ("Vista previa", "Preview"),
            ("Tipo", "Type"),
            ("Parada", "Stop"),
            ("Parada de autobús", "Bus stop"),
            ("Selección de aeropuerto", "Airport selection"),
            ("Destinos", "Destinations"),
            (
                "Refit solo en depósito, sin carga y con tipos alternativos.",
                "Refit requires a depot, no cargo and alternative types.",
            ),
            ("Unidades", "Units"),
            ("Cap. resultante", "Resulting capacity"),
            (
                "Clic en unidad para seleccionar; clic en carga para aplicar.",
                "Click a unit to select; click cargo to apply.",
            ),
            ("Coste", "Cost"),
            ("gratis", "free"),
            (
                "Clic en una carga de la lista para aplicar.",
                "Click a cargo in the list to apply.",
            ),
            ("cap.", "cap."),
            ("Detalles", "Details"),
            ("Capacidad", "Capacity"),
            ("Totales", "Totals"),
            ("Cualquiera", "Any"),
            ("Peso", "Weight"),
            ("Potencia", "Power"),
            ("Beneficio este año", "Profit this year"),
            ("renovar", "renew"),
            ("depósito", "depot"),
            ("fiab.", "rel."),
            ("año", "year"),
            ("Velocidad", "Speed"),
            ("máx.", "max."),
            ("Activa", "Active"),
            ("Selección de objeto", "Object selection"),
            ("Selección de puente", "Bridge selection"),
            ("Objeto", "Object"),
            ("Transmisor", "Transmitter"),
            ("Faro", "Lighthouse"),
            ("Seleccionado:", "Selected:"),
            ("Carretera", "Road"),
            ("Tren", "Rail"),
            ("Barco", "Ship"),
            ("Estación tren", "Train station"),
            ("Waypoint", "Waypoint"),
            ("Selección de estación", "Station selection"),
            ("Clase / tipo (NewGRF)", "Class / type (NewGRF)"),
            ("Orientación", "Orientation"),
            ("Eje X", "Axis X"),
            ("Eje Y", "Axis Y"),
            ("Cartel de texto", "Text sign"),
            ("Centrar", "Center"),
            ("Renombrar", "Rename"),
            ("Eliminar cartel", "Delete sign"),
            ("Aplicar", "Apply"),
            (
                "Clic en una fila para seleccionar:",
                "Click a row to select:",
            ),
            ("Eléc", "Elec"),
            (
                "Tipo de vía: normal (construir / convertir)",
                "Rail type: normal (build / convert)",
            ),
            (
                "Tipo de vía: eléctrica (construir / convertir)",
                "Rail type: electric (build / convert)",
            ),
            (
                "Tipo de vía: monorail (construir / convertir)",
                "Rail type: monorail (build / convert)",
            ),
            (
                "Tipo de vía: maglev (construir / convertir)",
                "Rail type: maglev (build / convert)",
            ),
            (
                "Tipo de carretera (vanilla + NewGRF Action0/1/3)",
                "Road type (vanilla + NewGRF Action0/1/3)",
            ),
            (
                "Tipo de tranvía (vanilla + NewGRF Action0/1/3)",
                "Tram type (vanilla + NewGRF Action0/1/3)",
            ),
            (
                "Coloca la boya en agua navegable para abrir rutas.",
                "Place the buoy in navigable water to open routes.",
            ),
            ("Waypoint ferroviario", "Rail waypoint"),
            (
                "Coloca el waypoint sobre vía férrea recta.",
                "Place the waypoint on straight rail track.",
            ),
            ("Waypoint de carretera", "Road waypoint"),
            (
                "Coloca el waypoint sobre carretera recta.",
                "Place the waypoint on straight road.",
            ),
            ("Arbolado", "Trees"),
            (
                "Planta o hace crecer árboles en hierba / bosque.",
                "Plant or grow trees on grass / forest.",
            ),
            (
                "Elevar, bajar, nivelar o comprar terreno con la herramienta activa.",
                "Raise, lower, level or buy land with the active tool.",
            ),
            (
                "Coloca un cartel de texto en el mapa.",
                "Place a text sign on the map.",
            ),
            ("Número de andenes", "Number of platforms"),
            ("Longitud de andén", "Platform length"),
            ("Mostrar área de cobertura", "Show coverage area"),
            ("Desactivado", "Off"),
            ("Activado", "On"),
            ("Cobertura", "Coverage"),
            ("Cobertura:", "Coverage:"),
            ("oculta", "hidden"),
            ("casas", "houses"),
            ("stock ind.", "industry stock"),
            ("apunta al mapa", "point to the map"),
            ("Acepta:", "Accepts:"),
            ("Suministra:", "Supplies:"),
            ("Nada", "Nothing"),
            ("Lista de estaciones", "Station list"),
            ("Todas", "All"),
            ("Mía", "Mine"),
            ("Tipo*", "Type*"),
            ("Camión", "Truck"),
            ("Tren", "Rail"),
            ("Muelle", "Dock"),
            ("Aero", "Air"),
            ("Carga*", "Cargo*"),
            ("Carbón", "Coal"),
            ("Madera", "Wood"),
            ("Petróleo", "Oil"),
            ("Grano", "Grain"),
            ("Hierro", "Iron"),
            ("Acero", "Steel"),
            ("Ganado", "Livestock"),
            ("Valor", "Valuables"),
            ("WP road", "Road WP"),
            ("Aeropuerto", "Airport"),
            (
                "No hay estaciones con estos filtros.",
                "No stations match these filters.",
            ),
            ("Directorio de industrias", "Industry directory"),
            ("buscar industria…", "search industry…"),
            ("Mina de carbón", "Coal mine"),
            ("Central eléctrica", "Power station"),
            ("Granja tropical", "Tropical farm"),
            ("Pozos petroleros", "Oil wells"),
            ("Acería", "Steel mill"),
            ("Banco (ártico/trópico)", "Bank (arctic/tropic)"),
            ("No hay industrias.", "No industries."),
            (
                "Ninguna industria coincide con el filtro.",
                "No industries match the filter.",
            ),
            ("pasajeros", "passengers"),
            ("mineral de hierro", "iron ore"),
            ("objetos de valor", "valuables"),
            ("carga personalizada", "custom cargo"),
            ("Carbón", "Coal"),
            ("Pozos", "Oil wells"),
            ("Granja tropic", "Tropical farm"),
            ("Fábrica tropic", "Tropical factory"),
            ("Aserradero tropic", "Tropical lumber mill"),
            ("Gaseosa", "Fizzy drinks"),
            ("Directorio de pueblos", "Town directory"),
            ("buscar pueblo…", "search town…"),
            ("Población", "Population"),
            ("Fundar pueblo", "Found town"),
            ("Loc", "Center"),
            ("Pub", "Advertise"),
            ("Fondos", "Fund"),
            ("Aut.", "Auth."),
            ("invierno", "winter"),
            ("desierto", "desert"),
            ("meta", "goal"),
            (
                "sin metas activas en este clima",
                "no active goals in this climate",
            ),
            ("buena", "good"),
            ("neutral", "neutral"),
            ("mala", "poor"),
            ("Historial mensual", "Monthly history"),
            (
                "avanza el tiempo para ver series",
                "time advances to show series",
            ),
            ("sí", "yes"),
            ("no", "no"),
            ("Habitantes", "Population"),
            ("Pasajeros", "Passengers"),
            ("Correo", "Mail"),
            ("Bienes", "Goods"),
            ("Comida", "Food"),
            ("Creciendo", "Growing"),
            ("Financiación edificios", "Building funding"),
            ("mes(es)", "month(s)"),
            ("Servicio acumulado", "Accumulated service"),
            ("partida", "game"),
            ("Crecimiento financiado", "Funded growth"),
            (
                "Metas de carga (mes anterior)",
                "Cargo goals (previous month)",
            ),
            (
                "Demanda teórica por ciclo (casas × tasa)",
                "Theoretical demand per cycle (houses × rate)",
            ),
            ("Pasajeros máx.", "Max passengers"),
            ("Correo máx.", "Max mail"),
            ("No hay pueblos.", "No towns."),
            (
                "Ningún pueblo coincide con el filtro.",
                "No towns match the filter.",
            ),
            ("sin sprite", "without sprite"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
        }
        assert_eq!(
            localized_text(
                Locale::En,
                "Silencio = sin noticias · Resumen = ticker · Completo = periódico"
            ),
            "Off = silence · Summary = ticker · Full = newspaper"
        );
    }

    #[test]
    fn catalog_translates_static_economy_news_without_touching_dynamic_text() {
        for (spanish, english) in [
            (
                "Comienza una recesión económica",
                "An economic recession begins",
            ),
            (
                "La demanda de carga y la producción industrial se reducirán.",
                "Cargo demand and industrial production will decrease.",
            ),
            ("La recesión ha terminado", "The recession has ended"),
            (
                "La economía vuelve a la normalidad.",
                "The economy returns to normal.",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        assert_eq!(
            localized_text(Locale::En, "Entrega de muchas u. de Carbón"),
            "Entrega de muchas u. de Carbón"
        );
    }

    #[test]
    fn catalog_translates_static_disaster_headlines_without_dynamic_bodies() {
        for (spanish, english) in [
            ("OVNI pequeño avistado", "Small UFO sighted"),
            ("OVNI enorme avistado", "Large UFO sighted"),
            ("Accidente aéreo", "Aircraft accident"),
            ("Accidente de helicóptero", "Helicopter accident"),
            ("Submarino a la deriva", "Submarine adrift"),
            ("Hundimiento minero", "Mine subsidence"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        assert_eq!(
            localized_text(Locale::En, "Un avión se estrella cerca de (4, 9)."),
            "An aircraft crashes near (4, 9)."
        );
    }

    #[test]
    fn catalog_translates_all_disaster_bodies_without_mutating_coordinates() {
        for (spanish, english) in [
            (
                "Un OVNI pequeño se aproxima a (1, -2).",
                "A small UFO is approaching (1, -2).",
            ),
            (
                "Un OVNI enorme se aproxima a (-3, 4).",
                "A large UFO is approaching (-3, 4).",
            ),
            (
                "Un avión se estrella cerca de (5, 6).",
                "An aircraft crashes near (5, 6).",
            ),
            (
                "Un helicóptero se estrella en (7, -8).",
                "A helicopter crashes at (7, -8).",
            ),
            (
                "Un submarino provoca daños en (-9, 10).",
                "A submarine causes damage at (-9, 10).",
            ),
            (
                "Un hundimiento en mina afecta (11, -12).",
                "A mine subsidence affects (11, -12).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Un OVNI pequeño se aproxima a (uno, 2).",
            "Un avión se estrella cerca de (4, 9)",
            "Un helicóptero se estrella en (7, -8) (GS).",
            "Un submarino provoca daños en (7, 8",
            "Un hundimiento en mina afecta (7, 8). texto",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_vehicle_advice_headlines_without_mutating_ids() {
        for (spanish, english) in [
            (
                "Sin ruta por red: vehículo 42 (orden 3)",
                "No network route: vehicle 42 (order 3)",
            ),
            ("Sin órdenes: vehículo 7", "No orders: vehicle 7"),
            (
                "Parada incompatible: vehículo 8",
                "Incompatible stop: vehicle 8",
            ),
            (
                "Sin carga disponible: vehículo 9",
                "No cargo available: vehicle 9",
            ),
            (
                "Sin camino reservado: vehículo 10",
                "No reserved path: vehicle 10",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Sin ruta por red: vehículo 42 (orden tres)",
            "Sin ruta por red: vehículo 42 (orden 3",
            "Sin órdenes: vehículo cuarenta",
            "Parada incompatible: vehículo 8 (GS)",
            "Sin carga disponible: vehículo 9 extra",
            "Sin camino reservado: vehículo ",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_train_collision_news_without_mutating_values() {
        for (spanish, english) in [
            (
                "Choque de trenes (2 víctimas)",
                "Train collision (2 victims)",
            ),
            (
                "Los trenes #17 y #23 colisionaron en (-4, 9).",
                "Trains #17 and #23 collided at (-4, 9).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Choque de trenes (dos víctimas)",
            "Choque de trenes (2 víctimas",
            "Los trenes #17 y #23 colisionaron en (x, 9).",
            "Los trenes #17 y #23 colisionaron en (-4, 9)",
            "Los trenes #17 y #23 colisionaron en (-4, 9). (GS)",
            "Los trenes #17 y # colisionaron en (-4, 9).",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_aircraft_crash_body_without_mutating_coordinates() {
        for (spanish, english) in [
            (
                "Un jet intentó aterrizar en pista corta en (4, -9).",
                "A jet attempted to land on a short runway at (4, -9).",
            ),
            (
                "Un jet intentó aterrizar en pista corta en (-12, 7).",
                "A jet attempted to land on a short runway at (-12, 7).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Un jet intentó aterrizar en pista corta en (cuatro, 9).",
            "Un jet intentó aterrizar en pista corta en (4, 9)",
            "Un jet intentó aterrizar en pista corta en (4, 9). (GS)",
            "Un jet intentó aterrizar en pista corta en (4, 9",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_level_crossing_crash_news_without_mutating_values() {
        for (spanish, english) in [
            (
                "Choque en paso a nivel (vehículo #17)",
                "Level crossing crash (vehicle #17)",
            ),
            (
                "Un vehículo de carretera chocó con un tren en (-4, 9).",
                "A road vehicle collided with a train at (-4, 9).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Choque en paso a nivel (vehículo #diecisiete)",
            "Choque en paso a nivel (vehículo #17",
            "Choque en paso a nivel (vehículo #17) (GS)",
            "Un vehículo de carretera chocó con un tren en (x, 9).",
            "Un vehículo de carretera chocó con un tren en (-4, 9)",
            "Un vehículo de carretera chocó con un tren en (-4, 9). (GS)",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_flooded_vehicle_body_without_mutating_coordinates() {
        for (spanish, english) in [
            (
                "Un vehículo quedó bajo el agua en (4, -9).",
                "A vehicle was flooded at (4, -9).",
            ),
            (
                "Un vehículo quedó bajo el agua en (-12, 7).",
                "A vehicle was flooded at (-12, 7).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Un vehículo quedó bajo el agua en (cuatro, 9).",
            "Un vehículo quedó bajo el agua en (4, 9)",
            "Un vehículo quedó bajo el agua en (4, 9). (GS)",
            "Un vehículo quedó bajo el agua en (4, 9",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_named_vehicle_accident_headlines_safely() {
        for (spanish, english) in [
            (
                "Avión #42 se estrelló al aterrizar",
                "Avión #42 crashed while landing",
            ),
            ("Nave Azul inundado", "Nave Azul flooded"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            " se estrelló al aterrizar",
            "Avión (GS) se estrelló al aterrizar",
            "Avión #42 se estrelló al aterrizar (GS)",
            "Nave (GS) inundado",
            "Nave Azul inundado (GS)",
            "Nave Azul inundad",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_industry_closing_body_and_headline() {
        assert_eq!(
            localized_text(
                Locale::En,
                "Dejará de producir y desaparecerá el mes que viene."
            ),
            "It will stop producing and disappear next month."
        );
        assert_eq!(
            localized_text(
                Locale::Es,
                "Dejará de producir y desaparecerá el mes que viene."
            ),
            "Dejará de producir y desaparecerá el mes que viene."
        );
        assert_eq!(
            localized_text(Locale::En, "Industria en (3, 7) anuncia su cierre"),
            "Industry at (3, 7) announces its closure"
        );
    }

    #[test]
    fn catalog_translates_first_vehicle_headlines_without_vehicle_ids() {
        for (spanish, english) in [
            (
                "¡Tu primer autobús está en marcha!",
                "Your first bus is running!",
            ),
            (
                "¡Tu primer camión está en marcha!",
                "Your first truck is running!",
            ),
            (
                "¡Tu primer tranvía está en marcha!",
                "Your first tram is running!",
            ),
            (
                "¡Tu primer tren está en marcha!",
                "Your first train is running!",
            ),
            (
                "¡Tu primer barco está en marcha!",
                "Your first ship is underway!",
            ),
            (
                "¡Tu primer avión está en marcha!",
                "Your first aircraft is in the air!",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        assert_eq!(
            localized_text(Locale::En, "El vehículo 42 está detenido."),
            "El vehículo 42 está detenido."
        );
    }

    #[test]
    fn catalog_translates_transport_news_templates_without_mutating_values() {
        for (spanish, english) in [
            (
                "Entrega de 12 u. de Carbón",
                "Delivery of 12 units of Carbón",
            ),
            (
                "¡Primera entrega! 7 u. de Pasajeros",
                "First delivery! 7 units of Pasajeros",
            ),
            (
                "Tu compañía ha cobrado -$2.4K por transportar Petróleo.",
                "Your company earned -$2.4K for transporting Petróleo.",
            ),
            (
                "El vehículo 429 ha salido a operar.",
                "Vehicle 429 has started operating.",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Entrega de muchas u. de Carbón",
            "Entrega de 12 u. de",
            "Tu compañía ha cobrado dinero por transportar Petróleo.",
            "El vehículo 42 ha salido a operar!",
            "Entrega de 12 u. de Carbón (GameScript)",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_subsidy_templates_without_mutating_values() {
        for (spanish, english) in [
            ("Subvención: Carbón", "Subsidy: Carbón"),
            (
                "Subvención adjudicada: Pasajeros",
                "Subsidy awarded: Pasajeros",
            ),
            (
                "Transportar Petróleo desde (3, -2) hacia la estación (8, 11).",
                "Transport Petróleo from (3, -2) to station (8, 11).",
            ),
            (
                "«Transportes Sur» se adjudica el transporte de Correo (pago ×2).",
                "«Transportes Sur» wins the Correo transport contract (payment ×2).",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Transportar Petróleo desde (3, dos) hacia la estación (8, 11).",
            "Transportar Petróleo desde (3, -2) hacia la estación (8, 11)",
            "Subvención: Carbón (GameScript)",
            "«Empresa (GS)» se adjudica el transporte de Correo (pago ×2).",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_company_news_templates_without_mutating_values() {
        for (spanish, english) in [
            (
                "Logro rival: Transportes Sur",
                "Rival achievement: Transportes Sur",
            ),
            (
                "«Transportes Sur» cumplió el objetivo: Entregar 100 t de Carbón",
                "«Transportes Sur» completed the goal: Entregar 100 t de Carbón",
            ),
            (
                "Quiebra: Transportes Norte",
                "Bankruptcy: Transportes Norte",
            ),
            (
                "La compañía «Transportes Norte» está en quiebra (mes 3/12).",
                "Company «Transportes Norte» is bankrupt (month 3/12).",
            ),
            (
                "Comprada Transportes Centro",
                "Company bought Transportes Centro",
            ),
            (
                "La compañía «Transportes Centro» fue adquirida por £120000.",
                "Company «Transportes Centro» was acquired for £120000.",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Logro rival: ",
            "Logro rival: Transportes (GameScript)",
            "«Transportes Sur» cumplió el objetivo: ",
            "«Transportes (GS)» cumplió el objetivo: Entregar 100 t",
            "Quiebra: Transportes Norte (GS)",
            "La compañía «Transportes Norte» está en quiebra (mes tres/12).",
            "La compañía «Transportes Norte» está en quiebra (mes 3/12)",
            "La compañía «Transportes Norte» está en quiebra (mes 3/12) (GS).",
            "Comprada ",
            "Comprada Transportes (GS)",
            "La compañía «Transportes Centro» fue adquirida por £ciento.",
            "La compañía «Transportes Centro» fue adquirida por £120000",
            "La compañía «Transportes Centro» fue adquirida por £120000. (GS)",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_autoreplace_failure_headline_without_touching_body() {
        for (spanish, english) in [
            (
                "Autoreemplazo falló (vehículo 42)",
                "Autoreplace failed (vehicle 42)",
            ),
            (
                "Autoreemplazo falló (vehículo 1007)",
                "Autoreplace failed (vehicle 1007)",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Autoreemplazo falló (vehículo )",
            "Autoreemplazo falló (vehículo cuarenta)",
            "Autoreemplazo falló (vehículo 42",
            "Autoreemplazo falló (vehículo 42) (GameScript)",
            "Autoreemplazo falló (vehículo 42) motivo",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
        assert_eq!(
            localized_text(Locale::En, "No hay regla de autoreemplazo para ese motor."),
            "No autoreplace rule exists for that engine."
        );
    }

    #[test]
    fn catalog_translates_industry_closure_headlines_with_valid_coordinates() {
        for (spanish, english) in [
            (
                "Industria en (3, 7) anuncia su cierre",
                "Industry at (3, 7) announces its closure",
            ),
            (
                "Industria cerrada en (-4, 12)",
                "Industry closed at (-4, 12)",
            ),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
        for malformed in [
            "Industria en (3, siete) anuncia su cierre",
            "Industria en (3, 7) anuncia su cierre (GS)",
            "Industria en (3, 7 anuncia su cierre",
            "Industria cerrada en (x, 12)",
            "Industria cerrada en (-4, 12) (GameScript)",
            "Industria cerrada en (-4, 12",
        ] {
            assert_eq!(localized_text(Locale::En, malformed), malformed);
        }
    }

    #[test]
    fn catalog_translates_display_options_and_transparency_categories() {
        for (spanish, english) in [
            ("Nombres de pueblos", "Town names"),
            ("Nombres de estaciones", "Station names"),
            ("Nombres de puntos de paso", "Waypoint names"),
            ("Nombres de competidores", "Competitor names"),
            ("Animación completa", "Full animation"),
            ("Detalle completo", "Full detail"),
            ("Reservas PBS", "PBS reservations"),
            ("Overlay Link Graph", "Link graph overlay"),
            ("Gizmos de depuración", "Debug gizmos"),
            ("Overlay de diagnóstico", "Diagnostics overlay"),
            ("Clásico", "Classic"),
            ("Carteles", "Signs"),
            ("Árboles", "Trees"),
            ("Casas", "Houses"),
            ("Industrias", "Industries"),
            ("Edificios", "Buildings"),
            ("Puentes", "Bridges"),
            ("Estructuras", "Structures"),
            ("Catenaria", "Catenary"),
            ("Textos", "Text"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
        }
    }

    #[test]
    fn catalog_translates_cargo_distribution_modes_and_explanation() {
        assert_eq!(localized_text(Locale::En, "Asimétrica"), "Asymmetric");
        assert_eq!(localized_text(Locale::En, "Simétrica"), "Symmetric");
        assert_eq!(
            localized_text(
                Locale::En,
                "Manual: hop desde órdenes del vehículo.\nAsimétrica: Demand + MCF OpenTTD (Dijkstra distancia/capacidad).\nSimétrica: Demand Symmetric OpenTTD (geografía + supply) + MCF."
            ),
            "Manual: hop from vehicle orders.\nAsymmetric: OpenTTD Demand + MCF (distance/capacity Dijkstra).\nSymmetric: OpenTTD Demand Symmetric (geography + supply) + MCF."
        );
    }

    #[test]
    fn catalog_translates_pathfinding_window_labels() {
        assert_eq!(localized_text(Locale::En, "Señales PBS"), "PBS signals");
        assert_eq!(localized_text(Locale::En, "Espera"), "Wait");
        assert_eq!(
            localized_text(
                Locale::En,
                "Espera ante path sin reserva (días). 255 = nunca girar."
            ),
            "Wait for path without reservation (days). 255 = never turn around."
        );
        for (spanish, english) in [
            ("Señales", "Signals"),
            ("Bloque", "Block"),
            ("Entrada", "Entry"),
            ("Salida", "Exit"),
            ("Combinada", "Combo"),
            ("Ruta PBS", "Path"),
            ("Ruta 1vía", "One-way path"),
            ("Eléctrica", "Electric"),
            ("Semáforo", "Semaphore"),
            ("densidad", "density"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
            assert_eq!(localized_text(Locale::Es, spanish), spanish);
        }
    }

    #[test]
    fn catalog_translates_depot_chrome() {
        for (spanish, english) in [
            ("Depósito de Trenes", "Train depot"),
            ("Depósito de Barcos", "Ship depot"),
            ("Depósito de Carretera", "Road depot"),
            ("Hangar de Aviones", "Aircraft hangar"),
            ("Vender", "Sell"),
            ("Cadena", "Chain"),
            ("Nuevos", "New"),
            ("Clonar", "Clone"),
            ("Deseng.", "Detach"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
        }
    }

    #[test]
    fn catalog_translates_station_chrome_and_summaries() {
        for (spanish, english) in [
            ("Parada de bus", "Bus stop"),
            ("Parada de camión", "Truck stop"),
            ("Estación de tren", "Train station"),
            ("Plataforma petrolera", "Oil rig"),
            ("Ruta", "Route"),
            ("Órd.", "Ord."),
            ("Nom.", "Name"),
            ("Carga", "Cargo"),
            ("Unir", "Join"),
            ("Cerrar", "Close"),
            ("todas", "all"),
            ("con espera", "waiting"),
            ("aceptadas", "accepted"),
            ("ninguna", "none"),
            ("ingresos", "income"),
            ("activo", "active"),
            ("Cargas en espera", "Cargo waiting"),
            ("Vehículos en ruta", "Vehicles en route"),
        ] {
            assert_eq!(localized_text(Locale::En, spanish), english);
        }
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
    fn localization_plugin_translates_static_economy_news_only() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Comienza una recesión económica"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "La demanda de carga y la producción industrial se reducirán.",
            ))
            .id();
        let dynamic = app
            .world_mut()
            .spawn(Text::new("Entrega de muchas u. de Carbón"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "An economic recession begins"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Cargo demand and industrial production will decrease."
        );
        assert_eq!(
            app.world().get::<Text>(dynamic).unwrap().as_str(),
            "Entrega de muchas u. de Carbón"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Comienza una recesión económica"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "La demanda de carga y la producción industrial se reducirán."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_disaster_headline_and_body() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Accidente de helicóptero"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new("Un helicóptero se estrella en (8, 11)."))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Helicopter accident"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "A helicopter crashes at (8, 11)."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Accidente de helicóptero"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_all_disaster_bodies_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let entries = [
            (
                "Un OVNI pequeño se aproxima a (1, -2).",
                "A small UFO is approaching (1, -2).",
            ),
            (
                "Un OVNI enorme se aproxima a (-3, 4).",
                "A large UFO is approaching (-3, 4).",
            ),
            (
                "Un avión se estrella cerca de (5, 6).",
                "An aircraft crashes near (5, 6).",
            ),
            (
                "Un helicóptero se estrella en (7, -8).",
                "A helicopter crashes at (7, -8).",
            ),
            (
                "Un submarino provoca daños en (-9, 10).",
                "A submarine causes damage at (-9, 10).",
            ),
            (
                "Un hundimiento en mina afecta (11, -12).",
                "A mine subsidence affects (11, -12).",
            ),
        ];
        let entities: Vec<_> = entries
            .iter()
            .map(|(spanish, _)| app.world_mut().spawn(Text::new(*spanish)).id())
            .collect();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        for ((_, english), entity) in entries.iter().zip(entities.iter()) {
            assert_eq!(app.world().get::<Text>(*entity).unwrap().as_str(), *english);
        }

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        for ((spanish, _), entity) in entries.iter().zip(entities.iter()) {
            assert_eq!(app.world().get::<Text>(*entity).unwrap().as_str(), *spanish);
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_vehicle_advice_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let entries = [
            (
                "Sin ruta por red: vehículo 42 (orden 3)",
                "No network route: vehicle 42 (order 3)",
            ),
            ("Sin órdenes: vehículo 7", "No orders: vehicle 7"),
            (
                "Parada incompatible: vehículo 8",
                "Incompatible stop: vehicle 8",
            ),
            (
                "Sin carga disponible: vehículo 9",
                "No cargo available: vehicle 9",
            ),
            (
                "Sin camino reservado: vehículo 10",
                "No reserved path: vehicle 10",
            ),
        ];
        let entities: Vec<_> = entries
            .iter()
            .map(|(spanish, _)| app.world_mut().spawn(Text::new(*spanish)).id())
            .collect();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        for ((_, english), entity) in entries.iter().zip(entities.iter()) {
            assert_eq!(app.world().get::<Text>(*entity).unwrap().as_str(), *english);
        }

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        for ((spanish, _), entity) in entries.iter().zip(entities.iter()) {
            assert_eq!(app.world().get::<Text>(*entity).unwrap().as_str(), *spanish);
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_train_collision_news_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Choque de trenes (2 víctimas)"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new("Los trenes #17 y #23 colisionaron en (-4, 9)."))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Los trenes #17 y #23 colisionaron en (x, 9)."))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Train collision (2 victims)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Trains #17 and #23 collided at (-4, 9)."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Los trenes #17 y #23 colisionaron en (x, 9)."
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Choque de trenes (12 víctimas)"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "Train collision (12 victims)"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Choque de trenes (2 víctimas)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Los trenes #17 y #23 colisionaron en (-4, 9)."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_aircraft_crash_body_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let body = app
            .world_mut()
            .spawn(Text::new(
                "Un jet intentó aterrizar en pista corta en (4, -9).",
            ))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new(
                "Un jet intentó aterrizar en pista corta en (cuatro, 9).",
            ))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "A jet attempted to land on a short runway at (4, -9)."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Un jet intentó aterrizar en pista corta en (cuatro, 9)."
        );

        let late = app
            .world_mut()
            .spawn(Text::new(
                "Un jet intentó aterrizar en pista corta en (-12, 7).",
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "A jet attempted to land on a short runway at (-12, 7)."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Un jet intentó aterrizar en pista corta en (4, -9)."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_level_crossing_crash_news_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Choque en paso a nivel (vehículo #17)"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "Un vehículo de carretera chocó con un tren en (-4, 9).",
            ))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new(
                "Un vehículo de carretera chocó con un tren en (x, 9).",
            ))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Level crossing crash (vehicle #17)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "A road vehicle collided with a train at (-4, 9)."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Un vehículo de carretera chocó con un tren en (x, 9)."
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Choque en paso a nivel (vehículo #42)"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "Level crossing crash (vehicle #42)"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Choque en paso a nivel (vehículo #17)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Un vehículo de carretera chocó con un tren en (-4, 9)."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_flooded_vehicle_body_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let body = app
            .world_mut()
            .spawn(Text::new("Un vehículo quedó bajo el agua en (4, -9)."))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Un vehículo quedó bajo el agua en (cuatro, 9)."))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "A vehicle was flooded at (4, -9)."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Un vehículo quedó bajo el agua en (cuatro, 9)."
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Un vehículo quedó bajo el agua en (-12, 7)."))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "A vehicle was flooded at (-12, 7)."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Un vehículo quedó bajo el agua en (4, -9)."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_named_vehicle_accident_headlines_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let aircraft = app
            .world_mut()
            .spawn(Text::new("Avión #42 se estrelló al aterrizar"))
            .id();
        let flooded = app.world_mut().spawn(Text::new("Nave Azul inundado")).id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Avión (GS) se estrelló al aterrizar"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(aircraft).unwrap().as_str(),
            "Avión #42 crashed while landing"
        );
        assert_eq!(
            app.world().get::<Text>(flooded).unwrap().as_str(),
            "Nave Azul flooded"
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Avión (GS) se estrelló al aterrizar"
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Avión #100 se estrelló al aterrizar"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "Avión #100 crashed while landing"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(aircraft).unwrap().as_str(),
            "Avión #42 se estrelló al aterrizar"
        );
        assert_eq!(
            app.world().get::<Text>(flooded).unwrap().as_str(),
            "Nave Azul inundado"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_industry_closing_body_and_headline() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Industria en (3, 7) anuncia su cierre"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "Dejará de producir y desaparecerá el mes que viene.",
            ))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Industry at (3, 7) announces its closure"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "It will stop producing and disappear next month."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Dejará de producir y desaparecerá el mes que viene."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_first_vehicle_headline_only() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("¡Tu primer avión está en marcha!"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new("El vehículo 42 está detenido."))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Your first aircraft is in the air!"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "El vehículo 42 está detenido."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "¡Tu primer avión está en marcha!"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_dynamic_transport_news_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Entrega de 12 u. de Carbón"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "Tu compañía ha cobrado $42 por transportar Carbón.",
            ))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Entrega de muchas u. de Carbón"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Delivery of 12 units of Carbón"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Your company earned $42 for transporting Carbón."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Entrega de muchas u. de Carbón"
        );

        let late = app
            .world_mut()
            .spawn(Text::new("¡Primera entrega! 7 u. de Pasajeros"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "First delivery! 7 units of Pasajeros"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Entrega de 12 u. de Carbón"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Tu compañía ha cobrado $42 por transportar Carbón."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_subsidy_news_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app.world_mut().spawn(Text::new("Subvención: Carbón")).id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "Transportar Petróleo desde (3, -2) hacia la estación (8, 11).",
            ))
            .id();
        let awarded = app
            .world_mut()
            .spawn(Text::new(
                "«Transportes Sur» se adjudica el transporte de Correo (pago ×2).",
            ))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Subsidy: Carbón"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Transport Petróleo from (3, -2) to station (8, 11)."
        );
        assert_eq!(
            app.world().get::<Text>(awarded).unwrap().as_str(),
            "«Transportes Sur» wins the Correo transport contract (payment ×2)."
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Transportar Petróleo desde (3, -2) hacia la estación (8, 11)."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_company_news_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Logro rival: Transportes Sur"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new(
                "La compañía «Transportes Norte» está en quiebra (mes 3/12).",
            ))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Quiebra: Transportes Norte (GS)"))
            .id();
        let acquired = app
            .world_mut()
            .spawn(Text::new(
                "La compañía «Transportes Centro» fue adquirida por £120000.",
            ))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Rival achievement: Transportes Sur"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "Company «Transportes Norte» is bankrupt (month 3/12)."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Quiebra: Transportes Norte (GS)"
        );
        assert_eq!(
            app.world().get::<Text>(acquired).unwrap().as_str(),
            "Company «Transportes Centro» was acquired for £120000."
        );

        let late = app
            .world_mut()
            .spawn(Text::new(
                "«Transportes Sur» cumplió el objetivo: Entregar 100 t de Carbón",
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "«Transportes Sur» completed the goal: Entregar 100 t de Carbón"
        );

        let late_headline = app
            .world_mut()
            .spawn(Text::new("Comprada Transportes Centro"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late_headline).unwrap().as_str(),
            "Company bought Transportes Centro"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Logro rival: Transportes Sur"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "La compañía «Transportes Norte» está en quiebra (mes 3/12)."
        );
        assert_eq!(
            app.world().get::<Text>(acquired).unwrap().as_str(),
            "La compañía «Transportes Centro» fue adquirida por £120000."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_autoreplace_failure_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let headline = app
            .world_mut()
            .spawn(Text::new("Autoreemplazo falló (vehículo 42)"))
            .id();
        let body = app
            .world_mut()
            .spawn(Text::new("No hay regla de autoreemplazo para ese motor."))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Autoreemplazo falló (vehículo 42) (GS)"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Autoreplace failed (vehicle 42)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "No autoreplace rule exists for that engine."
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Autoreemplazo falló (vehículo 42) (GS)"
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Autoreemplazo falló (vehículo 1007)"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "Autoreplace failed (vehicle 1007)"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(headline).unwrap().as_str(),
            "Autoreemplazo falló (vehículo 42)"
        );
        assert_eq!(
            app.world().get::<Text>(body).unwrap().as_str(),
            "No hay regla de autoreemplazo para ese motor."
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn localization_plugin_translates_industry_closure_headlines_late() {
        let mut app = App::new();
        app.insert_resource(ClientPreferences::default());
        app.add_plugins(LocalizationPlugin);
        let closing = app
            .world_mut()
            .spawn(Text::new("Industria en (3, 7) anuncia su cierre"))
            .id();
        let closed = app
            .world_mut()
            .spawn(Text::new("Industria cerrada en (-4, 12)"))
            .id();
        let malformed = app
            .world_mut()
            .spawn(Text::new("Industria cerrada en (x, 12)"))
            .id();

        app.update();
        app.world_mut().resource_mut::<ClientPreferences>().language = "en".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(closing).unwrap().as_str(),
            "Industry at (3, 7) announces its closure"
        );
        assert_eq!(
            app.world().get::<Text>(closed).unwrap().as_str(),
            "Industry closed at (-4, 12)"
        );
        assert_eq!(
            app.world().get::<Text>(malformed).unwrap().as_str(),
            "Industria cerrada en (x, 12)"
        );

        let late = app
            .world_mut()
            .spawn(Text::new("Industria en (-10, -2) anuncia su cierre"))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Text>(late).unwrap().as_str(),
            "Industry at (-10, -2) announces its closure"
        );

        app.world_mut().resource_mut::<ClientPreferences>().language = "es-AR".into();
        app.update();
        assert_eq!(
            app.world().get::<Text>(closing).unwrap().as_str(),
            "Industria en (3, 7) anuncia su cierre"
        );
        assert_eq!(
            app.world().get::<Text>(closed).unwrap().as_str(),
            "Industria cerrada en (-4, 12)"
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
