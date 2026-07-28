//! Aplicación de bloques Action5 desde el `NewGRF` stack (IDs `OpenTTD` 15.3).

use std::path::Path;

use crate::GameState;
use crate::newgrf_sprites::{Action5Block, DecodedSprite, collect_action5_blocks};

fn apply_action5_table(
    state: &mut GameState,
    search_dirs: &[&Path],
    slot_count: usize,
    merge: fn(&mut [Option<DecodedSprite>], &Action5Block),
) -> Vec<Option<DecodedSprite>> {
    let mut slots = vec![None; slot_count];
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
            merge(&mut slots, block);
        }
    }
    slots
}

macro_rules! define_action5_apply {
    ($apply:ident, $default:ident, $field:ident, $count:expr, $merge:path) => {
        pub fn $apply(state: &mut GameState, search_dirs: &[&Path]) {
            state.runtime.$field = apply_action5_table(state, search_dirs, $count, $merge);
        }

        pub fn $default(state: &mut GameState) {
            let owned = super::default_newgrf_search_dirs();
            let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
            $apply(state, &refs);
        }
    };
}

define_action5_apply!(
    apply_newgrf_action5_shore,
    apply_newgrf_action5_shore_default_dirs,
    shore_newgrf_sprites,
    crate::newgrf_sprites::SHORE_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_shore_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_catenary,
    apply_newgrf_action5_catenary_default_dirs,
    catenary_newgrf_sprites,
    crate::newgrf_sprites::CATENARY_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_catenary_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_foundations,
    apply_newgrf_action5_foundations_default_dirs,
    foundation_newgrf_sprites,
    crate::newgrf_sprites::FOUNDATION_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_foundation_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_oneway,
    apply_newgrf_action5_oneway_default_dirs,
    oneway_newgrf_sprites,
    crate::newgrf_sprites::ONEWAY_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_oneway_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_roadstops,
    apply_newgrf_action5_roadstops_default_dirs,
    roadstop_action5_newgrf_sprites,
    crate::newgrf_sprites::ROADSTOP_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_roadstop_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_openttd_gui,
    apply_newgrf_action5_openttd_gui_default_dirs,
    openttd_gui_newgrf_sprites,
    crate::newgrf_sprites::OPENTTD_GUI_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_openttd_gui_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_airport_preview,
    apply_newgrf_action5_airport_preview_default_dirs,
    airport_preview_newgrf_sprites,
    crate::newgrf_sprites::AIRPORT_PREVIEW_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_airport_preview_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_bridge_decks,
    apply_newgrf_action5_bridge_decks_default_dirs,
    bridge_decks_newgrf_sprites,
    crate::newgrf_sprites::BRIDGE_DECKS_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_bridge_decks_action5_block
);

/// Aplica todos los tipos Action5 runtime soportados.
pub fn apply_newgrf_action5_all_default_dirs(state: &mut GameState) {
    apply_newgrf_action5_shore_default_dirs(state);
    apply_newgrf_action5_catenary_default_dirs(state);
    apply_newgrf_action5_foundations_default_dirs(state);
    apply_newgrf_action5_oneway_default_dirs(state);
    apply_newgrf_action5_roadstops_default_dirs(state);
    apply_newgrf_action5_openttd_gui_default_dirs(state);
    apply_newgrf_action5_airport_preview_default_dirs(state);
    apply_newgrf_action5_bridge_decks_default_dirs(state);
}
