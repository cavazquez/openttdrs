use openttdrs_core::{Map, TileCoord, TileKind, station_site_adjacent_to_transport};

use crate::ui::toolbar::BuildMenuAction;

pub(crate) fn preview_target_is_valid(action: BuildMenuAction, kind: TileKind) -> bool {
    match action {
        BuildMenuAction::Road
        | BuildMenuAction::RoadX
        | BuildMenuAction::RoadY
        | BuildMenuAction::RoadDepot
        | BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::Rail
        | BuildMenuAction::RailDepot
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Station
        | BuildMenuAction::BusStop
        | BuildMenuAction::BuildHouse
        | BuildMenuAction::BuildCoalMine
        | BuildMenuAction::BuildIronOreMine
        | BuildMenuAction::BuildGoldMine
        | BuildMenuAction::BuildOilWell
        | BuildMenuAction::BuildOilRefinery
        | BuildMenuAction::BuildFactory
        | BuildMenuAction::BuildSawmill
        | BuildMenuAction::BuildForest
        | BuildMenuAction::BuildFarm => !matches!(kind, TileKind::Water | TileKind::Void),
        BuildMenuAction::Clear | BuildMenuAction::Orders => !matches!(kind, TileKind::Void),
    }
}

pub(crate) fn action_is_tunnel(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RoadTunnel | BuildMenuAction::RailTunnel
    )
}

#[must_use]
pub(crate) fn preview_station_has_transport_neighbor(map: &Map, pos: TileCoord) -> bool {
    station_site_adjacent_to_transport(map, pos)
}

pub(crate) fn tunnel_preview_is_valid(
    map: &Map,
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> bool {
    if !action_is_tunnel(action) {
        return true;
    }
    if tiles.len() < 3 {
        return false;
    }
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    let Some(start) = map.get(TileCoord::new(sx, sy)) else {
        return false;
    };
    let Some(end) = map.get(TileCoord::new(ex, ey)) else {
        return false;
    };
    !matches!(start.kind, TileKind::Water | TileKind::Void)
        && !matches!(end.kind, TileKind::Water | TileKind::Void)
        && start.height == end.height
}
