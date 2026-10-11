//! Aplicación de bloques Action5 desde el `NewGRF` stack (IDs `OpenTTD` 15.3).

use std::path::Path;
use std::sync::OnceLock;

use crate::GameState;
use crate::newgrf_sprites::{
    Action5Block, Action5LoadContext, DecodedSprite, collect_active_action5_blocks,
};

fn merge_action5_stack_into(
    slots: &mut [Option<DecodedSprite>],
    state: &GameState,
    search_dirs: &[&Path],
    merge: fn(&mut [Option<DecodedSprite>], &Action5Block),
) {
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
        let context = Action5LoadContext::new(state.climate.newgrf_landscape_id())
            .with_parameters(entry.params.clone());
        let Ok(blocks) = collect_active_action5_blocks(&data, &context) else {
            continue;
        };
        for block in &blocks {
            merge(slots, block);
        }
    }
}

fn apply_action5_table(
    state: &mut GameState,
    search_dirs: &[&Path],
    slot_count: usize,
    merge: fn(&mut [Option<DecodedSprite>], &Action5Block),
) -> Vec<Option<DecodedSprite>> {
    let mut slots = vec![None; slot_count];
    merge_action5_stack_into(&mut slots, state, search_dirs, merge);
    slots
}

/// Carga los 90 sprites Action5 que forman parte del set gráfico base.
///
/// Los cimientos extra no pertenecen a `ogfx1_base`; `OpenGFX` los publica en
/// el GRF *extra*. Se cargan antes del stack de la partida para que un `NewGRF`
/// real pueda sobrescribirlos igual que en `OpenTTD`.
fn load_default_foundation_action5_table(landscape: u8) -> Vec<Option<DecodedSprite>> {
    // These sprites are part of the base set, not an optional NewGRF. A
    // packaged client has neither a development checkout nor GRFs under its
    // working directory. Embed the pinned base set so the parent/child
    // geometry is the same in a fresh Snap profile and in development.
    const BASE_EXTRA: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/opengfx-8.0/ogfxe_extra.grf"
    ));
    let context = Action5LoadContext::new(landscape);
    let mut slots = vec![None; crate::newgrf_sprites::FOUNDATION_ACTION5_SLOT_COUNT];
    if let Ok(blocks) = collect_active_action5_blocks(BASE_EXTRA, &context) {
        for block in &blocks {
            crate::newgrf_sprites::merge_foundation_action5_block(&mut slots, block);
        }
    }
    slots
}

fn default_foundation_action5_table(climate: crate::Climate) -> Vec<Option<DecodedSprite>> {
    static BASE_FOUNDATIONS: OnceLock<[Vec<Option<DecodedSprite>>; 4]> = OnceLock::new();
    let tables = BASE_FOUNDATIONS.get_or_init(|| {
        std::array::from_fn(|landscape| {
            load_default_foundation_action5_table(u8::try_from(landscape).unwrap_or(0))
        })
    });
    tables[usize::from(climate.newgrf_landscape_id())].clone()
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
    apply_newgrf_action5_signals,
    apply_newgrf_action5_signals_default_dirs,
    signal_action5_newgrf_sprites,
    crate::newgrf_sprites::SIGNAL_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_signals_action5_block
);
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
/// Aplica únicamente los Action5 foundations del stack explícito.
///
/// Se conserva sin assets base implícitos para que los callers de bajo nivel y
/// sus tests puedan inspeccionar sólo los reemplazos que aportó un `NewGRF`.
pub fn apply_newgrf_action5_foundations(state: &mut GameState, search_dirs: &[&Path]) {
    state.runtime.foundation_newgrf_sprites = apply_action5_table(
        state,
        search_dirs,
        crate::newgrf_sprites::FOUNDATION_ACTION5_SLOT_COUNT,
        crate::newgrf_sprites::merge_foundation_action5_block,
    );
}

/// Aplica foundations del set gráfico base y después los reemplazos de la
/// partida. Es la variante que usa el bootstrap normal del cliente.
pub fn apply_newgrf_action5_foundations_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    let mut slots = default_foundation_action5_table(state.climate);
    merge_action5_stack_into(
        &mut slots,
        state,
        &refs,
        crate::newgrf_sprites::merge_foundation_action5_block,
    );
    state.runtime.foundation_newgrf_sprites = slots;
}
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
define_action5_apply!(
    apply_newgrf_action5_canals,
    apply_newgrf_action5_canals_default_dirs,
    canal_action5_newgrf_sprites,
    crate::newgrf_sprites::CANALS_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_canals_action5_block
);
define_action5_apply!(
    apply_newgrf_action5_twocc,
    apply_newgrf_action5_twocc_default_dirs,
    twocc_action5_newgrf_sprites,
    crate::newgrf_sprites::TWOCC_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_twocc_action5_block
);

/// Aplica Action5 tramway y conserva el último modo de depósito que tocó sus
/// slots relocatables. La tabla y el modo se reconstruyen juntos porque la
/// presencia final de un sprite no preserva el orden de carga entre GRFs.
fn apply_tramway_action5_table(
    state: &GameState,
    search_dirs: &[&Path],
) -> (
    Vec<Option<DecodedSprite>>,
    crate::newgrf_sprites::TramwayDepotReplacement,
) {
    let mut slots = vec![None; crate::newgrf_sprites::TRAMWAY_ACTION5_SLOT_COUNT];
    let mut replacement = crate::newgrf_sprites::TramwayDepotReplacement::default();
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|dir| dir.join(&entry.filename))
            .find(|path| path.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let context = Action5LoadContext::new(state.climate.newgrf_landscape_id())
            .with_parameters(entry.params.clone());
        let Ok(blocks) = collect_active_action5_blocks(&data, &context) else {
            continue;
        };
        for block in &blocks {
            crate::newgrf_sprites::merge_tramway_action5_block(&mut slots, block);
            replacement = crate::newgrf_sprites::tramway_depot_replacement_after_action5_block(
                replacement,
                block,
            );
        }
    }
    (slots, replacement)
}

/// Aplica los replacements Action5 `0x0B` y su estado de depósito asociado.
pub fn apply_newgrf_action5_tramway(state: &mut GameState, search_dirs: &[&Path]) {
    let (slots, replacement) = apply_tramway_action5_table(state, search_dirs);
    state.runtime.tramway_action5_newgrf_sprites = slots;
    state.runtime.tramway_depot_replacement = replacement;
}

/// Variante que busca los GRFs activos en las rutas habituales del cliente.
pub fn apply_newgrf_action5_tramway_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_action5_tramway(state, &refs);
}

/// Aplica todos los tipos Action5 runtime soportados.
pub fn apply_newgrf_action5_all_default_dirs(state: &mut GameState) {
    apply_newgrf_action5_signals_default_dirs(state);
    apply_newgrf_action5_shore_default_dirs(state);
    apply_newgrf_action5_catenary_default_dirs(state);
    apply_newgrf_action5_foundations_default_dirs(state);
    apply_newgrf_action5_oneway_default_dirs(state);
    apply_newgrf_action5_roadstops_default_dirs(state);
    apply_newgrf_action5_openttd_gui_default_dirs(state);
    apply_newgrf_action5_airport_preview_default_dirs(state);
    apply_newgrf_action5_bridge_decks_default_dirs(state);
    apply_newgrf_action5_canals_default_dirs(state);
    apply_newgrf_action5_twocc_default_dirs(state);
    apply_newgrf_action5_tramway_default_dirs(state);
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn bundled_base_foundations_are_complete_in_every_climate() {
        for landscape in 0..4 {
            let slots = load_default_foundation_action5_table(landscape);
            assert_eq!(slots.len(), 90);
            let mut visible = 0;
            for (index, slot) in slots.iter().enumerate() {
                let sprite = slot.as_ref().expect("missing bundled foundation");
                assert!(sprite.width > 0 && sprite.height > 0, "slot {index}");
                assert_eq!(
                    sprite.rgba.len(),
                    usize::from(sprite.width) * usize::from(sprite.height) * 4,
                    "landscape {landscape}, slot {index}"
                );
                if sprite
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] > 0)
                {
                    visible += 1;
                }
            }
            // Some native slots are intentionally transparent; completeness
            // means a decoded entry in each slot, not opacity in each one.
            assert!(
                visible > 0,
                "landscape {landscape}: {visible} visible slots"
            );
        }
    }
}
