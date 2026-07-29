//! Directorio global de industrias: cadenas I/O y fundación desde el menú.

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::text::EditableText;
use openttdrs_core::prelude::*;
use openttdrs_core::{Climate, Industry, IndustryKind, IndustrySpec, cargo_display_name};

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::industry_panel::{IndustryPanelState, kind_label, spec_label};
use crate::ui::list_window::{
    LIST_DEFAULT_HEIGHT, SortDir, apply_list_search_keyboard, clear_list_children,
    spawn_list_empty_label, spawn_list_filter_input, spawn_list_row_button, spawn_list_scroll_area,
    spawn_list_sort_button, sync_list_sort_colors, text_filter_matches,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, DragBuildState, ToolbarGroup, ToolbarState, UiToolState,
    economy_industry_tool_visible,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum IndustryDirectorySort {
    #[default]
    Type,
    Stock,
}

#[derive(Resource, Default)]
pub(crate) struct IndustryDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: IndustryDirectorySort,
    pub(crate) sort_dir: SortDir,
    pub(crate) filter_text: String,
}

#[derive(Component)]
pub(crate) struct IndustryDirectoryListRoot;

#[derive(Component)]
pub(crate) struct IndustryDirectoryFundRoot;

#[derive(Component)]
pub(crate) struct IndustryDirectorySearchInput;

#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDirectoryRow {
    pos: TileCoord,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IndustryDirectorySortButton(IndustryDirectorySort);

#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDirectoryFundButton(BuildMenuAction);

#[derive(Default)]
pub(crate) struct IndustryDirectoryCache {
    sort: IndustryDirectorySort,
    sort_dir: SortDir,
    filter: String,
    climate: Option<Climate>,
    rows: Vec<(
        TileCoord,
        IndustryKind,
        Option<IndustrySpec>,
        u32,
        u32,
        String,
    )>,
}

pub(crate) fn setup_industry_directory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::IndustryDirectory,
        "Directorio de industrias",
        TITLE_BROWN,
        Vec2::new(520.0, 90.0),
        480.0,
    );
    commands.entity(content).with_children(|body| {
        spawn_list_filter_input(
            body,
            asset_server,
            IndustryDirectorySearchInput,
            "buscar industria…",
        );
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_list_sort_button(
                row,
                asset_server,
                "Tipo",
                IndustryDirectorySortButton(IndustryDirectorySort::Type),
                90.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Stock",
                IndustryDirectorySortButton(IndustryDirectorySort::Stock),
                90.0,
            );
        });
        body.spawn((
            Text::new("Fundar (clic en el mapa):"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
        ));
        body.spawn((
            IndustryDirectoryFundRoot,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        spawn_list_scroll_area(
            body,
            asset_server,
            IndustryDirectoryListRoot,
            LIST_DEFAULT_HEIGHT - 50.0,
        );
    });
}

fn fund_actions_for_climate(climate: Climate) -> Vec<(BuildMenuAction, &'static str)> {
    [
        (BuildMenuAction::BuildCoalMine, "Carbón"),
        (BuildMenuAction::BuildIronOreMine, "Hierro"),
        (BuildMenuAction::BuildGoldMine, "Oro"),
        (BuildMenuAction::BuildOilWell, "Pozos"),
        (BuildMenuAction::BuildOilRefinery, "Refinería"),
        (BuildMenuAction::BuildForest, "Bosque"),
        (BuildMenuAction::BuildFarm, "Granja"),
        (BuildMenuAction::BuildSawmill, "Aserradero"),
        (BuildMenuAction::BuildFactory, "Fábrica"),
        (BuildMenuAction::BuildCottonCandy, "Algodón"),
        (BuildMenuAction::BuildCandyFactory, "Caramelos"),
        (BuildMenuAction::BuildBatteryFarm, "Baterías"),
        (BuildMenuAction::BuildColaWells, "Cola"),
        (BuildMenuAction::BuildToyFactory, "Juguetes"),
        (BuildMenuAction::BuildPlasticFountain, "Plástico"),
        (BuildMenuAction::BuildFizzyDrinkFactory, "Gaseosa"),
        (BuildMenuAction::BuildBubbleGenerator, "Burbujas"),
        (BuildMenuAction::BuildToffeeQuarry, "Toffee"),
        (BuildMenuAction::BuildSugarMine, "Azúcar"),
    ]
    .into_iter()
    .filter(|(action, _)| economy_industry_tool_visible(*action, climate))
    .collect()
}

/// Cadena input → output visible en la lista (MVP de cadenas).
pub(crate) fn industry_chain_label(industry: &Industry) -> String {
    let output = cargo_display_name(industry.output_cargo());
    let inputs = industry.station_input_requirements();
    if inputs.is_empty() {
        format!("→ {output}")
    } else {
        let joined = inputs
            .iter()
            .map(|(cargo, _)| cargo_display_name(*cargo))
            .collect::<Vec<_>>()
            .join("+");
        format!("{joined} → {output}")
    }
}

pub(crate) fn open_industry_directory_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<IndustryDirectoryState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Industries {
            state.open = true;
        }
    }
}

pub(crate) fn industry_directory_search_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut state: ResMut<IndustryDirectoryState>,
    mut inputs: Query<(&mut EditableText, &mut Text), With<IndustryDirectorySearchInput>>,
) {
    if !state.open {
        key_events.clear();
        return;
    }
    let Ok((mut editable, mut text)) = inputs.single_mut() else {
        key_events.clear();
        return;
    };
    apply_list_search_keyboard(
        &mut key_events,
        &mut editable,
        &mut text,
        &mut state.filter_text,
        32,
        "buscar industria…",
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_industry_directory_buttons(
    mut state: ResMut<IndustryDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &IndustryDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    fund_buttons: Query<
        (&Interaction, &IndustryDirectoryFundButton),
        (Changed<Interaction>, With<Button>),
    >,
    rows: Query<(&Interaction, &IndustryDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut panel: ResMut<IndustryPanelState>,
    mut toolbar: ResMut<ToolbarState>,
    mut tool_state: ResMut<UiToolState>,
    mut drag_state: ResMut<DragBuildState>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    for (interaction, button) in &sort_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if state.sort == button.0 {
            state.sort_dir = state.sort_dir.toggle();
        } else {
            state.sort = button.0;
            state.sort_dir = match button.0 {
                IndustryDirectorySort::Type => SortDir::Asc,
                IndustryDirectorySort::Stock => SortDir::Desc,
            };
        }
    }
    for (interaction, button) in &fund_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        toolbar.active_group = Some(ToolbarGroup::Economy);
        tool_state.active_tool = Some(button.0);
        cancel_placement(&mut drag_state);
        state.open = false;
    }
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        panel.open = true;
        panel.focus_tile = Some(row.pos);
        let height = sim.state.map.get(row.pos).map_or(0, |tile| tile.height);
        let center = tile_pos(row.pos.x, row.pos.y, height, 0.0);
        if let Ok(mut transform) = cam_q.single_mut() {
            transform.translation.x = center.x;
            transform.translation.y = center.y;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn sync_industry_directory(
    state: Res<IndustryDirectoryState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<IndustryDirectoryListRoot>>,
    fund_roots: Query<Entity, With<IndustryDirectoryFundRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<IndustryDirectoryCache>,
    mut sort_buttons: Query<
        (
            &IndustryDirectorySortButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        With<Button>,
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::IndustryDirectory)
    else {
        return;
    };
    if !state.open {
        *visibility = Visibility::Hidden;
        cache.rows.clear();
        return;
    }
    *visibility = Visibility::Visible;

    sync_list_sort_colors(&mut sort_buttons, IndustryDirectorySortButton(state.sort));

    let climate = sim.state.climate;
    if cache.climate != Some(climate)
        && let Ok(fund_root) = fund_roots.single()
    {
        clear_list_children(&mut commands, fund_root, &children_q);
        let actions = fund_actions_for_climate(climate);
        commands.entity(fund_root).with_children(|row| {
            for (action, label) in actions {
                row.spawn((
                    Button,
                    IndustryDirectoryFundButton(action),
                    Node {
                        min_width: Val::Px(78.0),
                        height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.32, 0.38, 0.28)),
                    BorderColor::all(Color::srgb(0.50, 0.58, 0.40)),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new(label),
                        window_text_font(&asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ));
            }
        });
        cache.climate = Some(climate);
    }

    let mut rows: Vec<_> = sim
        .state
        .industries
        .iter()
        .filter(|industry| {
            let name = industry
                .spec
                .map_or_else(|| kind_label(industry.kind), spec_label);
            let chain = industry_chain_label(industry);
            text_filter_matches(&state.filter_text, name)
                || text_filter_matches(&state.filter_text, &chain)
        })
        .map(|industry| {
            (
                industry.pos,
                industry.kind,
                industry.spec,
                industry.stock,
                industry.capacity,
                industry_chain_label(industry),
            )
        })
        .collect();
    match state.sort {
        IndustryDirectorySort::Type => rows.sort_by(|a, b| {
            let la = a.2.map_or_else(|| kind_label(a.1), spec_label);
            let lb = b.2.map_or_else(|| kind_label(b.1), spec_label);
            state.sort_dir.apply(
                la.cmp(lb)
                    .then_with(|| a.0.x.cmp(&b.0.x))
                    .then_with(|| a.0.y.cmp(&b.0.y)),
            )
        }),
        IndustryDirectorySort::Stock => {
            rows.sort_by(|a, b| {
                state.sort_dir.apply(a.3.cmp(&b.3).then_with(|| {
                    let la = a.2.map_or_else(|| kind_label(a.1), spec_label);
                    let lb = b.2.map_or_else(|| kind_label(b.1), spec_label);
                    la.cmp(lb)
                }))
            });
        }
    }
    if cache.sort == state.sort
        && cache.sort_dir == state.sort_dir
        && cache.filter == state.filter_text
        && cache.rows == rows
    {
        return;
    }
    cache.sort = state.sort;
    cache.sort_dir = state.sort_dir;
    cache.filter.clone_from(&state.filter_text);
    cache.rows.clone_from(&rows);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            spawn_list_empty_label(
                list,
                &asset_server,
                if state.filter_text.trim().is_empty() {
                    "No hay industrias."
                } else {
                    "Ninguna industria coincide con el filtro."
                },
            );
            return;
        }
        for (pos, kind, spec, stock, capacity, chain) in rows {
            let name = spec.map_or_else(|| kind_label(kind), spec_label);
            spawn_list_row_button(
                list,
                &asset_server,
                format!(
                    "{name}  ·  {chain}  ·  stock {stock}/{capacity}  ·  ({}, {})",
                    pos.x, pos.y
                ),
                IndustryDirectoryRow { pos },
                false,
            );
        }
    });
}

pub(crate) fn industry_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<IndustryDirectoryState>,
    mut search_q: Query<(&mut EditableText, &mut Text), With<IndustryDirectorySearchInput>>,
) {
    for message in closed.read() {
        if message.0.class == FloatingWindowId::IndustryDirectory {
            state.open = false;
            state.filter_text.clear();
            if let Ok((mut editable, mut text)) = search_q.single_mut() {
                *editable = EditableText::new("");
                **text = "buscar industria…".into();
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::CargoType;

    #[test]
    fn route_opens_industry_directory() {
        let mut world = World::new();
        world.init_resource::<IndustryDirectoryState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Industries));
        world
            .run_system_once(open_industry_directory_from_routes)
            .unwrap();
        assert!(world.resource::<IndustryDirectoryState>().open);
    }

    #[test]
    fn row_opens_industry_panel() {
        let mut world = World::new();
        world.init_resource::<IndustryDirectoryState>();
        world.init_resource::<IndustryPanelState>();
        world.init_resource::<ToolbarState>();
        world.init_resource::<UiToolState>();
        world.insert_resource(DragBuildState::default());
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            ..SimWorld::default()
        });
        let pos = TileCoord::new(3, 4);
        world.spawn((Button, IndustryDirectoryRow { pos }, Interaction::Pressed));
        world
            .run_system_once(handle_industry_directory_buttons)
            .unwrap();
        let panel = world.resource::<IndustryPanelState>();
        assert!(panel.open);
        assert_eq!(panel.focus_tile, Some(pos));
    }

    #[test]
    fn fund_button_arms_economy_tool_and_closes() {
        let mut world = World::new();
        world.insert_resource(IndustryDirectoryState {
            open: true,
            ..default()
        });
        world.init_resource::<IndustryPanelState>();
        world.init_resource::<ToolbarState>();
        world.init_resource::<UiToolState>();
        world.insert_resource(DragBuildState::default());
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            ..SimWorld::default()
        });
        world.spawn((
            Button,
            IndustryDirectoryFundButton(BuildMenuAction::BuildFactory),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_industry_directory_buttons)
            .unwrap();
        assert!(!world.resource::<IndustryDirectoryState>().open);
        assert!(matches!(
            world.resource::<ToolbarState>().active_group,
            Some(ToolbarGroup::Economy)
        ));
        assert_eq!(
            world.resource::<UiToolState>().active_tool,
            Some(BuildMenuAction::BuildFactory)
        );
    }

    #[test]
    fn chain_label_shows_inputs_for_factory() {
        let mut industry = Industry::new(TileCoord::new(0, 0), IndustryKind::Factory);
        industry.spec = Some(IndustrySpec::Factory);
        let label = industry_chain_label(&industry);
        assert!(label.contains(cargo_display_name(CargoType::Wood)));
        assert!(label.contains(cargo_display_name(CargoType::Coal)));
        assert!(label.contains(cargo_display_name(CargoType::Goods)));
    }

    #[test]
    fn chain_label_producer_only_shows_output() {
        let industry = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        assert_eq!(
            industry_chain_label(&industry),
            format!("→ {}", cargo_display_name(CargoType::Coal))
        );
    }

    #[test]
    fn fund_actions_respect_climate() {
        let temperate = fund_actions_for_climate(Climate::Temperate);
        assert!(
            temperate
                .iter()
                .any(|(a, _)| *a == BuildMenuAction::BuildCoalMine)
        );
        assert!(
            !temperate
                .iter()
                .any(|(a, _)| *a == BuildMenuAction::BuildToyFactory)
        );
        let toyland = fund_actions_for_climate(Climate::Toyland);
        assert!(
            toyland
                .iter()
                .any(|(a, _)| *a == BuildMenuAction::BuildToyFactory)
        );
        let _ = GameState::new(4, 4);
    }
}
