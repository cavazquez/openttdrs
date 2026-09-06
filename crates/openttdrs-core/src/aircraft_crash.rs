//! Crash de jets en pista corta (`MaybeCrashAirplane` / `AIR_FAST` + `ShortStrip`).
//!
//! `OpenTTD`: `aircraft_cmd.cpp` — al frenar (`FLAG_BRAKE`) con pista corta y jet,
//! probabilidad fija `3276 / 2²²` (salvo cheat `no_jetcrash`).

use crate::GameState;
use crate::cargodist::parity::rng::Randomizer;
use crate::map::TileCoord;
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};
use crate::sim_events::SimEvent;
use crate::vehicle::{Vehicle, VehicleKind};

/// Probabilidad `OpenTTD` para `ShortStrip` + `AIR_FAST` (`GB(Random(),0,22) > 3276` → sobrevive).
pub const SHORT_STRIP_JET_CRASH_PROB: u32 = 3276;
const RANDOM_BITS: u32 = 22;
const RANDOM_MASK: u32 = (1 << RANDOM_BITS) - 1;

/// ¿Debe estrellarse un avión según el tramo, ajuste y tirada?
#[must_use]
pub fn should_crash_aircraft(
    short_strip: bool,
    is_jet: bool,
    no_jetcrash: bool,
    plane_crashes: u8,
    roll_22bit: u32,
) -> bool {
    let probability = if short_strip && is_jet {
        if no_jetcrash {
            return false;
        }
        SHORT_STRIP_JET_CRASH_PROB
    } else {
        if plane_crashes == 0 {
            return false;
        }
        // OpenTTD: (0x4000 << plane_crashes) / 1500, with 0/1/2 as the
        // none/reduced/normal setting values. Clamp malformed JSON/SAV input
        // to the native maximum instead of shifting an unbounded value.
        (0x4000_u32 << u32::from(plane_crashes.min(2))) / 1500
    };
    (roll_22bit & RANDOM_MASK) <= probability
}

/// Compatibilidad de la API histórica para el caso especial de jet en pista
/// corta, cuyo umbral no depende de `vehicle.plane_crashes`.
#[must_use]
pub fn should_crash_short_strip_jet(
    short_strip: bool,
    is_jet: bool,
    no_jetcrash: bool,
    roll_22bit: u32,
) -> bool {
    if !short_strip || !is_jet {
        return false;
    }
    should_crash_aircraft(short_strip, is_jet, no_jetcrash, 2, roll_22bit)
}

/// Tirada con el RNG de partida (`Random` / 22 bits bajos).
#[must_use]
pub fn roll_crash_die(rng: &mut Randomizer) -> u32 {
    rng.next() & RANDOM_MASK
}

/// ¿El vehículo acaba de entrar en un nodo `FLAG_BRAKE`?
#[must_use]
pub fn entered_brake_node(
    v: &Vehicle,
    prev_airport_pos: u8,
    prev_fta_active: bool,
    node_flags: u16,
) -> bool {
    v.kind == VehicleKind::Aircraft
        && v.airport_fta_active
        && node_flags & crate::airport_fta::FLAG_BRAKE != 0
        && (v.airport_pos != prev_airport_pos || !prev_fta_active)
}

/// Destruye el avión, emite evento y noticia de accidente.
pub fn crash_airplane(state: &mut GameState, vehicle_id: u32, at: TileCoord) {
    let Some(v) = state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return;
    };
    if v.kind != VehicleKind::Aircraft {
        return;
    }
    let name = v
        .name
        .clone()
        .unwrap_or_else(|| format!("Avión #{vehicle_id}"));
    let held = v.airport_blocks_held;
    let pos = v.pos;
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::AircraftCrash { vehicle_id, at });
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = crate::news::NewsItem::new(
        id,
        format!("{name} se estrelló al aterrizar"),
        Some(format!(
            "Un jet intentó aterrizar en pista corta en ({}, {}).",
            at.x, at.y
        )),
        NewsType::Accident,
        default_display_for_type(NewsType::Accident),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
    if held != 0
        && let Some(st) = state
            .stations
            .iter_mut()
            .find(|s| s.covers_tile(pos) || s.covers_tile(at))
    {
        st.airport_blocks &= !held;
    }
    state.vehicles.retain(|v| v.id != vehicle_id);
}

/// Tras un tick FTA: si entró en freno ShortStrip+jet, tira el dado.
pub fn maybe_crash_after_brake_tick(
    state: &mut GameState,
    vehicle_id: u32,
    prev_airport_pos: u8,
    prev_fta_active: bool,
) -> bool {
    let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return false;
    };
    let v = &state.vehicles[idx];
    let Some(st) = state
        .stations
        .iter()
        .find(|s| s.covers_tile(v.pos) || (v.airport_fta_active && s.covers_tile(v.dest)))
        .or_else(|| {
            state
                .stations
                .iter()
                .find(|s| crate::airport_fta::station_uses_airport_fta(s) && s.covers_tile(v.pos))
        })
    else {
        return false;
    };
    let Some(profile) = crate::airport_fta::fta_profile_for_spec(st.airport_spec) else {
        return false;
    };
    let md = profile.moving_data[usize::from(v.airport_pos).min(profile.moving_data.len() - 1)];
    if !entered_brake_node(v, prev_airport_pos, prev_fta_active, md.flags) {
        return false;
    }
    let Some(def) = crate::airport_class::airport_spec_def(st.airport_spec) else {
        return false;
    };
    let engine_id = v.engine_id.unwrap_or(0);
    let is_jet = crate::engine::aircraft_is_jet(engine_id);
    let no_jetcrash = state.cheats.no_jetcrash_active();
    let roll = roll_crash_die(&mut state.random);
    if !should_crash_aircraft(
        def.fta_flags.short_strip(),
        is_jet,
        no_jetcrash,
        state.construction.plane_crashes,
        roll,
    ) {
        return false;
    }
    let at = state.vehicles[idx].pos;
    crash_airplane(state, vehicle_id, at);
    true
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::{ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_FOKKER};
    use crate::{AirportSpecId, Command, TileCoord, VehicleKind, apply_command};

    #[test]
    fn short_strip_jet_crashes_on_low_roll() {
        assert!(should_crash_short_strip_jet(true, true, false, 0));
        assert!(should_crash_short_strip_jet(
            true,
            true,
            false,
            SHORT_STRIP_JET_CRASH_PROB
        ));
        assert!(!should_crash_short_strip_jet(
            true,
            true,
            false,
            SHORT_STRIP_JET_CRASH_PROB + 1
        ));
        assert!(!should_crash_short_strip_jet(true, true, true, 0));
        assert!(!should_crash_short_strip_jet(false, true, false, 0));
        assert!(!should_crash_short_strip_jet(true, false, false, 0));
    }

    #[test]
    fn plane_crashes_setting_controls_non_special_probability() {
        let reduced_limit = (0x4000_u32 << 1) / 1500;
        let normal_limit = (0x4000_u32 << 2) / 1500;
        assert!(!should_crash_aircraft(false, false, false, 0, 0));
        assert!(should_crash_aircraft(false, false, false, 1, reduced_limit));
        assert!(!should_crash_aircraft(
            false,
            false,
            false,
            1,
            reduced_limit + 1
        ));
        assert!(should_crash_aircraft(false, false, false, 2, normal_limit));
        assert!(!should_crash_aircraft(
            false,
            false,
            false,
            2,
            normal_limit + 1
        ));
    }

    #[test]
    fn fokker_is_jet_dakota_is_not() {
        assert!(crate::engine::aircraft_is_jet(ENGINE_AIRCRAFT_FOKKER));
        assert!(!crate::engine::aircraft_is_jet(ENGINE_AIRCRAFT_DAKOTA));
    }

    #[test]
    fn jet_crashes_on_country_brake_with_forced_rng() {
        let mut s = GameState::new(24, 24);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
        let hangar = s.stations[0].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_FOKKER),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        s.vehicles[0].running = true;
        s.vehicles[0].airport_fta_active = true;
        // Entrar en nodo 12 (FLAG_BRAKE) desde 11.
        s.vehicles[0].airport_pos = 12;
        s.vehicles[0].airport_prev_pos = 11;
        s.vehicles[0].pos = hangar;
        // Forzar tirada mortal: seed que produzca next() & mask == 0.
        s.random.set_seed(0);
        // Probar varios seeds hasta tirada ≤ prob (determinista en test).
        let mut crashed = false;
        for seed in 0..10_000u32 {
            s.random.set_seed(seed);
            let roll = roll_crash_die(&mut s.random);
            if should_crash_short_strip_jet(true, true, false, roll) {
                s.random.set_seed(seed);
                assert!(maybe_crash_after_brake_tick(&mut s, id, 11, true));
                crashed = true;
                break;
            }
        }
        assert!(crashed);
        assert!(s.vehicles.iter().all(|v| v.id != id));
        assert!(
            s.runtime
                .pending_sim_events
                .drain()
                .iter()
                .any(|e| matches!(e, SimEvent::AircraftCrash { .. }))
        );
        assert!(
            s.news
                .items
                .iter()
                .any(|n| n.news_type == NewsType::Accident)
        );
    }

    #[test]
    fn dakota_survives_short_strip_brake() {
        let mut s = GameState::new(24, 24);
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
        let hangar = s.stations[0].pos;
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
        )
        .unwrap();
        let id = s.vehicles[0].id;
        s.vehicles[0].running = true;
        s.vehicles[0].airport_fta_active = true;
        s.vehicles[0].airport_pos = 12;
        s.vehicles[0].kind = VehicleKind::Aircraft;
        s.random.set_seed(0);
        assert!(!maybe_crash_after_brake_tick(&mut s, id, 11, true));
        assert!(s.vehicles.iter().any(|v| v.id == id));
    }
}
