//! Textos genéricos de `NewGRF` (Action4).
//!
//! El parser cubre la parte que necesitan los callbacks que devuelven texto:
//! IDs genéricos (`0xD000` en adelante), variantes por idioma, cadenas
//! terminadas en NUL y los controles NFO básicos que `OpenTTD` traduce al cargar
//! un Action4/Action13. Choice-lists, pluralización y parámetros del text stack
//! que requieren estado de juego quedan representados con marcadores visibles.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Inicio del rango de strings genéricos de GRF (`GRFSTR_MISC_GRF_TEXT`).
pub const GRF_STRING_GENERIC_BASE: u32 = 0xD000;
/// Código de idioma extendido para inglés en el esquema Action4 nuevo.
pub const NEWGRF_LANGUAGE_ENGLISH: u8 = 1;
/// Código de idioma extendido para español en el esquema Action4 nuevo.
pub const NEWGRF_LANGUAGE_SPANISH: u8 = 4;
/// Variante genérica/fallback usada por Action13 de GRF v7 o anteriores.
pub const NEWGRF_LANGUAGE_UNSPECIFIED: u8 = 0x7F;

const MAX_INLINE_TEXT_EXPANSION_DEPTH: usize = 8;
const INLINE_TEXT_MARKER_PREFIX: &str = "⟦grf-string:0x";

/// Una cadena genérica definida por un Action4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewGrfString {
    pub grfid: u32,
    pub string_id: u32,
    /// ID normalizado: 0..=63 en el esquema extendido o bit del esquema viejo.
    pub language: u8,
    pub text: String,
}

/// Catálogo efímero de strings genéricos del stack activo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewGrfStringCatalog {
    entries: Vec<NewGrfString>,
}

/// Traduce los controles NFO que no necesitan un text stack ni un scope de
/// juego.
///
/// `OpenTTD` convierte estos bytes antes de guardar un `GRFTextList`. Los
/// parámetros dinámicos se mantienen como `⟦...⟧`: de ese modo la UI no recibe
/// bytes de control invisibles y una etapa posterior puede reemplazar el
/// marcador sin volver a parsear el pseudo-sprite. La función nunca indexa
/// fuera del payload; un control truncado se representa explícitamente.
#[must_use]
pub fn decode_newgrf_text(raw: &[u8]) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let byte = raw[cursor];
        cursor += 1;
        match byte {
            0 => break,
            0x01 => {
                if cursor < raw.len() {
                    cursor += 1;
                }
                out.push(' ');
            }
            0x0A | 0x0E | 0x0F | 0x88..=0x98 => {}
            0x0D => out.push('\n'),
            0x1F => {
                if cursor + 1 < raw.len() {
                    cursor += 2;
                    out.push(' ');
                } else {
                    push_marker(&mut out, "truncated:1F");
                    break;
                }
            }
            0x20..=0x7A => out.push(char::from(byte)),
            0x7B..=0x7F => {
                let label = match byte {
                    0x7B => "param-dword-signed",
                    0x7C => "param-dword",
                    0x7D => "param-word-signed",
                    0x7E => "param-word",
                    _ => "param-byte",
                };
                push_marker(&mut out, label);
            }
            0x80 => push_marker(&mut out, "param-string"),
            0x81 => {
                let Some(string_id) = read_u16_le(raw, &mut cursor) else {
                    push_marker(&mut out, "truncated:81");
                    break;
                };
                let _ = write!(out, "⟦grf-string:0x{string_id:04X}⟧");
            }
            0x82 => push_marker(&mut out, "date-long"),
            0x83 => push_marker(&mut out, "date-short"),
            0x84 => push_marker(&mut out, "date-iso"),
            0x85 => push_marker(&mut out, "discard-word"),
            0x86 => push_marker(&mut out, "rotate-words"),
            0x87 => push_marker(&mut out, "volume"),
            0x9A => decode_extended_control(raw, &mut cursor, &mut out),
            0x9B => push_marker(&mut out, "town"),
            0x9C => push_marker(&mut out, "city"),
            0x9E => out.push('€'),
            0x9F => out.push('Ÿ'),
            0xA0 | 0xBC => out.push('↑'),
            0xAA | 0xBD => out.push('↓'),
            0xAC => out.push('✓'),
            0xAD => out.push('✕'),
            0xAF => out.push('→'),
            0xB4 => out.push('🚂'),
            0xB5 => out.push('🚚'),
            0xB6 => out.push('🚌'),
            0xB7 => out.push('✈'),
            0xB8 => out.push('🚢'),
            0xB9 => out.push('⁻'),
            byte if byte >= 0xC2 => {
                if !push_utf8_char(raw, byte, &mut cursor, &mut out) {
                    let _ = write!(out, "⟦byte:{byte:02X}⟧");
                }
            }
            byte => {
                let _ = write!(out, "⟦byte:{byte:02X}⟧");
            }
        }
    }
    out
}

fn push_marker(out: &mut String, marker: &str) {
    out.push('⟦');
    out.push_str(marker);
    out.push('⟧');
}

fn read_u16_le(raw: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes = raw.get(*cursor..cursor.saturating_add(2))?;
    let value = u16::from_le_bytes([bytes[0], bytes[1]]);
    *cursor += 2;
    Some(value)
}

fn decode_extended_control(raw: &[u8], cursor: &mut usize, out: &mut String) {
    let Some(&code) = raw.get(*cursor) else {
        push_marker(out, "truncated:9A");
        return;
    };
    *cursor += 1;
    match code {
        0x00 => *cursor = raw.len(),
        0x01 => push_marker(out, "currency"),
        0x03 => {
            let Some(value) = read_u16_le(raw, cursor) else {
                push_marker(out, "truncated:9A03");
                return;
            };
            let _ = write!(out, "⟦push-word:0x{value:04X}⟧");
        }
        0x06 => push_marker(out, "param-byte-hex"),
        0x07 => push_marker(out, "param-word-hex"),
        0x08 => push_marker(out, "param-dword-hex"),
        0x0B => push_marker(out, "param-qword-hex"),
        0x0C => push_marker(out, "station-name"),
        0x0D => push_marker(out, "weight"),
        0x0E | 0x0F => {
            let Some(&index) = raw.get(*cursor) else {
                push_marker(out, "truncated:9A0E");
                return;
            };
            *cursor += 1;
            let label = if code == 0x0E { "gender" } else { "case" };
            let _ = write!(out, "⟦{label}:{index}⟧");
        }
        0x10 => {
            let Some(&index) = raw.get(*cursor) else {
                push_marker(out, "truncated:9A10");
                return;
            };
            *cursor += 1;
            let _ = write!(out, "⟦choice-next:{index}⟧");
        }
        0x11 => push_marker(out, "choice-default"),
        0x12 => push_marker(out, "choice-end"),
        0x13..=0x15 => {
            let label = match code {
                0x13 => "gender-list",
                0x14 => "case-list",
                _ => "plural-list",
            };
            if code != 0x14 {
                if raw.get(*cursor).is_none() {
                    push_marker(out, "truncated:9A13");
                    return;
                }
                *cursor += 1;
            }
            push_marker(out, label);
        }
        0x16..=0x1E => push_marker(out, "date-dword"),
        0x1F | 0x20 => {}
        0x21 => push_marker(out, "param-dword-force"),
        _ => {
            let _ = write!(out, "⟦ext-9A:{code:02X}⟧");
        }
    }
}

fn push_utf8_char(raw: &[u8], first: u8, cursor: &mut usize, out: &mut String) -> bool {
    let width = match first {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return false,
    };
    let start = cursor.saturating_sub(1);
    let Some(end) = start.checked_add(width) else {
        return false;
    };
    let Some(bytes) = raw.get(start..end) else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let Some(character) = text.chars().next() else {
        return false;
    };
    out.push(character);
    *cursor = end;
    true
}

impl NewGrfStringCatalog {
    /// Borra todas las cadenas del stack anterior.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Añade una cadena respetando el orden del stack (la última definición
    /// gana al resolver una misma pareja GRFID/ID/idioma).
    pub fn push(&mut self, value: NewGrfString) {
        self.entries.push(value);
    }

    /// Añade todas las cadenas de un GRF.
    pub fn extend<I>(&mut self, values: I)
    where
        I: IntoIterator<Item = NewGrfString>,
    {
        self.entries.extend(values);
    }

    /// Lista de entradas, útil para inspección y pruebas de paridad.
    #[must_use]
    pub fn entries(&self) -> &[NewGrfString] {
        &self.entries
    }

    /// Resuelve un texto con fallback de idioma determinista.
    ///
    /// La prioridad reproduce la intención de `AddGRFString`: idioma pedido,
    /// inglés y por último la variante más recientemente declarada.
    #[must_use]
    pub fn lookup(&self, grfid: u32, string_id: u32, language: u8) -> Option<&str> {
        let matching = self
            .entries
            .iter()
            .rev()
            .filter(|entry| entry.grfid == grfid && entry.string_id == string_id);
        let mut unspecified = None;
        let mut english = None;
        for entry in matching {
            if entry.language == language {
                return Some(entry.text.as_str());
            }
            if unspecified.is_none() && entry.language == NEWGRF_LANGUAGE_UNSPECIFIED {
                unspecified = Some(entry.text.as_str());
            }
            if english.is_none() && entry.language == NEWGRF_LANGUAGE_ENGLISH {
                english = Some(entry.text.as_str());
            }
        }
        unspecified.or(english).or_else(|| {
            self.entries
                .iter()
                .rev()
                .find(|entry| entry.grfid == grfid && entry.string_id == string_id)
                .map(|entry| entry.text.as_str())
        })
    }

    /// Resuelve una cadena y expande referencias inline `0x81` ya traducidas
    /// por [`decode_newgrf_text`].
    ///
    /// El byte `0x81` contiene un ID local de GRF; el decoder lo conserva como
    /// `⟦grf-string:0xNNNN⟧` para no mezclar el parseo de pseudo-sprites con el
    /// catálogo. Al resolverlo, los IDs menores a `GRF_STRING_GENERIC_BASE` se
    /// mapean al rango genérico del GRF y los IDs ya genéricos se conservan.
    /// Cada referencia se busca con el mismo fallback de idioma que [`lookup`].
    /// Las cadenas faltantes permanecen como marcadores visibles y los ciclos
    /// se cortan sin panic.
    #[must_use]
    pub fn lookup_expanded(&self, grfid: u32, string_id: u32, language: u8) -> Option<String> {
        let text = self.lookup(grfid, string_id, language)?.to_owned();
        let mut stack = vec![string_id];
        Some(self.expand_inline_references(&text, grfid, language, &mut stack, 0))
    }

    fn expand_inline_references(
        &self,
        text: &str,
        grfid: u32,
        language: u8,
        stack: &mut Vec<u32>,
        depth: usize,
    ) -> String {
        if depth >= MAX_INLINE_TEXT_EXPANSION_DEPTH {
            return text.to_owned();
        }

        let mut output = String::with_capacity(text.len());
        let mut cursor = 0usize;
        while let Some(relative_start) = text[cursor..].find(INLINE_TEXT_MARKER_PREFIX) {
            let start = cursor + relative_start;
            output.push_str(&text[cursor..start]);
            let value_start = start + INLINE_TEXT_MARKER_PREFIX.len();
            let Some(relative_end) = text[value_start..].find('⟧') else {
                output.push_str(&text[start..]);
                return output;
            };
            let value_end = value_start + relative_end;
            let marker_end = value_end + '⟧'.len_utf8();
            let marker = &text[start..marker_end];
            let Some(raw_id) = u32::from_str_radix(&text[value_start..value_end], 16).ok() else {
                output.push_str(marker);
                cursor = marker_end;
                continue;
            };
            let target_id = if raw_id < GRF_STRING_GENERIC_BASE {
                let Some(target_id) = GRF_STRING_GENERIC_BASE.checked_add(raw_id) else {
                    output.push_str(marker);
                    cursor = marker_end;
                    continue;
                };
                target_id
            } else {
                raw_id
            };

            if stack.contains(&target_id) {
                output.push_str(marker);
                cursor = marker_end;
                continue;
            }

            let Some(replacement) = self.lookup(grfid, target_id, language).map(str::to_owned)
            else {
                output.push_str(marker);
                cursor = marker_end;
                continue;
            };
            stack.push(target_id);
            let expanded =
                self.expand_inline_references(&replacement, grfid, language, stack, depth + 1);
            stack.pop();
            output.push_str(&expanded);
            cursor = marker_end;
        }
        output.push_str(&text[cursor..]);
        output
    }
}

/// Recorre pseudo-sprites y recoge Action13 (`TranslateGRFStrings`).
///
/// `active_grfids` representa los GRF que `OpenTTD` ya aceptó en el stack; una
/// traducción a un GRFID desconocido se ignora, igual que upstream.
#[must_use]
pub fn collect_action13_translations_from_grf(
    data: &[u8],
    source_grf_version: u8,
    active_grfids: &[u32],
) -> Vec<NewGrfString> {
    let mut out = Vec::new();
    let _ = crate::newgrf_actions::for_each_pseudo_payload(data, |payload| {
        parse_action13_payload(payload, source_grf_version, active_grfids, &mut out);
    });
    out
}

/// Recorre pseudo-sprites y recoge únicamente Action4 genéricos.
#[must_use]
pub fn collect_action4_generic_strings_from_grf(data: &[u8], grfid: u32) -> Vec<NewGrfString> {
    let mut out = Vec::new();
    let _ = crate::newgrf_actions::for_each_pseudo_payload(data, |payload| {
        parse_action4_generic_payload(payload, grfid, &mut out);
    });
    out
}

/// Interpreta un payload Action4 sin asumir que la entrada está completa.
fn parse_action4_generic_payload(payload: &[u8], grfid: u32, out: &mut Vec<NewGrfString>) {
    // 04, feature, language, count, WORD offset.
    if payload.len() < 6 || payload[0] != 0x04 || payload[2] & 0x80 == 0 {
        return;
    }
    let raw_language = payload[2] & 0x7F;
    let languages = action4_languages(raw_language);
    let count = usize::from(payload[3]);
    let offset = u32::from(u16::from_le_bytes([payload[4], payload[5]]));
    let mut cursor = 6usize;
    let mut texts = Vec::with_capacity(count);
    for index in 0..count {
        let Some(end) = payload[cursor..].iter().position(|&byte| byte == 0) else {
            // No agregamos una cadena parcial si el pseudo-sprite está truncado.
            break;
        };
        let end = cursor + end;
        let Some(index) = u32::try_from(index).ok() else {
            break;
        };
        let Some(string_id) = offset.checked_add(index) else {
            break;
        };
        let text = decode_newgrf_text(&payload[cursor..end]);
        texts.push((string_id, text));
        cursor = end + 1;
    }
    // Las entradas completas previas siguen siendo válidas; sólo se evita
    // inventar el registro cuyo terminador no apareció.
    for language in languages {
        for (string_id, text) in &texts {
            out.push(NewGrfString {
                grfid,
                string_id: *string_id,
                language,
                text: text.clone(),
            });
        }
    }
}

fn parse_action13_payload(
    payload: &[u8],
    source_grf_version: u8,
    active_grfids: &[u32],
    out: &mut Vec<NewGrfString>,
) {
    // 13, GRFID (4), [language v8+], count, WORD first-id, strings...
    if payload.len() < 8 || payload[0] != 0x13 {
        return;
    }
    let target_grfid =
        crate::newgrf_config::grfid_from_bytes([payload[1], payload[2], payload[3], payload[4]]);
    if !active_grfids.contains(&target_grfid) {
        return;
    }
    let (language, count_index, first_id_index) = if source_grf_version >= 8 {
        if payload.len() < 10 {
            return;
        }
        (payload[5], 6usize, 7usize)
    } else {
        (NEWGRF_LANGUAGE_UNSPECIFIED, 5usize, 6usize)
    };
    let count = usize::from(payload[count_index]);
    let first_id = u32::from(u16::from_le_bytes([
        payload[first_id_index],
        payload[first_id_index + 1],
    ]));
    let end_id = first_id.saturating_add(u32::from(payload[count_index]));
    let in_generic_range = (0xD000..0xD400).contains(&first_id) && end_id <= 0xD400
        || (0xD800..0x10000).contains(&first_id) && end_id <= 0x10000;
    if !in_generic_range {
        return;
    }
    let mut cursor = first_id_index + 2;
    for index in 0..count {
        let Some(end) = payload.get(cursor..).and_then(|rest| {
            rest.iter()
                .position(|&byte| byte == 0)
                .map(|offset| cursor + offset)
        }) else {
            break;
        };
        let Some(index) = u32::try_from(index).ok() else {
            break;
        };
        let Some(string_id) = first_id.checked_add(index) else {
            break;
        };
        let text = decode_newgrf_text(&payload[cursor..end]);
        cursor = end + 1;
        if text.is_empty() {
            continue;
        }
        out.push(NewGrfString {
            grfid: target_grfid,
            string_id,
            language,
            text,
        });
    }
}

fn action4_languages(raw_language: u8) -> Vec<u8> {
    if raw_language == NEWGRF_LANGUAGE_UNSPECIFIED {
        return vec![NEWGRF_LANGUAGE_UNSPECIFIED];
    }
    if raw_language & 0x40 != 0 {
        return vec![raw_language & 0x3F];
    }
    let mut languages = Vec::new();
    for bit in 0..6 {
        if raw_language & (1 << bit) != 0 {
            languages.push(bit);
        }
    }
    if languages.is_empty() {
        languages.push(0);
    }
    languages
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn v2_with_payloads(payloads: &[&[u8]]) -> Vec<u8> {
        const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
        let mut section = Vec::new();
        for payload in payloads {
            section.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
            section.push(0xFF);
            section.extend_from_slice(payload);
        }
        section.extend_from_slice(&0u32.to_le_bytes());
        let sprite_offset = u32::try_from(section.len() + 1).unwrap();
        let mut data = vec![0, 0];
        data.extend_from_slice(&SIG);
        data.extend_from_slice(&sprite_offset.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&section);
        data.extend_from_slice(&0u32.to_le_bytes());
        data
    }

    #[test]
    fn decodes_basic_nfo_controls_and_inline_references() {
        let raw = [
            b'A', 0x01, b'x', b'B', 0x0D, 0x0E, b'C', 0x81, 0x34, 0x12, 0x9A, 0x03, 0x78, 0x56,
            0x9E, 0xB4, 0x88, b'D',
        ];
        assert_eq!(
            decode_newgrf_text(&raw),
            "A B\nC⟦grf-string:0x1234⟧⟦push-word:0x5678⟧€🚂D"
        );
    }

    #[test]
    fn decodes_dynamic_controls_to_visible_markers_and_survives_truncation() {
        let raw = [0x7B, 0x9A, 0x10, 3, 0x80, 0x9A, 0x06, 0x81, 0x34];
        assert_eq!(
            decode_newgrf_text(&raw),
            "⟦param-dword-signed⟧⟦choice-next:3⟧⟦param-string⟧⟦param-byte-hex⟧⟦truncated:81⟧"
        );
    }

    #[test]
    fn keeps_regular_utf8_text_unchanged() {
        assert_eq!(decode_newgrf_text("Español ✓".as_bytes()), "Español ✓");
    }

    #[test]
    fn action4_stores_decoded_control_text() {
        let payload = [0x04, 0x48, 0xC1, 1, 0x00, 0xD0, b'A', 0x01, b'x', 0x9E, 0];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action4_generic_strings_from_grf(&data, 7);
        assert_eq!(strings[0].text, "A €");
    }

    #[test]
    fn parses_generic_action4_with_extended_languages() {
        let payload = [
            0x04, 0x48, 0xC1, 2, 0x00, 0xD0, b'E', b'n', 0, b'T', b'w', 0,
        ];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action4_generic_strings_from_grf(&data, 0x0102_0304);
        assert_eq!(strings.len(), 2);
        assert_eq!(strings[0].string_id, 0xD000);
        assert_eq!(strings[0].language, NEWGRF_LANGUAGE_ENGLISH);
        assert_eq!(strings[1].text, "Tw");
    }

    #[test]
    fn old_language_bitmask_creates_one_variant_per_language() {
        let payload = [0x04, 0x48, 0x12 | 0x80, 1, 0x01, 0xD0, b'X', 0];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action4_generic_strings_from_grf(&data, 7);
        assert_eq!(
            strings.iter().map(|s| s.language).collect::<Vec<_>>(),
            [1, 4]
        );
    }

    #[test]
    fn truncated_string_does_not_create_partial_entry() {
        let payload = [0x04, 0x48, 0xC1, 2, 0x00, 0xD0, b'X', 0];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action4_generic_strings_from_grf(&data, 7);
        assert_eq!(strings.len(), 1);
    }

    #[test]
    fn lookup_prefers_requested_then_english_then_latest() {
        let mut catalog = NewGrfStringCatalog::default();
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: 4,
            text: "es".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: 1,
            text: "en".into(),
        });
        assert_eq!(catalog.lookup(1, 0xD000, 4), Some("es"));
        assert_eq!(catalog.lookup(1, 0xD000, 2), Some("en"));
        assert_eq!(catalog.lookup(2, 0xD000, 2), None);
    }

    #[test]
    fn lookup_expanded_resolves_nested_local_and_generic_references() {
        let mut catalog = NewGrfStringCatalog::default();
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "Base ⟦grf-string:0x0001⟧".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD001,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "child ⟦grf-string:0xD002⟧".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD002,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "leaf".into(),
        });

        assert_eq!(
            catalog.lookup_expanded(1, 0xD000, NEWGRF_LANGUAGE_SPANISH),
            Some("Base child leaf".into())
        );
    }

    #[test]
    fn lookup_expanded_preserves_missing_and_cyclic_references() {
        let mut catalog = NewGrfStringCatalog::default();
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "root ⟦grf-string:0x0001⟧ ⟦grf-string:0x0003⟧".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD001,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "child ⟦grf-string:0x0000⟧".into(),
        });

        assert_eq!(
            catalog.lookup_expanded(1, 0xD000, NEWGRF_LANGUAGE_ENGLISH),
            Some("root child ⟦grf-string:0x0000⟧ ⟦grf-string:0x0003⟧".into())
        );
    }

    #[test]
    fn lookup_expanded_uses_locale_fallback_for_nested_text() {
        let mut catalog = NewGrfStringCatalog::default();
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: NEWGRF_LANGUAGE_SPANISH,
            text: "Principal ⟦grf-string:0x0001⟧".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD001,
            language: NEWGRF_LANGUAGE_UNSPECIFIED,
            text: "genérico".into(),
        });

        assert_eq!(
            catalog.lookup_expanded(1, 0xD000, NEWGRF_LANGUAGE_SPANISH),
            Some("Principal genérico".into())
        );
    }

    #[test]
    fn action13_v8_requires_active_target_and_overrides_base_language() {
        let payload = [
            0x13,
            0x01,
            0x02,
            0x03,
            0x04,
            NEWGRF_LANGUAGE_SPANISH,
            1,
            0x00,
            0xD0,
            b'E',
            b's',
            0,
        ];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action13_translations_from_grf(
            &data,
            8,
            &[crate::newgrf_config::grfid_from_bytes([1, 2, 3, 4])],
        );
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].grfid, 0x0102_0304);
        assert_eq!(strings[0].language, NEWGRF_LANGUAGE_SPANISH);
        assert_eq!(strings[0].text, "Es");
        assert!(collect_action13_translations_from_grf(&data, 8, &[]).is_empty());
    }

    #[test]
    fn action13_v7_uses_unspecified_language_and_rejects_out_of_range_ids() {
        let payload = [0x13, 9, 8, 7, 6, 1, 0x00, 0xD0, b'v', b'7', 0];
        let data = v2_with_payloads(&[&payload]);
        let strings = collect_action13_translations_from_grf(&data, 7, &[0x0908_0706]);
        assert_eq!(strings[0].language, NEWGRF_LANGUAGE_UNSPECIFIED);

        let invalid = [0x13, 9, 8, 7, 6, 1, 0x00, 0xC0, b'x', 0];
        let invalid_data = v2_with_payloads(&[&invalid]);
        assert!(
            collect_action13_translations_from_grf(&invalid_data, 7, &[0x0908_0706]).is_empty()
        );
    }

    #[test]
    fn unspecified_translation_beats_english_fallback() {
        let mut catalog = NewGrfStringCatalog::default();
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: NEWGRF_LANGUAGE_ENGLISH,
            text: "base".into(),
        });
        catalog.push(NewGrfString {
            grfid: 1,
            string_id: 0xD000,
            language: NEWGRF_LANGUAGE_UNSPECIFIED,
            text: "translation".into(),
        });
        assert_eq!(
            catalog.lookup(1, 0xD000, NEWGRF_LANGUAGE_SPANISH),
            Some("translation")
        );
    }
}
