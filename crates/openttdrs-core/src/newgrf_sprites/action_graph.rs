//! Parseo y construcción de grafos Action1/2/3 + builders sintéticos de GRF.

use crate::newgrf_actions::{
    ACTION0_FEATURE_AIRCRAFT, ACTION0_FEATURE_AIRPORTS, ACTION0_FEATURE_AIRPORTTILES,
    ACTION0_FEATURE_CANALS, ACTION0_FEATURE_CARGOES, ACTION0_FEATURE_HOUSES,
    ACTION0_FEATURE_INDUSTRIES, ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_OBJECTS,
    ACTION0_FEATURE_RAILTYPES, ACTION0_FEATURE_ROAD_VEHICLES, ACTION0_FEATURE_ROADSTOPS,
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_SHIPS, ACTION0_FEATURE_STATIONS,
    ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::model::{
    Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
    DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics, WagonOverrideAssign,
};
use super::pixel_codec::{decode_real_sprite_entry, index_sprite_section, resolve_fd_sprite};

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

/// Action2 básico (vehículos / stations / roadtypes): `02 <feat> <set-id> <n1> <n2> <words…>`.
///
/// Devuelve `(action2_set_id, primer set Action1)`. Variational (`n1≥0x80`) → None.
/// Roadtypes usan `01 00` (un set); stations pueden tener `n1=0` y `n2>0`.
pub(super) fn parse_action2_basic(payload: &[u8], feature: u8) -> Option<(u8, u16)> {
    if payload.len() < 5 || payload[0] != 0x02 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let num_ent1 = payload[3];
    let num_ent2 = payload[4];
    // Variational / random Action2 → `parse_action2_variational` / `parse_action2_random`.
    if num_ent1 >= 0x80 {
        return None;
    }
    let n_words = usize::from(num_ent1) + usize::from(num_ent2);
    if n_words == 0 {
        return None;
    }
    let words_start = 5usize;
    let words_end = words_start.checked_add(n_words.checked_mul(2)?)?;
    if payload.len() < words_end {
        return None;
    }
    let a1 = u16::from_le_bytes([payload[words_start], payload[words_start + 1]]);
    Some((set_id, a1))
}

/// Lee un valor little-endian del ancho indicado por el tipo Action2.
///
/// Los tipos deterministas de Action2 codifican todos sus operandos con el
/// mismo ancho que la variable principal: byte (`0x81/0x82`), word
/// (`0x85/0x86`) o dword (`0x89/0x8A`). Mantener el lector aquí evita que una
/// máscara de word/dword se interprete como una secuencia de bytes y desplace
/// el resto del grupo.
fn read_var_size(payload: &[u8], i: &mut usize, size: usize) -> Option<u32> {
    let end = i.checked_add(size)?;
    let bytes = payload.get(*i..end)?;
    *i = end;
    match size {
        1 => Some(u32::from(bytes[0])),
        2 => Some(u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))),
        4 => Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        _ => None,
    }
}

/// Lee `variable` [+param `60+x`] + `varadjust`. Devuelve `(término, bit5_continúa)`.
fn parse_var_term(
    payload: &[u8],
    i: &mut usize,
    var_size: usize,
) -> Option<(Action2VarTerm, bool)> {
    if *i >= payload.len() {
        return None;
    }
    let variable = payload[*i];
    *i += 1;
    let param = if (0x60..=0x7F).contains(&variable) {
        if *i >= payload.len() {
            return None;
        }
        let p = payload[*i];
        *i += 1;
        Some(p)
    } else {
        None
    };
    if *i >= payload.len() {
        return None;
    }
    let shift_num = payload[*i];
    *i += 1;
    let continued = shift_num & 0x20 != 0;
    let do_divide = shift_num & 0x40 != 0;
    let do_modulo = shift_num & 0x80 != 0;
    if do_divide && do_modulo {
        return None;
    }
    let and_mask = read_var_size(payload, i, var_size)?;
    let mut add_val = None;
    let mut divide_val = None;
    let mut modulo_val = None;
    if do_divide || do_modulo {
        add_val = Some(read_var_size(payload, i, var_size)?);
        let operand = read_var_size(payload, i, var_size)?;
        if do_divide {
            divide_val = Some(operand);
        } else {
            modulo_val = Some(operand);
        }
    }
    Some((
        Action2VarTerm {
            variable,
            param,
            adjust: Action2VarAdjust {
                shift: shift_num & 0x1F,
                and_mask,
                add_val,
                divide_val,
                modulo_val,
            },
        },
        continued,
    ))
}

/// Action2 variational determinista: byte/word/dword, divide/modulo o
/// advanced (bit 5).
pub(super) fn parse_action2_variational(
    payload: &[u8],
    feature: u8,
) -> Option<(u8, Action2VarEntry)> {
    if payload.len() < 8 || payload[0] != 0x02 || payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let typ = payload[3];
    let var_size = match typ {
        0x81 | 0x82 => 1,
        0x85 | 0x86 => 2,
        0x89 | 0x8A => 4,
        _ => return None,
    };
    let mut i = 4usize;
    let (first, mut continued) = parse_var_term(payload, &mut i, var_size)?;
    let mut ops = Vec::new();
    while continued {
        if i >= payload.len() {
            return None;
        }
        let operator = payload[i];
        i += 1;
        let (rhs, next) = parse_var_term(payload, &mut i, var_size)?;
        ops.push(Action2VarOp { operator, rhs });
        continued = next;
        if ops.len() > 32 {
            return None;
        }
    }
    if i >= payload.len() {
        return None;
    }
    let nvar = payload[i];
    i += 1;
    let mut ranges = Vec::with_capacity(usize::from(nvar));
    for _ in 0..nvar {
        let result_bytes = payload.get(i..i.checked_add(2)?)?;
        let result = u16::from_le_bytes([result_bytes[0], result_bytes[1]]);
        i += 2;
        let low = read_var_size(payload, &mut i, var_size)?;
        let high = read_var_size(payload, &mut i, var_size)?;
        ranges.push((result, low, high));
    }
    if i + 2 > payload.len() {
        return None;
    }
    let default = u16::from_le_bytes([payload[i], payload[i + 1]]);
    Some((
        set_id,
        Action2VarEntry {
            first,
            ops,
            ranges,
            default,
        },
    ))
}

/// Action2 random `0x80`/`0x83`/`0x84`: triggers + randbit + n sets (potencia de 2).
pub(super) fn parse_action2_random(
    payload: &[u8],
    feature: u8,
) -> Option<(u8, Action2RandomEntry)> {
    if payload.len() < 8 || payload[0] != 0x02 || payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let typ = payload[3];
    if typ != 0x80 && typ != 0x83 && typ != 0x84 {
        return None;
    }
    let mut i = 4usize;
    let consist_count = if typ == 0x84 {
        if i >= payload.len() {
            return None;
        }
        let c = payload[i];
        i += 1;
        c
    } else {
        0
    };
    if i + 3 > payload.len() {
        return None;
    }
    let triggers = payload[i];
    let randbit = payload[i + 1];
    let nrand = payload[i + 2];
    i += 3;
    if nrand == 0 || !nrand.is_power_of_two() {
        return None;
    }
    let n = usize::from(nrand);
    let words_end = i.checked_add(n.checked_mul(2)?)?;
    if payload.len() < words_end {
        return None;
    }
    let mut sets = Vec::with_capacity(n);
    for k in 0..n {
        let o = i + k * 2;
        sets.push(u16::from_le_bytes([payload[o], payload[o + 1]]));
    }
    Some((
        set_id,
        Action2RandomEntry {
            typ,
            consist_count,
            triggers,
            randbit,
            sets,
        },
    ))
}

/// Action2 real group used by vehicle features:
/// `02 feature set-id num-loaded num-loading [loaded ids] [loading ids]`.
///
/// Both lists are meaningful. `OpenTTD` picks a proportional entry from the
/// active list according to the vehicle cargo amount and resolves it as an
/// Action1 sprite set.
fn parse_action2_real(payload: &[u8], feature: u8) -> Option<(u8, super::model::Action2RealEntry)> {
    if payload.len() < 6
        || payload[0] != 0x02
        || !matches!(
            feature,
            ACTION0_FEATURE_TRAINS
                | ACTION0_FEATURE_ROAD_VEHICLES
                | ACTION0_FEATURE_SHIPS
                | ACTION0_FEATURE_AIRCRAFT
        )
    {
        return None;
    }
    let set_id = payload[2];
    let num_loaded = usize::from(payload[3]);
    let num_loading = usize::from(payload[4]);
    if num_loaded.saturating_add(num_loading) == 0 {
        return None;
    }
    let words = num_loaded.saturating_add(num_loading);
    let end = 5usize.checked_add(words.checked_mul(2)?)?;
    if payload.len() < end {
        return None;
    }
    let mut loaded = Vec::with_capacity(num_loaded);
    for index in 0..num_loaded {
        let offset = 5 + index * 2;
        loaded.push(u16::from_le_bytes([payload[offset], payload[offset + 1]]));
    }
    let mut loading = Vec::with_capacity(num_loading);
    let start = 5 + num_loaded * 2;
    for index in 0..num_loading {
        let offset = start + index * 2;
        loading.push(u16::from_le_bytes([payload[offset], payload[offset + 1]]));
    }
    Some((set_id, super::model::Action2RealEntry { loaded, loading }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedAction3 {
    /// Bit 7 de `n-id`: la definición se aplica a los motores de la cadena
    /// Action3 anterior, en vez de establecer el grupo propio del vehículo.
    wagon_override: bool,
    /// IDs locales de esta definición (byte o `ExtendedByte`).
    ids: Vec<u16>,
    assigns: Vec<TrainSpriteAssign>,
    specific: Vec<((u8, u8), u16)>,
    extended: Vec<(u16, u16)>,
    extended_specific: Vec<((u16, u8), u16)>,
}

fn read_extended_byte(payload: &[u8], i: &mut usize) -> Option<u16> {
    let value = u16::from(*payload.get(*i)?);
    *i += 1;
    if value == 0xFF {
        let bytes = payload.get(*i..i.checked_add(2)?)?;
        *i += 2;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    } else {
        Some(value)
    }
}

fn parse_action3_feature(payload: &[u8], feature: u8) -> Option<ParsedAction3> {
    // 03 <feature> <n-id> <ids…> <num-cid> [cargo…] <default:u16>
    if payload.len() < 6 || payload[0] != 0x03 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    // Bit 7 marks a wagon-override definition for vehicle features.  Other
    // features use all eight bits for the number of local IDs.
    let wagon_override = is_vehicle_feature(feature) && payload[2] & 0x80 != 0;
    let n_id = if wagon_override {
        payload[2] & 0x7F
    } else {
        payload[2]
    };
    if n_id == 0 {
        return None;
    }
    let mut i = 3usize;
    let mut ids = Vec::with_capacity(usize::from(n_id));
    for _ in 0..n_id {
        ids.push(read_extended_byte(payload, &mut i)?);
    }
    let num_cid = *payload.get(i)?;
    i += 1;
    let mut specific = Vec::with_capacity(usize::from(num_cid) * ids.len());
    let mut extended_specific = Vec::new();
    for _ in 0..num_cid {
        let selector = *payload.get(i)?;
        let set_bytes = payload.get(i + 1..i.checked_add(3)?)?;
        let set_id = u16::from_le_bytes([set_bytes[0], set_bytes[1]]);
        for &local_id in &ids {
            if let Ok(byte_id) = u8::try_from(local_id)
                && byte_id != u8::MAX
            {
                specific.push(((byte_id, selector), set_id));
            } else {
                extended_specific.push(((local_id, selector), set_id));
            }
        }
        i += 3;
    }
    let default_bytes = payload.get(i..i.checked_add(2)?)?;
    let default_set = u16::from_le_bytes([default_bytes[0], default_bytes[1]]);
    let mut defaults = Vec::new();
    let mut extended = Vec::new();
    for &local_id in &ids {
        if let Ok(byte_id) = u8::try_from(local_id)
            && byte_id != u8::MAX
        {
            defaults.push(TrainSpriteAssign {
                local_id: byte_id,
                set_id: default_set,
            });
        } else {
            extended.push((local_id, default_set));
        }
    }
    Some(ParsedAction3 {
        wagon_override,
        ids,
        assigns: defaults,
        specific,
        extended,
        extended_specific,
    })
}

const fn is_vehicle_feature(feature: u8) -> bool {
    matches!(
        feature,
        ACTION0_FEATURE_TRAINS
            | ACTION0_FEATURE_ROAD_VEHICLES
            | ACTION0_FEATURE_SHIPS
            | ACTION0_FEATURE_AIRCRAFT
    )
}

/// Aplica una definición Action3 al grafo, incluyendo la cadena de motores
/// que precede a un *wagon override*.
fn apply_action3(
    out: &mut TrainSpriteGraphics,
    parsed: ParsedAction3,
    feature: u8,
    last_engines: &mut Vec<u16>,
) {
    if parsed.wagon_override {
        // OpenTTD ignora una definición de override sin una cadena de motores
        // previa. Mantener ese comportamiento evita inventar asignaciones.
        if !last_engines.is_empty() {
            for &((wagon_local_id, selector), set_id) in &parsed.specific {
                for &overriding_local_id in last_engines.iter() {
                    out.wagon_overrides.push(WagonOverrideAssign {
                        wagon_local_id: u16::from(wagon_local_id),
                        overriding_local_id,
                        selector,
                        set_id,
                    });
                }
            }
            for &((wagon_local_id, selector), set_id) in &parsed.extended_specific {
                for &overriding_local_id in last_engines.iter() {
                    out.wagon_overrides.push(WagonOverrideAssign {
                        wagon_local_id,
                        overriding_local_id,
                        selector,
                        set_id,
                    });
                }
            }
            for (wagon_local_id, set_id) in parsed
                .assigns
                .iter()
                .map(|assign| (u16::from(assign.local_id), assign.set_id))
            {
                for &overriding_local_id in last_engines.iter() {
                    out.wagon_overrides.push(WagonOverrideAssign {
                        wagon_local_id,
                        overriding_local_id,
                        selector: 0xFF,
                        set_id,
                    });
                }
            }
            for &(wagon_local_id, set_id) in &parsed.extended {
                for &overriding_local_id in last_engines.iter() {
                    out.wagon_overrides.push(WagonOverrideAssign {
                        wagon_local_id,
                        overriding_local_id,
                        selector: 0xFF,
                        set_id,
                    });
                }
            }
        }
        return;
    }

    if is_vehicle_feature(feature) {
        // The next override references exactly this list, including extended
        // IDs, not the Action3 set IDs.
        last_engines.clone_from(&parsed.ids);
    }
    out.assigns.extend(parsed.assigns);
    out.specific_assigns.extend(parsed.specific);
    out.extended_assigns.extend(parsed.extended);
    out.extended_specific_assigns
        .extend(parsed.extended_specific);
}

/// Índice sprite section v2: `id` → lista `(info, body)` (body = tras el BYTE info).
#[must_use]
/// Features con cadena Action3→Action2→Action1 (vehículos, estaciones,
/// industrias/teselas y tipos de infraestructura).
fn supports_action2_chain(feature: u8) -> bool {
    matches!(
        feature,
        ACTION0_FEATURE_TRAINS
            | ACTION0_FEATURE_ROAD_VEHICLES
            | ACTION0_FEATURE_SHIPS
            | ACTION0_FEATURE_AIRCRAFT
            | ACTION0_FEATURE_STATIONS
            | ACTION0_FEATURE_OBJECTS
            | ACTION0_FEATURE_RAILTYPES
            | ACTION0_FEATURE_ROADTYPES
            | ACTION0_FEATURE_ROADSTOPS
            | ACTION0_FEATURE_INDUSTRIES
            | ACTION0_FEATURE_INDUSTRYTILES
            | ACTION0_FEATURE_HOUSES
            | ACTION0_FEATURE_AIRPORTTILES
            | ACTION0_FEATURE_AIRPORTS
            | ACTION0_FEATURE_CARGOES
    )
}

/// Recorre el GRF y extrae sets Action1 + Action2 + asignaciones Action3.
///
/// # Errors
///
/// Contenedor inválido.
#[allow(clippy::too_many_lines)]
pub fn collect_feature_sprite_graphics(
    data: &[u8],
    feature: u8,
) -> Result<TrainSpriteGraphics, GrfScanError> {
    let parsed = parse_grf_full(data)?;
    let container = parsed.container;
    let section = parsed.data_section;
    let sprite_index = index_sprite_section(parsed.sprite_section);
    let mut out = TrainSpriteGraphics::default();
    let mut current_set: Vec<DecodedSprite> = Vec::new();
    let mut views_left_in_set = 0u8;
    let mut sets_left = 0u8;
    let mut views_per_set = 0u8;
    // `VehicleMapSpriteGroup` keeps the engine IDs from the last non-override
    // Action3 until the next override definition in the same feature.
    let mut last_engines = Vec::new();

    walk_grf_entries(section, container, |entry| match entry {
        GrfEntry::Pseudo(payload) => {
            if let Some((ns, ne)) = parse_action1_feature(payload, feature) {
                if !current_set.is_empty() {
                    out.sets.push(std::mem::take(&mut current_set));
                }
                sets_left = ns;
                views_per_set = ne;
                views_left_in_set = ne;
            } else if let Some((a2_id, real)) = parse_action2_real(payload, feature) {
                // Keep the historical static mapping for preview/legacy callers;
                // runtime contexts select the proportional loaded/loading entry.
                if let Some(&first) = real.loaded.first().or_else(|| real.loading.first()) {
                    out.action2_to_action1.insert(a2_id, first);
                }
                out.action2_real.insert(a2_id, real);
            } else if supports_action2_chain(feature)
                && let Some((a2_id, a1_idx)) = parse_action2_basic(payload, feature)
            {
                out.action2_to_action1.insert(a2_id, a1_idx);
            } else if supports_action2_chain(feature)
                && let Some((a2_id, var)) = parse_action2_variational(payload, feature)
            {
                out.action2_var.insert(a2_id, var);
            } else if supports_action2_chain(feature)
                && let Some((a2_id, rnd)) = parse_action2_random(payload, feature)
            {
                out.action2_random.insert(a2_id, rnd);
            } else if let Some(parsed_action3) = parse_action3_feature(payload, feature) {
                apply_action3(&mut out, parsed_action3, feature, &mut last_engines);
            }
        }
        GrfEntry::Real { info, payload } => {
            if sets_left > 0 || views_left_in_set > 0 {
                let decoded = if container == GrfContainerVersion::V2 && info == 0xFD {
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
                if let Some(decoded) = decoded {
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
            }
        }
    });

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

/// Action1/2/3 road vehicles, incluido el selector Action3 por cargo.
pub fn collect_road_vehicle_sprite_graphics(
    data: &[u8],
) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROAD_VEHICLES)
}

/// Action1/2/3 ships, incluido el selector Action3 por cargo.
pub fn collect_ship_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_SHIPS)
}

/// Action1/2/3 aircraft, incluido el selector Action3 por cargo.
pub fn collect_aircraft_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_AIRCRAFT)
}

/// Action1/3 roadtypes.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADTYPES)
}

/// Action1/2/3 `RailTypes` (`0x10`), incluidos grupos Action3 por `RailSpriteType`.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_railtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_RAILTYPES)
}

/// Action1/3 stations.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_station_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_STATIONS)
}

/// Action1/3 industry tiles.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_industry_tile_sprite_graphics(
    data: &[u8],
) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_INDUSTRYTILES)
}

/// Action1/2/3 industries (`0x0A`), incluidos callbacks de ubicación.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_industry_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_INDUSTRIES)
}

/// Action1/3 houses (`0x07`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_house_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_HOUSES)
}

/// Action1/3 objects.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_object_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_OBJECTS)
}

/// Action1/3 road stops.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadstop_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADSTOPS)
}

/// Action1/3 canals (`0x05`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_canal_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_CANALS)
}

/// Action1/3 airport tiles (`0x11`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_airport_tile_sprite_graphics(
    data: &[u8],
) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_AIRPORTTILES)
}

/// Action1/3 airports (`0x0D`): default + purchase (`0xFF`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_airport_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_AIRPORTS)
}

/// Action1/3 cargoes (`0x0B`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_cargo_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_CARGOES)
}

#[cfg(test)]
mod tests {
    use crate::newgrf_sprites::Action2EvalCtx;

    use super::*;

    #[test]
    fn parse_vehicle_real_group_keeps_loaded_and_loading_lists() {
        // 02 <trains> <set=7> <loaded=2> <loading=1> <10,11> <12>
        let payload = [0x02, ACTION0_FEATURE_TRAINS, 7, 2, 1, 10, 0, 11, 0, 12, 0];
        let Some((_, group)) = parse_action2_real(&payload, ACTION0_FEATURE_TRAINS) else {
            panic!("vehicle real group should parse");
        };
        assert_eq!(group.loaded, vec![10, 11]);
        assert_eq!(group.loading, vec![12]);
    }

    #[test]
    fn parse_action2_variational_supports_byte_word_and_dword_masks() {
        let byte = [
            0x02,
            ACTION0_FEATURE_TRAINS,
            1,
            0x81,
            0x1A,
            0,
            0x7F,
            0,
            0,
            0,
        ];
        let Some((_, byte_entry)) = parse_action2_variational(&byte, ACTION0_FEATURE_TRAINS) else {
            panic!("byte deterministic group should parse");
        };
        assert_eq!(byte_entry.first.adjust.and_mask, 0x7F);

        let word = [
            0x02,
            ACTION0_FEATURE_TRAINS,
            2,
            0x85,
            0x1A,
            0,
            0x34,
            0x12,
            0,
            0,
            0,
            0,
        ];
        let Some((_, word_entry)) = parse_action2_variational(&word, ACTION0_FEATURE_TRAINS) else {
            panic!("word deterministic group should parse");
        };
        assert_eq!(word_entry.first.adjust.and_mask, 0x1234);

        let dword = [
            0x02,
            ACTION0_FEATURE_TRAINS,
            3,
            0x89,
            0x1A,
            0,
            0x78,
            0x56,
            0x34,
            0x12,
            0,
            0,
            0,
        ];
        let Some((_, dword_entry)) = parse_action2_variational(&dword, ACTION0_FEATURE_TRAINS)
        else {
            panic!("dword deterministic group should parse");
        };
        assert_eq!(dword_entry.first.adjust.and_mask, 0x1234_5678);
    }

    #[test]
    fn action2_sto_writes_extended_sprite_stack_register() {
        // 7 + (STO target 0x100) writes register 0x100 while resolving the
        // calculated result. This is the contract used by SpriteStack.
        let payload = [
            0x02,
            ACTION0_FEATURE_TRAINS,
            7,
            0x85,
            0x1A,
            0x20,
            0x07,
            0x00,
            0x0E,
            0x1A,
            0x00,
            0x00,
            0x01,
            0,
            0,
            0,
        ];
        let Some((set_id, entry)) = parse_action2_variational(&payload, ACTION0_FEATURE_TRAINS)
        else {
            panic!("word advanced group should parse");
        };
        assert_eq!(entry.ops.len(), 1);
        assert_eq!(entry.ops[0].rhs.adjust.and_mask, 0x100);

        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(set_id, entry);
        let mut ctx = Action2EvalCtx::default();
        let _ = gfx.resolve_action1_set_ctx(u16::from(set_id), &mut ctx);
        assert_eq!(ctx.registers_100.get(&0x100), Some(&7));
    }

    #[test]
    fn parse_action3_accepts_extended_vehicle_local_id() {
        // 03 <trains> <1 id> <extended 1234> <1 cargo> <passengers> <default>
        let payload = [
            0x03,
            ACTION0_FEATURE_TRAINS,
            1,
            0xFF,
            0xD2,
            0x04,
            1,
            0,
            2,
            0,
            2,
            0,
        ];
        let Some(parsed) = parse_action3_feature(&payload, ACTION0_FEATURE_TRAINS) else {
            panic!("extended Action3 mapping should parse");
        };
        assert!(!parsed.wagon_override);
        assert!(parsed.assigns.is_empty());
        assert!(parsed.specific.is_empty());
        assert_eq!(parsed.extended, vec![(1234, 2)]);
        assert_eq!(parsed.extended_specific, vec![((1234, 0), 2)]);

        let mut gfx = TrainSpriteGraphics {
            sets: vec![
                vec![],
                vec![],
                vec![DecodedSprite {
                    width: 1,
                    height: 1,
                    x_offs: 0,
                    y_offs: 0,
                    rgba: vec![255, 255, 255, 255],
                    mask: Vec::new(),
                }],
            ],
            ..TrainSpriteGraphics::default()
        };
        gfx.extended_assigns = parsed.extended;
        gfx.extended_specific_assigns = parsed.extended_specific.into_iter().collect();
        assert!(gfx.views_for_local_id_u16(1234).is_some());
        assert!(
            gfx.views_for_specific_u16_ctx(1234, 0, &mut Action2EvalCtx::default())
                .is_some()
        );
    }

    #[test]
    fn action3_wagon_override_keeps_previous_engine_chain() {
        // Base chain: engines 3 and 0x1234. The high bit in the next n-id
        // marks a wagon override for wagon 7.
        let base = [
            0x03,
            ACTION0_FEATURE_TRAINS,
            2,
            3,
            0xFF,
            0xD2,
            0x04,
            1,
            0,
            5,
            0,
            6,
            0,
        ];
        let override_payload = [0x03, ACTION0_FEATURE_TRAINS, 0x80 | 1, 7, 1, 0, 8, 0, 9, 0];
        let Some(base) = parse_action3_feature(&base, ACTION0_FEATURE_TRAINS) else {
            panic!("base Action3 mapping should parse");
        };
        let Some(override_payload) =
            parse_action3_feature(&override_payload, ACTION0_FEATURE_TRAINS)
        else {
            panic!("override Action3 mapping should parse");
        };
        assert!(!base.wagon_override);
        assert!(override_payload.wagon_override);

        let mut gfx = TrainSpriteGraphics::default();
        let mut last_engines = Vec::new();
        apply_action3(&mut gfx, base, ACTION0_FEATURE_TRAINS, &mut last_engines);
        apply_action3(
            &mut gfx,
            override_payload,
            ACTION0_FEATURE_TRAINS,
            &mut last_engines,
        );
        assert_eq!(last_engines, vec![3, 1234]);
        assert_eq!(
            gfx.wagon_overrides,
            vec![
                WagonOverrideAssign {
                    wagon_local_id: 7,
                    overriding_local_id: 3,
                    selector: 0,
                    set_id: 8,
                },
                WagonOverrideAssign {
                    wagon_local_id: 7,
                    overriding_local_id: 1234,
                    selector: 0,
                    set_id: 8,
                },
                WagonOverrideAssign {
                    wagon_local_id: 7,
                    overriding_local_id: 3,
                    selector: 0xFF,
                    set_id: 9,
                },
                WagonOverrideAssign {
                    wagon_local_id: 7,
                    overriding_local_id: 1234,
                    selector: 0xFF,
                    set_id: 9,
                },
            ]
        );

        let sprite = |red: u8| DecodedSprite {
            width: 1,
            height: 1,
            x_offs: i16::from(red),
            y_offs: 0,
            rgba: vec![red, 0, 0, 255],
            mask: Vec::new(),
        };
        gfx.sets = (0_u8..10).map(|index| vec![sprite(index)]).collect();
        let Some(specific) = gfx.views_for_wagon_override_u16_ctx(
            7,
            3,
            Some(crate::cargo::CargoType::Passengers),
            &mut Action2EvalCtx::default(),
        ) else {
            panic!("cargo-specific wagon override");
        };
        assert_eq!(specific[0].x_offs, 8);
        let Some(default) = gfx.views_for_wagon_override_u16_ctx(
            7,
            3,
            Some(crate::cargo::CargoType::Coal),
            &mut Action2EvalCtx::default(),
        ) else {
            panic!("default wagon override");
        };
        assert_eq!(default[0].x_offs, 9);
    }
}
