//! Aplicación de bloques Action5 (shore/catenary) desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;

/// Aplica bloques Action5 shore (`0x0D`) del stack enabled → `shore_newgrf_sprites`.
pub fn apply_newgrf_action5_shore(state: &mut GameState, search_dirs: &[&Path]) {
    use crate::newgrf_sprites::{
        SHORE_ACTION5_SLOT_COUNT, collect_action5_blocks, merge_shore_action5_block,
    };
    let mut slots = vec![None; SHORE_ACTION5_SLOT_COUNT];
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(blocks) = collect_action5_blocks(&data) else {
            continue;
        };
        for block in &blocks {
            merge_shore_action5_block(&mut slots, block);
        }
    }
    state.runtime.shore_newgrf_sprites = slots;
}

/// Action5 shore con directorios de búsqueda por defecto.
pub fn apply_newgrf_action5_shore_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_action5_shore(state, &refs);
}

/// Aplica bloques Action5 catenary (`0x05`) → `catenary_newgrf_sprites`.
pub fn apply_newgrf_action5_catenary(state: &mut GameState, search_dirs: &[&Path]) {
    use crate::newgrf_sprites::{
        CATENARY_ACTION5_SLOT_COUNT, collect_action5_blocks, merge_catenary_action5_block,
    };
    let mut slots = vec![None; CATENARY_ACTION5_SLOT_COUNT];
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(blocks) = collect_action5_blocks(&data) else {
            continue;
        };
        for block in &blocks {
            merge_catenary_action5_block(&mut slots, block);
        }
    }
    state.runtime.catenary_newgrf_sprites = slots;
}

/// Action5 catenary con directorios de búsqueda por defecto.
pub fn apply_newgrf_action5_catenary_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_action5_catenary(state, &refs);
}
