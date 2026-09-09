//! Burbujas libres del generador Toyland (`EV_BUBBLE`).

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, partial_pixel_z};

use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, tile_slope_and_min_z, wang_hash};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    MapVisualLayer, ViewportSortableParent, WorldAssets, palette_animations_should_run,
    viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{BUBBLE_FRAMES, BUBBLE_META, TransparencyOption, with_to_alpha};
use crate::state::{ClientScreen, SimWorld};

const SPAWN_X: [i16; 4] = [11, 0, -4, -14];
const SPAWN_Y: [i16; 4] = [-4, -10, -4, 1];
const SPAWN_Z: [i16; 4] = [49, 59, 60, 65];

/// Las burbujas llegan al compositor durante el pase de vehículos, después
/// de los productores de tesela de la misma posición. Reservar el bit alto
/// para este pase y los restantes para el contador evita que dos instancias
/// creadas en el mismo tick dependan del orden de entidades ECS.
const BUBBLE_PARENT_ORDINAL_BASE: u8 = 0x80;

/// Sólo identifica el parent en las trazas del compositor: la imagen animada
/// sigue siendo el frame real de la burbuja. No puede ser `SPR_EMPTY_BOUNDING_BOX`.
const BUBBLE_SORT_SPRITE_ID: u32 = 0xFFFE_0001;
const TILE_SIZE_PX: i32 = 16;

#[derive(Resource, Default)]
pub(crate) struct BubbleSpawnQueue(Vec<(TileCoord, u8)>);

impl BubbleSpawnQueue {
    pub(crate) fn push(&mut self, at: TileCoord, direction: u8) {
        self.0.push((at, direction & 3));
    }
}

/// Desempate estable entre efectos que comparten tesela. Los `EffectVehicle`
/// no forman parte del estado guardado, así que el contador sólo vive durante
/// la sesión de render y se reinicia junto con ella.
#[derive(Resource, Default)]
struct BubbleSortSequence(u8);

pub(crate) struct BubbleEffectPlugin;

impl Plugin for BubbleEffectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BubbleSpawnQueue>()
            .init_resource::<BubbleSortSequence>()
            .add_systems(
                Update,
                (spawn_queued_bubbles, animate_bubbles)
                    .chain()
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame))
                    .run_if(palette_animations_should_run),
            );
    }
}

#[derive(Component)]
struct BubbleEffect {
    at: TileCoord,
    direction: u8,
    float_direction: u8,
    seed: u32,
    started_tick: u64,
    sort_ordinal: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BubblePhase {
    Generate,
    Float,
    Burst,
    Absorb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BubbleState {
    frame: usize,
    x: i16,
    y: i16,
    z: i16,
    phase: BubblePhase,
}

/// Coordenadas de mundo del `EffectVehicle`. `x` e `y` se conservan sin
/// recortar, como `CreateEffectVehicleAbove`; sólo la consulta de suelo se
/// limita al borde del mapa antes de llamar a `GetSlopePixelZ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BubbleWorldPosition {
    x: i32,
    y: i32,
    z: i32,
    source_tile: TileCoord,
}

fn burst_on_cycle(seed: u32, cycle: u32) -> bool {
    wang_hash(seed, cycle, 0xBABB_1E55).is_multiple_of(96)
}

fn apply_delta(state: &mut BubbleState, dx: i16, dy: i16, dz: i16, frame: usize) {
    state.x += dx;
    state.y += dy;
    state.z += dz;
    state.frame = frame;
}

fn apply_float_step(state: &mut BubbleState, direction: u8, step: usize) {
    let phase = step % 4;
    let (dx, dy) = if phase == 1 || phase == 3 {
        match direction & 3 {
            0 => (1, 0),
            1 => (-1, 0),
            2 => (0, 1),
            _ => (0, -1),
        }
    } else {
        (0, 0)
    };
    apply_delta(state, dx, dy, 1, [0, 1, 0, 2][phase]);
}

fn apply_absorb_step(state: &mut BubbleState, step: usize) -> bool {
    if step < 64 {
        apply_delta(state, 0, 0, 1, [0, 1, 0, 2][step % 4]);
        return true;
    }
    const APPROACH: [(i16, i16, i16, usize); 8] = [
        (2, 1, 3, 0),
        (1, 1, 3, 1),
        (2, 1, 3, 0),
        (1, 1, 3, 2),
        (2, 1, 3, 0),
        (1, 1, 3, 1),
        (2, 1, 3, 0),
        (1, 0, 1, 2),
    ];
    if let Some(&(dx, dy, dz, frame)) = APPROACH.get(step - 64) {
        apply_delta(state, dx, dy, dz, frame);
        return true;
    }
    if step < 80 {
        let drift = step - 72;
        apply_delta(
            state,
            i16::from(drift % 2 == 1),
            0,
            1,
            [0, 1, 0, 2][drift % 4],
        );
        return true;
    }
    if step < 85 {
        state.frame = 10 + step - 80;
        return true;
    }
    false
}

/// Evalúa las tablas `_bubble_movement` una vez cada cuatro ticks.
#[must_use]
fn bubble_state(
    age_ticks: u64,
    direction: u8,
    float_direction: u8,
    seed: u32,
) -> Option<BubbleState> {
    let actions = usize::try_from(age_ticks / 4).unwrap_or(usize::MAX);
    let mut state = BubbleState {
        frame: 3,
        x: 0,
        y: 0,
        z: 0,
        phase: BubblePhase::Generate,
    };
    match actions {
        0 => return Some(state),
        1 => {
            state.frame = 4;
            return Some(state);
        }
        2 => {
            state.frame = 5;
            return Some(state);
        }
        _ => {}
    }

    let movement_actions = actions - 2;
    if direction & 3 == 0 {
        state.phase = BubblePhase::Absorb;
        for step in 0..movement_actions {
            if !apply_absorb_step(&mut state, step) {
                return None;
            }
        }
        return Some(state);
    }

    state.phase = BubblePhase::Float;
    let mut burst_step = None;
    for step in 0..movement_actions {
        if let Some(index) = burst_step {
            if index >= 4 {
                return None;
            }
            state.phase = BubblePhase::Burst;
            apply_delta(&mut state, 0, 0, 1, [2, 7, 8, 9][index]);
            burst_step = Some(index + 1);
            continue;
        }
        if step > 0
            && step.is_multiple_of(4)
            && (SPAWN_Z[usize::from(direction & 3)] + state.z > 180
                || burst_on_cycle(seed, u32::try_from(step / 4).unwrap_or(u32::MAX)))
        {
            state.phase = BubblePhase::Burst;
            apply_delta(&mut state, 0, 0, 1, 2);
            burst_step = Some(1);
        } else {
            apply_float_step(&mut state, float_direction, step);
        }
    }
    Some(state)
}

/// Tesela/subcoordenada seguras que usa `CreateEffectVehicleAbove` para la
/// altura de terreno. El efecto puede empezar unos píxeles fuera de la tesela
/// emisora; en ese caso OpenTTD conserva sus X/Y, pero toma la Z del borde.
fn bubble_ground_sample(map: &Map, x: i32, y: i32) -> (TileCoord, i32, i32) {
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

/// Posición exacta del `EffectVehicle` creado por
/// `CreateEffectVehicleAbove` y avanzado por `BubbleTick`.
fn bubble_world_position(
    map: &Map,
    effect: &BubbleEffect,
    state: BubbleState,
) -> BubbleWorldPosition {
    let index = usize::from(effect.direction & 3);
    let x = effect
        .at
        .x
        .saturating_mul(TILE_SIZE_PX)
        .saturating_add(i32::from(SPAWN_X[index]))
        .saturating_add(i32::from(state.x));
    let y = effect
        .at
        .y
        .saturating_mul(TILE_SIZE_PX)
        .saturating_add(i32::from(SPAWN_Y[index]))
        .saturating_add(i32::from(state.y));
    let (source_tile, sub_x, sub_y) = bubble_ground_sample(map, x, y);
    let tile_x = u32::try_from(source_tile.x).unwrap_or(0);
    let tile_y = u32::try_from(source_tile.y).unwrap_or(0);
    let (tileh, base_z) = tile_slope_and_min_z(map, tile_x, tile_y);
    let sub_x = i16::try_from(sub_x).unwrap_or(0);
    let sub_y = i16::try_from(sub_y).unwrap_or(0);
    let ground_z = i32::from(base_z) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        + i32::from(partial_pixel_z(f32::from(sub_x), f32::from(sub_y), tileh));
    BubbleWorldPosition {
        x,
        y,
        z: ground_z
            .saturating_add(i32::from(SPAWN_Z[index]))
            .saturating_add(i32::from(state.z)),
        source_tile,
    }
}

/// Profundidad de origen equivalente al slot de la pasada de vehículos. La
/// altura de mundo se expresa en niveles de 8 px, igual que `sortable_draw_z`;
/// el sorter sustituirá este valor por el orden final si se solapa con otro
/// parent.
fn bubble_source_depth(position: BubbleWorldPosition, map_width: u32) -> f32 {
    let diagonal_depth = (position.source_tile.x + position.source_tile.y) as f32 * 0.01;
    let height_depth = position.z as f32 / f32::from(openttdrs_core::TILE_PIXEL_HEIGHT) * 0.0001;
    viewport_source_depth(
        diagonal_depth + height_depth + 0.001,
        u32::try_from(position.source_tile.x).unwrap_or(0),
        map_width,
    )
}

/// `EffectVehicle::UpdateDeltaXY` fija `{ {}, {1, 1, 1}, {} }`, que se
/// convierte en un prisma inclusivo de un único píxel para el sorter.
fn bubble_parent(
    effect: &BubbleEffect,
    position: BubbleWorldPosition,
    map_width: u32,
) -> ViewportSortableParent {
    let source_x = u32::try_from(position.source_tile.x).unwrap_or(0);
    let source_y = u32::try_from(position.source_tile.y).unwrap_or(0);
    ViewportSortableParent {
        sprite_id: BUBBLE_SORT_SPRITE_ID,
        bounds: ParentSpriteBounds::new(
            position.x, position.y, position.z, position.x, position.y, position.z,
        ),
        insertion_key: viewport_insertion_key(
            source_x,
            source_y,
            BUBBLE_PARENT_ORDINAL_BASE | (effect.sort_ordinal & 0x7F),
        ),
        source_depth: bubble_source_depth(position, map_width),
    }
}

fn bubble_translation(
    position: BubbleWorldPosition,
    state: BubbleState,
    source_depth: f32,
) -> Vec3 {
    let anchor = road_vehicle_tile_anchor(
        0,
        0,
        position.x as f32,
        position.y as f32,
        position.z as f32,
    );
    let (w, h, xrel, yrel) = BUBBLE_META[state.frame];
    Vec3::new(
        anchor.x + xrel + w * 0.5,
        anchor.y - (yrel + h * 0.5),
        source_depth,
    )
}

/// La Z de un parent sortable es salida del compositor. Una animación de la
/// burbuja puede mover su prisma cada cuatro ticks, pero no debe restaurar la
/// profundidad fuente entre dos ejecuciones del sorter.
fn set_bubble_translation_if_changed(
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

fn spawn_queued_bubbles(
    mut queue: ResMut<BubbleSpawnQueue>,
    mut sort_sequence: ResMut<BubbleSortSequence>,
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        return;
    };
    if assets.bubble.len() != BUBBLE_FRAMES {
        queue.0.clear();
        return;
    }
    let tick = sim.state.tick.get();
    for (at, direction) in queue.0.drain(..) {
        let seed = wang_hash(at.x as u32, at.y as u32, tick as u32);
        let sort_ordinal = sort_sequence.0;
        sort_sequence.0 = sort_sequence.0.wrapping_add(1);
        let effect = BubbleEffect {
            at,
            direction,
            float_direction: (seed & 3) as u8,
            seed,
            started_tick: tick,
            sort_ordinal,
        };
        let Some(state) = bubble_state(0, direction, effect.float_direction, seed) else {
            continue;
        };
        let mut sprite = assets.bubble[state.frame].sprite();
        sprite.color = with_to_alpha(sprite.color, TransparencyOption::Industries);
        let position = bubble_world_position(&sim.state.map, &effect, state);
        let parent = bubble_parent(&effect, position, sim.state.map.dimensions().0);
        let translation = bubble_translation(position, state, parent.source_depth);
        commands.spawn((
            MapVisualLayer,
            effect,
            sprite,
            Transform::from_translation(translation),
            Visibility::Visible,
            parent,
        ));
    }
}

fn animate_bubbles(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut bubbles: Query<(
        Entity,
        &BubbleEffect,
        &mut Sprite,
        &mut Transform,
        Option<&mut ViewportSortableParent>,
    )>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        return;
    };
    let tick = sim.state.tick.get();
    for (entity, effect, mut sprite, mut transform, parent) in &mut bubbles {
        let age = tick.saturating_sub(effect.started_tick);
        let Some(state) = bubble_state(age, effect.direction, effect.float_direction, effect.seed)
        else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Some(frame) = assets.bubble.get(state.frame)
            && !frame.matches(&sprite)
        {
            frame.apply_to(&mut sprite);
        }
        let position = bubble_world_position(&sim.state.map, effect, state);
        let next_parent = bubble_parent(effect, position, sim.state.map.dimensions().0);
        let translation = bubble_translation(position, state, next_parent.source_depth);
        let preserves_sorted_depth = parent.is_some();
        set_bubble_translation_if_changed(&mut transform, translation, preserves_sorted_depth);
        if let Some(mut parent) = parent
            && *parent != next_parent
        {
            *parent = next_parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn bubble_effect(at: TileCoord, direction: u8, sort_ordinal: u8) -> BubbleEffect {
        BubbleEffect {
            at,
            direction,
            float_direction: 0,
            seed: 1,
            started_tick: 0,
            sort_ordinal,
        }
    }

    fn initial_bubble_state() -> BubbleState {
        BubbleState {
            frame: 3,
            x: 0,
            y: 0,
            z: 0,
            phase: BubblePhase::Generate,
        }
    }

    #[test]
    fn bubble_parent_uses_the_real_sloped_effect_vehicle_prism() {
        let mut map = Map::new_flat(4, 4, 4);
        // En (1, 1), esta esquina norte elevada produce `SLOPE_N`. La
        // burbuja de dirección 1 cae en la subcoordenada (0, 6), donde
        // `GetSlopePixelZ` suma 5 píxeles por encima de la base de 32.
        map.set_height(TileCoord::new(1, 1), 5)
            .expect("height on bubble slope");
        let effect = bubble_effect(TileCoord::new(1, 2), 1, 5);
        let position = bubble_world_position(&map, &effect, initial_bubble_state());

        assert_eq!(
            position,
            BubbleWorldPosition {
                x: 16,
                y: 22,
                z: 96,
                source_tile: TileCoord::new(1, 1),
            }
        );
        let parent = bubble_parent(&effect, position, map.dimensions().0);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(16, 22, 96, 16, 22, 96),
            "EffectVehicle::UpdateDeltaXY usa exactamente un prisma 1×1×1"
        );
        assert_eq!(
            parent.insertion_key,
            viewport_insertion_key(1, 1, BUBBLE_PARENT_ORDINAL_BASE | 5),
            "la burbuja se incorpora después de los productores estáticos de su tesela"
        );
        assert_eq!(
            bubble_translation(position, initial_bubble_state(), parent.source_depth).z,
            parent.source_depth,
            "la posición inicial conserva el slot fuente que luego reasigna el sorter"
        );
    }

    #[test]
    fn bubble_keeps_raw_xy_but_clamps_terrain_sample_at_map_edge() {
        let map = Map::new_flat(3, 3, 2);
        // Dirección 3: x = -14 respecto de la tesela (0, 1). OpenTTD no
        // recorta el vehículo, sólo consulta el suelo seguro en (0, 1).
        let effect = bubble_effect(TileCoord::new(0, 1), 3, 9);
        let position = bubble_world_position(&map, &effect, initial_bubble_state());

        assert_eq!(
            position,
            BubbleWorldPosition {
                x: -14,
                y: 17,
                z: 81,
                source_tile: TileCoord::new(0, 1),
            }
        );
        assert_eq!(
            bubble_parent(&effect, position, map.dimensions().0).bounds,
            ParentSpriteBounds::new(-14, 17, 81, -14, 17, 81)
        );
    }

    #[test]
    fn bubble_parent_moves_with_the_effect_across_a_tile_boundary() {
        let map = Map::new_flat(4, 4, 0);
        let effect = bubble_effect(TileCoord::new(1, 1), 1, 3);
        let start = bubble_world_position(&map, &effect, initial_bubble_state());
        let crossed = bubble_world_position(
            &map,
            &effect,
            BubbleState {
                x: -1,
                ..initial_bubble_state()
            },
        );

        assert_eq!(start.source_tile, TileCoord::new(1, 0));
        assert_eq!(crossed.source_tile, TileCoord::new(0, 0));
        assert_ne!(
            bubble_parent(&effect, start, map.dimensions().0).insertion_key,
            bubble_parent(&effect, crossed, map.dimensions().0).insertion_key,
            "el cambio de tesela vuelve a ejecutar el sorter sobre el alcance correcto"
        );
    }

    #[test]
    fn bubble_parent_enters_the_global_viewport_sorter() {
        let map = Map::new_flat(4, 4, 0);
        let effect = bubble_effect(TileCoord::new(1, 2), 1, 5);
        let position = bubble_world_position(&map, &effect, initial_bubble_state());
        let parent = bubble_parent(&effect, position, map.dimensions().0);

        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let bubble = world
            .spawn((
                parent,
                Transform::from_translation(bubble_translation(
                    position,
                    initial_bubble_state(),
                    parent.source_depth,
                )),
            ))
            .id();
        // Solapa exactamente el píxel del efecto en su borde este. El sorter
        // debe adelantar esta caja y reasignar el slot de la burbuja, lo que
        // demuestra que no queda en la capa local `MapVisualLayer`.
        world.spawn((
            ViewportSortableParent {
                sprite_id: 9_999,
                bounds: ParentSpriteBounds::new(
                    position.x - 1,
                    position.y,
                    position.z,
                    position.x,
                    position.y,
                    position.z,
                ),
                insertion_key: parent.insertion_key + 1,
                source_depth: parent.source_depth + 0.000_5,
            },
            Transform::from_xyz(0.0, 0.0, parent.source_depth + 0.000_5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        let sorted_depth = world
            .entity(bubble)
            .get::<Transform>()
            .expect("bubble transform")
            .translation
            .z;
        assert!(
            sorted_depth > parent.source_depth,
            "el orden global debe poder reemplazar la profundidad fuente de la burbuja"
        );
    }

    #[test]
    fn generation_uses_three_frames_then_enters_movement() {
        assert_eq!(bubble_state(0, 1, 0, 1).map(|s| s.frame), Some(3));
        assert_eq!(bubble_state(4, 1, 0, 1).map(|s| s.frame), Some(4));
        assert_eq!(bubble_state(8, 1, 0, 1).map(|s| s.frame), Some(5));
        assert_eq!(
            bubble_state(12, 1, 0, 1).map(|s| s.phase),
            Some(BubblePhase::Float)
        );
    }

    #[test]
    fn four_float_directions_follow_openttd_deltas() {
        let sw = bubble_state(28, 1, 0, 1).map(|s| (s.x, s.y));
        let ne = bubble_state(28, 1, 1, 1).map(|s| (s.x, s.y));
        let se = bubble_state(28, 1, 2, 1).map(|s| (s.x, s.y));
        let nw = bubble_state(28, 1, 3, 1).map(|s| (s.x, s.y));
        assert_eq!(sw, Some((2, 0)));
        assert_eq!(ne, Some((-2, 0)));
        assert_eq!(se, Some((0, 2)));
        assert_eq!(nw, Some((0, -2)));
    }

    #[test]
    fn absorb_path_reaches_final_frames_and_culls() {
        assert_eq!(
            bubble_state((3 + 80) * 4, 0, 0, 1).map(|s| s.frame),
            Some(10)
        );
        assert_eq!(
            bubble_state((3 + 84) * 4, 0, 0, 1).map(|s| s.frame),
            Some(14)
        );
        assert!(bubble_state((3 + 85) * 4, 0, 0, 1).is_none());
    }

    #[test]
    fn high_bubble_switches_to_burst_then_culls() {
        let state = bubble_state(600, 3, 0, 1);
        assert!(state.is_none(), "debe superar z=180, estallar y borrarse");
    }
}
