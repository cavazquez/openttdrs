//! Contexto Action2 para `AirportTile` (`GSF_AIRPORTTILES`, feature `0x11`).
//!
//! `OpenTTD` evalúa cada tesela del aeropuerto con dos scopes: el tile actual
//! y el aeropuerto que lo contiene. Este módulo conserva las variables que
//! afectan la selección de sprites (posición, frame, random, layout padre y
//! consultas a teselas vecinas) para que el cliente no tenga que elegir
//! siempre el preview del primer `Action1`.

use std::collections::BTreeSet;

use crate::airport_tile_spec::{AirportTileSpecDef, NEW_AIRPORT_TILE_OFFSET};
use crate::map::{Map, Tile, TileCoord, TileKind, tile_slope_and_z, water_class};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::station::{Station, StopKind, station_at_tile};
use crate::world_gen::{CLEAR_GROUND_DESERT, Climate};

/// Construye el contexto de una tesela de aeropuerto con la estación padre.
///
/// Las variables `0x60`–`0x62` se materializan sólo para los parámetros que el
/// grafo Action2 del spec realmente solicita. Esto mantiene el fingerprint de
/// caché pequeño y, a la vez, permite comparar teselas vecinas del mismo
/// aeropuerto como hace `AirportTileScopeResolver`.
#[must_use]
pub fn action2_eval_ctx_for_airport_tile(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    tile_catalog: &[AirportTileSpecDef],
    current_spec: &AirportTileSpecDef,
    climate: Climate,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(station) = station_at_tile(map, stations, coord)
        .filter(|candidate| candidate.stop_kind == StopKind::Airport)
    else {
        return ctx;
    };
    let tile = map.get(coord);
    let random = u32::from(station.newgrf_random_bits)
        | (u32::from(tile.map_or(0, |candidate| candidate.m3)) << 16);
    ctx.random_bits = random;
    ctx.parent_random_bits = u32::from(station.newgrf_random_bits);
    ctx.persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    ctx.parent_persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    ctx.vars.insert(
        0x5F,
        random
            .wrapping_shl(8)
            .wrapping_add(u32::from(station.newgrf_waiting_random_triggers)),
    );

    // AirportTileScopeResolver::GetVariable(0x41).
    ctx.vars
        .insert(0x41, airport_terrain_type(map, coord, climate, tile));
    // `GetRelativePosition(tile, st->airport.tile)` = 00yxYYXX.
    let dx = coord.x.wrapping_sub(station.pos.x).to_le_bytes()[0];
    let dy = coord.y.wrapping_sub(station.pos.y).to_le_bytes()[0];
    ctx.vars.insert(
        0x43,
        (u32::from(dy & 0x0F) << 20)
            | (u32::from(dx & 0x0F) << 16)
            | (u32::from(dy) << 8)
            | u32::from(dx),
    );
    // `GetAnimationFrame(tile)` is MAP7 in the imported map model.
    ctx.vars
        .insert(0x44, u32::from(tile.map_or(0, |candidate| candidate.m7)));
    // Parent scope: AirportScopeResolver var 40 = selected layout.
    ctx.parent_vars
        .insert(0x40, u32::from(station.airport_layout));

    if let Some(runtime) = current_spec.newgrf_runtime.as_ref() {
        for (variable, parameter) in requested_nearby_vars(runtime) {
            let nearby = nearby_tile(map, coord, parameter);
            let value = match variable {
                0x60 => nearby_land_info(map, stations, station, nearby, climate),
                0x61 => nearby_animation_frame(map, stations, station, nearby),
                0x62 => airport_tile_id_at_offset(
                    map,
                    stations,
                    nearby,
                    station,
                    tile_catalog,
                    current_spec.newgrf_grfid,
                ),
                _ => continue,
            };
            ctx.parameterized_vars.insert((variable, parameter), value);
        }
    }
    ctx
}

fn requested_nearby_vars(
    runtime: &crate::newgrf_sprites::TrainSpriteGraphics,
) -> BTreeSet<(u8, u8)> {
    let mut requested = BTreeSet::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if matches!(term.variable, 0x60..=0x62)
                && let Some(parameter) = term.param
            {
                requested.insert((term.variable, parameter));
            }
        }
    }
    requested
}

fn nearby_tile(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let signed_nibble = |value: u8| {
        let value = i32::from(value & 0x0F);
        if value >= 8 { value - 16 } else { value }
    };
    TileCoord::new(
        base.x
            .saturating_add(signed_nibble(parameter))
            .rem_euclid(width),
        base.y
            .saturating_add(signed_nibble(parameter >> 4))
            .rem_euclid(height),
    )
}

fn nearby_animation_frame(
    map: &Map,
    stations: &[Station],
    source: &Station,
    nearby: TileCoord,
) -> u32 {
    station_at_tile(map, stations, nearby)
        .filter(|candidate| candidate.stop_kind == StopKind::Airport && candidate.pos == source.pos)
        .map_or(u32::MAX, |candidate| {
            if candidate.airport_tiles.contains(&nearby) || candidate.pos == nearby {
                u32::from(map.get(nearby).map_or(0, |tile| tile.m7))
            } else {
                u32::MAX
            }
        })
}

fn nearby_land_info(
    map: &Map,
    stations: &[Station],
    source: &Station,
    nearby: TileCoord,
    climate: Climate,
) -> u32 {
    let Some(tile) = map.get(nearby) else {
        return 0;
    };
    let (tileh, raw_z) = tile_slope_and_z(map, nearby).unwrap_or((0, 0));
    // Airport tiles currently use the post-GRF v8 encoding. Keeping the
    // conversion in one helper makes the old v7 branch explicit when the
    // parser starts retaining Action8's version per tile.
    let z = raw_z;
    let water_bits = water_class(tile).map_or(0, |class| {
        u32::from((class.as_u8().saturating_add(1) & 0x03) << 5)
    });
    let terrain = airport_terrain_type(map, nearby, climate, Some(tile));
    let tile_type = u32::from(tile_kind_as_ottd(map, stations, nearby, tile));
    let same_airport = station_at_tile(map, stations, nearby).is_some_and(|candidate| {
        candidate.stop_kind == StopKind::Airport
            && candidate.pos == source.pos
            && (candidate.airport_tiles.contains(&nearby) || candidate.pos == nearby)
    });
    let terrain_bits = water_bits | (u32::from(tile.kind == TileKind::Water) << 1) | (terrain << 2);
    tile_type << 24
        | u32::from(z) << 16
        | ((terrain_bits << 8) | u32::from(tileh))
        | (u32::from(same_airport) << 8)
}

fn airport_tile_id_at_offset(
    map: &Map,
    stations: &[Station],
    nearby: TileCoord,
    source: &Station,
    tile_catalog: &[AirportTileSpecDef],
    current_grfid: u32,
) -> u32 {
    let Some(candidate) = station_at_tile(map, stations, nearby).filter(|station| {
        station.stop_kind == StopKind::Airport
            && station.pos == source.pos
            && (station.airport_tiles.contains(&nearby) || station.pos == nearby)
    }) else {
        return u32::from(u16::MAX);
    };
    let Some(gfx) = airport_tile_gfx(candidate, map, nearby) else {
        return u32::from(u16::MAX);
    };
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return 0xFF00 | u32::from(gfx);
    }
    let Some(def) = tile_catalog
        .iter()
        .find(|definition| definition.gfx.as_u16() == gfx)
    else {
        return 0xFFFE;
    };
    if !def.has_newgrf_sprites() {
        return 0xFF00 | u32::from(def.subst_id);
    }
    if def.newgrf_grfid == current_grfid {
        u32::from(def.newgrf_local_id)
    } else {
        0xFFFE
    }
}

fn airport_tile_gfx(station: &Station, map: &Map, coord: TileCoord) -> Option<u16> {
    station
        .airport_tile_gfx
        .iter()
        .find(|(candidate, _)| *candidate == coord)
        .map(|(_, gfx)| *gfx)
        .or_else(|| map.get(coord).map(|tile| u16::from(tile.m5)))
}

fn airport_terrain_type(map: &Map, coord: TileCoord, climate: Climate, tile: Option<Tile>) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        if tile.is_some_and(|candidate| candidate.m7 & 0x20 != 0) {
            return 1;
        }
        if tile.is_some_and(|candidate| {
            candidate.kind == TileKind::Grass && candidate.m5 & 0x07 == CLEAR_GROUND_DESERT
        }) {
            return 1;
        }
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(nearby).is_some_and(|candidate| {
                candidate.kind == TileKind::Grass && candidate.m5 & 0x07 == CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

fn tile_kind_as_ottd(map: &Map, stations: &[Station], coord: TileCoord, tile: Tile) -> u8 {
    if tile.kind == TileKind::Station
        && station_at_tile(map, stations, coord)
            .is_some_and(|station| station.stop_kind == StopKind::Airport)
    {
        return 5;
    }
    match tile.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
        TileKind::House => 3,
        TileKind::Forest => 4,
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        TileKind::Grass | TileKind::CoalField | TileKind::Unknown(_) => 0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::too_many_lines)]
mod tests {
    use super::*;
    use crate::airport_tile_spec::AirportTileGfxId;
    use crate::map::Tile;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, DecodedSprite, TrainSpriteAssign,
        TrainSpriteGraphics,
    };

    fn sprite(r: u8, b: u8) -> DecodedSprite {
        DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![r, 0, b, 255],
            mask: Vec::new(),
        }
    }

    #[test]
    fn airport_context_exposes_position_frame_layout_and_neighbours() {
        let mut map = Map::new_flat(4, 4, 2);
        let first = TileCoord::new(1, 1);
        let second = TileCoord::new(2, 1);
        map.set_tile(
            first,
            Tile {
                kind: TileKind::Airport,
                m3: 0x12,
                m7: 3,
                ..map.get(first).expect("tile")
            },
        )
        .expect("first");
        map.set_tile(
            second,
            Tile {
                kind: TileKind::Airport,
                m3: 0x34,
                m7: 4,
                ..map.get(second).expect("tile")
            },
        )
        .expect("second");
        let mut station = Station::new_with_kind(first, StopKind::Airport);
        station.airport_layout = 2;
        station.airport_tiles = vec![first, second];
        station.airport_tile_gfx = vec![(first, 74), (second, 24)];
        station.newgrf_random_bits = 0x55AA;
        let stations = vec![station];
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![sprite(255, 0)], vec![sprite(0, 255)]],
            assigns: vec![TrainSpriteAssign {
                local_id: 3,
                set_id: 7,
            }],
            action2_to_action1: [(0, 0), (1, 1)].into_iter().collect(),
            ..Default::default()
        };
        runtime.action2_var.insert(
            7,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x44,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(0, 0, 3), (1, 4, u32::MAX)],
                default: 0,
            },
        );
        runtime.action2_var.insert(
            8,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x62,
                    param: Some(0x01),
                    adjust: Action2VarAdjust {
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(0, 0, 3), (1, 4, u32::MAX)],
                default: 0,
            },
        );
        let current = AirportTileSpecDef {
            gfx: AirportTileGfxId(74),
            subst_id: 24,
            from_newgrf: true,
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            animation_special_flags: 0,
            newgrf_local_id: 3,
            newgrf_grfid: 0xAABB_CCDD,
            newgrf_preview: Some(sprite(255, 0)),
            newgrf_views: vec![sprite(255, 0), sprite(0, 255)],
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let vanilla = AirportTileSpecDef {
            gfx: AirportTileGfxId(24),
            subst_id: 24,
            from_newgrf: false,
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            animation_special_flags: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 0,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        let catalog = vec![current.clone(), vanilla];
        let mut ctx = action2_eval_ctx_for_airport_tile(
            &map,
            &stations,
            first,
            &catalog,
            &current,
            Climate::Temperate,
        );
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
        assert_eq!(ctx.vars.get(&0x44), Some(&3));
        assert_eq!(ctx.parent_vars.get(&0x40), Some(&2));
        assert_eq!(ctx.parameterized_vars.get(&(0x62, 1)), Some(&0xFF18));
        let selected = current.newgrf_view_runtime(0, &mut ctx);
        assert_eq!(selected.as_ref().map(|sprite| sprite.rgba[0]), Some(255));
    }
}
