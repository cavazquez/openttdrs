//! Hidratación semántica común de entidades de un `.sav`.
//!
//! El decoder conserva los datos de chunks (`INDY`, `CAPA`, `STNN`); este
//! módulo los convierte a las estructuras de simulación. Mantenerlo en core
//! evita que el cliente y las herramientas de línea de comandos discrepen
//! sobre qué industrias y qué carga contiene una misma partida.

use std::collections::{HashSet, VecDeque};

use crate::industry::{INDUSTRY_STOCK_CAPACITY, PRODLEVEL_DEFAULT};
use crate::industry_tile::get_clean_industry_gfx;
use crate::{
    Climate, GameState, Industry, IndustryKind, IndustrySpec, OttdmapExtras, TileCoord, TileKind,
};

use super::entities::SavIndustry;

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
        industry.last_prod_year = saved.last_prod_year;
        industry.was_cargo_delivered = saved.was_cargo_delivered;
        industry.control_flags = saved.control_flags;
        industry.founder = saved.founder.map(crate::company::CompanyId);
        industry.construction_date = saved.construction_date;
        industry.construction_type = saved.construction_type;
        industry.prod_level = saved.prod_level;
        import_industry_output_stock(&mut industry, saved, state.climate);
        state.industries.push(industry);
    }
}

fn import_industry_output_stock(industry: &mut Industry, saved: &SavIndustry, climate: Climate) {
    let outputs = industry.produced_cargos();
    for produced in &saved.produced {
        let Some(cargo) = crate::CargoType::from_climate_slot(climate, produced.cargo_slot) else {
            continue;
        };
        let waiting = u32::from(produced.waiting);
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
        let Some(cargo) = crate::CargoType::from_climate_slot(climate, accepted.cargo_slot) else {
            continue;
        };
        industry.add_accepted_cargo_waiting(cargo, u32::from(accepted.waiting));
        industry.set_last_accepted_date(cargo, accepted.last_accepted);
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
    #[allow(clippy::expect_used)] // fixture fijo: una tesela ausente es un bug del test
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
            industry_type: 0,
            random_colour: 14,
            counter: 123,
            selected_layout: 2,
            random: 0xBEEF,
            last_prod_year: 1972,
            was_cargo_delivered: true,
            control_flags: 5,
            founder: Some(2),
            construction_date: crate::industry::OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR + 17,
            construction_type: crate::industry::INDUSTRY_CONSTRUCTION_MAP_GENERATION,
            prod_level: 32,
            valid_history: 0,
            persistent_storage_id: None,
            produced: vec![
                super::super::entities::SavIndustryProducedCargo {
                    cargo_slot: 1,
                    waiting: 77,
                    rate: 15,
                    history: Vec::new(),
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
                accumulated_waiting: 0,
                history: Vec::new(),
            }],
        };

        hydrate_sav_industries(&mut state, &[saved], &OttdmapExtras::default());

        assert_eq!(state.industries.len(), 1);
        let industry = &state.industries[0];
        assert_eq!(industry.tiles.len(), 2);
        assert_eq!(industry.spec, Some(IndustrySpec::CoalMine));
        assert_eq!(industry.stock, 77);
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
        assert_eq!(industry.random_colour, 14);
    }
}
