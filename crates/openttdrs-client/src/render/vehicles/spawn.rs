use bevy::prelude::*;
use openttdrs_core::extrapolate_vehicle_pose;
use openttdrs_core::prelude::*;

use crate::render::{
    CompanyColoredSprites, MapVisualLayer, ViewportSortableChild, ViewportSortableParent,
    viewport_source_depth,
};
use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, NewGrfVehicleLayer, TruckHandles, vehicle_layers};
use super::pose::{
    aircraft_aux_sprite_pos_at, vehicle_insertion_key, vehicle_parent_bounds, vehicle_source_depth,
    vehicle_sprite_pos_at_offsets, vehicle_sprite_pos_at_with_catalog,
};
use super::sync::{
    AircraftRotorSprite, AircraftShadowSprite, ConsistUnitSprite, VehicleCargoLabel, VehicleSprite,
};

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

fn vehicle_cargo_label_pos(vehicle_pos: Vec3) -> Vec3 {
    Vec3::new(vehicle_pos.x, vehicle_pos.y + 21.0, vehicle_pos.z + 0.35)
}

const VEHICLE_SORT_SPRITE_ID: u32 = 0xFFFE_0000;

fn vehicle_uses_newgrf_stack(sim: &SimWorld, v: &Vehicle) -> bool {
    v.engine_id
        .and_then(|id| super::engine_in_sim(sim, id))
        .is_some_and(|engine| engine.sprite_stack)
}

#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_stack_children(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
    vehicle: &Vehicle,
    pose: openttdrs_core::VehiclePose,
    parent: Entity,
    parent_pos: Vec3,
    visibility: Visibility,
    layers: &[NewGrfVehicleLayer],
) {
    if !vehicle_uses_newgrf_stack(sim, vehicle) {
        return;
    }
    let fallback = layers
        .first()
        .map(|layer| layer.handle.clone())
        .unwrap_or_else(|| trucks.for_vehicle(vehicle, pose, None, None));
    // VehicleSpriteSeq has eight slots in OpenTTD. Keep stable child entities
    // for all slots so a later load/unload transition can reveal a new layer
    // without rebuilding the map hierarchy.
    for stack_index in 1..8 {
        let Some(layer) = layers.get(stack_index) else {
            commands.spawn((
                MapVisualLayer,
                super::sync::VehicleNewGrfStackSprite {
                    vehicle_id: vehicle.id,
                    stack_index,
                },
                Sprite {
                    image: fallback.clone(),
                    ..default()
                },
                Transform::from_translation(parent_pos),
                Visibility::Hidden,
                ViewportSortableChild {
                    parent,
                    source_depth: parent_pos.z,
                },
            ));
            continue;
        };
        let mut layer_pos = vehicle_sprite_pos_at_offsets(
            vehicle,
            &sim.state.map,
            pose,
            f32::from(layer.x_offs),
            f32::from(layer.y_offs),
            f32::from(layer.width),
            f32::from(layer.height),
        );
        let source_depth = vehicle_source_depth(vehicle, &sim.state.map, pose, layer_pos);
        layer_pos.z = source_depth;
        commands.spawn((
            MapVisualLayer,
            super::sync::VehicleNewGrfStackSprite {
                vehicle_id: vehicle.id,
                stack_index,
            },
            Sprite {
                image: layer.handle.clone(),
                ..default()
            },
            Transform::from_translation(layer_pos),
            visibility,
            ViewportSortableChild {
                parent,
                source_depth,
            },
        ));
    }
}

pub(super) fn vehicle_cargo_label(v: &Vehicle) -> String {
    let cargo = v.cargo_type.map_or("ANY", openttdrs_core::CargoType::label);
    format!("{cargo} {}/{}", v.cargo, v.capacity)
}

pub(super) fn vehicle_cargo_color(v: &Vehicle) -> Color {
    if v.cargo > 0 {
        Color::srgb(0.95, 0.9, 0.35)
    } else {
        Color::srgba(0.8, 0.85, 0.9, 0.72)
    }
}

pub(crate) fn spawn_initial_vehicles(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
    company: &mut CompanyColoredSprites,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
) {
    let mut fleet_index = openttdrs_core::FleetIndex::default();
    fleet_index.rebuild(&sim.state.vehicles);
    for c in &sim.state.companies {
        company.ensure_palette(crate::sprites::CompanyColour::from_u8(c.colour), images);
    }
    for vehicle in &sim.state.vehicles {
        if vehicle.is_wagon_unit() {
            continue;
        }
        let pose = extrapolate_vehicle_pose(vehicle, 0.0);
        let layers = trucks.for_vehicle_with_newgrf_layers(
            vehicle,
            pose,
            Some(company),
            Some(vehicle_owner_colour(sim, vehicle)),
            sim,
            cache,
            images,
        );
        let mut pos3 = layers.first().map_or_else(
            || {
                vehicle_sprite_pos_at_with_catalog(
                    vehicle,
                    &sim.state.map,
                    pose,
                    Some(&sim.state.engine_catalog),
                )
            },
            |layer| {
                vehicle_sprite_pos_at_offsets(
                    vehicle,
                    &sim.state.map,
                    pose,
                    f32::from(layer.x_offs),
                    f32::from(layer.y_offs),
                    f32::from(layer.width),
                    f32::from(layer.height),
                )
            },
        );
        let vis = if vehicle_is_hidden_from_view(sim, vehicle, pose) {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        let source_depth = vehicle_source_depth(vehicle, &sim.state.map, pose, pos3);
        pos3.z = source_depth;
        let vehicle_image = layers
            .first()
            .map(|layer| layer.handle.clone())
            .unwrap_or_else(|| {
                trucks.for_vehicle(
                    vehicle,
                    pose,
                    Some(company),
                    Some(vehicle_owner_colour(sim, vehicle)),
                )
            });
        let vehicle_entity = commands
            .spawn((
                MapVisualLayer,
                VehicleSprite(vehicle.id),
                Sprite {
                    image: vehicle_image,
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(pos3),
                vis,
                ViewportSortableParent {
                    sprite_id: VEHICLE_SORT_SPRITE_ID,
                    bounds: vehicle_parent_bounds(vehicle, &sim.state.map, pose),
                    insertion_key: vehicle_insertion_key(vehicle, pose),
                    source_depth,
                },
            ))
            .id();
        spawn_newgrf_stack_children(
            commands,
            sim,
            trucks,
            vehicle,
            pose,
            vehicle_entity,
            pos3,
            vis,
            &layers,
        );
        if vehicle.kind == VehicleKind::Aircraft {
            let layer = &vehicle_layers(vehicle)
                [openttdrs_core::vehicle_render_direction_at(vehicle, pose).min(7) as usize];
            let mut shadow_pos =
                aircraft_aux_sprite_pos_at(vehicle, &sim.state.map, pose, layer, false, 0.85);
            let shadow_source_depth = viewport_source_depth(
                shadow_pos.z,
                u32::try_from(pose.pos.x).unwrap_or(0),
                sim.state.map.dimensions().0,
            );
            shadow_pos.z = shadow_source_depth;
            commands.spawn((
                MapVisualLayer,
                AircraftShadowSprite(vehicle.id),
                Sprite {
                    image: trucks.for_vehicle(vehicle, pose, None, None),
                    color: Color::srgba(0.08, 0.08, 0.08, 0.5),
                    ..default()
                },
                Transform::from_translation(shadow_pos),
                vis,
                ViewportSortableChild {
                    parent: vehicle_entity,
                    source_depth: shadow_source_depth,
                },
            ));
            if vehicle
                .engine_id
                .is_some_and(openttdrs_core::aircraft_is_helicopter)
            {
                let rotor = &super::assets::AIRCRAFT_ROTOR_LAYERS[0];
                let mut rotor_pos =
                    aircraft_aux_sprite_pos_at(vehicle, &sim.state.map, pose, rotor, true, 1.1);
                let rotor_source_depth = viewport_source_depth(
                    rotor_pos.z,
                    u32::try_from(pose.pos.x).unwrap_or(0),
                    sim.state.map.dimensions().0,
                );
                rotor_pos.z = rotor_source_depth;
                commands.spawn((
                    MapVisualLayer,
                    AircraftRotorSprite(vehicle.id),
                    Sprite {
                        image: trucks.aircraft_rotor(0),
                        ..default()
                    },
                    Transform::from_translation(rotor_pos),
                    vis,
                    ViewportSortableChild {
                        parent: vehicle_entity,
                        source_depth: rotor_source_depth,
                    },
                ));
            }
        }
        if vehicle.kind == VehicleKind::Train {
            spawn_consist_trailer_sprites(
                commands,
                sim,
                trucks,
                company,
                vehicle,
                vis,
                cache,
                images,
                &fleet_index,
            );
        }
        // OpenTTD no dibuja texto libre de carga sobre cada vehículo en el
        // viewport. Mantenerlo sólo como ayuda explícita de diagnóstico evita
        // que esos textos compitan con carteles y estaciones en zoom lejano.
        if crate::config::env_flag("OPENTTDRS_DEBUG_VEHICLE_CARGO_LABELS")
            && !crate::sprites::is_hidden(crate::sprites::TransparencyOption::Text)
        {
            commands.spawn((
                MapVisualLayer,
                VehicleCargoLabel(vehicle.id),
                Text2d::new(vehicle_cargo_label(vehicle)),
                TextFont {
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(crate::sprites::text_color(
                    crate::sprites::TransparencyOption::Text,
                    vehicle_cargo_color(vehicle),
                )),
                Transform::from_translation(vehicle_cargo_label_pos(pos3)),
                vis,
            ));
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn spawn_consist_trailer_sprites(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
    company: &CompanyColoredSprites,
    head: &Vehicle,
    vis: Visibility,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
    fleet_index: &openttdrs_core::FleetIndex,
) {
    let owner_colour = Some(vehicle_owner_colour(sim, head));
    let ids = fleet_index.consist(head.id);
    for (i, &uid) in ids.iter().enumerate().skip(1) {
        let Some(unit) = fleet_index
            .slot(uid)
            .and_then(|slot| sim.state.vehicles.get(slot))
        else {
            continue;
        };
        let unit_pose = openttdrs_core::VehiclePose::from_vehicle(unit);
        let layers = trucks.for_vehicle_with_newgrf_layers(
            unit,
            unit_pose,
            Some(company),
            owner_colour,
            sim,
            cache,
            images,
        );
        let mut base = layers.first().map_or_else(
            || {
                vehicle_sprite_pos_at_with_catalog(
                    unit,
                    &sim.state.map,
                    unit_pose,
                    Some(&sim.state.engine_catalog),
                )
            },
            |layer| {
                vehicle_sprite_pos_at_offsets(
                    unit,
                    &sim.state.map,
                    unit_pose,
                    f32::from(layer.x_offs),
                    f32::from(layer.y_offs),
                    f32::from(layer.width),
                    f32::from(layer.height),
                )
            },
        );
        let source_depth = vehicle_source_depth(unit, &sim.state.map, unit_pose, base);
        base.z = source_depth;
        let unit_image = layers
            .first()
            .map(|layer| layer.handle.clone())
            .unwrap_or_else(|| trucks.for_vehicle(unit, unit_pose, Some(company), owner_colour));
        let unit_entity = commands
            .spawn((
                MapVisualLayer,
                ConsistUnitSprite {
                    head_id: head.id,
                    unit_index: i,
                },
                Sprite {
                    image: unit_image,
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(base),
                vis,
                ViewportSortableParent {
                    sprite_id: VEHICLE_SORT_SPRITE_ID,
                    bounds: vehicle_parent_bounds(unit, &sim.state.map, unit_pose),
                    insertion_key: vehicle_insertion_key(unit, unit_pose),
                    source_depth,
                },
            ))
            .id();
        spawn_newgrf_stack_children(
            commands,
            sim,
            trucks,
            unit,
            unit_pose,
            unit_entity,
            base,
            vis,
            &layers,
        );
    }
}
