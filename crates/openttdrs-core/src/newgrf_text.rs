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
        let mut fallback = None;
        for entry in matching {
            if entry.language == language {
                return Some(entry.text.as_str());
            }
            if fallback.is_none() && entry.language == NEWGRF_LANGUAGE_ENGLISH {
                fallback = Some(entry.text.as_str());
            }
        }
        fallback.or_else(|| {
            self.entries
                .iter()
                .rev()
                .find(|entry| entry.grfid == grfid && entry.string_id == string_id)
                .map(|entry| entry.text.as_str())
        })
    }
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

fn action4_languages(raw_language: u8) -> Vec<u8> {
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
}
