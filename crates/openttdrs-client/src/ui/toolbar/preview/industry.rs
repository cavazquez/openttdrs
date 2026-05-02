use bevy::prelude::*;
use openttdrs_core::{IndustrySpec, Map, TileCoord, industry_template};

use crate::iso::{iso, overlay_pos, tile_pos, tile_slope_and_min_z};
use crate::sprites::industry_gfx_entry;
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

pub(crate) fn industry_spec_for_action(action: BuildMenuAction) -> Option<IndustrySpec> {
    match action {
        BuildMenuAction::BuildCoalMine => Some(IndustrySpec::CoalMine),
        BuildMenuAction::BuildIronOreMine => Some(IndustrySpec::IronOreMine),
        BuildMenuAction::BuildGoldMine => Some(IndustrySpec::GoldMine),
        BuildMenuAction::BuildOilWell => Some(IndustrySpec::OilWells),
        BuildMenuAction::BuildOilRefinery => Some(IndustrySpec::OilRefinery),
        BuildMenuAction::BuildFactory => Some(IndustrySpec::Factory),
        BuildMenuAction::BuildSawmill => Some(IndustrySpec::Sawmill),
        BuildMenuAction::BuildForest => Some(IndustrySpec::Forest),
        BuildMenuAction::BuildFarm => Some(IndustrySpec::Farm),
        _ => None,
    }
}

pub(crate) fn spawn_industry_template_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    origin: TileCoord,
    spec: IndustrySpec,
    tint: Color,
) {
    for (coord, m5) in industry_template(origin, spec) {
        if map.get(coord).is_none() {
            continue;
        }
        let (tileh, base_z) =
            tile_slope_and_min_z(map, coord.x.max(0) as u32, coord.y.max(0) as u32);
        let ground = if tileh == 0 {
            asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png")
        } else {
            asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/terrain_rough_slope_{tileh:02}.png"
            ))
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: ground,
                color: tint.with_alpha(tint.alpha() * 0.75),
                ..default()
            },
            Transform::from_translation(tile_pos(coord.x, coord.y, base_z, 2.9))
                .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));

        let Some(entry) = industry_gfx_entry(u16::from(m5)) else {
            continue;
        };
        let ref_pos = iso(coord.x, coord.y);
        if entry.ground_sprite_id != 0 {
            let img = asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/industry_{}.png",
                entry.ground_sprite_id
            ));
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: img,
                    color: tint,
                    ..default()
                },
                Transform::from_translation(overlay_pos(
                    ref_pos, entry.xrel, entry.yrel, entry.w, entry.h, base_z, 3.2, coord.x,
                    coord.y,
                )),
            ));
        }
        if entry.sprite_id != 0 {
            let img = asset_server.load::<Image>(format!(
                "assets/opengfx/tiles/industry_{}.png",
                entry.sprite_id
            ));
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: img,
                    color: tint,
                    ..default()
                },
                Transform::from_translation(overlay_pos(
                    ref_pos, entry.xrel, entry.yrel, entry.w, entry.h, base_z, 3.3, coord.x,
                    coord.y,
                )),
            ));
        }
    }
}
