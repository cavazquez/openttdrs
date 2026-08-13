// Generado por scripts/gen_airport_station_draw_data.py — NO EDITAR A MANO.
// Fuente: OpenTTD table/station_land.h (los 74 StationGfx vanilla) + NFO OpenGFX.
// Los IDs Action5 se resuelven desde el GRF extra del mismo perfil gráfico.
// Modo gráfico detectado: 8bpp.
#![cfg_attr(rustfmt, rustfmt_skip)]

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirportStationSprite {
    pub sprite_id: u32,
    pub w: f32,
    pub h: f32,
    pub x_offs: f32,
    pub y_offs: f32,
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportStationBase {
    pub sprite_id: u32,
    pub company_coloured: bool,
}

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
    pub company_coloured: bool,
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AirportStationGroundLayer {
    pub sprite_id: u32,
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub w: f32,
    pub h: f32,
    pub x_offs: f32,
    pub y_offs: f32,
    pub company_coloured: bool,
    pub path: &'static str,
}

pub static AIRPORT_STATION_SPRITES: [AirportStationSprite; 66] = [
    AirportStationSprite { sprite_id: 2095, w: 36.0, h: 26.0, x_offs: -14.0, y_offs: -10.0, path: "assets/opengfx/tiles/airport_helidepot_office.png" },
    AirportStationSprite { sprite_id: 2601, w: 55.0, h: 77.0, x_offs: -26.0, y_offs: -71.0, path: "assets/opengfx/tiles/airport_transmitter.png" },
    AirportStationSprite { sprite_id: 2633, w: 64.0, h: 87.0, x_offs: -31.0, y_offs: -56.0, path: "assets/opengfx/tiles/airport_heliport.png" },
    AirportStationSprite { sprite_id: 2634, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_apron.png" },
    AirportStationSprite { sprite_id: 2635, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_stand.png" },
    AirportStationSprite { sprite_id: 2636, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_0.png" },
    AirportStationSprite { sprite_id: 2637, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_1.png" },
    AirportStationSprite { sprite_id: 2638, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_2.png" },
    AirportStationSprite { sprite_id: 2639, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_3.png" },
    AirportStationSprite { sprite_id: 2640, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_4.png" },
    AirportStationSprite { sprite_id: 2641, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_5.png" },
    AirportStationSprite { sprite_id: 2642, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_6.png" },
    AirportStationSprite { sprite_id: 2643, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_7.png" },
    AirportStationSprite { sprite_id: 2644, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_taxiway_8.png" },
    AirportStationSprite { sprite_id: 2645, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_runway_0.png" },
    AirportStationSprite { sprite_id: 2646, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_runway_1.png" },
    AirportStationSprite { sprite_id: 2647, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_runway_2.png" },
    AirportStationSprite { sprite_id: 2648, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_runway_3.png" },
    AirportStationSprite { sprite_id: 2649, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_runway_4.png" },
    AirportStationSprite { sprite_id: 2650, w: 57.0, h: 64.0, x_offs: -22.0, y_offs: -35.0, path: "assets/opengfx/tiles/airport_terminal_a.png" },
    AirportStationSprite { sprite_id: 2651, w: 42.0, h: 79.0, x_offs: -19.0, y_offs: -60.0, path: "assets/opengfx/tiles/airport_tower.png" },
    AirportStationSprite { sprite_id: 2652, w: 38.0, h: 41.0, x_offs: -19.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_concourse.png" },
    AirportStationSprite { sprite_id: 2653, w: 38.0, h: 44.0, x_offs: -17.0, y_offs: -26.0, path: "assets/opengfx/tiles/airport_terminal_b.png" },
    AirportStationSprite { sprite_id: 2654, w: 54.0, h: 48.0, x_offs: -36.0, y_offs: -21.0, path: "assets/opengfx/tiles/airport_terminal_c.png" },
    AirportStationSprite { sprite_id: 2655, w: 64.0, h: 55.0, x_offs: -4.0, y_offs: -38.0, path: "assets/opengfx/tiles/airport_hangar_front.png" },
    AirportStationSprite { sprite_id: 2656, w: 17.0, h: 16.0, x_offs: 11.0, y_offs: -2.0, path: "assets/opengfx/tiles/airport_hangar_rear.png" },
    AirportStationSprite { sprite_id: 2657, w: 63.0, h: 50.0, x_offs: -2.0, y_offs: -33.0, path: "assets/opengfx/tiles/airport_airfield_hangar_front.png" },
    AirportStationSprite { sprite_id: 2658, w: 17.0, h: 17.0, x_offs: 16.0, y_offs: -1.0, path: "assets/opengfx/tiles/airport_airfield_hangar_rear.png" },
    AirportStationSprite { sprite_id: 2659, w: 17.0, h: 17.0, x_offs: -8.0, y_offs: -9.0, path: "assets/opengfx/tiles/airport_jetway_1.png" },
    AirportStationSprite { sprite_id: 2660, w: 27.0, h: 23.0, x_offs: -9.0, y_offs: -16.0, path: "assets/opengfx/tiles/airport_jetway_2.png" },
    AirportStationSprite { sprite_id: 2661, w: 28.0, h: 20.0, x_offs: -12.0, y_offs: -14.0, path: "assets/opengfx/tiles/airport_jetway_3.png" },
    AirportStationSprite { sprite_id: 2662, w: 36.0, h: 24.0, x_offs: -29.0, y_offs: -10.0, path: "assets/opengfx/tiles/airport_passenger_tunnel.png" },
    AirportStationSprite { sprite_id: 2663, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationSprite { sprite_id: 2664, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, path: "assets/opengfx/tiles/airport_fence_x.png" },
    AirportStationSprite { sprite_id: 2665, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_terminal_a.png" },
    AirportStationSprite { sprite_id: 2666, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_terminal_b.png" },
    AirportStationSprite { sprite_id: 2667, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_terminal_c_ground.png" },
    AirportStationSprite { sprite_id: 2668, w: 50.0, h: 26.0, x_offs: -21.0, y_offs: -13.0, path: "assets/opengfx/tiles/airport_airfield_terminal_c_build.png" },
    AirportStationSprite { sprite_id: 2669, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_apron_a.png" },
    AirportStationSprite { sprite_id: 2670, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_apron_b.png" },
    AirportStationSprite { sprite_id: 2671, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_apron_c.png" },
    AirportStationSprite { sprite_id: 2672, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_apron_d.png" },
    AirportStationSprite { sprite_id: 2673, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_runway_near.png" },
    AirportStationSprite { sprite_id: 2674, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_runway_middle.png" },
    AirportStationSprite { sprite_id: 2675, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_airfield_runway_far.png" },
    AirportStationSprite { sprite_id: 2676, w: 10.0, h: 21.0, x_offs: 0.0, y_offs: -24.0, path: "assets/opengfx/tiles/airport_wind_0.png" },
    AirportStationSprite { sprite_id: 2677, w: 10.0, h: 21.0, x_offs: 0.0, y_offs: -24.0, path: "assets/opengfx/tiles/airport_wind_1.png" },
    AirportStationSprite { sprite_id: 2678, w: 10.0, h: 21.0, x_offs: 0.0, y_offs: -24.0, path: "assets/opengfx/tiles/airport_wind_2.png" },
    AirportStationSprite { sprite_id: 2679, w: 9.0, h: 21.0, x_offs: 1.0, y_offs: -24.0, path: "assets/opengfx/tiles/airport_wind_3.png" },
    AirportStationSprite { sprite_id: 2680, w: 5.0, h: 24.0, x_offs: -2.0, y_offs: -21.0, path: "assets/opengfx/tiles/airport_radar_00.png" },
    AirportStationSprite { sprite_id: 2681, w: 10.0, h: 22.0, x_offs: -5.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_01.png" },
    AirportStationSprite { sprite_id: 2682, w: 20.0, h: 22.0, x_offs: -10.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_02.png" },
    AirportStationSprite { sprite_id: 2683, w: 26.0, h: 17.0, x_offs: -12.0, y_offs: -14.0, path: "assets/opengfx/tiles/airport_radar_03.png" },
    AirportStationSprite { sprite_id: 2684, w: 20.0, h: 22.0, x_offs: -8.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_04.png" },
    AirportStationSprite { sprite_id: 2685, w: 10.0, h: 22.0, x_offs: -3.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_05.png" },
    AirportStationSprite { sprite_id: 2686, w: 5.0, h: 24.0, x_offs: 0.0, y_offs: -21.0, path: "assets/opengfx/tiles/airport_radar_06.png" },
    AirportStationSprite { sprite_id: 2687, w: 10.0, h: 22.0, x_offs: -4.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_07.png" },
    AirportStationSprite { sprite_id: 2688, w: 20.0, h: 22.0, x_offs: -9.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_08.png" },
    AirportStationSprite { sprite_id: 2689, w: 26.0, h: 17.0, x_offs: -12.0, y_offs: -14.0, path: "assets/opengfx/tiles/airport_radar_09.png" },
    AirportStationSprite { sprite_id: 2690, w: 19.0, h: 22.0, x_offs: -8.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_10.png" },
    AirportStationSprite { sprite_id: 2691, w: 10.0, h: 22.0, x_offs: -4.0, y_offs: -19.0, path: "assets/opengfx/tiles/airport_radar_11.png" },
    AirportStationSprite { sprite_id: 3981, w: 64.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/grass.png" },
    AirportStationSprite { sprite_id: 4982, w: 39.0, h: 18.0, x_offs: -9.0, y_offs: -9.0, path: "assets/opengfx/tiles/airport_helipad.png" },
    AirportStationSprite { sprite_id: 5966, w: 24.0, h: 14.0, x_offs: -11.0, y_offs: 8.0, path: "assets/opengfx/tiles/airport_new_helipad.png" },
    AirportStationSprite { sprite_id: 5967, w: 34.0, h: 31.0, x_offs: -1.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_grass_right.png" },
    AirportStationSprite { sprite_id: 5968, w: 34.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, path: "assets/opengfx/tiles/airport_grass_left.png" },
];

/// APT_APRON (`StationGfx 0`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_0_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_NW (`StationGfx 1`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_1_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_SW (`StationGfx 2`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_2_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_STAND (`StationGfx 3`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_3_BASE: AirportStationBase = AirportStationBase { sprite_id: 2635, company_coloured: false };
/// APT_APRON_W (`StationGfx 4`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_4_BASE: AirportStationBase = AirportStationBase { sprite_id: 2636, company_coloured: false };
/// APT_APRON_S (`StationGfx 5`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_5_BASE: AirportStationBase = AirportStationBase { sprite_id: 2637, company_coloured: false };
/// APT_APRON_VER_CROSSING_S (`StationGfx 6`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_6_BASE: AirportStationBase = AirportStationBase { sprite_id: 2638, company_coloured: false };
/// APT_APRON_HOR_CROSSING_W (`StationGfx 7`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_7_BASE: AirportStationBase = AirportStationBase { sprite_id: 2639, company_coloured: false };
/// APT_APRON_VER_CROSSING_N (`StationGfx 8`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_8_BASE: AirportStationBase = AirportStationBase { sprite_id: 2640, company_coloured: false };
/// APT_APRON_HOR_CROSSING_E (`StationGfx 9`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_9_BASE: AirportStationBase = AirportStationBase { sprite_id: 2641, company_coloured: false };
/// APT_APRON_E (`StationGfx 10`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_10_BASE: AirportStationBase = AirportStationBase { sprite_id: 2642, company_coloured: false };
/// APT_ARPON_N (`StationGfx 11`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_11_BASE: AirportStationBase = AirportStationBase { sprite_id: 2643, company_coloured: false };
/// APT_APRON_HOR (`StationGfx 12`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_12_BASE: AirportStationBase = AirportStationBase { sprite_id: 2644, company_coloured: false };
/// APT_APRON_N_FENCE_SW (`StationGfx 13`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_13_BASE: AirportStationBase = AirportStationBase { sprite_id: 2643, company_coloured: false };
/// APT_RUNWAY_1 (`StationGfx 14`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_14_BASE: AirportStationBase = AirportStationBase { sprite_id: 2645, company_coloured: false };
/// APT_RUNWAY_2 (`StationGfx 15`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_15_BASE: AirportStationBase = AirportStationBase { sprite_id: 2646, company_coloured: false };
/// APT_RUNWAY_3 (`StationGfx 16`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_16_BASE: AirportStationBase = AirportStationBase { sprite_id: 2647, company_coloured: false };
/// APT_RUNWAY_4 (`StationGfx 17`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_17_BASE: AirportStationBase = AirportStationBase { sprite_id: 2648, company_coloured: false };
/// APT_RUNWAY_END_FENCE_SE (`StationGfx 18`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_18_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_BUILDING_2 (`StationGfx 19`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_19_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_TOWER_FENCE_SW (`StationGfx 20`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_20_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_ROUND_TERMINAL (`StationGfx 21`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_21_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_BUILDING_3 (`StationGfx 22`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_22_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_BUILDING_1 (`StationGfx 23`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_23_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_DEPOT_SE (`StationGfx 24`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_24_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_STAND_1 (`StationGfx 25`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_25_BASE: AirportStationBase = AirportStationBase { sprite_id: 2635, company_coloured: false };
/// APT_STAND_PIER_NE (`StationGfx 26`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_26_BASE: AirportStationBase = AirportStationBase { sprite_id: 2635, company_coloured: false };
/// APT_PIER_NW_NE (`StationGfx 27`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_27_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_PIER (`StationGfx 28`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_28_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_EMPTY (`StationGfx 29`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_29_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_EMPTY_FENCE_NE (`StationGfx 30`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_30_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_RADAR_GRASS_FENCE_SW (`StationGfx 31`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_31_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_RADIO_TOWER_FENCE_NE (`StationGfx 32`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_32_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_SMALL_BUILDING_3 (`StationGfx 33`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_33_BASE: AirportStationBase = AirportStationBase { sprite_id: 2665, company_coloured: false };
/// APT_SMALL_BUILDING_2 (`StationGfx 34`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_34_BASE: AirportStationBase = AirportStationBase { sprite_id: 2666, company_coloured: false };
/// APT_SMALL_BUILDING_1 (`StationGfx 35`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_35_BASE: AirportStationBase = AirportStationBase { sprite_id: 2667, company_coloured: true };
/// APT_GRASS_FENCE_SW (`StationGfx 36`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_36_BASE: AirportStationBase = AirportStationBase { sprite_id: 2669, company_coloured: false };
/// APT_GRASS_2 (`StationGfx 37`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_37_BASE: AirportStationBase = AirportStationBase { sprite_id: 2670, company_coloured: false };
/// APT_GRASS_1 (`StationGfx 38`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_38_BASE: AirportStationBase = AirportStationBase { sprite_id: 2671, company_coloured: false };
/// APT_GRASS_FENCE_NE_FLAG (`StationGfx 39`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_39_BASE: AirportStationBase = AirportStationBase { sprite_id: 2672, company_coloured: false };
/// APT_RUNWAY_SMALL_NEAR_END (`StationGfx 40`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_40_BASE: AirportStationBase = AirportStationBase { sprite_id: 2673, company_coloured: false };
/// APT_RUNWAY_SMALL_MIDDLE (`StationGfx 41`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_41_BASE: AirportStationBase = AirportStationBase { sprite_id: 2674, company_coloured: false };
/// APT_RUNWAY_SMALL_FAR_END (`StationGfx 42`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_42_BASE: AirportStationBase = AirportStationBase { sprite_id: 2675, company_coloured: false };
/// APT_SMALL_DEPOT_SE (`StationGfx 43`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_43_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPORT (`StationGfx 44`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_44_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };
/// APT_RUNWAY_END (`StationGfx 45`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_45_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_RUNWAY_5 (`StationGfx 46`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_46_BASE: AirportStationBase = AirportStationBase { sprite_id: 2646, company_coloured: false };
/// APT_TOWER (`StationGfx 47`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_47_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_NE (`StationGfx 48`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_48_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_RUNWAY_END_FENCE_NW (`StationGfx 49`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_49_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_RUNWAY_FENCE_NW (`StationGfx 50`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_50_BASE: AirportStationBase = AirportStationBase { sprite_id: 2646, company_coloured: false };
/// APT_RADAR_FENCE_SW (`StationGfx 51`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_51_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_RADAR_FENCE_NE (`StationGfx 52`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_52_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_1 (`StationGfx 53`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_53_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_2_FENCE_NW (`StationGfx 54`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_54_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_2 (`StationGfx 55`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_55_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_NE_SW (`StationGfx 56`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_56_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_RUNWAY_END_FENCE_NW_SW (`StationGfx 57`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_57_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_RUNWAY_END_FENCE_SE_SW (`StationGfx 58`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_58_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_RUNWAY_END_FENCE_NE_NW (`StationGfx 59`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_59_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_RUNWAY_END_FENCE_NE_SE (`StationGfx 60`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_60_BASE: AirportStationBase = AirportStationBase { sprite_id: 2649, company_coloured: false };
/// APT_HELIPAD_2_FENCE_NE_SE (`StationGfx 61`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_61_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_SE_SW (`StationGfx 62`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_62_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_LOW_BUILDING_FENCE_N (`StationGfx 63`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_63_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_LOW_BUILDING_FENCE_NW (`StationGfx 64`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_64_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_SE (`StationGfx 65`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_65_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_3_FENCE_SE_SW (`StationGfx 66`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_66_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_3_FENCE_NW_SW (`StationGfx 67`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_67_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_HELIPAD_3_FENCE_NW (`StationGfx 68`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_68_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_LOW_BUILDING (`StationGfx 69`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_69_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_FENCE_NE_SE (`StationGfx 70`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_70_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_HALF_EAST (`StationGfx 71`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_71_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_APRON_HALF_WEST (`StationGfx 72`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_72_BASE: AirportStationBase = AirportStationBase { sprite_id: 2634, company_coloured: false };
/// APT_GRASS_FENCE_NE_FLAG_2 (`StationGfx 73`), base `DrawGroundSprite` de OpenTTD.
pub const AIRPORT_GFX_73_BASE: AirportStationBase = AirportStationBase { sprite_id: 3981, company_coloured: false };

/// Capas `TILE_SEQ_LINE` de StationGfx 19, en orden upstream.
pub static AIRPORT_GFX_19_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2650, dx: 2.0, dy: 0.0, dz: 0.0, sx: 11, sy: 16, sz: 40, z: 0.050, w: 57.0, h: 64.0, x_offs: -22.0, y_offs: -35.0, company_coloured: true, path: "assets/opengfx/tiles/airport_terminal_a.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 20, en orden upstream.
pub static AIRPORT_GFX_20_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2651, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 10, sz: 60, z: 0.050, w: 42.0, h: 79.0, x_offs: -19.0, y_offs: -60.0, company_coloured: true, path: "assets/opengfx/tiles/airport_tower.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 21, en orden upstream.
pub static AIRPORT_GFX_21_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2652, dx: 0.0, dy: 1.0, dz: 0.0, sx: 14, sy: 14, sz: 30, z: 0.050, w: 38.0, h: 41.0, x_offs: -19.0, y_offs: -19.0, company_coloured: true, path: "assets/opengfx/tiles/airport_concourse.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 22, en orden upstream.
pub static AIRPORT_GFX_22_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2653, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 11, sz: 35, z: 0.050, w: 38.0, h: 44.0, x_offs: -17.0, y_offs: -26.0, company_coloured: true, path: "assets/opengfx/tiles/airport_terminal_b.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 23, en orden upstream.
pub static AIRPORT_GFX_23_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2654, dx: 0.0, dy: 3.0, dz: 0.0, sx: 16, sy: 11, sz: 40, z: 0.050, w: 54.0, h: 48.0, x_offs: -36.0, y_offs: -21.0, company_coloured: true, path: "assets/opengfx/tiles/airport_terminal_c.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 24, en orden upstream.
pub static AIRPORT_GFX_24_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2655, dx: 14.0, dy: 0.0, dz: 0.0, sx: 2, sy: 17, sz: 28, z: 0.050, w: 64.0, h: 55.0, x_offs: -4.0, y_offs: -38.0, company_coloured: true, path: "assets/opengfx/tiles/airport_hangar_front.png" },
    AirportStationLayer { sprite_id: 2656, dx: 0.0, dy: 0.0, dz: 0.0, sx: 2, sy: 17, sz: 28, z: 0.050, w: 17.0, h: 16.0, x_offs: 11.0, y_offs: -2.0, company_coloured: true, path: "assets/opengfx/tiles/airport_hangar_rear.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 25, en orden upstream.
pub static AIRPORT_GFX_25_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2659, dx: 7.0, dy: 11.0, dz: 0.0, sx: 3, sy: 3, sz: 14, z: 0.050, w: 17.0, h: 17.0, x_offs: -8.0, y_offs: -9.0, company_coloured: true, path: "assets/opengfx/tiles/airport_jetway_1.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 26, en orden upstream.
pub static AIRPORT_GFX_26_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2660, dx: 2.0, dy: 7.0, dz: 0.0, sx: 3, sy: 3, sz: 14, z: 0.050, w: 27.0, h: 23.0, x_offs: -9.0, y_offs: -16.0, company_coloured: true, path: "assets/opengfx/tiles/airport_jetway_2.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 27, en orden upstream.
pub static AIRPORT_GFX_27_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2661, dx: 3.0, dy: 2.0, dz: 0.0, sx: 3, sy: 3, sz: 14, z: 0.050, w: 28.0, h: 20.0, x_offs: -12.0, y_offs: -14.0, company_coloured: true, path: "assets/opengfx/tiles/airport_jetway_3.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 28, en orden upstream.
pub static AIRPORT_GFX_28_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2662, dx: 0.0, dy: 8.0, dz: 0.0, sx: 14, sy: 3, sz: 14, z: 0.050, w: 36.0, h: 24.0, x_offs: -29.0, y_offs: -10.0, company_coloured: true, path: "assets/opengfx/tiles/airport_passenger_tunnel.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 31, en orden upstream.
pub static AIRPORT_GFX_31_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2680, dx: 7.0, dy: 7.0, dz: 0.0, sx: 2, sy: 2, sz: 8, z: 0.050, w: 5.0, h: 24.0, x_offs: -2.0, y_offs: -21.0, company_coloured: false, path: "assets/opengfx/tiles/airport_radar_00.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 32, en orden upstream.
pub static AIRPORT_GFX_32_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2601, dx: 7.0, dy: 7.0, dz: 0.0, sx: 2, sy: 2, sz: 70, z: 0.050, w: 55.0, h: 77.0, x_offs: -26.0, y_offs: -71.0, company_coloured: false, path: "assets/opengfx/tiles/airport_transmitter.png" },
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 35, en orden upstream.
pub static AIRPORT_GFX_35_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2668, dx: 0.0, dy: 0.0, dz: 0.0, sx: 15, sy: 15, sz: 30, z: 0.050, w: 50.0, h: 26.0, x_offs: -21.0, y_offs: -13.0, company_coloured: true, path: "assets/opengfx/tiles/airport_airfield_terminal_c_build.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 39, en orden upstream.
pub static AIRPORT_GFX_39_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2676, dx: 4.0, dy: 11.0, dz: 0.0, sx: 1, sy: 1, sz: 20, z: 0.050, w: 10.0, h: 21.0, x_offs: 0.0, y_offs: -24.0, company_coloured: true, path: "assets/opengfx/tiles/airport_wind_0.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 43, en orden upstream.
pub static AIRPORT_GFX_43_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2657, dx: 14.0, dy: 0.0, dz: 0.0, sx: 2, sy: 17, sz: 28, z: 0.050, w: 63.0, h: 50.0, x_offs: -2.0, y_offs: -33.0, company_coloured: true, path: "assets/opengfx/tiles/airport_airfield_hangar_front.png" },
    AirportStationLayer { sprite_id: 2658, dx: 0.0, dy: 0.0, dz: 0.0, sx: 2, sy: 17, sz: 28, z: 0.050, w: 17.0, h: 17.0, x_offs: 16.0, y_offs: -1.0, company_coloured: true, path: "assets/opengfx/tiles/airport_airfield_hangar_rear.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 44, en orden upstream.
pub static AIRPORT_GFX_44_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2633, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 16, sz: 60, z: 0.050, w: 64.0, h: 87.0, x_offs: -31.0, y_offs: -56.0, company_coloured: true, path: "assets/opengfx/tiles/airport_heliport.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 47, en orden upstream.
pub static AIRPORT_GFX_47_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2651, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 10, sz: 60, z: 0.050, w: 42.0, h: 79.0, x_offs: -19.0, y_offs: -60.0, company_coloured: true, path: "assets/opengfx/tiles/airport_tower.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 51, en orden upstream.
pub static AIRPORT_GFX_51_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2680, dx: 7.0, dy: 7.0, dz: 0.0, sx: 2, sy: 2, sz: 8, z: 0.050, w: 5.0, h: 24.0, x_offs: -2.0, y_offs: -21.0, company_coloured: false, path: "assets/opengfx/tiles/airport_radar_00.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 52, en orden upstream.
pub static AIRPORT_GFX_52_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2680, dx: 7.0, dy: 7.0, dz: 0.0, sx: 2, sy: 2, sz: 8, z: 0.050, w: 5.0, h: 24.0, x_offs: -2.0, y_offs: -21.0, company_coloured: false, path: "assets/opengfx/tiles/airport_radar_00.png" },
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 53, en orden upstream.
pub static AIRPORT_GFX_53_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 4982, dx: 10.0, dy: 6.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 39.0, h: 18.0, x_offs: -9.0, y_offs: -9.0, company_coloured: false, path: "assets/opengfx/tiles/airport_helipad.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 54, en orden upstream.
pub static AIRPORT_GFX_54_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 4982, dx: 10.0, dy: 6.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 39.0, h: 18.0, x_offs: -9.0, y_offs: -9.0, company_coloured: false, path: "assets/opengfx/tiles/airport_helipad.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 55, en orden upstream.
pub static AIRPORT_GFX_55_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 4982, dx: 10.0, dy: 6.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 39.0, h: 18.0, x_offs: -9.0, y_offs: -9.0, company_coloured: false, path: "assets/opengfx/tiles/airport_helipad.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 61, en orden upstream.
pub static AIRPORT_GFX_61_LAYERS: [AirportStationLayer; 3] = [
    AirportStationLayer { sprite_id: 4982, dx: 10.0, dy: 6.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 39.0, h: 18.0, x_offs: -9.0, y_offs: -9.0, company_coloured: false, path: "assets/opengfx/tiles/airport_helipad.png" },
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 63, en orden upstream.
pub static AIRPORT_GFX_63_LAYERS: [AirportStationLayer; 3] = [
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2095, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 10, sz: 60, z: 0.050, w: 36.0, h: 26.0, x_offs: -14.0, y_offs: -10.0, company_coloured: true, path: "assets/opengfx/tiles/airport_helidepot_office.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 64, en orden upstream.
pub static AIRPORT_GFX_64_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
    AirportStationLayer { sprite_id: 2095, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 10, sz: 60, z: 0.050, w: 36.0, h: 26.0, x_offs: -14.0, y_offs: -10.0, company_coloured: true, path: "assets/opengfx/tiles/airport_helidepot_office.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 66, en orden upstream.
pub static AIRPORT_GFX_66_LAYERS: [AirportStationLayer; 3] = [
    AirportStationLayer { sprite_id: 5966, dx: 0.0, dy: 1.0, dz: 2.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 24.0, h: 14.0, x_offs: -11.0, y_offs: 8.0, company_coloured: false, path: "assets/opengfx/tiles/airport_new_helipad.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 67, en orden upstream.
pub static AIRPORT_GFX_67_LAYERS: [AirportStationLayer; 3] = [
    AirportStationLayer { sprite_id: 5966, dx: 0.0, dy: 1.0, dz: 2.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 24.0, h: 14.0, x_offs: -11.0, y_offs: 8.0, company_coloured: false, path: "assets/opengfx/tiles/airport_new_helipad.png" },
    AirportStationLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 68, en orden upstream.
pub static AIRPORT_GFX_68_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 5966, dx: 0.0, dy: 1.0, dz: 2.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 24.0, h: 14.0, x_offs: -11.0, y_offs: 8.0, company_coloured: false, path: "assets/opengfx/tiles/airport_new_helipad.png" },
    AirportStationLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, sx: 16, sy: 1, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 69, en orden upstream.
pub static AIRPORT_GFX_69_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 2095, dx: 3.0, dy: 3.0, dz: 0.0, sx: 10, sy: 10, sz: 60, z: 0.050, w: 36.0, h: 26.0, x_offs: -14.0, y_offs: -10.0, company_coloured: true, path: "assets/opengfx/tiles/airport_helidepot_office.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 71, en orden upstream.
pub static AIRPORT_GFX_71_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 5968, dx: 0.0, dy: 0.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 34.0, h: 31.0, x_offs: -31.0, y_offs: 0.0, company_coloured: false, path: "assets/opengfx/tiles/airport_grass_left.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 72, en orden upstream.
pub static AIRPORT_GFX_72_LAYERS: [AirportStationLayer; 1] = [
    AirportStationLayer { sprite_id: 5967, dx: 0.0, dy: 0.0, dz: 0.0, sx: 0, sy: 0, sz: 0, z: 0.050, w: 34.0, h: 31.0, x_offs: -1.0, y_offs: 0.0, company_coloured: false, path: "assets/opengfx/tiles/airport_grass_right.png" },
];

/// Capas `TILE_SEQ_LINE` de StationGfx 73, en orden upstream.
pub static AIRPORT_GFX_73_LAYERS: [AirportStationLayer; 2] = [
    AirportStationLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, sx: 1, sy: 16, sz: 6, z: 0.050, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationLayer { sprite_id: 2676, dx: 4.0, dy: 11.0, dz: 0.0, sx: 1, sy: 1, sz: 20, z: 0.050, w: 10.0, h: 21.0, x_offs: 0.0, y_offs: -24.0, company_coloured: true, path: "assets/opengfx/tiles/airport_wind_0.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 1, en orden upstream.
pub static AIRPORT_GFX_1_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 2, en orden upstream.
pub static AIRPORT_GFX_2_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 13, en orden upstream.
pub static AIRPORT_GFX_13_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 14, en orden upstream.
pub static AIRPORT_GFX_14_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 15, en orden upstream.
pub static AIRPORT_GFX_15_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 16, en orden upstream.
pub static AIRPORT_GFX_16_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 17, en orden upstream.
pub static AIRPORT_GFX_17_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 18, en orden upstream.
pub static AIRPORT_GFX_18_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 30, en orden upstream.
pub static AIRPORT_GFX_30_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 36, en orden upstream.
pub static AIRPORT_GFX_36_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 40, en orden upstream.
pub static AIRPORT_GFX_40_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 41, en orden upstream.
pub static AIRPORT_GFX_41_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 42, en orden upstream.
pub static AIRPORT_GFX_42_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 45, en orden upstream.
pub static AIRPORT_GFX_45_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 48, en orden upstream.
pub static AIRPORT_GFX_48_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 49, en orden upstream.
pub static AIRPORT_GFX_49_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 50, en orden upstream.
pub static AIRPORT_GFX_50_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 56, en orden upstream.
pub static AIRPORT_GFX_56_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 57, en orden upstream.
pub static AIRPORT_GFX_57_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 58, en orden upstream.
pub static AIRPORT_GFX_58_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 59, en orden upstream.
pub static AIRPORT_GFX_59_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 60, en orden upstream.
pub static AIRPORT_GFX_60_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 62, en orden upstream.
pub static AIRPORT_GFX_62_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 15.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 65, en orden upstream.
pub static AIRPORT_GFX_65_GROUND_LAYERS: [AirportStationGroundLayer; 1] = [
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Capas `TILE_SEQ_GROUND` de StationGfx 70, en orden upstream.
pub static AIRPORT_GFX_70_GROUND_LAYERS: [AirportStationGroundLayer; 2] = [
    AirportStationGroundLayer { sprite_id: 2663, dx: 0.0, dy: 0.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -2.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_y.png" },
    AirportStationGroundLayer { sprite_id: 2664, dx: 0.0, dy: 15.0, dz: 0.0, w: 33.0, h: 20.0, x_offs: -31.0, y_offs: -4.0, company_coloured: true, path: "assets/opengfx/tiles/airport_fence_x.png" },
];

/// Metadato NFO de un SpriteID airport usado por el renderer.
#[must_use]
pub fn airport_station_sprite_for_id(sprite_id: u32) -> Option<&'static AirportStationSprite> {
    match sprite_id {
        2095 => Some(&AIRPORT_STATION_SPRITES[0]),
        2601 => Some(&AIRPORT_STATION_SPRITES[1]),
        2633 => Some(&AIRPORT_STATION_SPRITES[2]),
        2634 => Some(&AIRPORT_STATION_SPRITES[3]),
        2635 => Some(&AIRPORT_STATION_SPRITES[4]),
        2636 => Some(&AIRPORT_STATION_SPRITES[5]),
        2637 => Some(&AIRPORT_STATION_SPRITES[6]),
        2638 => Some(&AIRPORT_STATION_SPRITES[7]),
        2639 => Some(&AIRPORT_STATION_SPRITES[8]),
        2640 => Some(&AIRPORT_STATION_SPRITES[9]),
        2641 => Some(&AIRPORT_STATION_SPRITES[10]),
        2642 => Some(&AIRPORT_STATION_SPRITES[11]),
        2643 => Some(&AIRPORT_STATION_SPRITES[12]),
        2644 => Some(&AIRPORT_STATION_SPRITES[13]),
        2645 => Some(&AIRPORT_STATION_SPRITES[14]),
        2646 => Some(&AIRPORT_STATION_SPRITES[15]),
        2647 => Some(&AIRPORT_STATION_SPRITES[16]),
        2648 => Some(&AIRPORT_STATION_SPRITES[17]),
        2649 => Some(&AIRPORT_STATION_SPRITES[18]),
        2650 => Some(&AIRPORT_STATION_SPRITES[19]),
        2651 => Some(&AIRPORT_STATION_SPRITES[20]),
        2652 => Some(&AIRPORT_STATION_SPRITES[21]),
        2653 => Some(&AIRPORT_STATION_SPRITES[22]),
        2654 => Some(&AIRPORT_STATION_SPRITES[23]),
        2655 => Some(&AIRPORT_STATION_SPRITES[24]),
        2656 => Some(&AIRPORT_STATION_SPRITES[25]),
        2657 => Some(&AIRPORT_STATION_SPRITES[26]),
        2658 => Some(&AIRPORT_STATION_SPRITES[27]),
        2659 => Some(&AIRPORT_STATION_SPRITES[28]),
        2660 => Some(&AIRPORT_STATION_SPRITES[29]),
        2661 => Some(&AIRPORT_STATION_SPRITES[30]),
        2662 => Some(&AIRPORT_STATION_SPRITES[31]),
        2663 => Some(&AIRPORT_STATION_SPRITES[32]),
        2664 => Some(&AIRPORT_STATION_SPRITES[33]),
        2665 => Some(&AIRPORT_STATION_SPRITES[34]),
        2666 => Some(&AIRPORT_STATION_SPRITES[35]),
        2667 => Some(&AIRPORT_STATION_SPRITES[36]),
        2668 => Some(&AIRPORT_STATION_SPRITES[37]),
        2669 => Some(&AIRPORT_STATION_SPRITES[38]),
        2670 => Some(&AIRPORT_STATION_SPRITES[39]),
        2671 => Some(&AIRPORT_STATION_SPRITES[40]),
        2672 => Some(&AIRPORT_STATION_SPRITES[41]),
        2673 => Some(&AIRPORT_STATION_SPRITES[42]),
        2674 => Some(&AIRPORT_STATION_SPRITES[43]),
        2675 => Some(&AIRPORT_STATION_SPRITES[44]),
        2676 => Some(&AIRPORT_STATION_SPRITES[45]),
        2677 => Some(&AIRPORT_STATION_SPRITES[46]),
        2678 => Some(&AIRPORT_STATION_SPRITES[47]),
        2679 => Some(&AIRPORT_STATION_SPRITES[48]),
        2680 => Some(&AIRPORT_STATION_SPRITES[49]),
        2681 => Some(&AIRPORT_STATION_SPRITES[50]),
        2682 => Some(&AIRPORT_STATION_SPRITES[51]),
        2683 => Some(&AIRPORT_STATION_SPRITES[52]),
        2684 => Some(&AIRPORT_STATION_SPRITES[53]),
        2685 => Some(&AIRPORT_STATION_SPRITES[54]),
        2686 => Some(&AIRPORT_STATION_SPRITES[55]),
        2687 => Some(&AIRPORT_STATION_SPRITES[56]),
        2688 => Some(&AIRPORT_STATION_SPRITES[57]),
        2689 => Some(&AIRPORT_STATION_SPRITES[58]),
        2690 => Some(&AIRPORT_STATION_SPRITES[59]),
        2691 => Some(&AIRPORT_STATION_SPRITES[60]),
        3981 => Some(&AIRPORT_STATION_SPRITES[61]),
        4982 => Some(&AIRPORT_STATION_SPRITES[62]),
        5966 => Some(&AIRPORT_STATION_SPRITES[63]),
        5967 => Some(&AIRPORT_STATION_SPRITES[64]),
        5968 => Some(&AIRPORT_STATION_SPRITES[65]),
        _ => None,
    }
}

/// Base de suelo exacta para un `StationGfx` airport vanilla (0..=73).
#[must_use]
pub const fn airport_station_base_for_gfx(gfx: u8) -> Option<AirportStationBase> {
    match gfx {
        0 => Some(AIRPORT_GFX_0_BASE),
        1 => Some(AIRPORT_GFX_1_BASE),
        2 => Some(AIRPORT_GFX_2_BASE),
        3 => Some(AIRPORT_GFX_3_BASE),
        4 => Some(AIRPORT_GFX_4_BASE),
        5 => Some(AIRPORT_GFX_5_BASE),
        6 => Some(AIRPORT_GFX_6_BASE),
        7 => Some(AIRPORT_GFX_7_BASE),
        8 => Some(AIRPORT_GFX_8_BASE),
        9 => Some(AIRPORT_GFX_9_BASE),
        10 => Some(AIRPORT_GFX_10_BASE),
        11 => Some(AIRPORT_GFX_11_BASE),
        12 => Some(AIRPORT_GFX_12_BASE),
        13 => Some(AIRPORT_GFX_13_BASE),
        14 => Some(AIRPORT_GFX_14_BASE),
        15 => Some(AIRPORT_GFX_15_BASE),
        16 => Some(AIRPORT_GFX_16_BASE),
        17 => Some(AIRPORT_GFX_17_BASE),
        18 => Some(AIRPORT_GFX_18_BASE),
        19 => Some(AIRPORT_GFX_19_BASE),
        20 => Some(AIRPORT_GFX_20_BASE),
        21 => Some(AIRPORT_GFX_21_BASE),
        22 => Some(AIRPORT_GFX_22_BASE),
        23 => Some(AIRPORT_GFX_23_BASE),
        24 => Some(AIRPORT_GFX_24_BASE),
        25 => Some(AIRPORT_GFX_25_BASE),
        26 => Some(AIRPORT_GFX_26_BASE),
        27 => Some(AIRPORT_GFX_27_BASE),
        28 => Some(AIRPORT_GFX_28_BASE),
        29 => Some(AIRPORT_GFX_29_BASE),
        30 => Some(AIRPORT_GFX_30_BASE),
        31 => Some(AIRPORT_GFX_31_BASE),
        32 => Some(AIRPORT_GFX_32_BASE),
        33 => Some(AIRPORT_GFX_33_BASE),
        34 => Some(AIRPORT_GFX_34_BASE),
        35 => Some(AIRPORT_GFX_35_BASE),
        36 => Some(AIRPORT_GFX_36_BASE),
        37 => Some(AIRPORT_GFX_37_BASE),
        38 => Some(AIRPORT_GFX_38_BASE),
        39 => Some(AIRPORT_GFX_39_BASE),
        40 => Some(AIRPORT_GFX_40_BASE),
        41 => Some(AIRPORT_GFX_41_BASE),
        42 => Some(AIRPORT_GFX_42_BASE),
        43 => Some(AIRPORT_GFX_43_BASE),
        44 => Some(AIRPORT_GFX_44_BASE),
        45 => Some(AIRPORT_GFX_45_BASE),
        46 => Some(AIRPORT_GFX_46_BASE),
        47 => Some(AIRPORT_GFX_47_BASE),
        48 => Some(AIRPORT_GFX_48_BASE),
        49 => Some(AIRPORT_GFX_49_BASE),
        50 => Some(AIRPORT_GFX_50_BASE),
        51 => Some(AIRPORT_GFX_51_BASE),
        52 => Some(AIRPORT_GFX_52_BASE),
        53 => Some(AIRPORT_GFX_53_BASE),
        54 => Some(AIRPORT_GFX_54_BASE),
        55 => Some(AIRPORT_GFX_55_BASE),
        56 => Some(AIRPORT_GFX_56_BASE),
        57 => Some(AIRPORT_GFX_57_BASE),
        58 => Some(AIRPORT_GFX_58_BASE),
        59 => Some(AIRPORT_GFX_59_BASE),
        60 => Some(AIRPORT_GFX_60_BASE),
        61 => Some(AIRPORT_GFX_61_BASE),
        62 => Some(AIRPORT_GFX_62_BASE),
        63 => Some(AIRPORT_GFX_63_BASE),
        64 => Some(AIRPORT_GFX_64_BASE),
        65 => Some(AIRPORT_GFX_65_BASE),
        66 => Some(AIRPORT_GFX_66_BASE),
        67 => Some(AIRPORT_GFX_67_BASE),
        68 => Some(AIRPORT_GFX_68_BASE),
        69 => Some(AIRPORT_GFX_69_BASE),
        70 => Some(AIRPORT_GFX_70_BASE),
        71 => Some(AIRPORT_GFX_71_BASE),
        72 => Some(AIRPORT_GFX_72_BASE),
        73 => Some(AIRPORT_GFX_73_BASE),
        _ => None,
    }
}

/// Capas ordenables `TILE_SEQ_LINE` del `StationGfx` airport.
#[must_use]
pub const fn airport_station_layers_for_gfx(gfx: u8) -> &'static [AirportStationLayer] {
    match gfx {
        19 => &AIRPORT_GFX_19_LAYERS,
        20 => &AIRPORT_GFX_20_LAYERS,
        21 => &AIRPORT_GFX_21_LAYERS,
        22 => &AIRPORT_GFX_22_LAYERS,
        23 => &AIRPORT_GFX_23_LAYERS,
        24 => &AIRPORT_GFX_24_LAYERS,
        25 => &AIRPORT_GFX_25_LAYERS,
        26 => &AIRPORT_GFX_26_LAYERS,
        27 => &AIRPORT_GFX_27_LAYERS,
        28 => &AIRPORT_GFX_28_LAYERS,
        31 => &AIRPORT_GFX_31_LAYERS,
        32 => &AIRPORT_GFX_32_LAYERS,
        35 => &AIRPORT_GFX_35_LAYERS,
        39 => &AIRPORT_GFX_39_LAYERS,
        43 => &AIRPORT_GFX_43_LAYERS,
        44 => &AIRPORT_GFX_44_LAYERS,
        47 => &AIRPORT_GFX_47_LAYERS,
        51 => &AIRPORT_GFX_51_LAYERS,
        52 => &AIRPORT_GFX_52_LAYERS,
        53 => &AIRPORT_GFX_53_LAYERS,
        54 => &AIRPORT_GFX_54_LAYERS,
        55 => &AIRPORT_GFX_55_LAYERS,
        61 => &AIRPORT_GFX_61_LAYERS,
        63 => &AIRPORT_GFX_63_LAYERS,
        64 => &AIRPORT_GFX_64_LAYERS,
        66 => &AIRPORT_GFX_66_LAYERS,
        67 => &AIRPORT_GFX_67_LAYERS,
        68 => &AIRPORT_GFX_68_LAYERS,
        69 => &AIRPORT_GFX_69_LAYERS,
        71 => &AIRPORT_GFX_71_LAYERS,
        72 => &AIRPORT_GFX_72_LAYERS,
        73 => &AIRPORT_GFX_73_LAYERS,
        _ => &[],
    }
}

/// Capas `TILE_SEQ_GROUND` del `StationGfx` airport.
#[must_use]
pub const fn airport_station_ground_layers_for_gfx(gfx: u8) -> &'static [AirportStationGroundLayer] {
    match gfx {
        1 => &AIRPORT_GFX_1_GROUND_LAYERS,
        2 => &AIRPORT_GFX_2_GROUND_LAYERS,
        13 => &AIRPORT_GFX_13_GROUND_LAYERS,
        14 => &AIRPORT_GFX_14_GROUND_LAYERS,
        15 => &AIRPORT_GFX_15_GROUND_LAYERS,
        16 => &AIRPORT_GFX_16_GROUND_LAYERS,
        17 => &AIRPORT_GFX_17_GROUND_LAYERS,
        18 => &AIRPORT_GFX_18_GROUND_LAYERS,
        30 => &AIRPORT_GFX_30_GROUND_LAYERS,
        36 => &AIRPORT_GFX_36_GROUND_LAYERS,
        40 => &AIRPORT_GFX_40_GROUND_LAYERS,
        41 => &AIRPORT_GFX_41_GROUND_LAYERS,
        42 => &AIRPORT_GFX_42_GROUND_LAYERS,
        45 => &AIRPORT_GFX_45_GROUND_LAYERS,
        48 => &AIRPORT_GFX_48_GROUND_LAYERS,
        49 => &AIRPORT_GFX_49_GROUND_LAYERS,
        50 => &AIRPORT_GFX_50_GROUND_LAYERS,
        56 => &AIRPORT_GFX_56_GROUND_LAYERS,
        57 => &AIRPORT_GFX_57_GROUND_LAYERS,
        58 => &AIRPORT_GFX_58_GROUND_LAYERS,
        59 => &AIRPORT_GFX_59_GROUND_LAYERS,
        60 => &AIRPORT_GFX_60_GROUND_LAYERS,
        62 => &AIRPORT_GFX_62_GROUND_LAYERS,
        65 => &AIRPORT_GFX_65_GROUND_LAYERS,
        70 => &AIRPORT_GFX_70_GROUND_LAYERS,
        _ => &[],
    }
}