use bevy::prelude::*;

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::ui::hud::SimHudControls;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

use super::SaveMenuAction;

pub(crate) fn handle_settings_menu_buttons(
    mut q: Query<(&Interaction, &SaveMenuAction), (Changed<Interaction>, With<Button>)>,
    mut hud: ResMut<SimHudControls>,
    mut save_window: ResMut<SaveWindowState>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SaveMenuAction::SaveAs => {
                save_window.open_in_mode(SaveWindowMode::Save, &save_dir_from(&hud.json_save_path));
            }
            SaveMenuAction::LoadFrom => {
                save_window.open_in_mode(SaveWindowMode::Load, &save_dir_from(&hud.json_save_path));
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::ui::hud::SimHudControls;
    use crate::ui::save_window::{SaveWindowMode, SaveWindowState};
    use crate::ui::toolbar::SaveMenuAction;

    use super::handle_settings_menu_buttons;

    #[test]
    fn save_and_load_buttons_open_save_window() {
        let mut world = World::new();
        world.insert_resource(SimHudControls::default());
        world.insert_resource(SaveWindowState::default());

        world.spawn((Button, SaveMenuAction::SaveAs, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        {
            let w = world.resource::<SaveWindowState>();
            assert!(w.open);
            assert_eq!(w.mode, SaveWindowMode::Save);
        }

        world.spawn((Button, SaveMenuAction::LoadFrom, Interaction::Pressed));
        world.run_system_once(handle_settings_menu_buttons).unwrap();
        let w = world.resource::<SaveWindowState>();
        assert!(w.open);
        assert_eq!(w.mode, SaveWindowMode::Load);
    }
}
