//! Integración ligera: mismos comandos del core que aplicaría la toolbar sobre [`SimWorld`].

use openttdrs_core::{Command, TileCoord, TileKind, apply_command};

use super::SimWorld;

fn first_tile_with_kind(sim: &SimWorld, kind: TileKind) -> Option<TileCoord> {
    let (mw, mh) = sim.state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if sim.state.map.get_kind(c) == Some(kind) {
                return Some(c);
            }
        }
    }
    None
}

#[test]
fn place_road_on_grass_matches_toolbar_command_stack() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(c)).is_ok(),
        "PlaceRoad sobre hierba (como la toolbar)"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Road));
}

#[test]
fn clear_tile_after_road_restores_grass() {
    let mut sim = SimWorld::default();
    let Some(c) = first_tile_with_kind(&sim, TileKind::Grass) else {
        panic!("mapa procedural debe tener al menos una tesela de hierba");
    };
    assert!(
        apply_command(&mut sim.state, &Command::PlaceRoad(c)).is_ok(),
        "PlaceRoad sobre hierba"
    );
    assert!(
        apply_command(&mut sim.state, &Command::ClearTile(c)).is_ok(),
        "ClearTile tras carretera"
    );
    assert_eq!(sim.state.map.get_kind(c), Some(TileKind::Grass));
}
