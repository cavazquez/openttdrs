//! Aplicación de Action0 `Trains` desde el `NewGRF` stack.

use std::path::Path;

use crate::GameState;
use crate::engine::{EngineDef, next_free_engine_id, vanilla_engine_catalog};
use crate::vehicle::VehicleKind;

use super::super::action0::{
    ACTION0_FEATURE_AIRCRAFT, ACTION0_FEATURE_ROAD_VEHICLES, ACTION0_FEATURE_SHIPS,
    collect_train_metas_from_grf, collect_vehicle_metas_from_grf,
};

fn vehicle_price_bases(kind: VehicleKind) -> (i64, i64) {
    match kind {
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => (14_000, 1_600),
        VehicleKind::Ship => (65_000, 5_600),
        VehicleKind::Aircraft => (700_000, 9_600),
        VehicleKind::Train => (400_000, 5_200),
    }
}

fn resolve_vehicle_badges(
    local_ids: &[u16],
    badge_labels: &[String],
    badge_catalog: &[crate::badge::BadgeDef],
    grfid: u32,
) -> (Vec<u16>, Vec<u16>, Vec<String>) {
    crate::badge::resolve_badge_local_ids(local_ids, badge_labels, badge_catalog, grfid)
}

#[allow(clippy::too_many_arguments)]
fn push_feature_vehicles(
    catalog: &mut Vec<EngineDef>,
    data: &[u8],
    feature: u8,
    grfid: u32,
    climate_bit: u8,
    badge_catalog: &[crate::badge::BadgeDef],
    badge_labels: &[String],
    diagnostics: &mut Vec<String>,
) {
    let metas = collect_vehicle_metas_from_grf(data, feature);
    let gfx =
        crate::newgrf_sprites::collect_feature_sprite_graphics(data, feature).unwrap_or_default();
    for meta in metas {
        if meta.climate_mask & climate_bit == 0 {
            continue;
        }
        let Some(id) = next_free_engine_id(catalog) else {
            break;
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        let views = gfx
            .views_for_local_id_cargo_u16_ctx(meta.local_id, meta.cargo, &mut ctx)
            .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
            .unwrap_or_default();
        let has_cargo_groups = gfx
            .specific_assigns
            .keys()
            .any(|(local_id, _)| u16::from(*local_id) == meta.local_id);
        let has_cargo_groups = has_cargo_groups
            || gfx
                .extended_specific_assigns
                .keys()
                .any(|(local_id, _)| *local_id == meta.local_id);
        let has_extended_id = gfx
            .extended_assigns
            .iter()
            .any(|(local_id, _)| *local_id == meta.local_id);
        let newgrf_runtime = if gfx.needs_runtime_resolve()
            || has_cargo_groups
            || has_extended_id
            || !gfx.wagon_overrides.is_empty()
        {
            Some(Box::new(gfx.clone()))
        } else {
            None
        };
        let (price_base, running_base) = vehicle_price_bases(meta.kind);
        let (badges, newgrf_badge_translation, unresolved_badges) =
            resolve_vehicle_badges(&meta.badge_local_ids, badge_labels, badge_catalog, grfid);
        for label in unresolved_badges {
            diagnostics.push(format!(
                "vehicle '{}': badge '{}' no resuelto",
                meta.name, label
            ));
        }
        catalog.push(EngineDef {
            id,
            kind: meta.kind,
            name: meta.name,
            max_speed: meta.max_speed,
            price: (price_base * i64::from(meta.price_factor)) >> 8,
            running_cost_year: (running_base * i64::from(meta.running_cost_factor)) >> 8,
            capacity: meta.capacity,
            cargo: meta.cargo,
            power_hp: meta.power_hp,
            weight_t: meta.weight_t,
            intro_year: meta.intro_year,
            reliability_pct: 85,
            reliability_spd_dec: meta.reliability_spd_dec,
            lifelength_years: meta.lifelength_years,
            model_life_years: meta.model_life_years,
            load_amount: meta.load_amount,
            train_image_index: 0,
            dual_headed: false,
            rail_tilts: false,
            curve_speed_mod: 0,
            pow_wag_power: 0,
            pow_wag_weight: 0,
            from_newgrf: true,
            tractive_effort: 0,
            air_drag: 0,
            shorten_factor: 0,
            required_rail_type: None,
            refit_mask: meta.refit_mask & !meta.refit_exclude_mask,
            is_helicopter: meta.is_helicopter,
            is_large_aircraft: meta.is_large_aircraft,
            sprite_stack: meta.sprite_stack,
            ocean_speed_frac: meta.ocean_speed_frac,
            canal_speed_frac: meta.canal_speed_frac,
            sound_effect: meta.sound_effect,
            visual_effect: meta.visual_effect,
            newgrf_views: views,
            newgrf_local_id: meta.local_id,
            newgrf_runtime,
            newgrf_grfid: grfid,
            vehicle_callback_mask: meta.callback_mask,
            badges,
            newgrf_badge_translation,
        });
    }
}

/// Reconstruye el catálogo de motores (vanilla + Action0/1/2/3 de los cuatro
/// features de vehículos).
#[allow(clippy::too_many_lines)]
pub fn apply_newgrf_vehicles_trains(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_engine_catalog();
    let badge_catalog = state.badge_catalog.clone();
    let stack = state.newgrf_stack.clone();
    let climate_bit = state.climate.newgrf_landscape_bit();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let metas = collect_train_metas_from_grf(&data);
        let badge_labels = crate::newgrf_type_tables::collect_type_tables_from_grf(&data).badges;
        let gfx = crate::newgrf_sprites::collect_train_sprite_graphics(&data).unwrap_or_default();
        // Action0 conserva el id local del primer vehículo del bloque; no lo
        // sustituimos por el índice de aparición porque CB16 necesita volver a
        // localizar partes articuladas dentro del mismo GRF.
        for meta in metas {
            if meta.climate_mask & climate_bit == 0 {
                continue;
            }
            let Some(id) = next_free_engine_id(&catalog) else {
                break;
            };
            let local_id = meta.local_id;
            let views = gfx
                .views_for_local_id_u16(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let has_extended_id = gfx.extended_assigns.iter().any(|(id, _)| *id == local_id);
            let newgrf_runtime = if gfx.needs_runtime_resolve()
                || has_extended_id
                || !gfx.wagon_overrides.is_empty()
            {
                Some(Box::new(gfx.clone()))
            } else {
                None
            };
            let (badges, newgrf_badge_translation, unresolved_badges) = resolve_vehicle_badges(
                &meta.badge_local_ids,
                &badge_labels,
                &badge_catalog,
                entry.grfid,
            );
            for label in unresolved_badges {
                state.runtime.newgrf_diagnostics.push(format!(
                    "vehicle '{}': badge '{}' no resuelto",
                    meta.name, label
                ));
            }
            catalog.push(EngineDef {
                id,
                kind: VehicleKind::Train,
                name: meta.name,
                max_speed: meta.max_speed,
                price: (400_000_i64 * i64::from(meta.price_factor)) >> 8,
                running_cost_year: (5_200 * i64::from(meta.running_cost_factor)) >> 8,
                capacity: meta.capacity,
                cargo: meta.cargo,
                power_hp: meta.power_hp,
                weight_t: meta.weight_t,
                intro_year: meta.intro_year,
                reliability_pct: 85,
                reliability_spd_dec: meta.reliability_spd_dec,
                lifelength_years: meta.lifelength_years,
                model_life_years: meta.model_life_years,
                load_amount: meta.load_amount,
                train_image_index: meta.train_image_index,
                dual_headed: meta.dual_headed,
                rail_tilts: meta.rail_tilts,
                curve_speed_mod: meta.curve_speed_mod,
                pow_wag_power: meta.pow_wag_power,
                pow_wag_weight: meta.pow_wag_weight,
                from_newgrf: true,
                tractive_effort: meta.tractive_effort,
                air_drag: meta.air_drag,
                shorten_factor: meta.shorten_factor,
                required_rail_type: meta.required_rail_type,
                refit_mask: meta.refit_mask,
                is_helicopter: false,
                is_large_aircraft: false,
                sprite_stack: meta.sprite_stack,
                ocean_speed_frac: 0,
                canal_speed_frac: 0,
                sound_effect: 0,
                visual_effect: meta.visual_effect,
                newgrf_views: views,
                newgrf_local_id: local_id,
                newgrf_runtime,
                newgrf_grfid: entry.grfid,
                vehicle_callback_mask: meta.callback_mask,
                badges,
                newgrf_badge_translation,
            });
        }

        for feature in [
            ACTION0_FEATURE_ROAD_VEHICLES,
            ACTION0_FEATURE_SHIPS,
            ACTION0_FEATURE_AIRCRAFT,
        ] {
            push_feature_vehicles(
                &mut catalog,
                &data,
                feature,
                entry.grfid,
                climate_bit,
                &badge_catalog,
                &badge_labels,
                &mut state.runtime.newgrf_diagnostics,
            );
        }
    }
    state.engine_catalog = catalog;
}

/// Aplica trains con directorios de búsqueda por defecto.
pub fn apply_newgrf_vehicles_trains_default_dirs(state: &mut GameState) {
    let owned = super::default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_vehicles_trains(state, &refs);
}
