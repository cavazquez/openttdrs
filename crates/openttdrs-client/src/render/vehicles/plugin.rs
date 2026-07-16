use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;

use super::assets::NewGrfTrainSpriteCache;
use super::sync::{VehicleIndex, rebuild_vehicle_index, update_vehicles};

pub(crate) struct VehicleRenderPlugin;

impl Plugin for VehicleRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIndex>()
            .init_resource::<NewGrfTrainSpriteCache>()
            .add_systems(OnEnter(ClientScreen::InGame), rebuild_vehicle_index)
            .add_systems(
                Update,
                (crate::simulation::sync_tick_alpha, update_vehicles)
                    .chain()
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
