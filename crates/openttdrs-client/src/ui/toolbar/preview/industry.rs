use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{IndustrySpec, industry_template};

use crate::iso::{iso, overlay_pos, tile_pos, tile_slope_and_min_z};
use crate::render::leveled_foundation_overlay_pos;
use crate::sprites::{foundation_asset_path, foundation_gfx_for_tileh, industry_gfx_entry};
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
        BuildMenuAction::BuildCottonCandy => Some(IndustrySpec::CottonCandy),
        BuildMenuAction::BuildCandyFactory => Some(IndustrySpec::CandyFactory),
        BuildMenuAction::BuildBatteryFarm => Some(IndustrySpec::BatteryFarm),
        BuildMenuAction::BuildColaWells => Some(IndustrySpec::ColaWells),
        BuildMenuAction::BuildToyFactory => Some(IndustrySpec::ToyFactory),
        BuildMenuAction::BuildPlasticFountain => Some(IndustrySpec::PlasticFountain),
        BuildMenuAction::BuildFizzyDrinkFactory => Some(IndustrySpec::FizzyDrinkFactory),
        BuildMenuAction::BuildBubbleGenerator => Some(IndustrySpec::BubbleGenerator),
        BuildMenuAction::BuildToffeeQuarry => Some(IndustrySpec::ToffeeQuarry),
        BuildMenuAction::BuildSugarMine => Some(IndustrySpec::SugarMine),
        _ => None,
    }
}

/// Herramientas de industria del panel Economía visibles según el clima del mapa.
#[must_use]
pub(crate) fn economy_industry_tool_visible(
    action: BuildMenuAction,
    climate: openttdrs_core::Climate,
) -> bool {
    match industry_spec_for_action(action) {
        Some(spec) => spec.available_in(climate),
        None => true,
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

        let leveled = tileh != 0;
        if leveled
            && let (Some(path), Some(gfx)) = (
                foundation_asset_path(tileh),
                foundation_gfx_for_tileh(tileh),
            )
        {
            let img = asset_server.load::<Image>(path);
            let ref_pos = iso(coord.x, coord.y);
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: img,
                    color: tint.with_alpha(tint.alpha() * 0.85),
                    ..default()
                },
                Transform::from_translation(overlay_pos(
                    ref_pos, gfx.xrel, gfx.yrel, gfx.w, gfx.h, base_z, 3.05, coord.x, coord.y,
                )),
            ));
        }

        let Some(entry) = industry_gfx_entry(u16::from(m5)) else {
            continue;
        };
        let ref_pos = iso(coord.x, coord.y);
        let overlay_at = |xrel, yrel, w, h, layer| {
            if leveled {
                leveled_foundation_overlay_pos(
                    ref_pos, xrel, yrel, w, h, base_z, layer, coord.x, coord.y,
                )
            } else {
                overlay_pos(ref_pos, xrel, yrel, w, h, base_z, layer, coord.x, coord.y)
            }
        };
        if entry.ground_sprite_id != 0 && entry.ground_w > 0.0 && entry.ground_h > 0.0 {
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
                Transform::from_translation(overlay_at(
                    entry.ground_xrel,
                    entry.ground_yrel,
                    entry.ground_w,
                    entry.ground_h,
                    3.2,
                )),
            ));
        }
        if entry.sprite_id != 0 && entry.w > 0.0 && entry.h > 0.0 {
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
                Transform::from_translation(overlay_at(
                    entry.xrel, entry.yrel, entry.w, entry.h, 3.3,
                )),
            ));
        }
    }
}
