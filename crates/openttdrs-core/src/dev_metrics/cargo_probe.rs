//! Sonda un vehículo durante N ticks y mide si cargó, descargó y cuánto ingresó.

use crate::GameState;
use crate::cargo::CargoType;

/// Opciones de la sonda de ciclo carga/descarga.
#[derive(Debug, Clone, Copy)]
pub struct CargoProbeOptions {
    pub vehicle_id: u32,
    pub max_ticks: u64,
}

/// Resultado de observar un vehículo hasta `max_ticks` o hasta completar descarga.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VehicleCargoReport {
    pub vehicle_id: u32,
    pub ticks_run: u64,
    pub loaded: bool,
    pub delivered: bool,
    pub cargo_type: Option<CargoType>,
    /// Máximo de unidades a bordo tras la carga.
    pub units_loaded_peak: u32,
    /// Unidades entregadas en la primera descarga detectada.
    pub units_delivered: u32,
    /// Ingreso por transporte (`stats.cargo_income_earned` acumulado en la ventana).
    pub delivery_income: u64,
    /// Variación neta de `economy.money` (incluye costes de explotación).
    pub money_net: i64,
    pub tick_loaded: Option<u64>,
    pub tick_delivered: Option<u64>,
}

/// Avanza la simulación y registra carga → descarga → ingresos de un vehículo.
///
/// La carga se detecta cuando `cargo` pasa de 0 a `> 0`. La descarga, cuando vuelve
/// a 0 tras haber cargado y aumenta `stats.cargo_deliveries`.
#[must_use]
pub fn probe_vehicle_cargo_cycle(
    state: &mut GameState,
    opts: &CargoProbeOptions,
) -> VehicleCargoReport {
    let money_start = state.economy.money;
    let income_start = state.stats.cargo_income_earned;
    let deliveries_start = state.stats.cargo_deliveries;

    if !state.vehicles.iter().any(|v| v.id == opts.vehicle_id) {
        return VehicleCargoReport {
            vehicle_id: opts.vehicle_id,
            ticks_run: 0,
            loaded: false,
            delivered: false,
            cargo_type: None,
            units_loaded_peak: 0,
            units_delivered: 0,
            delivery_income: 0,
            money_net: 0,
            tick_loaded: None,
            tick_delivered: None,
        };
    }

    let mut loaded = false;
    let mut delivered = false;
    let mut units_loaded_peak = 0u32;
    let mut units_delivered = 0u32;
    let mut cargo_type = None;
    let mut tick_loaded = None;
    let mut tick_delivered = None;
    let mut ticks_run = 0u64;

    for _ in 0..opts.max_ticks {
        let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == opts.vehicle_id) else {
            break;
        };
        let cargo_before = state.vehicles[vehicle_idx].cargo;
        state.step();
        ticks_run += 1;
        let tick = state.tick.get();
        let Some(v) = state.vehicles.iter().find(|v| v.id == opts.vehicle_id) else {
            break;
        };

        if !loaded && v.cargo > 0 {
            loaded = true;
            units_loaded_peak = v.cargo;
            cargo_type = v.cargo_type;
            tick_loaded = Some(tick);
        } else if loaded && v.cargo > units_loaded_peak {
            units_loaded_peak = v.cargo;
        }

        if loaded
            && !delivered
            && state.stats.cargo_deliveries > deliveries_start
            && (v.cargo == 0 || (cargo_before > 0 && v.cargo < cargo_before))
        {
            // Descarga gradual: contar entrega al primer tick con pago/stats,
            // o cuando el vehículo queda vacío.
            if v.cargo == 0 || state.stats.cargo_units_delivered > 0 {
                delivered = true;
                units_delivered = units_loaded_peak.max(cargo_before.saturating_sub(v.cargo));
                tick_delivered = Some(tick);
                if v.cargo == 0 {
                    break;
                }
            }
        }
        if loaded && delivered && v.cargo == 0 {
            break;
        }
    }

    let delivery_income = state.stats.cargo_income_earned.saturating_sub(income_start);
    let money_net = state.economy.money.saturating_sub(money_start);

    VehicleCargoReport {
        vehicle_id: opts.vehicle_id,
        ticks_run,
        loaded,
        delivered,
        cargo_type,
        units_loaded_peak,
        units_delivered,
        delivery_income,
        money_net,
        tick_loaded,
        tick_delivered,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TileCoord;
    use crate::parity::{
        self, TRAIN_DUAL_VEHICLE_OUT_ID, TRAIN_LINE_VEHICLE_ID, TRAIN_SUPPLY_VEHICLE_ID,
    };

    #[test]
    fn train_line_vehicle_loads_delivers_and_earns_income() {
        let Some(mut state) = parity::build_scenario("train_line") else {
            panic!("escenario train_line");
        };
        let report = probe_vehicle_cargo_cycle(
            &mut state,
            &CargoProbeOptions {
                vehicle_id: TRAIN_LINE_VEHICLE_ID,
                max_ticks: 12_000,
            },
        );
        assert!(report.loaded, "debe cargar en estación A: {report:?}");
        assert!(report.delivered, "debe descargar en estación B: {report:?}");
        assert!(report.units_loaded_peak > 0);
        assert!(report.units_delivered > 0);
        assert!(
            report.delivery_income > 0,
            "ingreso por transporte: {report:?}"
        );
        assert_eq!(report.cargo_type, Some(CargoType::Goods));
    }

    #[test]
    fn train_supply_loads_coal_from_mine_and_delivers() {
        let Some(mut state) = parity::build_scenario("train_supply") else {
            panic!("escenario train_supply");
        };
        let station_a = state
            .stations
            .iter()
            .find(|s| s.pos == parity::TRAIN_LINE_STATION_A)
            .expect("estación A");
        assert_eq!(station_a.cargo_stock.goods, 0, "sin goods precolocados");

        let report = probe_vehicle_cargo_cycle(
            &mut state,
            &CargoProbeOptions {
                vehicle_id: TRAIN_SUPPLY_VEHICLE_ID,
                max_ticks: 12_000,
            },
        );
        assert!(
            report.loaded,
            "debe cargar carbón en estación A: {report:?}"
        );
        assert!(report.delivered, "debe descargar en estación B: {report:?}");
        assert_eq!(report.cargo_type, Some(CargoType::Coal));
        assert!(
            report.delivery_income > 0,
            "ingreso por transporte: {report:?}"
        );
    }

    #[test]
    fn train_supply_dual_outbound_delivers_coal() {
        let Some(mut state) = parity::build_scenario("train_supply_dual") else {
            panic!("escenario train_supply_dual");
        };
        let report = probe_vehicle_cargo_cycle(
            &mut state,
            &CargoProbeOptions {
                vehicle_id: TRAIN_DUAL_VEHICLE_OUT_ID,
                max_ticks: 12_000,
            },
        );
        assert!(report.loaded, "ida: debe cargar: {report:?}");
        assert!(report.delivered, "ida: debe descargar en B: {report:?}");
        assert_eq!(report.cargo_type, Some(CargoType::Coal));
    }

    #[test]
    fn train_supply_dual_signals_face_traffic_direction() {
        let Some(state) = parity::build_scenario("train_supply_dual") else {
            panic!("escenario train_supply_dual");
        };
        let out_sig = state
            .map
            .get(TileCoord::new(7, parity::TRAIN_DUAL_TRACK_OUT_Y))
            .expect("señal ida");
        let ret_sig = state
            .map
            .get(TileCoord::new(7, parity::TRAIN_DUAL_TRACK_RET_Y))
            .expect("señal vuelta");
        assert_eq!(
            crate::rail_signals::rail_signal_present_mask(out_sig.m3),
            0b0100,
            "ida (+x): bit 2 / orientación 0"
        );
        assert_eq!(
            crate::rail_signals::rail_signal_present_mask(ret_sig.m3),
            0b1000,
            "vuelta (-x): bit 3 / orientación 2"
        );
    }

    #[test]
    fn missing_vehicle_returns_empty_report() {
        let mut state = GameState::new(4, 4);
        let report = probe_vehicle_cargo_cycle(
            &mut state,
            &CargoProbeOptions {
                vehicle_id: 99,
                max_ticks: 10,
            },
        );
        assert!(!report.loaded);
        assert!(!report.delivered);
        assert_eq!(report.ticks_run, 0);
    }
}
