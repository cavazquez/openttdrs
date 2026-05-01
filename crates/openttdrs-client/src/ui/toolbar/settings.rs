use bevy::prelude::*;
use openttdrs_core::save;
#[cfg(not(test))]
use std::path::Path;

use crate::render::{
    IndustryPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, VehicleIndex,
};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;

use super::SaveMenuAction;

pub(crate) fn handle_settings_menu_buttons(
    mut q: Query<(&Interaction, &SaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut hud: ResMut<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SaveMenuAction::SaveAs => {
                let Some(save_path) = choose_save_path(&hud.json_save_path) else {
                    continue;
                };
                hud.json_save_path = save_path.clone();
                match save::save(&sim.state, std::path::Path::new(&save_path)) {
                    Ok(()) => info!("Guardado en {save_path}"),
                    Err(e) => error!("No se pudo guardar en {save_path}: {e}"),
                }
            }
            SaveMenuAction::LoadFrom => {
                let Some(save_path) = choose_load_path(&hud.json_save_path) else {
                    continue;
                };
                hud.json_save_path = save_path.clone();
                match std::fs::read_to_string(&save_path) {
                    Ok(text) => match save::load_from_str(&text) {
                        Ok(loaded) => {
                            let prev = sim.state.map.dimensions();
                            let nw = loaded.map.dimensions();
                            sim.state = loaded;
                            sim.ottdmap_extras = None;
                            sim.loaded_file = true;
                            vehicle_index.rebuild(&sim.state.vehicles);
                            remap.pending = true;
                            remap.sync_camera = true;
                            if prev != nw {
                                info!("Mapa {prev:?} -> {nw:?}; recarga visual y camara.");
                            } else {
                                info!("Estado cargado desde {save_path}; recarga visual.");
                            }
                        }
                        Err(e) => error!("Carga: JSON invalido ({save_path}): {e}"),
                    },
                    Err(e) => error!("Carga: no se pudo leer {save_path}: {e}"),
                }
            }
            SaveMenuAction::PauseResume => {
                hud.paused = !hud.paused;
                info!("Pausa: {}", if hud.paused { "ON" } else { "OFF" });
            }
            SaveMenuAction::SpeedUp => {
                hud.sim_speed = if hud.sim_speed < 1.5 {
                    2.0
                } else if hud.sim_speed < 3.5 {
                    4.0
                } else {
                    1.0
                };
                info!("Velocidad simulacion: {:.0}x", hud.sim_speed);
            }
            SaveMenuAction::Normalize => {
                hud.sim_speed = 1.0;
                if let Ok((mut cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    let keep_pos = cam_tf.translation;
                    o.scale = 1.0;
                    cam_tf.translation = keep_pos;
                }
                info!("Normalizado: velocidad 1x y zoom 1.0x");
            }
            SaveMenuAction::ZoomIn => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 0.85).max(0.25);
                }
            }
            SaveMenuAction::ZoomOut => {
                if let Ok((_cam_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 1.15).min(20.0);
                }
            }
        }
    }
}

#[cfg(test)]
fn choose_save_path(current: &str) -> Option<String> {
    Some(current.to_string())
}

#[cfg(not(test))]
fn choose_save_path(current: &str) -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let mut dialog = rfd::FileDialog::new().add_filter("JSON", &["json"]);
        if let Some(parent) = Path::new(current).parent() {
            dialog = dialog.set_directory(parent);
        }
        if let Some(name) = Path::new(current).file_name().and_then(|n| n.to_str()) {
            dialog = dialog.set_file_name(name);
        }
        return dialog.save_file().map(|p| p.to_string_lossy().to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection")
            .arg("--save")
            .arg("--confirm-overwrite")
            .arg("--title=Guardar simulacion JSON")
            .arg("--file-filter=*.json");
        if Path::new(current).exists() || Path::new(current).parent().is_some() {
            cmd.arg("--filename").arg(current);
        }
        match cmd.output() {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if path.is_empty() { None } else { Some(path) }
            }
            Ok(_) => None,
            Err(e) => {
                error!("No se pudo abrir selector de archivo (zenity): {e}");
                None
            }
        }
    }
}

#[cfg(test)]
fn choose_load_path(current: &str) -> Option<String> {
    Some(current.to_string())
}

#[cfg(not(test))]
fn choose_load_path(current: &str) -> Option<String> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let mut dialog = rfd::FileDialog::new().add_filter("JSON", &["json"]);
        if let Some(parent) = Path::new(current).parent() {
            dialog = dialog.set_directory(parent);
        }
        return dialog.pick_file().map(|p| p.to_string_lossy().to_string());
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let mut cmd = std::process::Command::new("zenity");
        cmd.arg("--file-selection")
            .arg("--title=Cargar simulacion JSON")
            .arg("--file-filter=*.json");
        if Path::new(current).exists() || Path::new(current).parent().is_some() {
            cmd.arg("--filename").arg(current);
        }
        match cmd.output() {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if path.is_empty() { None } else { Some(path) }
            }
            Ok(_) => None,
            Err(e) => {
                error!("No se pudo abrir selector de archivo (zenity): {e}");
                None
            }
        }
    }
}
