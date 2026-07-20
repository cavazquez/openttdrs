//! Fases de boca para depósitos de buses, camiones y tranvías.

use crate::road_movement::straight_subtile;
use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW, RoadDepotPhase, VehicleDirection};

/// Frame 6 de `_road_drive_data`, convertido a progreso sub-tesela.
pub const ROAD_DEPOT_EXIT_START: u8 = 102;
/// Frame 11 de `_road_drive_data`: el vehículo entra y se oculta.
pub const ROAD_DEPOT_ENTRY_STOP: u8 = 187;
/// Avance visual por tick mientras cruza la boca.
pub const ROAD_DEPOT_PROGRESS_STEP: u8 = 32;

/// Dirección gráfica orientada desde la boca hacia la red.
#[must_use]
pub const fn road_depot_exit_direction(mouth: u8) -> VehicleDirection {
    match mouth & 0x03 {
        0 => DIR_NE,
        1 => DIR_SE,
        2 => DIR_SW,
        _ => DIR_NW,
    }
}

/// Dirección inversa para la entrada desde la red.
#[must_use]
pub const fn road_depot_entry_direction(mouth: u8) -> VehicleDirection {
    (road_depot_exit_direction(mouth) + 4) % 8
}

/// Pose sub-tesela de un vehículo road animando la boca del depósito.
#[must_use]
pub fn road_depot_subtile(phase: RoadDepotPhase) -> Option<(f32, f32)> {
    match phase {
        RoadDepotPhase::Entering {
            direction,
            progress,
        }
        | RoadDepotPhase::Exiting {
            direction,
            progress,
        } => Some(straight_subtile(direction, f32::from(progress))),
        _ => None,
    }
}

/// Dirección de sprite mientras atraviesa la boca del depósito.
#[must_use]
pub const fn road_depot_direction(phase: RoadDepotPhase) -> Option<VehicleDirection> {
    match phase {
        RoadDepotPhase::Entering { direction, .. } | RoadDepotPhase::Exiting { direction, .. } => {
            Some(direction)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouth_directions_are_opposites() {
        for mouth in 0..4 {
            assert_eq!(
                road_depot_entry_direction(mouth),
                (road_depot_exit_direction(mouth) + 4) % 8
            );
        }
    }

    #[test]
    fn animated_phase_has_subtile() {
        assert!(road_depot_subtile(RoadDepotPhase::InDepot).is_none());
        assert!(
            road_depot_subtile(RoadDepotPhase::Exiting {
                direction: DIR_NE,
                progress: ROAD_DEPOT_EXIT_START,
            })
            .is_some()
        );
    }
}
