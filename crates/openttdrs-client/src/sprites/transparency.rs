//! Opciones de transparencia / invisibilidad (`TransparencyOption` de OpenTTD).
//!
//! Tres modos por categoría (UI): Visible → Transparente → Oculta.
//! Almacenamiento fiel a upstream: bit en `_transparency_opt` y, si oculta,
//! también en `_invisibility_opt` (`IsInvisibilitySet` exige ambos).

use std::sync::atomic::{AtomicU32, Ordering};

use bevy::prelude::Color;

/// Categorías de `transparency.h` (índice = bit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TransparencyOption {
    Signs = 0,
    Trees = 1,
    Houses = 2,
    Industries = 3,
    Buildings = 4,
    Bridges = 5,
    Structures = 6,
    Catenary = 7,
    Text = 8,
}

impl TransparencyOption {
    pub const ALL: [Self; 9] = [
        Self::Signs,
        Self::Trees,
        Self::Houses,
        Self::Industries,
        Self::Buildings,
        Self::Bridges,
        Self::Structures,
        Self::Catenary,
        Self::Text,
    ];

    #[must_use]
    pub const fn bit(self) -> u32 {
        1u32 << (self as u8)
    }

    #[must_use]
    pub const fn label_es(self) -> &'static str {
        match self {
            Self::Signs => "Carteles",
            Self::Trees => "Árboles",
            Self::Houses => "Casas",
            Self::Industries => "Industrias",
            Self::Buildings => "Edificios",
            Self::Bridges => "Puentes",
            Self::Structures => "Estructuras",
            Self::Catenary => "Catenaria",
            Self::Text => "Textos",
        }
    }
}

/// Modo de UI (ciclo Visible / Transparente / Oculta).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransparencyMode {
    #[default]
    Visible,
    Transparent,
    Hidden,
}

impl TransparencyMode {
    #[must_use]
    pub const fn label_es(self) -> &'static str {
        match self {
            Self::Visible => "Visible",
            Self::Transparent => "Transparente",
            Self::Hidden => "Oculta",
        }
    }
}

static TRANSPARENCY_OPT: AtomicU32 = AtomicU32::new(0);
static INVISIBILITY_OPT: AtomicU32 = AtomicU32::new(0);

/// Alpha de sprites/textos en modo transparente (catenaria legacy usaba 0.45).
pub const TRANSPARENT_ALPHA: f32 = 0.45;

/// Sincroniza bitsets persistidos con el render (llamar al hidratar / cambiar prefs).
pub fn set_transparency_preferences(transparency_opt: u32, invisibility_opt: u32) {
    TRANSPARENCY_OPT.store(transparency_opt, Ordering::Relaxed);
    INVISIBILITY_OPT.store(invisibility_opt, Ordering::Relaxed);
}

#[must_use]
pub fn transparency_opt() -> u32 {
    TRANSPARENCY_OPT.load(Ordering::Relaxed)
}

#[must_use]
pub fn invisibility_opt() -> u32 {
    INVISIBILITY_OPT.load(Ordering::Relaxed)
}

/// Modo efectivo de una categoría (puro sobre bitsets).
#[must_use]
pub fn mode_from_bits(
    transparency: u32,
    invisibility: u32,
    to: TransparencyOption,
) -> TransparencyMode {
    let bit = to.bit();
    if invisibility & bit != 0 {
        TransparencyMode::Hidden
    } else if transparency & bit != 0 {
        TransparencyMode::Transparent
    } else {
        TransparencyMode::Visible
    }
}

/// Escribe el modo en los bitsets (mutación pura, testeable).
#[must_use]
pub fn apply_mode_to_bits(
    transparency: u32,
    invisibility: u32,
    to: TransparencyOption,
    mode: TransparencyMode,
) -> (u32, u32) {
    let bit = to.bit();
    match mode {
        TransparencyMode::Visible => (transparency & !bit, invisibility & !bit),
        TransparencyMode::Transparent => (transparency | bit, invisibility & !bit),
        // OpenTTD: invisibilidad implica transparencia.
        TransparencyMode::Hidden => (transparency | bit, invisibility | bit),
    }
}

#[must_use]
pub fn mode(to: TransparencyOption) -> TransparencyMode {
    mode_from_bits(transparency_opt(), invisibility_opt(), to)
}

#[must_use]
pub fn is_hidden(to: TransparencyOption) -> bool {
    mode(to) == TransparencyMode::Hidden
}

#[must_use]
pub fn is_transparent(to: TransparencyOption) -> bool {
    mode(to) == TransparencyMode::Transparent
}

/// Tint para sprites de la categoría (blanco u alpha).
#[must_use]
pub fn sprite_color(to: TransparencyOption) -> Color {
    if is_transparent(to) {
        Color::srgba(1.0, 1.0, 1.0, TRANSPARENT_ALPHA)
    } else {
        Color::WHITE
    }
}

/// Color de texto 2D (carteles / indicadores).
#[must_use]
pub fn text_color(to: TransparencyOption, base: Color) -> Color {
    if !is_transparent(to) {
        return base;
    }
    let c = base.to_srgba();
    Color::srgba(c.red, c.green, c.blue, TRANSPARENT_ALPHA)
}

/// Conserva RGB y aplica alpha de transparencia si corresponde.
#[must_use]
pub fn with_to_alpha(base: Color, to: TransparencyOption) -> Color {
    if !is_transparent(to) {
        return base;
    }
    let c = base.to_srgba();
    Color::srgba(c.red, c.green, c.blue, TRANSPARENT_ALPHA)
}

// --- Compat catenaria (API previa) -----------------------------------------

/// Equivalente a `set_catenary_preferences` legacy.
#[allow(dead_code)]
pub fn set_catenary_preferences(hidden: bool, transparent: bool) {
    let mode = if hidden {
        TransparencyMode::Hidden
    } else if transparent {
        TransparencyMode::Transparent
    } else {
        TransparencyMode::Visible
    };
    let (t, i) = apply_mode_to_bits(
        transparency_opt(),
        invisibility_opt(),
        TransparencyOption::Catenary,
        mode,
    );
    set_transparency_preferences(t, i);
}

#[must_use]
pub fn catenary_hidden() -> bool {
    is_hidden(TransparencyOption::Catenary) || crate::config::env_flag("OPENTTDRS_HIDE_CATENARY")
}

#[must_use]
pub fn catenary_transparent() -> bool {
    if crate::config::env_flag("OPENTTDRS_HIDE_CATENARY") {
        return false;
    }
    is_transparent(TransparencyOption::Catenary)
        || crate::config::env_flag("OPENTTDRS_TRANSPARENT_CATENARY")
}

#[must_use]
pub fn catenary_sprite_color() -> Color {
    if catenary_transparent() {
        Color::srgba(1.0, 1.0, 1.0, TRANSPARENT_ALPHA)
    } else {
        Color::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_roundtrip_bits() {
        let (t, i) = apply_mode_to_bits(
            0,
            0,
            TransparencyOption::Trees,
            TransparencyMode::Transparent,
        );
        assert_eq!(
            mode_from_bits(t, i, TransparencyOption::Trees),
            TransparencyMode::Transparent
        );
        let (t, i) = apply_mode_to_bits(t, i, TransparencyOption::Trees, TransparencyMode::Hidden);
        assert_eq!(
            mode_from_bits(t, i, TransparencyOption::Trees),
            TransparencyMode::Hidden
        );
        assert!(t & TransparencyOption::Trees.bit() != 0);
        assert!(i & TransparencyOption::Trees.bit() != 0);
        let (t, i) = apply_mode_to_bits(t, i, TransparencyOption::Trees, TransparencyMode::Visible);
        assert_eq!(
            mode_from_bits(t, i, TransparencyOption::Trees),
            TransparencyMode::Visible
        );
        assert_eq!(t & TransparencyOption::Trees.bit(), 0);
    }

    #[test]
    fn catenary_wrappers_update_atomics() {
        set_transparency_preferences(0, 0);
        set_catenary_preferences(false, true);
        assert!(catenary_transparent());
        assert!(!catenary_hidden());
        set_catenary_preferences(true, false);
        assert!(catenary_hidden());
        set_catenary_preferences(false, false);
        assert!(!catenary_hidden() && !catenary_transparent());
    }
}
