//! Icono de bandeja del sistema (Linux StatusNotifierItem vía `ksni`).
//!
//! Usa `static/app/openttdrs-icon.png` (el del README), escalado a 64×64.
//! En GNOME hace falta la extensión AppIndicator / StatusNotifier.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

use bevy::app::AppExit;
use bevy::log::{info, warn};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use image::imageops::FilterType;
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;

use crate::app_icon::APP_ICON_RELATIVE_PATH;

const TRAY_ICON_PX: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayCommand {
    ShowWindow,
    Quit,
}

#[derive(Resource)]
struct TrayCommandRx(Receiver<TrayCommand>);

/// Mantiene vivo el servicio D-Bus del tray (hilo interno de `ksni`).
#[allow(dead_code)]
struct TrayServiceHandle(ksni::blocking::Handle<OpenttdrsTray>);

struct OpenttdrsTray {
    tx: Sender<TrayCommand>,
    icon: ksni::Icon,
}

impl ksni::Tray for OpenttdrsTray {
    fn id(&self) -> String {
        "openttdrs".into()
    }

    fn title(&self) -> String {
        "openttdrs".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "openttdrs".into(),
            description: "Cliente OpenTTDRS".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCommand::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Mostrar ventana".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Salir".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub(crate) struct TrayIconPlugin {
    icon_path: PathBuf,
}

impl TrayIconPlugin {
    pub(crate) fn new(asset_root: &str) -> Self {
        Self {
            icon_path: Path::new(asset_root).join(APP_ICON_RELATIVE_PATH),
        }
    }
}

impl Plugin for TrayIconPlugin {
    fn build(&self, app: &mut App) {
        let Some(icon) = load_tray_ksni_icon(&self.icon_path) else {
            warn!(
                "Bandeja: no se pudo cargar el icono desde {}",
                self.icon_path.display()
            );
            return;
        };
        let (tx, rx) = crossbeam_channel::unbounded();
        let tray = OpenttdrsTray { tx, icon };
        match tray.spawn() {
            Ok(handle) => {
                info!(
                    "Icono de bandeja activo ({}×{} desde {})",
                    TRAY_ICON_PX,
                    TRAY_ICON_PX,
                    self.icon_path.display()
                );
                app.insert_non_send(TrayServiceHandle(handle))
                    .insert_resource(TrayCommandRx(rx))
                    .add_systems(Update, handle_tray_commands);
            }
            Err(err) => {
                warn!("Bandeja no disponible (¿sesión D-Bus / AppIndicator?): {err}");
            }
        }
    }
}

fn handle_tray_commands(
    rx: Option<Res<TrayCommandRx>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(rx) = rx else {
        return;
    };
    loop {
        match rx.0.try_recv() {
            Ok(TrayCommand::ShowWindow) => {
                if let Ok(mut window) = windows.single_mut() {
                    window.visible = true;
                    window.focused = true;
                }
            }
            Ok(TrayCommand::Quit) => {
                exit.write(AppExit::Success);
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
}

fn load_tray_ksni_icon(path: &Path) -> Option<ksni::Icon> {
    let image = image::ImageReader::open(path)
        .ok()?
        .decode()
        .ok()?
        .into_rgba8();
    let image = if image.width() == TRAY_ICON_PX && image.height() == TRAY_ICON_PX {
        image
    } else {
        image::imageops::resize(&image, TRAY_ICON_PX, TRAY_ICON_PX, FilterType::Lanczos3)
    };
    let (width, height) = image.dimensions();
    let mut data = image.into_raw();
    // StatusNotifierItem espera ARGB; `image` entrega RGBA.
    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    Some(ksni::Icon {
        width: i32::try_from(width).ok()?,
        height: i32::try_from(height).ok()?,
        data,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_loads_for_tray() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(APP_ICON_RELATIVE_PATH);
        let icon = load_tray_ksni_icon(&path).expect("icon");
        assert_eq!(icon.width, i32::try_from(TRAY_ICON_PX).unwrap());
        assert_eq!(icon.height, i32::try_from(TRAY_ICON_PX).unwrap());
        assert_eq!(
            icon.data.len(),
            usize::try_from(TRAY_ICON_PX * TRAY_ICON_PX * 4).unwrap()
        );
    }
}
