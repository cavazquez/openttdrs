//! Perfiles FTA por `AirportSpecId`.

use crate::airport_class::AirportSpecId;

use super::country::{COUNTRY_ENTRIES, COUNTRY_MOVING_DATA, country_fta_edges};
use super::helidepot::{HELIDEPOT_ENTRIES, HELIDEPOT_MOVING_DATA, helidepot_fta_edges};
use super::types::{AirportFtaKind, AirportFtaProfile};

/// Perfil FTA si el spec está soportado en este corte.
#[must_use]
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
        _ => None,
    }
}
