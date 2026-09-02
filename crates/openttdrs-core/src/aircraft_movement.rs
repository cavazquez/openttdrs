//! Movimiento y fases de vuelo de aviones.

use crate::airport::{AirportPiece, airport_runway_tile, airport_tile_is_hangar};
use crate::map::{Map, TileCoord, TileKind};
use crate::station::Station;
use crate::vehicle::{AircraftPhase, Vehicle, VehicleKind};

/// Altitud de crucero (unidades de altura de tesela para offset visual).
pub const AIRCRAFT_CRUISE_ALTITUDE: u8 = 8;
/// Ticks de despegue / aterrizaje.
pub const AIRCRAFT_TAKEOFF_TICKS: u16 = 24;
pub const AIRCRAFT_LANDING_TICKS: u16 = 24;
/// Distancia Manhattan al destino para iniciar aterrizaje.
pub const AIRCRAFT_LANDING_APPROACH: i32 = 3;

/// Ruta en línea recta (pasos Manhattan hacia el destino).
#[must_use]
pub fn straight_line_path(from: TileCoord, to: TileCoord) -> Vec<TileCoord> {
    if from == to {
        return vec![];
    }
    let mut path = Vec::new();
    let mut cur = from;
    while cur != to {
        let dx = to.x - cur.x;
        let dy = to.y - cur.y;
        cur = if dx.abs() >= dy.abs() {
            TileCoord::new(cur.x + dx.signum(), cur.y)
        } else {
            TileCoord::new(cur.x, cur.y + dy.signum())
        };
        path.push(cur);
    }
    path
}

#[must_use]
pub fn aircraft_requires_path(kind: VehicleKind) -> bool {
    kind == VehicleKind::Aircraft
}

/// Estación aeropuerto que cubre `pos` (hangar o footprint).
#[must_use]
pub fn airport_station_at(stations: &[Station], pos: TileCoord) -> Option<&Station> {
    stations
        .iter()
        .find(|s| s.stop_kind == crate::station::StopKind::Airport && s.covers_tile(pos))
}

/// Actualiza fase/altitud de un avión una vez por tick (antes o después de `step`).
pub fn tick_aircraft_phase(
    v: &mut Vehicle,
    map: &Map,
    stations: &mut [Station],
) -> AircraftPhaseEvent {
    tick_aircraft_phase_with_catalog(v, map, stations, &[])
}

/// Variante que resuelve velocidad y callbacks contra el catálogo activo de
/// la partida. La API histórica conserva el fallback vanilla.
pub fn tick_aircraft_phase_with_catalog(
    v: &mut Vehicle,
    map: &Map,
    stations: &mut [Station],
    engine_catalog: &[crate::engine::EngineDef],
) -> AircraftPhaseEvent {
    if v.kind != VehicleKind::Aircraft {
        return AircraftPhaseEvent::None;
    }
    // Country/Small: motor FTA cuando aplica.
    if let Some(ev) =
        crate::airport_fta::tick_country_airport_fta_with_catalog(v, map, stations, engine_catalog)
    {
        return ev;
    }
    match v.aircraft_phase {
        AircraftPhase::InHangar => {
            if v.running && v.pos != v.dest && !v.orders.is_empty() {
                // Salir a taxi hacia runway (o dest si helipuerto).
                let runway = airport_station_at(stations, v.pos)
                    .and_then(|s| airport_runway_tile(s, map))
                    .unwrap_or(v.dest);
                v.aircraft_phase = AircraftPhase::Taxi;
                v.altitude = 0;
                if runway != v.pos {
                    v.path = straight_line_path(v.pos, runway).into();
                }
            }
            AircraftPhaseEvent::None
        }
        AircraftPhase::Taxi => {
            let on_runway = map.get(v.pos).is_some_and(|t| {
                t.kind == TileKind::Airport && AirportPiece::from_m5(t.m5).is_runway()
            }) || (airport_tile_is_hangar(map, v.pos)
                && airport_station_at(stations, v.pos).is_some_and(|s| s.airport_tiles.len() <= 1));
            if on_runway && v.path.is_empty() {
                v.aircraft_phase = AircraftPhase::Takeoff;
                v.aircraft_phase_ticks = AIRCRAFT_TAKEOFF_TICKS;
                v.cur_speed = v.cur_speed.max(32);
                return AircraftPhaseEvent::Takeoff;
            }
            AircraftPhaseEvent::None
        }
        AircraftPhase::Takeoff => {
            if v.aircraft_phase_ticks > 0 {
                v.aircraft_phase_ticks -= 1;
                let done = AIRCRAFT_TAKEOFF_TICKS.saturating_sub(v.aircraft_phase_ticks);
                let alt = (u32::from(done) * u32::from(AIRCRAFT_CRUISE_ALTITUDE))
                    / u32::from(AIRCRAFT_TAKEOFF_TICKS.max(1));
                v.altitude = u8::try_from(alt.min(u32::from(AIRCRAFT_CRUISE_ALTITUDE)))
                    .unwrap_or(AIRCRAFT_CRUISE_ALTITUDE);
            }
            if v.aircraft_phase_ticks == 0 {
                v.aircraft_phase = AircraftPhase::Flying;
                v.altitude = AIRCRAFT_CRUISE_ALTITUDE;
                v.path = straight_line_path(v.pos, v.dest).into();
                let engine = crate::newgrf_callback::engine_for_vehicle_catalog(engine_catalog, v);
                v.cur_speed = crate::newgrf_callback::vehicle_max_speed(engine, v);
                v.subspeed = 0;
            }
            AircraftPhaseEvent::None
        }
        AircraftPhase::Flying => {
            let dist = (v.pos.x - v.dest.x).abs() + (v.pos.y - v.dest.y).abs();
            if dist <= AIRCRAFT_LANDING_APPROACH || v.pos == v.dest {
                let runway = airport_station_at(stations, v.dest)
                    .and_then(|s| airport_runway_tile(s, map))
                    .unwrap_or(v.dest);
                v.aircraft_phase = AircraftPhase::Landing;
                v.aircraft_phase_ticks = AIRCRAFT_LANDING_TICKS;
                if v.pos != runway {
                    v.path = straight_line_path(v.pos, runway).into();
                }
                return AircraftPhaseEvent::Landing;
            }
            AircraftPhaseEvent::None
        }
        AircraftPhase::Landing => {
            if v.aircraft_phase_ticks > 0 {
                v.aircraft_phase_ticks -= 1;
                let left = v.aircraft_phase_ticks;
                let alt = (u32::from(left) * u32::from(AIRCRAFT_CRUISE_ALTITUDE))
                    / u32::from(AIRCRAFT_LANDING_TICKS.max(1));
                v.altitude = u8::try_from(alt.min(u32::from(AIRCRAFT_CRUISE_ALTITUDE)))
                    .unwrap_or(AIRCRAFT_CRUISE_ALTITUDE);
            }
            if v.aircraft_phase_ticks == 0 {
                v.altitude = 0;
                // Taxi al destino de carga / hangar.
                if v.pos != v.dest {
                    v.aircraft_phase = AircraftPhase::Taxi;
                    v.path = straight_line_path(v.pos, v.dest).into();
                } else if airport_tile_is_hangar(map, v.pos) {
                    v.aircraft_phase = AircraftPhase::InHangar;
                } else {
                    v.aircraft_phase = AircraftPhase::Taxi;
                }
            }
            AircraftPhaseEvent::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AircraftPhaseEvent {
    None,
    Takeoff,
    Landing,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::GameState;
    use crate::engine::ENGINE_AIRCRAFT_TRICARIO;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::{Command, TileCoord, TileKind, apply_command};

    use super::*;

    #[test]
    fn straight_line_path_is_direct() {
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(4, 2);
        let path = straight_line_path(from, to);
        assert_eq!(path.last().copied(), Some(to));
        assert_eq!(path.len(), 6);
    }

    #[test]
    fn air_pathfinder_ignores_terrain() {
        let s = GameState::new(8, 8);
        let from = TileCoord::new(0, 0);
        let to = TileCoord::new(5, 3);
        let path = find_path(&s.map, from, to, PathNetwork::Air).expect("ruta aérea");
        assert_eq!(path.last().copied(), Some(to));
    }

    #[test]
    fn aircraft_flies_straight_to_destination() {
        let mut s = GameState::new(16, 16);
        let airport = TileCoord::new(2, 2);
        s.map.set_kind(airport, TileKind::Airport).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(airport, ENGINE_AIRCRAFT_TRICARIO),
        )
        .unwrap();
        let dest = TileCoord::new(10, 6);
        s.vehicles[0].dest = dest;
        s.vehicles[0].running = true;
        s.vehicles[0].aircraft_phase = AircraftPhase::Flying;
        s.vehicles[0].altitude = AIRCRAFT_CRUISE_ALTITUDE;
        s.vehicles[0].path = find_path(&s.map, airport, dest, PathNetwork::Air)
            .unwrap()
            .into();
        s.vehicles[0].set_cruise_speed();
        for _ in 0..800 {
            s.vehicles[0].step();
            if s.vehicles[0].pos == dest {
                break;
            }
        }
        assert_eq!(s.vehicles[0].pos, dest);
    }
}
