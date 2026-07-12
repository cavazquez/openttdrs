//! Comandos de edición del stack `NewGRF` (config + apply Action0 metadatos).

use crate::newgrf_actions::apply_newgrf_stack_catalogs_default_dirs;
use crate::newgrf_config::{GrfStackIssue, NewGrfEntry, validate_stack};
use crate::{GameState, command::CommandError};

fn index_ok(state: &GameState, index: usize) -> Result<(), CommandError> {
    if index >= state.newgrf_stack.len() {
        Err(CommandError::NewGrfIndexOutOfRange)
    } else {
        Ok(())
    }
}

fn refresh_newgrf_catalogs(state: &mut GameState) {
    apply_newgrf_stack_catalogs_default_dirs(state);
}

/// Activa o desactiva una entrada. Las estáticas no se pueden desactivar.
pub(crate) fn set_newgrf_enabled(
    state: &mut GameState,
    index: usize,
    enabled: bool,
) -> Result<(), CommandError> {
    index_ok(state, index)?;
    let entry = &mut state.newgrf_stack[index];
    if entry.is_static && !enabled {
        return Err(CommandError::NewGrfStaticImmutable);
    }
    entry.enabled = enabled;
    refresh_newgrf_catalogs(state);
    Ok(())
}

/// Reordena una entrada del stack (`from` → `to`).
pub(crate) fn move_newgrf_in_stack(
    state: &mut GameState,
    from: usize,
    to: usize,
) -> Result<(), CommandError> {
    index_ok(state, from)?;
    index_ok(state, to)?;
    if from == to {
        return Ok(());
    }
    let entry = state.newgrf_stack.remove(from);
    state.newgrf_stack.insert(to, entry);
    refresh_newgrf_catalogs(state);
    Ok(())
}

/// Quita una entrada no estática del stack.
pub(crate) fn remove_newgrf_from_stack(
    state: &mut GameState,
    index: usize,
) -> Result<(), CommandError> {
    index_ok(state, index)?;
    if state.newgrf_stack[index].is_static {
        return Err(CommandError::NewGrfStaticImmutable);
    }
    state.newgrf_stack.remove(index);
    refresh_newgrf_catalogs(state);
    Ok(())
}

/// Añade una entrada al final (rechaza GRFID duplicado).
pub(crate) fn add_newgrf_to_stack(
    state: &mut GameState,
    entry: NewGrfEntry,
) -> Result<(), CommandError> {
    if entry.filename.trim().is_empty() {
        return Err(CommandError::NewGrfInvalidEntry);
    }
    let mut probe = state.newgrf_stack.clone();
    probe.push(entry.clone());
    let issues = validate_stack(&probe, &[]);
    if issues
        .iter()
        .any(|i| matches!(i, GrfStackIssue::DuplicateGrfid(_)))
    {
        return Err(CommandError::NewGrfDuplicateGrfid);
    }
    if issues
        .iter()
        .any(|i| matches!(i, GrfStackIssue::EmptyFilename))
    {
        return Err(CommandError::NewGrfInvalidEntry);
    }
    state.newgrf_stack.push(entry);
    refresh_newgrf_catalogs(state);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::newgrf_config::grfid_from_bytes;

    fn sample_entry(name: &str, grfid: u32) -> NewGrfEntry {
        let mut e = NewGrfEntry::new(name, grfid);
        e.name = name.into();
        e
    }

    #[test]
    fn toggle_move_remove_newgrf_stack() {
        let mut s = GameState::new(4, 4);
        assert_eq!(s.newgrf_stack.len(), 1);
        assert!(s.newgrf_stack[0].is_static);

        apply_command(
            &mut s,
            &Command::AddNewGrfToStack {
                entry: sample_entry("extra.grf", grfid_from_bytes([0x12, 0x34, 0x56, 0x78])),
            },
        )
        .unwrap();
        assert_eq!(s.newgrf_stack.len(), 2);

        apply_command(
            &mut s,
            &Command::SetNewGrfEnabled {
                index: 1,
                enabled: false,
            },
        )
        .unwrap();
        assert!(!s.newgrf_stack[1].enabled);

        assert_eq!(
            apply_command(
                &mut s,
                &Command::SetNewGrfEnabled {
                    index: 0,
                    enabled: false,
                },
            ),
            Err(CommandError::NewGrfStaticImmutable)
        );

        apply_command(&mut s, &Command::MoveNewGrfInStack { from: 1, to: 0 }).unwrap();
        assert_eq!(s.newgrf_stack[0].filename, "extra.grf");
        assert!(s.newgrf_stack[1].is_static);

        apply_command(&mut s, &Command::RemoveNewGrfFromStack { index: 0 }).unwrap();
        assert_eq!(s.newgrf_stack.len(), 1);
        assert!(s.newgrf_stack[0].is_static);

        assert_eq!(
            apply_command(&mut s, &Command::RemoveNewGrfFromStack { index: 0 }),
            Err(CommandError::NewGrfStaticImmutable)
        );
    }

    #[test]
    fn add_rejects_duplicate_grfid() {
        let mut s = GameState::new(4, 4);
        let grfid = s.newgrf_stack[0].grfid;
        let e = apply_command(
            &mut s,
            &Command::AddNewGrfToStack {
                entry: sample_entry("dup.grf", grfid),
            },
        )
        .unwrap_err();
        assert_eq!(e, CommandError::NewGrfDuplicateGrfid);
    }
}
