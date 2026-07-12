//! Motores base `OpenGFX` (velocidad máxima en unidades internas de `OpenTTD`).
//!
//! Catálogo con los vehículos originales del clima templado; los valores
//! provienen de `_orig_rail_vehicle_info` / `_orig_road_vehicle_info` y
//! `_orig_engine_info` (`src/table/engines.h` del upstream), con precios y
//! costes de operación derivados de `src/table/pricebase.h` (`cost = base ×
//! cost_factor >> 8`).

use crate::cargo::CargoType;
use crate::vehicle::{VehicleDirection, VehicleKind};
use serde::{Deserialize, Serialize};

/// Primer ID reservado para motores Action0 `NewGRF` (trains).
pub const NEWGRF_ENGINE_ID_BASE: u16 = 1000;

/// Definición de motor (paridad con `_orig_*_vehicle_info` del upstream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDef {
    pub id: u16,
    pub kind: VehicleKind,
    pub name: String,
    /// Unidades `OpenTTD` (`RVI` ≈ 1 km/h por unidad; `ROV` ≈ 0,5 km/h).
    pub max_speed: u16,
    /// Precio de compra (libras internas TTD: `base_price × cost_factor >> 8`).
    pub price: i64,
    /// Coste de explotación anual (libras internas TTD).
    pub running_cost_year: i64,
    /// Capacidad del modelo (pasajeros/sacas/cajas). 0 = solo locomotora.
    pub capacity: u32,
    /// Carga de diseño del modelo (`None` = locomotora sin carga propia).
    pub cargo: Option<CargoType>,
    pub power_hp: u32,
    pub weight_t: u16,
    pub intro_year: u16,
    /// Fiabilidad inicial mostrada en la compra (aprox. por clase de motor).
    pub reliability_pct: u8,
    /// Índice de sprite de locomotora (`OpenTTD` `image_index`; 0 en carretera).
    pub train_image_index: u8,
    /// Procedente de Action0 Vehicles `NewGRF`.
    #[serde(default)]
    pub from_newgrf: bool,
}

impl EngineDef {
    /// Velocidad máxima en km/h para mostrar en UI (conversión por tipo).
    #[must_use]
    pub fn speed_kmh(&self) -> u16 {
        match self.kind {
            VehicleKind::Train | VehicleKind::Aircraft => self.max_speed,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Ship => {
                self.max_speed / 2
            }
        }
    }

    /// Vagón: tren sin potencia y con capacidad de carga.
    #[must_use]
    pub fn is_wagon(&self) -> bool {
        matches!(self.kind, VehicleKind::Train) && self.power_hp == 0 && self.capacity > 0
    }

    /// Locomotora o DMU (puede ser cabeza de consist).
    #[must_use]
    pub fn is_train_engine(&self) -> bool {
        matches!(self.kind, VehicleKind::Train) && !self.is_wagon()
    }
}

pub const ENGINE_BUS_MPS: u16 = 0;
pub const ENGINE_BUS_HEREFORD: u16 = 1;
pub const ENGINE_BUS_FOSTER: u16 = 2;
/// Tranvía vanilla (pasajeros; sprites de bus como placeholder).
pub const ENGINE_TRAM_MPS: u16 = 5;
pub const ENGINE_TRUCK_MPS: u16 = 10;
pub const ENGINE_TRUCK_BALOGH_GOODS: u16 = 11;
pub const ENGINE_TRUCK_CRAIGHEAD_GOODS: u16 = 12;
pub const ENGINE_TRUCK_GOSS_GOODS: u16 = 13;
pub const ENGINE_TRAIN_KIRBY: u16 = 100;
pub const ENGINE_TRAIN_CHANEY_JUBILEE: u16 = 101;
pub const ENGINE_TRAIN_GINZU_A4: u16 = 102;
pub const ENGINE_TRAIN_SH_8P: u16 = 103;
pub const ENGINE_TRAIN_MANLEY_MOREL: u16 = 104;
pub const ENGINE_TRAIN_DASH: u16 = 105;
pub const ENGINE_TRAIN_SH_HENDRY_25: u16 = 106;
pub const ENGINE_TRAIN_UU_37: u16 = 107;
pub const ENGINE_TRAIN_FLOSS_47: u16 = 108;
pub const ENGINE_TRAIN_SH_125: u16 = 109;
pub const ENGINE_TRAIN_SH_30: u16 = 110;
pub const ENGINE_TRAIN_SH_40: u16 = 111;
pub const ENGINE_TRAIN_TIM: u16 = 112;
pub const ENGINE_TRAIN_ASIASTAR: u16 = 113;
/// Monorail X2001 (`OpenTTD` id 54 → +100).
pub const ENGINE_TRAIN_X2001: u16 = 154;
/// Maglev Lev1 (`OpenTTD` id 84 → +100).
pub const ENGINE_TRAIN_LEV1: u16 = 184;
/// Vagón de pasajeros (`OpenGFX` temperate, catálogo sandbox).
pub const ENGINE_WAGON_PASSENGER: u16 = 150;
/// Vagón de correo.
pub const ENGINE_WAGON_MAIL: u16 = 151;
/// Vagón de mercancías.
pub const ENGINE_WAGON_GOODS: u16 = 152;
/// Tolva de carbón.
pub const ENGINE_WAGON_COAL: u16 = 153;
pub const ENGINE_SHIP_MPS: u16 = 200;
pub const ENGINE_SHIP_OIL: u16 = 201;
pub const ENGINE_SHIP_COAL: u16 = 202;
pub const ENGINE_SHIP_FERRY: u16 = 203;
pub const ENGINE_AIRCRAFT_DAKOTA: u16 = 300;
pub const ENGINE_AIRCRAFT_FOKKER: u16 = 301;
/// Helicóptero `OpenGFX` (`image_index` 9 → sprites 3813..3820).
pub const ENGINE_AIRCRAFT_TRICARIO: u16 = 302;

/// Paso sub-tile del bus MPS en diagonal a velocidad de crucero (`GetAdvanceSpeed` ×
/// `255/192` sobre `GetAdvanceDistance` diagonal — `vehicle_base.h:439-455`).
pub const REFERENCE_PROGRESS_STEP: u8 = 112;

/// Aceleración carretera modelo original (`RoadVehicle::UpdateSpeed`, `AM_ORIGINAL`).
pub const ROAD_ACCEL_ORIGINAL: u16 = 256;

const REFERENCE_MAX_SPEED: u16 = 112;
const TILE_AXIAL_DISTANCE: u32 = 192;
const TILE_CORNER_DISTANCE: u32 = 256;

/// Fiabilidad inicial aproximada por clase de motor del original.
const RELIABILITY_STEAM: u8 = 75;
const RELIABILITY_DIESEL: u8 = 85;
const RELIABILITY_ELECTRIC: u8 = 90;
const RELIABILITY_ROAD: u8 = 85;

macro_rules! road {
    ($id:expr, $kind:expr, $name:expr, $speed:expr, $cf:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr) => {
        EngineDef {
            id: $id,
            kind: $kind,
            name: ($name).into(),
            max_speed: $speed,
            price: (14_000 * $cf) >> 8,
            running_cost_year: (1_600 * $rc) >> 8,
            capacity: $cap,
            cargo: $cargo,
            power_hp: $hp,
            weight_t: $wt,
            intro_year: $year,
            reliability_pct: RELIABILITY_ROAD,
            train_image_index: 0,
            from_newgrf: false,
        }
    };
}

macro_rules! train {
    ($id:expr, $name:expr, $speed:expr, $cf:expr, $rc_base:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $rel:expr, $img:expr) => {
        EngineDef {
            id: $id,
            kind: VehicleKind::Train,
            name: ($name).into(),
            max_speed: $speed,
            price: (400_000_i64 * $cf) >> 8,
            running_cost_year: ($rc_base * $rc) >> 8,
            capacity: $cap,
            cargo: $cargo,
            power_hp: $hp,
            weight_t: $wt,
            intro_year: $year,
            reliability_pct: $rel,
            train_image_index: $img,
            from_newgrf: false,
        }
    };
}

const RC_STEAM: i64 = 5_600;
const RC_DIESEL: i64 = 5_200;
const RC_ELECTRIC: i64 = 4_800;
const RC_MONORAIL: i64 = 4_400;
const RC_MAGLEV: i64 = 4_000;
const RELIABILITY_MONORAIL: u8 = 92;
const RELIABILITY_MAGLEV: u8 = 95;

#[allow(clippy::too_many_lines)]
fn build_vanilla_engines() -> Vec<EngineDef> {
    vec![
        // Carretera: buses (pesos del upstream en cuartos de tonelada, redondeados).
        road!(
            ENGINE_BUS_MPS,
            VehicleKind::Bus,
            "MPS Regal Bus",
            112,
            120,
            91,
            31,
            Some(CargoType::Passengers),
            90,
            11,
            1929
        ),
        road!(
            ENGINE_BUS_HEREFORD,
            VehicleKind::Bus,
            "Hereford Leopard Bus",
            176,
            140,
            128,
            35,
            Some(CargoType::Passengers),
            120,
            15,
            1964
        ),
        road!(
            ENGINE_BUS_FOSTER,
            VehicleKind::Bus,
            "Foster Bus",
            224,
            150,
            178,
            37,
            Some(CargoType::Passengers),
            150,
            18,
            1986
        ),
        road!(
            ENGINE_TRAM_MPS,
            VehicleKind::Tram,
            "MPS Electric Tram",
            128,
            130,
            100,
            33,
            Some(CargoType::Passengers),
            100,
            12,
            1935
        ),
        // Carretera: camiones.
        road!(
            ENGINE_TRUCK_MPS,
            VehicleKind::Truck,
            "MPS Mail Truck",
            96,
            115,
            90,
            22,
            Some(CargoType::Mail),
            120,
            10,
            1935
        ),
        road!(
            ENGINE_TRUCK_BALOGH_GOODS,
            VehicleKind::Truck,
            "Balogh Goods Truck",
            96,
            107,
            90,
            14,
            Some(CargoType::Goods),
            120,
            10,
            1935
        ),
        road!(
            ENGINE_TRUCK_CRAIGHEAD_GOODS,
            VehicleKind::Truck,
            "Craighead Goods Truck",
            176,
            130,
            168,
            16,
            Some(CargoType::Goods),
            220,
            12,
            1974
        ),
        road!(
            ENGINE_TRUCK_GOSS_GOODS,
            VehicleKind::Truck,
            "Goss Goods Truck",
            224,
            140,
            240,
            18,
            Some(CargoType::Goods),
            450,
            17,
            2005
        ),
        // Trenes (clima templado del original).
        train!(
            ENGINE_TRAIN_KIRBY,
            "Kirby Paul Tank (Vapor)",
            64,
            7,
            RC_STEAM,
            50,
            0,
            None,
            300,
            47,
            1925,
            RELIABILITY_STEAM,
            2
        ),
        train!(
            ENGINE_TRAIN_CHANEY_JUBILEE,
            "Chaney 'Jubilee' (Vapor)",
            112,
            13,
            RC_STEAM,
            120,
            0,
            None,
            1_000,
            131,
            1934,
            RELIABILITY_STEAM,
            0
        ),
        train!(
            ENGINE_TRAIN_GINZU_A4,
            "Ginzu 'A4' (Vapor)",
            128,
            19,
            RC_STEAM,
            140,
            0,
            None,
            1_200,
            162,
            1935,
            RELIABILITY_STEAM,
            1
        ),
        train!(
            ENGINE_TRAIN_SH_8P,
            "SH '8P' (Vapor)",
            144,
            22,
            RC_STEAM,
            130,
            0,
            None,
            1_600,
            170,
            1954,
            RELIABILITY_STEAM,
            0
        ),
        train!(
            ENGINE_TRAIN_MANLEY_MOREL,
            "Manley-Morel DMU (Diésel)",
            112,
            11,
            RC_DIESEL,
            85,
            38,
            Some(CargoType::Passengers),
            600,
            32,
            1956,
            RELIABILITY_DIESEL,
            8
        ),
        train!(
            ENGINE_TRAIN_DASH,
            "'Dash' (Diésel)",
            120,
            14,
            RC_DIESEL,
            70,
            40,
            Some(CargoType::Passengers),
            700,
            38,
            1984,
            RELIABILITY_DIESEL,
            10
        ),
        train!(
            ENGINE_TRAIN_SH_HENDRY_25,
            "SH/Hendry '25' (Diésel)",
            128,
            15,
            RC_DIESEL,
            95,
            0,
            None,
            1_250,
            72,
            1961,
            RELIABILITY_DIESEL,
            4
        ),
        train!(
            ENGINE_TRAIN_UU_37,
            "UU '37' (Diésel)",
            144,
            17,
            RC_DIESEL,
            120,
            0,
            None,
            1_750,
            101,
            1959,
            RELIABILITY_DIESEL,
            5
        ),
        train!(
            ENGINE_TRAIN_FLOSS_47,
            "Floss '47' (Diésel)",
            160,
            18,
            RC_DIESEL,
            140,
            0,
            None,
            2_580,
            112,
            1962,
            RELIABILITY_DIESEL,
            4
        ),
        train!(
            ENGINE_TRAIN_SH_125,
            "SH '125' (Diésel)",
            200,
            20,
            RC_DIESEL,
            190,
            4,
            Some(CargoType::Mail),
            4_500,
            70,
            1977,
            RELIABILITY_DIESEL,
            6
        ),
        train!(
            ENGINE_TRAIN_SH_30,
            "SH '30' (Eléctrico)",
            160,
            26,
            RC_ELECTRIC,
            180,
            0,
            None,
            3_600,
            84,
            1965,
            RELIABILITY_ELECTRIC,
            20
        ),
        train!(
            ENGINE_TRAIN_SH_40,
            "SH '40' (Eléctrico)",
            176,
            30,
            RC_ELECTRIC,
            205,
            0,
            None,
            5_000,
            82,
            1973,
            RELIABILITY_ELECTRIC,
            20
        ),
        train!(
            ENGINE_TRAIN_TIM,
            "'T.I.M.' (Eléctrico)",
            240,
            40,
            RC_ELECTRIC,
            240,
            0,
            None,
            7_000,
            90,
            1984,
            RELIABILITY_ELECTRIC,
            21
        ),
        train!(
            ENGINE_TRAIN_ASIASTAR,
            "'AsiaStar' (Eléctrico)",
            264,
            43,
            RC_ELECTRIC,
            250,
            0,
            None,
            8_000,
            95,
            1992,
            RELIABILITY_ELECTRIC,
            23
        ),
        // Vagones (power_hp = 0): se enganchan a locomotoras.
        train!(
            ENGINE_WAGON_PASSENGER,
            "Passenger Carriage",
            0,
            20,
            RC_STEAM,
            10,
            40,
            Some(CargoType::Passengers),
            0,
            25,
            1920,
            RELIABILITY_STEAM,
            2
        ),
        train!(
            ENGINE_WAGON_MAIL,
            "Mail Van",
            0,
            18,
            RC_STEAM,
            8,
            30,
            Some(CargoType::Mail),
            0,
            20,
            1920,
            RELIABILITY_STEAM,
            2
        ),
        train!(
            ENGINE_WAGON_GOODS,
            "Goods Wagon",
            0,
            16,
            RC_STEAM,
            8,
            25,
            Some(CargoType::Goods),
            0,
            18,
            1920,
            RELIABILITY_STEAM,
            2
        ),
        train!(
            ENGINE_WAGON_COAL,
            "Coal Hopper",
            0,
            15,
            RC_STEAM,
            8,
            30,
            Some(CargoType::Coal),
            0,
            22,
            1920,
            RELIABILITY_STEAM,
            2
        ),
        // Monorail / Maglev (Fase 6; ids OpenTTD +100).
        train!(
            ENGINE_TRAIN_X2001,
            "X2001 (Monorail)",
            240,
            40,
            RC_MONORAIL,
            200,
            0,
            None,
            5_000,
            70,
            1990,
            RELIABILITY_MONORAIL,
            24
        ),
        train!(
            ENGINE_TRAIN_LEV1,
            "Lev1 (Maglev)",
            320,
            50,
            RC_MAGLEV,
            220,
            0,
            None,
            6_000,
            65,
            2000,
            RELIABILITY_MAGLEV,
            25
        ),
        road!(
            ENGINE_SHIP_MPS,
            VehicleKind::Ship,
            "MPS Channel Ferry",
            96,
            120,
            95,
            60,
            Some(CargoType::Goods),
            400,
            80,
            1920
        ),
        road!(
            ENGINE_SHIP_OIL,
            VehicleKind::Ship,
            "MPS Oil Tanker",
            80,
            140,
            110,
            90,
            Some(CargoType::Oil),
            500,
            120,
            1930
        ),
        road!(
            ENGINE_SHIP_COAL,
            VehicleKind::Ship,
            "MPS Coal Trader",
            72,
            130,
            100,
            100,
            Some(CargoType::Coal),
            450,
            110,
            1925
        ),
        road!(
            ENGINE_SHIP_FERRY,
            VehicleKind::Ship,
            "FFP Passenger Ferry",
            112,
            150,
            120,
            80,
            Some(CargoType::Passengers),
            600,
            70,
            1950
        ),
        road!(
            ENGINE_AIRCRAFT_DAKOTA,
            VehicleKind::Aircraft,
            "Dakota",
            320,
            200,
            180,
            25,
            Some(CargoType::Passengers),
            1_200,
            20,
            1944
        ),
        road!(
            ENGINE_AIRCRAFT_FOKKER,
            VehicleKind::Aircraft,
            "Fokker F27",
            380,
            240,
            200,
            40,
            Some(CargoType::Passengers),
            1_800,
            25,
            1958
        ),
        road!(
            ENGINE_AIRCRAFT_TRICARIO,
            VehicleKind::Aircraft,
            "Tricario",
            240,
            160,
            140,
            15,
            Some(CargoType::Passengers),
            800,
            8,
            1960
        ),
    ]
}

fn engines_table() -> &'static [EngineDef] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<EngineDef>> = OnceLock::new();
    TABLE.get_or_init(build_vanilla_engines).as_slice()
}

/// Catálogo vanilla (owned) para inicializar `GameState.engine_catalog`.
#[must_use]
pub fn vanilla_engine_catalog() -> Vec<EngineDef> {
    build_vanilla_engines()
}

/// Catálogo completo de motores vanilla (estático).
#[must_use]
pub fn engine_catalog() -> &'static [EngineDef] {
    engines_table()
}

/// Siguiente ID libre en el rango `NewGRF` (≥ [`NEWGRF_ENGINE_ID_BASE`]).
#[must_use]
pub fn next_free_engine_id(catalog: &[EngineDef]) -> Option<u16> {
    (NEWGRF_ENGINE_ID_BASE..=u16::MAX).find(|&id| !catalog.iter().any(|e| e.id == id))
}

/// Busca un motor en un catálogo runtime.
#[must_use]
pub fn engine_in_catalog(catalog: &[EngineDef], id: u16) -> Option<&EngineDef> {
    catalog.iter().find(|e| e.id == id)
}

/// Motores de un tipo de vehículo concreto (orden del catálogo).
pub fn engines_of_kind(kind: VehicleKind) -> impl Iterator<Item = &'static EngineDef> {
    engines_table().iter().filter(move |e| e.kind == kind)
}

/// Orden de la lista de compra en ventana de depósito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineCatalogSort {
    #[default]
    Catalog,
    Name,
    Price,
    Speed,
    IntroYear,
}

/// Filtro de carretera en ventana de compra (ignorado en depósito de vía).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoadEngineFilter {
    #[default]
    All,
    BusOnly,
    TruckOnly,
    TramOnly,
}

/// `true` si el modelo ya está disponible en el año calendario dado.
#[must_use]
pub fn engine_available_in_year(engine: &EngineDef, calendar_year: u32) -> bool {
    calendar_year >= u32::from(engine.intro_year)
}

/// Motores visibles en la ventana de compra de un depósito, filtrados y ordenados.
#[must_use]
pub fn engines_for_depot_purchase(
    depot_is_rail: bool,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&'static EngineDef> {
    engines_for_depot_kind(
        if depot_is_rail {
            DepotPurchaseKind::Rail
        } else {
            DepotPurchaseKind::Road
        },
        calendar_year,
        sort,
        road_filter,
    )
}

/// Tipo de depósito para filtrar el catálogo de compra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepotPurchaseKind {
    Rail,
    Road,
    Ship,
    Aircraft,
}

/// Motores visibles según el tipo de depósito.
#[must_use]
pub fn engines_for_depot_kind(
    depot_kind: DepotPurchaseKind,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&'static EngineDef> {
    engines_for_depot_kind_in(
        engines_table(),
        depot_kind,
        calendar_year,
        sort,
        road_filter,
    )
}

/// Como [`engines_for_depot_kind`] sobre un catálogo runtime (vanilla + `NewGRF`).
#[must_use]
pub fn engines_for_depot_kind_in(
    catalog: &[EngineDef],
    depot_kind: DepotPurchaseKind,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&EngineDef> {
    let mut list: Vec<&EngineDef> = catalog
        .iter()
        .filter(|engine| {
            if !engine_available_in_year(engine, calendar_year) {
                return false;
            }
            match (depot_kind, engine.kind) {
                (DepotPurchaseKind::Rail, VehicleKind::Train)
                | (DepotPurchaseKind::Ship, VehicleKind::Ship)
                | (DepotPurchaseKind::Aircraft, VehicleKind::Aircraft) => true,
                (DepotPurchaseKind::Road, VehicleKind::Bus) => {
                    road_filter != RoadEngineFilter::TruckOnly
                        && road_filter != RoadEngineFilter::TramOnly
                }
                (DepotPurchaseKind::Road, VehicleKind::Truck) => {
                    road_filter != RoadEngineFilter::BusOnly
                        && road_filter != RoadEngineFilter::TramOnly
                }
                (DepotPurchaseKind::Road, VehicleKind::Tram) => {
                    road_filter != RoadEngineFilter::BusOnly
                        && road_filter != RoadEngineFilter::TruckOnly
                }
                _ => false,
            }
        })
        .collect();
    match sort {
        EngineCatalogSort::Catalog => {}
        EngineCatalogSort::Name => list.sort_by_key(|e| e.name.as_str()),
        EngineCatalogSort::Price => list.sort_by_key(|e| e.price),
        EngineCatalogSort::Speed => list.sort_by_key(|e| std::cmp::Reverse(e.max_speed)),
        EngineCatalogSort::IntroYear => list.sort_by_key(|e| e.intro_year),
    }
    list
}

/// Agrupa `train_image_index` en uno de los conjuntos de sprites descargados.
#[must_use]
pub const fn train_sprite_group(image_index: u8) -> u8 {
    match image_index {
        0 | 3 | 7 | 10 => 0,
        1 | 9 => 1,
        4 | 5 | 6 | 8 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 22 => 3,
        20 | 21 | 23 => 4,
        _ => 2,
    }
}

/// Busca un motor por id, sin importar el tipo.
#[must_use]
pub fn engine_by_id(id: u16) -> Option<&'static EngineDef> {
    engines_table().iter().find(|e| e.id == id)
}

/// Tipo de humo/chispas de locomotora según fiabilidad/clase del motor.
#[must_use]
pub fn train_smoke_kind(engine_id: u16) -> crate::sim_events::TrainSmokeKind {
    let engine = engine_by_id(engine_id).unwrap_or_else(|| {
        engine_for_vehicle(VehicleKind::Train, default_engine_id(VehicleKind::Train))
    });
    match engine.reliability_pct {
        RELIABILITY_STEAM => crate::sim_events::TrainSmokeKind::Steam,
        RELIABILITY_ELECTRIC => crate::sim_events::TrainSmokeKind::Electric,
        _ => crate::sim_events::TrainSmokeKind::Diesel,
    }
}

#[must_use]
pub const fn default_engine_id(kind: VehicleKind) -> u16 {
    match kind {
        VehicleKind::Bus => ENGINE_BUS_MPS,
        VehicleKind::Truck => ENGINE_TRUCK_MPS,
        VehicleKind::Tram => ENGINE_TRAM_MPS,
        VehicleKind::Train => ENGINE_TRAIN_KIRBY,
        VehicleKind::Ship => ENGINE_SHIP_MPS,
        VehicleKind::Aircraft => ENGINE_AIRCRAFT_DAKOTA,
    }
}

/// ¿El motor aéreo es helicóptero (solo helipuertos 1×1)?
#[must_use]
pub const fn aircraft_is_helicopter(engine_id: u16) -> bool {
    engine_id == ENGINE_AIRCRAFT_TRICARIO
}

#[must_use]
pub fn engine_for_vehicle(kind: VehicleKind, id: u16) -> &'static EngineDef {
    if let Some(engine) = engines_table()
        .iter()
        .find(|engine| engine.kind == kind && engine.id == id)
    {
        return engine;
    }
    engine_for_vehicle(kind, default_engine_id(kind))
}

/// Longitud lógica de tesela (`GetAdvanceDistance` de `OpenTTD`).
#[must_use]
pub const fn tile_progress_length(direction: VehicleDirection) -> u32 {
    if direction & 1 == 1 {
        TILE_AXIAL_DISTANCE
    } else {
        TILE_CORNER_DISTANCE
    }
}

/// Actualiza `cur_speed`/`subspeed` (`GroundVehicleBase::DoUpdateSpeed`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn update_road_speed(
    cur_speed: u16,
    subspeed: u8,
    accel: u16,
    min_speed: u16,
    max_speed: u16,
) -> (u16, u8) {
    let spd = u16::from(subspeed).saturating_add(accel);
    let new_subspeed = spd as u8;
    let cur = i32::from(cur_speed);
    let max_i = i32::from(max_speed);
    let tempmax = if cur > max_i {
        std::cmp::max(cur - (cur / 10) - 1, max_i)
    } else {
        max_i
    };
    let new_cur = std::cmp::max(
        std::cmp::min(cur + i32::from(spd >> 8), tempmax),
        i32::from(min_speed),
    );
    (u16::try_from(new_cur).unwrap_or(0), new_subspeed)
}

/// Aceleración `AM_ORIGINAL` de tren (`Train::UpdateAcceleration`, `train_cmd.cpp:451`).
#[must_use]
pub fn train_acceleration(power_hp: u32, weight_t: u16) -> u8 {
    let weight = u32::from(weight_t.max(1));
    ((power_hp / weight) * 4).clamp(1, 255) as u8
}

/// Avance de velocidad de tren `AM_ORIGINAL` (`Train::UpdateSpeed`, `accel·2`).
#[must_use]
pub fn accelerate_train_speed(
    cur_speed: u16,
    subspeed: u8,
    power_hp: u32,
    weight_t: u16,
    max_speed: u16,
) -> (u16, u8) {
    let accel = u16::from(train_acceleration(power_hp, weight_t));
    let delta = accel.saturating_mul(2);
    update_road_speed(cur_speed, subspeed, delta, 0, max_speed)
}

/// Frenado de tren `AM_ORIGINAL` (`Train::UpdateSpeed`, `accel·4` hacia 0).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn decelerate_train_speed(cur_speed: u16, subspeed: u8, accel: u8) -> (u16, u8) {
    let delta = u16::from(accel).saturating_mul(4);
    let spd = u16::from(subspeed).saturating_add(delta);
    let new_subspeed = spd as u8;
    let dec = i32::from(spd >> 8);
    let new_cur = i32::from(cur_speed).saturating_sub(dec);
    let new_cur_u16 = u16::try_from(new_cur).unwrap_or(0);
    let final_sub = if new_cur_u16 == 0 { 0 } else { new_subspeed };
    (new_cur_u16, final_sub)
}

/// Frenado simétrico al acelerador original (hacia velocidad 0).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn decelerate_road_speed(cur_speed: u16, subspeed: u8) -> (u16, u8) {
    let spd = u16::from(subspeed).saturating_add(ROAD_ACCEL_ORIGINAL);
    let new_subspeed = spd as u8;
    let dec = i32::from(spd >> 8);
    let new_cur = i32::from(cur_speed).saturating_sub(dec);
    let new_cur_u16 = u16::try_from(new_cur).unwrap_or(0);
    let final_sub = if new_cur_u16 == 0 { 0 } else { new_subspeed };
    (new_cur_u16, final_sub)
}

/// Avance sub-tile por tick (`GetAdvanceSpeed` × escala a `progress` 0–255).
#[must_use]
pub fn progress_step_for_speed(max_speed: u16, direction: VehicleDirection) -> u8 {
    if max_speed == 0 {
        return 0;
    }
    let advance = u32::from(max_speed) * 3 / 4;
    let tile_len = tile_progress_length(direction);
    let reference_advance = u32::from(REFERENCE_MAX_SPEED) * 3 / 4;
    let step = advance * u32::from(REFERENCE_PROGRESS_STEP) * TILE_AXIAL_DISTANCE
        / (reference_advance * tile_len);
    if step == 0 {
        return 0;
    }
    step.clamp(1, 255) as u8
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::vehicle::DIR_SW;

    #[test]
    fn reference_bus_diagonal_tile_matches_openttd_advance_speed() {
        let step = progress_step_for_speed(112, DIR_SW);
        assert_eq!(step, REFERENCE_PROGRESS_STEP);
        let ticks = 255_u32.div_ceil(u32::from(step));
        // OpenTTD: 192 / (112*3/4) ≈ 2,3 ticks/tesela a crucero.
        assert_eq!(ticks, 3);
    }

    #[test]
    fn standstill_yields_zero_progress_step() {
        assert_eq!(progress_step_for_speed(0, DIR_SW), 0);
    }

    #[test]
    fn original_accel_reaches_max_in_reasonable_ticks() {
        let max = 112_u16;
        let mut cur = 0_u16;
        let mut sub = 0_u8;
        let mut ticks = 0_u32;
        while cur < max && ticks < 160 {
            (cur, sub) = update_road_speed(cur, sub, ROAD_ACCEL_ORIGINAL, 0, max);
            ticks += 1;
        }
        assert_eq!(cur, max);
        assert!(ticks > 1);
    }

    #[test]
    fn decelerate_from_cruise_stops_vehicle() {
        let mut cur = 112_u16;
        let mut sub = 0_u8;
        let mut ticks = 0_u32;
        while cur > 0 && ticks < 160 {
            (cur, sub) = decelerate_road_speed(cur, sub);
            ticks += 1;
        }
        assert_eq!(cur, 0);
        assert_eq!(sub, 0);
    }

    #[test]
    fn kirby_train_acceleration_matches_upstream() {
        assert_eq!(train_acceleration(300, 47), 24);
    }

    #[test]
    fn train_accel_slower_than_road_at_standstill() {
        let mut road_cur = 0_u16;
        let road_sub;
        (road_cur, road_sub) = update_road_speed(road_cur, 0, ROAD_ACCEL_ORIGINAL, 0, 64);
        let _ = road_sub;
        assert_eq!(road_cur, 1, "carretera: +1 en el primer tick");

        let mut train_cur = 0_u16;
        let mut train_sub = 0_u8;
        let mut ticks = 0_u32;
        while train_cur < 1 && ticks < 20 {
            (train_cur, train_sub) = accelerate_train_speed(train_cur, train_sub, 300, 47, 64);
            ticks += 1;
        }
        assert_eq!(train_cur, 1);
        assert!(
            ticks > 1,
            "Kirby AM_ORIGINAL tarda más que carretera en el primer +1"
        );
    }

    #[test]
    fn truck_is_slower_than_bus_train_slowest() {
        let bus = progress_step_for_speed(112, DIR_SW);
        let truck = progress_step_for_speed(96, DIR_SW);
        let train = progress_step_for_speed(64, DIR_SW);
        assert!(bus > truck);
        assert!(truck > train);
    }

    #[test]
    fn engines_for_depot_purchase_filters_by_year_and_kind() {
        let list = engines_for_depot_purchase(
            true,
            1950,
            EngineCatalogSort::Catalog,
            RoadEngineFilter::All,
        );
        assert!(list.iter().any(|e| e.id == ENGINE_TRAIN_KIRBY));
        assert!(!list.iter().any(|e| e.id == ENGINE_TRAIN_ASIASTAR));

        let road = engines_for_depot_purchase(
            false,
            1950,
            EngineCatalogSort::Catalog,
            RoadEngineFilter::BusOnly,
        );
        assert!(road.iter().all(|e| e.kind == VehicleKind::Bus));
    }

    #[test]
    fn train_sprite_group_maps_indices() {
        assert_eq!(train_sprite_group(2), 2);
        assert_eq!(train_sprite_group(23), 4);
    }
}
