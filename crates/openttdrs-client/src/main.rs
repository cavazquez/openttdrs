//! Cliente isométrico: sprites de OpenGFX + gizmos de overlay para el [`GameState`] del core.
//!
//! Para cargar un mapa real de OpenTTD, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! ```

#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]

use std::collections::HashMap;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{
    find_path, GameState, Industry, IndustryKind, Map, Station, TileCoord, TileKind, Vehicle,
    VehicleKind,
};

// ── Constantes isométricas ────────────────────────────────────────────────────
/// Dimensiones del mapa generado proceduralmente (sin OTTDMAP_FILE).
const MAP_W: u32 = 24;
const MAP_H: u32 = 18;
/// Desplazamiento horizontal por tesela en pantalla (la tesela mide 64 px de ancho).
const ISO_HW: f32 = 32.0;
/// Desplazamiento vertical por tesela en pantalla (ratio 2:1 isométrico).
const ISO_QH: f32 = 16.0;
/// Factor de escala para los sprites de camiones (son 20×14 px nativo).
const TRUCK_SCALE: f32 = 2.0;
/// Píxeles de elevación en Y por cada unidad de altura de OpenTTD.
/// En el juego original, 1 unidad = 8 px de pantalla.
const HEIGHT_PX: f32 = 8.0;
/// Paneo con botón derecho: factor × `OrthographicProjection::scale` × delta en píxeles.
const PAN_RMB_SCALE: f32 = 1.05;
/// Zoom con teclado (+/-): fracción de `scale` por segundo al mantener pulsado.
const ZOOM_KEY_RATE: f32 = 3.5;
/// Zoom con rueda: multiplicador por unidad de `scroll.delta.y`.
const ZOOM_WHEEL_SENS: f32 = 0.16;

// ── Utilidades de proyección ──────────────────────────────────────────────────

/// Convierte coordenadas de tesela a posición del vértice superior del rombo (Bevy Y-up).
fn iso(tx: i32, ty: i32) -> Vec2 {
    Vec2::new(
        (tx - ty) as f32 * ISO_HW,
        (tx + ty) as f32 * -ISO_QH,
    )
}

/// Convierte posición del mundo a coordenadas de tesela (inversa de `iso`).
fn world_to_tile(world_pos: Vec2) -> (i32, i32) {
    // iso: screen_x = (tx - ty) * ISO_HW
    //      screen_y = (tx + ty) * -ISO_QH
    // Inversa:
    //      tx - ty = screen_x / ISO_HW
    //      tx + ty = screen_y / -ISO_QH
    //      2*tx = screen_x/ISO_HW + screen_y/-ISO_QH
    //      2*ty = screen_y/-ISO_QH - screen_x/ISO_HW
    let a = world_pos.x / ISO_HW;
    let b = world_pos.y / -ISO_QH;
    let tx = (a + b) / 2.0;
    let ty = (b - a) / 2.0;
    (tx.floor() as i32, ty.floor() as i32)
}

/// La mitad de la altura de los sprites de tesela (64×31 → 15.5 px).
const TILE_HALF_H: f32 = 15.5;

/// Vec3 para teselas de suelo con soporte de altura isométrica.
///
/// `half_h` es la mitad de la altura del sprite en píxeles (anclaje al vértice superior del rombo).
fn tile_pos_half(tx: i32, ty: i32, height: u8, layer: f32, half_h: f32) -> Vec3 {
    let p = iso(tx, ty);
    let elev = f32::from(height) * HEIGHT_PX;
    Vec3::new(
        p.x,
        p.y - half_h + elev,
        (tx + ty) as f32 * 0.01 + f32::from(height) * 0.001 + layer,
    )
}

/// [`tile_pos_half`] con la altura estándar de tesela 64×31.
fn tile_pos(tx: i32, ty: i32, height: u8, layer: f32) -> Vec3 {
    tile_pos_half(tx, ty, height, layer, TILE_HALF_H)
}

/// Calcula la posición del centro de un sprite overlay a partir del xrel/yrel del NFO.
///
/// - `ref_pos`: vértice superior del rombo de la tesela (salida de `iso()`).
/// - `xrel`, `yrel`: offsets del NFO (en coords pantalla Y-down).
/// - `w`, `h`: dimensiones del sprite en píxeles.
/// - `height`: elevación de la tesela (OpenTTD units).
fn overlay_pos(
    ref_pos: Vec2, xrel: f32, yrel: f32, w: f32, h: f32,
    height: u8, layer: f32, tx: i32, ty: i32,
) -> Vec3 {
    let elev = f32::from(height) * HEIGHT_PX;
    Vec3::new(
        ref_pos.x + xrel + w / 2.0,
        ref_pos.y - yrel - h / 2.0 + elev,
        (tx + ty) as f32 * 0.01 + f32::from(height) * 0.001 + layer,
    )
}

/// Dibuja el contorno de un rombo isométrico.
fn gizmo_diamond(gizmos: &mut Gizmos, center: Vec2, hw: f32, hh: f32, color: Color) {
    let t = center + Vec2::new(0.0, hh);
    let r = center + Vec2::new(hw, 0.0);
    let b = center + Vec2::new(0.0, -hh);
    let l = center + Vec2::new(-hw, 0.0);
    gizmos.line_2d(t, r, color);
    gizmos.line_2d(r, b, color);
    gizmos.line_2d(b, l, color);
    gizmos.line_2d(l, t, color);
}

// ── Constantes de renderizado de carreteras y vías ───────────────────────────

/// Tipos de tesela OpenTTD (nibble alto del byte MAPT).
const OTTD_MP_RAIL: u8 = 1;
const OTTD_MP_ROAD: u8 = 2;
const OTTD_MP_TUNNELBRIDGE: u8 = 9;

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`rail_map.h`).
const RAIL_TILE_NORMAL: u8 = 0;
const RAIL_TILE_SIGNALS: u8 = 1;
const RAIL_TILE_DEPOT: u8 = 3;

/// Desplazamiento dentro del grupo SPR_ROAD para tesela plana (`GetRoadSpriteOffset`, `road_cmd.cpp`).
const ROAD_FLAT_OFFSET_TBL: [u8; 16] = [
    0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2,
];

/// Mitad de la altura en px de cada variante `road_flat_XX` (desde NFO OpenGFX 8.0, sprites 1332–1350).
const ROAD_FLAT_HALF_H: [f32; 19] = [
    15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 15.5, 19.5, 11.5, 11.5, 19.5, 15.5,
    15.5, 15.5, 15.5,
];

/// `TrackBits` en vía clásica (`track_type.h`): piezas sobre tesela plana.
const RAIL_TB_X: u8 = 1;
const RAIL_TB_Y: u8 = 2;
const RAIL_TB_UPPER: u8 = 4;
const RAIL_TB_LOWER: u8 = 8;
const RAIL_TB_LEFT: u8 = 16;
const RAIL_TB_RIGHT: u8 = 32;
const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;
const RAIL_TB_HORZ: u8 = RAIL_TB_UPPER | RAIL_TB_LOWER;
const RAIL_TB_VERT: u8 = RAIL_TB_LEFT | RAIL_TB_RIGHT;

/// Máscaras 3 vías por esquina (`GetJunctionGroundSpriteOffset`, `rail_cmd.cpp`).
const RAIL_3WAY_NE: u8 = RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_RIGHT;
const RAIL_3WAY_SW: u8 = RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_LEFT;
const RAIL_3WAY_NW: u8 = RAIL_TB_Y | RAIL_TB_UPPER | RAIL_TB_LEFT;
const RAIL_3WAY_SE: u8 = RAIL_TB_Y | RAIL_TB_LOWER | RAIL_TB_RIGHT;

/// Intercambia bits NW (0) ↔ SE (2) para compensar que nuestro eje Y isométrico
/// está invertido respecto a OpenTTD.
#[inline]
fn swap_y_road_bits(bits: u8) -> u8 {
    (bits & 0b1010) | ((bits & 0b0001) << 2) | ((bits & 0b0100) >> 2)
}

/// Decodifica los road bits efectivos desde m5 según el tipo de tesela OpenTTD.
///
/// - `MP_ROAD` normal: bits 0-3 = road bits; cruces ferroviarios (`subtype 1`) guardan
///   el eje de la carretera en el bit 0, no como road bits; depósitos (`subtype 2`)
///   guardan `DiagDirection` en bits 0-1.
/// - `MP_TUNNELBRIDGE` con `TileKind::Road`: dirección en bits 0-1 (igual que depósito).
///
/// Ver `road_map.h` (`GetCrossingRoadBits`, `GetRoadDepotDirection`, `GetTunnelBridgeDirection`).
///
/// **Nota**: Los bits se intercambian (NW↔SE) porque nuestro sistema isométrico tiene
/// el eje Y invertido respecto a OpenTTD.
fn effective_road_bits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    let tt = (mapt >> 4) & 0xF;
    let raw = match tt {
        OTTD_MP_ROAD => {
            let subtype = (m5 >> 6) & 0x3;
            match subtype {
                0 => {
                    let rb = m5 & 0x0F;
                    if rb == 0 { None } else { Some(rb) }
                }
                // Cruce a nivel: bit 0 = eje de la carretera (AXIS_X → ROAD_X).
                1 => {
                    let axis = m5 & 1;
                    Some(if axis == 0 { 0x0A } else { 0x05 })
                }
                // Depósito de carretera: DiagDirection en bits 0-1 → un solo road bit.
                2 => {
                    let d = m5 & 0x3;
                    Some((1u8 << (3 ^ d)) & 0x0F)
                }
                _ => None,
            }
        }
        OTTD_MP_TUNNELBRIDGE if kind == TileKind::Road => {
            let d = m5 & 0x3;
            Some((1u8 << (3 ^ d)) & 0x0F)
        }
        _ => None,
    };
    raw.map(swap_y_road_bits)
}

#[inline]
fn road_flat_index(road_bits: u8) -> usize {
    usize::from(ROAD_FLAT_OFFSET_TBL[usize::from(road_bits & 0x0F)])
}

/// Road bits para dibujar: `m5` / vecinos (mapa procedural).
/// RoadBits: NW=1, SW=2, SE=4, NE=8 (sentido horario desde NW).
/// Offsets OpenTTD: NE=(-1,0), SE=(0,+1), SW=(+1,0), NW=(0,-1).
fn road_bits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    if let Some(t) = map.get(pos) {
        if let Some(rb) = effective_road_bits(t.mapt, t.m5, t.kind) {
            if rb != 0 {
                return rb & 0x0F;
            }
        }
    }
    // Fallback: detectar conexiones en las 4 direcciones individualmente.
    let is_road_or_station = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(
            map.get_kind(c),
            Some(TileKind::Road | TileKind::Station | TileKind::Industry | TileKind::House)
        )
    };
    let mut bits = 0u8;
    // Nuestro sistema tiene Y invertido respecto a OpenTTD:
    // - Nuestro +Y muestra hacia NW en pantalla
    // - Nuestro -Y muestra hacia SE en pantalla
    // NE: (-1, 0) → bit 3
    if is_road_or_station(TileCoord::new(pos.x - 1, pos.y)) {
        bits |= 8;
    }
    // NW: (0, +1) en nuestras coords → bit 0
    if is_road_or_station(TileCoord::new(pos.x, pos.y + 1)) {
        bits |= 1;
    }
    // SW: (+1, 0) → bit 1
    if is_road_or_station(TileCoord::new(pos.x + 1, pos.y)) {
        bits |= 2;
    }
    // SE: (0, -1) en nuestras coords → bit 2
    if is_road_or_station(TileCoord::new(pos.x, pos.y - 1)) {
        bits |= 4;
    }
    // Si no hay conexiones, asumir eje Y (NW + SE).
    if bits == 0 {
        bits = 0x05;
    }
    bits
}

fn effective_rail_trackbits(mapt: u8, m5: u8, kind: TileKind) -> Option<u8> {
    if kind != TileKind::Rail {
        return None;
    }
    let tt = (mapt >> 4) & 0xF;
    if tt != OTTD_MP_RAIL {
        return None;
    }
    let subtype = (m5 >> 6) & 0x3;
    match subtype {
        RAIL_TILE_NORMAL | RAIL_TILE_SIGNALS => Some(m5 & 0x3F),
        RAIL_TILE_DEPOT => {
            let d = m5 & 0x3;
            Some(if d == 1 || d == 3 { RAIL_TB_X } else { RAIL_TB_Y })
        }
        _ => None,
    }
}

fn synthetic_rail_trackbits(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    let rail_neighbor = |c: TileCoord| -> bool {
        if c.x < 0 || c.y < 0 || c.x >= mw as i32 || c.y >= mh as i32 {
            return false;
        }
        matches!(
            map.get_kind(c),
            Some(TileKind::Rail | TileKind::Station)
        )
    };
    let has_tx = rail_neighbor(TileCoord::new(pos.x - 1, pos.y))
        || rail_neighbor(TileCoord::new(pos.x + 1, pos.y));
    let has_ty = rail_neighbor(TileCoord::new(pos.x, pos.y - 1))
        || rail_neighbor(TileCoord::new(pos.x, pos.y + 1));
    match (has_tx, has_ty) {
        (true, false) => RAIL_TB_Y,
        (false, true) => RAIL_TB_X,
        (true, true) => RAIL_TB_CROSS,
        (false, false) => RAIL_TB_Y,
    }
}

fn rail_trackbits_for_render(map: &Map, pos: TileCoord, mw: u32, mh: u32) -> u8 {
    if let Some(t) = map.get(pos) {
        if let Some(tb) = effective_rail_trackbits(t.mapt, t.m5, t.kind) {
            if tb != 0 {
                return tb & 0x3F;
            }
        }
    }
    synthetic_rail_trackbits(map, pos, mw, mh)
}

#[inline]
fn junction_ground_off(tb: u8) -> u8 {
    let t = tb & 0x3F;
    if t & RAIL_3WAY_NE == 0 {
        return 0;
    }
    if t & RAIL_3WAY_SW == 0 {
        return 1;
    }
    if t & RAIL_3WAY_NW == 0 {
        return 2;
    }
    if t & RAIL_3WAY_SE == 0 {
        return 3;
    }
    4
}

/// Lista de sprites OpenGFX (`rail_<id>.png`) en orden de pintado (suelo de cruce y superposiciones).
fn collect_rail_sprites(tb: u8, out: &mut Vec<u32>) {
    out.clear();
    let t = tb & 0x3F;
    match t {
        RAIL_TB_Y => out.push(1011),
        RAIL_TB_X => out.push(1012),
        RAIL_TB_UPPER => out.push(1013),
        RAIL_TB_LOWER => out.push(1014),
        RAIL_TB_RIGHT => out.push(1015),
        RAIL_TB_LEFT => out.push(1016),
        RAIL_TB_CROSS => out.push(1017),
        RAIL_TB_HORZ => out.push(1035),
        RAIL_TB_VERT => out.push(1036),
        _ => {
            out.push(1018_u32 + u32::from(junction_ground_off(t)));
            if t & RAIL_TB_X != 0 {
                out.push(1005);
            }
            if t & RAIL_TB_Y != 0 {
                out.push(1006);
            }
            if t & RAIL_TB_UPPER != 0 {
                out.push(1007);
            }
            if t & RAIL_TB_LOWER != 0 {
                out.push(1008);
            }
            if t & RAIL_TB_RIGHT != 0 {
                out.push(1009);
            }
            if t & RAIL_TB_LEFT != 0 {
                out.push(1010);
            }
        }
    }
}

// ── Dirección de vehículo ─────────────────────────────────────────────────────

/// Dirección de movimiento de un vehículo en pantalla isométrica.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum VehicleDir {
    #[default]
    Ne,
    Se,
    Sw,
    Nw,
}

/// Deduce la dirección del vehículo a partir del siguiente paso en su path.
fn vehicle_dir(v: &Vehicle) -> VehicleDir {
    let Some(next) = v.path.front() else {
        return VehicleDir::default();
    };
    let dx = next.x - v.pos.x;
    let dy = next.y - v.pos.y;
    // En isométrico: +tx = SE, -tx = NW, +ty = SW, -ty = NE
    match (dx.signum(), dy.signum()) {
        (1, _) => VehicleDir::Se,
        (-1, _) => VehicleDir::Nw,
        (_, 1) => VehicleDir::Sw,
        _ => VehicleDir::Ne,
    }
}

// ── Recursos ──────────────────────────────────────────────────────────────────

/// Handles de los sprites de camiones en las 4 direcciones isométricas.
#[derive(Resource)]
struct TruckHandles {
    ne: Handle<Image>,
    se: Handle<Image>,
    sw: Handle<Image>,
    nw: Handle<Image>,
}

impl TruckHandles {
    fn for_dir(&self, dir: VehicleDir) -> Handle<Image> {
        match dir {
            VehicleDir::Ne => self.ne.clone(),
            VehicleDir::Se => self.se.clone(),
            VehicleDir::Sw => self.sw.clone(),
            VehicleDir::Nw => self.nw.clone(),
        }
    }
}

/// Información del tile actualmente seleccionado (click izquierdo).
#[derive(Resource, Default)]
struct SelectedTileInfo {
    /// Coordenadas del tile seleccionado.
    pos: Option<TileCoord>,
}

// ── Componentes ───────────────────────────────────────────────────────────────

/// Marcador para el texto de información del tile.
#[derive(Component)]
struct TileInfoText;

/// Marca la entidad sprite de un vehículo; el valor es el `id` del vehículo.
#[derive(Component)]
struct VehicleSprite(u32);

// ── App ───────────────────────────────────────────────────────────────────────

fn main() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "openttdrs".into(),
                        resolution: (1280_u32, 720_u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root.into(),
                    ..default()
                }),
        )
        .init_resource::<SimWorld>()
        .init_resource::<SelectedTileInfo>()
        .add_systems(Startup, (setup, setup_tile_info_ui))
        .add_systems(
            Update,
            (
                advance_sim,
                sync_window_title,
                update_vehicles,
                draw_industries,
                draw_stations,
                move_camera,
                handle_tile_click,
                update_tile_info_text,
            )
                .chain(),
        )
        .run();
}

// ── Simulación ────────────────────────────────────────────────────────────────

#[derive(Resource)]
struct SimWorld {
    state:       GameState,
    /// Indica que el mapa se cargó desde un archivo .ottdmap externo.
    loaded_file: bool,
}

impl Default for SimWorld {
    fn default() -> Self {
        // Si la variable de entorno OTTDMAP_FILE apunta a un .ottdmap válido, cargarlo.
        if let Ok(path) = std::env::var("OTTDMAP_FILE") {
            match std::fs::read(&path) {
                Ok(data) => match Map::from_ottd_binary(&data) {
                    Ok(map) => {
                        info!("Mapa cargado desde {path}");
                        let mut state = GameState::from_map(map);
                        // Detectar industrias a partir de los tiles de tipo Industry.
                        place_industries(&mut state, true);
                        info!("Industrias detectadas: {}", state.industries.len());
                        return Self {
                            state,
                            loaded_file: true,
                        };
                    }
                    Err(e) => error!("Error al parsear {path}: {e:?}"),
                },
                Err(e) => error!("No se pudo leer {path}: {e}"),
            }
        }
        // Mapa generado proceduralmente (modo de desarrollo).
        let mut state = GameState::new(MAP_W, MAP_H);
        distribute_tile_kinds(&mut state, 0xDEAD_BEEF_CAFE_1234);
        place_industries(&mut state, false);
        place_stations(&mut state);
        place_roads(&mut state);
        place_vehicles(&mut state);
        Self { state, loaded_file: false }
    }
}

fn distribute_tile_kinds(state: &mut GameState, seed: u64) {
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let kind = tile_kind_hash(x, y, seed);
            let c = TileCoord::new(x as i32, y as i32);
            let _ = state.map.set_kind(c, kind);
        }
    }
}

fn tile_kind_hash(x: u32, y: u32, seed: u64) -> TileKind {
    let mut h = seed
        .wrapping_add(u64::from(x).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(u64::from(y).wrapping_mul(0x6C62_272E_07BB_0142));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    match h % 10 {
        0 | 1 => TileKind::Water,
        2 | 3 => TileKind::Forest,
        4 => TileKind::CoalField,
        _ => TileKind::Grass,
    }
}

fn place_industries(state: &mut GameState, from_ottd_file: bool) {
    let (mw, mh) = state.map.dimensions();
    let mut coal_n = 0u32;
    let mut forest_n = 0u32;
    let mut industry_n = 0u32;

    // Para mapas de OpenTTD, solo usar TileKind::Industry (stride alto para no saturar).
    // Para mapas procedurales, usar CoalField y Forest.
    let stride_proc = 4u32;
    let stride_ottd = 16u32;

    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            match state.map.get_kind(c) {
                Some(TileKind::CoalField) if !from_ottd_file => {
                    if coal_n % stride_proc == 0 {
                        state.industries.push(Industry::new(c, IndustryKind::CoalMine));
                    }
                    coal_n += 1;
                }
                Some(TileKind::Forest) if !from_ottd_file => {
                    if forest_n % stride_proc == 0 {
                        state.industries.push(Industry::new(c, IndustryKind::Forest));
                    }
                    forest_n += 1;
                }
                Some(TileKind::Industry) => {
                    // Industrias reales de savegames OpenTTD
                    if industry_n % stride_ottd == 0 {
                        let kind = if industry_n % 2 == 0 {
                            IndustryKind::CoalMine
                        } else {
                            IndustryKind::Forest
                        };
                        state.industries.push(Industry::new(c, kind));
                    }
                    industry_n += 1;
                }
                _ => {}
            }
        }
    }
}

fn place_stations(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    let positions: Vec<TileCoord> = state
        .industries
        .iter()
        .enumerate()
        .map(|(i, ind)| {
            let dy = if i % 2 == 0 { 3i32 } else { -3i32 };
            TileCoord::new(
                (ind.pos.x + 3).clamp(0, mw as i32 - 1),
                (ind.pos.y + dy).clamp(0, mh as i32 - 1),
            )
        })
        .collect();
    for pos in positions {
        state.stations.push(Station::new(pos));
    }
}

fn place_roads(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (from, to) in routes {
        let mut cur = from;
        while cur.x != to.x {
            cur.x += (to.x - cur.x).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
        while cur.y != to.y {
            cur.y += (to.y - cur.y).signum();
            if cur != to && cur != from {
                let _ = state.map.set_kind(cur, TileKind::Road);
            }
        }
    }
}

fn place_vehicles(state: &mut GameState) {
    let routes: Vec<(TileCoord, TileCoord)> = state
        .industries
        .iter()
        .zip(state.stations.iter())
        .map(|(ind, st)| (ind.pos, st.pos))
        .collect();

    for (i, (a, b)) in routes.into_iter().enumerate() {
        let mut v = Vehicle::new(i as u32, VehicleKind::Truck, a, b);
        if let Some(path) = find_path(&state.map, a, b) {
            v.path = path.into_iter().collect();
        }
        state.vehicles.push(v);
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
fn setup(mut commands: Commands, asset_server: Res<AssetServer>, sim: Res<SimWorld>) {
    let (mw, mh) = sim.state.map.dimensions();

    // Centro del mapa isométrico (funciona para cualquier tamaño).
    let cam_x = ((mw as i32 - 1) - (mh as i32 - 1)) as f32 / 2.0 * ISO_HW;
    let cam_y = -((mw as i32 - 1) + (mh as i32 - 1)) as f32 / 2.0 * ISO_QH - TILE_HALF_H;

    // Zoom inicial: para el mapa generado (24×18) scale=1; para mapas grandes escalar
    // para que quepan ≈64 teselas de ancho en la ventana de 1280 px.
    let target_tiles_wide: f32 = if sim.loaded_file { 64.0 } else { mw as f32 };
    let cam_scale = (target_tiles_wide * ISO_HW * 2.0 / 1280.0).max(1.0);

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.45, 0.60, 0.75)),
            ..default()
        },
        Transform::from_translation(Vec3::new(cam_x, cam_y, 999.9)),
        Projection::Orthographic(OrthographicProjection {
            scale: cam_scale,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // ── Handles de teselas de suelo ───────────────────────────────────────────
    let h_grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
    let h_rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
    let h_water = asset_server.load::<Image>("opengfx/tiles/water.png");
    let road_flat: Vec<Handle<Image>> = (0..19)
        .map(|i| {
            asset_server.load::<Image>(format!("opengfx/tiles/road_flat_{i:02}.png"))
        })
        .collect();
    let rail_ids = [
        1005_u32, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018,
        1019, 1020, 1021, 1022, 1035, 1036,
    ];
    let rail_tex: HashMap<u32, Handle<Image>> = rail_ids
        .iter()
        .copied()
        .map(|id| {
            (
                id,
                asset_server.load::<Image>(format!("opengfx/tiles/rail_{id}.png")),
            )
        })
        .collect();

    // ── Handles de estaciones (suelo de parada de camión, 64×31 estándar) ──────
    let station_grounds: Vec<Handle<Image>> = (0..4)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/truck_stop_ground_{i}.png")))
        .collect();

    // ── Handles de casas urbanas (8 variantes) ────────────────────────────────
    // Sprites 1424–1429, 1433, 1437 con dimensiones y offsets variables.
    let house_tex: Vec<Handle<Image>> = (0..8)
        .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/house_{i}.png")))
        .collect();
    // (w, h, xrel, yrel) extraídos del NFO para cada house_X.png
    #[allow(clippy::type_complexity)]
    let house_meta: [(f32, f32, f32, f32); 8] = [
        (64.0, 37.0, -31.0, -6.0),  // house_0 (1424)
        (65.0, 71.0, -31.0, -40.0), // house_1 (1425)
        (64.0, 36.0, -31.0, -5.0),  // house_2 (1426)
        (66.0, 80.0, -32.0, -49.0), // house_3 (1427)
        (66.0, 87.0, -32.0, -56.0), // house_4 (1428)
        (64.0, 36.0, -31.0, -5.0),  // house_5 (1429)
        (64.0, 35.0, -31.0, -4.0),  // house_6 (1433)
        (64.0, 34.0, -31.0, -3.0),  // house_7 (1437)
    ];

    // ── Handles de overlays ───────────────────────────────────────────────────
    let h_tree_1 = asset_server.load::<Image>("opengfx/tiles/tree_00.png");
    let h_tree_2 = asset_server.load::<Image>("opengfx/tiles/tree_07.png");
    let h_tree_3 = asset_server.load::<Image>("opengfx/tiles/tree_14.png");
    // sid=2013 (58×50): headframe principal de la Coal Mine (IT_COAL_MINE = 0).
    let h_coal = asset_server.load::<Image>("opengfx/tiles/industry_coalmine_hq.png");
    let trees = [h_tree_1, h_tree_2, h_tree_3];

    // ── Handles de camiones ───────────────────────────────────────────────────
    let h_truck_ne = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_se = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_sw = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    let h_truck_nw = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    commands.insert_resource(TruckHandles {
        ne: h_truck_ne,
        se: h_truck_se,
        sw: h_truck_sw,
        nw: h_truck_nw,
    });

    // ── Teselas de suelo ──────────────────────────────────────────────────────
    let (mw, mh) = sim.state.map.dimensions();
    let mut rail_layers: Vec<u32> = Vec::with_capacity(8);
    for ty in 0..mh {
        for tx in 0..mw {
            let c = TileCoord::new(tx as i32, ty as i32);
            let tile = sim.state.map.get(c);
            let kind    = tile.map(|t| t.kind).unwrap_or(TileKind::Grass);
            let height  = tile.map(|t| t.height).unwrap_or(0);
            let p = iso(tx as i32, ty as i32);

            // Void: borde del mapa, no renderizar
            if kind == TileKind::Void {
                continue;
            }

            if kind == TileKind::Road {
                let fi = road_flat_index(road_bits_for_render(&sim.state.map, c, mw, mh));
                let pos_road = tile_pos_half(
                    tx as i32,
                    ty as i32,
                    height,
                    0.0,
                    ROAD_FLAT_HALF_H[fi],
                );
                commands.spawn((
                    Sprite {
                        image: road_flat[fi].clone(),
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(pos_road),
                ));
            } else if kind == TileKind::Rail {
                collect_rail_sprites(
                    rail_trackbits_for_render(&sim.state.map, c, mw, mh),
                    &mut rail_layers,
                );
                for (i, sid) in rail_layers.iter().copied().enumerate() {
                    let Some(img) = rail_tex.get(&sid) else {
                        continue;
                    };
                    let z = i as f32 * 0.0004;
                    commands.spawn((
                        Sprite {
                            image: img.clone(),
                            color: Color::srgb(0.88, 0.88, 0.97),
                            ..default()
                        },
                        Transform::from_translation(tile_pos(
                            tx as i32,
                            ty as i32,
                            height,
                            z,
                        )),
                    ));
                }
            } else if kind == TileKind::House {
                // Suelo de hierba + edificio encima (overlay).
                commands.spawn((
                    Sprite { image: h_grass.clone(), color: Color::WHITE, ..default() },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
                let hi = wang_hash(tx, ty, 0xBEEF) as usize % house_tex.len();
                let (w, h, xr, yr) = house_meta[hi];
                let pos3 = overlay_pos(p, xr, yr, w, h, height, 0.5, tx as i32, ty as i32);
                commands.spawn((
                    Sprite { image: house_tex[hi].clone(), color: Color::WHITE, ..default() },
                    Transform::from_translation(pos3),
                ));
            } else if kind == TileKind::Station {
                // Sprite de suelo de parada de camión (64×31).
                let dir = wang_hash(tx, ty, 0xCAFE) as usize % station_grounds.len();
                commands.spawn((
                    Sprite { image: station_grounds[dir].clone(), color: Color::WHITE, ..default() },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
            } else {
                let (image, color) = match kind {
                    TileKind::Grass   => (h_grass.clone(), Color::WHITE),
                    TileKind::Forest  => (h_rough.clone(), Color::srgb(0.6, 1.0, 0.45)),
                    TileKind::CoalField | TileKind::Industry
                                      => (h_rough.clone(), Color::srgb(0.55, 0.50, 0.45)),
                    TileKind::Water   => (h_water.clone(), Color::WHITE),
                    TileKind::Unknown(_) => (h_grass.clone(), Color::srgb(1.0, 0.0, 1.0)),
                    TileKind::House | TileKind::Station | TileKind::Road
                        | TileKind::Rail | TileKind::Void => unreachable!(),
                };

                commands.spawn((
                    Sprite { image, color, ..default() },
                    Transform::from_translation(tile_pos(tx as i32, ty as i32, height, 0.0)),
                ));
            }

            // ── Árbol sobre teselas Forest ────────────────────────────────────
            if kind == TileKind::Forest {
                let h = wang_hash(tx, ty, 0xCAFE);
                let tree_idx = (h % 3) as usize;
                let ox = ((h >> 2) % 17) as f32 - 8.0;
                // NFO: tree_1-3 → xrel=-19 yrel=-36 w=35 h=43
                let pos3 = overlay_pos(
                    Vec2::new(p.x + ox, p.y),
                    -19.0, -36.0, 35.0, 43.0,
                    height, 0.3,
                    tx as i32, ty as i32,
                );
                commands.spawn((
                    Sprite { image: trees[tree_idx].clone(), ..default() },
                    Transform::from_translation(pos3),
                ));
            }
        }
    }

    // ── Edificios de industrias ───────────────────────────────────────────────
    for industry in &sim.state.industries {
        if industry.kind == IndustryKind::CoalMine {
            let pos = industry.pos;
            let h = sim.state.map.get(pos).map(|t| t.height).unwrap_or(0);
            let p = iso(pos.x, pos.y);
            // sid=2013: headframe de la Coal Mine, 58×50, xrel=-16, yrel=-33
            let pos3 = overlay_pos(p, -16.0, -33.0, 58.0, 50.0, h, 0.6, pos.x, pos.y);
            commands.spawn((
                Sprite { image: h_coal.clone(), ..default() },
                Transform::from_translation(pos3),
            ));
        }
    }

    // ── Sprites de vehículos ──────────────────────────────────────────────────
    // Se spawnean aquí y se actualizan en update_vehicles cada tick.
    // TruckHandles se insertó arriba; los accedemos desde el resource en update_vehicles.
    // Por ahora cargamos el handle SW como placeholder; update_vehicles lo corrige en el primer frame.
    let h_truck_ne_init = asset_server.load::<Image>("opengfx/tiles/vehicle_bus_sw.png");
    for vehicle in &sim.state.vehicles {
        let vh = sim.state.map.get(vehicle.pos).map(|t| t.height).unwrap_or(0);
        let p = iso(vehicle.pos.x, vehicle.pos.y);
        // NFO truck_ne: xrel=-14 yrel=-5 w=20 h=14
        let pos3 = overlay_pos(p, -14.0, -5.0, 20.0, 14.0, vh, 1.0, vehicle.pos.x, vehicle.pos.y);
        commands.spawn((
            VehicleSprite(vehicle.id),
            Sprite {
                image: h_truck_ne_init.clone(),
                ..default()
            },
            Transform::from_translation(pos3).with_scale(Vec3::splat(TRUCK_SCALE)),
        ));
    }
}

// ── Sistemas de actualización ─────────────────────────────────────────────────

fn advance_sim(time: Res<Time>, mut sim: ResMut<SimWorld>, mut acc: Local<f32>) {
    const TICK_HZ: f32 = 15.0;
    *acc += time.delta_secs();
    let period = 1.0 / TICK_HZ;
    while *acc >= period {
        *acc -= period;
        sim.state.step();
    }
}

fn sync_window_title(sim: Res<SimWorld>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.title = format!("openttdrs — tick {}", sim.state.tick.get());
    }
}

/// Actualiza posición y dirección de cada sprite de vehículo.
fn update_vehicles(
    sim: Res<SimWorld>,
    trucks: Res<TruckHandles>,
    mut q: Query<(&VehicleSprite, &mut Transform, &mut Sprite)>,
) {
    for (vs, mut transform, mut sprite) in &mut q {
        let Some(v) = sim.state.vehicles.iter().find(|v| v.id == vs.0) else {
            continue;
        };
        let dir = vehicle_dir(v);
        let vh = sim.state.map.get(v.pos).map(|t| t.height).unwrap_or(0);
        let p = iso(v.pos.x, v.pos.y);

        // Offset según dirección (xrel/yrel del NFO para cada vista del camión).
        let (xrel, yrel, w, h) = match dir {
            VehicleDir::Ne => (-14.0, -5.0, 20.0, 14.0),
            VehicleDir::Se => (-6.0, -6.0, 20.0, 15.0),
            VehicleDir::Sw => (-14.0, -6.0, 20.0, 15.0),
            VehicleDir::Nw => (-6.0, -5.0, 20.0, 14.0),
        };
        let pos3 = overlay_pos(p, xrel, yrel, w, h, vh, 1.0, v.pos.x, v.pos.y);
        transform.translation = pos3;
        sprite.image = trucks.for_dir(dir);
    }
}

/// Dibuja contorno de rombo para cada industria + barra de stock.
fn draw_industries(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    for industry in &sim.state.industries {
        let center = iso(industry.pos.x, industry.pos.y);
        let color = match industry.kind {
            IndustryKind::CoalMine => Color::srgb(1.0, 0.9, 0.1),
            IndustryKind::Forest => Color::srgb(1.0, 0.5, 0.05),
        };
        gizmo_diamond(&mut gizmos, center, 30.0, 14.0, color);

        if industry.stock > 0 {
            let fill = industry.stock as f32 / industry.capacity as f32;
            let bar_w = 56.0 * fill;
            let bar_y = center.y - 12.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::WHITE,
            );
        }
    }
}

/// Dibuja contorno cian para cada estación + barra de income.
fn draw_stations(sim: Res<SimWorld>, mut gizmos: Gizmos) {
    for station in &sim.state.stations {
        let center = iso(station.pos.x, station.pos.y);
        gizmo_diamond(&mut gizmos, center, 26.0, 12.0, Color::srgb(0.0, 0.9, 0.9));

        if station.income > 0 {
            let fill = ((station.income as f32).log2() / 10.0).min(1.0);
            let bar_w = 48.0 * fill;
            let bar_y = center.y - 10.0;
            gizmos.line_2d(
                Vec2::new(center.x - bar_w / 2.0, bar_y),
                Vec2::new(center.x + bar_w / 2.0, bar_y),
                Color::srgb(1.0, 1.0, 0.0),
            );
        }
    }
}

/// Mueve la cámara con WASD, arrastre con botón derecho y rueda del ratón; velocidad acorde al zoom.
///
/// | Entrada              | Acción              |
/// |----------------------|---------------------|
/// | W/A/S/D o flechas    | Mover cámara        |
/// | Botón derecho + arrastre | Desplazar vista |
/// | + / =                | Zoom in             |
/// | -                    | Zoom out            |
/// | Rueda ratón          | Zoom hacia cursor   |
fn move_camera(
    time:   Res<Time>,
    kbd:    Res<ButtonInput<KeyCode>>,
    mouse:  Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = cam_q.single_mut() else { return };
    let Projection::Orthographic(ref mut proj) = *projection else { return };

    let speed = 300.0 * proj.scale * time.delta_secs();

    if mouse.pressed(MouseButton::Right) && motion.delta != Vec2::ZERO {
        let s = proj.scale * PAN_RMB_SCALE;
        // Horizontal: opuesto al ratón; vertical: igual que antes (sensación correcta).
        transform.translation.x -= motion.delta.x * s;
        transform.translation.y += motion.delta.y * s;
    }

    if kbd.pressed(KeyCode::KeyW) || kbd.pressed(KeyCode::ArrowUp) {
        transform.translation.y += speed;
    }
    if kbd.pressed(KeyCode::KeyS) || kbd.pressed(KeyCode::ArrowDown) {
        transform.translation.y -= speed;
    }
    if kbd.pressed(KeyCode::KeyA) || kbd.pressed(KeyCode::ArrowLeft) {
        transform.translation.x -= speed;
    }
    if kbd.pressed(KeyCode::KeyD) || kbd.pressed(KeyCode::ArrowRight) {
        transform.translation.x += speed;
    }

    // Zoom con teclado (mantener pulsado, hacia el centro)
    let z = ZOOM_KEY_RATE * time.delta_secs();
    if kbd.pressed(KeyCode::Equal) || kbd.pressed(KeyCode::NumpadAdd) {
        proj.scale = (proj.scale * (1.0 - z)).max(0.25);
    }
    if kbd.pressed(KeyCode::Minus) || kbd.pressed(KeyCode::NumpadSubtract) {
        proj.scale = (proj.scale * (1.0 + z)).min(20.0);
    }

    // Zoom con rueda del ratón hacia la posición del cursor
    if scroll.delta.y.abs() > 0.0 {
        let Ok(window) = windows.single() else { return };
        let Some(cursor_pos) = window.cursor_position() else { return };

        // Convertir posición del cursor (esquina sup-izq) a offset desde el centro de ventana
        let window_size = Vec2::new(window.width(), window.height());
        let cursor_offset = cursor_pos - window_size / 2.0;
        // En Bevy Y-up, el cursor tiene Y invertido respecto al mundo
        let cursor_offset_world = Vec2::new(cursor_offset.x, -cursor_offset.y);

        // Posición del mundo bajo el cursor antes del zoom
        let world_pos = Vec2::new(transform.translation.x, transform.translation.y)
            + cursor_offset_world * proj.scale;

        // Nuevo scale
        let old_scale = proj.scale;
        let new_scale =
            (old_scale * (1.0 - scroll.delta.y * ZOOM_WHEEL_SENS)).clamp(0.25, 20.0);
        proj.scale = new_scale;

        // Ajustar cámara para que el punto bajo el cursor no se mueva
        let new_cam_pos = world_pos - cursor_offset_world * new_scale;
        transform.translation.x = new_cam_pos.x;
        transform.translation.y = new_cam_pos.y;
    }
}

// ── UI de información de tile ─────────────────────────────────────────────────

/// Crea el texto de información del tile (flotante, sigue a la cámara).
fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new("Click en tile para ver info"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.8)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
    ));
}

/// Detecta click izquierdo y actualiza el tile seleccionado.
fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), With<Camera2d>>,
    mut selected: ResMut<SelectedTileInfo>,
    sim: Res<SimWorld>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
    let Ok((cam_transform, projection)) = cam_q.single() else { return };
    let Projection::Orthographic(proj) = projection else { return };

    // Convertir posición del cursor a coordenadas del mundo
    let window_size = Vec2::new(window.width(), window.height());
    let cursor_offset = cursor_pos - window_size / 2.0;
    let cursor_offset_world = Vec2::new(cursor_offset.x, -cursor_offset.y);
    let world_pos = Vec2::new(cam_transform.translation.x, cam_transform.translation.y)
        + cursor_offset_world * proj.scale;

    // Convertir a coordenadas de tile
    let (tx, ty) = world_to_tile(world_pos);
    let (mw, mh) = sim.state.map.dimensions();

    if tx >= 0 && ty >= 0 && tx < mw as i32 && ty < mh as i32 {
        selected.pos = Some(TileCoord::new(tx, ty));
    } else {
        selected.pos = None;
    }
}

/// Actualiza el texto de información del tile seleccionado.
fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    cam_q: Query<(&Transform, &Projection), With<Camera2d>>,
    mut text_q: Query<(&mut Text2d, &mut Transform), (With<TileInfoText>, Without<Camera2d>)>,
) {
    let Ok((mut text, mut text_transform)) = text_q.single_mut() else { return };
    let Ok((cam_transform, projection)) = cam_q.single() else { return };
    let Projection::Orthographic(proj) = projection else { return };

    // Posicionar el texto en la esquina superior izquierda de la vista
    let offset_x = -580.0 * proj.scale;
    let offset_y = 320.0 * proj.scale;
    text_transform.translation.x = cam_transform.translation.x + offset_x;
    text_transform.translation.y = cam_transform.translation.y + offset_y;
    text_transform.scale = Vec3::splat(proj.scale);

    let Some(pos) = selected.pos else {
        **text = "Click en tile para ver info".to_string();
        return;
    };

    let Some(tile) = sim.state.map.get(pos) else {
        **text = format!("({}, {}): fuera del mapa", pos.x, pos.y);
        return;
    };

    let kind_str = match tile.kind {
        TileKind::Void => "Void",
        TileKind::Grass => "Grass",
        TileKind::Water => "Water",
        TileKind::Road => "Road",
        TileKind::Rail => "Rail",
        TileKind::House => "House",
        TileKind::Industry => "Industry",
        TileKind::Station => "Station",
        TileKind::Forest => "Forest",
        TileKind::CoalField => "CoalField",
        TileKind::Unknown(n) => {
            **text = format!("({}, {}): Unknown({})", pos.x, pos.y, n);
            return;
        }
    };

    let extra = if tile.kind == TileKind::Road {
        let rb = road_bits_for_render(&sim.state.map, pos, sim.state.map.dimensions().0, sim.state.map.dimensions().1);
        format!(" rb:0x{:02X}", rb)
    } else {
        String::new()
    };

    **text = format!(
        "Tile ({},{}) {}\nh:{} mapt:0x{:02X} m5:0x{:02X}{}",
        pos.x, pos.y, kind_str, tile.height, tile.mapt, tile.m5, extra
    );
}

// ── Utilidades ────────────────────────────────────────────────────────────────

/// Hash de Wang para generar variación determinista (sin RNG en el core).
fn wang_hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = seed
        .wrapping_add(x.wrapping_mul(0x9E37_79B9))
        .wrapping_add(y.wrapping_mul(0x6C62_272E));
    h ^= h >> 16;
    h = h.wrapping_mul(0x45D9_F3B);
    h ^= h >> 16;
    h
}
