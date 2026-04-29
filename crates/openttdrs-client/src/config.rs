//! Configuración del cliente leída desde variables de entorno.

pub(crate) const DEFAULT_JSON_SAVE_PATH: &str = "openttdrs_sim.json";

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

pub(crate) fn json_save_path() -> String {
    env_string("OPENTTDRS_JSON_SAVE").unwrap_or_else(|| DEFAULT_JSON_SAVE_PATH.into())
}
