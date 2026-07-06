//! Identificadores de efectos de sonido (subset de `SoundFx` en `OpenTTD` `sound_type.h`).

/// Subconjunto prioritario de los 73 SFX de `OpenSFX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundId {
    GoodYear,
    BadYear,
    ConstructionWater,
    DepartureSteam,
    TrainThroughTunnel,
    LevelCrossing,
    Beep,
    NewsTicker,
    SkidPlane,
    TakeoffHelicopter,
    DepartureRoad,
    DepartureTrain,
    ConstructionBridge,
    ConstructionRail,
    RoadWorks,
    Explosion,
    CashTill,
    Applause,
    NewEngine,
    ConstructionOther,
    TrainCollision,
}

impl SoundId {
    /// Ruta relativa al asset root (`assets/sounds/`).
    #[must_use]
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::GoodYear => "assets/sounds/good_year.wav",
            Self::BadYear => "assets/sounds/bad_year.wav",
            Self::ConstructionWater => "assets/sounds/construction_water.wav",
            Self::DepartureSteam => "assets/sounds/departure_steam.wav",
            Self::TrainThroughTunnel => "assets/sounds/train_tunnel.wav",
            Self::LevelCrossing => "assets/sounds/level_crossing.wav",
            Self::Beep => "assets/sounds/hud_soft.wav",
            Self::ConstructionOther => "assets/sounds/construction_other.wav",
            Self::NewsTicker => "assets/sounds/news_ticker.wav",
            Self::SkidPlane => "assets/sounds/skid_plane.wav",
            Self::TakeoffHelicopter => "assets/sounds/takeoff_heli.wav",
            Self::DepartureRoad => "assets/sounds/departure_road.wav",
            Self::DepartureTrain => "assets/sounds/departure_train.wav",
            Self::ConstructionBridge => "assets/sounds/construction_bridge.wav",
            Self::ConstructionRail => "assets/sounds/construction_rail.wav",
            Self::RoadWorks => "assets/sounds/road_works.wav",
            Self::Explosion => "assets/sounds/explosion.wav",
            Self::CashTill => "assets/sounds/income.wav",
            Self::Applause => "assets/sounds/news_applause.wav",
            Self::NewEngine => "assets/sounds/news_chime.wav",
            Self::TrainCollision => "assets/sounds/train_collision.wav",
        }
    }
}
