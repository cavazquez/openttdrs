use bevy::prelude::*;

use crate::render::{MapSpriteBatches, MapVisualLayer, WaterTile};

pub(crate) fn flush_map_batches(commands: &mut Commands, batches: MapSpriteBatches) {
    for (chunk, sp, tr) in batches.water {
        commands.spawn((MapVisualLayer, chunk, WaterTile, sp, tr));
    }
    for (chunk, st, sp, tr) in batches.shore {
        commands.spawn((MapVisualLayer, chunk, st, sp, tr));
    }
    for (chunk, sp, tr) in batches.trees {
        commands.spawn((MapVisualLayer, chunk, sp, tr));
    }
}
