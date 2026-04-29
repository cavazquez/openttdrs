//! Sistemas de avance de simulación independiente del render.

use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::SimHudControls;
use crate::vehicle_render::VehicleIndex;

pub(crate) fn advance_sim(
    time: Res<Time>,
    hud: Res<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut acc: Local<f32>,
) {
    if hud.paused {
        return;
    }
    const TICK_HZ: f32 = 15.0;
    *acc += time.delta_secs();
    let period = 1.0 / TICK_HZ;
    let mut stepped = false;
    while *acc >= period {
        *acc -= period;
        sim.state.step();
        stepped = true;
    }
    if stepped {
        vehicle_index.rebuild(&sim.state.vehicles);
    }
}
