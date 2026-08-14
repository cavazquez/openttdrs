//! Constantes y lógica de sprites de `OpenGFX`.

use bevy::prelude::Color;
use openttdrs_core::prelude::*;

#[path = "sprites/airport_station_draw_data_generated.rs"]
mod airport_station_draw_data_generated;
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
#[path = "sprites/house_palette.rs"]
pub(crate) mod house_palette;
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
#[path = "sprites/signal_sprite_meta_generated.rs"]
mod signal_sprite_meta_generated;
#[path = "sprites/smoke_draw_data_generated.rs"]
mod smoke_draw_data_generated;
#[path = "sprites/station.rs"]
pub(crate) mod station;
#[path = "sprites/tile_atlas_generated.rs"]
mod tile_atlas_generated;
#[path = "sprites/track_fence.rs"]
mod track_fence;
#[path = "sprites/track_fence_meta_generated.rs"]
mod track_fence_meta_generated;
#[path = "sprites/transparency.rs"]
mod transparency;
#[path = "sprites/tree_draw_data_generated.rs"]
mod tree_draw_data_generated;
#[path = "sprites/tunnel.rs"]
mod tunnel;
#[path = "sprites/water_palette_generated.rs"]
mod water_palette_generated;

pub(crate) use tile_atlas_generated::{
    TILE_ATLAS_NAMES, TILE_ATLAS_PAGE_COUNT, TILE_ATLAS_PAGE_RANGES, TILE_ATLAS_PAGE_SIZES,
    TILE_ATLAS_RECTS,
};
pub(crate) use water_palette_generated::{
    DARK_WATER_FRAME_COUNT, GLITTER_WATER_FRAME_COUNT, WATER_PALETTE_FRAME_COUNT,
};

// ── Constantes de renderizado de carreteras y vías ───────────────────────────

/// Tipos de tesela `OpenTTD` (nibble alto del byte MAPT).
pub use openttdrs_core::{OTTD_MP_RAILWAY as OTTD_MP_RAIL, OTTD_MP_ROAD, OTTD_MP_TUNNELBRIDGE};

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

pub use road::{
    ROAD_DEPOT_GROUND_PATH, ROAD_FLAT_OFFSET_TBL, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS,
    ROADSIDE_TREE_META, ROADSIDE_TREES, SPR_ROADSIDE_TREE, road_depot_build_layers,
    road_depot_entrance_road_bits, road_depot_seq_gfx, road_ground_sprite_id,
    road_streetlight_sprite_id, road_tile_roadside, roadside_is_paved,
};
pub(crate) use track_fence::{
    track_fence_draws_for_tile, track_fence_height_px, track_fence_sprite_meta,
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
    CATENARY_ENTRANCE_SPRITE_BASE, CatenarySpriteDraw, CatenaryWireDraw, MAGLEV_RAIL_SPRITE_OFFSET,
    MONO_RAIL_SPRITE_OFFSET, PYLON_SPRITE_BASE, RAIL_DEPOT_GROUND_TRACK,
    RAIL_DEPOT_VISUAL_TYPE_COUNT, RAIL_SPRITE_IDS, RAIL_SPRITE_TRACK_X, RAIL_SPRITE_TRACK_Y,
    RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y, RAIL_TILE_DEPOT, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS,
    SignalSpriteDraw, WIRE_SPRITE_BASE, WIRE_SPRITE_LAST, catenary_pylon_sprite_ids,
    catenary_pylon_world_z_delta, catenary_reference_sprite_id, catenary_tile_location_group,
    catenary_tileh_selector, catenary_tunnel_wire_sprite, catenary_wire_sprite_ids,
    catenary_wire_world_z_delta, collect_catenary_bridge_draws, collect_catenary_pylons_from_map,
    collect_catenary_pylons_from_map_with_pcp_override, collect_catenary_sprites,
    collect_catenary_sprites_from_map, collect_catenary_wire_draws_from_map,
    collect_rail_ghost_sprites, collect_rail_ghost_sprites_for_type, collect_rail_sprites,
    collect_rail_sprites_for_type, collect_signal_sprite_draws, collect_signal_sprite_ids,
    is_typed_rail_track_sprite, level_crossing_ground_sprite_id_for_type,
    level_crossing_has_rail_reservation, level_crossing_rail_sprite_id,
    level_crossing_rail_sprite_id_for_type, rail_depot_build_layers, rail_depot_seq_gfx,
    rail_depot_visual_type_index, rail_ghost_overlay_offset, rail_pbs_reservation_offset,
    rail_pbs_sprite_ids_for_preload, rail_signal_present_mask, rail_signal_state_mask,
    rail_signal_subtile_offset, rail_signal_subtile_offset_for_side, rail_sprite_atlas_keys,
    rail_sprite_ids_for_preload, rail_tile_has_pbs_reservation, rail_tile_is_signals,
    remap_rail_sprite_id, signal_draw_pos, signal_safe_slope_position_for_side,
    signal_screen_anchor_for_side, signal_screen_position, signal_screen_position_for_side,
    signal_sprite_bases, signal_sprite_center_offset, signal_sprite_ids_for_preload,
    signal_sprite_metadata, signal_sprite_texture_id, signal_world_position_for_side,
};
pub(crate) use rail::{collect_rail_pbs_reservation_draws, collect_rail_sprites_for_surface};
#[allow(unused_imports)]
pub use station::{
    RailStationLayer, RoadStopLayerGfx, StationTileClass, log_unknown_station_type_once,
    rail_station_axis_y, rail_station_draw_layers, rail_station_ground_track_sprite,
    rail_station_ground_track_sprite_for_type, rail_station_layer_bounds,
    rail_station_layer_for_type, rail_station_overlay_rel, rail_station_roof_glass_sprite,
    rail_station_sprite_base_id, rail_station_sprite_id_for_type, rail_station_sprite_layers,
    rail_station_sprite_meta, rail_waypoint_draw_layers, rail_waypoint_layer_meta,
    rail_waypoint_sprite_center, road_stop_build_layers, road_stop_drive_through_layers,
    road_stop_ground_index, road_stop_ground_sprite_id, road_stop_seq_gfx, station_tile_class,
    station_type_from_m6, stop_kind_from_m6,
};
#[allow(unused_imports)]
pub use transparency::{
    TRANSPARENT_ALPHA, TransparencyMode, TransparencyOption, apply_mode_to_bits, catenary_hidden,
    catenary_sprite_color, is_hidden, mode_from_bits, set_transparency_preferences, sprite_color,
    text_color, with_to_alpha,
};

/// Especificación de dibujo de una casa (stage completado).
///
/// `s1` es el sprite de suelo/base que OpenTTD pasa a `DrawGroundSprite`.
/// No debe confundirse con un overlay ni sustituirse por césped: 3924 es
/// `SPR_FLAT_BARE_LAND` y 3981 es `SPR_FLAT_GRASS_TILE`.
/// `s1_palette` y `s2_palette` son las `PaletteID` de la entrada `M(...)`.
/// Los PNG de casas comparten rampas de color, por lo que descartar estas
/// paletas hace que bloques completos aparezcan con el tono equivocado.
/// `s2` es el sprite del edificio principal (0 = sin overlay).
/// `draw_proc` es el último campo `p` de `M(...)` en `town_land.h` (`1` = ascensor).
pub struct HouseDrawSpec {
    pub s1: u32,
    pub s1_palette: u32,
    pub s1_w: f32,
    pub s1_h: f32,
    pub s1_xrel: f32,
    pub s1_yrel: f32,
    pub s2: u32,
    pub s2_palette: u32,
    pub s2_w: f32,
    pub s2_h: f32,
    pub s2_xrel: f32,
    pub s2_yrel: f32,
    pub draw_proc: u8,
}

/// Tabla `_town_draw_tile_data` (`town_land.h`): **110** casas originales × **16** filas.
///
/// OpenTTD: `house_id * 16 + TileHash2Bit(x,y) * 4 + GetHouseBuildingStage()`.
/// Regenerar: `python3 scripts/gen_house_draw_data.py`.
///
/// Los sprites de ambas capas viven en `house_s{id}.png`; los dos suelos
/// comunes 3924/3981 se resuelven como aliases de `terrain_bare`/`grass`.
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

#[allow(unused_imports)]
pub use airport_station_draw_data_generated::{
    AIRPORT_STATION_SPRITES, AirportStationBase, AirportStationLayer, AirportStationSprite,
    airport_station_base_for_gfx, airport_station_ground_layers_for_gfx,
    airport_station_layers_for_gfx, airport_station_sprite_for_id,
};
pub use bridge_sprites_generated::{
    BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_ramp_sprite_id, bridge_sprite_meta,
};
/// Set completo de orillas (`SPR_SHORE_BASE + 0..17`, Action5 0x0D).
/// Regenerar: `python3 scripts/gen_shore_full_set.py`.
pub(crate) use bridge_structure_palette::{
    BridgePaletteSprites, bridge_structure_palette_for_sprite,
};
pub(crate) use house_palette::HousePaletteSprites;

/// Variante para una capa aeroportuaria animada.
///
/// El `TILE_SEQ_LINE` conserva su caja lógica, pero los frames de radar y
/// bandera tienen offsets NFO distintos. OpenTTD cambia la tabla completa por
/// frame; reutilizar el offset del frame 0 desplaza visualmente las aspas.
#[must_use]
pub fn airport_station_overlay_rel_for_sprite(
    layer: &AirportStationLayer,
    sprite: &AirportStationSprite,
) -> (f32, f32) {
    let off = crate::iso::remap_tile_offset(layer.dx, layer.dy, layer.dz) * 0.5;
    (off.x + sprite.x_offs, sprite.y_offs - off.y)
}
pub use shore_draw_data_generated::{SHORE_META, SHORE_SPRITE_COUNT, TILEH_TO_SHORE_SPRITE};
pub use tunnel::{
    rail_tunnel_front_atlas_name, rail_tunnel_front_sprite_id, rail_tunnel_rear_atlas_name,
    rail_tunnel_rear_sprite_id, tunnel_catenary_translation, tunnel_front_atlas_name,
    tunnel_front_sprite_id, tunnel_front_trace_geometry, tunnel_front_translation,
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
    BREAKDOWN_SMOKE_FRAMES, BREAKDOWN_SMOKE_META, BUBBLE_FRAMES, BUBBLE_META, DIESEL_SMOKE_FRAMES,
    DIESEL_SMOKE_META, ELECTRIC_SPARK_FRAMES, ELECTRIC_SPARK_META, EXPLOSION_LARGE_FRAMES,
    EXPLOSION_LARGE_META, STEAM_SMOKE_FRAMES, STEAM_SMOKE_META,
};

/// Devuelve el nombre de archivo (relativo a `assets/opengfx/tiles/`) para un sprite de casa.
/// Usa el naming genérico `house_s{id}.png` para todos los sprites extraídos.
pub fn house_sprite_filename(sprite_id: u32) -> String {
    format!("house_s{sprite_id}.png")
}

/// Nombre canónico en el atlas para un sprite de una entrada de casa.
///
/// Los dos sprites de suelo frecuentes no son archivos ``house_s*``: se
/// comparten con el terreno general. Centralizar esta excepción evita que el
/// atlas y las copias recoloreadas busquen archivos diferentes.
#[must_use]
pub(crate) fn house_sprite_asset_filename(sprite_id: u32) -> String {
    match sprite_id {
        3924 => "terrain_bare.png".to_owned(),
        3981 => "grass.png".to_owned(),
        _ => house_sprite_filename(sprite_id),
    }
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

/// HouseID efectivo para [`HOUSE_DRAW_DATA`] (fallback `% 110` sin catálogo).
#[must_use]
#[allow(dead_code)] // Wrapper sin catálogo; el render usa `*_with_catalog`.
pub fn house_id_for_draw_table(clean_house_id: u16) -> usize {
    house_id_for_draw_table_with_catalog(clean_house_id, &[])
}

/// Resuelve id de dibujo vía `resolve_house_draw_id` (vistas / subst / `% 110`).
#[must_use]
pub fn house_id_for_draw_table_with_catalog(
    clean_house_id: u16,
    catalog: &[openttdrs_core::HouseSpecDef],
) -> usize {
    let draw = openttdrs_core::resolve_house_draw_id(clean_house_id, catalog);
    let id = usize::from(draw & 0xFFF);
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
#[allow(dead_code)] // Wrapper sin catálogo; el render usa `*_with_catalog`.
pub fn house_draw_data_index_for_tile(
    clean_house_id: u16,
    tx: i32,
    ty: i32,
    building_stage: usize,
) -> usize {
    house_draw_data_index_for_tile_with_catalog(clean_house_id, tx, ty, building_stage, &[])
}

/// Índice en [`HOUSE_DRAW_DATA`] con catálogo NewGRF (subst / `% 110`).
#[must_use]
pub fn house_draw_data_index_for_tile_with_catalog(
    clean_house_id: u16,
    tx: i32,
    ty: i32,
    building_stage: usize,
    catalog: &[openttdrs_core::HouseSpecDef],
) -> usize {
    const ROWS_PER_HOUSE: usize = 16;
    const MAX_STAGE: usize = 3;

    let house_id = house_id_for_draw_table_with_catalog(clean_house_id, catalog);
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

/// Índice `tram_flat_{idx:02}` (0–18); `None` si no hay trazado en `m3`.
#[must_use]
pub fn tram_flat_sprite_index(tileh: u8, m3: u8) -> Option<usize> {
    road::tram_flat_sprite_index(tileh, m3, &ROAD_FLAT_OFFSET_TBL)
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
    fn park_row_keeps_its_bare_ground_without_building_overlay() {
        // Parques / suelo-only: `s2=0` es intencional, pero `s1` sigue siendo
        // el suelo real (`SPR_FLAT_BARE_LAND`), no una señal para reemplazarlo
        // por césped en el cliente.
        let spec = &HOUSE_DRAW_DATA[144];
        assert_eq!(spec.s1, 3924);
        assert_eq!(spec.s2, 0);
    }

    #[test]
    fn first_house_row_keeps_openttd_bare_ground_sprite() {
        assert_eq!(HOUSE_DRAW_DATA[0].s1, 3924);
        assert_eq!(HOUSE_DRAW_DATA[0].s2, 1421);
    }

    #[test]
    fn house_draw_rows_keep_both_openttd_palette_fields() {
        // `town_land.h` filas 36 y 40: el mismo `0x58d` se dibuja blanco y
        // concreto según `p2`. Si el generador vuelve a descartar p1/p2, la
        // traza puede coincidir en sprite ID pero la ciudad ya no coincide
        // visualmente con OpenTTD.
        assert_eq!(HOUSE_DRAW_DATA[8].s2, 1421);
        assert_eq!(HOUSE_DRAW_DATA[8].s2_palette, 797);
        assert_eq!(HOUSE_DRAW_DATA[12].s2, 1421);
        assert_eq!(HOUSE_DRAW_DATA[12].s2_palette, 800);
        // Una entrada posterior aplica la paleta a ambas capas (`p1` y
        // `p2`), no sólo al edificio sortable.
        assert_eq!(HOUSE_DRAW_DATA[469].s1_palette, 797);
        assert_eq!(HOUSE_DRAW_DATA[469].s2_palette, 797);
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
    use super::{road, road_flat_sprite_index, tram_flat_sprite_index};

    #[test]
    fn tram_bits_zero_means_no_overlay_index() {
        assert_eq!(road::tram_track_bits_m3(0xF0), 0);
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
mod airport_station_draw_tests {
    use super::{
        airport_station_base_for_gfx, airport_station_ground_layers_for_gfx,
        airport_station_layers_for_gfx, airport_station_overlay_rel_for_sprite,
        airport_station_sprite_for_id,
    };

    #[test]
    #[allow(clippy::expect_used)] // La tabla debe contener el sprite referenciado por su capa.
    fn airport_pier_tile_seq_matches_openttd_station_land_contract() {
        // `station_land.h`: APT_PIER_NW_NE usa (3, 2, 0, 3, 3, 14, 2661)
        // y APT_PIER usa (0, 8, 0, 14, 3, 14, 2662). Estos son los bounds
        // que OpenTTD entrega a AddSortableSpriteToDraw.
        let jetway = airport_station_layers_for_gfx(27);
        assert_eq!(jetway.len(), 1);
        assert_eq!(jetway[0].sprite_id, 2661);
        assert_eq!((jetway[0].dx, jetway[0].dy, jetway[0].dz), (3.0, 2.0, 0.0));
        assert_eq!((jetway[0].sx, jetway[0].sy, jetway[0].sz), (3, 3, 14));

        let tunnel = airport_station_layers_for_gfx(28);
        assert_eq!(tunnel.len(), 1);
        assert_eq!(tunnel[0].sprite_id, 2662);
        assert_eq!((tunnel[0].dx, tunnel[0].dy, tunnel[0].dz), (0.0, 8.0, 0.0));
        assert_eq!((tunnel[0].sx, tunnel[0].sy, tunnel[0].sz), (14, 3, 14));
        // RemapCoords × 0.5 para dy=8: (+16, -8); NFO = (-29, -10).
        // El resultado no puede volver a ser el centro del tile.
        let tunnel_sprite = airport_station_sprite_for_id(tunnel[0].sprite_id).expect("sprite");
        assert_eq!(
            airport_station_overlay_rel_for_sprite(&tunnel[0], tunnel_sprite),
            (-13.0, -2.0)
        );
        let jetway_2 = airport_station_layers_for_gfx(26);
        assert_eq!(jetway_2.len(), 1);
        assert_eq!(jetway_2[0].sprite_id, 2660);
        assert!(airport_station_layers_for_gfx(29).is_empty());
    }

    #[test]
    fn airport_apron_fences_keep_the_ground_sequence_from_openttd() {
        // `station_land.h`: APT_APRON_FENCE_NW usa FENCE_X en (0,0),
        // APT_APRON_FENCE_SW usa FENCE_Y en (15,0). Ambas capas pertenecen
        // a DrawGroundSpriteAt, no a la pila sortable del terminal.
        assert_eq!(
            airport_station_base_for_gfx(1).map(|base| base.sprite_id),
            Some(2634)
        );
        assert_eq!(
            airport_station_base_for_gfx(2).map(|base| base.sprite_id),
            Some(2634)
        );
        assert_eq!(
            airport_station_base_for_gfx(27).map(|base| base.sprite_id),
            Some(2634)
        );

        let north_west = airport_station_ground_layers_for_gfx(1);
        assert_eq!(north_west.len(), 1);
        assert_eq!(north_west[0].sprite_id, 2664);
        assert_eq!((north_west[0].dx, north_west[0].dy), (0.0, 0.0));

        let south_west = airport_station_ground_layers_for_gfx(2);
        assert_eq!(south_west.len(), 1);
        assert_eq!(south_west[0].sprite_id, 2663);
        assert_eq!((south_west[0].dx, south_west[0].dy), (15.0, 0.0));

        let dual = airport_station_ground_layers_for_gfx(56);
        assert_eq!(dual.len(), 2);
        assert_eq!(dual[0].sprite_id, 2663);
        assert_eq!(dual[1].sprite_id, 2663);
        assert_eq!((dual[1].dx, dual[1].dy), (15.0, 0.0));
    }

    #[test]
    fn airport_table_covers_every_vanilla_station_gfx_and_dynamic_frames() {
        for gfx in 0..=73 {
            assert!(airport_station_base_for_gfx(gfx).is_some(), "gfx={gfx}");
        }
        assert_eq!(airport_station_layers_for_gfx(44)[0].sprite_id, 2633);
        assert_eq!(airport_station_layers_for_gfx(47)[0].sprite_id, 2651);
        assert_eq!(airport_station_layers_for_gfx(71)[0].sprite_id, 5968);
        assert_eq!(airport_station_layers_for_gfx(72)[0].sprite_id, 5967);
        for sprite_id in 2676..=2691 {
            assert!(
                airport_station_sprite_for_id(sprite_id).is_some(),
                "sprite={sprite_id}"
            );
        }
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
        let ids_e = collect_signal_sprite_ids(0, m3, m3hi, m5); // SIG_ELECTRIC
        // SIG_SEMAPHORE: bit 3 de m2 en pistas X/Y
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
            mx <= 5412,
            "máx sprite id {mx}: señales Action5 (~5327) y PBS de puente terminan en 5412"
        );
        assert!(
            ids.iter().any(|&id| (5088..5328).contains(&id)),
            "preload debe incluir señales Action5 (rail_5088..)"
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
    fn opengfx_default_signal_alt_is_signals_base_minus_sixteen() {
        // `DrawSingleSignal`: SPR_SIGNALS_BASE (5088) - 16; clásico eléctrico en 1275.
        assert_eq!(5072_u32, 5088_u32 - 16);
        assert_eq!(1275_u32, super::rail::signal_sprite_bases().0);
        assert_eq!(5072_u32, super::rail::signal_sprite_bases().1);
    }
}
