use bevy::prelude::*;
use openttdrs_core::{Vehicle, VehicleKind, extrapolate_vehicle_pose};

use crate::render::{CompanyColoredSprites, MapVisualLayer};
use crate::state::SimWorld;

use super::assets::{NewGrfTrainSpriteCache, TruckHandles};
use super::pose::vehicle_sprite_pos_at_with_catalog;
use super::sync::{ConsistUnitSprite, VehicleCargoLabel, VehicleSprite};

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

pub(super) fn vehicle_cargo_label(v: &Vehicle) -> String {
    let cargo = match v.cargo_type {
        Some(openttdrs_core::CargoType::Passengers) => "PAX",
        Some(openttdrs_core::CargoType::Mail) => "MAIL",
        Some(openttdrs_core::CargoType::Goods) => "GOODS",
        Some(openttdrs_core::CargoType::Coal) => "COAL",
        Some(openttdrs_core::CargoType::Wood) => "WOOD",
        Some(openttdrs_core::CargoType::Oil) => "OIL",
        Some(openttdrs_core::CargoType::Livestock) => "LIVE",
        Some(openttdrs_core::CargoType::Grain) => "GRAIN",
        Some(openttdrs_core::CargoType::IronOre) => "ORE",
        Some(openttdrs_core::CargoType::Steel) => "STEEL",
        Some(openttdrs_core::CargoType::Valuables) => "VAL",
        None => "ANY",
    };
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
    for c in &sim.state.companies {
        company.ensure_palette(crate::sprites::CompanyColour::from_u8(c.colour), images);
    }
    for vehicle in &sim.state.vehicles {
        if vehicle.is_wagon_unit() {
            continue;
        }
        let pose = extrapolate_vehicle_pose(vehicle, 0.0);
        let pos3 = vehicle_sprite_pos_at_with_catalog(
            vehicle,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let vis = if vehicle_is_hidden_from_view(sim, vehicle, pose) {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        commands.spawn((
            MapVisualLayer,
            VehicleSprite(vehicle.id),
            Sprite {
                image: trucks.for_vehicle_with_newgrf(
                    vehicle,
                    pose,
                    Some(company),
                    Some(vehicle_owner_colour(sim, vehicle)),
                    sim,
                    cache,
                    images,
                ),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(pos3),
            vis,
        ));
        if vehicle.kind == VehicleKind::Train {
            spawn_consist_trailer_sprites(
                commands, sim, trucks, company, vehicle, pose, vis, cache, images,
            );
        }
        if !crate::sprites::is_hidden(crate::sprites::TransparencyOption::Text) {
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
    pose: openttdrs_core::VehiclePose,
    vis: Visibility,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
) {
    let owner_colour = Some(vehicle_owner_colour(sim, head));
    let ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, head.id);
    for (i, &uid) in ids.iter().enumerate().skip(1) {
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == uid) else {
            continue;
        };
        let unit_pose = pose;
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
        let base = vehicle_sprite_pos_at_with_catalog(
            head,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let fi = i as f32;
        commands.spawn((
            MapVisualLayer,
            ConsistUnitSprite {
                head_id: head.id,
                unit_index: i,
            },
            Sprite {
                image: trucks.for_vehicle_with_newgrf(
                    unit,
                    unit_pose,
                    Some(company),
                    owner_colour,
                    sim,
                    cache,
                    images,
                ),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(Vec3::new(
                base.x + dx * fi,
                base.y + dy * fi,
                base.z - 0.01 * fi,
            )),
            vis,
        ));
    }
}
