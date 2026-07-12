//! Ascensor de Large Office (`SPR_LIFT` = 1443 / `house_lift.png`).
//!
//! OpenTTD: `TownDrawHouseLift` — `AddChildSpriteScreen(SPR_LIFT, …, 14, 60 - pos)`.
//! MVP: 4 pasos de Y sincronizados con [`TileAnimClock`] (`frame & 3`).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::palette_animations_should_run;
use crate::render::tile_anims::TileAnimClock;
use crate::state::ClientScreen;

pub(crate) struct HouseLiftAnimPlugin;

impl Plugin for HouseLiftAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_house_lift
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Overlay del ascensor; `base` = traslación en frame 0.
#[derive(Component, Clone, Copy)]
pub(crate) struct HouseLiftAnim {
    pub(crate) base: Vec3,
}

/// Pasos de elevación (px Bevy, Y+ = arriba). OpenTTD usa `60 - pos` en pantalla.
pub(crate) const HOUSE_LIFT_Y_OFFSETS: [f32; 4] = [0.0, 6.0, 12.0, 18.0];

/// Índice de frame del ascensor (`TileAnimClock.frame & 3`).
#[must_use]
pub(crate) fn house_lift_frame_index(clock_frame: u8) -> usize {
    usize::from(clock_frame & 3)
}

/// Offset Y Bevy para el frame dado.
#[must_use]
pub(crate) fn house_lift_y_offset(clock_frame: u8) -> f32 {
    HOUSE_LIFT_Y_OFFSETS[house_lift_frame_index(clock_frame)]
}

/// Sprites de edificio que llevan ascensor (`draw_proc == 1` en `town_land.h`).
#[must_use]
pub(crate) fn house_sprite_has_lift(s2: u32) -> bool {
    matches!(s2, 1442 | 4569)
}

fn animate_house_lift(
    clock: Res<TileAnimClock>,
    mut last_frame: Local<Option<u8>>,
    mut q: Query<(&HouseLiftAnim, &mut Transform)>,
) {
    let frame = clock.frame & 3;
    if *last_frame == Some(frame) {
        return;
    }
    *last_frame = Some(frame);
    let dy = house_lift_y_offset(clock.frame);
    for (anim, mut tf) in &mut q {
        tf.translation = Vec3::new(anim.base.x, anim.base.y + dy, anim.base.z);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_masks_low_bits() {
        assert_eq!(house_lift_frame_index(0), 0);
        assert_eq!(house_lift_frame_index(1), 1);
        assert_eq!(house_lift_frame_index(3), 3);
        assert_eq!(house_lift_frame_index(4), 0);
        assert_eq!(house_lift_frame_index(15), 3);
    }

    #[test]
    fn y_offset_steps_up() {
        assert_eq!(house_lift_y_offset(0), 0.0);
        assert_eq!(house_lift_y_offset(1), 6.0);
        assert_eq!(house_lift_y_offset(2), 12.0);
        assert_eq!(house_lift_y_offset(3), 18.0);
        assert_eq!(house_lift_y_offset(5), 6.0);
    }

    #[test]
    fn large_office_s2_has_lift() {
        assert!(house_sprite_has_lift(1442));
        assert!(house_sprite_has_lift(4569));
        assert!(!house_sprite_has_lift(1483));
        assert!(!house_sprite_has_lift(0));
    }
}
