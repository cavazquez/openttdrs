//! Tablas de traducción `NewGRF` cargo/rail/road/tram (Action0 `GlobalVar`).
//!
//! Feature `0x08`, props `0x09` (cargo), `0x12` (rail), `0x16` (road),
//! `0x17` (tram). Vars Action2: estaciones `42` (rail) y CB140 `18`
//! (cargo); road `45` (`__RRttrr`).

use crate::cargo::CargoType;
use crate::rail_type::RailType;
use crate::road_type::{RoadTramType, RoadType, RoadTypeDef};
use crate::world_gen::Climate;

/// Feature Action0: variables globales (`GSF_GLOBALVAR`).
pub const ACTION0_FEATURE_GLOBALVAR: u8 = 0x08;
/// Prop tabla de traducción de cargos (`CargoLabel`).
pub const PROP_CARGO_TRANSLATION: u8 = 0x09;
/// Prop tabla de traducción railtype.
pub const PROP_RAILTYPE_TRANSLATION: u8 = 0x12;
/// Prop tabla de traducción roadtype.
pub const PROP_ROADTYPE_TRANSLATION: u8 = 0x16;
/// Prop tabla de traducción tramtype.
pub const PROP_TRAMTYPE_TRANSLATION: u8 = 0x17;
/// Prop tabla de traducción de badges (`Badge`); sus valores son C-strings.
pub const PROP_BADGE_TRANSLATION: u8 = 0x18;

/// Etiqueta de 4 caracteres (`RailTypeLabel` / `RoadTypeLabel`).
pub type TypeLabel = [u8; 4];

/// Tablas locales de un GRF (índice → etiqueta).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrfTypeTranslationTables {
    /// Tabla Cargo Translation Table: id local → label (`PASS`, `COAL`, …).
    pub cargo: Vec<TypeLabel>,
    pub rail: Vec<TypeLabel>,
    pub road: Vec<TypeLabel>,
    pub tram: Vec<TypeLabel>,
    /// Tabla Badge Translation Table: índice local → etiqueta global.
    pub badges: Vec<String>,
}

impl GrfTypeTranslationTables {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cargo.is_empty()
            && self.rail.is_empty()
            && self.road.is_empty()
            && self.tram.is_empty()
            && self.badges.is_empty()
    }

    /// Fusiona tablas no vacías de `other` por rango; la última definición
    /// gana sólo en los índices que realmente declaró. `GlobalVar` permite
    /// instalar las tablas en varios bloques Action0.
    pub fn merge_from(&mut self, other: &Self) {
        merge_table(&mut self.cargo, &other.cargo);
        merge_table(&mut self.rail, &other.rail);
        merge_table(&mut self.road, &other.road);
        merge_table(&mut self.tram, &other.tram);
        merge_string_table(&mut self.badges, &other.badges);
    }
}

const INVALID_LABEL: TypeLabel = [0; 4];

fn merge_table(dest: &mut Vec<TypeLabel>, source: &[TypeLabel]) {
    if source.is_empty() {
        return;
    }
    if dest.len() < source.len() {
        dest.resize(source.len(), INVALID_LABEL);
    }
    for (idx, &label) in source.iter().enumerate() {
        if label != INVALID_LABEL {
            dest[idx] = label;
        }
    }
}

fn merge_string_table(dest: &mut Vec<String>, source: &[String]) {
    if source.is_empty() {
        return;
    }
    if dest.len() < source.len() {
        dest.resize(source.len(), String::new());
    }
    for (idx, label) in source.iter().enumerate() {
        if !label.is_empty() {
            dest[idx].clone_from(label);
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

/// Traducción `CargoType` global → slot local del GRF (`GRFFile::cargo_map`).
///
/// Un GRF con CTT explícita usa el índice del label. Sin ella, `OpenTTD` usa el
/// slot del clima para formatos anteriores a v7 y el `bitnum` global a partir
/// de v7. Una versión desconocida (`0`, común al crear fixtures antes de
/// escanear Action8) se interpreta como v8, el formato vigente seguro.
#[must_use]
pub fn local_cargo_id(
    tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    cargo: CargoType,
    climate: Climate,
) -> u8 {
    if let Some(tables) = tables
        && !tables.cargo.is_empty()
    {
        return reverse_in_table(&tables.cargo, cargo.label_u32().to_be_bytes());
    }
    if (1..7).contains(&grf_version) {
        cargo.climate_slot(climate).unwrap_or(0xFF)
    } else {
        cargo.bitnum()
    }
}

/// Traduce el parámetro local de una variable Action2 de carga al tipo global.
///
/// `OpenTTD` usa la CTT explícita cuando existe; para GRF antiguos usa el slot
/// del clima y, desde `GRFv7`, el `bitnum` global. Mantener la inversa junto a
/// [`local_cargo_id`] evita que los scopes de estación inventen un orden local
/// distinto al usado por los callbacks.
#[must_use]
pub fn cargo_from_local_id(
    tables: Option<&GrfTypeTranslationTables>,
    grf_version: u8,
    local_id: u8,
    climate: Climate,
) -> Option<CargoType> {
    if let Some(tables) = tables
        && !tables.cargo.is_empty()
    {
        let label = *tables.cargo.get(usize::from(local_id))?;
        if label == INVALID_LABEL {
            return None;
        }
        return CargoType::from_label(std::str::from_utf8(&label).ok()?);
    }
    if (1..7).contains(&grf_version) {
        return CargoType::from_climate_slot(climate, local_id);
    }
    CargoType::for_climate(climate)
        .iter()
        .copied()
        .find(|cargo| cargo.bitnum() == local_id)
}

/// Traducción directa índice local GRF → `RailType` (`GetRailTypeTranslation`).
///
/// Sin tabla o índice fuera de rango: `None`. Etiqueta desconocida: `None`.
#[must_use]
pub fn forward_rail_type(tables: Option<&GrfTypeTranslationTables>, index: u8) -> Option<RailType> {
    let tables = tables?;
    let label = *tables.rail.get(usize::from(index))?;
    rail_type_from_label(label)
}

/// Traducción directa índice local → `RoadType` según clase road/tram.
#[must_use]
pub fn forward_road_type(
    tables: Option<&GrfTypeTranslationTables>,
    catalog: &[RoadTypeDef],
    class: RoadTramType,
    index: u8,
) -> Option<RoadType> {
    let tables = tables?;
    let list = match class {
        RoadTramType::Road => &tables.road,
        RoadTramType::Tram => &tables.tram,
    };
    let label = *list.get(usize::from(index))?;
    catalog
        .iter()
        .find(|d| d.class == class && road_type_label(d) == label)
        .map(|d| d.id)
}

#[must_use]
pub const fn rail_type_from_label(label: TypeLabel) -> Option<RailType> {
    match &label {
        b"RAIL" => Some(RailType::Rail),
        b"ELRL" => Some(RailType::Electric),
        b"MONO" => Some(RailType::Monorail),
        b"MGLV" => Some(RailType::Maglev),
        _ => None,
    }
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
    if feature != ACTION0_FEATURE_GLOBALVAR || num_ids == 0 {
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
            PROP_CARGO_TRANSLATION
            | PROP_RAILTYPE_TRANSLATION
            | PROP_ROADTYPE_TRANSLATION
            | PROP_TRAMTYPE_TRANSLATION => {
                let need = usize::from(num_ids).saturating_mul(4);
                if i + need > payload.len() {
                    break;
                }
                let mut labels = vec![INVALID_LABEL; usize::from(first_id) + usize::from(num_ids)];
                for offset in 0..num_ids {
                    let mut lab = [0u8; 4];
                    lab.copy_from_slice(&payload[i..i + 4]);
                    i += 4;
                    labels[usize::from(first_id) + usize::from(offset)] = lab;
                }
                match prop {
                    PROP_CARGO_TRANSLATION => out.cargo = labels,
                    PROP_RAILTYPE_TRANSLATION => out.rail = labels,
                    PROP_ROADTYPE_TRANSLATION => out.road = labels,
                    _ => out.tram = labels,
                }
                any = true;
            }
            PROP_BADGE_TRANSLATION => {
                let mut labels = vec![String::new(); usize::from(first_id) + usize::from(num_ids)];
                for offset in 0..num_ids {
                    let end = payload[i..].iter().position(|&b| b == 0)?;
                    labels[usize::from(first_id) + usize::from(offset)] =
                        String::from_utf8_lossy(&payload[i..i + end]).to_string();
                    i += end + 1;
                }
                out.badges = labels;
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
        assert_eq!(forward_rail_type(Some(&t), 0), Some(RailType::Electric));
        assert_eq!(forward_rail_type(Some(&t), 1), Some(RailType::Rail));
        assert_eq!(forward_rail_type(Some(&t), 2), Some(RailType::Monorail));
        assert_eq!(forward_rail_type(Some(&t), 3), None);
    }

    #[test]
    fn cargo_translation_uses_explicit_ctt_then_versioned_fallback() {
        let payload = build_action0_type_translation_payload(
            PROP_CARGO_TRANSLATION,
            &[*b"MAIL", *b"GOOD", *b"COAL"],
        );
        let tables = parse_action0_type_translation_tables(&payload).unwrap();
        assert_eq!(tables.cargo, vec![*b"MAIL", *b"GOOD", *b"COAL"]);
        assert_eq!(
            local_cargo_id(Some(&tables), 8, CargoType::Coal, Climate::Temperate),
            2
        );
        assert_eq!(
            local_cargo_id(Some(&tables), 8, CargoType::Passengers, Climate::Temperate),
            0xFF
        );
        assert_eq!(
            local_cargo_id(None, 8, CargoType::Paper, Climate::SubArctic),
            11,
            "v7+ usa bitnum, no CargoType::cargo_id"
        );
        assert_eq!(
            local_cargo_id(None, 6, CargoType::Paper, Climate::SubArctic),
            9,
            "v6 conserva el slot clásico del clima"
        );
    }

    #[test]
    fn cargo_translation_inverse_matches_ctt_and_climate_fallbacks() {
        let tables = GrfTypeTranslationTables {
            cargo: vec![*b"PASS", *b"COAL", *b"WOOD"],
            ..GrfTypeTranslationTables::default()
        };
        assert_eq!(
            cargo_from_local_id(Some(&tables), 8, 2, Climate::Temperate),
            Some(CargoType::Wood)
        );
        assert_eq!(
            cargo_from_local_id(None, 6, 9, Climate::SubArctic),
            Some(CargoType::Paper)
        );
        assert_eq!(
            cargo_from_local_id(None, 8, CargoType::Paper.bitnum(), Climate::SubArctic),
            Some(CargoType::Paper)
        );
        assert_eq!(cargo_from_local_id(None, 8, 0xFF, Climate::Temperate), None);
    }

    #[test]
    fn translation_ranges_merge_without_erasing_earlier_slots() {
        let first = vec![
            0x00,
            ACTION0_FEATURE_GLOBALVAR,
            0x01,
            0x02,
            0x00,
            PROP_CARGO_TRANSLATION,
            b'P',
            b'A',
            b'S',
            b'S',
            b'M',
            b'A',
            b'I',
            b'L',
        ];
        let second = vec![
            0x00,
            ACTION0_FEATURE_GLOBALVAR,
            0x01,
            0x01,
            0x02,
            PROP_CARGO_TRANSLATION,
            b'C',
            b'O',
            b'A',
            b'L',
        ];
        let mut merged = parse_action0_type_translation_tables(&first).unwrap();
        merged.merge_from(&parse_action0_type_translation_tables(&second).unwrap());
        assert_eq!(merged.cargo, vec![*b"PASS", *b"MAIL", *b"COAL"]);
    }

    #[test]
    fn badge_translation_table_reads_local_cstrings() {
        let payload = vec![
            0x00,
            ACTION0_FEATURE_GLOBALVAR,
            0x01,
            0x02,
            0x00,
            PROP_BADGE_TRANSLATION,
            b'E',
            b'L',
            b'E',
            b'C',
            0,
            b'D',
            b'I',
            b'E',
            b'S',
            0,
        ];
        let tables = parse_action0_type_translation_tables(&payload).unwrap();
        assert_eq!(tables.badges, vec!["ELEC", "DIES"]);
        assert!(!tables.is_empty());
    }

    #[test]
    fn forward_reverse_rail_roundtrip() {
        let t = GrfTypeTranslationTables {
            rail: vec![*b"RAIL", *b"ELRL", *b"MONO", *b"MGLV"],
            ..Default::default()
        };
        for rt in [
            RailType::Rail,
            RailType::Electric,
            RailType::Monorail,
            RailType::Maglev,
        ] {
            let idx = reverse_rail_type(Some(&t), rt);
            assert_ne!(idx, 0xFF);
            assert_eq!(forward_rail_type(Some(&t), idx), Some(rt));
        }
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
