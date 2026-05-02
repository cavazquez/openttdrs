use bevy::prelude::*;
use openttdrs_core::TileKind;

pub(super) fn minimap_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Water => Color::srgb(0.08, 0.25, 0.55),
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel => {
            Color::srgb(0.48, 0.42, 0.32)
        }
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel => {
            Color::srgb(0.68, 0.68, 0.62)
        }
        TileKind::House => Color::srgb(0.72, 0.28, 0.2),
        TileKind::Industry | TileKind::CoalField => Color::srgb(0.78, 0.64, 0.2),
        TileKind::Station => Color::srgb(0.95, 0.95, 0.86),
        TileKind::Forest => Color::srgb(0.05, 0.34, 0.1),
        TileKind::Grass => Color::srgb(0.16, 0.48, 0.12),
        TileKind::Void => Color::srgb(0.02, 0.02, 0.02),
        TileKind::Unknown(_) => Color::srgb(0.38, 0.12, 0.45),
    }
}
