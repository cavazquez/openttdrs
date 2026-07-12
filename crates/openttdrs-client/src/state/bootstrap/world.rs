//! Arranque del mapa procedural con opciones de nueva partida.

use openttdrs_core::{Climate, GameState, WorldGenConfig, apply_world_gen, tick_for_calendar_year};

use super::demo_layout::{
    apply_optional_world_gen, demo_preserve_rects, fill_flat_grass, place_bridge_demo_gap,
    place_clean_demo_transport, place_demo_economy_loop,
};
use super::gameplay_showcase::place_gameplay_showcase;
use super::procedural_population::{populate_procedural_world, should_populate_procedurally};
use super::terrain::place_tunnel_demo_ridge;

/// Tamaños de mapa disponibles en «Nueva partida» (OpenTTD usa potencias de 2; compacta = demo 24×18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MapSizePreset {
    #[default]
    Compact,
    Small,
    Medium,
    Large,
}

impl MapSizePreset {
    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Compact => (24, 18),
            Self::Small => (64, 64),
            Self::Medium => (128, 128),
            Self::Large => (256, 256),
        }
    }

    #[must_use]
    pub const fn menu_label(self) -> &'static str {
        match self {
            Self::Compact => "24×18",
            Self::Small => "64²",
            Self::Medium => "128²",
            Self::Large => "256²",
        }
    }

    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Compact, Self::Small, Self::Medium, Self::Large]
    }
}

/// Años de inicio habituales en OpenTTD (calendario desde 1950).
pub const START_YEARS: [u32; 8] = [1950, 1960, 1970, 1980, 1990, 2000, 2010, 2020];

/// Cifras de dinero inicial habituales en OpenTTD (libras).
pub const STARTING_MONEY_OPTIONS: [i64; 4] = [50_000, 100_000, 500_000, 1_000_000];

/// Densidad de generación procedural (pueblos o industrias).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PopulationDensity {
    Sparse,
    #[default]
    Normal,
    Dense,
}

impl PopulationDensity {
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Sparse, Self::Normal, Self::Dense]
    }

    #[must_use]
    pub const fn menu_label(self) -> &'static str {
        match self {
            Self::Sparse => "Baja",
            Self::Normal => "Media",
            Self::Dense => "Alta",
        }
    }

    #[must_use]
    pub const fn multiplier_bps(self) -> u32 {
        match self {
            Self::Sparse => 50,
            Self::Normal => 100,
            Self::Dense => 175,
        }
    }
}

/// Opciones de «Nueva partida» (menú principal o tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGameSettings {
    pub climate: Climate,
    pub map_size: MapSizePreset,
    pub start_year: u32,
    pub world_gen: bool,
    pub island: bool,
    /// Conserva carretera/vía/puente demo al generar terreno (solo mapa compacta 24×18).
    pub preserve_demo: bool,
    pub seed: u64,
    pub town_density: PopulationDensity,
    pub industry_density: PopulationDensity,
    pub starting_money: i64,
    /// Si true, añade rival IA TransCargo al iniciar.
    pub rival_ai: bool,
}

impl Default for NewGameSettings {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            map_size: MapSizePreset::Compact,
            start_year: START_YEARS[0],
            world_gen: false,
            island: false,
            preserve_demo: true,
            seed: 0,
            town_density: PopulationDensity::Normal,
            industry_density: PopulationDensity::Normal,
            starting_money: STARTING_MONEY_OPTIONS[1],
            rival_ai: true,
        }
    }
}

impl NewGameSettings {
    #[must_use]
    pub const fn map_dimensions(self) -> (u32, u32) {
        self.map_size.dimensions()
    }

    /// Ajusta opciones inválidas (demo solo en compacta, año acotado).
    #[must_use]
    pub fn sanitized(self) -> Self {
        let mut s = self;
        if s.map_size != MapSizePreset::Compact {
            s.preserve_demo = false;
        }
        if s.start_year < START_YEARS[0] {
            s.start_year = START_YEARS[0];
        } else if s.start_year > START_YEARS[7] {
            s.start_year = START_YEARS[7];
        }
        if !STARTING_MONEY_OPTIONS.contains(&s.starting_money) {
            s.starting_money = STARTING_MONEY_OPTIONS[1];
        }
        s
    }

    /// Partida con isla procedural completa (sin reservar zonas demo).
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn procedural_island(climate: Climate, seed: u64) -> Self {
        Self {
            climate,
            map_size: MapSizePreset::Small,
            start_year: START_YEARS[0],
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed,
            town_density: PopulationDensity::Normal,
            industry_density: PopulationDensity::Normal,
            starting_money: STARTING_MONEY_OPTIONS[1],
            rival_ai: true,
        }
    }
}

/// Mapa demo jugable: hierba plana o terreno procedural + transporte/industrias.
pub(crate) fn build_procedural_demo_world(settings: &NewGameSettings) -> GameState {
    let settings = settings.sanitized();
    let (mw, mh) = settings.map_dimensions();
    let mut state = GameState::new(mw, mh);
    state.climate = settings.climate;
    state.tick = tick_for_calendar_year(settings.start_year);
    fill_flat_grass(&mut state);
    let preserve = if settings.preserve_demo {
        demo_preserve_rects()
    } else {
        Vec::new()
    };
    if settings.world_gen {
        let seed = if settings.seed == 0 {
            0xDEAD_BEEF_u64
        } else {
            settings.seed
        };
        state.world_seed = seed;
        apply_optional_world_gen(
            &mut state,
            WorldGenConfig {
                climate: settings.climate,
                seed,
                sea_level: 1,
                island: settings.island,
            },
            &preserve,
        );
    }
    if should_populate_procedurally(&settings) {
        populate_procedural_world(&mut state, &settings, &preserve);
    }
    if settings.preserve_demo {
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);
        place_gameplay_showcase(&mut state);
        place_tunnel_demo_ridge(&mut state);
        place_bridge_demo_gap(&mut state);
    } else {
        state.economy.money = settings.starting_money;
    }
    state.ensure_companies();
    state.sync_active_from_mirrors();
    if settings.rival_ai {
        state.ensure_rival_transcargo();
    }
    state
}

/// Genera un mapa vacío con solo terreno (sin demo de transporte).
#[allow(dead_code)]
pub(crate) fn build_empty_procedural_world(
    width: u32,
    height: u32,
    settings: &NewGameSettings,
) -> GameState {
    let mut state = GameState::new(width, height);
    state.climate = settings.climate;
    fill_flat_grass(&mut state);
    if settings.world_gen {
        let seed = if settings.seed == 0 {
            0xCAFE_BABE_u64
        } else {
            settings.seed
        };
        state.world_seed = seed;
        let _ = apply_world_gen(
            &mut state.map,
            &WorldGenConfig {
                climate: settings.climate,
                seed,
                sea_level: 1,
                island: settings.island,
            },
            &[],
        );
    }
    state
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{format_calendar_date, tick_for_calendar_year};

    #[test]
    fn starting_money_applies_on_large_maps() {
        let settings = NewGameSettings {
            map_size: MapSizePreset::Medium,
            start_year: 1980,
            world_gen: true,
            preserve_demo: true,
            starting_money: 1_000_000,
            ..NewGameSettings::default()
        };
        let state = build_procedural_demo_world(&settings);
        assert_eq!(state.map.dimensions(), (128, 128));
        assert_eq!(format_calendar_date(state.tick), "1 ene 1980");
        assert!(!state.towns.is_empty());
        assert!(!state.industries.is_empty());
        assert_eq!(state.economy.money, 1_000_000);
    }

    #[test]
    fn rival_ai_adds_transcargo_on_new_game() {
        let with_rival = build_procedural_demo_world(&NewGameSettings {
            rival_ai: true,
            map_size: MapSizePreset::Compact,
            preserve_demo: false,
            starting_money: 100_000,
            ..NewGameSettings::default()
        });
        assert!(with_rival.companies.iter().any(|c| c.is_ai));
        assert!(with_rival.companies.len() >= 2);

        let without = build_procedural_demo_world(&NewGameSettings {
            rival_ai: false,
            map_size: MapSizePreset::Compact,
            preserve_demo: false,
            starting_money: 100_000,
            ..NewGameSettings::default()
        });
        assert!(!without.companies.iter().any(|c| c.is_ai));
    }

    #[test]
    fn sanitized_clears_demo_on_large_maps() {
        let settings = NewGameSettings {
            map_size: MapSizePreset::Large,
            preserve_demo: true,
            ..NewGameSettings::default()
        }
        .sanitized();
        assert!(!settings.preserve_demo);
    }

    #[test]
    fn tick_for_calendar_year_matches_news_helper() {
        assert_eq!(
            tick_for_calendar_year(2000),
            openttdrs_core::GameTick::new(50 * openttdrs_core::economy::TICKS_PER_YEAR)
        );
    }

    #[test]
    fn new_game_world_gen_includes_water() {
        use openttdrs_core::{TileCoord, TileKind};

        let settings = NewGameSettings {
            map_size: MapSizePreset::Small,
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed: 0xDEAD_BEEF,
            ..NewGameSettings::default()
        };
        let state = build_procedural_demo_world(&settings);
        let water = (0..64i32)
            .flat_map(|y| (0..64).map(move |x| (x, y)))
            .filter(|&(x, y)| state.map.get_kind(TileCoord::new(x, y)) == Some(TileKind::Water))
            .count();
        assert!(
            water >= 32,
            "new game should generate visible water, got {water} tiles"
        );
    }
}
