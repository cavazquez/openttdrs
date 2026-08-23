//! Evaluación de Action2 (variational, random, callbacks).

use super::model::{
    Action2EvalCtx, Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarTerm,
    CALLBACK_FAILED, TrainSpriteGraphics,
};

fn apply_var_adjust(raw: u32, adj: &Action2VarAdjust) -> i32 {
    // Cast wrapping: literales `0x1A` usan `0xFFFFFFFF` → `-1` en i32.
    let mut value = raw.wrapping_shr(u32::from(adj.shift & 0x1F)).cast_signed();
    value &= i32::from(adj.and_mask);
    if let Some(add) = adj.add_val {
        value = value.wrapping_add(i32::from(add));
    }
    if let Some(div) = adj.divide_val
        && div != 0
    {
        value /= i32::from(div);
    } else if let Some(modulo) = adj.modulo_val
        && modulo != 0
    {
        value %= i32::from(modulo);
    }
    value
}

fn read_action2_var(
    gfx: &TrainSpriteGraphics,
    ctx: &mut Action2EvalCtx,
    term: &Action2VarTerm,
    depth: u8,
) -> Option<u32> {
    match term.variable {
        // Literal: `and-mask` selecciona bits de 0xFFFFFFFF.
        0x1A => Some(0xFFFF_FFFF),
        // Resultado del VarAction2 anterior / procedure.
        0x1C => Some(ctx.last_result),
        // Registro persistente `7C[param]`.
        0x7C => {
            let idx = term.param.unwrap_or(0);
            Some(ctx.persistent_registers.get(&idx).copied().unwrap_or(0))
        }
        // Registro temporal `7D[param]`.
        0x7D => {
            let idx = term.param.unwrap_or(0);
            Some(ctx.temp_registers.get(&idx).copied().unwrap_or(0))
        }
        // Procedure call: parámetro = set-id Action2 a invocar.
        0x7E => {
            let proc_id = term.param.unwrap_or(0);
            Some(invoke_action2_procedure(gfx, proc_id, ctx, depth))
        }
        // Parámetro del GRF (`GRFFile::GetParam`).
        0x7F => {
            let idx = usize::from(term.param.unwrap_or(0));
            Some(ctx.grf_params.get(idx).copied().unwrap_or(0))
        }
        v => term
            .param
            .and_then(|parameter| ctx.parameterized_vars.get(&(v, parameter)).copied())
            .or_else(|| ctx.vars.get(&v).copied()),
    }
}

fn eval_term(
    gfx: &TrainSpriteGraphics,
    ctx: &mut Action2EvalCtx,
    term: &Action2VarTerm,
    depth: u8,
) -> Option<i32> {
    let raw = read_action2_var(gfx, ctx, term, depth)?;
    Some(apply_var_adjust(raw, &term.adjust))
}

fn apply_advanced_op(op: u8, val1: i32, val2: i32, ctx: &mut Action2EvalCtx) -> i32 {
    match op {
        0x00 => val1.wrapping_add(val2),
        0x01 => val1.wrapping_sub(val2),
        0x02 => val1.min(val2),
        0x03 => val1.max(val2),
        0x04 => val1.cast_unsigned().min(val2.cast_unsigned()).cast_signed(),
        0x05 => val1.cast_unsigned().max(val2.cast_unsigned()).cast_signed(),
        0x06 => {
            if val2 != 0 {
                val1 / val2
            } else {
                val1
            }
        }
        0x07 => {
            if val2 != 0 {
                val1 % val2
            } else {
                val1
            }
        }
        0x08 => {
            if val2 != 0 {
                (val1.cast_unsigned() / val2.cast_unsigned()).cast_signed()
            } else {
                val1
            }
        }
        0x09 => {
            if val2 != 0 {
                (val1.cast_unsigned() % val2.cast_unsigned()).cast_signed()
            } else {
                val1
            }
        }
        0x0A => val1.wrapping_mul(val2),
        0x0B => val1 & val2,
        0x0C => val1 | val2,
        0x0D => val1 ^ val2,
        // \2sto: temp_registers[val2] = val1
        0x0E => {
            let idx = u8::try_from(val2 & 0xFF).unwrap_or(0);
            ctx.temp_registers.insert(idx, val1.cast_unsigned());
            val1
        }
        // \2rst: result = val2
        0x0F => val2,
        // \2psto: persistent_registers[val2] = val1
        0x10 => {
            let idx = u8::try_from(val2 & 0xFF).unwrap_or(0);
            ctx.persistent_registers.insert(idx, val1.cast_unsigned());
            val1
        }
        0x11 => val1.rotate_right(val2.cast_unsigned() & 31),
        0x12 => match val1.cmp(&val2) {
            std::cmp::Ordering::Less => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Greater => 2,
        },
        0x13 => {
            let a = val1.cast_unsigned();
            let b = val2.cast_unsigned();
            match a.cmp(&b) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Greater => 2,
            }
        }
        0x14 => val1.wrapping_shl(val2.cast_unsigned() & 31),
        0x15 => (val1.cast_unsigned() >> (val2.cast_unsigned() & 31)).cast_signed(),
        0x16 => val1.wrapping_shr(val2.cast_unsigned() & 31),
        _ => val1,
    }
}

/// Invoca un Action2 como procedure (`7E`); el valor calculado alimenta la variable.
fn invoke_action2_procedure(
    gfx: &TrainSpriteGraphics,
    set_id: u8,
    ctx: &mut Action2EvalCtx,
    depth: u8,
) -> u32 {
    if depth >= 8 {
        return 0;
    }
    let mut id = u16::from(set_id);
    for _ in 0..8 {
        let a2 = u8::try_from(id).unwrap_or(u8::MAX);
        if let Some(rnd) = gfx.action2_random.get(&a2) {
            let next = eval_action2_random(rnd, ctx);
            if next & 0x8000 != 0 {
                let v = u32::from(next & 0x7FFF);
                ctx.last_result = v;
                return v;
            }
            id = next;
            continue;
        }
        if let Some(var) = gfx.action2_var.get(&a2).cloned() {
            let next = eval_action2_var(gfx, &var, ctx, depth.saturating_add(1));
            if next & 0x8000 != 0 {
                // `last_result` guarda el acumulador completo (32-bit).
                return ctx.last_result;
            }
            id = next;
            continue;
        }
        // Cadena termina en Action2 básico → 0xFFFF.
        ctx.last_result = 0xFFFF;
        return 0xFFFF;
    }
    ctx.last_result = 0xFFFF;
    0xFFFF
}

pub(super) fn resolve_callback_chain(
    gfx: &TrainSpriteGraphics,
    start_set: u16,
    ctx: &mut Action2EvalCtx,
) -> u16 {
    let mut id = start_set;
    for _ in 0..8 {
        let a2 = u8::try_from(id).unwrap_or(u8::MAX);
        if let Some(rnd) = gfx.action2_random.get(&a2) {
            let next = eval_action2_random(rnd, ctx);
            if next & 0x8000 != 0 {
                return next & 0x7FFF;
            }
            id = next;
            continue;
        }
        if let Some(var) = gfx.action2_var.get(&a2).cloned() {
            let next = eval_action2_var(gfx, &var, ctx, 0);
            if next & 0x8000 != 0 {
                return u16::try_from(ctx.last_result & 0xFFFF).unwrap_or(CALLBACK_FAILED);
            }
            id = next;
            continue;
        }
        // Action2 básico / sin entrada: no es resultado de callback.
        return CALLBACK_FAILED;
    }
    CALLBACK_FAILED
}

pub(super) fn eval_action2_var(
    gfx: &TrainSpriteGraphics,
    entry: &Action2VarEntry,
    ctx: &mut Action2EvalCtx,
    depth: u8,
) -> u16 {
    let Some(mut acc) = eval_term(gfx, ctx, &entry.first, depth) else {
        return entry.default;
    };
    for op in &entry.ops {
        let Some(rhs) = eval_term(gfx, ctx, &op.rhs, depth) else {
            return entry.default;
        };
        acc = apply_advanced_op(op.operator, acc, rhs, ctx);
    }
    ctx.last_result = acc.cast_unsigned();
    // nvar=0: devolver el valor calculado como callback (procedure / result).
    if entry.ranges.is_empty() {
        let low = u16::try_from(acc & 0x7FFF).unwrap_or(0);
        return low | 0x8000;
    }
    let value_u8 = u8::try_from(acc & 0xFF).unwrap_or(0);
    for &(result, low, high) in &entry.ranges {
        if value_u8 >= low && value_u8 <= high {
            return result;
        }
    }
    entry.default
}

pub(super) fn eval_action2_random(entry: &Action2RandomEntry, ctx: &Action2EvalCtx) -> u16 {
    let count_key = entry.consist_count & 0x0F;
    let bits = if entry.typ == 0x84 {
        ctx.consist_random_bits
            .get(&count_key)
            .copied()
            .unwrap_or(ctx.random_bits)
    } else {
        ctx.random_bits
    };
    let n = entry.sets.len();
    if n == 0 {
        return 0;
    }
    let mask = n.next_power_of_two().saturating_sub(1);
    let idx = (usize::try_from(bits >> entry.randbit).unwrap_or(0) & mask) % n;
    entry.sets[idx]
}
