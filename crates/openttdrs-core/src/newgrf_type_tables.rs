//! Tablas de traducción `NewGRF` rail/road/tram (Action0 `GlobalVar`).
//!
//! Feature `0x08`, props `0x12` (rail), `0x16` (road), `0x17` (tram).
//! Vars Action2: estaciones `42` (rail), road `45` (`__RRttrr`).

use crate::rail_type::RailType;
use crate::road_type::{RoadTramType, RoadType, RoadTypeDef};

/// Feature Action0: variables globales (`GSF_GLOBALVAR`).
pub const ACTION0_FEATURE_GLOBALVAR: u8 = 0x08;
/// Prop tabla de traducción railtype.
pub const PROP_RAILTYPE_TRANSLATION: u8 = 0x12;
/// Prop tabla de traducción roadtype.
pub const PROP_ROADTYPE_TRANSLATION: u8 = 0x16;
/// Prop tabla de traducción tramtype.
pub const PROP_TRAMTYPE_TRANSLATION: u8 = 0x17;

/// Etiqueta de 4 caracteres (`RailTypeLabel` / `RoadTypeLabel`).
pub type TypeLabel = [u8; 4];

/// Tablas locales de un GRF (índice → etiqueta).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrfTypeTranslationTables {
    pub rail: Vec<TypeLabel>,
    pub road: Vec<TypeLabel>,
    pub tram: Vec<TypeLabel>,
}

impl GrfTypeTranslationTables {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rail.is_empty() && self.road.is_empty() && self.tram.is_empty()
    }

    /// Fusiona tablas no vacías de `other` (última definición gana por feature).
    pub fn merge_from(&mut self, other: &Self) {
        if !other.rail.is_empty() {
            self.rail.clone_from(&other.rail);
        }
        if !other.road.is_empty() {
            self.road.clone_from(&other.road);
        }
        if !other.tram.is_empty() {
            self.tram.clone_from(&other.tram);
        }
    }
}

/// Etiqueta vanilla / `OpenTTD` de un `RailType`.
#[must_use]
pub const fn rail_type_label(rt: RailType) -> TypeLabel {
    match rt {
        RailType::Rail => *b"RAIL",
        RailType::Electric => *b"ELRL",
        RailType::Monorail => *b"MONO",
        RailType::Maglev => *b"MGLV",
    }
}

fn label_from_short(short: &str) -> TypeLabel {
    let b = short.as_bytes();
    let mut out = [b' '; 4];
    for (i, c) in b.iter().take(4).enumerate() {
        out[i] = *c;
    }
    out
}

/// Etiqueta de un road/tram type (vanilla `ROAD`/`ELRL`; `NewGRF` = `short_label`).
#[must_use]
pub fn road_type_label(def: &RoadTypeDef) -> TypeLabel {
    if !def.from_newgrf {
        return match def.id {
            RoadType::ROAD => *b"ROAD",
            // OpenTTD: `ROADTYPE_LABEL_TRAM = 'ELRL'`.
            RoadType::TRAM => *b"ELRL",
            _ => label_from_short(&def.short_label),
        };
    }
    label_from_short(&def.short_label)
}

fn reverse_in_table(table: &[TypeLabel], label: TypeLabel) -> u8 {
    table
        .iter()
        .position(|l| *l == label)
        .map_or(0xFF, |i| u8::try_from(i).unwrap_or(0xFF))
}

/// Traducción inversa rail → índice local GRF (`GetReverseRailTypeTranslation`).
///
/// Sin tabla: ID global tal cual. Con tabla y sin match: `0xFF`.
#[must_use]
pub fn reverse_rail_type(tables: Option<&GrfTypeTranslationTables>, rt: RailType) -> u8 {
    let Some(tables) = tables else {
        return rt.as_u8();
    };
    if tables.rail.is_empty() {
        return rt.as_u8();
    }
    reverse_in_table(&tables.rail, rail_type_label(rt))
}

/// Traducción inversa road/tram → índice local (`GetReverseRoadTypeTranslation`).
#[must_use]
pub fn reverse_road_type(
    tables: Option<&GrfTypeTranslationTables>,
    catalog: &[RoadTypeDef],
    rt: RoadType,
) -> u8 {
    let Some(def) = catalog.iter().find(|d| d.id == rt) else {
        return 0xFF;
    };
    let Some(tables) = tables else {
        return rt.as_u8();
    };
    let list = match def.class {
        RoadTramType::Road => &tables.road,
        RoadTramType::Tram => &tables.tram,
    };
    if list.is_empty() {
        return rt.as_u8();
    }
    reverse_in_table(list, road_type_label(def))
}

/// Como `OpenTTD` `GetTrackTypes`: presente pero no en tabla → `0xFE`.
#[must_use]
pub fn reverse_road_type_for_var45(
    tables: Option<&GrfTypeTranslationTables>,
    catalog: &[RoadTypeDef],
    rt: RoadType,
) -> u8 {
    let v = reverse_road_type(tables, catalog, rt);
    if v == 0xFF { 0xFE } else { v }
}

/// Como `OpenTTD` `GetTrackTypes` para rail en var 45.
#[must_use]
pub fn reverse_rail_type_for_var45(tables: Option<&GrfTypeTranslationTables>, rt: RailType) -> u8 {
    let v = reverse_rail_type(tables, rt);
    if v == 0xFF { 0xFE } else { v }
}

/// Parsea Action0 `GlobalVar` con props de tablas de traducción.
#[must_use]
pub fn parse_action0_type_translation_tables(payload: &[u8]) -> Option<GrfTypeTranslationTables> {
    if payload.len() < 5 || payload[0] != 0x00 {
        return None;
    }
    let feature = payload[1];
    let num_props = payload[2];
    let num_ids = payload[3];
    let first_id = payload[4];
    if feature != ACTION0_FEATURE_GLOBALVAR || num_ids == 0 || first_id != 0 {
        return None;
    }
    let mut i = 5usize;
    let mut out = GrfTypeTranslationTables::default();
    let mut any = false;
    for _ in 0..num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_RAILTYPE_TRANSLATION | PROP_ROADTYPE_TRANSLATION | PROP_TRAMTYPE_TRANSLATION => {
                let need = usize::from(num_ids).saturating_mul(4);
                if i + need > payload.len() {
                    break;
                }
                let mut labels = Vec::with_capacity(usize::from(num_ids));
                for _ in 0..num_ids {
                    let mut lab = [0u8; 4];
                    lab.copy_from_slice(&payload[i..i + 4]);
                    i += 4;
                    labels.push(lab);
                }
                match prop {
                    PROP_RAILTYPE_TRANSLATION => out.rail = labels,
                    PROP_ROADTYPE_TRANSLATION => out.road = labels,
                    _ => out.tram = labels,
                }
                any = true;
            }
            _ => break,
        }
    }
    any.then_some(out)
}

/// Recoge tablas de traducción de un `.grf` (última no vacía por feature gana).
#[must_use]
pub fn collect_type_tables_from_grf(data: &[u8]) -> GrfTypeTranslationTables {
    let mut out = GrfTypeTranslationTables::default();
    let _ = crate::newgrf_actions::for_each_pseudo_payload(data, |payload| {
        if let Some(partial) = parse_action0_type_translation_tables(payload) {
            out.merge_from(&partial);
        }
    });
    out
}

/// Construye payload Action0 `GlobalVar` con una tabla (tests / GRFs sintéticos).
#[must_use]
pub fn build_action0_type_translation_payload(prop: u8, labels: &[[u8; 4]]) -> Vec<u8> {
    let n = u8::try_from(labels.len()).unwrap_or(0);
    let mut p = vec![0x00, ACTION0_FEATURE_GLOBALVAR, 0x01, n, 0x00, prop];
    for lab in labels {
        p.extend_from_slice(lab);
    }
    p
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_actions::build_grf_v2_with_action0_and_action8;
    use crate::road_type::vanilla_road_type_catalog;

    #[test]
    fn parse_rail_translation_table() {
        let payload = build_action0_type_translation_payload(
            PROP_RAILTYPE_TRANSLATION,
            &[*b"ELRL", *b"RAIL", *b"MONO"],
        );
        let t = parse_action0_type_translation_tables(&payload).unwrap();
        assert_eq!(t.rail, vec![*b"ELRL", *b"RAIL", *b"MONO"]);
        assert!(t.road.is_empty());
        // Índice local: Rail=1, Electric=0, Maglev ausente=0xFF
        assert_eq!(reverse_rail_type(Some(&t), RailType::Electric), 0);
        assert_eq!(reverse_rail_type(Some(&t), RailType::Rail), 1);
        assert_eq!(reverse_rail_type(Some(&t), RailType::Maglev), 0xFF);
        assert_eq!(reverse_rail_type(None, RailType::Rail), 0);
    }

    #[test]
    fn reverse_road_var45_fe_when_missing() {
        let catalog = vanilla_road_type_catalog();
        let t = GrfTypeTranslationTables {
            road: vec![*b"COBB"],
            ..Default::default()
        };
        assert_eq!(
            reverse_road_type_for_var45(Some(&t), &catalog, RoadType::ROAD),
            0xFE
        );
        assert_eq!(reverse_road_type(Some(&t), &catalog, RoadType::ROAD), 0xFF);
        // Sin tabla: ID global.
        assert_eq!(reverse_road_type(None, &catalog, RoadType::ROAD), 0);
    }

    #[test]
    fn collect_from_grf_bytes() {
        let a0 = build_action0_type_translation_payload(
            PROP_ROADTYPE_TRANSLATION,
            &[*b"ROAD", *b"COBB"],
        );
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'B', 0x47, 0x01], "T", "D");
        let tables = collect_type_tables_from_grf(&bytes);
        assert_eq!(tables.road, vec![*b"ROAD", *b"COBB"]);
    }
}
