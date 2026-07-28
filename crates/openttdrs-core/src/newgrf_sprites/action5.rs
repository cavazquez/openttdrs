//! Action5: sprites de reemplazo global (IDs `OpenTTD` 15.3 / `newgrf_act5.cpp`).

use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::model::{Action5Block, DecodedSprite};
use super::pixel_codec::{decode_real_sprite_entry, index_sprite_section, resolve_fd_sprite};

/// Tipo Action5: señales (`0x04`).
pub const ACTION5_TYPE_SIGNALS: u8 = 0x04;
/// Tipo Action5: catenaria (`ACT5_ELRAIL`).
pub const ACTION5_TYPE_CATENARY: u8 = 0x05;
/// Tipo Action5: foundations (`0x06`).
pub const ACTION5_TYPE_FOUNDATIONS: u8 = 0x06;
/// Tipo Action5: canals (`0x08`).
pub const ACTION5_TYPE_CANALS: u8 = 0x08;
/// Tipo Action5: one-way roads (`0x09`).
pub const ACTION5_TYPE_ONEWAY: u8 = 0x09;
/// Tipo Action5: 2CC colour maps (`0x0A`).
pub const ACTION5_TYPE_TWOCC: u8 = 0x0A;
/// Tipo Action5: tramway (`0x0B`).
pub const ACTION5_TYPE_TRAMWAY: u8 = 0x0B;
/// Tipo Action5: shore / coastline (`ACT5_SHORELINE`).
pub const ACTION5_TYPE_SHORE: u8 = 0x0D;
/// Tipo Action5: road stop graphics (`0x11`).
pub const ACTION5_TYPE_ROADSTOPS: u8 = 0x11;
/// Tipo Action5: `OpenTTD` GUI (`0x15`).
pub const ACTION5_TYPE_OPENTTD_GUI: u8 = 0x15;
/// Tipo Action5: airport preview (`0x16`).
pub const ACTION5_TYPE_AIRPORT_PREVIEW: u8 = 0x16;
/// Tipo Action5: bridge decks (`0x1B`).
pub const ACTION5_TYPE_BRIDGE_DECKS: u8 = 0x1B;

/// `PRESIGNAL_SEMAPHORE_AND_PBS_SPRITE_COUNT` (Action5 tipo `0x04`).
pub const SIGNAL_ACTION5_SLOT_COUNT: usize = 240;
/// `SPR_SIGNALS_BASE` en el atlas cliente (`rail_5088..`).
pub const SPR_SIGNALS_ACTION5_BASE: u32 = 5088;
/// Slots `SPR_SHORE_BASE + 0..17`.
pub const SHORE_ACTION5_SLOT_COUNT: usize = 18;
/// Orden del bloque de 10 («missing shore sprites», `newgrf_act5.cpp`).
pub const SHORE_MISSING_BLOCK_SLOTS: [usize; 10] = [0, 5, 7, 10, 11, 13, 14, 15, 16, 17];
/// Slots Action5 catenary `OpenGFX`: wires 0..23 + entrances 24..27 + pylons 28..35.
pub const CATENARY_ACTION5_SLOT_COUNT: usize = 36;
/// `NORMAL_AND_HALFTILE_FOUNDATION_SPRITE_COUNT`.
pub const FOUNDATION_ACTION5_SLOT_COUNT: usize = 90;
/// `ONEWAY_SPRITE_COUNT`.
pub const ONEWAY_ACTION5_SLOT_COUNT: usize = 18;
/// `ROADSTOP_SPRITE_COUNT`.
pub const ROADSTOP_ACTION5_SLOT_COUNT: usize = 8;
/// `OPENTTD_SPRITE_COUNT`.
pub const OPENTTD_GUI_ACTION5_SLOT_COUNT: usize = 192;
/// `AIRPORT_PREVIEW_SPRITE_COUNT`.
pub const AIRPORT_PREVIEW_ACTION5_SLOT_COUNT: usize = 9;
/// `BRIDGE_DECKS_SPRITE_COUNT` (6 direcciones × 4 tipos).
pub const BRIDGE_DECKS_ACTION5_SLOT_COUNT: usize = 24;
/// `CANALS_SPRITE_COUNT` (Action5 tipo `0x08`).
pub const CANALS_ACTION5_SLOT_COUNT: usize = 65;
/// Primer slot de esclusa en Action5 canals (`SPR_LOCK_BASE - SPR_CANALS_BASE`).
pub const CANALS_ACTION5_LOCK_SLOT: usize = 4;

/// Base `OpenTTD` de wires (`SPR_WIRE_*` / `rail_1039`).
pub const CATENARY_WIRE_SPRITE_BASE: u32 = 1039;
/// IDs virtuales de entrada de túnel en el cliente (`WSO_ENTRANCE_*`).
pub const CATENARY_ENTRANCE_SPRITE_BASE: u32 = 910_063;
/// IDs virtuales de postes PPP en el cliente (`PSO_*`).
pub const CATENARY_PYLON_SPRITE_BASE: u32 = 910_067;

/// Nombre corto de tipos Action5 conocidos (resto = `other`). IDs = `OpenTTD` 15.3.
#[must_use]
pub fn action5_type_name(type_id: u8) -> &'static str {
    match type_id {
        0x04 => "signals",
        0x05 => "catenary",
        0x06 => "foundations",
        0x07 => "ttdp-gui-unused",
        0x08 => "canals",
        0x09 => "oneway-road",
        0x0A => "2cc-maps",
        0x0B => "tramway",
        0x0C => "snowy-tree-unused",
        0x0D => "shore",
        0x11 => "roadstops",
        0x15 => "openttd-gui",
        0x16 => "airport-preview",
        0x1B => "bridge-decks",
        _ => "other",
    }
}

fn parse_action5_header(payload: &[u8]) -> Option<(u8, u8, u16)> {
    // 05 <type> <num-sprites> <offset:u16 LE>
    if payload.len() < 5 || payload[0] != 0x05 {
        return None;
    }
    let type_id = payload[1];
    let num_sprites = payload[2];
    if num_sprites == 0 {
        return None;
    }
    let offset = u16::from_le_bytes([payload[3], payload[4]]);
    Some((type_id, num_sprites, offset))
}

fn finish_action5_block(
    type_id: u8,
    num_sprites: u8,
    offset: u16,
    sprites: Vec<DecodedSprite>,
) -> Action5Block {
    Action5Block {
        type_id,
        num_sprites,
        offset,
        first_preview: sprites.first().cloned(),
        sprites,
    }
}

/// Recorre el GRF y extrae bloques Action5 + sprites decodificados de cada bloque.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_action5_blocks(data: &[u8]) -> Result<Vec<Action5Block>, GrfScanError> {
    let parsed = parse_grf_full(data)?;
    let container = parsed.container;
    let section = parsed.data_section;
    let sprite_index = index_sprite_section(parsed.sprite_section);
    let mut out = Vec::new();
    let mut sprites_left = 0u8;
    let mut cur_type = 0u8;
    let mut cur_num = 0u8;
    let mut cur_offset = 0u16;
    let mut sprites: Vec<DecodedSprite> = Vec::new();
    let mut in_block = false;

    walk_grf_entries(section, container, |entry| match entry {
        GrfEntry::Pseudo(payload) => {
            if let Some((type_id, num, offset)) = parse_action5_header(payload) {
                if in_block {
                    out.push(finish_action5_block(
                        cur_type,
                        cur_num,
                        cur_offset,
                        std::mem::take(&mut sprites),
                    ));
                }
                cur_type = type_id;
                cur_num = num;
                cur_offset = offset;
                sprites_left = num;
                sprites.clear();
                in_block = true;
            }
        }
        GrfEntry::Real { info, payload } => {
            if in_block && sprites_left > 0 {
                let spr = if container == GrfContainerVersion::V2 && info == 0xFD {
                    if payload.len() >= 4 {
                        let id =
                            u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                        resolve_fd_sprite(&sprite_index, id)
                    } else {
                        None
                    }
                } else {
                    decode_real_sprite_entry(container, info, payload)
                };
                if let Some(spr) = spr {
                    sprites.push(spr);
                }
                sprites_left = sprites_left.saturating_sub(1);
                if sprites_left == 0 {
                    out.push(finish_action5_block(
                        cur_type,
                        cur_num,
                        cur_offset,
                        std::mem::take(&mut sprites),
                    ));
                    in_block = false;
                }
            }
        }
    });

    if in_block {
        out.push(finish_action5_block(cur_type, cur_num, cur_offset, sprites));
    }
    Ok(out)
}

/// Fusiona un bloque Action5 con offset local (tipos `A5BLOCK_ALLOW_OFFSET` de 15.3).
///
/// Escribe desde `offset` si `offset < slot_count`; si no, desde 0. Nunca escribe
/// fuera de la tabla propia.
pub fn merge_action5_offset_block(
    slots: &mut [Option<DecodedSprite>],
    block: &Action5Block,
    type_id: u8,
    slot_count: usize,
) {
    if block.type_id != type_id || slots.len() < slot_count || block.sprites.is_empty() {
        return;
    }
    let base = if usize::from(block.offset) < slot_count {
        usize::from(block.offset)
    } else {
        0
    };
    for (i, spr) in block.sprites.iter().enumerate() {
        let slot = base + i;
        if slot >= slot_count {
            break;
        }
        slots[slot] = Some(spr.clone());
    }
}

/// Fusiona un bloque Action5 shore (`0x0D`) en la tabla de 18 slots.
///
/// - 10 sprites → tabla «missing» de `OpenTTD`.
/// - 16 sprites → slots `0..15`.
/// - resto → escribe desde `offset` si `offset < 18`; si no, desde el slot 0.
pub fn merge_shore_action5_block(slots: &mut [Option<DecodedSprite>], block: &Action5Block) {
    if block.type_id != ACTION5_TYPE_SHORE || slots.len() < SHORE_ACTION5_SLOT_COUNT {
        return;
    }
    let sprites = &block.sprites;
    if sprites.is_empty() {
        return;
    }
    if block.num_sprites == 10 && sprites.len() >= 10 {
        for (i, &slot) in SHORE_MISSING_BLOCK_SLOTS.iter().enumerate() {
            slots[slot] = Some(sprites[i].clone());
        }
        return;
    }
    if block.num_sprites == 16 && sprites.len() >= 16 {
        for i in 0..16 {
            slots[i] = Some(sprites[i].clone());
        }
        return;
    }
    merge_action5_offset_block(slots, block, ACTION5_TYPE_SHORE, SHORE_ACTION5_SLOT_COUNT);
}

/// Índice local Action5 (0..35) para un `sprite_id` de catenaria del cliente.
#[must_use]
pub fn catenary_action5_local_slot(sprite_id: u32) -> Option<usize> {
    if (CATENARY_WIRE_SPRITE_BASE..=CATENARY_WIRE_SPRITE_BASE + 23).contains(&sprite_id) {
        return Some((sprite_id - CATENARY_WIRE_SPRITE_BASE) as usize);
    }
    if (CATENARY_ENTRANCE_SPRITE_BASE..=CATENARY_ENTRANCE_SPRITE_BASE + 3).contains(&sprite_id) {
        return Some(24 + (sprite_id - CATENARY_ENTRANCE_SPRITE_BASE) as usize);
    }
    if (CATENARY_PYLON_SPRITE_BASE..=CATENARY_PYLON_SPRITE_BASE + 7).contains(&sprite_id) {
        return Some(28 + (sprite_id - CATENARY_PYLON_SPRITE_BASE) as usize);
    }
    None
}

/// Fusiona un bloque Action5 catenary (`0x05`) en la tabla de slots locales.
///
/// El `offset` 1039 (base `OpenTTD`) o 0 empieza en el slot 0; un offset `< 36`
/// se usa como índice de inicio (GRFs de prueba).
pub fn merge_catenary_action5_block(slots: &mut [Option<DecodedSprite>], block: &Action5Block) {
    if block.type_id != ACTION5_TYPE_CATENARY || slots.len() < CATENARY_ACTION5_SLOT_COUNT {
        return;
    }
    if block.sprites.is_empty() {
        return;
    }
    let wire_base = u16::try_from(CATENARY_WIRE_SPRITE_BASE).unwrap_or(1039);
    let mut normalized = block.clone();
    if normalized.offset == wire_base {
        normalized.offset = 0;
    }
    merge_action5_offset_block(
        slots,
        &normalized,
        ACTION5_TYPE_CATENARY,
        CATENARY_ACTION5_SLOT_COUNT,
    );
}

macro_rules! define_action5_merge {
    ($fn_name:ident, $type_const:ident, $count_const:ident) => {
        pub fn $fn_name(slots: &mut [Option<DecodedSprite>], block: &Action5Block) {
            merge_action5_offset_block(slots, block, $type_const, $count_const);
        }
    };
}

define_action5_merge!(
    merge_signals_action5_block,
    ACTION5_TYPE_SIGNALS,
    SIGNAL_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_foundation_action5_block,
    ACTION5_TYPE_FOUNDATIONS,
    FOUNDATION_ACTION5_SLOT_COUNT
);

/// Slot Action5 `0x04` para un `sprite_id` del banco `SPR_SIGNALS_BASE`.
#[must_use]
pub fn signal_action5_slot(sprite_id: u32) -> Option<usize> {
    let base = SPR_SIGNALS_ACTION5_BASE;
    let end = base + u32::try_from(SIGNAL_ACTION5_SLOT_COUNT).unwrap_or(0);
    if !(sprite_id >= base && sprite_id < end) {
        return None;
    }
    usize::try_from(sprite_id - base).ok()
}
define_action5_merge!(
    merge_oneway_action5_block,
    ACTION5_TYPE_ONEWAY,
    ONEWAY_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_roadstop_action5_block,
    ACTION5_TYPE_ROADSTOPS,
    ROADSTOP_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_openttd_gui_action5_block,
    ACTION5_TYPE_OPENTTD_GUI,
    OPENTTD_GUI_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_airport_preview_action5_block,
    ACTION5_TYPE_AIRPORT_PREVIEW,
    AIRPORT_PREVIEW_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_bridge_decks_action5_block,
    ACTION5_TYPE_BRIDGE_DECKS,
    BRIDGE_DECKS_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_canals_action5_block,
    ACTION5_TYPE_CANALS,
    CANALS_ACTION5_SLOT_COUNT
);

/// Slot Action5 foundations para `tileh` 1..=14 (cimientos nivelados del cliente).
#[must_use]
pub fn foundation_action5_slot_for_tileh(tileh: u8) -> Option<usize> {
    if (1..=14).contains(&tileh) {
        Some(usize::from(tileh - 1))
    } else {
        None
    }
}

/// Offset de pendiente norte (`SLOPE_NE`/`SLOPE_NW`) en la tabla one-way.
pub const ONEWAY_SLOPE_N_OFFSET: usize = 6;
/// Offset de pendiente sur (`SLOPE_SE`/`SLOPE_SW`) en la tabla one-way.
pub const ONEWAY_SLOPE_S_OFFSET: usize = 12;

/// `GetDisallowedRoadDirections`: bits 4..5 de `m5` en carretera normal (subtype 0).
#[must_use]
pub fn disallowed_road_directions(m5: u8) -> u8 {
    if (m5 >> 6) & 0x3 != 0 {
        return 0;
    }
    (m5 >> 4) & 0x3
}

/// Slot Action5 one-way (`0x09`) según `DrawRoadBits` de `OpenTTD`.
///
/// `drd` es 1..=3 (`DRD_SOUTHBOUND`…`DRD_BOTH`); `road_x_axis` = bits `ROAD_X` (0x0A).
#[must_use]
pub fn oneway_action5_slot(tileh: u8, road_x_axis: bool, drd: u8) -> Option<usize> {
    if !(1..=3).contains(&drd) {
        return None;
    }
    let slope = match tileh {
        crate::map::SLOPE_NE | crate::map::SLOPE_NW => ONEWAY_SLOPE_N_OFFSET,
        crate::map::SLOPE_SE | crate::map::SLOPE_SW => ONEWAY_SLOPE_S_OFFSET,
        _ => 0,
    };
    let axis = if road_x_axis { 0 } else { 3 };
    Some(slope + axis + usize::from(drd) - 1)
}

/// Slot Action5 roadstops (`0x11`): bus 0..3 / truck 4..7 según dirección 0..3.
#[must_use]
pub fn roadstop_action5_slot(is_truck: bool, dir: usize) -> Option<usize> {
    if dir > 3 {
        return None;
    }
    Some(if is_truck { 4 + dir } else { dir })
}

/// Base de tablero Action5 por tipo de vía (`SPR_BRIDGE_DECKS_*`).
#[must_use]
pub fn bridge_decks_action5_base(rail: bool, rail_type: crate::rail_type::RailType) -> usize {
    if !rail {
        return 18;
    }
    match rail_type {
        crate::rail_type::RailType::Monorail => 6,
        crate::rail_type::RailType::Maglev => 12,
        crate::rail_type::RailType::Rail | crate::rail_type::RailType::Electric => 0,
    }
}

/// Slot Action5 bridge decks (`0x1B`) para vano (`surface + axis`).
#[must_use]
pub fn bridge_decks_action5_slot(
    rail: bool,
    rail_type: crate::rail_type::RailType,
    axis: usize,
) -> Option<usize> {
    let slot = bridge_decks_action5_base(rail, rail_type) + (axis & 1);
    (slot < BRIDGE_DECKS_ACTION5_SLOT_COUNT).then_some(slot)
}

/// Slot Action5 airport preview (`0x16`) alineado con `SPR_AIRPORT_PREVIEW_*`.
#[must_use]
pub fn airport_preview_action5_slot(spec: crate::airport_class::AirportSpecId) -> Option<usize> {
    use crate::airport_class::AirportSpecId;
    let slot = match spec {
        AirportSpecId::Small => 0,
        AirportSpecId::City => 1,
        AirportSpecId::Heliport | AirportSpecId::Oilrig => 2,
        AirportSpecId::Metropolitan => 3,
        AirportSpecId::International => 4,
        AirportSpecId::Commuter => 5,
        AirportSpecId::Helidepot => 6,
        AirportSpecId::Intercontinental => 7,
        AirportSpecId::Helistation => 8,
    };
    Some(slot)
}

#[cfg(test)]
mod slot_helper_tests {
    use super::*;
    use crate::airport_class::AirportSpecId;
    use crate::map::{SLOPE_NE, SLOPE_SE};
    use crate::rail_type::RailType;

    #[test]
    fn oneway_and_roadstop_and_bridge_slots() {
        assert_eq!(disallowed_road_directions(0x15), 1); // bits + ROAD_Y
        assert_eq!(oneway_action5_slot(0, true, 1), Some(0));
        assert_eq!(oneway_action5_slot(SLOPE_NE, false, 2), Some(6 + 3 + 1));
        assert_eq!(oneway_action5_slot(SLOPE_SE, true, 3), Some(12 + 2));
        assert_eq!(roadstop_action5_slot(false, 2), Some(2));
        assert_eq!(roadstop_action5_slot(true, 1), Some(5));
        assert_eq!(bridge_decks_action5_slot(true, RailType::Rail, 1), Some(1));
        assert_eq!(
            bridge_decks_action5_slot(false, RailType::Rail, 0),
            Some(18)
        );
        assert_eq!(
            airport_preview_action5_slot(AirportSpecId::International),
            Some(4)
        );
    }
}
