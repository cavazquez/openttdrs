//! Action5: sprites de reemplazo global (shore / catenary / GUI / …).

use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::model::{Action5Block, DecodedSprite};
use super::pixel_codec::{decode_real_sprite_entry, index_sprite_section, resolve_fd_sprite};

/// Tipo Action5: shore / coastline (`ACT5_SHORELINE`).
pub const ACTION5_TYPE_SHORE: u8 = 0x0D;
/// Tipo Action5: catenaria (`ACT5_ELRAIL`).
pub const ACTION5_TYPE_CATENARY: u8 = 0x05;
/// Slots `SPR_SHORE_BASE + 0..17`.
pub const SHORE_ACTION5_SLOT_COUNT: usize = 18;
/// Orden del bloque de 10 («missing shore sprites», `newgrf_act5.cpp`).
pub const SHORE_MISSING_BLOCK_SLOTS: [usize; 10] = [0, 5, 7, 10, 11, 13, 14, 15, 16, 17];
/// Slots Action5 catenary `OpenGFX`: wires 0..23 + entrances 24..27 + pylons 28..35.
pub const CATENARY_ACTION5_SLOT_COUNT: usize = 36;
/// Base `OpenTTD` de wires (`SPR_WIRE_*` / `rail_1039`).
pub const CATENARY_WIRE_SPRITE_BASE: u32 = 1039;
/// IDs virtuales de entrada de túnel en el cliente (`WSO_ENTRANCE_*`).
pub const CATENARY_ENTRANCE_SPRITE_BASE: u32 = 910_063;
/// IDs virtuales de postes PPP en el cliente (`PSO_*`).
pub const CATENARY_PYLON_SPRITE_BASE: u32 = 910_067;

/// Nombre corto de tipos Action5 conocidos (resto = `other`).
#[must_use]
pub fn action5_type_name(type_id: u8) -> &'static str {
    match type_id {
        0x04 | 0x06 => "foundations",
        0x05 => "catenary",
        0x07 => "gui",
        0x08 => "airport-preview",
        0x09 => "roadstops",
        0x0A => "oneway-road",
        0x0B => "bridge",
        0x0C => "grass",
        0x0D => "shore",
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
                        let id = u32::from_le_bytes([
                            payload[0],
                            payload[1],
                            payload[2],
                            payload[3],
                        ]);
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
    let base = if usize::from(block.offset) < SHORE_ACTION5_SLOT_COUNT {
        usize::from(block.offset)
    } else {
        0
    };
    for (i, spr) in sprites.iter().enumerate() {
        let slot = base + i;
        if slot >= SHORE_ACTION5_SLOT_COUNT {
            break;
        }
        slots[slot] = Some(spr.clone());
    }
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
    let base = if block.offset == wire_base || block.offset == 0 {
        0
    } else if usize::from(block.offset) < CATENARY_ACTION5_SLOT_COUNT {
        usize::from(block.offset)
    } else {
        0
    };
    for (i, spr) in block.sprites.iter().enumerate() {
        let slot = base + i;
        if slot >= CATENARY_ACTION5_SLOT_COUNT {
            break;
        }
        slots[slot] = Some(spr.clone());
    }
}

/// GRF v2 sintético: Action5 + un sprite + Action8.
#[must_use]
pub fn build_grf_v2_action5_with_sprite(
    type_id: u8,
    offset: u16,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    use super::action_graph::append_v2_real_sprite;
    use super::pixel_codec::build_real_sprite_v1_uncompressed_payload;
    
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action5 = vec![0x05, type_id, 0x01];
    action5.extend_from_slice(&offset.to_le_bytes());
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    let sz = u32::try_from(action5.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0xFF);
    data_section.extend_from_slice(&action5);

    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    let sz = u32::try_from(action8.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0xFF);
    data_section.extend_from_slice(&action8);
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}
