//! Inspección parse-only de acciones `NewGRF` (Action0–14) y apply mínimo.
//!
//! El walker cuenta acciones sin aplicar. Action0 features registradas:
//! - `RoadTypes` (0x12) → `GameState.road_type_catalog`
//! - `Stations` (0x04) → `station_class_catalog` / `station_spec_catalog`
//! - `IndustryTiles` (0x09) → `industry_tile_spec_catalog`
//! - `Industries` (0x0A) → `industry_spec_catalog`
//! - `AirportTiles` (0x11) → `airport_tile_spec_catalog`
//! - `Airports` (0x0D) → `airport_spec_catalog`
//! - `Houses` (0x07) → `house_spec_catalog`
//! - `Cargoes` (0x0B) → `cargo_spec_catalog`
//! - `Objects` (0x0F) → `object_spec_catalog`
//! - `RoadStops` (0x14) → `road_stop_class_catalog` / `road_stop_spec_catalog`
//! - `Badges` (0x15) → `badge_catalog`
//! - `Sounds` (0x0C) → `sound_effect_catalog` (samples Action11)
//! - `Canals` (0x05) → `canal_feature_catalog` + Action5 `0x08`
//! - `Bridges` (0x06) → `bridge_spec_catalog` (13 slots in-place)

pub mod action0;
pub mod apply;
pub mod inspect;

// Re-exports públicos
pub use action0::{
    ACTION0_FEATURE_AIRCRAFT, ACTION0_FEATURE_AIRPORTS, ACTION0_FEATURE_AIRPORTTILES,
    ACTION0_FEATURE_BADGES, ACTION0_FEATURE_BRIDGES, ACTION0_FEATURE_CANALS,
    ACTION0_FEATURE_CARGOES, ACTION0_FEATURE_HOUSES, ACTION0_FEATURE_INDUSTRIES,
    ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_OBJECTS, ACTION0_FEATURE_RAILTYPES,
    ACTION0_FEATURE_ROAD_VEHICLES, ACTION0_FEATURE_ROADSTOPS, ACTION0_FEATURE_ROADTYPES,
    ACTION0_FEATURE_SHIPS, ACTION0_FEATURE_SOUNDS, ACTION0_FEATURE_STATIONS,
    ACTION0_FEATURE_TRAINS, ACTION0_FEATURE_TRAMTYPES, Action0Header, ParsedAirportLayout,
    ParsedAirportLayoutTile, ParsedAirportMeta, ParsedAirportTileMeta, ParsedBadgeMeta,
    ParsedBridgeMeta, ParsedCanalMeta, ParsedCargoMeta, ParsedHouseMeta, ParsedIndustryMeta,
    ParsedIndustryTileMeta, ParsedObjectMeta, ParsedRailTypeMeta, ParsedRoadStopMeta,
    ParsedRoadTypeMeta, ParsedSoundMeta, ParsedStationMeta, ParsedTrainMeta, ParsedVehicleMeta,
    collect_airport_metas_from_grf, collect_airport_tile_metas_from_grf,
    collect_badge_metas_from_grf, collect_bridge_metas_from_grf, collect_canal_metas_from_grf,
    collect_cargo_metas_from_grf, collect_house_metas_from_grf, collect_industry_metas_from_grf,
    collect_industry_tile_metas_from_grf, collect_object_metas_from_grf,
    collect_railtype_metas_from_grf, collect_roadstop_metas_from_grf,
    collect_roadtype_metas_from_grf, collect_sound_metas_from_grf, collect_station_metas_from_grf,
    collect_train_metas_from_grf, collect_vehicle_metas_from_grf, for_each_pseudo_payload,
    parse_action0_airport_meta, parse_action0_airport_tile_meta, parse_action0_badge_meta,
    parse_action0_bridge_meta, parse_action0_canal_meta, parse_action0_cargo_meta,
    parse_action0_header, parse_action0_house_meta, parse_action0_industry_meta,
    parse_action0_industry_tile_meta, parse_action0_object_meta, parse_action0_railtype_metas,
    parse_action0_roadstop_meta, parse_action0_roadtype_meta, parse_action0_sound_meta,
    parse_action0_station_meta, parse_action0_train_meta, parse_action0_vehicle_metas,
};

pub use apply::{
    action5::{
        apply_newgrf_action5_airport_preview, apply_newgrf_action5_airport_preview_default_dirs,
        apply_newgrf_action5_all_default_dirs, apply_newgrf_action5_bridge_decks,
        apply_newgrf_action5_bridge_decks_default_dirs, apply_newgrf_action5_canals,
        apply_newgrf_action5_canals_default_dirs, apply_newgrf_action5_catenary,
        apply_newgrf_action5_catenary_default_dirs, apply_newgrf_action5_foundations,
        apply_newgrf_action5_foundations_default_dirs, apply_newgrf_action5_oneway,
        apply_newgrf_action5_oneway_default_dirs, apply_newgrf_action5_openttd_gui,
        apply_newgrf_action5_openttd_gui_default_dirs, apply_newgrf_action5_roadstops,
        apply_newgrf_action5_roadstops_default_dirs, apply_newgrf_action5_shore,
        apply_newgrf_action5_shore_default_dirs, apply_newgrf_action5_signals,
        apply_newgrf_action5_signals_default_dirs,
    },
    airport::{
        apply_newgrf_airport_tiles, apply_newgrf_airport_tiles_default_dirs, apply_newgrf_airports,
        apply_newgrf_airports_default_dirs,
    },
    apply_newgrf_stack_catalogs_default_dirs,
    badges::{apply_newgrf_badges, apply_newgrf_badges_default_dirs},
    bridges::{apply_newgrf_bridges, apply_newgrf_bridges_default_dirs},
    canals::{apply_newgrf_canals, apply_newgrf_canals_default_dirs},
    cargo::{apply_newgrf_cargoes, apply_newgrf_cargoes_default_dirs},
    houses::{apply_newgrf_houses, apply_newgrf_houses_default_dirs},
    industry::{
        apply_newgrf_industries, apply_newgrf_industries_default_dirs, apply_newgrf_industry_tiles,
        apply_newgrf_industry_tiles_default_dirs,
    },
    objects::{apply_newgrf_objects, apply_newgrf_objects_default_dirs},
    rail::{apply_newgrf_rail_signals, apply_newgrf_rail_signals_default_dirs},
    road::{apply_newgrf_road_types, apply_newgrf_road_types_default_dirs},
    roadstop::{apply_newgrf_roadstops, apply_newgrf_roadstops_default_dirs},
    sounds::{apply_newgrf_sounds, apply_newgrf_sounds_default_dirs},
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
    build_action0_railtype_payload_full(local_id, label, max_speed, 0, 0, &[], &[])
}

/// Action0 `RailTypes` con speed, coste y listas compatible/powered.
#[must_use]
pub fn build_action0_railtype_payload_full(
    local_id: u8,
    label: &[u8; 4],
    max_speed: u16,
    cost_multiplier: u16,
    flags: u8,
    compatible: &[[u8; 4]],
    powered: &[[u8; 4]],
) -> Vec<u8> {
    let mut num_props = 2u8; // 08 + 14
    if cost_multiplier > 0 {
        num_props += 1;
    }
    if flags > 0 {
        num_props += 1;
    }
    if !compatible.is_empty() {
        num_props += 1;
    }
    if !powered.is_empty() {
        num_props += 1;
    }
    let mut payload = vec![
        0x00,
        ACTION0_FEATURE_RAILTYPES,
        num_props,
        0x01,
        local_id,
        0x08,
    ];
    payload.extend_from_slice(label);
    if !compatible.is_empty() {
        payload.push(0x0E);
        payload.push(u8::try_from(compatible.len()).unwrap_or(0));
        for l in compatible {
            payload.extend_from_slice(l);
        }
    }
    if !powered.is_empty() {
        payload.push(0x0F);
        payload.push(u8::try_from(powered.len()).unwrap_or(0));
        for l in powered {
            payload.extend_from_slice(l);
        }
    }
    if flags > 0 {
        payload.push(0x10);
        payload.push(flags);
    }
    if cost_multiplier > 0 {
        payload.push(0x13);
        payload.extend_from_slice(&cost_multiplier.to_le_bytes());
    }
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
    spec_short: &[u8; 4],
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    name: &str,
) -> Vec<u8> {
    build_action0_station_payload_with_callback_mask(
        class_label,
        spec_short,
        disallowed_platforms,
        disallowed_lengths,
        0,
        name,
    )
}

/// Fixture Action0 `Stations` con la máscara de callbacks `0x0B` opcional.
#[must_use]
pub fn build_action0_station_payload_with_callback_mask(
    class_label: &[u8; 4],
    _spec_short: &[u8; 4],
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    callback_mask: u8,
    name: &str,
) -> Vec<u8> {
    // IDs OpenTTD 15.3: 0x0C/0x0D = disallowed; short label se deriva del nombre.
    let num_props = 0x04 + u8::from(callback_mask != 0);
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_STATIONS,
        num_props,
        0x01,
        0x00,
        0x08, // PROP_LABEL
    ];
    p.extend_from_slice(class_label);
    if callback_mask != 0 {
        p.push(0x0B); // PROP_STATION_CALLBACK_MASK
        p.push(callback_mask);
    }
    p.push(0x0C); // PROP_STATION_DISALLOWED_PLATFORMS
    p.push(disallowed_platforms);
    p.push(0x0D); // PROP_STATION_DISALLOWED_LENGTHS
    p.push(disallowed_lengths);
    p.push(0xFE); // PROP_NAME_CSTRING
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

/// Fixture Action0 `Stations` con propiedades de animación `0x13` y `0x16`–`0x18`.
#[must_use]
#[allow(clippy::too_many_arguments)] // Fixture: conserva explícitos los campos wire-format de Action0.
pub fn build_action0_station_payload_with_animation(
    class_label: &[u8; 4],
    spec_short: &[u8; 4],
    callback_mask: u8,
    flags: u8,
    animation_frames: u8,
    animation_status: u8,
    animation_speed: u8,
    animation_triggers: u16,
    name: &str,
) -> Vec<u8> {
    let mut p = build_action0_station_payload_with_callback_mask(
        class_label,
        spec_short,
        0,
        0,
        callback_mask,
        name,
    );
    // Mantener las propiedades antes de la cadena 0xFE para que el fixture
    // siga el orden usual de Action0, aunque el parser admite ambos órdenes.
    let name_property_len = name.len() + 2; // id 0xFE + C-string terminada en NUL
    let insertion = p.len().saturating_sub(name_property_len);
    let tail = p.split_off(insertion);
    p[2] = p[2].saturating_add(4);
    p.extend_from_slice(&[
        0x13,
        flags,
        0x16,
        animation_frames,
        animation_status,
        0x17,
        animation_speed,
        0x18,
    ]);
    p.extend_from_slice(&animation_triggers.to_le_bytes());
    p.extend(tail);
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
    build_action0_industry_tile_payload_ex(0, subst_id, override_of, &[], 0)
}

/// Action0 `AirportTiles` (`0x11`) con subst (+ override/callback opcionales).
#[must_use]
pub fn build_action0_airport_tile_payload(
    local_id: u8,
    subst_id: u8,
    override_of: Option<u8>,
    callback_mask: u8,
) -> Vec<u8> {
    let mut num_props = 1u8;
    if override_of.is_some() {
        num_props += 1;
    }
    if callback_mask != 0 {
        num_props += 1;
    }
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_AIRPORTTILES,
        num_props,
        0x01,
        local_id,
        0x08,
        subst_id,
    ];
    if let Some(o) = override_of {
        p.push(0x09);
        p.push(o);
    }
    if callback_mask != 0 {
        p.push(0x0E);
        p.push(callback_mask);
    }
    p
}

/// Action0 `Airports` (`0x0D`) con layout 0xFE + nombre C-string.
///
/// `layout_tiles`: `(x, y, local_tile_id)`.
#[must_use]
pub fn build_action0_airport_payload(
    local_id: u8,
    subst_ottd: u8,
    layout_tiles: &[(i8, i8, u16)],
    catchment: u8,
    noise: u8,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_AIRPORTS,
        0x05, // 08, 0A, 0E, 0F, 10
        0x01,
        local_id,
        0x08,
        subst_ottd,
        0x0A,
        1, // num_layouts
    ];
    p.extend_from_slice(&0u32.to_le_bytes()); // size dword
    p.push(0); // rotation NORTH
    for &(x, y, local_tile) in layout_tiles {
        p.push(x.cast_unsigned());
        p.push(y.cast_unsigned());
        p.push(0xFE);
        p.extend_from_slice(&local_tile.to_le_bytes());
    }
    p.push(0);
    p.push(0x80); // terminator
    p.push(0x0E);
    p.push(catchment);
    p.push(0x0F);
    p.push(noise);
    p.push(0x10);
    p.extend_from_slice(&0xFEu16.to_le_bytes());
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

/// Action0 `IndustryTiles` con acceptance / `callback_mask`.
///
/// `acceptance`: pares `(cargo_idx, acceptance_amt)` para props `0x0A`…
#[must_use]
pub fn build_action0_industry_tile_payload_ex(
    local_id: u8,
    subst_id: u8,
    override_of: Option<u8>,
    acceptance: &[(u8, u8)],
    callback_mask: u8,
) -> Vec<u8> {
    let mut num_props = 1u8; // subst
    if override_of.is_some() {
        num_props += 1;
    }
    num_props += u8::try_from(acceptance.len().min(3)).unwrap_or(0);
    if callback_mask != 0 {
        num_props += 1;
    }
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_INDUSTRYTILES,
        num_props,
        0x01,
        local_id,
        0x08,
        subst_id,
    ];
    if let Some(o) = override_of {
        p.push(0x09);
        p.push(o);
    }
    for (i, &(cargo, amt)) in acceptance.iter().take(3).enumerate() {
        p.push(0x0A + u8::try_from(i).unwrap_or(0));
        p.push(cargo);
        p.push(amt);
    }
    if callback_mask != 0 {
        p.push(0x0E);
        p.push(callback_mask);
    }
    p
}

/// Action0 `Industries` (`0x0A`) con layout, cargos y `callback_mask`.
///
/// `layout_tiles`: `(x, y, local_tile_id)` — siempre `0xFE` + WORD local.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_industry_payload(
    local_id: u8,
    subst_id: u8,
    override_of: Option<u8>,
    layout_tiles: &[(i8, i8, u16)],
    produced: &[u8],
    accepted: &[u8],
    production_rates: &[u8],
    callback_mask: u16,
    name: &str,
) -> Vec<u8> {
    let mut num_props = 3u8; // 08, 0A, FE
    if override_of.is_some() {
        num_props += 1;
    }
    if !produced.is_empty() {
        num_props += 1; // 25
    }
    if !accepted.is_empty() {
        num_props += 1; // 26
    }
    if !production_rates.is_empty() {
        num_props += 1; // 27
    }
    if callback_mask != 0 {
        num_props += 2; // 21 + 22
    }
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_INDUSTRIES,
        num_props,
        0x01,
        local_id,
        0x08,
        subst_id,
    ];
    if let Some(o) = override_of {
        p.push(0x09);
        p.push(o);
    }
    // Layout 0x0A
    p.push(0x0A);
    p.push(1); // num_layouts
    let mut layout_body = Vec::new();
    for &(x, y, local_tile) in layout_tiles {
        layout_body.push(x.cast_unsigned());
        layout_body.push(y.cast_unsigned());
        layout_body.push(0xFE);
        layout_body.extend_from_slice(&local_tile.to_le_bytes());
    }
    layout_body.extend_from_slice(&[0x00, 0x80]); // terminator
    let def_size = u32::try_from(layout_body.len()).unwrap_or(0);
    p.extend_from_slice(&def_size.to_le_bytes());
    p.extend_from_slice(&layout_body);
    if !produced.is_empty() {
        p.push(0x25);
        p.push(u8::try_from(produced.len()).unwrap_or(0));
        p.extend_from_slice(produced);
    }
    if !accepted.is_empty() {
        p.push(0x26);
        p.push(u8::try_from(accepted.len()).unwrap_or(0));
        p.extend_from_slice(accepted);
    }
    if !production_rates.is_empty() {
        p.push(0x27);
        p.push(u8::try_from(production_rates.len()).unwrap_or(0));
        p.extend_from_slice(production_rates);
    }
    if callback_mask != 0 {
        p.push(0x21);
        p.push((callback_mask & 0xFF) as u8);
        p.push(0x22);
        p.push((callback_mask >> 8) as u8);
    }
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

/// Action0 `Houses` (`0x07`): subst, flags, years, availability, probability, nombre.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_house_payload(
    local_id: u8,
    subst: u8,
    flags: u8,
    min_year: u32,
    max_year: u32,
    availability: u16,
    probability: u8,
    name: &str,
) -> Vec<u8> {
    build_action0_house_payload_ex(
        local_id,
        subst,
        flags,
        min_year,
        max_year,
        availability,
        probability,
        None,
        0,
        name,
    )
}

/// Action0 Houses con override y `callback_mask` opcionales.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_house_payload_ex(
    local_id: u8,
    subst: u8,
    flags: u8,
    min_year: u32,
    max_year: u32,
    availability: u16,
    probability: u8,
    override_of: Option<u8>,
    callback_mask: u16,
    name: &str,
) -> Vec<u8> {
    let lo_year = min_year.saturating_sub(1920).min(255) as u8;
    let hi_year = if max_year >= crate::house_spec::HOUSE_YEAR_MAX {
        0xFFu8
    } else {
        max_year.saturating_sub(1920).min(255) as u8
    };
    let mut num_props = 7u8; // 08,09,0A,0B,13,18,FE
    if override_of.is_some() {
        num_props += 1;
    }
    if callback_mask != 0 {
        num_props += 2; // 14 + 1D
    }
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_HOUSES,
        num_props,
        0x01,
        local_id,
        0x08,
        subst,
        0x09,
        flags,
        0x0A,
        lo_year,
        hi_year,
        0x0B,
        40, // population default (pool de expansión)
        0x13,
    ];
    p.extend_from_slice(&availability.to_le_bytes());
    p.push(0x18);
    p.push(probability);
    if let Some(o) = override_of {
        p.push(0x15);
        p.push(o);
    }
    if callback_mask != 0 {
        p.push(0x14);
        p.push((callback_mask & 0xFF) as u8);
        p.push(0x1D);
        p.push((callback_mask >> 8) as u8);
    }
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_roadstop_payload(
    class_label: &[u8; 4],
    stop_type: u8,
    name: &str,
    badge_labels: &[[u8; 4]],
) -> Vec<u8> {
    build_action0_roadstop_payload_ex(
        class_label,
        stop_type,
        name,
        badge_labels,
        crate::road_stop_spec::ROADSTOP_DRAW_MODE_DEFAULT,
        0,
    )
}

/// Action0 `RoadStops` con `0x0C` `draw_mode` y `0x12` flags.
#[must_use]
pub fn build_action0_roadstop_payload_ex(
    class_label: &[u8; 4],
    stop_type: u8,
    name: &str,
    badge_labels: &[[u8; 4]],
    draw_mode: u8,
    flags: u32,
) -> Vec<u8> {
    build_action0_roadstop_payload_with_callback_mask(
        class_label,
        stop_type,
        name,
        badge_labels,
        draw_mode,
        flags,
        0,
    )
}

/// Action0 `RoadStops` con máscara de callbacks `0x11`.
///
/// La máscara se emite incluso cuando sólo se usa el callback de disponibilidad
/// (`bit 0` / `CBID_STATION_AVAILABILITY`) para que los fixtures cubran el
/// camino Action0 → Action2/3 → construcción.
#[must_use]
pub fn build_action0_roadstop_payload_with_callback_mask(
    class_label: &[u8; 4],
    stop_type: u8,
    name: &str,
    badge_labels: &[[u8; 4]],
    draw_mode: u8,
    flags: u32,
    callback_mask: u8,
) -> Vec<u8> {
    let num_props = 6 + u8::from(!badge_labels.is_empty());
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_ROADSTOPS,
        num_props,
        0x01,
        0x00,
        0x08, // PROP_LABEL
    ];
    p.extend_from_slice(class_label);
    p.push(0x09); // PROP_ROADSTOP_STOP_TYPE
    p.push(stop_type);
    p.push(0x0C); // PROP_ROADSTOP_DRAW_MODE
    p.push(draw_mode);
    p.push(0x12); // PROP_ROADSTOP_FLAGS
    p.extend_from_slice(&flags.to_le_bytes());
    p.push(0x11); // RoadStopCallbackMask
    p.push(callback_mask);
    p.push(0xFE); // PROP_NAME_CSTRING
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    append_badge_association_prop(&mut p, badge_labels);
    p
}

/// Action0 `RoadStops` con propiedades de animación `0x0E`/`0x0F`/`0x10`.
///
/// Se usa en fixtures para comprobar el recorrido Action0 → catálogo →
/// scheduler `CB140/141/142`; los campos se agregan después de las cadenas,
/// que sigue siendo un orden válido de propiedades Action0.
#[must_use]
#[allow(clippy::too_many_arguments)] // Fixture: mantiene explícitos los campos wire-format de Action0.
pub fn build_action0_roadstop_payload_with_animation(
    class_label: &[u8; 4],
    stop_type: u8,
    name: &str,
    callback_mask: u8,
    animation_frames: u8,
    animation_status: u8,
    animation_speed: u8,
    animation_triggers: u16,
) -> Vec<u8> {
    let mut payload = build_action0_roadstop_payload_with_callback_mask(
        class_label,
        stop_type,
        name,
        &[],
        crate::road_stop_spec::ROADSTOP_DRAW_MODE_DEFAULT,
        0,
        callback_mask,
    );
    payload[2] = payload[2].saturating_add(3);
    payload.extend_from_slice(&[
        0x0E,
        animation_frames,
        animation_status,
        0x0F,
        animation_speed,
    ]);
    payload.push(0x10);
    payload.extend_from_slice(&animation_triggers.to_le_bytes());
    payload
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
    build_action0_cargo_payload_full(local_id, bitnum, label, name, 0, 0, 0, 0, false, 0, 0x100)
}

/// Action0 `Cargoes` con pagos / freight / capacity multiplier.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_cargo_payload_full(
    local_id: u8,
    bitnum: u8,
    label: &[u8; 4],
    name: &str,
    weight: u8,
    initial_payment: u32,
    transit_fast: u8,
    transit_slow: u8,
    is_freight: bool,
    classes: u16,
    capacity_multiplier: u16,
) -> Vec<u8> {
    build_action0_cargo_payload_with_callback_mask(
        local_id,
        bitnum,
        label,
        name,
        weight,
        initial_payment,
        transit_fast,
        transit_slow,
        is_freight,
        classes,
        capacity_multiplier,
        0,
    )
}

/// Action0 `Cargoes` con máscara de callbacks BYTE (`0x1A`).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_cargo_payload_with_callback_mask(
    local_id: u8,
    bitnum: u8,
    label: &[u8; 4],
    name: &str,
    weight: u8,
    initial_payment: u32,
    transit_fast: u8,
    transit_slow: u8,
    is_freight: bool,
    classes: u16,
    capacity_multiplier: u16,
    callback_mask: u8,
) -> Vec<u8> {
    // 08 bitnum, 0F weight, 10/11 transit, 12 payment, 15 freight, 16 classes,
    // 17 label, 1D multiplier, 1A callback mask, FE name = 10/11 props.
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_CARGOES,
        0x0A + u8::from(callback_mask != 0),
        0x01,
        local_id,
        0x08,
        bitnum,
        0x0F,
        weight,
        0x10,
        transit_fast,
        0x11,
        transit_slow,
        0x12,
    ];
    p.extend_from_slice(&initial_payment.to_le_bytes());
    p.push(0x15);
    p.push(u8::from(is_freight));
    p.push(0x16);
    p.extend_from_slice(&classes.to_le_bytes());
    p.push(0x17);
    p.extend_from_slice(label);
    p.push(0x1D);
    p.extend_from_slice(&capacity_multiplier.to_le_bytes());
    if callback_mask != 0 {
        p.push(0x1A);
        p.push(callback_mask);
    }
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
    badge_labels: &[[u8; 4]],
) -> Vec<u8> {
    build_action0_object_payload_full(
        local_id,
        class_label,
        size,
        crate::object_spec::DEFAULT_OBJECT_CLIMATE_MASK,
        crate::object_spec::DEFAULT_OBJECT_BUILD_COST_FACTOR,
        name,
        badge_labels,
    )
}

/// Action0 Objects con climate (`0x0B`) y cost factor (`0x0D`).
#[must_use]
pub fn build_action0_object_payload_full(
    local_id: u8,
    class_label: &[u8; 4],
    size: u8,
    climate_mask: u8,
    cost_factor: u8,
    name: &str,
    badge_labels: &[[u8; 4]],
) -> Vec<u8> {
    build_action0_object_payload_with_callback_mask(
        local_id,
        class_label,
        size,
        climate_mask,
        cost_factor,
        0,
        name,
        badge_labels,
    )
}

/// Action0 Objects con máscara de callbacks WORD (`0x15`).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action0_object_payload_with_callback_mask(
    local_id: u8,
    class_label: &[u8; 4],
    size: u8,
    climate_mask: u8,
    cost_factor: u8,
    callback_mask: u16,
    name: &str,
    badge_labels: &[[u8; 4]],
) -> Vec<u8> {
    let num_props = 5 + u8::from(callback_mask != 0) + u8::from(!badge_labels.is_empty());
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_OBJECTS,
        num_props,
        0x01,
        local_id,
        0x08, // class label
    ];
    p.extend_from_slice(class_label);
    p.push(0x0B); // climate
    p.push(climate_mask);
    p.push(0x0C); // size
    p.push(size);
    p.push(0x0D); // build cost multiplier
    p.push(cost_factor);
    if callback_mask != 0 {
        p.push(0x15); // callback mask (WORD)
        p.extend_from_slice(&callback_mask.to_le_bytes());
    }
    p.push(0xFE);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    append_badge_association_prop(&mut p, badge_labels);
    p
}

/// Extensión local `0xFD`: BYTE count + N× label 4 chars.
fn append_badge_association_prop(p: &mut Vec<u8>, badge_labels: &[[u8; 4]]) {
    if badge_labels.is_empty() {
        return;
    }
    let count = u8::try_from(badge_labels.len()).unwrap_or(u8::MAX);
    p.push(0xFD);
    p.push(count);
    for label in badge_labels.iter().take(usize::from(count)) {
        p.extend_from_slice(label);
    }
}

/// Action0 `Sounds` (`0x0C`): volume `0x08`, priority `0x09`, override opcional `0x0A`.
#[must_use]
pub fn build_action0_sound_payload(
    local_id: u8,
    volume: u8,
    priority: u8,
    override_old: Option<u8>,
) -> Vec<u8> {
    let num_props = if override_old.is_some() { 3u8 } else { 2u8 };
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_SOUNDS,
        num_props,
        0x01,
        local_id,
        0x08,
        volume,
        0x09,
        priority,
    ];
    if let Some(old) = override_old {
        p.push(0x0A);
        p.push(old);
    }
    p
}

/// Action0 `Canals` (`0x05`): `callback_mask` `0x08`, flags `0x09`.
#[must_use]
pub fn build_action0_canal_payload(local_id: u8, callback_mask: u8, flags: u8) -> Vec<u8> {
    vec![
        0x00,
        ACTION0_FEATURE_CANALS,
        0x02,
        0x01,
        local_id,
        0x08,
        callback_mask,
        0x09,
        flags,
    ]
}

/// Action0 `Bridges` (`0x06`): year/min/max/price/speed + nombre `0xFE` opcional.
#[must_use]
pub fn build_action0_bridge_payload(
    local_id: u8,
    year: u8,
    min_len: u8,
    max_len: u8,
    price: u8,
    speed: u16,
    name: &str,
) -> Vec<u8> {
    let num_props = if name.is_empty() { 5u8 } else { 6u8 };
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_BRIDGES,
        num_props,
        0x01,
        local_id,
        0x08,
        year,
        0x09,
        min_len,
        0x0A,
        max_len,
        0x0B,
        price,
        0x0C,
    ];
    p.extend_from_slice(&speed.to_le_bytes());
    if !name.is_empty() {
        p.push(0xFE);
        p.extend_from_slice(name.as_bytes());
        p.push(0);
    }
    p
}

/// Payload Action11 fixture: `0x11`, count, luego N× (`WORD` size LE + PCM).
#[must_use]
pub fn build_action11_sounds_payload(samples: &[&[u8]]) -> Vec<u8> {
    let count = u8::try_from(samples.len()).unwrap_or(u8::MAX);
    let mut p = vec![0x11, count];
    for sample in samples.iter().take(usize::from(count)) {
        let size = u16::try_from(sample.len()).unwrap_or(u16::MAX);
        p.extend_from_slice(&size.to_le_bytes());
        p.extend_from_slice(&sample[..usize::from(size)]);
    }
    p
}

/// GRF v2 con Action11 samples + Action0 Sounds + Action8 (fixtures #254).
#[must_use]
pub fn build_grf_v2_with_action11_sounds_and_action0(
    samples: &[&[u8]],
    action0s: &[&[u8]],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let sounds_payload = build_action11_sounds_payload(samples);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0); // description vacío

    let mut data_section = Vec::new();
    for payload in std::iter::once(sounds_payload.as_slice()).chain(action0s.iter().copied()) {
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        let mut a0 = build_action0_train_payload(1955, 120, 1500, "Locomotora NewGRF");
        a0[4] = 7;
        let meta = parse_action0_train_meta(&a0).unwrap();
        assert_eq!(meta.local_id, 7);
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
            assert_eq!(eng.newgrf_local_id, u16::from(action0[4]));
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
    fn vehicle_action0_preserves_sound_effect_callback_mask() {
        let road = [
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x01,
            0x01,
            0x00,
            0x17,
            0x80,
        ];
        assert_eq!(
            parse_action0_vehicle_metas(&road).unwrap()[0].callback_mask,
            0x80
        );

        let ship = [
            0x00,
            ACTION0_FEATURE_SHIPS,
            0x02,
            0x01,
            0x00,
            0x12,
            0x80,
            0x22,
            0x01,
        ];
        assert_eq!(
            parse_action0_vehicle_metas(&ship).unwrap()[0].callback_mask,
            0x180
        );

        let aircraft = [
            0x00,
            ACTION0_FEATURE_AIRCRAFT,
            0x02,
            0x01,
            0x00,
            0x14,
            0x80,
            0x22,
            0x01,
        ];
        assert_eq!(
            parse_action0_vehicle_metas(&aircraft).unwrap()[0].callback_mask,
            0x180
        );

        let train = [
            0x00,
            ACTION0_FEATURE_TRAINS,
            0x02,
            0x01,
            0x00,
            0x1E,
            0x80,
            0x31,
            0x01,
        ];
        assert_eq!(
            parse_action0_train_meta(&train).unwrap().callback_mask,
            0x180
        );

        let train_stack = [0x00, ACTION0_FEATURE_TRAINS, 0x01, 0x01, 0x00, 0x27, 0x80];
        assert!(parse_action0_train_meta(&train_stack).unwrap().sprite_stack);
        let road_stack = [
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x01,
            0x01,
            0x00,
            0x1C,
            0x80,
        ];
        assert!(parse_action0_vehicle_metas(&road_stack).unwrap()[0].sprite_stack);
    }

    #[test]
    fn vehicle_action0_preserves_visual_effect_for_each_ground_feature() {
        let train = [0x00, ACTION0_FEATURE_TRAINS, 0x01, 0x01, 0x00, 0x22, 0xFF];
        // VE_DEFAULT se normaliza como en `UpdateVisualEffect`: bit de
        // desactivación conservado, bits de tipo limpios.
        assert_eq!(
            parse_action0_train_meta(&train).unwrap().visual_effect,
            0xCF
        );

        let road = [
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x01,
            0x01,
            0x00,
            0x21,
            0x20,
        ];
        assert_eq!(
            parse_action0_vehicle_metas(&road).unwrap()[0].visual_effect,
            0x20
        );

        let ship = [0x00, ACTION0_FEATURE_SHIPS, 0x01, 0x01, 0x00, 0x1C, 0x40];
        assert_eq!(
            parse_action0_vehicle_metas(&ship).unwrap()[0].visual_effect,
            0x40
        );
    }

    #[test]
    fn vehicle_action0_reads_extended_first_local_id() {
        let road = [
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x02,
            0x01,
            0xFF,
            0xD2,
            0x04,
            0x09,
            100,
            0x0F,
            40,
            0,
        ];
        let metas = parse_action0_vehicle_metas(&road).expect("extended Action0 should parse");
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].local_id, 1234);
        assert_eq!(metas[0].capacity, 40);
    }

    #[test]
    fn extended_vehicle_local_id_survives_catalog_application() {
        let action0 = [
            0x00,
            ACTION0_FEATURE_ROAD_VEHICLES,
            0x00,
            0x01,
            0xFF,
            0xD2,
            0x04,
        ];
        let filename = "vehicle-extended.grf";
        let bytes =
            build_grf_v2_with_action0_and_action8(&action0, *b"EXID", "extended vehicle", "");
        let dir = tempfile_dir_with(filename, &bytes);
        let mut state = GameState::new(4, 4);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            filename,
            u32::from_be_bytes(*b"EXID"),
        ));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let engine = state
            .engine_catalog
            .iter()
            .find(|candidate| candidate.from_newgrf)
            .expect("extended vehicle should enter catalog");
        assert_eq!(engine.newgrf_local_id, 1234);
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
    fn parse_and_apply_station_animation_props_13_16_17_18() {
        let callback_mask = crate::STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK
            | crate::STATION_CALLBACK_ANIMATION_SPEED_MASK;
        let triggers =
            crate::STATION_ANIMATION_TRIGGER_BUILT | crate::STATION_ANIMATION_TRIGGER_TILE_LOOP;
        let a0 = build_action0_station_payload_with_animation(
            b"ANIM",
            b"Spec",
            callback_mask,
            crate::STATION_FLAG_CB141_RANDOM_BITS,
            7,
            1,
            3,
            triggers,
            "Andén animado",
        );
        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.callback_mask, callback_mask);
        assert_eq!(meta.flags, crate::STATION_FLAG_CB141_RANDOM_BITS);
        assert_eq!(meta.animation_frames, 7);
        assert_eq!(meta.animation_status, 1);
        assert_eq!(meta.animation_speed, 3);
        assert_eq!(meta.animation_triggers, triggers);

        let bytes =
            build_grf_v2_with_action0_and_action8(&a0, [b'A', b'N', 0, 1], "station-animation", "");
        let dir = tempfile_dir_with("station_animation.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            "station_animation.grf",
            0x414E_0001,
        ));
        apply_newgrf_stations(&mut state, &[&dir]);
        let spec = state
            .station_spec_catalog
            .iter()
            .find(|spec| spec.from_newgrf)
            .unwrap();
        assert_eq!(spec.animation_frames, 7);
        assert!(spec.animation_loops());
        assert!(spec.has_animation_next_frame_callback());
        assert!(spec.has_animation_speed_callback());
        assert!(spec.animation_next_frame_uses_random_bits());
    }

    #[test]
    fn station_animation_props_survive_legacy_layout_and_intermediate_props() {
        let a0_base = build_action0_station_payload_with_animation(
            b"ANIM",
            b"Spec",
            crate::STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK,
            0,
            5,
            1,
            2,
            crate::STATION_ANIMATION_TRIGGER_TILE_LOOP,
            "Legacy layout",
        );
        let mut a0 = a0_base;
        // Action0 suele declarar 0x09 antes de las props de animación. El
        // layout de cuatro ceros es el atajo vanilla de OpenTTD para una
        // tesela sin secuencia y permite comprobar que el parser lo salta.
        let before_later_props = 5 + 1 + 4; // header + `0x08` + class label
        let skipped_props = [
            0x09, 0x01, 0, 0, 0, 0, // un legacy layout vacío
            0x10, 0, 0, // cargo threshold WORD
            0x11, 0, // pylons
            0x12, 0, 0, 0, 0, // cargo triggers DWORD
            0x14, 0, // overhead wires
            0x15, 0, // blocked tiles
        ];
        a0[2] = a0[2].saturating_add(6);
        a0.splice(before_later_props..before_later_props, skipped_props);

        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.animation_frames, 5);
        assert_eq!(meta.animation_status, 1);
        assert_eq!(meta.animation_speed, 2);
        assert_eq!(
            meta.animation_triggers,
            crate::STATION_ANIMATION_TRIGGER_TILE_LOOP
        );
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
        let a0 = build_action0_station_payload_with_callback_mask(
            b"MODN",
            b"XXXX",
            0b0000_0010,
            0b0000_0100,
            1,
            "Plat",
        );
        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.disallowed_platforms, 0b0000_0010);
        assert_eq!(meta.disallowed_lengths, 0b0000_0100);
        assert_eq!(meta.callback_mask, 1);
        assert_eq!(meta.short_label, "Plat");
        assert!(a0.windows(2).any(|w| w == [0x0B, 1]));
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
        let mut a0 = build_action0_roadstop_payload_ex(
            b"BUSC",
            0,
            "Parada bus",
            &[],
            0x03,
            crate::ROADSTOP_FLAG_DRIVE_THROUGH_ONLY | crate::ROADSTOP_FLAG_ROAD_ONLY,
        );
        // Action0 0x0D: PASS (0) y GOOD (5) son los cargos que habilitan la
        // re-randomización del RoadStop.
        a0[2] = a0[2].saturating_add(1);
        a0.push(0x0D);
        a0.extend_from_slice(&((1_u32 << 0) | (1_u32 << 5)).to_le_bytes());
        let meta = parse_action0_roadstop_meta(&a0).unwrap();
        assert_eq!(meta.class_short_label, "BUSC");
        assert_eq!(meta.stop_type, 0);
        assert_eq!(meta.label, "Parada bus");
        assert_eq!(meta.short_label, "Para");
        assert_eq!(meta.draw_mode, 0x03);
        assert_eq!(
            meta.flags,
            crate::ROADSTOP_FLAG_DRIVE_THROUGH_ONLY | crate::ROADSTOP_FLAG_ROAD_ONLY
        );
        assert_eq!(meta.callback_mask, 0);
        assert_eq!(meta.random_cargo_triggers, (1 << 0) | (1 << 5));
        assert!(meta.badge_labels.is_empty());

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'R', b'S', 0, 1], "rstop", "");
        let dir = tempfile_dir_with("rstop.grf", &bytes);
        let mut state = GameState::new(4, 4);
        let mut entry = crate::NewGrfEntry::new("rstop.grf", 20);
        entry.grf_version = 8;
        state.newgrf_stack.push(entry);
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
        assert_eq!(def.newgrf_local_id, 0);
        assert_eq!(def.newgrf_grf_version, 8);
        assert_eq!(def.draw_mode, 0x03);
        assert!(
            def.cargo_triggers_randomisation(
                crate::CargoType::Passengers,
                crate::Climate::Temperate
            )
        );
        assert!(
            def.cargo_triggers_randomisation(crate::CargoType::Goods, crate::Climate::Temperate)
        );
        assert!(def.drive_through_only());
        assert!(def.road_only());
        assert_eq!(def.callback_mask, 0);
        assert!(def.newgrf_views.is_empty());
        assert!(def.newgrf_runtime.is_none());
        assert!(def.associated_badges.is_empty());
    }

    #[test]
    fn roadstop_animation_properties_survive_action0_and_catalog_apply() {
        let a0 = build_action0_roadstop_payload_with_animation(
            b"ANIM",
            0,
            "Parada animada",
            crate::ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME
                | crate::ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED,
            7,
            1,
            3,
            crate::ROADSTOP_ANIMATION_TRIGGER_BUILT | crate::ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP,
        );
        let meta = parse_action0_roadstop_meta(&a0).unwrap();
        assert_eq!(meta.animation_frames, 7);
        assert_eq!(meta.animation_status, 1);
        assert_eq!(meta.animation_speed, 3);
        assert_eq!(
            meta.animation_triggers,
            crate::ROADSTOP_ANIMATION_TRIGGER_BUILT | crate::ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP
        );

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'A', b'N', 0, 1], "anim", "");
        let dir = tempfile_dir_with("anim.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("anim.grf", 77));
        apply_newgrf_roadstops(&mut state, &[&dir]);
        let def = &state.road_stop_spec_catalog[0];
        assert_eq!(def.animation_frames, 7);
        assert!(def.animation_loops());
        assert_eq!(def.animation_speed, 3);
        assert!(def.has_animation_next_frame_callback());
        assert!(def.has_animation_speed_callback());
    }

    /// El `CB13` de `RoadStops` no puede quedarse sólo como un bit parseado: el
    /// mismo GRF que lo declara debe bloquear la query y el execute del comando.
    #[test]
    fn roadstop_availability_callback_is_loaded_and_blocks_construction() {
        use crate::{Command, CommandError, apply_command, command_would_fail};

        let a0 = build_action0_roadstop_payload_with_callback_mask(
            b"CBRS",
            0,
            "Parada controlada",
            &[],
            crate::ROADSTOP_DRAW_MODE_DEFAULT,
            0,
            crate::ROADSTOP_CALLBACK_MASK_AVAILABILITY,
        );
        let action2 = crate::newgrf_sprites::build_action2_callback_literal_payload(
            ACTION0_FEATURE_ROADSTOPS,
            0x21,
            0,
        );
        let bytes = crate::newgrf_sprites::build_grf_v2_feature_with_action2_chain(
            &a0,
            ACTION0_FEATURE_ROADSTOPS,
            0,
            0x21,
            &action2,
            1,
            1,
            &[1],
            *b"CBRS",
            "roadstop-callback",
        );
        let dir = tempfile_dir_with("roadstop-callback.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            "roadstop-callback.grf",
            crate::newgrf_config::grfid_from_bytes(*b"CBRS"),
        ));
        apply_newgrf_roadstops(&mut state, &[&dir]);

        let spec = state.road_stop_spec_catalog[0].id;
        assert!(state.road_stop_spec_catalog[0].has_availability_callback());
        assert!(state.road_stop_spec_catalog[0].newgrf_runtime.is_some());
        apply_command(&mut state, &Command::SetCurrentRoadStopSpec(spec)).unwrap();

        let stop = crate::TileCoord::new(3, 3);
        apply_command(&mut state, &Command::PlaceRoad(crate::TileCoord::new(3, 2))).unwrap();
        assert_eq!(
            command_would_fail(&state, &Command::PlaceBusStop(stop, 3)),
            Some(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(
            apply_command(&mut state, &Command::PlaceBusStop(stop, 3)),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(state.stations.len(), 0);
    }

    #[test]
    fn roadstop_built_trigger_registers_newgrf_animation() {
        use crate::{Command, apply_command};

        let a0 = build_action0_roadstop_payload_with_animation(
            b"ANCB",
            0,
            "Parada animada",
            0,
            2,
            1,
            0,
            crate::ROADSTOP_ANIMATION_TRIGGER_BUILT,
        );
        // El mismo Action2 devuelve FE para CB140: ActivateAnimation sin
        // fijar frame. No se activa CB13 porque su bit no está en la máscara.
        let action2 = crate::newgrf_sprites::build_action2_callback_literal_payload(
            ACTION0_FEATURE_ROADSTOPS,
            0x22,
            0xFE,
        );
        let bytes = crate::newgrf_sprites::build_grf_v2_feature_with_action2_chain(
            &a0,
            ACTION0_FEATURE_ROADSTOPS,
            0,
            0x22,
            &action2,
            1,
            1,
            &[1],
            *b"ANCB",
            "roadstop-animation",
        );
        let dir = tempfile_dir_with("roadstop-animation.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            "roadstop-animation.grf",
            crate::newgrf_config::grfid_from_bytes(*b"ANCB"),
        ));
        apply_newgrf_roadstops(&mut state, &[&dir]);
        let spec = state.road_stop_spec_catalog[0].id;
        apply_command(&mut state, &Command::SetCurrentRoadStopSpec(spec)).unwrap();
        apply_command(&mut state, &Command::PlaceRoad(crate::TileCoord::new(3, 2))).unwrap();
        apply_command(
            &mut state,
            &Command::PlaceBusStop(crate::TileCoord::new(3, 3), 3),
        )
        .unwrap();

        assert!(state.stations[0].road_stop_animation_active);
        assert_eq!(state.stations[0].road_stop_animation_frame, 0);
        // El scheduler se integra en la fase real `AnimateAnimatedTiles`, no
        // sólo en el helper: sin CB141 usa el frame Action0 de fallback.
        state.step();
        assert_eq!(state.stations[0].road_stop_animation_frame, 1);
    }

    #[test]
    fn apply_roadstops_multi_grf_two_specs() {
        let a0_a = build_action0_roadstop_payload(b"AAA ", 0, "Stop A", &[]);
        let a0_b = build_action0_roadstop_payload_ex(
            b"BBB ",
            1,
            "Stop B",
            &[],
            0x01,
            crate::ROADSTOP_FLAG_TRAM_ONLY,
        );
        let bytes_a = build_grf_v2_with_action0_and_action8(&a0_a, [b'R', b'A', 0, 1], "ra", "");
        let bytes_b = build_grf_v2_with_action0_and_action8(&a0_b, [b'R', b'B', 0, 1], "rb", "");
        let dir_a = tempfile_dir_with("ra.grf", &bytes_a);
        let dir_b = tempfile_dir_with("rb.grf", &bytes_b);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ra.grf", 100));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("rb.grf", 200));
        apply_newgrf_roadstops(&mut state, &[&dir_a, &dir_b]);
        assert_eq!(state.road_stop_spec_catalog.len(), 2);
        let a = state
            .road_stop_spec_catalog
            .iter()
            .find(|d| d.grfid == 100)
            .unwrap();
        let b = state
            .road_stop_spec_catalog
            .iter()
            .find(|d| d.grfid == 200)
            .unwrap();
        assert_eq!(a.label, "Stop A");
        assert_eq!(a.stop_type, 0);
        assert_eq!(a.newgrf_local_id, 0);
        assert_eq!(b.label, "Stop B");
        assert_eq!(b.stop_type, 1);
        assert!(b.tram_only());
        assert_ne!(a.id, b.id);
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
    fn parse_badge_two_labels_distinct_no_collision() {
        let a0_a = build_action0_badge_payload(b"ELEC", 1, None);
        let a0_b = build_action0_badge_payload(b"DIES", 2, Some("Diésel"));
        let meta_a = parse_action0_badge_meta(&a0_a).unwrap();
        let meta_b = parse_action0_badge_meta(&a0_b).unwrap();
        assert_eq!(meta_a.label, "ELEC");
        assert_eq!(meta_b.label, "Diésel");
        assert_ne!(meta_a.label, meta_b.label);

        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_a, &a0_b],
            [b'B', b'D', 0, 2],
            "badges2",
            "",
        );
        let dir = tempfile_dir_with("badges2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("badges2.grf", 15));
        apply_newgrf_badges(&mut state, &[&dir]);
        assert_eq!(state.badge_catalog.len(), 2);
        assert_ne!(state.badge_catalog[0].label, state.badge_catalog[1].label);
        assert_ne!(state.badge_catalog[0].id, state.badge_catalog[1].id);
        assert_eq!(crate::list_badges(&state.badge_catalog, "").len(), 2);
    }

    #[test]
    fn apply_badge_association_to_roadstop_and_object() {
        let a0_badge = build_action0_badge_payload(b"ELEC", 0, None);
        let a0_stop = build_action0_roadstop_payload(b"BUSC", 0, "Parada", &[*b"ELEC"]);
        let a0_obj = build_action0_object_payload(0, b"LIGT", 0x11, "Faro", &[*b"ELEC"]);
        let meta_stop = parse_action0_roadstop_meta(&a0_stop).unwrap();
        assert_eq!(meta_stop.badge_labels, vec!["ELEC".to_string()]);
        let meta_obj = parse_action0_object_meta(&a0_obj).unwrap();
        assert_eq!(meta_obj.badge_labels, vec!["ELEC".to_string()]);

        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_badge, &a0_stop, &a0_obj],
            [b'B', b'A', 0, 1],
            "assoc",
            "",
        );
        let dir = tempfile_dir_with("assoc.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("assoc.grf", 42));
        apply_newgrf_badges(&mut state, &[&dir]);
        apply_newgrf_roadstops(&mut state, &[&dir]);
        apply_newgrf_objects(&mut state, &[&dir]);

        assert_eq!(state.badge_catalog.len(), 1);
        let badge_id = state.badge_catalog[0].id;
        assert_eq!(
            state.road_stop_spec_catalog[0].associated_badges,
            vec![badge_id]
        );
        assert_eq!(
            state.object_spec_catalog[0].associated_badges,
            vec![badge_id]
        );
        assert_eq!(
            crate::badges_for_spec(
                &state.object_spec_catalog[0].associated_badges,
                &state.badge_catalog
            )
            .len(),
            1
        );
    }

    #[test]
    fn apply_invalid_badge_label_skips_association() {
        let a0_badge = build_action0_badge_payload(b"ELEC", 0, None);
        let a0_stop = build_action0_roadstop_payload(b"BUSC", 0, "Parada", &[*b"NOPE", *b"ELEC"]);
        let a0_obj = build_action0_object_payload(0, b"LIGT", 0x11, "Faro", &[*b"MISS"]);
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_badge, &a0_stop, &a0_obj],
            [b'B', b'X', 0, 1],
            "badlbl",
            "",
        );
        let dir = tempfile_dir_with("badlbl.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("badlbl.grf", 7));
        apply_newgrf_badges(&mut state, &[&dir]);
        apply_newgrf_roadstops(&mut state, &[&dir]);
        apply_newgrf_objects(&mut state, &[&dir]);

        let badge_id = state.badge_catalog[0].id;
        assert_eq!(
            state.road_stop_spec_catalog[0].associated_badges,
            vec![badge_id]
        );
        assert!(state.object_spec_catalog[0].associated_badges.is_empty());
        assert!(
            state
                .runtime
                .newgrf_diagnostics
                .iter()
                .any(|d| d.contains("NOPE") || d.contains("desconocido")),
            "diagnósticos: {:?}",
            state.runtime.newgrf_diagnostics
        );
        assert!(
            state
                .runtime
                .newgrf_diagnostics
                .iter()
                .any(|d| d.contains("MISS")),
            "diagnósticos: {:?}",
            state.runtime.newgrf_diagnostics
        );
    }

    #[test]
    fn merge_same_badge_label_across_grfs_no_collision() {
        let a0_a = build_action0_badge_payload(b"ELEC", 1, None);
        let a0_b = build_action0_badge_payload(b"elec", 9, None);
        let bytes_a = build_grf_v2_with_action0_and_action8(&a0_a, [b'B', b'1', 0, 1], "ba", "");
        let bytes_b = build_grf_v2_with_action0_and_action8(&a0_b, [b'B', b'2', 0, 2], "bb", "");
        let shared =
            std::env::temp_dir().join(format!("openttdrs_ngr_merge_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&shared);
        std::fs::write(shared.join("ba.grf"), &bytes_a).unwrap();
        std::fs::write(shared.join("bb.grf"), &bytes_b).unwrap();

        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ba.grf", 0x4231_0001));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("bb.grf", 0x4232_0002));
        apply_newgrf_badges(&mut state, &[&shared]);
        assert_eq!(state.badge_catalog.len(), 1, "mismo label → un BadgeDef");
        assert_eq!(state.badge_catalog[0].grfid, 0x4231_0001);
        assert_eq!(state.badge_catalog[0].flags, 9); // último gana flags
        assert!(state.badge_catalog[0].label.eq_ignore_ascii_case("elec"));
    }

    #[test]
    fn badge_identity_prefers_fe_cstring_over_08() {
        // FE primero, luego 08 — debe ganar FE.
        let mut p = vec![0x00, ACTION0_FEATURE_BADGES, 0x03, 0x01, 0x00];
        p.push(0xFE);
        p.extend_from_slice(b"Electric\0");
        p.push(0x08);
        p.extend_from_slice(b"XXXX");
        p.push(0x09);
        p.extend_from_slice(&7u32.to_le_bytes());
        let meta = parse_action0_badge_meta(&p).unwrap();
        assert_eq!(meta.label, "Electric");
        assert_eq!(meta.flags, 7);
    }

    #[test]
    fn truncated_badge_list_emits_diagnostics_and_inspect_warning() {
        let a0_badge = build_action0_badge_payload(b"ELEC", 0, None);
        // Roadstop con 0xFD count=2 pero sólo 4 bytes (1 label) → truncado.
        let mut a0_stop = build_action0_roadstop_payload(b"BUSC", 0, "Parada", &[]);
        // Sustituir num_props e inyectar 0xFD truncado.
        a0_stop[2] = a0_stop[2].saturating_add(1);
        a0_stop.push(0xFD);
        a0_stop.push(2); // pide 2 labels
        a0_stop.extend_from_slice(b"ELEC"); // sólo 1

        let meta = parse_action0_roadstop_meta(&a0_stop).unwrap();
        assert!(
            meta.badge_list_error
                .as_ref()
                .is_some_and(|e| e.contains("truncad"))
        );
        assert_eq!(meta.badge_labels, vec!["ELEC".to_string()]);

        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_badge, &a0_stop],
            [b'B', b'T', 0, 1],
            "trunc",
            "",
        );
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("truncad") || w.contains("0xFD")),
            "warnings: {:?}",
            report.warnings
        );
        assert!(report.badge_labels.iter().any(|l| l == "ELEC"));

        let dir = tempfile_dir_with("trunc.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("trunc.grf", 99));
        apply_newgrf_badges(&mut state, &[&dir]);
        apply_newgrf_roadstops(&mut state, &[&dir]);
        assert!(
            state
                .runtime
                .newgrf_diagnostics
                .iter()
                .any(|d| d.contains("truncad") || d.contains("0xFD")),
            "diagnostics: {:?}",
            state.runtime.newgrf_diagnostics
        );
    }

    #[test]
    fn badge_associations_survive_save_load_and_reapply() {
        let a0_badge = build_action0_badge_payload(b"ELEC", 0, None);
        let a0_stop = build_action0_roadstop_payload(b"BUSC", 0, "Parada", &[*b"ELEC"]);
        let a0_obj = build_action0_object_payload(0, b"LIGT", 0x11, "Faro", &[*b"ELEC"]);
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_badge, &a0_stop, &a0_obj],
            [b'B', b'S', 0, 1],
            "svbdg",
            "",
        );
        let dir = tempfile_dir_with("svbdg.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("svbdg.grf", 0x4253_0001));
        apply_newgrf_badges(&mut state, &[&dir]);
        apply_newgrf_roadstops(&mut state, &[&dir]);
        apply_newgrf_objects(&mut state, &[&dir]);
        let badge_id = state.badge_catalog[0].id;
        assert_eq!(state.badge_catalog[0].grfid, 0x4253_0001);
        assert_eq!(
            state.road_stop_spec_catalog[0].associated_badges,
            vec![badge_id]
        );

        let json = state.save_json().expect("save");
        let mut loaded = GameState::load_json(&json).expect("load");
        assert_eq!(loaded.badge_catalog.len(), 1);
        assert_eq!(loaded.badge_catalog[0].grfid, 0x4253_0001);
        assert_eq!(
            loaded.road_stop_spec_catalog[0].associated_badges,
            vec![badge_id]
        );
        assert_eq!(
            loaded.object_spec_catalog[0].associated_badges,
            vec![badge_id]
        );

        // Reaplicar catálogos desde el GRF: asociaciones deben resolverse de nuevo.
        apply_newgrf_badges(&mut loaded, &[&dir]);
        apply_newgrf_roadstops(&mut loaded, &[&dir]);
        apply_newgrf_objects(&mut loaded, &[&dir]);
        let badge_id2 = loaded.badge_catalog[0].id;
        assert_eq!(
            loaded.road_stop_spec_catalog[0].associated_badges,
            vec![badge_id2]
        );
        assert_eq!(
            loaded.object_spec_catalog[0].associated_badges,
            vec![badge_id2]
        );
    }

    #[test]
    fn inspect_lists_badge_labels_and_associations() {
        let a0_badge = build_action0_badge_payload(b"ELEC", 0, Some("Electric"));
        let a0_stop = build_action0_roadstop_payload(b"BUSC", 0, "Parada", &[*b"ELEC"]);
        let a0_obj = build_action0_object_payload(0, b"LIGT", 0x11, "Faro", &[*b"ELEC"]);
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_badge, &a0_stop, &a0_obj],
            [b'B', b'I', 0, 1],
            "insp",
            "",
        );
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert!(
            report.badge_labels.iter().any(|l| l == "Electric"),
            "labels: {:?}",
            report.badge_labels
        );
        assert!(
            report.badge_associations.iter().any(|a| a.contains("ELEC")),
            "assoc: {:?}",
            report.badge_associations
        );
        let summary = report.format_summary();
        assert!(summary.contains("Badges:"));
        assert!(summary.contains("Badge assoc:"));
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
        let a0 = build_action0_object_payload_with_callback_mask(
            0,
            b"LIGT",
            0x12,
            0x05,
            7,
            crate::OBJECT_CALLBACK_SLOPE_CHECK_MASK,
            "Faro",
            &[],
        );
        let meta = parse_action0_object_meta(&a0).unwrap();
        assert_eq!(meta.class_label, "LIGT");
        assert_eq!(meta.size, 0x12);
        assert_eq!(meta.climate_mask, 0x05);
        assert_eq!(meta.build_cost_factor, 7);
        assert_eq!(meta.callback_mask, crate::OBJECT_CALLBACK_SLOPE_CHECK_MASK);
        assert_eq!(meta.name, "Faro");
        assert!(meta.badge_labels.is_empty());

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
        assert_eq!(def.id, crate::object_spec::NEW_OBJECT_OFFSET);
        assert_eq!(def.class_label, "LIGT");
        assert_eq!(def.name, "Faro");
        assert_eq!(def.size, 0x12);
        assert_eq!(def.climate_mask, 0x05);
        assert_eq!(def.build_cost_factor, 7);
        assert_eq!(def.callback_mask, crate::OBJECT_CALLBACK_SLOPE_CHECK_MASK);
        assert!(def.has_slope_check_callback());
        assert_eq!(def.local_id, 0);
        assert!(def.associated_badges.is_empty());
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

    /// #255: sin `NewGRF`, Action5 señales no altera vanilla (todos los slots `None`).
    #[test]
    fn signals_ac_no_grf_leaves_action5_empty() {
        let mut state = GameState::new(4, 4);
        apply_newgrf_action5_signals(&mut state, &[]);
        assert!(
            state
                .runtime
                .signal_action5_newgrf_sprites
                .iter()
                .all(Option::is_none)
        );
        assert!(state.runtime.rail_signal_newgrf.iter().all(Option::is_none));
    }

    /// #255: Action5 custom llena su slot; vanilla `OpenGFX` fuera de rango intacto.
    #[test]
    fn signals_ac_action5_custom_without_clobbering_other_slots() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_action5_with_sprite(
            0x04,
            5,
            8,
            8,
            &indices,
            [b'S', b'G', 0, 5],
            "sig5ac",
        );
        let dir = tempfile_dir_with("sig5ac.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sig5ac.grf", 5));
        apply_newgrf_action5_signals(&mut state, &[&dir]);
        assert_eq!(state.runtime.signal_action5_newgrf_sprites.len(), 240);
        assert!(state.runtime.signal_action5_newgrf_sprites[5].is_some());
        assert!(state.runtime.signal_action5_newgrf_sprites[0].is_none());
        assert!(state.runtime.signal_action5_newgrf_sprites[12].is_none());
    }

    /// #255: orientación × rojo/verde × PBS (path) vía Action3 `RailTypes`.
    #[test]
    fn signals_ac_orientation_red_green_pbs_and_fallback_slot() {
        let action0 = build_action0_railtype_payload(0, b"RAIL");
        let red = vec![10u8; 16 * 24];
        let green = vec![200u8; 16 * 24];
        let bytes = crate::newgrf_sprites::build_grf_v2_railtype_signal_sprites(
            &action0,
            0,
            16,
            24,
            &red,
            &green,
            [b'S', b'I', 0, 2],
            "sigac",
        );
        let dir = tempfile_dir_with("sigac.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sigac.grf", 0x5349_0002));
        apply_newgrf_rail_signals(&mut state, &[&dir]);
        let spec = state.runtime.rail_signal_newgrf[0]
            .as_ref()
            .expect("RAIL signals");
        // PBS path = signal_type 4; block = 0; semaphore variant = 1.
        for (sig_type, variant) in [(0u8, 0u8), (4u8, 0u8), (4u8, 1u8)] {
            for image in [0u8, 3u8, 7u8] {
                let mut red_ctx = crate::newgrf_sprites::Action2EvalCtx::default();
                let r = spec
                    .resolve_sprite(image, sig_type, variant, false, &mut red_ctx)
                    .expect("red");
                let mut green_ctx = crate::newgrf_sprites::Action2EvalCtx::default();
                let g = spec
                    .resolve_sprite(image, sig_type, variant, true, &mut green_ctx)
                    .expect("green");
                assert_ne!(r.rgba, g.rgba, "type={sig_type} var={variant} img={image}");
                assert_eq!(
                    red_ctx.vars.get(&0x18),
                    Some(&((u32::from(sig_type) << 16) | (u32::from(variant) << 8)))
                );
            }
        }
        // Fallback Action5: slot mapping sigue válido aunque no haya sprite en 0.
        assert_eq!(crate::signal_action5_slot(5088), Some(0));
    }

    /// #255: tipo/variante de señal persisten en m2 tras save/load.
    #[test]
    fn signals_ac_type_variant_survive_save_load() {
        use crate::rail_signals::{
            SIGTYPE_PATH, SignalTrack, signal_type_for_track, signal_variant_for_track,
        };
        use crate::{Command, apply_command};
        let mut state = GameState::new(8, 8);
        let c = crate::map::TileCoord::new(2, 2);
        apply_command(
            &mut state,
            &Command::PlaceRailBits(c, crate::map::RAIL_TB_X),
        )
        .unwrap();
        apply_command(
            &mut state,
            &Command::PlaceRailSignalWithVariant(c, 0, 128, 128, SIGTYPE_PATH, 1),
        )
        .unwrap();

        let json = state.save_json().expect("save");
        let loaded = GameState::load_json(&json).expect("load");
        let t = loaded.map.get(c).unwrap();
        assert_eq!(signal_type_for_track(t.m2, SignalTrack::X), SIGTYPE_PATH);
        assert_eq!(signal_variant_for_track(t.m2, SignalTrack::X), 1);
    }

    /// #263: rail props compatible/powered/cost + road/tram IDs separados.
    #[test]
    fn types_ac_rail_road_tram_labels_compat_and_cost() {
        let rail_a0 = build_action0_railtype_payload_full(
            0,
            b"RAIL",
            120,
            16, // ×2 coste
            0x01,
            &[*b"ELRL"],
            &[*b"RAIL", *b"ELRL"],
        );
        let road_a0 = build_action0_roadtype_payload_with_speed(b"COBB", false, 1850, 60, "Cobble");
        let tram_a0 = build_action0_roadtype_payload_with_speed(b"TRAM", true, 1900, 40, "Tram NG");
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&rail_a0, &road_a0, &tram_a0],
            [b'T', b'Y', 0, 1],
            "types",
            "",
        );
        let dir = tempfile_dir_with("types.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("types.grf", 0x5459_0001));
        apply_newgrf_rail_signals(&mut state, &[&dir]);
        apply_newgrf_road_types(&mut state, &[&dir]);

        let rail_props = state.runtime.rail_type_props[0];
        assert_eq!(rail_props.max_speed, 120);
        assert_eq!(rail_props.cost_multiplier, 16);
        assert_ne!(rail_props.compatible_mask, 0);
        assert_ne!(rail_props.powered_mask, 0);
        assert!(crate::rail_types_compatible_with_props(
            crate::RailType::Rail,
            crate::RailType::Electric,
            &state.runtime.rail_type_props,
        ));

        let roads: Vec<_> = state
            .road_type_catalog
            .iter()
            .filter(|d| d.from_newgrf)
            .collect();
        assert_eq!(roads.len(), 2);
        let road = roads.iter().find(|d| d.short_label == "COBB").unwrap();
        let tram = roads.iter().find(|d| d.short_label == "TRAM").unwrap();
        assert_ne!(road.id, tram.id);
        assert_eq!(road.class, crate::RoadTramType::Road);
        assert_eq!(tram.class, crate::RoadTramType::Tram);
        assert!(tram.from_tramtypes_feature);
        assert!(!road.from_tramtypes_feature);
        assert_eq!(road.max_speed, 60);
        assert_eq!(tram.max_speed, 40);

        // Coste rail ×2 vs default.
        let base = crate::economy::rail_build_cost(&state.global_economy);
        let factored = crate::economy::rail_build_cost_factored(&state.global_economy, 16);
        assert_eq!(factored, base * 2);
    }

    /// #263: Action3 sprite types underlay/signals resuelven grupo o fallback vacío.
    #[test]
    fn types_ac_rail_sprite_types_resolve_or_fallback() {
        let action0 = build_action0_railtype_payload(1, b"ELRL");
        let red = vec![1u8; 8 * 8];
        let green = vec![2u8; 8 * 8];
        let bytes = crate::newgrf_sprites::build_grf_v2_railtype_signal_sprites(
            &action0,
            1,
            8,
            8,
            &red,
            &green,
            [b'T', b'S', 0, 3],
            "rst",
        );
        let dir = tempfile_dir_with("rst.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("rst.grf", 3));
        apply_newgrf_rail_signals(&mut state, &[&dir]);
        let idx = usize::from(crate::RailType::Electric.as_u8());
        assert!(state.runtime.rail_signal_newgrf[idx].is_some());
        // Sin underlay/overlay en el fixture → fallback None (OpenGFX).
        assert!(state.runtime.rail_type_underlay_newgrf[idx].is_none());
        assert!(state.runtime.rail_type_overlay_newgrf[idx].is_none());
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        assert!(
            state.runtime.rail_signal_newgrf[idx]
                .as_ref()
                .unwrap()
                .resolve_group(0, &mut ctx)
                .is_some()
        );
    }

    /// #264: dos cargos, pagos/capacidad/textos y traducción local/global estable.
    #[test]
    fn cargoes_ac_identity_payment_capacity_and_inspect() {
        let a0_pass = build_action0_cargo_payload_full(
            0,
            0,
            b"PASS",
            "Pax Custom",
            1,
            5000,
            1,
            20,
            false,
            1,
            0x200,
        );
        let a0_coal = build_action0_cargo_payload_full(
            1,
            1,
            b"COAL",
            "Carbón Custom",
            16,
            8000,
            5,
            100,
            true,
            2,
            0x100,
        );
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[&a0_pass, &a0_coal],
            [b'C', b'G', 0, 2],
            "cargoac",
            "",
        );
        let dir = tempfile_dir_with("cargoac.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("cargoac.grf", 0x4347_0002));
        apply_newgrf_cargoes(&mut state, &[&dir]);
        assert_eq!(state.cargo_spec_catalog.len(), 2);
        let pass = crate::cargo_spec_by_label(&state.cargo_spec_catalog, "PASS").unwrap();
        let coal = crate::cargo_spec_by_label(&state.cargo_spec_catalog, "COAL").unwrap();
        assert_eq!(pass.name, "Pax Custom");
        assert_eq!(coal.name, "Carbón Custom");
        assert_eq!(pass.grfid, 0x4347_0002);
        assert_ne!(pass.label, coal.label);

        let pay = crate::payment_spec_for_cargo(crate::CargoType::Coal, &state.cargo_spec_catalog);
        assert_eq!(pay.base_rate, 8000);
        assert_eq!(
            crate::apply_cargo_capacity_multiplier(
                40,
                &state.cargo_spec_catalog,
                crate::CargoType::Passengers
            ),
            80
        );
        assert_eq!(
            crate::cargo_spec_display_name(crate::CargoType::Passengers, &state.cargo_spec_catalog),
            "Pax Custom"
        );

        let vanilla = crate::transported_goods_income(
            10,
            8,
            4,
            crate::CargoType::Coal,
            state.global_economy.inflation_payment,
        );
        let custom = crate::transported_goods_income_with_spec(
            10,
            8,
            4,
            pay,
            state.global_economy.inflation_payment,
        );
        assert_ne!(vanilla, custom);

        let report = inspect_grf_bytes(&bytes).unwrap();
        assert!(report.cargo_labels.iter().any(|l| l.contains("PASS")));
        assert!(report.cargo_labels.iter().any(|l| l.contains("COAL")));

        // Re-apply tras “load”: misma traducción local→label.
        apply_newgrf_cargoes(&mut state, &[&dir]);
        assert_eq!(
            crate::cargo_spec_by_label(&state.cargo_spec_catalog, "pass")
                .unwrap()
                .name,
            "Pax Custom"
        );
    }

    /// CB39 debe venir del Action2 de un GRF cargado y reemplazar el pago
    /// durante la descarga real de un packet, no limitarse a quedar parseado.
    #[test]
    fn cargo_profit_callback_runs_from_loaded_action2_graph_during_delivery() {
        use crate::newgrf_sprites::{
            build_action2_variational_payload, build_grf_v2_feature_with_action2_chain,
        };
        use crate::test_fixtures::SimHarness;
        use crate::{CargoType, Station, TileCoord, Vehicle, VehicleKind};

        let action0 = build_action0_cargo_payload_with_callback_mask(
            0,
            1,
            b"COAL",
            "Carbón CB39",
            16,
            8192,
            0,
            0,
            true,
            0,
            0x100,
            crate::CARGO_CALLBACK_PROFIT_CALC_MASK,
        );
        // var18 >> 16 = cantidad del packet; el resultado es el multiplicador
        // firmado de 15 bits del CB39. Con cuatro unidades, devuelve cuatro.
        let action2 = build_action2_variational_payload(
            ACTION0_FEATURE_CARGOES,
            7,
            0x18,
            16,
            u8::MAX,
            &[],
            0,
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_CARGOES,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            *b"CP39",
            "cargo-profit-callback",
        );
        let dir = tempfile_dir_with("cargo-profit-callback.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            "cargo-profit-callback.grf",
            crate::newgrf_config::grfid_from_bytes(*b"CP39"),
        ));
        apply_newgrf_cargoes(&mut state, &[&dir]);
        {
            let spec = crate::cargo_spec_by_label(&state.cargo_spec_catalog, "COAL").unwrap();
            assert!(spec.has_profit_calc_callback());
            assert!(spec.newgrf_runtime.is_some());
        }

        let destination = TileCoord::new(3, 3);
        let source = TileCoord::new(0, 0);
        state.stations.push(Station::new(destination));
        let mut truck = Vehicle::new(0, VehicleKind::Truck, destination, destination);
        truck.cargo = 4;
        truck.cargo_type = Some(CargoType::Coal);
        truck.mark_cargo_loaded(source);
        truck.ensure_packets_from_legacy();
        state.vehicles.push(truck);

        let payment_spec =
            crate::payment_spec_for_cargo(CargoType::Coal, &state.cargo_spec_catalog);
        let current_payment =
            crate::cargo_current_payment(payment_spec, state.global_economy.inflation_payment);
        let expected = 4_i64 * 4 * current_payment / 8192;
        let vanilla = crate::transported_goods_income_with_spec(
            4,
            6,
            0,
            payment_spec,
            state.global_economy.inflation_payment,
        );
        assert_ne!(
            expected, vanilla,
            "el callback debe sustituir la fórmula base"
        );

        SimHarness::until_vehicle_cargo(&mut state, 0, 0, 8);
        assert_eq!(
            state.stations[0].income,
            u64::try_from(expected).unwrap(),
            "CB39 debe controlar el ingreso de la entrega"
        );
    }

    /// CB145 debe usar la máscara de cargo y el Action2 cargado durante el
    /// barrido periódico de rating, no quedarse como metadato del catálogo.
    #[test]
    fn cargo_station_rating_callback_runs_from_loaded_action2_graph_during_station_sweep() {
        use crate::newgrf_sprites::{
            build_action2_variational_payload, build_grf_v2_feature_with_action2_chain,
        };
        use crate::{CargoType, Station, TileCoord, VehicleKind};

        let action0 = build_action0_cargo_payload_with_callback_mask(
            0,
            1,
            b"COAL",
            "Carbón CB145",
            16,
            8192,
            0,
            0,
            true,
            0,
            0x100,
            crate::CARGO_CALLBACK_STATION_RATING_CALC_MASK,
        );
        // var10 contiene el tipo histórico: un bus debe verse como road vehicle
        // 0x11 y devolver el target 17, no el algoritmo vanilla.
        let action2 =
            build_action2_variational_payload(ACTION0_FEATURE_CARGOES, 7, 0x10, 0, u8::MAX, &[], 0);
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_CARGOES,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            *b"CR45",
            "cargo-station-rating-callback",
        );
        let dir = tempfile_dir_with("cargo-station-rating-callback.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state.newgrf_stack.push(crate::NewGrfEntry::new(
            "cargo-station-rating-callback.grf",
            crate::newgrf_config::grfid_from_bytes(*b"CR45"),
        ));
        apply_newgrf_cargoes(&mut state, &[&dir]);
        assert!(
            crate::cargo_spec_by_label(&state.cargo_spec_catalog, "COAL")
                .unwrap()
                .has_station_rating_callback()
        );

        let mut station = Station::new(TileCoord::new(3, 3));
        station.add_waiting_cargo(CargoType::Coal, 10);
        station.goods.get_mut(CargoType::Coal).rating = 10;
        station.goods.get_mut(CargoType::Coal).last_speed = 100;
        station.last_vehicle_type = Some(VehicleKind::Bus);
        state.stations.push(station);
        let sweep = u64::from(crate::economy::STATION_RATING_TICKS);
        while state.tick.get() < sweep {
            state.step();
        }

        assert_eq!(
            crate::station_rating_for_cargo(&state.stations[0], CargoType::Coal),
            12,
            "CB145 debe recibir var10=0x11 para bus y conservar convergencia ±2"
        );
    }

    #[test]
    fn sounds_ac_register_and_play() {
        let pcm: &[u8] = &[0x10, 0x20, 0x30, 0x40];
        let a0 = build_action0_sound_payload(0, 64, 10, None);
        let bytes = build_grf_v2_with_action11_sounds_and_action0(
            &[pcm],
            &[&a0],
            [b'S', b'F', 0, 1],
            "sfx",
        );
        let dir = tempfile_dir_with("sfx.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sfx.grf", 0x5346_0001));
        apply_newgrf_sounds(&mut state, &[&dir]);

        let def = crate::sound_effect_def(&state.sound_effect_catalog, 0x5346_0001, 0).unwrap();
        assert!(def.has_sample);
        assert_eq!(def.volume, 64);
        assert_eq!(def.priority, 10);
        assert_eq!(def.sample_pcm, pcm);
        assert!((crate::effective_volume(def) - 0.5).abs() < f32::EPSILON);

        assert!(crate::play_newgrf_sound(&mut state, 0x5346_0001, 0).is_ok());
        assert_eq!(state.runtime.pending_newgrf_sounds.len(), 1);
        assert_eq!(state.runtime.pending_newgrf_sounds[0].grfid, 0x5346_0001);
        assert_eq!(state.runtime.pending_newgrf_sounds[0].local_id, 0);
        assert!((state.runtime.pending_newgrf_sounds[0].volume - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.runtime.pending_newgrf_sounds[0].priority, 10);

        let report = inspect_grf_bytes(&bytes).unwrap();
        assert!(report.action0_features.contains(&ACTION0_FEATURE_SOUNDS));
        assert!(report.sound_local_ids.contains(&0));
    }

    #[test]
    fn sounds_ac_two_grfs_isolate_local_ids() {
        let pcm_a: &[u8] = &[1, 2, 3];
        let pcm_b: &[u8] = &[9, 8, 7, 6];
        let a0_a = build_action0_sound_payload(0, 32, 1, None);
        let a0_b = build_action0_sound_payload(0, 96, 2, None);
        let bytes_a = build_grf_v2_with_action11_sounds_and_action0(
            &[pcm_a],
            &[&a0_a],
            [b'S', b'A', 0, 1],
            "sa",
        );
        let bytes_b = build_grf_v2_with_action11_sounds_and_action0(
            &[pcm_b],
            &[&a0_b],
            [b'S', b'B', 0, 2],
            "sb",
        );
        let dir_a = tempfile_dir_with("sa.grf", &bytes_a);
        let dir_b = tempfile_dir_with("sb.grf", &bytes_b);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sa.grf", 0x5341_0001));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sb.grf", 0x5342_0002));
        apply_newgrf_sounds(&mut state, &[&dir_a, &dir_b]);

        assert_eq!(state.sound_effect_catalog.len(), 2);
        let a = crate::sound_effect_def(&state.sound_effect_catalog, 0x5341_0001, 0).unwrap();
        let b = crate::sound_effect_def(&state.sound_effect_catalog, 0x5342_0002, 0).unwrap();
        assert_eq!(a.sample_pcm, pcm_a);
        assert_eq!(a.volume, 32);
        assert_eq!(b.sample_pcm, pcm_b);
        assert_eq!(b.volume, 96);
        assert_ne!(a.sample_pcm, b.sample_pcm);
    }

    #[test]
    fn sounds_ac_invalid_missing_sample() {
        let a0 = build_action0_sound_payload(0, 128, 0, None);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'M', 0, 1], "nosamp", "");
        let dir = tempfile_dir_with("nosamp.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("nosamp.grf", 0x534D_0001));
        apply_newgrf_sounds(&mut state, &[&dir]);

        assert!(
            state
                .runtime
                .newgrf_diagnostics
                .iter()
                .any(|d| d.contains("missing sample") && d.contains("local_id=0"))
        );
        assert_eq!(
            crate::play_newgrf_sound(&mut state, 0x534D_0001, 0),
            Err(crate::SoundPlayError::InvalidSample)
        );
        assert!(state.runtime.pending_newgrf_sounds.is_empty());
    }

    #[test]
    fn sounds_ac_truncated_action11_diagnostic() {
        const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
        // count=2 pero solo un sample completo → truncated.
        let mut action11 = vec![0x11, 2];
        action11.extend_from_slice(&3u16.to_le_bytes());
        action11.extend_from_slice(&[1, 2, 3]);
        // segundo sample: size pedida sin bytes suficientes
        action11.extend_from_slice(&8u16.to_le_bytes());
        let a0 = build_action0_sound_payload(0, 128, 0, Some(12));
        let mut action8 = vec![0x08, 0x07];
        action8.extend_from_slice(&[b'S', b'T', 0, 1]);
        action8.extend_from_slice(b"trunc\0\0");
        let mut data_section = Vec::new();
        for payload in [action11.as_slice(), a0.as_slice(), action8.as_slice()] {
            let size = u32::try_from(payload.len()).unwrap_or(0);
            data_section.extend_from_slice(&size.to_le_bytes());
            data_section.push(0xFF);
            data_section.extend_from_slice(payload);
        }
        data_section.extend_from_slice(&0u32.to_le_bytes());
        let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x00, 0x00]);
        bytes.extend_from_slice(&SIG);
        bytes.extend_from_slice(&sprite_offs.to_le_bytes());
        bytes.push(0x00);
        bytes.extend_from_slice(&data_section);
        bytes.extend_from_slice(&0u32.to_le_bytes());

        let dir = tempfile_dir_with("trunc.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("trunc.grf", 0x5354_0001));
        apply_newgrf_sounds(&mut state, &[&dir]);

        assert!(
            state
                .runtime
                .newgrf_diagnostics
                .iter()
                .any(|d| d.contains("Action11 truncated"))
        );
        // Sample 0 sí llegó; override baseset LevelCrossing (12).
        assert!(crate::play_newgrf_sound(&mut state, 0x5354_0001, 0).is_ok());
        assert_eq!(state.runtime.sound_overrides[12], Some((0x5354_0001, 0)));
        state.runtime.pending_newgrf_sounds.clear();
        assert!(crate::play_sound_or_override(&mut state, crate::SoundId::LevelCrossing).is_ok());
        assert_eq!(state.runtime.pending_newgrf_sounds.len(), 1);
    }

    /// #249: Vehicles AC — TE, railtype y refit mask en runtime.
    #[test]
    fn vehicles_ac_train_te_railtype_refit_mask() {
        // Bits temperate: Passengers=0, Coal=1 → mask 0x0003.
        let a0 = vec![
            0x00,
            ACTION0_FEATURE_TRAINS,
            0x04,
            0x01,
            0x00,
            0x05,
            0x01, // required_rail_type = Electric
            0x1F,
            200, // tractive_effort
            0x1D,
            0x03,
            0x00, // refit_mask WORD
            0xFE,
            b'T',
            b'E',
            0,
        ];
        let meta = parse_action0_train_meta(&a0).unwrap();
        assert_eq!(meta.required_rail_type, Some(1));
        assert_eq!(meta.tractive_effort, 200);
        assert_eq!(meta.refit_mask, 3);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'E', 0, 1], "te", "");
        let dir = tempfile_dir_with("te.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("te.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        assert_eq!(eng.required_rail_type, Some(1));
        assert_eq!(eng.tractive_effort, 200);
        assert_eq!(eng.refit_mask, 3);
        assert_eq!(crate::engine_tractive_effort(eng), 200);
        assert!(!crate::engine_compatible_with_rail(
            eng,
            crate::RailType::Rail
        ));
        assert!(crate::engine_compatible_with_rail(
            eng,
            crate::RailType::Electric
        ));
        let cargos = crate::refittable_cargo_types_for_engine(eng);
        assert_eq!(
            cargos,
            vec![crate::CargoType::Passengers, crate::CargoType::Coal]
        );

        // Truncated Action0: no panic.
        assert!(
            parse_action0_train_meta(&[0x00, ACTION0_FEATURE_TRAINS, 0x01, 0x01, 0x00, 0x1F])
                .is_none()
        );
        assert!(
            parse_action0_train_meta(&[0x00, ACTION0_FEATURE_TRAINS, 0x01, 0x01, 0x00, 0x1D, 0x01])
                .is_none()
        );
    }

    /// #274: Ships Action0 `0x1E` CTT include list → `refit_mask` runtime.
    #[test]
    fn ships_ctt_include_list_wires_refit_mask() {
        // CTT include: count=2, temperate ids Goods=5, Oil=3 → mask bits 3|5.
        let a0 = vec![
            0x00,
            ACTION0_FEATURE_SHIPS,
            0x01,
            0x01,
            0x00,
            0x1E,
            0x02,
            0x05,
            0x03,
        ];
        let meta = parse_action0_vehicle_metas(&a0).unwrap().remove(0);
        let expected = (1u32 << 5) | (1u32 << 3);
        assert_eq!(meta.refit_mask, expected);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'C', b'T', 0, 1], "ctt", "");
        let dir = tempfile_dir_with("ctt.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ctt.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf && e.kind == crate::VehicleKind::Ship)
            .unwrap();
        assert_eq!(eng.refit_mask, expected);
        let cargos = crate::refittable_cargo_types_for_engine(eng);
        assert_eq!(cargos, vec![crate::CargoType::Oil, crate::CargoType::Goods]);
    }

    /// #274: el CTT exclude debe restarse del include antes de ofrecer refit.
    #[test]
    fn ships_ctt_exclude_list_removes_cargo_from_refit_mask() {
        // Include Goods+Oil y exclude Oil: sólo Goods queda disponible.
        let a0 = vec![
            0x00,
            ACTION0_FEATURE_SHIPS,
            0x02,
            0x01,
            0x00,
            0x1E,
            0x02,
            0x05,
            0x03,
            0x1F,
            0x01,
            0x03,
        ];
        let meta = parse_action0_vehicle_metas(&a0).unwrap().remove(0);
        assert_eq!(meta.refit_mask, (1u32 << 5) | (1u32 << 3));
        assert_eq!(meta.refit_exclude_mask, 1u32 << 3);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'X', b'C', 0, 1], "ctt-ex", "");
        let dir = tempfile_dir_with("ctt-ex.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ctt-ex.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf && e.kind == crate::VehicleKind::Ship)
            .unwrap();
        assert_eq!(eng.refit_mask, 1u32 << 5);
        assert_eq!(
            crate::refittable_cargo_types_for_engine(eng),
            vec![crate::CargoType::Goods]
        );
    }

    /// #249: Vehicles AC — flag helicóptero aircraft prop 0x09.
    #[test]
    fn vehicles_ac_aircraft_heli_flag() {
        let a0 = vec![0x00, ACTION0_FEATURE_AIRCRAFT, 0x01, 0x01, 0x00, 0x09, 0x01];
        let meta = parse_action0_vehicle_metas(&a0).unwrap().remove(0);
        assert!(meta.is_helicopter);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'H', b'E', 0, 1], "heli", "");
        let dir = tempfile_dir_with("heli.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("heli.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf && e.kind == VehicleKind::Aircraft)
            .unwrap();
        assert!(eng.is_helicopter);
        assert!(crate::aircraft_is_helicopter_def(eng));
        assert!(
            parse_action0_vehicle_metas(&[0x00, ACTION0_FEATURE_AIRCRAFT, 0x01, 0x01, 0x00, 0x09])
                .is_none()
        );
    }

    /// #259: Bridges — Action0 override precio/año del puente madera.
    #[test]
    fn infra_ac_bridge_override_cost_and_avail() {
        use crate::bridge_spec::{BridgeType, bridge_build_cost, bridge_build_cost_in};
        use crate::command::{Command, apply_command};
        use crate::map::{TileCoord, TileKind};

        // year=50 → 1970; price=10 (vanilla madera=80); max_len unlimited; speed 40.
        let a0 = build_action0_bridge_payload(0, 50, 0, 255, 10, 40, "Madera Custom");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'B', b'R', 0, 1], "br", "");
        let dir = tempfile_dir_with("br.grf", &bytes);
        let mut state = GameState::new(12, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("br.grf", 0x4252_0001));
        apply_newgrf_bridges(&mut state, &[&dir]);

        let def = crate::bridge_spec_def(&state.bridge_spec_catalog, BridgeType::Wooden).unwrap();
        assert!(def.from_newgrf);
        assert_eq!(def.grfid, 0x4252_0001);
        assert_eq!(def.available_from_year, 1970);
        assert_eq!(def.price_mult, 10);
        assert_eq!(def.max_speed, 40);
        assert_eq!(def.name, "Madera Custom");
        assert_eq!(def.max_middle_len, None);

        let a = TileCoord::new(1, 2);
        let b = TileCoord::new(5, 2);
        for x in 2..=4 {
            state
                .map
                .set_kind(TileCoord::new(x, 2), TileKind::Water)
                .unwrap();
        }
        let vanilla = bridge_build_cost(BridgeType::Wooden, a, b);
        let custom = bridge_build_cost_in(&state.bridge_spec_catalog, BridgeType::Wooden, a, b);
        assert_ne!(vanilla, custom);
        assert_eq!(
            custom,
            10 * (i64::from(crate::bridge_total_length(a, b)) + 1)
        );

        // Año calendario inicial (1950) < 1970 → no disponible.
        assert_eq!(
            apply_command(
                &mut state,
                &Command::PlaceRailBridge(a, b, BridgeType::Wooden)
            ),
            Err(crate::CommandError::BridgeTypeNotAvailable)
        );
        // Avanzar calendario a 1970+.
        state.tick = crate::news::tick_for_calendar_year(1970);
        apply_command(
            &mut state,
            &Command::PlaceRailBridge(a, b, BridgeType::Wooden),
        )
        .unwrap();
        assert_eq!(state.map.get_kind(a), Some(TileKind::RailBridge));

        // Truncated Action0 no panica.
        let _ = parse_action0_bridge_meta(&[0x00, ACTION0_FEATURE_BRIDGES, 0x01, 0x01, 0x00, 0x08]);
        let _ = parse_action0_bridge_meta(&[0x00, ACTION0_FEATURE_BRIDGES, 0x02, 0x01, 0x00]);
    }

    /// #259: Bridges — dos GRFs mismo `local_id`; el último gana.
    #[test]
    fn infra_ac_bridge_two_grf_stack_last_wins() {
        use crate::bridge_spec::BridgeType;

        let a0_a = build_action0_bridge_payload(0, 0, 0, 255, 11, 32, "A");
        let a0_b = build_action0_bridge_payload(0, 0, 0, 255, 99, 64, "B");
        let bytes_a = build_grf_v2_with_action0_and_action8(&a0_a, [b'B', b'A', 0, 1], "ba", "");
        let bytes_b = build_grf_v2_with_action0_and_action8(&a0_b, [b'B', b'B', 0, 2], "bb", "");
        let dir_a = tempfile_dir_with("ba.grf", &bytes_a);
        let dir_b = tempfile_dir_with("bb.grf", &bytes_b);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ba.grf", 0x4241_0001));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("bb.grf", 0x4242_0002));
        apply_newgrf_bridges(&mut state, &[&dir_a, &dir_b]);

        let def = crate::bridge_spec_def(&state.bridge_spec_catalog, BridgeType::Wooden).unwrap();
        assert_eq!(def.price_mult, 99);
        assert_eq!(def.max_speed, 64);
        assert_eq!(def.name, "B");
        assert_eq!(def.grfid, 0x4242_0002);
    }

    /// #259: Canals — Action0 feature + Action5 slot; `PlaceCanal` en hierba.
    #[test]
    fn infra_ac_canal_feature_and_action5() {
        use crate::command::{Command, apply_command};
        use crate::map::TileCoord;
        use crate::newgrf_sprites::{
            ACTION5_TYPE_CANALS, CANALS_ACTION5_LOCK_SLOT, build_grf_v2_action5_with_sprite,
        };

        let a0 = build_action0_canal_payload(crate::CF_LOCKS, 0x03, 0x05);
        let bytes_a0 = build_grf_v2_with_action0_and_action8(&a0, [b'C', b'A', 0, 1], "ca", "");
        let indices = vec![1u8; 8 * 8];
        let bytes_a5 = build_grf_v2_action5_with_sprite(
            ACTION5_TYPE_CANALS,
            u16::try_from(CANALS_ACTION5_LOCK_SLOT).unwrap(),
            8,
            8,
            &indices,
            [b'C', b'5', 0, 1],
            "c5",
        );
        let dir_a0 = tempfile_dir_with("ca.grf", &bytes_a0);
        let dir_a5 = tempfile_dir_with("c5.grf", &bytes_a5);
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ca.grf", 0x4341_0001));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("c5.grf", 0x4335_0001));
        apply_newgrf_canals(&mut state, &[&dir_a0, &dir_a5]);
        apply_newgrf_action5_canals(&mut state, &[&dir_a0, &dir_a5]);

        let def = crate::canal_feature_def(&state.canal_feature_catalog, crate::CF_LOCKS).unwrap();
        assert!(def.from_newgrf);
        assert_eq!(def.callback_mask, 0x03);
        assert_eq!(def.flags, 0x05);
        assert_eq!(def.grfid, 0x4341_0001);

        assert_eq!(
            state.runtime.canal_action5_newgrf_sprites.len(),
            crate::CANALS_ACTION5_SLOT_COUNT
        );
        assert!(state.runtime.canal_action5_newgrf_sprites[CANALS_ACTION5_LOCK_SLOT].is_some());
        assert!(state.runtime.canal_action5_newgrf_sprites[0].is_none());

        let c = TileCoord::new(3, 3);
        apply_command(&mut state, &Command::PlaceCanal(c)).unwrap();
        assert_eq!(state.map.get_kind(c), Some(crate::TileKind::Water));

        // Truncated Action0 canal no panica.
        let _ = parse_action0_canal_meta(&[0x00, ACTION0_FEATURE_CANALS, 0x01, 0x01, 0x01, 0x08]);
        let _ = parse_action0_canal_meta(&[0x00, ACTION0_FEATURE_CANALS, 0x02, 0x01]);
    }

    /// #259: Canals — dos GRFs features distintos no se pisan; mismo id last-wins.
    #[test]
    fn infra_ac_canal_two_grf_isolate() {
        let a0_locks = build_action0_canal_payload(crate::CF_LOCKS, 0x01, 0x02);
        let a0_buoy = build_action0_canal_payload(crate::CF_BUOY, 0x10, 0x20);
        let a0_locks2 = build_action0_canal_payload(crate::CF_LOCKS, 0x77, 0x88);
        let bytes_a =
            build_grf_v2_with_action0_and_action8(&a0_locks, [b'C', b'1', 0, 1], "c1", "");
        let bytes_b = build_grf_v2_with_action0_and_action8(&a0_buoy, [b'C', b'2', 0, 2], "c2", "");
        let bytes_c =
            build_grf_v2_with_action0_and_action8(&a0_locks2, [b'C', b'3', 0, 3], "c3", "");
        let dir_a = tempfile_dir_with("c1.grf", &bytes_a);
        let dir_b = tempfile_dir_with("c2.grf", &bytes_b);
        let dir_c = tempfile_dir_with("c3.grf", &bytes_c);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("c1.grf", 0x4331_0001));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("c2.grf", 0x4332_0002));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("c3.grf", 0x4333_0003));
        apply_newgrf_canals(&mut state, &[&dir_a, &dir_b, &dir_c]);

        let locks =
            crate::canal_feature_def(&state.canal_feature_catalog, crate::CF_LOCKS).unwrap();
        let buoy = crate::canal_feature_def(&state.canal_feature_catalog, crate::CF_BUOY).unwrap();
        let slope =
            crate::canal_feature_def(&state.canal_feature_catalog, crate::CF_WATERSLOPE).unwrap();
        assert_eq!(locks.callback_mask, 0x77);
        assert_eq!(locks.flags, 0x88);
        assert_eq!(locks.grfid, 0x4333_0003);
        assert_eq!(buoy.callback_mask, 0x10);
        assert_eq!(buoy.flags, 0x20);
        assert_eq!(buoy.grfid, 0x4332_0002);
        assert!(!slope.from_newgrf);
    }

    /// #249: Vehicles AC — fracciones de velocidad océano/canal.
    #[test]
    fn vehicles_ac_ship_speed_fracs() {
        let a0 = vec![
            0x00,
            ACTION0_FEATURE_SHIPS,
            0x03,
            0x01,
            0x00,
            0x0B,
            100, // max_speed
            0x14,
            128, // ocean = 50%
            0x15,
            64, // canal = 25%
        ];
        let meta = parse_action0_vehicle_metas(&a0).unwrap().remove(0);
        assert_eq!(meta.ocean_speed_frac, 128);
        assert_eq!(meta.canal_speed_frac, 64);

        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'P', 0, 1], "ship", "");
        let dir = tempfile_dir_with("shipf.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("shipf.grf", 1));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state
            .engine_catalog
            .iter()
            .find(|e| e.from_newgrf && e.kind == VehicleKind::Ship)
            .unwrap();
        assert_eq!(eng.ocean_speed_frac, 128);
        assert_eq!(eng.canal_speed_frac, 64);
        assert_eq!(crate::ship_speed_for_tile(eng, false), 50);
        assert_eq!(crate::ship_speed_for_tile(eng, true), 25);
        assert!(
            parse_action0_vehicle_metas(&[0x00, ACTION0_FEATURE_SHIPS, 0x01, 0x01, 0x00, 0x14])
                .is_none()
        );
    }

    #[test]
    fn parse_and_apply_airports_registers_catalog_and_blocks_fta() {
        use crate::airport_class::{NEW_AIRPORT_OFFSET, newgrf_airport_spec_def};
        use crate::airport_tile_spec::NEW_AIRPORT_TILE_OFFSET;
        use crate::{AirportSpecId, Command, TileCoord, apply_command, station_uses_airport_fta};

        let tile = build_action0_airport_tile_payload(0, 24, None, 0x01); // subst hangar gfx
        let tile2 = build_action0_airport_tile_payload(1, 14, None, 0); // runway
        let air = build_action0_airport_payload(
            0,
            0, // AT_SMALL
            &[(0, 0, 0), (1, 0, 1), (0, 1, 0), (1, 1, 1)],
            5,
            4,
            "MiniPort",
        );
        let bytes = build_grf_v2_with_action0s_and_action8(
            &[tile.as_slice(), tile2.as_slice(), air.as_slice()],
            [b'A', b'P', 0, 1],
            "ap",
            "",
        );
        let dir = tempfile_dir_with("ap.grf", &bytes);
        let mut state = GameState::new(16, 16);
        state.economy.money = 1_000_000;
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ap.grf", 0x4150_0001));
        apply_newgrf_airport_tiles(&mut state, &[&dir]);
        apply_newgrf_airports(&mut state, &[&dir]);
        assert_eq!(state.airport_tile_spec_catalog.len(), 2);
        assert!(state.airport_tile_spec_catalog[0].gfx.as_u16() >= NEW_AIRPORT_TILE_OFFSET);
        assert_eq!(state.airport_tile_spec_catalog[0].callback_mask, 0x01);
        assert_eq!(state.airport_spec_catalog.len(), 1);
        {
            let def = &state.airport_spec_catalog[0];
            assert!(def.id >= NEW_AIRPORT_OFFSET);
            assert_eq!(def.label, "MiniPort");
            assert_eq!(def.subst_id, AirportSpecId::Small);
            assert_eq!(def.catchment, 5);
            assert_eq!(def.noise_level, 4);
            assert!(!def.layouts.is_empty());
            assert_eq!(def.layouts[0].tiles.len(), 4);
        }
        let newgrf_id = state.airport_spec_catalog[0].id;

        apply_command(&mut state, &Command::SetCurrentAirportNewgrfSpec(newgrf_id)).unwrap();
        assert_eq!(state.current_airport_newgrf_id, Some(newgrf_id));
        apply_command(
            &mut state,
            &Command::PlaceAirportArea {
                origin: TileCoord::new(2, 2),
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        )
        .unwrap();
        assert_eq!(state.stations.len(), 1);
        let st = &state.stations[0];
        assert_eq!(st.airport_newgrf_spec_id, Some(newgrf_id));
        assert_eq!(st.airport_spec, AirportSpecId::Small);
        assert_eq!(st.airport_tiles.len(), 4);
        assert!(
            !station_uses_airport_fta(st),
            "NewGRF airport must not use vanilla FTA"
        );
        assert!(newgrf_airport_spec_def(&state.airport_spec_catalog, newgrf_id).is_some());
    }

    fn tempfile_dir_with(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openttdrs_ngr_{}_{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(name), bytes).unwrap();
        dir
    }

    fn house_avail(zones: u16, climate_bit: u8) -> u16 {
        zones | (1u16 << climate_bit)
    }

    #[test]
    fn houses_ac_catalog_climate_zone_year() {
        use crate::house_spec::{
            BUILDING_FLAG_SIZE_1X1, NEW_HOUSE_OFFSET, pick_town_house_id_with_catalog,
        };
        use crate::town::{HouseZone, Town};
        use crate::world_gen::Climate;

        let centre = 1u16 << (HouseZone::TownCentre as u8);
        let temp = HouseZone::ClimateTemperate as u8;
        let toy = HouseZone::ClimateToyland as u8;
        let a0 = build_action0_house_payload(
            0,
            0,
            BUILDING_FLAG_SIZE_1X1,
            1950,
            2000,
            house_avail(centre, temp),
            200,
            "CasaTemp",
        );
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'H', b'S', 0, 1], "hs", "");
        let dir = tempfile_dir_with("hs.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("hs.grf", 7));
        apply_newgrf_houses(&mut state, &[&dir]);
        assert_eq!(state.house_spec_catalog.len(), 1);
        let def_id = state.house_spec_catalog[0].id;
        let def = &state.house_spec_catalog[0];
        assert!(def_id >= NEW_HOUSE_OFFSET);
        assert_eq!(def.callback_mask, 0);
        let centre_temp = centre | (1u16 << temp);
        let centre_toy = centre | (1u16 << toy);
        assert!(def.matches_zones(centre_temp));
        assert!(!def.matches_zones(centre_toy));

        // Sin vanilla en el pool: overrides ocupan todos los ids 0..109.
        state.house_overrides.fill(def_id);
        let town = Town {
            pos: crate::map::TileCoord::new(10, 10),
            num_houses: 48,
            ..Default::default()
        };
        // Toyland: NewGRF no entra → pool vacío.
        assert!(
            pick_town_house_id_with_catalog(
                &town,
                HouseZone::TownCentre,
                Climate::Toyland,
                1,
                1980,
                0,
                &state.house_spec_catalog,
                &state.house_overrides,
            )
            .is_none()
        );
        // Temperate + centre + year: solo NewGRF.
        assert_eq!(
            pick_town_house_id_with_catalog(
                &town,
                HouseZone::TownCentre,
                Climate::Temperate,
                1,
                1980,
                0,
                &state.house_spec_catalog,
                &state.house_overrides,
            ),
            Some(def_id)
        );
        // Año fuera de rango.
        assert!(
            pick_town_house_id_with_catalog(
                &town,
                HouseZone::TownCentre,
                Climate::Temperate,
                1,
                1900,
                0,
                &state.house_spec_catalog,
                &state.house_overrides,
            )
            .is_none()
        );
    }

    #[test]
    fn houses_ac_subst_and_action3_views() {
        use crate::house_spec::{BUILDING_FLAG_SIZE_1X1, resolve_house_draw_id};
        let a0 =
            build_action0_house_payload(0, 3, BUILDING_FLAG_SIZE_1X1, 0, 5000, 0xFFFF, 16, "Vista");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::newgrf_sprites::build_grf_v2_house_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'H', b'V', 0, 1],
            "hview",
        );
        let dir = tempfile_dir_with("hview.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("hview.grf", 8));
        apply_newgrf_houses(&mut state, &[&dir]);
        let def = &state.house_spec_catalog[0];
        assert_eq!(def.subst_id, 3);
        assert!(!def.newgrf_views.is_empty());
        assert!(def.has_newgrf_sprites());
        assert_eq!(
            resolve_house_draw_id(def.id, &state.house_spec_catalog),
            def.id
        );
    }

    /// CB17 debe llegar desde Action0/2/3 cargado hasta el crecimiento físico
    /// del pueblo; un cero rechaza sin reservar ninguna tesela de casa.
    #[test]
    fn houses_ac_construction_callback_runs_from_loaded_action2_graph() {
        use crate::house_spec::BUILDING_FLAG_SIZE_1X1;
        use crate::map::{Map, TileCoord, TileKind};
        use crate::newgrf_sprites::{
            build_action2_callback_literal_payload, build_grf_v2_feature_with_action2_chain,
        };
        use crate::town::Town;
        use crate::town_expand::{TownExpandContext, place_house_with_spec};
        use crate::world_gen::Climate;

        let action0 = build_action0_house_payload_ex(
            0,
            0,
            BUILDING_FLAG_SIZE_1X1,
            0,
            5000,
            u16::MAX,
            16,
            None,
            crate::house_spec::HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK,
            "House callback",
        );
        let action2 = build_action2_callback_literal_payload(
            ACTION0_FEATURE_HOUSES,
            7,
            0, // Booleano de ocho bits cero: CB17 deniega la construcción.
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_HOUSES,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            [b'H', b'C', 0, 1],
            "house-cb",
        );
        let dir = tempfile_dir_with("house_cb.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("house_cb.grf", 0x4843_0001));
        apply_newgrf_houses(&mut state, &[&dir]);

        let def = state.house_spec_catalog.first().unwrap();
        assert!(def.has_construction_callback());
        assert!(def.newgrf_runtime.is_some());
        let mut overrides = state.house_overrides.clone();
        overrides.fill(def.id);
        let mut map = Map::new_flat(8, 8, 1);
        let pos = TileCoord::new(4, 4);
        let mut town = Town {
            id: 1,
            pos: TileCoord::new(3, 3),
            name: "Callback Town".into(),
            ..Town::default()
        };
        let ctx = TownExpandContext {
            climate: Climate::Temperate,
            calendar_year: 1980,
            house_catalog: &state.house_spec_catalog,
            house_overrides: &overrides,
        };

        assert_eq!(
            place_house_with_spec(&mut map, &mut town, pos, ctx, 0),
            None
        );
        assert_eq!(map.get_kind(pos), Some(TileKind::Grass));
        assert_eq!(town.num_houses, 0);
        assert_eq!(town.population, 0);
    }

    #[test]
    fn houses_ac_override_and_two_grf() {
        use crate::house_spec::{
            BUILDING_FLAG_SIZE_1X1, NEW_HOUSE_OFFSET, get_translated_house_id,
        };
        let a0_a = build_action0_house_payload_ex(
            0,
            0,
            BUILDING_FLAG_SIZE_1X1,
            0,
            5000,
            0xFFFF,
            16,
            Some(5),
            0x0102,
            "A",
        );
        let a0_b = build_action0_house_payload_ex(
            0,
            1,
            BUILDING_FLAG_SIZE_1X1,
            0,
            5000,
            0xFFFF,
            16,
            Some(5),
            0,
            "B",
        );
        let bytes_a = build_grf_v2_with_action0_and_action8(&a0_a, [b'H', b'A', 0, 1], "ha", "");
        let bytes_b = build_grf_v2_with_action0_and_action8(&a0_b, [b'H', b'B', 0, 1], "hb", "");
        let dir = tempfile_dir_with("ha.grf", &bytes_a);
        std::fs::write(dir.join("hb.grf"), &bytes_b).unwrap();
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ha.grf", 1));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("hb.grf", 2));
        apply_newgrf_houses(&mut state, &[&dir]);
        assert_eq!(state.house_spec_catalog.len(), 2);
        let last = state
            .house_spec_catalog
            .iter()
            .find(|d| d.grfid == 2)
            .unwrap();
        assert_eq!(state.house_overrides[5], last.id);
        assert_eq!(get_translated_house_id(5, &state.house_overrides), last.id);
        assert!(last.id >= NEW_HOUSE_OFFSET);
        let first = state
            .house_spec_catalog
            .iter()
            .find(|d| d.grfid == 1)
            .unwrap();
        assert_eq!(first.callback_mask, 0x0102);
    }

    #[test]
    fn houses_ac_multitile_geometry() {
        use crate::house_spec::{
            BUILDING_FLAG_SIZE_2X2, house_footprint_offsets, next_free_house_id,
        };
        let flags = BUILDING_FLAG_SIZE_2X2;
        let offs = house_footprint_offsets(flags);
        assert_eq!(offs.len(), 4);
        // Cuatro Action0 consecutivos → ids globales consecutivos.
        let mut payloads = Vec::new();
        for local in 0u8..4 {
            let f = if local == 0 { flags } else { 0 };
            payloads.push(build_action0_house_payload(
                local, 0, f, 0, 5000, 0xFFFF, 16, "MT",
            ));
        }
        let refs: Vec<&[u8]> = payloads.iter().map(Vec::as_slice).collect();
        let bytes = build_grf_v2_with_action0s_and_action8(&refs, [b'H', b'M', 0, 1], "hm", "");
        let dir = tempfile_dir_with("hm.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("hm.grf", 9));
        apply_newgrf_houses(&mut state, &[&dir]);
        assert_eq!(state.house_spec_catalog.len(), 4);
        let base = state.house_spec_catalog[0].id;
        for (i, def) in state.house_spec_catalog.iter().enumerate() {
            assert_eq!(def.id, base + u16::try_from(i).unwrap());
        }
        assert_eq!(
            next_free_house_id(&state.house_spec_catalog),
            Some(base + 4)
        );
    }

    #[test]
    fn houses_ac_truncated_payload_no_panic() {
        let _ = parse_action0_house_meta(&[0x00, ACTION0_FEATURE_HOUSES, 0x01, 0x01, 0x00]);
        let _ = parse_action0_house_meta(&[0x00, ACTION0_FEATURE_HOUSES, 0x02, 0x01, 0x00, 0x08]);
        let _ = parse_action0_house_meta(&[]);
    }

    #[test]
    fn houses_ac_deterministic_pick() {
        use crate::house_spec::{BUILDING_FLAG_SIZE_1X1, pick_town_house_id_with_catalog};
        use crate::town::{HouseZone, Town};
        use crate::world_gen::Climate;

        let a0 =
            build_action0_house_payload(0, 0, BUILDING_FLAG_SIZE_1X1, 0, 5000, 0xFFFF, 16, "Det");
        let meta = parse_action0_house_meta(&a0).unwrap();
        assert_eq!(meta.subst_id, 0);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'H', b'D', 0, 1], "hd", "");
        let dir = tempfile_dir_with("hd.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("hd.grf", 3));
        apply_newgrf_houses(&mut state, &[&dir]);
        let town = Town::default();
        let a = pick_town_house_id_with_catalog(
            &town,
            HouseZone::TownCentre,
            Climate::Temperate,
            1,
            1980,
            42,
            &state.house_spec_catalog,
            &state.house_overrides,
        );
        let b = pick_town_house_id_with_catalog(
            &town,
            HouseZone::TownCentre,
            Climate::Temperate,
            1,
            1980,
            42,
            &state.house_spec_catalog,
            &state.house_overrides,
        );
        assert_eq!(a, b);
    }

    fn sample_tile_indices() -> Vec<u8> {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        indices
    }

    /// #256: industria custom multitile con inputs/outputs correctos.
    #[test]
    fn industries_ac_multitile_io() {
        use crate::command::place_industry_spec_def_sandbox;
        use crate::industry_spec::NEW_INDUSTRY_OFFSET;
        use crate::map::{TileCoord, industry_gfx};
        use crate::newgrf_sprites::build_grf_v2_industries_with_tiles;

        let tile0 = build_action0_industry_tile_payload_ex(0, 0, None, &[(5, 8)], 0x01);
        let tile1 = build_action0_industry_tile_payload_ex(1, 1, None, &[], 0);
        let ind = build_action0_industry_payload(
            0,
            0,
            None,
            &[(0, 0, 0), (1, 0, 1)],
            &[1], // COAL
            &[7], // WOOD input
            &[15],
            0x0102,
            "MultiIO",
        );
        let indices = sample_tile_indices();
        let bytes = build_grf_v2_industries_with_tiles(
            &[(tile0, 0, indices.clone()), (tile1, 1, indices)],
            &ind,
            8,
            8,
            [b'I', b'N', 0, 1],
            "indio",
        );
        let dir = tempfile_dir_with("ind_io.grf", &bytes);
        let mut state = GameState::new(16, 16);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ind_io.grf", 0x494E_0001));
        apply_newgrf_industry_tiles(&mut state, &[&dir]);
        apply_newgrf_industries(&mut state, &[&dir]);
        assert_eq!(state.industry_tile_spec_catalog.len(), 2);
        assert_eq!(state.industry_spec_catalog.len(), 1);
        let def = &state.industry_spec_catalog[0];
        assert!(def.id >= NEW_INDUSTRY_OFFSET);
        assert_eq!(def.produced_cargo_labels, vec!["COAL".to_string()]);
        assert_eq!(def.accepted_cargo_labels, vec!["WOOD".to_string()]);
        assert_eq!(def.production_rates, vec![15]);
        assert_eq!(def.callback_mask, 0x0102);
        assert_eq!(def.layouts[0].len(), 2);
        assert_eq!(
            state.industry_tile_spec_catalog[0].accepts_cargo_labels,
            vec!["GOOD".to_string()]
        );
        assert_eq!(state.industry_tile_spec_catalog[0].callback_mask, 0x01);
        let def_id = def.id;

        place_industry_spec_def_sandbox(&mut state, TileCoord::new(2, 2), def_id).unwrap();
        assert_eq!(state.industries.len(), 1);
        let ind = &state.industries[0];
        assert_eq!(ind.tiles.len(), 2);
        assert_eq!(ind.newgrf_type_id, Some(def_id));
        assert_eq!(ind.newgrf_production_rate, Some(15));
        assert_eq!(ind.newgrf_output_cargo, Some(crate::CargoType::Coal));
        let g0 = industry_gfx(&state.map.get(TileCoord::new(2, 2)).unwrap());
        let g1 = industry_gfx(&state.map.get(TileCoord::new(3, 2)).unwrap());
        assert!(g0 >= crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET);
        assert!(g1 >= crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET);
        assert_ne!(g0, g1);
    }

    /// #256: IDs locales/globales y overrides estables con varios GRF.
    #[test]
    fn industries_ac_multi_grf_ids_overrides() {
        use crate::industry_spec::{NEW_INDUSTRY_OFFSET, get_translated_industry_id};

        let a0_a = build_action0_industry_payload(
            0,
            0,
            Some(3),
            &[(0, 0, 0)],
            &[1],
            &[],
            &[10],
            0x00AB,
            "A",
        );
        let a0_b =
            build_action0_industry_payload(0, 1, Some(3), &[(0, 0, 0)], &[7], &[], &[12], 0, "B");
        let tile_a = build_action0_industry_tile_payload_ex(0, 0, None, &[], 0);
        let tile_b = build_action0_industry_tile_payload_ex(0, 2, None, &[], 0);
        let bytes_a = build_grf_v2_with_action0s_and_action8(
            &[tile_a.as_slice(), a0_a.as_slice()],
            [b'I', b'A', 0, 1],
            "ia",
            "",
        );
        let bytes_b = build_grf_v2_with_action0s_and_action8(
            &[tile_b.as_slice(), a0_b.as_slice()],
            [b'I', b'B', 0, 1],
            "ib",
            "",
        );
        let dir = tempfile_dir_with("ia.grf", &bytes_a);
        std::fs::write(dir.join("ib.grf"), &bytes_b).unwrap();
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ia.grf", 1));
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ib.grf", 2));
        apply_newgrf_industry_tiles(&mut state, &[&dir]);
        apply_newgrf_industries(&mut state, &[&dir]);
        assert_eq!(state.industry_spec_catalog.len(), 2);
        let last = state
            .industry_spec_catalog
            .iter()
            .find(|d| d.grfid == 2)
            .unwrap();
        assert_eq!(state.industry_overrides[3], last.id);
        assert_eq!(
            get_translated_industry_id(3, &state.industry_overrides),
            last.id
        );
        assert!(last.id >= NEW_INDUSTRY_OFFSET);
        let first = state
            .industry_spec_catalog
            .iter()
            .find(|d| d.grfid == 1)
            .unwrap();
        assert_eq!(first.callback_mask, 0x00AB);
        assert_ne!(first.id, last.id);
        // Tiles: ids globales estables y distintos por GRF.
        assert_eq!(state.industry_tile_spec_catalog.len(), 2);
        assert_ne!(
            state.industry_tile_spec_catalog[0].gfx,
            state.industry_tile_spec_catalog[1].gfx
        );
    }

    /// #256: Action3 selecciona sprites por tile y respeta fallback subst.
    #[test]
    fn industries_ac_action3_sprite_fallback_per_tile() {
        use crate::industry_tile::resolve_industry_tile_draw_gfx;
        use crate::newgrf_sprites::build_grf_v2_industries_with_tiles;

        let tile0 = build_action0_industry_tile_payload_ex(0, 5, None, &[], 0);
        let tile1 = build_action0_industry_tile_payload_ex(1, 9, None, &[], 0);
        let ind = build_action0_industry_payload(
            0,
            0,
            None,
            &[(0, 0, 0), (1, 0, 1)],
            &[1],
            &[],
            &[8],
            0,
            "Spr",
        );
        let indices = sample_tile_indices();
        let bytes = build_grf_v2_industries_with_tiles(
            &[(tile0, 0, indices.clone()), (tile1, 1, indices)],
            &ind,
            8,
            8,
            [b'I', b'S', 0, 1],
            "indspr",
        );
        let dir = tempfile_dir_with("ind_spr.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("ind_spr.grf", 3));
        apply_newgrf_industry_tiles(&mut state, &[&dir]);
        apply_newgrf_industries(&mut state, &[&dir]);
        let t0 = &state.industry_tile_spec_catalog[0];
        let t1 = &state.industry_tile_spec_catalog[1];
        assert!(t0.has_newgrf_sprites());
        assert!(t1.has_newgrf_sprites());
        assert!(!t0.newgrf_views.is_empty());
        assert!(!t1.newgrf_views.is_empty());
        assert_eq!(
            resolve_industry_tile_draw_gfx(t0.gfx.as_u16(), &state.industry_tile_spec_catalog),
            t0.gfx.as_u16()
        );
        // Sin vistas → fallback subst.
        let mut bare = t0.clone();
        bare.newgrf_views.clear();
        bare.newgrf_preview = None;
        bare.newgrf_runtime = None;
        let cat = vec![bare];
        assert_eq!(resolve_industry_tile_draw_gfx(cat[0].gfx.as_u16(), &cat), 5);
        // Layout apunta a gfx globales distintos por tile.
        let layout = &state.industry_spec_catalog[0].layouts[0];
        assert_eq!(layout[0].gfx, t0.gfx.as_u16());
        assert_eq!(layout[1].gfx, t1.gfx.as_u16());
    }

    /// #256: `callback_mask` y cargo labels almacenados (sin ejecutar CBs).
    #[test]
    fn industries_ac_callback_mask_and_cargo_labels_stored() {
        let tile = build_action0_industry_tile_payload_ex(0, 0, Some(10), &[(1, 4), (5, 8)], 0x3C);
        let ind = build_action0_industry_payload(
            0,
            0,
            Some(2),
            &[(0, 0, 0)],
            &[1, 9], // COAL, STEL
            &[7, 5], // WOOD, GOOD
            &[11, 3],
            0x55AA,
            "CB",
        );
        let meta_t = parse_action0_industry_tile_meta(&tile).unwrap();
        assert_eq!(meta_t.callback_mask, 0x3C);
        assert_eq!(meta_t.accepts_cargo_indices, vec![1, 5]);
        assert_eq!(meta_t.override_of, Some(10));
        let meta_i = parse_action0_industry_meta(&ind).unwrap();
        assert_eq!(meta_i.callback_mask, 0x55AA);
        assert_eq!(meta_i.produced_cargo_indices, vec![1, 9]);
        assert_eq!(meta_i.accepted_cargo_indices, vec![7, 5]);

        let bytes = build_grf_v2_with_action0s_and_action8(
            &[tile.as_slice(), ind.as_slice()],
            [b'I', b'C', 0, 1],
            "icb",
            "",
        );
        let dir = tempfile_dir_with("icb.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("icb.grf", 4));
        apply_newgrf_industry_tiles(&mut state, &[&dir]);
        apply_newgrf_industries(&mut state, &[&dir]);
        let tdef = &state.industry_tile_spec_catalog[0];
        assert_eq!(tdef.callback_mask, 0x3C);
        assert_eq!(
            tdef.accepts_cargo_labels,
            vec!["COAL".to_string(), "GOOD".to_string()]
        );
        let idef = &state.industry_spec_catalog[0];
        assert_eq!(idef.callback_mask, 0x55AA);
        assert_eq!(
            idef.produced_cargo_labels,
            vec!["COAL".to_string(), "STEL".to_string()]
        );
        assert_eq!(
            idef.accepted_cargo_labels,
            vec!["WOOD".to_string(), "GOOD".to_string()]
        );
        // Truncated payloads: no panic.
        let _ = parse_action0_industry_meta(&[0x00, ACTION0_FEATURE_INDUSTRIES, 0x01, 0x01, 0x00]);
        let _ = parse_action0_industry_tile_meta(&[
            0x00,
            ACTION0_FEATURE_INDUSTRYTILES,
            0x02,
            0x01,
            0x00,
            0x08,
        ]);
    }

    /// El CB28 debe venir del Action2 del GRF cargado, no solo de una fixture
    /// inyectada manualmente en el catálogo.
    #[test]
    fn industries_ac_location_callback_runs_from_loaded_action2_graph() {
        use crate::command::{CommandError, place_industry_spec_def_sandbox};
        use crate::industry_spec::INDUSTRY_CALLBACK_LOCATION_MASK;
        use crate::map::TileCoord;
        use crate::newgrf_sprites::{
            build_action2_callback_literal_payload, build_grf_v2_feature_with_action2_chain,
        };

        let action0 = build_action0_industry_payload(
            0,
            0,
            None,
            &[(0, 0, 0)],
            &[1],
            &[],
            &[10],
            INDUSTRY_CALLBACK_LOCATION_MASK,
            "Location callback",
        );
        let action2 = build_action2_callback_literal_payload(
            ACTION0_FEATURE_INDUSTRIES,
            7,
            0x10, // No es FAILED/0x400/0xFF: OpenTTD rechaza la ubicación.
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_INDUSTRIES,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            [b'I', b'C', 0, 1],
            "industry-cb",
        );
        let dir = tempfile_dir_with("industry_cb.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("industry_cb.grf", 0x4943_0001));

        apply_newgrf_industries(&mut state, &[&dir]);
        let def = state.industry_spec_catalog.first().unwrap();
        assert!(def.newgrf_runtime.is_some());
        assert!(def.has_location_callback());
        let type_id = def.id;

        assert_eq!(
            place_industry_spec_def_sandbox(&mut state, TileCoord::new(1, 1), type_id),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert!(state.industries.is_empty());
    }

    /// CB13 debe venir del Action2 de un GRF cargado y bloquear tanto query
    /// como execute antes de crear una estación o modificar el mapa.
    #[test]
    fn station_availability_callback_runs_from_loaded_action2_graph() {
        use crate::command::{Command, CommandError, apply_command, command_would_fail};
        use crate::map::{TileCoord, TileKind};
        use crate::newgrf_sprites::{
            build_action2_callback_literal_payload, build_grf_v2_feature_with_action2_chain,
        };

        let action0 = build_action0_station_payload_with_callback_mask(
            b"CBST",
            b"Spec",
            0,
            0,
            1,
            "Station callback",
        );
        let action2 = build_action2_callback_literal_payload(
            ACTION0_FEATURE_STATIONS,
            7,
            0, // Booleano de 8 bits cero: CB13 deniega la construcción.
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_STATIONS,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            [b'C', b'S', 0, 1],
            "station-cb",
        );
        let dir = tempfile_dir_with("station_cb.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("station_cb.grf", 0x4353_0001));
        apply_newgrf_stations(&mut state, &[&dir]);

        let (class_id, spec_id) = {
            let spec = state
                .station_spec_catalog
                .iter()
                .find(|spec| spec.from_newgrf)
                .unwrap();
            assert_eq!(spec.callback_mask, 1);
            assert!(spec.has_availability_callback());
            assert!(spec.newgrf_runtime.is_some());
            (spec.class, spec.id)
        };
        state.current_station_class = class_id;
        state.current_station_spec = spec_id;

        let area_origin = TileCoord::new(3, 3);
        let area = Command::PlaceRailStationArea {
            origin: area_origin,
            axis_y: false,
            platforms: 1,
            length: 1,
        };
        assert_eq!(
            command_would_fail(&state, &area),
            Some(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(
            apply_command(&mut state, &area),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert!(state.stations.is_empty());
        assert_eq!(state.map.get_kind(area_origin), Some(TileKind::Grass));

        let single = TileCoord::new(2, 2);
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
        let one_tile = Command::PlaceRailStation(single, 0);
        assert_eq!(
            command_would_fail(&state, &one_tile),
            Some(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(
            apply_command(&mut state, &one_tile),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert!(state.stations.is_empty());
        assert_eq!(state.map.get_kind(single), Some(TileKind::Grass));
    }

    /// CB149 debe conservar la cadena Action0/3→Action2 cargada y bloquear
    /// tanto query como execute, para área y el comando ferroviario 1×1.
    #[test]
    fn station_slope_callback_runs_from_loaded_action2_graph() {
        use crate::command::{Command, CommandError, apply_command, command_would_fail};
        use crate::map::{TileCoord, TileKind};
        use crate::newgrf_sprites::{
            build_action2_callback_literal_payload, build_grf_v2_feature_with_action2_chain,
        };

        let action0 = build_action0_station_payload_with_callback_mask(
            b"CBSL",
            b"Spec",
            0,
            0,
            1 << 4,
            "Station slope callback",
        );
        let action2 = build_action2_callback_literal_payload(
            ACTION0_FEATURE_STATIONS,
            7,
            0, // No es FAILED ni 0x400: CB149 rechaza la pendiente/sitio.
        );
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_STATIONS,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            [b'C', b'L', 0, 1],
            "station-slope-cb",
        );
        let dir = tempfile_dir_with("station_slope_cb.grf", &bytes);
        let mut state = GameState::new(8, 8);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("station_slope_cb.grf", 0x434C_0001));
        apply_newgrf_stations(&mut state, &[&dir]);

        let (class_id, spec_id) = {
            let spec = state
                .station_spec_catalog
                .iter()
                .find(|spec| spec.from_newgrf)
                .unwrap();
            assert_eq!(spec.callback_mask, 1 << 4);
            assert!(spec.has_slope_check_callback());
            assert!(spec.newgrf_runtime.is_some());
            (spec.class, spec.id)
        };
        state.current_station_class = class_id;
        state.current_station_spec = spec_id;

        let area_origin = TileCoord::new(3, 3);
        let area = Command::PlaceRailStationArea {
            origin: area_origin,
            axis_y: true,
            platforms: 1,
            length: 1,
        };
        assert_eq!(
            command_would_fail(&state, &area),
            Some(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(
            apply_command(&mut state, &area),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert!(state.stations.is_empty());
        assert_eq!(state.map.get_kind(area_origin), Some(TileKind::Grass));

        let single = TileCoord::new(2, 2);
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
        let one_tile = Command::PlaceRailStation(single, 0);
        assert_eq!(
            command_would_fail(&state, &one_tile),
            Some(CommandError::NewGrfCallbackDenied)
        );
        assert_eq!(
            apply_command(&mut state, &one_tile),
            Err(CommandError::NewGrfCallbackDenied)
        );
        assert!(state.stations.is_empty());
        assert_eq!(state.map.get_kind(single), Some(TileKind::Grass));
    }

    /// CB14 debe sobrevivir la carga Action0/3→Action2 y seleccionar el
    /// tiletype que consumirá el renderer, conservando el eje de la tesela.
    #[test]
    fn station_draw_layout_callback_runs_from_loaded_action2_graph() {
        use crate::newgrf_sprites::{
            Action2EvalCtx, build_action2_callback_literal_payload,
            build_grf_v2_feature_with_action2_chain,
        };

        let action0 = build_action0_station_payload_with_callback_mask(
            b"CBDR",
            b"Spec",
            0,
            0,
            1 << 1,
            "Station draw callback",
        );
        let action2 = build_action2_callback_literal_payload(ACTION0_FEATURE_STATIONS, 7, 6);
        let bytes = build_grf_v2_feature_with_action2_chain(
            &action0,
            ACTION0_FEATURE_STATIONS,
            0,
            7,
            &action2,
            1,
            1,
            &[174],
            [b'C', b'D', 0, 1],
            "station-draw-cb",
        );
        let dir = tempfile_dir_with("station_draw_cb.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("station_draw_cb.grf", 0x4344_0001));
        apply_newgrf_stations(&mut state, &[&dir]);

        let spec = state
            .station_spec_catalog
            .iter()
            .find(|spec| spec.from_newgrf)
            .unwrap();
        assert_eq!(spec.callback_mask, 1 << 1);
        assert!(spec.has_draw_tile_layout_callback());
        let mut ctx = Action2EvalCtx::default();
        assert_eq!(
            crate::apply_station_draw_tile_layout_callback(spec, 3, true, &mut ctx),
            7,
        );
    }

    #[test]
    fn industry_tile_animation_properties_are_parsed_separately() {
        let tile = [
            0x00,
            ACTION0_FEATURE_INDUSTRYTILES,
            0x06,
            0x01,
            0x00,
            0x08,
            0x00,
            0x0E,
            0x03,
            0x0F,
            0x05,
            0x01,
            0x10,
            0x02,
            0x11,
            0x04,
            0x12,
            0x01,
        ];
        let meta = parse_action0_industry_tile_meta(&tile).unwrap();
        assert_eq!(meta.callback_mask, 0x03);
        assert_eq!(meta.animation_frames, 5);
        assert_eq!(meta.animation_status, 1);
        assert_eq!(meta.animation_speed, 2);
        assert_eq!(meta.animation_triggers, 4);
        assert_eq!(meta.animation_special_flags, 1);
    }

    /// #231: Action3 elige sprite de airport tile; sin Action3 cae a subst.
    #[test]
    fn airport_tiles_ac_action3_sprite_fallback() {
        use crate::airport_tile_spec::resolve_airport_tile_draw_gfx;
        use crate::newgrf_sprites::build_grf_v2_airport_tile_with_preview_sprite;

        let tile = build_action0_airport_tile_payload(0, 24, None, 0);
        let indices = sample_tile_indices();
        let bytes = build_grf_v2_airport_tile_with_preview_sprite(
            &tile,
            0,
            8,
            8,
            &indices,
            [b'A', b'T', 0, 1],
            "atile",
        );
        let dir = tempfile_dir_with("atile.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("atile.grf", 0x4154_0001));
        apply_newgrf_airport_tiles(&mut state, &[&dir]);
        assert_eq!(state.airport_tile_spec_catalog.len(), 1);
        let t0 = &state.airport_tile_spec_catalog[0];
        assert_eq!(t0.subst_id, 24);
        assert!(t0.has_newgrf_sprites());
        assert!(!t0.newgrf_views.is_empty());
        assert_eq!(
            resolve_airport_tile_draw_gfx(t0.gfx.as_u16(), &state.airport_tile_spec_catalog),
            t0.gfx.as_u16()
        );
        // Sin vistas → fallback subst; no contamina vecinos.
        let mut bare = t0.clone();
        bare.newgrf_views.clear();
        bare.newgrf_preview = None;
        bare.newgrf_runtime = None;
        let cat = vec![bare];
        assert_eq!(resolve_airport_tile_draw_gfx(cat[0].gfx.as_u16(), &cat), 24);
        assert_eq!(resolve_airport_tile_draw_gfx(23, &cat), 23);
    }

    /// #231: Airport purchase (`0xFF`) + default group usables en picker/preview.
    #[test]
    fn airports_ac_purchase_default_groups() {
        use crate::newgrf_sprites::build_grf_v2_airport_purchase_default_sprites;

        let air = build_action0_airport_payload(0, 0, &[(0, 0, 0)], 4, 3, "PickPort");
        let mut purchase = sample_tile_indices();
        purchase[8 * 4 + 4] = 10;
        let mut default = sample_tile_indices();
        default[8 * 4 + 4] = 200;
        let bytes = build_grf_v2_airport_purchase_default_sprites(
            &air,
            0,
            8,
            8,
            &purchase,
            &default,
            [b'A', b'P', 0, 2],
            "apick",
        );
        let dir = tempfile_dir_with("apick.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("apick.grf", 0x4150_0002));
        apply_newgrf_airports(&mut state, &[&dir]);
        assert_eq!(state.airport_spec_catalog.len(), 1);
        let def = &state.airport_spec_catalog[0];
        assert!(def.has_newgrf_sprites());
        assert!(!def.newgrf_views.is_empty());
        assert!(!def.newgrf_purchase_views.is_empty());
        assert_ne!(def.newgrf_purchase_views[0].rgba, def.newgrf_views[0].rgba);
        let preview = def.newgrf_preview_sprite().unwrap();
        assert_eq!(preview.rgba, def.newgrf_purchase_views[0].rgba);
    }

    /// #231: Cargo Action3 adjunta group/views al `CargoSpecDef` sin contaminar ids.
    #[test]
    fn cargoes_ac_action3_views() {
        use crate::newgrf_sprites::build_grf_v2_cargo_with_preview_sprite;

        let a0 = build_action0_cargo_payload(0, 1, b"PASS", "Pasajeros");
        let indices = sample_tile_indices();
        let bytes = build_grf_v2_cargo_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'C', b'G', 0, 1],
            "cview",
        );
        let dir = tempfile_dir_with("cview.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("cview.grf", 0x4347_0003));
        apply_newgrf_cargoes(&mut state, &[&dir]);
        assert_eq!(state.cargo_spec_catalog.len(), 1);
        let pass = &state.cargo_spec_catalog[0];
        assert_eq!(pass.label, "PASS");
        assert!(pass.has_newgrf_sprites());
        assert!(!pass.newgrf_views.is_empty());
        assert!(pass.newgrf_view(0).is_some());
        // Id vecino no inventado.
        assert!(crate::cargo_spec_def(&state.cargo_spec_catalog, 1).is_none());
    }
}
