//! Caché de `GetCustomSignalSprite` para grupos Action3 RailType.

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{Action2EvalCtx, RailSignalSpriteSpec};

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};

#[derive(Resource, Default)]
pub(crate) struct NewGrfSignalSpriteCache {
    handles: HashMap<(u32, u8, u8, u32), Handle<Image>>,
}

pub(crate) struct ResolvedSignalSprite {
    pub sprite: Sprite,
    pub center_offset: Vec2,
}

impl NewGrfSignalSpriteCache {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sprite_for(
        &mut self,
        spec: &RailSignalSpriteSpec,
        image: u8,
        signal_type: u8,
        variant: u8,
        green: bool,
        ctx: &mut Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<ResolvedSignalSprite> {
        let decoded = spec.resolve_sprite(image, signal_type, variant, green, ctx)?;
        let fingerprint = runtime_fingerprint(ctx, vars::RAIL_SIGNAL, false);
        let key = (spec.grfid, spec.rail_type.as_u8(), image, fingerprint);
        let handle = self
            .handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(
                    &decoded,
                    DecodedSpriteImagePolicy::Raw,
                ))
            })
            .clone();
        let center_offset = Vec2::new(
            f32::from(decoded.x_offs) + f32::from(decoded.width) * 0.5,
            -(f32::from(decoded.y_offs) + f32::from(decoded.height) * 0.5),
        );
        Some(ResolvedSignalSprite {
            sprite: Sprite {
                image: handle,
                ..default()
            },
            center_offset,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use openttdrs_core::newgrf_actions::build_action0_railtype_payload;
    use openttdrs_core::newgrf_sprites::build_grf_v2_railtype_signal_sprites;
    use openttdrs_core::{GameState, NewGrfEntry, RailType, apply_newgrf_rail_signals};

    #[test]
    fn cache_selects_state_and_preserves_hd_offsets() {
        let action0 = build_action0_railtype_payload(0, b"RAIL");
        let bytes = build_grf_v2_railtype_signal_sprites(
            &action0,
            0,
            32,
            48,
            &vec![174; 32 * 48],
            &vec![79; 32 * 48],
            [b'H', b'D', 0, 1],
            "hd-signals",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("hd.grf"), bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(NewGrfEntry::new("hd.grf", 0x4844_0001));
        apply_newgrf_rail_signals(&mut state, &[dir.path()]);
        let spec = state.runtime.rail_signal_newgrf[usize::from(RailType::Rail.as_u8())]
            .as_ref()
            .expect("signal spec");

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfSignalSpriteCache::default();
        let mut red_ctx = Action2EvalCtx::default();
        let red = cache
            .sprite_for(spec, 6, 4, 1, false, &mut red_ctx, &mut images)
            .expect("red");
        let mut green_ctx = Action2EvalCtx::default();
        let green = cache
            .sprite_for(spec, 6, 4, 1, true, &mut green_ctx, &mut images)
            .expect("green");
        assert_ne!(red.sprite.image, green.sprite.image);
        assert_eq!(red.center_offset, Vec2::new(6.0, 24.0));

        let mut same_ctx = Action2EvalCtx::default();
        let same = cache
            .sprite_for(spec, 6, 4, 1, false, &mut same_ctx, &mut images)
            .expect("same cached");
        assert_eq!(red.sprite.image, same.sprite.image);
    }
}
