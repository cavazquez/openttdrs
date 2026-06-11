use crate::map::TileCoord;
use crate::{IndustryKind, IndustrySpec, VehicleKind};

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Coloca o combina una pieza de carretera `OpenTTD` (`RoadBits`, bits 0..3).
    PlaceRoadBits(TileCoord, u8),
    /// Reemplaza la geometría de carretera de la tesela con `RoadBits` exactos.
    SetRoadBits(TileCoord, u8),
    /// Coloca via de tren en la tesela (MVP: validacion de terreno).
    PlaceRail(TileCoord),
    /// Coloca o combina `TrackBits` de vía (`0x0C` HORZ, `0x30` VERT, etc.).
    PlaceRailBits(TileCoord, u8),
    /// Reemplaza la geometría de vía con `TrackBits` exactos (drag de herramientas HORZ/VERT).
    SetRailBits(TileCoord, u8),
    PlaceRoadDepot(TileCoord),
    PlaceRoadDepotDir(TileCoord, u8),
    PlaceRailDepot(TileCoord),
    /// Depósito de tren con orientación de entrada `0..3`.
    PlaceRailDepotDir(TileCoord, u8),
    PlaceRoadTunnel(TileCoord, TileCoord),
    PlaceRailTunnel(TileCoord, TileCoord),
    PlaceRoadBridge(TileCoord, TileCoord),
    PlaceRailBridge(TileCoord, TileCoord),
    SetVehicleOrders(u32, Vec<TileCoord>),
    SetVehicleStationOrders(u32, Vec<TileCoord>),
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
    SellVehicle(u32),
    ToggleVehicleRunning(u32),
    CloneVehicleOrders {
        from_vehicle_id: u32,
        to_vehicle_id: u32,
    },
    /// Limpia la tesela y vuelve a `TileKind::Grass`.
    ClearTile(TileCoord),
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
    StationNotFound,
    VehicleNotFound,
    /// Solo se puede vender un vehículo estacionado en un depósito.
    VehicleNotInDepot,
    InvalidDepotTile,
    VehicleKindNotAllowed,
    /// El motor pedido no existe en el catálogo.
    EngineNotFound,
    /// No hay dinero suficiente para pagar la compra.
    InsufficientFunds,
    IncompatibleStopForVehicle,
    /// Extremos de túnel inválidos (pendiente, salida, etc.).
    InvalidTunnelEndpoints,
    /// Puente sin hueco que salvar (agua o terreno más bajo bajo el tramo).
    InvalidBridgeSpan,
}

/// Texto breve en español para mostrar al jugador cuando falla un comando.
#[must_use]
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
        CommandError::StationNotFound => "No hay estación en esta tesela.",
        CommandError::VehicleNotFound => "Vehículo no encontrado.",
        CommandError::VehicleNotInDepot => {
            "Solo se puede vender un vehículo dentro de un depósito."
        }
        CommandError::InvalidDepotTile => "Ubicación de depósito inválida.",
        CommandError::VehicleKindNotAllowed => "Tipo de vehículo no permitido aquí.",
        CommandError::EngineNotFound => "Modelo de vehículo desconocido.",
        CommandError::InsufficientFunds => "No hay dinero suficiente.",
        CommandError::IncompatibleStopForVehicle => "Parada incompatible con este vehículo.",
        CommandError::InvalidTunnelEndpoints => {
            "Túnel inválido: entrada en pendiente inclinada (NE/SE/SW/NW) y salida al mismo nivel."
        }
        CommandError::InvalidBridgeSpan => {
            "Puente inválido: las orillas al mismo nivel y agua o terreno más bajo bajo el tramo."
        }
    }
}
