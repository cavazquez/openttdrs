//! Errores de comando (dominio).

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
    /// La construcción acuática queda demasiado cerca del borde sin
    /// `construction.freeform_edges`.
    TooCloseToMapEdge,
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
    /// El pool de estaciones alcanzó la capacidad que puede representar MAP2.
    StationPoolFull,
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
    /// No se puede demoler una tesela que ocupa un vehículo.
    VehicleInTheWay,
    InvalidDepotTile,
    /// El pool común de depósitos alcanzó la capacidad nativa.
    DepotPoolFull,
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
    /// Nombre de depósito demasiado largo.
    DepotNameTooLong,
    /// Ya existe otro depósito con ese nombre.
    DepotNameTaken,
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
    /// Hay un puente sobre una de las teselas del depósito naval.
    MustDemolishBridgeFirst,
    /// Hay un túnel en una de las teselas y no se puede limpiar automáticamente.
    MustDemolishTunnelFirst,
    /// El depósito naval sólo admite dos teselas de agua planas.
    SiteUnsuitable,
    /// Una estructura existente no puede limpiarse automáticamente.
    BuildingMustBeDemolished,
    /// Un muelle existente bloquea la limpieza automática del agua.
    MustDemolishDockFirst,
    /// Una boya existente bloquea la limpieza automática del agua.
    BuoyInTheWay,
    /// Una plataforma petrolera existente bloquea la limpieza automática del agua.
    OilRigInTheWay,
    /// Una industria existente bloquea la limpieza automática del agua.
    IndustryInTheWay,
    /// Puente sin hueco que salvar (agua o terreno más bajo bajo el tramo).
    BridgeTypeNotAvailable,
    InvalidBridgeSpan,
    /// El tablero no deja el despeje declarado por un `RoadStop` `NewGRF`.
    BridgeTooLowForRoadStop,
    /// El tablero no deja el despeje mínimo sobre una parte de una esclusa.
    BridgeTooLowForLock,
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
    /// Solo hierba/bosque libre admite faro o transmisor.
    CannotBuildObjectHere,
    /// El objeto existente tiene `ObjectFlag::CannotRemove`.
    ObjectCannotBeRemoved,
    /// Un objeto no admite limpieza automática durante otra construcción.
    ObjectInTheWay,
    /// Ya hay un faro o transmisor de ese tipo en el mapa (límite 1).
    ObjectLimitReached,
    /// Esta industria no está disponible en el clima actual del mapa.
    IndustryNotAvailableInClimate,
    /// Una industria existente no se puede limpiar automáticamente para
    /// construir otra encima.
    IndustryTileOccupied,
    /// El clear automático de una industria no puede demoler esta tesela.
    IndustryTileCannotBeCleared,
    /// La industria sólo puede ocupar edificios ya pertenecientes a un pueblo.
    IndustryMustBeBuiltInTown,
    /// La industria marítima sólo puede fundarse sobre agua navegable válida.
    IndustryMustBeBuiltOnWater,
    /// Préstamo ya al máximo permitido.
    LoanAtMaximum,
    /// No hay préstamo suficiente para devolver.
    NoLoanToRepay,
    /// Ciudad no encontrada.
    TownNotFound,
    /// Acción de autoridad no disponible (máscara / settings / cooldown).
    TownActionNotAvailable,
    /// No hay sitio libre para la estatua.
    StatueNoPlace,
    /// No se puede fundar pueblo en esta tesela.
    CannotFoundTownHere,
    /// Hay otro pueblo demasiado cerca.
    TownTooClose,
    /// Cheats desactivados.
    CheatsDisabled,
    /// Año de cheat fuera de rango.
    InvalidCheatYear,
    /// Compañía no encontrada en el pool.
    CompanyNotFound,
    /// El color ya lo usa otra compañía.
    CompanyColourTaken,
    /// No se puede comprar la propia compañía.
    CannotBuyOwnCompany,
    /// La compañía rival no está en quiebra.
    CompanyNotBankrupt,
    /// La autoridad local no permite construir aquí.
    AuthorityRatingTooLow,
    /// El pueblo rechaza el aeropuerto por exceso de ruido.
    AirportNoiseTooHigh,
    /// El layout Action0 solicitado no existe para el aeropuerto `NewGRF` activo.
    InvalidAirportLayout,
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
    /// Índice de parámetro `NewGRF` fuera de rango (`≥ 128`).
    NewGrfParamOutOfRange,
    /// Spec de road stop incompatible con bus/camión.
    RoadStopSpecTypeMismatch,
    /// Spec `RoadOnly`/`TramOnly` incompatible con el tipo de vía actual.
    RoadStopRoadTypeMismatch,
    /// Spec `DriveThroughOnly` colocado como bahía (o viceversa no aplicable).
    RoadStopDriveThroughRequired,
    /// Callback `NewGRF` denegó la acción (p. ej. `CBID_VEHICLE_START_STOP_CHECK`).
    NewGrfCallbackDenied,
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
