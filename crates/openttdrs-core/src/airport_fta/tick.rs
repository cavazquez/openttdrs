//! Loop FTA por tick (`AirportGoToNextPosition` simplificado).

use crate::aircraft_movement::{AIRCRAFT_CRUISE_ALTITUDE, AircraftPhaseEvent, straight_line_path};
use crate::map::{Map, TileCoord};
use crate::station::Station;
use crate::vehicle::{
    AircraftPhase, DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, Vehicle, VehicleKind,
};

use super::profile::fta_profile_for_spec;
use super::types::{
    AirportFtaEdge, AirportFtaKind, AirportFtaProfile, AirportHeading, FLAG_BRAKE, FLAG_HELI_LOWER,
    FLAG_HELI_RAISE, FLAG_HOLD, FLAG_LAND, FLAG_NO_SPEED_CLAMP, FLAG_TAKEOFF,
};

const FTA_DWELL_TICKS: u16 = 6;
const FTA_HOLD_DWELL_TICKS: u16 = 10;

/// `true` si la estación usa motor FTA (perfil + footprint completo).
#[must_use]
pub fn station_uses_airport_fta(station: &Station) -> bool {
    station.stop_kind == crate::station::StopKind::Airport
        && fta_profile_for_spec(station.airport_spec).is_some_and(|p| {
            station.airport_tiles.len()
                >= usize::try_from(p.footprint_w * p.footprint_h).unwrap_or(1)
        })
}

/// Compat: alias del predicado Country del corte 1.
#[must_use]
pub fn station_uses_country_fta(station: &Station) -> bool {
    station_uses_airport_fta(station)
        && matches!(
            station.airport_spec,
            crate::airport_class::AirportSpecId::Small
        )
}

/// Ancla NW del footprint aeroportuario.
#[must_use]
pub fn airport_nw_origin(station: &Station) -> TileCoord {
    station.airport_tiles.iter().fold(station.pos, |acc, &c| {
        TileCoord::new(acc.x.min(c.x), acc.y.min(c.y))
    })
}

/// Tick FTA. `None` = el caller debe usar el FSM MVP.
pub fn tick_country_airport_fta(
    v: &mut Vehicle,
    map: &Map,
    stations: &mut [Station],
) -> Option<AircraftPhaseEvent> {
    tick_airport_fta(v, map, stations)
}

/// Tick FTA para specs soportados hasta Helistation.
pub fn tick_airport_fta(
    v: &mut Vehicle,
    map: &Map,
    stations: &mut [Station],
) -> Option<AircraftPhaseEvent> {
    if v.kind != VehicleKind::Aircraft || !v.running {
        return None;
    }
    if let Some(ev) = try_enter_approach(v, stations) {
        return Some(ev);
    }
    if v.aircraft_phase == AircraftPhase::Flying && !v.airport_fta_active {
        return None;
    }
    let st_idx = resolve_fta_station_idx(v, stations)?;
    if v.airport_fta_station.is_none() {
        v.airport_fta_station = Some(stations[st_idx].pos);
    }
    let profile = fta_profile_for_spec(stations[st_idx].airport_spec)?;
    if !station_uses_airport_fta(&stations[st_idx]) {
        return None;
    }
    if !v.airport_fta_active {
        if v.aircraft_phase == AircraftPhase::InHangar || stations[st_idx].covers_tile(v.pos) {
            activate_fta_in_hangar(v);
        } else {
            return None;
        }
    }
    update_heading_for_orders(v, &stations[st_idx], profile.kind);
    if v.airport_loading_stand_reached && (v.awaiting_load_window || v.cargo_transfer_active()) {
        v.cur_speed = 0;
        return Some(sync_phase_from_node(v, &profile));
    }

    let just_reached = move_towards_waypoint(v, map, &stations[st_idx], &profile);
    let ev = sync_phase_from_node(v, &profile);
    if just_reached && airport_node_is_loading_stand(profile.kind, v.airport_pos) {
        // La orden se completa recién al alcanzar físicamente el stand. Antes,
        // `apply_waypoint_pose` adelantaba el avión y abría la carga a distancia.
        v.dest = v.pos;
        v.cur_speed = 0;
        v.airport_loading_stand_reached = true;
    }
    if v.aircraft_phase_ticks > 0 {
        v.aircraft_phase_ticks -= 1;
        return Some(ev);
    }
    if just_reached {
        // Conservar el nodo alcanzado al menos hasta el siguiente tick. Esto
        // permite abrir la ventana de carga y evita atravesar un stand en el
        // mismo tick en que se llega físicamente a él.
        return Some(ev);
    }
    if !v.airport_waypoint_reached {
        return Some(ev);
    }
    if should_finish_takeoff(v, &profile) {
        return Some(finish_takeoff(v, &mut stations[st_idx]));
    }
    Some(advance_fta_node(v, &mut stations[st_idx], &profile))
}

fn try_enter_approach(v: &mut Vehicle, stations: &[Station]) -> Option<AircraftPhaseEvent> {
    if v.aircraft_phase != AircraftPhase::Flying || v.airport_fta_active {
        return None;
    }
    let target = v.dest;
    let st = stations
        .iter()
        .find(|s| s.covers_tile(target) && station_uses_airport_fta(s))?;
    let profile = fta_profile_for_spec(st.airport_spec)?;
    let manhattan = (v.pos.x - target.x).abs() + (v.pos.y - target.y).abs();
    if manhattan > 4 && !v.path.is_empty() {
        return None;
    }
    let entry = profile.entries[0];
    v.airport_fta_active = true;
    v.airport_fta_station = Some(st.pos);
    v.airport_blocks_held = 0;
    v.airport_pos = entry;
    v.airport_prev_pos = entry;
    v.airport_waypoint_reached = false;
    v.airport_loading_stand_reached = false;
    v.airport_heading = match profile.kind {
        AirportFtaKind::Helidepot | AirportFtaKind::Heliport | AirportFtaKind::Helistation => {
            AirportHeading::HeliLanding
        }
        AirportFtaKind::Country
        | AirportFtaKind::Commuter
        | AirportFtaKind::City
        | AirportFtaKind::Metropolitan
        | AirportFtaKind::International
        | AirportFtaKind::Intercontinental => AirportHeading::Landing,
    };
    v.aircraft_phase = AircraftPhase::Landing;
    v.aircraft_phase_ticks = FTA_HOLD_DWELL_TICKS;
    v.path.clear();
    v.progress = 0;
    ensure_airport_subpos(v);
    Some(AircraftPhaseEvent::Landing)
}

fn should_finish_takeoff(v: &Vehicle, profile: &AirportFtaProfile) -> bool {
    if !v.airport_fta_active || v.aircraft_phase_ticks != 0 {
        return false;
    }
    let at_fixedwing_takeoff = profile.fixedwing_takeoff_pos == Some(v.airport_pos)
        || (profile.kind == AirportFtaKind::Intercontinental && v.airport_pos == 62);
    if at_fixedwing_takeoff
        && matches!(
            v.airport_heading,
            AirportHeading::EndTakeoff | AirportHeading::Takeoff | AirportHeading::StartTakeoff
        )
    {
        return true;
    }
    // Helidepot / Commuter heli: raise + heading heli takeoff.
    let heli_takeoff_node = match profile.kind {
        AirportFtaKind::Helidepot => matches!(v.airport_pos, 11 | 15),
        AirportFtaKind::Heliport => v.airport_pos == 1,
        AirportFtaKind::Helistation => matches!(v.airport_pos, 3 | 12 | 13 | 14),
        AirportFtaKind::Commuter => matches!(v.airport_pos, 31 | 32 | 37),
        AirportFtaKind::City => v.airport_pos == 22,
        AirportFtaKind::Metropolitan => v.airport_pos == 24,
        AirportFtaKind::International => matches!(v.airport_pos, 47 | 48 | 51 | 52),
        AirportFtaKind::Intercontinental => matches!(v.airport_pos, 53 | 54 | 74 | 75),
        AirportFtaKind::Country => false,
    };
    heli_takeoff_node && matches!(v.airport_heading, AirportHeading::HeliTakeoff)
}

fn advance_fta_node(
    v: &mut Vehicle,
    station: &mut Station,
    profile: &AirportFtaProfile,
) -> AircraftPhaseEvent {
    let prev = v.airport_pos;
    nudge_heading_at_node(v, profile);

    let Some(edge) = choose_next_edge(v, profile) else {
        if should_finish_takeoff(v, profile)
            || (profile.fixedwing_takeoff_pos == Some(v.airport_pos))
        {
            return finish_takeoff(v, station);
        }
        return AircraftPhaseEvent::None;
    };

    // next=0 en arista de takeoff heli = salir a crucero.
    if edge.next_position == 0
        && matches!(
            edge.heading,
            AirportHeading::HeliTakeoff | AirportHeading::EndTakeoff
        )
    {
        return finish_takeoff(v, station);
    }

    let next = edge.next_position;
    // Multi-avión: no pisar reservas ajenas; mantener las propias hasta poder avanzar.
    if !blocks_free_for(station, v.airport_blocks_held, edge.blocks) {
        hold_or_wait(v, profile);
        return sync_phase_from_node(v, profile);
    }
    release_held_blocks(station, v);
    acquire_blocks(station, v, edge.blocks);

    v.airport_prev_pos = prev;
    v.airport_pos = next;
    v.airport_waypoint_reached = false;
    v.airport_loading_stand_reached = false;
    let md = profile.moving_data[usize::from(next).min(profile.moving_data.len() - 1)];
    v.direction = md.direction;
    v.aircraft_phase_ticks = dwell_for_node(next, md.flags, profile);
    apply_enter_heading(v, next, profile);

    let ev = sync_phase_from_node(v, profile);
    if profile.fixedwing_takeoff_pos == Some(next)
        || (profile.kind == AirportFtaKind::Intercontinental && next == 62)
        || (matches!(
            profile.kind,
            AirportFtaKind::Helidepot
                | AirportFtaKind::Heliport
                | AirportFtaKind::Helistation
                | AirportFtaKind::Commuter
                | AirportFtaKind::City
                | AirportFtaKind::Metropolitan
                | AirportFtaKind::International
                | AirportFtaKind::Intercontinental
        ) && md.flags & FLAG_HELI_RAISE != 0)
    {
        return AircraftPhaseEvent::Takeoff;
    }
    if md.flags & FLAG_LAND != 0 || md.flags & FLAG_HELI_LOWER != 0 {
        return AircraftPhaseEvent::Landing;
    }
    ev
}

fn airport_node_is_loading_stand(kind: AirportFtaKind, node: u8) -> bool {
    match kind {
        AirportFtaKind::Country => matches!(node, 2 | 3),
        AirportFtaKind::Commuter => matches!(node, 3..=7),
        AirportFtaKind::City | AirportFtaKind::Metropolitan => matches!(node, 2..=4),
        AirportFtaKind::International => matches!(node, 4..=11),
        AirportFtaKind::Intercontinental => matches!(node, 4..=13),
        AirportFtaKind::Helidepot => node == 14,
        AirportFtaKind::Heliport => node == 0,
        AirportFtaKind::Helistation => matches!(node, 6..=8),
    }
}

fn nudge_heading_at_node(v: &mut Vehicle, profile: &AirportFtaProfile) {
    match profile.kind {
        AirportFtaKind::Country => match v.airport_pos {
            8 => v.airport_heading = AirportHeading::EndTakeoff,
            7 => v.airport_heading = AirportHeading::StartTakeoff,
            11 => v.airport_heading = AirportHeading::Landing,
            12 | 13 => v.airport_heading = AirportHeading::EndLanding,
            _ => {}
        },
        AirportFtaKind::Helidepot => match v.airport_pos {
            7 => v.airport_heading = AirportHeading::HeliLanding,
            // En 8, `HELIENDLANDING` es self-loop: salir al pad con HELIPAD1.
            8..=10 => v.airport_heading = AirportHeading::Helipad1,
            11 | 15 | 17 => v.airport_heading = AirportHeading::HeliTakeoff,
            _ => {}
        },
        AirportFtaKind::Heliport => match v.airport_pos {
            1 => v.airport_heading = AirportHeading::HeliTakeoff,
            2 | 3 => v.airport_heading = AirportHeading::HeliLanding,
            // 4: self-loop HELIENDLANDING → pad.
            4 => v.airport_heading = AirportHeading::Helipad1,
            _ => {}
        },
        AirportFtaKind::Helistation => match v.airport_pos {
            3 | 12 | 13 | 14 => v.airport_heading = AirportHeading::HeliTakeoff,
            2 | 15 => v.airport_heading = AirportHeading::HeliLanding,
            // 16: self-loop HELIENDLANDING → pad1.
            16 => v.airport_heading = AirportHeading::Helipad1,
            _ => {}
        },
        AirportFtaKind::Commuter => match v.airport_pos {
            12 => v.airport_heading = AirportHeading::Takeoff,
            14 => v.airport_heading = AirportHeading::StartTakeoff,
            15 => v.airport_heading = AirportHeading::EndTakeoff,
            16 | 17 => v.airport_heading = AirportHeading::Landing,
            20 => v.airport_heading = AirportHeading::EndLanding,
            // 26: self-loop HELIENDLANDING → salir a pad.
            26 => v.airport_heading = AirportHeading::Helipad1,
            31 | 32 | 35 | 36 | 37 => v.airport_heading = AirportHeading::HeliTakeoff,
            _ => {}
        },
        AirportFtaKind::City => match v.airport_pos {
            10 => v.airport_heading = AirportHeading::Takeoff,
            11 => v.airport_heading = AirportHeading::StartTakeoff,
            12 => v.airport_heading = AirportHeading::EndTakeoff,
            13 | 14 => v.airport_heading = AirportHeading::Landing,
            17 => v.airport_heading = AirportHeading::EndLanding,
            22 => v.airport_heading = AirportHeading::HeliTakeoff,
            23 | 24 => v.airport_heading = AirportHeading::HeliLanding,
            _ => {}
        },
        AirportFtaKind::Metropolitan => match v.airport_pos {
            10 => v.airport_heading = AirportHeading::Takeoff,
            11 => v.airport_heading = AirportHeading::StartTakeoff,
            12 => v.airport_heading = AirportHeading::EndTakeoff,
            13 | 14 => v.airport_heading = AirportHeading::Landing,
            16..=18 | 27 => v.airport_heading = AirportHeading::EndLanding,
            24 => v.airport_heading = AirportHeading::HeliTakeoff,
            25 | 26 => v.airport_heading = AirportHeading::HeliLanding,
            _ => {}
        },
        AirportFtaKind::International => match v.airport_pos {
            28 => v.airport_heading = AirportHeading::Takeoff,
            30 => v.airport_heading = AirportHeading::StartTakeoff,
            31 => v.airport_heading = AirportHeading::EndTakeoff,
            32 | 33 => v.airport_heading = AirportHeading::Landing,
            35 | 36 => v.airport_heading = AirportHeading::EndLanding,
            // 42: self-loop HELIENDLANDING → salir a pad.
            41 => v.airport_heading = AirportHeading::HeliLanding,
            42 => v.airport_heading = AirportHeading::Helipad1,
            47 | 48 | 51 | 52 => v.airport_heading = AirportHeading::HeliTakeoff,
            _ => {}
        },
        AirportFtaKind::Intercontinental => match v.airport_pos {
            32 | 59 => v.airport_heading = AirportHeading::Takeoff,
            34 | 61 => v.airport_heading = AirportHeading::StartTakeoff,
            35 | 62 => v.airport_heading = AirportHeading::EndTakeoff,
            37 | 63 | 76 | 69 => v.airport_heading = AirportHeading::Landing,
            40..=42 | 66..=68 => v.airport_heading = AirportHeading::EndLanding,
            47 => v.airport_heading = AirportHeading::HeliLanding,
            48 => v.airport_heading = AirportHeading::Helipad1,
            53 | 54 | 74 | 75 => v.airport_heading = AirportHeading::HeliTakeoff,
            _ => {}
        },
    }
}

fn hold_or_wait(v: &mut Vehicle, profile: &AirportFtaProfile) {
    if (profile.hold_min..=profile.hold_max).contains(&v.airport_pos) {
        v.airport_prev_pos = v.airport_pos;
        v.airport_pos = if v.airport_pos >= profile.hold_max {
            match profile.kind {
                AirportFtaKind::Country => 10,
                AirportFtaKind::Helidepot
                | AirportFtaKind::Heliport
                | AirportFtaKind::Helistation => 2,
                AirportFtaKind::Commuter => 16,
                AirportFtaKind::City | AirportFtaKind::Metropolitan => 13,
                AirportFtaKind::International => 32,
                AirportFtaKind::Intercontinental => 76,
            }
        } else {
            v.airport_pos.saturating_add(1)
        };
        v.airport_waypoint_reached = false;
        v.airport_loading_stand_reached = false;
        v.aircraft_phase_ticks = FTA_HOLD_DWELL_TICKS;
    } else {
        v.aircraft_phase_ticks = FTA_DWELL_TICKS;
    }
}

fn dwell_for_node(next: u8, flags: u16, profile: &AirportFtaProfile) -> u16 {
    if flags & (FLAG_TAKEOFF | FLAG_HELI_RAISE) != 0 {
        12
    } else if (profile.hold_min..=profile.hold_max).contains(&next) {
        FTA_HOLD_DWELL_TICKS
    } else {
        FTA_DWELL_TICKS
    }
}

fn apply_enter_heading_city(v: &mut Vehicle, next: u8) {
    match next {
        2 => v.airport_heading = AirportHeading::Term1,
        3 => v.airport_heading = AirportHeading::Term2,
        4 => v.airport_heading = AirportHeading::Term3,
        10 => v.airport_heading = AirportHeading::Takeoff,
        11 => v.airport_heading = AirportHeading::StartTakeoff,
        12 => v.airport_heading = AirportHeading::EndTakeoff,
        14 => v.airport_heading = AirportHeading::Landing,
        17 => v.airport_heading = AirportHeading::EndLanding,
        7 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
            v.airport_heading = AirportHeading::Term1;
        }
        22 => v.airport_heading = AirportHeading::HeliTakeoff,
        _ => {}
    }
}

fn apply_enter_heading_metropolitan(v: &mut Vehicle, next: u8) {
    match next {
        2 => v.airport_heading = AirportHeading::Term1,
        3 => v.airport_heading = AirportHeading::Term2,
        4 => v.airport_heading = AirportHeading::Term3,
        10 => v.airport_heading = AirportHeading::Takeoff,
        11 => v.airport_heading = AirportHeading::StartTakeoff,
        12 => v.airport_heading = AirportHeading::EndTakeoff,
        14 => v.airport_heading = AirportHeading::Landing,
        16..=18 | 27 => v.airport_heading = AirportHeading::EndLanding,
        24 => v.airport_heading = AirportHeading::HeliTakeoff,
        _ => {}
    }
}

fn apply_enter_heading_international(v: &mut Vehicle, next: u8) {
    match next {
        4 => v.airport_heading = AirportHeading::Term1,
        5 => v.airport_heading = AirportHeading::Term2,
        6 => v.airport_heading = AirportHeading::Term3,
        7 => v.airport_heading = AirportHeading::Term4,
        8 => v.airport_heading = AirportHeading::Term5,
        9 => v.airport_heading = AirportHeading::Term6,
        10 => v.airport_heading = AirportHeading::Helipad1,
        11 => v.airport_heading = AirportHeading::Helipad2,
        26 | 28 => v.airport_heading = AirportHeading::Takeoff,
        30 => v.airport_heading = AirportHeading::StartTakeoff,
        31 => v.airport_heading = AirportHeading::EndTakeoff,
        33 => v.airport_heading = AirportHeading::Landing,
        35 | 36 => v.airport_heading = AirportHeading::EndLanding,
        23 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
            v.airport_heading = AirportHeading::Term1;
        }
        47 | 48 | 51 | 52 => v.airport_heading = AirportHeading::HeliTakeoff,
        _ => {}
    }
}

fn apply_enter_heading_intercontinental(v: &mut Vehicle, next: u8) {
    match next {
        4 => v.airport_heading = AirportHeading::Term1,
        5 => v.airport_heading = AirportHeading::Term2,
        6 => v.airport_heading = AirportHeading::Term3,
        7 => v.airport_heading = AirportHeading::Term4,
        8 => v.airport_heading = AirportHeading::Term5,
        9 => v.airport_heading = AirportHeading::Term6,
        10 => v.airport_heading = AirportHeading::Term7,
        11 => v.airport_heading = AirportHeading::Term8,
        12 => v.airport_heading = AirportHeading::Helipad1,
        13 => v.airport_heading = AirportHeading::Helipad2,
        30 | 32 | 57 | 59 => v.airport_heading = AirportHeading::Takeoff,
        34 | 61 => v.airport_heading = AirportHeading::StartTakeoff,
        35 | 62 => v.airport_heading = AirportHeading::EndTakeoff,
        37 | 63 => v.airport_heading = AirportHeading::Landing,
        40..=42 | 66..=68 => v.airport_heading = AirportHeading::EndLanding,
        26 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
            v.airport_heading = AirportHeading::Term1;
        }
        53 | 54 | 74 | 75 => v.airport_heading = AirportHeading::HeliTakeoff,
        _ => {}
    }
}

fn apply_enter_heading(v: &mut Vehicle, next: u8, profile: &AirportFtaProfile) {
    match profile.kind {
        AirportFtaKind::Country => match next {
            7 => v.airport_heading = AirportHeading::Takeoff,
            8 => v.airport_heading = AirportHeading::StartTakeoff,
            9 => v.airport_heading = AirportHeading::EndTakeoff,
            2 | 14 => v.airport_heading = AirportHeading::Term1,
            3 => v.airport_heading = AirportHeading::Term2,
            13 => v.airport_heading = AirportHeading::EndLanding,
            1 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
                v.airport_heading = AirportHeading::Term1;
            }
            _ => {}
        },
        AirportFtaKind::Helidepot => match next {
            10 | 14 => v.airport_heading = AirportHeading::Helipad1,
            11 | 15 | 17 => v.airport_heading = AirportHeading::HeliTakeoff,
            7 | 8 => v.airport_heading = AirportHeading::HeliLanding,
            _ => {}
        },
        AirportFtaKind::Heliport => match next {
            0 => v.airport_heading = AirportHeading::Helipad1,
            1 => v.airport_heading = AirportHeading::HeliTakeoff,
            2 | 3 => v.airport_heading = AirportHeading::HeliLanding,
            4 => v.airport_heading = AirportHeading::HeliEndLanding,
            _ => {}
        },
        AirportFtaKind::Helistation => match next {
            6 => v.airport_heading = AirportHeading::Helipad1,
            7 => v.airport_heading = AirportHeading::Helipad2,
            8 => v.airport_heading = AirportHeading::Helipad3,
            3 | 12 | 13 | 14 => v.airport_heading = AirportHeading::HeliTakeoff,
            15 => v.airport_heading = AirportHeading::HeliLanding,
            16 => v.airport_heading = AirportHeading::HeliEndLanding,
            _ => {}
        },
        AirportFtaKind::Commuter => match next {
            3 => v.airport_heading = AirportHeading::Term1,
            4 => v.airport_heading = AirportHeading::Term2,
            5 => v.airport_heading = AirportHeading::Term3,
            6 => v.airport_heading = AirportHeading::Helipad1,
            7 => v.airport_heading = AirportHeading::Helipad2,
            12 => v.airport_heading = AirportHeading::Takeoff,
            14 => v.airport_heading = AirportHeading::StartTakeoff,
            15 => v.airport_heading = AirportHeading::EndTakeoff,
            17 => v.airport_heading = AirportHeading::Landing,
            20 => v.airport_heading = AirportHeading::EndLanding,
            2 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
                v.airport_heading = AirportHeading::Term1;
            }
            31 | 32 | 37 => v.airport_heading = AirportHeading::HeliTakeoff,
            _ => {}
        },
        AirportFtaKind::City => apply_enter_heading_city(v, next),
        AirportFtaKind::Metropolitan => apply_enter_heading_metropolitan(v, next),
        AirportFtaKind::International => apply_enter_heading_international(v, next),
        AirportFtaKind::Intercontinental => apply_enter_heading_intercontinental(v, next),
    }
}

fn resolve_fta_station_idx(v: &Vehicle, stations: &[Station]) -> Option<usize> {
    if v.airport_fta_active
        && let Some(anchor) = v.airport_fta_station
        && let Some(i) = stations
            .iter()
            .position(|s| station_uses_airport_fta(s) && s.pos == anchor)
    {
        return Some(i);
    }
    if let Some(i) = stations
        .iter()
        .position(|s| station_uses_airport_fta(s) && s.covers_tile(v.pos))
    {
        return Some(i);
    }
    if v.airport_fta_active
        && let Some(i) = stations
            .iter()
            .position(|s| station_uses_airport_fta(s) && s.covers_tile(v.dest))
    {
        return Some(i);
    }
    None
}

fn activate_fta_in_hangar(v: &mut Vehicle) {
    v.airport_fta_active = true;
    v.airport_pos = 0;
    v.airport_prev_pos = 0;
    v.airport_heading = AirportHeading::Hangar;
    v.aircraft_phase = AircraftPhase::InHangar;
    v.aircraft_phase_ticks = 0;
    v.altitude = 0;
    v.airport_blocks_held = 0;
    v.airport_waypoint_reached = true;
    v.airport_loading_stand_reached = false;
    v.airport_subpos_valid = false;
    ensure_airport_subpos(v);
}

fn activate_fta_on_pad(v: &mut Vehicle) {
    v.airport_fta_active = true;
    v.airport_pos = 0;
    v.airport_prev_pos = 0;
    v.airport_heading = AirportHeading::Helipad1;
    v.aircraft_phase = AircraftPhase::Taxi;
    v.aircraft_phase_ticks = 0;
    v.altitude = 0;
    v.airport_blocks_held = 0;
    v.airport_waypoint_reached = true;
    v.airport_loading_stand_reached = false;
    v.airport_subpos_valid = false;
    ensure_airport_subpos(v);
}

/// Inicializa estado FTA al comprar en hangar con perfil soportado.
pub fn init_country_fta_on_purchase(v: &mut Vehicle) {
    activate_fta_in_hangar(v);
}

/// Inicializa FTA según el kind del perfil (hangar o pad Heliport/Oilrig).
pub fn init_airport_fta_on_purchase(v: &mut Vehicle, kind: AirportFtaKind) {
    if kind == AirportFtaKind::Heliport {
        activate_fta_on_pad(v);
    } else {
        activate_fta_in_hangar(v);
    }
}

fn fixedwing_approach_locked(v: &Vehicle, kind: AirportFtaKind) -> bool {
    matches!(
        v.airport_heading,
        AirportHeading::Landing
            | AirportHeading::EndLanding
            | AirportHeading::StartTakeoff
            | AirportHeading::EndTakeoff
    ) || (kind == AirportFtaKind::Commuter && matches!(v.airport_pos, 11..=24))
        || (kind == AirportFtaKind::City && matches!(v.airport_pos, 8..=21 | 25..=29))
        || (kind == AirportFtaKind::Metropolitan && matches!(v.airport_pos, 8..=22 | 27))
        || (kind == AirportFtaKind::International
            && (matches!(v.airport_pos, 19..=52)
                || (matches!(v.airport_pos, 2 | 12..=18 | 23..=25)
                    && matches!(
                        v.airport_heading,
                        AirportHeading::Landing
                            | AirportHeading::EndLanding
                            | AirportHeading::Term1
                    ))))
        || (kind == AirportFtaKind::Intercontinental
            && (matches!(v.airport_pos, 30..=76)
                || (matches!(v.airport_pos, 2 | 14..=29)
                    && matches!(
                        v.airport_heading,
                        AirportHeading::Landing
                            | AirportHeading::EndLanding
                            | AirportHeading::Term1
                    ))))
}

fn nudge_endlanding_to_term1(v: &mut Vehicle, kind: AirportFtaKind) {
    let at = match kind {
        AirportFtaKind::Commuter => v.airport_pos == 2,
        AirportFtaKind::City => v.airport_pos == 7,
        AirportFtaKind::Metropolitan => v.airport_pos == 27,
        AirportFtaKind::International => matches!(v.airport_pos, 2 | 23 | 36),
        AirportFtaKind::Intercontinental => matches!(v.airport_pos, 2 | 26 | 42 | 66 | 68),
        _ => false,
    };
    if at && matches!(v.airport_heading, AirportHeading::EndLanding) {
        v.airport_heading = AirportHeading::Term1;
    }
}

fn update_heading_helidepot(v: &mut Vehicle, remote: bool) {
    if v.orders.is_empty() {
        return;
    }
    if matches!(
        v.airport_heading,
        AirportHeading::HeliLanding | AirportHeading::HeliEndLanding
    ) || matches!(v.airport_pos, 2..=9 | 12 | 13)
    {
        if v.airport_pos == 8 && matches!(v.airport_heading, AirportHeading::HeliEndLanding) {
            v.airport_heading = AirportHeading::Helipad1;
        }
        return;
    }
    if remote {
        if matches!(v.airport_pos, 14 | 10 | 17) {
            v.airport_heading = AirportHeading::HeliTakeoff;
        } else if matches!(v.airport_pos, 0 | 1) {
            v.airport_heading = AirportHeading::HeliTakeoff;
            if v.aircraft_phase == AircraftPhase::InHangar {
                v.aircraft_phase = AircraftPhase::Taxi;
            }
        }
    } else {
        v.airport_heading = AirportHeading::Helipad1;
    }
}

fn update_heading_heliport(v: &mut Vehicle, remote: bool) {
    if v.orders.is_empty() {
        return;
    }
    if matches!(
        v.airport_heading,
        AirportHeading::HeliLanding | AirportHeading::HeliEndLanding
    ) || matches!(v.airport_pos, 2..=8)
    {
        if v.airport_pos == 4 && matches!(v.airport_heading, AirportHeading::HeliEndLanding) {
            v.airport_heading = AirportHeading::Helipad1;
        }
        return;
    }
    if remote {
        v.airport_heading = AirportHeading::HeliTakeoff;
    } else {
        v.airport_heading = AirportHeading::Helipad1;
    }
}

fn update_heading_helistation(v: &mut Vehicle, remote: bool) {
    if v.orders.is_empty() {
        return;
    }
    if matches!(
        v.airport_heading,
        AirportHeading::HeliLanding | AirportHeading::HeliEndLanding
    ) || matches!(v.airport_pos, 2 | 15..=32)
    {
        if v.airport_pos == 16 && matches!(v.airport_heading, AirportHeading::HeliEndLanding) {
            v.airport_heading = AirportHeading::Helipad1;
        }
        return;
    }
    if remote {
        if matches!(v.airport_pos, 6..=14 | 0 | 1 | 3) {
            v.airport_heading = AirportHeading::HeliTakeoff;
            if v.aircraft_phase == AircraftPhase::InHangar {
                v.aircraft_phase = AircraftPhase::Taxi;
            }
        }
    } else {
        v.airport_heading = AirportHeading::Helipad1;
    }
}

fn update_heading_for_orders(v: &mut Vehicle, station: &Station, kind: AirportFtaKind) {
    let remote = !station.covers_tile(v.dest) && v.pos != v.dest;
    if kind == AirportFtaKind::Helidepot {
        update_heading_helidepot(v, remote);
        return;
    }
    if kind == AirportFtaKind::Heliport {
        update_heading_heliport(v, remote);
        return;
    }
    if kind == AirportFtaKind::Helistation {
        update_heading_helistation(v, remote);
        return;
    }
    // Country / Commuter / City / Metropolitan / International / Intercontinental.
    if fixedwing_approach_locked(v, kind) {
        nudge_endlanding_to_term1(v, kind);
        return;
    }
    if v.airport_pos != 0
        && !matches!(
            v.airport_heading,
            AirportHeading::Hangar | AirportHeading::Term1 | AirportHeading::Takeoff
        )
        && kind == AirportFtaKind::Country
    {
        return;
    }
    if v.orders.is_empty() {
        return;
    }
    if remote {
        v.airport_heading = AirportHeading::Takeoff;
        if v.aircraft_phase == AircraftPhase::InHangar {
            v.aircraft_phase = AircraftPhase::Taxi;
        }
    } else {
        v.airport_heading = AirportHeading::Term1;
    }
}

fn finish_takeoff(v: &mut Vehicle, station: &mut Station) -> AircraftPhaseEvent {
    release_held_blocks(station, v);
    v.airport_fta_active = false;
    v.airport_fta_station = None;
    v.aircraft_phase = AircraftPhase::Flying;
    v.altitude = AIRCRAFT_CRUISE_ALTITUDE;
    v.airport_heading = AirportHeading::Flying;
    v.aircraft_phase_ticks = 0;
    v.airport_waypoint_reached = false;
    v.airport_loading_stand_reached = false;
    v.airport_subpos_valid = false;
    v.path = straight_line_path(v.pos, v.dest).into();
    v.progress = 0;
    v.set_cruise_speed();
    AircraftPhaseEvent::None
}

/// Resultado de un override FTA por kind/posición.
enum EdgeOverride {
    /// Seguir con la lógica genérica.
    Continue,
    /// Devolver este resultado (incluso `None` = fin de takeoff / sin arista).
    Done(Option<AirportFtaEdge>),
}

fn choose_next_edge_city_metro(
    v: &Vehicle,
    kind: AirportFtaKind,
    edges: &[AirportFtaEdge],
) -> EdgeOverride {
    if v.airport_pos == 11 {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 12).copied());
    }
    if v.airport_pos == 13 && matches!(v.airport_heading, AirportHeading::Landing) {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 14).copied());
    }
    if v.airport_pos == 12 {
        return EdgeOverride::Done(None);
    }
    if kind == AirportFtaKind::City && v.airport_pos == 22 {
        return EdgeOverride::Done(None);
    }
    if kind == AirportFtaKind::Metropolitan && v.airport_pos == 24 {
        return EdgeOverride::Done(None);
    }
    if kind == AirportFtaKind::Metropolitan
        && matches!(v.airport_pos, 16 | 17)
        && matches!(v.airport_heading, AirportHeading::EndLanding)
    {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::EndLanding)
                .copied(),
        );
    }
    if kind == AirportFtaKind::Metropolitan
        && v.airport_pos == 27
        && matches!(
            v.airport_heading,
            AirportHeading::Term1 | AirportHeading::EndLanding
        )
    {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::Term1)
                .copied(),
        );
    }
    EdgeOverride::Continue
}

fn choose_next_edge_helistation(v: &Vehicle, edges: &[AirportFtaEdge]) -> EdgeOverride {
    if v.airport_pos == 2 && matches!(v.airport_heading, AirportHeading::HeliLanding) {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 15).copied());
    }
    if matches!(v.airport_pos, 3 | 12 | 13 | 14) {
        return EdgeOverride::Done(None);
    }
    if v.airport_pos == 16
        && matches!(
            v.airport_heading,
            AirportHeading::HeliEndLanding | AirportHeading::Helipad1
        )
    {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::Helipad1)
                .copied(),
        );
    }
    if v.airport_pos == 5 && matches!(v.airport_heading, AirportHeading::Helipad1) {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::Helipad1)
                .copied(),
        );
    }
    EdgeOverride::Continue
}

fn choose_next_edge_international(v: &Vehicle, edges: &[AirportFtaEdge]) -> EdgeOverride {
    if v.airport_pos == 30 {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 31).copied());
    }
    if v.airport_pos == 32 && matches!(v.airport_heading, AirportHeading::Landing) {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 33).copied());
    }
    if v.airport_pos == 31 || matches!(v.airport_pos, 47 | 48 | 51 | 52) {
        return EdgeOverride::Done(None);
    }
    if v.airport_pos == 36
        && matches!(
            v.airport_heading,
            AirportHeading::EndLanding | AirportHeading::Term1
        )
    {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::ToAll)
                .copied(),
        );
    }
    if v.airport_pos == 23 && matches!(v.airport_heading, AirportHeading::Term1) {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::Term1)
                .copied(),
        );
    }
    EdgeOverride::Continue
}

fn choose_next_edge_intercontinental(v: &Vehicle, edges: &[AirportFtaEdge]) -> EdgeOverride {
    if v.airport_pos == 34 {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 35).copied());
    }
    if v.airport_pos == 61 {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 62).copied());
    }
    // Preferir pista 1: en entry 44 no tomar LANDING→69 todavía.
    if v.airport_pos == 44 && matches!(v.airport_heading, AirportHeading::Landing) {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::ToAll)
                .copied(),
        );
    }
    if v.airport_pos == 46 && matches!(v.airport_heading, AirportHeading::Landing) {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 76).copied());
    }
    if v.airport_pos == 76
        && matches!(
            v.airport_heading,
            AirportHeading::Landing | AirportHeading::ToAll
        )
    {
        return EdgeOverride::Done(edges.iter().find(|e| e.next_position == 37).copied());
    }
    if matches!(v.airport_pos, 35 | 62 | 53 | 54 | 74 | 75) {
        return EdgeOverride::Done(None);
    }
    if matches!(v.airport_pos, 42 | 66)
        && matches!(
            v.airport_heading,
            AirportHeading::EndLanding | AirportHeading::Term1
        )
    {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::ToAll)
                .copied(),
        );
    }
    if v.airport_pos == 26 && matches!(v.airport_heading, AirportHeading::Term1) {
        return EdgeOverride::Done(
            edges
                .iter()
                .find(|e| e.heading == AirportHeading::Term1)
                .copied(),
        );
    }
    EdgeOverride::Continue
}

fn choose_next_edge(v: &Vehicle, profile: &AirportFtaProfile) -> Option<AirportFtaEdge> {
    let edges = (profile.fta_edges)(v.airport_pos);
    if edges.is_empty() {
        return None;
    }
    let want = match v.airport_heading {
        AirportHeading::TermGroup => match profile.kind {
            AirportFtaKind::Helidepot | AirportFtaKind::Heliport | AirportFtaKind::Helistation => {
                AirportHeading::Helipad1
            }
            _ => AirportHeading::Term1,
        },
        AirportHeading::EndTakeoff
            if v.airport_pos == 8 && profile.kind == AirportFtaKind::Country =>
        {
            AirportHeading::StartTakeoff
        }
        h => h,
    };
    if let Some(e) = edges.iter().find(|e| e.heading == want) {
        return Some(*e);
    }
    if profile.kind == AirportFtaKind::Country {
        if v.airport_pos == 8 {
            return edges.iter().find(|e| e.next_position == 9).copied();
        }
        if v.airport_pos == 7 {
            return edges.iter().find(|e| e.next_position == 8).copied();
        }
        if v.airport_pos == 10 && matches!(v.airport_heading, AirportHeading::Landing) {
            return edges.iter().find(|e| e.next_position == 11).copied();
        }
        if v.airport_pos == 9 {
            return None;
        }
    }
    if profile.kind == AirportFtaKind::Helidepot {
        if v.airport_pos == 2 && matches!(v.airport_heading, AirportHeading::HeliLanding) {
            return edges.iter().find(|e| e.next_position == 7).copied();
        }
        if matches!(v.airport_pos, 11 | 15) {
            return None;
        }
    }
    if profile.kind == AirportFtaKind::Heliport {
        if v.airport_pos == 8 && matches!(v.airport_heading, AirportHeading::HeliLanding) {
            return edges.iter().find(|e| e.next_position == 2).copied();
        }
        if v.airport_pos == 1 {
            return None;
        }
        if v.airport_pos == 4
            && matches!(
                v.airport_heading,
                AirportHeading::HeliEndLanding | AirportHeading::Helipad1
            )
        {
            return edges
                .iter()
                .find(|e| e.heading == AirportHeading::Helipad1)
                .copied();
        }
    }
    if profile.kind == AirportFtaKind::Helistation
        && let EdgeOverride::Done(special) = choose_next_edge_helistation(v, &edges)
    {
        return special;
    }
    if profile.kind == AirportFtaKind::Commuter {
        if v.airport_pos == 14 {
            return edges.iter().find(|e| e.next_position == 15).copied();
        }
        if v.airport_pos == 16 && matches!(v.airport_heading, AirportHeading::Landing) {
            return edges.iter().find(|e| e.next_position == 17).copied();
        }
        if v.airport_pos == 15 || matches!(v.airport_pos, 31 | 32 | 37) {
            return None;
        }
    }
    if matches!(
        profile.kind,
        AirportFtaKind::City | AirportFtaKind::Metropolitan
    ) && let EdgeOverride::Done(special) = choose_next_edge_city_metro(v, profile.kind, &edges)
    {
        return special;
    }
    if profile.kind == AirportFtaKind::International
        && let EdgeOverride::Done(special) = choose_next_edge_international(v, &edges)
    {
        return special;
    }
    if profile.kind == AirportFtaKind::Intercontinental
        && let EdgeOverride::Done(special) = choose_next_edge_intercontinental(v, &edges)
    {
        return special;
    }
    if let Some(e) = edges.iter().find(|e| e.heading == AirportHeading::ToAll) {
        return Some(*e);
    }
    edges.first().copied()
}

/// `true` si `blocks` está libre respecto a reservas ajenas (`held` = las del propio avión).
fn blocks_free_for(station: &Station, held: u64, blocks: u64) -> bool {
    blocks == 0 || (station.airport_blocks & !held & blocks) == 0
}

fn release_held_blocks(station: &mut Station, v: &mut Vehicle) {
    station.airport_blocks &= !v.airport_blocks_held;
    v.airport_blocks_held = 0;
}

fn acquire_blocks(station: &mut Station, v: &mut Vehicle, blocks: u64) {
    station.airport_blocks |= blocks;
    v.airport_blocks_held = blocks;
}

fn sync_phase_from_node(v: &mut Vehicle, profile: &AirportFtaProfile) -> AircraftPhaseEvent {
    let idx = usize::from(v.airport_pos).min(profile.moving_data.len() - 1);
    let flags = profile.moving_data[idx].flags;
    if flags & (FLAG_TAKEOFF | FLAG_HELI_RAISE) != 0 {
        v.aircraft_phase = AircraftPhase::Takeoff;
        let done = 12u16.saturating_sub(v.aircraft_phase_ticks);
        #[allow(clippy::cast_possible_truncation)]
        {
            v.altitude = ((u32::from(done) * u32::from(AIRCRAFT_CRUISE_ALTITUDE)) / 12)
                .min(u32::from(AIRCRAFT_CRUISE_ALTITUDE)) as u8;
        }
        return AircraftPhaseEvent::None;
    }
    if flags & (FLAG_LAND | FLAG_BRAKE | FLAG_HELI_LOWER) != 0 {
        v.aircraft_phase = AircraftPhase::Landing;
        #[allow(clippy::cast_possible_truncation)]
        {
            v.altitude = ((u32::from(v.aircraft_phase_ticks) * u32::from(AIRCRAFT_CRUISE_ALTITUDE))
                / 12)
                .min(u32::from(AIRCRAFT_CRUISE_ALTITUDE)) as u8;
        }
        return AircraftPhaseEvent::None;
    }
    if (profile.hold_min..=profile.hold_max).contains(&v.airport_pos)
        || (profile.kind == AirportFtaKind::Country && v.airport_pos == 10)
        || (profile.kind == AirportFtaKind::Helidepot && matches!(v.airport_pos, 2 | 12))
        || (profile.kind == AirportFtaKind::Heliport && matches!(v.airport_pos, 2 | 5..=8))
        || (profile.kind == AirportFtaKind::Helistation
            && matches!(v.airport_pos, 2 | 15 | 23 | 25..=32))
        || (profile.kind == AirportFtaKind::Commuter && matches!(v.airport_pos, 16 | 25 | 26 | 33))
        || (profile.kind == AirportFtaKind::City && matches!(v.airport_pos, 13 | 18..=21 | 25..=29))
        || (profile.kind == AirportFtaKind::Metropolitan && matches!(v.airport_pos, 13 | 19..=22))
        || (profile.kind == AirportFtaKind::International
            && matches!(v.airport_pos, 32 | 37..=40 | 41..=44 | 49))
        || (profile.kind == AirportFtaKind::Intercontinental
            && matches!(v.airport_pos, 36 | 43..=50 | 55 | 69 | 76))
    {
        v.aircraft_phase = AircraftPhase::Flying;
        v.altitude = AIRCRAFT_CRUISE_ALTITUDE;
    } else if v.airport_pos == 0 {
        if profile.kind == AirportFtaKind::Heliport {
            v.aircraft_phase = AircraftPhase::Taxi;
        } else {
            v.aircraft_phase = AircraftPhase::InHangar;
        }
        v.altitude = 0;
    } else {
        v.aircraft_phase = AircraftPhase::Taxi;
        v.altitude = 0;
    }
    AircraftPhaseEvent::None
}

fn ensure_airport_subpos(v: &mut Vehicle) {
    if v.airport_subpos_valid {
        return;
    }
    v.airport_sub_x = v.pos.x.saturating_mul(16).saturating_add(8);
    v.airport_sub_y = v.pos.y.saturating_mul(16).saturating_add(8);
    v.airport_subpos_valid = true;
}

fn direction_from_subpixel_step(dx: i32, dy: i32, fallback: u8) -> u8 {
    match (dx.signum(), dy.signum()) {
        (-1, -1) => DIR_N,
        (-1, 0) => DIR_NE,
        (-1, 1) => DIR_E,
        (0, 1) => DIR_SE,
        (1, 1) => DIR_S,
        (1, 0) => DIR_SW,
        (1, -1) => DIR_W,
        (0, -1) => DIR_NW,
        _ => fallback,
    }
}

/// Avanza hacia el `AirportMovingData` actual en coordenadas 1/16 de tesela.
/// `OpenTTD` no salta entre nodos FTA: sólo elige el nodo siguiente cuando la
/// posición de píxel alcanza estas coordenadas.
fn move_towards_waypoint(
    v: &mut Vehicle,
    map: &Map,
    station: &Station,
    profile: &AirportFtaProfile,
) -> bool {
    ensure_airport_subpos(v);
    v.path.clear();
    v.progress = 0;
    if v.airport_waypoint_reached {
        return false;
    }

    let nw = airport_nw_origin(station);
    let idx = usize::from(v.airport_pos).min(profile.moving_data.len() - 1);
    let md = profile.moving_data[idx];
    let (map_w, map_h) = map.dimensions();
    let max_x = i32::try_from(map_w)
        .unwrap_or(i32::MAX / 16)
        .saturating_mul(16)
        .saturating_sub(1);
    let max_y = i32::try_from(map_h)
        .unwrap_or(i32::MAX / 16)
        .saturating_mul(16)
        .saturating_sub(1);
    // La tabla Oilrig usa el NW de la plataforma industrial original; nuestro
    // Oilrig simplificado guarda directamente la tesela del helipad. Normalizar
    // los nodos de suelo evita desplazar el helicóptero una tesela al este.
    let local_x =
        if profile.spec == crate::airport_class::AirportSpecId::Oilrig && v.airport_pos <= 4 {
            i32::from(md.x) - 26
        } else {
            i32::from(md.x)
        };
    let target_x =
        nw.x.saturating_mul(16)
            .saturating_add(local_x)
            .clamp(0, max_x);
    let target_y =
        nw.y.saturating_mul(16)
            .saturating_add(i32::from(md.y))
            .clamp(0, max_y);
    let dx = target_x - v.airport_sub_x;
    let dy = target_y - v.airport_sub_y;
    if dx == 0 && dy == 0 {
        v.airport_waypoint_reached = true;
        return true;
    }

    let fast = md.flags
        & (FLAG_NO_SPEED_CLAMP
            | FLAG_HOLD
            | FLAG_TAKEOFF
            | FLAG_LAND
            | FLAG_HELI_RAISE
            | FLAG_HELI_LOWER)
        != 0;
    let rate = if fast { 4 } else { 1 };
    let step_x = dx.clamp(-rate, rate);
    let step_y = dy.clamp(-rate, rate);
    v.airport_sub_x += step_x;
    v.airport_sub_y += step_y;
    v.direction = direction_from_subpixel_step(step_x, step_y, md.direction);
    v.pos = TileCoord::new(
        v.airport_sub_x.div_euclid(16),
        v.airport_sub_y.div_euclid(16),
    );
    if v.airport_sub_x == target_x && v.airport_sub_y == target_y {
        v.airport_waypoint_reached = true;
        return true;
    }
    false
}
