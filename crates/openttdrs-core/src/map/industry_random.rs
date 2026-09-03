//! Random bits (`m3`) y triggers (`m6` bits 3–5) de teselas industria.
//!
//! Paridad con `industry_map.h` / `TriggerIndustryTileRandomisation`
//! (`newgrf_industrytiles.cpp`). La ruta con catálogo respeta el no-op vanilla,
//! los triggers pendientes de grupos estáticos y la máscara de
//! `ResolveRerandomisation`; la API sin catálogo conserva un fallback
//! determinista para herramientas legacy.

use super::{Map, Tile, TileCoord, TileKind, industry_gfx, industry_instance_id};
use crate::industry::Industry;
use crate::industry_spec::IndustrySpecDef;
use crate::industry_tile::{IndustryTileSpecDef, industry_tile_spec_def};
use crate::newgrf_callback::writeback_industry_tile_parent_persistent_registers;
use crate::town::Town;
use crate::world_gen::Climate;

/// `IndustryRandomTrigger` (`industry_type.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IndustryRandomTrigger {
    /// Tile loop periódico.
    TileLoop = 1 << 0,
    /// Tick de la industria.
    IndustryTick = 1 << 1,
    /// Cargo entregado / recibido.
    CargoReceived = 1 << 2,
}

impl IndustryRandomTrigger {
    #[must_use]
    pub const fn bit(self) -> u8 {
        self as u8
    }
}

/// Máscara de los 3 bits de triggers en `m6` (bits 3–5).
pub const INDUSTRY_RANDOM_TRIGGERS_MASK: u8 = 0x38; // bits 3..5

/// OpenTTD `GetIndustryRandomBits` — byte completo `m3`.
#[must_use]
pub const fn industry_random_bits(tile: &Tile) -> u8 {
    tile.m3
}

/// OpenTTD `SetIndustryRandomBits`.
pub fn set_industry_random_bits(tile: &mut Tile, bits: u8) {
    tile.m3 = bits;
}

/// OpenTTD `GetIndustryRandomTriggers` — `GB(m6, 3, 3)`.
#[must_use]
pub const fn industry_random_triggers(tile: &Tile) -> u8 {
    (tile.m6 >> 3) & 0x07
}

/// OpenTTD `SetIndustryRandomTriggers` — `SB(m6, 3, 3, …)` preservando gfx bit 2 y 6–7.
pub fn set_industry_random_triggers(tile: &mut Tile, triggers: u8) {
    tile.m6 = (tile.m6 & !INDUSTRY_RANDOM_TRIGGERS_MASK) | ((triggers & 0x07) << 3);
}

/// RNG determinista para bits de industria (estilo `tree_rng`).
#[must_use]
pub fn industry_tile_rng(world_seed: u64, tick: u64, c: TileCoord, salt: u64) -> u8 {
    let mut x = world_seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick)
        .wrapping_add(u64::from(c.x.cast_unsigned()).wrapping_mul(0xC2B2_AE3D))
        .wrapping_add(u64::from(c.y.cast_unsigned()).wrapping_mul(0x1656_67B1))
        .wrapping_add(salt.wrapping_mul(0x27BB_2EE6_87B0_B0FD));
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    u8::try_from(x & 0xFF).unwrap_or(0)
}

fn industry_random_word(world_seed: u64, tick: u64, c: TileCoord, salt: u64) -> u16 {
    u16::from(industry_tile_rng(world_seed, tick, c, salt))
        | (u16::from(industry_tile_rng(world_seed, tick, c, salt ^ 0xA5A5_5A5A)) << 8)
}

/// Inicializa `m3` / triggers como `MakeIndustry` (triggers vacíos).
pub fn init_industry_tile_random(tile: &mut Tile, random_bits: u8) {
    set_industry_random_bits(tile, random_bits);
    set_industry_random_triggers(tile, 0);
}

/// Acumula un trigger, reseedea `m3` y limpia triggers (fallback legacy).
///
/// #266: el reseed alimenta `resolve_industry_tile_random_trigger` cuando el
/// caller tiene un `IndustryTileSpecDef` con Action2 random (call site separado).
///
/// Devuelve `true` si la tesela cambió.
pub fn trigger_industry_tile_randomisation(
    tile: &mut Tile,
    trigger: IndustryRandomTrigger,
    world_seed: u64,
    tick: u64,
    c: TileCoord,
) -> bool {
    if tile.kind != TileKind::Industry {
        return false;
    }
    let before_m3 = tile.m3;
    let before_m6 = tile.m6;
    let waiting = industry_random_triggers(tile) | trigger.bit();
    set_industry_random_triggers(tile, waiting);
    // Consume todos los triggers pendientes: reseed completo de m3.
    let new_bits = industry_tile_rng(world_seed, tick, c, u64::from(waiting));
    set_industry_random_bits(tile, new_bits);
    set_industry_random_triggers(tile, 0);
    tile.m3 != before_m3 || tile.m6 != before_m6
}

/// Tile loop: `IndustryRandomTrigger::TileLoop` sobre teselas ya visitadas.
///
/// Para la simulación con NewGRF usar
/// [`advance_industry_tile_randomisation_from_visits_with_catalog`].
pub fn advance_industry_tile_randomisation_from_visits(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, Tile)],
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &(coord, tile) in visits {
        if tile.kind != TileKind::Industry {
            continue;
        }
        let Some(mut live) = map.get(coord) else {
            continue;
        };
        if !trigger_industry_tile_randomisation(
            &mut live,
            IndustryRandomTrigger::TileLoop,
            world_seed,
            tick,
            coord,
        ) {
            continue;
        }
        if map.set_tile(coord, live).is_ok() {
            dirty.push(coord);
        }
    }
    dirty
}

/// Variante con los catálogos completos de la partida.
///
/// `OpenTTD` sólo ejecuta `TriggerIndustryTileRandomisation` cuando la
/// tesela tiene un grupo de sprites `NewGRF`. Las teselas vanilla no consumen
/// RNG ni modifican `m3`; las teselas `NewGRF` conservan los triggers pendientes
/// hasta que el grafo Action2 alcanzable declara que debe consumirlos. Esta
/// ruta mantiene ese contrato y reseedea únicamente la máscara devuelta por
/// `ResolveRerandomisation`, en lugar de reemplazar el byte entero.
#[allow(clippy::too_many_arguments)]
pub fn advance_industry_tile_randomisation_from_visits_with_catalog(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, Tile)],
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
) -> Vec<TileCoord> {
    advance_industry_tile_randomisation_from_visits_with_catalog_inner(
        map,
        tick,
        world_seed,
        visits,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        IndustryRandomTrigger::TileLoop,
        |_, _, _, _| {},
        |_, _, _| {},
    )
}

/// Variante con industrias mutables: la evaluación de `ResolveRerandomisation`
/// puede escribir `\\2psto` en el scope parent de la industria propietaria.
///
/// El snapshot evita aliasar el slice mutable mientras se consultan variables
/// globales. El PSA vivo se inyecta antes de cada grupo y se copia de vuelta
/// después, por lo que varias teselas de la misma huella comparten el storage
/// como en `IndustriesScopeResolver::StorePSA`.
#[allow(clippy::too_many_arguments)]
pub fn advance_industry_tile_randomisation_from_visits_with_catalog_and_world(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, Tile)],
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
) -> Vec<TileCoord> {
    trigger_industry_randomisation_from_visits_with_catalog_and_world(
        map,
        tick,
        world_seed,
        visits,
        IndustryRandomTrigger::TileLoop,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
    )
}

/// Dispara un trigger NewGRF sobre un conjunto de teselas y conserva el
/// contexto parent de las industrias vivas. El `IndustryID`/footprint se
/// resuelve por `m2` antes de aplicar cualquier reseed parent.
#[allow(clippy::too_many_arguments)]
pub fn trigger_industry_randomisation_at_with_catalog_and_world(
    map: &mut Map,
    tiles: &[TileCoord],
    trigger: IndustryRandomTrigger,
    world_seed: u64,
    tick: u64,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
) -> Vec<TileCoord> {
    let visits: Vec<_> = tiles
        .iter()
        .filter_map(|&coord| map.get(coord).map(|tile| (coord, tile)))
        .collect();
    trigger_industry_randomisation_from_visits_with_catalog_and_world(
        map,
        tick,
        world_seed,
        &visits,
        trigger,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
    )
}

#[allow(clippy::too_many_arguments)]
fn trigger_industry_randomisation_from_visits_with_catalog_and_world(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, Tile)],
    trigger: IndustryRandomTrigger,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
) -> Vec<TileCoord> {
    let snapshot = industries.to_vec();
    let mut parent_reseed_masks = std::collections::HashMap::<usize, u32>::new();
    let dirty = advance_industry_tile_randomisation_from_visits_with_catalog_inner(
        map,
        tick,
        world_seed,
        visits,
        &snapshot,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        trigger,
        |map, coord, ctx, write_back| {
            if let Some(index) = industry_index_for_tile(map, &snapshot, coord) {
                if write_back {
                    writeback_industry_tile_parent_persistent_registers(
                        &mut industries[index],
                        ctx,
                    );
                } else {
                    ctx.parent_persistent_registers
                        .clone_from(&industries[index].newgrf_persistent_regs);
                    ctx.parent_random_bits = u32::from(industries[index].newgrf_random);
                }
            }
        },
        |map, coord, mask| {
            if let Some(index) = industry_index_for_tile(map, &snapshot, coord) {
                parent_reseed_masks
                    .entry(index)
                    .and_modify(|current| *current |= mask)
                    .or_insert(mask);
            }
        },
    );
    for (index, mask) in parent_reseed_masks {
        if mask == 0 {
            continue;
        }
        let coord = snapshot[index].pos;
        let random = industry_random_word(
            world_seed,
            tick,
            coord,
            u64::from(trigger.bit())
                | (u64::from(mask) << 8)
                | u64::from(snapshot[index].instance_id),
        );
        let before = industries[index].newgrf_random;
        let reseed_mask = u16::try_from(mask & u32::from(u16::MAX)).unwrap_or(u16::MAX);
        industries[index].newgrf_random = (before & !reseed_mask) | (random & reseed_mask);
    }
    dirty
}

#[allow(clippy::too_many_arguments)]
fn advance_industry_tile_randomisation_from_visits_with_catalog_inner(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    visits: &[(TileCoord, Tile)],
    industries: &[Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    trigger: IndustryRandomTrigger,
    mut sync_ctx: impl FnMut(&Map, TileCoord, &mut crate::newgrf_sprites::Action2EvalCtx, bool),
    mut sync_parent_reseed: impl FnMut(&Map, TileCoord, u32),
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &(coord, snapshot) in visits {
        if snapshot.kind != TileKind::Industry {
            continue;
        }
        let Some(mut live) = map.get(coord) else {
            continue;
        };
        if live.kind != TileKind::Industry {
            continue;
        }
        let Some(spec) = industry_tile_spec_def(tile_spec_catalog, industry_gfx(&live)) else {
            // No Action3 assignment means vanilla IndustryTile behaviour.
            continue;
        };
        if !spec.from_newgrf || !spec.has_newgrf_sprites() {
            continue;
        }

        let before = live;
        let waiting = industry_random_triggers(&live) | trigger.bit();
        set_industry_random_triggers(&mut live, waiting);

        // The scope resolver reads the live MAP1/MAP2 fields. Publish the
        // waiting trigger before evaluating Action2, then write the final
        // random/mask state back once the resolver has selected its branch.
        if map.set_tile(coord, live).is_err() {
            continue;
        }
        let mut ctx = crate::map::industry_action2::action2_eval_ctx_for_industry_tile_with_world(
            map,
            coord,
            industries,
            towns,
            tile_spec_catalog,
            industry_catalog,
            climate,
            Some(spec),
            &[],
        );
        sync_ctx(&*map, coord, &mut ctx, false);

        let Some(runtime) = spec.newgrf_runtime.as_ref() else {
            // A static NewGRF sprite group has no rerandomisation graph. The
            // trigger remains pending, matching OpenTTD's resolver.
            if live != before {
                dirty.push(coord);
            }
            continue;
        };
        let (reseed_self, reseed_parent, used) = runtime.rerandomisation_for_local_id_u16_scoped(
            u16::from(spec.newgrf_local_id),
            &mut ctx,
            waiting,
        );
        sync_ctx(&*map, coord, &mut ctx, true);
        sync_parent_reseed(&*map, coord, reseed_parent);
        let Some(mut updated) = map.get(coord) else {
            continue;
        };
        set_industry_random_triggers(&mut updated, waiting & !used);
        let reseed_mask = u8::try_from(reseed_self & 0xFF).unwrap_or(0);
        if reseed_mask != 0 {
            let random = industry_tile_rng(
                world_seed,
                tick,
                coord,
                u64::from(trigger.bit()) | (u64::from(waiting) << 8),
            );
            updated.m3 = (updated.m3 & !reseed_mask) | (random & reseed_mask);
        }
        // OpenTTD marks the tile dirty for every resolved trigger, even when
        // the selected mask happens to reproduce the previous random value.
        if map.set_tile(coord, updated).is_ok() {
            dirty.push(coord);
        }
    }
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

fn industry_index_for_tile(map: &Map, industries: &[Industry], coord: TileCoord) -> Option<usize> {
    let tile = map.get(coord)?;
    if tile.kind != TileKind::Industry {
        return None;
    }
    let instance_id = industry_instance_id(&tile);
    industries
        .iter()
        .position(|industry| {
            industry.contains_tile(coord)
                && (instance_id == 0 || industry.instance_id == instance_id)
        })
        .or_else(|| {
            industries
                .iter()
                .position(|industry| industry.instance_id == instance_id)
        })
}

/// Tile loop: `IndustryRandomTrigger::TileLoop` en la franja (`tick % 256`).
pub fn advance_industry_tile_randomisation(
    map: &mut Map,
    tick: u64,
    world_seed: u64,
    loop_state: &mut super::tile_loop::TileLoopState,
) -> Vec<TileCoord> {
    let visits =
        super::tile_loop::collect_tile_loop_visits(map, tick, &mut loop_state.cur_tileloop_tile);
    advance_industry_tile_randomisation_from_visits(map, tick, world_seed, &visits)
}

/// Dispara un trigger en todas las teselas de una industria (por `m2` / footprint).
pub fn trigger_industry_randomisation_at(
    map: &mut Map,
    tiles: &[TileCoord],
    trigger: IndustryRandomTrigger,
    world_seed: u64,
    tick: u64,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &coord in tiles {
        let Some(mut tile) = map.get(coord) else {
            continue;
        };
        if !trigger_industry_tile_randomisation(&mut tile, trigger, world_seed, tick, coord) {
            continue;
        }
        if map.set_tile(coord, tile).is_ok() {
            dirty.push(coord);
        }
    }
    dirty
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::map::{TileKind, set_industry_gfx};
    use crate::newgrf_sprites::{
        Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
        TrainSpriteAssign, TrainSpriteGraphics,
    };

    fn newgrf_spec(
        gfx: u16,
        views: Vec<crate::newgrf_sprites::DecodedSprite>,
        runtime: Option<TrainSpriteGraphics>,
    ) -> IndustryTileSpecDef {
        IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(gfx),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 0x1234,
            newgrf_preview: views.first().cloned(),
            newgrf_views: views,
            newgrf_runtime: runtime.map(Box::new),
        }
    }

    fn parent_psto_runtime() -> TrainSpriteGraphics {
        let literal = |value: u8| Action2VarTerm {
            variable: 0x1A,
            param: None,
            adjust: Action2VarAdjust {
                shift: 0,
                and_mask: u32::from(value),
                ..Action2VarAdjust::default()
            },
        };
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: literal(42),
                ops: vec![
                    Action2VarOp {
                        operator: 0x10,
                        rhs: literal(5),
                    },
                    Action2VarOp {
                        operator: 0x0F,
                        rhs: literal(0),
                    },
                ],
                ranges: Vec::new(),
                default: 0,
            },
        );
        let entry = gfx.action2_var.get_mut(&2).unwrap();
        entry.first.adjust.shift |= 0x80;
        entry.ops[0].rhs.adjust.shift |= 0x80;
        entry.ops[1].rhs.adjust.shift |= 0x80;
        gfx
    }

    fn white_sprite() -> crate::newgrf_sprites::DecodedSprite {
        crate::newgrf_sprites::DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![255, 255, 255, 255],
            mask: Vec::new(),
        }
    }

    fn industry_tile(gfx: u16) -> Tile {
        let mut t = Tile {
            kind: TileKind::Industry,
            height: 0,
            mapt: 0x80,
            m5: 0,
            m6: 0,
            m1: 0x80,
            m2: 1,
            m3: 0,
            m7: 0,
            m8: 0,
            m3hi: 0,
            m2_hi: 0,
        };
        set_industry_gfx(&mut t, gfx);
        t
    }

    #[test]
    fn randomisation_writes_live_industry_parent_psa() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = industry_tile(175);
        tile.m3 = 0xA5;
        map.set_tile(coord, tile).unwrap();
        let spec = newgrf_spec(175, vec![white_sprite()], Some(parent_psto_runtime()));
        let visits = vec![(coord, tile)];
        let mut industries =
            vec![Industry::new(coord, crate::industry::IndustryKind::CoalMine).with_instance_id(1)];

        let dirty = advance_industry_tile_randomisation_from_visits_with_catalog_and_world(
            &mut map,
            1,
            42,
            &visits,
            &mut industries,
            &[],
            std::slice::from_ref(&spec),
            &[],
            Climate::Temperate,
        );

        assert_eq!(dirty, vec![coord]);
        assert_eq!(industries[0].newgrf_persistent_regs.get(&5), Some(&42));
    }

    #[test]
    fn industry_tick_and_cargo_triggers_use_live_parent_scope() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let tile = industry_tile(175);
        map.set_tile(coord, tile).unwrap();
        let spec = newgrf_spec(175, vec![white_sprite()], Some(parent_psto_runtime()));
        let mut industries =
            vec![Industry::new(coord, crate::industry::IndustryKind::CoalMine).with_instance_id(1)];
        let catalog = std::slice::from_ref(&spec);

        let dirty = trigger_industry_randomisation_at_with_catalog_and_world(
            &mut map,
            &[coord],
            IndustryRandomTrigger::IndustryTick,
            42,
            7,
            &mut industries,
            &[],
            catalog,
            &[],
            Climate::Temperate,
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(industries[0].newgrf_persistent_regs.get(&5), Some(&42));
        assert_eq!(
            industry_random_triggers(&map.get(coord).unwrap()),
            IndustryRandomTrigger::IndustryTick.bit()
        );

        let dirty = trigger_industry_randomisation_at_with_catalog_and_world(
            &mut map,
            &[coord],
            IndustryRandomTrigger::CargoReceived,
            42,
            8,
            &mut industries,
            &[],
            catalog,
            &[],
            Climate::Temperate,
        );
        assert_eq!(dirty, vec![coord]);
        assert_eq!(industries[0].newgrf_persistent_regs.get(&5), Some(&42));
        assert_eq!(
            industry_random_triggers(&map.get(coord).unwrap()),
            IndustryRandomTrigger::IndustryTick.bit() | IndustryRandomTrigger::CargoReceived.bit()
        );
    }

    #[test]
    fn industry_parent_randomisation_reseeds_once_for_footprint() {
        let origin = TileCoord::new(1, 1);
        let second = TileCoord::new(2, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let tile = industry_tile(175);
        map.set_tile(origin, tile).unwrap();
        map.set_tile(second, tile).unwrap();
        let mut runtime = TrainSpriteGraphics::default();
        runtime.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 1,
        });
        runtime.action2_random.insert(
            1,
            Action2RandomEntry {
                typ: 0x83,
                consist_count: 0,
                triggers: IndustryRandomTrigger::CargoReceived.bit(),
                randbit: 0,
                sets: vec![2, 2],
            },
        );
        let spec = newgrf_spec(175, vec![white_sprite()], Some(runtime));
        let mut industries = vec![
            Industry::with_tiles(
                origin,
                crate::industry::IndustryKind::CoalMine,
                vec![origin, second],
            )
            .with_instance_id(1)
            .with_newgrf_random(0),
        ];
        let dirty = trigger_industry_randomisation_at_with_catalog_and_world(
            &mut map,
            &[origin, second],
            IndustryRandomTrigger::CargoReceived,
            42,
            7,
            &mut industries,
            &[],
            std::slice::from_ref(&spec),
            &[],
            Climate::Temperate,
        );
        assert_eq!(dirty, vec![origin, second]);
        let expected = industry_random_word(
            42,
            7,
            origin,
            u64::from(IndustryRandomTrigger::CargoReceived.bit()) | (1_u64 << 8) | 1,
        );
        assert_eq!(industries[0].newgrf_random & 1, expected & 1);
        assert_eq!(industry_random_triggers(&map.get(origin).unwrap()), 0);
        assert_eq!(industry_random_triggers(&map.get(second).unwrap()), 0);
    }

    #[test]
    fn triggers_occupy_m6_bits_3_to_5_without_clobbering_gfx_bit() {
        let mut tile = industry_tile(0x100); // bit 8 → m6 bit 2
        assert_eq!((tile.m6 >> 2) & 1, 1);
        set_industry_random_triggers(&mut tile, 0b101);
        assert_eq!(industry_random_triggers(&tile), 0b101);
        assert_eq!((tile.m6 >> 2) & 1, 1, "gfx bit 8 debe preservarse");
        assert_eq!(tile.m6 & INDUSTRY_RANDOM_TRIGGERS_MASK, 0b101 << 3);
    }

    #[test]
    fn make_industry_seeds_m3_and_clears_triggers() {
        let mut tile = industry_tile(0);
        init_industry_tile_random(&mut tile, 0xA5);
        assert_eq!(industry_random_bits(&tile), 0xA5);
        assert_eq!(industry_random_triggers(&tile), 0);
    }

    #[test]
    fn tile_loop_trigger_reseeds_m3() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = industry_tile(1);
        init_industry_tile_random(&mut tile, 0x11);
        map.set_kind(c, TileKind::Industry).unwrap();
        map.set_tile(c, tile).unwrap();
        let mut loop_state = crate::map::TileLoopState::default();
        let mut dirty = Vec::new();
        for tick in 0..4096u64 {
            dirty = advance_industry_tile_randomisation(&mut map, tick, 42, &mut loop_state);
            if dirty.contains(&c) {
                break;
            }
        }
        assert!(dirty.contains(&c));
        let after = map.get(c).unwrap();
        assert_ne!(industry_random_bits(&after), 0x11);
        assert_eq!(industry_random_triggers(&after), 0);
    }

    #[test]
    fn tile_loop_skips_tiles_outside_stripe() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = industry_tile(1);
        init_industry_tile_random(&mut tile, 0x11);
        map.set_tile(c, tile).unwrap();
        let mut cur = crate::map::default_cur_tileloop_tile();
        let visits = crate::map::collect_tile_loop_visits(&map, 1, &mut cur);
        if visits.iter().any(|(coord, _)| *coord == c) {
            return;
        }
        let dirty = advance_industry_tile_randomisation_from_visits(&mut map, 1, 42, &visits);
        assert!(!dirty.contains(&c));
        assert_eq!(industry_random_bits(&map.get(c).unwrap()), 0x11);
    }

    #[test]
    fn catalog_path_leaves_vanilla_industry_random_state_untouched() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = industry_tile(1);
        init_industry_tile_random(&mut tile, 0x11);
        map.set_tile(c, tile).unwrap();
        let visits = vec![(c, tile)];

        let dirty = advance_industry_tile_randomisation_from_visits_with_catalog(
            &mut map,
            1,
            42,
            &visits,
            &[],
            &[],
            &[],
            &[],
            Climate::Temperate,
        );

        assert!(dirty.is_empty());
        assert_eq!(map.get(c).unwrap(), tile);
    }

    #[test]
    fn static_newgrf_group_keeps_tile_loop_trigger_pending() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = industry_tile(175);
        init_industry_tile_random(&mut tile, 0x11);
        map.set_tile(c, tile).unwrap();
        let visits = vec![(c, tile)];
        let catalog = vec![newgrf_spec(175, vec![white_sprite()], None)];

        let dirty = advance_industry_tile_randomisation_from_visits_with_catalog(
            &mut map,
            1,
            42,
            &visits,
            &[],
            &[],
            &catalog,
            &[],
            Climate::Temperate,
        );

        let after = map.get(c).unwrap();
        assert_eq!(dirty, vec![c]);
        assert_eq!(industry_random_bits(&after), 0x11);
        assert_eq!(
            industry_random_triggers(&after),
            IndustryRandomTrigger::TileLoop.bit()
        );
    }

    #[test]
    fn random_newgrf_group_reseeds_only_declared_bits_and_consumes_trigger() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = industry_tile(175);
        init_industry_tile_random(&mut tile, 0b1010_1100);
        map.set_tile(c, tile).unwrap();
        let visits = vec![(c, tile)];
        let mut runtime = TrainSpriteGraphics {
            assigns: vec![TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            ..Default::default()
        };
        runtime.action2_random.insert(
            0,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: IndustryRandomTrigger::TileLoop.bit(),
                randbit: 0,
                sets: vec![1, 2],
            },
        );
        let catalog = vec![newgrf_spec(175, Vec::new(), Some(runtime))];

        let dirty = advance_industry_tile_randomisation_from_visits_with_catalog(
            &mut map,
            1,
            42,
            &visits,
            &[],
            &[],
            &catalog,
            &[],
            Climate::Temperate,
        );

        let after = map.get(c).unwrap();
        assert_eq!(dirty, vec![c]);
        assert_eq!(industry_random_triggers(&after), 0);
        assert_eq!(industry_random_bits(&after) & !1, 0b1010_1100 & !1);
    }
}
