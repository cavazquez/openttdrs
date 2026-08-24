//! Identificadores de efectos de sonido (`SoundFx` en `OpenTTD` `sound_type.h`).
//!
//! Orden = índice TTD/NewGRF (0..72). Los WAV en disco usan `snd_XX.wav` (XX = este
//! índice). La tabla [`SOUND_IDX`] traduce al slot del `.cat` `OpenSFX` (`osfx_NN`).

/// Cantidad de samples del baseset original.
pub const SOUND_COUNT: usize = 73;

/// Traducción `SoundFx` → índice en el catálogo `OpenSFX` (`sound.cpp` `_sound_idx`).
pub const SOUND_IDX: [u8; SOUND_COUNT] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27,
    28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 0, 1, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72,
];

/// Volumen base 0..=255 (`sound.cpp` `_sound_base_vol`); 128 ≈ 1.0.
pub const SOUND_BASE_VOL: [u8; SOUND_COUNT] = [
    128, 90, 128, 128, 128, 128, 128, 128, 128, 90, 90, 128, 128, 128, 128, 128, 128, 128, 128, 80,
    128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 90, 90, 90, 128, 90, 128,
    128, 90, 128, 128, 128, 90, 128, 128, 128, 128, 128, 128, 90, 128, 128, 128, 128, 90, 128, 128,
    128, 128, 128, 128, 128, 128, 90, 90, 90, 128, 128, 128, 90,
];

/// Catálogo completo de 73 SFX `OpenTTD` / `OpenSFX`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundId {
    ConstructionWater = 0,
    Factory = 1,
    DepartureSteam = 2,
    TrainThroughTunnel = 3,
    DepartureCargoShip = 4,
    DepartureFerry = 5,
    TakeoffPropeller = 6,
    TakeoffJet = 7,
    DepartureTrain = 8,
    Mine = 9,
    PowerStation = 10,
    Unused0d = 11,
    LevelCrossing = 12,
    BreakdownRoad = 13,
    BreakdownTrainShip = 14,
    Unused11 = 15,
    Explosion = 16,
    TrainCollision = 17,
    CashTill = 18,
    Beep = 19,
    NewsTicker = 20,
    SkidPlane = 21,
    TakeoffHelicopter = 22,
    DepartureOldRv1 = 23,
    DepartureOldRv2 = 24,
    DepartureModernBus = 25,
    DepartureOldBus = 26,
    Applause = 27,
    NewEngine = 28,
    ConstructionOther = 29,
    ConstructionRail = 30,
    RoadWorks = 31,
    Unused22 = 32,
    Unused23 = 33,
    Farm1 = 34,
    Farm2 = 35,
    Farm3 = 36,
    ConstructionBridge = 37,
    Sawmill = 38,
    GoodYear = 39,
    BadYear = 40,
    SugarMine2 = 41,
    ToyFactory3 = 42,
    ToyFactory2 = 43,
    ToyFactory1 = 44,
    SugarMine1 = 45,
    BubbleGenerator = 46,
    BubbleGeneratorFail = 47,
    ToffeeQuarry = 48,
    BubbleGeneratorSuccess = 49,
    Unused32 = 50,
    PlasticMine = 51,
    ArcticSnow1 = 52,
    BreakdownRoadToyland = 53,
    LumberMill3 = 54,
    LumberMill2 = 55,
    LumberMill1 = 56,
    ArcticSnow2 = 57,
    BreakdownTrainShipToyland = 58,
    TakeoffJetFast = 59,
    DepartureBusToyland1 = 60,
    TakeoffJetBig = 61,
    DepartureBusToyland2 = 62,
    DepartureTruckToyland1 = 63,
    DepartureTruckToyland2 = 64,
    DepartureMaglev = 65,
    Rainforest1 = 66,
    Rainforest2 = 67,
    Rainforest3 = 68,
    TakeoffPropellerToyland1 = 69,
    TakeoffPropellerToyland2 = 70,
    DepartureMonorail = 71,
    Rainforest4 = 72,
}

/// Catálogo `OpenSFX` completo (`SoundFx` 0..72).
impl SoundId {
    /// Todos los índices 0..72.
    pub const ALL: [Self; SOUND_COUNT] = [
        Self::ConstructionWater,
        Self::Factory,
        Self::DepartureSteam,
        Self::TrainThroughTunnel,
        Self::DepartureCargoShip,
        Self::DepartureFerry,
        Self::TakeoffPropeller,
        Self::TakeoffJet,
        Self::DepartureTrain,
        Self::Mine,
        Self::PowerStation,
        Self::Unused0d,
        Self::LevelCrossing,
        Self::BreakdownRoad,
        Self::BreakdownTrainShip,
        Self::Unused11,
        Self::Explosion,
        Self::TrainCollision,
        Self::CashTill,
        Self::Beep,
        Self::NewsTicker,
        Self::SkidPlane,
        Self::TakeoffHelicopter,
        Self::DepartureOldRv1,
        Self::DepartureOldRv2,
        Self::DepartureModernBus,
        Self::DepartureOldBus,
        Self::Applause,
        Self::NewEngine,
        Self::ConstructionOther,
        Self::ConstructionRail,
        Self::RoadWorks,
        Self::Unused22,
        Self::Unused23,
        Self::Farm1,
        Self::Farm2,
        Self::Farm3,
        Self::ConstructionBridge,
        Self::Sawmill,
        Self::GoodYear,
        Self::BadYear,
        Self::SugarMine2,
        Self::ToyFactory3,
        Self::ToyFactory2,
        Self::ToyFactory1,
        Self::SugarMine1,
        Self::BubbleGenerator,
        Self::BubbleGeneratorFail,
        Self::ToffeeQuarry,
        Self::BubbleGeneratorSuccess,
        Self::Unused32,
        Self::PlasticMine,
        Self::ArcticSnow1,
        Self::BreakdownRoadToyland,
        Self::LumberMill3,
        Self::LumberMill2,
        Self::LumberMill1,
        Self::ArcticSnow2,
        Self::BreakdownTrainShipToyland,
        Self::TakeoffJetFast,
        Self::DepartureBusToyland1,
        Self::TakeoffJetBig,
        Self::DepartureBusToyland2,
        Self::DepartureTruckToyland1,
        Self::DepartureTruckToyland2,
        Self::DepartureMaglev,
        Self::Rainforest1,
        Self::Rainforest2,
        Self::Rainforest3,
        Self::TakeoffPropellerToyland1,
        Self::TakeoffPropellerToyland2,
        Self::DepartureMonorail,
        Self::Rainforest4,
    ];

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        if (value as usize) >= SOUND_COUNT {
            return None;
        }
        Some(Self::ALL[value as usize])
    }

    #[must_use]
    pub const fn is_unused(self) -> bool {
        matches!(
            self,
            Self::Unused0d | Self::Unused11 | Self::Unused22 | Self::Unused23 | Self::Unused32
        )
    }

    /// Índice en el archivo `opensfx.cat` / `osfx_NN.wav`.
    #[must_use]
    pub const fn opensfx_index(self) -> u8 {
        SOUND_IDX[self as usize]
    }

    /// Factor de volumen baseset (1.0 = 128).
    #[must_use]
    pub const fn base_volume_factor(self) -> f32 {
        SOUND_BASE_VOL[self as usize] as f32 / 128.0
    }

    /// Ruta relativa al asset root (`assets/sounds/snd_XX.wav`).
    #[must_use]
    pub const fn asset_path(self) -> &'static str {
        ASSET_PATHS[self as usize]
    }

    /// Sonido de salida de estación / arranque según tipo de vehículo.
    #[must_use]
    pub const fn departure_for_kind(kind: crate::vehicle::VehicleKind) -> Self {
        use crate::vehicle::VehicleKind;
        match kind {
            VehicleKind::Train => Self::DepartureTrain,
            VehicleKind::Truck | VehicleKind::Tram => Self::DepartureOldRv1,
            VehicleKind::Bus => Self::DepartureModernBus,
            VehicleKind::Ship => Self::DepartureFerry,
            VehicleKind::Aircraft => Self::TakeoffPropeller,
        }
    }

    /// Sonido vanilla de salida según el modelo, no sólo el tipo de vehículo.
    ///
    /// `OpenTTD` selecciona la entrada `sfx` del catálogo: buses antiguos,
    /// modernos, monorail y maglev no comparten la misma muestra aunque todos
    /// sean vehículos de la misma familia.
    #[must_use]
    pub const fn departure_for_engine_id(engine_id: u16) -> Option<Self> {
        use crate::engine::{
            ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_FOKKER, ENGINE_AIRCRAFT_TRICARIO,
            ENGINE_BUS_FOSTER, ENGINE_BUS_HEREFORD, ENGINE_BUS_MPS, ENGINE_SHIP_FERRY,
            ENGINE_SHIP_MPS, ENGINE_SHIP_OIL, ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_CHANEY_JUBILEE,
            ENGINE_TRAIN_GINZU_A4, ENGINE_TRAIN_KIRBY, ENGINE_TRAIN_LEV1,
            ENGINE_TRAIN_MANLEY_MOREL, ENGINE_TRAIN_SH_8P, ENGINE_TRAIN_X2001, ENGINE_TRAM_MPS,
            ENGINE_TRUCK_MPS,
        };
        match engine_id {
            ENGINE_TRAIN_KIRBY
            | ENGINE_TRAIN_CHANEY_JUBILEE
            | ENGINE_TRAIN_GINZU_A4
            | ENGINE_TRAIN_SH_8P => Some(Self::DepartureSteam),
            ENGINE_TRAIN_X2001 => Some(Self::DepartureMonorail),
            ENGINE_TRAIN_LEV1 => Some(Self::DepartureMaglev),
            ENGINE_TRAIN_ASIASTAR | ENGINE_TRAIN_MANLEY_MOREL => Some(Self::DepartureTrain),
            ENGINE_BUS_MPS | ENGINE_TRAM_MPS | ENGINE_TRUCK_MPS => Some(Self::DepartureOldRv1),
            ENGINE_BUS_HEREFORD => Some(Self::DepartureOldBus),
            ENGINE_BUS_FOSTER => Some(Self::DepartureModernBus),
            ENGINE_SHIP_MPS | ENGINE_SHIP_OIL => Some(Self::DepartureCargoShip),
            ENGINE_SHIP_FERRY => Some(Self::DepartureFerry),
            ENGINE_AIRCRAFT_DAKOTA => Some(Self::TakeoffPropeller),
            ENGINE_AIRCRAFT_FOKKER => Some(Self::TakeoffJet),
            ENGINE_AIRCRAFT_TRICARIO => Some(Self::TakeoffHelicopter),
            _ => None,
        }
    }

    /// Sonido de avería según tipo.
    #[must_use]
    pub const fn breakdown_for_kind(kind: crate::vehicle::VehicleKind) -> Self {
        use crate::vehicle::VehicleKind;
        match kind {
            VehicleKind::Truck | VehicleKind::Bus | VehicleKind::Tram => Self::BreakdownRoad,
            VehicleKind::Train | VehicleKind::Ship => Self::BreakdownTrainShip,
            VehicleKind::Aircraft => Self::Beep,
        }
    }

    /// Proxy de motor en marcha (baseset; sin `NewGRF` Action11).
    #[must_use]
    pub const fn running_for_kind(kind: crate::vehicle::VehicleKind) -> Self {
        use crate::vehicle::VehicleKind;
        match kind {
            VehicleKind::Train => Self::DepartureTrain,
            VehicleKind::Truck | VehicleKind::Tram => Self::DepartureOldRv1,
            VehicleKind::Bus => Self::DepartureModernBus,
            VehicleKind::Ship => Self::DepartureCargoShip,
            VehicleKind::Aircraft => Self::TakeoffJet,
        }
    }
}

const ASSET_PATHS: [&str; SOUND_COUNT] = [
    "assets/sounds/snd_00.wav",
    "assets/sounds/snd_01.wav",
    "assets/sounds/snd_02.wav",
    "assets/sounds/snd_03.wav",
    "assets/sounds/snd_04.wav",
    "assets/sounds/snd_05.wav",
    "assets/sounds/snd_06.wav",
    "assets/sounds/snd_07.wav",
    "assets/sounds/snd_08.wav",
    "assets/sounds/snd_09.wav",
    "assets/sounds/snd_10.wav",
    "assets/sounds/snd_11.wav",
    "assets/sounds/snd_12.wav",
    "assets/sounds/snd_13.wav",
    "assets/sounds/snd_14.wav",
    "assets/sounds/snd_15.wav",
    "assets/sounds/snd_16.wav",
    "assets/sounds/snd_17.wav",
    "assets/sounds/snd_18.wav",
    "assets/sounds/snd_19.wav",
    "assets/sounds/snd_20.wav",
    "assets/sounds/snd_21.wav",
    "assets/sounds/snd_22.wav",
    "assets/sounds/snd_23.wav",
    "assets/sounds/snd_24.wav",
    "assets/sounds/snd_25.wav",
    "assets/sounds/snd_26.wav",
    "assets/sounds/snd_27.wav",
    "assets/sounds/snd_28.wav",
    "assets/sounds/snd_29.wav",
    "assets/sounds/snd_30.wav",
    "assets/sounds/snd_31.wav",
    "assets/sounds/snd_32.wav",
    "assets/sounds/snd_33.wav",
    "assets/sounds/snd_34.wav",
    "assets/sounds/snd_35.wav",
    "assets/sounds/snd_36.wav",
    "assets/sounds/snd_37.wav",
    "assets/sounds/snd_38.wav",
    "assets/sounds/snd_39.wav",
    "assets/sounds/snd_40.wav",
    "assets/sounds/snd_41.wav",
    "assets/sounds/snd_42.wav",
    "assets/sounds/snd_43.wav",
    "assets/sounds/snd_44.wav",
    "assets/sounds/snd_45.wav",
    "assets/sounds/snd_46.wav",
    "assets/sounds/snd_47.wav",
    "assets/sounds/snd_48.wav",
    "assets/sounds/snd_49.wav",
    "assets/sounds/snd_50.wav",
    "assets/sounds/snd_51.wav",
    "assets/sounds/snd_52.wav",
    "assets/sounds/snd_53.wav",
    "assets/sounds/snd_54.wav",
    "assets/sounds/snd_55.wav",
    "assets/sounds/snd_56.wav",
    "assets/sounds/snd_57.wav",
    "assets/sounds/snd_58.wav",
    "assets/sounds/snd_59.wav",
    "assets/sounds/snd_60.wav",
    "assets/sounds/snd_61.wav",
    "assets/sounds/snd_62.wav",
    "assets/sounds/snd_63.wav",
    "assets/sounds/snd_64.wav",
    "assets/sounds/snd_65.wav",
    "assets/sounds/snd_66.wav",
    "assets/sounds/snd_67.wav",
    "assets/sounds/snd_68.wav",
    "assets/sounds/snd_69.wav",
    "assets/sounds/snd_70.wav",
    "assets/sounds/snd_71.wav",
    "assets/sounds/snd_72.wav",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_seventy_three_entries() {
        assert_eq!(SoundId::ALL.len(), SOUND_COUNT);
        assert_eq!(SOUND_IDX.len(), SOUND_COUNT);
        assert_eq!(SOUND_BASE_VOL.len(), SOUND_COUNT);
        for (i, id) in SoundId::ALL.iter().enumerate() {
            assert_eq!(id.as_u8() as usize, i);
            let Ok(idx) = u8::try_from(i) else {
                panic!("SoundId index fits in u8");
            };
            assert_eq!(SoundId::from_u8(idx), Some(*id));
        }
    }

    #[test]
    fn sound_idx_matches_openttd() {
        assert_eq!(SOUND_IDX[0], 2);
        assert_eq!(SOUND_IDX[39], 0); // GoodYear
        assert_eq!(SOUND_IDX[40], 1); // BadYear
        assert_eq!(SoundId::GoodYear.opensfx_index(), 0);
        assert_eq!(SoundId::ConstructionWater.opensfx_index(), 2);
    }

    #[test]
    fn departure_helpers() {
        assert_eq!(
            SoundId::departure_for_kind(crate::vehicle::VehicleKind::Bus),
            SoundId::DepartureModernBus
        );
        assert_eq!(
            SoundId::departure_for_kind(crate::vehicle::VehicleKind::Truck),
            SoundId::DepartureOldRv1
        );
        assert_eq!(
            SoundId::departure_for_engine_id(crate::engine::ENGINE_BUS_HEREFORD),
            Some(SoundId::DepartureOldBus)
        );
        assert_eq!(
            SoundId::departure_for_engine_id(crate::engine::ENGINE_TRAIN_X2001),
            Some(SoundId::DepartureMonorail)
        );
        assert_eq!(SoundId::departure_for_engine_id(0xFFFF), None);
    }
}
