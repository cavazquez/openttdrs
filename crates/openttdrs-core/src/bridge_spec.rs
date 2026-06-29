//! Especificaciones de puentes vanilla (`_orig_bridge[]` en `bridge_land.h`).

use crate::map::TileCoord;
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

/// Coste aproximado del puente (`CalcBridgeLenCostFactor` × `price_mult`).
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

#[must_use]
pub fn set_bridge_type_m6(m6: u8, bt: BridgeType) -> u8 {
    (m6 & 0xC3) | ((bt.as_u8() & 0x0F) << 2)
}

/// Teselas del tramo completo (incluye rampas).
#[must_use]
pub fn bridge_line_tiles(start: TileCoord, end: TileCoord) -> Vec<TileCoord> {
    axis_line(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
