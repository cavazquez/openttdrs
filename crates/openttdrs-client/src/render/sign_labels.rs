//! Etiquetas de carteles del jugador en el viewport.

use bevy::prelude::*;
use openttdrs_core::{CompanyId, Sign, SignOwner};

use crate::iso::{tile_pos, tile_slope_and_min_z};
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

pub(crate) fn spawn_sign_labels(
    commands: &mut Commands,
    sim: &SimWorld,
    font: &Handle<Font>,
    candidates: &MapLabelCandidates,
    show_competitors: bool,
) {
    use crate::sprites::{TransparencyOption, is_hidden, text_color, with_to_alpha};
    if is_hidden(TransparencyOption::Signs) {
        return;
    }
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
                    color: with_to_alpha(background, TransparencyOption::Signs),
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
            TextColor(text_color(
                TransparencyOption::Signs,
                Color::srgb(0.05, 0.05, 0.05),
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
}
