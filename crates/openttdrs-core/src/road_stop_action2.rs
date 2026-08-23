//! Contexto Action2 para sprites runtime de `RoadStop`.
//!
//! El resolver conserva los valores propios de una parada vial que ya están
//! representados por el modelo: vista, tipo, terreno, road/tram, frame,
//! random y triggers pendientes. Los scopes de teselas vecinas siguen fuera
//! de alcance y por eso no se sintetizan valores para `66`–`6A`.

use crate::map::{Map, Tile, TileCoord, tile_slope_and_z};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{GrfTypeTranslationTables, reverse_road_type};
use crate::road_type::{road_type_from_tile, tram_road_type_from_tile, vanilla_road_type_catalog};
use crate::station::{Station, StopKind, station_at_tile};
use crate::world_gen::Climate;

/// Construye el contexto Action2 de una tesela `RoadStop` para render runtime.
///
/// Corresponde al subconjunto local de `RoadStopScopeResolver`: `40` (vista),
/// `41` (tipo), `42` (terreno/pendiente), `43`/`44` (road/tram), `49`
/// (frame), `50` (instancia in-world) y `5F` (random + triggers). La
/// traducción de road/tram usa los tipos vanilla disponibles en core; si el
/// save contiene un tipo externo que no está en ese catálogo queda en
/// `0xFF`, de forma observable y sin inventar una identidad local.
#[must_use]
pub fn action2_eval_ctx_for_road_stop_tile(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    view: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(station) = station_at_tile(map, stations, coord) else {
        return ctx;
    };
    let tile = map.get(coord);
    let (tileh, _) = tile_slope_and_z(map, coord).unwrap_or((0, 0));
    let random = u32::from(station.newgrf_random_bits)
        | (u32::from(station.road_stop_random_bits_at(coord)) << 16);

    ctx.random_bits = random;
    ctx.persistent_registers
        .clone_from(&station.newgrf_persistent_regs);
    ctx.vars.insert(
        0x5F,
        random.wrapping_shl(8) | u32::from(station.newgrf_waiting_random_triggers),
    );
    ctx.vars.insert(0x40, u32::from(view));
    ctx.vars.insert(
        0x41,
        match station.stop_kind {
            StopKind::BusStop => 0,
            StopKind::TruckStop => 1,
            _ => 2,
        },
    );
    ctx.vars.insert(
        0x42,
        terrain_type_for_road_stop_tile(map, coord, climate, tile) | (u32::from(tileh) << 8),
    );

    let vanilla_types = vanilla_road_type_catalog();
    let road_type = tile.map_or(u32::MAX, |tile| {
        u32::from(reverse_road_type(
            type_tables,
            &vanilla_types,
            road_type_from_tile(&tile),
        ))
    });
    let tram_type = tile.map_or(u32::MAX, |tile| {
        tram_road_type_from_tile(&tile).map_or(u32::MAX, |tram| {
            u32::from(reverse_road_type(type_tables, &vanilla_types, tram))
        })
    });
    ctx.vars.insert(0x43, road_type);
    ctx.vars.insert(0x44, tram_type);
    ctx.vars
        .insert(0x49, u32::from(station.road_stop_animation_frame_at(coord)));
    // Bit 4 de var 50 sólo se usa cuando no existe tesela (picker/callback de
    // disponibilidad); esta ruta siempre resuelve una instancia en el mapa.
    ctx.vars.insert(0x50, 0);
    ctx
}

fn terrain_type_for_road_stop_tile(
    map: &Map,
    coord: TileCoord,
    climate: Climate,
    tile: Option<Tile>,
) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        if tile.is_some_and(|tile| (tile.m7 & 0x20) != 0) {
            return 1;
        }
        // Una parada tapa el clear original; conservar el chequeo inmediato
        // de StationScope para que desierto tropical siga visible a Action2.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let nearby = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(nearby).is_some_and(|tile| {
                tile.kind == crate::map::TileKind::Grass
                    && (tile.m5 & 0x07) == crate::world_gen::CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::StationRandomTrigger;
    use crate::map::{Tile, TileKind};

    #[test]
    fn road_stop_ctx_exposes_runtime_random_view_type_and_frame() {
        let mut map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 2);
        map.set_tile(
            coord,
            Tile {
                height: 0,
                kind: TileKind::Station,
                mapt: 0,
                m5: 4,
                m1: 0,
                m6: 1,
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            },
        )
        .unwrap();
        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.newgrf_random_bits = 0xA55A;
        station.road_stop_newgrf_random_bits = 0x3C;
        station.newgrf_waiting_random_triggers = StationRandomTrigger::VehicleLoads.mask();
        station.road_stop_animation_frame = 7;
        {
            let state = station.ensure_road_stop_tile_state(coord);
            state.random_bits = 0x3C;
            state.animation_frame = 7;
        }
        station.sync_legacy_road_stop_anchor();
        station.newgrf_persistent_regs.insert(4, 99);

        let ctx = action2_eval_ctx_for_road_stop_tile(
            &map,
            &[station],
            coord,
            4,
            Climate::Temperate,
            None,
        );
        assert_eq!(ctx.random_bits, 0x003C_A55A);
        assert_eq!(
            ctx.vars.get(&0x5F),
            Some(&(0x003C_A55A_u32 << 8 | u32::from(StationRandomTrigger::VehicleLoads.mask())))
        );
        assert_eq!(ctx.vars.get(&0x40), Some(&4));
        assert_eq!(ctx.vars.get(&0x41), Some(&0));
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
        assert_eq!(ctx.vars.get(&0x44), Some(&u32::MAX));
        assert_eq!(ctx.vars.get(&0x49), Some(&7));
        assert_eq!(ctx.persistent_registers.get(&4), Some(&99));
    }
}
