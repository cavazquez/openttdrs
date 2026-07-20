//! Loop FTA por tick (`AirportGoToNextPosition` simplificado).

use crate::aircraft_movement::{AIRCRAFT_CRUISE_ALTITUDE, AircraftPhaseEvent, straight_line_path};
use crate::map::{Map, TileCoord};
use crate::station::Station;
use crate::vehicle::{AircraftPhase, Vehicle, VehicleKind};

use super::profile::fta_profile_for_spec;
use super::types::{
    AirportFtaEdge, AirportFtaKind, AirportFtaProfile, AirportHeading, BLOCK_AIRPORT_BUSY,
    FLAG_BRAKE, FLAG_HELI_LOWER, FLAG_HELI_RAISE, FLAG_LAND, FLAG_TAKEOFF,
};

const FTA_DWELL_TICKS: u16 = 6;
const FTA_HOLD_DWELL_TICKS: u16 = 10;

/// `true` si la estación usa motor FTA (Country/Small o Helidepot).
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

/// Tick FTA para specs soportados (Country + Helidepot).
pub fn tick_airport_fta(
    v: &mut Vehicle,
    _map: &Map,
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
    if should_finish_takeoff(v, &profile) {
        return Some(finish_takeoff(v, &mut stations[st_idx]));
    }
    update_heading_for_orders(v, &stations[st_idx], profile.kind);
    if v.aircraft_phase_ticks > 0 {
        v.aircraft_phase_ticks -= 1;
        let ev = sync_phase_from_node(v, &profile);
        apply_waypoint_pose(v, &stations[st_idx], &profile);
        return Some(ev);
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
    v.airport_pos = entry;
    v.airport_prev_pos = entry;
    v.airport_heading = match profile.kind {
        AirportFtaKind::Helidepot => AirportHeading::HeliLanding,
        AirportFtaKind::Country => AirportHeading::Landing,
    };
    v.aircraft_phase = AircraftPhase::Landing;
    v.aircraft_phase_ticks = FTA_HOLD_DWELL_TICKS;
    v.path.clear();
    apply_waypoint_pose(v, st, &profile);
    Some(AircraftPhaseEvent::Landing)
}

fn should_finish_takeoff(v: &Vehicle, profile: &AirportFtaProfile) -> bool {
    if !v.airport_fta_active || v.aircraft_phase_ticks != 0 {
        return false;
    }
    if let Some(pos) = profile.fixedwing_takeoff_pos
        && v.airport_pos == pos
        && matches!(
            v.airport_heading,
            AirportHeading::EndTakeoff | AirportHeading::Takeoff | AirportHeading::StartTakeoff
        )
    {
        return true;
    }
    // Helidepot: nodos 11/15 con raise y heading heli takeoff.
    if profile.kind == AirportFtaKind::Helidepot
        && matches!(v.airport_pos, 11 | 15)
        && matches!(v.airport_heading, AirportHeading::HeliTakeoff)
    {
        return true;
    }
    false
}

fn advance_fta_node(
    v: &mut Vehicle,
    station: &mut Station,
    profile: &AirportFtaProfile,
) -> AircraftPhaseEvent {
    let prev = v.airport_pos;
    clear_blocks_for_pos(station, prev, profile);
    // Un avión: liberar antes de reservar el siguiente (multi-avión vendrá después).
    station.airport_blocks = 0;
    nudge_heading_at_node(v, profile);

    let Some(edge) = choose_next_edge(v, profile) else {
        if should_finish_takeoff(v, profile)
            || (profile.fixedwing_takeoff_pos == Some(v.airport_pos))
        {
            return finish_takeoff(v, station);
        }
        apply_waypoint_pose(v, station, profile);
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
    if !try_reserve_blocks(station, edge.blocks) {
        hold_or_wait(v, profile);
        let ev = sync_phase_from_node(v, profile);
        apply_waypoint_pose(v, station, profile);
        return ev;
    }

    v.airport_prev_pos = prev;
    v.airport_pos = next;
    let md = profile.moving_data[usize::from(next).min(profile.moving_data.len() - 1)];
    v.direction = md.direction;
    v.aircraft_phase_ticks = dwell_for_node(next, md.flags, profile);
    apply_enter_heading(v, next, profile);

    let ev = sync_phase_from_node(v, profile);
    apply_waypoint_pose(v, station, profile);
    if profile.fixedwing_takeoff_pos == Some(next)
        || (profile.kind == AirportFtaKind::Helidepot
            && matches!(next, 11 | 15)
            && md.flags & FLAG_HELI_RAISE != 0)
    {
        return AircraftPhaseEvent::Takeoff;
    }
    if md.flags & FLAG_LAND != 0 || md.flags & FLAG_HELI_LOWER != 0 {
        return AircraftPhaseEvent::Landing;
    }
    ev
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
    }
}

fn hold_or_wait(v: &mut Vehicle, profile: &AirportFtaProfile) {
    if (profile.hold_min..=profile.hold_max).contains(&v.airport_pos) {
        v.airport_prev_pos = v.airport_pos;
        v.airport_pos = if v.airport_pos >= profile.hold_max {
            match profile.kind {
                AirportFtaKind::Country => 10,
                AirportFtaKind::Helidepot => 2,
            }
        } else {
            v.airport_pos.saturating_add(1)
        };
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

fn apply_enter_heading(v: &mut Vehicle, next: u8, profile: &AirportFtaProfile) {
    match profile.kind {
        AirportFtaKind::Country => match next {
            7 => v.airport_heading = AirportHeading::Takeoff,
            8 => v.airport_heading = AirportHeading::StartTakeoff,
            9 => v.airport_heading = AirportHeading::EndTakeoff,
            2 => {
                v.airport_heading = AirportHeading::Term1;
                v.dest = v.pos;
            }
            3 => {
                v.airport_heading = AirportHeading::Term2;
                v.dest = v.pos;
            }
            13 => v.airport_heading = AirportHeading::EndLanding,
            14 => v.airport_heading = AirportHeading::Term1,
            1 if matches!(v.airport_heading, AirportHeading::EndLanding) => {
                v.airport_heading = AirportHeading::Term1;
            }
            _ => {}
        },
        AirportFtaKind::Helidepot => match next {
            14 => {
                v.airport_heading = AirportHeading::Helipad1;
                v.dest = v.pos;
            }
            11 | 15 | 17 => v.airport_heading = AirportHeading::HeliTakeoff,
            7 | 8 => v.airport_heading = AirportHeading::HeliLanding,
            10 => v.airport_heading = AirportHeading::Helipad1,
            _ => {}
        },
    }
}

fn resolve_fta_station_idx(v: &Vehicle, stations: &[Station]) -> Option<usize> {
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
}

/// Inicializa estado FTA al comprar en hangar con perfil soportado.
pub fn init_country_fta_on_purchase(v: &mut Vehicle) {
    activate_fta_in_hangar(v);
}

/// Alias explícito del init FTA.
pub fn init_airport_fta_on_purchase(v: &mut Vehicle) {
    init_country_fta_on_purchase(v);
}

fn update_heading_for_orders(v: &mut Vehicle, station: &Station, kind: AirportFtaKind) {
    let remote = !station.covers_tile(v.dest) && v.pos != v.dest;
    match kind {
        AirportFtaKind::Country => {
            if v.airport_pos != 0 && !matches!(v.airport_heading, AirportHeading::Hangar) {
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
        AirportFtaKind::Helidepot => {
            if v.orders.is_empty() {
                return;
            }
            // No pisar hold/aproximación/aterrizaje (evita circular en 2..6 con Helipad1).
            if matches!(
                v.airport_heading,
                AirportHeading::HeliLanding | AirportHeading::HeliEndLanding
            ) || matches!(v.airport_pos, 2..=9 | 12 | 13)
            {
                if v.airport_pos == 8 && matches!(v.airport_heading, AirportHeading::HeliEndLanding)
                {
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
    }
}

fn finish_takeoff(v: &mut Vehicle, station: &mut Station) -> AircraftPhaseEvent {
    station.airport_blocks = 0;
    v.airport_fta_active = false;
    v.aircraft_phase = AircraftPhase::Flying;
    v.altitude = AIRCRAFT_CRUISE_ALTITUDE;
    v.airport_heading = AirportHeading::Flying;
    v.aircraft_phase_ticks = 0;
    v.path = straight_line_path(v.pos, v.dest).into();
    v.set_cruise_speed();
    AircraftPhaseEvent::None
}

fn choose_next_edge(v: &Vehicle, profile: &AirportFtaProfile) -> Option<AirportFtaEdge> {
    let edges = (profile.fta_edges)(v.airport_pos);
    if edges.is_empty() {
        return None;
    }
    let want = match v.airport_heading {
        AirportHeading::TermGroup => match profile.kind {
            AirportFtaKind::Country => AirportHeading::Term1,
            AirportFtaKind::Helidepot => AirportHeading::Helipad1,
        },
        AirportHeading::EndTakeoff if v.airport_pos == 8 => AirportHeading::StartTakeoff,
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
    if let Some(e) = edges.iter().find(|e| e.heading == AirportHeading::ToAll) {
        return Some(*e);
    }
    edges.first().copied()
}

fn try_reserve_blocks(station: &mut Station, blocks: u64) -> bool {
    if blocks == 0 {
        return true;
    }
    if station.airport_blocks & blocks != 0 {
        return false;
    }
    station.airport_blocks |= blocks;
    true
}

fn clear_blocks_for_pos(station: &mut Station, pos: u8, profile: &AirportFtaProfile) {
    for e in (profile.fta_edges)(pos) {
        station.airport_blocks &= !e.blocks;
    }
    if profile.kind == AirportFtaKind::Country && matches!(pos, 6 | 7 | 11 | 12 | 13 | 14) {
        station.airport_blocks &= !BLOCK_AIRPORT_BUSY;
    }
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
    {
        v.aircraft_phase = AircraftPhase::Flying;
        v.altitude = AIRCRAFT_CRUISE_ALTITUDE;
    } else if v.airport_pos == 0 {
        v.aircraft_phase = AircraftPhase::InHangar;
        v.altitude = 0;
    } else {
        v.aircraft_phase = AircraftPhase::Taxi;
        v.altitude = 0;
    }
    AircraftPhaseEvent::None
}

fn apply_waypoint_pose(v: &mut Vehicle, station: &Station, profile: &AirportFtaProfile) {
    let nw = airport_nw_origin(station);
    let idx = usize::from(v.airport_pos).min(profile.moving_data.len() - 1);
    let md = profile.moving_data[idx];
    let mut tx = i32::from(md.x) / 16;
    let mut ty = i32::from(md.y) / 16;
    let in_hold = (profile.hold_min..=profile.hold_max).contains(&v.airport_pos);
    let airish = in_hold
        || (profile.kind == AirportFtaKind::Country && matches!(v.airport_pos, 9 | 10))
        || (profile.kind == AirportFtaKind::Helidepot
            && matches!(v.airport_pos, 2 | 11 | 12 | 15 | 16));
    if airish {
        tx = tx.clamp(-1, profile.footprint_w);
        ty = ty.clamp(-1, profile.footprint_h);
    } else {
        tx = tx.clamp(0, profile.footprint_w - 1);
        ty = ty.clamp(0, profile.footprint_h - 1);
    }
    let candidate = TileCoord::new(nw.x + tx, nw.y + ty);
    if station.covers_tile(candidate) || airish {
        v.pos = candidate;
    } else if let Some(&c) = station.airport_tiles.first() {
        v.pos = c;
    }
    v.path.clear();
}
