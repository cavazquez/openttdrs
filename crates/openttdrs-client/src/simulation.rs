//! Sistemas de avance de simulación independiente del render.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    MapTileChunk, RemapMapVisualsPending, VehicleIndex, large_map_viewport_cull_enabled,
};
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

/// Frecuencia del tick de simulación (debe coincidir con `Time<Fixed>`).
/// Calibrado con `REFERENCE_PROGRESS_STEP` (~5 ticks/tesela) y 74 ticks/día de OpenTTD.
pub(crate) const SIM_TICK_HZ: f64 = 5.0;

/// Fracción del tick de simulación actual (0..1) para interpolar el render entre pasos.
#[derive(Resource, Default)]
pub(crate) struct SimClock {
    pub(crate) tick_alpha: f32,
}

pub(crate) struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimClock>()
            .add_systems(Startup, init_sim_fixed_timestep)
            .add_systems(
                PreUpdate,
                sync_sim_time_controls.run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (step_sim, flag_map_tile_dirty_remap)
                    .chain()
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                sync_tick_alpha
                    .in_set(UpdateSet::Sim)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

fn init_sim_fixed_timestep(mut fixed: ResMut<Time<Fixed>>) {
    fixed.set_timestep_hz(SIM_TICK_HZ);
}

/// Pausa y velocidad de la simulación vía `Time<Virtual>` (antes del bucle FixedUpdate).
fn sync_sim_time_controls(hud: Res<SimHudControls>, mut virtual_time: ResMut<Time<Virtual>>) {
    if hud.paused {
        virtual_time.pause();
    } else {
        virtual_time.unpause();
    }
    virtual_time.set_relative_speed(hud.sim_speed.max(0.1));
}

/// Un paso de simulación por tick fijo (5 Hz).
fn step_sim(mut sim: ResMut<SimWorld>, mut vehicle_index: ResMut<VehicleIndex>) {
    sim.state.step();
    vehicle_index.rebuild(&sim.state.vehicles);
}

fn flag_map_tile_dirty_remap(sim: Res<SimWorld>, mut pending: ResMut<RemapMapVisualsPending>) {
    if sim.state.industry_tile_dirty.is_empty() && sim.state.signal_tile_dirty.is_empty() {
        return;
    }
    let (mw, mh) = sim.state.map.dimensions();
    pending.pending = true;
    pending.sync_camera = false;
    pending.full =
        !sim.state.signal_tile_dirty.is_empty() && !large_map_viewport_cull_enabled(mw, mh);
    for coord in sim
        .state
        .industry_tile_dirty
        .iter()
        .chain(sim.state.signal_tile_dirty.iter())
    {
        let ch = MapTileChunk::from_tile(coord.x.max(0) as u32, coord.y.max(0) as u32);
        pending.refresh_chunks.insert((ch.cx, ch.cy));
    }
}

/// Interpolación render: fracción del siguiente tick fijo.
fn sync_tick_alpha(
    hud: Res<SimHudControls>,
    fixed_time: Res<Time<Fixed>>,
    mut sim_clock: ResMut<SimClock>,
) {
    sim_clock.tick_alpha = if hud.paused {
        0.0
    } else {
        fixed_time.overstep_fraction()
    };
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        SIM_TICK_HZ, SimClock, init_sim_fixed_timestep, step_sim, sync_sim_time_controls,
        sync_tick_alpha,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::render::VehicleIndex;
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    fn sim_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<SimClock>();
        app.insert_resource(SimWorld::default());
        app.insert_resource(VehicleIndex::default());
        app.insert_resource(SimHudControls::default());
        app.add_systems(Startup, init_sim_fixed_timestep);
        app.add_systems(PreUpdate, sync_sim_time_controls);
        app.add_systems(FixedUpdate, step_sim);
        app.add_systems(Update, sync_tick_alpha);
        app
    }

    fn advance_app_time(app: &mut App, millis: u64) {
        let delta = std::time::Duration::from_millis(millis);
        let mut virtual_time = *app.world().resource::<Time<Virtual>>();
        virtual_time.advance_by(delta);
        app.world_mut().insert_resource(virtual_time);
        let mut time = *app.world().resource::<Time<()>>();
        time.advance_by(delta);
        app.world_mut().insert_resource(time);
        app.update();
    }

    #[test]
    fn init_sim_fixed_timestep_sets_5hz() {
        let mut app = sim_test_app();
        app.update();
        let hz = app
            .world()
            .resource::<Time<Fixed>>()
            .timestep()
            .as_secs_f64()
            .recip();
        assert!((hz - SIM_TICK_HZ).abs() < 0.01);
    }

    #[test]
    fn step_sim_paused_does_not_step() {
        let mut app = sim_test_app();
        app.update();
        {
            let mut hud = app.world_mut().resource_mut::<SimHudControls>();
            hud.paused = true;
        }
        let before = app.world().resource::<SimWorld>().state.tick.get();
        advance_app_time(&mut app, 500);
        let after = app.world().resource::<SimWorld>().state.tick.get();
        assert_eq!(before, after);
    }

    #[test]
    fn step_sim_steps_and_syncs_tick_alpha() {
        let mut app = sim_test_app();
        app.update();
        let before = app.world().resource::<SimWorld>().state.tick.get();
        app.world_mut().run_system_once(step_sim).unwrap();
        let after = app.world().resource::<SimWorld>().state.tick.get();
        assert!(after > before);
        app.world_mut().run_system_once(sync_tick_alpha).unwrap();
        assert!(app.world().resource::<SimClock>().tick_alpha >= 0.0);
    }
}
