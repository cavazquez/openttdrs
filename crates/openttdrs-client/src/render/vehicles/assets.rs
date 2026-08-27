use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::EngineDef;
use openttdrs_core::prelude::*;

use crate::render::CompanyColoredSprites;
use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};
use crate::sprites::CompanyColour;

#[path = "../../sprites/vehicle_gfx_data_generated.rs"]
#[cfg(not(test))]
mod vehicle_gfx;

#[path = "../../sprites/vehicle_gfx_data_generated.rs"]
#[cfg(test)]
pub(crate) mod vehicle_gfx;

use vehicle_gfx::{
    AIRCRAFT_VEHICLE_LAYERS, AIRCRAFT_VEHICLE_LAYERS_FOKKER, AIRCRAFT_VEHICLE_LAYERS_TRICARIO,
    BUS_VEHICLE_LAYERS, BUS_VEHICLE_LAYERS_LOADED, SHIP_VEHICLE_LAYERS, SHIP_VEHICLE_LAYERS_COAL,
    SHIP_VEHICLE_LAYERS_FERRY, SHIP_VEHICLE_LAYERS_OIL, TRAIN_VEHICLE_LAYERS,
    TRAIN_VEHICLE_LAYERS_T0, TRAIN_VEHICLE_LAYERS_T1, TRAIN_VEHICLE_LAYERS_TDIESEL,
    TRAIN_VEHICLE_LAYERS_TELECTRIC, TRAIN_WAGON_COAL_LAYERS, TRAIN_WAGON_COAL_LOADED_LAYERS,
    TRAIN_WAGON_GOODS_LAYERS, TRAIN_WAGON_MAIL_LAYERS, TRAIN_WAGON_PASSENGER_LAYERS,
    TRUCK_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS_LOADED,
};

/// Una entrada de la secuencia visual que devuelve `GetCustomVehicleSprite`.
///
/// Los offsets forman parte del sprite, no del parent: una capa apilada puede
/// tener un origen distinto aunque comparta dirección y vehículo.
#[derive(Clone)]
pub(crate) struct NewGrfVehicleLayer {
    pub handle: Handle<Image>,
    pub x_offs: i16,
    pub y_offs: i16,
    pub width: u16,
    pub height: u16,
}

pub(crate) use vehicle_gfx::{AIRCRAFT_ROTOR_LAYERS, VehicleLayerGfx};

const TRAIN_GROUP_COUNT: usize = 10;
const TRAIN_GROUP_COAL_LOADED: usize = 9;

fn train_group_for_vehicle(v: &Vehicle) -> usize {
    let engine_id = v
        .engine_id
        .unwrap_or_else(|| openttdrs_core::default_engine_id(v.kind));
    let engine = openttdrs_core::engine_for_vehicle(v.kind, engine_id);
    if engine_id == openttdrs_core::ENGINE_WAGON_COAL
        && v.capacity > 0
        && v.cargo.saturating_mul(2) >= v.capacity
    {
        TRAIN_GROUP_COAL_LOADED
    } else {
        usize::from(openttdrs_core::train_sprite_group(engine.train_image_index))
            .min(TRAIN_GROUP_COUNT - 2)
    }
}

fn train_layers_for(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
    match train_group_for_vehicle(v) {
        0 => &TRAIN_VEHICLE_LAYERS_T0,
        1 => &TRAIN_VEHICLE_LAYERS_T1,
        2 => &TRAIN_VEHICLE_LAYERS,
        3 => &TRAIN_VEHICLE_LAYERS_TDIESEL,
        4 => &TRAIN_VEHICLE_LAYERS_TELECTRIC,
        5 => &TRAIN_WAGON_PASSENGER_LAYERS,
        6 => &TRAIN_WAGON_MAIL_LAYERS,
        7 => &TRAIN_WAGON_GOODS_LAYERS,
        8 => &TRAIN_WAGON_COAL_LAYERS,
        _ => &TRAIN_WAGON_COAL_LOADED_LAYERS,
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

pub(crate) fn vehicle_layers(v: &Vehicle) -> &'static [vehicle_gfx::VehicleLayerGfx; 8] {
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

/// Devuelve el ID local del motor que encabeza la cadena de un vehículo.
///
/// `OpenTTD` guarda este valor como `first_engine` en la caché de vehículos;
/// aquí se reconstruye desde `prev_unit` porque la caché del cliente es
/// efímera y no forma parte de `SimWorld`.
fn overriding_engine_local_id(
    sim: &crate::state::SimWorld,
    vehicle: &Vehicle,
    wagon_engine: &EngineDef,
) -> Option<u16> {
    if wagon_engine.newgrf_grfid == 0 {
        return None;
    }
    let mut current_id = vehicle.id;
    let mut head_engine_id = None;
    for _ in 0..256 {
        let current = if current_id == vehicle.id {
            Some(vehicle)
        } else {
            sim.state
                .vehicles
                .iter()
                .find(|candidate| candidate.id == current_id)
        }?;
        match current.prev_unit {
            Some(previous_id) if previous_id != current.id => current_id = previous_id,
            _ => {
                head_engine_id = current.engine_id;
                break;
            }
        }
    }
    let head_engine_id = head_engine_id?;
    let head_engine = super::engine_in_sim(sim, head_engine_id)?;
    (head_engine.newgrf_grfid == wagon_engine.newgrf_grfid).then_some(head_engine.newgrf_local_id)
}

/// Caché in-world / preview: `(engine_id, view_idx, company_colour)` → textura.
#[derive(Resource, Default)]
pub(crate) struct NewGrfTrainSpriteCache {
    /// `(engine_id, view_idx, colour, stack, palette, runtime_fp)` → textura.
    handles: HashMap<(u16, u8, u8, u8, u16, u32), Handle<Image>>,
}

impl NewGrfTrainSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.handles.len()
    }

    /// Textura para la vista `dir` (0..=7) de un motor NewGRF (vistas horneadas).
    pub(crate) fn handle_for(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        colour: CompanyColour,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let view = engine.newgrf_view(dir)?;
        let view_idx = u8::try_from(dir % engine.newgrf_views.len()).unwrap_or(0);
        let key = (engine.id, view_idx, colour.as_u8(), 0, 0, 0);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| {
                    images.add(decoded_sprite_image(
                        view,
                        DecodedSpriteImagePolicy::Masked { colour },
                    ))
                })
                .clone(),
        )
    }

    /// Textura re-resolviendo Action2 con bits del vehículo / consist.
    #[allow(dead_code)]
    pub(crate) fn handle_for_runtime(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        cargo: Option<openttdrs_core::CargoType>,
        colour: CompanyColour,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        self.handles_for_runtime(engine, dir, cargo, colour, ctx, images)
            .into_iter()
            .next()
            .map(|layer| layer.handle)
    }

    /// Resuelve la secuencia visual de un vehículo, incluyendo las capas que
    /// `EngineMiscFlag::SpriteStack` pide repetir con `var 10` alto.
    ///
    /// El registro `100h` que indica el final de la secuencia sólo existe en
    /// callbacks completos. Mientras ese callback siga fuera del subconjunto,
    /// se usa el contrato seguro de OpenTTD: como máximo ocho capas y se corta
    /// en la primera vista repetida. Esto evita duplicar una vista estática sin
    /// inventar sprites cuando el GRF no implementa stack.
    pub(crate) fn handles_for_runtime(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        cargo: Option<openttdrs_core::CargoType>,
        colour: CompanyColour,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Vec<NewGrfVehicleLayer> {
        self.handles_for_runtime_with_override(engine, dir, cargo, colour, None, None, ctx, images)
    }

    /// Igual que [`Self::handles_for_runtime`], pero aplicando el motor que
    /// encabeza el consist para resolver los *wagon overrides* de Action3.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handles_for_runtime_with_override(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        cargo: Option<openttdrs_core::CargoType>,
        colour: CompanyColour,
        overriding_local_id: Option<u16>,
        palette_override: Option<u16>,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Vec<NewGrfVehicleLayer> {
        let Some(runtime) = engine.newgrf_runtime.as_ref() else {
            return Vec::new();
        };
        let max_stack = if engine.sprite_stack { 8 } else { 1 };
        let mut layers = Vec::with_capacity(max_stack);
        let base_ctx = ctx.clone();
        let mut previous: Option<openttdrs_core::DecodedSprite> = None;
        for stack in 0..max_stack {
            let mut stack_ctx = base_ctx.clone();
            // EIT_ON_MAP = 0; the high byte is the sprite-stack index.
            stack_ctx
                .vars
                .insert(0x10, u32::try_from(stack).unwrap_or(0) << 8);
            let views = overriding_local_id
                .and_then(|overriding_id| {
                    runtime.views_for_wagon_override_u16_ctx(
                        engine.newgrf_local_id,
                        overriding_id,
                        cargo,
                        &mut stack_ctx,
                    )
                })
                .or_else(|| {
                    runtime.views_for_local_id_cargo_u16_ctx(
                        engine.newgrf_local_id,
                        cargo,
                        &mut stack_ctx,
                    )
                });
            let register_100 = engine
                .sprite_stack
                .then(|| stack_ctx.registers_100.get(&0x100).copied())
                .flatten();
            let palette_id = palette_override
                .or_else(|| register_100.and_then(|value| u16::try_from(value & 0xFFFF).ok()))
                .unwrap_or(0);
            let Some(views) = views else {
                if register_100.is_some_and(|value| value & 0x8000_0000 != 0) {
                    continue;
                }
                break;
            };
            if views.is_empty() {
                if register_100.is_some_and(|value| value & 0x8000_0000 != 0) {
                    continue;
                }
                break;
            }
            let view = views[dir % views.len()].clone();
            // GRFs that implement SpriteStack explicitly set bit 31 in
            // register 0x100 while another layer follows. Legacy fixtures do
            // not write the register, so retain the repeated-view guard only
            // for that fallback path.
            if stack > 0 && register_100.is_none() && previous.as_ref() == Some(&view) {
                break;
            }
            let view_idx = u8::try_from(dir % views.len()).unwrap_or(0);
            let fp = runtime_fingerprint(&stack_ctx, vars::TRAIN, true);
            let stack_idx = u8::try_from(stack).unwrap_or(u8::MAX);
            let key = (
                engine.id,
                view_idx,
                colour.as_u8(),
                stack_idx,
                palette_id,
                fp,
            );
            let image_policy = if palette_id == 0 {
                if palette_override.is_some() {
                    DecodedSpriteImagePolicy::Raw
                } else {
                    DecodedSpriteImagePolicy::Masked { colour }
                }
            } else if (775..=790).contains(&palette_id) {
                let palette_colour =
                    CompanyColour::from_u8(u8::try_from(palette_id - 775).unwrap_or(0));
                DecodedSpriteImagePolicy::CompanyPalette {
                    colour: palette_colour,
                }
            } else {
                // Other PaletteIDs (2CC, crash, pulsating overlays) require
                // a palette table that Bevy does not currently expose. Keep
                // the decoded pixels instead of applying the owner's colour
                // to a palette chosen explicitly by the GRF.
                DecodedSpriteImagePolicy::Raw
            };
            let handle = self
                .handles
                .entry(key)
                .or_insert_with(|| images.add(decoded_sprite_image(&view, image_policy)))
                .clone();
            layers.push(NewGrfVehicleLayer {
                handle,
                x_offs: view.x_offs,
                y_offs: view.y_offs,
                width: view.width,
                height: view.height,
            });
            previous = Some(view);
            if register_100.is_some_and(|value| value & 0x8000_0000 == 0) {
                break;
            }
        }
        layers
    }
}

type DirHandles = [Handle<Image>; 8];

#[derive(Resource)]
pub(crate) struct TruckHandles {
    pub(super) bus: DirHandles,
    pub(super) bus_loaded: DirHandles,
    pub(super) truck: DirHandles,
    pub(super) truck_loaded: DirHandles,
    pub(super) ship: DirHandles,
    pub(super) ship_oil: DirHandles,
    pub(super) ship_coal: DirHandles,
    pub(super) ship_ferry: DirHandles,
    pub(super) aircraft: DirHandles,
    pub(super) aircraft_fokker: DirHandles,
    pub(super) aircraft_tricario: DirHandles,
    pub(super) aircraft_rotor: [Handle<Image>; 4],
    pub(super) train_groups: [DirHandles; TRAIN_GROUP_COUNT],
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
            aircraft_rotor: std::array::from_fn(|i| {
                asset_server.load(AIRCRAFT_ROTOR_LAYERS[i].path)
            }),
            train_groups: [
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_T0),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_T1),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_TDIESEL),
                load_set(asset_server, &TRAIN_VEHICLE_LAYERS_TELECTRIC),
                load_set(asset_server, &TRAIN_WAGON_PASSENGER_LAYERS),
                load_set(asset_server, &TRAIN_WAGON_MAIL_LAYERS),
                load_set(asset_server, &TRAIN_WAGON_GOODS_LAYERS),
                load_set(asset_server, &TRAIN_WAGON_COAL_LAYERS),
                load_set(asset_server, &TRAIN_WAGON_COAL_LOADED_LAYERS),
            ],
        }
    }

    pub(super) fn aircraft_rotor(&self, frame: usize) -> Handle<Image> {
        self.aircraft_rotor[frame.min(self.aircraft_rotor.len() - 1)].clone()
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
        let group =
            usize::from(openttdrs_core::train_sprite_group(image_index)).min(TRAIN_GROUP_COUNT - 2);
        self.train_groups[group][dir.min(7)].clone()
    }

    /// Textura del sprite según la pose de render (extrapolada entre ticks de
    /// sim): la dirección del sprite acompaña la posición dibujada en curvas,
    /// en vez de usar la dirección lógica del último tick.
    pub(super) fn for_vehicle(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        company: Option<&CompanyColoredSprites>,
        owner_colour: Option<crate::sprites::CompanyColour>,
    ) -> Handle<Image> {
        let dir = openttdrs_core::vehicle_sprite_direction_at(v, pose).min(7) as usize;
        let layer = &vehicle_layers(v)[dir];
        if let Some(c) = company {
            let handle = match owner_colour {
                Some(col) => c.vehicle_handle_for_colour(col, layer.path),
                None => c.vehicle_handle(layer.path),
            };
            if let Some(handle) = handle {
                return handle.clone();
            }
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
                let group = train_group_for_vehicle(v);
                self.train_groups[group][i].clone()
            }
        }
    }

    /// Capas NewGRF con offsets individuales para el parent y sus children.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_vehicle_with_newgrf_layers(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        _company: Option<&CompanyColoredSprites>,
        owner_colour: Option<crate::sprites::CompanyColour>,
        sim: &crate::state::SimWorld,
        cache: &mut NewGrfTrainSpriteCache,
        images: &mut Assets<Image>,
    ) -> Vec<NewGrfVehicleLayer> {
        let dir = openttdrs_core::vehicle_sprite_direction_at(v, pose).min(7) as usize;
        if let Some(eid) = v.engine_id
            && let Some(eng) = super::engine_in_sim(sim, eid)
        {
            let colour = owner_colour.unwrap_or(CompanyColour::DarkBlue);
            if eng.newgrf_runtime.is_some() {
                let colour_u8 = colour.as_u8();
                let mut ctx = openttdrs_core::action2_eval_ctx_for_unit(
                    &sim.state.vehicles,
                    v.id,
                    sim.state.tick,
                    &sim.state.engine_catalog,
                    colour_u8,
                );
                openttdrs_core::enrich_vehicle_track_badge_vars(
                    &mut ctx,
                    &sim.state.vehicles,
                    v.id,
                    &sim.state.map,
                    &sim.state.engine_catalog,
                    &sim.state.runtime.rail_type_badges,
                    &sim.state.road_type_catalog,
                );
                ctx.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    &sim.state.newgrf_stack,
                    eng.newgrf_grfid,
                ));
                let overriding_local_id = overriding_engine_local_id(sim, v, eng);
                let palette_override =
                    openttdrs_core::resolve_vehicle_colour_mapping_callback(eng, v)
                        .map(|mapping| mapping.palette_for_company(colour.as_u8()));
                let layers = cache.handles_for_runtime_with_override(
                    eng,
                    dir,
                    v.cargo_type,
                    colour,
                    overriding_local_id,
                    palette_override,
                    &mut ctx,
                    images,
                );
                if !layers.is_empty() {
                    return layers;
                }
            } else if let Some(handle) = cache.handle_for(eng, dir, colour, images)
                && let Some(view) = eng.newgrf_view(dir)
            {
                return vec![NewGrfVehicleLayer {
                    handle,
                    x_offs: view.x_offs,
                    y_offs: view.y_offs,
                    width: view.width,
                    height: view.height,
                }];
            }
        }
        Vec::new()
    }
}
