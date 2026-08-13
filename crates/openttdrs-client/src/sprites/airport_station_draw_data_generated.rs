// Generado por scripts/gen_airport_station_draw_data.py — NO EDITAR A MANO.
// Fuente: OpenTTD table/station_land.h (APT_PIER_NW_NE/APT_PIER) + NFO OpenGFX.
// Modo gráfico detectado: 8bpp.
#![cfg_attr(rustfmt, rustfmt_skip)]

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirportStationLayer {
    pub sprite_id: u32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub sx: i32,
    pub sy: i32,
    pub sz: i32,
    pub z: f32,
    pub w: f32,
    pub h: f32,
    pub x_offs: f32,
    pub y_offs: f32,
    pub path: &'static str,
}

/// APT_PIER_NW_NE (StationGfx 27); capa `TILE_SEQ_LINE` coloreada por compañía.
pub static AIRPORT_PIER_NW_NE_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2661, dx: 3.0, dy: 2.0, dz: 0.0, sx: 3, sy: 3, sz: 14, z: 0.050, w: 28.0, h: 20.0, x_offs: -12.0, y_offs: -14.0, path: "assets/opengfx/tiles/airport_jetway_3.png" },
];

/// APT_PIER (StationGfx 28); capa `TILE_SEQ_LINE` coloreada por compañía.
pub static AIRPORT_PIER_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2662, dx: 0.0, dy: 8.0, dz: 0.0, sx: 14, sy: 3, sz: 14, z: 0.050, w: 36.0, h: 24.0, x_offs: -29.0, y_offs: -10.0, path: "assets/opengfx/tiles/airport_passenger_tunnel.png" },
];

/// Capas ordenables de los `StationGfx` airport vanilla cubiertos.
#[must_use]
pub const fn airport_station_layers_for_gfx(gfx: u8) -> &'static [AirportStationLayer] {
    match gfx {
        27 => &AIRPORT_PIER_NW_NE_LAYERS,
        28 => &AIRPORT_PIER_LAYERS,
        _ => &[],
    }
}
