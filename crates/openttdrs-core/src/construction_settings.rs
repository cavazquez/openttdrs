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
#[allow(clippy::struct_excessive_bools)]
pub struct ConstructionSettings {
    /// Altura máxima de mapa (`construction.map_height_limit`).
    ///
    /// El valor `0` conserva el modo automático de `OpenTTD`; al crear el
    /// mundo, `GenerateWorld` lo reemplaza por la estimación TGP + 15 (como
    /// mínimo 30) antes de generar industrias y árboles. Mantener el valor
    /// crudo permite reemitir el setting sin perder la preferencia de una
    /// partida nueva; la resolución dependiente del relieve vive en
    /// `world_gen::effective_new_game_map_height_limit`.
    #[serde(default)]
    pub map_height_limit: u8,
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
    /// Permite unir una parte nueva a una estación no adyacente
    /// (`station.distant_join_stations`).
    ///
    /// `OpenTTD` lo habilita por defecto desde `SLV_106`. El comando reducido
    /// `JoinStations` lo consulta antes de fusionar dos estaciones existentes.
    #[serde(default = "default_distant_join_stations")]
    pub distant_join_stations: bool,
    /// Aplica el límite de velocidad declarado por cada vagón
    /// (`vehicle.wagon_speed_limits`).
    ///
    /// `OpenTTD` lo habilita por defecto. Cuando se desactiva, la velocidad
    /// máxima del consist no queda limitada por unidades que sean vagones.
    #[serde(default = "default_wagon_speed_limits")]
    pub wagon_speed_limits: bool,
    /// Desactiva la red eléctrica vanilla (`vehicle.disable_elrails`).
    ///
    /// Al activarlo, las locomotoras eléctricas se consideran aptas para rail
    /// normal y una conversión Electric → Rail no necesita cambiar la tesela.
    #[serde(default)]
    pub disable_elrails: bool,
}

const fn default_freeform_edges() -> bool {
    true
}

const fn default_distant_join_stations() -> bool {
    true
}

const fn default_wagon_speed_limits() -> bool {
    true
}

impl Default for ConstructionSettings {
    fn default() -> Self {
        Self {
            map_height_limit: 0,
            train_signal_side: TrainSignalSide::default(),
            road_vehicle_driving_side: RoadVehicleDrivingSide::default(),
            freeform_edges: default_freeform_edges(),
            distant_join_stations: default_distant_join_stations(),
            wagon_speed_limits: default_wagon_speed_limits(),
            disable_elrails: false,
        }
    }
}

impl ConstructionSettings {
    /// Límite efectivo para un estado genérico sin configuración de relieve.
    ///
    /// Un save ya cargado conserva el valor resuelto por `GenerateWorld`.
    /// Las partidas nuevas deben usar
    /// `world_gen::effective_new_game_map_height_limit`, que incorpora la
    /// estimación TGP del tamaño y relieve elegidos.
    #[must_use]
    pub const fn effective_map_height_limit(self) -> u8 {
        if self.map_height_limit == 0 {
            30
        } else {
            self.map_height_limit
        }
    }

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

    #[test]
    fn distant_join_stations_follows_openttd_default() {
        assert!(ConstructionSettings::default().distant_join_stations);
    }

    #[test]
    fn wagon_speed_limits_follows_openttd_default() {
        assert!(ConstructionSettings::default().wagon_speed_limits);
    }

    #[test]
    fn disable_elrails_is_off_by_default() {
        assert!(!ConstructionSettings::default().disable_elrails);
    }

    #[test]
    fn automatic_map_height_resolves_to_openttd_minimum() {
        assert_eq!(ConstructionSettings::default().map_height_limit, 0);
        assert_eq!(
            ConstructionSettings::default().effective_map_height_limit(),
            30
        );

        let settings = ConstructionSettings {
            map_height_limit: 75,
            ..ConstructionSettings::default()
        };
        assert_eq!(settings.effective_map_height_limit(), 75);
    }
}
