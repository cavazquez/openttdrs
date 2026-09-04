//! Hidratación semántica común de entidades de un `.sav`.
//!
//! El decoder conserva los datos de chunks (`INDY`, `CAPA`, `STNN`); este
//! módulo los convierte a las estructuras de simulación. Mantenerlo en core
//! evita que el cliente y las herramientas de línea de comandos discrepen
//! sobre qué industrias y qué carga contiene una misma partida.

use std::collections::{HashSet, VecDeque};

use crate::company::OWNER_NONE_M1;
use crate::game_state::{LegacySavAfterload, LegacySavIndustry};
use crate::industry::{INDUSTRY_STOCK_CAPACITY, PRODLEVEL_DEFAULT};
use crate::industry_tile::get_clean_industry_gfx;
use crate::world_gen::{CLEAR_GROUND_FIELDS, CLEAR_GROUND_GRASS, clear_ground_m5};
use crate::{
    Climate, GameState, Industry, IndustryKind, IndustrySpec, OttdmapExtras, TileCoord, TileKind,
};

use super::entities::SavIndustry;

/// Desde `SLV_32` `OpenTTD` persiste el `IndustryID` que creó cada campo.
/// Antes de esa versión `AfterLoadGame()` los elimina y los vuelve a plantar.
pub(crate) const LEGACY_FARMLAND_SAVE_VERSION: u16 = 32;

/// Desde `SLV_55` los campos de cargo de las tablas nativas dejaron de usar
/// el índice local del landscape y pasaron a guardar el `CargoType` global.
///
/// `STNN.goods`, `INDY.{accepted,produced}` y `VEHS.common.cargo_type` tienen
/// el mismo corte de versión. Antes de él, por ejemplo, el slot 6 era trigo
/// en el ártico y grano en el templado; después ambos valores usan sus IDs
/// globales (`WHEA = 11`, `GRAI = 6`).
pub(crate) const SAV_GLOBAL_CARGO_SLOTS_VERSION: u16 = 55;

/// Resuelve un byte de cargo según la codificación nativa del `.sav`.
///
/// Los saves anteriores a [`SAV_GLOBAL_CARGO_SLOTS_VERSION`] codifican el
/// slot relativo al clima. Los modernos usan el ID global de `CargoType` y
/// por eso también pueden transportar cargos `NewGRF` (`31..62`) aunque el
/// catálogo del GRF todavía no esté instalado. El catálogo sólo actúa como
/// fallback para IDs que el runtime no materializa (por ejemplo `63`).
#[must_use]
pub(crate) fn cargo_from_sav_slot(
    slot: u8,
    climate: Climate,
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
    save_version: u16,
) -> Option<crate::CargoType> {
    let from_catalog = || {
        cargo_catalog
            .iter()
            .find(|def| def.id == slot || def.bitnum == slot)
            .and_then(|def| {
                def.cargo_type()
                    .or_else(|| crate::CargoType::from_label(&def.label))
            })
    };

    if save_version < SAV_GLOBAL_CARGO_SLOTS_VERSION {
        crate::CargoType::from_climate_slot(climate, slot).or_else(from_catalog)
    } else {
        crate::CargoType::from_cargo_id(slot)
            .or_else(from_catalog)
            // `from_cargo_id` is authoritative for modern saves. Keep the
            // climate fallback only for malformed/hand-built fixtures whose
            // row carries a legacy slot despite a modern version header.
            .or_else(|| crate::CargoType::from_climate_slot(climate, slot))
    }
}

/// Conserva la información de industria que no forma parte de `GameState` y
/// que el hook nativo de afterload necesita antes de tener el catálogo GRF.
pub(crate) fn queue_legacy_sav_afterload(
    state: &mut GameState,
    save_version: u16,
    sav_industries: &[SavIndustry],
) {
    if save_version >= LEGACY_FARMLAND_SAVE_VERSION {
        return;
    }
    state.runtime.legacy_sav_afterload = Some(LegacySavAfterload {
        version: save_version,
        industries: sav_industries
            .iter()
            .map(|saved| LegacySavIndustry {
                industry_id: saved.industry_id,
                pos: saved.pos,
                industry_type: u16::from(saved.industry_type),
                width: i32::from(saved.width.max(1)),
                height: i32::from(saved.height.max(1)),
            })
            .collect(),
    });
}

/// Ejecuta la parte de `AfterLoadGame()` relativa a campos de SAV antiguos.
///
/// Se llama inmediatamente para partidas vanilla y desde el refresco de
/// catálogos cuando el save llevaba un GRF custom. Devuelve `true` sólo cuando
/// consumió una pasada pendiente, evitando volver a avanzar `Random()` si el
/// stack se refresca más de una vez.
#[allow(clippy::too_many_lines)]
pub(crate) fn apply_legacy_sav_afterload(state: &mut GameState) -> bool {
    let Some(pending) = state.runtime.legacy_sav_afterload.take() else {
        return false;
    };
    debug_assert!(pending.version < LEGACY_FARMLAND_SAVE_VERSION);

    // `MakeClear(t, CLEAR_GRASS, 3)` resetea todos los bytes auxiliares y
    // conserva solamente altura y el nibble de zona de MAPT.
    let (map_w, map_h) = state.map.dimensions();
    let mut dirty = HashSet::new();
    for y in 0..map_h.cast_signed() {
        for x in 0..map_w.cast_signed() {
            let coord = TileCoord::new(x, y);
            let Some(previous) = state.map.get(coord) else {
                continue;
            };
            if previous.kind != TileKind::Grass
                || crate::map::tree_tile_loop::clear_ground_type(previous.m5) != CLEAR_GROUND_FIELDS
            {
                continue;
            }
            let cleared = crate::map::Tile {
                height: previous.height,
                kind: TileKind::Grass,
                mapt: previous.mapt & 0x0F,
                m5: clear_ground_m5(CLEAR_GROUND_GRASS, 3),
                m1: OWNER_NONE_M1,
                m6: 0,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            };
            if state.map.set_tile(coord, cleared).is_ok() {
                dirty.insert(coord);
            }
        }
    }

    // Las filas legacy no siempre conservan rectángulo; en ese caso el
    // componente de tiles importado es la mejor reconstrucción disponible.
    let plant_plans: Vec<(TileCoord, i32, i32, u16)> = state
        .industries
        .iter()
        .enumerate()
        .filter_map(|(index, industry)| {
            let saved = pending
                .industries
                .iter()
                .find(|saved| u16::try_from(saved.industry_id).ok() == Some(industry.instance_id))
                .or_else(|| {
                    pending
                        .industries
                        .iter()
                        .find(|saved| saved.pos == industry.pos)
                })
                .or_else(|| pending.industries.get(index));

            let custom_def = saved
                .and_then(|saved| {
                    let translated = crate::industry_spec::get_translated_industry_id(
                        saved.industry_type,
                        &state.industry_overrides,
                    );
                    state
                        .industry_spec_catalog
                        .iter()
                        .find(|def| def.id == translated)
                })
                .or_else(|| {
                    industry.newgrf_type_id.and_then(|type_id| {
                        state
                            .industry_spec_catalog
                            .iter()
                            .find(|def| def.id == type_id)
                    })
                });
            let plant_on_build = custom_def.map_or(
                matches!(
                    industry.spec,
                    Some(IndustrySpec::Farm | IndustrySpec::FarmTropic)
                ),
                |def| {
                    def.behaviour & crate::industry_spec::INDUSTRY_BEHAVIOUR_PLANT_ON_BUILD_MASK
                        != 0
                },
            );
            if !plant_on_build {
                return None;
            }

            let (origin, mut width, mut height) = saved.map_or((industry.pos, 1, 1), |saved| {
                (saved.pos, saved.width, saved.height)
            });
            if width <= 1 && height <= 1 {
                let (derived_width, derived_height) = industry_tiles_dimensions(industry);
                width = derived_width;
                height = derived_height;
            }
            Some((origin, width, height, industry.instance_id))
        })
        .collect();

    let mut rng = state.random;
    for (origin, width, height, industry_id) in plant_plans {
        crate::world_gen::plant_random_farm_fields_runtime(
            state,
            origin,
            width,
            height,
            industry_id,
            &mut rng,
        );
    }
    state.random = rng;

    // El cliente remapea tanto campos eliminados como los nuevos; el barrido
    // es barato frente a la carga del SAV y cubre cercas mutadas por cada lote.
    for y in 0..map_h.cast_signed() {
        for x in 0..map_w.cast_signed() {
            let coord = TileCoord::new(x, y);
            if state.map.get(coord).is_some_and(|tile| {
                tile.kind == TileKind::Grass
                    && crate::map::tree_tile_loop::clear_ground_type(tile.m5) == CLEAR_GROUND_FIELDS
            }) {
                dirty.insert(coord);
            }
        }
    }
    state.runtime.industry_tile_dirty.extend(dirty);
    true
}

fn industry_tiles_dimensions(industry: &Industry) -> (i32, i32) {
    let min_x = industry
        .tiles
        .iter()
        .map(|tile| tile.x)
        .min()
        .unwrap_or(industry.pos.x);
    let max_x = industry
        .tiles
        .iter()
        .map(|tile| tile.x)
        .max()
        .unwrap_or(industry.pos.x);
    let min_y = industry
        .tiles
        .iter()
        .map(|tile| tile.y)
        .min()
        .unwrap_or(industry.pos.y);
    let max_y = industry
        .tiles
        .iter()
        .map(|tile| tile.y)
        .max()
        .unwrap_or(industry.pos.y);
    (
        max_x.saturating_sub(min_x).saturating_add(1).max(1),
        max_y.saturating_sub(min_y).saturating_add(1).max(1),
    )
}

/// Mapea los tipos vanilla de `IndustryType` a nuestro modelo económico
/// reducido. El gfx de tesela, cuando está disponible, es más específico y
/// por eso tiene prioridad mediante [`industry_kind_from_gfx`]. La hidratación
/// del estado completo se realiza internamente por `hydrate_sav_industries`.
#[must_use]
pub fn industry_kind_from_ottd_type(industry_type: u8) -> IndustryKind {
    match industry_type {
        2 | 3 | 9 | 14 | 19 | 20 | 24 | 25 => IndustryKind::Forest,
        11..=13 => IndustryKind::Factory,
        16 | 17 | 33 => IndustryKind::OilWell,
        _ => IndustryKind::CoalMine,
    }
}

/// Color determinista usado cuando un formato antiguo no conserva
/// `Industry.random_colour`.
#[must_use]
pub const fn industry_random_colour_from_instance(instance_id: u16) -> u8 {
    (instance_id.wrapping_mul(5) % 16) as u8
}

type IndustryGfxRange = (u16, u16, &'static str, Option<IndustryKind>);

const INDUSTRY_GFX_RANGES: [IndustryGfxRange; 32] = [
    (0, 6, "Coal Mine", Some(IndustryKind::CoalMine)),
    (7, 10, "Power Station", None),
    (11, 15, "Sawmill", None),
    (16, 17, "Forest", Some(IndustryKind::Forest)),
    (18, 23, "Oil Refinery", Some(IndustryKind::OilWell)),
    (24, 28, "Oil Rig", Some(IndustryKind::OilWell)),
    (29, 32, "Oil Wells", Some(IndustryKind::OilWell)),
    (33, 38, "Farm", Some(IndustryKind::Forest)),
    (39, 42, "Factory", Some(IndustryKind::Factory)),
    (43, 46, "Printing Works", None),
    (47, 51, "Copper Ore Mine", Some(IndustryKind::CoalMine)),
    (52, 57, "Steel Mill", None),
    (58, 59, "Bank", None),
    (60, 67, "Food Processing Plant", Some(IndustryKind::Factory)),
    (68, 75, "Paper Mill", Some(IndustryKind::Factory)),
    (76, 88, "Gold Mine", Some(IndustryKind::CoalMine)),
    (89, 90, "Bank", None),
    (91, 99, "Diamond Mine", Some(IndustryKind::CoalMine)),
    (100, 115, "Iron Ore Mine", Some(IndustryKind::CoalMine)),
    (116, 119, "Other climates", None),
    (120, 124, "Candy Factory", None),
    (125, 128, "Sweets Shop", None),
    (129, 130, "Cotton Candy Forest", Some(IndustryKind::Forest)),
    (131, 134, "Candy Factory", Some(IndustryKind::Factory)),
    (135, 136, "Battery Farm", Some(IndustryKind::CoalMine)),
    (137, 141, "Cola Wells", Some(IndustryKind::OilWell)),
    (142, 147, "Toy Factory", Some(IndustryKind::Factory)),
    (148, 154, "Plastic Fountain", Some(IndustryKind::CoalMine)),
    (156, 159, "Fizzy Drink Factory", Some(IndustryKind::Factory)),
    (160, 163, "Bubble Generator", Some(IndustryKind::Forest)),
    (164, 166, "Toffee Quarry", Some(IndustryKind::CoalMine)),
    (167, 174, "Sugar Mine", Some(IndustryKind::CoalMine)),
];

fn gfx_range_info(gfx: u16) -> Option<IndustryGfxRange> {
    INDUSTRY_GFX_RANGES
        .iter()
        .copied()
        .find(|(start, end, _, _)| (*start..=*end).contains(&gfx))
}

/// Grupo visible vanilla de una tesela de industria.
#[must_use]
pub fn industry_group_from_gfx(gfx: u16) -> &'static str {
    gfx_range_info(gfx).map_or("Unknown gfx", |(_, _, label, _)| label)
}

/// Clasificación económica de respaldo por gfx de tesela.
#[must_use]
pub fn industry_kind_from_gfx(gfx: u16) -> IndustryKind {
    if let Some((_, _, _, kind)) = gfx_range_info(gfx) {
        // Los grupos sin relación 1:1 con el modelo reducido son procesadores.
        return kind.unwrap_or(IndustryKind::Factory);
    }
    if gfx.is_multiple_of(2) {
        IndustryKind::CoalMine
    } else {
        IndustryKind::Forest
    }
}

/// `IndustrySpec` vanilla identificable por el gfx de tesela.
#[must_use]
pub fn industry_spec_from_gfx(gfx: u16) -> Option<IndustrySpec> {
    match gfx {
        0..=6 => Some(IndustrySpec::CoalMine),
        7..=10 => Some(IndustrySpec::PowerStation),
        11..=15 => Some(IndustrySpec::Sawmill),
        16..=17 => Some(IndustrySpec::Forest),
        18..=23 => Some(IndustrySpec::OilRefinery),
        24..=32 => Some(IndustrySpec::OilWells),
        33..=38 => Some(IndustrySpec::Farm),
        39..=42 => Some(IndustrySpec::Factory),
        43..=46 => Some(IndustrySpec::PrintingWorks),
        47..=51 => Some(IndustrySpec::CopperOreMine),
        52..=57 => Some(IndustrySpec::SteelMill),
        58..=59 | 89..=90 => Some(IndustrySpec::Bank),
        60..=67 => Some(IndustrySpec::FoodProcessingPlant),
        68..=75 => Some(IndustrySpec::PaperMill),
        76..=88 => Some(IndustrySpec::GoldMine),
        91..=99 => Some(IndustrySpec::DiamondMine),
        100..=115 => Some(IndustrySpec::IronOreMine),
        129..=130 => Some(IndustrySpec::CottonCandy),
        131..=134 => Some(IndustrySpec::CandyFactory),
        135..=136 => Some(IndustrySpec::BatteryFarm),
        137..=141 => Some(IndustrySpec::ColaWells),
        142..=147 => Some(IndustrySpec::ToyFactory),
        148..=154 => Some(IndustrySpec::PlasticFountain),
        156..=159 => Some(IndustrySpec::FizzyDrinkFactory),
        160..=163 => Some(IndustrySpec::BubbleGenerator),
        164..=166 => Some(IndustrySpec::ToffeeQuarry),
        167..=174 => Some(IndustrySpec::SugarMine),
        _ => None,
    }
}

/// Añade las industrias del save a `state` desde un origen único core.
///
/// `INDY` conserva los límites reales, aun si dos industrias son adyacentes.
/// Si no existe (saves legacy), se usa la misma heurística por componentes que
/// antes vivía en el cliente, con el footer `INDP` si está disponible.
pub(crate) fn hydrate_sav_industries(
    state: &mut GameState,
    sav_industries: &[SavIndustry],
    extras: &OttdmapExtras,
) {
    if sav_industries.is_empty() {
        hydrate_industries_from_map_tiles(state, Some(extras));
        return;
    }
    state.industries.clear();

    for saved in sav_industries {
        let mut tiles = Vec::new();
        for dy in 0..i32::from(saved.height.max(1)) {
            for dx in 0..i32::from(saved.width.max(1)) {
                let coord = TileCoord::new(saved.pos.x + dx, saved.pos.y + dy);
                if state.map.get_kind(coord) == Some(TileKind::Industry) {
                    tiles.push(coord);
                }
            }
        }
        let origin = tiles.first().copied().unwrap_or(saved.pos);
        if tiles.is_empty() {
            tiles.push(origin);
        }
        let gfx = state
            .map
            .get(origin)
            .map(|tile| get_clean_industry_gfx(tile.m5, tile.m6));
        let spec = gfx.and_then(industry_spec_from_gfx);
        let kind = spec
            .map(IndustrySpec::kind)
            .or_else(|| gfx.map(industry_kind_from_gfx))
            .unwrap_or_else(|| industry_kind_from_ottd_type(saved.industry_type));
        let instance_id = state.map.get(origin).map_or_else(
            || u16::try_from(saved.industry_id).unwrap_or(0),
            |tile| crate::map::industry_instance_id(&tile),
        );
        let mut industry = if let Some(spec) = spec {
            Industry::with_tiles_spec(origin, kind, spec, tiles, saved.random_colour)
        } else {
            Industry::with_tiles(origin, kind, tiles).with_random_colour(saved.random_colour)
        }
        .with_instance_id(instance_id)
        .with_counter(saved.counter);
        industry.selected_layout = saved.selected_layout;
        industry.newgrf_random = saved.random;
        industry.newgrf_persistent_storage_id = saved.persistent_storage_id;
        industry.valid_history = saved.valid_history;
        industry.last_prod_year = saved.last_prod_year;
        industry.was_cargo_delivered = saved.was_cargo_delivered;
        industry.control_flags = saved.control_flags;
        industry.neutral_station_id = saved.neutral_station_id;
        industry.exclusive_supplier = saved.exclusive_supplier.map(crate::company::CompanyId);
        industry.founder = saved.founder.map(crate::company::CompanyId);
        industry.construction_date = saved.construction_date;
        industry.construction_type = saved.construction_type;
        industry.prod_level = saved.prod_level;
        import_industry_output_stock(
            &mut industry,
            saved,
            state.climate,
            state.sav_version.unwrap_or(SAV_GLOBAL_CARGO_SLOTS_VERSION),
        );
        state.industries.push(industry);
    }
}

fn import_industry_output_stock(
    industry: &mut Industry,
    saved: &SavIndustry,
    climate: Climate,
    save_version: u16,
) {
    let outputs = industry.produced_cargos();
    for produced in &saved.produced {
        let Some(cargo) = cargo_from_sav_slot(produced.cargo_slot, climate, &[], save_version)
        else {
            continue;
        };
        let waiting = u32::from(produced.waiting);
        if !produced.history.is_empty() {
            industry.produced_history.insert(
                cargo,
                produced
                    .history
                    .iter()
                    .take(crate::entity_history::INDUSTRY_HISTORY_RECORDS)
                    .map(
                        |sample| crate::entity_history::IndustryProducedHistorySample {
                            production: sample.production,
                            transported: sample.transported,
                        },
                    )
                    .collect(),
            );
        }
        if outputs.first().copied() == Some(cargo) {
            industry.stock = waiting;
            continue;
        }
        if outputs.get(1).copied() == Some(cargo) {
            industry.secondary_stock = waiting;
            continue;
        }
        // Una industria NewGRF puede producir más de dos cargos. No los
        // confundimos con los stocks legacy: el runtime de callbacks y el
        // transporte los consumen desde este buffer separado.
        industry.newgrf_extra_produced_cargo.add(cargo, waiting);
    }

    // En OpenTTD las entradas no se almacenan en las estaciones una vez que
    // fueron aceptadas por la industria: quedan en `accepted[i].waiting` a la
    // espera de CB1/CB2. El parser ya conserva esa lista; hidratarla aquí
    // evita perderla al abrir un `.sav` y permite que el callback la consuma.
    for accepted in &saved.accepted {
        let Some(cargo) = cargo_from_sav_slot(accepted.cargo_slot, climate, &[], save_version)
        else {
            continue;
        };
        industry.add_accepted_cargo_waiting(cargo, u32::from(accepted.waiting));
        industry.set_last_accepted_date(cargo, accepted.last_accepted);
        industry
            .accepted_accumulated_waiting
            .set(cargo, accepted.accumulated_waiting);
        if !accepted.history.is_empty() {
            industry.accepted_history.insert(
                cargo,
                accepted
                    .history
                    .iter()
                    .map(
                        |sample| crate::entity_history::IndustryAcceptedHistorySample {
                            accepted: sample.accepted,
                            waiting: sample.waiting,
                        },
                    )
                    .collect(),
            );
        }
    }
    industry.capacity = INDUSTRY_STOCK_CAPACITY.max(industry.stock.max(industry.secondary_stock));
}

/// Hidrata los registros `7C` de industrias desde las filas `PSAC` importadas.
///
/// Sólo los valores distintos de cero se materializan en el mapa disperso de
/// runtime; la fila completa permanece en `GameState::sav_persistent_storages`
/// para conservar ceros explícitos y storages de otras entidades al exportar.
pub(crate) fn hydrate_sav_industry_persistent_storage(
    state: &mut GameState,
    sav_industries: &[SavIndustry],
    persistent_storages: &[crate::sav::SavPersistentStorage],
) {
    if sav_industries.is_empty() || persistent_storages.is_empty() {
        return;
    }
    let by_id: std::collections::HashMap<u32, &crate::sav::SavPersistentStorage> =
        persistent_storages
            .iter()
            .map(|storage| (storage.storage_id, storage))
            .collect();
    for (industry, saved) in state.industries.iter_mut().zip(sav_industries) {
        let Some(storage_id) = saved.persistent_storage_id else {
            continue;
        };
        let Some(storage) = by_id.get(&storage_id) else {
            continue;
        };
        industry.newgrf_persistent_storage_id = Some(storage_id);
        for (index, &value) in storage.storage.iter().enumerate() {
            if value == 0 {
                continue;
            }
            let Ok(index) = u8::try_from(index) else {
                break;
            };
            industry.newgrf_persistent_regs.insert(index, value);
        }
    }
}

/// Reasocia las industrias importadas desde `INDY` al catálogo `NewGRF` activo.
///
/// `OpenTTD` no vuelve a ejecutar los callbacks de fundación al cargar una
/// partida: las listas `Industry::accepted`/`Industry::produced` serializadas
/// son la fuente de verdad. El importador conserva esas filas en
/// `GameState::sav_industry_histories`; cuando el catálogo se aplica después
/// del SAV, esta pasada vuelve a enlazar el `IndustrySpecDef` y reconstruye los
/// cargos efectivos, tasas, multiplicadores y stocks sin consumir callbacks ni
/// cambiar el estado aleatorio de la industria.
#[allow(clippy::too_many_lines)]
pub(crate) fn rehydrate_sav_industries_with_catalog(state: &mut GameState) -> usize {
    if state.sav_industry_histories.is_empty() || state.industry_spec_catalog.is_empty() {
        return 0;
    }

    let saved_rows = state.sav_industry_histories.clone();
    let catalog = state.industry_spec_catalog.clone();
    let overrides = state.industry_overrides.clone();
    let cargo_catalog = state.cargo_spec_catalog.clone();
    let climate = state.climate;
    let save_version = state.sav_version.unwrap_or(SAV_GLOBAL_CARGO_SLOTS_VERSION);
    let mut rehydrated = 0;

    for (index, industry) in state.industries.iter_mut().enumerate() {
        let Some(saved) = saved_rows
            .iter()
            .find(|saved| u16::try_from(saved.industry_id).ok() == Some(industry.instance_id))
            .or_else(|| saved_rows.iter().find(|saved| saved.pos == industry.pos))
            .or_else(|| saved_rows.get(index))
            .cloned()
        else {
            continue;
        };

        industry.valid_history = saved.valid_history;

        let clean_id = u16::from(saved.industry_type);
        let translated_id = crate::industry_spec::get_translated_industry_id(clean_id, &overrides);
        let Some(def) = catalog.iter().find(|def| def.id == translated_id).cloned() else {
            // Uninstalled GRF: retain the explicit vanilla/fallback instance
            // and its opaque INDY row. It can be retried if the stack changes.
            continue;
        };

        let needs_rebind = industry.newgrf_type_id != Some(def.id);
        let first_attachment = industry.newgrf_type_id.is_none();
        if needs_rebind {
            *industry =
                industry
                    .clone()
                    .with_newgrf_spec_and_cargo_catalog(def.id, &def, &cargo_catalog);
        }

        // A modern INDY row always contains the dynamic vectors, including
        // vectors made exclusively of INVALID_CARGO placeholders. Empty
        // vectors are left to `with_newgrf_spec` because old saves did not
        // serialize the reorganised cargo lists.
        let has_saved_slots = !saved.produced.is_empty() || !saved.accepted.is_empty();
        let dynamic_callbacks =
            def.has_input_cargo_types_callback() || def.has_output_cargo_types_callback();
        industry.newgrf_dynamic_cargo_types = dynamic_callbacks;
        if has_saved_slots {
            let input_slots = saved
                .accepted
                .iter()
                .map(|entry| {
                    cargo_from_sav_slot(entry.cargo_slot, climate, &cargo_catalog, save_version)
                })
                .collect::<Vec<_>>();
            let output_slots = saved
                .produced
                .iter()
                .map(|entry| {
                    cargo_from_sav_slot(entry.cargo_slot, climate, &cargo_catalog, save_version)
                })
                .collect::<Vec<_>>();
            industry.newgrf_input_cargo_slots.clone_from(&input_slots);
            industry.newgrf_output_cargo_slots.clone_from(&output_slots);

            let valid_inputs = input_slots.iter().flatten().copied().collect::<Vec<_>>();
            let valid_outputs = output_slots.iter().flatten().copied().collect::<Vec<_>>();
            industry.newgrf_output_cargo = valid_outputs.first().copied();
            industry.newgrf_secondary_output_cargo = valid_outputs.get(1).copied();
            industry.newgrf_extra_output_cargos = valid_outputs.iter().copied().skip(2).collect();

            let saved_rates = saved
                .produced
                .iter()
                .zip(output_slots.iter())
                .filter_map(|(entry, cargo)| cargo.map(|cargo| (cargo, entry.rate)))
                .collect::<Vec<_>>();
            if let Some(&(_, rate)) = saved_rates.first() {
                industry.newgrf_production_rate = Some(rate);
            }
            if let Some(&(_, rate)) = saved_rates.get(1) {
                industry.newgrf_secondary_production_rate = Some(rate);
            }
            industry.newgrf_extra_production_rates =
                saved_rates.iter().skip(2).map(|(_, rate)| *rate).collect();

            rebuild_sav_industry_processing(&def, industry, &valid_inputs, &valid_outputs);

            // The first import may have classified custom output slots using
            // the vanilla fallback. Rebuild all per-cargo stocks from their
            // native positions exactly once, before the simulation starts.
            if first_attachment {
                industry.stock = 0;
                industry.secondary_stock = 0;
                industry.newgrf_extra_produced_cargo = crate::cargo::CargoStock::default();
                industry.newgrf_accepted_cargo_waiting = crate::cargo::CargoStock::default();
                industry.newgrf_last_accepted = crate::cargo::CargoStock::default();
                let mut valid_output_slot = 0usize;
                for (slot, cargo) in output_slots.iter().enumerate() {
                    let Some(cargo) = cargo else { continue };
                    let Some(entry) = saved.produced.get(slot) else {
                        continue;
                    };
                    if !entry.history.is_empty() {
                        industry.produced_history.insert(
                            *cargo,
                            entry
                                .history
                                .iter()
                                .take(crate::entity_history::INDUSTRY_HISTORY_RECORDS)
                                .map(|sample| {
                                    crate::entity_history::IndustryProducedHistorySample {
                                        production: sample.production,
                                        transported: sample.transported,
                                    }
                                })
                                .collect(),
                        );
                    }
                    let waiting = u32::from(entry.waiting);
                    match valid_output_slot {
                        0 => industry.stock = waiting,
                        1 => industry.secondary_stock = waiting,
                        _ => industry.newgrf_extra_produced_cargo.set(*cargo, waiting),
                    }
                    valid_output_slot += 1;
                }
                for (slot, cargo) in input_slots.iter().enumerate() {
                    let Some(cargo) = cargo else { continue };
                    let Some(entry) = saved.accepted.get(slot) else {
                        continue;
                    };
                    industry
                        .newgrf_accepted_cargo_waiting
                        .set(*cargo, u32::from(entry.waiting));
                    industry
                        .newgrf_last_accepted
                        .set(*cargo, entry.last_accepted);
                    industry
                        .accepted_accumulated_waiting
                        .set(*cargo, entry.accumulated_waiting);
                    if !entry.history.is_empty() {
                        industry.accepted_history.insert(
                            *cargo,
                            entry
                                .history
                                .iter()
                                .map(|sample| {
                                    crate::entity_history::IndustryAcceptedHistorySample {
                                        accepted: sample.accepted,
                                        waiting: sample.waiting,
                                    }
                                })
                                .collect(),
                        );
                    }
                }
                industry.capacity =
                    INDUSTRY_STOCK_CAPACITY.max(industry.stock.max(industry.secondary_stock));
            }
        }
        rehydrated += 1;
    }
    rehydrated
}

fn industry_source_index(
    def: &crate::industry_spec::IndustrySpecDef,
    cargo: crate::CargoType,
    output: bool,
) -> Option<usize> {
    let (indices, labels) = if output {
        (&def.produced_cargo_indices, &def.produced_cargo_labels)
    } else {
        (&def.accepted_cargo_indices, &def.accepted_cargo_labels)
    };
    labels
        .iter()
        .position(|label| crate::industry_spec::cargo_type_from_label(Some(label)) == Some(cargo))
        .or_else(|| {
            indices
                .iter()
                .position(|&index| crate::CargoType::from_cargo_id(index) == Some(cargo))
        })
}

fn industry_matrix_multiplier(
    def: &crate::industry_spec::IndustrySpecDef,
    input_source: usize,
    output_source: usize,
) -> u16 {
    let output_count = def.produced_cargo_indices.len();
    if output_count == 0 {
        return 0;
    }
    def.input_multipliers
        .get(input_source.saturating_mul(output_count) + output_source)
        .copied()
        .or_else(|| def.input_multipliers.get(input_source).copied())
        .unwrap_or(256)
}

fn rebuild_sav_industry_processing(
    def: &crate::industry_spec::IndustrySpecDef,
    industry: &mut Industry,
    inputs: &[crate::CargoType],
    outputs: &[crate::CargoType],
) {
    let output_sources = outputs
        .iter()
        .map(|&cargo| industry_source_index(def, cargo, true))
        .collect::<Vec<_>>();
    let primary_output_source = output_sources.first().copied().flatten();
    industry.newgrf_processing_inputs = inputs
        .iter()
        .filter_map(|&cargo| {
            let source = industry_source_index(def, cargo, false)?;
            Some(crate::industry::IndustryProcessingInput {
                cargo,
                batch: 8,
                multiplier: primary_output_source.map_or(256, |output| {
                    industry_matrix_multiplier(def, source, output)
                }),
            })
        })
        .collect();
    industry.newgrf_processing_secondary_multipliers = inputs
        .iter()
        .filter_map(|&cargo| industry_source_index(def, cargo, false))
        .map(|input| {
            output_sources
                .get(1)
                .copied()
                .flatten()
                .map_or(0, |output| industry_matrix_multiplier(def, input, output))
        })
        .collect();
    industry.newgrf_processing_extra_multipliers = inputs
        .iter()
        .filter_map(|&cargo| industry_source_index(def, cargo, false))
        .flat_map(|input| {
            output_sources.iter().skip(2).copied().map(move |output| {
                output.map_or(0, |output| industry_matrix_multiplier(def, input, output))
            })
        })
        .collect();
}

/// Hidrata los registros `7C` del aeropuerto de cada estación desde `PSAC`.
///
/// `STNN.normal.airport.psa` referencia la misma tabla nativa que `INDY.psa`,
/// pero el runtime mantiene los registros en la entidad `Station`. Los ceros
/// siguen en `GameState::sav_persistent_storages` para que una futura entidad
/// pueda reclamarlos sin perder bytes durante el round-trip.
pub(crate) fn hydrate_sav_station_persistent_storage(
    state: &mut GameState,
    sav_stations: &[crate::sav::SavStation],
    persistent_storages: &[crate::sav::SavPersistentStorage],
) {
    if sav_stations.is_empty() || persistent_storages.is_empty() {
        return;
    }
    let by_id: std::collections::HashMap<u32, &crate::sav::SavPersistentStorage> =
        persistent_storages
            .iter()
            .map(|storage| (storage.storage_id, storage))
            .collect();
    for station in &mut state.stations {
        let Some(station_id) = station.ottd_station_id else {
            continue;
        };
        let Some(saved) = sav_stations
            .iter()
            .find(|saved| saved.station_id == station_id)
        else {
            continue;
        };
        let Some(storage_id) = saved.airport_persistent_storage_id else {
            continue;
        };
        let Some(storage) = by_id.get(&storage_id) else {
            continue;
        };
        station.newgrf_persistent_storage_id = Some(storage_id);
        for (index, &value) in storage.storage.iter().enumerate() {
            if value == 0 {
                continue;
            }
            let Ok(index) = u8::try_from(index) else {
                break;
            };
            station.newgrf_persistent_regs.insert(index, value);
        }
    }
}

/// Hidrata el PSA de cada pueblo desde `CITY.psa_list` y `PSAC`.
///
/// Un pueblo puede tener una fila por GRFID. Se conservan tanto el índice
/// nativo como los valores no nulos para que los scopes parent de casas y
/// objetos puedan consultar `7C` sin mezclar dos `NewGRF`; la fila densa
/// completa continúa en
/// `GameState::sav_persistent_storages` para el exportador.
pub(crate) fn hydrate_sav_town_persistent_storage(
    state: &mut GameState,
    town_refs: &std::collections::HashMap<u32, Vec<u32>>,
    persistent_storages: &[crate::sav::SavPersistentStorage],
) {
    if town_refs.is_empty() || persistent_storages.is_empty() {
        return;
    }
    let by_id: std::collections::HashMap<u32, &crate::sav::SavPersistentStorage> =
        persistent_storages
            .iter()
            .map(|storage| (storage.storage_id, storage))
            .collect();
    for (&town_index, ids) in town_refs {
        let Ok(town_index) = usize::try_from(town_index) else {
            continue;
        };
        let Some(town) = state.towns.get_mut(town_index) else {
            continue;
        };
        for &storage_id in ids {
            let Some(storage) = by_id.get(&storage_id) else {
                continue;
            };
            // OpenTTD crea a lo sumo un town PSA por GRFID. Un save malformado
            // con referencias duplicadas conserva la primera fila nativa en
            // vez de sobrescribir sus registros.
            if town
                .newgrf_persistent_storage_ids
                .contains_key(&storage.grfid)
            {
                continue;
            }
            town.newgrf_persistent_storage_ids
                .insert(storage.grfid, storage_id);
            let registers = town
                .newgrf_persistent_regs
                .entry(storage.grfid)
                .or_default();
            for (index, &value) in storage.storage.iter().enumerate() {
                if value == 0 {
                    continue;
                }
                let Ok(index) = u8::try_from(index) else {
                    break;
                };
                registers.insert(index, value);
            }
        }
    }
}

/// Hidrata industrias de un `.ottdmap` que no trae el chunk `INDY`.
///
/// La agrupación por componentes sólo es un fallback: los `.sav` modernos
/// siempre prefieren el límite exacto declarado en `INDY`.
pub fn hydrate_industries_from_map_tiles(state: &mut GameState, extras: Option<&OttdmapExtras>) {
    state.industries.clear();
    for component in industry_components(state) {
        let Some(&origin) = component.first() else {
            continue;
        };
        let Some(tile) = state.map.get(origin) else {
            continue;
        };
        let gfx = get_clean_industry_gfx(tile.m5, tile.m6);
        let kind = extras
            .and_then(|extras| {
                extras.industry_type_for_instance(crate::map::industry_instance_id(&tile))
            })
            .map_or_else(|| industry_kind_from_gfx(gfx), industry_kind_from_ottd_type);
        let mut industry = if let Some(spec) = industry_spec_from_gfx(gfx) {
            Industry::with_tiles_spec(
                origin,
                kind,
                spec,
                component,
                industry_random_colour_from_instance(crate::map::industry_instance_id(&tile)),
            )
        } else {
            Industry::with_tiles(origin, kind, component).with_random_colour(
                industry_random_colour_from_instance(crate::map::industry_instance_id(&tile)),
            )
        };
        industry.instance_id = crate::map::industry_instance_id(&tile);
        industry.prod_level = PRODLEVEL_DEFAULT;
        state.industries.push(industry);
    }
}

fn industry_components(state: &GameState) -> Vec<Vec<TileCoord>> {
    let (map_w, map_h) = state.map.dimensions();
    let mut visited = HashSet::new();
    let mut out = Vec::new();

    let map_w = map_w.cast_signed();
    let map_h = map_h.cast_signed();
    for y in 0..map_h {
        for x in 0..map_w {
            let start = TileCoord::new(x, y);
            let Some(start_tile) = state.map.get(start) else {
                continue;
            };
            if start_tile.kind != TileKind::Industry || !visited.insert((x, y)) {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            let mut component = Vec::new();
            while let Some(current) = queue.pop_front() {
                component.push(current);
                let Some(current_tile) = state.map.get(current) else {
                    continue;
                };
                let current_gfx = get_clean_industry_gfx(current_tile.m5, current_tile.m6);
                let current_group = industry_group_from_gfx(current_gfx);
                for next in [
                    TileCoord::new(current.x - 1, current.y),
                    TileCoord::new(current.x + 1, current.y),
                    TileCoord::new(current.x, current.y - 1),
                    TileCoord::new(current.x, current.y + 1),
                ] {
                    if next.x < 0 || next.y < 0 || next.x >= map_w || next.y >= map_h {
                        continue;
                    }
                    let Some(next_tile) = state.map.get(next) else {
                        continue;
                    };
                    if next_tile.kind != TileKind::Industry {
                        continue;
                    }
                    let next_gfx = get_clean_industry_gfx(next_tile.m5, next_tile.m6);
                    let next_group = industry_group_from_gfx(next_gfx);
                    let anonymous_same = current_group == "Unknown gfx"
                        || next_group == "Unknown gfx"
                        || current_group == next_group;
                    if crate::industry_tiles_mergeable(&current_tile, &next_tile, anonymous_same)
                        && visited.insert((next.x, next.y))
                    {
                        queue.push_back(next);
                    }
                }
            }
            out.push(component);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    #[test]
    fn classify_known_gfx_ranges() {
        assert_eq!(industry_spec_from_gfx(7), Some(IndustrySpec::PowerStation));
        assert_eq!(industry_spec_from_gfx(24), Some(IndustrySpec::OilWells));
        assert_eq!(industry_spec_from_gfx(33), Some(IndustrySpec::Farm));
        assert_eq!(industry_kind_from_gfx(48), IndustryKind::CoalMine);
        assert_eq!(industry_group_from_gfx(142), "Toy Factory");
    }

    #[test]
    fn cargo_slots_follow_save_version_and_global_custom_ids() {
        assert_eq!(
            cargo_from_sav_slot(
                6,
                Climate::SubArctic,
                &[],
                SAV_GLOBAL_CARGO_SLOTS_VERSION - 1,
            ),
            Some(crate::CargoType::Wheat)
        );
        assert_eq!(
            cargo_from_sav_slot(6, Climate::SubArctic, &[], SAV_GLOBAL_CARGO_SLOTS_VERSION),
            Some(crate::CargoType::Grain)
        );
        assert_eq!(
            cargo_from_sav_slot(11, Climate::SubArctic, &[], SAV_GLOBAL_CARGO_SLOTS_VERSION),
            Some(crate::CargoType::Wheat)
        );
        assert_eq!(
            cargo_from_sav_slot(42, Climate::Temperate, &[], SAV_GLOBAL_CARGO_SLOTS_VERSION),
            Some(crate::CargoType::Custom(11))
        );
        assert_eq!(
            cargo_from_sav_slot(
                42,
                Climate::Temperate,
                &[],
                SAV_GLOBAL_CARGO_SLOTS_VERSION - 1
            ),
            None
        );
    }

    #[test]
    #[allow(clippy::expect_used)] // fixture fijo: una tesela ausente es un bug del test
    #[allow(clippy::too_many_lines)] // fixture de round-trip cubre todos los campos de INDY
    fn indy_hydration_uses_real_rect_production_and_counter() {
        let mut state = GameState::from_map(Map::new_flat(8, 8, 0));
        state.climate = Climate::Temperate;
        for (x, y, gfx) in [(2, 2, 0u8), (2, 3, 1)] {
            let coord = TileCoord::new(x, y);
            let mut tile = state.map.get(coord).expect("fixture tile");
            tile.kind = TileKind::Industry;
            tile.m5 = gfx;
            tile.m2 = 7;
            state.map.set_tile(coord, tile).expect("set fixture tile");
        }
        let saved = SavIndustry {
            industry_id: 7,
            pos: TileCoord::new(2, 2),
            width: 1,
            height: 2,
            neutral_station_id: None,
            industry_type: 0,
            random_colour: 14,
            counter: 123,
            selected_layout: 2,
            random: 0xBEEF,
            last_prod_year: 1972,
            was_cargo_delivered: true,
            control_flags: 5,
            exclusive_supplier: None,
            founder: Some(2),
            construction_date: crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17,
            construction_type: crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION,
            prod_level: 32,
            valid_history: 0xA,
            persistent_storage_id: None,
            produced: vec![
                super::super::entities::SavIndustryProducedCargo {
                    cargo_slot: 1,
                    waiting: 77,
                    rate: 15,
                    history: vec![super::super::entities::SavIndustryProducedHistory {
                        production: 42,
                        transported: 17,
                    }],
                },
                super::super::entities::SavIndustryProducedCargo {
                    cargo_slot: 9,
                    waiting: 22,
                    rate: 4,
                    history: Vec::new(),
                },
            ],
            accepted: vec![super::super::entities::SavIndustryAcceptedCargo {
                cargo_slot: 6,
                waiting: 15,
                last_accepted: 10_974,
                accumulated_waiting: 77,
                history: vec![super::super::entities::SavIndustryAcceptedHistory {
                    accepted: 12,
                    waiting: 8,
                }],
            }],
        };

        hydrate_sav_industries(&mut state, &[saved], &OttdmapExtras::default());

        assert_eq!(state.industries.len(), 1);
        let industry = &state.industries[0];
        assert_eq!(industry.tiles.len(), 2);
        assert_eq!(industry.spec, Some(IndustrySpec::CoalMine));
        assert_eq!(industry.stock, 77);
        assert_eq!(
            industry.produced_history_for(crate::CargoType::Coal),
            Some(
                [crate::entity_history::IndustryProducedHistorySample {
                    production: 42,
                    transported: 17,
                }]
                .as_slice()
            )
        );
        assert_eq!(industry.extra_produced_cargo(crate::CargoType::Steel), 22);
        assert_eq!(industry.accepted_cargo_waiting(crate::CargoType::Grain), 15);
        assert_eq!(industry.last_accepted_date(crate::CargoType::Grain), 10_974);
        assert_eq!(industry.counter, 123);
        assert_eq!(industry.selected_layout, 2);
        assert_eq!(industry.newgrf_random, 0xBEEF);
        assert_eq!(industry.last_prod_year, 1972);
        assert!(industry.was_cargo_delivered);
        assert_eq!(industry.control_flags, 5);
        assert_eq!(industry.founder, Some(crate::company::CompanyId(2)));
        assert_eq!(
            industry.construction_date,
            crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17
        );
        assert_eq!(
            industry.construction_type,
            crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION
        );
        assert_eq!(industry.prod_level, 32);
        assert_eq!(industry.valid_history, 0xA);
        assert_eq!(
            industry
                .accepted_accumulated_waiting
                .get(crate::CargoType::Grain),
            77
        );
        assert_eq!(
            industry.accepted_history_for(crate::CargoType::Grain),
            Some(
                [crate::entity_history::IndustryAcceptedHistorySample {
                    accepted: 12,
                    waiting: 8,
                }]
                .as_slice()
            )
        );
        assert_eq!(industry.random_colour, 14);
    }
}
