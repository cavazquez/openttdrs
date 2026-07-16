use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{Vehicle, extrapolate_vehicle_pose};

use crate::render::CompanyColoredSprites;
use crate::simulation::SimClock;
use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, TruckHandles};
use super::pose::{vehicle_sprite_pos, vehicle_sprite_pos_at_with_catalog};
use super::spawn::{vehicle_cargo_color, vehicle_cargo_label};

/// Índice `Vehicle.id` → posición en `GameState::vehicles` (evita `find` O(V) por sprite).
#[derive(Resource, Default)]
pub(crate) struct VehicleIndex {
    pub(super) by_id: HashMap<u32, usize>,
}

impl VehicleIndex {
    pub(crate) fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_id.clear();
        self.by_id.reserve(vehicles.len());
        for (i, v) in vehicles.iter().enumerate() {
            self.by_id.insert(v.id, i);
        }
    }
}

pub(crate) fn rebuild_vehicle_index(sim: Res<SimWorld>, mut idx: ResMut<VehicleIndex>) {
    idx.rebuild(&sim.state.vehicles);
}

#[derive(Component)]
pub(crate) struct VehicleSprite(pub(super) u32);

/// Sprite de vagón enganchado (mismo id de cabeza + offset visual).
#[derive(Component)]
pub(crate) struct ConsistUnitSprite {
    pub(super) head_id: u32,
    pub(super) unit_index: usize,
}

#[derive(Component)]
pub(crate) struct VehicleCargoLabel(pub(super) u32);

fn vehicle_is_hidden_from_view(
    sim: &SimWorld,
    v: &Vehicle,
    pose: openttdrs_core::VehiclePose,
) -> bool {
    openttdrs_core::vehicle_hidden_from_view(&sim.state.map, v, pose.pos, pose.progress)
}

fn vehicle_owner_colour(sim: &SimWorld, v: &Vehicle) -> crate::sprites::CompanyColour {
    crate::sprites::CompanyColour::from_u8(
        sim.state
            .companies
            .get(v.owner.index())
            .map(|c| c.colour)
            .unwrap_or(sim.state.company_colour),
    )
}

pub(super) fn vehicle_tint(v: &Vehicle) -> Color {
    if v.pbs_stuck {
        Color::srgb(1.0, 0.75, 0.35)
    } else {
        Color::WHITE
    }
}

fn vehicle_cargo_label_pos(vehicle_pos: Vec3) -> Vec3 {
    Vec3::new(vehicle_pos.x, vehicle_pos.y + 21.0, vehicle_pos.z + 0.35)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_vehicles(
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    trucks: Res<TruckHandles>,
    mut company: ResMut<CompanyColoredSprites>,
    vehicle_index: Res<VehicleIndex>,
    mut cache: ResMut<NewGrfTrainSpriteCache>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite, &mut Visibility)>,
    mut trailers: Query<
        (
            &ConsistUnitSprite,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        Without<VehicleSprite>,
    >,
    mut labels: Query<
        (
            &VehicleCargoLabel,
            &mut Transform,
            &mut Text2d,
            &mut TextColor,
            &mut Visibility,
        ),
        (Without<VehicleSprite>, Without<ConsistUnitSprite>),
    >,
) {
    for c in &sim.state.companies {
        company.ensure_palette(
            crate::sprites::CompanyColour::from_u8(c.colour),
            &mut images,
        );
    }
    for (vs, mut transform, mut sprite, mut visibility) in &mut q {
        let Some(&i) = vehicle_index.by_id.get(&vs.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        let pose = extrapolate_vehicle_pose(v, sim_clock.tick_alpha);
        if vehicle_is_hidden_from_view(&sim, v, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos3 = vehicle_sprite_pos_at_with_catalog(
            v,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        transform.translation = pos3;
        sprite.image = trucks.for_vehicle_with_newgrf(
            v,
            pose,
            Some(&company),
            Some(vehicle_owner_colour(&sim, v)),
            &sim,
            &mut cache,
            &mut images,
        );
        sprite.color = vehicle_tint(v);
    }

    for (trailer, mut transform, mut sprite, mut visibility) in &mut trailers {
        let Some(&i) = vehicle_index.by_id.get(&trailer.head_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(head) = sim.state.vehicles.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, head.id);
        let Some(&uid) = ids.get(trailer.unit_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == uid) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let pose = extrapolate_vehicle_pose(head, sim_clock.tick_alpha);
        if vehicle_is_hidden_from_view(&sim, head, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let base = vehicle_sprite_pos_at_with_catalog(
            head,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let back = openttdrs_core::reverse_direction(head.direction);
        let (dx, dy) = match back {
            0 => (0.0, -8.0),
            1 => (6.0, -4.0),
            2 => (8.0, 0.0),
            3 => (6.0, 4.0),
            4 => (0.0, 8.0),
            5 => (-6.0, 4.0),
            6 => (-8.0, 0.0),
            _ => (-6.0, -4.0),
        };
        let i = trailer.unit_index as f32;
        transform.translation = Vec3::new(base.x + dx * i, base.y + dy * i, base.z - 0.01 * i);
        sprite.image = trucks.for_vehicle_with_newgrf(
            unit,
            pose,
            Some(&company),
            Some(vehicle_owner_colour(&sim, unit)),
            &sim,
            &mut cache,
            &mut images,
        );
        sprite.color = vehicle_tint(head);
    }

    for (label, mut transform, mut text, mut color, mut visibility) in &mut labels {
        let Some(&i) = vehicle_index.by_id.get(&label.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        let pose = extrapolate_vehicle_pose(v, sim_clock.tick_alpha);
        if vehicle_is_hidden_from_view(&sim, v, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos3 = vehicle_sprite_pos(v, &sim.state.map, sim_clock.tick_alpha);
        transform.translation = vehicle_cargo_label_pos(pos3);
        **text = vehicle_cargo_label(v);
        color.0 = vehicle_cargo_color(v);
    }
}
