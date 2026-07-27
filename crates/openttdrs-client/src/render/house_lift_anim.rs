//! Ascensor de Large Office (`SPR_LIFT` = 1443 / `house_lift.png`).
//!
//! OpenTTD: `TownDrawHouseLift` dibuja en `(14, 60 - pos)`. La posición y el
//! destino viven en `Tile.m6/m7`; el cliente solo proyecta ese estado.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct HouseLiftAnimPlugin;

impl Plugin for HouseLiftAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_house_lift
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Overlay del ascensor; `base` corresponde a `GetLiftPosition == 0`.
#[derive(Component, Clone, Copy)]
pub(crate) struct HouseLiftAnim {
    pub(crate) base: Vec3,
    pub(crate) coord: openttdrs_core::TileCoord,
}

/// Offset X de pantalla OpenTTD (`AddChildSpriteScreen`).
pub(crate) const HOUSE_LIFT_SCREEN_X: f32 = 14.0;
/// Offset Y de pantalla OpenTTD con `GetLiftPosition == 0` (`60 - pos`).
pub(crate) const HOUSE_LIFT_SCREEN_Y: f32 = 60.0;

/// Offset Y Bevy: restar `pos` en pantalla equivale a subir `pos` píxeles.
#[must_use]
pub(crate) const fn house_lift_y_offset(position: u8) -> f32 {
    let clamped = if position > openttdrs_core::map::LIFT_MAX_POSITION {
        openttdrs_core::map::LIFT_MAX_POSITION
    } else {
        position
    };
    clamped as f32
}

/// Sprites de edificio que llevan ascensor (`draw_proc == 1` en `town_land.h`).
#[must_use]
pub(crate) fn house_sprite_has_lift(s2: u32) -> bool {
    matches!(s2, 1442 | 4569)
}

fn animate_house_lift(sim: Res<SimWorld>, mut q: Query<(&HouseLiftAnim, &mut Transform)>) {
    for (anim, mut transform) in &mut q {
        let position = sim
            .state
            .map
            .get(anim.coord)
            .map(openttdrs_core::lift_position)
            .unwrap_or(0);
        let next = Vec3::new(
            anim.base.x,
            anim.base.y + house_lift_y_offset(position),
            anim.base.z,
        );
        if transform.translation != next {
            transform.translation = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn y_offset_tracks_all_37_lift_positions() {
        assert_eq!(house_lift_y_offset(0), 0.0);
        assert_eq!(house_lift_y_offset(1), 1.0);
        assert_eq!(house_lift_y_offset(12), 12.0);
        assert_eq!(house_lift_y_offset(36), 36.0);
        assert_eq!(house_lift_y_offset(63), 36.0);
    }

    #[test]
    fn large_office_s2_has_lift() {
        assert!(house_sprite_has_lift(1442));
        assert!(house_sprite_has_lift(4569));
        assert!(!house_sprite_has_lift(1483));
        assert!(!house_sprite_has_lift(0));
    }

    #[test]
    fn large_office_lift_only_on_final_stage_draw_proc() {
        use crate::sprites::{HOUSE_DRAW_DATA, house_draw_data_index_for_tile};
        let stage2 = house_draw_data_index_for_tile(4, 0, 0, 2);
        let stage3 = house_draw_data_index_for_tile(4, 0, 0, 3);
        let s2 = &HOUSE_DRAW_DATA[stage2];
        let s3 = &HOUSE_DRAW_DATA[stage3];
        assert_eq!(s2.s2, 1442);
        assert_eq!(s3.s2, 1442);
        assert_eq!(s2.draw_proc, 0, "stage 2 = obra, sin ascensor");
        assert_eq!(s3.draw_proc, 1, "stage 3 = final, con ascensor");
        assert!(house_sprite_has_lift(s3.s2));
    }

    #[test]
    fn screen_offsets_are_child_sprite_pixels_not_tile_seq() {
        use crate::iso::{ISO_HW, remap_tile_offset};
        let bad = remap_tile_offset(HOUSE_LIFT_SCREEN_X, HOUSE_LIFT_SCREEN_Y, 0.0) * 0.5;
        assert!(bad.length() > ISO_HW * 2.0);
        assert_eq!(HOUSE_LIFT_SCREEN_X, 14.0);
        assert_eq!(HOUSE_LIFT_SCREEN_Y, 60.0);
    }
}
