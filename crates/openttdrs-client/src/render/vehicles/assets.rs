use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{EngineDef, Vehicle, VehicleKind};

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
    TRAIN_VEHICLE_LAYERS_TELECTRIC, TRUCK_VEHICLE_LAYERS, TRUCK_VEHICLE_LAYERS_LOADED,
};

pub(crate) use vehicle_gfx::VehicleLayerGfx;

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

/// Caché in-world / preview: `(engine_id, view_idx, company_colour)` → textura.
#[derive(Resource, Default)]
pub(crate) struct NewGrfTrainSpriteCache {
    /// `(engine_id, view_idx, colour, runtime_fp)` → textura.
    handles: HashMap<(u16, u8, u8, u32), Handle<Image>>,
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
        let key = (engine.id, view_idx, colour.as_u8(), 0);
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
    pub(crate) fn handle_for_runtime(
        &mut self,
        engine: &EngineDef,
        dir: usize,
        colour: CompanyColour,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let runtime = engine.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(engine.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        let view = &views[dir % views.len()];
        let view_idx = u8::try_from(dir % views.len()).unwrap_or(0);
        let fp = runtime_fingerprint(ctx, vars::TRAIN, true);
        let key = (engine.id, view_idx, colour.as_u8(), fp);
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
    pub(super) train_groups: [DirHandles; 5],
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
    pub(super) fn for_vehicle(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        company: Option<&CompanyColoredSprites>,
        owner_colour: Option<crate::sprites::CompanyColour>,
    ) -> Handle<Image> {
        let dir = openttdrs_core::vehicle_render_direction_at(v, pose).min(7) as usize;
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
    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_vehicle_with_newgrf(
        &self,
        v: &Vehicle,
        pose: openttdrs_core::VehiclePose,
        company: Option<&CompanyColoredSprites>,
        owner_colour: Option<crate::sprites::CompanyColour>,
        sim: &crate::state::SimWorld,
        cache: &mut NewGrfTrainSpriteCache,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let dir = openttdrs_core::vehicle_render_direction_at(v, pose).min(7) as usize;
        if v.kind == VehicleKind::Train
            && let Some(eid) = v.engine_id
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
                ctx.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    &sim.state.newgrf_stack,
                    eng.newgrf_grfid,
                ));
                if let Some(handle) = cache.handle_for_runtime(eng, dir, colour, &mut ctx, images) {
                    return handle;
                }
            } else if let Some(handle) = cache.handle_for(eng, dir, colour, images) {
                return handle;
            }
        }
        self.for_vehicle(v, pose, company, owner_colour)
    }
}
