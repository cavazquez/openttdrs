//! Mensajes de error de comando en español (UI).

use openttdrs_core::CommandError;

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
        CommandError::CannotBuildObjectHere => {
            "Solo se puede colocar el faro/transmisor en hierba o bosque libre."
        }
        CommandError::ObjectLimitReached => "Ya hay un faro o transmisor de ese tipo en el mapa.",
        CommandError::IndustryNotAvailableInClimate => {
            "Esta industria no está disponible en el clima de este mapa."
        }
        CommandError::LoanAtMaximum => "El préstamo ya está al máximo permitido.",
        CommandError::NoLoanToRepay => "No hay préstamo suficiente para devolver.",
        CommandError::TownNotFound => "Ciudad no encontrada.",
        CommandError::TownActionNotAvailable => "Esa acción de autoridad no está disponible ahora.",
        CommandError::StatueNoPlace => "No hay sitio libre para la estatua.",
        CommandError::CannotFoundTownHere => "No se puede fundar un pueblo aquí.",
        CommandError::TownTooClose => "Hay otro pueblo demasiado cerca.",
        CommandError::CheatsDisabled => "Cheats desactivados (consola: cheat on).",
        CommandError::InvalidCheatYear => "Año de cheat inválido (1950–2450).",
        CommandError::CompanyNotFound => "Compañía no encontrada.",
        CommandError::CompanyColourTaken => "Ese color ya lo usa otra compañía.",
        CommandError::CannotBuyOwnCompany => "No puedes comprar tu propia compañía.",
        CommandError::CompanyNotBankrupt => "La compañía no está en quiebra.",
        CommandError::AuthorityRatingTooLow => {
            "La autoridad local no permite construir una estación aquí."
        }
        CommandError::AirportNoiseTooHigh => {
            "La autoridad local rechaza el aeropuerto: demasiado ruido."
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
        CommandError::NewGrfParamOutOfRange => "Índice de parámetro NewGRF inválido.",
        CommandError::RoadStopSpecTypeMismatch => {
            "Esta parada NewGRF no admite bus o camión en esta herramienta."
        }
        CommandError::RoadStopRoadTypeMismatch => {
            "Esta parada NewGRF no admite el tipo de vía actual (carretera/tranvía)."
        }
        CommandError::RoadStopDriveThroughRequired => {
            "Esta parada NewGRF solo admite colocación drive-through."
        }
        CommandError::NewGrfCallbackDenied => "Un NewGRF denegó esta acción (callback).",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_all_errors_have_messages() {
        // Lista exhaustiva de errores para asegurar que todos tienen mensaje.
        let errors = [
            CommandError::OutOfBounds,
            CommandError::CannotPlaceRoadOnWater,
            CommandError::CannotPlaceRoadOnVoid,
            CommandError::CannotPlaceRailOnWater,
            CommandError::CannotPlaceRailOnVoid,
            CommandError::CannotPlaceStationOnWater,
            CommandError::CannotPlaceStationOnVoid,
            CommandError::CannotPlaceStationOnOccupiedTile,
            CommandError::StationNotAdjacentToTransport,
            CommandError::StationAlreadyExists,
            CommandError::StationSizeNotAllowed,
            CommandError::StationNotFound,
            CommandError::VehicleNotFound,
            CommandError::VehicleNotOwned,
            CommandError::TileNotOwned,
            CommandError::VehicleNotInDepot,
            CommandError::InvalidDepotTile,
            CommandError::VehicleKindNotAllowed,
            CommandError::EngineNotFound,
            CommandError::InsufficientFunds,
            CommandError::IncompatibleStopForVehicle,
            CommandError::OrderIndexOutOfRange,
            CommandError::OrderFlagNotApplicable,
            CommandError::DepotNotFound,
            CommandError::VehicleNameTooLong,
            CommandError::StationNameTooLong,
            CommandError::RefitNotAllowed,
            CommandError::TimetableNotApplicable,
            CommandError::AutoreplaceNotAllowed,
            CommandError::AutoReplaceRuleNotFound,
            CommandError::VehicleGroupNotFound,
            CommandError::VehicleGroupNameInvalid,
            CommandError::SharedOrdersNotFound,
            CommandError::TimetableWaitPending,
            CommandError::InvalidTunnelEndpoints,
            CommandError::BridgeTypeNotAvailable,
            CommandError::InvalidBridgeSpan,
            CommandError::InvalidRailOnSlope,
            CommandError::CannotPlaceWaypointOnTrack,
            CommandError::NoRailToRemove,
            CommandError::NoTramToRemove,
            CommandError::NoRailToConvert,
            CommandError::TrainIncompatibleWithRailType,
            CommandError::EngineRequiresElectricRail,
            CommandError::EngineRequiresMonorail,
            CommandError::EngineRequiresMaglev,
            CommandError::CannotPlaceSignalOnTrack,
            CommandError::SignalAlreadyPresent,
            CommandError::TileNotTerraformable,
            CommandError::TerrainTooHigh,
            CommandError::TerrainTooLow,
            CommandError::InvalidTerrainSlope,
            CommandError::LandAlreadyOwned,
            CommandError::CannotBuyLandHere,
            CommandError::CannotBuildObjectHere,
            CommandError::ObjectLimitReached,
            CommandError::IndustryNotAvailableInClimate,
            CommandError::LoanAtMaximum,
            CommandError::NoLoanToRepay,
            CommandError::TownNotFound,
            CommandError::TownActionNotAvailable,
            CommandError::StatueNoPlace,
            CommandError::CannotFoundTownHere,
            CommandError::TownTooClose,
            CommandError::CheatsDisabled,
            CommandError::InvalidCheatYear,
            CommandError::CompanyNotFound,
            CommandError::CompanyColourTaken,
            CommandError::CannotBuyOwnCompany,
            CommandError::CompanyNotBankrupt,
            CommandError::AuthorityRatingTooLow,
            CommandError::AirportNoiseTooHigh,
            CommandError::CannotPlantTreeHere,
            CommandError::NoTreeHere,
            CommandError::SignNotFound,
            CommandError::SignNameTooLong,
            CommandError::SignNameEmpty,
            CommandError::CannotJoinStations,
            CommandError::NewGrfIndexOutOfRange,
            CommandError::NewGrfStaticImmutable,
            CommandError::NewGrfDuplicateGrfid,
            CommandError::NewGrfInvalidEntry,
            CommandError::NewGrfParamOutOfRange,
            CommandError::RoadStopSpecTypeMismatch,
            CommandError::RoadStopRoadTypeMismatch,
            CommandError::RoadStopDriveThroughRequired,
            CommandError::NewGrfCallbackDenied,
        ];
        for err in errors {
            let msg = command_error_message(err);
            assert!(!msg.is_empty(), "{err:?}");
            assert!(
                msg.chars().any(char::is_alphabetic),
                "{err:?}: '{msg}' sin letras"
            );
        }
    }
}
