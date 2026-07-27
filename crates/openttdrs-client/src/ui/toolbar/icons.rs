//! Catálogo tipado de sprites `SPR_IMG_*` extraídos desde OpenGFX (#238).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToolbarIcon {
    Pause,
    FastForward,
    Settings,
    Save,
    SmallMap,
    Town,
    Industry,
    BuildRail,
    BuildRoad,
    BuildTram,
    BuildWater,
    BuildAir,
    Landscape,
    Trees,
    Sign,
    ZoomIn,
    ZoomOut,
    Music,
    Messages,
    Help,
    Switch,
    Fleet,
    Finances,
}

impl ToolbarIcon {
    #[must_use]
    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::Pause => "assets/opengfx/tiles/toolbar_pause.png",
            Self::FastForward => "assets/opengfx/tiles/toolbar_fast_forward.png",
            Self::Settings => "assets/opengfx/tiles/ui_settings.png",
            Self::Save => "assets/opengfx/tiles/toolbar_save.png",
            Self::SmallMap => "assets/opengfx/tiles/toolbar_smallmap.png",
            Self::Town => "assets/opengfx/tiles/toolbar_town.png",
            Self::Industry => "assets/opengfx/tiles/toolbar_industry.png",
            Self::BuildRail => "assets/opengfx/tiles/toolbar_build_rail.png",
            Self::BuildRoad => "assets/opengfx/tiles/toolbar_build_road.png",
            Self::BuildTram => "assets/opengfx/tiles/toolbar_build_tram.png",
            Self::BuildWater => "assets/opengfx/tiles/toolbar_build_water.png",
            Self::BuildAir => "assets/opengfx/tiles/toolbar_build_air.png",
            Self::Landscape => "assets/opengfx/tiles/toolbar_landscape.png",
            Self::Trees => "assets/opengfx/tiles/toolbar_trees.png",
            Self::Sign => "assets/opengfx/tiles/toolbar_sign.png",
            Self::ZoomIn => "assets/opengfx/tiles/toolbar_zoom_in.png",
            Self::ZoomOut => "assets/opengfx/tiles/toolbar_zoom_out.png",
            Self::Music => "assets/opengfx/tiles/ui_sound.png",
            Self::Messages => "assets/opengfx/tiles/toolbar_messages.png",
            Self::Help => "assets/opengfx/tiles/toolbar_help.png",
            Self::Switch => "assets/opengfx/tiles/toolbar_switch.png",
            Self::Fleet => "assets/opengfx/tiles/toolbar_trains.png",
            Self::Finances => "assets/opengfx/tiles/toolbar_finances.png",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_catalog_uses_unique_png_paths() {
        let icons = [
            ToolbarIcon::Pause,
            ToolbarIcon::FastForward,
            ToolbarIcon::Settings,
            ToolbarIcon::Save,
            ToolbarIcon::SmallMap,
            ToolbarIcon::Town,
            ToolbarIcon::Industry,
            ToolbarIcon::BuildRail,
            ToolbarIcon::BuildRoad,
            ToolbarIcon::BuildTram,
            ToolbarIcon::BuildWater,
            ToolbarIcon::BuildAir,
            ToolbarIcon::Landscape,
            ToolbarIcon::Trees,
            ToolbarIcon::Sign,
            ToolbarIcon::ZoomIn,
            ToolbarIcon::ZoomOut,
            ToolbarIcon::Music,
            ToolbarIcon::Messages,
            ToolbarIcon::Help,
            ToolbarIcon::Switch,
            ToolbarIcon::Fleet,
            ToolbarIcon::Finances,
        ];
        let mut paths = icons.iter().map(|icon| icon.path()).collect::<Vec<_>>();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), icons.len());
        assert!(paths.iter().all(|path| path.ends_with(".png")));
    }
}
