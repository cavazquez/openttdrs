use openttdrs_core::{autorail_trackbit_from_fract, rail_horz_lane_bit, rail_vert_lane_bit};

use crate::ui::toolbar::BuildMenuAction;

/// `TrackBits` de un solo carril / autorail según herramienta y fract del cursor.
#[must_use]
pub(crate) fn rail_lane_bits_for_action(
    action: BuildMenuAction,
    fract: Option<(u8, u8)>,
) -> Option<u8> {
    match action {
        BuildMenuAction::RailX => Some(0x01),
        BuildMenuAction::RailY => Some(0x02),
        BuildMenuAction::Rail => {
            let (fx, fy) = fract.unwrap_or((128, 128));
            Some(autorail_trackbit_from_fract(fx, fy))
        }
        BuildMenuAction::RailHorz => {
            let (fx, fy) = fract.unwrap_or((128, 128));
            Some(rail_horz_lane_bit(fx, fy))
        }
        BuildMenuAction::RailVert => {
            let (fx, fy) = fract.unwrap_or((128, 128));
            Some(rail_vert_lane_bit(fx, fy))
        }
        _ => None,
    }
}
