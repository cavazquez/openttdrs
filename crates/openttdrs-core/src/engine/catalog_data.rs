//! Catálogo vanilla estático de motores `OpenGFX`.

use std::sync::OnceLock;

use crate::cargo::CargoType;
use crate::vehicle::VehicleKind;

use super::model::{DEFAULT_RELIABILITY_SPD_DEC, EngineDef, SHIP_RELIABILITY_SPD_DEC};

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

/// Fiabilidad inicial aproximada por clase de motor del original.
pub(crate) const RELIABILITY_STEAM: u8 = 75;
pub(crate) const RELIABILITY_DIESEL: u8 = 85;
pub(crate) const RELIABILITY_ELECTRIC: u8 = 90;
pub(crate) const RELIABILITY_ROAD: u8 = 85;
macro_rules! road {
    ($id:expr, $kind:expr, $name:expr, $speed:expr, $cf:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr) => {
        road!(
            $id,
            $kind,
            $name,
            $speed,
            $cf,
            $rc,
            $cap,
            $cargo,
            $hp,
            $wt,
            $year,
            40,
            DEFAULT_RELIABILITY_SPD_DEC
        )
    };
    ($id:expr, $kind:expr, $name:expr, $speed:expr, $cf:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $life:expr) => {
        road!(
            $id,
            $kind,
            $name,
            $speed,
            $cf,
            $rc,
            $cap,
            $cargo,
            $hp,
            $wt,
            $year,
            $life,
            DEFAULT_RELIABILITY_SPD_DEC
        )
    };
    ($id:expr, $kind:expr, $name:expr, $speed:expr, $cf:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $life:expr, $spd_dec:expr) => {
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
            reliability_spd_dec: $spd_dec,
            lifelength_years: $life,
            model_life_years: u8::MAX,
            load_amount: 0,
            train_image_index: 0,
            dual_headed: false,
            rail_tilts: false,
            curve_speed_mod: 0,
            pow_wag_power: 0,
            pow_wag_weight: 0,
            from_newgrf: false,
            tractive_effort: 0,
            air_drag: 0,
            shorten_factor: 0,
            required_rail_type: None,
            refit_mask: 0,
            is_helicopter: false,
            is_large_aircraft: false,
            ocean_speed_frac: 0,
            canal_speed_frac: 0,
            sound_effect: 0,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: None,
            newgrf_grfid: 0,
        }
    };
}

macro_rules! train {
    ($id:expr, $name:expr, $speed:expr, $cf:expr, $rc_base:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $rel:expr, $img:expr) => {
        train!(
            $id, $name, $speed, $cf, $rc_base, $rc, $cap, $cargo, $hp, $wt, $year, $rel, $img, 30,
            false
        )
    };
    ($id:expr, $name:expr, $speed:expr, $cf:expr, $rc_base:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $rel:expr, $img:expr, $life:expr) => {
        train!(
            $id, $name, $speed, $cf, $rc_base, $rc, $cap, $cargo, $hp, $wt, $year, $rel, $img,
            $life, false
        )
    };
    ($id:expr, $name:expr, $speed:expr, $cf:expr, $rc_base:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $rel:expr, $img:expr, $dual:expr) => {
        train!(
            $id, $name, $speed, $cf, $rc_base, $rc, $cap, $cargo, $hp, $wt, $year, $rel, $img, 30,
            $dual
        )
    };
    ($id:expr, $name:expr, $speed:expr, $cf:expr, $rc_base:expr, $rc:expr, $cap:expr, $cargo:expr, $hp:expr, $wt:expr, $year:expr, $rel:expr, $img:expr, $life:expr, $dual:expr) => {
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
            reliability_spd_dec: DEFAULT_RELIABILITY_SPD_DEC,
            lifelength_years: $life,
            model_life_years: u8::MAX,
            load_amount: 0,
            train_image_index: $img,
            dual_headed: $dual,
            rail_tilts: false,
            curve_speed_mod: 0,
            pow_wag_power: 0,
            pow_wag_weight: 0,
            from_newgrf: false,
            tractive_effort: 0,
            air_drag: 0,
            shorten_factor: 0,
            required_rail_type: None,
            refit_mask: 0,
            is_helicopter: false,
            is_large_aircraft: false,
            ocean_speed_frac: 0,
            canal_speed_frac: 0,
            sound_effect: 0,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: None,
            newgrf_grfid: 0,
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
            1964,
            30
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
            1935,
            55
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
            1935,
            55
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
            1974,
            85
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
            2005,
            85
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
            2,
            30
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
            0,
            25
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
            8,
            35,
            true
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
            10,
            35,
            true
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
            4,
            28
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
            4,
            33
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
            6,
            25,
            true
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
            20,
            80
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
            21,
            30,
            true
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
            23,
            50,
            true
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
            33,
            50
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
            35,
            50
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
            21, // OpenTTD RVI weight (engine 32 Goods Van)
            1920,
            RELIABILITY_STEAM,
            38,
            50
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
            34,
            50
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
            24,
            50
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
            25,
            50
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
            1920,
            30,
            SHIP_RELIABILITY_SPD_DEC
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
            1930,
            30,
            SHIP_RELIABILITY_SPD_DEC
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
            1925,
            30,
            SHIP_RELIABILITY_SPD_DEC
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
            1950,
            90,
            SHIP_RELIABILITY_SPD_DEC
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
            1944,
            20
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
            1958,
            24
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
            1960,
            25
        ),
    ]
}

pub(crate) fn engines_table() -> &'static [EngineDef] {
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
