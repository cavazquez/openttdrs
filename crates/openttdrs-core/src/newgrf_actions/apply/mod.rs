//! Orquestación compartida para aplicar `NewGRF` stack.

use std::path::PathBuf;

use crate::GameState;

pub mod action5;
pub mod airport;
pub mod badges;
pub mod bridges;
pub mod canals;
pub mod cargo;
pub mod houses;
pub mod industry;
pub mod objects;
pub mod rail;
pub mod road;
pub mod roadstop;
pub mod sounds;
pub mod station;
pub mod train;

/// Reconstruye el catálogo de strings genéricos Action4 del stack activo.
pub fn apply_newgrf_strings(state: &mut GameState, search_dirs: &[&std::path::Path]) {
    let mut catalog = crate::newgrf_text::NewGrfStringCatalog::default();
    let stack = state.newgrf_stack.clone();
    for entry in stack.iter().filter(|entry| entry.enabled) {
        let Some(path) = search_dirs
            .iter()
            .map(|dir| dir.join(&entry.filename))
            .find(|path| path.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(path) else {
            continue;
        };
        catalog.extend(
            crate::newgrf_text::collect_action4_generic_strings_from_grf(&data, entry.grfid),
        );
    }
    state.runtime.newgrf_string_catalog = catalog;
}

/// Aplica strings Action4 usando los directorios `NewGRF` estándar.
pub fn apply_newgrf_strings_default_dirs(state: &mut GameState) {
    let owned = default_newgrf_search_dirs();
    let refs: Vec<&std::path::Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_strings(state, &refs);
}

#[must_use]
pub fn default_newgrf_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("assets/opengfx/opengfx2-32ez"),
        PathBuf::from("assets/newgrf"),
    ];
    if let Ok(extra) = std::env::var("OPENTTDRS_NEWGRF_DIR")
        && !extra.trim().is_empty()
    {
        dirs.push(PathBuf::from(extra));
    }
    dirs
}

/// Refresco completo de catálogos Action0 tras cambiar el stack.
pub fn apply_newgrf_stack_catalogs_default_dirs(state: &mut GameState) {
    state.runtime.newgrf_diagnostics.clear();
    apply_newgrf_strings_default_dirs(state);
    // Badges deben existir antes de resolver listas `ReadBadgeList` de
    // road/rail types y vehículos.
    badges::apply_newgrf_badges_default_dirs(state);
    road::apply_newgrf_road_types_default_dirs(state);
    station::apply_newgrf_stations_default_dirs(state);
    // Badges ya fueron aplicados para resolver asociaciones `0xFD`.
    roadstop::apply_newgrf_roadstops_default_dirs(state);
    // Cargoes antes de vehículos e industries para `GetCargoTranslation` (#224).
    cargo::apply_newgrf_cargoes_default_dirs(state);
    train::apply_newgrf_vehicles_trains_default_dirs(state);
    // Industry tiles antes que industries (layouts `0xFE` → gfx global).
    industry::apply_newgrf_industry_tiles_default_dirs(state);
    industry::apply_newgrf_industries_default_dirs(state);
    // `INDY` guarda las listas efectivas de cargos de cada instancia. Tras
    // cargar un SAV, volver a enlazar esas filas al catálogo activo evita
    // perder tipos dinámicos, tasas y stocks al resolver el GRF después del
    // importador (sin volver a ejecutar callbacks de fundación).
    crate::sav::rehydrate_sav_industries_with_catalog(state);
    // Los SAV anteriores a `SLV_32` no guardaban qué industria creó cada
    // campo. Esperar a este punto permite resolver `PlantOnBuild` de GRF
    // custom antes de consumir el RNG global en la pasada de afterload.
    crate::sav::apply_legacy_sav_afterload(state);
    // Airport tiles antes que airports (layouts `0xFE` → gfx global).
    airport::apply_newgrf_airport_tiles_default_dirs(state);
    airport::apply_newgrf_airports_default_dirs(state);
    houses::apply_newgrf_houses_default_dirs(state);
    objects::apply_newgrf_objects_default_dirs(state);
    sounds::apply_newgrf_sounds_default_dirs(state);
    bridges::apply_newgrf_bridges_default_dirs(state);
    canals::apply_newgrf_canals_default_dirs(state);
    rail::apply_newgrf_rail_signals_default_dirs(state);
    action5::apply_newgrf_action5_all_default_dirs(state);
}
