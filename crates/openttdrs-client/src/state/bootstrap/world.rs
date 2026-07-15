//! Arranque del mapa procedural con opciones de nueva partida.

use openttdrs_core::{Climate, GameState, WorldGenConfig, apply_world_gen, tick_for_calendar_year};

use super::demo_layout::{
    apply_optional_world_gen, demo_preserve_rects, fill_flat_grass, place_bridge_demo_gap,
    place_clean_demo_transport, place_demo_economy_loop,
};
use super::gameplay_showcase::place_gameplay_showcase;
use super::procedural_population::{populate_procedural_world, should_populate_procedurally};
use super::terrain::place_tunnel_demo_ridge;

/// Bits de tamaño por eje (OpenTTD `MIN_MAP_SIZE_BITS`..=`MAX_MAP_SIZE_BITS`).
pub const MIN_MAP_SIZE_BITS: u8 = 6;
pub const MAX_MAP_SIZE_BITS: u8 = 12;

/// Tamaño de un eje del mapa: 2^bits teselas (64…4096), como OpenTTD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MapAxisSize {
    #[default]
    T64,
    T128,
    T256,
    T512,
    T1024,
    T2048,
    T4096,
}

impl MapAxisSize {
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::T64 => 6,
            Self::T128 => 7,
            Self::T256 => 8,
            Self::T512 => 9,
            Self::T1024 => 10,
            Self::T2048 => 11,
            Self::T4096 => 12,
        }
    }

    #[must_use]
    pub const fn tiles(self) -> u32 {
        1_u32 << self.bits()
    }

    #[must_use]
    pub const fn menu_label(self) -> &'static str {
        match self {
            Self::T64 => "64",
            Self::T128 => "128",
            Self::T256 => "256",
            Self::T512 => "512",
            Self::T1024 => "1024",
            Self::T2048 => "2048",
            Self::T4096 => "4096",
        }
    }

    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::T64,
            Self::T128,
            Self::T256,
            Self::T512,
            Self::T1024,
            Self::T2048,
            Self::T4096,
        ]
    }
}

const _: () = {
    assert!(MapAxisSize::T64.bits() == MIN_MAP_SIZE_BITS);
    assert!(MapAxisSize::T4096.bits() == MAX_MAP_SIZE_BITS);
    assert!(MapAxisSize::T4096.tiles() == 4096);
};

/// Tamaño de mapa en «Nueva partida»: demo compacta o ejes independientes (OpenTTD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MapSizePreset {
    /// Demo clásica 24×18 (no es potencia de 2).
    #[default]
    Compact,
    /// Ancho × alto en potencias de 2 (64…4096 por eje).
    Sized {
        width: MapAxisSize,
        height: MapAxisSize,
    },
}

impl MapSizePreset {
    pub const SMALL: Self = Self::square(MapAxisSize::T64);

    #[must_use]
    pub const fn square(axis: MapAxisSize) -> Self {
        Self::Sized {
            width: axis,
            height: axis,
        }
    }

    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    #[must_use]
    pub const fn dimensions(self) -> (u32, u32) {
        match self {
            Self::Compact => (24, 18),
            Self::Sized { width, height } => (width.tiles(), height.tiles()),
        }
    }

    #[must_use]
    pub fn menu_label(self) -> String {
        let (w, h) = self.dimensions();
        if w == h {
            format!("{w}²")
        } else {
            format!("{w}×{h}")
        }
    }

    /// Pasa a modo Sized (64×64) si estaba en Compact; útil al elegir un eje.
    pub fn ensure_sized(&mut self) {
        if self.is_compact() {
            *self = Self::SMALL;
        }
    }

    pub fn set_width(&mut self, width: MapAxisSize) {
        self.ensure_sized();
        if let Self::Sized { height, .. } = *self {
            *self = Self::Sized { width, height };
        }
    }

    pub fn set_height(&mut self, height: MapAxisSize) {
        self.ensure_sized();
        if let Self::Sized { width, .. } = *self {
            *self = Self::Sized { width, height };
        }
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

/// Relieve procedural (amplitud de colinas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerrainRoughness {
    Flat,
    #[default]
    Normal,
    Hilly,
}

impl TerrainRoughness {
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Flat, Self::Normal, Self::Hilly]
    }

    #[must_use]
    pub const fn menu_label(self) -> &'static str {
        match self {
            Self::Flat => "Llano",
            Self::Normal => "Normal",
            Self::Hilly => "Montañoso",
        }
    }

    #[must_use]
    pub const fn height_span(self) -> u8 {
        match self {
            Self::Flat => 3,
            Self::Normal => 6,
            Self::Hilly => 10,
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
    /// Si true, pueden ocurrir desastres ambientales.
    pub disasters_enabled: bool,
    pub terrain_roughness: TerrainRoughness,
    /// Si true, siembra GameScript-lite demo (story/goals) al iniciar (#43).
    pub gamescript_demo: bool,
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
            disasters_enabled: true,
            terrain_roughness: TerrainRoughness::Normal,
            gamescript_demo: true,
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
        if !s.map_size.is_compact() {
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
            map_size: MapSizePreset::SMALL,
            start_year: START_YEARS[0],
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed,
            town_density: PopulationDensity::Normal,
            industry_density: PopulationDensity::Normal,
            starting_money: STARTING_MONEY_OPTIONS[1],
            rival_ai: true,
            disasters_enabled: true,
            terrain_roughness: TerrainRoughness::Normal,
            gamescript_demo: true,
        }
    }
}

/// Mapa demo jugable: hierba plana o terreno procedural + transporte/industrias.
pub(crate) fn build_procedural_demo_world(settings: &NewGameSettings) -> GameState {
    let settings = settings.sanitized();
    let (mw, mh) = settings.map_dimensions();
    let mut state = GameState::new(mw, mh);
    state.climate = settings.climate;
    state.disasters_enabled = settings.disasters_enabled;
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
                height_span: settings.terrain_roughness.height_span(),
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
    }
    state.economy.money = settings.starting_money;
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
                height_span: settings.terrain_roughness.height_span(),
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
    fn map_size_matches_openttd_axis_limits() {
        assert_eq!(MapAxisSize::T64.tiles(), 64);
        assert_eq!(MapAxisSize::T4096.tiles(), 4096);
        assert_eq!(
            MapSizePreset::square(MapAxisSize::T4096).dimensions(),
            (4096, 4096)
        );
        let asym = MapSizePreset::Sized {
            width: MapAxisSize::T256,
            height: MapAxisSize::T512,
        };
        assert_eq!(asym.dimensions(), (256, 512));
        assert_eq!(asym.menu_label(), "256×512");
    }

    #[test]
    fn starting_money_applies_on_large_maps() {
        let settings = NewGameSettings {
            map_size: MapSizePreset::square(MapAxisSize::T128),
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
            map_size: MapSizePreset::square(MapAxisSize::T256),
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
            map_size: MapSizePreset::SMALL,
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
