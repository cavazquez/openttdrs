//! Tras el oráculo de 40 ticks, el tren que vuelve a Valle puede quedar sin
//! ruta si el destino cae en el andén x=25; debe reintentar x=26.
//! También: head-on en x=26 no debe congelar la partida para siempre.

use openttdrs_core::prelude::*;

#[test]
fn dual_fixture_finds_alt_platform_after_oracle_window() -> Result<(), Box<dyn std::error::Error>> {
    let raw = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
    ))?;
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&raw)?);
    for _ in 0..120 {
        state.step();
    }
    let stuck = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.no_network_route_to_order)
        .count();
    assert_eq!(
        stuck, 0,
        "tras 120 ticks no debe haber trenes sin ruta por andén alineado"
    );
    Ok(())
}

#[test]
fn dual_fixture_recovers_from_head_on_deadlock() -> Result<(), Box<dyn std::error::Error>> {
    let raw = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
    ))?;
    let mut state = GameState::from_sav_game(openttdrs_core::sav::load(&raw)?);
    let mut saw_motion_after_deadlock = false;
    for tick in 0..350 {
        state.step();
        if tick < 150 {
            continue;
        }
        if state
            .vehicles
            .iter()
            .any(|v| v.kind == VehicleKind::Train && v.is_consist_head() && v.cur_speed > 0)
        {
            saw_motion_after_deadlock = true;
            break;
        }
    }
    assert!(
        saw_motion_after_deadlock,
        "tras head-on en x=26 algún tren debe girar/reanudar (~120 ticks de espera)"
    );
    Ok(())
}
