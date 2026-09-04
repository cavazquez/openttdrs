//! Métricas del consist: capacidad, peso, potencia, longitud en teselas.

use crate::cargo::CargoType;
use crate::engine::engine_by_id;
use crate::map::TileCoord;
use crate::vehicle::Vehicle;

use super::topology::consist_unit_ids;

pub(crate) fn cargo_unit_weight_16ths(cargo: Option<CargoType>) -> u8 {
    cargo_unit_weight_16ths_with_catalog(cargo, &[])
}

pub(crate) fn cargo_unit_weight_16ths_with_catalog(
    cargo: Option<CargoType>,
    catalog: &[crate::cargo_spec::CargoSpecDef],
) -> u8 {
    let fallback = match cargo {
        Some(CargoType::Passengers) => 1,
        Some(
            CargoType::Mail
            | CargoType::Valuables
            | CargoType::Batteries
            | CargoType::Toys
            | CargoType::FizzyDrinks,
        ) => 4,
        Some(
            CargoType::Goods
            | CargoType::Livestock
            | CargoType::Candy
            | CargoType::Bubbles
            | CargoType::Gold
            | CargoType::Diamonds,
        ) => 8,
        Some(_) => 16,
        None => 0,
    };
    let Some(cargo) = cargo else {
        return fallback;
    };
    catalog
        .iter()
        .find(|spec| spec.cargo_type() == Some(cargo))
        .and_then(|spec| (spec.weight > 0).then_some(spec.weight))
        .unwrap_or(fallback)
}

/// Peso de una carga en toneladas enteras como `CargoSpec::WeightOfNUnits`.
///
/// Aunque nació junto a las métricas de consist, la misma conversión la usa
/// `RoadVehicle::GetWeight`; centralizarla evita que trenes y carretera
/// redondeen distinto un mismo `CargoSpec` vanilla.
#[must_use]
pub(crate) fn cargo_weight_t(
    cargo: u32,
    cargo_type: Option<CargoType>,
    catalog: &[crate::cargo_spec::CargoSpecDef],
) -> u16 {
    let sixteenths =
        u64::from(cargo) * u64::from(cargo_unit_weight_16ths_with_catalog(cargo_type, catalog));
    u16::try_from(sixteenths / 16).unwrap_or(u16::MAX)
}

/// Peso de carga ferroviaria aplicando `vehicle.freight_trains`.
///
/// `OpenTTD` multiplica sólo los cargos cuyo `CargoSpec::is_freight` está activo;
/// para cargos vanilla o specs ausentes se conserva la clasificación conocida
/// de [`CargoType`]. El ajuste mínimo válido del setting es uno.
#[must_use]
pub(crate) fn train_cargo_weight_t(
    cargo: u32,
    cargo_type: Option<CargoType>,
    catalog: &[crate::cargo_spec::CargoSpecDef],
    freight_trains: u8,
) -> u16 {
    let multiplier = if cargo_type.is_some_and(|cargo_type| {
        catalog
            .iter()
            .find(|spec| spec.cargo_type() == Some(cargo_type))
            .map_or(cargo_type.is_freight(), |spec| spec.is_freight)
    }) {
        u32::from(freight_trains.max(1))
    } else {
        1
    };
    cargo_weight_t(cargo.saturating_mul(multiplier), cargo_type, catalog)
}

/// Capacidad total del consist (cabeza).
#[must_use]
pub fn consist_capacity(vehicles: &[Vehicle], head_id: u32) -> u32 {
    vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(0, |v| v.capacity)
}

/// Peso total (t) del consist.
#[must_use]
pub fn consist_weight_t(vehicles: &[Vehicle], head_id: u32) -> u16 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.weight_t))
        .fold(0_u16, u16::saturating_add)
}

/// Potencia total (HP) del consist.
#[must_use]
pub fn consist_power_hp(vehicles: &[Vehicle], head_id: u32) -> u32 {
    consist_unit_ids(vehicles, head_id)
        .into_iter()
        .filter_map(|id| vehicles.iter().find(|v| v.id == id))
        .map(|v| v.engine_id.and_then(engine_by_id).map_or(0, |e| e.power_hp))
        .fold(0_u32, u32::saturating_add)
}

/// Número de teselas que ocupa el consist (redondeo hacia arriba).
#[must_use]
pub fn consist_tile_span(vehicles: &[Vehicle], head_id: u32) -> u32 {
    let len = vehicles
        .iter()
        .find(|v| v.id == head_id)
        .map_or(u16::from(super::VEHICLE_LENGTH), |v| v.cached_total_length);
    u32::from(len)
        .div_ceil(u32::from(super::TILE_FRACTIONS))
        .max(1)
}

/// Teselas ocupadas por el consist según las posiciones persistentes de cada
/// unidad (no la proyección efímera: en depósito varios consists pueden compartir
/// tesela y la proyección «detrás» sacaría vagones a la boca).
#[must_use]
pub fn consist_occupied_tiles(vehicles: &[Vehicle], head_id: u32) -> Vec<TileCoord> {
    let mut index = crate::fleet_index::FleetIndex::default();
    index.rebuild(vehicles);
    consist_occupied_tiles_indexed(vehicles, &index, head_id)
}

/// Como [`consist_occupied_tiles`], reutilizando el índice de flota del tick.
///
/// Los hot paths de PBS consultan muchas huellas sobre una misma topología; no
/// deben reconstruir `FleetIndex` para cada tren o tesela.
#[must_use]
pub fn consist_occupied_tiles_indexed(
    vehicles: &[Vehicle],
    index: &crate::fleet_index::FleetIndex,
    head_id: u32,
) -> Vec<TileCoord> {
    let ids = index.consist(head_id);
    let mut tiles = Vec::new();
    for &id in ids {
        let Some(slot) = index.slot(id) else {
            continue;
        };
        let unit = &vehicles[slot];
        if !tiles.contains(&unit.pos) {
            tiles.push(unit.pos);
        }
    }
    // Saves/escenarios antiguos pueden declarar longitud de consist sin
    // unidades encadenadas. Conservamos la huella conservadora hasta que se
    // reconstruya la topología física al cargar.
    if ids.len() <= 1 {
        let span = consist_tile_span(vehicles, head_id) as usize;
        if let Some(slot) = index.slot(head_id) {
            let head = &vehicles[slot];
            for &tile in head.rail_tile_history.iter().take(span.saturating_sub(1)) {
                if !tiles.contains(&tile) {
                    tiles.push(tile);
                }
            }
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_cargo_weight_uses_active_cargo_spec() {
        let catalog = [crate::cargo_spec::CargoSpecDef {
            id: CargoType::Custom(3).cargo_id(),
            label: "TEST".to_owned(),
            name: "Carga de prueba".to_owned(),
            weight: 32,
            from_newgrf: true,
            ..crate::cargo_spec::CargoSpecDef::default()
        }];
        assert_eq!(cargo_weight_t(9, Some(CargoType::Custom(3)), &catalog), 18);
    }

    #[test]
    fn zero_spec_weight_keeps_vanilla_fallback() {
        let catalog = [crate::cargo_spec::CargoSpecDef {
            id: CargoType::Custom(3).cargo_id(),
            label: "TEST".to_owned(),
            name: "Carga de prueba".to_owned(),
            from_newgrf: true,
            ..crate::cargo_spec::CargoSpecDef::default()
        }];
        assert_eq!(cargo_weight_t(9, Some(CargoType::Custom(3)), &catalog), 9);
    }

    #[test]
    fn train_cargo_weight_applies_freight_multiplier_only_to_freight_specs() {
        let freight = [crate::cargo_spec::CargoSpecDef {
            id: CargoType::Custom(3).cargo_id(),
            label: "FRT".to_owned(),
            name: "Carga freight".to_owned(),
            weight: 16,
            is_freight: true,
            from_newgrf: true,
            ..crate::cargo_spec::CargoSpecDef::default()
        }];
        assert_eq!(
            train_cargo_weight_t(4, Some(CargoType::Custom(3)), &freight, 3),
            12
        );

        let non_freight = [crate::cargo_spec::CargoSpecDef {
            id: CargoType::Custom(3).cargo_id(),
            label: "PAX".to_owned(),
            name: "Carga no freight".to_owned(),
            weight: 16,
            is_freight: false,
            from_newgrf: true,
            ..crate::cargo_spec::CargoSpecDef::default()
        }];
        assert_eq!(
            train_cargo_weight_t(4, Some(CargoType::Custom(3)), &non_freight, 3),
            4
        );
    }
}
