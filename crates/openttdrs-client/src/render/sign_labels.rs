//! Etiquetas de carteles del jugador en el viewport.

use bevy::prelude::*;
use openttdrs_core::{CompanyId, Sign, SignOwner};

use crate::iso::{tile_pos, tile_slope_and_min_z};
use crate::render::newgrf_cache::tile_layout_destination_transparent_color;
use crate::render::{MapLabelCandidates, MapLabelLod, MapLabelText, MapVisualLayer};
use crate::state::SimWorld;

const LABEL_Z: f32 = 901.0;
const FONT_SIZE: f32 = 10.0;
const SMALL_FONT_SIZE: f32 = 7.0;
const CHAR_ADVANCE: f32 = FONT_SIZE * 0.602;
const LABEL_RAISE: f32 = 22.0;
const LABEL_BACKGROUND_ALPHA: f32 = 1.0;
const UNOWNED_LABEL_COLOUR: Color = Color::srgb(0.42, 0.42, 0.42);

#[derive(Component)]
pub(crate) struct SignLabel;

/// `true` si el cartel pasa el filtro de competidores de OpenTTD.
#[must_use]
pub(crate) fn sign_label_visible(
    sign: &Sign,
    local_company: CompanyId,
    show_competitors: bool,
) -> bool {
    sign.owner.visible_to(local_company, show_competitors)
}

fn label_background_colour(sim: &SimWorld, sign: &Sign) -> Option<Color> {
    let base = match sign.owner {
        SignOwner::Company(owner) => sim
            .state
            .companies
            .iter()
            .find(|company| company.id == owner)
            .map(|company| crate::sprites::company_colour_swatch_color(company.colour))
            .unwrap_or(UNOWNED_LABEL_COLOUR),
        SignOwner::Unowned => UNOWNED_LABEL_COLOUR,
        // Los carteles de GameScript usan `INVALID_COLOUR` y no llevan marco.
        SignOwner::Deity => return None,
    };
    let colour = base.to_srgba();
    Some(Color::srgba(
        colour.red,
        colour.green,
        colour.blue,
        LABEL_BACKGROUND_ALPHA,
    ))
}

#[must_use]
fn sign_background_sprite_color(background: Color, signs_transparent: bool) -> Color {
    if signs_transparent {
        tile_layout_destination_transparent_color()
    } else {
        background
    }
}

#[must_use]
fn sign_text_color(has_frame: bool, signs_transparent: bool) -> Color {
    if !has_frame || signs_transparent {
        Color::WHITE
    } else {
        Color::srgb(0.05, 0.05, 0.05)
    }
}

pub(crate) fn spawn_sign_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    candidates: &MapLabelCandidates,
    show_competitors: bool,
) {
    use crate::sprites::{TransparencyOption, is_hidden, is_transparent};
    if is_hidden(TransparencyOption::Signs) {
        return;
    }
    let signs_transparent = is_transparent(TransparencyOption::Signs);
    for &index in &candidates.signs {
        let Some(sign) = sim.state.signs.get(index) else {
            continue;
        };
        if !sign_label_visible(sign, sim.state.active_company, show_competitors) {
            continue;
        }
        let (tx, ty) = (sign.pos.x, sign.pos.y);
        let (tileh, base_z) = tile_slope_and_min_z(&sim.state.map, tx as u32, ty as u32);
        let ground = tile_pos(tx, ty, base_z, 0.0);
        let center = Vec2::new(
            ground.x,
            ground.y + LABEL_RAISE + f32::from(tileh & 0xF) * 2.0,
        );
        let width = sign.name.chars().count() as f32 * CHAR_ADVANCE + 6.0;
        let small_size = Vec2::new(
            sign.name.chars().count() as f32 * (SMALL_FONT_SIZE * 0.602) + 5.0,
            SMALL_FONT_SIZE + 4.0,
        );
        let lod = MapLabelLod {
            size: Vec2::new(width, FONT_SIZE + 4.0),
            small_size,
        };
        if let Some(background) = label_background_colour(sim, sign) {
            commands.spawn((
                MapVisualLayer,
                SignLabel,
                lod,
                Sprite {
                    color: sign_background_sprite_color(background, signs_transparent),
                    custom_size: Some(Vec2::new(width, FONT_SIZE + 4.0)),
                    ..default()
                },
                Transform::from_translation(center.extend(LABEL_Z)),
            ));
        }
        commands.spawn((
            MapVisualLayer,
            SignLabel,
            lod,
            MapLabelText {
                normal: sign.name.clone(),
                small: sign.name.clone(),
            },
            Text2d::new(sign.name.clone()),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(sign_text_color(
                label_background_colour(sim, sign).is_some(),
                signs_transparent,
            )),
            Transform::from_translation(center.extend(LABEL_Z + 0.1)),
        ));
    }
}

pub(crate) fn resync_sign_labels(
    commands: &mut Commands,
    label_entities: impl IntoIterator<Item = Entity>,
    sim: &SimWorld,
    font: &Handle<Font>,
    candidates: &MapLabelCandidates,
    show_competitors: bool,
) {
    for entity in label_entities {
        commands.entity(entity).despawn();
    }
    spawn_sign_labels(commands, sim, font, candidates, show_competitors);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_filter_keeps_local_and_deity_signs() {
        let local = CompanyId::PLAYER;
        let mut rival = Sign::new(1, openttdrs_core::TileCoord::new(2, 2), "Rival");
        rival.owner = SignOwner::Company(CompanyId(1));
        assert!(!sign_label_visible(&rival, local, false));
        assert!(sign_label_visible(&rival, local, true));

        rival.owner = SignOwner::Deity;
        assert!(sign_label_visible(&rival, local, false));
    }

    #[test]
    fn transparent_sign_frame_uses_destination_mask() {
        let colour = sign_background_sprite_color(Color::srgb(0.8, 0.2, 0.1), true).to_srgba();
        assert_eq!((colour.red, colour.green, colour.blue), (0.0, 0.0, 0.0));
        assert!((colour.alpha - (64.0 / 255.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn transparent_and_deity_signs_use_opaque_white_text() {
        assert_eq!(sign_text_color(true, true), Color::WHITE);
        assert_eq!(sign_text_color(false, false), Color::WHITE);
        assert_eq!(sign_text_color(true, false), Color::srgb(0.05, 0.05, 0.05));
    }
}
