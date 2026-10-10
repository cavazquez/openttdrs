//! Press-time actions for the classic UI, using Bevy's pointer events.
//!
//! Keep a small presentation state so existing `Changed` queries also see a
//! release (the native `Pressed` marker is removed rather than modified).

use bevy::picking::events::{PointerCancel, PointerDragEnd, PointerPress, PointerRelease};
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::picking::{Pickable, PickingSystems};
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, InteractionDisabled, Pressed};

#[derive(Component)]
#[require(Node, FocusPolicy::Block, UiInteraction)]
pub(crate) struct UiButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Hovered)]
pub(crate) enum UiInteraction {
    Pressed,
    Hovered,
    #[default]
    None,
}

pub(crate) struct UiInteractionPlugin;

impl Plugin for UiInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(press)
            .add_observer(release)
            .add_observer(cancel)
            .add_observer(drag_end)
            .add_systems(PreUpdate, sync_pickable.before(PickingSystems::Backend))
            .add_systems(PreUpdate, sync_interaction.after(PickingSystems::Last));
    }
}

/// UI picking ignores `FocusPolicy` and blocks lower nodes by default. Keep
/// decorative labels and transparent containers from intercepting controls.
#[allow(clippy::type_complexity)]
fn sync_pickable(
    nodes: Query<
        (
            Entity,
            Option<&FocusPolicy>,
            Has<UiInteraction>,
            Option<&Pickable>,
        ),
        (
            With<Node>,
            Or<(Added<Node>, Changed<FocusPolicy>, Added<UiInteraction>)>,
        ),
    >,
    mut commands: Commands,
) {
    for (entity, focus, interactive, current) in &nodes {
        let block = focus == Some(&FocusPolicy::Block);
        if current.is_none_or(|value| {
            value.should_block_lower != block || value.is_hoverable != interactive
        }) {
            commands.entity(entity).insert(Pickable {
                should_block_lower: block,
                is_hoverable: interactive,
            });
        }
    }
}

// Global observers receive the original target. Resolve an interactive parent
// when a pointer event originates at a label inside a control.
fn interaction_target(
    entity: Entity,
    parents: &Query<&ChildOf>,
    nodes: &Query<Has<InteractionDisabled>, With<UiInteraction>>,
) -> Option<(Entity, bool)> {
    std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .find_map(|target| nodes.get(target).ok().map(|disabled| (target, disabled)))
}

fn press(
    mut event: On<PointerPress>,
    nodes: Query<Has<InteractionDisabled>, With<UiInteraction>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if event.button == PointerButton::Primary
        && let Some((target, disabled)) = interaction_target(event.entity, &parents, &nodes)
    {
        event.propagate(false);
        if !disabled {
            commands.entity(target).insert(Pressed);
        }
    }
}

fn release(
    mut event: On<PointerRelease>,
    nodes: Query<Has<InteractionDisabled>, With<UiInteraction>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if event.button == PointerButton::Primary
        && let Some((target, _)) = interaction_target(event.entity, &parents, &nodes)
    {
        event.propagate(false);
        commands.entity(target).remove::<Pressed>();
    }
}

fn cancel(
    mut event: On<PointerCancel>,
    nodes: Query<Has<InteractionDisabled>, With<UiInteraction>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if let Some((target, _)) = interaction_target(event.entity, &parents, &nodes) {
        event.propagate(false);
        commands.entity(target).remove::<Pressed>();
    }
}

fn drag_end(
    mut event: On<PointerDragEnd>,
    nodes: Query<Has<InteractionDisabled>, With<UiInteraction>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    if event.button == PointerButton::Primary
        && let Some((target, _)) = interaction_target(event.entity, &parents, &nodes)
    {
        event.propagate(false);
        commands.entity(target).remove::<Pressed>();
    }
}

#[allow(clippy::type_complexity)]
fn sync_interaction(
    mut nodes: Query<(
        Entity,
        &mut UiInteraction,
        &Hovered,
        Has<Pressed>,
        Has<InteractionDisabled>,
        Option<&InheritedVisibility>,
        Option<&Visibility>,
        Option<&Node>,
    )>,
    mut commands: Commands,
) {
    for (entity, mut state, hovered, pressed, disabled, inherited, visibility, node) in &mut nodes {
        let hidden = disabled
            || inherited.is_some_and(|value| !value.get())
            || visibility == Some(&Visibility::Hidden)
            || node.is_some_and(|value| value.display == Display::None);
        let next = if hidden {
            if pressed {
                commands.entity(entity).remove::<Pressed>();
            }
            UiInteraction::None
        } else if pressed {
            UiInteraction::Pressed
        } else if hovered.get() {
            UiInteraction::Hovered
        } else {
            UiInteraction::None
        };
        state.set_if_neq(next);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::Pointer;
    use bevy::picking::pointer::{Location, PointerId};

    #[derive(Resource, Default)]
    struct Changes(Vec<UiInteraction>);

    fn setup() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, UiInteractionPlugin))
            .init_resource::<Changes>()
            .add_systems(
                Update,
                |states: Query<&UiInteraction, Changed<UiInteraction>>,
                 mut changes: ResMut<Changes>| {
                    changes.0.extend(states.iter().copied());
                },
            );
        let button = app
            .world_mut()
            .spawn((UiButton, InheritedVisibility::VISIBLE, Hovered(true)))
            .id();
        let label = app
            .world_mut()
            .spawn((Node::default(), ChildOf(button)))
            .id();
        app.update();
        app.world_mut().resource_mut::<Changes>().0.clear();
        (app, button, label)
    }

    fn pointer() -> Pointer {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 100,
                    height: 100,
                },
                position: Vec2::splat(10.0),
            },
        )
    }

    fn hit() -> HitData {
        HitData::new(Entity::PLACEHOLDER, 0.0, None, None)
    }

    fn down(app: &mut App, target: Entity, button: PointerButton) {
        app.world_mut().trigger(PointerPress {
            entity: target,
            pointer: pointer(),
            button,
            hit: hit(),
            count: 1,
        });
        app.update();
    }

    #[test]
    fn child_label_press_activates_once_and_release_updates_presentation() {
        let (mut app, button, label) = setup();
        down(&mut app, label, PointerButton::Primary);
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::Pressed
        );
        app.update();
        assert_eq!(
            app.world().resource::<Changes>().0,
            [UiInteraction::Pressed]
        );
        app.world_mut().trigger(PointerRelease {
            entity: label,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: hit(),
        });
        app.update();
        assert!(!app.world().entity(button).contains::<Pressed>());
        assert_eq!(
            app.world().resource::<Changes>().0,
            [UiInteraction::Pressed, UiInteraction::Hovered]
        );
    }

    #[test]
    fn secondary_press_does_not_activate_a_button() {
        let (mut app, button, label) = setup();
        down(&mut app, label, PointerButton::Secondary);
        assert!(!app.world().entity(button).contains::<Pressed>());
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::Hovered
        );
        assert!(app.world().resource::<Changes>().0.is_empty());
    }

    #[test]
    fn decorative_nodes_pass_through_and_focus_changes_update_picking() {
        let (mut app, button, label) = setup();
        let control = app.world().get::<Pickable>(button).unwrap();
        assert!(control.should_block_lower && control.is_hoverable);
        let decoration = app.world().get::<Pickable>(label).unwrap();
        assert!(!decoration.should_block_lower && !decoration.is_hoverable);
        app.world_mut().entity_mut(label).insert(FocusPolicy::Block);
        app.update();
        let decoration = app.world().get::<Pickable>(label).unwrap();
        assert!(decoration.should_block_lower && !decoration.is_hoverable);
        app.world_mut()
            .entity_mut(label)
            .insert(UiInteraction::default());
        app.update();
        assert!(app.world().get::<Pickable>(label).unwrap().is_hoverable);
    }

    #[test]
    fn drag_outside_the_widget_stays_pressed_until_drag_end() {
        let (mut app, button, label) = setup();
        down(&mut app, label, PointerButton::Primary);
        app.world_mut().entity_mut(button).insert(Hovered(false));
        app.update();
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::Pressed
        );
        app.world_mut().trigger(PointerDragEnd {
            entity: label,
            pointer: pointer(),
            button: PointerButton::Primary,
            distance: Vec2::splat(120.0),
        });
        app.update();
        assert!(!app.world().entity(button).contains::<Pressed>());
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::None
        );
    }

    #[test]
    fn cancel_and_hidden_nodes_clear_a_held_press() {
        let (mut app, button, label) = setup();
        down(&mut app, label, PointerButton::Primary);
        app.world_mut().trigger(PointerCancel {
            entity: label,
            pointer: pointer(),
            hit: hit(),
        });
        app.update();
        assert!(!app.world().entity(button).contains::<Pressed>());
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::Hovered
        );
        down(&mut app, label, PointerButton::Primary);
        app.world_mut()
            .entity_mut(button)
            .insert(Visibility::Hidden);
        app.update();
        assert!(!app.world().entity(button).contains::<Pressed>());
        assert_eq!(
            *app.world().get::<UiInteraction>(button).unwrap(),
            UiInteraction::None
        );
    }
}
