//! Texto de detalles por unidad y resúmenes de tab (OpenTTD `DrawTrainDetails` / #175).

use openttdrs_core::prelude::*;
use openttdrs_core::{cargo_display_name, format_money};

use crate::state::SimWorld;

use super::VehicleDetailsTab;

pub(crate) fn speed_to_kmh(kind: VehicleKind, units: u16) -> u16 {
    match kind {
        VehicleKind::Train | VehicleKind::Aircraft => units,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Ship => units / 2,
    }
}

pub(crate) fn cargo_type_label(vehicle: &openttdrs_core::Vehicle) -> String {
    vehicle.cargo_type.map_or_else(
        || "Cualquiera".to_string(),
        |c| cargo_display_name(c).to_string(),
    )
}

/// IDs de unidades a listar en Details (tren = consist; resto = una fila).
#[must_use]
pub(crate) fn details_unit_ids(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> Vec<u32> {
    openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id)
}

/// Resumen encima de la lista (tab Totales; vacío en el resto).
#[must_use]
pub(crate) fn vehicle_details_summary(
    vehicle: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    if tab != VehicleDetailsTab::Totals {
        return String::new();
    }
    let ids = details_unit_ids(vehicle, sim);
    let units = ids.len();
    let weight = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_weight_t(&sim.state.vehicles, vehicle.id)
    } else {
        vehicle.effective_engine().weight_t
    };
    let power = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_power_hp(&sim.state.vehicles, vehicle.id)
    } else {
        vehicle.effective_engine().power_hp
    };
    let (cargo, capacity) = ids.iter().fold((0_u32, 0_u32), |(c, cap), &id| {
        let Some(u) = sim.state.vehicles.iter().find(|v| v.id == id) else {
            return (c, cap);
        };
        (c.saturating_add(u.cargo), cap.saturating_add(u.capacity))
    });
    format!(
        "Unidades: {units} · Peso: {weight} t · Potencia: {power} CV · Carga: {cargo}/{capacity}\n\
         Beneficio este año: {} · Anterior: {}",
        format_money(vehicle.profit_this_year),
        format_money(vehicle.profit_last_year),
    )
}

/// Línea de datos de una unidad según el tab activo.
#[must_use]
pub(crate) fn vehicle_details_unit_line(
    unit: &openttdrs_core::Vehicle,
    head: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    let engine = unit.effective_engine();
    match tab {
        VehicleDetailsTab::Info => {
            let age = unit.vehicle_age_years(sim.state.tick.get());
            let age_note = if unit.needs_autorenewing(sim.state.tick.get()) {
                " · renovar"
            } else {
                ""
            };
            let depot_note = if openttdrs_core::vehicle_in_depot(&sim.state.map, unit.pos) {
                " · depósito"
            } else {
                ""
            };
            let is_head = unit.id == head.id;
            let power_note = if is_head || engine.power_hp > 0 {
                format!(" · {} CV", engine.power_hp)
            } else {
                String::new()
            };
            format!(
                "#{} {} · {} t{power_note} · {age}a{age_note} · fiab. {}%{depot_note}",
                unit.id,
                engine.name,
                engine.weight_t,
                unit.reliability / 100,
            )
        }
        VehicleDetailsTab::Cargo => format!(
            "#{} {} · {} {}/{} · packets {}",
            unit.id,
            engine.name,
            cargo_type_label(unit),
            unit.cargo,
            unit.capacity,
            unit.cargo_packets.packets.len(),
        ),
        VehicleDetailsTab::Capacity => format!(
            "#{} {} · cap. {} ({})",
            unit.id,
            engine.name,
            unit.capacity,
            cargo_type_label(unit),
        ),
        VehicleDetailsTab::Totals => format!(
            "#{} {} · {} t · {}/{} · ${}/año",
            unit.id,
            engine.name,
            engine.weight_t,
            unit.cargo,
            unit.capacity,
            engine.running_cost_year,
        ),
    }
}

/// Cuerpo agregado (tests / compat): resumen + una línea por unidad.
#[cfg(test)]
#[must_use]
pub(crate) fn vehicle_details_body(
    vehicle: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    let mut lines = Vec::new();
    let summary = vehicle_details_summary(vehicle, sim, tab);
    if !summary.is_empty() {
        lines.push(summary);
    }
    for unit_id in details_unit_ids(vehicle, sim) {
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id) else {
            continue;
        };
        lines.push(vehicle_details_unit_line(unit, vehicle, sim, tab));
    }
    // Tab Info de un solo vehículo: enriquecer con velocidad/órdenes (cabeza).
    if tab == VehicleDetailsTab::Info && details_unit_ids(vehicle, sim).len() == 1 {
        let engine = vehicle.effective_engine();
        let shared = vehicle
            .shared_order_id
            .map_or_else(String::new, |id| format!(" · Órdenes compartidas #{id}"));
        let active_order = if vehicle.orders.is_empty() {
            "—".to_string()
        } else {
            format!(
                "{}",
                vehicle
                    .current_order
                    .min(vehicle.orders.len().saturating_sub(1))
                    + 1
            )
        };
        lines.push(format!(
            "Posición: ({}, {}) · Velocidad: {} km/h (máx. {}) · Órdenes: {} · Activa: {active_order}{shared}",
            vehicle.pos.x,
            vehicle.pos.y,
            speed_to_kmh(vehicle.kind, vehicle.cur_speed),
            engine.speed_kmh(),
            vehicle.orders.len(),
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn road_speed_units_halve_for_display() {
        assert_eq!(speed_to_kmh(VehicleKind::Bus, 112), 56);
        assert_eq!(speed_to_kmh(VehicleKind::Truck, 96), 48);
        assert_eq!(speed_to_kmh(VehicleKind::Train, 128), 128);
    }

    #[test]
    fn vehicle_details_include_age_weight_and_runtime_reliability() {
        let mut state = GameState::new(8, 8);
        let vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        state.vehicles.push(vehicle.clone());
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let body = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Info);
        assert!(body.contains("fiab."));
        assert!(body.contains(" t "));
        let cargo = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Cargo);
        assert!(cargo.contains("packets"));
        let totals = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Totals);
        assert!(totals.contains("Unidades:"));
    }

    #[test]
    fn non_train_details_lists_single_unit() {
        let mut state = GameState::new(8, 8);
        let vehicle = Vehicle::new(
            9,
            VehicleKind::Truck,
            TileCoord::new(2, 2),
            TileCoord::new(2, 2),
        );
        state.vehicles.push(vehicle.clone());
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let ids = details_unit_ids(&vehicle, &sim);
        assert_eq!(ids, vec![9]);
        let line = vehicle_details_unit_line(&vehicle, &vehicle, &sim, VehicleDetailsTab::Capacity);
        assert!(line.contains("#9"));
        assert!(line.contains("cap."));
    }

    #[test]
    fn train_consist_lists_each_unit_in_cargo_tab() {
        let mut state = GameState::new(8, 8);
        let mut head = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        let mut wagon = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        head.next_unit = Some(2);
        wagon.prev_unit = Some(1);
        wagon.capacity = 40;
        wagon.cargo = 12;
        state.vehicles = vec![head, wagon];
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let head = sim.state.vehicles.iter().find(|v| v.id == 1).unwrap();
        let ids = details_unit_ids(head, &sim);
        assert_eq!(ids, vec![1, 2]);
        let body = vehicle_details_body(head, &sim, VehicleDetailsTab::Cargo);
        assert!(body.contains("#1"));
        assert!(body.contains("#2"));
        assert!(body.contains("12/40"));
    }
}
