//! Aplicación de Action0 `Cargoes` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::cargo_spec::{CargoSpecDef, empty_cargo_spec_catalog};

use super::super::action0::collect_cargo_metas_from_grf;

/// Reconstruye el catálogo de cargo specs desde el stack `enabled`.
pub fn apply_newgrf_cargoes(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = empty_cargo_spec_catalog();
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
        for meta in collect_cargo_metas_from_grf(&data) {
            if let Some(existing) = catalog.iter_mut().find(|d| d.id == meta.local_id) {
                existing.bitnum = meta.bitnum;
                existing.label = meta.label;
                existing.name = meta.name;
                existing.from_newgrf = true;
                existing.grfid = entry.grfid;
            } else {
                catalog.push(CargoSpecDef {
                    id: meta.local_id,
                    bitnum: meta.bitnum,
                    label: meta.label,
                    name: meta.name,
                    from_newgrf: true,
                    grfid: entry.grfid,
                });
            }
        }
    }
    state.cargo_spec_catalog = catalog;
}

/// Aplica Cargoes con directorios de búsqueda por defecto.
pub fn apply_newgrf_cargoes_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_cargoes(state, &refs);
}
