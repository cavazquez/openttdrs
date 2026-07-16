//! Lógica pura para calcular tiles que deben ser remapeados visualmente.

use openttdrs_core::{Map, TileCoord, industry_template};

use crate::ui::toolbar::preview::industry_spec_for_action;
use crate::ui::toolbar::BuildMenuAction;

use super::drag::{
    action_is_tunnel, rail_action_refreshes_neighbors, rail_remap_neighbor_tiles,
    tunnel_remap_tiles,
};

fn road_action_refreshes_neighbors(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::Road
            | BuildMenuAction::RoadX
            | BuildMenuAction::RoadY
            | BuildMenuAction::Tram
            | BuildMenuAction::TramX
            | BuildMenuAction::TramY
    )
}

/// Calcula los tiles que deben ser remapeados visualmente tras una acción.
pub(crate) fn tiles_for_visual_remap(
    map: Option<&Map>,
    action: BuildMenuAction,
    origin: TileCoord,
    drag_tiles: &[(i32, i32)],
) -> Vec<(i32, i32)> {
    let base = if action_is_tunnel(action) {
        if let Some(map) = map {
            return tunnel_remap_tiles(map, drag_tiles);
        }
        let start = drag_tiles.first().copied().unwrap_or((origin.x, origin.y));
        vec![start]
    } else if drag_tiles.len() > 1 {
        drag_tiles.to_vec()
    } else if let Some(spec) = industry_spec_for_action(action) {
        industry_template(origin, spec)
            .into_iter()
            .map(|(c, _)| (c.x, c.y))
            .collect()
    } else if let Some(&(tx, ty)) = drag_tiles.first() {
        vec![(tx, ty)]
    } else {
        vec![(origin.x, origin.y)]
    };
    if (rail_action_refreshes_neighbors(action) || road_action_refreshes_neighbors(action))
        && let Some(map) = map
    {
        return rail_remap_neighbor_tiles(map, &base);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiles_for_visual_remap_single_tile() {
        let origin = TileCoord::new(5, 10);
        let tiles = tiles_for_visual_remap(None, BuildMenuAction::Road, origin, &[]);
        assert_eq!(tiles, vec![(5, 10)]);
    }

    #[test]
    fn test_tiles_for_visual_remap_drag() {
        let origin = TileCoord::new(0, 0);
        let drag = vec![(1, 1), (2, 2), (3, 3)];
        let tiles = tiles_for_visual_remap(None, BuildMenuAction::Road, origin, &drag);
        assert_eq!(tiles, drag);
    }

    #[test]
    fn test_tiles_for_visual_remap_first_drag_tile() {
        let origin = TileCoord::new(0, 0);
        let drag = vec![(7, 8)];
        let tiles = tiles_for_visual_remap(None, BuildMenuAction::Road, origin, &drag);
        assert_eq!(tiles, vec![(7, 8)]);
    }
}
