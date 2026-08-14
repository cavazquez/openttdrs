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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConstructionSettings {
    #[serde(default)]
    pub train_signal_side: TrainSignalSide,
    #[serde(default)]
    pub road_vehicle_driving_side: RoadVehicleDrivingSide,
    /// Bordes libres del mapa (`construction.freeform_edges`).
    ///
    /// `OpenTTD` lo habilita por defecto: las teselas `Void` se dibujan como
    /// terreno desnudo con la paleta negra, no como agua infinita.
    #[serde(default = "default_freeform_edges")]
    pub freeform_edges: bool,
}

const fn default_freeform_edges() -> bool {
    true
}

impl Default for ConstructionSettings {
    fn default() -> Self {
        Self {
            train_signal_side: TrainSignalSide::default(),
            road_vehicle_driving_side: RoadVehicleDrivingSide::default(),
            freeform_edges: default_freeform_edges(),
        }
    }
}

impl ConstructionSettings {
    /// `true` si los vehículos de carretera circulan por la derecha.
    #[must_use]
    pub const fn road_drive_on_right(self) -> bool {
        matches!(
            self.road_vehicle_driving_side,
            RoadVehicleDrivingSide::Right
        )
    }

    /// Resuelve el modo relativo al lado de circulación.
    #[must_use]
    pub const fn signals_on_right(self) -> bool {
        match self.train_signal_side {
            TrainSignalSide::Left => false,
            TrainSignalSide::Right => true,
            TrainSignalSide::RoadVehicleDrivingSide => self.road_drive_on_right(),
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

    #[test]
    fn road_drive_on_right_tracks_setting() {
        let mut settings = ConstructionSettings::default();
        assert!(!settings.road_drive_on_right());
        settings.road_vehicle_driving_side = RoadVehicleDrivingSide::Right;
        assert!(settings.road_drive_on_right());
    }

    #[test]
    fn freeform_edges_follow_openttd_default() {
        assert!(ConstructionSettings::default().freeform_edges);
    }
}
