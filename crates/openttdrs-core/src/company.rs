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

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
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

/// Escribe el owner de infraestructura en `m1` (vía / carretera / depósitos).
#[must_use]
pub fn tile_with_owner(mut tile: crate::map::Tile, owner: CompanyId) -> crate::map::Tile {
    tile.m1 = owner.0;
    tile
}

/// Compañía jugable o IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub colour: u8,
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
}

impl Company {
    #[must_use]
    pub fn player(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId::PLAYER,
            name: "Jugador".to_string(),
            colour,
            economy,
            is_ai: false,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
        }
    }

    #[must_use]
    pub fn rival_transcargo(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId(1),
            name: "TransCargo".to_string(),
            colour,
            economy,
            is_ai: true,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
        }
    }
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
