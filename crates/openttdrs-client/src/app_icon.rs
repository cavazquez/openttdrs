use std::path::{Path, PathBuf};

use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;
use bevy::{
    log::{info, warn},
    prelude::*,
};
use image::imageops::FilterType;
use winit::window::Icon;

pub(crate) const APP_ICON_RELATIVE_PATH: &str = "static/app/openttdrs-icon.png";

/// Tamaño que aceptan bien la mayoría de compositors (icono 1254×1254 suele ignorarse).
const WINDOW_ICON_PX: u32 = 128;

#[derive(Resource)]
pub(crate) struct AppIconPath(PathBuf);

#[derive(Resource, Default)]
pub(crate) struct AppIconApplied(bool);

pub(crate) struct AppIconPlugin {
    icon_path: PathBuf,
}

impl AppIconPlugin {
    pub(crate) fn new(asset_root: &str) -> Self {
        Self {
            icon_path: Path::new(asset_root).join(APP_ICON_RELATIVE_PATH),
        }
    }
}

impl Plugin for AppIconPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AppIconPath(self.icon_path.clone()))
            .init_resource::<AppIconApplied>()
            .add_systems(PostStartup, apply_window_icon)
            .add_systems(Update, apply_window_icon);
    }
}

fn apply_window_icon(
    // `WINIT_WINDOWS` es thread-local del hilo principal: sin este marker el
    // sistema corre en el pool, ve la tabla vacía, nunca aplica el icono y
    // re-decodifica el PNG (~50 ms) en *cada frame* (bug de 20 FPS).
    _main_thread: bevy::ecs::system::NonSendMarker,
    mut applied: ResMut<AppIconApplied>,
    mut cached_icon: Local<Option<Icon>>,
    icon_path: Res<AppIconPath>,
    primary: Query<Entity, With<PrimaryWindow>>,
    all_windows: Query<Entity, With<Window>>,
) {
    if applied.0 {
        return;
    }

    if cached_icon.is_none() {
        *cached_icon = load_window_icon(&icon_path.0);
    }
    let Some(icon) = cached_icon.as_ref() else {
        warn!(
            "No se pudo cargar el icono de la aplicación desde {}",
            icon_path.0.display()
        );
        applied.0 = true;
        return;
    };

    let mut set_any = false;
    WINIT_WINDOWS.with_borrow(|winit_windows| {
        let targets: Vec<Entity> = if let Ok(p) = primary.single() {
            vec![p]
        } else {
            all_windows.iter().collect()
        };
        for entity in targets {
            let Some(window) = winit_windows.get_window(entity) else {
                continue;
            };
            window.set_window_icon(Some(icon.clone()));
            set_any = true;
        }
    });

    if set_any {
        info!(
            "Icono de ventana aplicado desde {} ({}×{} px)",
            icon_path.0.display(),
            WINDOW_ICON_PX,
            WINDOW_ICON_PX
        );
        applied.0 = true;
    }
}

fn load_window_icon(path: &Path) -> Option<Icon> {
    let image = image::ImageReader::open(path)
        .ok()?
        .decode()
        .ok()?
        .into_rgba8();
    let image = if image.width() == WINDOW_ICON_PX && image.height() == WINDOW_ICON_PX {
        image
    } else {
        image::imageops::resize(&image, WINDOW_ICON_PX, WINDOW_ICON_PX, FilterType::Lanczos3)
    };
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_icon_path_is_named_for_the_application() {
        assert_eq!(APP_ICON_RELATIVE_PATH, "static/app/openttdrs-icon.png");
    }

    #[test]
    fn missing_icon_returns_none() {
        assert!(load_window_icon(Path::new("assets/app/nope.png")).is_none());
    }

    #[test]
    fn bundled_icon_loads_at_window_size() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(APP_ICON_RELATIVE_PATH);
        assert!(load_window_icon(&path).is_some());
    }
}
