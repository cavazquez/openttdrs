//! Inspección parse-only de acciones `NewGRF` (Action0–14) y apply mínimo.
//!
//! El walker cuenta acciones sin aplicar. Action0 features registradas:
//! - `RoadTypes` (0x12) → `GameState.road_type_catalog`
//! - `Stations` (0x04) → `station_class_catalog` / `station_spec_catalog`
//! - `IndustryTiles` (0x09) → `industry_tile_spec_catalog`
//! - `Cargoes` (0x0B) → `cargo_spec_catalog`
//! - `Objects` (0x0F) → `object_spec_catalog`
//! - `RoadStops` (0x14) → `road_stop_class_catalog` / `road_stop_spec_catalog`
//! - `Badges` (0x15) → `badge_catalog`

pub mod action0;
pub mod apply;
pub mod inspect;

// Re-exports públicos
pub use action0::{
    ACTION0_FEATURE_AIRCRAFT, ACTION0_FEATURE_BADGES, ACTION0_FEATURE_CARGOES,
    ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_OBJECTS, ACTION0_FEATURE_RAILTYPES,
    ACTION0_FEATURE_ROAD_VEHICLES, ACTION0_FEATURE_ROADSTOPS, ACTION0_FEATURE_ROADTYPES,
    ACTION0_FEATURE_SHIPS, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
    ACTION0_FEATURE_TRAMTYPES, Action0Header, ParsedBadgeMeta, ParsedCargoMeta,
    ParsedIndustryTileMeta, ParsedObjectMeta, ParsedRailTypeMeta, ParsedRoadStopMeta,
    ParsedRoadTypeMeta, ParsedStationMeta, ParsedTrainMeta, ParsedVehicleMeta,
    collect_badge_metas_from_grf, collect_cargo_metas_from_grf,
    collect_industry_tile_metas_from_grf, collect_object_metas_from_grf,
    collect_railtype_metas_from_grf, collect_roadstop_metas_from_grf,
    collect_roadtype_metas_from_grf, collect_station_metas_from_grf, collect_train_metas_from_grf,
    collect_vehicle_metas_from_grf, for_each_pseudo_payload, parse_action0_badge_meta,
    parse_action0_cargo_meta, parse_action0_header, parse_action0_industry_tile_meta,
    parse_action0_object_meta, parse_action0_railtype_metas, parse_action0_roadstop_meta,
    parse_action0_roadtype_meta, parse_action0_station_meta, parse_action0_train_meta,
    parse_action0_vehicle_metas,
};

pub use apply::{
    action5::{
        apply_newgrf_action5_airport_preview, apply_newgrf_action5_airport_preview_default_dirs,
        apply_newgrf_action5_all_default_dirs, apply_newgrf_action5_bridge_decks,
        apply_newgrf_action5_bridge_decks_default_dirs, apply_newgrf_action5_catenary,
        apply_newgrf_action5_catenary_default_dirs, apply_newgrf_action5_foundations,
        apply_newgrf_action5_foundations_default_dirs, apply_newgrf_action5_oneway,
        apply_newgrf_action5_oneway_default_dirs, apply_newgrf_action5_openttd_gui,
        apply_newgrf_action5_openttd_gui_default_dirs, apply_newgrf_action5_roadstops,
        apply_newgrf_action5_roadstops_default_dirs, apply_newgrf_action5_shore,
        apply_newgrf_action5_shore_default_dirs, apply_newgrf_action5_signals,
        apply_newgrf_action5_signals_default_dirs,
    },
    apply_newgrf_stack_catalogs_default_dirs,
    badges::{apply_newgrf_badges, apply_newgrf_badges_default_dirs},
    cargo::{apply_newgrf_cargoes, apply_newgrf_cargoes_default_dirs},
    industry::{apply_newgrf_industry_tiles, apply_newgrf_industry_tiles_default_dirs},
    objects::{apply_newgrf_objects, apply_newgrf_objects_default_dirs},
    rail::{apply_newgrf_rail_signals, apply_newgrf_rail_signals_default_dirs},
    road::{apply_newgrf_road_types, apply_newgrf_road_types_default_dirs},
    roadstop::{apply_newgrf_roadstops, apply_newgrf_roadstops_default_dirs},
    station::{apply_newgrf_stations, apply_newgrf_stations_default_dirs},
    train::{apply_newgrf_vehicles_trains, apply_newgrf_vehicles_trains_default_dirs},
};

pub use inspect::{Action5SlotSummary, GrfInspectReport, inspect_grf_bytes, inspect_grf_file};

// Builders para tests
#[must_use]
pub fn build_action0_railtype_payload(local_id: u8, label: &[u8; 4]) -> Vec<u8> {
    let mut payload = vec![0x00, ACTION0_FEATURE_RAILTYPES, 0x01, 0x01, local_id, 0x08];
    payload.extend_from_slice(label);
    payload
}

/// Action0 `RailTypes` con label + `0x14` `max_speed`.
#[must_use]
pub fn build_action0_railtype_payload_with_speed(
    local_id: u8,
    label: &[u8; 4],
    max_speed: u16,
) -> Vec<u8> {
    let mut payload = vec![0x00, ACTION0_FEATURE_RAILTYPES, 0x02, 0x01, local_id, 0x08];
    payload.extend_from_slice(label);
    payload.push(0x14);
    payload.extend_from_slice(&max_speed.to_le_bytes());
    payload
}

#[must_use]
pub fn build_action0_roadtype_payload(
    short_label: &[u8; 4],
    is_tram: bool,
    intro_year: u16,
    name: &str,
) -> Vec<u8> {
    build_action0_roadtype_payload_with_speed(short_label, is_tram, intro_year, 0, name)
}

/// Action0 `RoadTypes`/`TramTypes` con `0x14` `max_speed`.
#[must_use]
pub fn build_action0_roadtype_payload_with_speed(
    short_label: &[u8; 4],
    is_tram: bool,
    intro_year: u16,
    max_speed: u16,
    name: &str,
) -> Vec<u8> {
    let feature = if is_tram {
        ACTION0_FEATURE_TRAMTYPES
    } else {
        ACTION0_FEATURE_ROADTYPES
    };
    let num_props = if is_tram { 0x04 } else { 0x05 };
    let mut p = vec![0x00, feature, num_props, 0x01, 0x00, 0x08];
    p.extend_from_slice(short_label);
    if !is_tram {
        p.push(0x09); // extensión local: flags tram
        p.push(0);
    }
    p.push(0x14); // max_speed
    p.extend_from_slice(&max_speed.to_le_bytes());
    p.push(0x16); // intro year (local WORD)
    p.extend_from_slice(&intro_year.to_le_bytes());
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_station_payload(
    class_label: &[u8; 4],
    _spec_short: &[u8; 4],
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    name: &str,
) -> Vec<u8> {
    // IDs OpenTTD 15.3: 0x0C/0x0D = disallowed; short label se deriva del nombre.
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_STATIONS,
        0x04,
        0x01,
        0x00,
        0x08, // PROP_LABEL
    ];
    p.extend_from_slice(class_label);
    p.push(0x0C); // PROP_STATION_DISALLOWED_PLATFORMS
    p.push(disallowed_platforms);
    p.push(0x0D); // PROP_STATION_DISALLOWED_LENGTHS
    p.push(disallowed_lengths);
    p.push(0xFE); // PROP_NAME_CSTRING
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_train_payload(
    intro_year: u16,
    max_speed: u16,
    power_hp: u16,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_TRAINS,
        0x04,
        0x01,
        0x00,
        0x16, // PROP_INTRO_YEAR
    ];
    p.extend_from_slice(&intro_year.to_le_bytes());
    p.push(0x09); // PROP_TRAIN_SPEED
    p.extend_from_slice(&max_speed.to_le_bytes());
    p.push(0x0B); // PROP_TRAIN_POWER
    p.extend_from_slice(&power_hp.to_le_bytes());
    p.push(0xFE); // PROP_NAME_CSTRING
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_industry_tile_payload(subst_id: u8, override_of: Option<u8>) -> Vec<u8> {
    let num_props = 1 + u8::from(override_of.is_some());
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_INDUSTRYTILES,
        num_props,
        0x01,
        0x00,
        0x08, // PROP_INDTILE_SUBST
        subst_id,
    ];
    if let Some(o) = override_of {
        p.push(0x09); // PROP_INDTILE_OVERRIDE
        p.push(o);
    }
    p
}

#[must_use]
pub fn build_action0_roadstop_payload(class_label: &[u8; 4], stop_type: u8, name: &str) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_ROADSTOPS,
        0x03,
        0x01,
        0x00,
        0x08, // PROP_LABEL
    ];
    p.extend_from_slice(class_label);
    p.push(0x09); // PROP_ROADSTOP_STOP_TYPE
    p.push(stop_type);
    p.push(0xFE); // PROP_NAME_CSTRING
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_badge_payload(label: &[u8; 4], flags: u32, name: Option<&str>) -> Vec<u8> {
    let num_props = 2 + u8::from(name.is_some());
    let mut p = vec![0x00, ACTION0_FEATURE_BADGES, num_props, 0x01, 0x00, 0x08];
    p.extend_from_slice(label);
    p.push(0x09);
    p.extend_from_slice(&flags.to_le_bytes());
    if let Some(n) = name {
        p.push(0xFE);
        p.extend_from_slice(n.as_bytes());
        p.push(0);
    }
    p
}

#[must_use]
pub fn build_action0_cargo_payload(
    local_id: u8,
    bitnum: u8,
    label: &[u8; 4],
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_CARGOES,
        0x03,
        0x01,
        local_id,
        0x08, // bitnum
        bitnum,
        0x17, // label
    ];
    p.extend_from_slice(label);
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_object_payload(
    local_id: u8,
    class_label: &[u8; 4],
    size: u8,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_OBJECTS,
        0x03,
        0x01,
        local_id,
        0x08, // class label
    ];
    p.extend_from_slice(class_label);
    p.push(0x0C); // size
    p.push(size);
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

/// GRF v2 con varios Action0 + Action8 (tests multi-feature / multi-id).
#[must_use]
pub fn build_grf_v2_with_action0s_and_action8(
    action0_payloads: &[&[u8]],
    grfid: [u8; 4],
    name: &str,
    description: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.extend_from_slice(description.as_bytes());
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in action0_payloads {
        let size = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&size.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    {
        let size = u32::try_from(action8.len()).unwrap_or(0);
        data_section.extend_from_slice(&size.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(&action8);
    }
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[must_use]
pub fn build_grf_v2_with_action0_and_action8(
    action0_payload: &[u8],
    grfid: [u8; 4],
    name: &str,
    description: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.extend_from_slice(description.as_bytes());
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0_payload, action8.as_slice()] {
        let size = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&size.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::newgrf_config::build_minimal_grf_v2;
    use crate::road_type::RoadTramType;
    use crate::vehicle::VehicleKind;

    #[test]
    fn inspect_minimal_counts_action8() {
        let bytes = build_minimal_grf_v2([b'T', b'E', 0x01, 0x00], "T", "D");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x08], 1);
        assert_eq!(report.pseudo_sprites, 1);
        assert!(report.action0_features.is_empty());
    }

    #[test]
    fn inspect_counts_action0_roadtypes_feature() {
        let a0 = build_action0_roadtype_payload(b"TEST", false, 1960, "Test Road");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'R', b'T', 0x00, 0x01], "RT", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x00], 1);
        assert_eq!(report.action_counts[0x08], 1);
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_ROADTYPES]);
    }

    #[test]
    fn truncated_bytes_do_not_panic() {
        let err = inspect_grf_bytes(&[0x00]).unwrap_err();
        assert!(matches!(err, crate::newgrf_config::GrfScanError::TooShort));
        let _ = inspect_grf_bytes(&[0x01, 0x02, 0x03]).unwrap();
    }

    #[test]
    fn parse_action0_header_reads_fields() {
        let h = parse_action0_header(&[0x00, 0x12, 0x03, 0x02]).unwrap();
        assert_eq!(h.feature, 0x12);
        assert_eq!(h.num_props, 3);
        assert_eq!(h.num_ids, 2);
    }

    #[test]
    fn parse_roadtype_meta_and_apply_from_bytes() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1850, "Adoquines");
        let meta = parse_action0_roadtype_meta(&a0).unwrap();
        assert_eq!(meta.short_label, "COBB");
        assert_eq!(meta.label, "Adoquines");
        assert_eq!(meta.class, RoadTramType::Road);
        assert_eq!(meta.intro_year, 1850);

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'C', b'B', 0, 1], "cobble", "");
        let dir = tempfile_dir_with("cobble.grf", &bytes);
        apply_newgrf_road_types(&mut state, &[&dir]);
        assert_eq!(state.road_type_catalog.len(), 2);

        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("cobble.grf", 1));
        apply_newgrf_road_types(&mut state, &[&dir]);
        assert!(
            state
                .road_type_catalog
                .iter()
                .any(|d| d.from_newgrf && d.short_label == "COBB")
        );
        let id = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap()
            .id;
        assert!(id.as_u8() >= 2);
        assert!(
            state
                .road_type_catalog
                .iter()
                .find(|d| d.from_newgrf)
                .unwrap()
                .newgrf_preview
                .is_none()
        );
    }

    #[test]
    fn apply_roadtype_with_action1_3_attaches_preview() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble Road");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'G', 0, 1],
            "rgfx",
        );
        let dir = tempfile_dir_with("rgfx.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("rgfx.grf", 2));
        apply_newgrf_road_types(&mut state, &[&dir]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        let preview = def.newgrf_preview_sprite().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(def.newgrf_views.len(), 1);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn parse_station_meta_and_apply_from_bytes() {
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén moderno");
        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.class_short_label, "MODN");
        // Short label se deriva del nombre (ASCII), no de un prop Action0.
        assert_eq!(meta.short_label, "Andn");
        assert_eq!(meta.label, "Andén moderno");

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'T', 0, 1], "stat", "");
        let dir = tempfile_dir_with("stat.grf", &bytes);
        apply_newgrf_stations(&mut state, &[&dir]);
        assert_eq!(state.station_spec_catalog.len(), 1);

        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("stat.grf", 1));
        apply_newgrf_stations(&mut state, &[&dir]);
        assert!(
            state
                .station_spec_catalog
                .iter()
                .any(|d| d.from_newgrf && d.short_label == "Andn")
        );
        assert!(
            state
                .station_class_catalog
                .iter()
                .any(|c| c.from_newgrf && c.short_label == "MODN")
        );
        let spec_id = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap()
            .id;
        assert!(spec_id.as_u16() >= 1);
        assert!(
            state
                .station_spec_catalog
                .iter()
                .find(|d| d.from_newgrf)
                .unwrap()
                .newgrf_preview
                .is_none()
        );
    }

    #[test]
    fn apply_station_with_action1_3_attaches_preview() {
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Sprite Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'G', 0, 1],
            "sgfx",
        );
        let dir = tempfile_dir_with("sgfx.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sgfx.grf", 2));
        apply_newgrf_stations(&mut state, &[&dir]);
        let def = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        let preview = def.newgrf_preview_sprite().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(def.newgrf_views.len(), 1);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn apply_station_with_action2_chain_attaches_views() {
        let a0 = build_action0_station_payload(b"A2ST", b"Plat", 0, 0, "A2 Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_station_with_action2_chain(
            &a0,
            0,
            7,
            8,
            8,
            &indices,
            [b'S', b'2', 0, 1],
            "sa2",
        );
        let dir = tempfile_dir_with("sa2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sa2.grf", 2));
        apply_newgrf_stations(&mut state, &[&dir]);
        let def = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        assert_eq!(def.newgrf_views.len(), 1);
        assert_eq!(def.newgrf_local_id, 0);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn apply_roadtype_with_action2_chain_attaches_views() {
        let a0 = build_action0_roadtype_payload(b"A2RD", false, 1970, "A2 Cobble");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_roadtype_with_action2_chain(
            &a0,
            0,
            8,
            8,
            8,
            &indices,
            [b'R', b'2', 0, 1],
            "ra2",
        );
        let dir = tempfile_dir_with("ra2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ra2.grf", 2));
        apply_newgrf_road_types(&mut state, &[&dir]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        assert_eq!(def.newgrf_views.len(), 1);
        assert_eq!(def.newgrf_local_id, 0);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn inspect_summarizes_action5_slots() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x05,
            1039,
            8,
            8,
            &indices,
            [b'E', b'L', 0, 1],
            "elrail",
        );
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x05], 1);
        assert_eq!(report.action5_slots.len(), 1);
        assert_eq!(report.action5_slots[0].type_id, 0x05);
        assert_eq!(report.action5_slots[0].offset, 1039);
        assert_eq!(report.action5_slots[0].preview_wh, Some((8, 8)));
        let summary = report.format_summary();
        assert!(summary.contains("Action5:"));
        assert!(summary.contains("catenary"));
    }

    #[test]
    fn apply_action5_shore_fills_slot_from_stack() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x0D,
            3,
            8,
            8,
            &indices,
            [b'S', b'H', 0, 2],
            "shore2",
        );
        let dir = tempfile_dir_with("shore2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("shore2.grf", 2));
        apply_newgrf_action5_shore(&mut state, &[&dir]);
        assert_eq!(state.runtime.shore_newgrf_sprites.len(), 18);
        assert!(state.runtime.shore_newgrf_sprites[3].is_some());
        assert!(state.runtime.shore_newgrf_sprites[0].is_none());
        let spr = state.runtime.shore_newgrf_sprites[3].as_ref().unwrap();
        assert_eq!(spr.width, 8);
    }

    #[test]
    fn apply_action5_catenary_fills_slot_from_stack() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x05,
            1039,
            8,
            8,
            &indices,
            [b'E', b'L', 0, 2],
            "el2",
        );
        let dir = tempfile_dir_with("el2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("el2.grf", 2));
        apply_newgrf_action5_catenary(&mut state, &[&dir]);
        assert_eq!(state.runtime.catenary_newgrf_sprites.len(), 36);
        assert!(state.runtime.catenary_newgrf_sprites[0].is_some());
        assert_eq!(crate::catenary_action5_local_slot(1039), Some(0));
        assert_eq!(crate::catenary_action5_local_slot(910_067), Some(28));
    }

    #[test]
    fn inspect_counts_action0_stations_feature() {
        let a0 = build_action0_station_payload(b"TEST", b"Spec", 0, 0, "Test Station");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'T', 0x00, 0x02], "ST", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_STATIONS]);
    }

    #[test]
    fn parse_train_runtime_props_dual_tilt_curve_and_weight() {
        let a0 = vec![
            0x00,
            ACTION0_FEATURE_TRAINS,
            0x08,
            0x01,
            0x00,
            0x12,
            0x04, // image_index TTD 4 → local 2
            0x13,
            0x01, // dual-headed
            0x14,
            40, // capacity
            0x16,
            90, // weight low
            0x17,
            30, // cost factor
            0x1B,
            0x10,
            0x00, // pow_wag_power = 16
            0x27,
            0x01, // RailTilts
            0x2E,
            0x00,
            0x01, // curve_speed_mod = 256
        ];
        let meta = parse_action0_train_meta(&a0).unwrap();
        assert_eq!(meta.train_image_index, 2);
        assert!(meta.dual_headed);
        assert_eq!(meta.capacity, 40);
        assert_eq!(meta.weight_t, 90);
        assert_eq!(meta.price_factor, 30);
        assert_eq!(meta.pow_wag_power, 16);
        assert!(meta.rail_tilts);
        assert_eq!(meta.curve_speed_mod, 256);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'R', 0, 9], "t9", "");
        let dir = tempfile_dir_with("t9.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("t9.grf", 2));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        assert!(eng.dual_headed);
        assert!(eng.rail_tilts);
        assert_eq!(eng.curve_speed_mod, 256);
        assert_eq!(eng.capacity, 40);
        assert_eq!(eng.weight_t, 90);
    }

    #[test]
    fn action5_foundations_and_bridge_decks_merge_respect_ranges() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let found = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x06,
            3,
            8,
            8,
            &indices,
            [b'F', b'N', 0, 1],
            "fn",
        );
        let bridge = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x1B,
            2,
            8,
            8,
            &indices,
            [b'B', b'D', 0, 1],
            "bd",
        );
        let dir = tempfile_dir_with("fn.grf", &found);
        std::fs::write(dir.join("bd.grf"), &bridge).unwrap();
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("fn.grf", 1));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("bd.grf", 2));
        apply_newgrf_action5_foundations(&mut state, &[&dir]);
        apply_newgrf_action5_bridge_decks(&mut state, &[&dir]);
        assert_eq!(state.runtime.foundation_newgrf_sprites.len(), 90);
        assert!(state.runtime.foundation_newgrf_sprites[3].is_some());
        assert!(state.runtime.foundation_newgrf_sprites[4].is_none());
        assert_eq!(state.runtime.bridge_decks_newgrf_sprites.len(), 24);
        assert!(state.runtime.bridge_decks_newgrf_sprites[2].is_some());
        assert_eq!(crate::action5_type_name(0x06), "foundations");
        assert_eq!(crate::action5_type_name(0x09), "oneway-road");
        assert_eq!(crate::action5_type_name(0x11), "roadstops");
        assert_eq!(crate::action5_type_name(0x15), "openttd-gui");
        assert_eq!(crate::action5_type_name(0x16), "airport-preview");
        assert_eq!(crate::action5_type_name(0x1B), "bridge-decks");
        assert_eq!(crate::action5_type_name(0x0C), "snowy-tree-unused");
    }

    #[test]
    fn action5_signals_merge_fills_slot_from_stack() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x04,
            12,
            8,
            8,
            &indices,
            [b'S', b'G', 0, 4],
            "sig5",
        );
        let dir = tempfile_dir_with("sig5.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sig5.grf", 4));
        apply_newgrf_action5_signals(&mut state, &[&dir]);
        assert_eq!(state.runtime.signal_action5_newgrf_sprites.len(), 240);
        assert!(state.runtime.signal_action5_newgrf_sprites[12].is_some());
        assert_eq!(crate::signal_action5_slot(5088 + 12), Some(12));
        assert_eq!(crate::action5_type_name(0x04), "signals");
    }

    #[test]
    fn action5_oneway_roadstops_gui_airport_merge() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let ow = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x09,
            1,
            8,
            8,
            &indices,
            [b'T', b'5', 0x09, 0],
            "ow",
        );
        let dir = tempfile_dir_with("ow.grf", &ow);
        for (name, ty, off) in [
            ("rs.grf", 0x11_u8, 2_u16),
            ("gui.grf", 0x15, 90),
            ("ap.grf", 0x16, 4),
        ] {
            let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
                ty,
                off,
                8,
                8,
                &indices,
                [b'T', b'5', ty, 0],
                name,
            );
            std::fs::write(dir.join(name), &bytes).unwrap();
        }
        let mut state = GameState::new(4, 4);
        for (name, id) in [("ow.grf", 1), ("rs.grf", 2), ("gui.grf", 3), ("ap.grf", 4)] {
            state.newgrf_stack.push(crate::NewGrfEntry::new(name, id));
        }
        let roots = [dir.as_path()];
        apply_newgrf_action5_oneway(&mut state, &roots);
        apply_newgrf_action5_roadstops(&mut state, &roots);
        apply_newgrf_action5_openttd_gui(&mut state, &roots);
        apply_newgrf_action5_airport_preview(&mut state, &roots);
        assert!(state.runtime.oneway_newgrf_sprites[1].is_some());
        assert!(state.runtime.roadstop_action5_newgrf_sprites[2].is_some());
        assert!(state.runtime.openttd_gui_newgrf_sprites[90].is_some());
        assert!(state.runtime.airport_preview_newgrf_sprites[4].is_some());
        assert_eq!(crate::oneway_action5_slot(0, true, 1), Some(0));
        assert_eq!(crate::roadstop_action5_slot(true, 0), Some(4));
    }

    #[test]
    fn parse_train_meta_and_apply_from_bytes() {
        let a0 = build_action0_train_payload(1955, 120, 1500, "Locomotora NewGRF");
        let meta = parse_action0_train_meta(&a0).unwrap();
        assert_eq!(meta.name, "Locomotora NewGRF");
        assert_eq!(meta.intro_year, 1955);
        assert_eq!(meta.max_speed, 120);
        assert_eq!(meta.power_hp, 1500);

        let mut state = GameState::new(4, 4);
        let vanilla_len = state.engine_catalog.len();
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'R', 0, 1], "train", "");
        let dir = tempfile_dir_with("train.grf", &bytes);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("train.grf", 2));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        assert_eq!(state.engine_catalog.len(), vanilla_len + 1);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        assert!(eng.id >= crate::NEWGRF_ENGINE_ID_BASE);
        assert_eq!(eng.name, "Locomotora NewGRF");
        assert_eq!(eng.kind, VehicleKind::Train);
        assert!(eng.newgrf_preview().is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn parse_action0_vehicle_features_with_upstream_widths() {
        let train = vec![
            0x00,
            ACTION0_FEATURE_TRAINS,
            0x06,
            0x01,
            0x00,
            0x00,
            0x42,
            0x0E, // 3650 days after 1920 -> 1930
            0x02,
            10,
            0x03,
            25,
            0x04,
            40,
            0x06,
            0x04,
            0x07,
            13,
        ];
        let meta = parse_action0_train_meta(&train).unwrap();
        assert_eq!(meta.intro_year, 1930);
        assert_eq!(meta.reliability_spd_dec, 40);
        assert_eq!(meta.lifelength_years, 25);
        assert_eq!(meta.model_life_years, 40);
        assert_eq!(meta.climate_mask, 0x04);
        assert_eq!(meta.load_amount, 13);

        let road = vec![
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x0B,
            0x01,
            0x07,
            0x00,
            0x42,
            0x0E, // 3650 days after 1920 -> 1930
            0x04,
            12,
            0x06,
            0x02,
            0x07,
            3,
            0x08,
            120,
            0x09,
            80,
            0x0F,
            32,
            0x10,
            0,
            0x11,
            144,
            0x13,
            90,
            0x14,
            44,
        ];
        let meta = parse_action0_vehicle_metas(&road).unwrap().remove(0);
        assert_eq!(meta.local_id, 7);
        assert_eq!(meta.kind, VehicleKind::Bus);
        assert_eq!(meta.intro_year, 1930);
        assert_eq!(meta.model_life_years, 12);
        assert_eq!(meta.climate_mask, 0x02);
        assert_eq!(meta.load_amount, 3);
        assert_eq!(meta.max_speed, 120);
        assert_eq!(meta.capacity, 32);
        assert_eq!(meta.power_hp, 900);
        assert_eq!(meta.weight_t, 11);

        let ship = vec![
            0x00,
            ACTION0_FEATURE_SHIPS,
            0x04,
            0x01,
            0x02,
            0x0A,
            100,
            0x0C,
            7,
            0x0D,
            0x2C,
            0x01,
            0x23,
            0x20,
            0x01,
        ];
        let meta = parse_action0_vehicle_metas(&ship).unwrap().remove(0);
        assert_eq!(meta.kind, VehicleKind::Ship);
        assert_eq!(meta.cargo, Some(crate::CargoType::Wood));
        assert_eq!(meta.capacity, 300);
        assert_eq!(meta.max_speed, 288);

        let aircraft = vec![
            0x00,
            ACTION0_FEATURE_AIRCRAFT,
            0x04,
            0x01,
            0x09,
            0x0B,
            160,
            0x0C,
            40,
            0x0E,
            90,
            0x0F,
            0x78,
            0x00,
        ];
        let meta = parse_action0_vehicle_metas(&aircraft).unwrap().remove(0);
        assert_eq!(meta.kind, VehicleKind::Aircraft);
        assert_eq!(meta.max_speed, 512);
        assert_eq!(meta.capacity, 120);
    }

    #[test]
    fn apply_registers_road_ship_and_aircraft_in_runtime_catalog() {
        let fixtures = [
            (
                ACTION0_FEATURE_ROAD_VEHICLES,
                vec![0x00, 0x01, 0x03, 0x01, 0x04, 0x08, 100, 0x0F, 25, 0x10, 5],
                VehicleKind::Truck,
            ),
            (
                ACTION0_FEATURE_SHIPS,
                vec![0x00, 0x02, 0x03, 0x01, 0x05, 0x0B, 80, 0x0C, 0, 0x0D, 90, 0],
                VehicleKind::Ship,
            ),
            (
                ACTION0_FEATURE_AIRCRAFT,
                vec![0x00, 0x03, 0x02, 0x01, 0x06, 0x0C, 32, 0x0F, 70, 0],
                VehicleKind::Aircraft,
            ),
        ];
        for (index, (feature, action0, expected_kind)) in fixtures.into_iter().enumerate() {
            let filename = format!("vehicle-{index}.grf");
            let bytes = build_grf_v2_with_action0_and_action8(
                &action0,
                [b'V', b'E', 0, u8::try_from(index).unwrap_or(0)],
                "vehicle",
                "",
            );
            let dir = tempfile_dir_with(&filename, &bytes);
            let mut state = GameState::new(4, 4);
            state
                .newgrf_stack
                .push(crate::NewGrfEntry::new(&filename, u32::from(feature) + 10));
            apply_newgrf_vehicles_trains(&mut state, &[&dir]);
            let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
            assert_eq!(eng.kind, expected_kind);
            assert_eq!(eng.newgrf_local_id, action0[4]);
            assert!(
                crate::engine::engines_for_depot_kind_in(
                    &state.engine_catalog,
                    match expected_kind {
                        VehicleKind::Ship => crate::engine::DepotPurchaseKind::Ship,
                        VehicleKind::Aircraft => crate::engine::DepotPurchaseKind::Aircraft,
                        _ => crate::engine::DepotPurchaseKind::Road,
                    },
                    2050,
                    crate::engine::EngineCatalogSort::Catalog,
                    crate::engine::RoadEngineFilter::All,
                )
                .iter()
                .any(|candidate| candidate.id == eng.id)
            );
        }
    }

    #[test]
    fn action0_climate_mask_filters_runtime_vehicle_catalog() {
        let action0 = vec![
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x02,
            0x01,
            0x04,
            0x06,
            0x02, // sólo subártico
            0x0F,
            25,
        ];
        let bytes =
            build_grf_v2_with_action0_and_action8(&action0, [b'C', b'L', 0, 1], "climate", "");
        let dir = tempfile_dir_with("climate.grf", &bytes);

        for (climate, expected) in [
            (crate::Climate::Temperate, false),
            (crate::Climate::SubArctic, true),
            (crate::Climate::SubTropical, false),
            (crate::Climate::Toyland, false),
        ] {
            let mut state = GameState::new(4, 4);
            state.climate = climate;
            state
                .newgrf_stack
                .push(crate::NewGrfEntry::new("climate.grf", 0x434C_0001));
            apply_newgrf_vehicles_trains(&mut state, &[&dir]);
            assert_eq!(
                state.engine_catalog.iter().any(|engine| engine.from_newgrf),
                expected
            );
        }
    }

    #[test]
    fn apply_train_with_action1_3_attaches_preview() {
        let a0 = build_action0_train_payload(1960, 100, 800, "Sprite Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'S', 0, 2],
            "tsprite",
        );
        let dir = tempfile_dir_with("tsprite.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("tsprite.grf", 3));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        let preview = eng.newgrf_preview().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(eng.newgrf_views.len(), 1);
        assert!(eng.newgrf_view(3).is_some());
    }

    #[test]
    fn apply_train_with_action2_chain_attaches_preview() {
        let a0 = build_action0_train_payload(1975, 120, 900, "A2 Loco Apply");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_train_with_action2_chain(
            &a0,
            0,
            7,
            8,
            8,
            &indices,
            [b'T', b'A', 0, 3],
            "ta2apply",
        );
        let dir = tempfile_dir_with("ta2apply.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ta2apply.grf", 4));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        let preview = eng.newgrf_preview().unwrap();
        assert_eq!(preview.width, 8);
        assert!(!eng.newgrf_views.is_empty());
    }

    #[test]
    fn inspect_counts_action0_trains_feature() {
        let a0 = build_action0_train_payload(1960, 100, 800, "T");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'0', 0, 1], "t", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_TRAINS]);
    }

    #[test]
    fn parse_station_custom_layout_prop_0e() {
        let mut p = vec![0x00, ACTION0_FEATURE_STATIONS, 0x02, 0x01, 0x00];
        p.push(0x08); // PROP_LABEL
        p.extend_from_slice(b"DFLT");
        p.push(0x0E); // PROP_STATION_CUSTOM_LAYOUT
        p.push(3); // length
        p.push(1); // platforms
        p.extend_from_slice(&[0, 2, 0]);
        p.push(0);
        p.push(0);
        let meta = parse_action0_station_meta(&p).unwrap();
        assert_eq!(meta.custom_layouts.get(&(1, 3)), Some(&vec![0, 2, 0]));
    }

    #[test]
    fn apply_stations_registers_multiple_classes_and_specs_without_collision() {
        const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
        let a0_a = build_action0_station_payload(b"CLSA", b"XXXX", 0, 0, "Alpha One");
        let a0_b = build_action0_station_payload(b"CLSB", b"YYYY", 0b0000_0010, 0, "Beta Two");
        let mut action8 = vec![0x08, 0x07];
        action8.extend_from_slice(&[b'M', b'C', 0, 1]);
        action8.extend_from_slice(b"multi\0d\0");
        let mut data_section = Vec::new();
        for payload in [a0_a.as_slice(), a0_b.as_slice(), action8.as_slice()] {
            let size = u32::try_from(payload.len()).unwrap();
            data_section.extend_from_slice(&size.to_le_bytes());
            data_section.push(0xFF);
            data_section.extend_from_slice(payload);
        }
        data_section.extend_from_slice(&0u32.to_le_bytes());
        let sprite_offs = u32::try_from(1 + data_section.len()).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x00, 0x00]);
        bytes.extend_from_slice(&SIG);
        bytes.extend_from_slice(&sprite_offs.to_le_bytes());
        bytes.push(0x00);
        bytes.extend_from_slice(&data_section);
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let dir = tempfile_dir_with("multi_stat.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("multi_stat.grf", 1));
        apply_newgrf_stations(&mut state, &[&dir]);

        let classes: Vec<_> = state
            .station_class_catalog
            .iter()
            .filter(|c| c.from_newgrf)
            .collect();
        assert_eq!(classes.len(), 2);
        assert_ne!(classes[0].id, classes[1].id);
        assert_ne!(classes[0].short_label, classes[1].short_label);

        let specs: Vec<_> = state
            .station_spec_catalog
            .iter()
            .filter(|s| s.from_newgrf)
            .collect();
        assert_eq!(specs.len(), 2);
        assert_ne!(specs[0].id, specs[1].id);
        assert_eq!(specs[0].class, classes[0].id);
        assert_eq!(specs[1].class, classes[1].id);
        assert_eq!(specs[1].disallowed_platforms, 0b0000_0010);
        assert_eq!(specs[0].short_label, "Alph");
        assert_eq!(specs[1].short_label, "Beta");
    }

    #[test]
    fn parse_station_disallowed_props_0c_0d() {
        let a0 = build_action0_station_payload(b"MODN", b"XXXX", 0b0000_0010, 0b0000_0100, "Plat");
        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.disallowed_platforms, 0b0000_0010);
        assert_eq!(meta.disallowed_lengths, 0b0000_0100);
        assert_eq!(meta.short_label, "Plat");
        assert!(a0.windows(2).any(|w| w == [0x0C, 0b0000_0010]));
        assert!(a0.windows(2).any(|w| w == [0x0D, 0b0000_0100]));
    }

    #[test]
    fn parse_roadtype_max_speed_and_tramtypes_feature() {
        let a0 = build_action0_roadtype_payload_with_speed(b"FAST", false, 2000, 80, "Fast Road");
        let meta = parse_action0_roadtype_meta(&a0).unwrap();
        assert_eq!(meta.max_speed, 80);
        assert_eq!(meta.class, RoadTramType::Road);

        let tram = build_action0_roadtype_payload_with_speed(b"TRAM", true, 1900, 40, "Tram");
        assert_eq!(tram[1], ACTION0_FEATURE_TRAMTYPES);
        let tmeta = parse_action0_roadtype_meta(&tram).unwrap();
        assert_eq!(tmeta.class, RoadTramType::Tram);
        assert_eq!(tmeta.max_speed, 40);
        assert_eq!(tmeta.short_label, "TRAM");

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'F', b'S', 0, 1], "fast", "");
        let dir = tempfile_dir_with("fast.grf", &bytes);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("fast.grf", 1));
        apply_newgrf_road_types(&mut state, &[&dir]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        assert_eq!(def.max_speed, 80);
    }

    #[test]
    fn parse_and_apply_railtype_max_speed_prop_14() {
        let a0 = build_action0_railtype_payload_with_speed(0, b"RAIL", 90);
        let meta = parse_action0_railtype_metas(&a0).unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].max_speed, 90);

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'R', b'S', 0, 1], "railspd", "");
        let dir = tempfile_dir_with("railspd.grf", &bytes);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("railspd.grf", 1));
        apply_newgrf_rail_signals(&mut state, &[&dir]);
        assert_eq!(
            state.runtime.rail_type_max_speed[usize::from(crate::RailType::Rail.as_u8())],
            90
        );
    }

    #[test]
    fn apply_embeds_type_translation_tables_on_road() {
        use crate::newgrf_type_tables::{
            PROP_RAILTYPE_TRANSLATION, build_action0_type_translation_payload,
        };
        const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
        let translation_a0 = build_action0_type_translation_payload(
            PROP_RAILTYPE_TRANSLATION,
            &[*b"ELRL", *b"RAIL"],
        );
        let roadtype_a0 = build_action0_roadtype_payload(b"COBB", false, 1850, "Adoquines");
        let mut action8 = vec![0x08, 0x07];
        action8.extend_from_slice(&[b'T', b'T', 0, 1]);
        action8.extend_from_slice(b"tt\0d\0");
        let mut data_section = Vec::new();
        for payload in [
            translation_a0.as_slice(),
            roadtype_a0.as_slice(),
            action8.as_slice(),
        ] {
            let size = u32::try_from(payload.len()).unwrap();
            data_section.extend_from_slice(&size.to_le_bytes());
            data_section.push(0xFF);
            data_section.extend_from_slice(payload);
        }
        data_section.extend_from_slice(&0u32.to_le_bytes());
        let sprite_offs = u32::try_from(1 + data_section.len()).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x00, 0x00]);
        bytes.extend_from_slice(&SIG);
        bytes.extend_from_slice(&sprite_offs.to_le_bytes());
        bytes.push(0x00);
        bytes.extend_from_slice(&data_section);
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let dir = tempfile_dir_with("ttables.grf", &bytes);
        let mut state = GameState::new(4, 4);
        let grfid = crate::newgrf_config::grfid_from_bytes([b'T', b'T', 0, 1]);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ttables.grf", grfid));
        apply_newgrf_road_types(&mut state, &[&dir]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        assert_eq!(def.newgrf_grfid, grfid);
        let tables = def.newgrf_type_tables.as_ref().unwrap();
        assert_eq!(tables.rail, vec![*b"ELRL", *b"RAIL"]);
    }

    #[test]
    fn parse_roadstop_meta_and_apply_from_bytes() {
        let a0 = build_action0_roadstop_payload(b"BUSC", 0, "Parada bus");
        let meta = parse_action0_roadstop_meta(&a0).unwrap();
        assert_eq!(meta.class_short_label, "BUSC");
        assert_eq!(meta.stop_type, 0);
        assert_eq!(meta.label, "Parada bus");
        assert_eq!(meta.short_label, "Para");

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'R', b'S', 0, 1], "rstop", "");
        let dir = tempfile_dir_with("rstop.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("rstop.grf", 20));
        apply_newgrf_roadstops(&mut state, &[&dir]);
        assert_eq!(state.road_stop_spec_catalog.len(), 1);
        assert!(
            state
                .road_stop_class_catalog
                .iter()
                .any(|c| c.from_newgrf && c.short_label == "BUSC")
        );
        let def = &state.road_stop_spec_catalog[0];
        assert!(def.from_newgrf);
        assert_eq!(def.label, "Parada bus");
        assert_eq!(def.stop_type, 0);
        assert_eq!(def.grfid, 20);
        assert!(def.newgrf_views.is_empty());
    }

    #[test]
    fn parse_badge_meta_and_apply_from_bytes() {
        let a0 = build_action0_badge_payload(b"ELEC", 0x0000_0003, None);
        let meta = parse_action0_badge_meta(&a0).unwrap();
        assert_eq!(meta.label, "ELEC");
        assert_eq!(meta.flags, 3);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'B', b'D', 0, 1], "badge", "");
        let dir = tempfile_dir_with("badge.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("badge.grf", 15));
        apply_newgrf_badges(&mut state, &[&dir]);
        assert_eq!(state.badge_catalog.len(), 1);
        let def = &state.badge_catalog[0];
        assert!(def.from_newgrf);
        assert_eq!(def.label, "ELEC");
        assert_eq!(def.flags, 3);
        assert_eq!(def.grfid, 15);
    }

    #[test]
    fn parse_cargo_two_labels_distinct() {
        let a0_a = build_action0_cargo_payload(0, 1, b"PASS", "Pasajeros");
        let a0_b = build_action0_cargo_payload(1, 2, b"COAL", "Carbón");
        let meta_a = parse_action0_cargo_meta(&a0_a).unwrap();
        let meta_b = parse_action0_cargo_meta(&a0_b).unwrap();
        assert_eq!(meta_a.label, "PASS");
        assert_eq!(meta_b.label, "COAL");
        assert_ne!(meta_a.label, meta_b.label);

        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_a, &a0_b],
            [b'C', b'G', 0, 1],
            "cargos",
            "",
        );
        let dir = tempfile_dir_with("cargos.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("cargos.grf", 11));
        apply_newgrf_cargoes(&mut state, &[&dir]);
        assert_eq!(state.cargo_spec_catalog.len(), 2);
        let labels: Vec<_> = state
            .cargo_spec_catalog
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert!(labels.contains(&"PASS"));
        assert!(labels.contains(&"COAL"));
        assert_ne!(
            state.cargo_spec_catalog[0].label,
            state.cargo_spec_catalog[1].label
        );
    }

    #[test]
    fn parse_object_meta_and_apply_registers() {
        let a0 = build_action0_object_payload(0, b"LIGT", 0x11, "Faro");
        let meta = parse_action0_object_meta(&a0).unwrap();
        assert_eq!(meta.class_label, "LIGT");
        assert_eq!(meta.size, 0x11);
        assert_eq!(meta.name, "Faro");

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'O', b'B', 0, 1], "obj", "");
        let dir = tempfile_dir_with("obj.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("obj.grf", 15));
        apply_newgrf_objects(&mut state, &[&dir]);
        assert_eq!(state.object_spec_catalog.len(), 1);
        let def = &state.object_spec_catalog[0];
        assert!(def.from_newgrf);
        assert_eq!(def.class_label, "LIGT");
        assert_eq!(def.name, "Faro");
        assert_eq!(def.size, 0x11);
        assert_eq!(def.local_id, 0);
    }

    #[test]
    fn parse_and_apply_industry_tiles_allocates_gfx_ge_175() {
        let a0 = build_action0_industry_tile_payload(0, Some(42));
        let meta = parse_action0_industry_tile_meta(&a0).unwrap();
        assert_eq!(meta.subst_id, 0);
        assert_eq!(meta.override_of, Some(42));
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_industry_tile_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'I', b'T', 0, 1],
            "itile",
        );
        let dir = tempfile_dir_with("itile.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("itile.grf", 9));
        apply_newgrf_industry_tiles(&mut state, &[&dir]);
        assert_eq!(state.industry_tile_spec_catalog.len(), 1);
        let def = &state.industry_tile_spec_catalog[0];
        assert!(def.gfx.as_u16() >= crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET);
        assert_eq!(def.subst_id, 0);
        assert!(!def.newgrf_views.is_empty());
        assert_eq!(state.industry_tile_overrides[42], def.gfx.as_u16());
        assert_eq!(
            crate::get_translated_industry_tile_id(42, &state.industry_tile_overrides),
            def.gfx.as_u16()
        );
    }

    fn tempfile_dir_with(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openttdrs_ngr_{}_{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(name), bytes).unwrap();
        dir
    }
}
