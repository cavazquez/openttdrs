//! Tipos y lógica de planificación para preview de construcción.
//!
//! Este módulo contiene la representación pura de qué se debe previsualizar,
//! separada de la lógica de creación de entidades Bevy.

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::ui::toolbar::{BuildMenuAction, DragBuildState, StationBuildState};

/// Contexto necesario para calcular el plan de preview.
pub(crate) struct PreviewContext<'a> {
    pub map: &'a Map,
    pub action: BuildMenuAction,
    pub cursor_tile: (i32, i32),
    pub tile_fract: (u8, u8),
    pub station_state: &'a StationBuildState,
    pub drag_state: &'a DragBuildState,
    pub rail_lane_bit: Option<u8>,
}

/// Plan de preview: qué tiles mostrar y cómo.
#[derive(Debug, Clone)]
pub(crate) enum PreviewPlan {
    /// Sin preview (tool inválido, fuera de mapa, etc.)
    None,
    /// Preview manejado por sistema dedicado (RailSignals)
    HandledByDedicatedSystem,
    /// Preview de estación de tren (área completa)
    RailStation {
        origin: TileCoord,
        show_coverage: bool,
    },
    /// Preview de aeropuerto
    Airport {
        origin: TileCoord,
        show_coverage: bool,
    },
    /// Preview de waypoint ferroviario
    RailWaypoint { coord: TileCoord, valid: bool },
    /// Preview de waypoint de carretera
    RoadWaypoint { coord: TileCoord, valid: bool },
    /// Preview de puente (span completo)
    BridgeSpan { tiles: Vec<(i32, i32)>, valid: bool },
    /// Preview de señales ferroviarias (arrastre multi-tile)
    RailSignalDrag {
        tiles: Vec<(i32, i32)>,
        signal_fract: (u8, u8),
    },
    /// Preview genérico por tile
    TileByTile { tiles: Vec<TilePreviewPlan> },
}

/// Plan de preview para un tile individual.
#[derive(Debug, Clone)]
pub(crate) struct TilePreviewPlan {
    pub coord: TileCoord,
    pub valid: bool,
    pub kind: TilePreviewKind,
}

/// Tipo de preview a mostrar en un tile.
#[derive(Debug, Clone)]
pub(crate) enum TilePreviewKind {
    /// Industria (template multi-tile)
    Industry { spec: openttdrs_core::IndustrySpec },
    /// Parada de carretera (bus/camión)
    RoadStop { is_bus: bool, dir: usize },
    /// Vía ferroviaria (ghost overlay)
    Rail {
        bits: u8,
        tileh: u8,
        rail_type: openttdrs_core::RailType,
    },
    /// Depósito de carretera
    RoadDepot { dir: usize },
    /// Depósito ferroviario
    RailDepot { dir: usize },
    /// Carretera/tranvía
    Road { path: String },
    /// Túnel (entrada)
    Tunnel,
    /// Sprite genérico (imagen del action)
    GenericSprite,
}

/// Calcula los tiles de preview según acción y estado de arrastre.
pub(crate) fn compute_preview_tiles(ctx: &PreviewContext) -> Vec<(i32, i32)> {
    let (tx, ty) = ctx.cursor_tile;
    let action = ctx.action;
    let drag_state = ctx.drag_state;

    // Parada bus/camión: siempre 1×1 en el cursor (no arrastre).
    if matches!(action, BuildMenuAction::BusStop | BuildMenuAction::Station) {
        return vec![(tx, ty)];
    }

    // Túnel: path de preview
    if super::validation::action_is_tunnel(action) {
        let start = TileCoord::new(tx, ty);
        return openttdrs_core::tunnel_preview_path(ctx.map, start)
            .map(|path| path.into_iter().map(|c| (c.x, c.y)).collect())
            .unwrap_or_else(|| vec![(tx, ty)]);
    }

    // Puente: arrastre armado
    if matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge | BuildMenuAction::Aqueduct
    ) && drag_state.armed
        && let Some(start) = drag_state.start_tile
    {
        return super::super::build_input::drag::drag_line_tiles(
            Some(ctx.map),
            action,
            start,
            (tx, ty),
        );
    }

    // Arrastre activo: tiles pendientes
    if drag_state.last_action == Some(action) && !drag_state.pending_tiles.is_empty() {
        return drag_state.pending_tiles.clone();
    }

    // Por defecto: tile bajo cursor
    vec![(tx, ty)]
}

/// Determina el tint (color de preview) según validez.
pub(crate) fn preview_tint(valid: bool) -> Color {
    if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.55)
    } else {
        Color::srgba(1.0, 0.25, 0.2, 0.55)
    }
}

/// Tint para señales ferroviarias (arrastre).
pub(crate) fn rail_signal_tint(valid: bool) -> Color {
    if valid {
        Color::srgba(0.2, 0.85, 0.35, 0.4)
    } else {
        Color::srgba(0.9, 0.2, 0.15, 0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_tint_colors() {
        let valid = preview_tint(true);
        let invalid = preview_tint(false);
        // Verificar que los tints se generan correctamente
        assert_eq!(valid, Color::srgba(1.0, 1.0, 1.0, 0.55));
        assert_eq!(invalid, Color::srgba(1.0, 0.25, 0.2, 0.55));
    }

    #[test]
    fn rail_signal_tint_green_when_valid() {
        let tint = rail_signal_tint(true);
        assert_eq!(tint, Color::srgba(0.2, 0.85, 0.35, 0.4));
    }

    #[test]
    fn compute_preview_tiles_bus_stop_single() {
        let map = Map::new_flat(10, 10, 0);
        let ctx = PreviewContext {
            map: &map,
            action: BuildMenuAction::BusStop,
            cursor_tile: (5, 5),
            tile_fract: (0, 0),
            station_state: &StationBuildState::default(),
            drag_state: &DragBuildState::default(),
            rail_lane_bit: None,
        };
        let tiles = compute_preview_tiles(&ctx);
        assert_eq!(tiles, vec![(5, 5)]);
    }
}
