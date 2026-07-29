//! Aplicación de Action0 `Cargoes` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::cargo_spec::{CargoSpecDef, DEFAULT_CARGO_CAPACITY_MULTIPLIER, empty_cargo_spec_catalog};

use super::super::action0::collect_cargo_metas_from_grf;

/// Reconstruye el catálogo de cargo specs desde el stack `enabled`.
///
/// Merge por `local_id` dentro del GRF; el mismo label en otro GRF posterior
/// actualiza el slot si comparte `id`, o añade entrada (identidad por label
/// vía [`crate::cargo_spec::cargo_spec_by_label`]).
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
        let gfx = crate::newgrf_sprites::collect_cargo_sprite_graphics(&data).unwrap_or_default();
        for meta in collect_cargo_metas_from_grf(&data) {
            let views = gfx
                .views_for_local_id(meta.local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let def = CargoSpecDef {
                id: meta.local_id,
                bitnum: meta.bitnum,
                label: meta.label,
                name: meta.name,
                from_newgrf: true,
                grfid: entry.grfid,
                weight: meta.weight,
                initial_payment: meta.initial_payment,
                transit_fast: meta.transit_fast,
                transit_slow: meta.transit_slow,
                is_freight: meta.is_freight,
                classes: meta.classes,
                capacity_multiplier: if meta.capacity_multiplier == 0 {
                    DEFAULT_CARGO_CAPACITY_MULTIPLIER
                } else {
                    meta.capacity_multiplier
                },
                rating_colour: meta.rating_colour,
                legend_colour: meta.legend_colour,
                newgrf_views: views,
            };
            if let Some(existing) = catalog.iter_mut().find(|d| d.id == def.id) {
                *existing = def;
            } else {
                catalog.push(def);
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
