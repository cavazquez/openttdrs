use bevy::prelude::*;

use crate::render::{MapSpriteBatches, MapVisualLayer};

pub(crate) fn flush_map_batches(commands: &mut Commands, batches: MapSpriteBatches) {
    for (chunk, water, sp, tr) in batches.water {
        commands.spawn((MapVisualLayer, chunk, water, sp, tr));
    }
    for (chunk, st, sp, tr) in batches.shore {
        commands.spawn((MapVisualLayer, chunk, st, sp, tr));
    }
}
