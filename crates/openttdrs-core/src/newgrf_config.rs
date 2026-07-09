//! Configuración `NewGRF` (Fase 7 MVP).
//!
//! Persistencia del stack activo + lectura de cabecera de contenedor `.grf` y
//! Action 8 (GRFID / nombre). No ejecuta Action0–14 ni resuelve sprites.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Firma de contenedor GRF v2 (`grfcodec` / `OpenTTD` `_grf_cont_v2_sig`).
const GRF_CONT_V2_SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];

/// Entrada del stack `NewGRF` (`GRFConfig` simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewGrfEntry {
    /// Nombre de archivo (relativo a `newgrf/` o ruta conocida).
    pub filename: String,
    /// Identificador de 4 bytes (primer byte del archivo = MSB).
    pub grfid: u32,
    /// Nombre corto (Action 8); vacío si aún no se escaneó.
    #[serde(default)]
    pub name: String,
    /// Descripción (Action 8).
    #[serde(default)]
    pub description: String,
    /// Versión de formato GRF del Action 8 (p. ej. 7 u 8), no la del set.
    #[serde(default)]
    pub grf_version: u8,
    /// ¿Activo en la partida?
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// GRF de base / no desactivable (p. ej. `OpenGFX` documentado).
    #[serde(default)]
    pub is_static: bool,
}

const fn default_true() -> bool {
    true
}

impl NewGrfEntry {
    #[must_use]
    pub fn new(filename: impl Into<String>, grfid: u32) -> Self {
        Self {
            filename: filename.into(),
            grfid,
            name: String::new(),
            description: String::new(),
            grf_version: 0,
            enabled: true,
            is_static: false,
        }
    }

    /// GRFID como ocho hex mayúsculas (`FF4F5401`).
    #[must_use]
    pub fn grfid_hex(&self) -> String {
        format_grfid(self.grfid)
    }
}

/// Versión de contenedor del archivo `.grf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrfContainerVersion {
    /// Formato clásico TTD (sin cabecera).
    V1 = 1,
    /// Contenedor con firma `GRF\x82`.
    V2 = 2,
}

/// Metadatos leídos de un `.grf` sin ejecutar acciones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrfFileInfo {
    pub container: GrfContainerVersion,
    pub grfid: Option<u32>,
    pub grf_version: Option<u8>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub file_size: u64,
}

/// Error al inspeccionar un `.grf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrfScanError {
    Io(String),
    TooShort,
    InvalidContainer,
    NoAction8,
}

impl std::fmt::Display for GrfScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "E/S: {e}"),
            Self::TooShort => write!(f, "archivo demasiado corto"),
            Self::InvalidContainer => write!(f, "contenedor GRF inválido"),
            Self::NoAction8 => write!(f, "no se encontró Action 8 (GRFID)"),
        }
    }
}

impl std::error::Error for GrfScanError {}

/// Formatea un GRFID a hex de 8 dígitos.
#[must_use]
pub fn format_grfid(grfid: u32) -> String {
    format!("{grfid:08X}")
}

/// Interpreta 4 bytes de Action 8 como GRFID (orden de archivo = big-endian lógico).
#[must_use]
pub const fn grfid_from_bytes(b: [u8; 4]) -> u32 {
    u32::from_be_bytes(b)
}

/// Stack por defecto: `OpenGFX` documentado (sprites ya pre-bakeados en el cliente).
#[must_use]
pub fn default_vanilla_stack() -> Vec<NewGrfEntry> {
    vec![NewGrfEntry {
        filename: "ogfx1_base.grf".into(),
        grfid: grfid_from_bytes([0xFF, b'O', b'T', 0x01]),
        name: "OpenGFX".into(),
        description: "Gráficos base (sprites pre-bakeados; runtime Action0–14 pendiente).".into(),
        grf_version: 8,
        enabled: true,
        is_static: true,
    }]
}

/// Problemas de validación del stack (sin I/O de disco salvo rutas dadas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrfStackIssue {
    DuplicateGrfid(u32),
    EmptyFilename,
    MissingFile(String),
}

/// Valida duplicados / nombres; opcionalmente comprueba existencia en `search_dirs`.
#[must_use]
pub fn validate_stack(stack: &[NewGrfEntry], search_dirs: &[&Path]) -> Vec<GrfStackIssue> {
    let mut issues = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for e in stack {
        if e.filename.trim().is_empty() {
            issues.push(GrfStackIssue::EmptyFilename);
        }
        if !seen.insert(e.grfid) {
            issues.push(GrfStackIssue::DuplicateGrfid(e.grfid));
        }
        if !search_dirs.is_empty() {
            let found = search_dirs
                .iter()
                .any(|dir| dir.join(&e.filename).is_file());
            if !found {
                issues.push(GrfStackIssue::MissingFile(e.filename.clone()));
            }
        }
    }
    issues
}

/// Detecta versión de contenedor y, si hay Action 8, rellena metadatos.
///
/// # Errors
///
/// E/S, archivo corrupto o sin Action 8.
pub fn scan_grf_file(path: &Path) -> Result<GrfFileInfo, GrfScanError> {
    let data = std::fs::read(path).map_err(|e| GrfScanError::Io(e.to_string()))?;
    scan_grf_bytes(&data)
}

/// Igual que [`scan_grf_file`] desde memoria (tests / fixtures).
///
/// # Errors
///
/// Contenedor inválido o sin Action 8.
pub fn scan_grf_bytes(data: &[u8]) -> Result<GrfFileInfo, GrfScanError> {
    if data.len() < 2 {
        return Err(GrfScanError::TooShort);
    }
    let (container, data_section) = split_data_section(data)?;
    let action8 = find_action8(data_section, container).ok_or(GrfScanError::NoAction8)?;
    Ok(GrfFileInfo {
        container,
        grfid: Some(action8.grfid),
        grf_version: Some(action8.grf_version),
        name: Some(action8.name),
        description: Some(action8.description),
        file_size: data.len() as u64,
    })
}

/// Solo cabecera de contenedor (sin exigir Action 8).
///
/// # Errors
///
/// Archivo demasiado corto o firma inválida.
pub fn parse_grf_container(data: &[u8]) -> Result<(GrfContainerVersion, &[u8]), GrfScanError> {
    split_data_section(data)
}

struct Action8Info {
    grf_version: u8,
    grfid: u32,
    name: String,
    description: String,
}

fn split_data_section(data: &[u8]) -> Result<(GrfContainerVersion, &[u8]), GrfScanError> {
    if data.len() < 2 {
        return Err(GrfScanError::TooShort);
    }
    // Contenedor v2: 00 00 + firma + DWORD sprite_offs + BYTE compr + data…
    if data[0] == 0 && data[1] == 0 {
        if data.len() < 15 {
            return Err(GrfScanError::TooShort);
        }
        if data[2..10] != GRF_CONT_V2_SIG {
            return Err(GrfScanError::InvalidContainer);
        }
        let sprite_offs = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
        // Bytes tras el DWORD: compr (1) + data section (= sprite_offs).
        if sprite_offs == 0 || data.len() < 14 + sprite_offs {
            return Err(GrfScanError::InvalidContainer);
        }
        // data[14] = compression; data section = data[15 .. 14+sprite_offs]
        let data_end = 14 + sprite_offs;
        let section = &data[15..data_end];
        return Ok((GrfContainerVersion::V2, section));
    }
    // Contenedor v1: todo el archivo es la secuencia de sprites (+ checksum final).
    Ok((GrfContainerVersion::V1, data))
}

fn find_action8(data_section: &[u8], container: GrfContainerVersion) -> Option<Action8Info> {
    let mut i = 0usize;
    while i < data_section.len() {
        let (size, header) = match container {
            GrfContainerVersion::V2 => {
                if i + 5 > data_section.len() {
                    break;
                }
                let size = u32::from_le_bytes(data_section[i..i + 4].try_into().ok()?) as usize;
                if size == 0 {
                    break;
                }
                (size, 5usize)
            }
            GrfContainerVersion::V1 => {
                if i + 3 > data_section.len() {
                    break;
                }
                let size = u16::from_le_bytes(data_section[i..i + 2].try_into().ok()?) as usize;
                if size == 0 {
                    break;
                }
                (size, 3usize)
            }
        };
        let info = data_section[i + header - 1];
        let payload_start = i + header;
        // Pseudo (0xFF): `size` = longitud de DATA sin el info byte.
        if info == 0xFF {
            let end = payload_start + size;
            if end > data_section.len() {
                break;
            }
            let payload = &data_section[payload_start..end];
            if let Some(a8) = parse_action8_payload(payload) {
                return Some(a8);
            }
            i = end;
            continue;
        }
        // Sprite real: en v1 `size` incluye el info; en v2 no (igual que pseudo).
        let next = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
        if next > data_section.len() {
            break;
        }
        i = next;
    }
    None
}

fn parse_action8_payload(payload: &[u8]) -> Option<Action8Info> {
    if payload.first().copied()? != 0x08 || payload.len() < 6 {
        return None;
    }
    let grf_version = payload[1];
    let grfid = grfid_from_bytes([payload[2], payload[3], payload[4], payload[5]]);
    let mut rest = &payload[6..];
    let name = read_c_string(&mut rest)?;
    let description = read_c_string(&mut rest).unwrap_or_default();
    Some(Action8Info {
        grf_version,
        grfid,
        name,
        description,
    })
}

fn read_c_string(data: &mut &[u8]) -> Option<String> {
    let nul = data.iter().position(|&b| b == 0)?;
    let s = std::str::from_utf8(&data[..nul]).ok()?.to_string();
    *data = &data[nul + 1..];
    Some(s)
}

/// Construye un `.grf` contenedor v2 mínimo con un Action 8 (solo tests / fixtures).
#[must_use]
pub fn build_minimal_grf_v2(grfid: [u8; 4], name: &str, description: &str) -> Vec<u8> {
    let mut action = vec![0x08, 0x07];
    action.extend_from_slice(&grfid);
    action.extend_from_slice(name.as_bytes());
    action.push(0);
    action.extend_from_slice(description.as_bytes());
    action.push(0);

    // Pseudo-sprite v2: DWORD size (= len payload), BYTE 0xFF, payload
    let mut data_section = Vec::new();
    let size = u32::try_from(action.len()).unwrap_or(0);
    data_section.extend_from_slice(&size.to_le_bytes());
    data_section.push(0xFF);
    data_section.extend_from_slice(&action);
    // Terminador
    data_section.extend_from_slice(&0u32.to_le_bytes());

    // sprite_offs = 1 (compr) + data_section.len()
    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);

    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&GRF_CONT_V2_SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00); // no compression
    out.extend_from_slice(&data_section);
    // Sprite section vacía (terminador)
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn format_and_bytes_roundtrip() {
        let id = grfid_from_bytes([0xFF, b'O', b'T', 0x01]);
        assert_eq!(format_grfid(id), "FF4F5401");
        assert_eq!(id.to_be_bytes(), [0xFF, b'O', b'T', 0x01]);
    }

    #[test]
    fn scan_minimal_v2_reads_action8() {
        let bytes = build_minimal_grf_v2([b'T', b'W', 0x01, 0x06], "Tutorial", "Demo desc");
        let info = scan_grf_bytes(&bytes).unwrap();
        assert_eq!(info.container, GrfContainerVersion::V2);
        assert_eq!(info.grfid, Some(grfid_from_bytes([b'T', b'W', 0x01, 0x06])));
        assert_eq!(info.name.as_deref(), Some("Tutorial"));
        assert_eq!(info.description.as_deref(), Some("Demo desc"));
        assert_eq!(info.grf_version, Some(7));
    }

    #[test]
    fn validate_detects_duplicate_grfid() {
        let mut a = NewGrfEntry::new("a.grf", 1);
        a.name = "A".into();
        let b = NewGrfEntry::new("b.grf", 1);
        let issues = validate_stack(&[a, b], &[]);
        assert!(issues.contains(&GrfStackIssue::DuplicateGrfid(1)));
    }

    #[test]
    fn default_stack_has_opengfx_static() {
        let stack = default_vanilla_stack();
        assert_eq!(stack.len(), 1);
        assert!(stack[0].is_static);
        assert_eq!(stack[0].grfid_hex(), "FF4F5401");
    }

    #[test]
    fn scan_real_opengfx_if_present() {
        let path = Path::new("assets/opengfx/.signal-src-8bpp/extract/opengfx-8.0/ogfx1_base.grf");
        if !path.is_file() {
            return;
        }
        let info = scan_grf_file(path).unwrap();
        assert!(matches!(
            info.container,
            GrfContainerVersion::V1 | GrfContainerVersion::V2
        ));
        assert!(info.grfid.is_some());
    }
}
