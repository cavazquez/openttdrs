use bevy::prelude::*;

use super::TILE_OVERLAP_SCALE;
use crate::render::{MapSpriteBatches, MapVisualLayer, WaterTile};

pub(crate) fn flush_map_batches(commands: &mut Commands, batches: MapSpriteBatches) {
    for (chunk, sp, tr) in batches.water {
        commands.spawn((MapVisualLayer, chunk, WaterTile, sp, tr));
    }
    for (chunk, st, sp, tr) in batches.shore {
        let mut tr = tr;
        tr.scale = Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0);
        commands.spawn((MapVisualLayer, chunk, st, sp, tr));
    }
    for (chunk, sp, tr) in batches.trees {
        commands.spawn((MapVisualLayer, chunk, sp, tr));
    }
}
