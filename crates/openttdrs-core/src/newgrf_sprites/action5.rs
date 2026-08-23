//! Action5: sprites de reemplazo global (IDs `OpenTTD` 15.3 / `newgrf_act5.cpp`).

use std::collections::HashMap;

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
/// `TWOCCMAP_SPRITE_COUNT` (Action5 tipo `0x0A`).
pub const TWOCC_ACTION5_SLOT_COUNT: usize = 256;
/// `TRAMWAY_SPRITE_COUNT` (Action5 tipo `0x0B`).
pub const TRAMWAY_ACTION5_SLOT_COUNT: usize = 119;

/// Base `OpenTTD` de wires (`SPR_WIRE_*` / `rail_1039`).
pub const CATENARY_WIRE_SPRITE_BASE: u32 = 1039;
/// IDs virtuales de entrada de túnel en el cliente (`WSO_ENTRANCE_*`).
pub const CATENARY_ENTRANCE_SPRITE_BASE: u32 = 910_063;
/// IDs virtuales de postes PPP en el cliente (`PSO_*`).
pub const CATENARY_PYLON_SPRITE_BASE: u32 = 910_067;

/// Contexto mínimo que necesitan las condiciones `Action7`/`Action9` al
/// cargar reemplazos globales Action5.
///
/// Los assets base de `OpenGFX` eligen bancos distintos según el paisaje con
/// una secuencia `ActionD` + `Action9`. Conservar los parámetros del GRF
/// además del clima permite seguir esa misma rama para reemplazos de una
/// partida, sin ejecutar callbacks de vehículos o estaciones en este paso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action5LoadContext {
    /// `LandscapeType` de `OpenTTD`: 0 temperado, 1 subártico, 2 subtropical,
    /// 3 toyland.
    pub landscape: u8,
    /// Parámetros iniciales configurados para el GRF (`param[]`).
    pub parameters: Vec<u32>,
}

impl Action5LoadContext {
    #[must_use]
    pub const fn new(landscape: u8) -> Self {
        Self {
            landscape,
            parameters: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_parameters(mut self, parameters: Vec<u32>) -> Self {
        self.parameters = parameters;
        self
    }
}

impl Default for Action5LoadContext {
    fn default() -> Self {
        Self::new(0)
    }
}

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

/// Estado de los parámetros que puede modificar `ActionD` mientras se carga
/// un GRF. Es intencionalmente pequeño: Action5 sólo necesita el subconjunto
/// de variables globales que condicionan bancos gráficos durante la carga.
struct Action5ControlState {
    landscape: u8,
    parameters: Vec<u32>,
}

impl From<&Action5LoadContext> for Action5ControlState {
    fn from(context: &Action5LoadContext) -> Self {
        Self {
            landscape: context.landscape,
            parameters: context.parameters.clone(),
        }
    }
}

impl Action5ControlState {
    fn global_value(&self, variable: u8) -> Option<u32> {
        // `GetGlobalVariable` de OpenTTD. Estos son los valores estables que
        // aparecen en los GRFs base y en los condicionantes de assets; una
        // variable desconocida no toma una rama especulativa.
        match variable {
            0x03 => Some(u32::from(self.landscape)), // current climate
            0x0D | 0x1D => Some(1),                  // Windows / OpenTTD
            0x1A => Some(u32::MAX),                  // always -1
            0x1B => Some(0x3F),                      // display options
            _ => None,
        }
    }

    fn value(&self, source: u8) -> Option<u32> {
        if source < 0x80 {
            return Some(
                self.parameters
                    .get(usize::from(source))
                    .copied()
                    .unwrap_or(0),
            );
        }
        self.global_value(source.wrapping_sub(0x80))
    }

    fn set_parameter(&mut self, target: u8, value: u32) {
        if target >= 0x80 {
            return;
        }
        let target = usize::from(target);
        if self.parameters.len() <= target {
            self.parameters.resize(target + 1, 0);
        }
        self.parameters[target] = value;
    }
}

fn decode_action5_sprite(
    container: GrfContainerVersion,
    sprite_index: &HashMap<u32, Vec<(u8, &[u8])>>,
    info: u8,
    payload: &[u8],
) -> Option<DecodedSprite> {
    if container == GrfContainerVersion::V2 && info == 0xFD {
        let id = u32::from_le_bytes(payload.get(..4)?.try_into().ok()?);
        return resolve_fd_sprite(sprite_index, id);
    }
    decode_real_sprite_entry(container, info, payload)
}

fn action5_condition(payload: &[u8], state: &Action5ControlState) -> Option<(bool, u8)> {
    // `07/09 <param> <size> <condition> <value> [<mask>] <skip-or-label>`.
    if payload.len() < 6 {
        return None;
    }
    let param = payload[1];
    let original_size = payload[2];
    let condition = payload[3];
    let size = if condition < 2 { 1 } else { original_size };
    let (value, mask, next) = match size {
        1 => (u32::from(*payload.get(4)?), 0xFF, 5),
        2 => (
            u32::from(u16::from_le_bytes(payload.get(4..6)?.try_into().ok()?)),
            0xFFFF,
            6,
        ),
        4 => (
            u32::from_le_bytes(payload.get(4..8)?.try_into().ok()?),
            u32::MAX,
            8,
        ),
        8 => (
            u32::from_le_bytes(payload.get(4..8)?.try_into().ok()?),
            u32::from_le_bytes(payload.get(8..12)?.try_into().ok()?),
            12,
        ),
        _ => return None,
    };
    let skip_target = *payload.get(next)?;
    let actual = state.value(param)?;
    let result = match condition {
        0x00 => value < 32 && actual & (1_u32 << value) != 0,
        0x01 => value < 32 && actual & (1_u32 << value) == 0,
        0x02 => actual & mask == value,
        0x03 => actual & mask != value,
        0x04 => actual & mask < value,
        0x05 => actual & mask > value,
        // Las condiciones 06..12 requieren estado de otros GRFs, cargos o
        // tipos. No ejecutar una rama ante información incompleta mantiene
        // el banco previo en lugar de escoger una variante arbitraria.
        _ => return None,
    };
    Some((result, skip_target))
}

fn apply_action5_param_set(payload: &[u8], state: &mut Action5ControlState) {
    // `0D <target> <operation> <source1> <source2> [data:u32 LE]`.
    if payload.len() < 5 || payload[0] != 0x0D {
        return;
    }
    let target = payload[1];
    let operation = payload[2];
    let source1 = payload[3];
    let source2 = payload[4];
    let data = payload
        .get(5..9)
        .and_then(|bytes| bytes.try_into().ok())
        .map_or(0, u32::from_le_bytes);
    let conditional_set = operation & 0x80 != 0;
    let operation = operation & 0x7F;
    if conditional_set && target < 0x80 && usize::from(target) < state.parameters.len() {
        return;
    }
    // Resource management (`source2 == FE`) requires a global allocation
    // phase; no Action5 del baseset usa esa rama, así que no la simulamos.
    if source2 == 0xFE {
        return;
    }
    let source = |id| {
        if id == 0xFF {
            Some(data)
        } else {
            state.value(id)
        }
    };
    let (Some(left), Some(right)) = (source(source1), source(source2)) else {
        return;
    };
    let result = match operation {
        0x00 => left,
        0x01 => left.wrapping_add(right),
        0x02 => left.wrapping_sub(right),
        0x03 => left.wrapping_mul(right),
        0x04 => left
            .cast_signed()
            .wrapping_mul(right.cast_signed())
            .cast_unsigned(),
        0x05 => {
            if right.cast_signed().is_negative() {
                left >> right.cast_signed().unsigned_abs()
            } else {
                left.wrapping_shl(right & 0x1F)
            }
        }
        0x06 => {
            if right.cast_signed().is_negative() {
                (left.cast_signed() >> right.cast_signed().unsigned_abs()).cast_unsigned()
            } else {
                left.cast_signed()
                    .wrapping_shl(right & 0x1F)
                    .cast_unsigned()
            }
        }
        0x07 => left & right,
        0x08 => left | right,
        0x09 => left.checked_div(right).unwrap_or(left),
        0x0A => {
            if right == 0 {
                left
            } else {
                left.cast_signed()
                    .wrapping_div(right.cast_signed())
                    .cast_unsigned()
            }
        }
        0x0B => {
            if right == 0 {
                left
            } else {
                left % right
            }
        }
        0x0C => {
            if right == 0 {
                left
            } else {
                left.cast_signed()
                    .wrapping_rem(right.cast_signed())
                    .cast_unsigned()
            }
        }
        _ => return,
    };
    state.set_parameter(target, result);
}

#[derive(Default)]
struct PendingAction5Block {
    type_id: u8,
    num_sprites: u8,
    offset: u16,
    seen_real_sprites: u16,
    sprites: Vec<DecodedSprite>,
}

impl PendingAction5Block {
    fn from_header(type_id: u8, num_sprites: u8, offset: u16) -> Self {
        Self {
            type_id,
            num_sprites,
            offset,
            ..Self::default()
        }
    }

    fn finish(self) -> Action5Block {
        finish_action5_block(self.type_id, self.num_sprites, self.offset, self.sprites)
    }
}

fn next_action5_label(
    labels: &HashMap<u8, Vec<usize>>,
    label: u8,
    current: usize,
) -> Option<usize> {
    let choices = labels.get(&label)?;
    choices
        .iter()
        .copied()
        .find(|&index| index > current)
        .or_else(|| choices.first().copied())
}

/// Extrae únicamente los bloques Action5 alcanzables con el contexto de carga.
///
/// A diferencia de [`collect_action5_blocks`], ejecuta el subconjunto de
/// `ActionD`, `Action7`, `Action9` y `Action10` que decide qué banco gráfico
/// se activa. Esto es decisivo para `OpenGFX`: sus cuatro bancos de foundations
/// comparten los mismos slots, pero están condicionados por el clima.
///
/// Las condiciones que requieren estado no disponible (otros GRFs, cargos,
/// railtypes, etc.) no fuerzan un salto. Así se conserva la variante previa
/// en vez de reemplazarla con una selección no verificable.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_active_action5_blocks(
    data: &[u8],
    context: &Action5LoadContext,
) -> Result<Vec<Action5Block>, GrfScanError> {
    let parsed = parse_grf_full(data)?;
    let sprite_index = index_sprite_section(parsed.sprite_section);
    let mut entries = Vec::new();
    walk_grf_entries(parsed.data_section, parsed.container, |entry| {
        entries.push(entry);
    });

    let mut labels: HashMap<u8, Vec<usize>> = HashMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if let GrfEntry::Pseudo(payload) = entry
            && payload.first() == Some(&0x10)
            && let Some(&label) = payload.get(1)
        {
            labels.entry(label).or_default().push(index);
        }
    }

    let mut state = Action5ControlState::from(context);
    let mut active: Option<PendingAction5Block> = None;
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < entries.len() {
        match entries[index] {
            GrfEntry::Pseudo(payload) => {
                if let Some(block) = active.take() {
                    out.push(block.finish());
                }
                match payload.first().copied() {
                    Some(0x0D) => apply_action5_param_set(payload, &mut state),
                    Some(0x07 | 0x09) => {
                        if let Some((true, label)) = action5_condition(payload, &state)
                            && let Some(target) = next_action5_label(&labels, label, index)
                        {
                            index = target;
                            continue;
                        }
                    }
                    Some(0x05) => {
                        if let Some((type_id, num_sprites, offset)) = parse_action5_header(payload)
                        {
                            active = Some(PendingAction5Block::from_header(
                                type_id,
                                num_sprites,
                                offset,
                            ));
                        }
                    }
                    _ => {}
                }
            }
            GrfEntry::Real { info, payload } => {
                if let Some(block) = active.as_mut() {
                    if let Some(sprite) =
                        decode_action5_sprite(parsed.container, &sprite_index, info, payload)
                    {
                        block.sprites.push(sprite);
                    }
                    block.seen_real_sprites = block.seen_real_sprites.saturating_add(1);
                    if block.seen_real_sprites >= u16::from(block.num_sprites)
                        && let Some(block) = active.take()
                    {
                        out.push(block.finish());
                    }
                }
            }
        }
        index += 1;
    }
    if let Some(block) = active {
        out.push(block.finish());
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
define_action5_merge!(
    merge_twocc_action5_block,
    ACTION5_TYPE_TWOCC,
    TWOCC_ACTION5_SLOT_COUNT
);
define_action5_merge!(
    merge_tramway_action5_block,
    ACTION5_TYPE_TRAMWAY,
    TRAMWAY_ACTION5_SLOT_COUNT
);

/// Slot Action5 foundations para un `SpriteID` de foundations extra.
///
/// Los 14 cimientos nivelados clásicos (`SPR_FOUNDATION_BASE` 989..1003) no
/// pertenecen a esta tabla; Action5 comienza en `SPR_SLOPES_BASE` (5413).
#[must_use]
pub fn foundation_action5_slot_for_sprite_id(sprite_id: u32) -> Option<usize> {
    let base = crate::map::FOUNDATION_ACTION5_SPRITE_BASE;
    let slot = sprite_id.checked_sub(base)?;
    (slot < u32::try_from(FOUNDATION_ACTION5_SLOT_COUNT).ok()?).then_some(slot as usize)
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
#[allow(clippy::unwrap_used)]
mod slot_helper_tests {
    use super::*;
    use crate::airport_class::AirportSpecId;
    use crate::map::{SLOPE_NE, SLOPE_SE};
    use crate::newgrf_sprites::{
        Action5Block, DecodedSprite, build_real_sprite_v1_uncompressed_payload,
    };
    use crate::rail_type::RailType;

    fn append_pseudo(data: &mut Vec<u8>, payload: &[u8]) {
        data.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_le_bytes());
        data.push(0xFF);
        data.extend_from_slice(payload);
    }

    fn append_real(data: &mut Vec<u8>, payload: &[u8]) {
        data.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_le_bytes());
        data.push(0x01);
        data.extend_from_slice(payload);
    }

    /// GRF mínimo con dos Action5, seleccionados por el mismo patrón
    /// `ActionD(current climate)` + `Action9(label)` de `ogfxe_extra.grf`.
    fn climate_selected_action5_fixture() -> Vec<u8> {
        const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
        let mut data = Vec::new();
        // param[0x7f] = global[0x03] & 0xff (climate).
        append_pseudo(
            &mut data,
            &[0x0D, 0x7F, 0x07, 0x83, 0xFF, 0xFF, 0x00, 0x00, 0x00],
        );
        // Si no es temperado, salta al banco de clima 1 (label 0x10).
        append_pseudo(
            &mut data,
            &[0x09, 0x7F, 0x04, 0x03, 0x00, 0x00, 0x00, 0x00, 0x10],
        );
        append_pseudo(&mut data, &[0x05, ACTION5_TYPE_FOUNDATIONS, 1, 0, 0]);
        append_real(
            &mut data,
            &build_real_sprite_v1_uncompressed_payload(1, 1, 0, 0, &[10]),
        );
        // El banco temperado ya elegido salta al final, sin sobreescribirse.
        append_pseudo(
            &mut data,
            &[0x09, 0x7F, 0x04, 0x02, 0x00, 0x00, 0x00, 0x00, 0x11],
        );
        append_pseudo(&mut data, &[0x10, 0x10]);
        append_pseudo(&mut data, &[0x05, ACTION5_TYPE_FOUNDATIONS, 1, 0, 0]);
        append_real(
            &mut data,
            &build_real_sprite_v1_uncompressed_payload(1, 1, 0, 0, &[20]),
        );
        append_pseudo(&mut data, &[0x10, 0x11]);
        data.extend_from_slice(&0u32.to_le_bytes());

        let sprite_offset = u32::try_from(1 + data.len()).unwrap_or(0);
        let mut out = Vec::new();
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&SIG);
        out.extend_from_slice(&sprite_offset.to_le_bytes());
        out.push(0);
        out.extend_from_slice(&data);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    #[test]
    fn active_action5_collection_honours_actiond_action9_climate_branch() {
        let bytes = climate_selected_action5_fixture();
        let all = collect_action5_blocks(&bytes).unwrap();
        assert_eq!(all.len(), 2);

        let temperate = collect_active_action5_blocks(&bytes, &Action5LoadContext::new(0)).unwrap();
        let arctic = collect_active_action5_blocks(&bytes, &Action5LoadContext::new(1)).unwrap();
        assert_eq!(temperate.len(), 1);
        assert_eq!(arctic.len(), 1);
        assert_eq!(temperate[0].sprites, all[0].sprites);
        assert_eq!(arctic[0].sprites, all[1].sprites);
        assert_ne!(temperate[0].sprites, arctic[0].sprites);
    }

    #[test]
    fn oneway_and_roadstop_and_bridge_slots() {
        assert_eq!(disallowed_road_directions(0x15), 1); // bits + ROAD_Y
        assert_eq!(oneway_action5_slot(0, true, 1), Some(0));
        // Kale_TitleGame (118,29)/(119,29): ROAD_Y, southbound / northbound
        // → `SPR_ONEWAY_BASE + 3/+4` = 6108/6109 en OpenTTD 15.3.
        assert_eq!(oneway_action5_slot(0, false, 1), Some(3));
        assert_eq!(oneway_action5_slot(0, false, 2), Some(4));
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

    fn dummy_sprite(marker: u8) -> DecodedSprite {
        DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![marker, 0, 0, 255],
            mask: Vec::new(),
        }
    }

    #[test]
    fn twocc_and_tramway_merge_do_not_clobber_neighbours() {
        let block_a = Action5Block {
            type_id: ACTION5_TYPE_TWOCC,
            num_sprites: 2,
            offset: 254,
            first_preview: None,
            sprites: vec![dummy_sprite(10), dummy_sprite(11)],
        };
        let mut twocc = vec![None; TWOCC_ACTION5_SLOT_COUNT];
        // Sentinel outside the write range must stay untouched (no neighbour clobber).
        let sentinel = dummy_sprite(99);
        // Pre-fill a slot that the block must not touch when truncated at table end.
        // offset 254 writes slots 254 and 255 only.
        twocc[0] = Some(sentinel.clone());
        merge_twocc_action5_block(&mut twocc, &block_a);
        assert!(twocc[254].is_some());
        assert!(twocc[255].is_some());
        assert_eq!(twocc[0].as_ref().unwrap().rgba[0], 99);
        assert_eq!(twocc.len(), TWOCC_ACTION5_SLOT_COUNT);

        let overflow = Action5Block {
            type_id: ACTION5_TYPE_TWOCC,
            num_sprites: 4,
            offset: 254,
            first_preview: None,
            sprites: vec![
                dummy_sprite(1),
                dummy_sprite(2),
                dummy_sprite(3),
                dummy_sprite(4),
            ],
        };
        let mut twocc2 = vec![None; TWOCC_ACTION5_SLOT_COUNT];
        merge_twocc_action5_block(&mut twocc2, &overflow);
        assert!(twocc2[254].is_some());
        assert!(twocc2[255].is_some());
        // Sprites 3/4 would be slots 256/257 — discarded, table length unchanged.
        assert_eq!(twocc2.len(), TWOCC_ACTION5_SLOT_COUNT);

        let tram = Action5Block {
            type_id: ACTION5_TYPE_TRAMWAY,
            num_sprites: 1,
            offset: 118,
            first_preview: None,
            sprites: vec![dummy_sprite(42)],
        };
        let mut tram_slots = vec![None; TRAMWAY_ACTION5_SLOT_COUNT];
        tram_slots[0] = Some(sentinel);
        merge_tramway_action5_block(&mut tram_slots, &tram);
        assert_eq!(tram_slots[118].as_ref().unwrap().rgba[0], 42);
        assert_eq!(tram_slots[0].as_ref().unwrap().rgba[0], 99);
        // Wrong type_id must not write.
        let wrong = Action5Block {
            type_id: ACTION5_TYPE_TWOCC,
            num_sprites: 1,
            offset: 0,
            first_preview: None,
            sprites: vec![dummy_sprite(7)],
        };
        merge_tramway_action5_block(&mut tram_slots, &wrong);
        assert_eq!(tram_slots[0].as_ref().unwrap().rgba[0], 99);
    }
}
