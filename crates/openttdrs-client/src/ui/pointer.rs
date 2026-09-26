//! Shared pointer ownership policy for world input and camera navigation.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::dev_console::{DevConsoleState, dev_console_captures_keyboard};
use super::hud::SimHudControls;
use super::modal_stack::ModalStack;
use super::save_window::SaveWindowState;
use super::toolbar::editor_toolbar::EditorExitConfirmRoot;
use super::toolbar::{MinimapLayerState, minimap_contains_cursor};

/// Interactions are resolved by Bevy's UI focus pass before Update. Do not
/// consume the wheel here: the focused list still needs to receive it.
#[derive(SystemParam)]
pub(crate) struct PointerCapture<'w, 's> {
    nodes: Query<
        'w,
        's,
        (
            &'static Interaction,
            &'static Node,
            Option<&'static Visibility>,
        ),
    >,
    exit_modal: Query<'w, 's, &'static Node, With<EditorExitConfirmRoot>>,
    modals: Option<Res<'w, ModalStack>>,
    save: Option<Res<'w, SaveWindowState>>,
    console: Option<Res<'w, DevConsoleState>>,
    hud: Option<Res<'w, SimHudControls>>,
    minimap: Option<Res<'w, MinimapLayerState>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

impl PointerCapture<'_, '_> {
    pub(crate) fn active(&self) -> bool {
        self.modals.as_deref().is_some_and(|m| !m.is_empty())
            || self.save.as_deref().is_some_and(|s| s.open)
            || self
                .console
                .as_deref()
                .is_some_and(dev_console_captures_keyboard)
            || self.exit_modal.iter().any(|n| n.display != Display::None)
            || self.nodes.iter().any(|(interaction, node, visibility)| {
                *interaction != Interaction::None
                    && node.display != Display::None
                    && visibility != Some(&Visibility::Hidden)
            })
            || self.minimap_active()
    }

    fn minimap_active(&self) -> bool {
        if !self.hud.as_deref().is_some_and(|h| h.minimap_visible) {
            return false;
        }
        let (Some(layers), Ok(window)) = (self.minimap.as_deref(), self.windows.single()) else {
            return false;
        };
        window
            .cursor_position()
            .is_some_and(|cursor| minimap_contains_cursor(cursor, window, layers))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(super) mod tests {
    use super::*;
    use crate::camera::{CameraVelocity, move_camera};
    use crate::render::PrimaryGameCamera;
    use crate::state::SimWorld;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};

    pub(crate) fn camera_world(scale: f32) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<Real>::default());
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<AccumulatedMouseMotion>();
        world.init_resource::<AccumulatedMouseScroll>();
        world.init_resource::<CameraVelocity>();
        world.insert_resource(SimWorld::default());
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
        world.spawn((window, PrimaryWindow));
        let camera = world
            .spawn((
                PrimaryGameCamera,
                Transform::default(),
                Projection::Orthographic(OrthographicProjection {
                    scale,
                    ..OrthographicProjection::default_2d()
                }),
            ))
            .id();
        (world, camera)
    }

    #[test]
    fn ui_pointer_capture_blocks_camera_mouse_at_every_zoom() {
        for scale in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0] {
            for modal in [false, true] {
                let (mut world, camera) = camera_world(scale);
                if modal {
                    world.spawn((EditorExitConfirmRoot, Node::default()));
                } else {
                    world.spawn((Node::default(), Interaction::Hovered));
                }
                world
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Right);
                world.resource_mut::<AccumulatedMouseMotion>().delta = Vec2::new(20.0, 0.0);
                world.insert_resource(AccumulatedMouseScroll {
                    unit: MouseScrollUnit::Line,
                    delta: Vec2::Y,
                });
                world.run_system_once(move_camera).unwrap();
                assert_eq!(
                    world.get::<Transform>(camera).unwrap().translation,
                    Vec3::ZERO
                );
                let Projection::Orthographic(proj) = world.get::<Projection>(camera).unwrap()
                else {
                    panic!()
                };
                assert_eq!(proj.scale, scale);
            }
        }
    }

    #[test]
    fn hidden_ui_releases_the_camera_wheel() {
        let (mut world, camera) = camera_world(1.0);
        world.spawn((
            Node {
                display: Display::None,
                ..default()
            },
            Interaction::Hovered,
        ));
        world.spawn((Node::default(), Interaction::Hovered, Visibility::Hidden));
        world.insert_resource(AccumulatedMouseScroll {
            unit: MouseScrollUnit::Line,
            delta: Vec2::Y,
        });
        world.run_system_once(move_camera).unwrap();
        let Projection::Orthographic(proj) = world.get::<Projection>(camera).unwrap() else {
            panic!()
        };
        assert_eq!(proj.scale, 0.5);
    }

    #[test]
    fn minimap_and_save_dialog_capture_without_hovered_buttons() {
        for save in [false, true] {
            let (mut world, camera) = camera_world(1.0);
            if save {
                world.insert_resource(SaveWindowState {
                    open: true,
                    ..default()
                });
            } else {
                world.insert_resource(SimHudControls {
                    minimap_visible: true,
                    ..default()
                });
                world.init_resource::<MinimapLayerState>();
                let window_entity = world
                    .query_filtered::<Entity, With<PrimaryWindow>>()
                    .single(&world)
                    .unwrap();
                let mut window = world.get_mut::<Window>(window_entity).unwrap();
                let cursor = Vec2::new(window.width() - 20.0, window.height() - 60.0);
                window.set_cursor_position(Some(cursor));
            }
            world.insert_resource(AccumulatedMouseScroll {
                unit: MouseScrollUnit::Line,
                delta: Vec2::Y,
            });
            world.run_system_once(move_camera).unwrap();
            let Projection::Orthographic(proj) = world.get::<Projection>(camera).unwrap() else {
                panic!()
            };
            assert_eq!(proj.scale, 1.0);
        }
    }
}
