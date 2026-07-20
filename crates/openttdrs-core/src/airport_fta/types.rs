//! Tipos FTA / `MovingData` (`airport.h`).

use crate::airport_class::AirportSpecId;

/// Flags de `AirportMovingDataFlag` como bits en `u16`.
pub const FLAG_NO_SPEED_CLAMP: u16 = 1 << 0;
pub const FLAG_TAKEOFF: u16 = 1 << 1;
pub const FLAG_SLOW_TURN: u16 = 1 << 2;
pub const FLAG_LAND: u16 = 1 << 3;
pub const FLAG_EXACT: u16 = 1 << 4;
pub const FLAG_BRAKE: u16 = 1 << 5;
pub const FLAG_HELI_RAISE: u16 = 1 << 6;
pub const FLAG_HELI_LOWER: u16 = 1 << 7;
pub const FLAG_HOLD: u16 = 1 << 8;

/// Heading / estado de movimiento aeroportuario (`AirportMovementStates`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum AirportHeading {
    #[default]
    ToAll = 0,
    Hangar = 1,
    Term1 = 2,
    Term2 = 3,
    Term3 = 4,
    Term4 = 5,
    Term5 = 6,
    Term6 = 7,
    Helipad1 = 8,
    Helipad2 = 9,
    Takeoff = 10,
    StartTakeoff = 11,
    EndTakeoff = 12,
    HeliTakeoff = 13,
    Flying = 14,
    Landing = 15,
    EndLanding = 16,
    HeliLanding = 17,
    HeliEndLanding = 18,
    Term7 = 19,
    Term8 = 20,
    Helipad3 = 21,
    /// Buscar terminal libre (`TERMGROUP = 255`).
    TermGroup = 255,
}

impl AirportHeading {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Hangar,
            2 => Self::Term1,
            3 => Self::Term2,
            4 => Self::Term3,
            5 => Self::Term4,
            6 => Self::Term5,
            7 => Self::Term6,
            8 => Self::Helipad1,
            9 => Self::Helipad2,
            10 => Self::Takeoff,
            11 => Self::StartTakeoff,
            12 => Self::EndTakeoff,
            13 => Self::HeliTakeoff,
            14 => Self::Flying,
            15 => Self::Landing,
            16 => Self::EndLanding,
            17 => Self::HeliLanding,
            18 => Self::HeliEndLanding,
            19 => Self::Term7,
            20 => Self::Term8,
            21 => Self::Helipad3,
            255 => Self::TermGroup,
            _ => Self::ToAll,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Bitset de bloques (`AirportBlocks`, `u64`).
pub type AirportBlockBits = u64;

/// Bit de `AirportBlock::AirportBusy` / `RunwayInOut` (índice 8).
pub const BLOCK_AIRPORT_BUSY: AirportBlockBits = 1 << 8;
/// Bit de `AirportBlock::Term1`.
pub const BLOCK_TERM1: AirportBlockBits = 1 << 0;
/// Bit de `AirportBlock::Term2`.
pub const BLOCK_TERM2: AirportBlockBits = 1 << 1;
/// Bit de `AirportBlock::Term3`.
pub const BLOCK_TERM3: AirportBlockBits = 1 << 2;
/// Bit de `AirportBlock::Helipad1` (índice 6).
pub const BLOCK_HELIPAD1: AirportBlockBits = 1 << 6;
/// Bit de `AirportBlock::Helipad2` (índice 7).
pub const BLOCK_HELIPAD2: AirportBlockBits = 1 << 7;
/// Bit de `AirportBlock::TaxiwayBusy` (índice 10).
pub const BLOCK_TAXIWAY_BUSY: AirportBlockBits = 1 << 10;
/// Bit de `AirportBlock::OutWay` (índice 11).
pub const BLOCK_OUT_WAY: AirportBlockBits = 1 << 11;
/// Bit de `AirportBlock::InWay` (índice 12).
pub const BLOCK_IN_WAY: AirportBlockBits = 1 << 12;
/// Bit de `AirportBlock::AirportEntrance` (índice 13).
pub const BLOCK_AIRPORT_ENTRANCE: AirportBlockBits = 1 << 13;
/// Bit de `AirportBlock::Hangar2Area` (índice 16).
pub const BLOCK_HANGAR2_AREA: AirportBlockBits = 1 << 16;
/// Bit de `AirportBlock::PreHelipad` (índice 21).
pub const BLOCK_PRE_HELIPAD: AirportBlockBits = 1 << 21;

/// Waypoint de movimiento (`AirportMovingData`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportMovingData {
    pub x: i16,
    pub y: i16,
    pub flags: AirportMovingDataFlags,
    /// Dirección `OpenTTD` 0..7 al llegar.
    pub direction: u8,
}

/// Alias de flags empaquetados.
pub type AirportMovingDataFlags = u16;

/// Arista FTA desde una posición (`AirportFTA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportFtaEdge {
    pub position: u8,
    pub heading: AirportHeading,
    pub blocks: AirportBlockBits,
    pub next_position: u8,
}

/// Variante de tablas FTA soportadas en este corte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirportFtaKind {
    Country,
    Helidepot,
    Commuter,
    City,
}

/// Descriptor de tablas + comportamiento especial por aeropuerto.
#[derive(Debug, Clone, Copy)]
pub struct AirportFtaProfile {
    pub kind: AirportFtaKind,
    pub spec: AirportSpecId,
    pub moving_data: &'static [AirportMovingData],
    pub entries: [u8; 4],
    pub fta_edges: fn(u8) -> Vec<AirportFtaEdge>,
    /// Nodo de despegue ala fija (Country = 9).
    pub fixedwing_takeoff_pos: Option<u8>,
    /// Nodos de hold en aire.
    pub hold_min: u8,
    pub hold_max: u8,
    /// Ancho/alto del footprint en teselas (clamp de pose).
    pub footprint_w: i32,
    pub footprint_h: i32,
}
