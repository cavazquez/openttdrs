//! Perfiles FTA por `AirportSpecId`.

use crate::airport_class::AirportSpecId;

use super::city::{CITY_ENTRIES, CITY_MOVING_DATA, city_fta_edges};
use super::commuter::{COMMUTER_ENTRIES, COMMUTER_MOVING_DATA, commuter_fta_edges};
use super::country::{COUNTRY_ENTRIES, COUNTRY_MOVING_DATA, country_fta_edges};
use super::helidepot::{HELIDEPOT_ENTRIES, HELIDEPOT_MOVING_DATA, helidepot_fta_edges};
use super::heliport::{HELIPORT_ENTRIES, HELIPORT_MOVING_DATA, heliport_fta_edges};
use super::intercontinental::{
    INTERCONTINENTAL_ENTRIES, INTERCONTINENTAL_MOVING_DATA, intercontinental_fta_edges,
};
use super::international::{
    INTERNATIONAL_ENTRIES, INTERNATIONAL_MOVING_DATA, international_fta_edges,
};
use super::metropolitan::{METROPOLITAN_ENTRIES, METROPOLITAN_MOVING_DATA, metropolitan_fta_edges};
use super::oilrig::{OILRIG_ENTRIES, OILRIG_MOVING_DATA, oilrig_fta_edges};
use super::types::{AirportFtaKind, AirportFtaProfile};

/// Perfil FTA si el spec está soportado en este corte.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn fta_profile_for_spec(spec: AirportSpecId) -> Option<AirportFtaProfile> {
    match spec {
        AirportSpecId::Small => Some(AirportFtaProfile {
            kind: AirportFtaKind::Country,
            spec,
            moving_data: &COUNTRY_MOVING_DATA,
            entries: COUNTRY_ENTRIES,
            fta_edges: country_fta_edges,
            fixedwing_takeoff_pos: Some(9),
            hold_min: 15,
            hold_max: 18,
            footprint_w: 4,
            footprint_h: 3,
        }),
        AirportSpecId::Helidepot => Some(AirportFtaProfile {
            kind: AirportFtaKind::Helidepot,
            spec,
            moving_data: &HELIDEPOT_MOVING_DATA,
            entries: HELIDEPOT_ENTRIES,
            fta_edges: helidepot_fta_edges,
            fixedwing_takeoff_pos: None,
            hold_min: 3,
            hold_max: 6,
            footprint_w: 2,
            footprint_h: 2,
        }),
        AirportSpecId::Heliport => Some(AirportFtaProfile {
            kind: AirportFtaKind::Heliport,
            spec,
            moving_data: &HELIPORT_MOVING_DATA,
            entries: HELIPORT_ENTRIES,
            fta_edges: heliport_fta_edges,
            fixedwing_takeoff_pos: None,
            hold_min: 5,
            hold_max: 8,
            footprint_w: 1,
            footprint_h: 1,
        }),
        AirportSpecId::Oilrig => Some(AirportFtaProfile {
            kind: AirportFtaKind::Heliport,
            spec,
            moving_data: &OILRIG_MOVING_DATA,
            entries: OILRIG_ENTRIES,
            fta_edges: oilrig_fta_edges,
            fixedwing_takeoff_pos: None,
            hold_min: 5,
            hold_max: 8,
            footprint_w: 1,
            footprint_h: 1,
        }),
        AirportSpecId::Commuter => Some(AirportFtaProfile {
            kind: AirportFtaKind::Commuter,
            spec,
            moving_data: &COMMUTER_MOVING_DATA,
            entries: COMMUTER_ENTRIES,
            fta_edges: commuter_fta_edges,
            fixedwing_takeoff_pos: Some(15),
            hold_min: 21,
            hold_max: 24,
            footprint_w: 5,
            footprint_h: 4,
        }),
        AirportSpecId::City => Some(AirportFtaProfile {
            kind: AirportFtaKind::City,
            spec,
            moving_data: &CITY_MOVING_DATA,
            entries: CITY_ENTRIES,
            fta_edges: city_fta_edges,
            fixedwing_takeoff_pos: Some(12),
            hold_min: 18,
            hold_max: 21,
            footprint_w: 6,
            footprint_h: 6,
        }),
        AirportSpecId::Metropolitan => Some(AirportFtaProfile {
            kind: AirportFtaKind::Metropolitan,
            spec,
            moving_data: &METROPOLITAN_MOVING_DATA,
            entries: METROPOLITAN_ENTRIES,
            fta_edges: metropolitan_fta_edges,
            fixedwing_takeoff_pos: Some(12),
            hold_min: 19,
            hold_max: 22,
            footprint_w: 6,
            footprint_h: 6,
        }),
        AirportSpecId::International => Some(AirportFtaProfile {
            kind: AirportFtaKind::International,
            spec,
            moving_data: &INTERNATIONAL_MOVING_DATA,
            entries: INTERNATIONAL_ENTRIES,
            fta_edges: international_fta_edges,
            fixedwing_takeoff_pos: Some(31),
            hold_min: 37,
            hold_max: 40,
            footprint_w: 7,
            footprint_h: 7,
        }),
        AirportSpecId::Intercontinental => Some(AirportFtaProfile {
            kind: AirportFtaKind::Intercontinental,
            spec,
            moving_data: &INTERCONTINENTAL_MOVING_DATA,
            entries: INTERCONTINENTAL_ENTRIES,
            fta_edges: intercontinental_fta_edges,
            fixedwing_takeoff_pos: Some(35),
            hold_min: 43,
            hold_max: 46,
            footprint_w: 9,
            footprint_h: 11,
        }),
    }
}
