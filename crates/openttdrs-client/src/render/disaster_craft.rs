//! OVNIs en vuelo (`DisasterCraft`, #188).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{DisasterCraft, DisasterKind};

use crate::bevy_app::UpdateSet;
use crate::iso::{iso, overlay_pos, tile_slope_and_min_z};
use crate::state::{ClientScreen, SimWorld};

const UFO_SMALL_PATH: &str = "assets/opengfx/tiles/ufo_small_scout.png";
const UFO_BIG_PATH: &str = "assets/opengfx/tiles/ufo_harvester.png";
const UFO_SHADOW_PATH: &str = "assets/opengfx/tiles/ufo_small_scout_darker.png";

/// (w, h, xrel, yrel) NFO / OpenGFX.
const UFO_SMALL_META: (f32, f32, f32, f32) = (19.0, 10.0, -8.0, -6.0);
const UFO_BIG_META: (f32, f32, f32, f32) = (35.0, 21.0, -15.0, -10.0);
const UFO_SHADOW_META: (f32, f32, f32, f32) = (19.0, 10.0, -8.0, -6.0);

#[derive(Resource)]
struct UfoSpriteHandles {
    small: Handle<Image>,
    big: Handle<Image>,
    shadow: Handle<Image>,
}

#[derive(Component)]
struct DisasterCraftSprite {
    id: u32,
    is_shadow: bool,
}

pub(crate) struct DisasterCraftPlugin;

impl Plugin for DisasterCraftPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_ufo_sprites).add_systems(
            Update,
            sync_disaster_crafts
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

fn load_ufo_sprites(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(UfoSpriteHandles {
        small: asset_server.load(UFO_SMALL_PATH),
        big: asset_server.load(UFO_BIG_PATH),
        shadow: asset_server.load(UFO_SHADOW_PATH),
    });
}

fn craft_meta(kind: DisasterKind) -> (f32, f32, f32, f32) {
    match kind {
        DisasterKind::BigUfo => UFO_BIG_META,
        _ => UFO_SMALL_META,
    }
}

fn craft_image(handles: &UfoSpriteHandles, kind: DisasterKind) -> Handle<Image> {
    match kind {
        DisasterKind::BigUfo => handles.big.clone(),
        _ => handles.small.clone(),
    }
}

fn craft_world_pos(craft: &DisasterCraft, map: &openttdrs_core::Map, bob: f32) -> Vec3 {
    let (tx, ty) = (craft.pos.x, craft.pos.y);
    let (_, base_z) =
        tile_slope_and_min_z(map, tx.max(0).cast_unsigned(), ty.max(0).cast_unsigned());
    let height = base_z.saturating_add(craft.altitude);
    let (w, h, xrel, yrel) = craft_meta(craft.kind);
    overlay_pos(iso(tx, ty), xrel, yrel - bob, w, h, height, 1.15, tx, ty)
}

fn shadow_world_pos(craft: &DisasterCraft, map: &openttdrs_core::Map) -> Vec3 {
    let (tx, ty) = (craft.pos.x, craft.pos.y);
    let (_, base_z) =
        tile_slope_and_min_z(map, tx.max(0).cast_unsigned(), ty.max(0).cast_unsigned());
    let (w, h, xrel, yrel) = UFO_SHADOW_META;
    // Sombra en el suelo; el harvester reutiliza el scout oscuro escalado vía custom_size.
    overlay_pos(iso(tx, ty), xrel, yrel, w, h, base_z, 0.55, tx, ty)
}

fn bob_offset(age: u16) -> f32 {
    // Oscilación suave (~2 px) para que el craft se sienta vivo.
    let t = f32::from(age) * 0.18;
    t.sin() * 2.0
}

fn sync_disaster_crafts(
    sim: Res<SimWorld>,
    handles: Option<Res<UfoSpriteHandles>>,
    mut commands: Commands,
    mut q: Query<(Entity, &DisasterCraftSprite, &mut Transform, &mut Sprite)>,
) {
    let Some(handles) = handles else {
        return;
    };
    let mut seen: HashMap<u32, (bool, bool)> = HashMap::new();
    for craft in &sim.state.disaster_crafts {
        if !craft.is_ufo() {
            continue;
        }
        seen.insert(craft.id, (false, false));
    }

    for (entity, sprite, mut transform, mut spr) in &mut q {
        let Some(craft) = sim.state.disaster_crafts.iter().find(|c| c.id == sprite.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !craft.is_ufo() {
            commands.entity(entity).despawn();
            continue;
        }
        if sprite.is_shadow {
            transform.translation = shadow_world_pos(craft, &sim.state.map);
            spr.image = handles.shadow.clone();
            spr.color = Color::srgba(0.0, 0.0, 0.0, 0.45);
            if craft.kind == DisasterKind::BigUfo {
                spr.custom_size = Some(Vec2::new(UFO_BIG_META.0 * 0.85, UFO_BIG_META.1 * 0.55));
            } else {
                spr.custom_size = None;
            }
            if let Some(flags) = seen.get_mut(&craft.id) {
                flags.1 = true;
            }
        } else {
            let bob = bob_offset(craft.age);
            transform.translation = craft_world_pos(craft, &sim.state.map, bob);
            spr.image = craft_image(&handles, craft.kind);
            spr.color = Color::WHITE;
            spr.custom_size = None;
            if let Some(flags) = seen.get_mut(&craft.id) {
                flags.0 = true;
            }
        }
    }

    for craft in &sim.state.disaster_crafts {
        if !craft.is_ufo() {
            continue;
        }
        let (has_body, has_shadow) = seen.get(&craft.id).copied().unwrap_or((false, false));
        if !has_body {
            let bob = bob_offset(craft.age);
            commands.spawn((
                DisasterCraftSprite {
                    id: craft.id,
                    is_shadow: false,
                },
                Sprite {
                    image: craft_image(&handles, craft.kind),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(craft_world_pos(craft, &sim.state.map, bob)),
                Visibility::Visible,
            ));
        }
        if !has_shadow {
            let mut shadow_sprite = Sprite {
                image: handles.shadow.clone(),
                color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                ..default()
            };
            if craft.kind == DisasterKind::BigUfo {
                shadow_sprite.custom_size =
                    Some(Vec2::new(UFO_BIG_META.0 * 0.85, UFO_BIG_META.1 * 0.55));
            }
            commands.spawn((
                DisasterCraftSprite {
                    id: craft.id,
                    is_shadow: true,
                },
                shadow_sprite,
                Transform::from_translation(shadow_world_pos(craft, &sim.state.map)),
                Visibility::Visible,
            ));
        }
    }
}
