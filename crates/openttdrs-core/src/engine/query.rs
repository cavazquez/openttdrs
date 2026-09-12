//! Consultas de catálogo: disponibilidad, depósito y helpers de render.

use crate::vehicle::VehicleKind;
use std::collections::HashSet;

use super::catalog_data::{
    ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_FOKKER, ENGINE_AIRCRAFT_TRICARIO, ENGINE_BUS_MPS,
    ENGINE_SHIP_MPS, ENGINE_TRAIN_KIRBY, ENGINE_TRAM_MPS, ENGINE_TRUCK_MPS, RELIABILITY_ELECTRIC,
    RELIABILITY_STEAM, engines_table,
};
use super::model::{EngineDef, NEWGRF_ENGINE_ID_BASE};

/// Siguiente ID libre en el rango `NewGRF` (≥ [`NEWGRF_ENGINE_ID_BASE`]).
#[must_use]
pub fn next_free_engine_id(catalog: &[EngineDef]) -> Option<u16> {
    (NEWGRF_ENGINE_ID_BASE..=u16::MAX).find(|&id| !catalog.iter().any(|e| e.id == id))
}

/// Busca un motor en un catálogo runtime.
#[must_use]
pub fn engine_in_catalog(catalog: &[EngineDef], id: u16) -> Option<&EngineDef> {
    catalog.iter().find(|e| e.id == id)
}

/// Bits de `EngineInfo::extra_flags` que afectan al ciclo de vida del motor.
///
/// `OpenTTD` define estos bits como una máscara `DWord` en Action0 `0x21`.
/// Mantenerlos nombrados evita que los consumidores runtime tengan que repetir
/// números mágicos y deja el resto de la máscara disponible para futuras
/// propiedades.
pub const EXTRA_ENGINE_FLAG_NO_NEWS: u32 = 1 << 0;
pub const EXTRA_ENGINE_FLAG_NO_PREVIEW: u32 = 1 << 1;
pub const EXTRA_ENGINE_FLAG_JOIN_PREVIEW: u32 = 1 << 2;
pub const EXTRA_ENGINE_FLAG_SYNC_RELIABILITY: u32 = 1 << 3;

/// Estado de introducción que puede observar el ciclo de previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineLifecycleState {
    /// Todavía no llegó el año de introducción.
    NotIntroduced,
    /// El motor está en la ventana de preview exclusiva de un año.
    ExclusivePreview,
    /// `NoPreview`: llegó la fecha, pero espera la disponibilidad general.
    PendingAvailability,
    /// El motor ya se puede ofrecer a todas las compañías.
    Available,
    /// La vida comercial del modelo terminó o quedó inválida.
    Retired,
}

/// Devuelve el motor cuya edad y fiabilidad debe compartir `engine`.
///
/// `SyncReliability` sigue la cadena de variantes hacia el padre, como
/// `CalcEngineReliability` en `OpenTTD`. Un enlace ausente o cíclico deja la
/// definición actual como fuente segura para que un catálogo parcialmente
/// materializado no bloquee la creación del vehículo.
#[must_use]
pub fn engine_reliability_source<'a>(
    engine: &'a EngineDef,
    catalog: &'a [EngineDef],
) -> &'a EngineDef {
    let mut source = engine;
    let fallback = engine;
    let mut visited = HashSet::new();
    loop {
        if source.extra_flags & EXTRA_ENGINE_FLAG_SYNC_RELIABILITY == 0
            || !visited.insert(source.id)
        {
            return source;
        }
        let Some(parent_id) = source.variant_parent_id else {
            return source;
        };
        if parent_id == source.id || visited.contains(&parent_id) {
            return fallback;
        }
        let Some(parent) = catalog.iter().find(|candidate| candidate.id == parent_id) else {
            return source;
        };
        source = parent;
    }
}

/// Motores de un tipo de vehículo concreto (orden del catálogo).
pub fn engines_of_kind(kind: VehicleKind) -> impl Iterator<Item = &'static EngineDef> {
    engines_table().iter().filter(move |e| e.kind == kind)
}

/// Orden de la lista de compra en ventana de depósito.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineCatalogSort {
    #[default]
    Catalog,
    Name,
    Price,
    Speed,
    IntroYear,
}

/// Filtro de carretera en ventana de compra (ignorado en depósito de vía).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoadEngineFilter {
    #[default]
    All,
    BusOnly,
    TruckOnly,
    TramOnly,
}

/// `true` si el modelo ya está disponible en el año calendario dado.
#[must_use]
pub fn engine_available_in_year(engine: &EngineDef, calendar_year: u32) -> bool {
    let intro = u32::from(engine.intro_year);
    let available_years =
        u32::from(engine.model_life_years).saturating_sub(u32::from(engine.retire_early_years));
    calendar_year >= intro
        && (engine.model_life_years == u8::MAX
            || calendar_year < intro.saturating_add(available_years))
}

/// Estado de introducción anual alineado con `CalendarEnginesMonthlyLoop`.
#[must_use]
pub fn engine_lifecycle_state_in_year(
    engine: &EngineDef,
    calendar_year: u32,
) -> EngineLifecycleState {
    let intro = u32::from(engine.intro_year);
    if calendar_year < intro {
        return EngineLifecycleState::NotIntroduced;
    }
    if !engine_available_in_year(engine, calendar_year) {
        return EngineLifecycleState::Retired;
    }
    if calendar_year == intro {
        return if engine.no_preview() {
            EngineLifecycleState::PendingAvailability
        } else {
            EngineLifecycleState::ExclusivePreview
        };
    }
    EngineLifecycleState::Available
}

/// Devuelve el grupo de preview de un motor, incluyendo sólo variantes que
/// declaran `JoinPreview` en cada enlace de la cadena.
#[must_use]
pub fn engine_preview_group_in(catalog: &[EngineDef], root_id: u16) -> Vec<&EngineDef> {
    let Some(root) = engine_in_catalog(catalog, root_id) else {
        return Vec::new();
    };
    let mut group = vec![root];
    let mut frontier = vec![root.id];
    let mut visited = HashSet::from([root.id]);
    while let Some(parent_id) = frontier.pop() {
        for candidate in catalog.iter().filter(|candidate| {
            candidate.variant_parent_id == Some(parent_id) && candidate.joins_preview()
        }) {
            if visited.insert(candidate.id) {
                group.push(candidate);
                frontier.push(candidate.id);
            }
        }
    }
    group
}

/// Busca la raíz de preview de un motor y corta enlaces inexistentes o cíclicos.
#[must_use]
pub fn engine_preview_root_id_in(catalog: &[EngineDef], engine_id: u16) -> Option<u16> {
    let mut current = engine_in_catalog(catalog, engine_id)?;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.id) || !current.joins_preview() {
            return Some(current.id);
        }
        let Some(parent_id) = current.variant_parent_id else {
            return Some(current.id);
        };
        let Some(parent) = engine_in_catalog(catalog, parent_id) else {
            return Some(current.id);
        };
        current = parent;
    }
}

/// Grupo de preview para un motor, resolviendo primero la raíz del enlace.
#[must_use]
pub fn engine_preview_group_for_in(catalog: &[EngineDef], engine_id: u16) -> Vec<&EngineDef> {
    let Some(root_id) = engine_preview_root_id_in(catalog, engine_id) else {
        return Vec::new();
    };
    engine_preview_group_in(catalog, root_id)
}

/// Motores visibles en la ventana de compra de un depósito, filtrados y ordenados.
#[must_use]
pub fn engines_for_depot_purchase(
    depot_is_rail: bool,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&'static EngineDef> {
    engines_for_depot_kind(
        if depot_is_rail {
            DepotPurchaseKind::Rail
        } else {
            DepotPurchaseKind::Road
        },
        calendar_year,
        sort,
        road_filter,
    )
}

/// Tipo de depósito para filtrar el catálogo de compra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepotPurchaseKind {
    Rail,
    Road,
    Ship,
    Aircraft,
}

/// Motores visibles según el tipo de depósito.
#[must_use]
pub fn engines_for_depot_kind(
    depot_kind: DepotPurchaseKind,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&'static EngineDef> {
    engines_for_depot_kind_in(
        engines_table(),
        depot_kind,
        calendar_year,
        sort,
        road_filter,
    )
}

/// Como [`engines_for_depot_kind`] sobre un catálogo runtime (vanilla + `NewGRF`).
#[must_use]
pub fn engines_for_depot_kind_in(
    catalog: &[EngineDef],
    depot_kind: DepotPurchaseKind,
    calendar_year: u32,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&EngineDef> {
    let mut list: Vec<&EngineDef> = catalog
        .iter()
        .filter(|engine| {
            if !engine_available_in_year(engine, calendar_year) {
                return false;
            }
            match (depot_kind, engine.kind) {
                (DepotPurchaseKind::Rail, VehicleKind::Train)
                | (DepotPurchaseKind::Ship, VehicleKind::Ship)
                | (DepotPurchaseKind::Aircraft, VehicleKind::Aircraft) => true,
                (DepotPurchaseKind::Road, VehicleKind::Bus) => {
                    road_filter != RoadEngineFilter::TruckOnly
                        && road_filter != RoadEngineFilter::TramOnly
                }
                (DepotPurchaseKind::Road, VehicleKind::Truck) => {
                    road_filter != RoadEngineFilter::BusOnly
                        && road_filter != RoadEngineFilter::TramOnly
                }
                (DepotPurchaseKind::Road, VehicleKind::Tram) => {
                    road_filter != RoadEngineFilter::BusOnly
                        && road_filter != RoadEngineFilter::TruckOnly
                }
                _ => false,
            }
        })
        .collect();
    match sort {
        EngineCatalogSort::Catalog => {}
        EngineCatalogSort::Name => list.sort_by_key(|e| e.name.as_str()),
        EngineCatalogSort::Price => list.sort_by_key(|e| e.price),
        EngineCatalogSort::Speed => list.sort_by_key(|e| std::cmp::Reverse(e.max_speed)),
        EngineCatalogSort::IntroYear => list.sort_by_key(|e| e.intro_year),
    }
    order_engine_variants(&mut list);
    list
}

/// Reordena una lista de compra como un árbol plano: cada motor padre aparece
/// antes que sus variantes directas y los hermanos conservan el orden que ya
/// determinó el filtro/orden elegido.
pub fn order_engine_variants(list: &mut Vec<&EngineDef>) {
    fn append_tree<'a>(
        engine: &'a EngineDef,
        source: &[&'a EngineDef],
        visited: &mut HashSet<u16>,
        ordered: &mut Vec<&'a EngineDef>,
    ) {
        if !visited.insert(engine.id) {
            return;
        }
        ordered.push(engine);
        for child in source
            .iter()
            .filter(|candidate| candidate.variant_parent_id == Some(engine.id))
        {
            append_tree(child, source, visited, ordered);
        }
    }

    let source = list.clone();
    let mut visited = HashSet::new();
    let mut ordered = Vec::with_capacity(source.len());
    for engine in &source {
        let parent_is_visible = engine
            .variant_parent_id
            .is_some_and(|parent| source.iter().any(|candidate| candidate.id == parent));
        if !parent_is_visible {
            append_tree(engine, &source, &mut visited, &mut ordered);
        }
    }
    for engine in &source {
        append_tree(engine, &source, &mut visited, &mut ordered);
    }
    *list = ordered;
}

/// Agrupa `train_image_index` en uno de los conjuntos de sprites descargados.
#[must_use]
pub const fn train_sprite_group(image_index: u8) -> u8 {
    match image_index {
        0 | 3 | 7 | 10 => 0,
        1 | 9 => 1,
        4 | 5 | 6 | 8 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 22 => 3,
        20 | 21 | 23 => 4,
        33 => 5, // Passenger Carriage
        35 => 6, // Mail Van
        38 => 7, // Goods Van
        34 => 8, // Coal Truck
        _ => 2,
    }
}

/// Busca un motor por id, sin importar el tipo.
#[must_use]
pub fn engine_by_id(id: u16) -> Option<&'static EngineDef> {
    engines_table().iter().find(|e| e.id == id)
}

/// Tipo de humo/chispas de locomotora según fiabilidad/clase del motor.
#[must_use]
pub fn train_smoke_kind(engine_id: u16) -> crate::sim_events::TrainSmokeKind {
    let engine = engine_by_id(engine_id).unwrap_or_else(|| {
        engine_for_vehicle(VehicleKind::Train, default_engine_id(VehicleKind::Train))
    });
    match engine.reliability_pct {
        RELIABILITY_STEAM => crate::sim_events::TrainSmokeKind::Steam,
        RELIABILITY_ELECTRIC => crate::sim_events::TrainSmokeKind::Electric,
        _ => crate::sim_events::TrainSmokeKind::Diesel,
    }
}

#[must_use]
pub const fn default_engine_id(kind: VehicleKind) -> u16 {
    match kind {
        VehicleKind::Bus => ENGINE_BUS_MPS,
        VehicleKind::Truck => ENGINE_TRUCK_MPS,
        VehicleKind::Tram => ENGINE_TRAM_MPS,
        VehicleKind::Train => ENGINE_TRAIN_KIRBY,
        VehicleKind::Ship => ENGINE_SHIP_MPS,
        VehicleKind::Aircraft => ENGINE_AIRCRAFT_DAKOTA,
    }
}

/// ¿El motor aéreo es helicóptero (solo helipuertos 1×1)?
#[must_use]
pub const fn aircraft_is_helicopter(engine_id: u16) -> bool {
    engine_id == ENGINE_AIRCRAFT_TRICARIO
}

/// Helicóptero por flag Action0 `0x09` o id vanilla (Tricario).
#[must_use]
pub fn aircraft_is_helicopter_def(engine: &EngineDef) -> bool {
    engine.is_helicopter || aircraft_is_helicopter(engine.id)
}

/// ¿El motor aéreo es jet (`AIR_FAST`)? Catálogo reducido: Fokker F27.
#[must_use]
pub const fn aircraft_is_jet(engine_id: u16) -> bool {
    engine_id == ENGINE_AIRCRAFT_FOKKER
}

#[must_use]
pub fn engine_for_vehicle(kind: VehicleKind, id: u16) -> &'static EngineDef {
    if let Some(engine) = engines_table()
        .iter()
        .find(|engine| engine.kind == kind && engine.id == id)
    {
        return engine;
    }
    engine_for_vehicle(kind, default_engine_id(kind))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::vehicle::VehicleKind;

    use super::super::catalog_data::{ENGINE_SHIP_OIL, ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_KIRBY};

    #[test]
    fn engines_for_depot_purchase_filters_by_year_and_kind() {
        let list = engines_for_depot_purchase(
            true,
            1950,
            EngineCatalogSort::Catalog,
            RoadEngineFilter::All,
        );
        assert!(list.iter().any(|e| e.id == ENGINE_TRAIN_KIRBY));
        assert!(!list.iter().any(|e| e.id == ENGINE_TRAIN_ASIASTAR));

        let road = engines_for_depot_purchase(
            false,
            1950,
            EngineCatalogSort::Catalog,
            RoadEngineFilter::BusOnly,
        );
        assert!(road.iter().all(|e| e.kind == VehicleKind::Bus));
    }

    #[test]
    fn model_life_retires_engine_but_ff_never_expires() {
        let mut engine = engine_for_vehicle(VehicleKind::Bus, ENGINE_BUS_MPS).clone();
        engine.intro_year = 1950;
        engine.model_life_years = 10;
        assert!(!engine_available_in_year(&engine, 1949));
        assert!(engine_available_in_year(&engine, 1950));
        assert!(engine_available_in_year(&engine, 1959));
        assert!(!engine_available_in_year(&engine, 1960));

        engine.model_life_years = u8::MAX;
        assert!(engine_available_in_year(&engine, 2200));
    }

    #[test]
    fn lifecycle_distinguishes_preview_no_preview_and_retirement() {
        let mut engine = engine_for_vehicle(VehicleKind::Bus, ENGINE_BUS_MPS).clone();
        engine.intro_year = 1950;
        engine.model_life_years = 3;
        assert_eq!(
            engine_lifecycle_state_in_year(&engine, 1949),
            EngineLifecycleState::NotIntroduced
        );
        assert_eq!(
            engine_lifecycle_state_in_year(&engine, 1950),
            EngineLifecycleState::ExclusivePreview
        );
        assert_eq!(
            engine_lifecycle_state_in_year(&engine, 1951),
            EngineLifecycleState::Available
        );
        assert_eq!(
            engine_lifecycle_state_in_year(&engine, 1953),
            EngineLifecycleState::Retired
        );

        engine.extra_flags = EXTRA_ENGINE_FLAG_NO_PREVIEW;
        assert_eq!(
            engine_lifecycle_state_in_year(&engine, 1950),
            EngineLifecycleState::PendingAvailability
        );
    }

    #[test]
    fn preview_group_follows_join_preview_links_without_cycles() {
        let mut parent = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_MPS).clone();
        parent.id = 20_101;
        let mut child = parent.clone();
        child.id = 20_102;
        child.variant_parent_id = Some(parent.id);
        child.extra_flags = EXTRA_ENGINE_FLAG_JOIN_PREVIEW;
        let mut grandchild = child.clone();
        grandchild.id = 20_103;
        grandchild.variant_parent_id = Some(child.id);
        let mut separate = child.clone();
        separate.id = 20_104;
        separate.extra_flags = 0;
        let catalog = vec![parent, child, grandchild, separate];

        assert_eq!(engine_preview_root_id_in(&catalog, 20_103), Some(20_101));
        assert_eq!(
            engine_preview_group_for_in(&catalog, 20_103)
                .iter()
                .map(|engine| engine.id)
                .collect::<Vec<_>>(),
            vec![20_101, 20_102, 20_103]
        );
        assert_eq!(engine_preview_root_id_in(&catalog, 20_104), Some(20_104));
    }

    #[test]
    fn train_sprite_group_maps_indices() {
        assert_eq!(train_sprite_group(2), 2);
        assert_eq!(train_sprite_group(23), 4);
        assert_eq!(train_sprite_group(33), 5);
        assert_eq!(train_sprite_group(35), 6);
        assert_eq!(train_sprite_group(38), 7);
        assert_eq!(train_sprite_group(34), 8);
    }

    #[test]
    fn mps_regal_speed_matches_openttd_internal_units() {
        let mps = engine_for_vehicle(VehicleKind::Bus, ENGINE_BUS_MPS);
        assert_eq!(mps.max_speed, 112);
        assert_eq!(mps.speed_kmh(), 56);
    }

    #[test]
    fn variant_parent_is_listed_before_child() {
        let parent = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_MPS).clone();
        let mut child = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_OIL).clone();
        child.variant_parent_id = Some(parent.id);
        let mut list = vec![&child, &parent];
        order_engine_variants(&mut list);
        assert_eq!(
            list.iter().map(|engine| engine.id).collect::<Vec<_>>(),
            vec![parent.id, child.id,]
        );
    }

    #[test]
    fn sync_reliability_uses_variant_parent_source() {
        let mut parent = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_MPS).clone();
        parent.id = 20_001;
        parent.reliability_pct = 61;
        parent.reliability_spd_dec = 44;
        parent.lifelength_years = 17;
        let mut child = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_OIL).clone();
        child.id = 20_002;
        child.variant_parent_id = Some(parent.id);
        child.extra_flags = EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;
        child.reliability_pct = 91;
        child.reliability_spd_dec = 99;
        child.lifelength_years = 30;

        let catalog = [parent.clone(), child.clone()];
        let source = engine_reliability_source(&child, &catalog);
        assert_eq!(source.id, parent.id);
        assert_eq!(source.reliability_pct, 61);
        assert_eq!(source.reliability_spd_dec, 44);
        assert_eq!(source.lifelength_years, 17);
    }

    #[test]
    fn sync_reliability_stops_at_missing_or_cyclic_parent() {
        let mut missing = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_OIL).clone();
        missing.id = 20_003;
        missing.variant_parent_id = Some(20_004);
        missing.extra_flags = EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;
        assert_eq!(
            engine_reliability_source(&missing, &[missing.clone()]).id,
            missing.id
        );

        let mut first = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_MPS).clone();
        first.id = 20_005;
        first.variant_parent_id = Some(20_006);
        first.extra_flags = EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;
        let mut second = engine_for_vehicle(VehicleKind::Ship, ENGINE_SHIP_OIL).clone();
        second.id = 20_006;
        second.variant_parent_id = Some(first.id);
        second.extra_flags = EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;
        let catalog = [first.clone(), second];
        let source = engine_reliability_source(&first, &catalog);
        assert_eq!(source.id, first.id);
    }
}
