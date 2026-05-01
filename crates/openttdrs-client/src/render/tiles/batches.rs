use bevy::prelude::*;

use super::TILE_OVERLAP_SCALE;
use crate::render::{MapSpriteBatches, MapVisualLayer};

pub(crate) fn flush_map_batches(commands: &mut Commands, batches: MapSpriteBatches) {
    for (wt, sp, tr) in batches.water {
        commands.spawn((MapVisualLayer, wt, sp, tr));
    }
    for (sp, tr) in batches.shore {
        let mut tr = tr;
        tr.scale = Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0);
        commands.spawn((MapVisualLayer, sp, tr));
    }
    for (sp, tr) in batches.trees {
        commands.spawn((MapVisualLayer, sp, tr));
    }
}
