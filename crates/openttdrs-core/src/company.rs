//! Pool de compañías (Fase 4 estructural).
//!
//! `GameState::economy` / `company_colour` siguen siendo el espejo de la compañía
//! activa (jugador) para no romper comandos/UI. El pool `companies` es la fuente
//! de verdad multi-compañía; `sync_company_mirrors` mantiene ambos alineados.

use serde::{Deserialize, Serialize};

use crate::game_state::CompanyEconomy;
use crate::map::TileCoord;
use crate::map::TileKind;

/// Identificador de compañía (índice en `GameState::companies`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct CompanyId(pub u8);

impl CompanyId {
    pub const PLAYER: Self = Self(0);
    /// `OWNER_NONE` de `OpenTTD`: infraestructura/estaciones neutrales sin
    /// compañía propietaria (valor nativo 0x10).
    pub const NONE: Self = Self(0x10);
    /// `OWNER_TOWN` nativo. No es una compañía jugable, pero puede aparecer
    /// como propietario de infraestructura municipal importada.
    pub const TOWN: Self = Self(0x0F);
    /// `OWNER_WATER` nativo, usado por tiles de agua y boyas.
    pub const WATER: Self = Self(0x11);
    /// `OWNER_DEITY` nativo, reservado a editor/GameScript.
    pub const DEITY: Self = Self(0x12);
    /// `INVALID_OWNER` nativo (`Owner::Invalid()`).
    pub const INVALID: Self = Self(u8::MAX);

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// ¿Representa el propietario neutral (`OWNER_NONE`) de `OpenTTD`?
    #[must_use]
    pub const fn is_neutral(self) -> bool {
        self.0 == Self::NONE.0
    }

    /// ¿El byte `m1` de tesela es el owner municipal (`OWNER_TOWN` en `OpenTTD`)?
    #[must_use]
    pub const fn is_town_owner_m1(m1: u8) -> bool {
        m1 == OWNER_TOWN_M1
    }

    /// Owner de tesela desde byte `m1` (MAPO), acotado a compañías existentes.
    #[must_use]
    pub fn from_tile_m1(m1: u8, company_count: usize) -> Self {
        let idx = usize::from(m1);
        if company_count == 0 || idx >= company_count {
            Self::PLAYER
        } else {
            Self(m1)
        }
    }
}

/// Valores de propietario que `OpenTTD` serializa en `MAPO` (`m1`).
///
/// No son identificadores de compañías Rust: son los valores reservados del
/// enum `Owner` de `OpenTTD` (`OWNER_TOWN=0x0F`, `OWNER_NONE=0x10` y
/// `OWNER_WATER=0x11`). Mantenerlos separados evita que un mapa recién
/// generado parezca construido por la compañía 0.
pub const OWNER_TOWN_M1: u8 = 0x0F;
pub const OWNER_NONE_M1: u8 = 0x10;
pub const OWNER_WATER_M1: u8 = 0x11;

/// Centinela nativo `COMPANY_MAX_LOAN_DEFAULT` de `OpenTTD`.
///
/// `PLYR.max_loan` con este valor no representa un límite negativo: ordena
/// usar el límite global escalado por inflación (`_economy.max_loan`). Cualquier
/// otro valor es un override específico de la compañía, normalmente creado
/// por el comando deity `SetCompanyMaxLoan`.
pub const COMPANY_MAX_LOAN_DEFAULT: i64 = i64::MIN;

/// Escribe el owner de infraestructura en `m1` (vía / carretera / depósitos).
#[must_use]
pub fn tile_with_owner(mut tile: crate::map::Tile, owner: CompanyId) -> crate::map::Tile {
    tile.m1 = owner.0;
    tile
}

/// Nombre canónico del rival ferroviario.
pub const RIVAL_NAME_TRANSCARGO: &str = "TransCargo";
/// Nombre canónico del rival de carretera / buses.
pub const RIVAL_NAME_ROADHAUL: &str = "RoadHaul";

/// Cantidad de esquemas de librea de una compañía en `OpenTTD` (`LS_END`).
///
/// Incluye el esquema por defecto, ferrocarril, carretera, barcos, aeronaves
/// y tranvías. Se mantiene como una lista para poder cargar saves antiguos
/// que sólo serializaban una parte de los esquemas.
pub const COMPANY_LIVERY_SCHEME_COUNT: usize = 23;

/// Bit `Livery::Flag::Primary` de `OpenTTD`.
pub const COMPANY_LIVERY_FLAG_PRIMARY: u8 = 1 << 0;
/// Bit `Livery::Flag::Secondary` de `OpenTTD`.
pub const COMPANY_LIVERY_FLAG_SECONDARY: u8 = 1 << 1;

/// Índices de `LiveryScheme` tal como los serializa `OpenTTD` (`LS_END = 23`).
pub const LIVERY_SCHEME_DEFAULT: usize = 0;
pub const LIVERY_SCHEME_STEAM: usize = 1;
pub const LIVERY_SCHEME_DIESEL: usize = 2;
pub const LIVERY_SCHEME_ELECTRIC: usize = 3;
pub const LIVERY_SCHEME_MONORAIL: usize = 4;
pub const LIVERY_SCHEME_MAGLEV: usize = 5;
pub const LIVERY_SCHEME_DMU: usize = 6;
pub const LIVERY_SCHEME_EMU: usize = 7;
pub const LIVERY_SCHEME_PASSENGER_WAGON_STEAM: usize = 8;
pub const LIVERY_SCHEME_PASSENGER_WAGON_DIESEL: usize = 9;
pub const LIVERY_SCHEME_PASSENGER_WAGON_ELECTRIC: usize = 10;
pub const LIVERY_SCHEME_PASSENGER_WAGON_MONORAIL: usize = 11;
pub const LIVERY_SCHEME_PASSENGER_WAGON_MAGLEV: usize = 12;
pub const LIVERY_SCHEME_FREIGHT_WAGON: usize = 13;
pub const LIVERY_SCHEME_BUS: usize = 14;
pub const LIVERY_SCHEME_TRUCK: usize = 15;
pub const LIVERY_SCHEME_PASSENGER_SHIP: usize = 16;
pub const LIVERY_SCHEME_FREIGHT_SHIP: usize = 17;
pub const LIVERY_SCHEME_HELICOPTER: usize = 18;
pub const LIVERY_SCHEME_SMALL_PLANE: usize = 19;
pub const LIVERY_SCHEME_LARGE_PLANE: usize = 20;
pub const LIVERY_SCHEME_PASSENGER_TRAM: usize = 21;
pub const LIVERY_SCHEME_FREIGHT_TRAM: usize = 22;

/// Colores y flags de un esquema de librea de compañía (`PLYR.liveries[]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanyLivery {
    /// Canales que dejan de heredar el color del esquema por defecto.
    #[serde(default)]
    pub in_use: u8,
    /// Color primario (`Colours`).
    #[serde(default)]
    pub colour1: u8,
    /// Color secundario (`Colours`).
    #[serde(default)]
    pub colour2: u8,
}

impl CompanyLivery {
    #[must_use]
    pub const fn with_company_colour(colour: u8) -> Self {
        Self {
            in_use: 0,
            colour1: colour,
            colour2: colour,
        }
    }
}

/// Esquemas por defecto equivalentes a `ResetCompanyLivery` de `OpenTTD`.
#[must_use]
pub fn default_company_liveries(colour: u8) -> Vec<CompanyLivery> {
    vec![
        CompanyLivery::with_company_colour(colour % COMPANY_COLOUR_SLOTS);
        COMPANY_LIVERY_SCHEME_COUNT
    ]
}

/// Compañía jugable o IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // flags de settings OpenTTD (engine_renew, servint, …)
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    /// Nombre del presidente/manager (`PLYR.president_name`), si fue
    /// personalizado en `OpenTTD`.
    #[serde(default)]
    pub president_name: Option<String>,
    /// Bitfield opaco del retrato de manager (`PLYR.face`). El core no lo
    /// interpreta, pero debe conservarlo para que `OpenTTD` mantenga el rostro.
    #[serde(default)]
    pub manager_face: u32,
    /// Estilo opcional del retrato de manager (`PLYR.face_style`, SLV 355).
    ///
    /// El bitfield histórico sigue siendo la fuente de los rasgos; el estilo
    /// es una etiqueta de apariencia que `OpenTTD` guarda por separado. El core
    /// no lo interpreta, pero debe conservarlo durante un round-trip SAV.
    #[serde(default)]
    pub manager_face_style: Option<String>,
    pub colour: u8,
    /// Esquemas nativos de color de vehículos (`PLYR.liveries`).
    ///
    /// Un JSON anterior a este campo se interpreta como la librea por defecto
    /// de la compañía, por compatibilidad hacia atrás.
    #[serde(default)]
    pub liveries: Vec<CompanyLivery>,
    pub economy: CompanyEconomy,
    /// `true` = controlada por [`crate::ai::CompanyAi`].
    #[serde(default)]
    pub is_ai: bool,
    /// Ingresos acumulados por entregas (esta compañía).
    #[serde(default)]
    pub cargo_income_earned: u64,
    /// Costes de explotación de vehículos acumulados.
    #[serde(default)]
    pub vehicle_running_costs: u64,
    /// Entregas de carga acumuladas.
    #[serde(default)]
    pub cargo_deliveries: u64,
    /// Series mensuales para gráficos (Income / Operating Profit / Value).
    #[serde(default)]
    pub economy_history: crate::game_state::EconomyHistory,
    /// Series trimestrales (`CompaniesGenStatistics` / rating + valoración con activos).
    #[serde(default)]
    pub quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory,
    /// Meses consecutivos en quiebra (rivales; el jugador usa `GameState::bankruptcy_streak`).
    #[serde(default)]
    pub bankruptcy_months: u8,
    /// Autorenovación de vehículos viejos (`settings.engine_renew`).
    #[serde(default = "default_engine_renew")]
    pub engine_renew: bool,
    /// Meses antes/después de `max_age` para renovar (`settings.engine_renew_months`).
    #[serde(default = "default_engine_renew_months")]
    pub engine_renew_months: i16,
    /// Dinero mínimo a conservar al renovar (`settings.engine_renew_money`).
    #[serde(default = "default_engine_renew_money")]
    pub engine_renew_money: i64,
    /// Cabeza de la lista `EngineRenew` de `OpenTTD` (`PLYR.settings.engine_renew_list`).
    ///
    /// Es un índice de pool, no la referencia serializada `index + 1`. El
    /// writer lo mantiene para que las reglas `ERNW` importadas sigan ligadas
    /// a la empresa correcta al volver a abrir el `.sav` en `OpenTTD`.
    #[serde(default)]
    pub engine_renew_list_head: Option<u16>,
    /// Quitar vagones al autoreemplazar si el consist crece (`renew_keep_length` / wagon removal).
    #[serde(default)]
    pub renew_keep_length: bool,
    /// Intervalo de servicio en % en lugar de días (`settings.vehicle.servint_ispercent`).
    #[serde(default)]
    pub servint_ispercent: bool,
    /// Intervalos de servicio por tipo de vehículo (`settings.vehicle.*`).
    /// `0` conserva la semántica de `OpenTTD`: usar el valor del tipo de vehículo.
    #[serde(default)]
    pub servint_trains: u16,
    #[serde(default)]
    pub servint_roadveh: u16,
    #[serde(default)]
    pub servint_aircraft: u16,
    #[serde(default)]
    pub servint_ships: u16,
}

const fn default_engine_renew() -> bool {
    true
}

const fn default_engine_renew_months() -> i16 {
    6
}

const fn default_engine_renew_money() -> i64 {
    100_000
}

impl Company {
    #[must_use]
    pub fn player(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId::PLAYER,
            name: "Jugador".to_string(),
            president_name: None,
            manager_face: 0,
            manager_face_style: None,
            colour,
            liveries: default_company_liveries(colour),
            economy,
            is_ai: false,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
            engine_renew: true,
            engine_renew_months: 6,
            engine_renew_money: 100_000,
            engine_renew_list_head: None,
            renew_keep_length: false,
            servint_ispercent: false,
            servint_trains: 0,
            servint_roadveh: 0,
            servint_aircraft: 0,
            servint_ships: 0,
        }
    }

    #[must_use]
    pub fn rival_transcargo(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId(1),
            name: RIVAL_NAME_TRANSCARGO.to_string(),
            president_name: None,
            manager_face: 0,
            manager_face_style: None,
            colour,
            liveries: default_company_liveries(colour),
            economy,
            is_ai: true,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
            engine_renew: true,
            engine_renew_months: 6,
            engine_renew_money: 100_000,
            engine_renew_list_head: None,
            renew_keep_length: false,
            servint_ispercent: false,
            servint_trains: 0,
            servint_roadveh: 0,
            servint_aircraft: 0,
            servint_ships: 0,
        }
    }

    #[must_use]
    pub fn rival_roadhaul(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId(2),
            name: RIVAL_NAME_ROADHAUL.to_string(),
            president_name: None,
            manager_face: 0,
            manager_face_style: None,
            colour,
            liveries: default_company_liveries(colour),
            economy,
            is_ai: true,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
            engine_renew: true,
            engine_renew_months: 6,
            engine_renew_money: 100_000,
            engine_renew_list_head: None,
            renew_keep_length: false,
            servint_ispercent: false,
            servint_trains: 0,
            servint_roadveh: 0,
            servint_aircraft: 0,
            servint_ships: 0,
        }
    }

    /// Devuelve exactamente los 23 esquemas que `OpenTTD` escribirá en `PLYR`.
    ///
    /// Los saves antiguos pueden traer menos esquemas; los ausentes heredan
    /// el color de compañía como hace `ResetCompanyLivery` de `OpenTTD`.
    #[must_use]
    pub fn effective_liveries(&self) -> Vec<CompanyLivery> {
        let mut liveries = default_company_liveries(self.colour);
        for (target, source) in liveries.iter_mut().zip(&self.liveries) {
            *target = *source;
        }
        liveries
    }

    /// Reemplaza las libreas importadas, acotadas al contrato actual de
    /// `OpenTTD` (`LS_END = 23`).
    pub fn set_liveries(&mut self, liveries: Vec<CompanyLivery>) {
        self.liveries = liveries
            .into_iter()
            .take(COMPANY_LIVERY_SCHEME_COUNT)
            .collect();
    }

    /// Restablece todos los esquemas al color de compañía.
    pub fn reset_liveries(&mut self) {
        self.liveries = default_company_liveries(self.colour);
    }

    /// Cambia el color principal y actualiza sólo los canales que heredan el
    /// esquema por defecto, igual que `UpdateCompanyLiveries` de `OpenTTD`.
    pub fn set_colour(&mut self, colour: u8) {
        let colour = colour % COMPANY_COLOUR_SLOTS;
        let mut liveries = self.effective_liveries();
        liveries[0].colour1 = colour;
        let default_colour2 = liveries[0].colour2;
        for livery in &mut liveries[1..] {
            if livery.in_use & COMPANY_LIVERY_FLAG_PRIMARY == 0 {
                livery.colour1 = colour;
            }
            if livery.in_use & COMPANY_LIVERY_FLAG_SECONDARY == 0 {
                livery.colour2 = default_colour2;
            }
        }
        self.colour = colour;
        self.liveries = liveries;
    }
}

/// Color primario efectivo para un esquema de librea.
///
/// `OpenTTD` sólo selecciona un esquema especializado cuando el esquema por
/// defecto tiene al menos un canal marcado como personalizado. Un canal no
/// marcado hereda el color del esquema por defecto aunque el registro guarde
/// un valor antiguo distinto.
#[must_use]
pub fn company_livery_primary_colour(company: &Company, scheme: usize) -> u8 {
    company_livery_colours(company, scheme).0
}

/// Colores efectivos de una librea de compañía `(primario, secundario)`.
///
/// El esquema especializado sólo se activa cuando el esquema por defecto
/// tiene al menos un canal personalizado. Cada canal especializado que no
/// esté marcado hereda el canal correspondiente del esquema por defecto, tal
/// como `GetEngineLivery` + `UpdateCompanyLiveries` de `OpenTTD`.
#[must_use]
pub fn company_livery_colours(company: &Company, scheme: usize) -> (u8, u8) {
    let liveries = company.effective_liveries();
    let default = liveries[LIVERY_SCHEME_DEFAULT];
    if default.in_use & (COMPANY_LIVERY_FLAG_PRIMARY | COMPANY_LIVERY_FLAG_SECONDARY) == 0 {
        return (default.colour1, default.colour2);
    }
    let specialized = liveries
        .get(scheme.min(COMPANY_LIVERY_SCHEME_COUNT - 1))
        .copied()
        .unwrap_or(default);
    let primary = if specialized.in_use & COMPANY_LIVERY_FLAG_PRIMARY != 0 {
        specialized.colour1
    } else {
        default.colour1
    };
    let secondary = if specialized.in_use & COMPANY_LIVERY_FLAG_SECONDARY != 0 {
        specialized.colour2
    } else {
        default.colour2
    };
    (primary, secondary)
}

/// Color secundario efectivo de una librea de compañía.
#[must_use]
pub fn company_livery_secondary_colour(company: &Company, scheme: usize) -> u8 {
    company_livery_colours(company, scheme).1
}

/// Esquema nativo de librea para una unidad de vehículo.
///
/// `parent_engine` debe ser el motor de la cabeza del consist cuando la unidad
/// es un vagón/articulado; `OpenTTD` usa esa cabeza para las libreas de sus
/// partes. La función cubre las 23 entradas de `LiveryScheme` y deja la
/// decisión de prioridad de grupos al llamador, que conoce el pool de grupos.
#[must_use]
pub fn vehicle_livery_scheme(
    vehicle: &crate::vehicle::Vehicle,
    engine: &crate::engine::EngineDef,
    parent_engine: Option<&crate::engine::EngineDef>,
) -> usize {
    let cargo = vehicle
        .cargo_type
        .or(engine.cargo)
        .unwrap_or(crate::cargo::CargoType::Goods);
    match vehicle.kind {
        crate::vehicle::VehicleKind::Train => {
            let parent = parent_engine.unwrap_or(engine);
            let is_wagon = vehicle.is_wagon_unit() || engine.is_wagon();
            if is_wagon {
                if cargo.is_freight() {
                    return LIVERY_SCHEME_FREIGHT_WAGON;
                }
                return match parent.rail_engine_class {
                    1 if parent.rail_is_mu => LIVERY_SCHEME_DMU,
                    1 => LIVERY_SCHEME_PASSENGER_WAGON_DIESEL,
                    2 if parent.rail_is_mu => LIVERY_SCHEME_EMU,
                    2 => LIVERY_SCHEME_PASSENGER_WAGON_ELECTRIC,
                    3 => LIVERY_SCHEME_PASSENGER_WAGON_MONORAIL,
                    4 => LIVERY_SCHEME_PASSENGER_WAGON_MAGLEV,
                    _ => LIVERY_SCHEME_PASSENGER_WAGON_STEAM,
                };
            }
            match engine.rail_engine_class {
                1 if engine.rail_is_mu => LIVERY_SCHEME_DMU,
                1 => LIVERY_SCHEME_DIESEL,
                2 if engine.rail_is_mu => LIVERY_SCHEME_EMU,
                2 => LIVERY_SCHEME_ELECTRIC,
                3 => LIVERY_SCHEME_MONORAIL,
                4 => LIVERY_SCHEME_MAGLEV,
                _ => LIVERY_SCHEME_STEAM,
            }
        }
        crate::vehicle::VehicleKind::Bus => LIVERY_SCHEME_BUS,
        crate::vehicle::VehicleKind::Truck => LIVERY_SCHEME_TRUCK,
        crate::vehicle::VehicleKind::Tram => {
            if cargo.is_town_cargo() {
                LIVERY_SCHEME_PASSENGER_TRAM
            } else {
                LIVERY_SCHEME_FREIGHT_TRAM
            }
        }
        crate::vehicle::VehicleKind::Ship => {
            if cargo.is_town_cargo() {
                LIVERY_SCHEME_PASSENGER_SHIP
            } else {
                LIVERY_SCHEME_FREIGHT_SHIP
            }
        }
        crate::vehicle::VehicleKind::Aircraft => {
            if crate::engine::aircraft_is_helicopter_def(engine) {
                LIVERY_SCHEME_HELICOPTER
            } else if engine.is_large_aircraft {
                LIVERY_SCHEME_LARGE_PLANE
            } else {
                LIVERY_SCHEME_SMALL_PLANE
            }
        }
    }
}

/// Id de compañía por nombre exacto.
#[must_use]
pub fn company_id_by_name(companies: &[Company], name: &str) -> Option<CompanyId> {
    companies.iter().find(|c| c.name == name).map(|c| c.id)
}

/// Fracción del pago feeder (`_settings_game.economy.feeder_payment_share`, default 75 %).
///
/// `OpenTTD` acumula `feeder_share` por packet; aquí se acredita al owner de
/// `first_station` si difiere del destino de descarga.
pub const FEEDER_SHARE_NUM: i64 = 75;
pub const FEEDER_SHARE_DEN: i64 = 100;

/// Colores de compañía `OpenTTD` (0–15).
pub const COMPANY_COLOUR_SLOTS: u8 = 16;

/// `true` si otra compañía (≠ `except`) ya usa ese color.
#[must_use]
pub fn company_colour_taken_by_other(companies: &[Company], except: CompanyId, colour: u8) -> bool {
    let colour = colour % COMPANY_COLOUR_SLOTS;
    companies
        .iter()
        .any(|c| c.id != except && c.colour % COMPANY_COLOUR_SLOTS == colour)
}

/// Primer índice 0–15 libre en el pool; si están todos ocupados, `0`.
#[must_use]
pub fn first_free_company_colour(companies: &[Company]) -> u8 {
    let mut used = [false; COMPANY_COLOUR_SLOTS as usize];
    for c in companies {
        used[usize::from(c.colour % COMPANY_COLOUR_SLOTS)] = true;
    }
    used.iter()
        .position(|&u| !u)
        .map_or(0, |i| u8::try_from(i).unwrap_or(0))
}

/// Parte del pago que corresponde al feeder (`first_station`).
#[must_use]
pub fn feeder_share_of(payment: i64) -> i64 {
    if payment <= 0 {
        return 0;
    }
    payment.saturating_mul(FEEDER_SHARE_NUM) / FEEDER_SHARE_DEN
}

/// Resuelve el índice de color de la compañía propietaria de una tesela.
///
/// Lógica de dominio puro extraída del cliente para:
/// - Estaciones: busca station que cubre la tesela o coincide con pos
/// - Depósitos/vías/carreteras: lee owner desde `m1` del tile
/// - Otros tipos: devuelve `None`
#[must_use]
pub fn tile_owner_colour(
    companies: &[Company],
    stations: &[crate::station::Station],
    map: &crate::map::Map,
    coord: TileCoord,
    kind: TileKind,
    fallback_colour: u8,
) -> Option<u8> {
    let colour_of = |owner: CompanyId| -> u8 {
        companies
            .get(owner.index())
            .map_or(fallback_colour, |c| c.colour)
    };

    // Estación que cubre la tesela
    if let Some(station) = stations.iter().find(|s| s.covers_tile(coord)) {
        return Some(colour_of(station.owner));
    }

    // Estación cuya posición coincide con coord
    if matches!(kind, TileKind::Station | TileKind::Airport)
        && let Some(station) = stations.iter().find(|s| s.pos == coord)
    {
        return Some(colour_of(station.owner));
    }

    // Depósitos y vías/carreteras: owner en m1
    if matches!(
        kind,
        TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::ShipDepot
            | TileKind::Rail
            | TileKind::Road
    ) {
        let m1 = map.get(coord).map_or(0, |t| t.m1);
        let owner = CompanyId::from_tile_m1(m1, companies.len());
        return Some(colour_of(owner));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::CompanyEconomy;
    use crate::map::{Map, TileCoord, TileKind};
    use crate::station::Station;

    #[test]
    fn feeder_share_is_three_quarters() {
        assert_eq!(feeder_share_of(100), 75);
        assert_eq!(feeder_share_of(0), 0);
        assert_eq!(feeder_share_of(-10), 0);
    }

    #[test]
    fn first_free_company_colour_skips_taken() {
        let player = Company::player(CompanyEconomy::default(), 0);
        assert_eq!(first_free_company_colour(std::slice::from_ref(&player)), 1);
        let mut rival = Company::rival_transcargo(CompanyEconomy::default(), 1);
        rival.id = CompanyId(1);
        assert_eq!(first_free_company_colour(&[player, rival]), 2);
    }

    #[test]
    fn company_colour_taken_ignores_self() {
        let player = Company::player(CompanyEconomy::default(), 3);
        assert!(!company_colour_taken_by_other(
            std::slice::from_ref(&player),
            CompanyId::PLAYER,
            3
        ));
        let mut rival = Company::rival_transcargo(CompanyEconomy::default(), 3);
        rival.id = CompanyId(1);
        assert!(company_colour_taken_by_other(
            &[player, rival],
            CompanyId::PLAYER,
            3
        ));
    }

    #[test]
    fn changing_company_colour_preserves_custom_livery_channels() {
        let mut company = Company::player(CompanyEconomy::default(), 3);
        company.liveries[1] = CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_PRIMARY,
            colour1: 9,
            colour2: 3,
        };
        company.liveries[2] = CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_SECONDARY,
            colour1: 3,
            colour2: 12,
        };

        company.set_colour(6);

        assert_eq!(company.colour, 6);
        assert_eq!(company.liveries.len(), COMPANY_LIVERY_SCHEME_COUNT);
        assert_eq!(company.liveries[0].colour1, 6);
        assert_eq!(company.liveries[0].colour2, 3);
        assert_eq!(company.liveries[1].colour1, 9);
        assert_eq!(company.liveries[1].colour2, 3);
        assert_eq!(company.liveries[2].colour1, 6);
        assert_eq!(company.liveries[2].colour2, 12);
    }

    #[test]
    fn livery_primary_uses_default_gate_and_custom_channel() {
        let mut company = Company::player(CompanyEconomy::default(), 3);
        company.liveries[LIVERY_SCHEME_STEAM] = CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_PRIMARY,
            colour1: 9,
            colour2: 3,
        };
        assert_eq!(
            company_livery_primary_colour(&company, LIVERY_SCHEME_STEAM),
            3
        );

        company.liveries[LIVERY_SCHEME_DEFAULT].in_use = COMPANY_LIVERY_FLAG_PRIMARY;
        assert_eq!(
            company_livery_primary_colour(&company, LIVERY_SCHEME_STEAM),
            9
        );
        assert_eq!(
            company_livery_primary_colour(&company, LIVERY_SCHEME_DIESEL),
            3
        );
    }

    #[test]
    fn livery_secondary_inherits_default_until_custom_channel_is_enabled() {
        let mut company = Company::player(CompanyEconomy::default(), 3);
        company.liveries[LIVERY_SCHEME_DEFAULT] = CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_PRIMARY | COMPANY_LIVERY_FLAG_SECONDARY,
            colour1: 4,
            colour2: 5,
        };
        company.liveries[LIVERY_SCHEME_STEAM] = CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_PRIMARY,
            colour1: 9,
            colour2: 12,
        };
        assert_eq!(
            company_livery_colours(&company, LIVERY_SCHEME_STEAM),
            (9, 5)
        );
        assert_eq!(
            company_livery_secondary_colour(&company, LIVERY_SCHEME_STEAM),
            5
        );
        company.liveries[LIVERY_SCHEME_STEAM].in_use |= COMPANY_LIVERY_FLAG_SECONDARY;
        assert_eq!(
            company_livery_colours(&company, LIVERY_SCHEME_STEAM),
            (9, 12)
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn vehicle_livery_scheme_matches_train_class_and_cargo() {
        let mut engine = crate::Vehicle::new(
            1,
            crate::vehicle::VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        engine.cargo_type = Some(crate::cargo::CargoType::Passengers);
        let steam = crate::engine::engine_by_id(crate::engine::ENGINE_TRAIN_KIRBY).unwrap();
        assert_eq!(
            vehicle_livery_scheme(&engine, steam, None),
            LIVERY_SCHEME_STEAM
        );

        let dmu = crate::engine::engine_by_id(crate::engine::ENGINE_TRAIN_MANLEY_MOREL).unwrap();
        assert_eq!(vehicle_livery_scheme(&engine, dmu, None), LIVERY_SCHEME_DMU);

        engine.prev_unit = Some(7);
        let wagon = crate::engine::engine_by_id(crate::engine::ENGINE_WAGON_PASSENGER).unwrap();
        assert_eq!(
            vehicle_livery_scheme(&engine, wagon, Some(dmu)),
            LIVERY_SCHEME_DMU
        );
        engine.cargo_type = Some(crate::cargo::CargoType::Coal);
        assert_eq!(
            vehicle_livery_scheme(&engine, wagon, Some(dmu)),
            LIVERY_SCHEME_FREIGHT_WAGON
        );
    }

    #[test]
    fn tile_owner_colour_returns_none_for_irrelevant_tiles() {
        let companies = vec![Company::player(CompanyEconomy::default(), 5)];
        let stations = vec![];
        let map = Map::new_flat(64, 64, 0);
        let coord = TileCoord::new(10, 10);

        assert_eq!(
            tile_owner_colour(&companies, &stations, &map, coord, TileKind::Grass, 0),
            None
        );
        assert_eq!(
            tile_owner_colour(&companies, &stations, &map, coord, TileKind::Water, 0),
            None
        );
        assert_eq!(
            tile_owner_colour(&companies, &stations, &map, coord, TileKind::Forest, 0),
            None
        );
    }

    #[test]
    fn tile_owner_colour_reads_m1_for_rail() {
        let companies = vec![
            Company::player(CompanyEconomy::default(), 5),
            Company::rival_transcargo(CompanyEconomy::default(), 12),
        ];
        let stations = vec![];
        let mut map = Map::new_flat(64, 64, 0);
        let coord = TileCoord::new(10, 10);

        // CompanyId(1) = TransCargo
        let _ = map.set_m1(coord, 1);

        let colour = tile_owner_colour(&companies, &stations, &map, coord, TileKind::Rail, 0);
        assert_eq!(colour, Some(12));
    }

    #[test]
    fn tile_owner_colour_finds_station_covering_tile() {
        let companies = vec![Company::player(CompanyEconomy::default(), 7)];
        let coord = TileCoord::new(10, 10);
        let mut station = Station::new(coord);
        station.owner = CompanyId::PLAYER;
        // Simular que la estación cubre coord (la implementación real depende de covers_tile)
        let stations = vec![station];
        let map = Map::new_flat(64, 64, 0);

        let colour = tile_owner_colour(&companies, &stations, &map, coord, TileKind::Road, 0);
        // Si covers_tile devuelve true para su propia pos
        assert_eq!(colour, Some(7));
    }

    #[test]
    fn tile_owner_colour_matches_station_pos() {
        let companies = vec![Company::player(CompanyEconomy::default(), 9)];
        let coord = TileCoord::new(15, 20);
        let mut station = Station::new(coord);
        station.owner = CompanyId::PLAYER;
        let stations = vec![station];
        let map = Map::new_flat(64, 64, 0);

        let colour = tile_owner_colour(&companies, &stations, &map, coord, TileKind::Station, 0);
        assert_eq!(colour, Some(9));
    }

    #[test]
    fn tile_owner_colour_uses_fallback_for_invalid_owner() {
        let companies = vec![];
        let stations = vec![];
        let mut map = Map::new_flat(64, 64, 0);
        let coord = TileCoord::new(10, 10);

        // owner inválido
        let _ = map.set_m1(coord, 5);

        let colour = tile_owner_colour(&companies, &stations, &map, coord, TileKind::Rail, 3);
        assert_eq!(colour, Some(3)); // fallback
    }
}
