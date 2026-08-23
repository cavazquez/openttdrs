//! Compra de compañía rival (`CmdBuyCompany` simplificado).

use crate::GameState;
use crate::company::CompanyId;
use crate::economy::check_bankruptcy;
use crate::economy_quarterly::calculate_company_value;
use crate::map::TileKind;
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};

use super::error::CommandError;

/// Precio de compra = valoración de activos del rival (mín. 1).
#[must_use]
pub fn buy_company_price(state: &GameState, target: CompanyId) -> i64 {
    calculate_company_value(state, target).max(1)
}

/// ¿El rival está en quiebra contable (deuda supera préstamo máx.)?
#[must_use]
pub fn company_is_bankrupt(state: &GameState, company_id: CompanyId) -> bool {
    state
        .companies
        .get(company_id.index())
        .is_some_and(|c| check_bankruptcy(c.economy.money, c.economy.loan, c.economy.max_loan))
}

/// Compra la compañía `target` (no puede ser la activa).
pub(crate) fn buy_company(state: &mut GameState, target: CompanyId) -> Result<(), CommandError> {
    if target == state.active_company {
        return Err(CommandError::CannotBuyOwnCompany);
    }
    let Some(idx) = state.companies.iter().position(|c| c.id == target) else {
        return Err(CommandError::CompanyNotFound);
    };
    if !company_is_bankrupt(state, target) {
        return Err(CommandError::CompanyNotBankrupt);
    }
    let price = buy_company_price(state, target);
    if state.economy.money < price {
        return Err(CommandError::InsufficientFunds);
    }
    let name = state.companies[idx].name.clone();
    state.economy.money -= price;
    // Transferir flota y estaciones.
    for v in &mut state.vehicles {
        if v.owner == target {
            v.owner = state.active_company;
        }
    }
    for st in &mut state.stations {
        if st.owner == target {
            st.owner = state.active_company;
        }
    }
    // Infraestructura con m1 del rival → compañía activa.
    transfer_tile_owners(state, target, state.active_company);
    state.companies.remove(idx);
    // Sincronizar espejo.
    if let Some(active) = state
        .companies
        .iter_mut()
        .find(|c| c.id == state.active_company)
    {
        active.economy = state.economy;
    }
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = crate::news::NewsItem::new(
        id,
        format!("Comprada {name}"),
        Some(format!("La compañía «{name}» fue adquirida por £{price}.")),
        NewsType::CompanyInfo,
        default_display_for_type(NewsType::CompanyInfo),
        state.tick,
        NewsReference::None,
    );
    add_news_item(state, item);
    Ok(())
}

fn transfer_tile_owners(state: &mut GameState, from: CompanyId, to: CompanyId) {
    let (w, h) = state.map.dimensions();
    for y in 0..h {
        for x in 0..w {
            let pos = crate::map::TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(mut tile) = state.map.get(pos) else {
                continue;
            };
            if !matches!(
                tile.kind,
                TileKind::Rail
                    | TileKind::Road
                    | TileKind::RailDepot
                    | TileKind::RoadDepot
                    | TileKind::ShipDepot
                    | TileKind::RailBridge
                    | TileKind::RoadBridge
                    | TileKind::RailTunnel
                    | TileKind::RoadTunnel
                    | TileKind::Airport
            ) {
                continue;
            }
            if tile.m1 == from.0 {
                tile.m1 = to.0;
                let _ = state.map.set_tile(pos, tile);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Command, CompanyId, GameState, Vehicle, VehicleKind, apply_command};

    #[test]
    fn buy_bankrupt_rival_transfers_assets() {
        let mut state = GameState::new(16, 16);
        state.ensure_rival_transcargo();
        let rival = CompanyId(1);
        // Poner rival en quiebra.
        let ridx = state.companies.iter().position(|c| c.id == rival).unwrap();
        state.companies[ridx].economy.money = -2_000_000;
        state.companies[ridx].economy.max_loan = 200_000;
        state.companies[ridx].economy.max_loan_override = Some(200_000);
        state.companies[ridx].economy.loan = 200_000;

        let pos = crate::map::TileCoord::new(3, 3);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        // Infra del rival.
        if let Some(mut tile) = state.map.get(pos) {
            tile.m1 = rival.0;
            state.map.set_tile(pos, tile).unwrap();
        }
        let mut train = Vehicle::new(10, VehicleKind::Train, pos, pos);
        train.owner = rival;
        state.vehicles.push(train);
        state.economy.money = 50_000_000;

        apply_command(&mut state, &Command::BuyCompany(rival)).unwrap();
        assert!(!state.companies.iter().any(|c| c.id == rival));
        assert_eq!(state.vehicles[0].owner, CompanyId::PLAYER);
        assert_eq!(state.map.get(pos).unwrap().m1, CompanyId::PLAYER.0);
        assert!(
            state
                .news
                .items
                .iter()
                .any(|n| n.news_type == NewsType::CompanyInfo)
        );
    }

    #[test]
    fn buy_rejects_non_bankrupt() {
        let mut state = GameState::new(8, 8);
        state.ensure_rival_transcargo();
        state.economy.money = 50_000_000;
        let err = apply_command(&mut state, &Command::BuyCompany(CompanyId(1))).unwrap_err();
        assert_eq!(err, CommandError::CompanyNotBankrupt);
    }
}
