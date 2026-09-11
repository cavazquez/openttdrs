//! Geometría compartida de las capas BUILD del depósito naval.
//!
//! Los datos son la tabla `DrawShipDepotSprite`/`TILE_SEQ_LINE` del renderer
//! nativo. El mapa y el preview consumen la misma tabla para no divergir en
//! orientación, anclaje o tamaño de la caja de ordenación.

/// Una capa OpenGFX de depósito naval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ShipDepotLayerGfx {
    /// Índice dentro de `SHIP_DEPOT_PATHS` y sprite nativo `4070 + index`.
    pub(crate) sprite_index: usize,
    /// Desplazamiento local `TILE_SEQ` antes del remap isométrico.
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    /// Anclaje y tamaño de la imagen declarados por `TILE_SEQ_LINE`.
    pub(crate) xrel: f32,
    pub(crate) yrel: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

/// Assets en el orden de los sprites nativos 4070..4075.
pub(crate) const SHIP_DEPOT_PATHS: [&str; 6] = [
    "assets/opengfx/tiles/ship_depot_se_front.png",
    "assets/opengfx/tiles/ship_depot_sw_front.png",
    "assets/opengfx/tiles/ship_depot_nw.png",
    "assets/opengfx/tiles/ship_depot_ne.png",
    "assets/opengfx/tiles/ship_depot_se_rear.png",
    "assets/opengfx/tiles/ship_depot_sw_rear.png",
];

const SHIP_DEPOT_X_NORTH: [ShipDepotLayerGfx; 1] = [ShipDepotLayerGfx {
    sprite_index: 2,
    dx: 0.0,
    dy: 15.0,
    xrel: -29.0,
    yrel: -37.0,
    width: 32.0,
    height: 53.0,
}];

const SHIP_DEPOT_X_SOUTH: [ShipDepotLayerGfx; 2] = [
    ShipDepotLayerGfx {
        sprite_index: 4,
        dx: 0.0,
        dy: 0.0,
        xrel: -31.0,
        yrel: 2.0,
        width: 14.0,
        height: 13.0,
    },
    ShipDepotLayerGfx {
        sprite_index: 0,
        dx: 0.0,
        dy: 15.0,
        xrel: -61.0,
        yrel: -48.0,
        width: 64.0,
        height: 64.0,
    },
];

const SHIP_DEPOT_Y_NORTH: [ShipDepotLayerGfx; 1] = [ShipDepotLayerGfx {
    sprite_index: 3,
    dx: 15.0,
    dy: 0.0,
    xrel: -1.0,
    yrel: -36.0,
    width: 32.0,
    height: 53.0,
}];

const SHIP_DEPOT_Y_SOUTH: [ShipDepotLayerGfx; 2] = [
    ShipDepotLayerGfx {
        sprite_index: 5,
        dx: 0.0,
        dy: 0.0,
        xrel: 19.0,
        yrel: 3.0,
        width: 14.0,
        height: 13.0,
    },
    ShipDepotLayerGfx {
        sprite_index: 1,
        dx: 15.0,
        dy: 0.0,
        xrel: -1.0,
        yrel: -47.0,
        width: 64.0,
        height: 64.0,
    },
];

/// Capas que emite una sección según eje y parte norte/sur de `m5`.
#[must_use]
pub(crate) const fn ship_depot_layers(
    axis_y: bool,
    part_south: bool,
) -> &'static [ShipDepotLayerGfx] {
    match (axis_y, part_south) {
        (false, false) => &SHIP_DEPOT_X_NORTH,
        (false, true) => &SHIP_DEPOT_X_SOUTH,
        (true, false) => &SHIP_DEPOT_Y_NORTH,
        (true, true) => &SHIP_DEPOT_Y_SOUTH,
    }
}

/// Extensión `TILE_SEQ_LINE` usada por el sorter para cada eje.
#[must_use]
pub(crate) const fn ship_depot_seq_extent(axis_y: bool) -> (i32, i32) {
    if axis_y { (1, 16) } else { (16, 1) }
}

#[cfg(test)]
mod tests {
    use super::{ship_depot_layers, ship_depot_seq_extent};

    #[test]
    fn layer_matrix_matches_native_sprite_sequence() {
        assert_eq!(
            ship_depot_layers(false, false)
                .iter()
                .map(|layer| layer.sprite_index)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(
            ship_depot_layers(false, true)
                .iter()
                .map(|layer| layer.sprite_index)
                .collect::<Vec<_>>(),
            vec![4, 0]
        );
        assert_eq!(
            ship_depot_layers(true, false)
                .iter()
                .map(|layer| layer.sprite_index)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(
            ship_depot_layers(true, true)
                .iter()
                .map(|layer| layer.sprite_index)
                .collect::<Vec<_>>(),
            vec![5, 1]
        );
    }

    #[test]
    fn sequence_extent_matches_native_axis() {
        assert_eq!(ship_depot_seq_extent(false), (16, 1));
        assert_eq!(ship_depot_seq_extent(true), (1, 16));
    }
}
