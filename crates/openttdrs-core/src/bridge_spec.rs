//! Especificaciones de puentes vanilla (`_orig_bridge[]` en `bridge_land.h`).
//!
//! El catálogo mutable [`BridgeSpecDef`] admite overrides Action0 `Bridges` (`0x06`).

use crate::map::{Tile, TileCoord};
use crate::rail_signals::calendar_year_at_tick;
use crate::tick::GameTick;

/// Línea recta entre dos teselas (misma regla que el arrastre de puente).
#[must_use]
pub fn axis_line(a: TileCoord, b: TileCoord) -> Vec<TileCoord> {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        let step = if b.x >= a.x { 1 } else { -1 };
        let mut out = Vec::new();
        let mut x = a.x;
        loop {
            out.push(TileCoord::new(x, a.y));
            if x == b.x {
                break;
            }
            x += step;
        }
        out
    } else {
        let step = if b.y >= a.y { 1 } else { -1 };
        let mut out = Vec::new();
        let mut y = a.y;
        loop {
            out.push(TileCoord::new(a.x, y));
            if y == b.y {
                break;
            }
            y += step;
        }
        out
    }
}

/// Índice de tipo de puente (`BridgeType` en `OpenTTD`); 13 tipos en el juego base.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default,
)]
#[repr(u8)]
pub enum BridgeType {
    #[default]
    Wooden = 0,
    Concrete = 1,
    GirderSteel = 2,
    SuspensionConcrete = 3,
    SuspensionSteel = 4,
    SuspensionSteelYellow = 5,
    CantileverSteel = 6,
    CantileverBrown = 7,
    CantileverRed = 8,
    GirderSteelAlt = 9,
    TubularSteel = 10,
    TubularYellow = 11,
    TubularSilicon = 12,
}

impl BridgeType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Wooden),
            1 => Some(Self::Concrete),
            2 => Some(Self::GirderSteel),
            3 => Some(Self::SuspensionConcrete),
            4 => Some(Self::SuspensionSteel),
            5 => Some(Self::SuspensionSteelYellow),
            6 => Some(Self::CantileverSteel),
            7 => Some(Self::CantileverBrown),
            8 => Some(Self::CantileverRed),
            9 => Some(Self::GirderSteelAlt),
            10 => Some(Self::TubularSteel),
            11 => Some(Self::TubularYellow),
            12 => Some(Self::TubularSilicon),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Metadatos de un tipo de puente (tabla `_orig_bridge`).
#[derive(Debug, Clone, Copy)]
pub struct BridgeSpec {
    pub bridge_type: BridgeType,
    /// Año calendario mínimo de disponibilidad.
    pub available_from_year: u32,
    /// Longitud mínima del vano central (sin rampas).
    pub min_middle_len: u16,
    /// Longitud máxima del vano central; `None` = sin límite.
    pub max_middle_len: Option<u16>,
    /// Multiplicador de precio base (`Price::BuildBridge` × factor de longitud).
    pub price_mult: u16,
    /// Velocidad máxima en km/h (× 1.6 en `OpenTTD` internamente).
    pub max_speed: u16,
    /// Nombre corto para UI.
    pub name: &'static str,
}

/// Spec de puente owned (catálogo `GameState` + overrides `NewGRF`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BridgeSpecDef {
    pub bridge_type: BridgeType,
    pub available_from_year: u32,
    pub min_middle_len: u16,
    pub max_middle_len: Option<u16>,
    pub price_mult: u16,
    pub max_speed: u16,
    pub name: String,
    pub from_newgrf: bool,
    pub grfid: u32,
    /// Action0 prop `0x0D` (tablas de sprites) presente en algún override.
    #[serde(default)]
    pub has_custom_sprites: bool,
}

impl BridgeSpecDef {
    #[must_use]
    pub fn from_vanilla(spec: &BridgeSpec) -> Self {
        Self {
            bridge_type: spec.bridge_type,
            available_from_year: spec.available_from_year,
            min_middle_len: spec.min_middle_len,
            max_middle_len: spec.max_middle_len,
            price_mult: spec.price_mult,
            max_speed: spec.max_speed,
            name: spec.name.to_string(),
            from_newgrf: false,
            grfid: 0,
            has_custom_sprites: false,
        }
    }
}

/// Catálogo de los 13 slots vanilla (clonado de [`BRIDGE_SPECS`]).
#[must_use]
pub fn vanilla_bridge_spec_catalog() -> Vec<BridgeSpecDef> {
    BRIDGE_SPECS
        .iter()
        .map(BridgeSpecDef::from_vanilla)
        .collect()
}

/// Spec del tipo en el catálogo (índice = `BridgeType`).
#[must_use]
pub fn bridge_spec_def(catalog: &[BridgeSpecDef], bt: BridgeType) -> Option<&BridgeSpecDef> {
    catalog.get(usize::from(bt.as_u8()))
}

/// Disponibilidad según catálogo (año + longitud de vano).
#[must_use]
pub fn bridge_available_in(
    catalog: &[BridgeSpecDef],
    bt: BridgeType,
    year: u32,
    middle_len: u16,
) -> bool {
    let Some(spec) = bridge_spec_def(catalog, bt) else {
        return false;
    };
    if year < spec.available_from_year {
        return false;
    }
    if middle_len < spec.min_middle_len {
        return false;
    }
    if let Some(max) = spec.max_middle_len
        && middle_len > max
    {
        return false;
    }
    true
}

/// Disponibilidad en el tick actual contra el catálogo.
#[must_use]
pub fn bridge_available_at_tick_in(
    catalog: &[BridgeSpecDef],
    bt: BridgeType,
    tick: GameTick,
    start: TileCoord,
    end: TileCoord,
) -> bool {
    let year = calendar_year_at_tick(tick);
    let middle = bridge_middle_length(start, end);
    bridge_available_in(catalog, bt, year, middle)
}

/// Coste de construcción según catálogo.
#[must_use]
pub fn bridge_build_cost_in(
    catalog: &[BridgeSpecDef],
    bt: BridgeType,
    start: TileCoord,
    end: TileCoord,
) -> i64 {
    let total = i64::from(bridge_total_length(start, end));
    let factor = total.saturating_add(1);
    let mult =
        bridge_spec_def(catalog, bt).map_or_else(|| bridge_spec(bt).price_mult, |s| s.price_mult);
    i64::from(mult) * factor
}

/// Los 13 puentes del juego base (`OpenTTD/src/table/bridge_land.h`).
pub const BRIDGE_SPECS: [BridgeSpec; 13] = [
    BridgeSpec {
        bridge_type: BridgeType::Wooden,
        available_from_year: 0,
        min_middle_len: 0,
        max_middle_len: None,
        price_mult: 80,
        max_speed: 32,
        name: "Madera",
    },
    BridgeSpec {
        bridge_type: BridgeType::Concrete,
        available_from_year: 0,
        min_middle_len: 0,
        max_middle_len: Some(2),
        price_mult: 112,
        max_speed: 48,
        name: "Hormigón",
    },
    BridgeSpec {
        bridge_type: BridgeType::GirderSteel,
        available_from_year: 1930,
        min_middle_len: 0,
        max_middle_len: Some(5),
        price_mult: 144,
        max_speed: 64,
        name: "Viga de acero",
    },
    BridgeSpec {
        bridge_type: BridgeType::SuspensionConcrete,
        available_from_year: 0,
        min_middle_len: 2,
        max_middle_len: Some(10),
        price_mult: 168,
        max_speed: 80,
        name: "Colgante (hormigón)",
    },
    BridgeSpec {
        bridge_type: BridgeType::SuspensionSteel,
        available_from_year: 1930,
        min_middle_len: 3,
        max_middle_len: None,
        price_mult: 185,
        max_speed: 96,
        name: "Colgante (acero)",
    },
    BridgeSpec {
        bridge_type: BridgeType::SuspensionSteelYellow,
        available_from_year: 1930,
        min_middle_len: 3,
        max_middle_len: None,
        price_mult: 192,
        max_speed: 112,
        name: "Colgante (acero amarillo)",
    },
    BridgeSpec {
        bridge_type: BridgeType::CantileverSteel,
        available_from_year: 1930,
        min_middle_len: 3,
        max_middle_len: Some(7),
        price_mult: 224,
        max_speed: 160,
        name: "Cantilever (acero)",
    },
    BridgeSpec {
        bridge_type: BridgeType::CantileverBrown,
        available_from_year: 1930,
        min_middle_len: 3,
        max_middle_len: Some(8),
        price_mult: 232,
        max_speed: 208,
        name: "Cantilever (marrón)",
    },
    BridgeSpec {
        bridge_type: BridgeType::CantileverRed,
        available_from_year: 1930,
        min_middle_len: 3,
        max_middle_len: Some(9),
        price_mult: 248,
        max_speed: 240,
        name: "Cantilever (rojo)",
    },
    BridgeSpec {
        bridge_type: BridgeType::GirderSteelAlt,
        available_from_year: 1930,
        min_middle_len: 0,
        max_middle_len: Some(2),
        price_mult: 240,
        max_speed: 256,
        name: "Viga de acero (II)",
    },
    BridgeSpec {
        bridge_type: BridgeType::TubularSteel,
        available_from_year: 1995,
        min_middle_len: 2,
        max_middle_len: None,
        price_mult: 255,
        max_speed: 320,
        name: "Tubular (acero)",
    },
    BridgeSpec {
        bridge_type: BridgeType::TubularYellow,
        available_from_year: 2005,
        min_middle_len: 2,
        max_middle_len: None,
        price_mult: 380,
        max_speed: 512,
        name: "Tubular (amarillo)",
    },
    BridgeSpec {
        bridge_type: BridgeType::TubularSilicon,
        available_from_year: 2010,
        min_middle_len: 2,
        max_middle_len: None,
        price_mult: 510,
        max_speed: 608,
        name: "Tubular (silicio)",
    },
];

#[must_use]
pub fn bridge_spec(bt: BridgeType) -> &'static BridgeSpec {
    &BRIDGE_SPECS[bt.as_u8() as usize]
}

/// Longitud del vano central (teselas entre rampas), como `GetTunnelBridgeLength`.
#[must_use]
pub fn bridge_middle_length(start: TileCoord, end: TileCoord) -> u16 {
    u16::try_from(
        axis_line(start, end)
            .len()
            .saturating_sub(2)
            .min(usize::from(u16::MAX)),
    )
    .unwrap_or(u16::MAX)
}

/// Longitud total del tramo incluyendo rampas.
#[must_use]
pub fn bridge_total_length(start: TileCoord, end: TileCoord) -> u16 {
    u16::try_from(axis_line(start, end).len().min(usize::from(u16::MAX))).unwrap_or(u16::MAX)
}

/// Disponibilidad contra la tabla estática vanilla ([`BRIDGE_SPECS`]).
#[must_use]
pub fn bridge_available(bt: BridgeType, year: u32, middle_len: u16) -> bool {
    let spec = bridge_spec(bt);
    if year < spec.available_from_year {
        return false;
    }
    if middle_len < spec.min_middle_len {
        return false;
    }
    if let Some(max) = spec.max_middle_len
        && middle_len > max
    {
        return false;
    }
    true
}

#[must_use]
pub fn bridge_available_at_tick(
    bt: BridgeType,
    tick: GameTick,
    start: TileCoord,
    end: TileCoord,
) -> bool {
    let year = calendar_year_at_tick(tick);
    let middle = bridge_middle_length(start, end);
    bridge_available(bt, year, middle)
}

/// Coste aproximado del puente vanilla (`CalcBridgeLenCostFactor` × `price_mult`).
#[must_use]
pub fn bridge_build_cost(bt: BridgeType, start: TileCoord, end: TileCoord) -> i64 {
    let total = i64::from(bridge_total_length(start, end));
    let factor = total.saturating_add(1);
    i64::from(bridge_spec(bt).price_mult) * factor
}

/// Tipo de puente en bits 2–5 de `m6` (`GetBridgeType`).
#[must_use]
pub fn bridge_type_from_m6(m6: u8) -> BridgeType {
    BridgeType::from_u8((m6 >> 2) & 0x0F).unwrap_or(BridgeType::Wooden)
}

/// Velocidad máxima del puente en la tesela (`None` si no hay puente).
///
/// Aplica a rampas `RailBridge` / `RoadBridge`. El vano central sigue siendo
/// `Water` con marca `mapt` (pathfinder aún no lo cruza).
#[must_use]
pub fn bridge_max_speed_for_tile(map: &crate::map::Map, pos: TileCoord) -> Option<u16> {
    let tile = map.get(pos)?;
    match tile.kind {
        crate::map::TileKind::RailBridge | crate::map::TileKind::RoadBridge => {
            Some(bridge_spec(bridge_type_from_m6(tile.m6)).max_speed)
        }
        _ => None,
    }
}

#[must_use]
pub fn set_bridge_type_m6(m6: u8, bt: BridgeType) -> u8 {
    (m6 & 0xC3) | ((bt.as_u8() & 0x0F) << 2)
}

/// Marca un vano de puente sobre la tesela (`SetBridgeMiddle` en `bridge_map.h`).
#[must_use]
pub fn set_bridge_middle_mapt(mapt: u8, axis_y: bool) -> u8 {
    let above = if axis_y { 0x08 } else { 0x04 };
    (mapt & !0x0C) | above
}

/// Eje del puente sobre la tesela (bits 2–3 de `mapt`): `None`, `Some(false)` = X, `Some(true)` = Y.
#[must_use]
pub fn bridge_above_axis_from_mapt(mapt: u8) -> Option<bool> {
    match (mapt >> 2) & 0x3 {
        1 => Some(false),
        2 => Some(true),
        _ => None,
    }
}

/// Reserva PBS de una rampa ferroviaria de túnel o puente.
///
/// A diferencia de una tesela `MP_RAILWAY` común, `OpenTTD` guarda este estado
/// en el bit 4 de `m5` (`HasTunnelBridgeReservation`), no en el byte alto de
/// `MAP2`. Mantenerlo aquí evita que los consumidores del `.sav` mezclen ambos
/// formatos de reserva.
#[must_use]
pub fn tunnel_bridge_rail_reserved(tile: Tile) -> bool {
    tile.is_tunnel_bridge_tile() && (tile.m5 & 0x0C) == 0 && (tile.m5 & 0x10) != 0
}

/// Otra rampa del puente `kind`, siguiendo la dirección persistida en `m5`.
///
/// `OpenTTD` no identifica el vano por el tipo de terreno inferior: un puente
/// puede cruzar tierra, vías, agua o una ciudad. `GetOtherBridgeEnd` avanza en
/// la dirección de la rampa hasta hallar una rampa de puente con dirección
/// opuesta. El enfoque anterior (solo agua con `IsBridgeAbove`) hacía que
/// puentes válidos parecieran cortados o conectados a una rampa lateral.
#[must_use]
fn bridge_other_end(
    map: &crate::map::Map,
    ramp: TileCoord,
    kind: crate::map::TileKind,
) -> Option<TileCoord> {
    let tile = map.get(ramp)?;
    if tile.kind != kind || !tile.is_tunnel_bridge_tile() || tile.m5 & 0x80 == 0 {
        return None;
    }
    let (map_w, map_h) = map.dimensions();
    let (step_x, step_y) = crate::map::diag_dir_offset(tile.m5 & 0x03);
    let reverse_direction = (tile.m5.wrapping_add(2)) & 0x03;
    let mut pos = ramp;
    // La longitud no puede exceder una dimensión del mapa; el límite también
    // evita un loop infinito si un save corrupto tiene una rampa huérfana.
    for _ in 0..map_w.max(map_h) {
        pos = TileCoord::new(pos.x + step_x, pos.y + step_y);
        let probe = map.get(pos)?;
        if probe.is_tunnel_bridge_tile()
            && probe.m5 & 0x80 != 0
            && probe.m5 & 0x03 == reverse_direction
        {
            return Some(pos);
        }
    }
    None
}

/// Otra rampa de un puente de carretera (`RoadBridge`), saltando el vano `Water`.
///
/// El pathfinder usa esto como wormhole (#187): el vano central sigue siendo agua.
#[must_use]
pub fn road_bridge_other_end(map: &crate::map::Map, ramp: TileCoord) -> Option<TileCoord> {
    bridge_other_end(map, ramp, crate::map::TileKind::RoadBridge)
}

/// Otra rampa de un puente ferroviario (`RailBridge`), saltando el vano `Water`.
///
/// Igual que carretera, la geometría visible del tablero no reemplaza el terreno
/// subyacente. El pathfinder ferroviario y PBS tratan las rampas como un enlace
/// lógico directo.
#[must_use]
pub fn rail_bridge_other_end(map: &crate::map::Map, ramp: TileCoord) -> Option<TileCoord> {
    bridge_other_end(map, ramp, crate::map::TileKind::RailBridge)
}

/// Pieza de vano según distancia a cada rampa (`CalcBridgePiece` en `tunnelbridge_cmd.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgePiece {
    North,
    South,
    InnerNorth,
    InnerSouth,
    MiddleOdd,
    MiddleEven,
}

#[must_use]
pub fn calc_bridge_piece(north_len: u32, south_len: u32) -> BridgePiece {
    if north_len == 1 {
        BridgePiece::North
    } else if south_len == 1 {
        BridgePiece::South
    } else if north_len < south_len {
        if north_len & 1 != 0 {
            BridgePiece::InnerSouth
        } else {
            BridgePiece::InnerNorth
        }
    } else if north_len > south_len {
        if south_len & 1 != 0 {
            BridgePiece::InnerNorth
        } else {
            BridgePiece::InnerSouth
        }
    } else if north_len & 1 != 0 {
        BridgePiece::MiddleEven
    } else {
        BridgePiece::MiddleOdd
    }
}

/// Teselas del tramo completo (incluye rampas).
#[must_use]
pub fn bridge_line_tiles(start: TileCoord, end: TileCoord) -> Vec<TileCoord> {
    axis_line(start, end)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::{GameState, TileKind};

    #[test]
    fn rail_reservation_uses_tunnel_bridge_m5_bit_not_map2() {
        let mut tile = Tile {
            height: 0,
            kind: TileKind::RailBridge,
            mapt: 0x90,
            m5: 0x80,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0x3F,
            m7: 0,
            m3hi: 0,
        };
        assert!(!tunnel_bridge_rail_reserved(tile));

        tile.m5 |= 0x10;
        assert!(tunnel_bridge_rail_reserved(tile));

        tile.kind = TileKind::RoadBridge;
        tile.m5 = 0x80 | 0x04 | 0x10;
        assert!(!tunnel_bridge_rail_reserved(tile));
    }

    #[test]
    fn wooden_always_available() {
        assert!(bridge_available(BridgeType::Wooden, 1950, 0));
    }

    #[test]
    fn concrete_max_middle_two() {
        assert!(bridge_available(BridgeType::Concrete, 1950, 2));
        assert!(!bridge_available(BridgeType::Concrete, 1950, 3));
    }

    #[test]
    fn tubular_requires_year() {
        assert!(!bridge_available(BridgeType::TubularSteel, 1990, 5));
        assert!(bridge_available(BridgeType::TubularSteel, 2000, 5));
    }

    #[test]
    fn middle_length_excludes_ramps() {
        let a = TileCoord::new(0, 0);
        let b = TileCoord::new(4, 0);
        assert_eq!(bridge_total_length(a, b), 5);
        assert_eq!(bridge_middle_length(a, b), 3);
    }

    #[test]
    fn place_bridge_keeps_water_under_span() {
        let mut s = GameState::new(10, 10);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        for x in 2..=4 {
            s.map.set_kind(c(x, 2), TileKind::Water).unwrap();
        }
        apply_command(
            &mut s,
            &Command::PlaceRoadBridge(c(1, 2), c(5, 2), BridgeType::CantileverRed),
        )
        .unwrap();
        assert_eq!(s.map.get_kind(c(3, 2)), Some(TileKind::Water));
        assert!(bridge_above_axis_from_mapt(s.map.get(c(3, 2)).unwrap().mapt).is_some());
        assert_eq!(s.map.get_kind(c(1, 2)), Some(TileKind::RoadBridge));
        assert_eq!(
            bridge_type_from_m6(s.map.get(c(1, 2)).unwrap().m6),
            BridgeType::CantileverRed
        );
    }

    #[test]
    fn other_end_follows_encoded_direction_over_non_water_span() {
        let mut map = crate::Map::new_flat(8, 4, 0);
        let west = TileCoord::new(1, 1);
        let east = TileCoord::new(5, 1);
        let mut west_tile = map.get(west).unwrap();
        west_tile.kind = TileKind::RoadBridge;
        west_tile.mapt = 0x90;
        // DiagDirection::SW: +X, transport road, bridge flag.
        west_tile.m5 = 0x80 | 0x04 | 0x02;
        map.set_tile(west, west_tile).unwrap();
        let mut east_tile = map.get(east).unwrap();
        east_tile.kind = TileKind::RoadBridge;
        east_tile.mapt = 0x90;
        // Dirección opuesta NE: -X.
        east_tile.m5 = 0x80 | 0x04;
        map.set_tile(east, east_tile).unwrap();

        // El vano queda sobre tierra: no depende de `Water` ni de MAPT flags.
        assert_eq!(road_bridge_other_end(&map, west), Some(east));
        assert_eq!(road_bridge_other_end(&map, east), Some(west));
    }

    #[test]
    fn train_on_wooden_bridge_is_speed_capped() {
        use crate::command::{Command, apply_command};
        use crate::vehicle::{Vehicle, VehicleKind};

        let mut s = GameState::new(16, 8);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        for x in 2..=5 {
            s.map.set_kind(c(x, 4), TileKind::Water).unwrap();
        }
        let west = c(1, 4);
        let east = c(6, 4);
        apply_command(
            &mut s,
            &Command::PlaceRailBridge(west, east, BridgeType::Wooden),
        )
        .expect("puente madera");
        let wood_cap = bridge_spec(BridgeType::Wooden).max_speed;
        assert_eq!(wood_cap, 32);
        assert_eq!(bridge_max_speed_for_tile(&s.map, west), Some(32));

        let mut train = Vehicle::new(1, VehicleKind::Train, west, east);
        train.running = true;
        train.path.push_back(east);
        train.set_cruise_speed();
        assert!(train.cur_speed > wood_cap, "motor más rápido que el puente");
        s.vehicles.push(train);

        // `DoUpdateSpeed` baja gradualmente hasta el tope (no clamp duro).
        let mut capped = false;
        for _ in 0..200 {
            s.step();
            if s.vehicles[0].cur_speed <= wood_cap {
                capped = true;
                break;
            }
        }
        assert!(
            capped,
            "velocidad {} no bajó al tope de puente {}",
            s.vehicles[0].cur_speed, wood_cap
        );
        for _ in 0..20 {
            s.step();
            assert!(
                s.vehicles[0].cur_speed <= wood_cap,
                "velocidad {} supera tope de puente {}",
                s.vehicles[0].cur_speed,
                wood_cap
            );
        }
    }
}
