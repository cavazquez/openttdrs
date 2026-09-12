//! Contexto Action2 para `AirportTile` (`GSF_AIRPORTTILES`, feature `0x11`).
//!
//! `OpenTTD` evalúa cada tesela del aeropuerto con dos scopes: el tile actual
//! y el aeropuerto que lo contiene. Este módulo conserva las variables que
//! afectan la selección de sprites (posición, frame, random, layout padre y
//! consultas a teselas vecinas) para que el cliente no tenga que elegir
//! siempre el preview del primer `Action1`.

use std::collections::BTreeSet;

use crate::airport_class::{NewgrfAirportSpecDef, newgrf_airport_spec_def};
use crate::airport_tile_spec::{AirportTileSpecDef, NEW_AIRPORT_TILE_OFFSET};
use crate::house_spec::{distance_square, get_town_radius_group};
use crate::map::{Map, Tile, TileCoord, TileKind, tile_slope_and_z, water_class};
use crate::newgrf_sprites::Action2EvalCtx;
#[cfg(test)]
use crate::station::StopKind;
use crate::station::{Station, station_at_tile};
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
    action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog(
        map,
        stations,
        &[],
        &[],
        coord,
        tile_catalog,
        current_spec,
        climate,
    )
}

/// Variante que materializa el pueblo más cercano para la variable `0x42`.
///
/// `OpenTTD` consulta `ClosestTownFromTile` en el scope de `AirportTile`; el
/// wrapper histórico sin pueblos conserva `TownEdge` para callers que sólo
/// tienen el mapa y las estaciones.
#[must_use]
pub fn action2_eval_ctx_for_airport_tile_with_towns(
    map: &Map,
    stations: &[Station],
    towns: &[crate::town::Town],
    coord: TileCoord,
    tile_catalog: &[AirportTileSpecDef],
    current_spec: &AirportTileSpecDef,
    climate: Climate,
) -> Action2EvalCtx {
    action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog(
        map,
        stations,
        towns,
        &[],
        coord,
        tile_catalog,
        current_spec,
        climate,
    )
}

/// Variante completa que recibe el catálogo `Airports` para el scope padre.
///
/// Un `AirportTile` conserva el GRFID y la tabla local de badges del tile que
/// está resolviendo. Su scope padre, en cambio, consulta los badges asociados
/// al aeropuerto construido. Mantener ambos catálogos separados reproduce
/// `AirportTileResolverObject`: la traducción de `0x7A[param]` viene del GRF
/// del tile, mientras que la presencia se pregunta al `AirportSpec` padre.
/// La API histórica sin catálogo conserva `UINT_MAX` para un padre `NewGRF` que
/// no se puede encontrar.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog(
    map: &Map,
    stations: &[Station],
    towns: &[crate::town::Town],
    airport_catalog: &[NewgrfAirportSpecDef],
    coord: TileCoord,
    tile_catalog: &[AirportTileSpecDef],
    current_spec: &AirportTileSpecDef,
    climate: Climate,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(station) = station_at_tile(map, stations, coord)
        .filter(|candidate| candidate.stop_kind.has_airport_facility())
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
    let town_zone = towns
        .iter()
        .min_by_key(|town| distance_square(town.pos, coord))
        .map_or(crate::town::HouseZone::TownEdge, |town| {
            get_town_radius_group(town, coord)
        });
    ctx.vars.insert(0x42, u32::from(town_zone as u8));
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
    // `AirportScopeResolver` expone las variables explícitas de estación
    // antes de delegar el resto a `Station::GetNewGRFVariable`.
    ctx.parent_vars
        .insert(0xF0, station.stop_kind.facilities_mask());
    ctx.parent_vars
        .insert(0xFA, station.newgrf_build_date_value());

    // El padre vanilla existe y simplemente no tiene badges. Un id NewGRF
    // ausente del catálogo activo, en cambio, equivale a un spec que OpenTTD
    // no puede resolver y debe permanecer como `UINT_MAX`.
    let parent_badges: Option<&[u16]> = match station.airport_newgrf_spec_id {
        Some(spec_id) => newgrf_airport_spec_def(airport_catalog, spec_id)
            .map(|spec| spec.associated_badges.as_slice()),
        None => Some(&[]),
    };

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
        let (self_badges, parent_badges_requested) = requested_badge_vars(runtime);
        for parameter in self_badges {
            let value = badge_variable_result(
                &current_spec.newgrf_badge_translation,
                Some(&current_spec.associated_badges),
                parameter,
            );
            ctx.parameterized_vars.insert((0x7A, parameter), value);
        }
        for parameter in parent_badges_requested {
            let value = badge_variable_result(
                &current_spec.newgrf_badge_translation,
                parent_badges,
                parameter,
            );
            ctx.parent_parameterized_vars
                .insert((0x7A, parameter), value);
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

fn requested_badge_vars(
    runtime: &crate::newgrf_sprites::TrainSpriteGraphics,
) -> (BTreeSet<u8>, BTreeSet<u8>) {
    let mut self_scope = BTreeSet::new();
    let mut parent_scope = BTreeSet::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if term.variable == 0x7A
                && let Some(parameter) = term.param
            {
                if term.adjust.is_parent_scope() {
                    parent_scope.insert(parameter);
                } else {
                    self_scope.insert(parameter);
                }
            }
        }
    }
    (self_scope, parent_scope)
}

/// `GetBadgeVariableResult`: la tabla pertenece al GRF que ejecuta Action2,
/// pero la lista de badges puede provenir del spec de la tesela o de su padre.
fn badge_variable_result(
    translation: &[u16],
    associated_badges: Option<&[u16]>,
    parameter: u8,
) -> u32 {
    let Some(associated_badges) = associated_badges else {
        return u32::MAX;
    };
    translation
        .get(usize::from(parameter))
        .map_or(u32::MAX, |&badge| {
            if badge == u16::MAX {
                u32::MAX
            } else {
                u32::from(associated_badges.contains(&badge))
            }
        })
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
        .filter(|candidate| {
            candidate.stop_kind.has_airport_facility() && candidate.pos == source.pos
        })
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
        candidate.stop_kind.has_airport_facility()
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
        station.stop_kind.has_airport_facility()
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
            .is_some_and(|station| station.stop_kind.has_airport_facility())
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
    use crate::airport_class::{AirportClassId, AirportSpecId, NewgrfAirportSpecDef};
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
        let mut town = crate::town::Town {
            pos: first,
            num_houses: 48,
            ..Default::default()
        };
        crate::town::update_town_radius(&mut town);
        let towns = vec![town];
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
        runtime.action2_var.insert(
            9,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7A,
                    param: Some(0),
                    adjust: Action2VarAdjust {
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(0, 0, 0)],
                default: 0,
            },
        );
        runtime.action2_var.insert(
            10,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7A,
                    param: Some(1),
                    adjust: Action2VarAdjust {
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(0, 0, 0)],
                default: 0,
            },
        );
        // The context builder only needs the variable graph to discover
        // parameterized reads; no Action1 assignment is required for this
        // scope-variable test.
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
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: vec![7],
            newgrf_badge_translation: vec![7],
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
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        let catalog = vec![current.clone(), vanilla];
        let mut ctx = action2_eval_ctx_for_airport_tile_with_towns(
            &map,
            &stations,
            &towns,
            first,
            &catalog,
            &current,
            Climate::Temperate,
        );
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
        assert_eq!(ctx.vars.get(&0x44), Some(&3));
        assert_eq!(ctx.vars.get(&0x42), Some(&4));
        assert_eq!(ctx.parent_vars.get(&0x40), Some(&2));
        assert_eq!(ctx.parameterized_vars.get(&(0x62, 1)), Some(&0xFF18));
        assert_eq!(ctx.parameterized_vars.get(&(0x7A, 0)), Some(&1));
        assert_eq!(ctx.parameterized_vars.get(&(0x7A, 1)), Some(&u32::MAX));
        let selected = current.newgrf_view_runtime(0, &mut ctx);
        assert_eq!(selected.as_ref().map(|sprite| sprite.rgba[0]), Some(255));
    }

    #[test]
    fn airport_tile_parent_badge_selects_the_parent_scope_action2_branch() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(1, 1);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = TileKind::Airport;
        map.set_tile(coord, tile).expect("airport tile");

        let mut station = Station::new_with_kind(coord, StopKind::Airport);
        station.airport_tiles = vec![coord];
        station.airport_newgrf_spec_id = Some(10);

        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![sprite(0xA1, 0)], vec![sprite(0xB2, 0)]],
            assigns: vec![TrainSpriteAssign {
                local_id: 3,
                set_id: 7,
            }],
            action2_to_action1: [(8, 0), (9, 1)].into_iter().collect(),
            ..Default::default()
        };
        // Type 0x82/0x86/0x8A carries the parent marker in `shift`; it must
        // read AirportScope `7A[0]`, not the AirportTile's own badge list.
        runtime.action2_var.insert(
            7,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7A,
                    param: Some(0),
                    adjust: Action2VarAdjust {
                        shift: 0x80,
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(8, 1, 1)],
                default: 9,
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
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            // Deliberadamente distinto: demuestra que la presencia proviene
            // de AirportSpec, pero el índice local sigue siendo del tile GRF.
            associated_badges: vec![31],
            newgrf_badge_translation: vec![42],
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let parent = NewgrfAirportSpecDef {
            id: 10,
            class: AirportClassId::Small,
            label: "Badge airport".into(),
            short_label: "Badge".into(),
            size_x: 1,
            size_y: 1,
            catchment: 4,
            noise_level: 1,
            subst_id: AirportSpecId::Small,
            ttd_airport_type: 0,
            layouts: Vec::new(),
            enabled: true,
            min_year: 0,
            max_year: u16::MAX,
            maintenance_cost: 0,
            associated_badges: vec![42],
            newgrf_local_id: 0,
            newgrf_grfid: 0x1122_3344,
            newgrf_views: Vec::new(),
            newgrf_purchase_views: Vec::new(),
            newgrf_runtime: None,
        };
        let tile_catalog = vec![current.clone()];
        let mut ctx = action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog(
            &map,
            &[station.clone()],
            &[],
            std::slice::from_ref(&parent),
            coord,
            &tile_catalog,
            &current,
            Climate::Temperate,
        );
        assert_eq!(ctx.parameterized_vars.get(&(0x7A, 0)), None);
        assert_eq!(ctx.parent_parameterized_vars.get(&(0x7A, 0)), Some(&1));
        assert_eq!(
            current
                .newgrf_view_runtime(0, &mut ctx)
                .map(|sprite| sprite.rgba[0]),
            Some(0xA1)
        );

        let mut parent_without_badge = parent;
        parent_without_badge.associated_badges.clear();
        let mut ctx_without_badge =
            action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog(
                &map,
                &[station],
                &[],
                &[parent_without_badge],
                coord,
                &tile_catalog,
                &current,
                Climate::Temperate,
            );
        assert_eq!(
            ctx_without_badge.parent_parameterized_vars.get(&(0x7A, 0)),
            Some(&0)
        );
        assert_eq!(
            current
                .newgrf_view_runtime(0, &mut ctx_without_badge)
                .map(|sprite| sprite.rgba[0]),
            Some(0xB2)
        );
    }

    #[test]
    fn airport_tile_parent_scope_exposes_facilities_and_build_date() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(1, 1);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = TileKind::Airport;
        map.set_tile(coord, tile).expect("airport tile");

        let mut station = Station::new_with_kind(coord, StopKind::Airport);
        station.airport_tiles = vec![coord];
        station.build_date = crate::station::STATION_BUILD_DATE_DEFAULT + 123;
        station.newgrf_persistent_regs.insert(7, 0xCAFE_BABE);

        let mut runtime = TrainSpriteGraphics {
            assigns: vec![TrainSpriteAssign {
                local_id: 3,
                set_id: 7,
            }],
            ..Default::default()
        };
        // El primer grupo exige la facility Airport. Si coincide, el segundo
        // devuelve la fecha de construcción del mismo scope padre.
        runtime.action2_var.insert(
            7,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0xF0,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0x80,
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(
                    8,
                    StopKind::Airport.facilities_mask(),
                    StopKind::Airport.facilities_mask(),
                )],
                default: 9,
            },
        );
        runtime.action2_var.insert(
            8,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0xFA,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0x80,
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        runtime.action2_var.insert(
            9,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: 0,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
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
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut ctx = action2_eval_ctx_for_airport_tile(
            &map,
            &[station],
            coord,
            std::slice::from_ref(&current),
            &current,
            Climate::Temperate,
        );
        assert_eq!(
            ctx.parent_vars.get(&0xF0),
            Some(&StopKind::Airport.facilities_mask())
        );
        assert_eq!(ctx.parent_vars.get(&0xFA), Some(&123));
        assert_eq!(ctx.parent_persistent_registers.get(&7), Some(&0xCAFE_BABE));
        assert_eq!(
            current
                .newgrf_runtime
                .as_ref()
                .expect("runtime")
                .resolve_callback_ctx(current.newgrf_local_id, 0, 0, 0, &mut ctx),
            123
        );
    }
}
