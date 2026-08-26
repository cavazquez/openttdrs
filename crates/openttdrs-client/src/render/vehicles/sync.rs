use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::render::{CompanyColoredSprites, ViewportSortableChild, ViewportSortableParent};
use crate::simulation::SimClock;
use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, TruckHandles, vehicle_layers};
use super::pose::{
    aircraft_aux_sprite_pos_at, vehicle_insertion_key, vehicle_parent_bounds,
    vehicle_pose_for_construction, vehicle_source_depth, vehicle_sprite_pos,
    vehicle_sprite_pos_at_with_catalog,
};
use super::spawn::{vehicle_cargo_color, vehicle_cargo_label};

/// Índice `Vehicle.id` → posición en `GameState::vehicles` (evita `find` O(V) por sprite).
#[derive(Resource, Default)]
pub(crate) struct VehicleIndex {
    pub(super) core: openttdrs_core::FleetIndex,
}

impl VehicleIndex {
    pub(crate) fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.core.rebuild(vehicles);
    }
}

pub(crate) fn rebuild_vehicle_index(sim: Res<SimWorld>, mut idx: ResMut<VehicleIndex>) {
    idx.rebuild(&sim.state.vehicles);
}

#[derive(Component)]
pub(crate) struct VehicleSprite(pub(super) u32);

#[derive(Component)]
pub(crate) struct AircraftShadowSprite(pub(super) u32);

#[derive(Component)]
pub(crate) struct AircraftRotorSprite(pub(super) u32);

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

#[must_use]
fn aircraft_rotor_frame(v: &Vehicle, tick: u64) -> usize {
    if !v.running || v.awaiting_load_window || v.cur_speed == 0 {
        0
    } else {
        1 + usize::try_from((tick / 2) % 3).unwrap_or(0)
    }
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
    mut q: Query<(
        &VehicleSprite,
        &mut Transform,
        &mut Sprite,
        &mut Visibility,
        Option<&mut ViewportSortableParent>,
    )>,
    mut trailers: Query<
        (
            &ConsistUnitSprite,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
            Option<&mut ViewportSortableParent>,
        ),
        (
            Without<VehicleSprite>,
            Without<AircraftShadowSprite>,
            Without<AircraftRotorSprite>,
        ),
    >,
    mut labels: Query<
        (
            &VehicleCargoLabel,
            &mut Transform,
            &mut Text2d,
            &mut TextColor,
            &mut Visibility,
        ),
        (
            Without<VehicleSprite>,
            Without<ConsistUnitSprite>,
            Without<AircraftShadowSprite>,
            Without<AircraftRotorSprite>,
        ),
    >,
    mut shadows: Query<
        (
            &AircraftShadowSprite,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
            Option<&mut ViewportSortableChild>,
        ),
        (
            Without<VehicleSprite>,
            Without<ConsistUnitSprite>,
            Without<VehicleCargoLabel>,
            Without<AircraftRotorSprite>,
        ),
    >,
    mut rotors: Query<
        (
            &AircraftRotorSprite,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
            Option<&mut ViewportSortableChild>,
        ),
        (
            Without<VehicleSprite>,
            Without<ConsistUnitSprite>,
            Without<VehicleCargoLabel>,
            Without<AircraftShadowSprite>,
        ),
    >,
) {
    for c in &sim.state.companies {
        company.ensure_palette(
            crate::sprites::CompanyColour::from_u8(c.colour),
            &mut images,
        );
    }
    for (vs, mut transform, mut sprite, mut visibility, mut parent) in &mut q {
        let Some(i) = vehicle_index.core.slot(vs.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        let pose = vehicle_pose_for_construction(v, sim_clock.tick_alpha, sim.state.construction);
        if vehicle_is_hidden_from_view(&sim, v, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let mut pos3 = vehicle_sprite_pos_at_with_catalog(
            v,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let source_depth = vehicle_source_depth(v, &sim.state.map, pose, pos3);
        pos3.z = source_depth;
        transform.translation = pos3;
        if let Some(parent) = parent.as_deref_mut() {
            parent.bounds = vehicle_parent_bounds(v, &sim.state.map, pose);
            parent.insertion_key = vehicle_insertion_key(v, pose);
            parent.source_depth = source_depth;
        }
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

    for (trailer, mut transform, mut sprite, mut visibility, mut parent) in &mut trailers {
        let Some(i) = vehicle_index.core.slot(trailer.head_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(head) = sim.state.vehicles.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let ids = vehicle_index.core.consist(head.id);
        let Some(&uid) = ids.get(trailer.unit_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(unit) = vehicle_index
            .core
            .slot(uid)
            .and_then(|slot| sim.state.vehicles.get(slot))
        else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let trailer_pose = openttdrs_core::VehiclePose::from_vehicle(unit)
            .with_drive_on_right(sim.state.construction.road_drive_on_right());
        if vehicle_is_hidden_from_view(&sim, unit, trailer_pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let base = vehicle_sprite_pos_at_with_catalog(
            unit,
            &sim.state.map,
            trailer_pose,
            Some(&sim.state.engine_catalog),
        );
        let source_depth = vehicle_source_depth(unit, &sim.state.map, trailer_pose, base);
        let mut sorted_base = base;
        sorted_base.z = source_depth;
        transform.translation = sorted_base;
        if let Some(parent) = parent.as_deref_mut() {
            parent.bounds = vehicle_parent_bounds(unit, &sim.state.map, trailer_pose);
            parent.insertion_key = vehicle_insertion_key(unit, trailer_pose);
            parent.source_depth = source_depth;
        }
        sprite.image = trucks.for_vehicle_with_newgrf(
            unit,
            trailer_pose,
            Some(&company),
            Some(vehicle_owner_colour(&sim, unit)),
            &sim,
            &mut cache,
            &mut images,
        );
        sprite.color = vehicle_tint(unit);
    }

    for (shadow, mut transform, mut sprite, mut visibility, mut child) in &mut shadows {
        let Some(i) = vehicle_index.core.slot(shadow.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let pose = vehicle_pose_for_construction(v, sim_clock.tick_alpha, sim.state.construction);
        if vehicle_is_hidden_from_view(&sim, v, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let dir = openttdrs_core::vehicle_render_direction_at(v, pose).min(7) as usize;
        let layer = &vehicle_layers(v)[dir];
        let mut shadow_pos =
            aircraft_aux_sprite_pos_at(v, &sim.state.map, pose, layer, false, 0.85);
        let source_depth = vehicle_source_depth(v, &sim.state.map, pose, shadow_pos);
        shadow_pos.z = source_depth;
        transform.translation = shadow_pos;
        if let Some(child) = child.as_deref_mut() {
            child.source_depth = source_depth;
        }
        sprite.image = trucks.for_vehicle(v, pose, None, None);
    }

    for (rotor, mut transform, mut sprite, mut visibility, mut child) in &mut rotors {
        let Some(i) = vehicle_index.core.slot(rotor.0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let pose = vehicle_pose_for_construction(v, sim_clock.tick_alpha, sim.state.construction);
        if vehicle_is_hidden_from_view(&sim, v, pose) {
            *visibility = Visibility::Hidden;
            continue;
        }
        let frame = aircraft_rotor_frame(v, sim.state.tick.get());
        let layer = &super::assets::AIRCRAFT_ROTOR_LAYERS[frame];
        *visibility = Visibility::Visible;
        let mut rotor_pos = aircraft_aux_sprite_pos_at(v, &sim.state.map, pose, layer, true, 1.1);
        let source_depth = vehicle_source_depth(v, &sim.state.map, pose, rotor_pos);
        rotor_pos.z = source_depth;
        transform.translation = rotor_pos;
        if let Some(child) = child.as_deref_mut() {
            child.source_depth = source_depth;
        }
        sprite.image = trucks.aircraft_rotor(frame);
    }

    for (label, mut transform, mut text, mut color, mut visibility) in &mut labels {
        let Some(i) = vehicle_index.core.slot(label.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        let pose = vehicle_pose_for_construction(v, sim_clock.tick_alpha, sim.state.construction);
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

#[cfg(test)]
mod aircraft_aux_tests {
    use super::*;

    #[test]
    fn rotor_stops_and_cycles_only_while_running() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Aircraft,
            TileCoord::new(1, 1),
            TileCoord::new(2, 2),
        );
        v.engine_id = Some(openttdrs_core::ENGINE_AIRCRAFT_TRICARIO);
        v.cur_speed = 40;
        assert_eq!(aircraft_rotor_frame(&v, 0), 1);
        assert_eq!(aircraft_rotor_frame(&v, 2), 2);
        assert_eq!(aircraft_rotor_frame(&v, 4), 3);
        assert_eq!(aircraft_rotor_frame(&v, 6), 1);
        v.awaiting_load_window = true;
        assert_eq!(aircraft_rotor_frame(&v, 8), 0);
        v.awaiting_load_window = false;
        v.running = false;
        assert_eq!(aircraft_rotor_frame(&v, 8), 0);
    }
}
