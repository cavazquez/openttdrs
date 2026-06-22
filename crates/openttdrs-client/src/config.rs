//! Configuración del cliente leída desde variables de entorno.

/// OpenTTD (~74 ticks/día a velocidad normal); solo etiqueta de HUD / fecha simulada.
pub(crate) const SIM_TICKS_PER_DAY: u64 = 74;
pub(crate) const SIM_DAYS_PER_YEAR: u64 = 365;

pub(crate) const DEFAULT_JSON_SAVE_PATH: &str = "save/openttdrs_sim.json";

pub(crate) fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub(crate) fn env_flag(name: &str) -> bool {
    env_string(name).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub(crate) fn env_u32_in_range(
    name: &str,
    default: u32,
    range: std::ops::RangeInclusive<u32>,
) -> u32 {
    env_string(name)
        .and_then(|s| s.parse().ok())
        .filter(|v| range.contains(v))
        .unwrap_or(default)
}

pub(crate) fn env_u8_in_range(name: &str, default: u8, range: std::ops::RangeInclusive<u8>) -> u8 {
    env_string(name)
        .and_then(|s| s.parse().ok())
        .filter(|v| range.contains(v))
        .unwrap_or(default)
}

/// Sobrescribe `GameState::company_colour` con `OPENTTDRS_COMPANY_COLOUR` (0–15) para QA.
pub(crate) fn apply_test_company_colour(state: &mut openttdrs_core::GameState) {
    let colour = env_u8_in_range("OPENTTDRS_COMPANY_COLOUR", state.company_colour, 0..=15);
    if std::env::var_os("OPENTTDRS_COMPANY_COLOUR").is_some() {
        state.company_colour = colour;
    }
}

pub(crate) fn json_save_path() -> String {
    env_string("OPENTTDRS_JSON_SAVE").unwrap_or_else(|| DEFAULT_JSON_SAVE_PATH.into())
}

/// Etiqueta corta para el HUD: carpeta + archivo (`save/partida.json`) cuando hay directorio;
/// sin carpeta (solo nombre de archivo) devuelve ese nombre; rutas absolutas usan el último
/// segmento de la carpeta contenedora (p. ej. `.../save/x.json` → `save/x.json`).
#[must_use]
pub(crate) fn json_save_hud_label(path: &str) -> String {
    let p = std::path::Path::new(path);
    let Some(file) = p
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
    else {
        return path.to_string();
    };
    let Some(parent) = p.parent() else {
        return file.to_string();
    };
    if parent.as_os_str().is_empty() {
        return file.to_string();
    }
    let Some(dir) = parent
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != ".")
    else {
        return file.to_string();
    };
    format!("{dir}/{file}")
}

/// Acorta texto para una sola línea de HUD (Unicode-contable).
#[must_use]
pub(crate) fn truncate_hud_line(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}
