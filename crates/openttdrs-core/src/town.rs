//! Demanda urbana mínima: casas en cobertura de parada generan pasajeros y correo.

use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::cargodist::parity::Randomizer;
use crate::company::CompanyId;
use crate::entity_history::TownHistory;
use crate::industry::Industry;
use crate::map::{Map, TileCoord, TileKind, tile_slope_and_z};
use crate::station::{self, STATION_COVERAGE_RADIUS, Station, StopKind};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, Climate, DEF_SNOW_LINE_HEIGHT, desert_patch, effective_clear_ground,
};

use crate::town_authority_serde as authority_serde;

/// Zonas urbanas / clima de casa (`HouseZone`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HouseZone {
    TownEdge = 0,
    TownOutskirt = 1,
    TownOuterSuburb = 2,
    TownInnerSuburb = 3,
    TownCentre = 4,
    ClimateSubarcticAboveSnow = 11,
    ClimateTemperate = 12,
    ClimateSubarcticBelowSnow = 13,
    ClimateSubtropic = 14,
    ClimateToyland = 15,
}

/// Número de zonas de radio (edge…centre).
pub const NUM_HOUSE_ZONES: usize = 5;

impl HouseZone {
    /// Índice 0..4 → zona urbana; `None` si está fuera de rango.
    #[must_use]
    pub const fn from_zone_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::TownEdge),
            1 => Some(Self::TownOutskirt),
            2 => Some(Self::TownOuterSuburb),
            3 => Some(Self::TownInnerSuburb),
            4 => Some(Self::TownCentre),
            _ => None,
        }
    }
}

/// Layout de calles del pueblo (`TownLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum TownLayout {
    #[default]
    Original = 0,
    BetterRoads = 1,
    Grid2x2 = 2,
    Grid3x3 = 3,
    Random = 4,
}

/// Tabla `_town_squared_town_zone_radius_data` (`town_cmd.cpp:1958`).
static TOWN_SQUARED_ZONE_RADIUS: [[u32; NUM_HOUSE_ZONES]; 23] = [
    [4, 0, 0, 0, 0],
    [16, 0, 0, 0, 0],
    [25, 0, 0, 0, 0],
    [36, 0, 0, 0, 0],
    [49, 0, 4, 0, 0],
    [64, 0, 4, 0, 0],
    [64, 0, 9, 0, 1],
    [64, 0, 9, 0, 4],
    [64, 0, 16, 0, 4],
    [81, 0, 16, 0, 4],
    [81, 0, 16, 0, 4],
    [81, 0, 25, 0, 9],
    [81, 36, 25, 0, 9],
    [81, 36, 25, 16, 9],
    [81, 49, 0, 25, 9],
    [81, 64, 0, 25, 9],
    [81, 64, 0, 36, 9],
    [81, 64, 0, 36, 16],
    [100, 81, 0, 49, 16],
    [100, 81, 0, 49, 25],
    [121, 81, 0, 49, 25],
    [121, 81, 0, 49, 25],
    [121, 81, 0, 49, 36],
];

/// Actualiza radios de zona según número de casas (`UpdateTownRadius`).
pub fn update_town_radius(town: &mut Town) {
    let n = usize::from(town.num_houses);
    if n < TOWN_SQUARED_ZONE_RADIUS.len() * 4 {
        town.squared_town_zone_radius = TOWN_SQUARED_ZONE_RADIUS[n / 4];
    } else {
        let mass = i32::from(town.num_houses) / 8;
        town.squared_town_zone_radius = [
            u32::try_from((mass * 15 - 40).max(0)).unwrap_or(0),
            u32::try_from((mass * 9 - 15).max(0)).unwrap_or(0),
            0,
            u32::try_from((mass * 5 - 5).max(0)).unwrap_or(0),
            u32::try_from((mass * 3 + 5).max(0)).unwrap_or(0),
        ];
    }
}

/// Recuenta casas del pueblo en el mapa y actualiza radios.
pub fn recount_town_houses(map: &Map, town: &mut Town) {
    let mut count = 0_u16;
    let (mw, mh) = map.dimensions();
    let search = i32::try_from(town.squared_town_zone_radius[0].saturating_add(4).max(64))
        .unwrap_or(64)
        .max(24);
    for dy in -search..=search {
        for dx in -search..=search {
            let pos = TileCoord::new(town.pos.x + dx, town.pos.y + dy);
            if pos.x < 0
                || pos.y < 0
                || pos.x >= i32::try_from(mw).unwrap_or(i32::MAX)
                || pos.y >= i32::try_from(mh).unwrap_or(i32::MAX)
            {
                continue;
            }
            if map.get_kind(pos) == Some(TileKind::House) {
                count = count.saturating_add(1);
            }
        }
    }
    town.num_houses = count;
    update_town_radius(town);
}

/// Efectos de carga que alimentan metas de crecimiento (`TownEffect` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TownGrowthEffect {
    Passengers = 0,
    Mail = 1,
    Goods = 2,
    /// Comida (ártico) — proxy vía `Goods` hasta existir cargo Food.
    Food = 3,
    /// Agua (trópico) — proxy vía `Oil` hasta existir cargo Water.
    Water = 4,
}

pub const TOWN_GROWTH_EFFECT_COUNT: usize = 5;

/// Meta especial: comida solo en invierno (`TOWN_GROWTH_WINTER`).
pub const TOWN_GROWTH_WINTER: u32 = u32::MAX - 1;
/// Meta especial: comida/agua en desierto (`TOWN_GROWTH_DESERT`).
pub const TOWN_GROWTH_DESERT: u32 = u32::MAX;
/// Umbral de población para exigir comida en ártico.
pub const TOWN_GROWTH_WINTER_POP_THRESHOLD: u32 = 90;
/// Umbral de población para exigir comida/agua en trópico.
pub const TOWN_GROWTH_DESERT_POP_THRESHOLD: u32 = 60;
/// Meses de crecimiento forzado al financiar edificios (`fund_buildings`).
pub const FUND_BUILDINGS_MONTHS: u8 = 3;
/// Valoración de partida de la autoridad local (`RATING_INITIAL`, `town_type.h:45`).
pub const TOWN_RATING_INITIAL: i16 = 500;
/// Rating máximo de recuperación mensual automática (`RATING_GROWTH_MAXIMUM`).
pub const RATING_GROWTH_MAXIMUM: i16 = 200;
/// Paso mensual de recuperación hacia `RATING_GROWTH_MAXIMUM`.
pub const RATING_GROWTH_UP_STEP: i8 = 5;
/// Bonus mensual por estación bien servida (`RATING_STATION_UP_STEP`).
pub const RATING_STATION_UP_STEP: i8 = 12;
/// Penalización mensual por estación mal servida (`RATING_STATION_DOWN_STEP`).
pub const RATING_STATION_DOWN_STEP: i8 = -15;
/// Días sin actividad para considerar una estación inactiva (`time_since_load` ≤ 20).
pub const STATION_ACTIVE_DAYS: u8 = 20;
/// Slots de rating por compañía (`ratings[MAX_COMPANIES]`).
pub const MAX_TOWN_AUTHORITY_COMPANIES: usize = 15;
/// Sin crecimiento (`TOWN_GROWTH_RATE_NONE`).
pub const TOWN_GROWTH_RATE_NONE: u16 = 0xFFFF;
/// Tope de ticks originales en `TownTicksToGameTicks`.
pub const MAX_TOWN_GROWTH_SOURCE_TICKS: u16 = 930;

/// Tolerancia del ayuntamiento a demolición municipal (`town_council_tolerance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TownCouncilTolerance {
    Lenient,
    #[default]
    Neutral,
    Hostile,
    Permissive,
}

/// Tipo de chequeo de rating para demolición (`TownRatingCheckType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownRatingCheckType {
    RoadRemove,
    TunnelBridgeRemove,
}

/// Muestra mensual de carga suministrada a un pueblo (`CITY.supplied`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TownSuppliedHistory {
    pub production: u32,
    pub transported: u32,
}

/// Estadísticas de un tipo de carga que el pueblo produjo (`CITY.supplied`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TownSuppliedCargo {
    /// ID de cargo nativo (`CargoType`), no un índice de efecto urbano.
    pub cargo: u8,
    pub history: Vec<TownSuppliedHistory>,
}

/// Estadísticas de carga recibida por un pueblo (`CITY.received`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TownReceivedCargo {
    pub old_max: u16,
    pub new_max: u16,
    pub old_act: u16,
    pub new_act: u16,
}

/// Ciudad (importada de saves de `OpenTTD` o creada por el juego).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Town {
    pub id: u32,
    pub pos: crate::map::TileCoord,
    pub name: String,
    /// Identidad del generador de nombres nativo (`CITY.townnamegrfid`).
    ///
    /// Se conserva separada del nombre resuelto para que una carga desde SAV
    /// no pierda la semilla de nombres aunque el runtime sólo muestre el
    /// texto resultante.
    #[serde(default)]
    pub townnamegrfid: u32,
    /// Tipo de nombre nativo (`CITY.townnametype`).
    #[serde(default)]
    pub townnametype: u16,
    /// Partes/semilla del nombre nativo (`CITY.townnameparts`).
    #[serde(default)]
    pub townnameparts: u32,
    pub population: u32,
    /// Bitset nativo `TownFlag` de `CITY.flags`.
    ///
    /// Las banderas que ya tienen un espejo semántico (`is_growing`,
    /// `has_church`, `has_stadium`) se sincronizan al importar; los bits
    /// restantes se preservan aunque todavía no tengan comportamiento propio.
    #[serde(default)]
    pub native_flags: u8,
    /// Valoración de la autoridad local por compañía (`ratings[MAX_COMPANIES]`).
    #[serde(
        default = "default_authority_ratings",
        serialize_with = "authority_serde::serialize",
        deserialize_with = "authority_serde::deserialize"
    )]
    pub authority_ratings: Vec<i16>,
    /// Máscara nativa de compañías con rating (`CITY.have_ratings`).
    #[serde(default)]
    pub have_ratings: u16,
    /// Campo legado en saves v22 (`local_authority_rating` único).
    #[serde(default, skip_serializing, rename = "local_authority_rating")]
    pub legacy_local_authority_rating: Option<i16>,
    /// Pasajeros entregados cerca de la ciudad (contador de crecimiento).
    #[serde(default)]
    pub passengers_served: u32,
    /// Correo entregado cerca de la ciudad.
    #[serde(default)]
    pub mail_served: u32,
    /// Veces que la compañía financió edificios (`TownFundBuildings`).
    #[serde(default)]
    pub growth_funded: u32,
    /// Metas mensuales por efecto (`town->goal[]`).
    #[serde(default)]
    pub goals: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Historial nativo por cargo suministrado (`CITY.supplied`).
    #[serde(default)]
    pub supplied_cargo: Vec<TownSuppliedCargo>,
    /// Estadísticas nativas por efecto recibido (`CITY.received`). El orden
    /// de la lista es el orden de `Town::received` en `OpenTTD`.
    #[serde(default)]
    pub received_cargo: Vec<TownReceivedCargo>,
    /// Entregas del mes en curso (`received_new`).
    #[serde(default)]
    pub received_new: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Entregas del mes anterior (`received_old`), usadas para el gate de crecimiento.
    #[serde(default)]
    pub received_old: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Meses restantes de crecimiento forzado por financiación.
    #[serde(default)]
    pub fund_buildings_months: u8,
    /// Resultado de `UpdateTownGrowth` (solo crece si es `true`).
    #[serde(default)]
    pub is_growing: bool,
    /// Ticks de juego entre intentos de expansión (`Town::growth_rate`).
    #[serde(default)]
    pub growth_rate: u16,
    /// Contador decreciente hacia el siguiente intento (`Town::grow_counter`).
    #[serde(default)]
    pub grow_counter: u16,
    /// Series mensuales (población / servicio).
    #[serde(default)]
    pub history: TownHistory,
    /// Ruido acumulado de aeropuertos (`Town::noise_reached`).
    #[serde(default)]
    pub noise_reached: u16,
    /// Número de casas (`Town::cache.num_houses`).
    #[serde(default)]
    pub num_houses: u16,
    /// Radios al cuadrado por zona (`squared_town_zone_radius`).
    #[serde(default)]
    pub squared_town_zone_radius: [u32; NUM_HOUSE_ZONES],
    /// Layout de calles (`Town::layout`).
    #[serde(default)]
    pub layout: TownLayout,
    /// ¿Ya tiene iglesia? (`TownFlag::HasChurch`).
    #[serde(default)]
    pub has_church: bool,
    /// ¿Ya tiene estadio? (`TownFlag::HasStadium`).
    #[serde(default)]
    pub has_stadium: bool,
    /// Contador hasta la siguiente renovación (`time_until_rebuild`).
    #[serde(default = "default_time_until_rebuild")]
    pub time_until_rebuild: u16,
    /// Meses de reconstrucción vial financiada (`road_build_months`).
    #[serde(default)]
    pub road_build_months: u8,
    /// `true` si el pueblo usa el algoritmo de crecimiento acelerado.
    #[serde(default)]
    pub larger_town: bool,
    /// Máscara de meses válidos del historial de suministros (`CITY.valid_history`).
    #[serde(default)]
    pub valid_history: u64,
    /// Texto adicional codificado por `GameScript` (`CITY.text`), conservado
    /// como UTF-8 lossless-best-effort por el parser de tablas.
    #[serde(default)]
    pub native_text: String,
    /// Meses restantes de derechos exclusivos (`exclusive_counter`).
    #[serde(default)]
    pub exclusive_counter: u8,
    /// Compañía con derechos exclusivos (`exclusivity`).
    #[serde(default)]
    pub exclusivity: Option<crate::company::CompanyId>,
    /// Bitset de estatuas por compañía (`statues`).
    #[serde(default)]
    pub statues: u16,
    /// Meses de «unwanted» por compañía tras soborno fallido.
    #[serde(default = "default_unwanted")]
    pub unwanted: Vec<u8>,
    /// Índices del pool nativo `PSAC` por GRFID para el scope de pueblo.
    ///
    /// `CITY.psa_list` puede contener un storage por `NewGRF`. El campo se
    /// mantiene fuera del JSON propio: el índice nativo y la fila completa
    /// viven en `GameState::sav_persistent_storages`, mientras este mapa
    /// permite que los resolvers lean los registros `7C` durante el runtime
    /// sin confundir dos GRF distintos. El writeback de town PSA queda
    /// pendiente de conectar con la mutación de callbacks.
    #[serde(skip, default)]
    pub newgrf_persistent_storage_ids: std::collections::HashMap<u32, u32>,
    /// Registros no nulos del PSA de pueblo, agrupados por GRFID.
    #[serde(skip, default)]
    pub newgrf_persistent_regs: std::collections::HashMap<u32, std::collections::HashMap<u8, u32>>,
}

fn default_unwanted() -> Vec<u8> {
    vec![0; MAX_TOWN_AUTHORITY_COMPANIES]
}

fn default_time_until_rebuild() -> u16 {
    10
}

fn default_authority_ratings() -> Vec<i16> {
    vec![TOWN_RATING_INITIAL; MAX_TOWN_AUTHORITY_COMPANIES]
}

impl Default for Town {
    fn default() -> Self {
        Self {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: String::new(),
            townnamegrfid: 0,
            townnametype: 0,
            townnameparts: 0,
            population: 0,
            native_flags: 0,
            authority_ratings: default_authority_ratings(),
            have_ratings: 0,
            legacy_local_authority_rating: None,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            goals: [0; TOWN_GROWTH_EFFECT_COUNT],
            supplied_cargo: Vec::new(),
            received_cargo: Vec::new(),
            received_new: [0; TOWN_GROWTH_EFFECT_COUNT],
            received_old: [0; TOWN_GROWTH_EFFECT_COUNT],
            fund_buildings_months: 0,
            is_growing: false,
            growth_rate: 0,
            grow_counter: 0,
            history: TownHistory::default(),
            noise_reached: 0,
            num_houses: 0,
            squared_town_zone_radius: [0; NUM_HOUSE_ZONES],
            layout: TownLayout::Original,
            has_church: false,
            has_stadium: false,
            time_until_rebuild: default_time_until_rebuild(),
            road_build_months: 0,
            larger_town: false,
            valid_history: 0,
            native_text: String::new(),
            exclusive_counter: 0,
            exclusivity: None,
            statues: 0,
            unwanted: default_unwanted(),
            newgrf_persistent_storage_ids: std::collections::HashMap::new(),
            newgrf_persistent_regs: std::collections::HashMap::new(),
        }
    }
}

impl Town {
    /// Elige layout a partir del hash de posición (`Town::InitializeLayout`).
    pub fn initialize_layout(&mut self, preferred: Option<TownLayout>) {
        if let Some(layout) = preferred.filter(|l| *l != TownLayout::Random) {
            self.layout = layout;
            return;
        }
        let hash = (self.pos.x.cast_unsigned().wrapping_mul(3))
            .wrapping_add(self.pos.y.cast_unsigned().wrapping_mul(5));
        self.layout = match hash % 4 {
            0 => TownLayout::Original,
            1 => TownLayout::BetterRoads,
            2 => TownLayout::Grid2x2,
            _ => TownLayout::Grid3x3,
        };
    }

    /// Rating de autoridad para una compañía (default `TOWN_RATING_INITIAL` si falta slot).
    #[must_use]
    pub fn authority_rating(&self, company: CompanyId) -> i16 {
        self.authority_ratings
            .get(company.index())
            .copied()
            .unwrap_or(TOWN_RATING_INITIAL)
    }

    /// Escribe el rating de autoridad (amplía el vector si hace falta).
    pub fn set_authority_rating(&mut self, company: CompanyId, rating: i16) {
        self.ensure_authority_ratings(company.index() + 1);
        if let Some(slot) = self.authority_ratings.get_mut(company.index()) {
            *slot = rating;
        }
    }

    /// `true` si la compañía ya tiene estatua en este pueblo.
    #[must_use]
    pub const fn has_statue(&self, company: CompanyId) -> bool {
        let bit = company.0 as u16;
        if bit >= 16 {
            return false;
        }
        self.statues & (1 << bit) != 0
    }

    /// Marca o limpia la estatua de la compañía.
    pub fn set_statue(&mut self, company: CompanyId, present: bool) {
        let bit = u16::from(company.0);
        if bit >= 16 {
            return;
        }
        if present {
            self.statues |= 1 << bit;
        } else {
            self.statues &= !(1 << bit);
        }
    }

    /// Meses de unwanted tras soborno fallido.
    #[must_use]
    pub fn unwanted_months(&self, company: CompanyId) -> u8 {
        self.unwanted.get(company.index()).copied().unwrap_or(0)
    }

    /// Asigna meses de unwanted.
    pub fn set_unwanted(&mut self, company: CompanyId, months: u8) {
        if self.unwanted.len() < MAX_TOWN_AUTHORITY_COMPANIES {
            self.unwanted.resize(MAX_TOWN_AUTHORITY_COMPANIES, 0);
        }
        if let Some(slot) = self.unwanted.get_mut(company.index()) {
            *slot = months;
        }
    }

    /// Expone las variables de `TownScopeResolver` que el modelo conserva en
    /// el scope parent de un `Action2`. Las variables sin representación
    /// equivalente (flags/cache de cargos custom) se dejan ausentes para que
    /// el evaluador aplique el fallback nativo cero.
    pub(crate) fn copy_newgrf_parent_scope(
        &self,
        grfid: u32,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) {
        let population = self.population.min(u32::from(u16::MAX));
        // `TownFlag` mantiene cuatro bits de comportamiento en OpenTTD. Los
        // tres que ya tienen espejo en el modelo se vuelven a componer para
        // que una mutación runtime no deje obsoleto el byte importado; los
        // bits restantes se conservan literalmente.
        let mut flags = self.native_flags & !0x07;
        if self.is_growing {
            flags |= 1 << 0;
        }
        if self.has_church {
            flags |= 1 << 1;
        }
        if self.has_stadium {
            flags |= 1 << 2;
        }
        ctx.parent_vars.insert(0x40, u32::from(self.larger_town));
        ctx.parent_vars.insert(0x41, self.id);
        ctx.parent_vars.insert(0x80, self.pos.x.cast_unsigned());
        ctx.parent_vars.insert(0x81, self.pos.y.cast_unsigned());
        ctx.parent_vars
            .insert(0x82, population & u32::from(u16::MAX));
        ctx.parent_vars.insert(0x83, population >> 8);
        ctx.parent_vars.insert(
            0x8A,
            u32::from(self.grow_counter) / u32::try_from(TOWN_GROWTH_TICKS).unwrap_or(1),
        );
        ctx.parent_vars.insert(0x92, u32::from(flags));
        ctx.parent_vars.insert(0x93, 0);
        for (index, &radius) in self.squared_town_zone_radius.iter().enumerate() {
            let Ok(offset) = u8::try_from(index.saturating_mul(2)) else {
                break;
            };
            let radius = radius.min(u32::from(u16::MAX));
            ctx.parent_vars
                .insert(0x94 + offset, radius & u32::from(u16::MAX));
            ctx.parent_vars.insert(0x95 + offset, radius >> 8);
        }
        ctx.parent_vars.insert(0xAE, u32::from(self.have_ratings));
        for (index, &rating) in self.authority_ratings.iter().take(8).enumerate() {
            let Ok(offset) = u8::try_from(index.saturating_mul(2)) else {
                break;
            };
            let rating = u32::from(u16::from_ne_bytes(rating.to_ne_bytes()));
            ctx.parent_vars
                .insert(0x9E + offset, rating & u32::from(u16::MAX));
            ctx.parent_vars.insert(0x9F + offset, rating >> 8);
        }
        ctx.parent_vars.insert(0xB2, u32::from(self.statues));
        ctx.parent_vars.insert(0xB6, u32::from(self.num_houses));
        ctx.parent_vars.insert(
            0xB9,
            u32::from(self.growth_rate) / u32::try_from(TOWN_GROWTH_TICKS).unwrap_or(1),
        );
        let current = self.history.samples.last().copied().unwrap_or_default();
        let previous = self
            .history
            .samples
            .iter()
            .rev()
            .nth(1)
            .copied()
            .unwrap_or_default();
        let history_pairs = [
            (0xBA, current.passengers_served),
            (0xBC, current.mail_served),
            (0xBE, current.passengers_served),
            (0xC0, current.mail_served),
            (0xC2, previous.passengers_served),
            (0xC4, previous.mail_served),
            (0xC6, previous.passengers_served),
            (0xC8, previous.mail_served),
        ];
        for (offset, value) in history_pairs {
            let value = value.min(u32::from(u16::MAX));
            ctx.parent_vars.insert(offset, value & u32::from(u16::MAX));
            ctx.parent_vars.insert(offset + 1, value >> 8);
        }
        let food_new = self.received_new[usize::from(TownGrowthEffect::Food as u8)];
        let water_new = self.received_new[usize::from(TownGrowthEffect::Water as u8)];
        let food_old = self.received_old[usize::from(TownGrowthEffect::Food as u8)];
        let water_old = self.received_old[usize::from(TownGrowthEffect::Water as u8)];
        for (offset, value) in [
            (0xCC, food_new),
            (0xCE, water_new),
            (0xD0, food_old),
            (0xD2, water_old),
        ] {
            let value = value.min(u32::from(u16::MAX));
            ctx.parent_vars.insert(offset, value & u32::from(u16::MAX));
            ctx.parent_vars.insert(offset + 1, value >> 8);
        }
        ctx.parent_vars
            .insert(0xD4, u32::from(self.road_build_months));
        ctx.parent_vars
            .insert(0xD5, u32::from(self.fund_buildings_months));

        // Un pueblo no tiene un único storage global: la selección nativa se
        // hace por GRFID, por eso el caller debe pasar el GRFID del objeto que
        // se está resolviendo.
        ctx.parent_persistent_registers.clear();
        if let Some(registers) = self.newgrf_persistent_regs.get(&grfid) {
            ctx.parent_persistent_registers.clone_from(registers);
        }
    }

    /// Asegura slots de rating para todas las compañías del pool.
    pub fn ensure_authority_ratings(&mut self, company_count: usize) {
        self.migrate_legacy_authority_rating();
        let need = company_count.max(MAX_TOWN_AUTHORITY_COMPANIES);
        if self.authority_ratings.len() < need {
            self.authority_ratings.resize(need, TOWN_RATING_INITIAL);
        }
    }

    /// Migra el rating único de saves antiguos al vector por compañía.
    pub fn migrate_legacy_authority_rating(&mut self) {
        if let Some(legacy) = self.legacy_local_authority_rating {
            self.authority_ratings = vec![legacy; MAX_TOWN_AUTHORITY_COMPANIES];
            self.legacy_local_authority_rating = None;
        }
    }

    /// Inicializa `grow_counter` dispersado (`InitTownAndName` / afterload).
    pub fn init_grow_counter(&mut self) {
        self.grow_counter =
            u16::try_from(self.id % u32::try_from(TOWN_GROWTH_TICKS).unwrap_or(1)).unwrap_or(0);
    }

    /// Inicializa metas según clima (`InitTownAndName` / clima ártico-trópico).
    pub fn init_growth_goals(&mut self, climate: Climate) {
        self.goals = [0; TOWN_GROWTH_EFFECT_COUNT];
        match climate {
            Climate::SubArctic => {
                self.goals[TownGrowthEffect::Food as usize] = TOWN_GROWTH_WINTER;
            }
            Climate::SubTropical => {
                self.goals[TownGrowthEffect::Food as usize] = TOWN_GROWTH_DESERT;
                self.goals[TownGrowthEffect::Water as usize] = TOWN_GROWTH_DESERT;
            }
            Climate::Temperate | Climate::Toyland => {}
        }
    }

    /// Ajusta la valoración de una compañía y devuelve el delta aplicado (clamp -1000..=1000).
    pub fn adjust_rating(&mut self, company: CompanyId, delta: i8) -> i8 {
        self.ensure_authority_ratings(company.index() + 1);
        let idx = company.index();
        let before = self.authority_ratings[idx];
        let next = i32::from(before) + i32::from(delta);
        self.authority_ratings[idx] = i16::try_from(next.clamp(-1000, 1000)).unwrap_or(0);
        i8::try_from(self.authority_ratings[idx] - before).unwrap_or(delta)
    }

    /// Registra entrega de carga urbana que impulsa el crecimiento.
    pub fn record_town_cargo_delivery(&mut self, cargo: CargoType, amount: u32) {
        match cargo {
            CargoType::Passengers => {
                self.passengers_served = self.passengers_served.saturating_add(amount);
                self.add_received(TownGrowthEffect::Passengers, amount);
            }
            CargoType::Mail => {
                self.mail_served = self.mail_served.saturating_add(amount);
                self.add_received(TownGrowthEffect::Mail, amount);
            }
            CargoType::Goods | CargoType::Candy => {
                self.add_received(TownGrowthEffect::Goods, amount);
            }
            CargoType::Food | CargoType::FizzyDrinks => {
                self.add_received(TownGrowthEffect::Food, amount);
            }
            CargoType::Water => {
                self.add_received(TownGrowthEffect::Water, amount);
            }
            _ => {}
        }
    }

    fn add_received(&mut self, effect: TownGrowthEffect, amount: u32) {
        let i = effect as usize;
        self.received_new[i] = self.received_new[i].saturating_add(amount);
    }
}

/// ¿La meta del efecto está satisfecha con las entregas del mes anterior?
#[must_use]
pub fn town_goal_satisfied(goal: u32, received: u32, population: u32) -> bool {
    if goal == 0 {
        return true;
    }
    if goal == TOWN_GROWTH_WINTER {
        if population <= TOWN_GROWTH_WINTER_POP_THRESHOLD {
            return true;
        }
        return received > 0;
    }
    if goal == TOWN_GROWTH_DESERT {
        if population <= TOWN_GROWTH_DESERT_POP_THRESHOLD {
            return true;
        }
        return received > 0;
    }
    received >= goal
}

/// Variante con contexto de mapa/clima para metas invierno/desierto (`UpdateTownGrowth`).
#[must_use]
pub fn town_goal_satisfied_with_context(
    goal: u32,
    received: u32,
    population: u32,
    town_pos: TileCoord,
    map: &Map,
    climate: Climate,
    world_seed: u64,
) -> bool {
    if goal == 0 {
        return true;
    }
    if goal == TOWN_GROWTH_WINTER {
        if population <= TOWN_GROWTH_WINTER_POP_THRESHOLD {
            return true;
        }
        let height = map
            .get(town_pos)
            .map(|t| t.height)
            .or_else(|| tile_slope_and_z(map, town_pos).map(|(_, z)| z))
            .unwrap_or(0);
        if i32::from(height) < i32::from(DEF_SNOW_LINE_HEIGHT) {
            return true;
        }
        return received > 0;
    }
    if goal == TOWN_GROWTH_DESERT {
        if population <= TOWN_GROWTH_DESERT_POP_THRESHOLD {
            return true;
        }
        if !is_desert_tile(map, climate, town_pos, world_seed) {
            return true;
        }
        return received > 0;
    }
    received >= goal
}

#[must_use]
fn is_desert_tile(map: &Map, climate: Climate, pos: TileCoord, world_seed: u64) -> bool {
    if climate != Climate::SubTropical {
        return false;
    }
    if let Some(tile) = map.get(pos) {
        let ground = effective_clear_ground(climate, tile.m5, pos.x, pos.y, world_seed);
        if ground == CLEAR_GROUND_DESERT {
            return true;
        }
    }
    desert_patch(pos.x, pos.y, world_seed)
}

/// Convierte ticks de ciudad a ticks de juego (`TownTicksToGameTicks`).
#[must_use]
pub fn town_ticks_to_game_ticks(ticks: u16) -> u16 {
    let capped = ticks.min(MAX_TOWN_GROWTH_SOURCE_TICKS);
    (capped + 1) * u16::try_from(TOWN_GROWTH_TICKS).unwrap_or(1) - 1
}

static GROW_COUNT_VALUES_FUNDED: [u16; 6] = [120, 120, 120, 100, 80, 60];
static GROW_COUNT_VALUES_NORMAL: [u16; 6] = [320, 420, 300, 220, 160, 100];

#[must_use]
fn count_houses_for_growth(map: &Map, industries: &[Industry], town: &Town) -> u32 {
    station::station_coverage_at(
        map,
        industries,
        town.pos,
        i32::try_from(TOWN_AUTHORITY_RADIUS).unwrap_or(i32::MAX),
    )
    .house_tiles
}

/// Estaciones cerca del pueblo que no son waypoints/boyas.
fn stations_near_town<'a>(town: &'a Town, stations: &'a [Station]) -> Vec<&'a Station> {
    stations
        .iter()
        .filter(|st| {
            !matches!(
                st.stop_kind,
                StopKind::RailWaypoint | StopKind::RoadWaypoint | StopKind::Buoy
            ) && crate::economy::manhattan_distance(st.pos, town.pos) <= TOWN_AUTHORITY_RADIUS
        })
        .collect()
}

#[must_use]
fn station_recently_active(station: &Station) -> bool {
    ALL_CARGO_TYPES
        .iter()
        .any(|cargo| station.time_since_pickup.get(*cargo) <= STATION_ACTIVE_DAYS)
}

#[must_use]
pub fn count_active_stations_near_town(town: &Town, stations: &[Station]) -> usize {
    stations_near_town(town, stations)
        .iter()
        .filter(|st| station_recently_active(st))
        .count()
}

#[must_use]
pub fn get_normal_growth_rate(
    town: &Town,
    stations: &[Station],
    map: &Map,
    industries: &[Industry],
) -> u16 {
    let n = count_active_stations_near_town(town, stations);
    let table = if town.fund_buildings_months > 0 {
        &GROW_COUNT_VALUES_FUNDED
    } else {
        &GROW_COUNT_VALUES_NORMAL
    };
    let idx = n.min(5);
    let m = table[idx];
    let houses = count_houses_for_growth(map, industries, town);
    let divisor = u16::try_from((houses / 50) + 1).unwrap_or(1);
    town_ticks_to_game_ticks(m / divisor)
}

fn update_town_grow_counter(town: &mut Town, prev_growth_rate: u16) {
    if town.growth_rate == TOWN_GROWTH_RATE_NONE {
        return;
    }
    if prev_growth_rate == TOWN_GROWTH_RATE_NONE {
        town.grow_counter = town.growth_rate.min(town.grow_counter);
        return;
    }
    let next = (u32::from(town.grow_counter) * (u32::from(town.growth_rate) + 1))
        / (u32::from(prev_growth_rate) + 1);
    town.grow_counter = u16::try_from(next).unwrap_or(town.growth_rate);
}

pub fn update_town_growth_rate(
    town: &mut Town,
    stations: &[Station],
    map: &Map,
    industries: &[Industry],
) {
    let old_rate = town.growth_rate;
    town.growth_rate = get_normal_growth_rate(town, stations, map, industries);
    update_town_grow_counter(town, old_rate);
}

fn chance16(rng: &mut Randomizer, a: u32, b: u32) -> bool {
    if b == 0 {
        return false;
    }
    rng.random_range(b) < a
}

/// Actualiza `is_growing` (`UpdateTownGrowth`).
pub fn update_town_growth_state(
    town: &mut Town,
    stations: &[Station],
    map: &Map,
    industries: &[Industry],
    climate: Climate,
    world_seed: u64,
    rng: &mut Randomizer,
) {
    update_town_growth_rate(town, stations, map, industries);
    town.is_growing = false;

    if town.fund_buildings_months > 0 {
        town.is_growing = true;
        return;
    }

    let has_station = !stations_near_town(town, stations).is_empty();
    if !has_station {
        return;
    }

    for (i, &goal) in town.goals.iter().enumerate() {
        if !town_goal_satisfied_with_context(
            goal,
            town.received_old[i],
            town.population,
            town.pos,
            map,
            climate,
            world_seed,
        ) {
            return;
        }
    }

    if count_active_stations_near_town(town, stations) == 0 && !chance16(rng, 1, 12) {
        return;
    }

    town.is_growing = true;
}

/// Rollover mensual de entregas + decaimiento de financiación + gate de crecimiento.
#[allow(clippy::too_many_arguments)]
pub fn process_town_monthly_growth(
    towns: &mut [Town],
    stations: &[Station],
    map: &Map,
    industries: &[Industry],
    climate: Climate,
    world_seed: u64,
    rng: &mut Randomizer,
    company_count: usize,
) {
    for town in &mut *towns {
        town.ensure_authority_ratings(company_count);
        update_town_rating(town, stations, company_count);
        town.received_old = town.received_new;
        town.received_new = [0; TOWN_GROWTH_EFFECT_COUNT];
        if town.fund_buildings_months > 0 {
            town.fund_buildings_months = town.fund_buildings_months.saturating_sub(1);
        }
        update_town_growth_state(town, stations, map, industries, climate, world_seed, rng);
    }
    crate::town_action::tick_town_authority_months(towns);
}

/// Evolución mensual de ratings (`UpdateTownRating`).
pub fn update_town_rating(town: &mut Town, stations: &[Station], company_count: usize) {
    town.ensure_authority_ratings(company_count);
    for i in 0..company_count.min(town.authority_ratings.len()) {
        if town.authority_ratings[i] < RATING_GROWTH_MAXIMUM {
            town.authority_ratings[i] = town.authority_ratings[i]
                .saturating_add(i16::from(RATING_GROWTH_UP_STEP))
                .min(RATING_GROWTH_MAXIMUM);
        }
    }

    let station_rating_updates: Vec<(usize, i32)> = stations_near_town(town, stations)
        .iter()
        .filter_map(|station| {
            let idx = station.owner.index();
            if idx >= town.authority_ratings.len() {
                return None;
            }
            let delta = if station_recently_active(station) {
                RATING_STATION_UP_STEP
            } else {
                RATING_STATION_DOWN_STEP
            };
            Some((
                idx,
                i32::from(town.authority_ratings[idx]) + i32::from(delta),
            ))
        })
        .collect();
    for (idx, next) in station_rating_updates {
        town.authority_ratings[idx] = i16::try_from(next.clamp(-1000, 1000)).unwrap_or(0);
    }
}

/// Periodo de generación (mismo orden de magnitud que [`crate::INDUSTRY_PRODUCE_TICKS`]).
pub const TOWN_PRODUCE_TICKS: u64 = 256;
/// Ciclo de intento de crecimiento urbano (`Ticks::TOWN_GROWTH_TICKS`).
pub const TOWN_GROWTH_TICKS: u64 = 70;
/// Población añadida al financiar edificios (feedback inmediato en UI).
pub const TOWN_GROWTH_POPULATION_STEP: u32 = 10;

/// Estimación media por casa y ciclo (solo UI; el runtime usa el spec de cada casa).
pub const PASSENGERS_PER_HOUSE: u32 = 2;
pub const MAIL_PER_HOUSE: u32 = 1;

/// Escala `population` / `mail_generation` al ciclo de 256 ticks (MVP `TileLoop_Town`).
const TOWN_CARGO_RATE_DIVISOR: u32 = 94;

/// Tope de espera en parada bus (análogo al stock de industria).
pub const STATION_TOWN_CARGO_CAPACITY: u32 = 500;

/// Radio de influencia de la autoridad local sobre nuevas estaciones.
pub const TOWN_AUTHORITY_RADIUS: u32 = 20;
/// Valoración mínima para construir estación cerca de una ciudad.
pub const AUTHORITY_MIN_STATION: i16 = -200;

pub const TOWN_ADVERTISE_COST: i64 = 1_000;
pub const TOWN_ADVERTISE_RATING_BOOST: i8 = 25;
pub const FUND_BUILDINGS_COST: i64 = 5_000;
pub const FUND_BUILDINGS_RATING_BOOST: i8 = 50;

/// Penalización al construir estación cerca de una ciudad.
pub const STATION_BUILD_RATING_PENALTY: i8 = -15;

/// Unidades de pasajeros/correo generadas por casa y ciclo según su spec.
#[must_use]
pub fn town_cargo_amount_per_cycle(rate: u16) -> u32 {
    if rate == 0 {
        return 0;
    }
    u32::from(rate).div_ceil(TOWN_CARGO_RATE_DIVISOR).max(1)
}

/// Añade pasajeros/correo por casa en cobertura de paradas bus/aeropuerto.
///
/// Cada casa usa `HouseSpec::population` y `mail_generation` de
/// [`crate::sav::house_spec_population`]. La cantidad pasa por
/// [`station::move_goods_to_station`]: el rating decide cuánto llega al andén y
/// las paradas que comparten casas compiten entre sí.
pub fn produce_town_cargo(
    map: &Map,
    _industries: &[Industry],
    stations: &mut [Station],
    towns: &[Town],
    tick: u64,
    selectgoods: bool,
) -> (u64, u64) {
    if tick == 0 || !tick.is_multiple_of(TOWN_PRODUCE_TICKS) {
        return (0, 0);
    }

    let station_coverage: Vec<(usize, TileCoord, i32)> = stations
        .iter()
        .enumerate()
        .filter(|(_, station)| matches!(station.stop_kind, StopKind::BusStop | StopKind::Airport))
        .map(|(idx, station)| (idx, station.pos, station::station_catchment_radius(station)))
        .collect();
    if station_coverage.is_empty() {
        return (0, 0);
    }

    let mut passengers = 0_u64;
    let mut mail = 0_u64;
    let (mw, mh) = map.dimensions();

    for y in 0..mh {
        for x in 0..mw {
            let tile_pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(tile) = map.get(tile_pos) else {
                continue;
            };
            if tile.kind != TileKind::House {
                continue;
            }
            let house_id = tile.m8 & 0x0FFF;
            let population = crate::sav::house_spec_population(house_id);
            let mail_rate = crate::sav::house_spec_mail_generation(house_id);
            if population == 0 && mail_rate == 0 {
                continue;
            }

            let covering: Vec<usize> = station_coverage
                .iter()
                .filter(|(_, station_pos, radius)| {
                    station::station_covers_tile(*station_pos, tile_pos, *radius)
                })
                .map(|(idx, _, _)| *idx)
                .collect();
            if covering.is_empty() {
                continue;
            }

            let exclusivity = closest_town_exclusivity(towns, tile_pos);
            let pax_amount = town_cargo_amount_per_cycle(population);
            if pax_amount > 0 {
                passengers += u64::from(station::move_goods_to_station(
                    stations,
                    &covering,
                    CargoType::Passengers,
                    pax_amount,
                    tile_pos,
                    selectgoods,
                    exclusivity,
                ));
            }
            let mail_amount = town_cargo_amount_per_cycle(mail_rate);
            if mail_amount > 0 {
                mail += u64::from(station::move_goods_to_station(
                    stations,
                    &covering,
                    CargoType::Mail,
                    mail_amount,
                    tile_pos,
                    selectgoods,
                    exclusivity,
                ));
            }
        }
    }

    (passengers, mail)
}

/// Dueño exclusivo del pueblo más cercano (si el contador sigue activo).
fn closest_town_exclusivity(towns: &[Town], pos: TileCoord) -> Option<crate::company::CompanyId> {
    let mut best: Option<(&Town, u32)> = None;
    for town in towns {
        let dist = crate::economy::manhattan_distance(town.pos, pos);
        if best.is_none_or(|(_, d)| dist < d) {
            best = Some((town, dist));
        }
    }
    best.and_then(|(town, _)| crate::town_action::town_exclusivity_owner(town))
}

/// Crece la población si `is_growing` (`TownTickHandler` / `GrowTown`).
///
/// Además intenta expansión física (calles/casas) y devuelve teselas dirty.
pub fn grow_town_if_served(
    map: &mut Map,
    industries: &[Industry],
    stations: &[Station],
    towns: &mut [Town],
    tick: u64,
) -> Vec<TileCoord> {
    grow_town_if_served_with_ctx(
        map,
        industries,
        stations,
        towns,
        tick,
        Climate::Temperate,
        1960,
        &[],
        &[],
    )
}

/// Variante con clima/año/catálogo para selección de casas (P3.5 / #253).
#[allow(clippy::too_many_arguments)]
pub fn grow_town_if_served_with_ctx(
    map: &mut Map,
    industries: &[Industry],
    stations: &[Station],
    towns: &mut [Town],
    tick: u64,
    climate: Climate,
    calendar_year: u32,
    house_catalog: &[crate::house_spec::HouseSpecDef],
    house_overrides: &[u16],
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for town in towns {
        if !town.is_growing {
            continue;
        }
        let mut counter = i32::from(town.grow_counter) - 1;
        if counter < 0 {
            // `growth_funded` es una estadística acumulada para la UI; sólo
            // el programa activo de tres meses habilita crecimiento sin
            // estación, igual que `Town::fund_buildings_months` en OpenTTD.
            let funded = town.fund_buildings_months > 0;
            let has_station = !stations_near_town(town, stations).is_empty();
            if !funded && !has_station {
                counter = i32::from(
                    town.growth_rate
                        .min(u16::try_from(TOWN_GROWTH_TICKS - 1).unwrap_or(0)),
                );
            } else if try_expand_growing_town_with_ctx(
                map,
                industries,
                stations,
                town,
                tick,
                climate,
                calendar_year,
                house_catalog,
                house_overrides,
                &mut dirty,
            ) {
                counter = i32::from(town.growth_rate);
            } else {
                counter = i32::from(
                    town.growth_rate
                        .min(u16::try_from(TOWN_GROWTH_TICKS - 1).unwrap_or(0)),
                );
            }
        }
        town.grow_counter = u16::try_from(counter.max(0)).unwrap_or(0);
    }
    dirty
}

/// Expansión con clima/año/catálogo (desde `GameState`).
#[allow(clippy::too_many_arguments)]
pub fn try_expand_growing_town_with_ctx(
    map: &mut Map,
    industries: &[Industry],
    stations: &[Station],
    town: &mut Town,
    tick: u64,
    climate: crate::world_gen::Climate,
    calendar_year: u32,
    house_catalog: &[crate::house_spec::HouseSpecDef],
    house_overrides: &[u16],
    dirty: &mut Vec<TileCoord>,
) -> bool {
    let funded = town.fund_buildings_months > 0;
    let has_station = !stations_near_town(town, stations).is_empty();
    if !funded && !has_station {
        return false;
    }
    let coverage = station::station_coverage_at(map, industries, town.pos, STATION_COVERAGE_RADIUS);
    if coverage.house_tiles == 0 && !funded && !has_station {
        return false;
    }
    let ctx = crate::town_expand::TownExpandContext {
        climate,
        calendar_year,
        house_catalog,
        house_overrides,
    };
    let before_houses = town.num_houses;
    let placed = crate::town_expand::expand_town_physically_with_ctx(map, town, tick, ctx);
    if placed.is_empty() {
        return false;
    }
    // Feedback de crecimiento si solo se extendió calle (sin casa nueva).
    if town.num_houses == before_houses {
        town.population = town.population.saturating_add(TOWN_GROWTH_POPULATION_STEP);
    }
    dirty.extend(placed);
    true
}

/// Probabilidad de demolición/renovación por visita de tile loop (`20/256`).
pub const HOUSE_REBUILD_CHANCE_NUM: u32 = 20;
pub const HOUSE_REBUILD_CHANCE_DEN: u32 = 256;

/// Incrementa la edad de todas las casas completadas (`IncrementHouseAge` anual).
pub fn increment_all_house_ages(map: &mut Map) {
    let (mw, mh) = map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(mut tile) = map.get(pos) else {
                continue;
            };
            if tile.kind != TileKind::House {
                continue;
            }
            // Completada: bit 7 de m3.
            if tile.m3 & 0x80 == 0 {
                continue;
            }
            if tile.m5 < 0xFF {
                tile.m5 = tile.m5.saturating_add(1);
                let _ = map.set_tile(pos, tile);
            }
        }
    }
}

/// Renovación urbana en visitas del tile loop (`TileLoop_Town` aging/rebuild).
///
/// Pasado `minimum_life`, con probabilidad 20/256 demuele y reconstruye.
#[allow(clippy::too_many_arguments)]
pub fn tile_loop_town_house_renovation(
    map: &mut Map,
    towns: &mut [Town],
    visits: &[TileCoord],
    climate: Climate,
    calendar_year: u32,
    house_catalog: &[crate::house_spec::HouseSpecDef],
    house_overrides: &[u16],
    rng: &mut Randomizer,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &pos in visits {
        let Some(tile) = map.get(pos) else {
            continue;
        };
        if tile.kind != TileKind::House || tile.m3 & 0x80 == 0 {
            continue;
        }
        let house_id = tile.m8 & 0x0FFF;
        let age = tile.m5;
        let Some(hs) = crate::house_spec::HouseSpec::get(house_id) else {
            continue;
        };
        if age < hs.minimum_life {
            continue;
        }
        let Some((town_idx, _)) = nearest_town_index(towns, pos) else {
            continue;
        };
        if !towns[town_idx].is_growing {
            continue;
        }
        // Contador global + chance 20/256 (plan P3.6).
        if towns[town_idx].time_until_rebuild > 0 {
            towns[town_idx].time_until_rebuild -= 1;
        }
        if towns[town_idx].time_until_rebuild != 0 {
            continue;
        }
        if !chance16(rng, HOUSE_REBUILD_CHANCE_NUM, HOUSE_REBUILD_CHANCE_DEN) {
            // Reprogramar aunque no demuela ahora.
            towns[town_idx].time_until_rebuild = u16::try_from(rng.random_range(256))
                .unwrap_or(192)
                .saturating_add(192);
            continue;
        }
        towns[town_idx].time_until_rebuild = u16::try_from(rng.random_range(256))
            .unwrap_or(192)
            .saturating_add(192);

        // Demoler → hierba.
        let height = tile.height;
        let mut clear = tile;
        clear.kind = TileKind::Grass;
        clear.mapt = 0;
        clear.m5 = 3; // hierba completa
        clear.m3 = 0;
        clear.m8 = 0;
        clear.m1 = 0;
        if map.set_tile(pos, clear).is_err() {
            continue;
        }
        towns[town_idx].num_houses = towns[town_idx].num_houses.saturating_sub(1);
        if hs.is_church() {
            towns[town_idx].has_church = false;
        }
        if hs.is_stadium() {
            towns[town_idx].has_stadium = false;
        }
        update_town_radius(&mut towns[town_idx]);
        dirty.push(pos);

        // Reconstruir (~244/256 en OpenTTD: GB(r,24,8) >= 12). Aquí siempre intentamos.
        let ctx = crate::town_expand::TownExpandContext {
            climate,
            calendar_year,
            house_catalog,
            house_overrides,
        };
        if crate::town_expand::place_house_with_spec(
            map,
            &mut towns[town_idx],
            pos,
            ctx,
            rng.next(),
        )
        .is_some()
        {
            dirty.push(pos);
        } else {
            // Dejar hierba si no hay spec válido.
            let _ = height;
        }
    }
    dirty
}

/// Límite de spam al financiar (`TownActionFundBuildings`).
pub fn cap_grow_counter_after_fund(town: &mut Town) {
    let growth_ticks = u16::try_from(TOWN_GROWTH_TICKS).unwrap_or(1);
    let modulo = if town.growth_rate > 0 {
        (town.growth_rate - (town.grow_counter % town.growth_rate)) % growth_ticks
    } else {
        0
    };
    let cap = 2 * growth_ticks - modulo;
    town.grow_counter = town.grow_counter.min(cap);
}

/// Registra el programa de financiación de edificios.
///
/// La cadencia se recalcula después desde el comando, con acceso a mapa y
/// estaciones. Hacerlo aquí con `growth_rate = 0` provocaba una expansión por
/// tick hasta que corriera el siguiente ciclo mensual.
pub fn apply_fund_buildings_boost(town: &mut Town) {
    town.growth_funded = town.growth_funded.saturating_add(1);
    town.fund_buildings_months = FUND_BUILDINGS_MONTHS;
}

#[must_use]
pub fn nearest_town_index(towns: &[Town], pos: TileCoord) -> Option<(usize, u32)> {
    towns
        .iter()
        .enumerate()
        .map(|(i, t)| (i, crate::economy::manhattan_distance(t.pos, pos)))
        .min_by_key(|(_, d)| *d)
}

/// Umbrales mínimos de rating para demolición municipal (`needed_rating` en `CheckforTownRating`).
#[must_use]
pub fn needed_rating_for_demolition(
    tolerance: TownCouncilTolerance,
    check_type: TownRatingCheckType,
) -> i16 {
    match tolerance {
        TownCouncilTolerance::Permissive => -1000,
        TownCouncilTolerance::Lenient => match check_type {
            TownRatingCheckType::RoadRemove => 16,
            TownRatingCheckType::TunnelBridgeRemove => 144,
        },
        TownCouncilTolerance::Neutral => match check_type {
            TownRatingCheckType::RoadRemove => 64,
            TownRatingCheckType::TunnelBridgeRemove => 208,
        },
        TownCouncilTolerance::Hostile => match check_type {
            TownRatingCheckType::RoadRemove => 112,
            TownRatingCheckType::TunnelBridgeRemove => 400,
        },
    }
}

/// ¿La autoridad permite la acción destructiva? (`CheckforTownRating`).
#[must_use]
pub fn check_town_rating(
    town: &Town,
    company: CompanyId,
    check_type: TownRatingCheckType,
    tolerance: TownCouncilTolerance,
) -> bool {
    if tolerance == TownCouncilTolerance::Permissive {
        return true;
    }
    town.authority_rating(company) >= needed_rating_for_demolition(tolerance, check_type)
}

/// Comprueba si la autoridad local permite una nueva estación en `pos`.
#[must_use]
pub fn authority_allows_new_station(towns: &[Town], pos: TileCoord, company: CompanyId) -> bool {
    let Some((idx, dist)) = nearest_town_index(towns, pos) else {
        return true;
    };
    if dist > TOWN_AUTHORITY_RADIUS {
        return true;
    }
    towns[idx].authority_rating(company) >= AUTHORITY_MIN_STATION
}

/// Aplica penalización de autoridad al construir estación cerca de una ciudad.
pub fn apply_station_build_rating_penalty(
    towns: &mut [Town],
    pos: TileCoord,
    company: CompanyId,
) -> Option<(u32, i8)> {
    let (idx, dist) = nearest_town_index(towns, pos)?;
    if dist > TOWN_AUTHORITY_RADIUS {
        return None;
    }
    let town_id = towns[idx].id;
    let delta = towns[idx].adjust_rating(company, STATION_BUILD_RATING_PENALTY);
    Some((town_id, delta))
}

/// Registra entrega de carga urbana en la ciudad más cercana dentro del radio.
pub fn record_delivery_near_town(
    towns: &mut [Town],
    station_pos: TileCoord,
    cargo: CargoType,
    amount: u32,
) {
    let Some((idx, dist)) = nearest_town_index(towns, station_pos) else {
        return;
    };
    if dist > TOWN_AUTHORITY_RADIUS {
        return;
    }
    towns[idx].record_town_cargo_delivery(cargo, amount);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{TileCoord, TileKind};

    #[test]
    fn newgrf_parent_scope_exposes_supported_town_variables() {
        let mut town = Town {
            id: 7,
            pos: TileCoord::new(0x12, 0x34),
            population: 0x1234,
            authority_ratings: vec![-1, 0x1234],
            growth_rate: 140,
            grow_counter: 70,
            num_houses: 9,
            squared_town_zone_radius: [0x1234, 0x5678, 0x9ABC, 0xDEF0, 0x1111],
            statues: 0x55,
            road_build_months: 4,
            fund_buildings_months: 3,
            native_flags: 0b111,
            have_ratings: 0xA5,
            larger_town: true,
            is_growing: true,
            has_church: true,
            has_stadium: true,
            ..Default::default()
        };
        town.history.samples = vec![
            crate::entity_history::TownHistorySample {
                population: 100,
                passengers_served: 11,
                mail_served: 13,
                rating: 500,
            },
            crate::entity_history::TownHistorySample {
                population: 120,
                passengers_served: 17,
                mail_served: 19,
                rating: 600,
            },
        ];
        town.received_new[TownGrowthEffect::Food as usize] = 0x1234;
        town.received_new[TownGrowthEffect::Water as usize] = 0x5678;
        town.received_old[TownGrowthEffect::Food as usize] = 0x9ABC;
        town.received_old[TownGrowthEffect::Water as usize] = 0xDEF0;
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        town.copy_newgrf_parent_scope(0, &mut ctx);

        assert_eq!(ctx.parent_vars.get(&0x41), Some(&7));
        assert_eq!(ctx.parent_vars.get(&0x40), Some(&1));
        assert_eq!(ctx.parent_vars.get(&0x80), Some(&0x12));
        assert_eq!(ctx.parent_vars.get(&0x81), Some(&0x34));
        assert_eq!(ctx.parent_vars.get(&0x82), Some(&0x1234));
        assert_eq!(ctx.parent_vars.get(&0x83), Some(&0x12));
        assert_eq!(ctx.parent_vars.get(&0x8A), Some(&1));
        assert_eq!(ctx.parent_vars.get(&0x92), Some(&0x07));
        assert_eq!(ctx.parent_vars.get(&0x93), Some(&0));
        assert_eq!(ctx.parent_vars.get(&0xB2), Some(&0x55));
        assert_eq!(ctx.parent_vars.get(&0xB6), Some(&9));
        assert_eq!(ctx.parent_vars.get(&0xB9), Some(&2));
        assert_eq!(ctx.parent_vars.get(&0x94), Some(&0x1234));
        assert_eq!(ctx.parent_vars.get(&0x95), Some(&0x12));
        assert_eq!(ctx.parent_vars.get(&0x9E), Some(&0xFFFF));
        assert_eq!(ctx.parent_vars.get(&0x9F), Some(&0xFF));
        assert_eq!(ctx.parent_vars.get(&0xBA), Some(&17));
        assert_eq!(ctx.parent_vars.get(&0xBC), Some(&19));
        assert_eq!(ctx.parent_vars.get(&0xC2), Some(&11));
        assert_eq!(ctx.parent_vars.get(&0xC4), Some(&13));
        assert_eq!(ctx.parent_vars.get(&0xCC), Some(&0x1234));
        assert_eq!(ctx.parent_vars.get(&0xD2), Some(&0xDEF0));
        assert_eq!(ctx.parent_vars.get(&0xD4), Some(&4));
        assert_eq!(ctx.parent_vars.get(&0xD5), Some(&3));
        assert_eq!(ctx.parent_vars.get(&0xAE), Some(&0xA5));
    }

    #[test]
    fn produce_adds_cargo_when_houses_in_coverage() {
        let mut map = Map::new_flat(16, 16, 0);
        let stop_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        map.set_kind(TileCoord::new(8, 7), TileKind::House).unwrap();

        let mut stations = vec![Station::new_with_kind(stop_pos, StopKind::BusStop)];
        // La parada ya tiene servicio: si no, selectgoods no deja llegar pasajeros.
        stations[0].goods.get_mut(CargoType::Passengers).last_speed = 1;
        stations[0].goods.get_mut(CargoType::Mail).last_speed = 1;

        let (pax, mail) =
            produce_town_cargo(&map, &[], &mut stations, &[], TOWN_PRODUCE_TICKS, true);
        // 4 pax × (175+1) >> 8 = 2; 2 mail × 176 >> 8 = 1.
        assert_eq!(pax, 2);
        assert_eq!(mail, 1);
        assert_eq!(stations[0].cargo_stock.passengers, 2);
        assert_eq!(stations[0].cargo_stock.mail, 1);
    }

    #[test]
    fn produce_uses_house_spec_population() {
        let mut map = Map::new_flat(16, 16, 0);
        let stop_pos = TileCoord::new(8, 8);
        map.set_completed_house(TileCoord::new(7, 8), 4, 0).unwrap(); // pop 220
        let mut stations = vec![Station::new_with_kind(stop_pos, StopKind::BusStop)];
        stations[0].goods.get_mut(CargoType::Passengers).last_speed = 1;

        assert_eq!(town_cargo_amount_per_cycle(220), 3);
        let (pax, _) = produce_town_cargo(&map, &[], &mut stations, &[], TOWN_PRODUCE_TICKS, true);
        // 3 pax × (175+1) >> 8 = 2
        assert_eq!(pax, 2);
        assert_eq!(stations[0].cargo_stock.passengers, 2);
    }

    #[test]
    fn competing_bus_stops_split_house_passengers_by_rating() {
        let mut map = Map::new_flat(16, 16, 0);
        let house = TileCoord::new(8, 8);
        map.set_completed_house(house, 0, 0).unwrap();
        let mut good = Station::new_with_kind(TileCoord::new(7, 8), StopKind::BusStop);
        let mut bad = Station::new_with_kind(TileCoord::new(9, 8), StopKind::BusStop);
        good.goods.get_mut(CargoType::Passengers).last_speed = 1;
        bad.goods.get_mut(CargoType::Passengers).last_speed = 1;
        good.goods.get_mut(CargoType::Passengers).rating = 200;
        bad.goods.get_mut(CargoType::Passengers).rating = 50;
        let mut stations = vec![good, bad];

        let (pax, _) = produce_town_cargo(&map, &[], &mut stations, &[], TOWN_PRODUCE_TICKS, true);
        assert!(pax > 0);
        assert!(
            stations[0].cargo_stock.passengers > stations[1].cargo_stock.passengers,
            "buena {} vs mala {}",
            stations[0].cargo_stock.passengers,
            stations[1].cargo_stock.passengers
        );
    }

    #[test]
    fn produce_skips_non_bus_stops() {
        let mut map = Map::new_flat(8, 8, 0);
        let pos = TileCoord::new(2, 2);
        map.set_kind(TileCoord::new(2, 1), TileKind::House).unwrap();
        let mut stations = vec![Station::new_with_kind(pos, StopKind::TruckStop)];

        let (pax, mail) =
            produce_town_cargo(&map, &[], &mut stations, &[], TOWN_PRODUCE_TICKS, true);
        assert_eq!(pax, 0);
        assert_eq!(mail, 0);
    }

    #[test]
    fn authority_blocks_station_when_rating_too_low() {
        let mut towns = vec![Town {
            id: 1,
            pos: TileCoord::new(5, 5),
            name: "Test".into(),
            population: 100,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        }];
        towns[0].authority_ratings[CompanyId::PLAYER.index()] = -500;
        assert!(!authority_allows_new_station(
            &towns,
            TileCoord::new(6, 5),
            CompanyId::PLAYER
        ));
        assert!(authority_allows_new_station(
            &towns,
            TileCoord::new(30, 30),
            CompanyId::PLAYER
        ));
    }

    #[test]
    fn town_grows_when_served() {
        let mut map = Map::new_flat(16, 16, 0);
        let town_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        let mut towns = vec![Town {
            id: 0,
            pos: town_pos,
            name: "Grow".into(),
            population: 100,
            passengers_served: 10,
            mail_served: 0,
            growth_funded: 0,
            is_growing: true,
            grow_counter: 0,
            growth_rate: 70,
            ..Default::default()
        }];
        let stations = vec![Station::new_with_kind(
            TileCoord::new(8, 9),
            StopKind::BusStop,
        )];
        grow_town_if_served(&mut map, &[], &stations, &mut towns, 1);
        assert!(towns[0].population > 100);
    }

    #[test]
    fn town_does_not_grow_when_goals_unmet() {
        let mut map = Map::new_flat(16, 16, 0);
        let town_pos = TileCoord::new(8, 8);
        map.set_height(town_pos, 12).unwrap();
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        let mut towns = vec![Town {
            id: 0,
            pos: town_pos,
            name: "Stuck".into(),
            population: 120,
            passengers_served: 10,
            is_growing: false,
            grow_counter: 0,
            ..Default::default()
        }];
        towns[0].init_growth_goals(Climate::SubArctic);
        let stations = vec![Station::new_with_kind(
            TileCoord::new(8, 9),
            StopKind::BusStop,
        )];
        let mut rng = Randomizer::new(1);
        update_town_growth_state(
            &mut towns[0],
            &stations,
            &map,
            &[],
            Climate::SubArctic,
            0,
            &mut rng,
        );
        grow_town_if_served(&mut map, &[], &stations, &mut towns, 1);
        assert_eq!(towns[0].population, 120);
        assert!(!towns[0].is_growing);
    }

    #[test]
    fn arctic_food_goal_blocks_large_town_without_goods() {
        let mut map = Map::new_flat(16, 16, 0);
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(5, 5),
            name: "Arctic".into(),
            population: 120,
            passengers_served: 50,
            ..Default::default()
        };
        map.set_height(town.pos, 12).unwrap();
        town.init_growth_goals(Climate::SubArctic);
        let stations = vec![Station::new_with_kind(
            TileCoord::new(5, 6),
            StopKind::BusStop,
        )];
        let mut rng = Randomizer::new(1);
        update_town_growth_state(
            &mut town,
            &stations,
            &map,
            &[],
            Climate::SubArctic,
            0,
            &mut rng,
        );
        assert!(!town.is_growing);
        town.received_old[TownGrowthEffect::Food as usize] = 1;
        update_town_growth_state(
            &mut town,
            &stations,
            &map,
            &[],
            Climate::SubArctic,
            0,
            &mut rng,
        );
        assert!(town.is_growing);
    }

    #[test]
    fn fund_buildings_forces_growth_gate_without_station() {
        let map = Map::new_flat(8, 8, 0);
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(5, 5),
            name: "Fund".into(),
            population: 200,
            fund_buildings_months: 3,
            ..Default::default()
        };
        town.init_growth_goals(Climate::SubArctic);
        let mut rng = Randomizer::new(1);
        update_town_growth_state(&mut town, &[], &map, &[], Climate::SubArctic, 0, &mut rng);
        assert!(town.is_growing);
    }

    #[test]
    fn fund_buildings_grows_without_station() {
        let mut map = Map::new_flat(16, 16, 0);
        let mut towns = vec![Town {
            id: 0,
            pos: TileCoord::new(8, 8),
            name: "Funded".into(),
            population: 50,
            fund_buildings_months: 3,
            is_growing: true,
            grow_counter: 0,
            growth_rate: 70,
            ..Default::default()
        }];
        grow_town_if_served(&mut map, &[], &[], &mut towns, 1);
        assert!(towns[0].population > 50);
        assert!(towns[0].is_growing);
    }

    #[test]
    fn historical_funding_count_does_not_keep_growth_forced() {
        let mut map = Map::new_flat(16, 16, 0);
        let mut towns = vec![Town {
            id: 0,
            pos: TileCoord::new(8, 8),
            name: "Expired funding".into(),
            population: 50,
            growth_funded: 1,
            is_growing: true,
            grow_counter: 0,
            growth_rate: 70,
            ..Default::default()
        }];

        let dirty = grow_town_if_served(&mut map, &[], &[], &mut towns, 1);

        assert!(dirty.is_empty());
        assert_eq!(towns[0].population, 50);
        assert_eq!(
            towns[0].grow_counter,
            u16::try_from(TOWN_GROWTH_TICKS - 1).unwrap_or(u16::MAX)
        );
    }

    #[test]
    fn apply_fund_boost_starts_only_the_three_month_program() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: "X".into(),
            population: 40,
            ..Default::default()
        };
        apply_fund_buildings_boost(&mut town);
        assert_eq!(town.fund_buildings_months, FUND_BUILDINGS_MONTHS);
        assert!(!town.is_growing);
        assert_eq!(town.population, 40);
        assert_eq!(town.growth_funded, 1);
    }

    #[test]
    fn adjust_rating_clamps() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: "X".into(),
            population: 0,
            authority_ratings: vec![990],
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        };
        town.adjust_rating(CompanyId::PLAYER, 50);
        assert_eq!(town.authority_rating(CompanyId::PLAYER), 1000);
    }

    #[test]
    fn update_town_rating_recovers_and_penalizes_by_station_service() {
        let mut town = Town {
            id: 1,
            pos: TileCoord::new(10, 10),
            name: "Rate".into(),
            authority_ratings: vec![0, 100],
            ..Default::default()
        };
        let mut good = Station::new_with_kind(TileCoord::new(10, 11), StopKind::BusStop);
        good.owner = CompanyId::PLAYER;
        good.time_since_pickup.passengers = 0;
        let mut bad = Station::new_with_kind(TileCoord::new(11, 10), StopKind::BusStop);
        bad.owner = CompanyId(1);
        for cargo in ALL_CARGO_TYPES {
            bad.time_since_pickup.set(cargo, 50);
        }
        update_town_rating(&mut town, &[good, bad], 2);
        assert_eq!(town.authority_rating(CompanyId::PLAYER), 17);
        assert_eq!(town.authority_rating(CompanyId(1)), 90);
    }

    #[test]
    fn authority_ratings_are_per_company() {
        let mut town = Town::default();
        town.adjust_rating(CompanyId::PLAYER, 10);
        town.adjust_rating(CompanyId(1), -20);
        assert_eq!(town.authority_rating(CompanyId::PLAYER), 510);
        assert_eq!(town.authority_rating(CompanyId(1)), 480);
    }

    #[test]
    fn growth_rate_scales_with_active_stations() {
        let map = Map::new_flat(16, 16, 0);
        let town = Town {
            id: 0,
            pos: TileCoord::new(8, 8),
            name: "Rate".into(),
            ..Default::default()
        };
        let unserved = get_normal_growth_rate(&town, &[], &map, &[]);
        let mut active: Vec<Station> = Vec::new();
        for i in 0..5 {
            let mut st = Station::new_with_kind(TileCoord::new(8 + i, 9), StopKind::BusStop);
            st.time_since_pickup.passengers = 0;
            active.push(st);
        }
        let well_served = get_normal_growth_rate(&town, &active, &map, &[]);
        assert!(
            well_served < unserved,
            "más estaciones activas aceleran el crecimiento"
        );
    }

    #[test]
    fn unserved_town_growth_requires_chance_without_funding() {
        let map = Map::new_flat(8, 8, 0);
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(4, 4),
            name: "Lonely".into(),
            population: 80,
            ..Default::default()
        };
        let mut station = Station::new_with_kind(TileCoord::new(4, 5), StopKind::BusStop);
        for cargo in ALL_CARGO_TYPES {
            station.time_since_pickup.set(cargo, 30);
        }
        let stations = vec![station];
        let mut never = Randomizer::new(42);
        update_town_growth_state(
            &mut town,
            &stations,
            &map,
            &[],
            Climate::Temperate,
            0,
            &mut never,
        );
        assert!(!town.is_growing);
        let mut lucky_found = false;
        for seed in 0..128 {
            let mut town2 = town.clone();
            let mut lucky = Randomizer::new(seed);
            update_town_growth_state(
                &mut town2,
                &stations,
                &map,
                &[],
                Climate::Temperate,
                0,
                &mut lucky,
            );
            if town2.is_growing {
                lucky_found = true;
                break;
            }
        }
        assert!(lucky_found, "alguna semilla debe pasar Chance16(1,12)");
    }

    #[test]
    fn update_town_radius_matches_table_for_small_towns() {
        let mut town = Town {
            num_houses: 0,
            ..Default::default()
        };
        update_town_radius(&mut town);
        assert_eq!(town.squared_town_zone_radius, [4, 0, 0, 0, 0]);
        town.num_houses = 20;
        update_town_radius(&mut town);
        assert_eq!(town.squared_town_zone_radius, [64, 0, 4, 0, 0]);
        town.num_houses = 88;
        update_town_radius(&mut town);
        assert_eq!(town.squared_town_zone_radius, [121, 81, 0, 49, 36]);
    }

    #[test]
    fn house_renovation_demolishes_when_chance_hits() {
        let mut map = Map::new_flat(16, 16, 0);
        let pos = TileCoord::new(8, 8);
        map.set_completed_house(pos, 6, 5).unwrap(); // town houses, min_life 0
        let mut towns = vec![Town {
            pos,
            is_growing: true,
            num_houses: 1,
            time_until_rebuild: 0,
            ..Default::default()
        }];
        update_town_radius(&mut towns[0]);
        let mut hit = false;
        for seed in 0..64 {
            let mut map2 = map.clone();
            let mut towns2 = towns.clone();
            let mut rng = Randomizer::new(seed);
            let dirty = tile_loop_town_house_renovation(
                &mut map2,
                &mut towns2,
                &[pos],
                Climate::Temperate,
                1980,
                &[],
                &[],
                &mut rng,
            );
            if !dirty.is_empty() {
                hit = true;
                break;
            }
        }
        assert!(hit, "alguna semilla debe disparar renovación 20/256");
    }

    /// `OpenTTD` arranca los pueblos en `RATING_INITIAL = 500` (`town_type.h:45`),
    /// no en neutral, así que la autoridad empieza siendo moderadamente favorable.
    #[test]
    fn new_town_starts_at_initial_rating() {
        let town = Town {
            pos: TileCoord::new(5, 5),
            ..Default::default()
        };
        assert_eq!(
            town.authority_rating(CompanyId::PLAYER),
            TOWN_RATING_INITIAL
        );
        assert_eq!(TOWN_RATING_INITIAL, 500);
        assert!(authority_allows_new_station(
            std::slice::from_ref(&town),
            TileCoord::new(6, 6),
            CompanyId::PLAYER
        ));
    }
}
