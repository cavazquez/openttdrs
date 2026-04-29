//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

use crate::map::{TileCoord, TileKind};
use crate::{GameState, Station};

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Añade una estación y marca la tesela como `TileKind::Station`.
    PlaceStation(TileCoord),
}

/// Fallo al aplicar un comando (estado sin cambios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    OutOfBounds,
    CannotPlaceRoadOnWater,
    CannotPlaceRoadOnVoid,
    CannotPlaceStationOnWater,
    CannotPlaceStationOnVoid,
    StationAlreadyExists,
}

/// Aplica `cmd` a `state` o devuelve error sin mutar.
///
/// # Errors
///
/// Ver variantes de [`CommandError`].
pub fn apply_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    match cmd {
        Command::PlaceRoad(c) => place_road(state, *c),
        Command::PlaceStation(c) => place_station(state, *c),
    }
}

fn place_road(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => {
            state
                .map
                .set_kind(c, TileKind::Road)
                .map_err(|_| CommandError::OutOfBounds)?;
            Ok(())
        }
    }
}

fn place_station(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    if state.stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceStationOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => {
            state
                .map
                .set_kind(c, TileKind::Station)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.stations.push(Station::new(c));
            Ok(())
        }
    }
}

fn in_bounds(map: &crate::map::Map, c: TileCoord) -> Result<(), CommandError> {
    if map.get(c).is_none() {
        Err(CommandError::OutOfBounds)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{GameState, TileKind};

    #[test]
    fn place_road_mutates_tile_kind() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(3, 4);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
        apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Road));
    }

    #[test]
    fn place_road_on_water_returns_error() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(1, 1);
        s.map.set_kind(c, TileKind::Water).unwrap();
        let e = apply_command(&mut s, &Command::PlaceRoad(c)).unwrap_err();
        assert_eq!(e, CommandError::CannotPlaceRoadOnWater);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
    }

    #[test]
    fn command_sequence_is_deterministic() {
        let cmds = [
            Command::PlaceRoad(TileCoord::new(0, 0)),
            Command::PlaceRoad(TileCoord::new(1, 0)),
            Command::PlaceStation(TileCoord::new(2, 0)),
        ];
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for cmd in &cmds {
            apply_command(&mut a, cmd).unwrap();
            apply_command(&mut b, cmd).unwrap();
        }
        let ja = a.save_json().unwrap();
        let jb = b.save_json().unwrap();
        assert_eq!(ja, jb);
    }

    #[test]
    fn place_station_duplicate_errors() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::PlaceStation(c)).unwrap();
        let e = apply_command(&mut s, &Command::PlaceStation(c)).unwrap_err();
        assert_eq!(e, CommandError::StationAlreadyExists);
        assert_eq!(s.stations.len(), 1);
    }
}
