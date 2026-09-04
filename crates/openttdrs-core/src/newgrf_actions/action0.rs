//! Parsing compartido de cabeceras y metadatos Action0.

use crate::newgrf_config::{GrfScanError, parse_grf_container};
use crate::newgrf_walk::for_each_pseudo_sprite;
use crate::road_type::RoadTramType;
use crate::vehicle::VehicleKind;

/// Feature Action0: `Trains` (`OpenTTD` `GSF_TRAINS`).
pub const ACTION0_FEATURE_TRAINS: u8 = 0x00;
/// Feature Action0: `RoadVehicles` (`OpenTTD` `GSF_ROADVEHICLES`).
pub const ACTION0_FEATURE_ROAD_VEHICLES: u8 = 0x01;
/// Feature Action0: `Ships` (`OpenTTD` `GSF_SHIPS`).
pub const ACTION0_FEATURE_SHIPS: u8 = 0x02;
/// Feature Action0: `Aircraft` (`OpenTTD` `GSF_AIRCRAFT`).
pub const ACTION0_FEATURE_AIRCRAFT: u8 = 0x03;
/// Feature Action0: `Stations` (`OpenTTD` `GSF_STATIONS`).
pub const ACTION0_FEATURE_STATIONS: u8 = 0x04;
/// Feature Action0: `Canals` (`OpenTTD` `GSF_CANALS`).
pub const ACTION0_FEATURE_CANALS: u8 = 0x05;
/// Feature Action0: `Bridges` (`OpenTTD` `GSF_BRIDGES`).
pub const ACTION0_FEATURE_BRIDGES: u8 = 0x06;
/// Feature Action0: `Houses` (`OpenTTD` `GSF_HOUSES`).
pub const ACTION0_FEATURE_HOUSES: u8 = 0x07;
/// Feature Action0: `IndustryTiles` (`OpenTTD` `GSF_INDUSTRYTILES`).
pub const ACTION0_FEATURE_INDUSTRYTILES: u8 = 0x09;
/// Feature Action0: `Industries` (`OpenTTD` `GSF_INDUSTRIES`).
pub const ACTION0_FEATURE_INDUSTRIES: u8 = 0x0A;
/// Feature Action0: `Cargoes` (`OpenTTD` `GSF_CARGOES`).
pub const ACTION0_FEATURE_CARGOES: u8 = 0x0B;
/// Feature Action0: `Sounds` (`OpenTTD` `GSF_SOUNDFX`).
pub const ACTION0_FEATURE_SOUNDS: u8 = 0x0C;
/// Feature Action0: `Airports` (`OpenTTD` `GSF_AIRPORTS`).
pub const ACTION0_FEATURE_AIRPORTS: u8 = 0x0D;
/// Feature Action0: `Objects` (`OpenTTD` `GSF_OBJECTS`).
pub const ACTION0_FEATURE_OBJECTS: u8 = 0x0F;
/// Feature Action0: `AirportTiles` (`OpenTTD` `GSF_AIRPORTTILES`).
pub const ACTION0_FEATURE_AIRPORTTILES: u8 = 0x11;
/// Feature Action0: `RailTypes` (`OpenTTD` `GSF_RAILTYPES`).
pub const ACTION0_FEATURE_RAILTYPES: u8 = 0x10;
/// Feature Action0: `RoadTypes` (`OpenTTD` `GSF_ROADTYPES`).
pub const ACTION0_FEATURE_ROADTYPES: u8 = 0x12;
/// Feature Action0: `RoadStops` (`OpenTTD` `GSF_ROADSTOPS`).
pub const ACTION0_FEATURE_ROADSTOPS: u8 = 0x14;
/// Feature Action0: `Badges` (`OpenTTD` `GSF_BADGES`).
pub const ACTION0_FEATURE_BADGES: u8 = 0x15;

/// `IndustryTiles`: substitute vanilla gfx (`prop 0x08`).
const PROP_INDTILE_SUBST: u8 = 0x08;
/// `IndustryTiles`: override vanilla gfx (`prop 0x09`).
const PROP_INDTILE_OVERRIDE: u8 = 0x09;
/// `IndustryTiles`: acceptance slot 0..2 WORD (`prop 0x0A`–`0x0C`).
const PROP_INDTILE_ACCEPT_0: u8 = 0x0A;
const PROP_INDTILE_ACCEPT_1: u8 = 0x0B;
const PROP_INDTILE_ACCEPT_2: u8 = 0x0C;
/// `IndustryTiles`: callback mask BYTE (`prop 0x0E`).
const PROP_INDTILE_CALLBACK_MASK: u8 = 0x0E;
/// `IndustryTiles`: variable-length acceptance (`prop 0x13`).
const PROP_INDTILE_ACCEPT_LIST: u8 = 0x13;
/// `IndustryTiles`: badge list WORD count + n×WORD (`prop 0x14`).
const PROP_INDTILE_BADGES: u8 = 0x14;

/// Prop etiqueta 4 chars (`RoadTypes` short / Stations class label / Badges / Objects / `RoadStops` class).
const PROP_LABEL: u8 = 0x08;
/// Prop flags (`RoadTypes`: bit0 = tram; `Badges`: DWORD).
const PROP_FLAGS: u8 = 0x09;
/// `RoadStops`: tipo de parada BYTE (`0` bus / `1` truck / `2` all; OTTD `0x09`).
const PROP_ROADSTOP_STOP_TYPE: u8 = 0x09;
/// `RoadStops`: draw modes BYTE (`OpenTTD` `0x0C`).
const PROP_ROADSTOP_DRAW_MODE: u8 = 0x0C;
/// `RoadStops`: general flags DWORD (`OpenTTD` `0x12`).
const PROP_ROADSTOP_FLAGS: u8 = 0x12;
/// Cargoes: bit number (`OpenTTD` `0x08`).
const PROP_CARGO_BITNUM: u8 = 0x08;
/// Cargoes: label 4 chars (`OpenTTD` `0x17`).
const PROP_CARGO_LABEL: u8 = 0x17;
/// Cargoes: callback mask BYTE (`OpenTTD` `0x1A`).
const PROP_CARGO_CALLBACK_MASK: u8 = 0x1A;
/// Sounds: relative volume BYTE (`OpenTTD` `0x08`; default 128).
const PROP_SOUND_VOLUME: u8 = 0x08;
/// Sounds: priority BYTE (`OpenTTD` `0x09`).
const PROP_SOUND_PRIORITY: u8 = 0x09;
/// Sounds: override old `SoundId` BYTE (`OpenTTD` `0x0A`).
const PROP_SOUND_OVERRIDE: u8 = 0x0A;
/// Canals: callback mask BYTE (`OpenTTD` `0x08`).
const PROP_CANAL_CALLBACK_MASK: u8 = 0x08;
/// Canals: flags BYTE (`OpenTTD` `0x09`).
const PROP_CANAL_FLAGS: u8 = 0x09;
/// Bridges: year of availability BYTE (`OpenTTD` `0x08`).
const PROP_BRIDGE_YEAR: u8 = 0x08;
/// Bridges: minimum length BYTE (`OpenTTD` `0x09`).
const PROP_BRIDGE_MIN_LEN: u8 = 0x09;
/// Bridges: maximum length BYTE (`OpenTTD` `0x0A`; `>16` → unlimited).
const PROP_BRIDGE_MAX_LEN: u8 = 0x0A;
/// Bridges: cost factor BYTE (`OpenTTD` `0x0B`).
const PROP_BRIDGE_PRICE: u8 = 0x0B;
/// Bridges: max speed WORD (`OpenTTD` `0x0C`; `0` → `u16::MAX`).
const PROP_BRIDGE_SPEED: u8 = 0x0C;
/// Bridges: sprite tables (consumida; compleja).
const PROP_BRIDGE_SPRITE_TABLES: u8 = 0x0D;
/// Bridges: flags BYTE (`OpenTTD` `0x0E`).
const PROP_BRIDGE_FLAGS: u8 = 0x0E;
/// Bridges: long format year DWORD (`OpenTTD` `0x0F`).
const PROP_BRIDGE_YEAR_LONG: u8 = 0x0F;
/// Bridges: purchase / rail / road string IDs WORD (`0x10`–`0x12`).
const PROP_BRIDGE_STR_PURCHASE: u8 = 0x10;
const PROP_BRIDGE_STR_RAIL: u8 = 0x11;
const PROP_BRIDGE_STR_ROAD: u8 = 0x12;
/// Bridges: 16-bit cost multiplier WORD (`OpenTTD` `0x13`).
const PROP_BRIDGE_PRICE_WORD: u8 = 0x13;
/// Bridges: pillar flags extended list (`OpenTTD` `0x15`).
const PROP_BRIDGE_PILLARS: u8 = 0x15;
/// `SPRITES_PER_BRIDGE_PIECE` en `OpenTTD`.
const BRIDGE_SPRITES_PER_PIECE: usize = 32;
/// Objects: climate mask BYTE (`OpenTTD` `0x0B`).
const PROP_OBJECT_CLIMATE: u8 = 0x0B;
/// Objects: size BYTE (`OpenTTD` `0x0C`).
const PROP_OBJECT_SIZE: u8 = 0x0C;
/// Objects: build cost multiplier BYTE (`OpenTTD` `0x0D`).
const PROP_OBJECT_BUILD_COST: u8 = 0x0D;
/// Objects: callback mask WORD (`OpenTTD` `0x15`).
const PROP_OBJECT_CALLBACK_MASK: u8 = 0x15;
/// Stations: callback mask (`OpenTTD` 15.3).
const PROP_STATION_CALLBACK_MASK: u8 = 0x0B;
/// Stations: platforms disallowed bitmask (`OpenTTD` `0x0C`).
const PROP_STATION_DISALLOWED_PLATFORMS: u8 = 0x0C;
/// Stations: lengths disallowed bitmask (`OpenTTD` `0x0D`).
const PROP_STATION_DISALLOWED_LENGTHS: u8 = 0x0D;
/// Stations: custom tile layout (platforms × length).
const PROP_STATION_CUSTOM_LAYOUT: u8 = 0x0E;
/// Stations: copy custom layout from another station id.
const PROP_STATION_COPY_LAYOUT: u8 = 0x0F;
/// Feature Action0: `TramTypes` (`OpenTTD` `GSF_TRAMTYPES`; mismo handler que `RoadTypes`).
pub const ACTION0_FEATURE_TRAMTYPES: u8 = 0x13;
/// Prop año introducción (uint16 LE).
const PROP_INTRO_YEAR: u8 = 0x16;
/// Extensión local: nombre C-string (tests / GRFs propios).
const PROP_NAME_CSTRING: u8 = 0xFE;
/// Extensión local: lista de badges asociados (BYTE count + N× label 4 chars).
const PROP_BADGE_ASSOCIATIONS: u8 = 0xFD;

/// Prop velocidad máxima tren (uint16 LE) — extensión local / subset Action0.
const PROP_TRAIN_SPEED: u8 = 0x09;
/// Prop potencia (uint16 LE).
const PROP_TRAIN_POWER: u8 = 0x0B;

/// Cabecera Action0 parseada (sin props).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action0Header {
    pub feature: u8,
    pub num_props: u8,
    pub num_ids: u8,
}

/// Metadatos `RoadTypes` leídos de un Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRoadTypeMeta {
    pub class: RoadTramType,
    pub label: String,
    pub short_label: String,
    pub intro_year: u16,
    /// Prop `0x14` speed limit (`0` = sin techo).
    pub max_speed: u16,
    /// Prop `0x13` construction cost factor (`0` = default).
    pub cost_multiplier: u16,
    /// Prop `0x1C` maintenance cost factor (`0` = default).
    pub maintenance_multiplier: u16,
    /// Prop `0x10` flags BYTE.
    pub flags: u8,
    /// Labels de `prop 0x0F` powered list (sin resolver).
    pub powered_labels: Vec<[u8; 4]>,
    /// Action0 `0x1E`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
    /// `true` si el bloque era feature `TramTypes` (`0x13`), no extensión local.
    pub from_tramtypes_feature: bool,
}

/// Metadatos `Stations` leídos de un Action0 (antes de asignar IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStationMeta {
    pub class_short_label: String,
    pub class_label: String,
    pub short_label: String,
    pub label: String,
    pub disallowed_platforms: u8,
    pub disallowed_lengths: u8,
    pub callback_mask: u8,
    /// Action0 `0x13`: flags generales (`Cb141RandomBits` = bit 2).
    pub flags: u8,
    /// Action0 `0x16`: estado y último frame de animación.
    pub animation_status: u8,
    pub animation_frames: u8,
    /// Action0 `0x17`: velocidad base de animación.
    pub animation_speed: u8,
    /// Action0 `0x18`: `StationAnimationTrigger` que invocan CB140.
    pub animation_triggers: u16,
    /// Layouts prop `0x0E`: `(platforms, length)` → tiletypes.
    pub custom_layouts: std::collections::HashMap<(u8, u8), Vec<u8>>,
    /// Prop `0x0F`: copiar layouts desde este id local (si definido).
    pub copy_layout_from: Option<u16>,
}

/// Metadatos `Trains` Action0 (antes de asignar ID ≥1000).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ParsedTrainMeta {
    /// Primer id local definido por el bloque Action0.
    pub local_id: u16,
    pub name: String,
    pub intro_year: u16,
    pub max_speed: u16,
    pub power_hp: u32,
    pub lifelength_years: u8,
    pub model_life_years: u8,
    pub reliability_spd_dec: u16,
    pub climate_mask: u8,
    pub load_amount: u8,
    pub train_image_index: u8,
    pub dual_headed: bool,
    /// Action0 train `0x19`: clase de tracción (`EngineClass`).
    pub rail_engine_class: u8,
    /// Action0 misc flags bit 2: unidad múltiple DMU/EMU.
    pub rail_is_mu: bool,
    /// Action0 misc flags bit 1: usa la segunda rampa de compañía (2CC).
    pub uses_2cc: bool,
    pub capacity: u32,
    pub cargo: Option<crate::cargo::CargoType>,
    /// Índice local de `0x15` antes de aplicar la CTT del GRF.
    #[allow(clippy::struct_field_names)]
    pub default_cargo_local_id: Option<u8>,
    pub weight_t: u16,
    pub price_factor: u8,
    pub running_cost_factor: u8,
    pub pow_wag_power: u32,
    pub pow_wag_weight: u16,
    pub rail_tilts: bool,
    pub curve_speed_mod: i16,
    pub tractive_effort: u8,
    pub air_drag: u8,
    pub shorten_factor: u8,
    pub required_rail_type: Option<u8>,
    pub refit_mask: u32,
    /// Listas CTT (`0x2C`/`0x2D`) antes de resolverlas contra el catálogo.
    pub ctt_include_cargo_indices: Vec<u8>,
    pub ctt_exclude_cargo_indices: Vec<u8>,
    /// Máscara de callbacks de vehículo (bit 7 = `SoundEffect`).
    pub callback_mask: u16,
    /// Action0 train `0x22`: efecto visual bit-stuffed.
    pub visual_effect: u8,
    /// Action0 misc flag bit 7: `OpenTTD` draws a sequence of stacked sprites.
    pub sprite_stack: bool,
    /// Action0 train `0x33`: índices locales de la tabla de traducción de badges.
    pub badge_local_ids: Vec<u16>,
}

/// Subset de propiedades Action0 que alimenta el catálogo jugable de vehículos.
///
/// Los campos no representados por [`crate::engine::EngineDef`] se consumen con
/// su ancho de `OpenTTD` 15.3, pero no se anuncian como aplicados.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ParsedVehicleMeta {
    pub local_id: u16,
    pub kind: VehicleKind,
    pub name: String,
    pub intro_year: u16,
    pub max_speed: u16,
    pub price_factor: u8,
    pub running_cost_factor: u8,
    pub capacity: u32,
    pub cargo: Option<crate::cargo::CargoType>,
    /// Índice local de la propiedad de cargo por defecto (`0x10`/`0x0C`).
    #[allow(clippy::struct_field_names)]
    pub default_cargo_local_id: Option<u8>,
    pub power_hp: u32,
    pub weight_t: u16,
    pub lifelength_years: u8,
    pub model_life_years: u8,
    pub climate_mask: u8,
    pub load_amount: u8,
    pub reliability_spd_dec: u16,
    pub is_helicopter: bool,
    pub is_large_aircraft: bool,
    pub ocean_speed_frac: u8,
    pub canal_speed_frac: u8,
    pub sound_effect: u8,
    /// Action0 visual effect (`road 0x21`, `ship 0x1C`).
    pub visual_effect: u8,
    /// Action0 ship `0x1E` CTT include → bitmask temperate (`0` = lista vanilla).
    pub refit_mask: u32,
    /// Action0 ship `0x1F` CTT exclude → bitmask temperate. Se resta de
    /// `refit_mask` cuando el GRF declara ambas listas y de la lista vanilla
    /// cuando sólo declara exclusiones.
    pub refit_exclude_mask: u32,
    /// Listas CTT antes de traducirlas con la tabla del GRF.
    pub ctt_include_cargo_indices: Vec<u8>,
    pub ctt_exclude_cargo_indices: Vec<u8>,
    /// Máscara de callbacks de vehículo (bit 7 = `SoundEffect`).
    pub callback_mask: u16,
    /// Action0 misc flags bit 1: usa la segunda rampa de compañía (2CC).
    pub uses_2cc: bool,
    /// Action0 misc flag bit 7: `OpenTTD` draws a sequence of stacked sprites.
    pub sprite_stack: bool,
    /// Action0 vehicle badge list (`road 0x2A`, `ship 0x26`, `aircraft 0x24`).
    /// Los índices se traducen contra `GlobalVar` `0x18` durante `apply`.
    pub badge_local_ids: Vec<u16>,
}

impl ParsedVehicleMeta {
    fn defaults(feature: u8, local_id: u16) -> Option<Self> {
        let (kind, name, speed, price, running, capacity, cargo, power, weight, life) =
            match feature {
                ACTION0_FEATURE_ROAD_VEHICLES => (
                    VehicleKind::Bus,
                    "NewGRF Road Vehicle",
                    96,
                    128,
                    128,
                    20,
                    Some(crate::cargo::CargoType::Passengers),
                    100,
                    10,
                    30,
                ),
                ACTION0_FEATURE_SHIPS => (
                    VehicleKind::Ship,
                    "NewGRF Ship",
                    80,
                    128,
                    128,
                    100,
                    Some(crate::cargo::CargoType::Passengers),
                    0,
                    100,
                    30,
                ),
                ACTION0_FEATURE_AIRCRAFT => (
                    VehicleKind::Aircraft,
                    "NewGRF Aircraft",
                    320,
                    128,
                    128,
                    100,
                    Some(crate::cargo::CargoType::Passengers),
                    0,
                    30,
                    30,
                ),
                _ => return None,
            };
        Some(Self {
            local_id,
            kind,
            name: name.into(),
            intro_year: 1920,
            max_speed: speed,
            price_factor: price,
            running_cost_factor: running,
            capacity,
            cargo,
            default_cargo_local_id: None,
            power_hp: power,
            weight_t: weight,
            lifelength_years: life,
            model_life_years: u8::MAX,
            climate_mask: 0x0F,
            load_amount: 0,
            reliability_spd_dec: if feature == ACTION0_FEATURE_SHIPS {
                crate::engine::SHIP_RELIABILITY_SPD_DEC
            } else {
                crate::engine::DEFAULT_RELIABILITY_SPD_DEC
            },
            is_helicopter: false,
            is_large_aircraft: false,
            ocean_speed_frac: 0,
            canal_speed_frac: 0,
            sound_effect: 0,
            visual_effect: crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT,
            refit_mask: 0,
            refit_exclude_mask: 0,
            ctt_include_cargo_indices: Vec::new(),
            ctt_exclude_cargo_indices: Vec::new(),
            callback_mask: 0,
            uses_2cc: false,
            sprite_stack: false,
            badge_local_ids: Vec::new(),
        })
    }
}

/// Asociación local de `RailType` Action0 (`prop 0x08`) con una etiqueta global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRailTypeMeta {
    pub local_id: u8,
    pub label: crate::newgrf_type_tables::TypeLabel,
    /// Prop `0x14` speed limit (`0` = sin techo).
    pub max_speed: u16,
    /// Prop `0x13` construction cost factor.
    pub cost_multiplier: u16,
    /// Prop `0x1C` maintenance cost factor.
    pub maintenance_multiplier: u16,
    /// Prop `0x10` flags.
    pub flags: u8,
    /// Prop `0x11` curve speed advantage.
    pub curve_speed: u8,
    /// Prop `0x17` introduction date (días desde epoch OTTD); `0` = siempre.
    pub introduction_date: u32,
    /// Labels `prop 0x0E` compatible.
    pub compatible_labels: Vec<[u8; 4]>,
    /// Labels `prop 0x0F` powered (implica compatible).
    pub powered_labels: Vec<[u8; 4]>,
    /// Action0 `0x1E`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
}

/// Metadatos `IndustryTiles` Action0 (antes de asignar gfx ≥175).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIndustryTileMeta {
    pub local_id: u8,
    /// Substitute vanilla (`prop 0x08`); obligatorio para crear slot.
    pub subst_id: u8,
    /// Override de gfx vanilla (`prop 0x09`).
    pub override_of: Option<u8>,
    /// Índices GRF-local de cargos aceptados (`0x0A`–`0x0C` / `0x13`).
    pub accepts_cargo_indices: Vec<u8>,
    /// Cantidades de aceptación (octavos; pueden ser negativas en `0x13`).
    pub acceptance: Vec<i8>,
    /// Máscara de pendientes rechazadas (`prop 0x0D`).
    pub slopes_refused: u8,
    /// Callback mask (`prop 0x0E`): bit 0 = next frame, bit 1 = speed.
    pub callback_mask: u8,
    /// `prop 0x0F`: frames y status de animación.
    pub animation_frames: u8,
    pub animation_status: u8,
    /// `prop 0x10`: velocidad base de animación.
    pub animation_speed: u8,
    /// `prop 0x11`: triggers que llaman el callback 0x25.
    pub animation_triggers: u8,
    /// `prop 0x12`: flags especiales (`NextFrameRandomBits` bit 0).
    pub animation_special_flags: u8,
    /// `prop 0x14`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
}

/// Tesela de layout aeropuerto (`prop 0x0A`); `local_tile` si gfx era `0xFE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedAirportLayoutTile {
    pub x: i8,
    pub y: i8,
    /// Gfx vanilla/airport-tile global, o id local si [`Self::use_local_tile`].
    pub gfx_or_local: u16,
    pub use_local_tile: bool,
}

/// Una rotación de layout de aeropuerto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAirportLayout {
    /// `Direction` `OpenTTD` (N/E/S/W); bits bajos.
    pub rotation: u8,
    pub tiles: Vec<ParsedAirportLayoutTile>,
}

/// Metadatos `AirportTiles` Action0 (antes de asignar gfx ≥74).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAirportTileMeta {
    pub local_id: u8,
    pub subst_id: u8,
    pub override_of: Option<u8>,
    /// Callback mask (`prop 0x0E`): bit 0 = next frame, bit 1 = speed.
    pub callback_mask: u8,
    /// `prop 0x0F`: último frame de animación permitido.
    pub animation_frames: u8,
    /// `prop 0x0F`: estado (`0` no-loop, `1` loop, `0xFF` sin animación).
    pub animation_status: u8,
    /// `prop 0x10`: espera como potencia de dos de ticks.
    pub animation_speed: u8,
    /// `prop 0x11`: máscara de triggers de `AirportTile`.
    pub animation_triggers: u8,
    /// Flags especiales no expuestos por Action0; se conserva para runtime.
    pub animation_special_flags: u8,
    /// `prop 0x12`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
}

/// Metadatos `Airports` Action0 (antes de asignar id ≥10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAirportMeta {
    pub local_id: u8,
    /// `0xFF` = disable vanilla id; else subst vanilla &lt;10.
    pub subst_id: u8,
    pub disabled: bool,
    pub layouts: Vec<ParsedAirportLayout>,
    pub size_x: u8,
    pub size_y: u8,
    pub min_year: u16,
    pub max_year: u16,
    pub ttd_airport_type: u8,
    pub catchment: u8,
    pub noise_level: u8,
    pub maintenance_cost: u16,
    pub name: String,
    /// `prop 0x12`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
}

/// Tesela cruda de layout industria (`prop 0x0A`); `local_tile` = gfx era `0xFE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedIndustryLayoutTile {
    pub x: i8,
    pub y: i8,
    /// Gfx vanilla, o id local de `IndustryTile` si [`Self::use_local_tile`].
    pub gfx_or_local: u16,
    pub use_local_tile: bool,
}

/// Metadatos `Industries` Action0 (antes de asignar id ≥37).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIndustryMeta {
    pub local_id: u8,
    pub subst_id: u8,
    pub override_id: Option<u8>,
    pub layouts: Vec<Vec<ParsedIndustryLayoutTile>>,
    pub produced_cargo_indices: Vec<u8>,
    pub accepted_cargo_indices: Vec<u8>,
    pub production_rates: Vec<u8>,
    pub input_multipliers: Vec<u16>,
    pub callback_mask: u16,
    /// `prop 0x1A`: bits de `IndustryBehaviour`.
    pub behaviour: u32,
    pub cost_multiplier: u8,
    /// `prop 0x29`: índices locales de la tabla Badge Translation Table.
    pub badge_local_ids: Vec<u16>,
    pub name: String,
}

/// Metadatos `Houses` Action0 (antes de asignar id ≥110).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHouseMeta {
    pub local_id: u8,
    pub subst_id: u8,
    pub building_flags: u8,
    pub min_year: u32,
    pub max_year: u32,
    pub population: u16,
    pub mail_generation: u16,
    pub availability: u16,
    pub probability: u8,
    pub override_id: Option<u8>,
    /// `0x14` lo + `0x1D` hi; almacenado sin ejecutar callbacks.
    pub callback_mask: u16,
    pub name: String,
}

/// Metadatos `RoadStops` Action0 (antes de asignar IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRoadStopMeta {
    pub class_short_label: String,
    pub class_label: String,
    pub short_label: String,
    pub label: String,
    /// `0` bus / `1` truck / `2` all (`RoadStopAvailabilityType`).
    pub stop_type: u8,
    /// Action0 `0x0C` draw modes.
    pub draw_mode: u8,
    /// Action0 `0x0D`: máscara de cargos locales para randomización.
    pub random_cargo_triggers: u32,
    /// Action0 `0x12` flags DWORD.
    pub flags: u32,
    /// Action0 `0x11` (`RoadStopCallbackMask`).
    pub callback_mask: u8,
    /// Action0 `0x0E`: último frame de animación.
    pub animation_frames: u8,
    /// Action0 `0x0E`: estado de animación (`0` no-loop, `1` loop, `0xFF` no animation).
    pub animation_status: u8,
    /// Action0 `0x0F`: espera `2^speed` ticks entre frames.
    pub animation_speed: u8,
    /// Action0 `0x10`: máscara `StationAnimationTrigger`.
    pub animation_triggers: u16,
    /// Etiquetas de badge (`prop 0xFD`); se resuelven en apply.
    pub badge_labels: Vec<String>,
    /// Lista `0xFD` truncada / inválida (diagnóstico observable).
    pub badge_list_error: Option<String>,
}

struct RoadStopMetaParse {
    class_short: String,
    label: String,
    stop_type: u8,
    draw_mode: u8,
    random_cargo_triggers: u32,
    flags: u32,
    callback_mask: u8,
    animation_frames: u8,
    animation_status: u8,
    animation_speed: u8,
    animation_triggers: u16,
    badge_labels: Vec<String>,
    badge_list_error: Option<String>,
}

impl Default for RoadStopMetaParse {
    fn default() -> Self {
        Self {
            class_short: String::from("NGRF"),
            label: String::new(),
            stop_type: 0,
            draw_mode: crate::road_stop_spec::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            badge_labels: Vec::new(),
            badge_list_error: None,
        }
    }
}

impl RoadStopMetaParse {
    fn finish(mut self) -> ParsedRoadStopMeta {
        let short_label: String = self
            .label
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(4)
            .collect();
        let short_label = if short_label.is_empty() {
            String::from("Stop")
        } else {
            short_label
        };
        if self.label.is_empty() {
            self.label.clone_from(&short_label);
        }
        ParsedRoadStopMeta {
            class_label: self.class_short.clone(),
            class_short_label: self.class_short,
            short_label,
            label: self.label,
            stop_type: self.stop_type,
            draw_mode: self.draw_mode,
            random_cargo_triggers: self.random_cargo_triggers,
            flags: self.flags,
            callback_mask: self.callback_mask,
            animation_frames: self.animation_frames,
            animation_status: self.animation_status,
            animation_speed: self.animation_speed,
            animation_triggers: self.animation_triggers,
            badge_labels: self.badge_labels,
            badge_list_error: self.badge_list_error,
        }
    }
}

/// Metadatos `Badges` Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBadgeMeta {
    pub label: String,
    pub flags: u32,
}

/// Resultado de leer la lista de asociaciones `0xFD`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BadgeAssocParse {
    pub labels: Vec<String>,
    /// Mensaje si la lista está truncada o contiene labels inválidos.
    pub error: Option<String>,
}

/// Metadatos `Cargoes` Action0 (antes de registrar en catálogo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCargoMeta {
    pub local_id: u8,
    pub bitnum: u8,
    pub label: String,
    pub name: String,
    pub weight: u8,
    pub initial_payment: u32,
    pub transit_fast: u8,
    pub transit_slow: u8,
    pub is_freight: bool,
    pub classes: u16,
    pub capacity_multiplier: u16,
    pub rating_colour: u8,
    pub legend_colour: u8,
    pub callback_mask: u8,
}

/// Metadatos `Sounds` Action0 (`0x0C`; props sobre samples Action11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSoundMeta {
    pub local_id: u8,
    pub volume: u8,
    pub priority: u8,
    pub override_old: Option<u8>,
}

/// Metadatos `Canals` Action0 (`0x05`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCanalMeta {
    pub local_id: u8,
    pub callback_mask: u8,
    pub flags: u8,
}

/// Metadatos `Bridges` Action0 (`0x06`; override in-place de slots 0..12).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ParsedBridgeMeta {
    pub local_id: u8,
    pub available_from_year: u32,
    pub min_middle_len: u16,
    pub max_middle_len: Option<u16>,
    pub price_mult: u16,
    pub max_speed: u16,
    pub name: Option<String>,
    pub has_custom_sprites: bool,
    /// Props runtime vistas (si no, el apply conserva el slot vanilla).
    pub year_set: bool,
    pub min_len_set: bool,
    pub max_len_set: bool,
    pub price_set: bool,
    pub speed_set: bool,
}

/// Metadatos `Objects` Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectMeta {
    pub local_id: u8,
    pub class_label: String,
    pub name: String,
    pub size: u8,
    /// Máscara de climas (`prop 0x0B`).
    pub climate_mask: u8,
    /// Multiplicador de coste de construcción (`prop 0x0D`).
    pub build_cost_factor: u8,
    /// Máscara de callbacks (`prop 0x15`, WORD).
    pub callback_mask: u16,
    /// Etiquetas de badge (`prop 0xFD`); se resuelven en apply.
    pub badge_labels: Vec<String>,
    /// Lista `0xFD` truncada / inválida (diagnóstico observable).
    pub badge_list_error: Option<String>,
}

#[must_use]
pub fn parse_action0_header(payload: &[u8]) -> Option<Action0Header> {
    if payload.len() < 4 || payload[0] != 0x00 {
        return None;
    }
    Some(Action0Header {
        feature: payload[1],
        num_props: payload[2],
        num_ids: payload[3],
    })
}

pub fn for_each_pseudo_payload(
    data: &[u8],
    mut visit: impl FnMut(&[u8]),
) -> Result<(), GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    for_each_pseudo_sprite(section, container, |payload| visit(payload));
    Ok(())
}

fn read_label_list(payload: &[u8], i: &mut usize) -> Option<Vec<[u8; 4]>> {
    if *i >= payload.len() {
        return None;
    }
    let n = usize::from(payload[*i]);
    *i += 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if *i + 4 > payload.len() {
            return None;
        }
        out.push([
            payload[*i],
            payload[*i + 1],
            payload[*i + 2],
            payload[*i + 3],
        ]);
        *i += 4;
    }
    Some(out)
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_roadtype_meta(payload: &[u8]) -> Option<ParsedRoadTypeMeta> {
    let header = parse_action0_header(payload)?;
    let feature_tram = header.feature == ACTION0_FEATURE_TRAMTYPES;
    if !(header.feature == ACTION0_FEATURE_ROADTYPES || feature_tram) || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut short_label = String::from("NGRF");
    let mut label = String::new();
    let mut intro_year = 0u16;
    let mut max_speed = 0u16;
    let mut cost_multiplier = 0u16;
    let mut maintenance_multiplier = 0u16;
    let mut flags = 0u8;
    let mut powered_labels = Vec::new();
    let mut badge_local_ids = Vec::new();
    // Extensión local en RoadTypes: bit0 de `0x09` = tram. En OTTD `0x09` es string WORD.
    let mut is_tram = feature_tram;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                if i + 4 > payload.len() {
                    break;
                }
                short_label = String::from_utf8_lossy(&payload[i..i + 4])
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if short_label.is_empty() {
                    short_label = "NGRF".into();
                }
                i += 4;
            }
            PROP_FLAGS if !feature_tram => {
                // Local: BYTE flags (bit0 tram). OTTD `0x09` es WORD string.
                if i >= payload.len() {
                    break;
                }
                flags = payload[i];
                is_tram = flags & 0x01 != 0;
                i += 1;
            }
            0x0F | 0x18 | 0x19 => {
                let Some(list) = read_label_list(payload, &mut i) else {
                    break;
                };
                if prop == 0x0F {
                    powered_labels = list;
                }
            }
            0x10 => {
                if i >= payload.len() {
                    break;
                }
                flags = payload[i];
                i += 1;
            }
            0x13 | 0x1C => {
                if i + 2 > payload.len() {
                    break;
                }
                let v = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
                if prop == 0x13 {
                    cost_multiplier = v;
                } else {
                    maintenance_multiplier = v;
                }
            }
            0x14 => {
                if i + 2 > payload.len() {
                    break;
                }
                max_speed = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            PROP_INTRO_YEAR => {
                // Extensión local WORD año; OTTD `0x16` es map colour BYTE.
                if i + 2 > payload.len() {
                    break;
                }
                intro_year = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            0x17 => {
                // Introduction date DWORD (OTTD); derivar año aproximado si local year=0.
                if i + 4 > payload.len() {
                    break;
                }
                let days = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
                if intro_year == 0 && days > 0 {
                    intro_year = 1920u16.saturating_add(u16::try_from(days / 365).unwrap_or(0));
                }
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                label = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            // Strings OTTD 0x09–0x0D / 0x1B: WORD.
            0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x1B if feature_tram => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            0x0A | 0x0B | 0x0C | 0x0D | 0x1B if !feature_tram => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            0x1A | 0x1D => {
                // sort BYTE / alternate label list
                if prop == 0x1A {
                    if i >= payload.len() {
                        break;
                    }
                    i += 1;
                } else if read_label_list(payload, &mut i).is_none() {
                    break;
                }
            }
            0x1E => {
                badge_local_ids = read_badge_local_ids(payload, &mut i)?;
            }
            _ => break,
        }
    }
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    Some(ParsedRoadTypeMeta {
        class: if is_tram {
            RoadTramType::Tram
        } else {
            RoadTramType::Road
        },
        label,
        short_label,
        intro_year,
        max_speed,
        cost_multiplier,
        maintenance_multiplier,
        flags,
        powered_labels,
        badge_local_ids,
        from_tramtypes_feature: feature_tram,
    })
}

#[must_use]
pub fn collect_roadtype_metas_from_grf(data: &[u8]) -> Vec<ParsedRoadTypeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_roadtype_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Lee etiquetas `RailType` de un Action0. La propiedad `0x08` contiene un DWORD
/// por id y suele ser la primera del bloque, como en la fase Reserve de `OpenTTD`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_railtype_metas(payload: &[u8]) -> Option<Vec<ParsedRailTypeMeta>> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_RAILTYPES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let first_id = payload[4];
    let n = usize::from(header.num_ids);
    let mut i = 5usize;
    let mut labels: Option<Vec<crate::newgrf_type_tables::TypeLabel>> = None;
    let mut max_speeds = vec![0u16; n];
    let mut cost_multipliers = vec![0u16; n];
    let mut maintenance_multipliers = vec![0u16; n];
    let mut flags = vec![0u8; n];
    let mut curve_speeds = vec![0u8; n];
    let mut introduction_dates = vec![0u32; n];
    let mut compatible_labels = vec![Vec::<[u8; 4]>::new(); n];
    let mut powered_labels = vec![Vec::<[u8; 4]>::new(); n];
    let mut badge_local_ids = vec![Vec::<u16>::new(); n];
    for _ in 0..header.num_props {
        let prop = *payload.get(i)?;
        i += 1;
        match prop {
            PROP_LABEL => {
                let bytes = n.checked_mul(4)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                let mut out = Vec::with_capacity(n);
                for offset in 0..n {
                    let start = i + offset * 4;
                    out.push(payload[start..start + 4].try_into().ok()?);
                }
                i += bytes;
                labels = Some(out);
            }
            0x0E | 0x0F | 0x18 | 0x19 => {
                for offset in 0..n {
                    let list = read_label_list(payload, &mut i)?;
                    match prop {
                        0x0E => compatible_labels[offset] = list,
                        0x0F => powered_labels[offset] = list,
                        _ => {}
                    }
                }
            }
            0x1E => {
                for badges in &mut badge_local_ids {
                    *badges = read_badge_local_ids(payload, &mut i)?;
                }
            }
            0x14 => {
                let bytes = n.checked_mul(2)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                for (offset, speed) in max_speeds.iter_mut().enumerate() {
                    let start = i + offset * 2;
                    *speed = u16::from_le_bytes([payload[start], payload[start + 1]]);
                }
                i += bytes;
            }
            0x13 | 0x1C => {
                let bytes = n.checked_mul(2)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                let target = if prop == 0x13 {
                    &mut cost_multipliers
                } else {
                    &mut maintenance_multipliers
                };
                for (offset, slot) in target.iter_mut().enumerate() {
                    let start = i + offset * 2;
                    *slot = u16::from_le_bytes([payload[start], payload[start + 1]]);
                }
                i += bytes;
            }
            0x09..=0x0D | 0x1B => {
                i = i.checked_add(2usize.checked_mul(n)?)?;
            }
            0x10 | 0x11 | 0x12 | 0x15 | 0x16 | 0x1A => {
                if i.checked_add(n)? > payload.len() {
                    return None;
                }
                for offset in 0..n {
                    match prop {
                        0x10 => flags[offset] = payload[i + offset],
                        0x11 => curve_speeds[offset] = payload[i + offset],
                        _ => {}
                    }
                }
                i += n;
            }
            0x17 => {
                let bytes = n.checked_mul(4)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                for (offset, slot) in introduction_dates.iter_mut().enumerate() {
                    let start = i + offset * 4;
                    *slot = u32::from_le_bytes([
                        payload[start],
                        payload[start + 1],
                        payload[start + 2],
                        payload[start + 3],
                    ]);
                }
                i += bytes;
            }
            0x1D => {
                // alternate rail type label list
                for _ in 0..n {
                    read_label_list(payload, &mut i)?;
                }
            }
            _ => break,
        }
        if i > payload.len() {
            return None;
        }
    }
    labels.map(|labs| {
        labs.into_iter()
            .enumerate()
            .map(|(offset, label)| ParsedRailTypeMeta {
                local_id: first_id.wrapping_add(u8::try_from(offset).unwrap_or(0)),
                label,
                max_speed: max_speeds.get(offset).copied().unwrap_or(0),
                cost_multiplier: cost_multipliers.get(offset).copied().unwrap_or(0),
                maintenance_multiplier: maintenance_multipliers.get(offset).copied().unwrap_or(0),
                flags: flags.get(offset).copied().unwrap_or(0),
                curve_speed: curve_speeds.get(offset).copied().unwrap_or(0),
                introduction_date: introduction_dates.get(offset).copied().unwrap_or(0),
                compatible_labels: compatible_labels.get(offset).cloned().unwrap_or_default(),
                powered_labels: powered_labels.get(offset).cloned().unwrap_or_default(),
                badge_local_ids: badge_local_ids.get(offset).cloned().unwrap_or_default(),
            })
            .collect()
    })
}

#[must_use]
pub fn collect_railtype_metas_from_grf(data: &[u8]) -> Vec<ParsedRailTypeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(metas) = parse_action0_railtype_metas(payload) {
            out.extend(metas);
        }
    });
    out
}

fn read_four_char_label(payload: &[u8], i: &mut usize, fallback: &str) -> Option<String> {
    if *i + 4 > payload.len() {
        return None;
    }
    let mut s = String::from_utf8_lossy(&payload[*i..*i + 4])
        .trim_end_matches('\0')
        .trim()
        .to_string();
    *i += 4;
    if s.is_empty() {
        s = fallback.into();
    }
    Some(s)
}

/// Lee `prop 0xFD`: BYTE count + N× label 4 chars.
///
/// Devuelve `None` sólo si falta el BYTE count. Truncado / label vacío → `error`.
fn read_badge_association_labels(payload: &[u8], i: &mut usize) -> Option<BadgeAssocParse> {
    if *i >= payload.len() {
        return None;
    }
    let count = usize::from(payload[*i]);
    *i += 1;
    let mut labels = Vec::with_capacity(count);
    let mut error = None;
    for n in 0..count {
        if *i + 4 > payload.len() {
            error = Some(format!(
                "lista de badges 0xFD truncada (pedía {count}, leídos {n})"
            ));
            break;
        }
        let Some(s) = read_four_char_label(payload, i, "") else {
            error = Some(format!(
                "lista de badges 0xFD truncada (pedía {count}, leídos {n})"
            ));
            break;
        };
        if s.is_empty() {
            error = Some("lista de badges 0xFD con label vacío/inválido".into());
            continue;
        }
        labels.push(s);
    }
    Some(BadgeAssocParse { labels, error })
}

/// Lee una lista estándar `ReadBadgeList`: WORD count + N×WORD de índices
/// locales de la tabla `GlobalVar` `Badge` del GRF.
fn read_badge_local_ids(payload: &[u8], i: &mut usize) -> Option<Vec<u16>> {
    let count = usize::from(read_u16(payload, i)?);
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(read_u16(payload, i)?);
    }
    Some(ids)
}

fn parse_station_custom_layouts(
    payload: &[u8],
    i: &mut usize,
) -> std::collections::HashMap<(u8, u8), Vec<u8>> {
    let mut custom_layouts = std::collections::HashMap::new();
    loop {
        if *i + 2 > payload.len() {
            break;
        }
        let length = payload[*i];
        let number = payload[*i + 1];
        *i += 2;
        if length == 0 || number == 0 {
            break;
        }
        let n = usize::from(length).saturating_mul(usize::from(number));
        if *i + n > payload.len() {
            break;
        }
        let mut tiles = payload[*i..*i + n].to_vec();
        *i += n;
        for t in &mut tiles {
            *t &= !1u8;
        }
        custom_layouts.insert((number, length), tiles);
    }
    custom_layouts
}

fn parse_station_copy_layout_id(payload: &[u8], i: &mut usize) -> Option<u16> {
    if *i >= payload.len() {
        return None;
    }
    let b = payload[*i];
    *i += 1;
    if b == 0xFF {
        if *i + 1 > payload.len() {
            return None;
        }
        let id = u16::from_le_bytes([payload[*i], payload[*i + 1]]);
        *i += 2;
        Some(id)
    } else {
        Some(u16::from(b))
    }
}

/// Lee el formato `extended byte` de Action0 (`BYTE`, o `0xFF` + WORD LE).
fn read_station_extended_byte(payload: &[u8], i: &mut usize) -> Option<usize> {
    let byte = *payload.get(*i)?;
    *i += 1;
    if byte != 0xFF {
        return Some(usize::from(byte));
    }
    let bytes = payload.get(*i..*i + 2)?;
    *i += 2;
    Some(usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
}

/// Salta el layout clásico de estación Action0 `0x09`.
///
/// El renderer actual no usa esta representación (consume Action1/2/3), pero
/// no debe impedir leer las props posteriores —en especial `0x13` y
/// `0x16`–`0x18` de animación. La forma antigua consiste en `n` layouts,
/// cada uno con un ground sprite de cuatro bytes y cero o más building
/// sprites de 10 bytes terminados por `delta_x = 0x80`; `0,0,0,0` es el
/// atajo vanilla sin secuencia.
fn skip_station_legacy_sprite_layouts(payload: &[u8], i: &mut usize) -> bool {
    let Some(layouts) = read_station_extended_byte(payload, i) else {
        return false;
    };
    for _ in 0..layouts {
        let Some(ground) = payload.get(*i..*i + 4) else {
            return false;
        };
        *i += 4;
        if ground == [0, 0, 0, 0] {
            continue;
        }
        loop {
            let Some(&delta_x) = payload.get(*i) else {
                return false;
            };
            *i += 1;
            if delta_x == 0x80 {
                break;
            }
            let Some(next) = (*i).checked_add(9) else {
                return false;
            };
            if next > payload.len() {
                return false;
            }
            *i = next;
        }
    }
    true
}

/// Salta una propiedad de ancho fijo cuyo runtime aún no está representado.
fn skip_station_fixed_property(payload: &[u8], i: &mut usize, width: usize) -> bool {
    let Some(next) = (*i).checked_add(width) else {
        return false;
    };
    if next > payload.len() {
        return false;
    }
    *i = next;
    true
}

#[must_use]
#[allow(clippy::too_many_lines)] // El wire format Action0 varía por propiedad.
pub fn parse_action0_station_meta(payload: &[u8]) -> Option<ParsedStationMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_STATIONS || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut class_short = String::from("NGRF");
    let mut label = String::new();
    let mut disallowed_platforms = 0u8;
    let mut disallowed_lengths = 0u8;
    let mut callback_mask = 0u8;
    let mut flags = 0u8;
    let mut animation_status = 0xFFu8;
    let mut animation_frames = 0u8;
    let mut animation_speed = 2u8;
    let mut animation_triggers = 0u16;
    let mut custom_layouts = std::collections::HashMap::new();
    let mut copy_layout_from = None;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "NGRF") else {
                    break;
                };
                class_short = s;
            }
            // 0x09 layout clásico. Se consume para alcanzar las propiedades
            // posteriores; sprites runtime siguen viniendo de Action1/2/3.
            0x09 => {
                if !skip_station_legacy_sprite_layouts(payload, &mut i) {
                    break;
                }
            }
            // 0x0A copy sprite layout: extended-byte id (consumida; gfx vía Action1/3).
            0x0A => {
                if parse_station_copy_layout_id(payload, &mut i).is_none() {
                    break;
                }
            }
            PROP_STATION_CALLBACK_MASK => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = payload[i];
                i += 1;
            }
            PROP_STATION_DISALLOWED_PLATFORMS => {
                if i >= payload.len() {
                    break;
                }
                disallowed_platforms = payload[i];
                i += 1;
            }
            PROP_STATION_DISALLOWED_LENGTHS => {
                if i >= payload.len() {
                    break;
                }
                disallowed_lengths = payload[i];
                i += 1;
            }
            PROP_STATION_CUSTOM_LAYOUT => {
                custom_layouts = parse_station_custom_layouts(payload, &mut i);
            }
            PROP_STATION_COPY_LAYOUT => {
                let Some(id) = parse_station_copy_layout_id(payload, &mut i) else {
                    break;
                };
                copy_layout_from = Some(id);
            }
            // Props intermedias de ancho fijo que no cambian aún el modelo
            // Rust, pero suelen preceder a la configuración de animación.
            0x10 => {
                if !skip_station_fixed_property(payload, &mut i, 2) {
                    break;
                }
            }
            0x11 | 0x14 | 0x15 => {
                if !skip_station_fixed_property(payload, &mut i, 1) {
                    break;
                }
            }
            0x12 => {
                if !skip_station_fixed_property(payload, &mut i, 4) {
                    break;
                }
            }
            // 0x13: flags generales; bit 2 indica random bits para CB141.
            0x13 => {
                if i >= payload.len() {
                    break;
                }
                flags = payload[i];
                i += 1;
            }
            // 0x16: último frame y estado (`0` no-loop, `1` loop, `0xFF` sin animación).
            0x16 => {
                let Some(bytes) = payload.get(i..i + 2) else {
                    break;
                };
                animation_frames = bytes[0];
                animation_status = bytes[1];
                i += 2;
            }
            // 0x17: espera como potencia de dos de ticks.
            0x17 => {
                if i >= payload.len() {
                    break;
                }
                animation_speed = payload[i];
                i += 1;
            }
            // 0x18: máscara `StationAnimationTrigger` little-endian.
            0x18 => {
                let Some(bytes) = payload.get(i..i + 2) else {
                    break;
                };
                animation_triggers = u16::from_le_bytes([bytes[0], bytes[1]]);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                label = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            // 0x09 sprite layout es variable; sin consumidor aún → cortar el bloque.
            _ => break,
        }
    }
    Some(finish_parsed_station_meta(
        class_short,
        label,
        disallowed_platforms,
        disallowed_lengths,
        callback_mask,
        flags,
        animation_status,
        animation_frames,
        animation_speed,
        animation_triggers,
        custom_layouts,
        copy_layout_from,
    ))
}

#[allow(clippy::too_many_arguments)] // Agrupa exactamente los campos Action0 ya validados.
fn finish_parsed_station_meta(
    class_short: String,
    mut label: String,
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    callback_mask: u8,
    flags: u8,
    animation_status: u8,
    animation_frames: u8,
    animation_speed: u8,
    animation_triggers: u16,
    custom_layouts: std::collections::HashMap<(u8, u8), Vec<u8>>,
    copy_layout_from: Option<u16>,
) -> ParsedStationMeta {
    let short_label = {
        let ascii: String = label
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(4)
            .collect();
        if ascii.is_empty() {
            String::from("Stat")
        } else {
            ascii
        }
    };
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    let class_label = if class_short.eq_ignore_ascii_case("DFLT") {
        "Por defecto".into()
    } else {
        class_short.clone()
    };
    ParsedStationMeta {
        class_short_label: class_short,
        class_label,
        short_label,
        label,
        disallowed_platforms,
        disallowed_lengths,
        callback_mask,
        flags,
        animation_status,
        animation_frames,
        animation_speed,
        animation_triggers,
        custom_layouts,
        copy_layout_from,
    }
}

#[must_use]
pub fn collect_station_metas_from_grf(data: &[u8]) -> Vec<ParsedStationMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_station_meta(payload) {
            out.push(meta);
        }
    });
    out
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_industry_tile_meta(payload: &[u8]) -> Option<ParsedIndustryTileMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_INDUSTRYTILES || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut override_of: Option<u8> = None;
    let mut accepts = [0xFFu8; 3];
    let mut acceptance = [0i8; 3];
    let mut slopes_refused = 0u8;
    let mut callback_mask = 0u8;
    let mut animation_frames = 0u8;
    let mut animation_status = 0u8;
    let mut animation_speed = 0u8;
    let mut animation_triggers = 0u8;
    let mut animation_special_flags = 0u8;
    let mut badge_local_ids = Vec::new();
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_INDTILE_SUBST => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if u16::from(s) < crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET {
                    subst_id = Some(s);
                }
            }
            PROP_INDTILE_OVERRIDE => {
                if i >= payload.len() {
                    break;
                }
                let o = payload[i];
                i += 1;
                if u16::from(o) < crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET {
                    override_of = Some(o);
                }
            }
            PROP_INDTILE_ACCEPT_0 | PROP_INDTILE_ACCEPT_1 | PROP_INDTILE_ACCEPT_2 => {
                if i + 2 > payload.len() {
                    break;
                }
                let acctp = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
                let slot = usize::from(prop - PROP_INDTILE_ACCEPT_0);
                accepts[slot] = (acctp & 0xFF) as u8;
                acceptance[slot] = ((acctp >> 8) as u8).min(16).cast_signed();
            }
            PROP_INDTILE_CALLBACK_MASK => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = payload[i];
                i += 1;
            }
            0x0D => {
                if i >= payload.len() {
                    break;
                }
                slopes_refused = payload[i];
                i += 1;
            }
            0x0F => {
                if i + 2 > payload.len() {
                    break;
                }
                animation_frames = payload[i];
                animation_status = payload[i + 1];
                i += 2;
            }
            0x10 => {
                if i >= payload.len() {
                    break;
                }
                animation_speed = payload[i];
                i += 1;
            }
            0x11 => {
                if i >= payload.len() {
                    break;
                }
                animation_triggers = payload[i];
                i += 1;
            }
            0x12 => {
                if i >= payload.len() {
                    break;
                }
                animation_special_flags = payload[i];
                i += 1;
            }
            PROP_INDTILE_ACCEPT_LIST => {
                if i >= payload.len() {
                    break;
                }
                let num = usize::from(payload[i]);
                i += 1;
                accepts = [0xFF; 3];
                acceptance = [0; 3];
                for slot in 0..3 {
                    if slot < num {
                        if i + 2 > payload.len() {
                            break;
                        }
                        accepts[slot] = payload[i];
                        acceptance[slot] = payload[i + 1].cast_signed();
                        i += 2;
                    }
                }
                // Consumir extras si num > 3 (OpenTTD deshabilita GRF; nosotros avanzamos).
                if num > 3 {
                    let extra = (num - 3).saturating_mul(2);
                    if i + extra > payload.len() {
                        break;
                    }
                    i += extra;
                }
            }
            PROP_INDTILE_BADGES => {
                // ReadBadgeList: WORD count + n×WORD (estilo houses `0x24`).
                if i + 2 > payload.len() {
                    break;
                }
                let count = usize::from(u16::from_le_bytes([payload[i], payload[i + 1]]));
                i += 2;
                let need = count.saturating_mul(2);
                if i + need > payload.len() {
                    break;
                }
                for _ in 0..count {
                    badge_local_ids.push(u16::from_le_bytes([payload[i], payload[i + 1]]));
                    i += 2;
                }
            }
            _ => break,
        }
    }
    let mut accepts_cargo_indices = Vec::new();
    let mut acceptance_out = Vec::new();
    for slot in 0..3 {
        if accepts[slot] != 0xFF {
            accepts_cargo_indices.push(accepts[slot]);
            acceptance_out.push(acceptance[slot]);
        }
    }
    Some(ParsedIndustryTileMeta {
        local_id,
        subst_id: subst_id?,
        override_of,
        accepts_cargo_indices,
        acceptance: acceptance_out,
        slopes_refused,
        callback_mask,
        animation_frames,
        animation_status,
        animation_speed,
        animation_triggers,
        animation_special_flags,
        badge_local_ids,
    })
}

#[must_use]
pub fn collect_industry_tile_metas_from_grf(data: &[u8]) -> Vec<ParsedIndustryTileMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_industry_tile_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Industries` (`0x0A`). Requiere `prop 0x08` (subst) para definir el slot.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_industry_meta(payload: &[u8]) -> Option<ParsedIndustryMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_INDUSTRIES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut override_id: Option<u8> = None;
    let mut layouts: Vec<Vec<ParsedIndustryLayoutTile>> = Vec::new();
    let mut produced_cargo_indices = Vec::new();
    let mut accepted_cargo_indices = Vec::new();
    let mut production_rates = vec![0u8; crate::industry_spec::INDUSTRY_ORIGINAL_NUM_OUTPUTS];
    let mut input_multipliers = Vec::new();
    let mut callback_mask = 0u16;
    let mut behaviour = 0u32;
    let mut cost_multiplier = 0u8;
    let mut badge_local_ids = Vec::new();
    let mut name = String::new();

    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x08 => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if s == 0xFF {
                    // Desactivar vanilla: no crea slot NewGRF.
                    return None;
                }
                if u16::from(s) < crate::industry_spec::NEW_INDUSTRY_OFFSET {
                    subst_id = Some(s);
                }
            }
            0x09 => {
                if i >= payload.len() {
                    break;
                }
                let o = payload[i];
                i += 1;
                if u16::from(o) < crate::industry_spec::NEW_INDUSTRY_OFFSET {
                    override_id = Some(o);
                }
            }
            0x0A => {
                let Some(parsed) = parse_industry_layouts(payload, &mut i) else {
                    break;
                };
                layouts = parsed;
            }
            0x0B | 0x0F | 0x14 | 0x17 | 0x18 | 0x19 => {
                if i >= payload.len() {
                    break;
                }
                if prop == 0x0F {
                    cost_multiplier = payload[i];
                }
                i += 1;
            }
            0x21 | 0x22 => {
                if i >= payload.len() {
                    break;
                }
                let shift = u16::from(prop - 0x21) * 8;
                callback_mask =
                    (callback_mask & !(0xFFu16 << shift)) | (u16::from(payload[i]) << shift);
                i += 1;
            }
            0x12 | 0x13 => {
                if i >= payload.len() {
                    break;
                }
                let idx = usize::from(prop - 0x12);
                if idx < production_rates.len() {
                    production_rates[idx] = payload[i];
                }
                i += 1;
            }
            0x0C | 0x0D | 0x0E | 0x1B | 0x1F | 0x24 => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            0x10 => {
                // INDUSTRY_ORIGINAL_NUM_OUTPUTS bytes.
                let n = crate::industry_spec::INDUSTRY_ORIGINAL_NUM_OUTPUTS;
                if i + n > payload.len() {
                    break;
                }
                produced_cargo_indices = payload[i..i + n].to_vec();
                i += n;
            }
            0x11 => {
                // INDUSTRY_ORIGINAL_NUM_INPUTS bytes + 1 unused.
                let n = crate::industry_spec::INDUSTRY_ORIGINAL_NUM_INPUTS;
                if i + n + 1 > payload.len() {
                    break;
                }
                accepted_cargo_indices = payload[i..i + n].to_vec();
                i += n + 1;
            }
            0x1A => {
                if i + 4 > payload.len() {
                    break;
                }
                behaviour = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
            }
            0x1C | 0x1D | 0x1E | 0x20 | 0x23 => {
                if i + 4 > payload.len() {
                    break;
                }
                if matches!(prop, 0x1C..=0x1E) {
                    let multiples = u32::from_le_bytes([
                        payload[i],
                        payload[i + 1],
                        payload[i + 2],
                        payload[i + 3],
                    ]);
                    input_multipliers.push((multiples & 0xFFFF) as u16);
                    input_multipliers.push((multiples >> 16) as u16);
                }
                i += 4;
            }
            0x15 | 0x25 | 0x26 | 0x27 => {
                if i >= payload.len() {
                    break;
                }
                let num = usize::from(payload[i]);
                i += 1;
                if i + num > payload.len() {
                    break;
                }
                match prop {
                    0x25 => {
                        produced_cargo_indices = payload[i..i + num].to_vec();
                    }
                    0x26 => {
                        accepted_cargo_indices = payload[i..i + num].to_vec();
                    }
                    0x27 => {
                        production_rates = payload[i..i + num].to_vec();
                    }
                    _ => {}
                }
                i += num;
            }
            0x16 => {
                // 3 conflicting industry types.
                if i + 3 > payload.len() {
                    break;
                }
                i += 3;
            }
            0x28 => {
                if i + 2 > payload.len() {
                    break;
                }
                let num_in = usize::from(payload[i]);
                let num_out = usize::from(payload[i + 1]);
                i += 2;
                let need = num_in.saturating_mul(num_out).saturating_mul(2);
                if i + need > payload.len() {
                    break;
                }
                input_multipliers.clear();
                for _ in 0..num_in * num_out {
                    input_multipliers.push(u16::from_le_bytes([payload[i], payload[i + 1]]));
                    i += 2;
                }
            }
            0x29 => {
                // Badge list: WORD count + n×WORD.
                if i + 2 > payload.len() {
                    break;
                }
                let count = usize::from(u16::from_le_bytes([payload[i], payload[i + 1]]));
                i += 2;
                let need = count.saturating_mul(2);
                if i + need > payload.len() {
                    break;
                }
                badge_local_ids = (0..count)
                    .map(|offset| {
                        let start = i + offset * 2;
                        u16::from_le_bytes([payload[start], payload[start + 1]])
                    })
                    .collect();
                i += need;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }

    let subst_id = subst_id?;
    if name.is_empty() {
        name = format!("Industry {local_id}");
    }
    // Conservar los slots inválidos. OpenTTD necesita que los huecos de las
    // listas legacy 3-in/2-out mantengan su posición para que los callbacks
    // dinámicos puedan omitir un slot sin compactar los multiplicadores.
    Some(ParsedIndustryMeta {
        local_id,
        subst_id,
        override_id,
        layouts,
        produced_cargo_indices,
        accepted_cargo_indices,
        production_rates,
        input_multipliers,
        callback_mask,
        behaviour,
        cost_multiplier,
        badge_local_ids,
        name,
    })
}

fn parse_industry_layouts(
    payload: &[u8],
    i: &mut usize,
) -> Option<Vec<Vec<ParsedIndustryLayoutTile>>> {
    if *i >= payload.len() {
        return None;
    }
    let num_layouts = usize::from(payload[*i]);
    *i += 1;
    if *i + 4 > payload.len() {
        return None;
    }
    let definition_size =
        usize::try_from(u32::from_le_bytes(payload[*i..*i + 4].try_into().ok()?)).ok()?;
    *i += 4;
    let layout_start = *i;
    let mut layouts = Vec::with_capacity(num_layouts);
    for _ in 0..num_layouts {
        let mut layout = Vec::new();
        let mut k = 0usize;
        loop {
            if *i >= payload.len() || (*i).saturating_sub(layout_start) >= definition_size {
                break;
            }
            let x = payload[*i];
            *i += 1;
            if x == 0xFE && k == 0 {
                // Borrow vanilla layout: type + laynbr (consumidos; sin expandir).
                if *i + 2 > payload.len() {
                    return Some(layouts);
                }
                *i += 2;
                break;
            }
            if *i >= payload.len() {
                return Some(layouts);
            }
            let y = payload[*i];
            *i += 1;
            if x == 0 && y == 0x80 {
                break;
            }
            if *i >= payload.len() {
                return Some(layouts);
            }
            let gfx = payload[*i];
            *i += 1;
            let (gfx_or_local, use_local_tile) = if gfx == 0xFE {
                if *i + 2 > payload.len() {
                    return Some(layouts);
                }
                let local = u16::from_le_bytes([payload[*i], payload[*i + 1]]);
                *i += 2;
                (local, true)
            } else {
                (u16::from(gfx), false)
            };
            layout.push(ParsedIndustryLayoutTile {
                x: x.cast_signed(),
                y: y.cast_signed(),
                gfx_or_local,
                use_local_tile,
            });
            k += 1;
        }
        if !layout.is_empty() {
            layouts.push(layout);
        }
    }
    // Asegurar consumo exacto del bloque definition_size si quedó padding.
    let consumed = (*i).saturating_sub(layout_start);
    if consumed < definition_size {
        let pad = definition_size - consumed;
        if *i + pad <= payload.len() {
            *i += pad;
        }
    }
    Some(layouts)
}

#[must_use]
pub fn collect_industry_metas_from_grf(data: &[u8]) -> Vec<ParsedIndustryMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_industry_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Houses` (`0x07`). Requiere `prop 0x08` (subst) para definir la casa.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_house_meta(payload: &[u8]) -> Option<ParsedHouseMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_HOUSES || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut building_flags = crate::house_spec::BUILDING_FLAG_SIZE_1X1;
    let mut min_year = 0u32;
    let mut max_year = crate::house_spec::HOUSE_YEAR_MAX;
    let mut population = 0u16;
    let mut mail_generation = 0u16;
    let mut availability = crate::house_spec::DEFAULT_HOUSE_AVAILABILITY;
    let mut probability = crate::house_spec::DEFAULT_HOUSE_PROBABILITY;
    let mut override_id: Option<u8> = None;
    let mut callback_mask = 0u16;
    let mut name = String::new();

    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x08 => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if s == 0xFF {
                    // Desactivar vanilla: no crea slot NewGRF.
                    return None;
                }
                if u16::from(s) < crate::house_spec::NEW_HOUSE_OFFSET {
                    subst_id = Some(s);
                }
            }
            0x09 => {
                if i >= payload.len() {
                    break;
                }
                building_flags = payload[i];
                i += 1;
            }
            0x0A => {
                if i + 2 > payload.len() {
                    break;
                }
                let years = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
                let lo = (years & 0xFF) as u8;
                let hi = (years >> 8) as u8;
                min_year = if lo > 150 {
                    crate::house_spec::HOUSE_YEAR_MAX
                } else {
                    1920u32.saturating_add(u32::from(lo))
                };
                max_year = if hi > 150 {
                    crate::house_spec::HOUSE_YEAR_MAX
                } else {
                    1920u32.saturating_add(u32::from(hi))
                };
            }
            0x0B => {
                if i >= payload.len() {
                    break;
                }
                population = u16::from(payload[i]);
                i += 1;
            }
            0x0C => {
                if i >= payload.len() {
                    break;
                }
                mail_generation = u16::from(payload[i]);
                i += 1;
            }
            0x0D | 0x0E | 0x0F | 0x11 | 0x16 | 0x18 | 0x19 | 0x1A | 0x1B | 0x1C | 0x1F => {
                if i >= payload.len() {
                    break;
                }
                if prop == 0x18 {
                    probability = payload[i];
                }
                i += 1;
            }
            0x14 => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = (callback_mask & 0xFF00) | u16::from(payload[i]);
                i += 1;
            }
            0x15 => {
                if i >= payload.len() {
                    break;
                }
                let o = payload[i];
                i += 1;
                if u16::from(o) < crate::house_spec::NEW_HOUSE_OFFSET {
                    override_id = Some(o);
                }
            }
            0x1D => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = (callback_mask & 0x00FF) | (u16::from(payload[i]) << 8);
                i += 1;
            }
            0x10 | 0x12 | 0x13 | 0x21 | 0x22 => {
                if i + 2 > payload.len() {
                    break;
                }
                let w = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
                match prop {
                    0x13 => availability = w,
                    0x21 => min_year = u32::from(w),
                    0x22 => {
                        max_year = if w == u16::MAX {
                            crate::house_spec::HOUSE_YEAR_MAX
                        } else {
                            u32::from(w)
                        };
                    }
                    _ => {}
                }
            }
            0x17 | 0x1E => {
                if i + 4 > payload.len() {
                    break;
                }
                i += 4;
            }
            0x20 => {
                if i >= payload.len() {
                    break;
                }
                let count = usize::from(payload[i]);
                i += 1;
                if i + count > payload.len() {
                    break;
                }
                i += count;
            }
            0x23 => {
                if i >= payload.len() {
                    break;
                }
                let count = usize::from(payload[i]);
                i += 1;
                let need = count.saturating_mul(2);
                if i + need > payload.len() {
                    break;
                }
                i += need;
            }
            0x24 => {
                // Badge list: WORD count + n×WORD (OpenTTD `ReadBadgeList`).
                if i + 2 > payload.len() {
                    break;
                }
                let count = usize::from(u16::from_le_bytes([payload[i], payload[i + 1]]));
                i += 2;
                let need = count.saturating_mul(2);
                if i + need > payload.len() {
                    break;
                }
                i += need;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }

    let subst_id = subst_id?;
    if name.is_empty() {
        name = format!("House {local_id}");
    }
    Some(ParsedHouseMeta {
        local_id,
        subst_id,
        building_flags,
        min_year,
        max_year,
        population,
        mail_generation,
        availability,
        probability,
        override_id,
        callback_mask,
        name,
    })
}

#[must_use]
pub fn collect_house_metas_from_grf(data: &[u8]) -> Vec<ParsedHouseMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_house_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `RoadStops` (`0x14`): class, stop type, `draw_mode`, flags, nombre `0xFE`, badges `0xFD`.
#[must_use]
#[allow(clippy::too_many_lines)] // El formato Action0 es de ancho variable por propiedad.
pub fn parse_action0_roadstop_meta(payload: &[u8]) -> Option<ParsedRoadStopMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_ROADSTOPS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut meta = RoadStopMetaParse::default();
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "NGRF") else {
                    break;
                };
                meta.class_short = s;
            }
            PROP_ROADSTOP_STOP_TYPE => {
                if i >= payload.len() {
                    break;
                }
                meta.stop_type = payload[i];
                i += 1;
            }
            PROP_ROADSTOP_DRAW_MODE => {
                if i >= payload.len() {
                    break;
                }
                meta.draw_mode = payload[i];
                i += 1;
            }
            0x0D => {
                if i + 4 > payload.len() {
                    break;
                }
                meta.random_cargo_triggers = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
            }
            PROP_ROADSTOP_FLAGS => {
                if i + 4 > payload.len() {
                    break;
                }
                meta.flags = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
            }
            0x11 => {
                if i >= payload.len() {
                    break;
                }
                meta.callback_mask = payload[i];
                i += 1;
            }
            0x0E => {
                if i + 2 > payload.len() {
                    break;
                }
                meta.animation_frames = payload[i];
                meta.animation_status = payload[i + 1];
                i += 2;
            }
            0x0F => {
                if i >= payload.len() {
                    break;
                }
                meta.animation_speed = payload[i];
                i += 1;
            }
            0x10 => {
                if i + 2 > payload.len() {
                    break;
                }
                meta.animation_triggers = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                meta.label = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            PROP_BADGE_ASSOCIATIONS => {
                let Some(parsed) = read_badge_association_labels(payload, &mut i) else {
                    meta.badge_list_error = Some("lista de badges 0xFD sin BYTE count".into());
                    break;
                };
                meta.badge_labels = parsed.labels;
                if parsed.error.is_some() {
                    meta.badge_list_error = parsed.error;
                }
            }
            // Anchos fijos OTTD (avanzar el bloque sin semántica).
            0x0A | 0x0B | 0x15 => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            _ => break,
        }
    }
    Some(meta.finish())
}

#[must_use]
pub fn collect_roadstop_metas_from_grf(data: &[u8]) -> Vec<ParsedRoadStopMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_roadstop_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Badges` (`0x15`): label 4 chars / `0xFE` nombre, flags DWORD.
///
/// Identidad: preferir `0xFE` C-string; si no hay, `0x08` 4-char.
#[must_use]
pub fn parse_action0_badge_meta(payload: &[u8]) -> Option<ParsedBadgeMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_BADGES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut label_4 = String::new();
    let mut label_cstr = String::new();
    let mut flags = 0u32;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "BDGE") else {
                    break;
                };
                label_4 = s;
            }
            PROP_FLAGS => {
                if i + 4 > payload.len() {
                    break;
                }
                flags = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                label_cstr = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    let label = if label_cstr.is_empty() {
        label_4
    } else {
        label_cstr
    };
    if label.is_empty() {
        return None;
    }
    Some(ParsedBadgeMeta { label, flags })
}

#[must_use]
pub fn collect_badge_metas_from_grf(data: &[u8]) -> Vec<ParsedBadgeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_badge_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Sounds` (`0x0C`): volume `0x08`, priority `0x09`, override `0x0A`.
#[must_use]
pub fn parse_action0_sound_meta(payload: &[u8]) -> Option<ParsedSoundMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_SOUNDS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut volume = 128u8;
    let mut priority = 0u8;
    let mut override_old = None;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_SOUND_VOLUME => {
                if i >= payload.len() {
                    break;
                }
                volume = payload[i].min(128);
                i += 1;
            }
            PROP_SOUND_PRIORITY => {
                if i >= payload.len() {
                    break;
                }
                priority = payload[i];
                i += 1;
            }
            PROP_SOUND_OVERRIDE => {
                if i >= payload.len() {
                    break;
                }
                override_old = Some(payload[i]);
                i += 1;
            }
            _ => break,
        }
    }
    Some(ParsedSoundMeta {
        local_id,
        volume,
        priority,
        override_old,
    })
}

#[must_use]
pub fn collect_sound_metas_from_grf(data: &[u8]) -> Vec<ParsedSoundMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_sound_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Canals` (`0x05`): `callback_mask` `0x08`, flags `0x09`.
#[must_use]
pub fn parse_action0_canal_meta(payload: &[u8]) -> Option<ParsedCanalMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_CANALS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    if usize::from(local_id) >= crate::canal_spec::CANAL_FEATURE_COUNT {
        return None;
    }
    let mut i = 5usize;
    let mut callback_mask = 0u8;
    let mut flags = 0u8;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_CANAL_CALLBACK_MASK => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = payload[i];
                i += 1;
            }
            PROP_CANAL_FLAGS => {
                if i >= payload.len() {
                    break;
                }
                flags = payload[i];
                i += 1;
            }
            _ => break,
        }
    }
    Some(ParsedCanalMeta {
        local_id,
        callback_mask,
        flags,
    })
}

#[must_use]
pub fn collect_canal_metas_from_grf(data: &[u8]) -> Vec<ParsedCanalMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_canal_meta(payload) {
            out.push(meta);
        }
    });
    out
}

fn skip_bridge_sprite_tables(payload: &[u8], i: &mut usize) -> bool {
    if *i + 2 > payload.len() {
        return false;
    }
    let numtables = payload[*i + 1];
    *i += 2; // skip tableid + numtables
    let bytes = usize::from(numtables).saturating_mul(BRIDGE_SPRITES_PER_PIECE * 4);
    if *i + bytes > payload.len() {
        return false;
    }
    *i += bytes;
    true
}

fn skip_bridge_pillars(payload: &[u8], i: &mut usize) -> bool {
    if *i >= payload.len() {
        return false;
    }
    let b = payload[*i];
    *i += 1;
    let tiles = if b == 0xFF {
        if *i + 1 > payload.len() {
            return false;
        }
        let v = u16::from_le_bytes([payload[*i], payload[*i + 1]]);
        *i += 2;
        usize::from(v)
    } else {
        usize::from(b)
    };
    let bytes = tiles.saturating_mul(2);
    if *i + bytes > payload.len() {
        return false;
    }
    *i += bytes;
    true
}

/// Parsea Action0 `Bridges` (`0x06`): year/len/price/speed (+ props consumidas).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_bridge_meta(payload: &[u8]) -> Option<ParsedBridgeMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_BRIDGES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    if local_id >= 13 {
        return None;
    }
    let mut i = 5usize;
    let mut available_from_year = 0u32;
    let mut min_middle_len = 0u16;
    let mut max_middle_len: Option<u16> = None;
    let mut price_mult = 0u16;
    let mut max_speed = u16::MAX;
    let mut name = None;
    let mut has_custom_sprites = false;
    let mut year_set = false;
    let mut min_len_set = false;
    let mut max_len_set = false;
    let mut price_set = false;
    let mut speed_set = false;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_BRIDGE_YEAR => {
                let Some(year) = read_u8(payload, &mut i) else {
                    break;
                };
                available_from_year = if year == 0 {
                    0
                } else {
                    crate::economy::ORIGINAL_BASE_YEAR + u32::from(year)
                };
                year_set = true;
            }
            PROP_BRIDGE_MIN_LEN => {
                let Some(v) = read_u8(payload, &mut i) else {
                    break;
                };
                min_middle_len = u16::from(v);
                min_len_set = true;
            }
            PROP_BRIDGE_MAX_LEN => {
                let Some(v) = read_u8(payload, &mut i) else {
                    break;
                };
                max_middle_len = if v > 16 { None } else { Some(u16::from(v)) };
                max_len_set = true;
            }
            PROP_BRIDGE_PRICE => {
                let Some(v) = read_u8(payload, &mut i) else {
                    break;
                };
                price_mult = u16::from(v);
                price_set = true;
            }
            PROP_BRIDGE_SPEED => {
                let Some(v) = read_u16(payload, &mut i) else {
                    break;
                };
                max_speed = if v == 0 { u16::MAX } else { v };
                speed_set = true;
            }
            PROP_BRIDGE_SPRITE_TABLES => {
                if !skip_bridge_sprite_tables(payload, &mut i) {
                    break;
                }
                has_custom_sprites = true;
            }
            PROP_BRIDGE_FLAGS => {
                if read_u8(payload, &mut i).is_none() {
                    break;
                }
            }
            PROP_BRIDGE_YEAR_LONG => {
                let Some(v) = read_u32(payload, &mut i) else {
                    break;
                };
                available_from_year = v;
                year_set = true;
            }
            PROP_BRIDGE_STR_PURCHASE | PROP_BRIDGE_STR_RAIL | PROP_BRIDGE_STR_ROAD => {
                if read_u16(payload, &mut i).is_none() {
                    break;
                }
            }
            PROP_BRIDGE_PRICE_WORD => {
                let Some(v) = read_u16(payload, &mut i) else {
                    break;
                };
                price_mult = v;
                price_set = true;
            }
            PROP_BRIDGE_PILLARS => {
                if !skip_bridge_pillars(payload, &mut i) {
                    break;
                }
            }
            PROP_NAME_CSTRING => {
                let mut bytes = Vec::new();
                while i < payload.len() && payload[i] != 0 {
                    bytes.push(payload[i]);
                    i += 1;
                }
                if i < payload.len() {
                    i += 1; // NUL
                }
                name = Some(String::from_utf8_lossy(&bytes).into_owned());
            }
            _ => break,
        }
    }
    Some(ParsedBridgeMeta {
        local_id,
        available_from_year,
        min_middle_len,
        max_middle_len,
        price_mult,
        max_speed,
        name,
        has_custom_sprites,
        year_set,
        min_len_set,
        max_len_set,
        price_set,
        speed_set,
    })
}

#[must_use]
pub fn collect_bridge_metas_from_grf(data: &[u8]) -> Vec<ParsedBridgeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_bridge_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Cargoes` (`0x0B`): bitnum, label, pagos, clases, nombre `0xFE`.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_cargo_meta(payload: &[u8]) -> Option<ParsedCargoMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_CARGOES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut bitnum = 0u8;
    let mut label = String::new();
    let mut name = String::new();
    let mut weight = 0u8;
    let mut initial_payment = 0u32;
    let mut transit_fast = 0u8;
    let mut transit_slow = 0u8;
    let mut is_freight = false;
    let mut classes = 0u16;
    let mut capacity_multiplier = crate::cargo_spec::DEFAULT_CARGO_CAPACITY_MULTIPLIER;
    let mut rating_colour = 0u8;
    let mut legend_colour = 0u8;
    let mut callback_mask = 0u8;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_CARGO_BITNUM => {
                if i >= payload.len() {
                    break;
                }
                bitnum = payload[i];
                i += 1;
            }
            // String IDs WORD (0x09–0x0D, 0x1B, 0x1C) — consumidas.
            0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x1B | 0x1C => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            0x0E => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2; // sprite WORD
            }
            0x0F => {
                if i >= payload.len() {
                    break;
                }
                weight = payload[i];
                i += 1;
            }
            0x10 => {
                if i >= payload.len() {
                    break;
                }
                transit_fast = payload[i];
                i += 1;
            }
            0x11 => {
                if i >= payload.len() {
                    break;
                }
                transit_slow = payload[i];
                i += 1;
            }
            0x12 => {
                if i + 4 > payload.len() {
                    break;
                }
                initial_payment = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
            }
            0x13 => {
                if i >= payload.len() {
                    break;
                }
                rating_colour = payload[i];
                i += 1;
            }
            0x14 => {
                if i >= payload.len() {
                    break;
                }
                legend_colour = payload[i];
                i += 1;
            }
            0x15 => {
                if i >= payload.len() {
                    break;
                }
                is_freight = payload[i] != 0;
                i += 1;
            }
            0x16 => {
                if i + 2 > payload.len() {
                    break;
                }
                classes = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            PROP_CARGO_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "CARG") else {
                    break;
                };
                label = s;
            }
            PROP_CARGO_CALLBACK_MASK => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = payload[i];
                i += 1;
            }
            0x18 | 0x1E => {
                if i >= payload.len() {
                    break;
                }
                i += 1; // town subst / callback mask / town prod subst
            }
            0x19 | 0x1F => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            0x1D => {
                if i + 2 > payload.len() {
                    break;
                }
                capacity_multiplier = u16::from_le_bytes([payload[i], payload[i + 1]]).max(1);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    if label.is_empty() {
        return None;
    }
    if name.is_empty() {
        name.clone_from(&label);
    }
    Some(ParsedCargoMeta {
        local_id,
        bitnum,
        label,
        name,
        weight,
        initial_payment,
        transit_fast,
        transit_slow,
        is_freight,
        classes,
        capacity_multiplier,
        rating_colour,
        legend_colour,
        callback_mask,
    })
}

#[must_use]
pub fn collect_cargo_metas_from_grf(data: &[u8]) -> Vec<ParsedCargoMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_cargo_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Objects` (`0x0F`): class label, size, nombre `0xFE`, badges `0xFD`.
#[must_use]
pub fn parse_action0_object_meta(payload: &[u8]) -> Option<ParsedObjectMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_OBJECTS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut class_label = String::new();
    let mut name = String::new();
    let mut size = crate::object_spec::OBJECT_SIZE_1X1;
    let mut climate_mask = crate::object_spec::DEFAULT_OBJECT_CLIMATE_MASK;
    let mut build_cost_factor = crate::object_spec::DEFAULT_OBJECT_BUILD_COST_FACTOR;
    let mut callback_mask = 0u16;
    let mut badge_labels = Vec::new();
    let mut badge_list_error = None;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "OBJT") else {
                    break;
                };
                class_label = s;
            }
            PROP_OBJECT_CLIMATE => {
                if i >= payload.len() {
                    break;
                }
                climate_mask = payload[i];
                i += 1;
            }
            PROP_OBJECT_SIZE => {
                if i >= payload.len() {
                    break;
                }
                size = payload[i];
                i += 1;
                let w = size & 0x0F;
                let h = (size >> 4) & 0x0F;
                if w == 0 || h == 0 {
                    size = crate::object_spec::OBJECT_SIZE_1X1;
                }
            }
            PROP_OBJECT_BUILD_COST => {
                if i >= payload.len() {
                    break;
                }
                build_cost_factor = payload[i];
                i += 1;
            }
            PROP_OBJECT_CALLBACK_MASK => {
                let Some(bytes) = payload.get(i..i + 2) else {
                    break;
                };
                callback_mask = u16::from_le_bytes([bytes[0], bytes[1]]);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            PROP_BADGE_ASSOCIATIONS => {
                let Some(parsed) = read_badge_association_labels(payload, &mut i) else {
                    badge_list_error = Some("lista de badges 0xFD sin BYTE count".into());
                    break;
                };
                badge_labels = parsed.labels;
                if parsed.error.is_some() {
                    badge_list_error = parsed.error;
                }
            }
            _ => break,
        }
    }
    if class_label.is_empty() {
        return None;
    }
    if name.is_empty() {
        name.clone_from(&class_label);
    }
    Some(ParsedObjectMeta {
        local_id,
        class_label,
        name,
        size,
        climate_mask,
        build_cost_factor,
        callback_mask,
        badge_labels,
        badge_list_error,
    })
}

#[must_use]
pub fn collect_object_metas_from_grf(data: &[u8]) -> Vec<ParsedObjectMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_object_meta(payload) {
            out.push(meta);
        }
    });
    out
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_train_meta(payload: &[u8]) -> Option<ParsedTrainMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_TRAINS || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 4usize;
    let local_id = read_extended_byte(payload, &mut i)?;
    let mut name = String::new();
    let mut intro_year = 1920u16;
    let mut max_speed = 96u16;
    let mut power_hp = 500u32;
    let mut lifelength_years = 30u8;
    let mut model_life_years = u8::MAX;
    let mut reliability_spd_dec = crate::engine::DEFAULT_RELIABILITY_SPD_DEC;
    let mut climate_mask = 0x0Fu8;
    let mut load_amount = 0u8;
    let mut train_image_index = 2u8;
    let mut dual_headed = false;
    let mut rail_engine_class = 0u8;
    let mut rail_is_mu = false;
    let mut uses_2cc = false;
    let mut capacity = 0u32;
    let mut cargo = None;
    let mut default_cargo_local_id = None;
    let mut weight_t = 80u16;
    let mut price_factor = 20u8;
    let mut running_cost_factor = 80u8;
    let mut pow_wag_power = 0u32;
    let mut pow_wag_weight = 0u16;
    let mut rail_tilts = false;
    let mut curve_speed_mod = 0i16;
    let mut tractive_effort = 0u8;
    let mut air_drag = 0u8;
    let mut shorten_factor = 0u8;
    let mut required_rail_type = None;
    let mut refit_mask = 0u32;
    let mut ctt_include_cargo_indices = Vec::new();
    let mut ctt_exclude_cargo_indices = Vec::new();
    let mut callback_mask = 0u16;
    let mut sprite_stack = false;
    let mut visual_effect = crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT;
    let mut badge_local_ids = Vec::new();
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x00 => {
                if i + 2 > payload.len() {
                    break;
                }
                let days = u16::from_le_bytes([payload[i], payload[i + 1]]);
                intro_year = 1920u16.saturating_add(days / 365);
                i += 2;
            }
            0x02 => {
                reliability_spd_dec = u16::from(read_u8(payload, &mut i)?) << 2;
            }
            0x03 => {
                lifelength_years = read_u8(payload, &mut i)?;
            }
            0x04 => {
                model_life_years = read_u8(payload, &mut i)?;
            }
            0x05 => {
                let v = read_u8(payload, &mut i)?;
                if v < 4 {
                    required_rail_type = Some(v);
                }
            }
            0x06 => {
                climate_mask = read_u8(payload, &mut i)?;
            }
            0x07 => {
                load_amount = read_u8(payload, &mut i)?;
            }
            0x0D => {
                running_cost_factor = read_u8(payload, &mut i)?;
            }
            0x12 => {
                let mut spriteid = read_u8(payload, &mut i)?;
                // TTD sprite IDs → índice local (`CUSTOM_VEHICLE_SPRITENUM` = 0xFD).
                if spriteid < 0xFD {
                    spriteid >>= 1;
                }
                train_image_index = spriteid;
            }
            0x13 => {
                dual_headed = read_u8(payload, &mut i)? != 0;
            }
            0x19 => {
                let traction = read_u8(payload, &mut i)?;
                rail_engine_class = match traction {
                    0x00..=0x07 => 0,
                    0x08..=0x27 => 1,
                    0x28..=0x31 => 2,
                    0x32..=0x37 => 3,
                    0x38..=0x41 => 4,
                    _ => rail_engine_class,
                };
            }
            0x14 => {
                capacity = u32::from(read_u8(payload, &mut i)?);
            }
            0x15 => {
                let ctype = read_u8(payload, &mut i)?;
                default_cargo_local_id = Some(ctype);
                cargo = if ctype == 0xFF {
                    None
                } else {
                    crate::cargo::CargoType::from_temperate_id(ctype)
                };
            }
            0x16 => {
                // Upstream: peso low BYTE. Fixtures locales usan WORD año (1800..3000).
                if let Some(year) = payload.get(i..i.checked_add(2)?).and_then(|bytes| {
                    let year = u16::from_le_bytes([bytes[0], bytes[1]]);
                    (1800..3000).contains(&year).then_some(year)
                }) {
                    intro_year = year;
                    i += 2;
                } else {
                    weight_t = (weight_t & 0xFF00) | u16::from(read_u8(payload, &mut i)?);
                }
            }
            0x17 => {
                price_factor = read_u8(payload, &mut i)?;
            }
            0x1B => {
                pow_wag_power = u32::from(read_u16(payload, &mut i)?);
            }
            0x1D => {
                // Fixtures locales: WORD (bits bajos del bitmask temperate).
                refit_mask = u32::from(read_u16(payload, &mut i)?);
            }
            0x1F => {
                tractive_effort = read_u8(payload, &mut i)?;
            }
            0x20 => {
                air_drag = read_u8(payload, &mut i)?;
            }
            0x21 => {
                shorten_factor = read_u8(payload, &mut i)?;
            }
            0x22 => {
                visual_effect = normalize_visual_effect(read_u8(payload, &mut i)?);
            }
            0x23 => {
                pow_wag_weight = u16::from(read_u8(payload, &mut i)?);
            }
            0x24 => {
                let hi = read_u8(payload, &mut i)?;
                if hi <= 4 {
                    weight_t = (u16::from(hi) << 8) | (weight_t & 0x00FF);
                }
            }
            0x27 => {
                // `EngineMiscFlag`: RailTilts = bit 0, Uses2CC = bit 1,
                // RailIsMU = bit 2 y SpriteStack = bit 7.
                let flags = read_u8(payload, &mut i)?;
                rail_tilts = flags & 0x01 != 0;
                uses_2cc = flags & 0x02 != 0;
                rail_is_mu = flags & 0x04 != 0;
                sprite_stack = flags & 0x80 != 0;
            }
            0x2E => {
                curve_speed_mod = i16::from_le_bytes(read_u16(payload, &mut i)?.to_le_bytes());
            }
            // Anchos fijos restantes consumidos sin semántica runtime.
            0x08 | 0x0A | 0x0C | 0x0F | 0x10 | 0x11 | 0x18 | 0x1C | 0x25 | 0x26 => {
                skip_bytes(payload, &mut i, 1)?;
            }
            0x1E => {
                callback_mask = (callback_mask & 0xFF00) | u16::from(read_u8(payload, &mut i)?);
            }
            0x1A => {
                // Extended byte sort order: BYTE en fixtures locales.
                skip_bytes(payload, &mut i, 1)?;
            }
            0x28 | 0x29 | 0x2B | 0x2F => {
                skip_bytes(payload, &mut i, 2)?;
            }
            // 0x0E running cost base; 0x2A/0x30 listas/DWORD: consumidas.
            0x0E | 0x2A | 0x30 => {
                skip_bytes(payload, &mut i, 4)?;
            }
            0x2C | 0x2D => {
                let count = usize::from(read_u8(payload, &mut i)?);
                let mut indices = Vec::with_capacity(count);
                for _ in 0..count {
                    indices.push(read_u8(payload, &mut i)?);
                }
                if prop == 0x2C {
                    ctt_include_cargo_indices = indices;
                } else {
                    ctt_exclude_cargo_indices = indices;
                }
            }
            0x33 => {
                badge_local_ids = read_badge_local_ids(payload, &mut i)?;
            }
            0x31 => {
                callback_mask =
                    (callback_mask & 0x00FF) | (u16::from(read_u8(payload, &mut i)?) << 8);
            }
            PROP_TRAIN_SPEED => {
                if i + 2 > payload.len() {
                    break;
                }
                max_speed = u16::from_le_bytes([payload[i], payload[i + 1]]).max(1);
                i += 2;
            }
            PROP_TRAIN_POWER => {
                if i + 2 > payload.len() {
                    break;
                }
                power_hp = u32::from(u16::from_le_bytes([payload[i], payload[i + 1]])).max(1);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    if name.is_empty() {
        name = "NewGRF Train".into();
    }
    Some(ParsedTrainMeta {
        local_id,
        name,
        intro_year,
        max_speed,
        power_hp,
        lifelength_years,
        model_life_years,
        reliability_spd_dec,
        climate_mask,
        load_amount,
        train_image_index,
        dual_headed,
        rail_engine_class,
        rail_is_mu,
        uses_2cc,
        capacity,
        cargo,
        default_cargo_local_id,
        weight_t,
        price_factor,
        running_cost_factor,
        pow_wag_power,
        pow_wag_weight,
        rail_tilts,
        curve_speed_mod,
        tractive_effort,
        air_drag,
        shorten_factor,
        required_rail_type,
        refit_mask,
        ctt_include_cargo_indices,
        ctt_exclude_cargo_indices,
        callback_mask,
        visual_effect,
        sprite_stack,
        badge_local_ids,
    })
}

/// Action0 representa `VE_DEFAULT` como un byte con el bit de desactivación
/// activo; se limpian sólo los bits de tipo para conservar la semántica nativa.
fn normalize_visual_effect(value: u8) -> u8 {
    if value == crate::engine::VEHICLE_VISUAL_EFFECT_DEFAULT {
        value & !(0x03 << 4)
    } else {
        value
    }
}

#[must_use]
pub fn collect_train_metas_from_grf(data: &[u8]) -> Vec<ParsedTrainMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_train_meta(payload) {
            out.push(meta);
        }
    });
    out
}

fn read_u8(payload: &[u8], i: &mut usize) -> Option<u8> {
    let value = *payload.get(*i)?;
    *i += 1;
    Some(value)
}

/// Lee un `ExtendedByte` del wire format `NewGRF` (BYTE, o WORD LE cuando el
/// byte sentinela es `0xFF`).  Action0 y Action3 usan esta codificación para
/// identificar entidades; no debe confundirse con los bytes de propiedades.
fn read_extended_byte(payload: &[u8], i: &mut usize) -> Option<u16> {
    let value = u16::from(read_u8(payload, i)?);
    if value == 0xFF {
        read_u16(payload, i)
    } else {
        Some(value)
    }
}

fn read_u16(payload: &[u8], i: &mut usize) -> Option<u16> {
    let bytes = payload.get(*i..i.checked_add(2)?)?;
    *i += 2;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32(payload: &[u8], i: &mut usize) -> Option<u32> {
    let bytes = payload.get(*i..i.checked_add(4)?)?;
    *i += 4;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn skip_bytes(payload: &[u8], i: &mut usize, amount: usize) -> Option<()> {
    let end = i.checked_add(amount)?;
    payload.get(*i..end)?;
    *i = end;
    Some(())
}

fn parse_common_vehicle_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<bool> {
    match prop {
        0x00 => {
            for meta in metas {
                let days = read_u16(payload, i)?;
                meta.intro_year = 1920u16.saturating_add(days / 365);
            }
        }
        0x02 => {
            for meta in metas {
                meta.reliability_spd_dec = u16::from(read_u8(payload, i)?) << 2;
            }
        }
        0x03 => {
            for meta in metas {
                meta.lifelength_years = read_u8(payload, i)?;
            }
        }
        0x04 => {
            for meta in metas {
                meta.model_life_years = read_u8(payload, i)?;
            }
        }
        0x06 => {
            for meta in metas {
                meta.climate_mask = read_u8(payload, i)?;
            }
        }
        0x07 => {
            for meta in metas {
                meta.load_amount = read_u8(payload, i)?;
            }
        }
        _ => return Some(false),
    }
    Some(true)
}

fn parse_road_vehicle_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x15 => {
            for meta in metas {
                meta.max_speed = u16::from(read_u8(payload, i)?).max(1);
            }
        }
        0x09 => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        // 0x0A/0x16/0x1F/0x27: dword props aún no mapeadas al runtime.
        0x0A | 0x16 | 0x1F | 0x27 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        0x1C => {
            for meta in metas {
                let flags = read_u8(payload, i)?;
                meta.uses_2cc = flags & 0x02 != 0;
                meta.sprite_stack = flags & 0x80 != 0;
            }
        }
        // 0x05 translation table; 0x20/0x28 extended byte (fixtures usan BYTE).
        0x05 | 0x0E | 0x18 | 0x19 | 0x1A | 0x1B | 0x20 | 0x23 | 0x28 => {
            skip_bytes(payload, i, metas.len())?;
        }
        0x2A => {
            for meta in metas {
                meta.badge_local_ids = read_badge_local_ids(payload, i)?;
            }
        }
        0x21 => {
            for meta in metas {
                meta.visual_effect = normalize_visual_effect(read_u8(payload, i)?);
            }
        }
        0x17 => {
            for meta in metas {
                meta.callback_mask = u16::from(read_u8(payload, i)?);
            }
        }
        0x0F => {
            for meta in metas {
                meta.capacity = u32::from(read_u8(payload, i)?);
            }
        }
        0x10 => {
            for meta in metas {
                let local_id = read_u8(payload, i)?;
                meta.default_cargo_local_id = Some(local_id);
                let cargo = crate::cargo::CargoType::from_temperate_id(local_id);
                meta.cargo = cargo;
                meta.kind = if cargo == Some(crate::cargo::CargoType::Passengers) {
                    VehicleKind::Bus
                } else {
                    VehicleKind::Truck
                };
            }
        }
        0x11 => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x12 => {
            for meta in metas {
                meta.sound_effect = read_u8(payload, i)?;
            }
        }
        0x13 => {
            for meta in metas {
                meta.power_hp = u32::from(read_u8(payload, i)?) * 10;
            }
        }
        0x14 => {
            for meta in metas {
                meta.weight_t = u16::from(read_u8(payload, i)?).div_ceil(4);
            }
        }
        // CTT refit include (`0x24`) / exclude (`0x25`).
        0x24 | 0x25 => {
            for meta in metas {
                let count = usize::from(read_u8(payload, i)?);
                let mut indices = Vec::with_capacity(count);
                for _ in 0..count {
                    indices.push(read_u8(payload, i)?);
                }
                if prop == 0x24 {
                    meta.ctt_include_cargo_indices = indices;
                } else {
                    meta.ctt_exclude_cargo_indices = indices;
                }
            }
        }
        0x1D | 0x1E | 0x22 | 0x26 | 0x29 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        _ => return None,
    }
    Some(())
}

#[allow(clippy::too_many_lines)]
fn parse_ship_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x09 | 0x13 | 0x16 | 0x1C | 0x24 => {
            if prop == 0x1C {
                for meta in metas {
                    meta.visual_effect = normalize_visual_effect(read_u8(payload, i)?);
                }
            } else {
                skip_bytes(payload, i, metas.len())?;
            }
        }
        0x17 => {
            for meta in metas {
                let flags = read_u8(payload, i)?;
                meta.uses_2cc = flags & 0x02 != 0;
                meta.sprite_stack = flags & 0x80 != 0;
            }
        }
        0x12 => {
            for meta in metas {
                meta.callback_mask = u16::from(read_u8(payload, i)?);
            }
        }
        0x0A => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x0B => {
            for meta in metas {
                meta.max_speed = u16::from(read_u8(payload, i)?).max(1);
            }
        }
        0x0C => {
            for meta in metas {
                let local_id = read_u8(payload, i)?;
                meta.default_cargo_local_id = Some(local_id);
                meta.cargo = crate::cargo::CargoType::from_temperate_id(local_id);
            }
        }
        0x0D => {
            for meta in metas {
                meta.capacity = u32::from(read_u16(payload, i)?);
            }
        }
        0x0F => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        0x10 => {
            for meta in metas {
                meta.sound_effect = read_u8(payload, i)?;
            }
        }
        0x11 | 0x1A | 0x21 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        0x14 => {
            for meta in metas {
                meta.ocean_speed_frac = read_u8(payload, i)?;
            }
        }
        0x15 => {
            for meta in metas {
                meta.canal_speed_frac = read_u8(payload, i)?;
            }
        }
        0x18 | 0x19 | 0x1D | 0x20 | 0x25 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        0x26 => {
            for meta in metas {
                meta.badge_local_ids = read_badge_local_ids(payload, i)?;
            }
        }
        0x1B => skip_bytes(payload, i, metas.len())?,
        0x22 => {
            for meta in metas {
                meta.callback_mask =
                    (meta.callback_mask & 0x00FF) | (u16::from(read_u8(payload, i)?) << 8);
            }
        }
        // CTT refit include (`0x1E`) / exclude (`0x1F`): BYTE count + N cargo indices.
        0x1E | 0x1F => {
            for meta in metas {
                let count = usize::from(read_u8(payload, i)?);
                let mut indices = Vec::with_capacity(count);
                let mut mask = 0u32;
                for _ in 0..count {
                    let ctype = read_u8(payload, i)?;
                    indices.push(ctype);
                    if let Some(cargo) = crate::cargo::CargoType::from_temperate_id(ctype) {
                        mask |= 1u32 << cargo.temperate_id();
                    }
                }
                if prop == 0x1E && count > 0 {
                    meta.refit_mask = mask;
                    meta.ctt_include_cargo_indices = indices;
                } else if prop == 0x1F {
                    meta.refit_exclude_mask = mask;
                    meta.ctt_exclude_cargo_indices = indices;
                }
            }
        }
        0x23 => {
            for meta in metas {
                meta.max_speed = read_u16(payload, i)?.max(1);
            }
        }
        _ => return None,
    }
    Some(())
}

fn parse_aircraft_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x0D | 0x15 | 0x16 | 0x1B => {
            skip_bytes(payload, i, metas.len())?;
        }
        0x17 => {
            for meta in metas {
                let flags = read_u8(payload, i)?;
                meta.uses_2cc = flags & 0x02 != 0;
                meta.sprite_stack = flags & 0x80 != 0;
            }
        }
        0x14 => {
            for meta in metas {
                meta.callback_mask =
                    (meta.callback_mask & 0xFF00) | u16::from(read_u8(payload, i)?);
            }
        }
        0x22 => {
            for meta in metas {
                meta.callback_mask =
                    (meta.callback_mask & 0x00FF) | (u16::from(read_u8(payload, i)?) << 8);
            }
        }
        0x09 => {
            for meta in metas {
                meta.is_helicopter = read_u8(payload, i)? != 0;
            }
        }
        0x0A => {
            for meta in metas {
                meta.is_large_aircraft = read_u8(payload, i)? != 0;
            }
        }
        0x0B => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x0C => {
            for meta in metas {
                meta.max_speed = (u16::from(read_u8(payload, i)?) * 128 / 10).max(1);
            }
        }
        0x0E => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        0x0F => {
            for meta in metas {
                meta.capacity = u32::from(read_u16(payload, i)?);
            }
        }
        0x11 => skip_bytes(payload, i, metas.len())?, // mail capacity: EngineDef has one cargo
        0x12 => {
            for meta in metas {
                meta.sound_effect = read_u8(payload, i)?;
            }
        }
        // CTT refit include (`0x1D`) / exclude (`0x1E`).
        0x1D | 0x1E => {
            for meta in metas {
                let count = usize::from(read_u8(payload, i)?);
                let mut indices = Vec::with_capacity(count);
                for _ in 0..count {
                    indices.push(read_u8(payload, i)?);
                }
                if prop == 0x1D {
                    meta.ctt_include_cargo_indices = indices;
                } else {
                    meta.ctt_exclude_cargo_indices = indices;
                }
            }
        }
        0x13 | 0x1A | 0x21 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        0x18 | 0x19 | 0x1F | 0x20 | 0x23 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        0x24 => {
            for meta in metas {
                meta.badge_local_ids = read_badge_local_ids(payload, i)?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// Parsea un bloque Action0 completo de road vehicles, ships o aircraft.
#[must_use]
pub fn parse_action0_vehicle_metas(payload: &[u8]) -> Option<Vec<ParsedVehicleMeta>> {
    let header = parse_action0_header(payload)?;
    if !matches!(
        header.feature,
        ACTION0_FEATURE_ROAD_VEHICLES | ACTION0_FEATURE_SHIPS | ACTION0_FEATURE_AIRCRAFT
    ) || header.num_ids == 0
        || payload.len() < 5
    {
        return None;
    }
    let mut i = 4usize;
    let first_id = read_extended_byte(payload, &mut i)?;
    let mut metas = (0..header.num_ids)
        .map(|offset| {
            ParsedVehicleMeta::defaults(header.feature, first_id.saturating_add(u16::from(offset)))
        })
        .collect::<Option<Vec<_>>>()?;
    for _ in 0..header.num_props {
        let prop = read_u8(payload, &mut i)?;
        if parse_common_vehicle_property(prop, payload, &mut i, &mut metas)? {
            continue;
        }
        match header.feature {
            ACTION0_FEATURE_ROAD_VEHICLES => {
                parse_road_vehicle_property(prop, payload, &mut i, &mut metas)?;
            }
            ACTION0_FEATURE_SHIPS => parse_ship_property(prop, payload, &mut i, &mut metas)?,
            ACTION0_FEATURE_AIRCRAFT => parse_aircraft_property(prop, payload, &mut i, &mut metas)?,
            _ => return None,
        }
    }
    Some(metas)
}

#[must_use]
pub fn collect_vehicle_metas_from_grf(data: &[u8], feature: u8) -> Vec<ParsedVehicleMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if payload.get(1) == Some(&feature)
            && let Some(metas) = parse_action0_vehicle_metas(payload)
        {
            out.extend(metas);
        }
    });
    out
}

/// Parsea Action0 `AirportTiles` (`0x11`). Requiere `prop 0x08` (subst).
#[must_use]
pub fn parse_action0_airport_tile_meta(payload: &[u8]) -> Option<ParsedAirportTileMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_AIRPORTTILES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut override_of: Option<u8> = None;
    let mut callback_mask = 0u8;
    let mut animation_frames = 0u8;
    let mut animation_status = 0xFFu8;
    let mut animation_speed = 2u8;
    let mut animation_triggers = 0u8;
    let mut badge_local_ids = Vec::new();
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x08 => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if u16::from(s) < crate::airport_tile_spec::NEW_AIRPORT_TILE_OFFSET {
                    subst_id = Some(s);
                }
            }
            0x09 => {
                if i >= payload.len() {
                    break;
                }
                let o = payload[i];
                i += 1;
                if u16::from(o) < crate::airport_tile_spec::NEW_AIRPORT_TILE_OFFSET {
                    override_of = Some(o);
                }
            }
            0x0E => {
                if i >= payload.len() {
                    break;
                }
                callback_mask = payload[i];
                i += 1;
            }
            0x0F => {
                // Animation: frames + status
                if i + 2 > payload.len() {
                    break;
                }
                animation_frames = payload[i];
                animation_status = payload[i + 1];
                i += 2;
            }
            0x10 => {
                if i >= payload.len() {
                    break;
                }
                animation_speed = payload[i];
                i += 1;
            }
            0x11 => {
                if i >= payload.len() {
                    break;
                }
                animation_triggers = payload[i];
                i += 1;
            }
            0x12 => {
                badge_local_ids = read_badge_local_ids(payload, &mut i)?;
            }
            _ => break,
        }
    }
    Some(ParsedAirportTileMeta {
        local_id,
        subst_id: subst_id?,
        override_of,
        callback_mask,
        animation_frames,
        animation_status,
        animation_speed,
        animation_triggers,
        animation_special_flags: 0,
        badge_local_ids,
    })
}

#[must_use]
pub fn collect_airport_tile_metas_from_grf(data: &[u8]) -> Vec<ParsedAirportTileMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_airport_tile_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Airports` (`0x0D`). Requiere `prop 0x08` (subst o disable).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn parse_action0_airport_meta(payload: &[u8]) -> Option<ParsedAirportMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_AIRPORTS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut disabled = false;
    let mut layouts: Vec<ParsedAirportLayout> = Vec::new();
    let mut size_x = 0u8;
    let mut size_y = 0u8;
    let mut min_year = 0u16;
    let mut max_year = 0xFFFFu16;
    let mut ttd_airport_type = 0u8;
    let mut catchment = 4u8;
    let mut noise_level = 3u8;
    let mut maintenance_cost = 0u16;
    let mut name = String::new();
    let mut badge_local_ids = Vec::new();

    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x08 => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if s == 0xFF {
                    disabled = true;
                    subst_id = Some(local_id); // disable target = local vanilla id
                } else if s < 10 {
                    subst_id = Some(s);
                }
            }
            0x0A => {
                if i >= payload.len() {
                    break;
                }
                let num_layouts = usize::from(payload[i]);
                i += 1;
                if i + 4 > payload.len() {
                    break;
                }
                i += 4; // total size DWORD (ignored)
                layouts.clear();
                size_x = 0;
                size_y = 0;
                for _j in 0..num_layouts {
                    if i >= payload.len() {
                        break;
                    }
                    let rotation = payload[i] & 6;
                    i += 1;
                    let mut tiles = Vec::new();
                    loop {
                        if i + 2 > payload.len() {
                            break;
                        }
                        let x = payload[i].cast_signed();
                        let y = payload[i + 1].cast_signed();
                        i += 2;
                        if x == 0 && y.cast_unsigned() == 0x80 {
                            break;
                        }
                        if i >= payload.len() {
                            break;
                        }
                        let gfx = payload[i];
                        i += 1;
                        let (gfx_or_local, use_local_tile) = if gfx == 0xFE {
                            if i + 2 > payload.len() {
                                break;
                            }
                            let local = u16::from_le_bytes([payload[i], payload[i + 1]]);
                            i += 2;
                            (local, true)
                        } else {
                            (u16::from(gfx), false)
                        };
                        // size from tile coords (N/S vs E/W)
                        let (sx, sy) = if rotation == 2 || rotation == 6 {
                            (
                                u8::try_from(i32::from(y) + 1).unwrap_or(1),
                                u8::try_from(i32::from(x) + 1).unwrap_or(1),
                            )
                        } else {
                            (
                                u8::try_from(i32::from(x) + 1).unwrap_or(1),
                                u8::try_from(i32::from(y) + 1).unwrap_or(1),
                            )
                        };
                        size_x = size_x.max(sx);
                        size_y = size_y.max(sy);
                        tiles.push(ParsedAirportLayoutTile {
                            x,
                            y,
                            gfx_or_local,
                            use_local_tile,
                        });
                    }
                    layouts.push(ParsedAirportLayout { rotation, tiles });
                }
            }
            0x0C => {
                if i + 4 > payload.len() {
                    break;
                }
                min_year = u16::from_le_bytes([payload[i], payload[i + 1]]);
                max_year = u16::from_le_bytes([payload[i + 2], payload[i + 3]]);
                i += 4;
            }
            0x0D => {
                if i >= payload.len() {
                    break;
                }
                ttd_airport_type = payload[i];
                i += 1;
            }
            0x0E => {
                if i >= payload.len() {
                    break;
                }
                catchment = payload[i].clamp(1, 10);
                i += 1;
            }
            0x0F => {
                if i >= payload.len() {
                    break;
                }
                noise_level = payload[i];
                i += 1;
            }
            0x10 => {
                if i + 2 > payload.len() {
                    break;
                }
                let sid = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
                if sid == 0xFE {
                    // C-string local name
                    let start = i;
                    while i < payload.len() && payload[i] != 0 {
                        i += 1;
                    }
                    name = String::from_utf8_lossy(&payload[start..i]).into_owned();
                    if i < payload.len() {
                        i += 1;
                    }
                } else {
                    name = format!("Airport#{sid}");
                }
            }
            0x11 => {
                if i + 2 > payload.len() {
                    break;
                }
                maintenance_cost = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            0x12 => {
                badge_local_ids = read_badge_local_ids(payload, &mut i)?;
            }
            _ => break,
        }
    }
    if disabled {
        return Some(ParsedAirportMeta {
            local_id,
            subst_id: subst_id.unwrap_or(local_id),
            disabled: true,
            layouts: Vec::new(),
            size_x: 0,
            size_y: 0,
            min_year,
            max_year,
            ttd_airport_type,
            catchment,
            noise_level,
            maintenance_cost,
            name,
            badge_local_ids,
        });
    }
    Some(ParsedAirportMeta {
        local_id,
        subst_id: subst_id?,
        disabled: false,
        layouts,
        size_x,
        size_y,
        min_year,
        max_year,
        ttd_airport_type,
        catchment,
        noise_level,
        maintenance_cost,
        name,
        badge_local_ids,
    })
}

#[must_use]
pub fn collect_airport_metas_from_grf(data: &[u8]) -> Vec<ParsedAirportMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_airport_meta(payload) {
            out.push(meta);
        }
    });
    out
}
