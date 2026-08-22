use openttdrs_core::{Climate, format_money};

use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, TerrainRoughness,
};
use crate::ui::font::UiFontRole;

use super::{MainMenuLocalizedText, MainMenuPanel};

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

pub(super) fn localized_climate_label(
    locale: crate::i18n::Locale,
    climate: Climate,
) -> &'static str {
    crate::i18n::text(locale, climate_label(climate))
}

pub(super) fn localized_density_label(
    locale: crate::i18n::Locale,
    density: PopulationDensity,
) -> &'static str {
    crate::i18n::text(locale, density.menu_label())
}

pub(super) fn localized_roughness_label(
    locale: crate::i18n::Locale,
    roughness: TerrainRoughness,
) -> &'static str {
    crate::i18n::text(locale, roughness.menu_label())
}

pub(super) fn map_size_label(size: MapSizePreset) -> String {
    size.menu_label()
}

pub(crate) fn summary_text(settings: NewGameSettings) -> String {
    summary_text_for(crate::i18n::Locale::Es, settings)
}

pub(crate) fn summary_text_for(locale: crate::i18n::Locale, settings: NewGameSettings) -> String {
    let settings = settings.sanitized();
    let mode_source = if settings.world_gen {
        if settings.island {
            "isla procedural + lagos"
        } else {
            "colinas procedural + lagos"
        }
    } else if settings.preserve_demo {
        "demo completa (plana)"
    } else {
        "mapa plano"
    };
    let yes = crate::i18n::text(locale, "sí");
    let no = crate::i18n::text(locale, "no");
    let seed = if settings.seed == 0 {
        crate::i18n::text(locale, "auto").to_owned()
    } else {
        settings.seed.to_string()
    };
    format!(
        "{} {} · {} {} · {} {} · {} · {}={}\n\
         {} {} · {} {} · {} {} · {} {} · {} {} · {} {}",
        crate::i18n::text(locale, "Mapa"),
        map_size_label(settings.map_size),
        crate::i18n::text(locale, "clima"),
        localized_climate_label(locale, settings.climate),
        crate::i18n::text(locale, "inicio"),
        settings.start_year,
        crate::i18n::text(locale, mode_source),
        crate::i18n::text(locale, "semilla"),
        seed,
        crate::i18n::text(locale, "Pueblos"),
        localized_density_label(locale, settings.town_density),
        crate::i18n::text(locale, "industrias"),
        localized_density_label(locale, settings.industry_density),
        crate::i18n::text(locale, "capital"),
        format_money(settings.starting_money),
        crate::i18n::text(locale, "relieve"),
        localized_roughness_label(locale, settings.terrain_roughness),
        crate::i18n::text(locale, "rival"),
        if settings.rival_ai { yes } else { no },
        crate::i18n::text(locale, "desastres"),
        if settings.disasters_enabled { yes } else { no },
    )
}

pub(super) fn option_section_label(text: &'static str) -> impl bevy::prelude::Bundle {
    use bevy::prelude::*;
    (
        MainMenuLocalizedText(text),
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
