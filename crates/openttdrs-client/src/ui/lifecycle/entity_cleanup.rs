//! Despawn de entidades de sesión InGame (markers inventariables).

use bevy::prelude::*;

use crate::audio::MusicPlayer;
use crate::debug_gizmos::DiagnosticsOverlayRoot;
use crate::render::{
    IndustryPreviewCamera, MapPreviewCamera, MapVisualLayer, PrimaryGameCamera, ShoreTile,
    WaterTile,
};
use crate::state::ingame_lifecycle::InGameUi;

use super::super::floating_window::FloatingWindow;
use super::super::hud::TileInfoText;
use super::super::statusbar::{NewsPopupRoot, StatusBarRoot};
use super::super::toolbar::{BuildGhostPreview, MinimapRoot, OrderPanelRoot, RailSignalGhost};

/// Entrada del registro de markers a despawnear al salir de InGame.
pub(super) struct EntityTeardown {
    /// Inventario / tests (`registry_tests`); no se lee en runtime.
    #[allow(dead_code)]
    pub name: &'static str,
    pub collect: fn(&mut World, &mut Vec<Entity>),
}

fn collect_matching<M: Component>(world: &mut World, out: &mut Vec<Entity>) {
    let mut query = world.query_filtered::<Entity, With<M>>();
    for entity in query.iter(world) {
        out.push(entity);
    }
}

/// Orden estable: mundo/UI primero; `MusicPlayer` al final (paridad con el teardown previo).
pub(super) static ENTITY_TEARDOWNS: &[EntityTeardown] = &[
    EntityTeardown {
        name: "PrimaryGameCamera",
        collect: collect_matching::<PrimaryGameCamera>,
    },
    EntityTeardown {
        name: "MapVisualLayer",
        collect: collect_matching::<MapVisualLayer>,
    },
    EntityTeardown {
        name: "WaterTile",
        collect: collect_matching::<WaterTile>,
    },
    EntityTeardown {
        name: "ShoreTile",
        collect: collect_matching::<ShoreTile>,
    },
    EntityTeardown {
        name: "StatusBarRoot",
        collect: collect_matching::<StatusBarRoot>,
    },
    EntityTeardown {
        name: "MinimapRoot",
        collect: collect_matching::<MinimapRoot>,
    },
    EntityTeardown {
        name: "OrderPanelRoot",
        collect: collect_matching::<OrderPanelRoot>,
    },
    EntityTeardown {
        name: "FloatingWindow",
        collect: collect_matching::<FloatingWindow>,
    },
    EntityTeardown {
        name: "TileInfoText",
        collect: collect_matching::<TileInfoText>,
    },
    EntityTeardown {
        name: "DiagnosticsOverlayRoot",
        collect: collect_matching::<DiagnosticsOverlayRoot>,
    },
    EntityTeardown {
        name: "NewsPopupRoot",
        collect: collect_matching::<NewsPopupRoot>,
    },
    EntityTeardown {
        name: "BuildGhostPreview",
        collect: collect_matching::<BuildGhostPreview>,
    },
    EntityTeardown {
        name: "RailSignalGhost",
        collect: collect_matching::<RailSignalGhost>,
    },
    EntityTeardown {
        name: "MapPreviewCamera",
        collect: collect_matching::<MapPreviewCamera>,
    },
    EntityTeardown {
        name: "IndustryPreviewCamera",
        collect: collect_matching::<IndustryPreviewCamera>,
    },
    EntityTeardown {
        name: "InGameUi",
        collect: collect_matching::<InGameUi>,
    },
    EntityTeardown {
        name: "MusicPlayer",
        collect: collect_matching::<MusicPlayer>,
    },
];

/// Despawna todas las entidades registradas para la sesión InGame.
pub(super) fn despawn_ingame_entities(world: &mut World) {
    let mut to_despawn: Vec<Entity> = Vec::new();
    for entry in ENTITY_TEARDOWNS {
        (entry.collect)(world, &mut to_despawn);
    }
    to_despawn.sort_unstable();
    to_despawn.dedup();

    let mut commands = world.commands();
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
}

/// Nombres de markers con política de despawn (para inventario).
#[cfg(test)]
pub(super) fn entity_teardown_names() -> Vec<&'static str> {
    ENTITY_TEARDOWNS.iter().map(|e| e.name).collect()
}
