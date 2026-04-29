use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileKind;

use crate::config;
use crate::iso::{
    compute_tileh, shore_png_index, shore_tileh_for_draw_shore, slope_label,
    tile_slope_bits_from_heights,
};
use crate::sprites::{
    is_road_level_crossing, level_crossing_rail_sprite_id, rail_tile_is_signals,
    road_bits_for_render,
};
use crate::state::SimWorld;

use super::{SelectedTileInfo, SimHudControls, TileInfoText};
use crate::ui::{BuildMenuAction, UiToolState};

/// Crea el texto de informacion del tile.
pub(crate) fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new("Clic en mapa: seleccionar tile · Toolbar: 1/2/3/C para tool, Esc cancela"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.8)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Actualiza el texto de informacion del tile seleccionado.
#[allow(clippy::type_complexity)]
pub(crate) fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    tool_state: Res<UiToolState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), With<Camera2d>>,
    mut text_q: Query<(&mut Text2d, &mut Transform), (With<TileInfoText>, Without<Camera2d>)>,
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
    text_transform.translation.x = cam_transform.translation.x - half_w + 10.0 * proj.scale;
    text_transform.translation.y = cam_transform.translation.y + half_h - 10.0 * proj.scale;
    text_transform.scale = Vec3::splat(proj.scale);

    let zoom_label = format!("Zoom {:.2}×", proj.scale);
    let pause_l = if hud.paused { "Pausa ON (P)" } else { "Pausa off (P)" };
    let tool_l = match tool_state.active_tool {
        Some(BuildMenuAction::Road) => "Road",
        Some(BuildMenuAction::Rail) => "Rail",
        Some(BuildMenuAction::Station) => "Station",
        Some(BuildMenuAction::Clear) => "Clear",
        None => "None",
    };
    let hud_footer = format!(
        "{pause_l} | Tool: {tool_l} | JSON: {} | F4 otra ruta",
        hud.json_save_path
    );

    let Some(pos) = selected.pos else {
        **text =
            format!("{zoom_label}\n{hud_footer}\nClic mapa: elegir tile · toolbar tools · 1/2/3/C/Esc");
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
            s.push_str(&format!(" Xing rail:{}", level_crossing_rail_sprite_id(tile.m5)));
        }
        s
    } else if tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
        format!(
            " signals present:0x{:X} m2:0x{:02X}",
            (tile.m3 >> 4) & 0xF,
            tile.m2
        )
    } else if tile.kind == TileKind::Industry {
        format!(" gfx:{} ind:{}", tile.m5, tile.m1 & 0x7F)
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

    **text = format!(
        "{zoom_label}\n{hud_footer}\nTile ({},{}) {}\nh:{} slope:{} ({}) mapt:0x{:02X} m5:0x{:02X} m1:0x{:02X} m2:0x{:02X} m7:0x{:02X} m3:0x{:02X} m3hi:0x{:02X}{}{}",
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
        coast_dbg
    );
}
