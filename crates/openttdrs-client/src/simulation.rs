//! Sistemas de avance de simulación independiente del render.

use std::time::Duration;

use bevy::prelude::*;

use openttdrs_core::SIM_TICKS_PER_SECOND;

use crate::bevy_app::FixedUpdateSet;
use crate::network::{NetworkRole, NetworkRuntime};
use crate::render::{
    MapTileChunk, RemapMapVisualsPending, VehicleIndex, large_map_viewport_cull_enabled,
};
use crate::state::{ClientScreen, SimRunState, SimWorld, sim_is_paused};
use crate::ui::SimHudControls;

/// Frecuencia del tick de simulación (debe coincidir con `Time<Fixed>`).
/// OpenTTD: `1000 / MILLISECONDS_PER_TICK` (~37 Hz, `timer_game_tick.h`).
pub(crate) const SIM_TICK_HZ: f64 = SIM_TICKS_PER_SECOND;

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
            .add_systems(OnEnter(SimRunState::Paused), pause_virtual_time)
            .add_systems(OnEnter(SimRunState::Running), unpause_virtual_time)
            .add_systems(
                PreUpdate,
                sync_sim_time_controls.run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (step_sim, flag_map_tile_dirty_remap)
                    .chain()
                    .in_set(FixedUpdateSet::Sim)
                    .run_if(
                        in_state(ClientScreen::InGame).and_then(in_state(SimRunState::Running)),
                    ),
            );
    }
}

fn init_sim_fixed_timestep(
    mut fixed: ResMut<Time<Fixed>>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    fixed.set_timestep_hz(SIM_TICK_HZ);
    // Como máximo un tick de sim por frame render: evita saltos de varias teselas tras lag.
    virtual_time.set_max_delta(Duration::from_secs_f64(1.0 / SIM_TICK_HZ));
}

fn pause_virtual_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.pause();
}

fn unpause_virtual_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.unpause();
}

/// Velocidad de la simulación vía `Time<Virtual>` (antes del bucle FixedUpdate).
fn sync_sim_time_controls(hud: Res<SimHudControls>, mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.set_relative_speed(hud.sim_speed.max(0.1));
}

/// Un paso de simulación por tick fijo (~37 Hz, paridad OpenTTD).
///
/// En cliente-only (`--client`) no avanza: los ticks llegan por `AdvanceTicks`.
fn step_sim(
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    net: Option<Res<NetworkRuntime>>,
) {
    if net.is_some_and(|n| n.role() == NetworkRole::Client) {
        return;
    }
    sim.state.step();
    vehicle_index.rebuild(&sim.state.vehicles);
}

fn flag_map_tile_dirty_remap(sim: Res<SimWorld>, mut pending: ResMut<RemapMapVisualsPending>) {
    if sim.state.runtime.industry_tile_dirty.is_empty()
        && sim.state.runtime.landscape_tile_dirty.is_empty()
        && sim.state.runtime.signal_tile_dirty.is_empty()
        && sim.state.runtime.reservation_tile_dirty.is_empty()
    {
        return;
    }
    let (mw, mh) = sim.state.map.dimensions();
    pending.pending = true;
    pending.sync_camera = false;
    pending.full = (!sim.state.runtime.signal_tile_dirty.is_empty()
        || !sim.state.runtime.reservation_tile_dirty.is_empty())
        && !large_map_viewport_cull_enabled(mw, mh);
    for coord in sim
        .state
        .runtime
        .industry_tile_dirty
        .iter()
        .chain(sim.state.runtime.landscape_tile_dirty.iter())
        .chain(sim.state.runtime.signal_tile_dirty.iter())
        .chain(sim.state.runtime.reservation_tile_dirty.iter())
    {
        let ch = MapTileChunk::from_tile(coord.x.max(0) as u32, coord.y.max(0) as u32);
        pending.refresh_chunks.insert((ch.cx, ch.cy));
    }
}

/// Interpolación render: fracción del siguiente tick fijo.
pub(crate) fn sync_tick_alpha(
    run_state: Res<State<SimRunState>>,
    fixed_time: Res<Time<Fixed>>,
    mut sim_clock: ResMut<SimClock>,
) {
    sim_clock.tick_alpha = if sim_is_paused(&run_state) {
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
    use bevy::state::app::StatesPlugin;

    use crate::render::VehicleIndex;
    use crate::state::{ClientScreen, SimRunState, SimWorld};
    use crate::ui::SimHudControls;

    fn sim_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<ClientScreen>();
        app.add_sub_state::<SimRunState>();
        app.init_resource::<SimClock>();
        app.insert_resource(SimWorld::default());
        app.insert_resource(VehicleIndex::default());
        app.insert_resource(SimHudControls::default());
        app.add_systems(Startup, init_sim_fixed_timestep);
        app.add_systems(OnEnter(SimRunState::Paused), super::pause_virtual_time);
        app.add_systems(OnEnter(SimRunState::Running), super::unpause_virtual_time);
        app.add_systems(PreUpdate, sync_sim_time_controls);
        app.add_systems(FixedUpdate, step_sim.run_if(in_state(SimRunState::Running)));
        app.add_systems(Update, sync_tick_alpha);
        app.world_mut()
            .resource_mut::<NextState<ClientScreen>>()
            .set(ClientScreen::InGame);
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
    fn init_sim_fixed_timestep_sets_openttd_tick_rate() {
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
        app.world_mut()
            .resource_mut::<NextState<SimRunState>>()
            .set(SimRunState::Paused);
        app.update();
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
