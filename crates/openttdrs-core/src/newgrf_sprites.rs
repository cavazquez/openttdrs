//! Decode mínimo de sprites reales `NewGRF` + Action1/3 (trains / roadtypes, preview).
//!
//! MVP: contenedor **v1** (o entradas reales inline), 8bpp **sin comprimir**
//! (sin bit 0x02 ni chunked 0x08). Action3 resuelve set-ID como índice de
//! Action1 (sin Action2). Action5 shore runtime parcial; 32bpp / callbacks OOS.

use serde::{Deserialize, Serialize};

use crate::newgrf_actions::{
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_container};
use crate::newgrf_palette_data::DOS_PALETTE_RGB;

/// Sprite RGBA decodificado (índice 0 → alpha 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSprite {
    pub width: u16,
    pub height: u16,
    pub x_offs: i16,
    pub y_offs: i16,
    /// `width * height * 4` bytes RGBA.
    pub rgba: Vec<u8>,
}

/// Asignación Action3: id local → set Action1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSpriteAssign {
    pub local_id: u8,
    pub set_id: u16,
}

/// Resultado de parsear Action1/3 de un feature (trains / roadtypes).
#[derive(Debug, Clone, Default)]
pub struct TrainSpriteGraphics {
    /// `sets[set_id][view]`.
    pub sets: Vec<Vec<DecodedSprite>>,
    pub assigns: Vec<TrainSpriteAssign>,
}

impl TrainSpriteGraphics {
    /// Preview (primera vista) para un id local.
    #[must_use]
    pub fn preview_for_local_id(&self, local_id: u8) -> Option<&DecodedSprite> {
        self.views_for_local_id(local_id)?.first()
    }

    /// Todas las vistas del set asignado al id local.
    #[must_use]
    pub fn views_for_local_id(&self, local_id: u8) -> Option<&[DecodedSprite]> {
        let set_id = self
            .assigns
            .iter()
            .find(|a| a.local_id == local_id)
            .map(|a| a.set_id)
            .or_else(|| (!self.sets.is_empty()).then_some(0))?;
        self.sets
            .get(usize::from(set_id))
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }
}

/// Convierte índices 8bpp → RGBA con paleta DOS.
#[must_use]
pub fn indices_to_rgba(indices: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    let expected = usize::from(width).checked_mul(usize::from(height))?;
    if indices.len() < expected {
        return None;
    }
    let mut rgba = Vec::with_capacity(expected * 4);
    for &idx in &indices[..expected] {
        if idx == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let [r, g, b] = DOS_PALETTE_RGB[usize::from(idx)];
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Some(rgba)
}

/// Decodifica un sprite real v1 **sin comprimir** (payload tras el BYTE type).
///
/// Layout tras `type`: `height:u8`, `width:u16 LE`, `x_offs:i16 LE`, `y_offs:i16 LE`,
/// luego `width*height` índices de paleta.
#[must_use]
pub fn decode_real_sprite_v1_uncompressed(type_and_rest: &[u8]) -> Option<DecodedSprite> {
    if type_and_rest.len() < 8 {
        return None;
    }
    let sprite_type = type_and_rest[0];
    // Comprimido / chunked → OOS en este MVP.
    if sprite_type & 0x02 != 0 || sprite_type & 0x08 != 0 {
        return None;
    }
    let height = u16::from(type_and_rest[1]);
    let width = u16::from_le_bytes([type_and_rest[2], type_and_rest[3]]);
    let x_offs = i16::from_le_bytes([type_and_rest[4], type_and_rest[5]]);
    let y_offs = i16::from_le_bytes([type_and_rest[6], type_and_rest[7]]);
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return None;
    }
    let pixels = &type_and_rest[8..];
    let rgba = indices_to_rgba(pixels, width, height)?;
    Some(DecodedSprite {
        width,
        height,
        x_offs,
        y_offs,
        rgba,
    })
}

fn parse_action1_feature(payload: &[u8], feature: u8) -> Option<(u8, u8)> {
    // 01 <feature> <num-sets> <num-ent>
    if payload.len() < 4 || payload[0] != 0x01 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let num_sets = payload[2];
    let num_ent = payload[3];
    if num_sets == 0 || num_ent == 0 {
        return None;
    }
    Some((num_sets, num_ent))
}

fn parse_action3_feature(payload: &[u8], feature: u8) -> Option<Vec<TrainSpriteAssign>> {
    // 03 <feature> <n-id> <ids…> <num-cid> [cargo…] <default:u16>
    if payload.len() < 6 || payload[0] != 0x03 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let n_id = payload[2];
    if n_id == 0 {
        return None;
    }
    let ids_end = 3 + usize::from(n_id);
    if payload.len() < ids_end + 1 + 2 {
        return None;
    }
    let ids = &payload[3..ids_end];
    let num_cid = payload[ids_end];
    // Saltar pares cargo (1+2 bytes) — MVP no los usa.
    let mut i = ids_end + 1;
    for _ in 0..num_cid {
        if i + 3 > payload.len() {
            return None;
        }
        i += 3; // cargo:u8 + set:u16
    }
    if i + 2 > payload.len() {
        return None;
    }
    let default_set = u16::from_le_bytes([payload[i], payload[i + 1]]);
    Some(
        ids.iter()
            .map(|&local_id| TrainSpriteAssign {
                local_id,
                set_id: default_set,
            })
            .collect(),
    )
}

/// Recorre el GRF y extrae sets Action1 + asignaciones Action3 para un feature.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_feature_sprite_graphics(
    data: &[u8],
    feature: u8,
) -> Result<TrainSpriteGraphics, GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut out = TrainSpriteGraphics::default();
    let mut current_set: Vec<DecodedSprite> = Vec::new();
    let mut views_left_in_set = 0u8;
    let mut sets_left = 0u8;
    let mut views_per_set = 0u8;

    let mut i = 0usize;
    while i < section.len() {
        let (size, header) = match container {
            GrfContainerVersion::V2 => {
                if i + 5 > section.len() {
                    break;
                }
                let size = u32::from_le_bytes([
                    section[i],
                    section[i + 1],
                    section[i + 2],
                    section[i + 3],
                ]) as usize;
                if size == 0 {
                    break;
                }
                (size, 5usize)
            }
            GrfContainerVersion::V1 => {
                if i + 3 > section.len() {
                    break;
                }
                let size = u16::from_le_bytes([section[i], section[i + 1]]) as usize;
                if size == 0 {
                    break;
                }
                (size, 3usize)
            }
        };
        let info = section[i + header - 1];
        let payload_start = i + header;
        if info == 0xFF {
            let end = payload_start + size;
            if end > section.len() {
                break;
            }
            let payload = &section[payload_start..end];
            if let Some((ns, ne)) = parse_action1_feature(payload, feature) {
                if !current_set.is_empty() {
                    out.sets.push(std::mem::take(&mut current_set));
                }
                sets_left = ns;
                views_per_set = ne;
                views_left_in_set = ne;
            } else if let Some(assigns) = parse_action3_feature(payload, feature) {
                out.assigns.extend(assigns);
            }
            i = end;
            continue;
        }

        let type_and_rest = match container {
            GrfContainerVersion::V1 => {
                let start = i + 2;
                let end = start + size;
                if end > section.len() {
                    break;
                }
                &section[start..end]
            }
            GrfContainerVersion::V2 => {
                let end = payload_start + size;
                if end > section.len() {
                    break;
                }
                &section[payload_start..end]
            }
        };

        if (sets_left > 0 || views_left_in_set > 0)
            && let Some(decoded) = decode_real_sprite_v1_uncompressed(type_and_rest)
        {
            current_set.push(decoded);
            views_left_in_set = views_left_in_set.saturating_sub(1);
            if views_left_in_set == 0 {
                out.sets.push(std::mem::take(&mut current_set));
                sets_left = sets_left.saturating_sub(1);
                if sets_left > 0 {
                    views_left_in_set = views_per_set;
                }
            }
        }

        i = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
    }
    if !current_set.is_empty() {
        out.sets.push(current_set);
    }
    Ok(out)
}

/// Action1/3 trains.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_train_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_TRAINS)
}

/// Action1/3 roadtypes.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADTYPES)
}

/// Construye un sprite real v1 sin comprimir (bytes tras el WORD size del contenedor).
#[must_use]
pub fn build_real_sprite_v1_uncompressed(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + indices.len());
    body.push(0x01); // type: image, uncompressed
    body.push(u8::try_from(height).unwrap_or(1));
    body.extend_from_slice(&width.to_le_bytes());
    body.extend_from_slice(&x_offs.to_le_bytes());
    body.extend_from_slice(&y_offs.to_le_bytes());
    body.extend_from_slice(indices);
    body
}

/// Action1: 1 set × `num_ent` vistas para un feature.
#[must_use]
pub fn build_action1_feature_payload(feature: u8, num_sets: u8, num_ent: u8) -> Vec<u8> {
    vec![0x01, feature, num_sets, num_ent]
}

/// Action1 trains: 1 set × `num_ent` vistas.
#[must_use]
pub fn build_action1_trains_payload(num_sets: u8, num_ent: u8) -> Vec<u8> {
    build_action1_feature_payload(ACTION0_FEATURE_TRAINS, num_sets, num_ent)
}

/// Action3: un id local → set por defecto (sin cargos).
#[must_use]
pub fn build_action3_feature_payload(feature: u8, local_id: u8, default_set: u16) -> Vec<u8> {
    let mut p = vec![0x03, feature, 0x01, local_id, 0x00];
    p.extend_from_slice(&default_set.to_le_bytes());
    p
}

/// Action3 trains: un id local → set por defecto (sin cargos).
#[must_use]
pub fn build_action3_trains_payload(local_id: u8, default_set: u16) -> Vec<u8> {
    build_action3_feature_payload(ACTION0_FEATURE_TRAINS, local_id, default_set)
}

/// GRF v2 sintético: Action0 + Action1 + sprite(s) + Action3 + Action8.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_with_preview_sprite(
    action0: &[u8],
    feature: u8,
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_feature_payload(feature, 1, 1);
    let action3 = build_action3_feature_payload(feature, local_id, 0);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    // Sprite real inline: info ≠ 0xFF; payload = cabecera v1 + índices.
    let sz = u32::try_from(sprite_body.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0x01);
    data_section.extend_from_slice(&sprite_body);

    for payload in [action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
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

/// GRF v2 sintético: Action0 train + Action1 + sprite(s) + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_train_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_TRAINS,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// GRF v2 sintético: Action0 roadtype + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_roadtype_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_ROADTYPES,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// Action1/3 stations.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_station_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_STATIONS)
}

/// GRF v2 sintético: Action0 station + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_station_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_STATIONS,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

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

/// Bloque Action5 parseado (tipo + offset + sprites siguientes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action5Block {
    pub type_id: u8,
    pub num_sprites: u8,
    pub offset: u16,
    /// Primer sprite real decodificado (8bpp sin comprimir), si se pudo.
    pub first_preview: Option<DecodedSprite>,
    /// Todos los sprites del bloque que se pudieron decodificar (orden de archivo).
    pub sprites: Vec<DecodedSprite>,
}

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

fn read_grf_entry_header(
    section: &[u8],
    i: usize,
    container: GrfContainerVersion,
) -> Option<(usize, usize)> {
    match container {
        GrfContainerVersion::V2 => {
            if i + 5 > section.len() {
                return None;
            }
            let size =
                u32::from_le_bytes([section[i], section[i + 1], section[i + 2], section[i + 3]])
                    as usize;
            (size > 0).then_some((size, 5))
        }
        GrfContainerVersion::V1 => {
            if i + 3 > section.len() {
                return None;
            }
            let size = u16::from_le_bytes([section[i], section[i + 1]]) as usize;
            (size > 0).then_some((size, 3))
        }
    }
}

fn real_sprite_payload(
    section: &[u8],
    i: usize,
    size: usize,
    header: usize,
    container: GrfContainerVersion,
) -> Option<&[u8]> {
    let payload_start = i + header;
    match container {
        GrfContainerVersion::V1 => {
            let start = i + 2;
            let end = start + size;
            (end <= section.len()).then(|| &section[start..end])
        }
        GrfContainerVersion::V2 => {
            let end = payload_start + size;
            (end <= section.len()).then(|| &section[payload_start..end])
        }
    }
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
    let (container, section) = parse_grf_container(data)?;
    let mut out = Vec::new();
    let mut sprites_left = 0u8;
    let mut cur_type = 0u8;
    let mut cur_num = 0u8;
    let mut cur_offset = 0u16;
    let mut sprites: Vec<DecodedSprite> = Vec::new();
    let mut in_block = false;

    let mut i = 0usize;
    while i < section.len() {
        let Some((size, header)) = read_grf_entry_header(section, i, container) else {
            break;
        };
        let info = section[i + header - 1];
        let payload_start = i + header;
        if info == 0xFF {
            let end = payload_start + size;
            if end > section.len() {
                break;
            }
            let payload = &section[payload_start..end];
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
            i = end;
            continue;
        }

        let Some(type_and_rest) = real_sprite_payload(section, i, size, header, container) else {
            break;
        };

        if in_block && sprites_left > 0 {
            if let Some(spr) = decode_real_sprite_v1_uncompressed(type_and_rest) {
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

        i = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
    }
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
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action5 = vec![0x05, type_id, 0x01];
    action5.extend_from_slice(&offset.to_le_bytes());
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed(
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

    let sz = u32::try_from(sprite_body.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0x01);
    data_section.extend_from_slice(&sprite_body);

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_actions::{build_action0_roadtype_payload, build_action0_train_payload};

    #[test]
    fn decode_flat_8bpp_applies_palette_and_transparency() {
        let w = 2u16;
        let h = 2u16;
        let indices = [0u8, 174, 174, 0]; // 174 ≈ rojo en DOS
        let body = build_real_sprite_v1_uncompressed(w, h, -1, -2, &indices);
        let spr = decode_real_sprite_v1_uncompressed(&body).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
        assert_eq!(spr.rgba.len(), 16);
        assert_eq!(&spr.rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(spr.rgba[7], 255); // alpha del pixel rojo
    }

    #[test]
    fn collect_action1_3_preview_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1960, 100, 800, "Sprite Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'S', 0, 1],
            "tsprite",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        assert_eq!(gfx.sets[0].len(), 1);
        assert_eq!(gfx.assigns.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_roadtype_preview_from_synthetic_grf() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble");
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 200;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'T', 0, 2],
            "rtgfx",
        );
        let gfx = collect_roadtype_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        // Feature distinto: trains no ve el set.
        let trains = collect_train_sprite_graphics(&bytes).unwrap();
        assert!(trains.sets.is_empty());
    }

    #[test]
    fn collect_station_preview_from_synthetic_grf() {
        use crate::newgrf_actions::build_action0_station_payload;
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'T', 0, 3],
            "stgfx",
        );
        let gfx = collect_station_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
    }

    #[test]
    fn collect_action5_block_with_preview() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_action5_with_sprite(
            0x0D,
            4804,
            8,
            8,
            &indices,
            [b'S', b'H', 0, 1],
            "shore",
        );
        let blocks = collect_action5_blocks(&bytes).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].type_id, 0x0D);
        assert_eq!(blocks[0].num_sprites, 1);
        assert_eq!(blocks[0].offset, 4804);
        assert_eq!(blocks[0].sprites.len(), 1);
        assert_eq!(action5_type_name(0x0D), "shore");
        let preview = blocks[0].first_preview.as_ref().unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        let mut slots = vec![None; SHORE_ACTION5_SLOT_COUNT];
        merge_shore_action5_block(&mut slots, &blocks[0]);
        // offset 4804 ≥ 18 → escribe en slot 0
        assert!(slots[0].is_some());
    }
}
