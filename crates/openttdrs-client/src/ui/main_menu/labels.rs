use openttdrs_core::{Climate, format_money};

use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, TerrainRoughness,
};
use crate::ui::font::UiFontRole;

use super::MainMenuPanel;

pub(super) fn dev_mode() -> bool {
    std::env::var_os("OPENTTDRS_DEV").is_some()
}

pub(super) fn climate_label(climate: Climate) -> &'static str {
    match climate {
        Climate::Temperate => "Templado",
        Climate::SubArctic => "Artico",
        Climate::SubTropical => "Tropical",
        Climate::Toyland => "Toyland",
    }
}

pub(super) fn map_size_label(size: MapSizePreset) -> String {
    size.menu_label()
}

pub(crate) fn summary_text(settings: NewGameSettings) -> String {
    let settings = settings.sanitized();
    let mode = if settings.world_gen {
        if settings.island {
            "isla procedural + lagos"
        } else {
            "colinas procedural + lagos"
        }
    } else if settings.preserve_demo {
        "demo clasica (plana)"
    } else {
        "mapa plano"
    };
    format!(
        "Mapa {} · clima {} · inicio {} · {} · semilla={}\n\
         Pueblos {} · industrias {} · capital {} · relieve {} · rival {}",
        map_size_label(settings.map_size),
        climate_label(settings.climate),
        settings.start_year,
        mode,
        if settings.seed == 0 {
            "auto".to_string()
        } else {
            settings.seed.to_string()
        },
        settings.town_density.menu_label(),
        settings.industry_density.menu_label(),
        format_money(settings.starting_money),
        settings.terrain_roughness.menu_label(),
        if settings.rival_ai { "sí" } else { "no" },
    )
}

pub(super) fn option_section_label(text: &str) -> impl bevy::prelude::Bundle {
    use bevy::prelude::*;
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Rem(UiFontRole::Caption.rem_size()),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.76, 0.62)),
    )
}

pub(super) fn panel_title(panel: MainMenuPanel) -> &'static str {
    match panel {
        MainMenuPanel::Root => "OpenTTDRS",
        MainMenuPanel::NewGame => "Nueva partida",
        MainMenuPanel::Highscores => "Mejores puntuaciones",
        MainMenuPanel::Scenarios => "Escenarios / heightmap",
        MainMenuPanel::Preferences => "Preferencias",
        MainMenuPanel::QuitConfirm => "Salir del juego",
    }
}

pub(super) fn panel_hints(panel: MainMenuPanel) -> &'static str {
    match panel {
        MainMenuPanel::Root => "Esc salir · raton para elegir",
        MainMenuPanel::NewGame => {
            "Enter iniciar · Esc volver · 1-4 clima · [ ] semilla · z/x densidad"
        }
        MainMenuPanel::Highscores | MainMenuPanel::Scenarios | MainMenuPanel::Preferences => {
            "Esc volver"
        }
        MainMenuPanel::QuitConfirm => "Esc cancelar",
    }
}

#[allow(dead_code)]
pub(super) fn roughness_label(roughness: TerrainRoughness) -> &'static str {
    roughness.menu_label()
}

pub(crate) fn cycle_density(density: &mut PopulationDensity) {
    *density = match density {
        PopulationDensity::Sparse => PopulationDensity::Normal,
        PopulationDensity::Normal => PopulationDensity::Dense,
        PopulationDensity::Dense => PopulationDensity::Sparse,
    };
}

pub(crate) fn adjust_seed(seed: &mut u64, delta: i32) {
    if delta < 0 {
        *seed = seed.saturating_sub(1);
    } else {
        *seed = seed.saturating_add(1);
    }
}
