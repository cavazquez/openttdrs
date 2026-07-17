//! Compra de bus y órdenes para `RoadHaul`.

use crate::GameState;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::map::{TileCoord, TileKind};
use crate::vehicle::{VehicleKind, VehicleOrder};

pub(super) fn buy_and_order_bus(
    state: &mut GameState,
    ai_id: CompanyId,
    stop_a: TileCoord,
    stop_b: TileCoord,
    depot: TileCoord,
) -> bool {
    if state.map.get_kind(depot) != Some(TileKind::RoadDepot) {
        return false;
    }

    let mut ok = false;
    with_ai_active(state, ai_id, |state| {
        if apply_command(
            state,
            &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
        )
        .is_err()
        {
            return;
        }
        let Some(vid) = state
            .vehicles
            .iter()
            .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Bus)
            .map(|v| v.id)
            .max()
        else {
            return;
        };
        let orders = vec![
            VehicleOrder::station(stop_a),
            VehicleOrder::station(stop_b),
        ];
        if apply_command(state, &Command::SetVehicleOrderList(vid, orders)).is_err() {
            return;
        }
        let _ = apply_command(state, &Command::ToggleVehicleRunning(vid));
        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == vid) {
            v.running = true;
            v.dest = stop_a;
        }
        ok = true;
    });
    ok
}

fn with_ai_active(state: &mut GameState, ai_id: CompanyId, f: impl FnOnce(&mut GameState)) {
    let prev_active = state.active_company;
    state.active_company = ai_id;
    if let Some(c) = state.companies.get(ai_id.index()) {
        state.economy = c.economy;
        state.company_colour = c.colour;
    }
    f(state);
    state.active_company = prev_active;
    state.sync_mirrors_from_active();
}
