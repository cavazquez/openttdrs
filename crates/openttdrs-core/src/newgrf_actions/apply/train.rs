//! Aplicación de Action0 `Trains` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::engine::{EngineDef, next_free_engine_id, vanilla_engine_catalog};
use crate::vehicle::VehicleKind;

use super::super::action0::collect_train_metas_from_grf;

/// Reconstruye el catálogo de motores (vanilla + Action0/1/3 trains).
pub fn apply_newgrf_vehicles_trains(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_engine_catalog();
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
        let metas = collect_train_metas_from_grf(&data);
        let gfx = crate::newgrf_sprites::collect_train_sprite_graphics(&data).unwrap_or_default();
        // Emparejar Action0 (orden de aparición) con ids locales 0,1,2,…
        for (local_idx, meta) in metas.into_iter().enumerate() {
            let Some(id) = next_free_engine_id(&catalog) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let newgrf_runtime = if gfx.needs_runtime_resolve() {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            catalog.push(EngineDef {
                id,
                kind: VehicleKind::Train,
                name: meta.name,
                max_speed: meta.max_speed,
                price: (400_000_i64 * 20) >> 8,
                running_cost_year: (5_200 * 80) >> 8,
                capacity: 0,
                cargo: None,
                power_hp: meta.power_hp,
                weight_t: 80,
                intro_year: meta.intro_year,
                reliability_pct: 85,
                train_image_index: 2,
                dual_headed: false,
                from_newgrf: true,
                newgrf_views: views,
                newgrf_local_id: local_id,
                newgrf_runtime,
                newgrf_grfid: entry.grfid,
            });
        }
    }
    state.engine_catalog = catalog;
}

/// Aplica trains con directorios de búsqueda por defecto.
pub fn apply_newgrf_vehicles_trains_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_vehicles_trains(state, &refs);
}
