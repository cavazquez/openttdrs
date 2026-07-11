//! Infraestructura compartida de listas (UI-0 / UI-1 `ListWindow`).
//!
//! Chrome (sort chips, scroll, filtro de texto) + utilidades de ordenación.
//! Los directorios concretos aportan filas y acciones.

mod chrome;
mod model;
mod text_filter;

pub(crate) use chrome::{
    LIST_BTN_ACTIVE, LIST_BTN_BG, LIST_BTN_HOVER, LIST_DEFAULT_HEIGHT, clear_list_children,
    list_chip_bg, spawn_list_empty_label, spawn_list_filter_input, spawn_list_row_button,
    spawn_list_scroll_area, spawn_list_sort_button, sync_list_sort_colors,
};
pub(crate) use model::SortDir;
pub(crate) use text_filter::{apply_list_search_keyboard, text_filter_matches};
