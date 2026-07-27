//! Scrollbar vertical común con chrome OpenGFX/OpenTTD.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::bevy_app::UpdateSet;
use crate::ui::toolbar::BuildMenuUi;

const BAR_WIDTH: f32 = 12.0;
const ARROW_SIZE: f32 = 8.0;
const SCROLL_STEP: f32 = 28.0;
const MIN_THUMB: f32 = 8.0;
const BUTTON_IDLE: Color = Color::srgb(0.45, 0.36, 0.26);
const BUTTON_HOVER: Color = Color::srgb(0.53, 0.43, 0.31);
const BUTTON_PRESSED: Color = Color::srgb(0.29, 0.23, 0.16);
const TRACK_BG: Color = Color::srgb(0.24, 0.19, 0.13);

#[derive(Component)]
struct ClassicScrollViewport;

#[derive(Component)]
struct ClassicScrollButton {
    viewport: Entity,
    direction: f32,
}

#[derive(Component)]
struct ClassicScrollTrack {
    viewport: Entity,
}

#[derive(Component)]
struct ClassicScrollThumb {
    viewport: Entity,
}

#[derive(Resource, Default)]
struct ScrollThumbDrag {
    thumb: Option<Entity>,
    viewport: Option<Entity>,
    start_cursor_y: f32,
    start_scroll_y: f32,
}

pub(crate) struct ClassicScrollbarPlugin;

impl Plugin for ClassicScrollbarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScrollThumbDrag>().add_systems(
            Update,
            (
                scroll_with_wheel,
                scroll_with_arrow_buttons,
                (begin_thumb_drag, drag_scroll_thumb).chain(),
                update_scroll_button_style,
                sync_scroll_thumbs,
            )
                .chain()
                .in_set(UpdateSet::Ui),
        );
    }
}

/// Crea viewport, contenido y scrollbar. Devuelve la entidad del viewport.
pub(crate) fn spawn_classic_scroll_area(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    list_root_marker: impl Bundle,
    height: f32,
    viewport_bg: Color,
    viewport_border: Color,
) -> Entity {
    spawn_classic_scroll_area_with(
        parent,
        asset_server,
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            overflow: Overflow::scroll_y(),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        viewport_bg,
        viewport_border,
        (),
        list_root_marker,
        |_| {},
        height,
    )
}

/// Variante para ventanas con filas o contenido propios.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_classic_scroll_area_with(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    viewport_node: Node,
    content_node: Node,
    viewport_bg: Color,
    viewport_border: Color,
    viewport_marker: impl Bundle,
    content_marker: impl Bundle,
    build_content: impl FnOnce(&mut ChildSpawnerCommands),
    height: f32,
) -> Entity {
    let mut viewport_entity = Entity::PLACEHOLDER;
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            viewport_entity = row
                .spawn((
                    ClassicScrollViewport,
                    viewport_marker,
                    Node { ..viewport_node },
                    ScrollPosition::default(),
                    Interaction::default(),
                    BackgroundColor(viewport_bg),
                    BorderColor::all(viewport_border),
                    BuildMenuUi,
                ))
                .with_children(|viewport| {
                    viewport
                        .spawn((content_marker, content_node, BuildMenuUi))
                        .with_children(build_content);
                })
                .id();

            row.spawn((
                Node {
                    width: Val::Px(BAR_WIDTH),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|bar| {
                spawn_arrow_button(bar, asset_server, viewport_entity, -1.0, "scroll_up.png");
                bar.spawn((
                    ClassicScrollTrack {
                        viewport: viewport_entity,
                    },
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    BackgroundColor(TRACK_BG),
                    BuildMenuUi,
                    children![(
                        ClassicScrollThumb {
                            viewport: viewport_entity,
                        },
                        Button,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(1.0),
                            right: Val::Px(1.0),
                            top: Val::Px(0.0),
                            height: Val::Px(MIN_THUMB),
                            ..default()
                        },
                        BackgroundColor(BUTTON_IDLE),
                        Interaction::default(),
                        BuildMenuUi,
                    )],
                ));
                spawn_arrow_button(bar, asset_server, viewport_entity, 1.0, "scroll_down.png");
            });
        });
    viewport_entity
}

fn spawn_arrow_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    viewport: Entity,
    direction: f32,
    icon: &'static str,
) {
    parent.spawn((
        ClassicScrollButton {
            viewport,
            direction,
        },
        Button,
        Node {
            width: Val::Px(BAR_WIDTH),
            height: Val::Px(BAR_WIDTH),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON_IDLE),
        Interaction::default(),
        BuildMenuUi,
        children![(
            ImageNode::new(asset_server.load::<Image>(format!("assets/opengfx/tiles/{icon}"))),
            Node {
                width: Val::Px(ARROW_SIZE),
                height: Val::Px(ARROW_SIZE),
                ..default()
            },
        )],
    ));
}

fn scroll_limit(viewport: &ComputedNode, content: Option<&ComputedNode>) -> f32 {
    content
        .map(|content| scroll_limit_from_sizes(viewport.size().y, content.size().y))
        .unwrap_or(0.0)
}

fn scroll_limit_from_sizes(viewport: f32, content: f32) -> f32 {
    (content - viewport).max(0.0)
}

fn scroll_with_arrow_buttons(
    buttons: Query<(&Interaction, &ClassicScrollButton), Changed<Interaction>>,
    mut viewports: Query<(&mut ScrollPosition, &ComputedNode, Option<&Children>)>,
    computed: Query<&ComputedNode>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok((mut position, viewport, children)) = viewports.get_mut(button.viewport) else {
            continue;
        };
        let content = children
            .and_then(|children| children.first())
            .and_then(|entity| computed.get(*entity).ok());
        position.y = (position.y + button.direction * SCROLL_STEP)
            .clamp(0.0, scroll_limit(viewport, content));
    }
}

fn scroll_with_wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut viewports: Query<
        (
            &Interaction,
            &mut ScrollPosition,
            &ComputedNode,
            Option<&Children>,
        ),
        With<ClassicScrollViewport>,
    >,
    computed: Query<&ComputedNode>,
) {
    let delta: f32 = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => -event.y * SCROLL_STEP,
            MouseScrollUnit::Pixel => -event.y,
        })
        .sum();
    if delta == 0.0 {
        return;
    }
    for (interaction, mut position, viewport, children) in &mut viewports {
        if *interaction != Interaction::Hovered {
            continue;
        }
        let content = children
            .and_then(|children| children.first())
            .and_then(|entity| computed.get(*entity).ok());
        position.y = (position.y + delta).clamp(0.0, scroll_limit(viewport, content));
    }
}

fn update_scroll_button_style(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            Or<(With<ClassicScrollButton>, With<ClassicScrollThumb>)>,
        ),
    >,
) {
    for (interaction, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::None => BUTTON_IDLE,
            Interaction::Hovered => BUTTON_HOVER,
            Interaction::Pressed => BUTTON_PRESSED,
        };
    }
}

fn begin_thumb_drag(
    thumbs: Query<(Entity, &Interaction, &ClassicScrollThumb), Changed<Interaction>>,
    primary: Query<&Window, With<bevy::window::PrimaryWindow>>,
    viewports: Query<&ScrollPosition>,
    mut drag: ResMut<ScrollThumbDrag>,
) {
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    for (entity, interaction, thumb) in &thumbs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(position) = viewports.get(thumb.viewport) else {
            continue;
        };
        drag.thumb = Some(entity);
        drag.viewport = Some(thumb.viewport);
        drag.start_cursor_y = cursor.y;
        drag.start_scroll_y = position.y;
    }
}

fn drag_scroll_thumb(
    mouse: Res<ButtonInput<MouseButton>>,
    primary: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut drag: ResMut<ScrollThumbDrag>,
    thumb_nodes: Query<(&ChildOf, &ComputedNode), With<ClassicScrollThumb>>,
    track_nodes: Query<&ComputedNode, With<ClassicScrollTrack>>,
    mut viewports: Query<(&mut ScrollPosition, &ComputedNode, Option<&Children>)>,
    computed: Query<&ComputedNode>,
) {
    let (Some(thumb_entity), Some(viewport_entity)) = (drag.thumb, drag.viewport) else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        drag.thumb = None;
        drag.viewport = None;
        return;
    }
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    let Ok((thumb_parent, thumb_node)) = thumb_nodes.get(thumb_entity) else {
        drag.thumb = None;
        drag.viewport = None;
        return;
    };
    let Ok(track_node) = track_nodes.get(thumb_parent.parent()) else {
        return;
    };
    let Ok((mut position, viewport, children)) = viewports.get_mut(viewport_entity) else {
        return;
    };
    let content = children
        .and_then(|children| children.first())
        .and_then(|entity| computed.get(*entity).ok());
    let scroll_limit = scroll_limit(viewport, content);
    let thumb_travel = (track_node.size().y - thumb_node.size().y).max(0.0);
    if scroll_limit <= 0.0 || thumb_travel <= 0.0 {
        position.y = 0.0;
        return;
    }
    let delta = cursor.y - drag.start_cursor_y;
    position.y =
        (drag.start_scroll_y + delta * scroll_limit / thumb_travel).clamp(0.0, scroll_limit);
}

fn sync_scroll_thumbs(
    tracks: Query<(&ClassicScrollTrack, &ComputedNode, &Children)>,
    viewports: Query<(&ScrollPosition, &ComputedNode, Option<&Children>)>,
    computed: Query<&ComputedNode>,
    mut thumbs: Query<&mut Node, With<ClassicScrollThumb>>,
) {
    for (track, track_node, track_children) in &tracks {
        let Ok((position, viewport, content_children)) = viewports.get(track.viewport) else {
            continue;
        };
        let content = content_children
            .and_then(|children| children.first())
            .and_then(|entity| computed.get(*entity).ok());
        let content_height = content.map_or(viewport.size().y, |node| node.size().y);
        let track_height = track_node.size().y;
        if track_height <= 0.0 {
            continue;
        }
        let thumb_height = if content_height <= 0.0 {
            track_height
        } else {
            (track_height * viewport.size().y / content_height)
                .max(MIN_THUMB)
                .min(track_height)
        };
        let limit = scroll_limit(viewport, content);
        let top = if limit > 0.0 {
            (track_height - thumb_height) * (position.y / limit).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(entity) = track_children.first()
            && let Ok(mut thumb) = thumbs.get_mut(*entity)
        {
            thumb.height = Val::Px(thumb_height);
            thumb.top = Val::Px(top);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::scroll_limit_from_sizes;

    #[test]
    fn scroll_limit_is_never_negative() {
        assert_eq!(scroll_limit_from_sizes(100.0, 80.0), 0.0);
        assert_eq!(scroll_limit_from_sizes(100.0, 280.0), 180.0);
    }
}
