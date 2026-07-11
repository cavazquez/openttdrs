//! Constantes y lógica de sprites de `OpenGFX`.

use bevy::prelude::Color;
use openttdrs_core::{Map, TileCoord, TileKind};

#[path = "sprites/bridge_draw_data_generated.rs"]
#[allow(dead_code)]
mod bridge_draw_data_generated;
#[path = "sprites/bridge_sprites_generated.rs"]
#[allow(dead_code)]
mod bridge_sprites_generated;
#[path = "sprites/bridge_structure_palette.rs"]
pub(crate) mod bridge_structure_palette;
#[path = "sprites/company_palette.rs"]
pub(crate) mod company_palette;
#[path = "sprites/copper_smoke_draw_data_generated.rs"]
mod copper_smoke_draw_data_generated;
#[path = "sprites/effect_vehicle_draw_data_generated.rs"]
mod effect_vehicle_draw_data_generated;
#[path = "sprites/field_draw_data_generated.rs"]
mod field_draw_data_generated;
#[path = "sprites/foundation.rs"]
mod foundation;
#[path = "sprites/house_draw_data_generated.rs"]
mod house_draw_data_generated;
#[path = "sprites/industry.rs"]
mod industry;
#[path = "sprites/industry_draw_proc.rs"]
mod industry_draw_proc;
#[path = "sprites/rail.rs"]
mod rail;
#[path = "sprites/road.rs"]
mod road;
#[path = "sprites/shore_draw_data_generated.rs"]
mod shore_draw_data_generated;
#[path = "sprites/smoke_draw_data_generated.rs"]
mod smoke_draw_data_generated;
#[path = "sprites/station.rs"]
pub(crate) mod station;
#[path = "sprites/tile_atlas_generated.rs"]
mod tile_atlas_generated;
#[path = "sprites/tree_draw_data_generated.rs"]
mod tree_draw_data_generated;
#[path = "sprites/tunnel.rs"]
mod tunnel;

pub(crate) use tile_atlas_generated::{
    TILE_ATLAS_NAMES, TILE_ATLAS_PAGE_COUNT, TILE_ATLAS_PAGE_RANGES, TILE_ATLAS_PAGE_SIZES,
    TILE_ATLAS_RECTS,
};

// ── Constantes de renderizado de carreteras y vías ───────────────────────────

/// Tipos de tesela `OpenTTD` (nibble alto del byte MAPT).
pub const OTTD_MP_RAIL: u8 = 1;
pub const OTTD_MP_ROAD: u8 = 2;
pub const OTTD_MP_TUNNELBRIDGE: u8 = 9;

/// `RailGroundType::SnowOrDesert` en los 4 bits bajos de `m4`/`m3` en vía normal (`rail_map.h`).
pub const RAIL_GROUND_SNOW_OR_DESERT: u8 = 12;

/// `IsOnSnowOrDesert` (`road_map.h`): bit 5 de **MAP7** en teselas `MP_ROAD`.
#[must_use]
pub fn road_tile_snow_or_desert(mapt: u8, kind: TileKind, m7: u8) -> bool {
    kind == TileKind::Road && (mapt >> 4) & 0xF == OTTD_MP_ROAD && (m7 & 0x20) != 0
}

/// Color del sprite `road_flat_*` (nieve/desierto vía MAP7).
#[must_use]
pub fn road_flat_sprite_color(mapt: u8, kind: TileKind, m7: u8) -> Color {
    if road_tile_snow_or_desert(mapt, kind, m7) {
        Color::srgb(0.82, 0.88, 0.98)
    } else {
        Color::WHITE
    }
}

/// Color base de raíles (vía + nieve en suelo cuando `m3` bajo coincide con `RAIL_GROUND_SNOW_OR_DESERT`).
#[must_use]
pub fn rail_track_base_color(mapt: u8, kind: TileKind, m5: u8, m3: u8) -> Color {
    const BASE: Color = Color::srgb(0.88, 0.88, 0.97);
    if kind != TileKind::Rail {
        return BASE;
    }
    if (mapt >> 4) & 0xF != OTTD_MP_RAIL {
        return BASE;
    }
    let subtype = (m5 >> 6) & 0x3;
    if subtype > RAIL_TILE_SIGNALS {
        return BASE;
    }
    if (m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT {
        Color::srgb(0.72, 0.80, 0.94)
    } else {
        BASE
    }
}

pub use road::{
    ROAD_DEPOT_GROUND_PATH, ROAD_FLAT_OFFSET_TBL, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS,
    road_depot_build_layers, road_depot_entrance_road_bits, road_depot_seq_gfx, road_tile_roadside,
    roadside_is_paved,
};

/// Mitad de la altura en px de cada variante `road_flat_XX`.
pub const ROAD_FLAT_HALF_H: [f32; 19] = [
    15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 19.5, 11.5, 11.5, 19.5, 15.5,
    15.5, 15.5, 15.5,
];

pub(crate) use company_palette::{
    CompanyColoredSprites, CompanyColour, company_colour_name, company_colour_swatch_color,
    company_colour_tooltip, tile_filename,
};
#[allow(unused_imports)]
pub use foundation::{
    FOUNDATION_LEVELED_GFX, FOUNDATION_SPRITE_BASE, FoundationGfx, foundation_asset_path,
    foundation_gfx_for_tileh, foundation_sprite_id, leveled_foundation_z_delta,
};
#[allow(unused_imports)]
pub use industry::{
    FIZZY_DRINK_SPRITE_IDS, INDUSTRY_GFX_DATA, INDUSTRY_GFX_STAGES, INDUSTRY_GFX_TABLE_LEN,
    IndustryGfxSprite, IndustryGfxStatus, REFINERY_FIRE_SPRITE_IDS, debug_log_industry_gfx_once,
    industry_anim_layer_used_in_any_frame, industry_animation_frame_from_m4,
    industry_building_needs_client_anim, industry_construction_stage_from_tile,
    industry_effective_m4_for_draw, industry_gfx_draw_index, industry_gfx_empty_row_is_expected,
    industry_gfx_entry, industry_gfx_entry_for_tile, industry_gfx_entry_staged,
    industry_gfx_status, industry_gfx_status_label, industry_gfx_table_subindex,
    industry_gfx_uses_fizzy_drink_anim, industry_gfx_uses_generic_fallback,
    industry_gfx_uses_random_colour, industry_gfx_uses_refinery_fire_anim,
    industry_palette_colour_for_instance, industry_sprite_for_gfx,
    industry_sprite_uses_fizzy_drink_anim, industry_tile_anim_state, log_industry_gfx_once,
};
#[allow(unused_imports)]
pub use industry_draw_proc::{
    DrawProcLayer, INDUSTRY_DRAW_PROC_SPRITE_IDS, industry_draw_proc,
    industry_draw_proc_anim_frame, industry_draw_proc_dynamic_layers, industry_draw_proc_extended,
    industry_draw_proc_for_tile,
};
#[allow(unused_imports)]
pub use rail::{
    CATENARY_ENTRANCE_SPRITE_BASE, CatenarySpriteDraw, MAGLEV_RAIL_SPRITE_OFFSET,
    MONO_RAIL_SPRITE_OFFSET, PYLON_SPRITE_BASE, RAIL_DEPOT_GROUND_TRACK, RAIL_SPRITE_IDS,
    RAIL_SPRITE_TRACK_X, RAIL_SPRITE_TRACK_Y, RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT,
    RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y,
    RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SignalSpriteDraw, WIRE_SPRITE_BASE,
    WIRE_SPRITE_LAST, catenary_hidden, catenary_pylon_sprite_ids, catenary_sprite_color,
    catenary_tile_location_group, catenary_tileh_selector, catenary_transparent,
    catenary_tunnel_wire_sprite, catenary_wire_sprite_ids, collect_catenary_bridge_draws,
    collect_catenary_pylons_from_map, collect_catenary_sprites, collect_catenary_sprites_from_map,
    collect_rail_ghost_sprites, collect_rail_ghost_sprites_for_type, collect_rail_sprites,
    collect_rail_sprites_for_type, collect_signal_sprite_draws, collect_signal_sprite_ids,
    is_typed_rail_track_sprite, level_crossing_has_rail_reservation, level_crossing_rail_sprite_id,
    level_crossing_rail_sprite_id_for_type, rail_depot_build_layers, rail_ghost_overlay_offset,
    rail_signal_present_mask, rail_signal_state_mask, rail_signal_subtile_offset,
    rail_sprite_atlas_keys, rail_sprite_ids_for_preload, rail_tile_has_pbs_reservation,
    rail_tile_is_signals, remap_rail_sprite_id, set_catenary_preferences, signal_draw_pos,
    signal_screen_position, signal_sprite_bases, signal_sprite_center_offset,
    signal_sprite_ids_for_preload, signal_sprite_texture_id,
};
#[allow(unused_imports)]
pub use station::{
    StationTileClass, rail_station_axis_y, rail_station_draw_layers,
    rail_station_ground_track_sprite, rail_station_overlay_rel, rail_station_sprite_layers,
    rail_station_sprite_meta, rail_waypoint_draw_layers, rail_waypoint_layer_meta,
    rail_waypoint_sprite_center, road_stop_build_layers, road_stop_ground_index, road_stop_seq_gfx,
    station_tile_class, station_type_from_m6, stop_kind_from_m6,
};

/// Especificación de dibujo de una casa (stage completado).
///
/// `s1` es el sprite de suelo/base del tile (0 = omitir, se usa grass).
/// `s2` es el sprite del edificio principal (0 = sin overlay).
pub struct HouseDrawSpec {
    pub s1: u32,
    pub s1_w: f32,
    pub s1_h: f32,
    pub s1_xrel: f32,
    pub s1_yrel: f32,
    pub s2: u32,
    pub s2_w: f32,
    pub s2_h: f32,
    pub s2_xrel: f32,
    pub s2_yrel: f32,
}

/// Tabla `_town_draw_tile_data` (`town_land.h`): **110** casas originales × **16** filas.
///
/// OpenTTD: `house_id * 16 + TileHash2Bit(x,y) * 4 + GetHouseBuildingStage()`.
/// Regenerar: `python3 scripts/gen_house_draw_data.py`.
///
/// `s1 = 0` → solo hierba base; sprites en `house_s{id}.png`.
pub use house_draw_data_generated::HOUSE_DRAW_DATA;

/// Árboles templados (`tree_land.h`): sprites, layout y metadatos NFO.
/// Regenerar: `python3 scripts/gen_tree_draw_data.py`.
pub use tree_draw_data_generated::{
    TREE_LAYOUT_SPRITE, TREE_LAYOUT_XY, TREE_SPRITE_COUNT, TREE_SPRITE_META,
};

/// Campos de cultivo y cercas (`clear_land.h`).
/// Regenerar: `python3 scripts/gen_field_draw_data.py`.
pub use field_draw_data_generated::{
    FENCE_MOD_BY_TILEH_NE, FENCE_MOD_BY_TILEH_NW, FENCE_MOD_BY_TILEH_SE, FENCE_MOD_BY_TILEH_SW,
    FENCE_SPRITE_META, FIELD_STATES,
};

pub use bridge_sprites_generated::{
    BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_sprite_meta,
};
/// Set completo de orillas (`SPR_SHORE_BASE + 0..17`, Action5 0x0D).
/// Regenerar: `python3 scripts/gen_shore_full_set.py`.
pub(crate) use bridge_structure_palette::{BridgePaletteSprites, bridge_structure_palette};
pub use shore_draw_data_generated::{SHORE_META, SHORE_SPRITE_COUNT, TILEH_TO_SHORE_SPRITE};
pub use tunnel::{
    tunnel_portal_translation, tunnel_rear_atlas_name, tunnel_rear_legacy_atlas_name,
    tunnel_rear_sprite_id,
};

/// Humo mina de cobre (`SPR_SMOKE_0..4`). Regenerar: `python3 scripts/gen_copper_mine_smoke.py`.
pub use copper_smoke_draw_data_generated::{COPPER_MINE_SMOKE_FRAMES, COPPER_MINE_SMOKE_META};
/// Humo de chimenea de la central eléctrica (`SPR_CHIMNEY_SMOKE_0..7`).
/// Regenerar: `python3 scripts/gen_chimney_smoke.py`.
pub use smoke_draw_data_generated::{CHIMNEY_SMOKE_FRAMES, CHIMNEY_SMOKE_META};

/// EffectVehicle tren / explosión. Regenerar: `python3 scripts/gen_effect_vehicle_sprites.py`.
pub use effect_vehicle_draw_data_generated::{
    BREAKDOWN_SMOKE_FRAMES, BREAKDOWN_SMOKE_META, DIESEL_SMOKE_FRAMES, DIESEL_SMOKE_META,
    ELECTRIC_SPARK_FRAMES, ELECTRIC_SPARK_META, EXPLOSION_LARGE_FRAMES, EXPLOSION_LARGE_META,
    STEAM_SMOKE_FRAMES, STEAM_SMOKE_META,
};

/// Devuelve el nombre de archivo (relativo a `assets/opengfx/tiles/`) para un sprite de casa.
/// Usa el naming genérico `house_s{id}.png` para todos los sprites extraídos.
pub fn house_sprite_filename(sprite_id: u32) -> String {
    format!("house_s{sprite_id}.png")
}

/// Etapa de obra para dibujo (`GetHouseBuildingStage` en `town_map.h`).
///
/// - `m3` bit 7 set (`IsHouseCompleted`): etapa **3** (terminado; `m5` guarda edad).
/// - Si no: bits 4..3 de `m5` (`GB(m5, 3, 2)`); bits 2..0 = contador de obra.
#[must_use]
pub fn house_building_stage_from_tile(m5: u8, m3: u8) -> usize {
    const TOWN_HOUSE_COMPLETED: usize = 3;
    if m3 & 0x80 != 0 {
        TOWN_HOUSE_COMPLETED
    } else {
        usize::from((m5 >> 3) & 0x3).min(TOWN_HOUSE_COMPLETED)
    }
}

/// HouseID ≥ este valor son casas NewGRF en OpenTTD (`house.h`).
pub const NEW_HOUSE_OFFSET: u16 = 110;

/// Casas originales con filas en `_town_draw_tile_data` (0..=109).
pub const ORIGINAL_HOUSE_COUNT: usize = NEW_HOUSE_OFFSET as usize;

/// HouseID efectivo para [`HOUSE_DRAW_DATA`] sin cargar NewGRF (sustituto `% 110`).
#[must_use]
pub fn house_id_for_draw_table(clean_house_id: u16) -> usize {
    let id = usize::from(clean_house_id & 0xFFF);
    if id >= ORIGINAL_HOUSE_COUNT {
        id % ORIGINAL_HOUSE_COUNT
    } else {
        id
    }
}

/// Índice en [`HOUSE_DRAW_DATA`] para una casa.
///
/// OpenTTD: `house_id * 16 + TileHash2Bit(x,y) * 4 + building_stage`.
#[must_use]
pub fn house_draw_data_index_for_tile(
    clean_house_id: u16,
    tx: i32,
    ty: i32,
    building_stage: usize,
) -> usize {
    const ROWS_PER_HOUSE: usize = 16;
    const MAX_STAGE: usize = 3;

    let house_id = house_id_for_draw_table(clean_house_id);
    let hash2 = tile_hash_2bit(tx, ty);
    let stage = building_stage.min(MAX_STAGE);
    house_id * ROWS_PER_HOUSE + hash2 * 4 + stage
}

/// `TileHash2Bit` de OpenTTD (`tile_map.h`) con coordenadas de mundo (×16):
/// `hash = (x>>4) ^ (x>>6) ^ (y>>4) - (y>>6)` y se toman los 2 bits bajos.
/// Con `x = 16·tx`: `hash = tx ^ (tx>>2) ^ ty − (ty>>2)`.
#[must_use]
pub fn tile_hash_2bit(tx: i32, ty: i32) -> usize {
    let (x, y) = (tx.cast_unsigned(), ty.cast_unsigned());
    let hash = (x ^ (x >> 2) ^ y).wrapping_sub(y >> 2);
    (hash & 3) as usize
}

/// `RoadTileType::Crossing` en bits 6–7 de `m5` (`road_map.h`).
#[must_use]
pub fn is_road_level_crossing(mapt: u8, m5: u8, kind: TileKind) -> bool {
    rail::is_road_level_crossing(mapt, m5, kind, OTTD_MP_ROAD)
}

/// Tranvía presente en la tesela de carretera (`GetRoadTypeTram` ≠ inválido; 6 bits altos de `m8`).
#[must_use]
pub fn road_tile_has_tram_track(m8: u16) -> bool {
    road::road_tile_has_tram_track(m8)
}

// ── Lógica de road bits ─────────────────────────────────────────────────────

/// Decodifica los road bits efectivos desde m5 según el tipo de tesela.
///
/// Los bits del savegame ya están en la orientación correcta de OpenTTD:
/// - NW (bit 0) = conexión hacia (x, y-1)  → visualmente arriba-izquierda
/// - SW (bit 1) = conexión hacia (x+1, y)  → visualmente abajo-izquierda
/// - SE (bit 2) = conexión hacia (x, y+1)  → visualmente abajo-derecha
/// - NE (bit 3) = conexión hacia (x-1, y)  → visualmente arriba-derecha
#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/road.rs.
pub fn effective_road_bits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    road::effective_road_bits(mapt, m5, kind, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE)
}

#[inline]
#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/road.rs.
pub fn road_flat_index(road_bits: u8) -> usize {
    road::road_flat_index(road_bits, &ROAD_FLAT_OFFSET_TBL)
}

/// Índice del PNG `road_flat_{idx:02}.png` (0–18), alineado con `GetRoadSpriteOffset` en
/// `road_cmd.cpp` de OpenTTD: en las cuatro pendientes “de borde” (dos esquinas contiguas
/// elevadas) el desplazamiento respecto a `SPR_ROAD_Y` (1332) es **11–14**; en terreno
/// plano se usa la tabla de cruces (`road_flat_index`).
///
/// Bitmask `tileh` igual que `Slope` (sin bit `STEEP`): `SLOPE_NE`=12, `SE`=6, `SW`=3, `NW`=9.
#[must_use]
pub fn road_flat_sprite_index(tileh: u8, road_bits: u8) -> usize {
    road::road_flat_sprite_index(tileh, road_bits, &ROAD_FLAT_OFFSET_TBL)
}

/// Bits de tranvía desde M3LO (0–3), alineados con `GetRoadBits` / track bits en OpenTTD.
#[inline]
#[must_use]
pub fn tram_track_bits_m3(m3: u8) -> u8 {
    road::tram_track_bits_m3(m3)
}

/// Índice `tram_flat_{idx:02}` (0–18); `None` si no hay trazado en `m3`.
#[must_use]
pub fn tram_flat_sprite_index(tileh: u8, m3: u8) -> Option<usize> {
    road::tram_flat_sprite_index(tileh, m3, &ROAD_FLAT_OFFSET_TBL)
}

/// Tinte de cruce o overlay cuando hay tranvía por tipo (`m8`) o por geometría (`m3`).
#[inline]
#[must_use]
pub fn road_tile_tram_visual_active(m3: u8, m8: u16) -> bool {
    tram_track_bits_m3(m3) != 0 || road_tile_has_tram_track(m8)
}

/// Road bits para dibujar: `m5` / vecinos (mapa procedural).
///
/// Asignación de bits conforme a OpenTTD (con iso correcta):
/// - NE (bit 3 = 8): vecino en (x-1, y) → arriba-derecha en pantalla
/// - NW (bit 0 = 1): vecino en (x, y-1) → arriba-izquierda en pantalla
/// - SW (bit 1 = 2): vecino en (x+1, y) → abajo-izquierda en pantalla
/// - SE (bit 2 = 4): vecino en (x, y+1) → abajo-derecha en pantalla
pub fn road_bits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    road::road_bits_for_render(map, pos, mw, mh, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE)
}

// ── Lógica de rail bits ─────────────────────────────────────────────────────

#[allow(dead_code)] // Wrapper de compatibilidad pública tras split a sprites/rail.rs.
pub fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    rail::effective_rail_trackbits(mapt, m5, kind, OTTD_MP_RAIL)
}

pub fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    rail::rail_trackbits_for_render(map, pos, mw, mh, OTTD_MP_RAIL)
}

#[cfg(test)]
mod house_draw_index_tests {
    use super::{
        HOUSE_DRAW_DATA, ORIGINAL_HOUSE_COUNT, house_building_stage_from_tile,
        house_draw_data_index_for_tile, tile_hash_2bit,
    };

    #[test]
    fn tile_hash_2bit_matches_openttd_tilehash() {
        // TileHash(16·tx, 16·ty) & 3 calculado a mano sobre la fórmula oficial.
        assert_eq!(tile_hash_2bit(0, 0), 0);
        assert_eq!(tile_hash_2bit(1, 0), 1);
        assert_eq!(tile_hash_2bit(5, 2), 2); // (5 ^ 1 ^ 2) − 0 = 6 → 2
        assert_eq!(tile_hash_2bit(7, 11), 3); // (7 ^ 1 ^ 11) − 2 = 11 → 3
    }

    #[test]
    fn house_type_zero_uses_finished_stage_row() {
        let h = tile_hash_2bit(0, 0);
        assert_eq!(house_draw_data_index_for_tile(0, 0, 0, 3), h * 4 + 3);
    }

    #[test]
    fn house_id_sixteen_offsets_by_sixteen_times_sixteen() {
        let h = tile_hash_2bit(5, 2);
        assert_eq!(
            house_draw_data_index_for_tile(16, 5, 2, 3),
            16 * 16 + h * 4 + 3
        );
    }

    #[test]
    fn newgrf_house_id_uses_modulo_fallback() {
        let h = tile_hash_2bit(1, 1);
        assert_eq!(
            house_draw_data_index_for_tile(128, 1, 1, 3),
            house_draw_data_index_for_tile(18, 1, 1, 3)
        );
        assert_eq!(128 % ORIGINAL_HOUSE_COUNT, 18);
        assert_eq!(
            house_draw_data_index_for_tile(128, 1, 1, 3),
            18 * 16 + h * 4 + 3
        );
    }

    #[test]
    fn stadium_draw_row_uses_stadium_ground_sprite() {
        let spec = &HOUSE_DRAW_DATA[320];
        assert_eq!(spec.s1, 1479, "HouseID 20 fila 0: suelo SPR_GRND_STADIUM_N");
        assert_eq!(spec.s2, 0);
    }

    #[test]
    fn park_row_may_use_grass_only_without_building_overlay() {
        // Parques / suelo-only: s1=0 (hierba) y s2=0 es intencional en town_land.h, no un bug.
        let spec = &HOUSE_DRAW_DATA[144];
        assert_eq!(spec.s1, 0);
        assert_eq!(spec.s2, 0);
    }

    #[test]
    fn building_stages_shift_row_within_house_band() {
        let tx = 7;
        let ty = 11;
        let h = tile_hash_2bit(tx, ty);
        let base = h * 4;
        for stage in 0..=3 {
            assert_eq!(
                house_draw_data_index_for_tile(0, tx, ty, stage),
                base + stage
            );
        }
    }

    #[test]
    fn house_building_stage_completed_via_m3_bit7() {
        assert_eq!(house_building_stage_from_tile(0, 0x80), 3);
        assert_eq!(house_building_stage_from_tile(200, 0x80), 3);
    }

    #[test]
    fn house_building_stage_under_construction_from_m5() {
        assert_eq!(house_building_stage_from_tile(0x00, 0), 0);
        assert_eq!(house_building_stage_from_tile(0x08, 0), 1);
        assert_eq!(house_building_stage_from_tile(0x10, 0), 2);
        assert_eq!(house_building_stage_from_tile(0x18, 0), 3);
    }
}

#[cfg(test)]
mod road_sprite_index_tests {
    use super::{road_flat_index, road_flat_sprite_index};

    #[test]
    fn flat_tile_uses_road_bits_table() {
        assert_eq!(road_flat_sprite_index(0, 0x05), road_flat_index(0x05));
    }

    #[test]
    fn full_road_diagonal_slopes_use_openttd_offsets_11_to_14() {
        assert_eq!(road_flat_sprite_index(12, 0x0F), 11); // SLOPE_NE
        assert_eq!(road_flat_sprite_index(6, 0x0F), 12); // SLOPE_SE
        assert_eq!(road_flat_sprite_index(3, 0x0F), 13); // SLOPE_SW
        assert_eq!(road_flat_sprite_index(9, 0x0F), 14); // SLOPE_NW
    }

    #[test]
    fn non_diagonal_slopes_use_flat_road_variant_from_bits() {
        let bits = 0x0A;
        assert_eq!(road_flat_sprite_index(1, bits), road_flat_index(bits)); // SLOPE_W
        assert_eq!(road_flat_sprite_index(12, bits), 11); // SLOPE_NE: upstream ignora bits
    }

    #[test]
    fn flat_road_bits_1_to_15_match_openttd_table() {
        let expected: [(u8, usize); 15] = [
            (0x01, 18),
            (0x02, 17),
            (0x03, 7),
            (0x04, 16),
            (0x05, 0),
            (0x06, 10),
            (0x07, 5),
            (0x08, 15),
            (0x09, 8),
            (0x0A, 1),
            (0x0B, 4),
            (0x0C, 9),
            (0x0D, 3),
            (0x0E, 6),
            (0x0F, 2),
        ];
        for (bits, idx) in expected {
            assert_eq!(road_flat_sprite_index(0, bits), idx, "bits 0x{bits:02X}");
        }
    }
}

#[cfg(test)]
mod tram_road_overlay_tests {
    use super::{road_flat_sprite_index, tram_flat_sprite_index, tram_track_bits_m3};

    #[test]
    fn tram_bits_zero_means_no_overlay_index() {
        assert_eq!(tram_track_bits_m3(0xF0), 0);
        assert!(tram_flat_sprite_index(0, 0).is_none());
        assert!(tram_flat_sprite_index(0, 0xF0).is_none());
    }

    #[test]
    fn tram_overlay_reuses_road_flat_table_for_same_track_mask() {
        assert_eq!(
            tram_flat_sprite_index(0, 0x05),
            Some(road_flat_sprite_index(0, 0x05))
        );
        assert_eq!(
            tram_flat_sprite_index(12, 0x0A),
            Some(road_flat_sprite_index(12, 0x0A))
        );
    }
}

#[cfg(test)]
mod level_crossing_tests {
    use super::{
        OTTD_MP_ROAD, RAIL_TILE_SIGNALS, is_road_level_crossing, level_crossing_rail_sprite_id,
        rail_tile_is_signals,
    };
    use openttdrs_core::TileKind;

    #[test]
    fn crossing_detected_on_mp_road_subtype_1() {
        let mapt = OTTD_MP_ROAD << 4;
        let m5 = 0x40; // Crossing << 6, road axis 0
        assert!(is_road_level_crossing(mapt, m5, TileKind::Road));
        assert!(!is_road_level_crossing(mapt, 0, TileKind::Road));
    }

    #[test]
    fn crossing_rail_sprite_alternates_with_road_axis() {
        assert_eq!(level_crossing_rail_sprite_id(0x40), 1371);
        assert_eq!(level_crossing_rail_sprite_id(0x41), 1370);
    }

    #[test]
    fn crossing_rail_sprite_adds_two_when_barred() {
        assert_eq!(level_crossing_rail_sprite_id(0x40 | 0x20), 1373);
        assert_eq!(level_crossing_rail_sprite_id(0x41 | 0x20), 1372);
    }

    #[test]
    fn signals_subtype_is_bit_pattern() {
        assert!(rail_tile_is_signals(0x01 | (RAIL_TILE_SIGNALS << 6)));
        assert!(!rail_tile_is_signals(0x01));
    }
}

#[cfg(test)]
mod signal_sprite_collect_tests {
    use super::{RAIL_TB_Y, RAIL_TILE_SIGNALS, collect_signal_sprite_ids, rail_tile_is_signals};

    #[test]
    fn semaphore_variant_changes_sprite_id() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_Y;
        let m3 = 0xC0;
        let m3hi = 0;
        let ids_e = collect_signal_sprite_ids(0, m3, m3hi, m5);
        // variant 1 en bit 3 del byte bajo de m2 para pistas que leen variante en bit 3 (TRACK_X/Y)
        let ids_s = collect_signal_sprite_ids(0x08, m3, m3hi, m5);
        assert_eq!(ids_e.len(), 2);
        assert_eq!(ids_s.len(), 2);
        assert_ne!(ids_e[0], ids_s[0]);
    }

    #[test]
    fn rail_preload_includes_crossings_snow_and_signals_bounded() {
        use super::{rail_sprite_ids_for_preload, signal_sprite_ids_for_preload};
        let ids = rail_sprite_ids_for_preload();
        assert!(!ids.is_empty());
        assert!(ids.contains(&1372));
        assert!(ids.contains(&1037));
        assert!(ids.contains(&1038));
        assert!(ids.contains(&1279));
        // IDs virtuales de catenaria Action5 (≥900_000) no cuentan para el techo OpenGFX.
        let mx = ids
            .iter()
            .copied()
            .filter(|id| *id < 900_000)
            .max()
            .unwrap_or(0);
        assert!(
            mx < 1700,
            "máx sprite id {mx}: ampliar range(1275,…) en descargar_graficos.sh si hace falta"
        );
        assert!(ids.contains(&super::PYLON_SPRITE_BASE));
        assert!(ids.contains(&super::CATENARY_ENTRANCE_SPRITE_BASE));
        let placeholders = [1438_u32, 1439, 1530, 1532, 1540, 1542, 1546, 1548];
        for pid in placeholders {
            assert!(
                !ids.contains(&pid),
                "preload no debe pedir placeholder {pid}"
            );
        }
        for sid in signal_sprite_ids_for_preload() {
            if super::rail::SIGNAL_SPRITE_OPENGFX_GAPS.contains(&sid) {
                continue;
            }
            assert!(ids.contains(&sid), "falta señal {sid} en preload");
        }
    }

    #[test]
    fn y_track_two_present_signals_two_sprites() {
        let m5 = (RAIL_TILE_SIGNALS << 6) | RAIL_TB_Y;
        assert!(rail_tile_is_signals(m5));
        let m3 = 0xC0; // bits 2,3 en nibble alto → presentes
        let m3hi = 0x80; // bit 3 del nibble alto de estados → verde en señal 3
        let ids = collect_signal_sprite_ids(0, m3, m3hi, m5);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn opengfx_default_signal_alt_is_seventy_seven_above_classic() {
        // Ancla documentada: bloque extendido de señales en `ogfx1_base.grf` sigue al rango 1275…
        assert_eq!(1352_u32 - 1275_u32, 77);
    }
}
