//! Comandos de carteles (`PlaceSign` / `RemoveSign` / `RenameSign`).

use crate::map::TileCoord;
use crate::sign::{MAX_SIGN_NAME_CHARS, Sign};
use crate::{GameState, command::CommandError, command::in_bounds};

fn normalize_sign_name(name: Option<String>) -> Result<Option<String>, CommandError> {
    let Some(raw) = name else {
        return Ok(None);
    };
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_SIGN_NAME_CHARS {
        return Err(CommandError::SignNameTooLong);
    }
    Ok(Some(trimmed))
}

/// Coloca un cartel en `pos` con nombre opcional (por defecto `Cartel {id}`).
pub(crate) fn place_sign(
    state: &mut GameState,
    pos: TileCoord,
    name: Option<String>,
) -> Result<(), CommandError> {
    in_bounds(&state.map, pos)?;
    let id = state.next_sign_id;
    state.next_sign_id = state.next_sign_id.saturating_add(1).max(1);
    let label = match normalize_sign_name(name)? {
        Some(n) => n,
        None => format!("Cartel {id}"),
    };
    state.signs.push(Sign::new(id, pos, label));
    Ok(())
}

pub(crate) fn remove_sign(state: &mut GameState, sign_id: u32) -> Result<(), CommandError> {
    let Some(idx) = state.signs.iter().position(|s| s.id == sign_id) else {
        return Err(CommandError::SignNotFound);
    };
    state.signs.remove(idx);
    Ok(())
}

pub(crate) fn rename_sign(
    state: &mut GameState,
    sign_id: u32,
    name: Option<String>,
) -> Result<(), CommandError> {
    let Some(sign) = state.signs.iter_mut().find(|s| s.id == sign_id) else {
        return Err(CommandError::SignNotFound);
    };
    let normalized = normalize_sign_name(name)?;
    let Some(label) = normalized else {
        return Err(CommandError::SignNameEmpty);
    };
    sign.name = label;
    Ok(())
}

/// Quita carteles anclados a la tesela (p. ej. al demoler).
pub(crate) fn remove_signs_at(state: &mut GameState, pos: TileCoord) {
    state.signs.retain(|s| s.pos != pos);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::command::{Command, apply_command};

    #[test]
    fn place_rename_remove_sign_roundtrip() {
        let mut state = GameState::new(8, 8);
        let pos = TileCoord::new(2, 3);
        apply_command(&mut state, &Command::PlaceSign { pos, name: None }).unwrap();
        assert_eq!(state.signs.len(), 1);
        let id = state.signs[0].id;
        assert_eq!(state.signs[0].name, format!("Cartel {id}"));
        apply_command(
            &mut state,
            &Command::RenameSign {
                sign_id: id,
                name: Some("  Mirador  ".into()),
            },
        )
        .unwrap();
        assert_eq!(state.signs[0].name, "Mirador");
        apply_command(&mut state, &Command::RemoveSign { sign_id: id }).unwrap();
        assert!(state.signs.is_empty());
    }

    #[test]
    fn clear_tile_removes_sign_at_tile() {
        let mut state = GameState::new(8, 8);
        let pos = TileCoord::new(1, 1);
        apply_command(
            &mut state,
            &Command::PlaceSign {
                pos,
                name: Some("X".into()),
            },
        )
        .unwrap();
        apply_command(&mut state, &Command::ClearTile(pos)).unwrap();
        assert!(state.signs.is_empty());
    }
}
