use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{CargoType, DecodedSprite, EngineDef, Map, Vehicle, VehicleKind};

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, road_vehicle_tile_anchor, tile_min_z, tile_slope_and_min_z};
use crate::render::{CompanyColoredSprites, MapVisualLayer};
use crate::state::{ClientScreen, SimWorld};
use openttdrs_core::{
    extrapolate_vehicle_pose, slope_dz_at_subtile, vehicle_render_direction_at,
    vehicle_render_direction_at_with_map, vehicle_subtile_at_with_map,
};

use crate::simulation::SimClock;

#[path = "../sprites/vehicle_gfx_data_generated.rs"]
mod vehicle_gfx;

use vehicle_gfx::{
    AIRCRAFT_VEHICLE_LAYERS, AIRCRAFT_VEHICLE_LAYERS_FOKKER, AIRCRAFT_VEHICLE_LAYERS_TRICARIO,
    BUS_VEHICLE_LAYERS, BUS_VEHICLE_LAYERS_LOADED, SHIP_VEHICLE_LAYERS, SHIP_VEHICLE_LAYERS_COAL,
    SHIP_VEHICLE_LAYERS_FERRY, SHIP_VEHICLE_LAYERS_OIL, TRAIN_VEHICLE_LAYERS,
    TRAIN_VEHICLE_LAYERS_T0, TRAIN_VEHICLE_LAYERS_T1, TRAIN_VEHICLE_LAYERS_TDIESEL,
    TRAIN_VEHICLE_LAYERS_TELECTRIC, TRUCK_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS_LOADED,
};

pub(crate) struct VehicleRenderPlugin;

impl Plugin for VehicleRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIndex>()
            .init_resource::<NewGrfTrainSpriteCache>()
            .add_systems(OnEnter(ClientScreen::InGame), rebuild_vehicle_index)
            .add_systems(
                Update,
                (crate::simulation::sync_tick_alpha, update_vehicles)
                    .chain()
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

/// Caché in-world / preview: `(engine_id, view_idx)` → textura.
#[derive(Resource, Default)]
pub(crate) struct NewGrfTrainSpriteCache {
    handles: HashMap<(u16, u8), Handle<Image>>,
}

impl NewGrfTrainSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    fn decoded_to_image(sprite: &DecodedSprite) -> Image {
        Image::new(
            Extent3d {
                width: u32::from(sprite.width),
                height: u32::from(sprite.height),
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            sprite.rgba.clone(),
            TextureFormat::Rgba8UnormSrgb,
            default(),
        )
    }

    /// Textura para la vista `dir` (0..=7) de un motor NewGRF.
    pub(crate) fn handle_for(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let view = engine.newgrf_view(dir)?;
        let view_idx = u8::try_from(dir % engine.newgrf_views.len()).unwrap_or(0);
        let key = (engine.id, view_idx);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| images.add(Self::decoded_to_image(view)))
                .clone(),
        )
    }
}

fn engine_in_sim(sim: &SimWorld, engine_id: u16) -> Option<&EngineDef> {
    openttdrs_core::engine_in_catalog(&sim.state.engine_catalog, engine_id)
        .or_else(|| openttdrs_core::engine_by_id(engine_id))
}

fn train_layers_for(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    let engine_id = v
        .engine_id
        .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
    let engine = openttdrs_core::engine_for_vehicle(v.kind, engine_id);
    match openttdrs_core::train_sprite_group(engine.train_image_index) {
        0 => &TRAIN_VEHICLE_LAYERS_T0,
        1 => &TRAIN_VEHICLE_LAYERS_T1,
        2 => &TRAIN_VEHICLE_LAYERS,
        3 => &TRAIN_VEHICLE_LAYERS_TDIESEL,
        _ => &TRAIN_VEHICLE_LAYERS_TELECTRIC,
    }
}

fn ship_layers_for(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    let engine_id = v
        .engine_id
        .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
    match engine_id {
        openttdrs_core::ENGINE_SHIP_OIL => &SHIP_VEHICLE_LAYERS_OIL,
        openttdrs_core::ENGINE_SHIP_COAL => &SHIP_VEHICLE_LAYERS_COAL,
        openttdrs_core::ENGINE_SHIP_FERRY => &SHIP_VEHICLE_LAYERS_FERRY,
        _ => &SHIP_VEHICLE_LAYERS,
    }
}

fn aircraft_layers_for(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    let engine_id = v
        .engine_id
        .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
    match engine_id {
        openttdrs_core::ENGINE_AIRCRAFT_FOKKER => &AIRCRAFT_VEHICLE_LAYERS_FOKKER,
        openttdrs_core::ENGINE_AIRCRAFT_TRICARIO => &AIRCRAFT_VEHICLE_LAYERS_TRICARIO,
        _ => &AIRCRAFT_VEHICLE_LAYERS,
    }
}

fn vehicle_layers(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    match v.kind {
        VehicleKind::Truck if v.uses_loaded_road_sprite() => &TRUCK_VEHICLE_LAYERS_LOADED,
        VehicleKind::Truck => &TRUCK_VEHICLE_LAYERS,
        VehicleKind::Ship => ship_layers_for(v),
        VehicleKind::Bus | VehicleKind::Tram if v.uses_loaded_road_sprite() => {
            &BUS_VEHICLE_LAYERS_LOADED
        }
        VehicleKind::Bus | VehicleKind::Tram => &BUS_VEHICLE_LAYERS,
        VehicleKind::Aircraft => aircraft_layers_for(v),
        VehicleKind::Train => train_layers_for(v),
    }
}

fn vehicle_layer(
    v: &Vehicle,
    map: Option<&Map>,
    pose: openttdrs_core::VehiclePose,
) -> &'static vehicle_gfx::VehicleLayerGfx {
    let dir = vehicle_render_direction_at_with_map(v, pose, map).min(7) as usize;
    &vehicle_layers(v)[dir]
}

pub(crate) fn vehicle_draw_anchor_from_pose(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
) -> (Vec2, u8, i32, i32) {
    let (tileh, _) = tile_slope_and_min_z(map, pose.pos.x as u32, pose.pos.y as u32);
    let base_z = tile_min_z(map, pose.pos);
    let (sub_x, sub_y) = vehicle_subtile_at_with_map(v, pose, Some(map));
    let sub_z = slope_dz_at_subtile(sub_x, sub_y, tileh);
    let anchor = road_vehicle_tile_anchor(pose.pos.x, pose.pos.y, sub_x, sub_y, sub_z);
    (anchor, base_z, pose.pos.x, pose.pos.y)
}

/// Posición mundo del sprite del vehículo (para cámara de seguimiento).
#[must_use]
pub(crate) fn vehicle_world_position(v: &Vehicle, map: &Map) -> Vec3 {
    vehicle_sprite_pos(v, map, 0.0)
}

pub(crate) fn vehicle_sprite_pos_at(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
) -> Vec3 {
    vehicle_sprite_pos_at_with_catalog(v, map, pose, None)
}

pub(crate) fn vehicle_sprite_pos_at_with_catalog(
    v: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    catalog: Option<&[EngineDef]>,
) -> Vec3 {
    let dir = vehicle_render_direction_at_with_map(v, pose, Some(map)).min(7) as usize;
    let (x_offs, y_offs, w, h) = if v.kind == VehicleKind::Train
        && let Some(cat) = catalog
        && let Some(eid) = v.engine_id
        && let Some(eng) = openttdrs_core::engine_in_catalog(cat, eid)
        && let Some(view) = eng.newgrf_view(dir)
    {
        (
            f32::from(view.x_offs),
            f32::from(view.y_offs),
            f32::from(view.width),
            f32::from(view.height),
        )
    } else {
        let layer = vehicle_layer(v, Some(map), pose);
        (layer.x_offs, layer.y_offs, layer.w, layer.h)
    };
    let (anchor, height, tx, ty) = vehicle_draw_anchor_from_pose(v, map, pose);
    let height = height.saturating_add(v.altitude);
    overlay_pos(anchor, x_offs, y_offs, w, h, height, 1.0, tx, ty)
}

pub(crate) fn vehicle_sprite_pos(v: &Vehicle, map: &Map, tick_alpha: f32) -> Vec3 {
    vehicle_sprite_pos_at(v, map, extrapolate_vehicle_pose(v, tick_alpha))
}

type DirHandles = [Handle<Image>; 8];

#[derive(Resource)]
pub(crate) struct TruckHandles {
    bus: DirHandles,
    bus_loaded: DirHandles,
    truck: DirHandles,
    truck_loaded: DirHandles,
    ship: DirHandles,
    ship_oil: DirHandles,
    ship_coal: DirHandles,
    ship_ferry: DirHandles,
    aircraft: DirHandles,
    aircraft_fokker: DirHandles,
    aircraft_tricario: DirHandles,
    train_groups: [DirHandles; 5],
}

impl TruckHandles {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        fn load_set(
            server: &AssetServer,
            layers: &[vehicle_gfx::VehicleLayerGfx; 8],
        ) -> DirHandles {
            [
                server.load(layers[0].path),
                server.load(layers[1].path),
                server.load(layers[2].path),
                server.load(layers[3].path),
                server.load(layers[4].path),
                server.load(layers[5].path),
                server.load(layers[6].path),
                server.load(layers[7].path),
            ]
        }
        Self {
            bus: load_set(asset_server, &BUS_VEHICLE_LAYERS),
            bus_loaded: load_set(asset_server, &BUS_VEHICLE_LAYERS_LOADED),
            truck: load_set(asset_server, &TRUCK_VEHICLE_LAYERS),
            truck_loaded: load_set(asset_server, &TRUCK_VEHICLE_LAYERS_LOADED),
            ship: load_set(asset_server, &SHIP_VEHICLE_LAYERS),
            ship_oil: load_set(asset_server, &SHIP_VEHICLE_LAYERS_OIL),
            ship_coal: load_set(asset_server, &SHIP_VEHICLE_LAYERS_COAL),
            ship_ferry: load_set(asset_server, &SHIP_VEHICLE_LAYERS_FERRY),
            aircraft: load_set(asset_server, &AIRCRAFT_VEHICLE_LAYERS),
            aircraft_fokker: load_set(asset_server, &AIRCRAFT_VEHICLE_LAYERS_FOKKER),
            aircraft_tricario: load_set(asset_server, &AIRCRAFT_VEHICLE_LAYERS_TRICARIO),
            train_groups: [
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_T0),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_T1),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_TDIESEL),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_TELECTRIC),
            ],
        }
    }

    pub(crate) fn intro_sprite(&self, kind: VehicleKind, dir: usize) -> Handle<Image> {
        let i = dir.min(7);
        match kind {
            VehicleKind::Bus | VehicleKind::Tram => self.bus[i].clone(),
            VehicleKind::Aircraft => self.aircraft[i].clone(),
            VehicleKind::Train => self.train_groups[2][i].clone(),
            VehicleKind::Truck => self.truck[i].clone(),
            VehicleKind::Ship => self.ship[i].clone(),
        }
    }

    pub(crate) fn intro_sprite_for_engine(
        &self,
        engine: &openttdrs_core::EngineDef,
        dir: usize,
    ) -> Handle<Image> {
        let i = dir.min(7);
        match engine.kind {
            VehicleKind::Ship => match engine.id {
                openttdrs_core::ENGINE_SHIP_OIL => self.ship_oil[i].clone(),
                openttdrs_core::ENGINE_SHIP_COAL => self.ship_coal[i].clone(),
                openttdrs_core::ENGINE_SHIP_FERRY => self.ship_ferry[i].clone(),
                _ => self.ship[i].clone(),
            },
            VehicleKind::Aircraft => match engine.id {
                openttdrs_core::ENGINE_AIRCRAFT_FOKKER => self.aircraft_fokker[i].clone(),
                openttdrs_core::ENGINE_AIRCRAFT_TRICARIO => self.aircraft_tricario[i].clone(),
                _ => self.aircraft[i].clone(),
            },
            other => self.intro_sprite(other, dir),
        }
    }

    pub(crate) fn train_preview(&self, image_index: u8, dir: usize) -> Handle<Image> {
        let group = openttdrs_core::train_sprite_group(image_index).min(4) as usize;
        self.train_groups[group][dir.min(7)].clone()
    }

    /// Textura del sprite según la pose de render (extrapolada entre ticks de
    /// sim): la dirección del sprite acompaña la posición dibujada en curvas,
    /// en vez de usar la dirección lógica del último tick.
    fn for_vehicle(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        company: Option<&CompanyColoredSprites>,
    ) -> Handle<Image> {
        let dir = vehicle_render_direction_at(v, pose).min(7) as usize;
        let layer = &vehicle_layers(v)[dir];
        if let Some(c) = company
            && let Some(handle) = c.vehicle_handle(layer.path)
        {
            return handle.clone();
        }
        let i = dir;
        match v.kind {
            VehicleKind::Truck if v.uses_loaded_road_sprite() => self.truck_loaded[i].clone(),
            VehicleKind::Truck => self.truck[i].clone(),
            VehicleKind::Ship => {
                let engine_id = v
                    .engine_id
                    .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
                match engine_id {
                    openttdrs_core::ENGINE_SHIP_OIL => self.ship_oil[i].clone(),
                    openttdrs_core::ENGINE_SHIP_COAL => self.ship_coal[i].clone(),
                    openttdrs_core::ENGINE_SHIP_FERRY => self.ship_ferry[i].clone(),
                    _ => self.ship[i].clone(),
                }
            }
            VehicleKind::Bus | VehicleKind::Tram if v.uses_loaded_road_sprite() => {
                self.bus_loaded[i].clone()
            }
            VehicleKind::Bus | VehicleKind::Tram => self.bus[i].clone(),
            VehicleKind::Aircraft => {
                let engine_id = v
                    .engine_id
                    .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
                match engine_id {
                    openttdrs_core::ENGINE_AIRCRAFT_FOKKER => self.aircraft_fokker[i].clone(),
                    openttdrs_core::ENGINE_AIRCRAFT_TRICARIO => self.aircraft_tricario[i].clone(),
                    _ => self.aircraft[i].clone(),
                }
            }
            VehicleKind::Train => {
                let engine_id = v
                    .engine_id
                    .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
                let engine = openttdrs_core::engine_for_vehicle(v.kind, engine_id);
                let group =
                    openttdrs_core::train_sprite_group(engine.train_image_index).min(4) as usize;
                self.train_groups[group][i].clone()
            }
        }
    }

    /// Textura del vehículo; prioriza vistas NewGRF del catálogo runtime.
    fn for_vehicle_with_newgrf(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        company: Option<&CompanyColoredSprites>,
        sim: &SimWorld,
        cache: &mut NewGrfTrainSpriteCache,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let dir = vehicle_render_direction_at(v, pose).min(7) as usize;
        if v.kind == VehicleKind::Train
            && let Some(eid) = v.engine_id
            && let Some(eng) = engine_in_sim(sim, eid)
            && let Some(handle) = cache.handle_for(eng, dir, images)
        {
            return handle;
        }
        self.for_vehicle(v, pose, company)
    }
}

/// Índice `Vehicle.id` → posición en `GameState::vehicles` (evita `find` O(V) por sprite).
#[derive(Resource, Default)]
pub(crate) struct VehicleIndex {
    by_id: HashMap<u32, usize>,
}

impl VehicleIndex {
    pub(crate) fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_id.clear();
        self.by_id.reserve(vehicles.len());
        for (i, v) in vehicles.iter().enumerate() {
            self.by_id.insert(v.id, i);
        }
    }
}

pub(crate) fn rebuild_vehicle_index(sim: Res<SimWorld>, mut idx: ResMut<VehicleIndex>) {
    idx.rebuild(&sim.state.vehicles);
}

pub(crate) fn spawn_initial_vehicles(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
    company: &CompanyColoredSprites,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
) {
    for vehicle in &sim.state.vehicles {
        // Vagones enganchados: se dibujan como partes del consist (offsets), no como entidad propia.
        if vehicle.is_wagon_unit() {
            continue;
        }
        let pose = extrapolate_vehicle_pose(vehicle, 0.0);
        let pos3 = vehicle_sprite_pos_at_with_catalog(
            vehicle,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let vis = if vehicle_is_hidden_in_depot(sim, vehicle) {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        commands.spawn((
            MapVisualLayer,
            VehicleSprite(vehicle.id),
            Sprite {
                image: trucks.for_vehicle_with_newgrf(
                    vehicle,
                    pose,
                    Some(company),
                    sim,
                    cache,
                    images,
                ),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(pos3),
            vis,
        ));
        // Unidades del consist detrás de la cabeza (sprites desplazados).
        if vehicle.kind == VehicleKind::Train {
            spawn_consist_trailer_sprites(
                commands, sim, trucks, company, vehicle, pose, vis, cache, images,
            );
        }
        if !crate::sprites::is_hidden(crate::sprites::TransparencyOption::Text) {
            commands.spawn((
                MapVisualLayer,
                VehicleCargoLabel(vehicle.id),
                Text2d::new(vehicle_cargo_label(vehicle)),
                TextFont {
                    font_size: FontSize::Px(8.0),
                    ..default()
                },
                TextColor(crate::sprites::text_color(
                    crate::sprites::TransparencyOption::Text,
                    vehicle_cargo_color(vehicle),
                )),
                Transform::from_translation(vehicle_cargo_label_pos(pos3)),
                vis,
            ));
        }
    }
}

/// Sprite de vagón enganchado (mismo id de cabeza + offset visual).
#[derive(Component)]
pub(crate) struct ConsistUnitSprite {
    head_id: u32,
    unit_index: usize,
}

#[expect(clippy::too_many_arguments)]
fn spawn_consist_trailer_sprites(
    commands: &mut Commands,
    sim: &SimWorld,
    trucks: &TruckHandles,
    company: &CompanyColoredSprites,
    head: &Vehicle,
    pose: openttdrs_core::VehiclePose,
    vis: Visibility,
    cache: &mut NewGrfTrainSpriteCache,
    images: &mut Assets<Image>,
) {
    let ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, head.id);
    for (i, &uid) in ids.iter().enumerate().skip(1) {
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == uid) else {
            continue;
        };
        let unit_pose = pose;
        // Offset atrás ~media tesela por unidad (aproximación visual).
        let back = openttdrs_core::reverse_direction(head.direction);
        let (dx, dy) = match back {
            0 => (0.0, -8.0),
            1 => (6.0, -4.0),
            2 => (8.0, 0.0),
            3 => (6.0, 4.0),
            4 => (0.0, 8.0),
            5 => (-6.0, 4.0),
            6 => (-8.0, 0.0),
            _ => (-6.0, -4.0),
        };
        let base = vehicle_sprite_pos_at_with_catalog(
            head,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let fi = i as f32;
        commands.spawn((
            MapVisualLayer,
            ConsistUnitSprite {
                head_id: head.id,
                unit_index: i,
            },
            Sprite {
                image: trucks.for_vehicle_with_newgrf(
                    unit,
                    unit_pose,
                    Some(company),
                    sim,
                    cache,
                    images,
                ),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(Vec3::new(
                base.x + dx * fi,
                base.y + dy * fi,
                base.z - 0.01 * fi,
            )),
            vis,
        ));
    }
}

#[derive(Component)]
pub(crate) struct VehicleSprite(u32);

#[derive(Component)]
pub(crate) struct VehicleCargoLabel(u32);

fn vehicle_cargo_label(v: &Vehicle) -> String {
    let cargo = match v.cargo_type {
        Some(CargoType::Passengers) => "PAX",
        Some(CargoType::Mail) => "MAIL",
        Some(CargoType::Goods) => "GOODS",
        Some(CargoType::Coal) => "COAL",
        Some(CargoType::Wood) => "WOOD",
        Some(CargoType::Oil) => "OIL",
        None => "ANY",
    };
    format!("{cargo} {}/{}", v.cargo, v.capacity)
}

fn vehicle_cargo_color(v: &Vehicle) -> Color {
    if v.cargo > 0 {
        Color::srgb(0.95, 0.9, 0.35)
    } else {
        Color::srgba(0.8, 0.85, 0.9, 0.72)
    }
}

fn vehicle_cargo_label_pos(vehicle_pos: Vec3) -> Vec3 {
    Vec3::new(vehicle_pos.x, vehicle_pos.y + 21.0, vehicle_pos.z + 0.35)
}

fn vehicle_tint(v: &Vehicle) -> Color {
    if v.pbs_stuck {
        Color::srgb(1.0, 0.75, 0.35)
    } else {
        Color::WHITE
    }
}

fn vehicle_is_hidden_in_depot(sim: &SimWorld, v: &Vehicle) -> bool {
    openttdrs_core::vehicle_hidden_on_map(&sim.state.map, v)
}

/// Radio de picking en unidades mundo (clic sobre el sprite del vehículo).
const VEHICLE_PICK_RADIUS_SQ: f32 = 34.0 * 34.0;

/// Vehículo visible bajo el cursor (prioriza el sprite más cercano).
#[must_use]
pub(crate) fn pick_vehicle_id_at_world(world_pos: Vec2, sim: &SimWorld) -> Option<u32> {
    sim.state
        .vehicles
        .iter()
        .filter(|v| !vehicle_is_hidden_in_depot(sim, v))
        .filter_map(|v| {
            let sprite_xy = vehicle_sprite_pos(v, &sim.state.map, 0.0).truncate();
            let dist_sq = sprite_xy.distance_squared(world_pos);
            (dist_sq <= VEHICLE_PICK_RADIUS_SQ).then_some((dist_sq, v.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn update_vehicles(
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    trucks: Res<TruckHandles>,
    company: Res<CompanyColoredSprites>,
    vehicle_index: Res<VehicleIndex>,
    mut cache: ResMut<NewGrfTrainSpriteCache>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite, &mut Visibility)>,
    mut trailers: Query<
        (
            &ConsistUnitSprite,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        Without<VehicleSprite>,
    >,
    mut labels: Query<
        (
            &VehicleCargoLabel,
            &mut Transform,
            &mut Text2d,
            &mut TextColor,
            &mut Visibility,
        ),
        (Without<VehicleSprite>, Without<ConsistUnitSprite>),
    >,
) {
    for (vs, mut transform, mut sprite, mut visibility) in &mut q {
        let Some(&i) = vehicle_index.by_id.get(&vs.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        if vehicle_is_hidden_in_depot(&sim, v) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pose = extrapolate_vehicle_pose(v, sim_clock.tick_alpha);
        let pos3 = vehicle_sprite_pos_at_with_catalog(
            v,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        transform.translation = pos3;
        sprite.image =
            trucks.for_vehicle_with_newgrf(v, pose, Some(&company), &sim, &mut cache, &mut images);
        sprite.color = vehicle_tint(v);
    }

    for (trailer, mut transform, mut sprite, mut visibility) in &mut trailers {
        let Some(&i) = vehicle_index.by_id.get(&trailer.head_id) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(head) = sim.state.vehicles.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let ids = openttdrs_core::consist_unit_ids(&sim.state.vehicles, head.id);
        let Some(&uid) = ids.get(trailer.unit_index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == uid) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if vehicle_is_hidden_in_depot(&sim, head) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pose = extrapolate_vehicle_pose(head, sim_clock.tick_alpha);
        let base = vehicle_sprite_pos_at_with_catalog(
            head,
            &sim.state.map,
            pose,
            Some(&sim.state.engine_catalog),
        );
        let back = openttdrs_core::reverse_direction(head.direction);
        let (dx, dy) = match back {
            0 => (0.0, -8.0),
            1 => (6.0, -4.0),
            2 => (8.0, 0.0),
            3 => (6.0, 4.0),
            4 => (0.0, 8.0),
            5 => (-6.0, 4.0),
            6 => (-8.0, 0.0),
            _ => (-6.0, -4.0),
        };
        let i = trailer.unit_index as f32;
        transform.translation = Vec3::new(base.x + dx * i, base.y + dy * i, base.z - 0.01 * i);
        sprite.image = trucks.for_vehicle_with_newgrf(
            unit,
            pose,
            Some(&company),
            &sim,
            &mut cache,
            &mut images,
        );
        sprite.color = vehicle_tint(head);
    }

    for (label, mut transform, mut text, mut color, mut visibility) in &mut labels {
        let Some(&i) = vehicle_index.by_id.get(&label.0) else {
            continue;
        };
        let Some(v) = sim.state.vehicles.get(i) else {
            continue;
        };
        if vehicle_is_hidden_in_depot(&sim, v) {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let pos3 = vehicle_sprite_pos(v, &sim.state.map, sim_clock.tick_alpha);
        transform.translation = vehicle_cargo_label_pos(pos3);
        **text = vehicle_cargo_label(v);
        color.0 = vehicle_cargo_color(v);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{DIR_S, DIR_SW, GameState, TileCoord, TileKind, VehicleKind};

    fn sample_vehicle(id: u32) -> Vehicle {
        let dest = TileCoord::new(2, 1);
        let mut v = Vehicle::new(id, VehicleKind::Truck, TileCoord::new(1, 1), dest);
        v.path = VecDeque::from([dest]);
        v.direction = DIR_SW;
        v.engine_id = Some(openttdrs_core::ENGINE_TRUCK_MPS);
        v.cur_speed = 96;
        v
    }

    fn default_handles() -> TruckHandles {
        TruckHandles {
            bus: Default::default(),
            bus_loaded: Default::default(),
            truck: Default::default(),
            truck_loaded: Default::default(),
            ship: Default::default(),
            ship_oil: Default::default(),
            ship_coal: Default::default(),
            ship_ferry: Default::default(),
            aircraft: Default::default(),
            aircraft_fokker: Default::default(),
            aircraft_tricario: Default::default(),
            train_groups: Default::default(),
        }
    }

    #[test]
    fn pick_vehicle_prefers_closest_sprite() {
        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let on_road = TileCoord::new(4, 4);
        sim.state
            .map
            .set_kind(on_road, TileKind::Road)
            .expect("road tile");
        sim.state
            .vehicles
            .push(Vehicle::new(42, VehicleKind::Bus, on_road, on_road));
        let anchor = vehicle_sprite_pos(&sim.state.vehicles[0], &sim.state.map, 0.0).truncate();
        assert_eq!(pick_vehicle_id_at_world(anchor, &sim), Some(42));
        assert_eq!(
            pick_vehicle_id_at_world(anchor + Vec2::new(200.0, 0.0), &sim),
            None
        );
    }

    #[test]
    fn vehicle_index_and_sprite_helpers_work() {
        let mut idx = VehicleIndex::default();
        let v = sample_vehicle(7);
        idx.rebuild(std::slice::from_ref(&v));
        assert_eq!(idx.by_id.get(&7), Some(&0));
        assert_eq!(v.render_direction(), DIR_SW);
        assert_ne!(vehicle_layers(&v)[1].path, vehicle_layers(&v)[3].path);
        assert!(!v.uses_loaded_road_sprite());
        let mut loaded = sample_vehicle(1);
        loaded.cargo = 15;
        assert!(loaded.uses_loaded_road_sprite());
        let mut empty_bus = sample_vehicle(2);
        empty_bus.kind = VehicleKind::Bus;
        let mut loaded_bus = sample_vehicle(3);
        loaded_bus.kind = VehicleKind::Bus;
        loaded_bus.cargo = 15;
        assert!(loaded_bus.uses_loaded_road_sprite());
        assert_ne!(
            vehicle_layers(&empty_bus)[5].path,
            vehicle_layers(&loaded_bus)[5].path
        );
    }

    #[test]
    fn vehicle_tint_amber_when_pbs_stuck() {
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Train;
        assert_eq!(vehicle_tint(&v), Color::WHITE);
        v.pbs_stuck = true;
        assert_eq!(vehicle_tint(&v), Color::srgb(1.0, 0.75, 0.35));
    }

    #[test]
    fn stopped_train_in_rail_depot_is_hidden_from_pick() {
        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let depot = TileCoord::new(5, 5);
        sim.state
            .map
            .set_kind(depot, TileKind::RailDepot)
            .expect("rail depot");
        let mut train = Vehicle::new(9, VehicleKind::Train, depot, depot);
        train.running = false;
        sim.state.vehicles.push(train);
        let anchor = vehicle_sprite_pos(&sim.state.vehicles[0], &sim.state.map, 0.0).truncate();
        assert!(vehicle_is_hidden_in_depot(&sim, &sim.state.vehicles[0]));
        assert_eq!(pick_vehicle_id_at_world(anchor, &sim), None);
    }

    #[test]
    fn rebuild_and_update_systems_run() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            loaded_file: false,
            ottdmap_extras: None,
        };
        sim.state.vehicles.push(sample_vehicle(11));

        let mut world = World::new();
        world.insert_resource(sim);
        world.insert_resource(crate::simulation::SimClock::default());
        world.insert_resource(default_handles());
        world.insert_resource(crate::sprites::CompanyColoredSprites::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(NewGrfTrainSpriteCache::default());
        world.init_resource::<Assets<Image>>();

        world.spawn((
            VehicleSprite(11),
            Transform::default(),
            Sprite::default(),
            Visibility::Visible,
        ));
        world.spawn((
            VehicleCargoLabel(11),
            Transform::default(),
            Text2d::new(""),
            TextColor(Color::WHITE),
            Visibility::Visible,
        ));

        world.run_system_once(rebuild_vehicle_index).unwrap();
        world.run_system_once(update_vehicles).unwrap();

        let mut labels = world.query_filtered::<&Text2d, With<VehicleCargoLabel>>();
        assert_eq!(labels.single(&world).unwrap().to_string(), "ANY 0/20");
    }

    #[test]
    fn newgrf_train_sprite_cache_and_pos_use_decoded_views() {
        use openttdrs_core::{
            apply_newgrf_vehicles_trains, build_action0_train_payload,
            build_grf_v2_train_with_preview_sprite,
        };

        let a0 = build_action0_train_payload(1960, 100, 800, "InWorld Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'I', 0, 1],
            "tinworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("tinworld.grf"), &bytes).expect("write grf");
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("tinworld.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[dir.path()]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf)
            .expect("newgrf engine")
            .clone();
        assert!(!eng.newgrf_views.is_empty());

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfTrainSpriteCache::default();
        let handle = cache
            .handle_for(&eng, 0, &mut images)
            .expect("newgrf texture");
        let handle_again = cache
            .handle_for(&eng, 3, &mut images)
            .expect("reuse single view");
        assert_eq!(handle, handle_again);
        assert_eq!(cache.handles.len(), 1);

        let mut v = sample_vehicle(99);
        v.kind = VehicleKind::Train;
        v.engine_id = Some(eng.id);
        let pose = extrapolate_vehicle_pose(&v, 0.0);
        let map = &state.map;
        let pos_vanilla = vehicle_sprite_pos_at(&v, map, pose);
        let pos_newgrf =
            vehicle_sprite_pos_at_with_catalog(&v, map, pose, Some(&state.engine_catalog));
        assert_ne!(
            (pos_vanilla.x, pos_vanilla.y),
            (pos_newgrf.x, pos_newgrf.y),
            "offsets NewGRF deben mover el sprite vs OpenGFX"
        );

        let sim = SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        };
        let trucks = default_handles();
        let selected =
            trucks.for_vehicle_with_newgrf(&v, pose, None, &sim, &mut cache, &mut images);
        assert_eq!(selected, handle);
    }

    #[test]
    fn train_layers_differ_from_bus() {
        assert_ne!(
            TRAIN_VEHICLE_LAYERS[DIR_SW as usize].path,
            BUS_VEHICLE_LAYERS[DIR_SW as usize].path
        );
    }

    #[test]
    fn sprite_selection_uses_extrapolated_pose_not_logical_direction() {
        // Bus a mitad de una curva NE→SE: el estado lógico está antes del punto
        // medio (sprite diagonal NE), pero la pose extrapolada al final del
        // frame ya cruzó progress 128 (sprite cardinal E). El selector debe
        // usar la pose extrapolada.
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Bus;
        v.pos = TileCoord::new(1, 1);
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(0, 2)]);
        v.set_cruise_speed();
        v.progress = 100;

        let logical_dir = v.render_direction().min(7) as usize;
        assert_eq!(logical_dir, openttdrs_core::DIR_NE as usize);

        let pose = extrapolate_vehicle_pose(&v, 1.0);
        assert!(
            pose.progress >= 128,
            "la extrapolación cruza el punto medio"
        );
        let render_dir = vehicle_render_direction_at(&v, pose).min(7) as usize;
        assert_eq!(render_dir, openttdrs_core::DIR_E as usize);

        // `for_vehicle` y `vehicle_layer` seleccionan la textura del sprite
        // cardinal (pose extrapolada), no la diagonal lógica.
        assert_eq!(
            vehicle_layer(&v, None, pose).path,
            vehicle_layers(&v)[render_dir].path
        );
        assert_ne!(
            vehicle_layer(&v, None, pose).path,
            vehicle_layers(&v)[logical_dir].path
        );

        let handles = default_handles();
        let selected = handles.for_vehicle(&v, pose, None);
        assert_eq!(selected, handles.bus[render_dir]);
    }

    #[test]
    fn sprite_selection_uses_extrapolated_pose_for_train() {
        use openttdrs_core::vehicle_subtile_at;

        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Train;
        v.pos = TileCoord::new(5, 6);
        v.path = VecDeque::from([TileCoord::new(6, 6)]);
        v.direction = openttdrs_core::DIR_NE;
        v.set_cruise_speed();
        v.progress = 40;

        let logical_pose = extrapolate_vehicle_pose(&v, 0.0);
        let extrap_pose = extrapolate_vehicle_pose(&v, 1.0);
        assert!(
            extrap_pose.progress > logical_pose.progress || extrap_pose.pos != logical_pose.pos,
            "la extrapolación avanza el tren entre ticks"
        );
        let logical_sub = vehicle_subtile_at(&v, logical_pose);
        let extrap_sub = vehicle_subtile_at(&v, extrap_pose);
        assert_ne!(
            logical_sub, extrap_sub,
            "sub-tesela extrapolada distinta de la lógica"
        );
        assert_eq!(
            vehicle_layer(&v, None, extrap_pose).path,
            vehicle_layers(&v)[vehicle_render_direction_at(&v, extrap_pose).min(7) as usize].path
        );
    }

    #[test]
    fn render_direction_cardinal_layer_differs_from_diagonal() {
        let mut v = sample_vehicle(1);
        v.kind = VehicleKind::Bus;
        v.pos = TileCoord::new(0, 0);
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(1, 1)]);
        v.progress = 200;
        assert_eq!(v.render_direction(), DIR_S);
        assert_ne!(
            BUS_VEHICLE_LAYERS[DIR_S as usize].path,
            BUS_VEHICLE_LAYERS[openttdrs_core::DIR_SE as usize].path
        );
    }
}
