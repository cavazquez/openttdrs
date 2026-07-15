use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileKind;

use crate::camera::zoom_display_magnification;
use crate::config::{self, json_save_hud_label, truncate_hud_line};
use crate::iso::{
    compute_tileh, shore_png_index, shore_tileh_for_draw_shore, slope_label,
    tile_slope_bits_from_heights,
};
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::sprites::{
    is_road_level_crossing, level_crossing_rail_sprite_id, rail_tile_is_signals,
    road_bits_for_render,
};
use crate::state::{SimRunState, SimWorld, sim_is_paused};

use super::{HudBuildFeedback, SelectedTileInfo, SimHudControls, TileInfoText};
use crate::settings::ClientPreferences;
use crate::ui::toolbar::StationBuildState;
use crate::ui::{BuildMenuAction, OrderEditState, UiToolState};

mod labels;
mod station_hud;

pub(crate) use labels::{tool_hud_hint, tool_hud_label};
pub(crate) use station_hud::{
    rail_depot_tile_details, road_depot_tile_details, station_details_text,
};

/// Alertas breves de vehículos para la tercera línea del HUD.
#[must_use]
pub(crate) fn vehicle_hud_alert_line(state: &openttdrs_core::GameState) -> String {
    let vehicles = &state.vehicles;
    let mut parts = Vec::new();
    let stuck_route = vehicles
        .iter()
        .filter(|v| v.running && v.no_network_route_to_order)
        .count();
    if stuck_route == 1 {
        if let Some(v) = vehicles
            .iter()
            .find(|v| v.running && v.no_network_route_to_order)
        {
            parts.push(format!(
                "sin ruta por red: vehículo {} (orden {})",
                v.id,
                v.current_order.saturating_add(1)
            ));
        }
    } else if stuck_route > 1 {
        parts.push(format!("sin ruta por red: {stuck_route} vehículos"));
    }

    let no_orders = vehicles
        .iter()
        .filter(|v| v.running && v.orders.is_empty())
        .count();
    if no_orders == 1 {
        if let Some(v) = vehicles.iter().find(|v| v.running && v.orders.is_empty()) {
            parts.push(format!("sin órdenes: vehículo {}", v.id));
        }
    } else if no_orders > 1 {
        parts.push(format!("sin órdenes: {no_orders} vehículos"));
    }

    let incompatible = vehicles
        .iter()
        .filter(|v| vehicle_has_incompatible_stop(state, v))
        .count();
    if incompatible == 1 {
        if let Some(v) = vehicles
            .iter()
            .find(|v| vehicle_has_incompatible_stop(state, v))
        {
            parts.push(format!("parada incompatible: vehículo {}", v.id));
        }
    } else if incompatible > 1 {
        parts.push(format!("parada incompatible: {incompatible} vehículos"));
    }

    let waiting_cargo = vehicles
        .iter()
        .filter(|v| vehicle_waiting_for_cargo(state, v))
        .count();
    if waiting_cargo == 1 {
        if let Some(v) = vehicles
            .iter()
            .find(|v| vehicle_waiting_for_cargo(state, v))
        {
            parts.push(format!("sin carga disponible: vehículo {}", v.id));
        }
    } else if waiting_cargo > 1 {
        parts.push(format!("sin carga disponible: {waiting_cargo} vehículos"));
    }

    let pbs_stuck = vehicles.iter().filter(|v| v.running && v.pbs_stuck).count();
    if pbs_stuck == 1 {
        if let Some(v) = vehicles.iter().find(|v| v.running && v.pbs_stuck) {
            parts.push(format!(
                "espera PBS: vehículo {} (orden {})",
                v.id,
                v.current_order.saturating_add(1)
            ));
        }
    } else if pbs_stuck > 1 {
        parts.push(format!("espera PBS: {pbs_stuck} vehículos"));
    }

    parts.join(" | ")
}

fn vehicle_has_incompatible_stop(
    state: &openttdrs_core::GameState,
    v: &openttdrs_core::Vehicle,
) -> bool {
    use openttdrs_core::VehicleOrder;

    if !v.running || v.orders.is_empty() {
        return false;
    }
    let Some(order) = v.orders.get(v.current_order) else {
        return false;
    };
    match order {
        VehicleOrder::Station { station, .. } => state
            .stations
            .iter()
            .find(|s| s.pos == *station)
            .is_some_and(|st| !st.can_service_vehicle(v.kind) || st.is_waypoint()),
        VehicleOrder::Waypoint { waypoint, .. } => state
            .stations
            .iter()
            .find(|s| s.pos == *waypoint)
            .is_none_or(|st| !st.can_service_vehicle(v.kind)),
        VehicleOrder::Depot { .. } | VehicleOrder::Tile(_) | VehicleOrder::Conditional { .. } => {
            false
        }
    }
}

fn vehicle_waiting_for_cargo(
    state: &openttdrs_core::GameState,
    v: &openttdrs_core::Vehicle,
) -> bool {
    use openttdrs_core::{STATION_COVERAGE_RADIUS, VehicleOrder, station_covers_tile};

    if !v.running || v.cargo > 0 || v.no_network_route_to_order || v.orders.is_empty() {
        return false;
    }
    let Some(VehicleOrder::Station { station, .. }) = v.orders.get(v.current_order).copied() else {
        return false;
    };
    if !station_covers_tile(station, v.pos, 1) && v.pos != station {
        return false;
    }
    let Some(st) = state.stations.iter().find(|s| s.pos == station) else {
        return false;
    };
    if !st.can_service_vehicle(v.kind) {
        return false;
    }
    let industry_has = state.industries.iter().any(|ind| {
        ind.stock > 0
            && openttdrs_core::industry_in_station_coverage(ind, station, STATION_COVERAGE_RADIUS)
            && st.accepts_cargo(ind.output_cargo())
    });
    let station_has = match v.kind {
        openttdrs_core::VehicleKind::Bus | openttdrs_core::VehicleKind::Tram => {
            st.cargo_stock.passengers > 0 || st.cargo_stock.mail > 0
        }
        openttdrs_core::VehicleKind::Truck
        | openttdrs_core::VehicleKind::Train
        | openttdrs_core::VehicleKind::Ship => {
            st.stock > 0 || st.cargo_stock.pick_freight_to_load(v.cargo_type).is_some()
        }
        openttdrs_core::VehicleKind::Aircraft => false,
    };
    !industry_has && !station_has
}

/// Crea el texto de informacion del tile.
pub(crate) fn setup_tile_info_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing_font: Option<Res<crate::ui::font::HudUiFont>>,
) {
    let font = if let Some(f) = existing_font {
        f.0.clone()
    } else {
        crate::ui::font::load_hud_ui_font(&asset_server, &mut commands)
    };
    commands.spawn((
        TileInfoText,
        Text2d::new(
            "Mapa: clic selecciona tile · Esc menú (sin herramienta) · Depósito ≠ parada (carga)",
        ),
        crate::ui::font::ui_text_font(font, crate::ui::font::UiFontRole::Body),
        TextColor(Color::srgb(0.96, 0.94, 0.82)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Campos que afectan el contenido del HUD de tesela (no la posición en pantalla).
#[derive(PartialEq, Clone)]
pub(crate) struct TileInfoHudKey {
    selected: Option<(i32, i32)>,
    tick: u64,
    cam_scale_bits: u32,
    paused: bool,
    sim_speed_bits: u32,
    minimap: bool,
    tool: Option<BuildMenuAction>,
    order_vehicle: Option<u32>,
    order_len: usize,
    feedback: Option<String>,
    money: i64,
    loan: i64,
    cargo_income: u64,
    vehicle_costs: u64,
    cargo_delivered: u64,
    cargo_loaded: u64,
    deliveries: u64,
    pickups: u64,
    industry_prod: u64,
    veh_n: usize,
    veh_running: usize,
    st_n: usize,
    save_path: String,
    tile_mapt: Option<u8>,
    tile_m5: Option<u8>,
    tile_m1: Option<u8>,
    tile_m2: Option<u8>,
    tile_m3: Option<u8>,
    tile_m3hi: Option<u8>,
    tile_m7: Option<u8>,
    tile_kind: Option<TileKind>,
    tile_height: Option<u8>,
    vehicle_alert: String,
    pbs_overlay: bool,
    catenary_mode: crate::sprites::TransparencyMode,
    signal_type: u8,
    signal_density: u8,
}

/// Actualiza el texto de informacion del tile seleccionado.
#[allow(clippy::too_many_arguments)] // firma dictada por el sistema ECS de Bevy
pub(crate) fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    prefs: Res<ClientPreferences>,
    station_state: Res<StationBuildState>,
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    tool_state: Res<UiToolState>,
    order_state: Res<OrderEditState>,
    run_state: Res<State<SimRunState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut text_q: Query<
        (&mut Text2d, &mut Transform),
        (With<TileInfoText>, Without<PrimaryGameCamera>),
    >,
    mut cache: Local<Option<TileInfoHudKey>>,
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
    // Deja espacio para barra/toolbar superior (varias líneas de estado).
    text_transform.translation.y = cam_transform.translation.y + half_h - 88.0 * proj.scale;
    text_transform.scale = Vec3::splat(proj.scale);

    if feedback.message.is_some() && time.elapsed_secs() >= feedback.expires_at_secs {
        feedback.message = None;
    }

    let vehicle_alert = vehicle_hud_alert_line(&sim.state);
    let tile_snapshot = selected.pos.and_then(|pos| {
        sim.state.map.get(pos).map(|tile| {
            (
                tile.mapt,
                tile.m5,
                tile.m1,
                tile.m2,
                tile.m3,
                tile.m3hi,
                tile.m7,
                tile.kind,
                tile.height,
            )
        })
    });
    let key = TileInfoHudKey {
        selected: selected.pos.map(|p| (p.x, p.y)),
        tick: sim.state.tick.get(),
        cam_scale_bits: proj.scale.to_bits(),
        paused: sim_is_paused(&run_state),
        sim_speed_bits: hud.sim_speed.to_bits(),
        minimap: hud.minimap_visible,
        tool: tool_state.active_tool,
        order_vehicle: order_state.vehicle_id,
        order_len: order_state.orders.len(),
        feedback: feedback.message.clone(),
        money: sim.state.economy.money,
        loan: sim.state.economy.loan,
        cargo_income: sim.state.stats.cargo_income_earned,
        vehicle_costs: sim.state.stats.vehicle_running_costs,
        cargo_delivered: sim.state.stats.cargo_units_delivered,
        cargo_loaded: sim.state.stats.cargo_units_loaded,
        deliveries: sim.state.stats.cargo_deliveries,
        pickups: sim.state.stats.cargo_pickups,
        industry_prod: sim.state.stats.industry_cargo_units_produced,
        veh_n: sim.state.vehicles.len(),
        veh_running: sim.state.vehicles.iter().filter(|v| v.running).count(),
        st_n: sim.state.stations.len(),
        save_path: hud.json_save_path.clone(),
        tile_mapt: tile_snapshot.map(|t| t.0),
        tile_m5: tile_snapshot.map(|t| t.1),
        tile_m1: tile_snapshot.map(|t| t.2),
        tile_m2: tile_snapshot.map(|t| t.3),
        tile_m3: tile_snapshot.map(|t| t.4),
        tile_m3hi: tile_snapshot.map(|t| t.5),
        tile_m7: tile_snapshot.map(|t| t.6),
        tile_kind: tile_snapshot.map(|t| t.7),
        tile_height: tile_snapshot.map(|t| t.8),
        vehicle_alert: vehicle_alert.clone(),
        pbs_overlay: prefs.show_pbs_reservations,
        catenary_mode: prefs.transparency_mode(crate::sprites::TransparencyOption::Catenary),
        signal_type: station_state.signal_type,
        signal_density: station_state.signal_density,
    };
    if cache.as_ref() == Some(&key) {
        return;
    }
    *cache = Some(key);

    let zoom_label = format!("Zoom {:.2}×", zoom_display_magnification(proj.scale));
    let pause_l = if sim_is_paused(&run_state) {
        "Pausa ON"
    } else {
        "Pausa OFF"
    };
    let speed_l = format!("Velocidad {:.0}x", hud.sim_speed);
    let tool_l = tool_state
        .active_tool
        .map(tool_hud_label)
        .unwrap_or("Ninguna");
    let tool_hint = tool_state.active_tool.and_then(tool_hud_hint);
    let order_l = order_state
        .vehicle_id
        .map(|id| format!(" | ordenes veh #{id}:{}", order_state.orders.len()))
        .unwrap_or_default();
    let minimap_l = if hud.minimap_visible {
        "mapa M:on"
    } else {
        "mapa M:off"
    };
    let pbs_l = if prefs.show_pbs_reservations {
        "PBS R:on"
    } else {
        "PBS R:off"
    };
    let catenary_l = match prefs.transparency_mode(crate::sprites::TransparencyOption::Catenary) {
        crate::sprites::TransparencyMode::Hidden => "cat:oculta",
        crate::sprites::TransparencyMode::Transparent => "cat:transp.",
        crate::sprites::TransparencyMode::Visible => "cat:visible",
    };
    let tick_n = sim.state.tick.get();

    let feedback_append = feedback.message.as_ref().map(|m| {
        let t = truncate_hud_line(m, 44);
        format!(" | {t}")
    });

    let veh_n = sim.state.vehicles.len();
    let veh_running = sim.state.vehicles.iter().filter(|v| v.running).count();
    let st_n = sim.state.stations.len();
    let stats = &sim.state.stats;
    let save_file = truncate_hud_line(&json_save_hud_label(&hud.json_save_path), 36);
    // Text2d no hace wrap: repartir el estado en líneas cortas evita recorte al borde derecho.
    let hud_line1 = format!("{pause_l} | {speed_l} | t{tick_n}");
    let hud_line2 = format!(
        "ingresos ${} · gastos veh ${} | u {}/{} · evt {}/{} · prod {} | veh {} ({}) | est {st_n}",
        stats.cargo_income_earned,
        stats.vehicle_running_costs,
        stats.cargo_units_delivered,
        stats.cargo_units_loaded,
        stats.cargo_deliveries,
        stats.cargo_pickups,
        stats.industry_cargo_units_produced,
        veh_n,
        veh_running,
    );
    let mut hud_lines = vec![hud_line1, hud_line2];
    if !vehicle_alert.is_empty() || feedback_append.is_some() {
        let mut alert = vehicle_alert;
        if let Some(ref fb) = feedback_append {
            if !alert.is_empty() {
                alert.push_str(" | ");
            }
            alert.push_str(fb.trim_start_matches(" | "));
        }
        hud_lines.push(truncate_hud_line(&alert, 72));
    }
    let signal_tool_extra = if tool_state.active_tool == Some(BuildMenuAction::RailSignals) {
        format!(
            " tipo:{} dens:{}",
            openttdrs_core::signal_type_label(station_state.signal_type),
            station_state.signal_density
        )
    } else {
        String::new()
    };
    let rail_type_extra = if tool_state.active_tool.is_some_and(|a| {
        matches!(
            a,
            BuildMenuAction::Rail
                | BuildMenuAction::RailX
                | BuildMenuAction::RailY
                | BuildMenuAction::RailHorz
                | BuildMenuAction::RailVert
                | BuildMenuAction::RailDepot
                | BuildMenuAction::RailStation
                | BuildMenuAction::RailBridge
                | BuildMenuAction::RailTunnel
                | BuildMenuAction::RailWaypoint
                | BuildMenuAction::RailConvert
        )
    }) {
        format!(" vía:{}", sim.state.current_rail_type.label())
    } else {
        String::new()
    };
    let road_type_extra = if tool_state.active_tool.is_some_and(|a| {
        matches!(
            a,
            BuildMenuAction::Road
                | BuildMenuAction::RoadX
                | BuildMenuAction::RoadY
                | BuildMenuAction::RoadDepot
                | BuildMenuAction::RoadBridge
                | BuildMenuAction::RoadTunnel
                | BuildMenuAction::Station
                | BuildMenuAction::BusStop
        )
    }) {
        format!(
            " ctra:{}",
            openttdrs_core::road_type_def(
                &sim.state.road_type_catalog,
                sim.state.current_road_type
            )
            .map(|d| d.short_label.as_str())
            .unwrap_or(sim.state.current_road_type.short_label())
        )
    } else if tool_state.active_tool.is_some_and(|a| {
        matches!(
            a,
            BuildMenuAction::Tram | BuildMenuAction::TramX | BuildMenuAction::TramY
        )
    }) {
        format!(
            " tram:{}",
            openttdrs_core::road_type_def(
                &sim.state.road_type_catalog,
                sim.state.current_tram_type
            )
            .map(|d| d.short_label.as_str())
            .unwrap_or(sim.state.current_tram_type.short_label())
        )
    } else {
        String::new()
    };
    let join_extra = if tool_state.active_tool == Some(BuildMenuAction::JoinStation) {
        match station_state.join_keep {
            Some(p) => format!(" keep:({},{})", p.x, p.y),
            None => " elige 1ª estación".into(),
        }
    } else {
        String::new()
    };
    hud_lines.push(format!(
        "Herramienta: {tool_l}{}{}{signal_tool_extra}{rail_type_extra}{road_type_extra}{join_extra} | {minimap_l} | {pbs_l} | {catenary_l} | {save_file} · F4",
        tool_hint.map_or(String::new(), |h| format!(" ({h})")),
        order_l,
    ));
    let hud_status = hud_lines.join("\n");

    let Some(pos) = selected.pos else {
        **text = format!(
            "{zoom_label}\n{hud_status}\nClic mapa: tile · depósito: comprar vehículo · parada: carga"
        );
        return;
    };

    let Some(tile) = sim.state.map.get(pos) else {
        **text = format!(
            "{zoom_label}\n{hud_status}\n({}, {}): fuera del mapa",
            pos.x, pos.y
        );
        return;
    };

    let kind_str = match tile.kind {
        TileKind::Void => "Void",
        TileKind::Grass => "Grass",
        TileKind::Water => {
            use openttdrs_core::{WaterClass, water_class_from_m1};
            match water_class_from_m1(tile.m1) {
                WaterClass::Canal => "Canal",
                WaterClass::River => "Río",
                WaterClass::Sea => "Mar",
                WaterClass::Invalid => "Water",
            }
        }
        TileKind::Road => "Road",
        TileKind::Rail => "Rail",
        TileKind::RoadDepot => "Depósito carretera",
        TileKind::RailDepot => "Depósito vía",
        TileKind::RoadTunnel => "RoadTunnel",
        TileKind::RailTunnel => "RailTunnel",
        TileKind::RoadBridge => "RoadBridge",
        TileKind::RailBridge => "RailBridge",
        TileKind::House => "House",
        TileKind::Industry => "Industry",
        TileKind::Station => "Station",
        TileKind::Forest => "Forest",
        TileKind::CoalField => "CoalField",
        TileKind::ShipDepot => "Depósito barcos",
        TileKind::Airport => "Aeropuerto",
        TileKind::Unknown(n) => {
            **text = format!(
                "{zoom_label}\n{hud_status}\n({}, {}): Unknown({})",
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
        let clean = openttdrs_core::get_clean_industry_gfx(tile.m5, tile.m6);
        let gfx9 = openttdrs_core::get_translated_industry_tile_id(
            clean,
            &sim.state.industry_tile_overrides,
        );
        let newgrf_ok = gfx9 >= openttdrs_core::NEW_INDUSTRY_TILE_OFFSET
            && openttdrs_core::industry_tile_spec_def(&sim.state.industry_tile_spec_catalog, gfx9)
                .is_some_and(openttdrs_core::IndustryTileSpecDef::has_newgrf_sprites);
        let status = if newgrf_ok {
            crate::sprites::IndustryGfxStatus::Resolved
        } else {
            crate::sprites::industry_gfx_status(gfx9)
        };
        let flag = if status == crate::sprites::IndustryGfxStatus::Resolved {
            String::new()
        } else {
            format!(" ⚠{}", crate::sprites::industry_gfx_status_label(status))
        };
        let stage = if crate::sprites::industry_tile_anim_state(gfx9) {
            format!(
                "frame:{}",
                crate::sprites::industry_animation_frame_from_m4(tile.m3hi)
            )
        } else {
            format!(
                "stage:{}",
                crate::sprites::industry_construction_stage_from_tile(tile.m1)
            )
        };
        format!(
            " gfx9:{gfx9}{flag} {stage} m1:0x{:02X} m2:{} m3hi:0x{:02X}",
            tile.m1, tile.m2, tile.m3hi
        )
    } else if tile.kind == TileKind::Station {
        station_details_text(&sim, pos, &tile)
    } else if tile.kind == TileKind::RoadDepot {
        road_depot_tile_details(tile.m5)
    } else if tile.kind == TileKind::RailDepot {
        rail_depot_tile_details(tile.m5)
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
                "\nveh #{} {:?} {} cargo:{:?} {}/{} dest:({}, {}) orders:{}",
                vehicle.id,
                vehicle.kind,
                if vehicle.running { "RUN" } else { "STOP" },
                vehicle.cargo_type,
                vehicle.cargo,
                vehicle.capacity,
                vehicle.dest.x,
                vehicle.dest.y,
                vehicle.orders.len()
            )
        })
        .unwrap_or_default();

    **text = format!(
        "{zoom_label}\n{hud_status}\nTile ({},{}) {}\nh:{} slope:{} ({}) mapt:0x{:02X} m5:0x{:02X} m1:0x{:02X} m2:0x{:02X} m7:0x{:02X} m3:0x{:02X} m3hi:0x{:02X}{}{}{}",
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        TileInfoText, setup_tile_info_ui, station_details_text, update_tile_info_text,
        vehicle_hud_alert_line,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::sprite::Anchor;
    use bevy::window::{PrimaryWindow, WindowResolution};
    use openttdrs_core::Vehicle;
    use openttdrs_core::{
        GameState, Industry, IndustryKind, Station, StopKind, Tile, TileCoord, TileKind,
    };

    use crate::render::PrimaryGameCamera;
    use crate::settings::ClientPreferences;
    use crate::state::SimWorld;
    use crate::ui::hud::{HudBuildFeedback, SelectedTileInfo, SimHudControls};
    use crate::ui::toolbar::StationBuildState;
    use crate::ui::{OrderEditState, UiToolState};

    #[test]
    fn setup_tile_info_ui_spawns_text() {
        use std::fs;
        use std::path::PathBuf;

        let dir = tempfile::tempdir().expect("tempdir");
        let font_dst = dir.path().join("static/fonts/DejaVuSansMono.ttf");
        fs::create_dir_all(font_dst.parent().expect("parent")).expect("mkdir");
        let font_src =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../static/fonts/DejaVuSansMono.ttf");
        if font_src.exists() {
            fs::copy(&font_src, &font_dst).expect("copy font");
        } else {
            fs::write(&font_dst, []).expect("touch font");
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin {
            file_path: dir.path().to_str().expect("utf8").into(),
            ..default()
        });
        app.init_asset::<Font>();
        app.update();
        app.world_mut().run_system_once(setup_tile_info_ui).unwrap();
    }

    #[test]
    fn update_tile_info_text_without_ui_query_returns_early() {
        let mut world = World::new();
        world.insert_resource(SelectedTileInfo::default());
        world.insert_resource(SimWorld::default());
        world.insert_resource(SimHudControls::default());
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(StationBuildState::default());
        world.insert_resource(HudBuildFeedback::default());
        world.insert_resource(Time::<()>::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(OrderEditState::default());
        crate::state::insert_test_sim_run_state(&mut world);
        world.run_system_once(update_tile_info_text).unwrap();
    }

    #[test]
    fn update_tile_info_text_covers_road_rail_station_and_unknown() {
        let mut world = World::new();
        world.insert_resource(SelectedTileInfo {
            pos: Some(TileCoord::new(1, 1)),
        });
        world.insert_resource(SimHudControls::default());
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(StationBuildState::default());
        world.insert_resource(HudBuildFeedback::default());
        world.insert_resource(Time::<()>::default());
        world.insert_resource(UiToolState::default());
        world.insert_resource(OrderEditState::default());
        crate::state::insert_test_sim_run_state(&mut world);

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
            let mut station = Station::new(c);
            station.stock = 12;
            station.income = 144;
            sim.state.stations.push(station);
        }
        world.run_system_once(update_tile_info_text).unwrap();
        let station_text = hud_text(&mut world);
        assert!(station_text.contains("stock:12 ingresos:$144"));
        assert!(station_text.contains("estación tren"));

        // Depósito carretera: etiqueta legible + aviso de no-parada.
        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state
                .map
                .set_tile(
                    c,
                    Tile {
                        kind: TileKind::RoadDepot,
                        mapt: 0x20,
                        m5: 0x82,
                        ..tile_template()
                    },
                )
                .unwrap();
        }
        world.run_system_once(update_tile_info_text).unwrap();
        let depot_text = hud_text(&mut world);
        assert!(depot_text.contains("Depósito carretera"));
        assert!(depot_text.contains("No es parada"));

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
    fn vehicle_hud_alert_line_reports_route_and_missing_orders() {
        let origin = TileCoord::new(0, 0);
        let mut v1 = Vehicle::new(1, openttdrs_core::VehicleKind::Bus, origin, origin);
        v1.running = true;
        v1.set_orders(vec![TileCoord::new(1, 0)]);
        v1.no_network_route_to_order = true;
        let mut v2 = Vehicle::new(2, openttdrs_core::VehicleKind::Truck, origin, origin);
        v2.running = true;
        let mut state = GameState::new(4, 4);
        state.vehicles = vec![v1, v2];
        let alert = vehicle_hud_alert_line(&state);
        assert!(alert.contains("sin ruta por red: vehículo 1"));
        assert!(alert.contains("sin órdenes: vehículo 2"));
    }

    #[test]
    fn vehicle_hud_alert_line_reports_incompatible_stop() {
        let stop = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        state
            .stations
            .push(Station::new_with_kind(stop, StopKind::BusStop));
        let mut truck = Vehicle::new(3, openttdrs_core::VehicleKind::Truck, stop, stop);
        truck.running = true;
        truck.set_station_orders(vec![stop]);
        state.vehicles.push(truck);

        let alert = vehicle_hud_alert_line(&state);
        assert!(alert.contains("parada incompatible: vehículo 3"));
    }

    #[test]
    fn vehicle_hud_alert_line_reports_empty_cargo_at_load_stop() {
        let stop = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        state
            .stations
            .push(Station::new_with_kind(stop, StopKind::TruckStop));
        state
            .industries
            .push(Industry::new(TileCoord::new(2, 4), IndustryKind::CoalMine));
        let mut truck = Vehicle::new(4, openttdrs_core::VehicleKind::Truck, stop, stop);
        truck.running = true;
        truck.set_station_orders(vec![stop]);
        state.vehicles.push(truck);

        let alert = vehicle_hud_alert_line(&state);
        assert!(alert.contains("sin carga disponible: vehículo 4"));
    }

    #[test]
    fn vehicle_hud_alert_line_reports_pbs_stuck() {
        let origin = TileCoord::new(0, 0);
        let mut train = Vehicle::new(5, openttdrs_core::VehicleKind::Train, origin, origin);
        train.running = true;
        train.pbs_stuck = true;
        train.set_orders(vec![TileCoord::new(3, 0)]);
        let mut state = GameState::new(4, 4);
        state.vehicles = vec![train];
        let alert = vehicle_hud_alert_line(&state);
        assert!(alert.contains("espera PBS: vehículo 5"));
    }

    #[test]
    fn station_details_text_includes_stock_income_and_sources() {
        let sim = SimWorld {
            state: GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let mut sim = sim;
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
        let mut station = Station::new(station_pos);
        station.stock = 7;
        station.income = 84;
        sim.state.stations.push(station);
        sim.state.industries.push(Industry {
            pos: industry_pos,
            tiles: vec![industry_pos],
            spec: None,
            kind: IndustryKind::CoalMine,
            stock: 42,
            capacity: 100,
            random_colour: 0,
            ..Default::default()
        });

        let tile = sim.state.map.get(station_pos).unwrap();
        let text = station_details_text(&sim, station_pos, &tile);
        assert!(text.contains("stock:7 ingresos:$84"));
        assert!(text.contains("coal:"));
        assert!(text.contains("source stock:42"));
    }
}
