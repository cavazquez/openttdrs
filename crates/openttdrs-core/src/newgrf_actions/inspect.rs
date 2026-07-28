//! Inspección parse-only de archivos `.grf` sin aplicar.

use std::path::Path;

use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_container};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::action0::{
    ACTION0_FEATURE_BADGES, ACTION0_FEATURE_OBJECTS, ACTION0_FEATURE_ROADSTOPS,
    parse_action0_badge_meta, parse_action0_header, parse_action0_object_meta,
    parse_action0_roadstop_meta,
};

/// Resumen de un bloque Action5 para Inspeccionar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action5SlotSummary {
    pub type_id: u8,
    pub num_sprites: u8,
    pub offset: u16,
    pub preview_wh: Option<(u16, u16)>,
}

/// Informe de inspección de un `.grf` (sin aplicar).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrfInspectReport {
    pub container: Option<GrfContainerVersion>,
    pub action_counts: [u32; 16],
    pub action0_features: Vec<u8>,
    pub action5_slots: Vec<Action5SlotSummary>,
    pub pseudo_sprites: u32,
    pub real_sprites: u32,
    pub warnings: Vec<String>,
    /// Labels de badges definidos (`Action0` feature `0x15`).
    pub badge_labels: Vec<String>,
    /// Asociaciones badge vistas en roadstops/objects (`prop 0xFD`).
    pub badge_associations: Vec<String>,
}

impl GrfInspectReport {
    #[must_use]
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        match self.container {
            Some(GrfContainerVersion::V1) => lines.push("Contenedor: v1".into()),
            Some(GrfContainerVersion::V2) => lines.push("Contenedor: v2".into()),
            None => lines.push("Contenedor: ?".into()),
        }
        lines.push(format!(
            "Pseudo: {} · reales: {}",
            self.pseudo_sprites, self.real_sprites
        ));
        let mut hist = Vec::new();
        for (action, count) in self.action_counts.iter().enumerate() {
            if *count > 0 {
                hist.push(format!("A{action:X}={count}"));
            }
        }
        if hist.is_empty() {
            lines.push("Acciones: (ninguna)".into());
        } else {
            lines.push(format!("Acciones: {}", hist.join(" ")));
        }
        if !self.action0_features.is_empty() {
            let feats: Vec<_> = self
                .action0_features
                .iter()
                .map(|f| format!("0x{f:02X}"))
                .collect();
            lines.push(format!("Action0 features: {}", feats.join(", ")));
        }
        if !self.badge_labels.is_empty() {
            lines.push(format!("Badges: {}", self.badge_labels.join(", ")));
        }
        if !self.badge_associations.is_empty() {
            lines.push(format!(
                "Badge assoc: {}",
                self.badge_associations.join("; ")
            ));
        }
        if !self.action5_slots.is_empty() {
            let slots: Vec<_> = self
                .action5_slots
                .iter()
                .map(|s| {
                    let name = crate::newgrf_sprites::action5_type_name(s.type_id);
                    let preview = s
                        .preview_wh
                        .map(|(w, h)| format!(" {w}×{h}"))
                        .unwrap_or_default();
                    format!(
                        "0x{:02X}×{} @{} ({name}){preview}",
                        s.type_id, s.num_sprites, s.offset
                    )
                })
                .collect();
            lines.push(format!("Action5: {}", slots.join("; ")));
        }
        for w in &self.warnings {
            lines.push(format!("! {w}"));
        }
        lines.join("\n")
    }
}

/// Inspecciona bytes de un `.grf` (parse-only).
///
/// # Errors
///
/// Contenedor inválido / demasiado corto.
pub fn inspect_grf_bytes(data: &[u8]) -> Result<GrfInspectReport, GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut report = GrfInspectReport {
        container: Some(container),
        ..Default::default()
    };
    walk_data_section(section, container, &mut report);
    if let Ok(blocks) = crate::newgrf_sprites::collect_action5_blocks(data) {
        report.action5_slots = blocks
            .into_iter()
            .map(|b| Action5SlotSummary {
                type_id: b.type_id,
                num_sprites: b.num_sprites,
                offset: b.offset,
                preview_wh: b.first_preview.as_ref().map(|s| (s.width, s.height)),
            })
            .collect();
    }
    Ok(report)
}

/// # Errors
///
/// E/S o contenedor inválido.
pub fn inspect_grf_file(path: &Path) -> Result<GrfInspectReport, GrfScanError> {
    let data = std::fs::read(path).map_err(|e| GrfScanError::Io(e.to_string()))?;
    inspect_grf_bytes(&data)
}

fn walk_data_section(
    data_section: &[u8],
    container: GrfContainerVersion,
    report: &mut GrfInspectReport,
) {
    walk_grf_entries(data_section, container, |entry| match entry {
        GrfEntry::Pseudo(payload) => {
            report.pseudo_sprites = report.pseudo_sprites.saturating_add(1);
            process_pseudo_payload(payload, report);
        }
        GrfEntry::Real { .. } => {
            report.real_sprites = report.real_sprites.saturating_add(1);
        }
    });
}

fn process_pseudo_payload(payload: &[u8], report: &mut GrfInspectReport) {
    let Some(&action) = payload.first() else {
        report.warnings.push("pseudo vacío".into());
        return;
    };
    if action == 0xFF {
        return;
    }
    if action <= 0x0F {
        report.action_counts[usize::from(action)] =
            report.action_counts[usize::from(action)].saturating_add(1);
    }
    if action == 0x00 {
        match parse_action0_header(payload) {
            Some(h) => {
                if !report.action0_features.contains(&h.feature) {
                    report.action0_features.push(h.feature);
                }
                inspect_action0_badges(payload, h.feature, report);
            }
            None => report
                .warnings
                .push("Action0 con cabecera incompleta".into()),
        }
    }
}

fn inspect_action0_badges(payload: &[u8], feature: u8, report: &mut GrfInspectReport) {
    match feature {
        ACTION0_FEATURE_BADGES => {
            if let Some(meta) = parse_action0_badge_meta(payload) {
                if !report
                    .badge_labels
                    .iter()
                    .any(|l| l.eq_ignore_ascii_case(&meta.label))
                {
                    report.badge_labels.push(meta.label);
                }
            }
        }
        ACTION0_FEATURE_ROADSTOPS => {
            if let Some(meta) = parse_action0_roadstop_meta(payload) {
                if let Some(err) = meta.badge_list_error {
                    report.warnings.push(format!("roadstop: {err}"));
                }
                if !meta.badge_labels.is_empty() {
                    report.badge_associations.push(format!(
                        "{}→[{}]",
                        meta.label,
                        meta.badge_labels.join(",")
                    ));
                }
            }
        }
        ACTION0_FEATURE_OBJECTS => {
            if let Some(meta) = parse_action0_object_meta(payload) {
                if let Some(err) = meta.badge_list_error {
                    report.warnings.push(format!("object: {err}"));
                }
                if !meta.badge_labels.is_empty() {
                    report.badge_associations.push(format!(
                        "{}→[{}]",
                        meta.name,
                        meta.badge_labels.join(",")
                    ));
                }
            }
        }
        _ => {}
    }
}
