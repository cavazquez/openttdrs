//! Modo editor de escenarios (#42, Fase 1 MVP).

use bevy::prelude::*;

use crate::state::SimWorld;
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS,
    TerrainRoughness,
};
use openttdrs_core::Climate;

/// Sesión de editor de escenarios (no es partida normal).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EditorSession {
    pub active: bool,
}

impl EditorSession {
    #[must_use]
    pub const fn inactive() -> Self {
        Self { active: false }
    }

    #[must_use]
    pub const fn active() -> Self {
        Self { active: true }
    }
}

/// Ajustes por defecto al entrar al editor desde el menú.
#[must_use]
pub fn editor_new_game_settings() -> NewGameSettings {
    NewGameSettings {
        climate: Climate::Temperate,
        map_size: MapSizePreset::Compact,
        start_year: START_YEARS[0],
        world_gen: true,
        island: false,
        preserve_demo: false,
        seed: 42,
        town_density: PopulationDensity::Sparse,
        industry_density: PopulationDensity::Sparse,
        starting_money: STARTING_MONEY_OPTIONS[3],
        rival_ai: false,
        disasters_enabled: false,
        terrain_roughness: TerrainRoughness::Normal,
    }
}

/// Activa cheats de sandbox para editar sin límites de dinero/propiedad.
pub fn apply_editor_sandbox(sim: &mut SimWorld) {
    sim.state.cheats.enabled = true;
    sim.state.cheats.infinite_money = true;
    sim.state.cheats.magic_bulldozer = true;
}

/// Directorio de escenarios JSON del menú.
#[must_use]
pub fn scenarios_save_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("save/scenarios")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{Command, apply_command};

    #[test]
    fn editor_settings_are_sandbox_friendly() {
        let s = editor_new_game_settings();
        assert!(s.world_gen);
        assert!(!s.preserve_demo);
        assert!(!s.rival_ai);
        assert!(!s.disasters_enabled);
        assert_eq!(s.town_density, PopulationDensity::Sparse);
    }

    #[test]
    fn apply_editor_sandbox_enables_cheats() {
        let mut sim = SimWorld::from_new_game(&editor_new_game_settings());
        apply_editor_sandbox(&mut sim);
        assert!(sim.state.cheats.enabled);
        assert!(sim.state.cheats.infinite_money_active());
        assert!(sim.state.cheats.magic_bulldozer_active());
        let money_before = sim.state.economy.money;
        apply_command(&mut sim.state, &Command::CheatAddMoney(1)).unwrap();
        assert!(sim.state.economy.money >= money_before);
    }
}
