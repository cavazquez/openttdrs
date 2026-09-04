//! Fachada Rust del contrato `ScriptCargoMonitor` de `OpenTTD`.
//!
//! La simulación mantiene el estado en [`crate::cargo_monitor::CargoMonitor`];
//! este tipo sólo reproduce la superficie que usaría un `GameScript` y sus
//! comprobaciones de límites. Las cantidades no se serializan y cada consulta
//! conserva la semántica de lectura/reset del runtime nativo.

use crate::{CargoType, CompanyId, GameState};

/// API equivalente a los métodos estáticos de `ScriptCargoMonitor`.
pub struct ScriptCargoMonitor;

impl ScriptCargoMonitor {
    /// Equivalente a `GetTownDeliveryAmount`.
    pub fn get_town_delivery_amount(
        state: &mut GameState,
        company: CompanyId,
        cargo: CargoType,
        town_id: u32,
        keep_monitoring: bool,
    ) -> i32 {
        state.get_town_delivery_amount(company, cargo, town_id, keep_monitoring)
    }

    /// Equivalente a `GetIndustryDeliveryAmount`.
    pub fn get_industry_delivery_amount(
        state: &mut GameState,
        company: CompanyId,
        cargo: CargoType,
        industry_id: u16,
        keep_monitoring: bool,
    ) -> i32 {
        state.get_industry_delivery_amount(company, cargo, industry_id, keep_monitoring)
    }

    /// Equivalente a `GetTownPickupAmount`.
    pub fn get_town_pickup_amount(
        state: &mut GameState,
        company: CompanyId,
        cargo: CargoType,
        town_id: u32,
        keep_monitoring: bool,
    ) -> i32 {
        state.get_town_pickup_amount(company, cargo, town_id, keep_monitoring)
    }

    /// Equivalente a `GetIndustryPickupAmount`.
    pub fn get_industry_pickup_amount(
        state: &mut GameState,
        company: CompanyId,
        cargo: CargoType,
        industry_id: u16,
        keep_monitoring: bool,
    ) -> i32 {
        state.get_industry_pickup_amount(company, cargo, industry_id, keep_monitoring)
    }

    /// Equivalente a `StopAllMonitoring`.
    pub fn stop_all_monitoring(state: &mut GameState) {
        state.stop_all_cargo_monitoring();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CargoSpecDef, Industry, IndustryKind, TileCoord, Town, cargo_monitor::CargoSource,
    };

    #[test]
    fn rejects_invalid_script_parameters_without_activating_monitor() {
        let mut state = GameState::new(8, 8);
        state.towns.push(Town {
            id: 4,
            pos: TileCoord::new(2, 2),
            ..Town::default()
        });
        state
            .industries
            .push(Industry::new(TileCoord::new(4, 4), IndustryKind::CoalMine).with_instance_id(7));

        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                CargoType::Passengers,
                4,
                true,
            ),
            0
        );
        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId(crate::company::MAX_COMPANIES),
                CargoType::Passengers,
                4,
                true,
            ),
            -1
        );
        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                CargoType::Wheat,
                4,
                true,
            ),
            -1
        );
        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                CargoType::Passengers,
                99,
                true,
            ),
            -1
        );
        assert_eq!(
            ScriptCargoMonitor::get_industry_pickup_amount(
                &mut state,
                CompanyId::PLAYER,
                CargoType::Coal,
                99,
                true,
            ),
            -1
        );
        assert_eq!(state.runtime.cargo_monitor.active_counts(), (0, 1));
    }

    #[test]
    fn stop_all_monitoring_clears_pickups_and_deliveries() {
        let mut state = GameState::new(8, 8);
        state.towns.push(Town {
            id: 4,
            pos: TileCoord::new(2, 2),
            ..Town::default()
        });
        state
            .industries
            .push(Industry::new(TileCoord::new(4, 4), IndustryKind::CoalMine).with_instance_id(7));
        let _ = ScriptCargoMonitor::get_town_delivery_amount(
            &mut state,
            CompanyId::PLAYER,
            CargoType::Passengers,
            4,
            true,
        );
        let _ = ScriptCargoMonitor::get_industry_pickup_amount(
            &mut state,
            CompanyId::PLAYER,
            CargoType::Coal,
            7,
            true,
        );
        assert_eq!(state.runtime.cargo_monitor.active_counts(), (1, 1));

        ScriptCargoMonitor::stop_all_monitoring(&mut state);
        assert_eq!(state.runtime.cargo_monitor.active_counts(), (0, 0));
    }

    #[test]
    fn registered_custom_cargo_is_valid_for_monitoring() {
        let mut state = GameState::new(8, 8);
        state.towns.push(Town {
            id: 4,
            pos: TileCoord::new(2, 2),
            ..Town::default()
        });
        state
            .industries
            .push(Industry::new(TileCoord::new(4, 4), IndustryKind::CoalMine).with_instance_id(7));

        let cargo = CargoType::Custom(11);
        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                cargo,
                4,
                true,
            ),
            -1
        );

        state.cargo_spec_catalog.push(CargoSpecDef {
            id: cargo.cargo_id(),
            label: "TEST".to_owned(),
            name: "Carga de prueba".to_owned(),
            from_newgrf: true,
            ..CargoSpecDef::default()
        });

        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                cargo,
                4,
                true,
            ),
            0
        );
        assert_eq!(
            ScriptCargoMonitor::get_industry_pickup_amount(
                &mut state,
                CompanyId::PLAYER,
                cargo,
                7,
                true,
            ),
            0
        );
        state.runtime.cargo_monitor.add_cargo_delivery(
            cargo,
            CompanyId::PLAYER,
            17,
            CargoSource::Industry(7),
            Some(4),
            Some(7),
        );
        assert_eq!(
            ScriptCargoMonitor::get_town_delivery_amount(
                &mut state,
                CompanyId::PLAYER,
                cargo,
                4,
                false,
            ),
            17
        );
        assert_eq!(
            ScriptCargoMonitor::get_industry_pickup_amount(
                &mut state,
                CompanyId::PLAYER,
                cargo,
                7,
                false,
            ),
            17
        );
    }
}
