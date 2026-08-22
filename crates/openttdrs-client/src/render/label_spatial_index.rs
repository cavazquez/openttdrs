//! Índice espacial de carteles del viewport.
//!
//! OpenTTD mantiene pueblos, carteles y estaciones en un KD-tree de signs del
//! viewport. Este índice por celdas conserva el mismo contrato útil para el
//! cliente: al panear no vuelve a recorrer todos los pools, consulta sólo las
//! celdas que tocan el recorte visible y entrega índices estables para componer
//! las tres capas en orden canónico.

use std::collections::BTreeMap;

use bevy::prelude::Resource;
use openttdrs_core::{GameState, TileCoord};

use crate::render::viewport::TileViewportBounds;

/// Tamaño de una celda del índice. Coincide con dos chunks de render de 16×16.
const LABEL_INDEX_CELL_TILES: u32 = 32;
/// Margen para incluir la caja de un label cuyo ancla cae apenas fuera del
/// viewport. Es equivalente a `ExpandRectWithViewportSignMargins` de OpenTTD.
const LABEL_QUERY_MARGIN_TILES: u32 = 8;

#[derive(Clone, Copy, Debug)]
struct LabelIndexEntry {
    index: usize,
    pos: TileCoord,
}

type Cells = BTreeMap<(u32, u32), Vec<LabelIndexEntry>>;

/// Índices en los pools de [`GameState`] que intersectan el viewport actual.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MapLabelCandidates {
    pub(crate) towns: Vec<usize>,
    pub(crate) signs: Vec<usize>,
    pub(crate) stations: Vec<usize>,
}

/// Índice por celdas de anclas de etiquetas del mapa.
#[derive(Resource, Debug, Default)]
pub(crate) struct MapLabelSpatialIndex {
    map_width: u32,
    map_height: u32,
    towns: Cells,
    signs: Cells,
    stations: Cells,
}

impl MapLabelSpatialIndex {
    #[must_use]
    pub(crate) fn from_state(state: &GameState) -> Self {
        let mut index = Self::default();
        index.rebuild(state);
        index
    }

    /// Reconstruye el índice tras añadir, mover, borrar o cargar una etiqueta.
    pub(crate) fn rebuild(&mut self, state: &GameState) {
        let (map_width, map_height) = state.map.dimensions();
        self.map_width = map_width;
        self.map_height = map_height;
        self.towns.clear();
        self.signs.clear();
        self.stations.clear();

        for (index, town) in state.towns.iter().enumerate() {
            Self::insert(&mut self.towns, town.pos, index, map_width, map_height);
        }
        for (index, sign) in state.signs.iter().enumerate() {
            Self::insert(&mut self.signs, sign.pos, index, map_width, map_height);
        }
        for (index, station) in state.stations.iter().enumerate() {
            Self::insert(
                &mut self.stations,
                station.pos,
                index,
                map_width,
                map_height,
            );
        }
    }

    #[must_use]
    pub(crate) fn candidates(&self, bounds: TileViewportBounds) -> MapLabelCandidates {
        MapLabelCandidates {
            towns: self.query(&self.towns, bounds),
            signs: self.query(&self.signs, bounds),
            stations: self.query(&self.stations, bounds),
        }
    }

    fn insert(cells: &mut Cells, pos: TileCoord, index: usize, map_width: u32, map_height: u32) {
        if pos.x < 0 || pos.y < 0 || (pos.x as u32) >= map_width || (pos.y as u32) >= map_height {
            return;
        }
        let cell = (
            (pos.x as u32) / LABEL_INDEX_CELL_TILES,
            (pos.y as u32) / LABEL_INDEX_CELL_TILES,
        );
        cells
            .entry(cell)
            .or_default()
            .push(LabelIndexEntry { index, pos });
    }

    fn query(&self, cells: &Cells, bounds: TileViewportBounds) -> Vec<usize> {
        if self.map_width == 0 || self.map_height == 0 {
            return Vec::new();
        }
        let expanded = bounds.expand(LABEL_QUERY_MARGIN_TILES, self.map_width, self.map_height);
        if expanded.tx0 >= expanded.tx1 || expanded.ty0 >= expanded.ty1 {
            return Vec::new();
        }
        let first_x = expanded.tx0 / LABEL_INDEX_CELL_TILES;
        let last_x = (expanded.tx1 - 1) / LABEL_INDEX_CELL_TILES;
        let first_y = expanded.ty0 / LABEL_INDEX_CELL_TILES;
        let last_y = (expanded.ty1 - 1) / LABEL_INDEX_CELL_TILES;

        let mut result = Vec::new();
        for cy in first_y..=last_y {
            for cx in first_x..=last_x {
                if let Some(entries) = cells.get(&(cx, cy)) {
                    // Una celda puede ser mayor que el rectángulo expandido.
                    // Filtrar el ancla aquí conserva la selección espacial
                    // exacta, sin volver a iterar el pool completo.
                    result.extend(entries.iter().filter_map(|entry| {
                        let x = u32::try_from(entry.pos.x).ok()?;
                        let y = u32::try_from(entry.pos.y).ok()?;
                        ((expanded.tx0..expanded.tx1).contains(&x)
                            && (expanded.ty0..expanded.ty1).contains(&y))
                        .then_some(entry.index)
                    }));
                }
            }
        }
        // Las celdas se recorren espacialmente; devolver el orden del pool
        // hace determinista el desempate interno de cada capa.
        result.sort_unstable();
        result
    }
}

#[cfg(test)]
mod tests {
    use openttdrs_core::prelude::*;

    use super::*;

    #[test]
    fn candidates_query_only_nearby_cells_with_label_margin() {
        let mut state = GameState::new(128, 128);
        state.towns.push(openttdrs_core::Town {
            id: 1,
            pos: TileCoord::new(31, 31),
            ..Default::default()
        });
        state.towns.push(openttdrs_core::Town {
            id: 2,
            pos: TileCoord::new(96, 96),
            ..Default::default()
        });
        state
            .signs
            .push(openttdrs_core::Sign::new(4, TileCoord::new(44, 44), "S"));
        state.stations.push(Station::new_with_kind(
            TileCoord::new(45, 45),
            StopKind::BusStop,
        ));

        let index = MapLabelSpatialIndex::from_state(&state);
        let candidates = index.candidates(TileViewportBounds {
            tx0: 32,
            ty0: 32,
            tx1: 36,
            ty1: 36,
        });

        // El pueblo en 31,31 entra por el margen de la caja de texto; la
        // estación y el cartel en 44/45 todavía quedan fuera del margen 8.
        assert_eq!(candidates.towns, vec![0]);
        assert!(candidates.signs.is_empty());
        assert!(candidates.stations.is_empty());
    }

    #[test]
    fn candidates_keep_pool_order_inside_multiple_cells() {
        let mut state = GameState::new(128, 128);
        state
            .signs
            .push(openttdrs_core::Sign::new(3, TileCoord::new(70, 2), "B"));
        state
            .signs
            .push(openttdrs_core::Sign::new(1, TileCoord::new(2, 2), "A"));
        let index = MapLabelSpatialIndex::from_state(&state);
        let candidates = index.candidates(TileViewportBounds::full(128, 128));
        assert_eq!(candidates.signs, vec![0, 1]);
    }
}
