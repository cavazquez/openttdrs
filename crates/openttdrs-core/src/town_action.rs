//! Acciones de autoridad local (`TownAction` / `CmdDoTownAction`).

use crate::company::CompanyId;
use crate::map::{TileCoord, TileKind};
use crate::station::{Station, modify_station_rating_around};
use crate::town::{
    FUND_BUILDINGS_RATING_BOOST, MAX_TOWN_AUTHORITY_COMPANIES, Town, apply_fund_buildings_boost,
};

/// Acciones de autoridad (`enum class TownAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum TownAction {
    AdvertiseSmall = 0,
    AdvertiseMedium = 1,
    AdvertiseLarge = 2,
    RoadRebuild = 3,
    BuildStatue = 4,
    FundBuildings = 5,
    BuyRights = 6,
    Bribe = 7,
}

impl TownAction {
    /// Factor de coste vanilla (`GetTownActionCost`).
    #[must_use]
    pub const fn cost_factor(self) -> u8 {
        match self {
            Self::AdvertiseSmall => 2,
            Self::AdvertiseMedium => 4,
            Self::AdvertiseLarge => 9,
            Self::RoadRebuild => 35,
            Self::BuildStatue => 48,
            Self::FundBuildings => 53,
            Self::BuyRights => 117,
            Self::Bribe => 175,
        }
    }

    /// Coste en libras internas (`factor × UNIT`, UNIT=250 → medium=1000).
    #[must_use]
    pub const fn cost(self) -> i64 {
        (self.cost_factor() as i64).saturating_mul(TOWN_ACTION_COST_UNIT)
    }

    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::AdvertiseSmall,
            Self::AdvertiseMedium,
            Self::AdvertiseLarge,
            Self::RoadRebuild,
            Self::BuildStatue,
            Self::FundBuildings,
            Self::BuyRights,
            Self::Bribe,
        ]
    }
}

/// Unidad de coste alineada con `TOWN_ADVERTISE_COST` (medium = 4×250).
pub const TOWN_ACTION_COST_UNIT: i64 = 250;

/// Boost / radio de publicidad (`ModifyStationRatingAround`).
pub const ADVERTISE_SMALL_BOOST: u8 = 0x40;
pub const ADVERTISE_SMALL_RADIUS: i32 = 10;
pub const ADVERTISE_MEDIUM_BOOST: u8 = 0x70;
pub const ADVERTISE_MEDIUM_RADIUS: i32 = 15;
pub const ADVERTISE_LARGE_BOOST: u8 = 0xA0;
pub const ADVERTISE_LARGE_RADIUS: i32 = 20;
/// Boost al comprar derechos exclusivos.
pub const BUY_RIGHTS_RATING_BOOST: u8 = 130;
pub const BUY_RIGHTS_RADIUS: i32 = 17;
/// Meses de reconstrucción vial / exclusividad / unwanted.
pub const ROAD_REBUILD_MONTHS: u8 = 6;
pub const EXCLUSIVE_RIGHTS_MONTHS: u8 = 12;
pub const BRIBE_UNWANTED_MONTHS: u8 = 6;
/// Bonus al construir una estatua (`CmdDoTownAction`): +26 en OpenTTD.
pub const BUILD_STATUE_AUTHORITY_RATING_BOOST: i16 = 26;
/// Rating al fallar soborno (`RATING_BRIBE_DOWN_TO`).
pub const RATING_BRIBE_DOWN_TO: i16 = -50;
/// Paso / tope de soborno exitoso.
pub const RATING_BRIBE_UP_STEP: i16 = 200;
pub const RATING_BRIBE_MAXIMUM: i16 = 800;

/// Ajustes de economía que habilitan acciones (defaults vanilla: todo ON).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct TownAuthoritySettings {
    #[serde(default = "default_true")]
    pub fund_buildings: bool,
    #[serde(default = "default_true")]
    pub fund_roads: bool,
    #[serde(default = "default_true")]
    pub exclusive_rights: bool,
    #[serde(default = "default_true")]
    pub bribe: bool,
}

const fn default_true() -> bool {
    true
}

impl Default for TownAuthoritySettings {
    fn default() -> Self {
        Self {
            fund_buildings: true,
            fund_roads: true,
            exclusive_rights: true,
            bribe: true,
        }
    }
}

/// Máscara de acciones disponibles para `company` en `town`.
#[must_use]
pub fn mask_of_town_actions(
    town: &Town,
    company: CompanyId,
    money: i64,
    settings: TownAuthoritySettings,
) -> u8 {
    let idx = company.index();
    if idx >= MAX_TOWN_AUTHORITY_COMPANIES {
        return 0;
    }
    if settings.bribe && town.unwanted_months(company) > 0 {
        return 0;
    }
    let mut mask = 0u8;
    for action in TownAction::all() {
        if !action_allowed(town, company, action, settings) {
            continue;
        }
        if money >= action.cost() {
            mask |= 1 << (action as u8);
        }
    }
    mask
}

fn action_allowed(
    town: &Town,
    company: CompanyId,
    action: TownAction,
    settings: TownAuthoritySettings,
) -> bool {
    match action {
        TownAction::AdvertiseSmall | TownAction::AdvertiseMedium | TownAction::AdvertiseLarge => {
            true
        }
        TownAction::RoadRebuild => settings.fund_roads && town.road_build_months == 0,
        TownAction::BuildStatue => !town.has_statue(company),
        TownAction::FundBuildings => settings.fund_buildings,
        TownAction::BuyRights => {
            settings.exclusive_rights && town.exclusive_counter == 0 && town.exclusivity.is_none()
        }
        TownAction::Bribe => {
            if !settings.bribe {
                return false;
            }
            let rating = town.authority_rating(company);
            if rating >= RATING_BRIBE_MAXIMUM {
                // Solo si otra compañía tiene exclusivos.
                return town.exclusive_counter > 0
                    && town.exclusivity.is_some_and(|c| c != company);
            }
            true
        }
    }
}

/// Ejecuta la acción (ya cobrada la tarifa por el caller).
pub fn execute_town_action(
    town: &mut Town,
    stations: &mut [Station],
    map_w: i32,
    map_h: i32,
    company: CompanyId,
    action: TownAction,
    bribe_fails: bool,
) -> Result<Option<TileCoord>, TownActionError> {
    town.ensure_authority_ratings(MAX_TOWN_AUTHORITY_COMPANIES);
    match action {
        TownAction::AdvertiseSmall => {
            let _ = modify_station_rating_around(
                stations,
                town.pos,
                company,
                ADVERTISE_SMALL_RADIUS,
                ADVERTISE_SMALL_BOOST,
            );
        }
        TownAction::AdvertiseMedium => {
            let _ = modify_station_rating_around(
                stations,
                town.pos,
                company,
                ADVERTISE_MEDIUM_RADIUS,
                ADVERTISE_MEDIUM_BOOST,
            );
        }
        TownAction::AdvertiseLarge => {
            let _ = modify_station_rating_around(
                stations,
                town.pos,
                company,
                ADVERTISE_LARGE_RADIUS,
                ADVERTISE_LARGE_BOOST,
            );
        }
        TownAction::RoadRebuild => {
            town.road_build_months = ROAD_REBUILD_MONTHS;
        }
        TownAction::BuildStatue => {
            if town.has_statue(company) {
                return Err(TownActionError::AlreadyHasStatue);
            }
            let tile =
                find_statue_tile(town.pos, map_w, map_h).ok_or(TownActionError::NoStatuePlace)?;
            town.set_statue(company, true);
            // Bonus de autoridad por estatua.
            let _ = town.adjust_rating(company, BUILD_STATUE_AUTHORITY_RATING_BOOST);
            return Ok(Some(tile));
        }
        TownAction::FundBuildings => {
            let _ = town.adjust_rating(company, FUND_BUILDINGS_RATING_BOOST);
            apply_fund_buildings_boost(town);
        }
        TownAction::BuyRights => {
            if town.exclusive_counter != 0 || town.exclusivity.is_some() {
                return Err(TownActionError::NotAvailable);
            }
            town.exclusive_counter = EXCLUSIVE_RIGHTS_MONTHS;
            town.exclusivity = Some(company);
            let _ = modify_station_rating_around(
                stations,
                town.pos,
                company,
                BUY_RIGHTS_RADIUS,
                BUY_RIGHTS_RATING_BOOST,
            );
        }
        TownAction::Bribe => {
            if bribe_fails {
                town.set_unwanted(company, BRIBE_UNWANTED_MONTHS);
                zero_company_station_ratings(stations, town, company);
                let cur = town.authority_rating(company);
                if cur > RATING_BRIBE_DOWN_TO {
                    town.set_authority_rating(company, RATING_BRIBE_DOWN_TO);
                }
            } else {
                let cur = town.authority_rating(company);
                let next = (cur.saturating_add(RATING_BRIBE_UP_STEP)).min(RATING_BRIBE_MAXIMUM);
                town.set_authority_rating(company, next);
                if town.exclusivity.is_some_and(|c| c != company) {
                    town.exclusivity = None;
                    town.exclusive_counter = 0;
                }
            }
        }
    }
    Ok(None)
}

/// Error de dominio de acciones de pueblo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownActionError {
    NotAvailable,
    AlreadyHasStatue,
    NoStatuePlace,
}

fn zero_company_station_ratings(stations: &mut [Station], town: &Town, company: CompanyId) {
    for st in stations.iter_mut() {
        if st.owner != company {
            continue;
        }
        let dx = (st.pos.x - town.pos.x).unsigned_abs();
        let dy = (st.pos.y - town.pos.y).unsigned_abs();
        if dx + dy > 40 {
            continue;
        }
        for cargo in crate::cargo::ALL_CARGO_TYPES {
            st.goods.get_mut(cargo).rating = 0;
        }
    }
}

/// Búsqueda espiral 9×9 simplificada: primera hierba/bosque libre.
fn find_statue_tile(center: TileCoord, map_w: i32, map_h: i32) -> Option<TileCoord> {
    for radius in 0_i32..=4 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius && radius != 0 {
                    continue;
                }
                let x = center.x + dx;
                let y = center.y + dy;
                if x < 0 || y < 0 || x >= map_w || y >= map_h {
                    continue;
                }
                return Some(TileCoord::new(x, y));
            }
        }
    }
    None
}

/// Decaimiento mensual de exclusivos / roads / unwanted.
pub fn tick_town_authority_months(towns: &mut [Town]) {
    for town in towns {
        if town.road_build_months > 0 {
            town.road_build_months = town.road_build_months.saturating_sub(1);
        }
        if town.exclusive_counter > 0 {
            town.exclusive_counter = town.exclusive_counter.saturating_sub(1);
            if town.exclusive_counter == 0 {
                town.exclusivity = None;
            }
        }
        for m in &mut town.unwanted {
            if *m > 0 {
                *m = m.saturating_sub(1);
            }
        }
    }
}

/// Compañía con derechos exclusivos activos, si aplica.
#[must_use]
pub fn town_exclusivity_owner(town: &Town) -> Option<CompanyId> {
    if town.exclusive_counter > 0 {
        town.exclusivity
    } else {
        None
    }
}

/// Valida tesela candidata a estatua contra el mapa (hierba/bosque).
#[must_use]
pub fn statue_tile_is_clear(kind: Option<TileKind>) -> bool {
    matches!(kind, Some(TileKind::Grass | TileKind::Forest))
}
