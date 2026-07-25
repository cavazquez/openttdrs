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
        gamescript_demo: false,
    }
}

/// Activa cheats de sandbox para editar sin límites de dinero/propiedad.
pub fn apply_editor_sandbox(sim: &mut SimWorld) {
    use openttdrs_core::Command;
    use openttdrs_core::prelude::apply_command;

    let _ = apply_command(&mut sim.state, &Command::CheatSetEnabled(true));
    if !sim.state.cheats.infinite_money {
        let _ = apply_command(&mut sim.state, &Command::CheatToggleInfiniteMoney);
    }
    if !sim.state.cheats.magic_bulldozer {
        let _ = apply_command(&mut sim.state, &Command::CheatToggleMagicBulldozer);
    }
}

/// Directorio de escenarios JSON del menú.
#[must_use]
pub fn scenarios_save_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("save/scenarios")
}

/// Regenera el paisaje in-place (helper de tests; producción usa `Command::RegenerateLandscape`).
#[cfg(test)]
pub fn regenerate_landscape_in_place(
    state: &mut openttdrs_core::GameState,
    climate: Climate,
    seed: u64,
    island: bool,
    roughness: TerrainRoughness,
) -> Result<(), String> {
    use openttdrs_core::Command;
    use openttdrs_core::prelude::apply_command;

    apply_command(
        state,
        &Command::RegenerateLandscape {
            climate,
            seed,
            island,
            height_span: roughness.height_span(),
        },
    )
    .map_err(|e| format!("world_gen: {e:?}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::Command;
    use openttdrs_core::prelude::*;

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

    #[test]
    fn regenerate_landscape_clears_transport_entities() {
        let mut sim = SimWorld::from_new_game(&editor_new_game_settings());
        regenerate_landscape_in_place(
            &mut sim.state,
            Climate::SubArctic,
            99,
            true,
            TerrainRoughness::Hilly,
        )
        .unwrap();
        assert_eq!(sim.state.climate, Climate::SubArctic);
        assert_eq!(sim.state.world_seed, 99);
        // La regeneración reemplaza pueblos e industrias con otra población,
        // pero no conserva infraestructura ni vehículos del escenario anterior.
        assert!(sim.state.stations.is_empty());
        assert!(sim.state.vehicles.is_empty());
    }
}
