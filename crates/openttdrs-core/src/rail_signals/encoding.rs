//! Codificación y edición de señales en tiles (`m2`/`m3`/`m3hi`).

use crate::map::{
    RAIL_TB_HORZ, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER, RAIL_TB_VERT,
    RAIL_TB_X, RAIL_TB_Y,
};
use crate::news::{CALENDAR_BASE_YEAR, calendar_day_index, calendar_year_day};
use crate::tick::GameTick;

/// Pieza de vía sobre la que se coloca una señal (`Track` en `track_type.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SignalTrack {
    X = 0,
    Y = 1,
    Upper = 2,
    Lower = 3,
    Left = 4,
    Right = 5,
}

impl SignalTrack {
    #[must_use]
    pub const fn track_bit(self) -> u8 {
        match self {
            Self::X => RAIL_TB_X,
            Self::Y => RAIL_TB_Y,
            Self::Upper => RAIL_TB_UPPER,
            Self::Lower => RAIL_TB_LOWER,
            Self::Left => RAIL_TB_LEFT,
            Self::Right => RAIL_TB_RIGHT,
        }
    }

    /// Índice `Track` de `OpenTTD` (0..5).
    #[must_use]
    pub const fn from_u8(track: u8) -> Option<Self> {
        match track {
            0 => Some(Self::X),
            1 => Some(Self::Y),
            2 => Some(Self::Upper),
            3 => Some(Self::Lower),
            4 => Some(Self::Left),
            5 => Some(Self::Right),
            _ => None,
        }
    }

    #[must_use]
    const fn from_track_bit(bit: u8) -> Option<Self> {
        match bit {
            RAIL_TB_X => Some(Self::X),
            RAIL_TB_Y => Some(Self::Y),
            RAIL_TB_UPPER => Some(Self::Upper),
            RAIL_TB_LOWER => Some(Self::Lower),
            RAIL_TB_LEFT => Some(Self::Left),
            RAIL_TB_RIGHT => Some(Self::Right),
            _ => None,
        }
    }

    /// `(DiagDir, sig_bit)` — alineado con `DrawSignals` / `_signal_along_trackdir`.
    #[must_use]
    const fn facings(self) -> &'static [(u8, u8)] {
        match self {
            Self::X => &[(0, 2), (2, 3)],
            Self::Y | Self::Left => &[(3, 2), (1, 3)],
            Self::Upper => &[(3, 3), (0, 2)],
            Self::Lower => &[(2, 0), (1, 1)],
            Self::Right => &[(3, 0), (1, 1)],
        }
    }
}

/// Más de un carril incompatible en la tesela (no señales en cruces).
#[must_use]
pub fn tracks_overlap(bits: u8) -> bool {
    if bits.count_ones() <= 1 {
        return false;
    }
    bits != RAIL_TB_HORZ && bits != RAIL_TB_VERT
}

/// Elige la pieza de vía bajo el cursor (`GenericPlaceSignals` en `rail_gui.cpp`).
#[must_use]
pub fn resolve_signal_track(trackbits: u8, fract_x: u8, fract_y: u8) -> Option<SignalTrack> {
    if tracks_overlap(trackbits) {
        return None;
    }
    let mut selected = trackbits;
    if selected & RAIL_TB_VERT != 0 {
        let pick = if fract_x <= fract_y {
            RAIL_TB_RIGHT
        } else {
            RAIL_TB_LEFT
        };
        if selected & pick == 0 {
            return None;
        }
        selected = pick;
    } else if selected & RAIL_TB_HORZ != 0 {
        let pick = if u16::from(fract_x) + u16::from(fract_y) <= 256 {
            RAIL_TB_UPPER
        } else {
            RAIL_TB_LOWER
        };
        if selected & pick == 0 {
            return None;
        }
        selected = pick;
    }
    SignalTrack::from_track_bit(selected)
}

/// Coste de colocación de una señal de bloque (`Price::BuildSignal` aprox.).
pub const SIGNAL_BUILD_COST: i64 = 40;
/// Reembolso parcial al quitar vía (`Price::ClearRail` aprox.).
pub const RAIL_REMOVE_REFUND: i64 = 10;
/// Reembolso al quitar una señal (mitad del coste de colocación).
pub const SIGNAL_REMOVE_REFUND: i64 = 20;

pub const SIGTYPE_BLOCK: u8 = 0;
pub const SIGTYPE_ENTRY: u8 = 1;
pub const SIGTYPE_EXIT: u8 = 2;
pub const SIGTYPE_COMBO: u8 = 3;
pub const SIGTYPE_PATH: u8 = 4;
pub const SIGTYPE_PATH_ONEWAY: u8 = 5;
pub const SIGTYPE_LAST_NOPBS: u8 = 3;

/// `true` si el tipo usa lógica PBS (`IsPbsSignal` en `OpenTTD`).
#[must_use]
pub const fn is_pbs_signal_type(sig_type: u8) -> bool {
    sig_type >= SIGTYPE_PATH
}

/// Siguiente tipo al ciclar con Ctrl (`CycleSignalType` en `rail_cmd.cpp`).
///
/// Orden `OpenTTD`: block → entry → exit → combo → path → path oneway → block.
#[must_use]
pub const fn next_placeable_signal_type(current: u8) -> u8 {
    match current {
        SIGTYPE_BLOCK => SIGTYPE_ENTRY,
        SIGTYPE_ENTRY => SIGTYPE_EXIT,
        SIGTYPE_EXIT => SIGTYPE_COMBO,
        SIGTYPE_COMBO => SIGTYPE_PATH,
        SIGTYPE_PATH => SIGTYPE_PATH_ONEWAY,
        _ => SIGTYPE_BLOCK,
    }
}

/// Nombre corto del tipo para UI / logs.
#[must_use]
pub const fn signal_type_label(sig_type: u8) -> &'static str {
    match sig_type {
        SIGTYPE_ENTRY => "entry",
        SIGTYPE_EXIT => "exit",
        SIGTYPE_COMBO => "combo",
        SIGTYPE_PATH => "path",
        SIGTYPE_PATH_ONEWAY => "path 1vía",
        _ => "block",
    }
}

/// Año calendario a partir del cual se colocan señales eléctricas por defecto
/// (`gui.semaphore_build_before` en `OpenTTD`).
pub const SEMAPHORE_BUILD_BEFORE_YEAR: u32 = CALENDAR_BASE_YEAR;

#[must_use]
pub fn calendar_year_at_tick(tick: GameTick) -> u32 {
    calendar_year_day(calendar_day_index(tick)).0
}

/// `0` = semáforo, `1` = eléctrica (`SignalVariant` en `OpenTTD`).
#[must_use]
pub fn default_signal_variant(year: u32) -> u8 {
    u8::from(year >= SEMAPHORE_BUILD_BEFORE_YEAR)
}

#[must_use]
pub fn rail_signal_present_mask(m3: u8) -> u8 {
    (m3 >> 4) & 0x0F
}

#[must_use]
pub fn rail_signal_state_mask(m3hi: u8) -> u8 {
    (m3hi >> 4) & 0x0F
}

#[must_use]
pub fn signal_is_green(m3hi: u8, sig_bit: u8) -> bool {
    (rail_signal_state_mask(m3hi) >> sig_bit) & 1 != 0
}

#[must_use]
pub fn signal_on_track_mask(track: SignalTrack) -> u8 {
    const MASKS: [u8; 6] = [0xC, 0xC, 0xC, 0x3, 0xC, 0x3];
    MASKS[track as usize]
}

#[must_use]
pub fn signal_type_for_track(m2: u8, track: SignalTrack) -> u8 {
    let base = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        0
    };
    (m2 >> base) & 7
}

/// Variante de señal en `m2` (`GetSignalVariant`: bit 3 o 7 según carril).
#[must_use]
pub fn signal_variant_for_track(m2: u8, track: SignalTrack) -> u8 {
    let bit = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        7
    } else {
        3
    };
    (m2 >> bit) & 1
}

/// Reemplaza el tipo PBS/block de una señal en `m2` (`SetSignalType` en `rail_map.h`).
#[must_use]
pub fn cycle_signal_type_m2(m2: u8, track: SignalTrack) -> u8 {
    let base = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        0
    };
    let current = signal_type_for_track(m2, track);
    let new_type = next_placeable_signal_type(current);
    (m2 & !(7 << base)) | ((new_type & 7) << base)
}

/// Limpia tipo y variante de señal del carril en `m2` al quitar la señal.
#[must_use]
pub fn clear_signal_type_bits_m2(m2: u8, track: SignalTrack) -> u8 {
    let (base, var_bit) = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        (4, 7)
    } else {
        (0, 3)
    };
    m2 & !(7 << base) & !(1 << var_bit)
}

/// Alterna one-way / two-way en el carril (`CycleSignalSide` en `rail_map.h`).
#[must_use]
pub fn cycle_signal_side_m3(m3: u8, track: SignalTrack, sig_type: u8) -> u8 {
    let pos = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        6
    };
    let mut side = (m3 >> pos) & 3;
    side = side.saturating_sub(1);
    if side == 0 {
        side = if sig_type > SIGTYPE_LAST_NOPBS { 2 } else { 3 };
    }
    (m3 & !(3 << pos)) | (side << pos)
}

#[must_use]
pub(crate) fn signal_track_for_bit(rails: u8, sig_bit: u8) -> Option<SignalTrack> {
    if tracks_overlap(rails) {
        return None;
    }
    if rails == RAIL_TB_HORZ {
        return Some(if sig_bit <= 1 {
            SignalTrack::Lower
        } else {
            SignalTrack::Upper
        });
    }
    if rails == RAIL_TB_VERT {
        return Some(if sig_bit <= 1 {
            SignalTrack::Right
        } else {
            SignalTrack::Left
        });
    }
    SignalTrack::from_track_bit(rails)
}

#[must_use]
pub(crate) fn signal_exit_dir(rails: u8, sig_bit: u8) -> u8 {
    let track = signal_track_for_bit(rails, sig_bit).or({
        if rails & RAIL_TB_Y != 0 {
            Some(SignalTrack::Y)
        } else if rails & RAIL_TB_X != 0 {
            Some(SignalTrack::X)
        } else {
            None
        }
    });
    let Some(track) = track else {
        return 0;
    };
    track
        .facings()
        .iter()
        .find(|(_, bit)| *bit == sig_bit)
        .map_or(0, |(face, _)| *face)
}

/// Datos de codificación de una señal de bloque en una tesela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalPlacement {
    pub sig_bit: u8,
    pub m2: u8,
    pub m3: u8,
    pub m3hi: u8,
}

/// Direcciones válidas para colocar señal en una pieza de vía (`DiagDir`).
#[must_use]
pub fn valid_signal_facings_track(track: SignalTrack) -> &'static [u8] {
    match track {
        SignalTrack::X => &[0, 2],
        SignalTrack::Y | SignalTrack::Left | SignalTrack::Right => &[3, 1],
        SignalTrack::Upper => &[3, 0],
        SignalTrack::Lower => &[2, 1],
    }
}

/// Elige la orientación de colocación más cercana a `orientation` (0..3).
#[must_use]
pub fn signal_facing_for_orientation(track: SignalTrack, orientation: u8) -> u8 {
    let facings = valid_signal_facings_track(track);
    let ori = orientation % 4;
    if let Some(f) = facings.iter().copied().find(|f| *f == ori) {
        return f;
    }
    facings.first().copied().unwrap_or(ori)
}

/// Siguiente orientación al rotar con RMB sobre vía.
#[must_use]
pub fn cycle_signal_facing(track: SignalTrack, current: u8) -> u8 {
    let facings = valid_signal_facings_track(track);
    if facings.is_empty() {
        return current % 4;
    }
    let cur = signal_facing_for_orientation(track, current);
    let idx = facings.iter().position(|&f| f == cur).unwrap_or(0);
    facings[(idx + 1) % facings.len()]
}

#[must_use]
pub fn signal_bit_for_facing(track: SignalTrack, face: u8) -> Option<u8> {
    track
        .facings()
        .iter()
        .find(|(f, _)| *f == face % 4)
        .map(|(_, bit)| *bit)
}

/// Codifica tipo y variante de señal en `m2` para un carril (`SetSignalType` / variante).
#[must_use]
pub fn m2_for_signal(sig_type: u8, variant: u8, track: SignalTrack) -> u8 {
    let (base, var_bit) = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        (4, 7)
    } else {
        (0, 3)
    };
    ((sig_type & 7) << base) | ((variant & 1) << var_bit)
}

/// Codifica una señal unidireccional (`sig_type`: block, path, path oneway).
#[must_use]
pub fn signal_placement_for_track(
    track: SignalTrack,
    face: u8,
    variant: u8,
    sig_type: u8,
) -> Option<SignalPlacement> {
    let sig_bit = signal_bit_for_facing(track, face)?;
    let present = 1 << sig_bit;
    Some(SignalPlacement {
        sig_bit,
        m2: m2_for_signal(sig_type, variant, track),
        m3: present << 4,
        m3hi: present << 4,
    })
}

/// Compatibilidad: resuelve la pieza con fracción centrada.
#[must_use]
pub fn signal_placement_for_facing(
    trackbits: u8,
    face: u8,
    variant: u8,
) -> Option<SignalPlacement> {
    let track = resolve_signal_track(trackbits, 128, 128)?;
    signal_placement_for_track(track, face, variant, SIGTYPE_BLOCK)
}

/// Compatibilidad con tests: señal por defecto en la primera dirección válida.
#[must_use]
pub fn encode_block_signal_on_track(trackbits: u8) -> (u8, u8, u8) {
    encode_block_signal_on_track_with_variant(trackbits, default_signal_variant(CALENDAR_BASE_YEAR))
}

#[must_use]
pub fn encode_block_signal_on_track_with_variant(trackbits: u8, variant: u8) -> (u8, u8, u8) {
    let track = resolve_signal_track(trackbits, 128, 128);
    let face = track
        .map(valid_signal_facings_track)
        .and_then(|f| f.first().copied())
        .unwrap_or(0);
    if let Some(t) = track
        && let Some(p) = signal_placement_for_track(t, face, variant, SIGTYPE_BLOCK)
    {
        (p.m2, p.m3, p.m3hi)
    } else {
        (0, 0, 0)
    }
}
