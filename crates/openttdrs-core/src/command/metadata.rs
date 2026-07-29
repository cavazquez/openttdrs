use crate::map::TileCoord;
use crate::sim_events::ConstructionKind;

use super::types::Command;

/// Efectos colaterales de un comando exitoso (eventos de audio, invalidaciones).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandEffects {
    /// El comando modifica el mapa (requiere invalidar caminos cacheados).
    pub modifies_map: bool,
    /// Evento de construcción opcional (SFX + coord).
    pub construction_event: Option<(ConstructionKind, TileCoord)>,
    /// Evento de demolición opcional (SFX explosión + coord).
    pub demolition_event: Option<TileCoord>,
}

impl CommandEffects {
    /// Constructor para comandos sin efectos colaterales (mayoría de comandos de vehículos).
    pub const fn none() -> Self {
        Self {
            modifies_map: false,
            construction_event: None,
            demolition_event: None,
        }
    }

    /// Constructor para comandos con construcción.
    pub const fn construction(kind: ConstructionKind, at: TileCoord) -> Self {
        Self {
            modifies_map: true,
            construction_event: Some((kind, at)),
            demolition_event: None,
        }
    }

    /// Constructor para comandos con demolición.
    pub const fn demolition(at: TileCoord) -> Self {
        Self {
            modifies_map: true,
            construction_event: None,
            demolition_event: Some(at),
        }
    }

    /// Constructor para comandos que modifican mapa sin eventos.
    pub const fn map_only() -> Self {
        Self {
            modifies_map: true,
            construction_event: None,
            demolition_event: None,
        }
    }
}

/// Retorna los efectos colaterales (eventos, invalidaciones) de `cmd` si se ejecutara con éxito.
///
/// Esta función es exhaustiva: cubre todas las variantes de [`Command`].
#[must_use]
#[allow(clippy::too_many_lines, clippy::match_same_arms)]
pub fn command_effects(cmd: &Command) -> CommandEffects {
    use ConstructionKind::{Bridge, Other, Rail, Road};

    match cmd {
        // ═══════════════════════════════════════════════════════════════════
        // Construcción de vía (Rail)
        // ═══════════════════════════════════════════════════════════════════
        Command::PlaceRail(c)
        | Command::PlaceRailBits(c, _)
        | Command::SetRailBits(c, _)
        | Command::PlaceRailWaypoint(c)
        | Command::PlaceRailDepot(c)
        | Command::PlaceRailDepotDir(c, _)
        | Command::PlaceRailSignal(c, _, _, _, _)
        | Command::PlaceRailSignalWithVariant(c, _, _, _, _, _)
        | Command::CycleRailSignalType(c, _, _)
        | Command::CycleRailSignalVariant(c, _, _)
        | Command::RemoveRailSignal(c, _, _)
        | Command::PlaceRailStation(c, _)
        | Command::PlaceRailTunnel(c, _) => CommandEffects::construction(Rail, *c),

        Command::PlaceRailStationArea { origin, .. } => CommandEffects::construction(Rail, *origin),

        // Quitar vía suena al SFX de rail (como en OpenTTD), no a explosión.
        Command::RemoveRail(c) | Command::RemoveRailBits(c, _) => {
            CommandEffects::construction(Rail, *c)
        }

        // Conversión de vía (modifica mapa pero no genera evento audio explícito).
        Command::ConvertRail(..) => CommandEffects::map_only(),

        // ═══════════════════════════════════════════════════════════════════
        // Construcción de puentes (Bridge)
        // ═══════════════════════════════════════════════════════════════════
        Command::PlaceRailBridge(c, _, _)
        | Command::PlaceRoadBridge(c, _, _)
        | Command::PlaceAqueduct(c, _) => CommandEffects::construction(Bridge, *c),

        // ═══════════════════════════════════════════════════════════════════
        // Construcción de carretera (Road)
        // ═══════════════════════════════════════════════════════════════════
        Command::PlaceRoad(c)
        | Command::PlaceRoadBits(c, _)
        | Command::PlaceTramBits(c, _)
        | Command::RemoveTramBits(c)
        | Command::SetRoadBits(c, _)
        | Command::PlaceRoadDepot(c)
        | Command::PlaceRoadDepotDir(c, _)
        | Command::PlaceRoadWaypoint(c)
        | Command::PlaceShipDepotDir(c, _)
        | Command::PlaceDock(c, _)
        | Command::PlaceAirport(c)
        | Command::PlaceCanal(c)
        | Command::PlaceRiver(c)
        | Command::PlaceBuoy(c)
        | Command::PlaceLock(c, _)
        | Command::PlaceStation(c)
        | Command::PlaceStationDir(c, _)
        | Command::PlaceBusStop(c, _)
        | Command::PlaceTruckStop(c, _)
        | Command::PlaceRoadTunnel(c, _) => CommandEffects::construction(Road, *c),

        Command::PlaceAirportArea { origin: c, .. } => CommandEffects::construction(Road, *c),

        // ═══════════════════════════════════════════════════════════════════
        // Construcción Other (terraform, objetos, industrias, casas, pueblos)
        // ═══════════════════════════════════════════════════════════════════
        Command::BuyLand(c)
        | Command::BuildObject { pos: c, .. }
        | Command::RaiseLand(c)
        | Command::LowerLand(c)
        | Command::PlaceIndustry(c)
        | Command::PlaceIndustryKind(c, _)
        | Command::PlaceIndustrySpec(c, _)
        | Command::PlaceHouse(c)
        | Command::PlaceForest(c)
        | Command::FoundTown(c) => CommandEffects::construction(Other, *c),

        Command::BuyLandArea { from, .. } | Command::LevelLand { from, .. } => {
            CommandEffects::construction(Other, *from)
        }

        // ═══════════════════════════════════════════════════════════════════
        // Demolición (ClearTile)
        // ═══════════════════════════════════════════════════════════════════
        Command::ClearTile(c) => CommandEffects::demolition(*c),

        // ═══════════════════════════════════════════════════════════════════
        // Árboles (modifican mapa pero sin eventos de construcción/demolición)
        // ═══════════════════════════════════════════════════════════════════
        Command::PlantTree(_) | Command::ClearTree(_) => CommandEffects::map_only(),

        // ═══════════════════════════════════════════════════════════════════
        // Unión de estaciones (modifica mapa)
        // ═══════════════════════════════════════════════════════════════════
        Command::JoinStations { .. } => CommandEffects::map_only(),

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de vehículos (NO modifican mapa)
        // ═══════════════════════════════════════════════════════════════════
        Command::SetVehicleOrders(..)
        | Command::SetVehicleStationOrders(..)
        | Command::SetVehicleOrderList(..)
        | Command::BuildRoadVehicleAtDepot(..)
        | Command::BuildVehicleAtDepot(..)
        | Command::AttachWagonToConsist { .. }
        | Command::DetachConsistUnit(..)
        | Command::MoveRailVehicle { .. }
        | Command::SellVehicle(..)
        | Command::ToggleVehicleRunning(..)
        | Command::CloneVehicleOrders { .. }
        | Command::CloneVehicleAtDepot { .. }
        | Command::SellAllVehiclesAtDepot(..)
        | Command::RemoveVehicleOrderAt { .. }
        | Command::SkipVehicleOrder(..)
        | Command::ToggleVehicleOrderFullLoad { .. }
        | Command::ToggleVehicleOrderNoUnload { .. }
        | Command::AppendGotoNearestDepot(..)
        | Command::RenameVehicle { .. }
        | Command::SetDepotVehiclesRunning { .. }
        | Command::MoveVehicleOrder { .. }
        | Command::ToggleVehicleOrderDepotStop { .. }
        | Command::ToggleVehicleOrderDepotUnbunch { .. }
        | Command::SetVehicleOrderMaxSpeed { .. }
        | Command::TurnAroundVehicle(..)
        | Command::ForceVehicleProceed(..)
        | Command::RefitVehicle { .. }
        | Command::CycleVehicleOrderDepotRefit { .. }
        | Command::ToggleVehicleTimetable(..)
        | Command::CycleVehicleOrderWait { .. }
        | Command::CycleVehicleOrderTravel { .. }
        | Command::SetAutoReplaceRule { .. }
        | Command::ClearAutoReplaceRule { .. }
        | Command::ToggleAutoReplaceRule { .. }
        | Command::CreateVehicleGroup { .. }
        | Command::RenameVehicleGroup { .. }
        | Command::AssignVehicleToGroup { .. }
        | Command::ClearVehicleTimetableLateness(..)
        | Command::SetVehicleOrderWaitTicks { .. }
        | Command::SetVehicleOrderTravelTicks { .. }
        | Command::ToggleVehicleTimetableAutofill(..)
        | Command::SetVehicleTimetableStart { .. }
        | Command::ToggleAutoReplaceOnlyWhenOld { .. }
        | Command::SetAutoReplaceRuleGroup { .. }
        | Command::DepotMassAutoreplace { .. }
        | Command::CreateSharedOrdersFromVehicle(..)
        | Command::LinkVehicleToSharedOrders { .. }
        | Command::UnlinkVehicleSharedOrders(..)
        | Command::SetSharedOrderAt { .. }
        | Command::SetVehicleOrderConditional { .. }
        | Command::DepotReorderVehicleSlot { .. } => CommandEffects::none(),

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de estaciones (NO modifican mapa, solo metadata)
        // ═══════════════════════════════════════════════════════════════════
        Command::RenameStation { .. } => CommandEffects::none(),

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de carteles (NO modifican mapa, solo overlays)
        // ═══════════════════════════════════════════════════════════════════
        Command::PlaceSign { .. } | Command::RemoveSign { .. } | Command::RenameSign { .. } => {
            CommandEffects::none()
        }

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de economía (NO modifican mapa)
        // ═══════════════════════════════════════════════════════════════════
        Command::IncreaseLoan | Command::DecreaseLoan | Command::BuyCompany(_) => {
            CommandEffects::none()
        }

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de ciudad (NO modifican mapa, solo flags)
        // ═══════════════════════════════════════════════════════════════════
        Command::TownAdvertise(..)
        | Command::TownFundBuildings(..)
        | Command::DoTownAction { .. } => CommandEffects::none(),

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de cheats (NO modifican mapa)
        // ═══════════════════════════════════════════════════════════════════
        Command::CheatSetEnabled(..)
        | Command::CheatAddMoney(..)
        | Command::CheatToggleInfiniteMoney
        | Command::CheatToggleMagicBulldozer
        | Command::CheatSetYear(..)
        | Command::CheatSwitchCompany(..) => CommandEffects::none(),

        // ═══════════════════════════════════════════════════════════════════
        // Comandos de NewGRF config (NO modifican mapa, solo config)
        // ═══════════════════════════════════════════════════════════════════
        Command::SetNewGrfEnabled { .. }
        | Command::MoveNewGrfInStack { .. }
        | Command::RemoveNewGrfFromStack { .. }
        | Command::AddNewGrfToStack { .. }
        | Command::SetNewGrfParam { .. } => CommandEffects::none(),

        // ═══════════════════════════════════════════════════════════════════
        // Settings de partida (NO modifican mapa)
        // ═══════════════════════════════════════════════════════════════════
        Command::SetPathfindingSettings(..)
        | Command::SetConstructionSettings(..)
        | Command::SetVehicleBreakdowns(..)
        | Command::SetCargoDistDistribution(..)
        | Command::SetCompanyColour(..)
        | Command::SetCurrentRailType(..)
        | Command::SetCurrentRoadType(..)
        | Command::SetCurrentTramType(..)
        | Command::SetCurrentStationClass(..)
        | Command::SetCurrentStationSpec(..)
        | Command::SetCurrentRoadStopClass(..)
        | Command::SetCurrentRoadStopSpec(..)
        | Command::SetCurrentAirportClass(..)
        | Command::SetCurrentAirportSpec(..)
        | Command::SetCurrentObjectSpec(..)
        | Command::SetAiSettings(..) => CommandEffects::none(),

        Command::FinalizeRoadDragLine { .. } | Command::RegenerateLandscape { .. } => {
            CommandEffects::map_only()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_effects_construction() {
        let cmd = Command::PlaceRail(TileCoord { x: 10, y: 20 });
        let effects = command_effects(&cmd);
        assert!(effects.modifies_map);
        assert_eq!(
            effects.construction_event,
            Some((ConstructionKind::Rail, TileCoord { x: 10, y: 20 }))
        );
        assert_eq!(effects.demolition_event, None);
    }

    #[test]
    fn test_command_effects_demolition() {
        let cmd = Command::ClearTile(TileCoord { x: 5, y: 15 });
        let effects = command_effects(&cmd);
        assert!(effects.modifies_map);
        assert_eq!(effects.construction_event, None);
        assert_eq!(effects.demolition_event, Some(TileCoord { x: 5, y: 15 }));
    }

    #[test]
    fn test_command_effects_vehicle_no_map() {
        let cmd = Command::ToggleVehicleRunning(42);
        let effects = command_effects(&cmd);
        assert!(!effects.modifies_map);
        assert_eq!(effects.construction_event, None);
        assert_eq!(effects.demolition_event, None);
    }

    #[test]
    fn test_command_effects_map_only() {
        let cmd = Command::ConvertRail(TileCoord { x: 1, y: 2 }, 1);
        let effects = command_effects(&cmd);
        assert!(effects.modifies_map);
        assert_eq!(effects.construction_event, None);
        assert_eq!(effects.demolition_event, None);
    }
}
