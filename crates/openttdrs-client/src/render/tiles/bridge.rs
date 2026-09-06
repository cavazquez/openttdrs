//! Tramos intermedios de puente sobre teselas con `IsBridgeAbove` en `mapt`.

use bevy::prelude::*;
use openttdrs_core::Climate;
use openttdrs_core::bridge_above_axis_from_mapt;
use openttdrs_core::prelude::*;

use crate::render::{TileRenderContext, WorldAssets};

use super::bridge_draw::bridge_span_at;

/// Dibuja el tablero si la tesela tiene un puente por encima (no rampa).
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn spawn_bridge_middle(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    show_pbs_reservations: bool,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    let road_catalog = openttdrs_core::vanilla_road_type_catalog();
    spawn_bridge_middle_with_road_types(
        commands,
        map,
        dims,
        assets,
        ctx,
        show_pbs_reservations,
        Climate::Temperate,
        &road_catalog,
        None,
        &[],
        catenary_newgrf,
        catenary_sprites,
        bridge_decks_newgrf,
        action5_sprites,
        images,
    );
}

/// Variante de [`spawn_bridge_middle`] con el catálogo de roadtypes para
/// resolver `ROTSG_BRIDGE`/`ROTSG_OVERLAY` desde la rampa sur.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_bridge_middle_with_road_types(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    show_pbs_reservations: bool,
    climate: Climate,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    road_sprites: Option<&mut crate::render::NewGrfRoadSpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    spawn_bridge_middle_with_road_types_and_stations(
        commands,
        map,
        dims,
        assets,
        ctx,
        show_pbs_reservations,
        climate,
        road_catalog,
        road_sprites,
        newgrf_stack,
        catenary_newgrf,
        catenary_sprites,
        bridge_decks_newgrf,
        action5_sprites,
        images,
        &[],
        &[],
        &[],
    );
}

/// Variante del pase de vanos que recibe el contexto de estaciones necesario
/// para aplicar `RoadStopSpec::bridgeable_info.disallowed_pillars`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_bridge_middle_with_road_types_and_stations(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    show_pbs_reservations: bool,
    climate: Climate,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    road_sprites: Option<&mut crate::render::NewGrfRoadSpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
    stations: &[openttdrs_core::Station],
    road_stop_catalog: &[openttdrs_core::RoadStopSpecDef],
    bridge_spec_catalog: &[openttdrs_core::BridgeSpecDef],
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    if bridge_above_axis_from_mapt(tile.mapt).is_none() {
        return;
    }
    let Some(span) = bridge_span_at(map, ctx.coord, dims) else {
        return;
    };
    super::bridge_draw::spawn_bridge_deck_with_road_types(
        commands,
        map,
        dims,
        assets,
        ctx,
        &span,
        true,
        show_pbs_reservations,
        catenary_newgrf,
        catenary_sprites,
        bridge_decks_newgrf,
        &[],
        climate,
        road_catalog,
        road_sprites,
        newgrf_stack,
        action5_sprites,
        images,
        stations,
        road_stop_catalog,
        bridge_spec_catalog,
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use openttdrs_core::{
        BridgeType, bridge_above_axis_from_mapt, set_bridge_middle_mapt, set_bridge_type_m6,
    };

    fn ramp_tile_template(m5: u8) -> Tile {
        Tile {
            height: 0,
            kind: TileKind::RoadBridge,
            mapt: 0x90,
            m5,
            m1: 0,
            m6: set_bridge_type_m6(0, BridgeType::CantileverRed),
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn span_at_resolves_on_ramp_endpoints() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_tile(c(1, 2), ramp_tile_template(0x86))
            .expect("ramp");
        map.set_tile(c(5, 2), ramp_tile_template(0x84))
            .expect("ramp");
        for x in 2..=4 {
            let water = Tile {
                height: 0,
                kind: TileKind::Water,
                mapt: set_bridge_middle_mapt(0x60, false),
                m5: 0,
                m1: 0,
                m6: set_bridge_type_m6(0, BridgeType::CantileverRed),
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            };
            map.set_tile(c(x, 2), water).expect("span");
        }
        let dims = map.dimensions();
        for x in 1..=5 {
            let span = bridge_span_at(&map, c(x, 2), dims).expect("span");
            assert_eq!(span.bridge_type, BridgeType::CantileverRed);
            assert_eq!(span.axis, 0);
        }
        assert_eq!(
            bridge_span_at(&map, c(1, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::North
        );
        assert_eq!(
            bridge_span_at(&map, c(5, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::South
        );
        // Las rampas no forman parte del cálculo de `CalcBridgePiece` de
        // OpenTTD: el vano de tres teselas es North, MiddleOdd, South.
        assert_eq!(
            bridge_span_at(&map, c(2, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::North
        );
        assert_eq!(
            bridge_span_at(&map, c(3, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::MiddleOdd
        );
        assert_eq!(
            bridge_span_at(&map, c(4, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::South
        );
    }

    #[test]
    fn bridge_above_reads_mapt_bits() {
        assert_eq!(bridge_above_axis_from_mapt(0x64), Some(false));
        assert_eq!(bridge_above_axis_from_mapt(0x68), Some(true));
        assert_eq!(bridge_above_axis_from_mapt(0x60), None);
    }

    #[test]
    fn wooden_flat_ramps_follow_upstream_head_table() {
        // `_bridge_sprite_table_wood_heads`: con terreno plano OpenTTD usa
        // las cuatro cabezas RAMP, indexadas SW, SE, NE, NW.
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::Wooden,
                true,
                openttdrs_core::RailType::Rail,
                0,
                2,
            ),
            2538
        );
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::Wooden,
                true,
                openttdrs_core::RailType::Rail,
                0,
                1,
            ),
            2537
        );
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::Wooden,
                true,
                openttdrs_core::RailType::Rail,
                0,
                0,
            ),
            2539
        );
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::Wooden,
                true,
                openttdrs_core::RailType::Rail,
                0,
                3,
            ),
            2540
        );
    }

    #[test]
    fn every_bridge_type_uses_directional_head_sprites() {
        use openttdrs_core::RailType;

        // El cantilever rojo de la partida no debe recibir el sprite recto
        // del vano (2508...), sino la cabecera genérica `BRIDGE_PIECE_HEAD`.
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::CantileverRed,
                true,
                RailType::Rail,
                0,
                2,
            ),
            2442
        );
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::CantileverRed,
                true,
                RailType::Maglev,
                0,
                2,
            ),
            4371
        );
        // Una rampa de carretera sobre fundación nivelada se selecciona con
        // la pendiente efectiva plana, no con el `tileh` crudo.
        assert_eq!(
            crate::sprites::bridge_ramp_sprite_id(
                BridgeType::CantileverRed,
                false,
                RailType::Rail,
                0,
                2,
            ),
            2450
        );
    }

    #[test]
    fn generic_bridge_heads_match_openttd_for_all_directions_and_transports() {
        use openttdrs_core::RailType;

        // `_bridge_sprite_table_generic_*_heads` de `bridge_land.h`, ya
        // convertida de su orden SW/SE/NE/NW a los bits m5 de la partida.
        // Esto protege las cuatro rampas: una tabla rotada parece una vía que
        // se corta justo en el ingreso al puente.
        let cases = [
            (
                true,
                RailType::Rail,
                [2437, 2440, 2438, 2439],
                [2441, 2444, 2442, 2443],
            ),
            (
                false,
                RailType::Rail,
                [2445, 2448, 2446, 2447],
                [2449, 2452, 2450, 2451],
            ),
            (
                true,
                RailType::Monorail,
                [4326, 4329, 4327, 4328],
                [4330, 4333, 4331, 4332],
            ),
            (
                true,
                RailType::Maglev,
                [4366, 4369, 4367, 4368],
                [4370, 4373, 4371, 4372],
            ),
        ];

        for (rail, rail_type, sloped, flat) in cases {
            for (dir, (&slope_id, &flat_id)) in sloped.iter().zip(flat.iter()).enumerate() {
                assert_eq!(
                    crate::sprites::bridge_ramp_sprite_id(
                        BridgeType::CantileverRed,
                        rail,
                        rail_type,
                        1,
                        dir as u8,
                    ),
                    slope_id,
                    "rampa inclinada dir {dir}, {rail_type:?}"
                );
                assert_eq!(
                    crate::sprites::bridge_ramp_sprite_id(
                        BridgeType::CantileverRed,
                        rail,
                        rail_type,
                        0,
                        dir as u8,
                    ),
                    flat_id,
                    "rampa plana dir {dir}, {rail_type:?}"
                );
            }
        }
    }

    #[test]
    fn span_at_propagates_pbs_reservation_from_rail_ramp() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        // Rampa oeste hacia el este (dir 2) y rampa este hacia el oeste
        // (dir 0): la pareja debe respetar la dirección persistida, igual que
        // un `.sav` real. Dos `0x80` sólo pasaban con el resolvedor por
        // escaneo y podían emparejar cabezas vecinas no relacionadas.
        let mut north = ramp_tile_template(0x92);
        north.kind = TileKind::RailBridge;
        let mut south = ramp_tile_template(0x80);
        south.kind = TileKind::RailBridge;
        map.set_tile(c(1, 2), north).expect("rampa norte");
        map.set_tile(c(4, 2), south).expect("rampa sur");
        for x in 2..=3 {
            let mut water = map.get(c(x, 2)).expect("agua");
            water.kind = TileKind::Water;
            water.mapt = set_bridge_middle_mapt(0x60, false);
            map.set_tile(c(x, 2), water).expect("vano");
        }

        let span = bridge_span_at(&map, c(2, 2), map.dimensions()).expect("puente");
        assert!(span.rail);
        assert!(span.pbs_reserved);
    }
}
