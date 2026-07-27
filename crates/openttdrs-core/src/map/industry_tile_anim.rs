//! Animación de industrias vanilla — `AnimateTile_Industry` / `TileLoop_Industry`.
//!
//! Ritmos separados (como OpenTTD):
//! - **TileLoop** (franja `tick % 256`): arranque pozo, idle↔torre de mina.
//! - **AnimateTile** (cada tick, footprints conocidos): frames `m3hi`, gfx animados,
//!   fuente plástico, `draw_proc`.
//!
//! Torres: gfx idle ↔ animado (0↔1, 47↔48, 79↔88) + frames `m3hi`.
//! Pozos: gfx 29 → 30–32 (+ vuelta a 29). Fuente Toyland: 148–155.

use super::tile_loop::TileLoopState;
use super::{Map, Tile, TileCoord, TileKind};

/// `GFX_COAL_MINE_TOWER_NOT_ANIMATED`
pub const GFX_COAL_MINE_TOWER_NOT_ANIMATED: u16 = 0;
/// `GFX_COAL_MINE_TOWER_ANIMATED`
pub const GFX_COAL_MINE_TOWER_ANIMATED: u16 = 1;
/// `GFX_OILWELL_NOT_ANIMATED`
pub const GFX_OILWELL_NOT_ANIMATED: u16 = 29;
/// `GFX_OILWELL_ANIMATED_1`
pub const GFX_OILWELL_ANIMATED_1: u16 = 30;
/// `GFX_OILWELL_ANIMATED_2`
pub const GFX_OILWELL_ANIMATED_2: u16 = 31;
/// `GFX_OILWELL_ANIMATED_3`
pub const GFX_OILWELL_ANIMATED_3: u16 = 32;
/// `GFX_COPPER_MINE_TOWER_NOT_ANIMATED`
pub const GFX_COPPER_MINE_TOWER_NOT_ANIMATED: u16 = 47;
/// `GFX_COPPER_MINE_TOWER_ANIMATED`
pub const GFX_COPPER_MINE_TOWER_ANIMATED: u16 = 48;
/// `GFX_GOLD_MINE_TOWER_NOT_ANIMATED`
pub const GFX_GOLD_MINE_TOWER_NOT_ANIMATED: u16 = 79;
/// `GFX_GOLD_MINE_TOWER_ANIMATED`
pub const GFX_GOLD_MINE_TOWER_ANIMATED: u16 = 88;
/// `GFX_PLASTIC_FOUNTAIN_ANIMATED_1`
pub const GFX_PLASTIC_FOUNTAIN_ANIMATED_1: u16 = 148;
/// `GFX_PLASTIC_FOUNTAIN_ANIMATED_8`
pub const GFX_PLASTIC_FOUNTAIN_ANIMATED_8: u16 = 155;
/// `GFX_BUBBLE_GENERATOR`: cada visita de TileLoop crea `EV_BUBBLE`.
pub const GFX_BUBBLE_GENERATOR: u16 = 161;

const TOWER_ANIM_GFX: [u16; 3] = [
    GFX_COAL_MINE_TOWER_ANIMATED,
    GFX_COPPER_MINE_TOWER_ANIMATED,
    GFX_GOLD_MINE_TOWER_ANIMATED,
];

const MINE_TOWER_GFX_PAIRS: [(u16, u16); 3] = [
    (
        GFX_COAL_MINE_TOWER_NOT_ANIMATED,
        GFX_COAL_MINE_TOWER_ANIMATED,
    ),
    (
        GFX_COPPER_MINE_TOWER_NOT_ANIMATED,
        GFX_COPPER_MINE_TOWER_ANIMATED,
    ),
    (
        GFX_GOLD_MINE_TOWER_NOT_ANIMATED,
        GFX_GOLD_MINE_TOWER_ANIMATED,
    ),
];

/// Escala tick de sim (≈37 Hz) al contador de animación por tick de OpenTTD.
const OTTD_ANIM_SCALE: u64 = 1;
const MINE_TOWER_QUIET_MASK: u64 = 0x400;

/// gfx de industria de 9 bits (`GetCleanIndustryGfx`).
#[must_use]
pub fn industry_gfx(tile: &Tile) -> u16 {
    u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8)
}

/// Escribe gfx de 9 bits en `m5` / bit 8 en `m6`.
pub fn set_industry_gfx(tile: &mut Tile, gfx: u16) {
    tile.m5 = u8::try_from(gfx & 0xFF).unwrap_or(0);
    tile.m6 = (tile.m6 & !0x04) | (((gfx >> 8) & 1) as u8) << 2;
}

/// Industria terminada (`IsIndustryCompleted`).
#[must_use]
pub fn is_industry_completed(tile: &Tile) -> bool {
    tile.m1 & 0x80 != 0
}

/// `IndustryTileSpec.anim_state` (subconjunto vanilla en tabla de 131 gfx).
#[must_use]
pub fn industry_tile_anim_state(gfx: u16) -> bool {
    matches!(
        gfx,
        GFX_COAL_MINE_TOWER_ANIMATED
            | GFX_OILWELL_ANIMATED_1
            | GFX_OILWELL_ANIMATED_2
            | GFX_OILWELL_ANIMATED_3
            | GFX_COPPER_MINE_TOWER_ANIMATED
            | GFX_GOLD_MINE_TOWER_ANIMATED
    )
}

/// Frame de animación (`GetAnimationFrame & 3`).
#[must_use]
pub fn industry_animation_frame(m3hi: u8) -> u8 {
    m3hi & 3
}

fn tile_anim_phase(x: i32, y: i32) -> u64 {
    (u64::try_from(x.max(0)).unwrap_or(0))
        .wrapping_mul(17)
        .wrapping_add(u64::try_from(y.max(0)).unwrap_or(0).wrapping_mul(31))
}

fn ottd_anim_counter(tick: u64) -> u64 {
    tick.wrapping_mul(OTTD_ANIM_SCALE)
}

fn mine_tower_quiet(tick: u64) -> bool {
    ottd_anim_counter(tick) & MINE_TOWER_QUIET_MASK == 0
}

fn mine_tower_active(tick: u64) -> bool {
    (ottd_anim_counter(tick) & 0x7FF) >= MINE_TOWER_QUIET_MASK
}

/// ~50% por tesela en ventana quiet (`Chance16(1, 2)` en `TileLoop_Industry`).
fn mine_tower_start_chance(tick: u64, x: i32, y: i32) -> bool {
    let window = ottd_anim_counter(tick) / MINE_TOWER_QUIET_MASK;
    ((window.wrapping_add(tile_anim_phase(x, y))) & 1) == 0
}

/// `Chance16(1, n)` estable por tesela y tick.
fn ottd_chance_1_in_n(tick: u64, x: i32, y: i32, n: u64) -> bool {
    let h = tile_anim_phase(x, y)
        .wrapping_mul(31)
        .wrapping_add(tick.wrapping_mul(17));
    h.is_multiple_of(n)
}

/// `TileLoop_Industry`: pozo idle → animado (`Chance16(1, 6)`).
fn try_start_oil_well_animation(tile: &mut Tile, gfx: u16, tick: u64, x: i32, y: i32) -> bool {
    if gfx != GFX_OILWELL_NOT_ANIMATED {
        return false;
    }
    if !ottd_chance_1_in_n(tick, x, y, 6) {
        return false;
    }
    set_industry_gfx(tile, GFX_OILWELL_ANIMATED_1);
    tile.m3hi = 0;
    true
}

/// `TileLoop_Industry`: idle ↔ torre animada (no frames).
fn tile_loop_mine_tower(tile: &mut Tile, gfx: u16, tick: u64, x: i32, y: i32) -> bool {
    if !mine_tower_quiet(tick) {
        return false;
    }
    for &(idle, active) in &MINE_TOWER_GFX_PAIRS {
        if gfx == idle && mine_tower_start_chance(tick, x, y) {
            set_industry_gfx(tile, active);
            tile.m3hi = 0x80;
            return true;
        }
        if gfx == active {
            set_industry_gfx(tile, idle);
            return true;
        }
    }
    false
}

/// `AnimateMineTower`: frames `m3hi` mientras la torre está en gfx animado.
fn animate_mine_tower(tile: &mut Tile, gfx: u16, tick: u64, x: i32, y: i32) -> bool {
    if mine_tower_active(tick) && TOWER_ANIM_GFX.contains(&gfx) {
        return advance_tower(tile, tick, x, y);
    }
    false
}

fn advance_tower(tile: &mut Tile, tick: u64, x: i32, y: i32) -> bool {
    if !tick.is_multiple_of(2) {
        return false;
    }
    let phase = tile_anim_phase(x, y);
    if !tick.wrapping_add(phase).is_multiple_of(8) {
        return false;
    }
    let frame = industry_animation_frame(tile.m3hi);
    tile.m3hi = (tile.m3hi & !3) | ((frame + 1) & 3);
    true
}

/// `AnimateOilWell` — cada 7 ticks OTTD; frames 0–3 y ciclo gfx 30→31→32.
fn advance_oil_well_animated(tile: &mut Tile, gfx: u16, tick: u64, x: i32, y: i32) -> bool {
    if !ottd_anim_counter(tick).is_multiple_of(7) {
        return false;
    }
    let revert_idle = ottd_chance_1_in_n(tick, x, y, 7);
    let mut frame = u16::from(industry_animation_frame(tile.m3hi)) + 1;
    if frame >= 4 {
        frame = 0;
        let mut next_gfx = gfx.saturating_add(1);
        if next_gfx > GFX_OILWELL_ANIMATED_3 {
            if revert_idle {
                set_industry_gfx(tile, GFX_OILWELL_NOT_ANIMATED);
                tile.m3hi = 0;
                return true;
            }
            next_gfx = GFX_OILWELL_ANIMATED_1;
        }
        set_industry_gfx(tile, next_gfx);
    }
    tile.m3hi = (tile.m3hi & !3) | (u8::try_from(frame).unwrap_or(0) & 3);
    true
}

/// `AnimatePlasticFountain` — ciclo gfx 148–155 cada 4 ticks OTTD.
fn advance_plastic_fountain(tile: &mut Tile, gfx: u16, tick: u64) -> bool {
    if !(GFX_PLASTIC_FOUNTAIN_ANIMATED_1..=GFX_PLASTIC_FOUNTAIN_ANIMATED_8).contains(&gfx) {
        return false;
    }
    if !ottd_anim_counter(tick).is_multiple_of(4) {
        return false;
    }
    let next = if gfx < GFX_PLASTIC_FOUNTAIN_ANIMATED_8 {
        gfx + 1
    } else {
        GFX_PLASTIC_FOUNTAIN_ANIMATED_1
    };
    set_industry_gfx(tile, next);
    true
}

fn industry_draw_proc_gfx(gfx: u16, m1: u8) -> u8 {
    // Bits 5–6 de m1 son WaterClass, no la etapa de obra.
    let stage = usize::from(super::industry_construction_stage(m1)).min(3);
    match gfx {
        10 if stage == 3 => 5,
        143 if stage == 3 => 4,
        162 if stage >= 1 => 3,
        165 => 2,
        174 => 1,
        _ => 0,
    }
}

fn advance_draw_proc(tile: &mut Tile, proc: u8, tick: u64) -> bool {
    match proc {
        1 if is_industry_completed(tile) && tick.is_multiple_of(2) => {
            let m = u16::from(tile.m3hi).saturating_add(1);
            tile.m3hi = u8::try_from(if m >= 96 { 0 } else { m }).unwrap_or(0);
            true
        }
        2 if tick.is_multiple_of(4) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 70;
            true
        }
        3 if tick.is_multiple_of(2) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 40;
            true
        }
        4 if tick.is_multiple_of(2) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 50;
            true
        }
        5 if is_industry_completed(tile) && tick.is_multiple_of(4) => {
            if tile.m3hi == 6 {
                tile.m3hi = 0;
            } else {
                tile.m3hi = tile.m3hi.saturating_add(1);
            }
            true
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndustryAnimUpdate {
    None,
    /// Solo `m3hi` / frame — el cliente `IndustryBuildingAnim` lee el mapa cada frame.
    Frame,
    /// Cambió `gfx` u otro dato que exige respawn de sprites.
    Visual,
}

fn apply_tile_loop_industry(tile: &mut Tile, tick: u64, x: i32, y: i32) -> IndustryAnimUpdate {
    if tile.kind != TileKind::Industry || !is_industry_completed(tile) {
        return IndustryAnimUpdate::None;
    }
    let gfx = industry_gfx(tile);
    if try_start_oil_well_animation(tile, gfx, tick, x, y) {
        return IndustryAnimUpdate::Visual;
    }
    if tile_loop_mine_tower(tile, gfx, tick, x, y) {
        return IndustryAnimUpdate::Visual;
    }
    IndustryAnimUpdate::None
}

fn apply_animate_industry(tile: &mut Tile, tick: u64, x: i32, y: i32) -> IndustryAnimUpdate {
    if tile.kind != TileKind::Industry {
        return IndustryAnimUpdate::None;
    }
    let gfx = industry_gfx(tile);
    let proc = industry_draw_proc_gfx(gfx, tile.m1);
    if proc > 0 && advance_draw_proc(tile, proc, tick) {
        return IndustryAnimUpdate::Frame;
    }
    if !is_industry_completed(tile) {
        return IndustryAnimUpdate::None;
    }
    if animate_mine_tower(tile, gfx, tick, x, y) {
        return IndustryAnimUpdate::Frame;
    }
    if (GFX_OILWELL_ANIMATED_1..=GFX_OILWELL_ANIMATED_3).contains(&gfx) {
        let before = industry_gfx(tile);
        if advance_oil_well_animated(tile, gfx, tick, x, y) {
            return if industry_gfx(tile) == before {
                IndustryAnimUpdate::Frame
            } else {
                IndustryAnimUpdate::Visual
            };
        }
    }
    if advance_plastic_fountain(tile, gfx, tick) {
        return IndustryAnimUpdate::Visual;
    }
    IndustryAnimUpdate::None
}

fn commit_industry_updates(
    map: &mut Map,
    coords: &[TileCoord],
    tick: u64,
    mut apply: impl FnMut(&mut Tile, u64, i32, i32) -> IndustryAnimUpdate,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    for &coord in coords {
        let Some(mut tile) = map.get(coord) else {
            continue;
        };
        let update = apply(&mut tile, tick, coord.x, coord.y);
        if update == IndustryAnimUpdate::None {
            continue;
        }
        if map.set_tile(coord, tile).is_err() {
            continue;
        }
        if update == IndustryAnimUpdate::Visual {
            dirty.push(coord);
        }
    }
    dirty
}

/// Eventos `TileLoop_Industry` sobre teselas ya visitadas por `RunTileLoop`.
pub fn advance_industry_tile_loop_events_from_visits(
    map: &mut Map,
    tick: u64,
    visits: &[(TileCoord, Tile)],
) -> Vec<TileCoord> {
    let candidates: Vec<TileCoord> = visits
        .iter()
        .filter(|(_, tile)| tile.kind == TileKind::Industry)
        .map(|(coord, _)| *coord)
        .collect();
    commit_industry_updates(map, &candidates, tick, apply_tile_loop_industry)
}

/// Generadores de burbujas terminados visitados por `RunTileLoop` este tick.
///
/// La creación del EffectVehicle ocurre una vez por visita, igual que
/// `TileLoopIndustry_BubbleGenerator`; no requiere barrer el mapa.
#[must_use]
pub fn bubble_generator_spawns_from_visits(visits: &[(TileCoord, Tile)]) -> Vec<TileCoord> {
    visits
        .iter()
        .filter(|(_, tile)| {
            tile.kind == TileKind::Industry
                && is_industry_completed(tile)
                && industry_gfx(tile) == GFX_BUBBLE_GENERATOR
        })
        .map(|(coord, _)| *coord)
        .collect()
}

/// Eventos `TileLoop_Industry` (franja cada 256 ticks).
pub fn advance_industry_tile_loop_events(
    map: &mut Map,
    tick: u64,
    loop_state: &mut super::tile_loop::TileLoopState,
) -> Vec<TileCoord> {
    let visits =
        super::tile_loop::collect_tile_loop_visits(map, tick, &mut loop_state.cur_tileloop_tile);
    advance_industry_tile_loop_events_from_visits(map, tick, &visits)
}

/// `AnimateTile_Industry` sobre footprints conocidos (cada tick).
pub fn advance_industry_animated_tiles(
    map: &mut Map,
    tick: u64,
    coords: &[TileCoord],
) -> Vec<TileCoord> {
    commit_industry_updates(map, coords, tick, apply_animate_industry)
}

/// Compat tests / herramientas: TileLoop (franja) + Animate sobre industrias del mapa.
pub fn advance_industry_tile_animations(
    map: &mut Map,
    tick: u64,
    loop_state: &mut TileLoopState,
) -> Vec<TileCoord> {
    let mut dirty = advance_industry_tile_loop_events(map, tick, loop_state);
    let (w, h) = map.dimensions();
    let mut coords = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if map.get_kind(c) == Some(TileKind::Industry) {
                coords.push(c);
            }
        }
    }
    dirty.extend(advance_industry_animated_tiles(map, tick, &coords));
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;

    fn industry_tile(gfx: u16, m1: u8, m3hi: u8) -> Tile {
        let mut tile = Tile {
            height: 0,
            kind: TileKind::Industry,
            mapt: 0,
            m5: 0,
            m1,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi,
        };
        set_industry_gfx(&mut tile, gfx);
        tile
    }

    #[test]
    fn coal_mine_headframe_promotes_to_animated_gfx() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_COAL_MINE_TOWER_NOT_ANIMATED, 0x80, 0),
        )
        .unwrap();
        let mut promoted = false;
        // TileLoop: tesela 0 solo en ticks múltiplo de 256.
        let mut loop_state = TileLoopState::default();
        for visit in 0..=32u64 {
            let tick = visit * 256;
            advance_industry_tile_loop_events(&mut map, tick, &mut loop_state);
            if industry_gfx(&map.get(TileCoord::new(0, 0)).unwrap()) == GFX_COAL_MINE_TOWER_ANIMATED
            {
                promoted = true;
                break;
            }
        }
        assert!(promoted, "gfx 0 debe pasar a gfx 1 en tile loop");
    }

    #[test]
    fn tower_cycles_animation_frame() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_COAL_MINE_TOWER_ANIMATED, 0x80, 0),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        let mut saw_change = false;
        let mut prev = map.get(TileCoord::new(0, 0)).unwrap().m3hi & 3;
        // Ventana activa (`counter & 0x7FF >= 0x400`) sin demote a gfx 0.
        // Con sim a ~37 Hz, `OTTD_ANIM_SCALE = 1` → counter ≈ tick (activo desde tick 1024).
        for tick in 1024..=1100 {
            advance_industry_tile_animations(&mut map, tick, &mut loop_state);
            let tile = map.get(TileCoord::new(0, 0)).unwrap();
            assert_eq!(industry_gfx(&tile), GFX_COAL_MINE_TOWER_ANIMATED);
            let frame = tile.m3hi & 3;
            if frame != prev {
                saw_change = true;
            }
            prev = frame;
        }
        assert!(saw_change);
    }

    #[test]
    fn oil_well_promotes_from_idle_gfx_29() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_OILWELL_NOT_ANIMATED, 0x80, 0),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        let mut promoted = false;
        for visit in 0..=64u64 {
            advance_industry_tile_loop_events(&mut map, visit * 256, &mut loop_state);
            if industry_gfx(&map.get(TileCoord::new(0, 0)).unwrap()) >= GFX_OILWELL_ANIMATED_1 {
                promoted = true;
                break;
            }
        }
        assert!(promoted, "gfx 29 debe pasar a gfx 30 en tile loop");
    }

    #[test]
    fn oil_well_can_revert_to_idle_gfx_29() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_OILWELL_ANIMATED_3, 0x80, 3),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        let mut reverted = false;
        for tick in 0..=4096 {
            advance_industry_tile_animations(&mut map, tick, &mut loop_state);
            if industry_gfx(&map.get(TileCoord::new(0, 0)).unwrap()) == GFX_OILWELL_NOT_ANIMATED {
                reverted = true;
                break;
            }
        }
        assert!(reverted, "ciclo completo puede volver a gfx 29");
    }

    #[test]
    fn plastic_fountain_cycles_gfx() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_PLASTIC_FOUNTAIN_ANIMATED_1, 0x80, 0),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        let mut changed = false;
        for tick in 0..=64 {
            advance_industry_tile_animations(&mut map, tick, &mut loop_state);
            if industry_gfx(&map.get(TileCoord::new(0, 0)).unwrap())
                > GFX_PLASTIC_FOUNTAIN_ANIMATED_1
            {
                changed = true;
                break;
            }
        }
        assert!(changed, "fuente plástico debe ciclar gfx 148→149…");
    }

    #[test]
    fn oil_well_advances_frame_or_gfx() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_OILWELL_ANIMATED_1, 0x80, 0),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        for tick in 1..=24 {
            advance_industry_tile_animations(&mut map, tick, &mut loop_state);
        }
        let tile = map.get(TileCoord::new(0, 0)).unwrap();
        assert!(
            industry_animation_frame(tile.m3hi) > 0
                || industry_gfx(&tile) != GFX_OILWELL_ANIMATED_1
        );
    }

    #[test]
    fn construction_tiles_do_not_animate_tower() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_COAL_MINE_TOWER_ANIMATED, 0x01, 0),
        )
        .unwrap();
        let mut loop_state = TileLoopState::default();
        advance_industry_tile_animations(&mut map, 8, &mut loop_state);
        assert_eq!(map.get(TileCoord::new(0, 0)).unwrap().m3hi, 0);
    }

    #[test]
    fn power_plant_sparks_cycle() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(TileCoord::new(0, 0), industry_tile(10, 0x80, 0))
            .unwrap();
        let mut loop_state = TileLoopState::default();
        advance_industry_tile_animations(&mut map, 4, &mut loop_state);
        assert_eq!(map.get(TileCoord::new(0, 0)).unwrap().m3hi, 1);
    }

    #[test]
    fn bubble_effect_spawns_only_from_completed_generator_visits() {
        let generator = industry_tile(GFX_BUBBLE_GENERATOR, 0x80, 0);
        let incomplete = industry_tile(GFX_BUBBLE_GENERATOR, 0x01, 0);
        let other = industry_tile(160, 0x80, 0);
        let visits = [
            (TileCoord::new(2, 3), generator),
            (TileCoord::new(3, 3), incomplete),
            (TileCoord::new(4, 3), other),
        ];
        assert_eq!(
            bubble_generator_spawns_from_visits(&visits),
            vec![TileCoord::new(2, 3)]
        );
    }
}
