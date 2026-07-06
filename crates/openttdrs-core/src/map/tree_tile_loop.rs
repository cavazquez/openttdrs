//! Crecimiento de árboles y campos (`tree_cmd.cpp`, `clear_cmd.cpp`).

use crate::GameState;
use crate::economy::TICKS_PER_TRANSIT_DAY;
use crate::map::{Map, TileCoord, TileKind};
use crate::world_gen::{CLEAR_GROUND_GRASS, CLEAR_GROUND_SNOW, Climate, clear_ground_m5};

/// Intervalo entre actualizaciones de crecimiento (8 ticks lógicos).
pub const TREE_GROWTH_TICK_INTERVAL: u64 = 8;
/// Etapa máxima de árbol o cultivo (0–7).
pub const MAX_TREE_OR_FIELD_STAGE: u8 = 7;
const FIELD_STAGE_MASK: u8 = 0x07;

#[must_use]
pub const fn tree_or_field_stage(m5: u8) -> u8 {
    m5 & FIELD_STAGE_MASK
}

#[must_use]
pub const fn with_tree_or_field_stage(m5: u8, stage: u8) -> u8 {
    (m5 & !FIELD_STAGE_MASK) | (stage & FIELD_STAGE_MASK)
}

/// Avanza árboles y cultivos cada [`TREE_GROWTH_TICK_INTERVAL`] ticks.
pub fn step_tree_and_field_growth(map: &mut Map, tick: u64) {
    if tick == 0 || !tick.is_multiple_of(TREE_GROWTH_TICK_INTERVAL) {
        return;
    }
    let (w, h) = map.dimensions();
    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(tile) = map.get(c) else {
                continue;
            };
            let stage = tree_or_field_stage(tile.m5);
            if stage >= MAX_TREE_OR_FIELD_STAGE {
                continue;
            }
            if matches!(tile.kind, TileKind::Forest | TileKind::CoalField)
                || (tile.kind == TileKind::Grass && stage > 0)
            {
                let new_m5 = with_tree_or_field_stage(tile.m5, stage + 1);
                let _ = map.set_mapt_m5(c, tile.mapt, new_m5);
            }
        }
    }
}

/// Nieve estacional simplificada para clima ártico (línea de nieve por latitud + estación).
pub fn apply_seasonal_snow(map: &mut Map, climate: Climate, tick: u64, world_seed: u64) {
    if !climate.uses_snow_ground() {
        return;
    }
    if tick == 0 || !tick.is_multiple_of(u64::from(TICKS_PER_TRANSIT_DAY)) {
        return;
    }
    let day = tick / u64::from(TICKS_PER_TRANSIT_DAY);
    let day_of_year = day % 365;
    let winter = (300..=364).contains(&day_of_year) || day_of_year < 75;
    let (_, h) = map.dimensions();
    let snow_line = i32::try_from(h).unwrap_or(0) * 2 / 5;

    for y in 0..h {
        for x in 0..map.dimensions().0 {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind != TileKind::Grass {
                continue;
            }
            let ground = if winter && c.y < snow_line {
                CLEAR_GROUND_SNOW
            } else {
                CLEAR_GROUND_GRASS
            };
            let density = tile.m5 & 0x03;
            let new_m5 = clear_ground_m5(ground, density);
            if new_m5 != tile.m5 {
                let _ = map.set_mapt_m5(c, tile.mapt, new_m5);
            }
        }
    }
    let _ = world_seed;
}

/// Coloca un árbol (hierba → bosque etapa 0, o incrementa etapa).
pub fn plant_tree(
    game_state: &mut GameState,
    c: TileCoord,
) -> Result<(), crate::command::CommandError> {
    use crate::command::{CommandError, in_bounds};
    in_bounds(&game_state.map, c)?;
    let kind = game_state
        .map
        .get_kind(c)
        .ok_or(CommandError::OutOfBounds)?;
    match kind {
        TileKind::Grass => {
            let m5 = with_tree_or_field_stage(0, 0);
            game_state
                .map
                .set_kind(c, TileKind::Forest)
                .map_err(|_| CommandError::OutOfBounds)?;
            game_state
                .map
                .set_mapt_m5(c, 0x40, m5)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        TileKind::Forest => {
            let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            let stage = tree_or_field_stage(tile.m5);
            if stage >= MAX_TREE_OR_FIELD_STAGE {
                return Err(CommandError::CannotPlantTreeHere);
            }
            let new_m5 = with_tree_or_field_stage(tile.m5, stage + 1);
            game_state
                .map
                .set_mapt_m5(c, tile.mapt, new_m5)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        _ => return Err(CommandError::CannotPlantTreeHere),
    }
    Ok(())
}

/// Quita árbol o reduce etapa; hierba/bosque etapa 0 → hierba limpia.
pub fn clear_tree(
    game_state: &mut GameState,
    c: TileCoord,
) -> Result<(), crate::command::CommandError> {
    use crate::command::{CommandError, in_bounds};
    in_bounds(&game_state.map, c)?;
    let kind = game_state
        .map
        .get_kind(c)
        .ok_or(CommandError::OutOfBounds)?;
    match kind {
        TileKind::Forest => {
            let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            let stage = tree_or_field_stage(tile.m5);
            if stage == 0 {
                game_state
                    .map
                    .set_kind(c, TileKind::Grass)
                    .map_err(|_| CommandError::OutOfBounds)?;
                game_state
                    .map
                    .set_mapt_m5(c, 0x00, 0x00)
                    .map_err(|_| CommandError::OutOfBounds)?;
            } else {
                let new_m5 = with_tree_or_field_stage(tile.m5, stage - 1);
                game_state
                    .map
                    .set_mapt_m5(c, tile.mapt, new_m5)
                    .map_err(|_| CommandError::OutOfBounds)?;
            }
        }
        TileKind::CoalField => {
            let tile = game_state.map.get(c).ok_or(CommandError::OutOfBounds)?;
            let growth = tree_or_field_stage(tile.m5);
            if growth == 0 {
                return Err(CommandError::NoTreeHere);
            }
            let new_m5 = with_tree_or_field_stage(tile.m5, growth - 1);
            game_state
                .map
                .set_mapt_m5(c, tile.mapt, new_m5)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        _ => return Err(CommandError::NoTreeHere),
    }
    Ok(())
}

/// Hook combinado para `sim_step`.
pub fn tick_tree_tile_loop(state: &mut GameState) {
    let tick = state.tick.get();
    step_tree_and_field_growth(&mut state.map, tick);
    apply_seasonal_snow(&mut state.map, state.climate, tick, state.world_seed);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Command, GameState, apply_command};

    #[test]
    fn tree_grows_every_eight_ticks() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        apply_command(&mut state, &Command::PlantTree(c)).unwrap();
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 0);
        state.tick = crate::GameTick::new(TREE_GROWTH_TICK_INTERVAL);
        step_tree_and_field_growth(&mut state.map, state.tick.get());
        assert_eq!(tree_or_field_stage(state.map.get(c).unwrap().m5), 1);
    }

    #[test]
    fn plant_and_clear_tree_on_grass() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(2, 2);
        plant_tree(&mut state, c).unwrap();
        assert_eq!(state.map.get_kind(c), Some(TileKind::Forest));
        clear_tree(&mut state, c).unwrap();
        assert_eq!(state.map.get_kind(c), Some(TileKind::Grass));
    }

    #[test]
    fn field_stage_caps_at_seven() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(0, 0);
        state.map.set_kind(c, TileKind::CoalField).unwrap();
        state
            .map
            .set_mapt_m5(c, 0x50, MAX_TREE_OR_FIELD_STAGE)
            .unwrap();
        step_tree_and_field_growth(&mut state.map, TREE_GROWTH_TICK_INTERVAL);
        assert_eq!(
            tree_or_field_stage(state.map.get(c).unwrap().m5),
            MAX_TREE_OR_FIELD_STAGE
        );
    }
}
