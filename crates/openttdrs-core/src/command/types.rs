use crate::bridge_spec::BridgeType;
use crate::map::TileCoord;
use crate::{IndustryKind, IndustrySpec, VehicleKind};

use super::error::OrderMoveDirection;

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
    /// Alterna «parar en depósito» en una orden [`crate::vehicle::VehicleOrder::Depot`].
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
    /// Cicla el refit automático de una orden [`crate::vehicle::VehicleOrder::Depot`].
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
    /// Coloca faro o transmisor vanilla (`CmdBuildObject`).
    BuildObject {
        pos: TileCoord,
        /// `OBJECT_TYPE_TRANSMITTER` o `OBJECT_TYPE_LIGHTHOUSE`.
        object_type: u8,
    },
    /// Solicita más préstamo bancario (`CmdIncreaseLoan`).
    IncreaseLoan,
    /// Devuelve parte del préstamo (`CmdDecreaseLoan`).
    DecreaseLoan,
    /// Compra una compañía rival en quiebra (`CmdBuyCompany`).
    BuyCompany(crate::company::CompanyId),
    /// Campaña publicitaria en una ciudad (`CmdTownAction::Advertise`).
    TownAdvertise(u32),
    /// Financia edificios en una ciudad (`CmdTownAction::FundBuildings`).
    TownFundBuildings(u32),
    /// Funda un pueblo nuevo en hierba (`CmdBuildTown`).
    FoundTown(TileCoord),
    /// Activa/desactiva cheats formales.
    CheatSetEnabled(bool),
    /// Añade dinero a la compañía activa (requiere cheats enabled).
    CheatAddMoney(i64),
    /// Alterna dinero infinito.
    CheatToggleInfiniteMoney,
    /// Alterna magic bulldozer (demoler sin dueño).
    CheatToggleMagicBulldozer,
    /// Cambia el año de calendario (`change_date` de `OpenTTD`).
    CheatSetYear(u32),
    /// Cambia la compañía activa (`switch_company`).
    CheatSwitchCompany(crate::company::CompanyId),
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
    /// Escribe un parámetro del GRF (`param[param_index] = value`).
    SetNewGrfParam {
        index: usize,
        param_index: u8,
        value: u32,
    },
    /// Sustituye los ajustes PBS / pathfinding de la partida (deuda I8 settings).
    SetPathfindingSettings(crate::pathfinding_settings::PathfindingSettings),
    /// Cambia el modo CargoDist y reconstruye flows de estación.
    SetCargoDistDistribution(crate::flow_stat::DistributionType),
    /// Color de la compañía activa (0..=15).
    SetCompanyColour(u8),
    /// Tipo de vía activo para construcción.
    SetCurrentRailType(crate::rail_type::RailType),
    /// Tipo de carretera activo para construcción.
    SetCurrentRoadType(crate::road_type::RoadType),
    /// Tipo de tranvía activo para construcción.
    SetCurrentTramType(crate::road_type::RoadType),
    /// Clase de estación activa (elige el primer spec de esa clase).
    SetCurrentStationClass(crate::station_class::StationClassId),
    /// Spec de estación activo.
    SetCurrentStationSpec(crate::station_class::StationSpecId),
    /// Clase de aeropuerto activa (elige el primer spec de esa clase).
    SetCurrentAirportClass(crate::airport_class::AirportClassId),
    /// Spec de aeropuerto activo.
    SetCurrentAirportSpec(crate::airport_class::AirportSpecId),
    /// Ajustes de IA TransCargo de la partida.
    SetAiSettings(crate::ai::AiSettings),
}
