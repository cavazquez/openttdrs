//! Filtro de texto case-insensitive (subcadena).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;

/// `true` si `haystack` contiene `query` (sin distinguir mayúsculas).
#[must_use]
pub(crate) fn text_filter_matches(query: &str, haystack: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&q.to_lowercase())
}

/// Aplica teclado a un campo `EditableText` y escribe el valor en `filter_out`.
pub(crate) fn apply_list_search_keyboard(
    key_events: &mut MessageReader<KeyboardInput>,
    editable: &mut EditableText,
    text: &mut Text,
    filter_out: &mut String,
    max_chars: usize,
    placeholder: &str,
) {
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(typed) = &ev.text else {
            continue;
        };
        for c in typed.chars() {
            if !c.is_control() && editable.value().chars().count() < max_chars {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
                ));
            }
        }
    }
    *filter_out = editable.value().to_string();
    if filter_out.is_empty() {
        **text = placeholder.into();
    } else {
        **text = filter_out.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_all() {
        assert!(text_filter_matches("", "Nuntburg"));
        assert!(text_filter_matches("  ", "x"));
    }

    #[test]
    fn substring_case_insensitive() {
        assert!(text_filter_matches("nun", "Nuntburg"));
        assert!(!text_filter_matches("xyz", "Nuntburg"));
    }
}
