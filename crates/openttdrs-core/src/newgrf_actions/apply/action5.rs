//! Aplicación de bloques Action5 desde el `NewGRF` stack (IDs `OpenTTD` 15.3).

use std::path::{Path, PathBuf};
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
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let asset_roots = [
        PathBuf::from("assets/opengfx"),
        workspace_root.join("assets/opengfx"),
    ];
    let mut candidates = Vec::new();
    for root in &asset_roots {
        candidates.push(root.join("opengfx2-32ez/ogfx2e_extra_32ez.grf"));
        candidates.push(root.join(".signal-src-8bpp/ogfxe_extra.grf"));
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut classic = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("opengfx-"))
                })
                .map(|path| path.join("ogfxe_extra.grf"))
                .collect::<Vec<_>>();
            classic.sort();
            candidates.extend(classic);
        }
    }

    for path in candidates {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let context = Action5LoadContext::new(landscape);
        let Ok(blocks) = collect_active_action5_blocks(&data, &context) else {
            continue;
        };
        let mut slots = vec![None; crate::newgrf_sprites::FOUNDATION_ACTION5_SLOT_COUNT];
        for block in &blocks {
            crate::newgrf_sprites::merge_foundation_action5_block(&mut slots, block);
        }
        if slots.iter().any(Option::is_some) {
            return slots;
        }
    }
    vec![None; crate::newgrf_sprites::FOUNDATION_ACTION5_SLOT_COUNT]
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
define_action5_apply!(
    apply_newgrf_action5_tramway,
    apply_newgrf_action5_tramway_default_dirs,
    tramway_action5_newgrf_sprites,
    crate::newgrf_sprites::TRAMWAY_ACTION5_SLOT_COUNT,
    crate::newgrf_sprites::merge_tramway_action5_block
);

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
