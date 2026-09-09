//! OVNIs en vuelo (`DisasterCraft`, #188).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{DisasterCraft, DisasterKind, Map, TileCoord, slope_dz_at_subtile};

use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, tile_slope_and_min_z};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    MapVisualLayer, ViewportSortableParent, viewport_insertion_key, viewport_source_depth,
};
use crate::state::{ClientScreen, SimWorld};

const UFO_SMALL_PATH: &str = "assets/opengfx/tiles/ufo_small_scout.png";
const UFO_BIG_PATH: &str = "assets/opengfx/tiles/ufo_harvester.png";
const UFO_SHADOW_PATH: &str = "assets/opengfx/tiles/ufo_small_scout_darker.png";

/// (w, h, xrel, yrel) NFO / OpenGFX.
const UFO_SMALL_META: (f32, f32, f32, f32) = (19.0, 10.0, -8.0, -6.0);
const UFO_BIG_META: (f32, f32, f32, f32) = (35.0, 21.0, -15.0, -10.0);
const UFO_SHADOW_META: (f32, f32, f32, f32) = (19.0, 10.0, -8.0, -6.0);

const TILE_SIZE_PX: i32 = 16;
const DISASTER_CRAFT_PARENT_ORDINAL_BASE: u8 = 0x80;
/// IDs sólo para la traza del compositor; no son índices del atlas.
const DISASTER_CRAFT_SORT_SPRITE_ID_BASE: u32 = 0xFFFE_0030;

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

/// Coordenada de mundo (píxeles nativos) de un `DisasterVehicle`.
/// `source_tile` sólo acota culling/orden de inserción: las coordenadas
/// físicas se conservan incluso al salir un píxel del mapa, como OpenTTD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisasterCraftWorldPosition {
    x: i32,
    y: i32,
    z: i32,
    source_tile: TileCoord,
}

/// Tesela segura para consultas de mapa. `DisasterVehicle::UpdatePosition`
/// conserva X/Y fuera del borde, pero usa el borde más próximo al consultar
/// `GetSlopePixelZ` para la sombra enlazada.
fn craft_ground_sample(map: &Map, x: i32, y: i32) -> (TileCoord, i32, i32) {
    let (width, height) = map.dimensions();
    let max_x = i32::try_from(width.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let max_y = i32::try_from(height.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let safe_x = x.clamp(0, max_x);
    let safe_y = y.clamp(0, max_y);
    (
        TileCoord::new(
            safe_x.div_euclid(TILE_SIZE_PX),
            safe_y.div_euclid(TILE_SIZE_PX),
        ),
        safe_x.rem_euclid(TILE_SIZE_PX),
        safe_y.rem_euclid(TILE_SIZE_PX),
    )
}

/// `GetSlopePixelZ` para la representación de mapa del cliente.
fn craft_ground_position(map: &Map, x: i32, y: i32) -> DisasterCraftWorldPosition {
    let (source_tile, sub_x, sub_y) = craft_ground_sample(map, x, y);
    let (tileh, base_z) = tile_slope_and_min_z(
        map,
        u32::try_from(source_tile.x).unwrap_or(0),
        u32::try_from(source_tile.y).unwrap_or(0),
    );
    let z = i32::from(base_z) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        + slope_dz_at_subtile(sub_x as f32, sub_y as f32, tileh).round() as i32;
    DisasterCraftWorldPosition {
        x,
        y,
        z,
        source_tile,
    }
}

/// Posición del cuerpo de `ST_SMALL_UFO` / `ST_BIG_UFO`. El estado reducido
/// guarda `altitude` en unidades de altura de tesela, por lo que la elevación
/// física conserva los ocho píxeles por unidad del renderer.
fn craft_body_world_position(craft: &DisasterCraft, map: &Map) -> DisasterCraftWorldPosition {
    let x = craft.pos.x.saturating_mul(TILE_SIZE_PX);
    let y = craft.pos.y.saturating_mul(TILE_SIZE_PX);
    let mut body = craft_ground_position(map, x, y);
    body.z = body
        .z
        .saturating_add(i32::from(craft.altitude) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT));
    body
}

/// `DisasterVehicle::UpdatePosition` mantiene la sombra como vehículo propio:
/// la desplaza hacia el norte según la distancia al suelo y vuelve a muestrear
/// su Z. No es un child de pantalla del cuerpo.
fn craft_shadow_world_position(craft: &DisasterCraft, map: &Map) -> DisasterCraftWorldPosition {
    let body = craft_body_world_position(craft, map);
    let ground_before_shadow = craft_ground_position(map, body.x, body.y.saturating_sub(1));
    let drop = body
        .z
        .saturating_sub(ground_before_shadow.z)
        .max(0)
        .div_euclid(i32::from(openttdrs_core::TILE_PIXEL_HEIGHT));
    craft_ground_position(map, body.x, body.y.saturating_sub(1).saturating_sub(drop))
}

fn craft_world_position(
    craft: &DisasterCraft,
    map: &Map,
    is_shadow: bool,
) -> DisasterCraftWorldPosition {
    if is_shadow {
        craft_shadow_world_position(craft, map)
    } else {
        craft_body_world_position(craft, map)
    }
}

fn craft_source_depth(position: DisasterCraftWorldPosition, map_width: u32) -> f32 {
    let diagonal_depth = (position.source_tile.x + position.source_tile.y) as f32 * 0.01;
    let height_depth = position.z as f32 / f32::from(openttdrs_core::TILE_PIXEL_HEIGHT) * 0.0001;
    viewport_source_depth(
        diagonal_depth + height_depth + 0.001,
        u32::try_from(position.source_tile.x).unwrap_or(0),
        map_width,
    )
}

fn craft_parent_sprite_id(kind: DisasterKind, is_shadow: bool) -> u32 {
    DISASTER_CRAFT_SORT_SPRITE_ID_BASE
        + match kind {
            DisasterKind::BigUfo => 2,
            _ => 0,
        }
        + if is_shadow { 1 } else { 0 }
}

fn craft_parent(
    craft: &DisasterCraft,
    position: DisasterCraftWorldPosition,
    is_shadow: bool,
    map_width: u32,
) -> ViewportSortableParent {
    let source_x = u32::try_from(position.source_tile.x).unwrap_or(0);
    let source_y = u32::try_from(position.source_tile.y).unwrap_or(0);
    // Cuerpo y sombra se crean en ese orden en OpenTTD. Los siete bits bajos
    // mantienen el desempate estable entre los pocos craft simultáneos.
    let local_ordinal = DISASTER_CRAFT_PARENT_ORDINAL_BASE
        | u8::try_from(
            craft
                .id
                .wrapping_mul(2)
                .wrapping_add(if is_shadow { 1 } else { 0 })
                & 0x7F,
        )
        .unwrap_or(0);
    ViewportSortableParent {
        sprite_id: craft_parent_sprite_id(craft.kind, is_shadow),
        // `DisasterVehicle::UpdateDeltaXY`: `{{-1, -1, 0}, {2, 2, 5}, {}}`.
        bounds: ParentSpriteBounds::new(
            position.x.saturating_sub(1),
            position.y.saturating_sub(1),
            position.z,
            position.x,
            position.y,
            position.z.saturating_add(4),
        ),
        insertion_key: viewport_insertion_key(source_x, source_y, local_ordinal),
        source_depth: craft_source_depth(position, map_width),
    }
}

fn craft_translation(
    craft: &DisasterCraft,
    position: DisasterCraftWorldPosition,
    is_shadow: bool,
    source_depth: f32,
) -> Vec3 {
    let (w, h, xrel, yrel) = if is_shadow {
        UFO_SHADOW_META
    } else {
        craft_meta(craft.kind)
    };
    let anchor = road_vehicle_tile_anchor(
        0,
        0,
        position.x as f32,
        position.y as f32,
        position.z as f32,
    );
    let bob = if is_shadow {
        0.0
    } else {
        bob_offset(craft.age)
    };
    Vec3::new(
        anchor.x + xrel + w * 0.5,
        anchor.y - (yrel + h * 0.5) + bob,
        source_depth,
    )
}

fn craft_sprite(handles: &UfoSpriteHandles, craft: &DisasterCraft, is_shadow: bool) -> Sprite {
    if is_shadow {
        let custom_size = (craft.kind == DisasterKind::BigUfo)
            .then_some(Vec2::new(UFO_BIG_META.0 * 0.85, UFO_BIG_META.1 * 0.55));
        Sprite {
            image: handles.shadow.clone(),
            color: Color::srgba(0.0, 0.0, 0.0, 0.45),
            custom_size,
            ..default()
        }
    } else {
        Sprite {
            image: craft_image(handles, craft.kind),
            color: Color::WHITE,
            ..default()
        }
    }
}

/// `Sprite` no implementa `PartialEq`, pero estos tres campos son toda la
/// salida que controla este producer. Evitar writes idénticos conserva el
/// change detection de Bevy en un frame donde el craft no avanzó.
fn set_craft_sprite_if_changed(sprite: &mut Mut<Sprite>, next: Sprite) {
    let Sprite {
        image,
        color,
        custom_size,
        ..
    } = next;
    if sprite.image != image {
        sprite.image = image;
    }
    if sprite.color != color {
        sprite.color = color;
    }
    if sprite.custom_size != custom_size {
        sprite.custom_size = custom_size;
    }
}

/// La Z efectiva pertenece al compositor. La simulación puede mover el craft
/// sin devolverlo a `source_depth` entre dos pases de sort.
fn set_craft_translation_if_changed(
    transform: &mut Mut<Transform>,
    source_translation: Vec3,
    preserves_sorted_depth: bool,
) {
    let translation = if preserves_sorted_depth {
        Vec3::new(
            source_translation.x,
            source_translation.y,
            transform.translation.z,
        )
    } else {
        source_translation
    };
    if transform.translation != translation {
        transform.translation = translation;
    }
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
    mut q: Query<(
        Entity,
        &DisasterCraftSprite,
        &mut Transform,
        &mut Sprite,
        Option<&mut ViewportSortableParent>,
    )>,
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

    let map_width = sim.state.map.dimensions().0;
    for (entity, sprite, mut transform, mut spr, sortable_parent) in &mut q {
        let Some(craft) = sim.state.disaster_crafts.iter().find(|c| c.id == sprite.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !craft.is_ufo() {
            commands.entity(entity).despawn();
            continue;
        }
        let position = craft_world_position(craft, &sim.state.map, sprite.is_shadow);
        let parent = craft_parent(craft, position, sprite.is_shadow, map_width);
        let translation = craft_translation(craft, position, sprite.is_shadow, parent.source_depth);
        let preserves_sorted_depth = sortable_parent.is_some();
        set_craft_translation_if_changed(&mut transform, translation, preserves_sorted_depth);
        if let Some(mut sortable_parent) = sortable_parent {
            if *sortable_parent != parent {
                *sortable_parent = parent;
            }
        } else {
            commands.entity(entity).insert(parent);
        }
        set_craft_sprite_if_changed(&mut spr, craft_sprite(&handles, craft, sprite.is_shadow));
        if sprite.is_shadow {
            if let Some(flags) = seen.get_mut(&craft.id) {
                flags.1 = true;
            }
        } else {
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
            let position = craft_world_position(craft, &sim.state.map, false);
            let parent = craft_parent(craft, position, false, map_width);
            commands.spawn((
                MapVisualLayer,
                DisasterCraftSprite {
                    id: craft.id,
                    is_shadow: false,
                },
                craft_sprite(&handles, craft, false),
                Transform::from_translation(craft_translation(
                    craft,
                    position,
                    false,
                    parent.source_depth,
                )),
                Visibility::Visible,
                parent,
            ));
        }
        if !has_shadow {
            let position = craft_world_position(craft, &sim.state.map, true);
            let parent = craft_parent(craft, position, true, map_width);
            commands.spawn((
                MapVisualLayer,
                DisasterCraftSprite {
                    id: craft.id,
                    is_shadow: true,
                },
                craft_sprite(&handles, craft, true),
                Transform::from_translation(craft_translation(
                    craft,
                    position,
                    true,
                    parent.source_depth,
                )),
                Visibility::Visible,
                parent,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::change_detection::DetectChanges;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, GameTick};

    use super::*;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn ufo(id: u32, kind: DisasterKind, pos: TileCoord, altitude: u8) -> DisasterCraft {
        DisasterCraft {
            id,
            kind,
            pos,
            target: pos,
            altitude,
            age: 0,
            ticks_to_impact: 74,
        }
    }

    fn test_handles() -> UfoSpriteHandles {
        UfoSpriteHandles {
            small: Handle::default(),
            big: Handle::default(),
            shadow: Handle::default(),
        }
    }

    #[test]
    fn ufo_body_and_shadow_keep_native_vehicle_prisms() {
        let map = Map::new_flat(8, 8, 0);
        let craft = ufo(9, DisasterKind::SmallUfo, TileCoord::new(3, 4), 24);
        let body = craft_body_world_position(&craft, &map);
        let shadow = craft_shadow_world_position(&craft, &map);

        assert_eq!(
            body,
            DisasterCraftWorldPosition {
                x: 48,
                y: 64,
                z: 192,
                source_tile: TileCoord::new(3, 4),
            }
        );
        assert_eq!(
            shadow,
            DisasterCraftWorldPosition {
                x: 48,
                y: 39,
                z: 0,
                source_tile: TileCoord::new(3, 2),
            },
            "la sombra es otro DisasterVehicle, desplazado por la altitud"
        );

        let body_parent = craft_parent(&craft, body, false, map.dimensions().0);
        let shadow_parent = craft_parent(&craft, shadow, true, map.dimensions().0);
        assert_eq!(
            body_parent.bounds,
            ParentSpriteBounds::new(47, 63, 192, 48, 64, 196)
        );
        assert_eq!(
            shadow_parent.bounds,
            ParentSpriteBounds::new(47, 38, 0, 48, 39, 4),
            "UpdateDeltaXY usa origin (-1,-1,0) y extent (2,2,5)"
        );
    }

    #[test]
    fn sync_spawns_both_ufo_vehicles_as_global_parents() {
        let craft = ufo(4, DisasterKind::BigUfo, TileCoord::new(2, 2), 8);
        let mut state = GameState::new(8, 8);
        state.map = Map::new_flat(8, 8, 0);
        state.tick = GameTick::new(17);
        state.disaster_crafts.push(craft.clone());
        let mut world = World::new();
        world.insert_resource(SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        });
        world.insert_resource(test_handles());

        world.run_system_once(sync_disaster_crafts).unwrap();

        let (body_entity, body_parent, shadow_parent) = {
            let mut crafts = world.query::<(
                Entity,
                &MapVisualLayer,
                &DisasterCraftSprite,
                &Transform,
                &ViewportSortableParent,
            )>();
            let mut body = None;
            let mut shadow = None;
            for (entity, _layer, sprite, _transform, parent) in crafts.iter(&world) {
                assert_eq!(sprite.id, craft.id);
                if sprite.is_shadow {
                    shadow = Some(*parent);
                } else {
                    body = Some((entity, *parent));
                }
            }
            let (body_entity, body_parent) = body.expect("cuerpo del OVNI");
            (body_entity, body_parent, shadow.expect("sombra del OVNI"))
        };
        let (expected_body_parent, expected_shadow_parent) = {
            let sim = world.resource::<SimWorld>();
            (
                craft_parent(
                    &craft,
                    craft_body_world_position(&craft, &sim.state.map),
                    false,
                    8,
                ),
                craft_parent(
                    &craft,
                    craft_shadow_world_position(&craft, &sim.state.map),
                    true,
                    8,
                ),
            )
        };
        assert_eq!(
            body_parent, expected_body_parent,
            "el cuerpo queda registrado como parent global"
        );
        assert_eq!(
            shadow_parent, expected_shadow_parent,
            "la sombra tiene su propio parent global"
        );

        world.clear_trackers();
        let unchanged_after_spawn = (
            world
                .entity(body_entity)
                .get_ref::<Transform>()
                .expect("transform del cuerpo")
                .last_changed(),
            world
                .entity(body_entity)
                .get_ref::<Sprite>()
                .expect("sprite del cuerpo")
                .last_changed(),
            world
                .entity(body_entity)
                .get_ref::<ViewportSortableParent>()
                .expect("parent del cuerpo")
                .last_changed(),
        );
        world.run_system_once(sync_disaster_crafts).unwrap();
        assert_eq!(
            world
                .entity(body_entity)
                .get_ref::<Transform>()
                .expect("transform estable")
                .last_changed(),
            unchanged_after_spawn.0,
            "un frame estable no invalida la pose"
        );
        assert_eq!(
            world
                .entity(body_entity)
                .get_ref::<Sprite>()
                .expect("sprite estable")
                .last_changed(),
            unchanged_after_spawn.1,
            "un frame estable no invalida el sprite"
        );
        assert_eq!(
            world
                .entity(body_entity)
                .get_ref::<ViewportSortableParent>()
                .expect("parent estable")
                .last_changed(),
            unchanged_after_spawn.2,
            "un frame estable no obliga a reordenar el viewport"
        );

        let sorted_depth = 9.75;
        world
            .get_mut::<Transform>(body_entity)
            .expect("transform del cuerpo")
            .translation
            .z = sorted_depth;
        world.run_system_once(sync_disaster_crafts).unwrap();
        assert_eq!(
            world
                .get::<Transform>(body_entity)
                .expect("transform actualizado")
                .translation
                .z,
            sorted_depth,
            "una actualización visual no debe restaurar source_depth antes del sorter"
        );
        assert_ne!(expected_body_parent.source_depth, sorted_depth);
    }

    #[test]
    fn ufo_parent_enters_the_global_viewport_sorter() {
        let map = Map::new_flat(8, 8, 0);
        let craft = ufo(1, DisasterKind::SmallUfo, TileCoord::new(3, 4), 8);
        let parent = craft_parent(
            &craft,
            craft_body_world_position(&craft, &map),
            false,
            map.dimensions().0,
        );
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let ufo = world
            .spawn((parent, Transform::from_xyz(0.0, 0.0, parent.source_depth)))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 9_998,
                bounds: ParentSpriteBounds::new(
                    parent.bounds.xmin.saturating_sub(1),
                    parent.bounds.ymin,
                    parent.bounds.zmin,
                    parent.bounds.xmin,
                    parent.bounds.ymax,
                    parent.bounds.zmax,
                ),
                insertion_key: parent.insertion_key + 1,
                source_depth: parent.source_depth + 0.000_5,
            },
            Transform::from_xyz(0.0, 0.0, parent.source_depth + 0.000_5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        assert!(
            world
                .entity(ufo)
                .get::<Transform>()
                .expect("transform del OVNI")
                .translation
                .z
                > parent.source_depth,
            "el cuerpo del OVNI debe recibir la profundidad del compositor global"
        );
    }
}
