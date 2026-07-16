//! Consultas de catálogo: disponibilidad, depósito y helpers de render.

use crate::vehicle::VehicleKind;

use super::catalog_data::{
    ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_TRICARIO, ENGINE_BUS_MPS, ENGINE_SHIP_MPS,
    ENGINE_TRAIN_KIRBY, ENGINE_TRAM_MPS, ENGINE_TRUCK_MPS, RELIABILITY_ELECTRIC, RELIABILITY_STEAM,
    engines_table,
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
    calendar_year >= u32::from(engine.intro_year)
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
    list
}

/// Agrupa `train_image_index` en uno de los conjuntos de sprites descargados.
#[must_use]
pub const fn train_sprite_group(image_index: u8) -> u8 {
    match image_index {
        0 | 3 | 7 | 10 => 0,
        1 | 9 => 1,
        4 | 5 | 6 | 8 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 22 => 3,
        20 | 21 | 23 => 4,
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

    use super::super::catalog_data::{ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_KIRBY};

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
    fn train_sprite_group_maps_indices() {
        assert_eq!(train_sprite_group(2), 2);
        assert_eq!(train_sprite_group(23), 4);
    }
}
