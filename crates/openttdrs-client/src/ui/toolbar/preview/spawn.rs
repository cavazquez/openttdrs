//! Spawn: traducir PreviewPlan en entidades Bevy.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::rail_station_footprint;

use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, tile_pos_half, tile_slope_and_min_z};
use crate::render::{CompanyColoredSprites, TileAtlas};
use crate::state::SimWorld;
use crate::ui::toolbar::StationBuildState;

use super::BuildGhostPreview;
use super::bridge::spawn_bridge_span_preview;
use super::industry::spawn_industry_template_preview;
use super::plan::{PreviewPlan, TilePreviewKind, TilePreviewPlan, preview_tint, rail_signal_tint};
use super::rail_depot::{RailDepotPreviewSpawn, spawn_rail_depot_preview};
use super::rail_station::spawn_rail_station_area_sprite_preview;
use super::rail_waypoint::spawn_rail_waypoint_preview;
use super::road_depot::{RoadDepotPreviewSpawn, spawn_road_depot_preview};
use super::road_stop::{
    RoadStopPreviewSpawn, bus_stop_ground_path, spawn_road_stop_preview, truck_stop_ground_path,
};
use super::road_waypoint::spawn_road_waypoint_preview;
use super::sprites::preview_image_for_action;
use super::station_coverage::{spawn_station_coverage_preview, station_preview_has_coverage};
use super::tunnel::spawn_tunnel_entrance_preview;
use super::validation::preview_build_command_valid;

use crate::sprites::StationTileClass;

/// Spawn de entidades según el plan de preview.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
pub(crate) fn spawn_preview_plan(
    commands: &mut Commands,
    plan: &PreviewPlan,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    company: Option<&CompanyColoredSprites>,
    sim: &SimWorld,
    station_state: &StationBuildState,
    action: crate::ui::toolbar::BuildMenuAction,
    anim_cursor_frame: u8,
) {
    match plan {
        PreviewPlan::None | PreviewPlan::HandledByDedicatedSystem => {}
        PreviewPlan::RailStation {
            origin,
            show_coverage,
        } => {
            spawn_rail_station_area_preview(
                commands,
                asset_server,
                atlas,
                company,
                sim,
                station_state,
                *origin,
                *show_coverage,
            );
        }
        PreviewPlan::Airport {
            origin,
            show_coverage,
        } => {
            spawn_airport_preview(
                commands,
                asset_server,
                sim,
                station_state,
                *origin,
                *show_coverage,
            );
        }
        PreviewPlan::RailWaypoint { coord, valid } => {
            spawn_rail_waypoint_preview(commands, atlas, company, &sim.state.map, *coord, *valid);
        }
        PreviewPlan::RoadWaypoint { coord, valid } => {
            spawn_road_waypoint_preview(commands, asset_server, &sim.state.map, *coord, *valid);
        }
        PreviewPlan::BridgeSpan { tiles, valid } => {
            spawn_bridge_span_preview(
                commands,
                asset_server,
                action,
                tiles,
                &sim.state.map,
                *valid,
            );
        }
        PreviewPlan::RailSignalDrag {
            tiles,
            signal_fract,
        } => {
            spawn_rail_signal_drag_preview(
                commands,
                &sim.state,
                tiles,
                *signal_fract,
                station_state,
            );
        }
        PreviewPlan::TileByTile { tiles } => {
            for tile_plan in tiles {
                spawn_tile_preview(
                    commands,
                    tile_plan,
                    asset_server,
                    atlas,
                    company,
                    &sim.state.map,
                    action,
                    anim_cursor_frame,
                );
            }
        }
    }
}

/// Spawn preview de estación de tren.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
fn spawn_rail_station_area_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    company: Option<&CompanyColoredSprites>,
    sim: &SimWorld,
    station_state: &StationBuildState,
    origin: TileCoord,
    show_coverage: bool,
) {
    let (w, h) = rail_station_footprint(
        station_state.rail_axis_y,
        station_state.rail_platforms,
        station_state.rail_length,
    );
    let cmd = Command::PlaceRailStationArea {
        origin,
        axis_y: station_state.rail_axis_y,
        platforms: station_state.rail_platforms,
        length: station_state.rail_length,
    };
    let valid = command_would_fail(&sim.state, &cmd).is_none();

    if show_coverage {
        let anchor = (origin.x + (w - 1) / 2, origin.y + (h - 1) / 2);
        spawn_station_coverage_preview(
            commands,
            asset_server,
            &sim.state.map,
            &[anchor],
            station_preview_has_coverage(&sim.state.map, &sim.state.industries, anchor.0, anchor.1),
        );
    }

    spawn_rail_station_area_sprite_preview(
        commands,
        atlas,
        company,
        &sim.state.map,
        origin,
        station_state.rail_axis_y,
        station_state.rail_platforms,
        station_state.rail_length,
        valid,
    );
}

/// Spawn preview de aeropuerto.
fn spawn_airport_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    sim: &SimWorld,
    station_state: &StationBuildState,
    origin: TileCoord,
    show_coverage: bool,
) {
    use openttdrs_core::prelude::*;
    use openttdrs_core::{
        STATION_COVERAGE_RADIUS, airport_spec_def, airport_spec_footprint, airport_spec_tiles,
    };

    let spec = station_state.airport_spec;
    let axis_y = station_state.airport_axis_y;
    let (w, h) = airport_spec_footprint(spec, axis_y);
    let cmd = Command::PlaceAirportArea {
        origin,
        axis_y,
        spec,
    };
    let valid = command_would_fail(&sim.state, &cmd).is_none();
    let select = asset_server.load::<Image>("assets/opengfx/tiles/tile_select.png");
    let tint = if valid {
        Color::srgba(0.85, 0.95, 1.0, 0.95)
    } else {
        Color::srgba(1.0, 0.3, 0.25, 0.95)
    };

    for (coord, _piece) in airport_spec_tiles(origin, spec, axis_y) {
        let Some(tile) = sim.state.map.get(coord) else {
            continue;
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: select.clone(),
                color: tint,
                ..default()
            },
            Transform::from_translation(crate::iso::tile_pos(coord.x, coord.y, tile.height, 3.0))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));
    }

    if show_coverage {
        let radius = airport_spec_def(spec)
            .map(|d| d.catchment)
            .unwrap_or(STATION_COVERAGE_RADIUS);
        let coverage_img = asset_server.load::<Image>("assets/opengfx/tiles/tile_select.png");
        let coverage_tint = Color::srgba(0.35, 0.85, 0.45, 0.28);
        for dy in -radius..=(h - 1 + radius) {
            for dx in -radius..=(w - 1 + radius) {
                let x = origin.x + dx;
                let y = origin.y + dy;
                if dx >= 0 && dy >= 0 && dx < w && dy < h {
                    continue;
                }
                let Some(tile) = sim.state.map.get(TileCoord::new(x, y)) else {
                    continue;
                };
                commands.spawn((
                    BuildGhostPreview,
                    Sprite {
                        image: coverage_img.clone(),
                        color: coverage_tint,
                        ..default()
                    },
                    Transform::from_translation(crate::iso::tile_pos(x, y, tile.height, 2.5))
                        .with_scale(Vec3::new(1.001, 1.001, 1.0)),
                ));
            }
        }
    }
}

/// Spawn preview de arrastre de señales ferroviarias.
fn spawn_rail_signal_drag_preview(
    commands: &mut Commands,
    state: &GameState,
    tiles: &[(i32, i32)],
    signal_fract: (u8, u8),
    station_state: &StationBuildState,
) {
    for &(px, py) in tiles {
        let coord = TileCoord::new(px, py);
        if state.map.get(coord).is_none() {
            continue;
        }
        let valid = preview_build_command_valid(
            state,
            crate::ui::toolbar::BuildMenuAction::RailSignals,
            coord,
            station_state,
            &[(px, py)],
            None,
            Some(signal_fract),
        );
        let tint = rail_signal_tint(valid);
        let (tileh, base_z) = tile_slope_and_min_z(&state.map, px as u32, py as u32);
        let half_h = if tileh == 0 {
            TILE_HALF_H
        } else {
            SLOPE_HALF_H[tileh as usize]
        };
        let pos = tile_pos_half(px, py, base_z, 0.05, half_h);
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                color: tint,
                custom_size: Some(Vec2::new(64.0, 32.0)),
                ..default()
            },
            Transform::from_translation(pos),
        ));
    }
}

/// Spawn preview de un tile individual.
#[allow(clippy::too_many_arguments)] // parámetros ECS/assets de spawn
fn spawn_tile_preview(
    commands: &mut Commands,
    tile_plan: &TilePreviewPlan,
    asset_server: &AssetServer,
    atlas: Option<&TileAtlas>,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    action: crate::ui::toolbar::BuildMenuAction,
    anim_cursor_frame: u8,
) {
    let coord = tile_plan.coord;
    let (px, py) = (coord.x, coord.y);
    let (tileh, base_z) = tile_slope_and_min_z(map, px as u32, py as u32);
    let half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let tint = preview_tint(tile_plan.valid);

    match &tile_plan.kind {
        TilePreviewKind::Industry { spec } => {
            spawn_industry_template_preview(commands, asset_server, map, coord, *spec, tint);
        }
        TilePreviewKind::RoadStop { is_bus, dir } => {
            let (class, ground) = if *is_bus {
                (StationTileClass::Bus, bus_stop_ground_path(*dir))
            } else {
                (StationTileClass::Truck, truck_stop_ground_path(*dir))
            };
            spawn_road_stop_preview(
                commands,
                RoadStopPreviewSpawn {
                    px,
                    py,
                    base_z,
                    half_h,
                    class,
                    dir: *dir,
                    ground_path: ground,
                    tint,
                    asset_server,
                    company,
                },
            );
        }
        TilePreviewKind::Rail {
            bits,
            tileh,
            rail_type,
        } => {
            if let Some(atlas) = atlas {
                spawn_rail_ghost_preview(
                    commands,
                    atlas,
                    RailGhostSpawn {
                        px,
                        py,
                        base_z,
                        half_h,
                        tileh: *tileh,
                        bits: *bits,
                        valid: tile_plan.valid,
                        rail_type: *rail_type,
                    },
                );
            }
        }
        TilePreviewKind::RoadDepot { dir } => {
            spawn_road_depot_preview(
                commands,
                RoadDepotPreviewSpawn {
                    px,
                    py,
                    base_z,
                    half_h,
                    dir: *dir,
                    tint,
                    asset_server,
                    company,
                },
            );
        }
        TilePreviewKind::RailDepot { dir } => {
            spawn_rail_depot_preview(
                commands,
                RailDepotPreviewSpawn {
                    px,
                    py,
                    base_z,
                    half_h,
                    dir: *dir,
                    tint,
                    asset_server,
                    company,
                },
            );
        }
        TilePreviewKind::Road { path } => {
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: asset_server.load::<Image>(path.clone()),
                    color: tint,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(px, py, base_z, 3.0, half_h))
                    .with_scale(Vec3::new(1.002, 1.002, 1.0)),
            ));
        }
        TilePreviewKind::Tunnel => {
            spawn_tunnel_entrance_preview(
                commands,
                asset_server,
                map,
                action,
                coord,
                tile_plan.valid,
            );
        }
        TilePreviewKind::GenericSprite => {
            if let Some(image) = preview_image_for_action(
                action,
                asset_server,
                &StationBuildState::default(),
                &[(px, py)],
                anim_cursor_frame,
            ) {
                commands.spawn((
                    BuildGhostPreview,
                    Sprite {
                        image,
                        color: tint,
                        ..default()
                    },
                    Transform::from_translation(tile_pos_half(px, py, base_z, 3.0, half_h))
                        .with_scale(Vec3::new(1.002, 1.002, 1.0)),
                ));
            }
        }
    }
}

struct RailGhostSpawn {
    px: i32,
    py: i32,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    bits: u8,
    valid: bool,
    rail_type: openttdrs_core::RailType,
}

/// Spawn overlay de vía ferroviaria.
fn spawn_rail_ghost_preview(commands: &mut Commands, atlas: &TileAtlas, spawn: RailGhostSpawn) {
    let mut ids = Vec::new();
    crate::sprites::collect_rail_ghost_sprites_for_type(
        spawn.bits,
        spawn.tileh,
        spawn.rail_type,
        &mut ids,
    );
    let center = tile_pos_half(spawn.px, spawn.py, spawn.base_z, 3.0, spawn.half_h);
    for (i, sid) in ids.iter().copied().enumerate() {
        let base_overlay = match sid {
            1087..=1092 => sid - crate::sprites::MONO_RAIL_SPRITE_OFFSET,
            1169..=1174 => sid - crate::sprites::MAGLEV_RAIL_SPRITE_OFFSET,
            other => other,
        };
        let is_overlay = (1005..=1010).contains(&base_overlay);
        let tint = if spawn.valid {
            if is_overlay {
                Color::srgba(2.2, 2.4, 2.8, 0.9)
            } else {
                Color::srgba(1.0, 1.02, 1.05, 0.38)
            }
        } else if is_overlay {
            Color::srgba(3.0, 0.5, 0.45, 0.9)
        } else {
            Color::srgba(1.0, 0.4, 0.35, 0.45)
        };
        let offset = crate::sprites::rail_ghost_overlay_offset(sid);
        let img = crate::sprites::rail_sprite_atlas_keys(sid)
            .into_iter()
            .find_map(|k| atlas.try_get(&k))
            .unwrap_or_else(|| atlas.get(&format!("rail_{sid}.png")));
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(center + Vec3::new(offset.x, offset.y, i as f32 * 0.001)),
        ));
    }
}
