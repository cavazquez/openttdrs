use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use openttdrs_core::{STATION_COVERAGE_RADIUS, TileKind, station_coverage_at};

use crate::config;
use crate::iso::{
    compute_tileh, shore_png_index, shore_tileh_for_draw_shore, slope_label,
    tile_slope_bits_from_heights,
};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::sprites::{
    is_road_level_crossing, level_crossing_rail_sprite_id, rail_tile_is_signals,
    road_bits_for_render,
};
use crate::state::SimWorld;

use super::{SelectedTileInfo, SimHudControls, TileInfoText};
use crate::ui::{BuildMenuAction, OrderEditState, UiToolState};

/// Crea el texto de informacion del tile.
pub(crate) fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new("Mapa: clic para seleccionar · Herramientas: 1/2/3/C · Esc cancela"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.96, 0.94, 0.82)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Actualiza el texto de informacion del tile seleccionado.
#[allow(clippy::too_many_arguments)] // firma dictada por el sistema ECS de Bevy
pub(crate) fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    tool_state: Res<UiToolState>,
    order_state: Res<OrderEditState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    mut text_q: Query<
        (&mut Text2d, &mut Transform),
        (With<TileInfoText>, Without<PrimaryGameCamera>),
    >,
) {
    let Ok((mut text, mut text_transform)) = text_q.single_mut() else {
        return;
    };
    let Ok((cam_transform, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    let half_w = window.width() / 2.0 * proj.scale;
    let half_h = window.height() / 2.0 * proj.scale;
    text_transform.translation.x = cam_transform.translation.x - half_w + 14.0 * proj.scale;
    // Deja espacio para barra/toolbar superior.
    text_transform.translation.y = cam_transform.translation.y + half_h - 74.0 * proj.scale;
    text_transform.scale = Vec3::splat(proj.scale);

    let zoom_label = format!("Zoom {:.2}×", proj.scale);
    let pause_l = if hud.paused { "Pausa ON" } else { "Pausa OFF" };
    let speed_l = format!("Velocidad {:.0}x", hud.sim_speed);
    let tool_l = match tool_state.active_tool {
        Some(BuildMenuAction::Road) => "Road+",
        Some(BuildMenuAction::RoadX) => "Road NE-SW",
        Some(BuildMenuAction::RoadY) => "Road NW-SE",
        Some(BuildMenuAction::RoadDepot) => "Road depot",
        Some(BuildMenuAction::RoadBridge) => "Road bridge",
        Some(BuildMenuAction::RoadTunnel) => "Road tunnel",
        Some(BuildMenuAction::Rail) => "Rail",
        Some(BuildMenuAction::RailDepot) => "Rail depot",
        Some(BuildMenuAction::RailBridge) => "Rail bridge",
        Some(BuildMenuAction::RailTunnel) => "Rail tunnel",
        Some(BuildMenuAction::Station) => "Station",
        Some(BuildMenuAction::Clear) => "Clear",
        Some(BuildMenuAction::Orders) => "Orders",
        Some(BuildMenuAction::BuildHouse) => "Build house",
        Some(BuildMenuAction::BuildCoalMine) => "Build coal mine",
        Some(BuildMenuAction::BuildIronOreMine) => "Build iron mine",
        Some(BuildMenuAction::BuildGoldMine) => "Build gold mine",
        Some(BuildMenuAction::BuildOilWell) => "Build oil well",
        Some(BuildMenuAction::BuildOilRefinery) => "Build oil refinery",
        Some(BuildMenuAction::BuildFactory) => "Build factory",
        Some(BuildMenuAction::BuildSawmill) => "Build sawmill",
        Some(BuildMenuAction::BuildForest) => "Build forest",
        Some(BuildMenuAction::BuildFarm) => "Build farm",
        None => "None",
    };
    let order_l = order_state
        .vehicle_id
        .map(|id| format!(" | ordenes veh #{id}:{}", order_state.orders.len()))
        .unwrap_or_default();
    let minimap_l = if hud.minimap_visible {
        "mapa M:on"
    } else {
        "mapa M:off"
    };
    let hud_footer = format!(
        "{pause_l} | {speed_l} | Tool: {tool_l}{order_l} | ${} | cargas {}/{} | {minimap_l} | JSON: {} | F4 ruta",
        sim.state.economy.money,
        sim.state.stats.cargo_units_delivered,
        sim.state.stats.cargo_units_loaded,
        hud.json_save_path
    );

    let Some(pos) = selected.pos else {
        **text = format!("{zoom_label}\n{hud_footer}\nClic mapa: elegir tile · tools 1/2/3/C/Esc");
        return;
    };

    let Some(tile) = sim.state.map.get(pos) else {
        **text = format!(
            "{zoom_label}\n{hud_footer}\n({}, {}): fuera del mapa",
            pos.x, pos.y
        );
        return;
    };

    let kind_str = match tile.kind {
        TileKind::Void => "Void",
        TileKind::Grass => "Grass",
        TileKind::Water => "Water",
        TileKind::Road => "Road",
        TileKind::Rail => "Rail",
        TileKind::RoadDepot => "RoadDepot",
        TileKind::RailDepot => "RailDepot",
        TileKind::RoadTunnel => "RoadTunnel",
        TileKind::RailTunnel => "RailTunnel",
        TileKind::RoadBridge => "RoadBridge",
        TileKind::RailBridge => "RailBridge",
        TileKind::House => "House",
        TileKind::Industry => "Industry",
        TileKind::Station => "Station",
        TileKind::Forest => "Forest",
        TileKind::CoalField => "CoalField",
        TileKind::Unknown(n) => {
            **text = format!(
                "{zoom_label}\n{hud_footer}\n({}, {}): Unknown({})",
                pos.x, pos.y, n
            );
            return;
        }
    };

    let extra = if tile.kind == TileKind::Road {
        let rb = road_bits_for_render(
            &sim.state.map,
            pos,
            sim.state.map.dimensions().0,
            sim.state.map.dimensions().1,
        );
        let mut s = format!(" rb:0x{rb:02X}");
        if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
            s.push_str(&format!(
                " Xing rail:{}",
                level_crossing_rail_sprite_id(tile.m5)
            ));
        }
        s
    } else if tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
        format!(
            " signals present:0x{:X} m2:0x{:02X}",
            (tile.m3 >> 4) & 0xF,
            tile.m2
        )
    } else if tile.kind == TileKind::Industry {
        // OpenTTD GetCleanIndustryGfx: 9 bits — no confundir con `m5` solo (HUD antes mostraba eso como "gfx").
        let gfx9 = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
        format!(" gfx9:{} m6:0x{:02X} ind:{}", gfx9, tile.m6, tile.m1 & 0x7F)
    } else if tile.kind == TileKind::Station {
        station_details_text(&sim, pos)
    } else {
        String::new()
    };

    let mw = sim.state.map.dimensions().0;
    let mh = sim.state.map.dimensions().1;
    let tileh = if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < mw && (pos.y as u32) < mh {
        compute_tileh(&sim.state.map, pos.x as u32, pos.y as u32)
    } else {
        0
    };
    let slope_str = slope_label(tileh);
    let coast_dbg = if config::env_flag("OPENTTDRS_DEBUG_COAST")
        && tile.kind == TileKind::Water
        && pos.x >= 0
        && pos.y >= 0
    {
        let ux = pos.x as u32;
        let uy = pos.y as u32;
        let (mw, mh) = sim.state.map.dimensions();
        let (raw, _) = tile_slope_bits_from_heights(&sim.state.map, ux, uy);
        let th = shore_tileh_for_draw_shore(&sim.state.map, ux, uy, mw, mh);
        let si = shore_png_index(th);
        format!("\ncoast dbg raw:{raw} th:{th} si:{si}")
    } else {
        String::new()
    };
    let vehicle_dbg = sim
        .state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.pos == pos)
        .map(|vehicle| {
            format!(
                "\nveh #{} {:?} cargo:{}/{} dest:({}, {}) orders:{}",
                vehicle.id,
                vehicle.kind,
                vehicle.cargo,
                vehicle.capacity,
                vehicle.dest.x,
                vehicle.dest.y,
                vehicle.orders.len()
            )
        })
        .unwrap_or_default();

    **text = format!(
        "{zoom_label}\n{hud_footer}\nTile ({},{}) {}\nh:{} slope:{} ({}) mapt:0x{:02X} m5:0x{:02X} m1:0x{:02X} m2:0x{:02X} m7:0x{:02X} m3:0x{:02X} m3hi:0x{:02X}{}{}{}",
        pos.x,
        pos.y,
        kind_str,
        tile.height,
        tileh,
        slope_str,
        tile.mapt,
        tile.m5,
        tile.m1,
        tile.m2,
        tile.m7,
        tile.m3,
        tile.m3hi,
        extra,
        coast_dbg,
        vehicle_dbg
    );
}

fn station_details_text(sim: &SimWorld, pos: openttdrs_core::TileCoord) -> String {
    let coverage = station_coverage_at(
        &sim.state.map,
        &sim.state.industries,
        pos,
        STATION_COVERAGE_RADIUS,
    );
    let station_line = sim
        .state
        .stations
        .iter()
        .find(|station| station.pos == pos)
        .map(|station| format!("stock:{} income:{}", station.stock, station.income))
        .unwrap_or_else(|| "stock:n/d income:n/d".to_string());
    format!(
        "\nStation {station_line}\nCoverage r{} accepts mail:{} goods:{}\nSupplies coal:{} wood:{} oil:{} source stock:{}",
        STATION_COVERAGE_RADIUS,
        coverage.accepts_mail,
        coverage.accepts_goods,
        coverage.supplies_coal,
        coverage.supplies_wood,
        coverage.supplies_oil,
        coverage.supplied_stock
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{TileInfoText, setup_tile_info_ui, station_details_text, update_tile_info_text};
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::sprite::Anchor;
    use bevy::window::{PrimaryWindow, WindowResolution};
    use openttdrs_core::{Industry, IndustryKind, Station, Tile, TileCoord, TileKind};

    use crate::render::PrimaryGameCamera;
    use crate::state::SimWorld;
    use crate::ui::hud::{SelectedTileInfo, SimHudControls};
    use crate::ui::{OrderEditState, UiToolState};

    #[test]
    fn setup_tile_info_ui_spawns_text() {
        let mut world = World::new();
        world.run_system_once(setup_tile_info_ui).unwrap();
    }

    #[test]
    fn update_tile_info_text_without_ui_query_returns_early() {
        let mut world = World::new();
        world.insert_resource(SelectedTileInfo::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(OrderEditState::default());
        world.run_system_once(update_tile_info_text).unwrap();
    }

    #[test]
    fn update_tile_info_text_covers_road_rail_station_and_unknown() {
        let mut world = World::new();
        world.insert_resource(SelectedTileInfo {
            pos: Some(TileCoord::new(1, 1)),
        });
        world.insert_resource(SimHudControls::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(OrderEditState::default());

        world.spawn((
            Window {
                resolution: WindowResolution::new(1280, 720),
                ..default()
            },
            PrimaryWindow,
        ));
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world.spawn((
            TileInfoText,
            Text2d::new(""),
            Transform::default(),
            Anchor::TOP_LEFT,
        ));

        let mut sim = SimWorld::default();
        let c = TileCoord::new(1, 1);

        // Road crossing path.
        sim.state
            .map
            .set_tile(
                c,
                Tile {
                    kind: TileKind::Road,
                    mapt: 0x20,
                    m5: 0x40,
                    ..tile_template()
                },
            )
            .unwrap();
        world.insert_resource(sim);
        world.run_system_once(update_tile_info_text).unwrap();
        assert!(hud_text(&mut world).contains("Road"));

        // Rail signals path.
        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state
                .map
                .set_tile(
                    c,
                    Tile {
                        kind: TileKind::Rail,
                        m5: 0x40 | 0x03,
                        m3: 0xA0,
                        m2: 0x2F,
                        ..tile_template()
                    },
                )
                .unwrap();
        }
        world.run_system_once(update_tile_info_text).unwrap();
        assert!(hud_text(&mut world).contains("signals present"));

        // Station coverage path.
        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state
                .map
                .set_tile(
                    c,
                    Tile {
                        kind: TileKind::Station,
                        ..tile_template()
                    },
                )
                .unwrap();
            sim.state.stations.push(Station {
                pos: c,
                stock: 12,
                income: 144,
            });
        }
        world.run_system_once(update_tile_info_text).unwrap();
        let station_text = hud_text(&mut world);
        assert!(station_text.contains("Station stock:12 income:144"));
        assert!(station_text.contains("Coverage r"));

        // Unknown kind early-return path.
        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state
                .map
                .set_tile(
                    c,
                    Tile {
                        kind: TileKind::Unknown(9),
                        ..tile_template()
                    },
                )
                .unwrap();
        }
        world.run_system_once(update_tile_info_text).unwrap();
        assert!(hud_text(&mut world).contains("Unknown(9)"));
    }

    fn tile_template() -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: 0,
            m5: 0,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    fn hud_text(world: &mut World) -> String {
        let mut q = world.query_filtered::<&Text2d, With<TileInfoText>>();
        q.single(world).unwrap().to_string()
    }

    #[test]
    fn station_details_text_includes_stock_income_and_sources() {
        let mut sim = SimWorld::default();
        let station_pos = TileCoord::new(2, 2);
        let industry_pos = TileCoord::new(3, 2);
        sim.state
            .map
            .set_kind(station_pos, TileKind::Station)
            .unwrap();
        sim.state
            .map
            .set_kind(industry_pos, TileKind::Industry)
            .unwrap();
        sim.state.stations.push(Station {
            pos: station_pos,
            stock: 7,
            income: 84,
        });
        sim.state.industries.push(Industry {
            pos: industry_pos,
            tiles: vec![industry_pos],
            spec: None,
            kind: IndustryKind::CoalMine,
            stock: 42,
            capacity: 100,
        });

        let text = station_details_text(&sim, station_pos);
        assert!(text.contains("stock:7 income:84"));
        assert!(text.contains("coal:"));
        assert!(text.contains("source stock:42"));
    }
}
