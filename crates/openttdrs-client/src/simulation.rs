//! Sistemas de avance de simulación independiente del render.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::VehicleIndex;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

pub(crate) struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            advance_sim
                .in_set(UpdateSet::Sim)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::advance_sim;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::render::VehicleIndex;
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    #[test]
    fn advance_sim_paused_does_not_step() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(SimHudControls {
            paused: true,
            ..default()
        });
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());

        let before = world.resource::<SimWorld>().state.tick.get();
        world.run_system_once(advance_sim).unwrap();
        let after = world.resource::<SimWorld>().state.tick.get();
        assert_eq!(before, after);
    }

    #[test]
    fn advance_sim_steps_and_rebuilds_index() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(200));
        world.insert_resource(time);
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());

        let before = world.resource::<SimWorld>().state.tick.get();
        world.run_system_once(advance_sim).unwrap();
        let after = world.resource::<SimWorld>().state.tick.get();
        assert!(after > before);
    }
}
