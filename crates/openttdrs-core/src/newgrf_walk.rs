//! Caminata unificada de entradas GRF v1/v2 (framing solamente).
//!
//! Provee iteradores y callbacks para recorrer la sección de datos de un GRF,
//! distinguiendo entre pseudo-sprites (0xFF) y sprites reales, sin interpretar acciones.

use crate::newgrf_config::GrfContainerVersion;

/// Una entrada del GRF parseada (framing solamente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrfEntry<'a> {
    /// Pseudo-sprite (info byte == 0xFF) con su payload.
    Pseudo(&'a [u8]),
    /// Sprite real con info byte y payload.
    Real { info: u8, payload: &'a [u8] },
}

/// Resultado de parseo de cabecera de entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntryHeader {
    /// Tamaño del payload (sin incluir cabecera).
    size: usize,
    /// Tamaño de la cabecera (3 para v1, 5 para v2).
    header_len: usize,
}

/// Lee la cabecera de una entrada GRF (v1 o v2).
///
/// Retorna `Some((size, header_len))` si hay suficientes bytes y `size > 0`.
fn read_entry_header(
    section: &[u8],
    i: usize,
    container: GrfContainerVersion,
) -> Option<EntryHeader> {
    match container {
        GrfContainerVersion::V2 => {
            if i + 5 > section.len() {
                return None;
            }
            let size =
                u32::from_le_bytes([section[i], section[i + 1], section[i + 2], section[i + 3]])
                    as usize;
            if size == 0 {
                return None;
            }
            Some(EntryHeader {
                size,
                header_len: 5,
            })
        }
        GrfContainerVersion::V1 => {
            if i + 3 > section.len() {
                return None;
            }
            let size = u16::from_le_bytes([section[i], section[i + 1]]) as usize;
            if size == 0 {
                return None;
            }
            Some(EntryHeader {
                size,
                header_len: 3,
            })
        }
    }
}

/// Calcula el payload de una entrada pseudo-sprite (info == 0xFF).
fn pseudo_payload<'a>(section: &'a [u8], i: usize, header: &EntryHeader) -> Option<&'a [u8]> {
    let payload_start = i + header.header_len;
    let end = payload_start + header.size;
    if end > section.len() {
        return None;
    }
    Some(&section[payload_start..end])
}

/// Calcula el payload de un sprite real (info != 0xFF).
///
/// En v1, el payload comienza en `i + 2` (saltando size, conserva info).
/// En v2, el payload comienza en `i + header_len`.
fn real_sprite_payload<'a>(
    section: &'a [u8],
    i: usize,
    header: &EntryHeader,
    container: GrfContainerVersion,
) -> Option<&'a [u8]> {
    match container {
        GrfContainerVersion::V1 => {
            let start = i + 2;
            let end = start + header.size;
            if end > section.len() {
                return None;
            }
            Some(&section[start..end])
        }
        GrfContainerVersion::V2 => {
            let payload_start = i + header.header_len;
            let end = payload_start + header.size;
            if end > section.len() {
                return None;
            }
            Some(&section[payload_start..end])
        }
    }
}

/// Calcula la posición de la siguiente entrada.
fn next_entry_position(
    i: usize,
    header: &EntryHeader,
    info: u8,
    container: GrfContainerVersion,
) -> usize {
    if info == 0xFF {
        // Pseudo: siguiente = payload_start + size
        i + header.header_len + header.size
    } else {
        // Real sprite:
        match container {
            GrfContainerVersion::V1 => i + 2 + header.size,
            GrfContainerVersion::V2 => i + header.header_len + header.size,
        }
    }
}

/// Itera sobre las entradas de la sección de datos de un GRF.
///
/// Procesa cada entrada (pseudo o real) y llama al callback `visit` con `GrfEntry`.
/// Se detiene al encontrar una entrada de tamaño 0 o al final de la sección.
pub fn walk_grf_entries<'a>(
    data_section: &'a [u8],
    container: GrfContainerVersion,
    mut visit: impl FnMut(GrfEntry<'a>),
) {
    let mut i = 0usize;
    while i < data_section.len() {
        let Some(header) = read_entry_header(data_section, i, container) else {
            break;
        };
        let info = data_section[i + header.header_len - 1];

        if info == 0xFF {
            // Pseudo-sprite
            if let Some(payload) = pseudo_payload(data_section, i, &header) {
                visit(GrfEntry::Pseudo(payload));
                i = next_entry_position(i, &header, info, container);
            } else {
                break;
            }
        } else {
            // Sprite real
            if let Some(payload) = real_sprite_payload(data_section, i, &header, container) {
                visit(GrfEntry::Real { info, payload });
                i = next_entry_position(i, &header, info, container);
            } else {
                break;
            }
        }
    }
}

/// Itera **solo** sobre pseudo-sprites (info == 0xFF), ignorando sprites reales.
///
/// Útil para procesar acciones `NewGRF` sin decodificar gráficos.
pub fn for_each_pseudo_sprite(
    data_section: &[u8],
    container: GrfContainerVersion,
    mut visit: impl FnMut(&[u8]),
) {
    walk_grf_entries(data_section, container, |entry| {
        if let GrfEntry::Pseudo(payload) = entry {
            visit(payload);
        }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::newgrf_config::parse_grf_container;

    #[test]
    fn walk_v2_empty_section() {
        let section = &[0u8, 0, 0, 0]; // size = 0 → termina
        let mut count = 0;
        walk_grf_entries(section, GrfContainerVersion::V2, |_| count += 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn walk_v2_single_pseudo() {
        let mut section = vec![];
        section.extend_from_slice(&5u32.to_le_bytes()); // size = 5
        section.push(0xFF); // info = pseudo
        section.extend_from_slice(b"HELLO"); // payload
        section.extend_from_slice(&0u32.to_le_bytes()); // terminator

        let mut count = 0;
        let mut payload_found = Vec::new();
        walk_grf_entries(&section, GrfContainerVersion::V2, |entry| {
            count += 1;
            if let GrfEntry::Pseudo(p) = entry {
                payload_found.extend_from_slice(p);
            }
        });
        assert_eq!(count, 1);
        assert_eq!(payload_found, b"HELLO");
    }

    #[test]
    fn walk_v1_single_pseudo() {
        let mut section = vec![];
        section.extend_from_slice(&3u16.to_le_bytes()); // size = 3
        section.push(0xFF); // info = pseudo
        section.extend_from_slice(b"ABC"); // payload
        section.extend_from_slice(&0u16.to_le_bytes()); // terminator

        let mut count = 0;
        walk_grf_entries(&section, GrfContainerVersion::V1, |entry| {
            count += 1;
            if let GrfEntry::Pseudo(p) = entry {
                assert_eq!(p, b"ABC");
            }
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn walk_v2_pseudo_and_real() {
        let mut section = vec![];
        // Pseudo
        section.extend_from_slice(&3u32.to_le_bytes());
        section.push(0xFF);
        section.extend_from_slice(b"ABC");
        // Real sprite (info = 0x01)
        section.extend_from_slice(&4u32.to_le_bytes());
        section.push(0x01);
        section.extend_from_slice(b"DATA");
        // Terminator
        section.extend_from_slice(&0u32.to_le_bytes());

        let mut pseudo_count = 0;
        let mut real_count = 0;
        walk_grf_entries(&section, GrfContainerVersion::V2, |entry| match entry {
            GrfEntry::Pseudo(_) => pseudo_count += 1,
            GrfEntry::Real { .. } => real_count += 1,
        });
        assert_eq!(pseudo_count, 1);
        assert_eq!(real_count, 1);
    }

    #[test]
    fn for_each_pseudo_ignores_real() {
        let mut section = vec![];
        // Pseudo
        section.extend_from_slice(&2u32.to_le_bytes());
        section.push(0xFF);
        section.extend_from_slice(b"AB");
        // Real
        section.extend_from_slice(&3u32.to_le_bytes());
        section.push(0x02);
        section.extend_from_slice(b"XYZ");
        // Pseudo
        section.extend_from_slice(&1u32.to_le_bytes());
        section.push(0xFF);
        section.extend_from_slice(b"C");
        // Terminator
        section.extend_from_slice(&0u32.to_le_bytes());

        let mut payloads = Vec::new();
        for_each_pseudo_sprite(&section, GrfContainerVersion::V2, |p| {
            payloads.extend_from_slice(p);
        });
        assert_eq!(payloads, b"ABC");
    }

    #[test]
    fn walk_truncated_section_stops_gracefully() {
        let section = &[0x05, 0x00, 0x00]; // size = 5 pero falta info
        let mut count = 0;
        walk_grf_entries(section, GrfContainerVersion::V2, |_| count += 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn walk_from_parsed_grf() {
        use crate::newgrf_config::build_minimal_grf_v2;
        let bytes = build_minimal_grf_v2([b'T', b'E', 0x01, 0x00], "Test", "Description");
        let (container, section) =
            parse_grf_container(&bytes).expect("minimal GRF v2 should parse");
        let mut pseudo_count = 0;
        for_each_pseudo_sprite(section, container, |_| pseudo_count += 1);
        assert_eq!(pseudo_count, 1); // Action8
    }
}
