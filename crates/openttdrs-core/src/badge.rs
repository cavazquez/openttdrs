//! Badges `NewGRF` (`Badges`, feature Action0 `0x15`).
//!
//! Catálogo runtime parcial: etiqueta + flags; asociaciones a roadstops/objects.

use serde::{Deserialize, Serialize};

/// Spec de badge definido por Action0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeDef {
    pub id: u16,
    pub label: String,
    pub flags: u32,
    pub from_newgrf: bool,
    /// GRFID del set que lo registró primero (`0` = vanilla / sin set).
    #[serde(default)]
    pub grfid: u32,
}

/// Catálogo vacío (no hay badges vanilla).
#[must_use]
pub fn empty_badge_catalog() -> Vec<BadgeDef> {
    Vec::new()
}

/// Siguiente id libre en el catálogo.
#[must_use]
pub fn next_free_badge_id(catalog: &[BadgeDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn badge_def(catalog: &[BadgeDef], id: u16) -> Option<&BadgeDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Lista badges del catálogo filtrando por etiqueta (subcadena, case-insensitive).
#[must_use]
pub fn list_badges<'a>(catalog: &'a [BadgeDef], filter: &str) -> Vec<&'a BadgeDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
        .iter()
        .filter(|b| needle.is_empty() || b.label.to_ascii_lowercase().contains(&needle))
        .collect()
}

/// Resuelve ids de asociación a entradas del catálogo (omite ids desconocidos).
#[must_use]
pub fn badges_for_spec<'a>(ids: &[u16], badge_catalog: &'a [BadgeDef]) -> Vec<&'a BadgeDef> {
    ids.iter()
        .filter_map(|&id| badge_def(badge_catalog, id))
        .collect()
}

/// Resuelve etiquetas de badge a ids del catálogo (mismo GRF primero, luego cualquiera).
///
/// Etiquetas sin match se omiten (sin panic). Ver [`resolve_badge_labels_detailed`]
/// para obtener también las no resueltas (diagnósticos).
#[must_use]
pub fn resolve_badge_labels(
    labels: &[String],
    badge_catalog: &[BadgeDef],
    preferred_grfid: u32,
) -> Vec<u16> {
    resolve_badge_labels_detailed(labels, badge_catalog, preferred_grfid).0
}

/// Como [`resolve_badge_labels`], pero también devuelve etiquetas sin match.
#[must_use]
pub fn resolve_badge_labels_detailed(
    labels: &[String],
    badge_catalog: &[BadgeDef],
    preferred_grfid: u32,
) -> (Vec<u16>, Vec<String>) {
    let mut out = Vec::with_capacity(labels.len());
    let mut unresolved = Vec::new();
    for label in labels {
        if let Some(b) = badge_catalog
            .iter()
            .find(|b| b.grfid == preferred_grfid && b.label.eq_ignore_ascii_case(label))
        {
            out.push(b.id);
            continue;
        }
        if let Some(b) = badge_catalog
            .iter()
            .find(|b| b.label.eq_ignore_ascii_case(label))
        {
            out.push(b.id);
            continue;
        }
        unresolved.push(label.clone());
    }
    (out, unresolved)
}

/// Resuelve una Badge Translation Table local (`GlobalVar` prop `0x18`).
///
/// Se conserva una entrada `u16::MAX` cuando el label no existe para que los
/// índices locales no se desplacen; `OpenTTD` mantiene la misma posición y
/// devuelve un resultado no disponible al consultarlo.
#[must_use]
pub fn resolve_badge_translation_table(
    labels: &[String],
    badge_catalog: &[BadgeDef],
    preferred_grfid: u32,
) -> (Vec<u16>, Vec<String>) {
    let mut translation = Vec::with_capacity(labels.len());
    let mut unresolved = Vec::new();
    for label in labels {
        let badge = badge_catalog
            .iter()
            .find(|badge| badge.grfid == preferred_grfid && badge.label.eq_ignore_ascii_case(label))
            .or_else(|| {
                badge_catalog
                    .iter()
                    .find(|badge| badge.label.eq_ignore_ascii_case(label))
            });
        if let Some(badge) = badge {
            translation.push(badge.id);
        } else {
            translation.push(u16::MAX);
            unresolved.push(label.clone());
        }
    }
    (translation, unresolved)
}

/// Convierte índices locales de una `ReadBadgeList` a ids globales.
#[must_use]
pub fn resolve_badge_local_ids(
    local_ids: &[u16],
    labels: &[String],
    badge_catalog: &[BadgeDef],
    preferred_grfid: u32,
) -> (Vec<u16>, Vec<u16>, Vec<String>) {
    let (translation, mut unresolved) =
        resolve_badge_translation_table(labels, badge_catalog, preferred_grfid);
    let mut badges = Vec::new();
    for &local_id in local_ids {
        let Some(global_id) = translation.get(usize::from(local_id)).copied() else {
            unresolved.push(format!(
                "índice local {local_id} fuera de Badge Translation Table"
            ));
            continue;
        };
        if global_id == u16::MAX {
            continue;
        }
        if !badges.contains(&global_id) {
            badges.push(global_id);
        }
    }
    (badges, translation, unresolved)
}

/// Busca un badge por etiqueta (case-insensitive).
#[must_use]
pub fn find_badge_by_label<'a>(catalog: &'a [BadgeDef], label: &str) -> Option<&'a BadgeDef> {
    catalog.iter().find(|b| b.label.eq_ignore_ascii_case(label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_resolve_badges_no_collision() {
        let catalog = vec![
            BadgeDef {
                id: 0,
                label: "ELEC".into(),
                flags: 0,
                from_newgrf: true,
                grfid: 1,
            },
            BadgeDef {
                id: 1,
                label: "DIESEL".into(),
                flags: 0,
                from_newgrf: true,
                grfid: 1,
            },
        ];
        assert_eq!(list_badges(&catalog, "").len(), 2);
        assert_eq!(list_badges(&catalog, "ele").len(), 1);
        assert_ne!(catalog[0].label, catalog[1].label);
        let (ids, unresolved) = resolve_badge_labels_detailed(
            &["ELEC".into(), "NOPE".into(), "DIESEL".into()],
            &catalog,
            1,
        );
        assert_eq!(ids, vec![0, 1]);
        assert_eq!(unresolved, vec!["NOPE".to_string()]);
        assert_eq!(badges_for_spec(&ids, &catalog).len(), 2);
        assert!(find_badge_by_label(&catalog, "elec").is_some());
    }
}
