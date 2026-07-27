//! Aplicación de grupos Action3 `RailType` para señales custom.

use std::collections::HashMap;
use std::path::Path;

use crate::GameState;
use crate::newgrf_actions::collect_railtype_metas_from_grf;
use crate::newgrf_type_tables::rail_type_from_label;
use crate::rail_type::{RAIL_SPRITE_TYPE_SIGNALS, RailSignalSpriteSpec, RailType};

/// Reconstruye los overrides `GetCustomSignalSprite` desde el stack habilitado.
pub fn apply_newgrf_rail_signals(state: &mut GameState, search_dirs: &[&Path]) {
    let mut slots: Vec<Option<RailSignalSpriteSpec>> = vec![None; 4];
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|dir| dir.join(&entry.filename))
            .find(|candidate| candidate.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        let Ok(graphics) = crate::newgrf_sprites::collect_railtype_sprite_graphics(&data) else {
            continue;
        };
        let labels: HashMap<u8, RailType> = collect_railtype_metas_from_grf(&data)
            .into_iter()
            .filter_map(|meta| rail_type_from_label(meta.label).map(|rt| (meta.local_id, rt)))
            .collect();
        let type_tables = crate::newgrf_type_tables::collect_type_tables_from_grf(&data);
        let type_tables = (!type_tables.is_empty()).then_some(type_tables);

        let mut local_ids: Vec<u8> = graphics
            .specific_assigns
            .keys()
            .filter_map(|&(local_id, selector)| {
                (selector == RAIL_SPRITE_TYPE_SIGNALS).then_some(local_id)
            })
            .collect();
        local_ids.sort_unstable();
        local_ids.dedup();
        for local_id in local_ids {
            let rail_type = labels
                .get(&local_id)
                .copied()
                .or_else(|| (local_id < 4).then(|| RailType::from_u8(local_id)));
            let Some(rail_type) = rail_type else {
                continue;
            };
            slots[usize::from(rail_type.as_u8())] = Some(RailSignalSpriteSpec {
                rail_type,
                local_id,
                grfid: entry.grfid,
                type_tables: type_tables.clone(),
                graphics: graphics.clone(),
            });
        }
    }
    state.runtime.rail_signal_newgrf = slots;
}

pub fn apply_newgrf_rail_signals_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_rail_signals(state, &refs);
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_actions::build_action0_railtype_payload;
    use crate::newgrf_sprites::{Action2EvalCtx, build_grf_v2_railtype_signal_sprites};

    #[test]
    fn action0_action3_and_action2_resolve_custom_hd_signals() {
        let action0 = build_action0_railtype_payload(7, b"ELRL");
        let red = vec![174u8; 32 * 48];
        let green = vec![79u8; 32 * 48];
        let bytes = build_grf_v2_railtype_signal_sprites(
            &action0,
            7,
            32,
            48,
            &red,
            &green,
            [b'S', b'I', 0, 1],
            "signals",
        );
        let dir =
            std::env::temp_dir().join(format!("openttdrs_rail_signal_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("signals.grf"), bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("signals.grf", 0x5349_0001));

        apply_newgrf_rail_signals(&mut state, &[&dir]);
        let spec = state.runtime.rail_signal_newgrf[usize::from(RailType::Electric.as_u8())]
            .as_ref()
            .expect("ELRL signal spec");
        assert_eq!(spec.local_id, 7);
        let mut red_ctx = Action2EvalCtx::default();
        let red_sprite = spec
            .resolve_sprite(3, 4, 0, false, &mut red_ctx)
            .expect("red path signal");
        let mut green_ctx = Action2EvalCtx::default();
        let green_sprite = spec
            .resolve_sprite(3, 4, 0, true, &mut green_ctx)
            .expect("green path signal");
        assert_eq!((red_sprite.width, red_sprite.height), (32, 48));
        assert_eq!(red_ctx.vars.get(&0x18), Some(&(4 << 16)));
        assert_eq!(green_ctx.vars.get(&0x18), Some(&((4 << 16) | 1)));
        assert_ne!(red_sprite.rgba, green_sprite.rgba);
    }
}
