//! Texto de detalles por unidad y resúmenes de tab (OpenTTD `DrawTrainDetails` / #175).

use openttdrs_core::prelude::*;
use openttdrs_core::{
    CargoType, cargo_spec_display_name, consist_power_hp_with_catalog,
    consist_weight_t_with_catalog, engine_for_vehicle_catalog, format_money,
};

use crate::i18n::{Locale, localized_text};
use crate::state::SimWorld;

use super::VehicleDetailsTab;

pub(crate) fn speed_to_kmh(kind: VehicleKind, units: u16) -> u16 {
    match kind {
        VehicleKind::Train | VehicleKind::Aircraft => units,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Ship => units / 2,
    }
}

pub(crate) fn cargo_type_label(
    locale: Locale,
    vehicle: &openttdrs_core::Vehicle,
    sim: &SimWorld,
) -> String {
    vehicle.cargo_type.map_or_else(
        || localized_text(locale, "Cualquiera"),
        |cargo| {
            let name = cargo_spec_display_name(cargo, &sim.state.cargo_spec_catalog);
            // Los nombres definidos por NewGRF son datos de la partida, no
            // claves del catálogo vanilla: se conservan literalmente.
            if matches!(cargo, CargoType::Custom(_)) {
                name
            } else {
                localized_text(locale, &name)
            }
        },
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
    locale: Locale,
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
        consist_weight_t_with_catalog(&sim.state.vehicles, vehicle.id, &sim.state.engine_catalog)
    } else {
        engine_for_vehicle_catalog(&sim.state.engine_catalog, vehicle).weight_t
    };
    let power = if vehicle.kind == VehicleKind::Train {
        consist_power_hp_with_catalog(&sim.state.vehicles, vehicle.id, &sim.state.engine_catalog)
    } else {
        engine_for_vehicle_catalog(&sim.state.engine_catalog, vehicle).power_hp
    };
    let (cargo, capacity) = ids.iter().fold((0_u32, 0_u32), |(c, cap), &id| {
        let Some(u) = sim.state.vehicles.iter().find(|v| v.id == id) else {
            return (c, cap);
        };
        (c.saturating_add(u.cargo), cap.saturating_add(u.capacity))
    });
    let power_unit = if locale == Locale::En { "hp" } else { "CV" };
    format!(
        "{}: {units} · {}: {weight} t · {}: {power} {power_unit} · {}: {cargo}/{capacity}\n\
         {}: {} · {}: {}",
        localized_text(locale, "Unidades"),
        localized_text(locale, "Peso"),
        localized_text(locale, "Potencia"),
        localized_text(locale, "Carga"),
        localized_text(locale, "Beneficio este año"),
        format_money(vehicle.profit_this_year),
        localized_text(locale, "Anterior"),
        format_money(vehicle.profit_last_year),
    )
}

/// Línea de datos de una unidad según el tab activo.
#[must_use]
pub(crate) fn vehicle_details_unit_line(
    locale: Locale,
    unit: &openttdrs_core::Vehicle,
    head: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    let engine = engine_for_vehicle_catalog(&sim.state.engine_catalog, unit);
    match tab {
        VehicleDetailsTab::Info => {
            let age = unit.vehicle_age_years(sim.state.tick.get());
            let renew_months = sim
                .state
                .companies
                .get(unit.owner.index())
                .map_or(6, |c| c.engine_renew_months);
            let age_note = if unit.needs_autorenewing(sim.state.tick.get(), renew_months) {
                format!(" · {}", localized_text(locale, "renovar"))
            } else {
                String::new()
            };
            let depot_note = if openttdrs_core::vehicle_is_in_depot(&sim.state.map, unit) {
                format!(" · {}", localized_text(locale, "depósito"))
            } else {
                String::new()
            };
            let is_head = unit.id == head.id;
            let power_note = if is_head || engine.power_hp > 0 {
                let power_unit = if locale == Locale::En { "hp" } else { "CV" };
                format!(" · {} {power_unit}", engine.power_hp)
            } else {
                String::new()
            };
            let age_unit = if locale == Locale::En { "y" } else { "a" };
            format!(
                "#{} {} · {} t{power_note} · {age}{age_unit}{age_note} · {} {}%{depot_note}",
                unit.id,
                engine.name,
                engine.weight_t,
                localized_text(locale, "fiab."),
                unit.reliability / 100,
            )
        }
        VehicleDetailsTab::Cargo => format!(
            "#{} {} · {} {}/{} · {} {}",
            unit.id,
            engine.name,
            cargo_type_label(locale, unit, sim),
            unit.cargo,
            unit.capacity,
            localized_text(locale, "packets"),
            unit.cargo_packets.packets.len(),
        ),
        VehicleDetailsTab::Capacity => format!(
            "#{} {} · cap. {} ({})",
            unit.id,
            engine.name,
            unit.capacity,
            cargo_type_label(locale, unit, sim),
        ),
        VehicleDetailsTab::Totals => format!(
            "#{} {} · {} t · {}/{} · ${}/{}",
            unit.id,
            engine.name,
            engine.weight_t,
            unit.cargo,
            unit.capacity,
            engine.running_cost_year,
            localized_text(locale, "año"),
        ),
    }
}

/// Cuerpo agregado (tests / compat): resumen + una línea por unidad.
#[cfg(test)]
#[must_use]
pub(crate) fn vehicle_details_body(
    locale: Locale,
    vehicle: &openttdrs_core::Vehicle,
    sim: &SimWorld,
    tab: VehicleDetailsTab,
) -> String {
    let mut lines = Vec::new();
    let summary = vehicle_details_summary(locale, vehicle, sim, tab);
    if !summary.is_empty() {
        lines.push(summary);
    }
    for unit_id in details_unit_ids(vehicle, sim) {
        let Some(unit) = sim.state.vehicles.iter().find(|v| v.id == unit_id) else {
            continue;
        };
        lines.push(vehicle_details_unit_line(locale, unit, vehicle, sim, tab));
    }
    // Tab Info de un solo vehículo: enriquecer con velocidad/órdenes (cabeza).
    if tab == VehicleDetailsTab::Info && details_unit_ids(vehicle, sim).len() == 1 {
        let engine = engine_for_vehicle_catalog(&sim.state.engine_catalog, vehicle);
        let shared = vehicle.shared_order_id.map_or_else(String::new, |id| {
            format!(" · {} #{id}", localized_text(locale, "Órdenes compartidas"))
        });
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
            "{} ({}, {}) · {}: {} km/h ({} {}) · {}: {} · {}: {active_order}{shared}",
            localized_text(locale, "Posición:"),
            vehicle.pos.x,
            vehicle.pos.y,
            localized_text(locale, "Velocidad"),
            speed_to_kmh(vehicle.kind, vehicle.cur_speed),
            localized_text(locale, "máx."),
            engine.speed_kmh(),
            localized_text(locale, "Órdenes"),
            vehicle.orders.len(),
            localized_text(locale, "Activa"),
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
        let body = vehicle_details_body(Locale::Es, &vehicle, &sim, VehicleDetailsTab::Info);
        assert!(body.contains("fiab."));
        assert!(body.contains(" t "));
        let cargo = vehicle_details_body(Locale::Es, &vehicle, &sim, VehicleDetailsTab::Cargo);
        assert!(cargo.contains("packets"));
        let totals = vehicle_details_body(Locale::Es, &vehicle, &sim, VehicleDetailsTab::Totals);
        assert!(totals.contains("Unidades:"));
    }

    #[test]
    fn vehicle_details_depot_note_follows_ship_state() {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(3, 3);
        state.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        let mut ship = Vehicle::new(1, VehicleKind::Ship, depot, depot);
        ship.ship_state = openttdrs_core::ship_movement::SHIP_STATE_DEPOT;
        state.vehicles.push(ship.clone());
        let inside = SimWorld {
            state,
            ..SimWorld::default()
        };
        let inside_line =
            vehicle_details_unit_line(Locale::Es, &ship, &ship, &inside, VehicleDetailsTab::Info);
        assert!(inside_line.contains("depósito"));

        ship.ship_state = openttdrs_core::ship_movement::SHIP_STATE_TRACK_X;
        let mut state = GameState::new(8, 8);
        state.map.set_kind(depot, TileKind::ShipDepot).unwrap();
        state.vehicles.push(ship.clone());
        let leaving = SimWorld {
            state,
            ..SimWorld::default()
        };
        let leaving_line =
            vehicle_details_unit_line(Locale::Es, &ship, &ship, &leaving, VehicleDetailsTab::Info);
        assert!(!leaving_line.contains("depósito"));
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
        let line = vehicle_details_unit_line(
            Locale::Es,
            &vehicle,
            &vehicle,
            &sim,
            VehicleDetailsTab::Capacity,
        );
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
        let head = &sim.state.vehicles[0];
        let ids = details_unit_ids(head, &sim);
        assert_eq!(ids, vec![1, 2]);
        let body = vehicle_details_body(Locale::Es, head, &sim, VehicleDetailsTab::Cargo);
        assert!(body.contains("#1"));
        assert!(body.contains("#2"));
        assert!(body.contains("12/40"));
    }

    #[test]
    fn english_details_localize_chrome_and_preserve_engine_custom_name_and_values() {
        let mut state = GameState::new(8, 8);
        let mut vehicle = Vehicle::new(
            42,
            VehicleKind::Bus,
            TileCoord::new(3, 4),
            TileCoord::new(3, 4),
        );
        vehicle.name = Some("Línea Ñandú".into());
        vehicle.cargo_type = Some(CargoType::Goods);
        vehicle.cargo = 7;
        vehicle.capacity = 12;
        vehicle.cur_speed = 80;
        state.vehicles.push(vehicle.clone());
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };

        let info = vehicle_details_body(Locale::En, &vehicle, &sim, VehicleDetailsTab::Info);
        assert!(info.contains("rel."));
        assert!(info.contains("Speed"));
        assert!(info.contains("Active"));
        assert!(info.contains("Línea Ñandú") || info.contains("Bus"));

        let cargo = vehicle_details_body(Locale::En, &vehicle, &sim, VehicleDetailsTab::Cargo);
        assert!(cargo.contains("goods"));
        assert!(cargo.contains("7/12"));

        let totals = vehicle_details_body(Locale::En, &vehicle, &sim, VehicleDetailsTab::Totals);
        assert!(totals.contains("Units:"));
        assert!(totals.contains("Weight:"));
        assert!(totals.contains("Profit this year:"));
        assert!(totals.contains("/year"));
        assert!(!totals.contains("Unidades:") && !totals.contains("Peso:"));
    }

    #[test]
    fn vehicle_details_use_active_catalog_engine_stats() {
        let custom_id = openttdrs_core::NEWGRF_ENGINE_ID_BASE + 56;
        let mut state = GameState::new(8, 8);
        let mut custom = openttdrs_core::engine_by_id(openttdrs_core::ENGINE_BUS_MPS)
            .unwrap()
            .clone();
        custom.id = custom_id;
        custom.name = "Bus NewGRF".into();
        custom.weight_t = 37;
        custom.power_hp = 777;
        custom.max_speed = 144;
        custom.from_newgrf = true;
        custom.newgrf_grfid = 0x4445_5441;
        custom.newgrf_local_id = 0;
        state.engine_catalog.push(custom);

        let mut vehicle = Vehicle::new(
            42,
            VehicleKind::Bus,
            TileCoord::new(3, 4),
            TileCoord::new(3, 4),
        );
        vehicle.engine_id = Some(custom_id);
        state.vehicles.push(vehicle.clone());
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };

        let info = vehicle_details_body(Locale::En, &vehicle, &sim, VehicleDetailsTab::Info);
        assert!(info.contains("Bus NewGRF"));
        assert!(info.contains("37 t"));
        assert!(info.contains("777 hp"));

        let totals = vehicle_details_body(Locale::En, &vehicle, &sim, VehicleDetailsTab::Totals);
        assert!(totals.contains("Weight: 37 t"));
        assert!(totals.contains("Power: 777 hp"));
    }
}
