//! Decode mínimo de sprites reales `NewGRF` + Action1/2/3 (trains / roadtypes, preview).
//!
//! Contenedor **v1** (inline) o **v2** (sprite section + `0xFD`).
//! 8bpp/32bpp plano / LZ77 / chunked; multi-zoom; máscara company-colour.
//! Action3→Action2 (básico / variational+advanced+7E/7C / random 80/83/84)→Action1.

mod action2;
mod action5;
mod action_graph;
pub mod fixture;
mod model;
mod pixel_codec;

// Re-exportar todos los tipos públicos del módulo model
pub use model::{
    Action2EvalCtx, Action2RandomEntry, Action2RealEntry, Action2VarAdjust, Action2VarEntry,
    Action2VarOp, Action2VarTerm, Action5Block, CALLBACK_FAILED, CBID_CARGO_PROFIT_CALC,
    CBID_CARGO_STATION_RATING_CALC, CBID_HOUSE_ALLOW_CONSTRUCTION, CBID_INDTILE_ANIM_NEXT_FRAME,
    CBID_INDTILE_ANIMATION_NEXT_FRAME, CBID_INDTILE_ANIMATION_SPEED,
    CBID_INDTILE_ANIMATION_TRIGGER, CBID_INDUSTRY_LOCATION, CBID_OBJECT_LAND_SLOPE_CHECK,
    CBID_STATION_ANIMATION_NEXT_FRAME, CBID_STATION_ANIMATION_SPEED,
    CBID_STATION_ANIMATION_TRIGGER, CBID_STATION_AVAILABILITY, CBID_STATION_BUILD_TILE_LAYOUT,
    CBID_STATION_DRAW_TILE_LAYOUT, CBID_STATION_LAND_SLOPE_CHECK, CBID_VEHICLE_LENGTH,
    CBID_VEHICLE_LOAD_AMOUNT, CBID_VEHICLE_REFIT_CAPACITY, CBID_VEHICLE_SOUND_EFFECT,
    CBID_VEHICLE_START_STOP_CHECK, CBID_VEHICLE_VISUAL_EFFECT, DecodedSprite, TrainSpriteAssign,
    TrainSpriteGraphics,
};

// Re-exportar funciones de runtime de pixel_codec
pub use pixel_codec::{
    SPRITE_V2_ZOOM_PREFERENCE, apply_company_colour_mask, bake_sprite_company_mask,
    decode_chunked_8bpp, decode_chunked_pixels, decode_real_sprite_v1,
    decode_real_sprite_v1_uncompressed, decode_real_sprite_v2_section,
    decode_real_sprite_v2_section_zoom, decompress_grf_lz77, encode_chunked_8bpp_full_rows,
    encode_chunked_pixels_full_rows, index_sprite_section, indices_to_rgba, resolve_fd_sprite,
    sprite_v2_bpp,
};

// Re-exportar funciones de runtime de action_graph
pub use action_graph::{
    collect_aircraft_sprite_graphics, collect_airport_sprite_graphics,
    collect_airport_tile_sprite_graphics, collect_canal_sprite_graphics,
    collect_cargo_sprite_graphics, collect_feature_sprite_graphics, collect_house_sprite_graphics,
    collect_industry_sprite_graphics, collect_industry_tile_sprite_graphics,
    collect_object_sprite_graphics, collect_railtype_sprite_graphics,
    collect_road_vehicle_sprite_graphics, collect_roadstop_sprite_graphics,
    collect_roadtype_sprite_graphics, collect_ship_sprite_graphics,
    collect_station_sprite_graphics, collect_train_sprite_graphics,
};

// Re-exportar funciones de runtime de action5
pub use action5::{
    ACTION5_TYPE_AIRPORT_PREVIEW, ACTION5_TYPE_BRIDGE_DECKS, ACTION5_TYPE_CANALS,
    ACTION5_TYPE_CATENARY, ACTION5_TYPE_FOUNDATIONS, ACTION5_TYPE_ONEWAY, ACTION5_TYPE_OPENTTD_GUI,
    ACTION5_TYPE_ROADSTOPS, ACTION5_TYPE_SHORE, ACTION5_TYPE_SIGNALS, ACTION5_TYPE_TRAMWAY,
    ACTION5_TYPE_TWOCC, AIRPORT_PREVIEW_ACTION5_SLOT_COUNT, Action5LoadContext,
    BRIDGE_DECKS_ACTION5_SLOT_COUNT, CANALS_ACTION5_LOCK_SLOT, CANALS_ACTION5_SLOT_COUNT,
    CATENARY_ACTION5_SLOT_COUNT, CATENARY_ENTRANCE_SPRITE_BASE, CATENARY_PYLON_SPRITE_BASE,
    CATENARY_WIRE_SPRITE_BASE, FOUNDATION_ACTION5_SLOT_COUNT, ONEWAY_ACTION5_SLOT_COUNT,
    OPENTTD_GUI_ACTION5_SLOT_COUNT, ROADSTOP_ACTION5_SLOT_COUNT, SHORE_ACTION5_SLOT_COUNT,
    SHORE_MISSING_BLOCK_SLOTS, SIGNAL_ACTION5_SLOT_COUNT, SPR_SIGNALS_ACTION5_BASE,
    TRAMWAY_ACTION5_SLOT_COUNT, TWOCC_ACTION5_SLOT_COUNT, action5_type_name,
    airport_preview_action5_slot, bridge_decks_action5_base, bridge_decks_action5_slot,
    catenary_action5_local_slot, collect_action5_blocks, collect_active_action5_blocks,
    disallowed_road_directions, foundation_action5_slot_for_sprite_id, merge_action5_offset_block,
    merge_airport_preview_action5_block, merge_bridge_decks_action5_block,
    merge_canals_action5_block, merge_catenary_action5_block, merge_foundation_action5_block,
    merge_oneway_action5_block, merge_openttd_gui_action5_block, merge_roadstop_action5_block,
    merge_shore_action5_block, merge_signals_action5_block, merge_tramway_action5_block,
    merge_twocc_action5_block, oneway_action5_slot, roadstop_action5_slot, signal_action5_slot,
};

// Re-exportar builders sintéticos desde fixture (para compatibilidad temporal)
pub use fixture::{
    build_action1_feature_payload, build_action1_trains_payload,
    build_action2_callback_literal_payload, build_action2_single_set_payload,
    build_action2_stations_payload, build_action2_trains_payload, build_action2_trains_random,
    build_action2_trains_random_consist, build_action2_trains_variational_default,
    build_action2_variational_advanced_add_literal, build_action2_variational_default_payload,
    build_action2_variational_divmod_payload, build_action2_variational_payload,
    build_action2_vehicle_payload, build_action3_feature_payload,
    build_action3_feature_specific_payload, build_action3_trains_payload,
    build_grf_v2_action5_with_sprite, build_grf_v2_airport_purchase_default_sprites,
    build_grf_v2_airport_tile_with_preview_sprite, build_grf_v2_cargo_with_preview_sprite,
    build_grf_v2_feature_with_action2_chain, build_grf_v2_house_with_preview_sprite,
    build_grf_v2_industries_with_tiles, build_grf_v2_industry_tile_with_preview_sprite,
    build_grf_v2_railtype_signal_sprites, build_grf_v2_roadtype_with_action2_chain,
    build_grf_v2_roadtype_with_preview_sprite, build_grf_v2_station_with_action2_chain,
    build_grf_v2_station_with_preview_sprite, build_grf_v2_train_with_action2_chain,
    build_grf_v2_train_with_chunked_sprite, build_grf_v2_train_with_compressed_sprite,
    build_grf_v2_train_with_fd_rgba_sprite, build_grf_v2_train_with_fd_sprite,
    build_grf_v2_train_with_preview_sprite, build_grf_v2_train_with_variational_chain,
    build_grf_v2_with_preview_sprite, build_real_sprite_v1_chunked,
    build_real_sprite_v1_chunked_payload, build_real_sprite_v1_compressed,
    build_real_sprite_v1_compressed_payload, build_real_sprite_v1_dims,
    build_real_sprite_v1_uncompressed, build_real_sprite_v1_uncompressed_payload,
    build_sprite_section_palette_entry, build_sprite_section_rgba_chunked_entry,
    build_sprite_section_rgba_entry, build_sprite_section_rgba_mask_entry,
    compress_grf_lz77_literals,
};
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::newgrf_actions::{
        ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_TRAINS, build_action0_roadtype_payload,
        build_action0_train_payload,
    };
    use crate::newgrf_company_ramp::AUTHOR_CC_PALETTE_FIRST;
    use crate::vehicle::Vehicle;

    use action_graph::{parse_action2_basic, parse_action2_random, parse_action2_variational};

    #[test]
    fn decode_flat_8bpp_applies_palette_and_transparency() {
        let w = 2u16;
        let h = 2u16;
        let indices = [0u8, 174, 174, 0]; // 174 ≈ rojo en DOS
        let body = build_real_sprite_v1_uncompressed(w, h, -1, -2, &indices);
        let spr = decode_real_sprite_v1_uncompressed(&body).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
        assert_eq!(spr.rgba.len(), 16);
        assert_eq!(&spr.rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(spr.rgba[7], 255); // alpha del pixel rojo
    }

    #[test]
    fn decompress_lz77_literals_and_backref() {
        // Literal 2 bytes + backref length 2, offset 2 (code 0xF0 = -16).
        let stream = [0x02u8, 0xAA, 0xBB, 0xF0, 0x02];
        let out = decompress_grf_lz77(&stream, 4).unwrap();
        assert_eq!(out, vec![0xAA, 0xBB, 0xAA, 0xBB]);
        let lit = compress_grf_lz77_literals(&[1, 2, 3, 4]);
        assert_eq!(decompress_grf_lz77(&lit, 4).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn decode_compressed_sprite_matches_uncompressed() {
        let indices = [0u8, 174, 174, 0];
        let plain = build_real_sprite_v1_uncompressed(2, 2, -1, -2, &indices);
        let compressed = build_real_sprite_v1_compressed(2, 2, -1, -2, &indices);
        let a = decode_real_sprite_v1_uncompressed(&plain).unwrap();
        let b = decode_real_sprite_v1_uncompressed(&compressed).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn decode_chunked_sprite_matches_flat() {
        let indices = [0u8, 174, 174, 0, 174, 0, 0, 174];
        let plain = build_real_sprite_v1_uncompressed(4, 2, -1, -2, &indices);
        let chunked = build_real_sprite_v1_chunked(4, 2, -1, -2, &indices).unwrap();
        let a = decode_real_sprite_v1_uncompressed(&plain).unwrap();
        let b = decode_real_sprite_v1_uncompressed(&chunked).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn collect_train_chunked_sprite_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1981, 95, 720, "Chunk Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_chunked_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'C', 0, 1],
            "tchunk",
        )
        .unwrap();
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_train_fd_sprite_from_sprite_section() {
        let a0 = build_action0_train_payload(1982, 100, 750, "FD Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_train_with_fd_sprite(&a0, 0, 1, 8, 8, &indices, [b'T', b'F', 0, 1], "tfd");
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn decode_v2_section_palette_roundtrip() {
        let indices = [0u8, 174, 174, 0];
        let entry = build_sprite_section_palette_entry(7, 0, 2, 2, -1, -2, &indices);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 7).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
    }

    #[test]
    fn resolve_fd_prefers_normal_zoom_over_2x_in() {
        let normal = [10u8, 11, 12, 13];
        let zoom2 = [20u8, 21, 22, 23];
        let mut section = build_sprite_section_palette_entry(3, 2, 2, 2, 0, 0, &zoom2);
        // Mismo ID, zoom normal después (OpenTTD agrupa por id).
        section.extend(build_sprite_section_palette_entry(
            3, 0, 2, 2, 0, 0, &normal,
        ));
        let index = index_sprite_section(&section);
        let spr = resolve_fd_sprite(&index, 3).unwrap();
        // Pixel (0,0) del zoom normal = índice 10 → no transparente.
        assert_ne!(&spr.rgba[0..4], &[0, 0, 0, 0]);
        let only_2x = build_sprite_section_palette_entry(4, 2, 2, 2, 0, 0, &zoom2);
        let index2 = index_sprite_section(&only_2x);
        let spr2 = resolve_fd_sprite(&index2, 4).unwrap();
        assert_eq!(spr2.width, 2);
    }

    #[test]
    fn decode_v2_section_rgba_roundtrip() {
        let rgba = [10u8, 20, 30, 255, 40, 50, 60, 128, 0, 0, 0, 0, 1, 2, 3, 200];
        let entry = build_sprite_section_rgba_entry(9, 0, 2, 2, -1, -2, &rgba);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 9).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
        assert_eq!(spr.rgba, rgba);
    }

    #[test]
    fn resolve_fd_prefers_32bpp_over_palette_same_zoom() {
        let indices = [174u8, 0, 0, 174];
        let rgba = [1u8, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let mut section = build_sprite_section_palette_entry(5, 0, 2, 2, 0, 0, &indices);
        section.extend(build_sprite_section_rgba_entry(5, 0, 2, 2, 0, 0, &rgba));
        let index = index_sprite_section(&section);
        let spr = resolve_fd_sprite(&index, 5).unwrap();
        assert_eq!(&spr.rgba[0..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn collect_train_fd_rgba_sprite_from_sprite_section() {
        let a0 = build_action0_train_payload(1983, 110, 780, "RGBA Loco");
        let mut rgba = vec![0u8; 8 * 8 * 4];
        for y in 2..6 {
            for x in 2..6 {
                let i = (y * 8 + x) * 4;
                rgba[i] = 200;
                rgba[i + 1] = 40;
                rgba[i + 2] = 40;
                rgba[i + 3] = 255;
            }
        }
        let bytes = build_grf_v2_train_with_fd_rgba_sprite(
            &a0,
            0,
            2,
            8,
            8,
            &rgba,
            [b'T', b'R', 0, 1],
            "trgba",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.windows(4).any(|p| p == [200, 40, 40, 255]));
    }

    #[test]
    fn collect_train_compressed_sprite_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1980, 90, 700, "LZ Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_compressed_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'Z', 0, 1],
            "tlz",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_action1_3_preview_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1960, 100, 800, "Sprite Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'S', 0, 1],
            "tsprite",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        assert_eq!(gfx.sets[0].len(), 1);
        assert_eq!(gfx.assigns.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_train_action2_chain_resolves_to_action1_set() {
        let a0 = build_action0_train_payload(1975, 120, 900, "A2 Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let a2_id = 7u8;
        let bytes = build_grf_v2_train_with_action2_chain(
            &a0,
            0,
            a2_id,
            8,
            8,
            &indices,
            [b'T', b'A', 0, 2],
            "ta2",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        assert_eq!(gfx.action2_to_action1.get(&a2_id), Some(&0));
        assert_eq!(gfx.assigns[0].set_id, u16::from(a2_id));
        // Sin Action2: sets[7] no existe; con resolución → set 0.
        assert!(gfx.sets.get(usize::from(a2_id)).is_none());
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(gfx.resolve_action1_set(u16::from(a2_id)), 0);
    }

    #[test]
    fn collect_station_action2_chain_resolves() {
        let a0 = crate::newgrf_actions::build_action0_station_payload(
            b"A2ST",
            b"Plat",
            0,
            0,
            "A2 Station",
        );
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let a2_id = 5u8;
        let bytes = build_grf_v2_station_with_action2_chain(
            &a0,
            0,
            a2_id,
            8,
            8,
            &indices,
            [b'S', b'A', 0, 2],
            "sa2",
        );
        let gfx = collect_station_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.action2_to_action1.get(&a2_id), Some(&0));
        assert_eq!(gfx.resolve_action1_set(u16::from(a2_id)), 0);
        assert!(gfx.preview_for_local_id(0).is_some());
    }

    #[test]
    fn collect_roadtype_action2_single_set_resolves() {
        let a0 =
            crate::newgrf_actions::build_action0_roadtype_payload(b"A2RD", false, 1970, "A2 Road");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let a2_id = 6u8;
        let bytes = build_grf_v2_roadtype_with_action2_chain(
            &a0,
            0,
            a2_id,
            8,
            8,
            &indices,
            [b'R', b'A', 0, 2],
            "ra2",
        );
        let gfx = collect_roadtype_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.action2_to_action1.get(&a2_id), Some(&0));
        assert_eq!(gfx.resolve_action1_set(u16::from(a2_id)), 0);
        assert!(gfx.preview_for_local_id(0).is_some());
        let single = build_action2_single_set_payload(ACTION0_FEATURE_ROADTYPES, 9, 3);
        assert_eq!(
            parse_action2_basic(&single, ACTION0_FEATURE_ROADTYPES),
            Some((9, 3))
        );
    }

    #[test]
    fn parse_action2_variational_default_only() {
        let payload = [0x02, ACTION0_FEATURE_TRAINS, 0x01, 0x81, 0x00];
        assert!(parse_action2_basic(&payload, ACTION0_FEATURE_TRAINS).is_none());
        let var = build_action2_trains_variational_default(9, 5);
        let parsed = parse_action2_variational(&var, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(parsed.0, 9);
        assert_eq!(parsed.1.default, 5);
        assert_eq!(parsed.1.ranges, vec![(5, 0, 0xFF)]);
        let basic = build_action2_trains_payload(3, 0, 0);
        assert_eq!(
            parse_action2_basic(&basic, ACTION0_FEATURE_TRAINS),
            Some((3, 0))
        );
        assert!(parse_action2_variational(&basic, ACTION0_FEATURE_TRAINS).is_none());
    }

    #[test]
    fn collect_train_variational_chain_follows_default() {
        let a0 = build_action0_train_payload(1976, 130, 920, "Var Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let var_id = 9u8;
        let basic_id = 7u8;
        let bytes = build_grf_v2_train_with_variational_chain(
            &a0,
            0,
            var_id,
            basic_id,
            8,
            8,
            &indices,
            [b'T', b'V', 0, 2],
            "tvar",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(
            gfx.action2_var.get(&var_id).map(|v| v.default),
            Some(u16::from(basic_id))
        );
        assert_eq!(gfx.action2_to_action1.get(&basic_id), Some(&0));
        assert_eq!(gfx.resolve_action1_set(u16::from(var_id)), 0);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
    }

    #[test]
    fn resolve_variational_divide_with_ctx() {
        let payload = build_action2_variational_divmod_payload(
            ACTION0_FEATURE_TRAINS,
            5,
            0x40,
            0,
            0xFF,
            Some(0),
            Some(10),
            None,
            &[(1, 2, 2)], // value 25/10 = 2
            9,
        );
        let (set_id, entry) = parse_action2_variational(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 5);
        assert_eq!(entry.first.adjust.divide_val, Some(10));
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(5, entry);
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 25);
        assert_eq!(gfx.resolve_action1_set_ctx(5, &mut ctx), 0);
        ctx.vars.insert(0x40, 5);
        assert_eq!(gfx.resolve_action1_set_ctx(5, &mut ctx), 1); // 5/10=0 → default 9
    }

    #[test]
    fn parse_and_resolve_advanced_variational_add_literal() {
        // var 0x40 (=5) + literal 3 = 8 → rango (1, 8, 8)
        let payload = build_action2_variational_advanced_add_literal(
            ACTION0_FEATURE_TRAINS,
            4,
            0x40,
            0xFF,
            3,
            &[(1, 8, 8)],
            9,
        );
        let (set_id, entry) = parse_action2_variational(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 4);
        assert_eq!(entry.ops.len(), 1);
        assert_eq!(entry.ops[0].operator, 0x00);
        assert_eq!(entry.ops[0].rhs.variable, 0x1A);
        assert_eq!(entry.ops[0].rhs.adjust.and_mask, 3);
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(4, entry);
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 5);
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 0);
        ctx.vars.insert(0x40, 0);
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 1); // 0+3=3 → default
        assert!(gfx.needs_runtime_resolve());
    }

    #[test]
    fn parse_and_resolve_random_consist_0x84() {
        let payload = build_action2_trains_random_consist(6, 2, 0, &[20, 21]);
        let (set_id, entry) = parse_action2_random(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 6);
        assert_eq!(entry.typ, 0x84);
        assert_eq!(entry.consist_count, 2);
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_random.insert(6, entry);
        gfx.action2_to_action1.insert(20, 0);
        gfx.action2_to_action1.insert(21, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.consist_random_bits.insert(2, 1);
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 1);
        ctx.consist_random_bits.insert(2, 0);
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 0);
    }

    #[test]
    fn resolve_variational_ranges_with_ctx() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 1, 1), (8, 2, 5)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(7, 0);
        gfx.action2_to_action1.insert(8, 1);
        gfx.action2_to_action1.insert(9, 2);
        let mut ctx = Action2EvalCtx::default();
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 2); // default → 9 → a1 2
        ctx.vars.insert(0x40, 1);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);
        ctx.vars.insert(0x40, 3);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 1);
    }

    #[test]
    fn resolve_random_action2_with_bits() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_random.insert(
            4,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 0,
                randbit: 0,
                sets: vec![10, 11],
            },
        );
        gfx.action2_to_action1.insert(10, 0);
        gfx.action2_to_action1.insert(11, 1);
        let mut ctx = Action2EvalCtx {
            random_bits: 0,
            ..Action2EvalCtx::default()
        };
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 0);
        ctx.random_bits = 1;
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 1);
        assert_eq!(gfx.resolve_action1_set(4), 0); // sin ctx → set[0]
    }

    #[test]
    fn resolve_real_action2_uses_loaded_and_loading_stage() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_real.insert(
            6,
            Action2RealEntry {
                loaded: vec![10, 11, 12],
                loading: vec![20, 21],
            },
        );
        gfx.action2_to_action1
            .extend([(10, 0), (11, 1), (12, 2), (20, 3), (21, 4)]);

        let mut ctx = Action2EvalCtx {
            vehicle_cargo: 0,
            vehicle_capacity: 100,
            ..Default::default()
        };
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 0);
        ctx.vehicle_cargo = 50;
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 1);
        ctx.vehicle_cargo = 100;
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 2);

        ctx.vehicle_loading = true;
        ctx.vehicle_cargo = 99;
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 4);
    }

    #[test]
    fn rerandomisation_respects_all_triggers_and_active_branch() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 1,
        });
        // Root: un evento VehicleArrives (bit 2) reseedea bit 0. Con el
        // random anterior en cero se entra además al grupo hijo 2.
        gfx.action2_random.insert(
            1,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 1 << 2,
                randbit: 0,
                sets: vec![2, 3],
            },
        );
        // Hijo: sólo se activa cuando ya están NewCargo (0) y VehicleArrives
        // (2); reseedea el bit 4. El bit 7 conserva el modo `all` en raw.
        gfx.action2_random.insert(
            2,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 0x80 | (1 << 0) | (1 << 2),
                randbit: 4,
                sets: vec![4, 5],
            },
        );

        let mut ctx = Action2EvalCtx::default();
        let (reseed, used) = gfx.rerandomisation_for_local_id(0, &mut ctx, 1 << 2);
        assert_eq!(reseed, 1, "el hijo all aún no puede reseedear");
        assert_eq!(used, 1 << 2);

        let (reseed, used) = gfx.rerandomisation_for_local_id(0, &mut ctx, (1 << 0) | (1 << 2));
        assert_eq!(reseed, (1 << 0) | (1 << 4));
        assert_eq!(used, (1 << 0) | (1 << 2));

        // Seleccionar la rama 3 no visita el grupo 2, aunque exista en el
        // mismo GRF: sólo su camino Action2 activo puede cambiar bits.
        ctx.random_bits = 1;
        let (reseed, used) = gfx.rerandomisation_for_local_id(0, &mut ctx, (1 << 0) | (1 << 2));
        assert_eq!(reseed, 1);
        assert_eq!(used, 1 << 2);
    }

    #[test]
    fn needs_runtime_resolve_for_any_variational() {
        let mut gfx = TrainSpriteGraphics::default();
        assert!(!gfx.needs_runtime_resolve());
        gfx.action2_var.insert(
            1,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(2, 1, 1)],
                default: 3,
            },
        );
        assert!(gfx.needs_runtime_resolve());
    }

    #[test]
    fn resolve_variational_var40_from_unit_ctx() {
        use crate::train_consist::action2_eval_ctx_for_unit;
        use crate::vehicle::VehicleKind;

        let mut vs = vec![
            Vehicle::new(
                1,
                VehicleKind::Train,
                TileCoord::new(0, 0),
                TileCoord::new(0, 0),
            ),
            Vehicle::new(
                2,
                VehicleKind::Train,
                TileCoord::new(0, 0),
                TileCoord::new(0, 0),
            ),
        ];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(crate::train_consist::attach_wagon(&mut vs, 1, 2).is_ok());

        let mut gfx = TrainSpriteGraphics::default();
        // shift 0, and FF → ff position; rango set 7 si ff==1
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 1, 1)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(7, 0);
        gfx.action2_to_action1.insert(9, 1);

        let mut ctx_head = action2_eval_ctx_for_unit(&vs, 1, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx_head), 1); // ff=0 → default

        let mut ctx_wagon =
            action2_eval_ctx_for_unit(&vs, 2, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx_wagon), 0); // ff=1 → set 7
    }

    #[test]
    fn resolve_procedure_7e_and_psto() {
        // Procedure set 8: nvar=0 → callback con valor de var 0x40 (=7)
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            8,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(), // nvar=0 → callback
                default: 0,
            },
        );
        // Caller set 3: 7E[8] → si valor==7 elige set 1
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7E,
                    param: Some(8),
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 7, 7)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 7);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);
        assert_eq!(ctx.last_result, 7);

        // \2psto: store 5 into persistent[2], then read 7C[2]
        gfx.action2_var.insert(
            4,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 5,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: vec![
                    Action2VarOp {
                        operator: 0x10, // psto
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: 2, // register index
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                    Action2VarOp {
                        operator: 0x0F, // rst → start fresh with 7C[2]
                        rhs: Action2VarTerm {
                            variable: 0x7C,
                            param: Some(2),
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: 0xFF,
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                ],
                ranges: vec![(1, 5, 5)],
                default: 9,
            },
        );
        let mut ctx2 = Action2EvalCtx::default();
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx2), 0);
        assert_eq!(ctx2.persistent_registers.get(&2), Some(&5));
    }

    #[test]
    fn resolve_callback_nvar0_literal() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0x0A,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        assert_eq!(
            gfx.resolve_callback(0, CBID_STATION_BUILD_TILE_LAYOUT, 0, 0),
            0x0A
        );
        // Sin variational callback → falla.
        let plain = TrainSpriteGraphics::default();
        assert_eq!(
            plain.resolve_callback(0, CBID_STATION_BUILD_TILE_LAYOUT, 0, 0),
            CALLBACK_FAILED
        );
    }

    #[test]
    fn resolve_grf_param_7f() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7F,
                    param: Some(1),
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 9, 9)],
                default: 2,
            },
        );
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(2, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.set_grf_params(&[0, 9, 0]);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);
        let mut ctx_miss = Action2EvalCtx::default();
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx_miss), 1);
    }

    #[test]
    fn resolve_parameterized_scope_value_before_generic_variable() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x68,
                    param: Some(0x01),
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 9, 9)],
                default: 2,
            },
        );
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(2, 1);

        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x68, 3);
        ctx.parameterized_vars.insert((0x68, 0x01), 9);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);

        ctx.parameterized_vars.clear();
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 1);
    }

    #[test]
    fn decode_v2_chunked_rgba_roundtrip() {
        let rgba = [10u8, 20, 30, 255, 40, 50, 60, 128, 0, 0, 0, 0, 1, 2, 3, 200];
        let entry = build_sprite_section_rgba_chunked_entry(11, 0, 2, 2, -1, -2, &rgba).unwrap();
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 11).unwrap();
        assert_eq!(spr.rgba, rgba);
    }

    #[test]
    fn bake_company_mask_remaps_author_ramp() {
        let rgba = vec![128u8, 128, 128, 255, 200, 200, 200, 255];
        let mask = vec![AUTHOR_CC_PALETTE_FIRST, 0];
        let entry = build_sprite_section_rgba_mask_entry(12, 0, 2, 1, 0, 0, &rgba, &mask);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 12).unwrap();
        assert_eq!(spr.mask, mask);
        let baked = bake_sprite_company_mask(&spr, 4); // Red
        // Pixel 0 masked → rampa red; pixel 1 sin máscara.
        assert_ne!(&baked[0..3], &rgba[0..3]);
        assert_eq!(&baked[4..8], &rgba[4..8]);
    }

    #[test]
    fn collect_roadtype_preview_from_synthetic_grf() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble");
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 200;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'T', 0, 2],
            "rtgfx",
        );
        let gfx = collect_roadtype_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        // Feature distinto: trains no ve el set.
        let trains = collect_train_sprite_graphics(&bytes).unwrap();
        assert!(trains.sets.is_empty());
    }

    #[test]
    fn collect_station_preview_from_synthetic_grf() {
        use crate::newgrf_actions::build_action0_station_payload;
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'T', 0, 3],
            "stgfx",
        );
        let gfx = collect_station_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
    }

    #[test]
    fn collect_action5_block_with_preview() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_action5_with_sprite(
            0x0D,
            4804,
            8,
            8,
            &indices,
            [b'S', b'H', 0, 1],
            "shore",
        );
        let blocks = collect_action5_blocks(&bytes).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].type_id, 0x0D);
        assert_eq!(blocks[0].num_sprites, 1);
        assert_eq!(blocks[0].offset, 4804);
        assert_eq!(blocks[0].sprites.len(), 1);
        assert_eq!(action5_type_name(0x0D), "shore");
        let preview = blocks[0].first_preview.as_ref().unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        let mut slots = vec![None; SHORE_ACTION5_SLOT_COUNT];
        merge_shore_action5_block(&mut slots, &blocks[0]);
        // offset 4804 ≥ 18 → escribe en slot 0
        assert!(slots[0].is_some());
    }

    #[test]
    fn action3_vehicle_cargo_group_overrides_default() {
        let sprite = |red| DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![red, 0, 0, 255],
            mask: Vec::new(),
        };
        let mut gfx = TrainSpriteGraphics {
            sets: vec![vec![sprite(10)], vec![sprite(20)]],
            assigns: vec![TrainSpriteAssign {
                local_id: 4,
                set_id: 0,
            }],
            ..Default::default()
        };
        gfx.specific_assigns
            .insert((4, crate::CargoType::Goods.temperate_id()), 1);
        let mut ctx = Action2EvalCtx::default();
        let goods = gfx
            .views_for_local_id_cargo_ctx(4, Some(crate::CargoType::Goods), &mut ctx)
            .unwrap();
        assert_eq!(goods[0].rgba[0], 20);
        let coal = gfx
            .views_for_local_id_cargo_ctx(4, Some(crate::CargoType::Coal), &mut ctx)
            .unwrap();
        assert_eq!(coal[0].rgba[0], 10);
    }

    #[test]
    fn action3_station_cargo_group_overrides_default_with_fallback() {
        // Stations comparten el grafo Action3 → cargo group / default (#251).
        let sprite = |red| DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![red, 0, 0, 255],
            mask: Vec::new(),
        };
        let mut gfx = TrainSpriteGraphics {
            sets: vec![vec![sprite(11)], vec![sprite(22)]],
            assigns: vec![TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            ..Default::default()
        };
        gfx.specific_assigns
            .insert((0, crate::CargoType::Goods.temperate_id()), 1);
        let mut ctx = Action2EvalCtx::default();
        let goods = gfx
            .views_for_local_id_cargo_ctx(0, Some(crate::CargoType::Goods), &mut ctx)
            .unwrap();
        assert_eq!(goods[0].rgba[0], 22);
        let fallback = gfx
            .views_for_local_id_cargo_ctx(0, Some(crate::CargoType::Coal), &mut ctx)
            .unwrap();
        assert_eq!(fallback[0].rgba[0], 11);
        assert_eq!(
            gfx.views_for_local_id_cargo_ctx(0, None, &mut ctx).unwrap()[0].rgba[0],
            11
        );
    }
}
