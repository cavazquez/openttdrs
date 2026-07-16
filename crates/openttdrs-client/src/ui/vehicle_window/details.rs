//! Constructores de texto para los detalles del vehículo (tabs Info, Cargo, Capacity, Totals).

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

pub(crate) fn vehicle_details_body(
    vehicle: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    match tab {
        VehicleDetailsTab::Info => vehicle_details_info(vehicle, sim),
        VehicleDetailsTab::Cargo => vehicle_details_cargo(vehicle, sim),
        VehicleDetailsTab::Capacity => vehicle_details_capacity(vehicle, sim),
        VehicleDetailsTab::Totals => vehicle_details_totals(vehicle, sim),
    }
}

fn vehicle_details_info(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> String {
    let engine = vehicle.effective_engine();
    let depot_note = if openttdrs_core::vehicle_in_depot(&sim.state.map, vehicle.pos) {
        " · En depósito"
    } else {
        ""
    };
    let age = vehicle.vehicle_age_years(sim.state.tick.get());
    let age_note = if vehicle.needs_autorenewing(sim.state.tick.get()) {
        " · renovar"
    } else {
        ""
    };
    let (weight_t, power_hp) = if vehicle.kind == VehicleKind::Train {
        (
            openttdrs_core::consist_weight_t(&sim.state.vehicles, vehicle.id),
            openttdrs_core::consist_power_hp(&sim.state.vehicles, vehicle.id),
        )
    } else {
        (engine.weight_t, engine.power_hp)
    };
    let consist_note = if vehicle.kind == VehicleKind::Train {
        let n = openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id).len();
        if n > 1 {
            format!(" · Consist: {n} unidades")
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let runtime_reliability = vehicle.reliability / 100;
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
    format!(
        "Modelo: {}{consist_note}\nTipo carga: {}\nPosición: ({}, {}){depot_note}\n\
         Edad: {age}a{age_note} · Peso: {weight_t} t · Potencia: {power_hp} CV\n\
         Velocidad: {} km/h (máx. {})\n\
         Coste: ${}/año · Fiabilidad: {runtime_reliability}% (diseño {}%)\n\
         Órdenes: {} · Orden activa: {active_order}{shared}",
        engine.name,
        cargo_type_label(vehicle),
        vehicle.pos.x,
        vehicle.pos.y,
        speed_to_kmh(vehicle.kind, vehicle.cur_speed),
        engine.speed_kmh(),
        engine.running_cost_year,
        engine.reliability_pct,
        vehicle.orders.len(),
    )
}

fn vehicle_details_cargo(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> String {
    let mut lines = vec![
        format!(
            "Tipo: {} · A bordo: {}/{}",
            cargo_type_label(vehicle),
            vehicle.cargo,
            vehicle.capacity
        ),
        format!(
            "Packets: {} · Tránsito máx.: {}d",
            vehicle.cargo_packets.packets.len(),
            vehicle.cargo_packets.max_periods_in_transit()
        ),
    ];
    if vehicle.kind == VehicleKind::Train {
        lines.push("Por unidad:".to_string());
        for unit_id in openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id) {
            let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id) else {
                continue;
            };
            lines.push(format!(
                "  #{} {} · {} {}/{}",
                unit.id,
                unit.effective_engine().name,
                cargo_type_label(unit),
                unit.cargo,
                unit.capacity
            ));
        }
    }
    lines.join("\n")
}

fn vehicle_details_capacity(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> String {
    let mut lines = Vec::new();
    let ids = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id)
    } else {
        vec![vehicle.id]
    };
    let mut total = 0_u32;
    for unit_id in ids {
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id) else {
            continue;
        };
        let engine = unit.effective_engine();
        total = total.saturating_add(unit.capacity);
        lines.push(format!(
            "#{} {} · cap. {} ({})",
            unit.id,
            engine.name,
            unit.capacity,
            cargo_type_label(unit)
        ));
    }
    lines.insert(0, format!("Capacidad total: {total}"));
    lines.join("\n")
}

fn vehicle_details_totals(vehicle: &openttdrs_core::Vehicle, sim: &SimWorld) -> String {
    let units = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id).len()
    } else {
        1
    };
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
    let capacity: u32 = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id)
            .into_iter()
            .filter_map(|id| sim.state.vehicles.iter().find(|v| v.id == id))
            .map(|v| v.capacity)
            .sum()
    } else {
        vehicle.capacity
    };
    let cargo: u32 = if vehicle.kind == VehicleKind::Train {
        openttdrs_core::consist_unit_ids(&sim.state.vehicles, vehicle.id)
            .into_iter()
            .filter_map(|id| sim.state.vehicles.iter().find(|v| v.id == id))
            .map(|v| v.cargo)
            .sum()
    } else {
        vehicle.cargo
    };
    let cost = vehicle.effective_engine().running_cost_year;
    format!(
        "Unidades: {units}\nPeso: {weight} t · Potencia: {power} CV\n\
         Carga: {cargo}/{capacity}\nCoste operación (cabeza): ${cost}/año\n\
         Beneficio este año: {}\nBeneficio anterior: {}",
        format_money(vehicle.profit_this_year),
        format_money(vehicle.profit_last_year),
    )
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
        let state = GameState::new(8, 8);
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        let body = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Info);
        assert!(body.contains("Edad:"));
        assert!(body.contains("Peso:"));
        assert!(body.contains("Potencia:"));
        assert!(body.contains("diseño"));
        let cargo = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Cargo);
        assert!(cargo.contains("A bordo:"));
        let totals = vehicle_details_body(&vehicle, &sim, VehicleDetailsTab::Totals);
        assert!(totals.contains("Unidades:"));
    }
}
