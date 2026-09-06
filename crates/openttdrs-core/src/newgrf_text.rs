//! Textos genéricos de `NewGRF` (Action4).
//!
//! El parser cubre la parte que necesitan los callbacks que devuelven texto:
//! IDs genéricos (`0xD000` en adelante), variantes por idioma y cadenas
//! terminadas en NUL. Los códigos de control de strings quedan conservados en
//! la cadena y se expanden en una etapa posterior del text stack.

use serde::{Deserialize, Serialize};

/// Inicio del rango de strings genéricos de GRF (`GRFSTR_MISC_GRF_TEXT`).
pub const GRF_STRING_GENERIC_BASE: u32 = 0xD000;
/// Código de idioma extendido para inglés en el esquema Action4 nuevo.
pub const NEWGRF_LANGUAGE_ENGLISH: u8 = 1;
/// Código de idioma extendido para español en el esquema Action4 nuevo.
pub const NEWGRF_LANGUAGE_SPANISH: u8 = 4;
/// Variante genérica/fallback usada por Action13 de GRF v7 o anteriores.
pub const NEWGRF_LANGUAGE_UNSPECIFIED: u8 = 0x7F;

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
        let text = String::from_utf8_lossy(&payload[cursor..end]).into_owned();
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
        let text = String::from_utf8_lossy(&payload[cursor..end]);
        cursor = end + 1;
        if text.is_empty() {
            continue;
        }
        out.push(NewGrfString {
            grfid: target_grfid,
            string_id,
            language,
            text: text.into_owned(),
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
