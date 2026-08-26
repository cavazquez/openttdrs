//! Fingerprint estable de contexto Action2 para keys de caché NewGRF.

use openttdrs_core::Action2EvalCtx;

/// Hash de vars/params (y opcionalmente consist random) usados por caches runtime.
pub(crate) fn runtime_fingerprint(
    ctx: &Action2EvalCtx,
    vars: &[u8],
    include_consist_random: bool,
) -> u32 {
    let mut h = ctx.random_bits;
    if include_consist_random {
        for offset in 0u8..=15 {
            if let Some(&bits) = ctx.consist_random_bits.get(&offset) {
                h = h
                    .wrapping_mul(31)
                    .wrapping_add(bits)
                    .wrapping_add(u32::from(offset) << 24);
            }
        }
        // Action2 real vehicle groups select a loaded/loading set by cargo
        // stage. Include that state or a cached empty sprite can survive a
        // station load window and make the runtime appear stuck.
        h = h
            .wrapping_mul(31)
            .wrapping_add(u32::from(ctx.vehicle_loading))
            .wrapping_add(ctx.vehicle_cargo)
            .wrapping_add(ctx.vehicle_capacity.rotate_left(16));
    }
    for &var in vars {
        if let Some(&v) = ctx.vars.get(&var) {
            h = h
                .wrapping_mul(31)
                .wrapping_add(v)
                .wrapping_add(u32::from(var) << 16);
        }
    }
    // Las variables `60+x` pueden consultar varios offsets del mismo scope
    // dentro de un Action2. Ordenar la tabla evita que el orden aleatorio del
    // `HashMap` haga inestable la clave de caché entre frames.
    let mut parameterized: Vec<_> = ctx.parameterized_vars.iter().collect();
    parameterized.sort_unstable_by_key(|entry| *entry.0);
    for (key, value) in parameterized {
        let (variable, parameter) = *key;
        h = h
            .wrapping_mul(31)
            .wrapping_add(*value)
            .wrapping_add(u32::from(variable) << 16)
            .wrapping_add(u32::from(parameter) << 24);
    }
    for (i, &p) in ctx.grf_params.iter().enumerate().take(16) {
        h = h
            .wrapping_mul(31)
            .wrapping_add(p)
            .wrapping_add(u32::try_from(i).unwrap_or(0) << 20);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::newgrf_cache::vars;
    use std::collections::HashMap;

    #[test]
    fn fingerprint_is_deterministic_for_same_vars() {
        let mut ctx = Action2EvalCtx {
            random_bits: 7,
            ..Default::default()
        };
        ctx.vars.insert(0x40, 11);
        ctx.vars.insert(0x5F, 22);
        let a = runtime_fingerprint(&ctx, vars::INDUSTRY, false);
        let b = runtime_fingerprint(&ctx, vars::INDUSTRY, false);
        assert_eq!(a, b);
        assert_ne!(a, 7);
    }

    #[test]
    fn consist_random_changes_train_fingerprint() {
        let mut ctx = Action2EvalCtx {
            random_bits: 1,
            ..Default::default()
        };
        let without = runtime_fingerprint(&ctx, vars::TRAIN, true);
        ctx.consist_random_bits = HashMap::from([(0u8, 99u32)]);
        let with = runtime_fingerprint(&ctx, vars::TRAIN, true);
        assert_ne!(without, with);
    }

    #[test]
    fn parameterized_scope_changes_fingerprint_deterministically() {
        let mut first = Action2EvalCtx::default();
        first.parameterized_vars.insert((0x68, 0x01), 11);
        first.parameterized_vars.insert((0x67, 0x0F), 22);
        let mut second = Action2EvalCtx::default();
        second.parameterized_vars.insert((0x67, 0x0F), 22);
        second.parameterized_vars.insert((0x68, 0x01), 11);
        assert_eq!(
            runtime_fingerprint(&first, vars::ROAD_STOP, false),
            runtime_fingerprint(&second, vars::ROAD_STOP, false)
        );

        second.parameterized_vars.insert((0x68, 0x01), 12);
        assert_ne!(
            runtime_fingerprint(&first, vars::ROAD_STOP, false),
            runtime_fingerprint(&second, vars::ROAD_STOP, false)
        );
    }
}
