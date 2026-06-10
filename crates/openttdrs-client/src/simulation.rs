//! Sistemas de avance de simulación independiente del render.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::VehicleIndex;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

/// Fracción del tick de simulación actual (0..1) para interpolar el render entre pasos.
#[derive(Resource, Default)]
pub(crate) struct SimClock {
    pub(crate) tick_alpha: f32,
}

pub(crate) struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimClock>().add_systems(
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
    mut sim_clock: ResMut<SimClock>,
    mut acc: Local<f32>,
) {
    if hud.paused {
        sim_clock.tick_alpha = 0.0;
        return;
    }
    const TICK_HZ: f32 = 5.0;
    *acc += time.delta_secs() * hud.sim_speed.max(0.1);
    let period = 1.0 / TICK_HZ;
    let mut stepped = false;
    while *acc >= period {
        *acc -= period;
        sim.state.step();
        stepped = true;
    }
    sim_clock.tick_alpha = (*acc / period).clamp(0.0, 1.0);
    if stepped {
        vehicle_index.rebuild(&sim.state.vehicles);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{SimClock, advance_sim};
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
            sim_speed: 1.0,
            ..default()
        });
        world.insert_resource(SimClock::default());
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
        world.insert_resource(SimClock::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());

        let before = world.resource::<SimWorld>().state.tick.get();
        world.run_system_once(advance_sim).unwrap();
        let after = world.resource::<SimWorld>().state.tick.get();
        assert!(after > before);
        assert!(world.resource::<SimClock>().tick_alpha >= 0.0);
    }
}
