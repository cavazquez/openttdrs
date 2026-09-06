//! Estructura del mapa y carga de `.ottdmap` versionado (`MAP1`).
#![allow(clippy::doc_markdown, clippy::expect_used, clippy::unwrap_used)]

mod binary;
pub mod house_lift;
pub mod index;
pub mod industry_action2;
pub mod industry_construction;
pub mod industry_link;
pub mod industry_random;
pub mod industry_terrain;
pub mod industry_tile_anim;
pub mod level_crossing;
pub mod object;
pub mod rail_bits;
pub mod rail_slope;
pub mod rail_topology;
pub mod road_bits;
pub mod slope;
pub mod station_tile_anim;
pub mod tile_loop;
pub mod tree_tile_loop;
mod types;
pub mod water_class;
pub mod water_flood;

#[cfg(test)]
use binary::{OTTDMAP_FLAG_HAS_M2_HI, OTTDMAP_FORMAT_VERSION_CURRENT};
pub(crate) use binary::{OTTDMAP_HEADER_LEN_VERSIONED, OTTDMAP_MAGIC_VERSIONED};
pub use house_lift::{
    LIFT_MAX_POSITION, LiftStep, advance_house_lift, halt_lift, house_tile_has_lift,
    lift_destination, lift_has_destination, lift_position, step_house_lifts, with_lift_destination,
    with_lift_position,
};
pub use index::{
    coord_from_linear_index, coord_to_dense_index, coord_to_linear_index,
    openttd_tile_index_to_coord,
};
pub use industry_action2::{
    action2_eval_ctx_for_industry_tile_with_world,
    action2_eval_ctx_for_industry_tile_with_world_and_cargo_catalog,
    action2_eval_ctx_for_industry_tile_with_world_and_parent,
    action2_eval_ctx_for_industry_tile_with_world_and_parent_and_cargo_catalog,
};
pub use industry_construction::{
    INDUSTRY_CONSTRUCTION_COMPLETED, advance_industry_construction,
    advance_industry_construction_tile_loop_at, industry_construction_counter,
    industry_construction_stage, is_industry_completed, make_industry_tile_bigger,
    step_industry_tiles, step_industry_tiles_with_seed, step_industry_tiles_with_seed_and_catalog,
    step_industry_tiles_with_seed_and_catalog_and_cargo_catalog,
    step_industry_tiles_with_seed_and_catalog_and_world,
    step_industry_tiles_with_seed_and_catalog_and_world_and_cargo_catalog,
};
pub use industry_link::{
    IndustryTileLink, industry_instance_id, industry_tile_link, industry_tiles_mergeable,
};
pub use industry_random::{
    INDUSTRY_RANDOM_TRIGGERS_MASK, IndustryRandomTrigger, advance_industry_tile_randomisation,
    advance_industry_tile_randomisation_from_visits_with_catalog,
    advance_industry_tile_randomisation_from_visits_with_catalog_and_cargo_catalog,
    advance_industry_tile_randomisation_from_visits_with_catalog_and_world,
    advance_industry_tile_randomisation_from_visits_with_catalog_and_world_and_cargo_catalog,
    industry_random_bits, industry_random_triggers, industry_tile_rng, init_industry_tile_random,
    set_industry_random_bits, set_industry_random_triggers, trigger_industry_randomisation_at,
    trigger_industry_randomisation_at_with_catalog_and_world,
    trigger_industry_randomisation_at_with_catalog_and_world_and_cargo_catalog,
    trigger_industry_tile_randomisation,
};
pub use industry_terrain::{
    GFX_OILRIG_FIRST, GFX_OILRIG_LAST, SPR_FLAT_BARE_LAND, SPR_FLAT_GRASS_TILE,
    SPR_FLAT_WATER_TILE, industry_gfx_is_oil_rig, industry_tile_on_water,
    industry_uses_water_ground, tile_adjacent_to_water,
};
pub use industry_tile_anim::{
    GFX_BUBBLE_GENERATOR, GFX_COAL_MINE_TOWER_ANIMATED, GFX_COPPER_MINE_TOWER_ANIMATED,
    GFX_GOLD_MINE_TOWER_ANIMATED, GFX_OILWELL_ANIMATED_1, GFX_OILWELL_ANIMATED_2,
    GFX_OILWELL_ANIMATED_3, IndustryAnimationTrigger, advance_industry_animated_tiles,
    advance_industry_tile_animations, advance_industry_tile_loop_events,
    advance_industry_tile_loop_events_from_visits_with_rng, advance_newgrf_industry_animated_tiles,
    advance_newgrf_industry_animated_tiles_with_world,
    advance_newgrf_industry_animated_tiles_with_world_and_cargo_catalog,
    advance_newgrf_industry_animation_frames, advance_newgrf_industry_animation_frames_with_world,
    advance_newgrf_industry_animation_frames_with_world_and_cargo_catalog,
    bubble_generator_spawns_from_visits, industry_animation_frame, industry_gfx,
    industry_tile_anim_state, set_industry_gfx, trigger_newgrf_industry_animation,
    trigger_newgrf_industry_animation_with_world,
    trigger_newgrf_industry_animation_with_world_and_cargo_catalog,
    trigger_newgrf_industry_animation_with_world_and_extra,
    trigger_newgrf_industry_animation_with_world_and_extra_and_cargo_catalog,
};
pub use level_crossing::is_road_level_crossing;
pub use object::{
    MP_OBJECT_MAPT, OBJECT_TYPE_LIGHTHOUSE, OBJECT_TYPE_OWNED_LAND, OBJECT_TYPE_STATUE_COMPANY,
    OBJECT_TYPE_TRANSMITTER, OTTD_MP_OBJECT, ObjectScopeCounts, action2_eval_ctx_for_object_tile,
    action2_eval_ctx_for_object_tile_with_counts, action2_eval_ctx_for_object_tile_with_map,
    action2_eval_ctx_for_object_tile_with_towns, action2_eval_ctx_for_object_tile_with_world,
    is_map_object_tile, is_newgrf_object_type, is_newgrf_object_type_id, is_owned_land_tile,
    object_footprint_at, object_footprint_tiles, object_id_from_tile, object_origin_from_tile,
    object_origin_from_tile_with_objects, object_spec_id_from_tile, object_tile_offset_byte,
    object_type_dims, object_type_dims_id, object_type_from_tile, object_view_index_for_tile,
    object_view_index_for_type,
};
pub use rail_bits::{
    OTTD_MP_RAILWAY, RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT,
    RAIL_TB_UPPER, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT, RAIL_TILE_NORMAL,
    RAIL_TILE_SIGNALS, effective_rail_trackbits, rail_tile_is_signals,
};
pub use rail_slope::{
    FOUNDATION_ACTION5_SPRITE_BASE, FOUNDATION_INCLINED_X, FOUNDATION_INCLINED_Y,
    FOUNDATION_LEVELED, FOUNDATION_ORIGINAL_SPRITE_BASE, FoundationSpriteBounds,
    RailFoundationDrawPlan, RailFoundationSpriteDraw, RailTrackDrawPlan, RailTrackSpritePass,
    bridge_foundation_for_axis, bridge_surface_slope_and_z, foundation_draw_plan,
    rail_foundation_draw_plan, rail_foundation_for_trackbits, rail_surface_slope_and_z,
    rail_track_draw_plan, rail_trackbits_valid_on_slope,
};
pub use rail_topology::{
    RAIL_TOUCHING_SIDE_NE, RAIL_TOUCHING_SIDE_NW, RAIL_TOUCHING_SIDE_SE, RAIL_TOUCHING_SIDE_SW,
    opposite_diag_dir, rail_bit_for_sides, rail_bits_touching_side, rail_signal_diag_dir_offset,
    rail_traversal_bits,
};
pub use road_bits::{OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE, effective_road_bits};
pub use slope::{
    SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_STEEP, SLOPE_SW, TILE_PIXEL_HEIGHT, complement_slope,
    diag_dir_offset, inclined_slope_direction, is_tunnel_entrance_slope, partial_pixel_z,
    resolve_existing_tunnel_end, resolve_tunnel_end, slope_dz_at_subtile, slope_dz_on_tile,
    slope_pixel_z, tile_slope_and_z, tunnel_entrance_m5, tunnel_path_tiles, tunnel_preview_path,
};
pub use station_tile_anim::{
    AIRPORT_RADAR_FRAMES, airport_radar_frame, is_airport_tower_tile, step_airport_tiles,
    step_newgrf_airport_tiles, step_newgrf_airport_tiles_with_towns, step_newgrf_road_stop_tiles,
    step_newgrf_road_stop_tiles_with_world, step_newgrf_station_tiles,
    step_newgrf_station_tiles_with_towns_and_world_and_cargo_catalog,
    step_newgrf_station_tiles_with_world, step_newgrf_station_tiles_with_world_and_cargo_catalog,
    trigger_newgrf_airport_animation_for_station,
    trigger_newgrf_airport_animation_for_station_with_towns,
    trigger_newgrf_airport_animation_for_station_with_towns_and_cargo_catalog,
    trigger_newgrf_airport_tile_animation, trigger_newgrf_airport_tile_animation_with_towns,
    trigger_newgrf_station_animation, trigger_newgrf_station_animation_for_platform,
    trigger_newgrf_station_animation_for_platform_with_towns_and_world_and_cargo_catalog,
    trigger_newgrf_station_animation_for_platform_with_world,
    trigger_newgrf_station_animation_for_platform_with_world_and_cargo_catalog,
    trigger_newgrf_station_animation_for_station,
    trigger_newgrf_station_animation_for_station_with_towns_and_world_and_cargo_catalog,
    trigger_newgrf_station_animation_for_station_with_world,
    trigger_newgrf_station_animation_for_station_with_world_and_cargo_catalog,
    trigger_newgrf_station_animation_with_towns_and_world_and_cargo_catalog,
    trigger_newgrf_station_animation_with_world,
    trigger_newgrf_station_animation_with_world_and_cargo_catalog,
};
pub use tile_loop::{
    MAP_FULL_SCAN_TILE_LIMIT, MAP_TILE_LOOP_STRIDE, TileLoopState, collect_tile_loop_visits,
    default_cur_tileloop_tile, for_each_map_tile_loop, for_each_map_tile_loop_stripe, map_log_x,
    map_log_y, run_tile_loop, tile_index_bits, tile_index_to_coord,
};
pub use tree_tile_loop::{
    MAX_TREE_OR_FIELD_STAGE, TILE_LOOP_FREQUENCY, TREE_GROWTH_DEAD, TREE_GROWTH_GROWING1,
    TREE_GROWTH_GROWN, TREE_GROWTH_TICK_INTERVAL, TREE_UPDATE_FREQUENCY,
    apply_desert_transition_from_visits, apply_seasonal_snow, clear_tree, is_tropic_desert_zone,
    landscape_tile_cycle, next_clear_update_tick, next_tree_update_tick, normalize_tree_growth,
    plant_tree, step_tree_and_field_growth, tick_tree_tile_loop, tile_loop_clear_desert,
    tree_count, tree_or_field_stage, with_tree_count, with_tree_or_field_stage,
};
pub use types::{
    MapError, OTTD_TILETYPE_TUNNELBRIDGE, TOWN_HOUSE_COMPLETED, Tile, TileCoord, TileKind,
    TownHouseFootprint, TownHouseSpec,
};
pub use water_class::{
    WaterClass, has_tile_water_ground, is_canal_tile, is_coast_tile, is_river_tile,
    make_water_tile, river_tile_is_ship_navigable, set_water_class_m1, tile_has_water_class,
    water_class, water_class_from_m1,
};
pub use water_flood::{
    FloodingBehaviour, clear_neighbour_non_flooding_states, do_flood_tile, flood_vehicles,
    get_flooding_behaviour, make_shore_tile, process_water_flood_from_visits, tick_water_flood,
    tile_loop_water_at,
};

/// Mapa rectangular denso en memoria.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Map {
    width: u32,
    height: u32,
    tiles: Vec<Tile>,
    /// Compatibilidad para exports `.ottdmap` heredados que escribieron cero en
    /// `MAPH` de agua aunque conservaron tierra alta alrededor. No representa
    /// una regla de OpenTTD: los `.sav` y mapas nuevos deben conservar el valor
    /// crudo de `MAPH`.
    #[serde(default)]
    legacy_zero_water_height_repair: bool,
    /// Tipos de objetos importados desde el pool `OBJS` de OpenTTD.
    ///
    /// `m5` en un `MP_OBJECT` forma parte del `ObjectID`; sólo los mapas
    /// locales heredados codifican el tipo visual directamente ahí. El `Option`
    /// distingue ese formato viejo de un import moderno con un pool vacío.
    #[serde(default)]
    imported_object_types: Option<std::collections::BTreeMap<u32, u16>>,
}

impl Map {
    /// Crea un mapa plano con la misma altura en todas las teselas.
    ///
    /// # Panics
    ///
    /// Si `width * height` desborda `u32` o no cabe en `usize` (caso atípico en 64 bits).
    #[must_use]
    pub fn new_flat(width: u32, height: u32, level: u8) -> Self {
        let len = width.checked_mul(height).expect("width*height overflow");
        let count = usize::try_from(len).expect("map tile count must fit usize");
        Self {
            width,
            height,
            tiles: vec![
                Tile {
                    height: level,
                    kind: TileKind::Grass,
                    mapt: 0,
                    m5: 0,
                    m1: 0,
                    m6: 0,
                    m8: 0,
                    m3: 0,
                    m2: 0,
                    m2_hi: 0,
                    m7: 0,
                    m3hi: 0,
                };
                count
            ],
            legacy_zero_water_height_repair: false,
            imported_object_types: None,
        }
    }

    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Indica si el renderer debe reparar el antiguo export `.ottdmap` que
    /// perdió alturas de agua a cero.
    ///
    /// Los datos de un save OpenTTD conservan `MAPH` y deben mantener este flag
    /// apagado para que `GetTileSlopeZ` se reproduzca literalmente.
    #[must_use]
    pub const fn legacy_zero_water_height_repair(&self) -> bool {
        self.legacy_zero_water_height_repair
    }

    /// Activa o desactiva la compatibilidad para exports `.ottdmap` heredados.
    ///
    /// Esta opción pertenece al origen del mapa, no a cada tesela. El cargador
    /// `.sav` la apaga explícitamente después de reutilizar el decodificador
    /// binario común.
    pub fn set_legacy_zero_water_height_repair(&mut self, enabled: bool) {
        self.legacy_zero_water_height_repair = enabled;
    }

    /// Vista densa de todas las teselas (orden fila-mayor: `y * width + x`).
    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Teselas `MP_CLEAR` con `m5 == 0` eran el valor por defecto de [`Self::new_flat`],
    /// no suelo desnudo explícito. Las pasa a hierba completa (`m5 = 3`).
    pub fn migrate_legacy_clear_grass_m5(&mut self) {
        const FULL_GRASS_M5: u8 = 3;
        for tile in &mut self.tiles {
            if tile.kind == TileKind::Grass && tile.mapt == 0 && tile.m5 == 0 {
                tile.m5 = FULL_GRASS_M5;
            }
        }
    }

    /// Cuenta extremos JGR (`tile_n` / `tile_s`) que caen en teselas `MP_TUNNELBRIDGE` del mapa.
    ///
    /// Devuelve `(coincidencias_norte, coincidencias_sur, total_registros)`.
    #[must_use]
    pub fn jgr_tunnel_endpoint_match_stats(
        &self,
        tunnels: &[crate::tnbp_decode::JgrTunnelRecord],
    ) -> (usize, usize, usize) {
        let w = self.width;
        let h = self.height;
        let mut n_ok = 0usize;
        let mut s_ok = 0usize;
        for t in tunnels {
            if let Some(c) = openttd_tile_index_to_coord(t.tile_n, w, h)
                && self.get(c).is_some_and(Tile::is_tunnel_bridge_tile)
            {
                n_ok += 1;
            }
            if let Some(c) = openttd_tile_index_to_coord(t.tile_s, w, h)
                && self.get(c).is_some_and(Tile::is_tunnel_bridge_tile)
            {
                s_ok += 1;
            }
        }
        (n_ok, s_ok, tunnels.len())
    }

    fn index(&self, c: TileCoord) -> Option<usize> {
        coord_to_dense_index(c, self.width, self.height)
    }

    #[must_use]
    pub fn get(&self, c: TileCoord) -> Option<Tile> {
        let i = self.index(c)?;
        self.tiles.get(i).copied()
    }

    pub fn set_height(&mut self, c: TileCoord, height: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].height = height;
        Ok(())
    }

    pub fn set_kind(&mut self, c: TileCoord, kind: TileKind) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].kind = kind;
        Ok(())
    }

    pub fn set_mapt_m5(&mut self, c: TileCoord, mapt: u8, m5: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].mapt = mapt;
        self.tiles[i].m5 = m5;
        Ok(())
    }

    pub fn set_m1(&mut self, c: TileCoord, m1: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].m1 = m1;
        Ok(())
    }

    pub fn set_m2(&mut self, c: TileCoord, m2: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].m2 = m2;
        Ok(())
    }

    /// Escribe un `IndustryID`/`TownID` completo en los dos bytes de MAP2.
    ///
    /// La API histórica [`Self::set_m2`] sigue siendo de un byte para
    /// reservas PBS, árboles y fixtures legacy; las entidades que usan el
    /// pool nativo deben pasar por esta variante para conservar IDs mayores
    /// que 255.
    pub fn set_m2_u16(&mut self, c: TileCoord, m2: u16) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        let [low, high] = m2.to_le_bytes();
        self.tiles[i].m2 = low;
        self.tiles[i].m2_hi = high;
        Ok(())
    }

    pub fn set_m3(&mut self, c: TileCoord, m3: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].m3 = m3;
        Ok(())
    }

    /// Coloca una casa terminada conservando la altura del terreno.
    pub fn set_completed_house(
        &mut self,
        c: TileCoord,
        house_id: u16,
        age: u8,
    ) -> Result<(), MapError> {
        let height = self.get(c).map_or(0, |t| t.height);
        self.set_tile(c, Tile::completed_house(house_id, age, height))
    }

    /// Materializa `MakeHouseTile` para una casa creada por el crecimiento de
    /// un pueblo. A diferencia de [`Self::set_completed_house`], conserva los
    /// bits aleatorios, el estado de obra y el nibble bajo de `MAPT`.
    pub fn make_town_house(&mut self, c: TileCoord, spec: TownHouseSpec) -> Result<(), MapError> {
        self.make_town_house_footprint(c, spec, TownHouseFootprint::OneByOne)
    }

    /// Materializa la secuencia completa de `MakeTownHouse` para una huella
    /// vanilla. Valida todos los límites antes de escribir, de modo que una
    /// huella que sobresale del mapa no deja una construcción parcial.
    ///
    /// Cada subtesela conserva sus propios `MAPH` y nibble bajo de `MAPT`.
    /// El `HouseID` se incrementa en el mismo orden de OpenTTD: base, `+Y`,
    /// `+X`, `+X+Y`.
    pub fn make_town_house_footprint(
        &mut self,
        base: TileCoord,
        spec: TownHouseSpec,
        footprint: TownHouseFootprint,
    ) -> Result<(), MapError> {
        let (parts, len) = footprint.parts(base);
        let mut indices = [0_usize; 4];
        for (index, part) in indices.iter_mut().zip(parts.iter()).take(len) {
            *index = self.index(*part).ok_or(MapError::OutOfBounds)?;
        }

        for (offset, index) in indices.iter().copied().take(len).enumerate() {
            // `ClearMakeHouseTile` ejecuta primero `DoClearSquare`, que
            // reactiva las costas/agua vecinas antes de escribir `MP_HOUSE`.
            // Repetirlo por subtesela conserva el orden de una huella
            // multitile y sus efectos laterales observables de MAP3.
            crate::map::water_flood::clear_neighbour_non_flooding_states(self, parts[offset]);
            let previous = self.tiles[index];
            let sub_spec = TownHouseSpec {
                house_id: spec.house_id.wrapping_add([0, 1, 2, 3][offset]),
                ..spec
            };
            self.tiles[index] = Tile::town_house(sub_spec, previous.height, previous.mapt);
        }
        Ok(())
    }

    /// Atribuye una casa al pueblo que la contiene (`MAP2`/`TownID`).
    pub fn set_house_town_id(&mut self, c: TileCoord, town_id: u32) -> Result<(), MapError> {
        let mut tile = self.get(c).ok_or(MapError::OutOfBounds)?;
        if tile.kind != TileKind::House {
            return Err(MapError::OutOfBounds);
        }
        let town_id = u16::try_from(town_id).unwrap_or(u16::MAX);
        let [town_id_lo, town_id_hi] = town_id.to_le_bytes();
        tile.m2 = town_id_lo;
        tile.m2_hi = town_id_hi;
        self.set_tile(c, tile)
    }

    /// Sustituye la tesela en `c` (tests, fixtures y herramientas de edición).
    pub fn set_tile(&mut self, c: TileCoord, tile: Tile) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i] = tile;
        Ok(())
    }

    #[must_use]
    pub fn get_kind(&self, c: TileCoord) -> Option<TileKind> {
        let i = self.index(c)?;
        Some(self.tiles[i].kind)
    }
}

#[cfg(test)]
mod ottdmap_binary_tests {
    use super::*;

    #[test]
    fn tile_layout_size_is_documented() {
        // OpenTTD: 12 B/tile (8+4). Nuestro AoS con TileKind redundante ≈ 14 B.
        assert_eq!(size_of::<Tile>(), 14);
        assert_eq!(size_of::<TileKind>(), 2);
        assert_eq!(align_of::<Tile>(), 2);
    }

    fn push_map1_header(v: &mut Vec<u8>, w: u32, h: u32) {
        v.extend_from_slice(OTTDMAP_MAGIC_VERSIONED);
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&OTTDMAP_FORMAT_VERSION_CURRENT.to_le_bytes());
        v.extend_from_slice(&OTTDMAP_FLAG_HAS_M2_HI.to_le_bytes());
    }

    #[allow(clippy::too_many_arguments)] // helper de test: un plano denso por argumento
    fn build_ottdmap_2x2(
        mapt: [u8; 4],
        heights: [u8; 4],
        m1: [u8; 4],
        m2: [u8; 4],
        m2_hi: [u8; 4],
        m3: [u8; 4],
        m3hi: [u8; 4],
        m5: [u8; 4],
        m6: [u8; 4],
        m7: [u8; 4],
        m8: [u16; 4],
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(16 + 12 * 4);
        push_map1_header(&mut v, 2, 2);
        v.extend_from_slice(&mapt);
        v.extend_from_slice(&heights);
        v.extend_from_slice(&m1);
        v.extend_from_slice(&m2);
        v.extend_from_slice(&m2_hi);
        v.extend_from_slice(&m3);
        v.extend_from_slice(&m3hi);
        v.extend_from_slice(&m5);
        v.extend_from_slice(&m6);
        v.extend_from_slice(&m7);
        for x in m8 {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v
    }

    /// Mapa binario 2×2 con una tesela casa y `m8 = 42` en el origen.
    fn minimal_ottdmap_v1() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00], // MAPT: tesela 0 = MP_HOUSE
            [1, 1, 1, 1],             // MAPH
            [0; 4],                   // m1
            [0; 4],                   // m2
            [0; 4],                   // m2_hi
            [0; 4],                   // m3
            [0; 4],                   // m3hi
            [0; 4],                   // m5
            [0; 4],                   // m6
            [0; 4],                   // m7
            [42, 0, 0, 0],            // m8
        )
    }

    fn minimal_ottdmap_with_m3() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0; 4],
            [0; 4],
            [0xAB, 0, 0, 0], // m3
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [42, 0, 0, 0],
        )
    }

    /// Formato v1 completo + footer INDP ficticio (ignorado por `from_ottd_binary`).
    fn minimal_ottdmap_v5_with_footer() -> Vec<u8> {
        let mut v = build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0x11, 0, 0, 0], // m2
            [0; 4],          // m2_hi
            [0xAB, 0, 0, 0], // m3
            [0x33, 0, 0, 0], // m3hi
            [0; 4],          // m5
            [0; 4],          // m6
            [0x22, 0, 0, 0], // m7
            [42, 0, 0, 0],   // m8
        );
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&0_u32.to_le_bytes()); // count = 0
        v
    }

    #[test]
    fn set_completed_house_marks_finished_and_sets_house_id() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        map.set_completed_house(c, 42, 10).unwrap();
        let t = map.get(c).expect("house tile");
        assert_eq!(t.kind, TileKind::House);
        assert_eq!(t.m8, 42);
        assert_eq!(t.m3 & 0x80, 0x80);
        assert_eq!(t.m5, 10);
    }

    #[test]
    fn set_house_town_id_roundtrips_the_map2_word() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        map.set_completed_house(c, 42, 10).unwrap();
        map.set_house_town_id(c, 0x1234).unwrap();
        let tile = map.get(c).expect("house tile");
        assert_eq!(u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8), 0x1234);
    }

    #[test]
    fn set_m2_u16_roundtrips_an_industry_id_above_255() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(1, 1);
        map.set_m2_u16(c, 0x0348).unwrap();
        let tile = map.get(c).expect("map tile");
        assert_eq!(tile.m2, 0x48);
        assert_eq!(tile.m2_hi, 0x03);
        assert_eq!(u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8), 0x0348);
    }

    #[test]
    fn make_town_house_replays_completed_native_house_bytes() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(1, 1);
        map.set_height(c, 3).unwrap();
        map.make_town_house(
            c,
            TownHouseSpec {
                house_id: 26,
                town_id: 0,
                random_bits: 157,
                construction_counter: 0,
                construction_stage: TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 0,
            },
        )
        .unwrap();

        let tile = map.get(c).expect("house tile");
        assert_eq!(tile.height, 3);
        assert_eq!(tile.kind, TileKind::House);
        assert_eq!(tile.mapt, 0x30);
        assert_eq!(tile.m1, 157);
        assert_eq!(tile.m2, 0);
        assert_eq!(tile.m2_hi, 0);
        assert_eq!(tile.m3, 0x80);
        assert_eq!(tile.m5, 0);
        assert_eq!(tile.m6, 0);
        assert_eq!(tile.m7, 0);
        assert_eq!(tile.m8, 26);
        assert_eq!(tile.m3hi, 0);
    }

    #[test]
    fn make_town_house_clears_neighbour_non_flooding_water_state() {
        let mut map = Map::new_flat(3, 3, 0);
        let house = TileCoord::new(1, 1);
        let coast = TileCoord::new(1, 0);
        assert!(crate::map::make_shore_tile(&mut map, coast).is_ok());
        let mut coast_tile = map.get(coast).expect("coast");
        coast_tile.m3 = 1;
        assert!(map.set_tile(coast, coast_tile).is_ok());

        assert!(
            map.make_town_house(
                house,
                TownHouseSpec {
                    house_id: 26,
                    town_id: 0,
                    random_bits: 157,
                    construction_counter: 0,
                    construction_stage: TOWN_HOUSE_COMPLETED,
                    is_protected: false,
                    processing_time: 0,
                },
            )
            .is_ok()
        );

        assert_eq!(map.get(coast).expect("coast").m3 & 1, 0);
    }

    #[test]
    fn make_town_house_encodes_construction_and_preserves_low_mapt_bits() {
        let mut map = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(1, 1);
        map.set_mapt_m5(c, 0x0B, 0).unwrap();
        map.make_town_house(
            c,
            TownHouseSpec {
                house_id: 0x1234,
                town_id: 0x1234,
                random_bits: 0xAB,
                construction_counter: 5,
                construction_stage: 2,
                is_protected: true,
                processing_time: 17,
            },
        )
        .unwrap();

        let tile = map.get(c).expect("house tile");
        assert_eq!(tile.mapt, 0x3B);
        assert_eq!(tile.m1, 0xAB);
        assert_eq!(tile.m2, 0x34);
        assert_eq!(tile.m2_hi, 0x12);
        assert_eq!(tile.m3, 0x20);
        assert_eq!(tile.m5, 0x15);
        assert_eq!(tile.m6, 17 << 2);
        assert_eq!(tile.m8, 0x234);
    }

    fn fixture_house_spec(
        house_id: u16,
        town_id: u32,
        random_bits: u8,
        construction_counter: u8,
        construction_stage: u8,
    ) -> TownHouseSpec {
        TownHouseSpec {
            house_id,
            town_id,
            random_bits,
            construction_counter,
            construction_stage,
            is_protected: false,
            processing_time: 0,
        }
    }

    fn assert_native_town_house(
        map: &Map,
        coord: TileCoord,
        height: u8,
        town_id: u8,
        random_bits: u8,
        construction: u8,
        house_id: u16,
    ) {
        assert_eq!(
            map.get(coord),
            Some(Tile {
                height,
                kind: TileKind::House,
                mapt: 0x30,
                m5: construction,
                m1: random_bits,
                m6: 0,
                m8: house_id,
                m3: if construction == 0 { 0x80 } else { 0 },
                m2: town_id,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            })
        );
    }

    #[test]
    fn make_town_house_footprint_replays_openttd_1x2_fixture() {
        // OpenTTD 128², ártico, 1975, seed 1330935378, fase towns:
        // base (84,52) HouseID 66; su segundo tile +Y es HouseID 67.
        let mut map = Map::new_flat(128, 128, 0);
        let base = TileCoord::new(84, 52);
        map.set_height(base, 5).unwrap();
        map.set_height(TileCoord::new(84, 53), 5).unwrap();
        map.make_town_house_footprint(
            base,
            fixture_house_spec(66, 1, 183, 0, TOWN_HOUSE_COMPLETED),
            TownHouseFootprint::OneByTwo,
        )
        .unwrap();

        assert_native_town_house(&map, base, 5, 1, 183, 0, 66);
        assert_native_town_house(&map, TileCoord::new(84, 53), 5, 1, 183, 0, 67);
    }

    #[test]
    fn make_town_house_footprint_replays_openttd_2x1_fixture() {
        // OpenTTD 128², ártico, 1975, seed 1330935378, fase towns:
        // base (105,62) HouseID 74; su segundo tile +X es HouseID 75.
        let mut map = Map::new_flat(128, 128, 0);
        let base = TileCoord::new(105, 62);
        map.set_height(base, 1).unwrap();
        map.set_height(TileCoord::new(106, 62), 1).unwrap();
        map.make_town_house_footprint(
            base,
            fixture_house_spec(74, 5, 215, 0, TOWN_HOUSE_COMPLETED),
            TownHouseFootprint::TwoByOne,
        )
        .unwrap();

        assert_native_town_house(&map, base, 1, 5, 215, 0, 74);
        assert_native_town_house(&map, TileCoord::new(106, 62), 1, 5, 215, 0, 75);
    }

    #[test]
    fn make_town_house_footprint_replays_openttd_2x2_fixture() {
        // OpenTTD 128², ártico, 1975, seed 1330935378, fase towns:
        // la obra de base (24,87) usa 32,33,34,35 en orden base,+Y,+X,+X+Y.
        let mut map = Map::new_flat(128, 128, 0);
        let base = TileCoord::new(24, 87);
        for coord in [
            base,
            TileCoord::new(24, 88),
            TileCoord::new(25, 87),
            TileCoord::new(25, 88),
        ] {
            map.set_height(coord, 1).unwrap();
        }
        map.make_town_house_footprint(
            base,
            fixture_house_spec(32, 4, 221, 3, 2),
            TownHouseFootprint::TwoByTwo,
        )
        .unwrap();

        assert_native_town_house(&map, base, 1, 4, 221, 19, 32);
        assert_native_town_house(&map, TileCoord::new(24, 88), 1, 4, 221, 19, 33);
        assert_native_town_house(&map, TileCoord::new(25, 87), 1, 4, 221, 19, 34);
        assert_native_town_house(&map, TileCoord::new(25, 88), 1, 4, 221, 19, 35);
    }

    #[test]
    fn make_town_house_footprint_is_atomic_when_a_subtile_is_out_of_bounds() {
        let mut map = Map::new_flat(2, 2, 0);
        map.set_mapt_m5(TileCoord::new(1, 1), 0x0D, 7).unwrap();
        let before = map.clone();

        assert_eq!(
            map.make_town_house_footprint(
                TileCoord::new(1, 1),
                fixture_house_spec(32, 0, 1, 0, TOWN_HOUSE_COMPLETED),
                TownHouseFootprint::TwoByTwo,
            ),
            Err(MapError::OutOfBounds)
        );
        assert_eq!(map.tiles(), before.tiles());
    }

    #[test]
    fn make_town_house_footprint_preserves_each_subtile_height_and_mapt_low_bits() {
        let mut map = Map::new_flat(2, 2, 0);
        let base = TileCoord::new(0, 0);
        let second = TileCoord::new(0, 1);
        map.set_height(base, 3).unwrap();
        map.set_height(second, 7).unwrap();
        map.set_mapt_m5(base, 0x03, 0).unwrap();
        map.set_mapt_m5(second, 0x0E, 0).unwrap();

        map.make_town_house_footprint(
            base,
            fixture_house_spec(66, 0, 1, 0, TOWN_HOUSE_COMPLETED),
            TownHouseFootprint::OneByTwo,
        )
        .unwrap();

        let first = map.get(base).unwrap();
        let last = map.get(second).unwrap();
        assert_eq!((first.height, first.mapt, first.m8), (3, 0x33, 66));
        assert_eq!((last.height, last.mapt, last.m8), (7, 0x3E, 67));
    }

    #[test]
    fn from_ottd_binary_loads_house_m8() {
        let bytes = minimal_ottdmap_v1();
        let map = Map::from_ottd_binary(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        assert!(map.legacy_zero_water_height_repair());
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.kind, TileKind::House);
        assert_eq!(t0.m8, 42);
        let t1 = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(t1.kind, TileKind::Grass);
        assert_eq!(t0.m3, 0);
        assert_eq!(t1.m3, 0);
    }

    #[test]
    fn from_ottd_binary_loads_m3_v4() {
        let bytes = minimal_ottdmap_with_m3();
        let map = Map::from_ottd_binary(&bytes).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m3, 0xAB);
        let t1 = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(t1.m3, 0);
    }

    #[test]
    fn from_ottd_binary_loads_v5_planes_and_ignores_indp_footer() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_v5_with_footer()).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m2, 0x11);
        assert_eq!(t0.m7, 0x22);
        assert_eq!(t0.m3hi, 0x33);
        assert_eq!(t0.m3, 0xAB);
    }

    #[test]
    fn from_ottd_binary_loads_versioned_header() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_v5_with_footer()).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m2, 0x11);
        assert_eq!(t0.m7, 0x22);
        assert_eq!(t0.m3hi, 0x33);
    }

    #[test]
    fn from_ottd_binary_with_extras_reads_indp() {
        let bytes = minimal_ottdmap_v5_with_footer();
        let (map, ex) = Map::from_ottd_binary_with_extras(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        assert!(ex.industry_types.is_empty());
    }

    #[test]
    fn from_ottd_binary_loads_m2_hi_plane() {
        let v = build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0x11, 0, 0, 0],
            [0xAA, 0, 0, 0xBB], // m2_hi (tesela 3 = 0xBB)
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
        );
        let map = Map::from_ottd_binary(&v).expect("mapa válido");
        let t3 = map.get(TileCoord::new(1, 1)).expect("tile");
        assert_eq!(t3.m2_hi, 0xBB);
        assert_eq!(t3.m2, 0);
    }

    #[test]
    fn from_ottd_binary_rejects_bad_magic() {
        let mut b = minimal_ottdmap_v1();
        b[0] = b'X';
        assert!(Map::from_ottd_binary(&b).is_err());
    }

    #[test]
    fn mp_tunnelbridge_maps_to_tunnel_and_bridge_kinds() {
        let base = (
            [1; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4],
        );
        // TransportType de OpenTTD en bits 2–3 de m5: 0 = rail, 1 = road.
        let road_tunnel = build_ottdmap_2x2(
            [0x90, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0x04, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&road_tunnel).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RoadTunnel
        );

        let rail_tunnel = build_ottdmap_2x2(
            [0x90, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&rail_tunnel).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RailTunnel
        );

        let rail_bridge = build_ottdmap_2x2(
            [0x90, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0x80, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&rail_bridge).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RailBridge
        );

        let road_bridge = build_ottdmap_2x2(
            [0x90, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0x84, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&road_bridge).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RoadBridge
        );
    }

    #[test]
    fn mp_road_and_rail_depot_subtypes_map_to_depot_kinds() {
        let base = (
            [1; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4], [0; 4],
        );
        let road_depot = build_ottdmap_2x2(
            [0x20, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0x82, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&road_depot).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RoadDepot
        );
        assert_eq!(map.get(TileCoord::new(0, 0)).expect("t").m5 & 0x03, 2);

        let rail_depot = build_ottdmap_2x2(
            [0x10, 0, 0, 0],
            base.0,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            [0xC0, 0, 0, 0],
            base.7,
            base.8,
            [0; 4],
        );
        let map = Map::from_ottd_binary(&rail_depot).expect("map");
        assert_eq!(
            map.get(TileCoord::new(0, 0)).expect("t").kind,
            TileKind::RailDepot
        );
    }

    #[test]
    fn from_ottd_binary_rejects_legacy_mapo_header() {
        let mut b = minimal_ottdmap_v1();
        b[0..4].copy_from_slice(b"MAPO");
        assert!(Map::from_ottd_binary(&b).is_err());
    }

    /// `.ottdmap` v1: una tesela `MP_WATER` Coast (`m5 = 0x10` en bits 4–7).
    fn minimal_ottdmap_water_coast_v1() -> Vec<u8> {
        let mut v = Vec::with_capacity(16 + 12);
        push_map1_header(&mut v, 1, 1);
        v.push(0x60); // MAPT nibble alto 6 = MP_WATER
        v.push(3); // MAPH
        v.push(0); // m1
        v.push(0); // m2
        v.push(0); // m2_hi
        v.push(0); // m3
        v.push(0); // m3hi
        v.push(0x10); // m5: Coast
        v.push(0); // m6
        v.push(0); // m7
        v.extend_from_slice(&0u16.to_le_bytes()); // m8
        v
    }

    /// 2×2 v1: hierba + agua Clear + agua Coast (comprueba que `m5` no se pierde por celda).
    fn minimal_ottdmap_mixed_water_v1() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x00, 0x60, 0x60, 0x60], // MAPT
            [4, 1, 1, 1],             // MAPH
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0, 0, 0x10, 0], // m5: Clear agua, Coast, Clear agua
            [0; 4],
            [0; 4],
            [0; 4],
        )
    }

    #[test]
    fn from_ottd_binary_preserves_water_coast_m5() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_water_coast_v1()).expect("mapa válido");
        assert_eq!(map.dimensions(), (1, 1));
        let t = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t.kind, TileKind::Water);
        assert_eq!(t.m5, 0x10, "WaterTileType::Coast en bits 4–7");
        assert_eq!((t.m5 >> 4) & 0x0F, 1);
    }

    #[test]
    fn from_ottd_binary_mixed_water_m5_per_tile() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_mixed_water_v1()).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        let clear_land = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(clear_land.kind, TileKind::Grass);
        assert_eq!(clear_land.m5, 0);

        let sea_clear = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(sea_clear.kind, TileKind::Water);
        assert_eq!(sea_clear.m5, 0);
        assert_eq!((sea_clear.m5 >> 4) & 0x0F, 0, "Clear");

        let coast = map.get(TileCoord::new(0, 1)).expect("tile");
        assert_eq!(coast.kind, TileKind::Water);
        assert_eq!(coast.m5, 0x10);
        assert_eq!((coast.m5 >> 4) & 0x0F, 1, "Coast");

        let sea2 = map.get(TileCoord::new(1, 1)).expect("tile");
        assert_eq!(sea2.kind, TileKind::Water);
        assert_eq!(sea2.m5, 0);
    }
}

#[cfg(test)]
mod map_set_tile_tests {
    use super::*;

    #[test]
    fn set_tile_replaces_cell() {
        let mut m = Map::new_flat(2, 2, 0);
        let c = TileCoord::new(0, 0);
        let mut t = m.get(c).expect("t");
        t.m5 = 0x2A;
        m.set_tile(c, t).expect("ok");
        assert_eq!(m.get(c).expect("t").m5, 0x2A);
    }
}
