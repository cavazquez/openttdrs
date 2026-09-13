//! Aplicación de Action0 `Bridges` (`0x06`) desde el stack `NewGRF`.

use std::collections::HashMap;
use std::path::Path;

use crate::GameState;
use crate::bridge_spec::{BridgeSpriteGraphicsTable, vanilla_bridge_spec_catalog};
use crate::newgrf_sprites::{DecodedSprite, NEWGRF_SPRITE_BASE, collect_global_sprite_graphics};

use super::super::action0::collect_bridge_metas_from_grf;

/// Reconstruye el catálogo de puentes: vanilla + overrides in-place (último stack gana).
pub fn apply_newgrf_bridges(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_bridge_spec_catalog();
    let stack = state.newgrf_stack.clone();
    let mut loaded_grfs = Vec::new();
    let mut next_sprite_id = NEWGRF_SPRITE_BASE;
    let mut global_sprites: HashMap<u32, DecodedSprite> = HashMap::new();
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
        if let Ok(graphics) = collect_global_sprite_graphics(&data, next_sprite_id) {
            next_sprite_id = graphics.next_sprite_id;
            global_sprites.extend(graphics.sprites);
        }
        loaded_grfs.push((entry.grfid, data));
    }
    for (grfid, data) in loaded_grfs {
        for meta in collect_bridge_metas_from_grf(&data) {
            for offset in 0..usize::from(meta.num_ids) {
                let Some(idx) = usize::from(meta.local_id).checked_add(offset) else {
                    break;
                };
                let Some(slot) = catalog.get_mut(idx) else {
                    break;
                };
                if meta.year_set {
                    slot.available_from_year = meta.available_from_year;
                }
                if meta.min_len_set {
                    slot.min_middle_len = meta.min_middle_len;
                }
                if meta.max_len_set {
                    slot.max_middle_len = meta.max_middle_len;
                }
                if meta.price_set {
                    slot.price_mult = meta.price_mult;
                }
                if meta.speed_set {
                    slot.max_speed = meta.max_speed;
                }
                if let Some(name) = meta.name.as_ref() {
                    slot.name.clone_from(name);
                }
                if meta.has_custom_sprites {
                    slot.has_custom_sprites = true;
                    for (destination, source) in slot
                        .custom_sprite_tables
                        .iter_mut()
                        .zip(meta.custom_sprite_tables.iter().copied())
                    {
                        if source.is_some() {
                            *destination = source;
                        }
                    }
                    for (destination, source) in slot
                        .custom_sprite_graphics
                        .iter_mut()
                        .zip(meta.custom_sprite_tables.iter())
                    {
                        if let Some(source) = source {
                            *destination =
                                Some(materialize_bridge_sprite_table(source, &global_sprites));
                        }
                    }
                }
                if meta.pillar_flags_set {
                    slot.pillar_flags = meta.pillar_flags;
                    slot.has_custom_pillar_flags = true;
                }
                slot.from_newgrf = true;
                slot.grfid = grfid;
            }
        }
    }
    state.bridge_spec_catalog = catalog;
}

fn materialize_bridge_sprite_table(
    table: &crate::bridge_spec::BridgeSpriteTable,
    global_sprites: &HashMap<u32, DecodedSprite>,
) -> BridgeSpriteGraphicsTable {
    std::array::from_fn(|index| {
        global_sprites
            .get(&u32::from(table[index].sprite_id))
            .cloned()
    })
}

/// Aplica Bridges con directorios de búsqueda por defecto.
pub fn apply_newgrf_bridges_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_bridges(state, &refs);
}
