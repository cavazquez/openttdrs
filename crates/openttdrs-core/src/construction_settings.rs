//! Ajustes de construcción que afectan la representación del mapa.

/// Lado de circulación de vehículos de carretera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoadVehicleDrivingSide {
    #[default]
    Left,
    Right,
}

/// Política de lado para señales ferroviarias (`TrainSignalSide` de `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainSignalSide {
    Left,
    #[default]
    RoadVehicleDrivingSide,
    Right,
}

/// Ajustes persistentes de construcción/conducción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ConstructionSettings {
    #[serde(default)]
    pub train_signal_side: TrainSignalSide,
    #[serde(default)]
    pub road_vehicle_driving_side: RoadVehicleDrivingSide,
}

impl ConstructionSettings {
    /// Resuelve el modo relativo al lado de circulación.
    #[must_use]
    pub const fn signals_on_right(self) -> bool {
        match self.train_signal_side {
            TrainSignalSide::Left => false,
            TrainSignalSide::Right => true,
            TrainSignalSide::RoadVehicleDrivingSide => {
                matches!(
                    self.road_vehicle_driving_side,
                    RoadVehicleDrivingSide::Right
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_side_can_follow_road_driving_side() {
        let mut settings = ConstructionSettings::default();
        assert!(!settings.signals_on_right());
        settings.road_vehicle_driving_side = RoadVehicleDrivingSide::Right;
        assert!(settings.signals_on_right());
        settings.train_signal_side = TrainSignalSide::Left;
        assert!(!settings.signals_on_right());
    }
}
