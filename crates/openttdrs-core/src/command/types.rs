use crate::bridge_spec::BridgeType;
use crate::map::TileCoord;
use crate::{IndustryKind, IndustrySpec, VehicleKind};

/// Modo de `LevelLand` (igual que `LevelMode` en `terraform_cmd.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LevelMode {
    /// Igualar al `TileHeight` de la tesela origen.
    Level,
    /// Subir el rectángulo un nivel respecto al origen.
    Raise,
    /// Bajar el rectángulo un nivel respecto al origen.
    Lower,
}

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Coloca o combina una pieza de carretera `OpenTTD` (`RoadBits`, bits 0..3).
    PlaceRoadBits(TileCoord, u8),
    /// Coloca o combina trazado de tranvía (`m3` bits 0..3; tipo en `m8`).
    PlaceTramBits(TileCoord, u8),
    /// Quita el overlay de tranvía sin demoler la carretera.
    RemoveTramBits(TileCoord),
    /// Reemplaza la geometría de carretera de la tesela con `RoadBits` exactos.
    SetRoadBits(TileCoord, u8),
    /// Coloca via de tren en la tesela (MVP: validacion de terreno).
    PlaceRail(TileCoord),
    /// Coloca o combina `TrackBits` de vía (`0x0C` HORZ, `0x30` VERT, etc.).
    PlaceRailBits(TileCoord, u8),
    /// Reemplaza la geometría de vía con `TrackBits` exactos (drag de herramientas HORZ/VERT).
    SetRailBits(TileCoord, u8),
    /// Convierte vía recta (solo X o Y) en waypoint ferroviario.
    PlaceRailWaypoint(TileCoord),
    /// Waypoint road 1×1 sobre carretera recta (`StationType::RoadWaypoint`).
    PlaceRoadWaypoint(TileCoord),
    /// Quita `TrackBits` de una tesela de vía (drag de «quitar vía»).
    RemoveRailBits(TileCoord, u8),
    /// Quita toda la vía de la tesela.
    RemoveRail(TileCoord),
    /// Convierte el tipo de vía (`Rail` ↔ `Electric`); `to_type` = `RailType` como `u8`.
    ConvertRail(TileCoord, u8),
    /// Coloca señal ferroviaria; `face` es `DiagDir` (0=NE..3=NW).
    /// `fract_x`/`fract_y` (0–255) eligen carril en teselas HORZ/VERT como en `OpenTTD`.
    /// `sig_type`: `SIGTYPE_BLOCK`, `SIGTYPE_PATH` o `SIGTYPE_PATH_ONEWAY`.
    PlaceRailSignal(TileCoord, u8, u8, u8, u8),
    /// Cicla el tipo de señal existente (Ctrl+clic en `OpenTTD`).
    CycleRailSignalType(TileCoord, u8, u8),
    /// Quita la señal del carril bajo el cursor sin demoler la vía.
    /// `fract_x`/`fract_y` eligen carril en teselas HORZ/VERT.
    RemoveRailSignal(TileCoord, u8, u8),
    PlaceRoadDepot(TileCoord),
    PlaceRoadDepotDir(TileCoord, u8),
    PlaceRailDepot(TileCoord),
    /// Depósito de tren con orientación de entrada `0..3`.
    PlaceRailDepotDir(TileCoord, u8),
    /// Depósito de barcos sobre agua; `dir` 0..3 = boca hacia agua.
    PlaceShipDepotDir(TileCoord, u8),
    /// Muelle 1×1 sobre agua costera; `dir` orienta el sprite (eje X/Y).
    PlaceDock(TileCoord, u8),
    /// Helipuerto / aeropuerto 1×1 (compra aviones + carga pasajeros).
    PlaceAirport(TileCoord),
    /// Aeropuerto por spec; `axis_y` rota el footprint.
    PlaceAirportArea {
        origin: TileCoord,
        axis_y: bool,
        #[serde(default)]
        spec: crate::airport_class::AirportSpecId,
    },
    /// Canal: convierte terreno en agua navegable.
    PlaceCanal(TileCoord),
    /// Pinta río (`WaterClass::River`); plano o pendiente inclinada.
    PlaceRiver(TileCoord),
    /// Boya: waypoint acuático sobre agua (`StationType::Buoy`).
    PlaceBuoy(TileCoord),
    /// Acueducto: puente de canal entre dos rampas en pendiente.
    PlaceAqueduct(TileCoord, TileCoord),
    /// Esclusa sobre agua; `axis_y` = eje N-S.
    PlaceLock(TileCoord, bool),
    PlaceRoadTunnel(TileCoord, TileCoord),
    PlaceRailTunnel(TileCoord, TileCoord),
    PlaceRoadBridge(TileCoord, TileCoord, BridgeType),
    PlaceRailBridge(TileCoord, TileCoord, BridgeType),
    SetVehicleOrders(u32, Vec<TileCoord>),
    SetVehicleStationOrders(u32, Vec<TileCoord>),
    /// Lista completa de órdenes (estaciones, waypoints, teselas).
    SetVehicleOrderList(u32, Vec<crate::vehicle::VehicleOrder>),
    PlaceHouse(TileCoord),
    PlaceIndustry(TileCoord),
    PlaceIndustryKind(TileCoord, IndustryKind),
    PlaceIndustrySpec(TileCoord, IndustrySpec),
    PlaceForest(TileCoord),
    /// Añade una estación y marca la tesela como `TileKind::Station`.
    PlaceStation(TileCoord),
    /// Añade una estación de carretera con orientación visual `0..3`.
    PlaceStationDir(TileCoord, u8),
    PlaceBusStop(TileCoord, u8),
    PlaceTruckStop(TileCoord, u8),
    /// Estación de tren 1×1 (`StationType::Rail`); `dir` 0..3 → eje vía en `m5`.
    PlaceRailStation(TileCoord, u8),
    /// Estación de tren multi-tesela (`CmdBuildRailStation`): `origin` es la esquina
    /// norte, `axis_y` el eje de los andenes, `platforms`/`length` en 1..=7.
    PlaceRailStationArea {
        origin: TileCoord,
        axis_y: bool,
        platforms: u8,
        length: u8,
    },
    /// Compra el motor por defecto del tipo en un depósito de carretera
    /// (conservado por compatibilidad; usa [`Command::BuildVehicleAtDepot`]).
    BuildRoadVehicleAtDepot(TileCoord, VehicleKind),
    /// Compra el modelo `engine_id` del catálogo en un depósito compatible
    /// (carretera o vía según el tipo del motor), validando fondos.
    BuildVehicleAtDepot(TileCoord, u16),
    /// Engancha el vagón `wagon_id` al final del consist de `head_id` (ambos en depósito).
    AttachWagonToConsist {
        head_id: u32,
        wagon_id: u32,
    },
    /// Desengancha `unit_id` del consist (queda suelto en el depósito).
    DetachConsistUnit(u32),
    /// Mueve/reordena: engancha `unit_id` tras `after_id` (`None` = al final de `head_id`).
    MoveRailVehicle {
        head_id: u32,
        unit_id: u32,
        after_id: Option<u32>,
    },
    SellVehicle(u32),
    ToggleVehicleRunning(u32),
    CloneVehicleOrders {
        from_vehicle_id: u32,
        to_vehicle_id: u32,
    },
    /// Compra un vehículo idéntico al origen (mismo motor y órdenes) en el depósito.
    CloneVehicleAtDepot {
        source_vehicle_id: u32,
        depot_pos: TileCoord,
    },
    /// Vende todos los vehículos estacionados en el depósito.
    SellAllVehiclesAtDepot(TileCoord),
    /// Elimina la orden en `index` y ajusta `current_order`.
    RemoveVehicleOrderAt {
        vehicle_id: u32,
        index: usize,
    },
    /// Salta la orden actual sin cumplirla (pasa a la siguiente).
    SkipVehicleOrder(u32),
    /// Alterna «carga completa» en la orden de estación `index`.
    ToggleVehicleOrderFullLoad {
        vehicle_id: u32,
        index: usize,
    },
    /// Alterna «no descargar» en la orden de estación `index`.
    ToggleVehicleOrderNoUnload {
        vehicle_id: u32,
        index: usize,
    },
    /// Añade orden al depósito compatible más cercano (Manhattan).
    AppendGotoNearestDepot(u32),
    /// Renombra un vehículo (`None` o cadena vacía → quitar nombre).
    RenameVehicle {
        vehicle_id: u32,
        name: Option<String>,
    },
    /// Renombra una estación (`None` o cadena vacía → quitar nombre).
    RenameStation {
        station_pos: TileCoord,
        name: Option<String>,
    },
    /// Pone todos los vehículos en `depot_pos` en marcha o detenidos.
    SetDepotVehiclesRunning {
        depot_pos: TileCoord,
        running: bool,
    },
    /// Intercambia la orden `index` con la anterior (`Up`) o siguiente (`Down`).
    MoveVehicleOrder {
        vehicle_id: u32,
        index: usize,
        direction: OrderMoveDirection,
    },
    /// Alterna «parar en depósito» en una orden [`VehicleOrder::Depot`].
    ToggleVehicleOrderDepotStop {
        vehicle_id: u32,
        index: usize,
    },
    /// Invierte el sentido de marcha (solo trenes).
    TurnAroundVehicle(u32),
    /// Ignora la señal roja en el próximo tick de movimiento (solo trenes).
    ForceVehicleProceed(u32),
    /// Cambia el tipo de carga aceptado (solo en depósito, sin carga a bordo).
    /// `unit_ids` vacío = solo la unidad `vehicle_id` (cabeza u otra).
    RefitVehicle {
        vehicle_id: u32,
        cargo: crate::cargo::CargoType,
        #[serde(default)]
        unit_ids: Vec<u32>,
    },
    /// Cicla el refit automático de una orden [`VehicleOrder::Depot`].
    CycleVehicleOrderDepotRefit {
        vehicle_id: u32,
        index: usize,
    },
    /// Activa/desactiva el horario del vehículo.
    ToggleVehicleTimetable(u32),
    /// Cicla la espera en parada de la orden `index`.
    CycleVehicleOrderWait {
        vehicle_id: u32,
        index: usize,
    },
    /// Cicla el tiempo mínimo de viaje hacia la orden `index`.
    CycleVehicleOrderTravel {
        vehicle_id: u32,
        index: usize,
    },
    /// Define o actualiza una regla de autoreemplazo global.
    SetAutoReplaceRule {
        from_engine_id: u16,
        to_engine_id: u16,
    },
    /// Elimina la regla de autoreemplazo para un motor origen.
    ClearAutoReplaceRule {
        from_engine_id: u16,
    },
    /// Activa/desactiva la regla existente para un motor origen.
    ToggleAutoReplaceRule {
        from_engine_id: u16,
    },
    CreateVehicleGroup {
        name: String,
    },
    RenameVehicleGroup {
        group_id: u32,
        name: String,
    },
    AssignVehicleToGroup {
        vehicle_id: u32,
        group_id: Option<u32>,
    },
    ClearVehicleTimetableLateness(u32),
    SetVehicleOrderWaitTicks {
        vehicle_id: u32,
        index: usize,
        wait_ticks: u32,
    },
    SetVehicleOrderTravelTicks {
        vehicle_id: u32,
        index: usize,
        travel_ticks: u32,
    },
    ToggleVehicleTimetableAutofill(u32),
    ToggleAutoReplaceOnlyWhenOld {
        from_engine_id: u16,
    },
    SetAutoReplaceRuleGroup {
        from_engine_id: u16,
        group_id: Option<u32>,
    },
    DepotMassAutoreplace {
        depot_pos: TileCoord,
    },
    CreateSharedOrdersFromVehicle(u32),
    LinkVehicleToSharedOrders {
        vehicle_id: u32,
        shared_id: u32,
    },
    UnlinkVehicleSharedOrders(u32),
    SetSharedOrderAt {
        shared_id: u32,
        index: usize,
        order: crate::vehicle::VehicleOrder,
    },
    SetVehicleOrderConditional {
        vehicle_id: u32,
        index: usize,
        condition: crate::vehicle::OrderConditionKind,
        value: u8,
        jump_to: usize,
    },
    DepotReorderVehicleSlot {
        depot_pos: TileCoord,
        from_slot: usize,
        to_slot: usize,
    },
    /// Limpia la tesela y vuelve a `TileKind::Grass`.
    ClearTile(TileCoord),
    /// Eleva la esquina norte de la tesela (terraform manual).
    RaiseLand(TileCoord),
    /// Baja la esquina norte de la tesela (terraform manual).
    LowerLand(TileCoord),
    /// Nivela un rectángulo de teselas (`CmdLevelLand`).
    LevelLand {
        from: TileCoord,
        to: TileCoord,
        mode: LevelMode,
    },
    /// Marca tesela como terreno comprado (`OBJECT_OWNED_LAND`).
    BuyLand(TileCoord),
    /// Compra un rectángulo de teselas (arrastre en panel paisaje).
    BuyLandArea {
        from: TileCoord,
        to: TileCoord,
    },
    /// Solicita más préstamo bancario (`CmdIncreaseLoan`).
    IncreaseLoan,
    /// Devuelve parte del préstamo (`CmdDecreaseLoan`).
    DecreaseLoan,
    /// Campaña publicitaria en una ciudad (`CmdTownAction::Advertise`).
    TownAdvertise(u32),
    /// Financia edificios en una ciudad (`CmdTownAction::FundBuildings`).
    TownFundBuildings(u32),
    /// Planta un árbol en hierba o incrementa densidad en bosque.
    PlantTree(TileCoord),
    /// Quita árbol o reduce etapa de cultivo.
    ClearTree(TileCoord),
    /// Coloca un cartel en la tesela (`CmdPlaceSign`).
    PlaceSign {
        pos: TileCoord,
        name: Option<String>,
    },
    /// Elimina un cartel por id.
    RemoveSign {
        sign_id: u32,
    },
    /// Renombra un cartel (nombre vacío no permitido).
    RenameSign {
        sign_id: u32,
        name: Option<String>,
    },
    /// Une dos paradas road 1×1 o estaciones rail con huellas adyacentes.
    /// `keep` permanece; `merge` se fusiona en `keep.joined_tiles`.
    JoinStations {
        keep: TileCoord,
        merge: TileCoord,
    },
    /// Activa/desactiva una entrada del stack `NewGRF` (config-only).
    SetNewGrfEnabled {
        index: usize,
        enabled: bool,
    },
    /// Reordena una entrada del stack `NewGRF`.
    MoveNewGrfInStack {
        from: usize,
        to: usize,
    },
    /// Quita una entrada no estática del stack `NewGRF`.
    RemoveNewGrfFromStack {
        index: usize,
    },
    /// Añade una entrada al stack `NewGRF` (rechaza `GRFID` duplicado).
    AddNewGrfToStack {
        entry: crate::newgrf_config::NewGrfEntry,
    },
}

/// Dirección para reordenar órdenes en la lista del vehículo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OrderMoveDirection {
    Up,
    Down,
}

/// Fallo al aplicar un comando (estado sin cambios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    OutOfBounds,
    CannotPlaceRoadOnWater,
    CannotPlaceRoadOnVoid,
    CannotPlaceRailOnWater,
    CannotPlaceRailOnVoid,
    CannotPlaceStationOnWater,
    CannotPlaceStationOnVoid,
    /// La tesela destino no es hierba libre (p. ej. carretera o vía existente).
    CannotPlaceStationOnOccupiedTile,
    /// La tesela no tiene ningún vecino con carretera/vía (ni equivalente transitable).
    StationNotAdjacentToTransport,
    StationAlreadyExists,
    /// Tamaño de andenes/longitud no permitido por el `StationSpec` activo.
    StationSizeNotAllowed,
    StationNotFound,
    VehicleNotFound,
    /// El vehículo no pertenece a la compañía activa.
    VehicleNotOwned,
    /// La infraestructura no pertenece a la compañía activa.
    TileNotOwned,
    /// Solo se puede vender un vehículo estacionado en un depósito.
    VehicleNotInDepot,
    InvalidDepotTile,
    VehicleKindNotAllowed,
    /// El motor pedido no existe en el catálogo.
    EngineNotFound,
    /// No hay dinero suficiente para pagar la compra.
    InsufficientFunds,
    IncompatibleStopForVehicle,
    /// Índice de orden fuera de rango o sin órdenes.
    OrderIndexOutOfRange,
    /// El flag solo aplica a órdenes de estación (no waypoints ni depósitos).
    OrderFlagNotApplicable,
    /// No hay depósito compatible en el mapa.
    DepotNotFound,
    /// Nombre de vehículo demasiado largo.
    VehicleNameTooLong,
    /// Nombre de estación demasiado largo.
    StationNameTooLong,
    /// Refit no permitido (fuera de depósito, con carga o tipo inválido).
    RefitNotAllowed,
    /// Horario: el ajuste no aplica a este tipo de orden.
    TimetableNotApplicable,
    /// Autoreemplazo no permitido (motores distintos, fuera de depósito, etc.).
    AutoreplaceNotAllowed,
    /// No hay regla de autoreemplazo para ese motor.
    AutoReplaceRuleNotFound,
    /// Grupo de vehículos no encontrado.
    VehicleGroupNotFound,
    /// Nombre de grupo inválido.
    VehicleGroupNameInvalid,
    /// Pool de órdenes compartidas no encontrado.
    SharedOrdersNotFound,
    /// Horario: aún queda tiempo de espera en depósito.
    TimetableWaitPending,
    /// Extremos de túnel inválidos (pendiente, salida, etc.).
    InvalidTunnelEndpoints,
    /// Puente sin hueco que salvar (agua o terreno más bajo bajo el tramo).
    BridgeTypeNotAvailable,
    InvalidBridgeSpan,
    /// `TrackBits` incompatibles con la pendiente de la tesela (`GetRailFoundation`).
    InvalidRailOnSlope,
    /// Solo vía recta (eje X o Y) admite waypoint.
    CannotPlaceWaypointOnTrack,
    /// No hay vía que quitar en esta tesela.
    NoRailToRemove,
    /// No hay overlay de tranvía que quitar.
    NoTramToRemove,
    /// No hay vía que convertir en esta tesela.
    NoRailToConvert,
    /// Un tren en la tesela no es compatible con el tipo de vía destino.
    TrainIncompatibleWithRailType,
    /// El motor requiere vía electrificada.
    EngineRequiresElectricRail,
    /// El motor requiere vía monorail.
    EngineRequiresMonorail,
    /// El motor requiere vía maglev.
    EngineRequiresMaglev,
    /// Solo vía recta admite señales de bloque (v1).
    CannotPlaceSignalOnTrack,
    /// Ya hay una señal en esa dirección en esta tesela.
    SignalAlreadyPresent,
    /// La tesela no admite terraform (solo hierba/bosque en T1).
    TileNotTerraformable,
    /// Altura de esquina por encima del límite del mapa.
    TerrainTooHigh,
    /// Altura de esquina por debajo del nivel mínimo.
    TerrainTooLow,
    /// Pendiente inválida tras el cambio de alturas.
    InvalidTerrainSlope,
    /// La tesela ya está marcada como terreno comprado.
    LandAlreadyOwned,
    /// Solo se puede comprar hierba o bosque libre.
    CannotBuyLandHere,
    /// Esta industria no está disponible en el clima actual del mapa.
    IndustryNotAvailableInClimate,
    /// Préstamo ya al máximo permitido.
    LoanAtMaximum,
    /// No hay préstamo suficiente para devolver.
    NoLoanToRepay,
    /// Ciudad no encontrada.
    TownNotFound,
    /// La autoridad local no permite construir aquí.
    AuthorityRatingTooLow,
    /// No se puede plantar un árbol aquí.
    CannotPlantTreeHere,
    /// No hay árbol ni cultivo que quitar.
    NoTreeHere,
    /// Cartel no encontrado.
    SignNotFound,
    /// Nombre de cartel demasiado largo.
    SignNameTooLong,
    /// El cartel necesita un nombre no vacío.
    SignNameEmpty,
    /// No se pueden unir estas estaciones.
    CannotJoinStations,
    /// Índice fuera del stack `NewGRF`.
    NewGrfIndexOutOfRange,
    /// Entrada base/estática: no se puede desactivar ni quitar.
    NewGrfStaticImmutable,
    /// Ya hay un `NewGRF` con ese `GRFID` en el stack.
    NewGrfDuplicateGrfid,
    /// Entrada `NewGRF` inválida (p. ej. nombre de archivo vacío).
    NewGrfInvalidEntry,
}

/// Texto breve en español para mostrar al jugador cuando falla un comando.
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn command_error_message(err: CommandError) -> &'static str {
    match err {
        CommandError::OutOfBounds => "Fuera del mapa.",
        CommandError::CannotPlaceRoadOnWater => "No se puede construir carretera en agua.",
        CommandError::CannotPlaceRoadOnVoid => "No se puede construir carretera aquí.",
        CommandError::CannotPlaceRailOnWater => "No se puede construir vía en agua.",
        CommandError::CannotPlaceRailOnVoid => "No se puede construir vía aquí.",
        CommandError::CannotPlaceStationOnWater => "No se puede construir estación en agua.",
        CommandError::CannotPlaceStationOnVoid => "No se puede construir estación aquí.",
        CommandError::CannotPlaceStationOnOccupiedTile => {
            "La parada debe ir en hierba o bosque limpiable, no sobre carretera ni vía."
        }
        CommandError::StationNotAdjacentToTransport => {
            "La entrada debe dar a la carretera o vía en esa dirección."
        }
        CommandError::StationAlreadyExists => "Ya hay una estación en esta tesela.",
        CommandError::StationSizeNotAllowed => {
            "Este tipo de estación no permite ese número de andenes o longitud."
        }
        CommandError::StationNotFound => "No hay estación en esta tesela.",
        CommandError::VehicleNotFound => "Vehículo no encontrado.",
        CommandError::VehicleNotOwned => "Ese vehículo pertenece a otra compañía.",
        CommandError::TileNotOwned => "Esta infraestructura pertenece a otra compañía.",
        CommandError::VehicleNotInDepot => {
            "Solo se puede vender un vehículo dentro de un depósito."
        }
        CommandError::InvalidDepotTile => "Ubicación de depósito inválida.",
        CommandError::VehicleKindNotAllowed => "Tipo de vehículo no permitido aquí.",
        CommandError::EngineNotFound => "Modelo de vehículo desconocido.",
        CommandError::InsufficientFunds => "No hay dinero suficiente.",
        CommandError::IncompatibleStopForVehicle => "Parada incompatible con este vehículo.",
        CommandError::OrderIndexOutOfRange => "Índice de orden inválido.",
        CommandError::OrderFlagNotApplicable => "Ese ajuste solo aplica a paradas de estación.",
        CommandError::DepotNotFound => "No hay depósito compatible en el mapa.",
        CommandError::VehicleNameTooLong => "El nombre del vehículo es demasiado largo.",
        CommandError::StationNameTooLong => "El nombre de la estación es demasiado largo.",
        CommandError::RefitNotAllowed => {
            "Solo se puede refit en depósito, sin carga y con un tipo compatible."
        }
        CommandError::TimetableNotApplicable => "Ese ajuste de horario no aplica a esta orden.",
        CommandError::AutoreplaceNotAllowed => {
            "Autoreemplazo no permitido para este vehículo o motor."
        }
        CommandError::AutoReplaceRuleNotFound => "No hay regla de autoreemplazo para ese motor.",
        CommandError::VehicleGroupNotFound => "Grupo de vehículos no encontrado.",
        CommandError::VehicleGroupNameInvalid => "Nombre de grupo inválido.",
        CommandError::SharedOrdersNotFound => "Pool de órdenes compartidas no encontrado.",
        CommandError::TimetableWaitPending => {
            "El vehículo aún espera según el horario antes de salir del depósito."
        }
        CommandError::InvalidTunnelEndpoints => {
            "Túnel inválido: entrada en pendiente inclinada (NE/SE/SW/NW) y salida al mismo nivel."
        }
        CommandError::BridgeTypeNotAvailable => {
            "Este tipo de puente no está disponible (año, longitud o presupuesto)."
        }
        CommandError::InvalidBridgeSpan => {
            "Puente inválido: las orillas al mismo nivel y agua o terreno más bajo bajo el tramo."
        }
        CommandError::InvalidRailOnSlope => {
            "La vía no puede construirse en esta pendiente con esa geometría."
        }
        CommandError::CannotPlaceWaypointOnTrack => {
            "El waypoint solo puede colocarse sobre vía recta (eje X o Y)."
        }
        CommandError::NoRailToRemove => "No hay vía que quitar aquí.",
        CommandError::NoTramToRemove => "No hay tranvía que quitar aquí.",
        CommandError::NoRailToConvert => "No hay vía que convertir aquí.",
        CommandError::TrainIncompatibleWithRailType => {
            "Hay un tren incompatible con ese tipo de vía."
        }
        CommandError::EngineRequiresElectricRail => {
            "Este motor requiere vía electrificada (convertí la vía o el depósito)."
        }
        CommandError::EngineRequiresMonorail => {
            "Este motor requiere vía monorail (convertí la vía adyacente)."
        }
        CommandError::EngineRequiresMaglev => {
            "Este motor requiere vía maglev (convertí la vía adyacente)."
        }
        CommandError::CannotPlaceSignalOnTrack => {
            "La señal solo puede colocarse sobre vía recta (eje X o Y)."
        }
        CommandError::SignalAlreadyPresent => "Ya hay una señal en esa dirección.",
        CommandError::TileNotTerraformable => {
            "Solo se puede modificar el terreno en hierba o bosque libre."
        }
        CommandError::TerrainTooHigh => "Demasiado alto: no se puede elevar más.",
        CommandError::TerrainTooLow => "Demasiado bajo: ya está al nivel del mar.",
        CommandError::InvalidTerrainSlope => "Pendiente inválida en el vecindario.",
        CommandError::LandAlreadyOwned => "Esta tesela ya es terreno comprado.",
        CommandError::CannotBuyLandHere => {
            "Solo se puede comprar hierba o bosque libre (sin objetos ni infra)."
        }
        CommandError::IndustryNotAvailableInClimate => {
            "Esta industria no está disponible en el clima de este mapa."
        }
        CommandError::LoanAtMaximum => "El préstamo ya está al máximo permitido.",
        CommandError::NoLoanToRepay => "No hay préstamo suficiente para devolver.",
        CommandError::TownNotFound => "Ciudad no encontrada.",
        CommandError::AuthorityRatingTooLow => {
            "La autoridad local no permite construir una estación aquí."
        }
        CommandError::CannotPlantTreeHere => "No se puede plantar un árbol aquí.",
        CommandError::NoTreeHere => "No hay árbol ni cultivo en esta tesela.",
        CommandError::SignNotFound => "Cartel no encontrado.",
        CommandError::SignNameTooLong => "El nombre del cartel es demasiado largo.",
        CommandError::SignNameEmpty => "El cartel necesita un nombre.",
        CommandError::CannotJoinStations => {
            "No se pueden unir: road 1×1 adyacentes o rail (huella/eje) del mismo tipo."
        }
        CommandError::NewGrfIndexOutOfRange => "Índice NewGRF inválido.",
        CommandError::NewGrfStaticImmutable => {
            "Ese NewGRF es base y no se puede desactivar ni quitar."
        }
        CommandError::NewGrfDuplicateGrfid => "Ya hay un NewGRF con ese GRFID.",
        CommandError::NewGrfInvalidEntry => "Entrada NewGRF inválida.",
    }
}
