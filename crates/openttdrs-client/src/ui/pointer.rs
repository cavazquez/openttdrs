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
    pub(crate) fn modal_active(&self) -> bool {
        self.modals.as_deref().is_some_and(|m| !m.is_empty())
            || self.save.as_deref().is_some_and(|s| s.open)
            || self
                .console
                .as_deref()
                .is_some_and(dev_console_captures_keyboard)
            || self.exit_modal.iter().any(|n| n.display != Display::None)
    }

    pub(crate) fn active(&self) -> bool {
        self.modal_active()
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

/// One owner for the complete right-button gesture. Contextual actions are
/// emitted on release, only if the gesture never became a drag.
#[derive(Resource, Default)]
pub(crate) struct RightPointerGesture {
    owned: bool,
    dragged: bool,
    distance: f32,
    pending_delta: Vec2,
    pub(crate) right_click: bool,
    pub(crate) pan_delta: Vec2,
}

pub(crate) fn update_right_pointer_gesture(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    capture: PointerCapture,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut gesture: ResMut<RightPointerGesture>,
) {
    gesture.right_click = false;
    gesture.pan_delta = Vec2::ZERO;
    if capture.modal_active()
        || !windows
            .single()
            .is_ok_and(|w| w.focused && w.cursor_position().is_some())
    {
        *gesture = RightPointerGesture::default();
        return;
    }
    let over_ui = capture.active();
    if mouse.just_pressed(MouseButton::Right) {
        *gesture = RightPointerGesture {
            owned: !over_ui,
            ..default()
        };
    }
    if !gesture.owned {
        return;
    }
    gesture.distance += motion.delta.length();
    gesture.pending_delta += motion.delta;
    // Screen motion, independent of zoom; total travel also catches a drag
    // that returns to its origin before the player releases the button.
    if gesture.distance >= 4.0 {
        gesture.dragged = true;
        if !over_ui {
            gesture.pan_delta = gesture.pending_delta;
        }
        gesture.pending_delta = Vec2::ZERO;
    }
    if mouse.just_released(MouseButton::Right) {
        gesture.right_click = !gesture.dragged && !over_ui;
        gesture.owned = false;
        gesture.pending_delta = Vec2::ZERO;
    } else if !mouse.pressed(MouseButton::Right) {
        *gesture = RightPointerGesture::default();
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

    fn gesture_frame(world: &mut World, motion: Vec2) {
        world.resource_mut::<AccumulatedMouseMotion>().delta = motion;
        world.run_system_once(update_right_pointer_gesture).unwrap();
        world
            .run_system_once(super::super::toolbar::rotate_station_with_right_click)
            .unwrap();
        world.run_system_once(move_camera).unwrap();
        world.resource_mut::<ButtonInput<MouseButton>>().clear();
    }

    fn gesture_world(scale: f32) -> (World, Entity) {
        use super::super::toolbar::*;
        let (mut world, camera) = camera_world(scale);
        world.init_resource::<RightPointerGesture>();
        world.init_resource::<StationBuildState>();
        world.init_resource::<DragBuildState>();
        world.insert_resource(UiToolState {
            active_tool: Some(BuildMenuAction::Station),
            ..default()
        });
        (world, camera)
    }

    #[test]
    fn right_drag_never_rotates_or_cancels_at_any_zoom() {
        use super::super::toolbar::{DragBuildState, StationBuildState};
        for scale in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0] {
            let (mut world, camera) = gesture_world(scale);
            world.resource_mut::<DragBuildState>().armed = true;
            let orientation = world.resource::<StationBuildState>().orientation;
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            gesture_frame(&mut world, Vec2::new(20.0, 0.0));
            assert!(world.get::<Transform>(camera).unwrap().translation.x < 0.0);
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Right);
            gesture_frame(&mut world, Vec2::ZERO);
            assert!(world.resource::<DragBuildState>().armed);
            assert_eq!(
                world.resource::<StationBuildState>().orientation,
                orientation
            );
        }
    }

    #[test]
    fn right_click_rotates_once_on_release_and_tolerates_small_jitter() {
        use super::super::toolbar::StationBuildState;
        for motion in [Vec2::ZERO, Vec2::new(2.0, 0.0)] {
            let (mut world, camera) = gesture_world(1.0);
            let orientation = world.resource::<StationBuildState>().orientation;
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            gesture_frame(&mut world, motion);
            assert_eq!(
                world.resource::<StationBuildState>().orientation,
                orientation
            );
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Right);
            gesture_frame(&mut world, Vec2::ZERO);
            assert_eq!(
                world.resource::<StationBuildState>().orientation,
                (orientation + 1) % 4
            );
            gesture_frame(&mut world, Vec2::ZERO);
            assert_eq!(
                world.resource::<StationBuildState>().orientation,
                (orientation + 1) % 4
            );
            assert_eq!(
                world.get::<Transform>(camera).unwrap().translation,
                Vec3::ZERO
            );
        }
    }

    #[test]
    fn right_gesture_cannot_start_in_ui_or_survive_focus_loss_or_modal() {
        use super::super::toolbar::StationBuildState;
        for interrupt in 0..3 {
            let (mut world, camera) = gesture_world(1.0);
            let orientation = world.resource::<StationBuildState>().orientation;
            let ui = world.spawn((Node::default(), Interaction::None)).id();
            if interrupt == 0 {
                *world.get_mut::<Interaction>(ui).unwrap() = Interaction::Hovered;
            }
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
            gesture_frame(&mut world, Vec2::ZERO);
            let window = world
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(&world)
                .unwrap();
            if interrupt == 1 {
                world.get_mut::<Window>(window).unwrap().focused = false;
            }
            if interrupt == 2 {
                world.insert_resource(SaveWindowState {
                    open: true,
                    ..default()
                });
            }
            gesture_frame(&mut world, Vec2::ZERO);
            *world.get_mut::<Interaction>(ui).unwrap() = Interaction::None;
            world.get_mut::<Window>(window).unwrap().focused = true;
            world.remove_resource::<SaveWindowState>();
            gesture_frame(&mut world, Vec2::new(20.0, 0.0));
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Right);
            gesture_frame(&mut world, Vec2::ZERO);
            assert_eq!(
                world.resource::<StationBuildState>().orientation,
                orientation
            );
            assert_eq!(
                world.get::<Transform>(camera).unwrap().translation,
                Vec3::ZERO
            );
        }
    }

    #[test]
    fn right_drag_returning_to_origin_is_not_a_click() {
        use super::super::toolbar::StationBuildState;
        let (mut world, _) = gesture_world(1.0);
        let orientation = world.resource::<StationBuildState>().orientation;
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        gesture_frame(&mut world, Vec2::new(3.0, 0.0));
        gesture_frame(&mut world, Vec2::new(-3.0, 0.0));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Right);
        gesture_frame(&mut world, Vec2::ZERO);
        assert_eq!(
            world.resource::<StationBuildState>().orientation,
            orientation
        );
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
