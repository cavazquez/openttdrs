//! Terraform de terreno (elevar / bajar / nivelar), al estilo `terraform_cmd.cpp`.

use std::collections::{HashMap, HashSet};

use crate::economy::terraform_cost_per_corner;
use crate::game_state::GameState;
use crate::map::{Map, TileCoord, TileKind};
use crate::tile_slope_and_z;

use super::error::CommandError;
use super::types::LevelMode;
use super::util::in_bounds;

const MAP_HEIGHT_LIMIT: u8 = 15;
const MAX_CORNER_STEPS: u8 = 32;
const MAPT_WATER: u8 = 0x60;

const DIAG_NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CornerKey(i32, i32);

struct TerraformModel<'a> {
    map: &'a Map,
    heights: HashMap<CornerKey, u8>,
    dirty_tiles: HashSet<TileCoord>,
    cost: i64,
    allow_water_source: bool,
    cost_per_corner: i64,
    /// Solo exige `Grass`/`Forest` en esta tesela (autoslope junto a vías vecinas).
    primary_tile: Option<TileCoord>,
}

impl<'a> TerraformModel<'a> {
    fn new(map: &'a Map, allow_water_source: bool, cost_per_corner: i64) -> Self {
        Self {
            map,
            heights: HashMap::new(),
            dirty_tiles: HashSet::new(),
            cost: 0,
            allow_water_source,
            cost_per_corner,
            primary_tile: None,
        }
    }

    fn with_primary_tile(mut self, c: TileCoord) -> Self {
        self.primary_tile = Some(c);
        self
    }

    fn corner_height(&self, cx: i32, cy: i32) -> u8 {
        if let Some(&h) = self.heights.get(&CornerKey(cx, cy)) {
            return h;
        }
        self.map.get(TileCoord::new(cx, cy)).map_or(0, |t| t.height)
    }

    fn mark_dirty_around_north_corner(&mut self, tx: i32, ty: i32) {
        if ty >= 1 {
            self.dirty_tiles.insert(TileCoord::new(tx, ty - 1));
        }
        if ty >= 1 && tx >= 1 {
            self.dirty_tiles.insert(TileCoord::new(tx - 1, ty - 1));
        }
        if tx >= 1 {
            self.dirty_tiles.insert(TileCoord::new(tx - 1, ty));
        }
        self.dirty_tiles.insert(TileCoord::new(tx, ty));
    }

    fn set_north_corner(&mut self, tx: i32, ty: i32, height: u8) {
        self.heights.insert(CornerKey(tx, ty), height);
        self.mark_dirty_around_north_corner(tx, ty);
    }

    fn terraform_north_corner(&mut self, tx: i32, ty: i32, target: u8) -> Result<(), CommandError> {
        in_bounds(self.map, TileCoord::new(tx, ty))?;
        let current = self.corner_height(tx, ty);
        if target == current {
            return Ok(());
        }
        if target > MAP_HEIGHT_LIMIT {
            return Err(CommandError::TerrainTooHigh);
        }

        self.set_north_corner(tx, ty, target);
        self.cost += self.cost_per_corner;

        for (dx, dy) in DIAG_NEIGHBORS {
            let nx = tx + dx;
            let ny = ty + dy;
            if self.map.get(TileCoord::new(nx, ny)).is_none() {
                continue;
            }
            let neighbor_h = self.corner_height(nx, ny);
            let diff = i16::from(target) - i16::from(neighbor_h);
            if diff.unsigned_abs() > 1 {
                let adjust = if diff < 0 { 1 } else { -1 };
                let next = i16::from(neighbor_h) + diff + adjust;
                if next < 0 {
                    return Err(CommandError::TerrainTooLow);
                }
                if next > i16::from(MAP_HEIGHT_LIMIT) {
                    return Err(CommandError::TerrainTooHigh);
                }
                let next_u8 = u8::try_from(next).map_err(|_| CommandError::TerrainTooHigh)?;
                self.terraform_north_corner(nx, ny, next_u8)?;
            }
        }
        Ok(())
    }

    fn tile_corners(&self, tx: i32, ty: i32) -> (u8, u8, u8, u8) {
        (
            self.corner_height(tx, ty),
            self.corner_height(tx + 1, ty),
            self.corner_height(tx, ty + 1),
            self.corner_height(tx + 1, ty + 1),
        )
    }

    fn validate_tile_slopes(&self, c: TileCoord) -> Result<(), CommandError> {
        let tx = c.x;
        let ty = c.y;
        let (hn, hw, he, hs) = self.tile_corners(tx, ty);
        let min_h = hn.min(hw).min(he).min(hs);
        let max_h = hn.max(hw).max(he).max(hs);
        if max_h.saturating_sub(min_h) > 2 {
            return Err(CommandError::InvalidTerrainSlope);
        }
        for (dx, dy) in DIAG_NEIGHBORS {
            let nx = tx + dx;
            let ny = ty + dy;
            if self.map.get(TileCoord::new(nx, ny)).is_some()
                && self
                    .corner_height(tx, ty)
                    .abs_diff(self.corner_height(nx, ny))
                    > 1
            {
                return Err(CommandError::InvalidTerrainSlope);
            }
        }
        Ok(())
    }

    fn validate_terraformable(&self) -> Result<(), CommandError> {
        for c in &self.dirty_tiles {
            if self.map.get(*c).is_none() {
                continue;
            }
            if self.primary_tile.is_none() || self.primary_tile == Some(*c) {
                let kind = self.map.get_kind(*c).unwrap_or(TileKind::Void);
                if !is_source_terraformable(kind, self.allow_water_source) {
                    return Err(CommandError::TileNotTerraformable);
                }
            }
            self.validate_tile_slopes(*c)?;
        }
        Ok(())
    }

    fn level_tile_north_corner(&mut self, c: TileCoord, target: u8) -> Result<(), CommandError> {
        let mut steps = 0_u8;
        while self.corner_height(c.x, c.y) != target {
            steps += 1;
            if steps > MAX_CORNER_STEPS {
                return Err(CommandError::InvalidTerrainSlope);
            }
            let cur = self.corner_height(c.x, c.y);
            if cur < target {
                self.terraform_north_corner(c.x, c.y, cur + 1)?;
            } else {
                self.terraform_north_corner(c.x, c.y, cur - 1)?;
            }
        }
        Ok(())
    }
}

const fn is_source_terraformable(kind: TileKind, allow_water: bool) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
        || (allow_water && matches!(kind, TileKind::Water))
}

struct TerraformResult {
    heights: Vec<(i32, i32, u8)>,
    cost: i64,
    dirty_tiles: HashSet<TileCoord>,
}

impl TerraformResult {
    fn empty() -> Self {
        Self {
            heights: Vec::new(),
            cost: 0,
            dirty_tiles: HashSet::new(),
        }
    }

    fn from_model(model: TerraformModel<'_>) -> Self {
        Self {
            heights: model
                .heights
                .into_iter()
                .map(|(CornerKey(tx, ty), height)| (tx, ty, height))
                .collect(),
            cost: model.cost,
            dirty_tiles: model.dirty_tiles,
        }
    }
}

fn tile_rect(from: TileCoord, to: TileCoord) -> (i32, i32, i32, i32) {
    (
        from.x.min(to.x),
        from.y.min(to.y),
        from.x.max(to.x),
        from.y.max(to.y),
    )
}

fn level_target_height(map: &Map, from: TileCoord, mode: LevelMode) -> Result<u8, CommandError> {
    in_bounds(map, from)?;
    let ref_h = map
        .get(from)
        .map(|t| t.height)
        .ok_or(CommandError::OutOfBounds)?;
    match mode {
        LevelMode::Level => Ok(ref_h),
        LevelMode::Raise => ref_h.checked_add(1).ok_or(CommandError::TerrainTooHigh),
        LevelMode::Lower => ref_h.checked_sub(1).ok_or(CommandError::TerrainTooLow),
    }
}

fn simulate_corner_delta(
    map: &Map,
    c: TileCoord,
    delta: i8,
    allow_water: bool,
    cost_per_corner: i64,
) -> Result<TerraformResult, CommandError> {
    in_bounds(map, c)?;
    let mut model = TerraformModel::new(map, allow_water, cost_per_corner);
    let current = model.corner_height(c.x, c.y);
    let target = i16::from(current) + i16::from(delta);
    if target < 0 {
        return Err(CommandError::TerrainTooLow);
    }
    if target > i16::from(MAP_HEIGHT_LIMIT) {
        return Err(CommandError::TerrainTooHigh);
    }
    let target_u8 = u8::try_from(target).map_err(|_| CommandError::TerrainTooHigh)?;
    if target_u8 == current {
        return Err(if delta > 0 {
            CommandError::TerrainTooHigh
        } else {
            CommandError::TerrainTooLow
        });
    }
    model.terraform_north_corner(c.x, c.y, target_u8)?;
    model.validate_terraformable()?;
    Ok(TerraformResult::from_model(model))
}

fn simulate_level_land(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    mode: LevelMode,
    cost_per_corner: i64,
) -> Result<TerraformResult, CommandError> {
    in_bounds(map, from)?;
    in_bounds(map, to)?;
    let target = level_target_height(map, from, mode)?;
    let allow_water = !matches!(mode, LevelMode::Lower);
    let mut model = TerraformModel::new(map, allow_water, cost_per_corner);
    let (min_x, min_y, max_x, max_y) = tile_rect(from, to);
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let c = TileCoord::new(x, y);
            if map.get(c).is_none() {
                continue;
            }
            model.level_tile_north_corner(c, target)?;
        }
    }
    if model.heights.is_empty() {
        return Err(CommandError::InvalidTerrainSlope);
    }
    model.validate_terraformable()?;
    Ok(TerraformResult::from_model(model))
}

fn sync_tile_kind_after_heights(map: &mut Map, c: TileCoord) {
    let Some((tileh, z)) = tile_slope_and_z(map, c) else {
        return;
    };
    let kind = map.get_kind(c).unwrap_or(TileKind::Void);
    if matches!(kind, TileKind::Grass | TileKind::Forest) && z == 0 {
        let _ = map.set_kind(c, TileKind::Water);
        let _ = map.set_mapt_m5(c, MAPT_WATER, 0);
    } else if kind == TileKind::Water && (z > 0 || tileh != 0) {
        let _ = map.set_kind(c, TileKind::Grass);
        let _ = map.set_mapt_m5(c, 0, 0);
    }
}

fn apply_terraform_result(
    state: &mut GameState,
    result: TerraformResult,
) -> Result<(), CommandError> {
    if state.economy.money < result.cost {
        return Err(CommandError::InsufficientFunds);
    }
    for (tx, ty, height) in result.heights {
        state
            .map
            .set_height(TileCoord::new(tx, ty), height)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    for c in &result.dirty_tiles {
        sync_tile_kind_after_heights(&mut state.map, *c);
    }
    state.economy.money -= result.cost;
    Ok(())
}

fn check_flat_water_raise(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    if map.get_kind(c) == Some(TileKind::Water) {
        let (tileh, z) = tile_slope_and_z(map, c).unwrap_or((1, 1));
        if tileh != 0 || z != 0 {
            return Err(CommandError::TileNotTerraformable);
        }
    }
    Ok(())
}

fn simulate_raise_land(
    map: &Map,
    c: TileCoord,
    cost_per_corner: i64,
) -> Result<TerraformResult, CommandError> {
    check_flat_water_raise(map, c)?;
    simulate_corner_delta(map, c, 1, true, cost_per_corner)
}

fn simulate_lower_land(
    map: &Map,
    c: TileCoord,
    cost_per_corner: i64,
) -> Result<TerraformResult, CommandError> {
    simulate_corner_delta(map, c, -1, false, cost_per_corner)
}

/// Nivela la tesela a `GetTileZ` (autoslope al construir vía/carretera).
fn simulate_autoslope_flat(
    map: &Map,
    c: TileCoord,
    cost_per_corner: i64,
) -> Result<TerraformResult, CommandError> {
    in_bounds(map, c)?;
    let (tileh, min_z) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh == 0 {
        return Ok(TerraformResult::empty());
    }
    match map.get_kind(c).unwrap_or(TileKind::Void) {
        TileKind::Grass | TileKind::Forest => {}
        _ => return Err(CommandError::TileNotTerraformable),
    }
    let mut model = TerraformModel::new(map, false, cost_per_corner).with_primary_tile(c);
    let tx = c.x;
    let ty = c.y;
    for (cx, cy) in [(tx, ty), (tx + 1, ty), (tx, ty + 1), (tx + 1, ty + 1)] {
        if model.corner_height(cx, cy) != min_z {
            model.terraform_north_corner(cx, cy, min_z)?;
        }
    }
    if model.heights.is_empty() {
        return Ok(TerraformResult::empty());
    }
    model.validate_terraformable()?;
    Ok(TerraformResult::from_model(model))
}

/// Coste previsto y validación de solo lectura para elevar terreno.
pub(crate) fn check_raise_land(map: &Map, c: TileCoord, tick: u64) -> Result<i64, CommandError> {
    Ok(simulate_raise_land(map, c, terraform_cost_per_corner(tick))?.cost)
}

/// Coste previsto y validación de solo lectura para bajar terreno.
pub(crate) fn check_lower_land(map: &Map, c: TileCoord, tick: u64) -> Result<i64, CommandError> {
    Ok(simulate_lower_land(map, c, terraform_cost_per_corner(tick))?.cost)
}

pub(crate) fn check_level_land(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    mode: LevelMode,
    tick: u64,
) -> Result<i64, CommandError> {
    Ok(simulate_level_land(map, from, to, mode, terraform_cost_per_corner(tick))?.cost)
}

pub(crate) fn check_autoslope_flat(
    map: &Map,
    c: TileCoord,
    tick: u64,
) -> Result<i64, CommandError> {
    Ok(simulate_autoslope_flat(map, c, terraform_cost_per_corner(tick))?.cost)
}

pub(super) fn apply_autoslope_if_needed(
    state: &mut GameState,
    c: TileCoord,
) -> Result<i64, CommandError> {
    let cost_per = terraform_cost_per_corner(state.tick.get());
    let result = simulate_autoslope_flat(&state.map, c, cost_per)?;
    let charged = result.cost;
    if charged > 0 {
        apply_terraform_result(state, result)?;
    }
    Ok(charged)
}

pub(super) fn raise_land(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    let cost_per = terraform_cost_per_corner(state.tick.get());
    let result = simulate_raise_land(&state.map, c, cost_per)?;
    apply_terraform_result(state, result)
}

pub(super) fn lower_land(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    let cost_per = terraform_cost_per_corner(state.tick.get());
    let result = simulate_lower_land(&state.map, c, cost_per)?;
    apply_terraform_result(state, result)
}

pub(super) fn level_land(
    state: &mut GameState,
    from: TileCoord,
    to: TileCoord,
    mode: LevelMode,
) -> Result<(), CommandError> {
    let cost_per = terraform_cost_per_corner(state.tick.get());
    let result = simulate_level_land(&state.map, from, to, mode, cost_per)?;
    apply_terraform_result(state, result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileKind;
    use crate::test_fixtures::SandboxMap;
    use crate::{Command, apply_command, tile_slope_and_z};

    #[test]
    fn raise_flat_grass_creates_north_slope() {
        let mut s = SandboxMap::flat(8, 8, 4);
        let c = TileCoord::new(3, 4);
        apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
        let (tileh, _) = tile_slope_and_z(&s.map, c).unwrap();
        assert_ne!(tileh, 0, "elevar esquina norte debe crear pendiente");
        assert_eq!(s.map.get(c).unwrap().height, 5);
    }

    #[test]
    fn lower_flat_grass_removes_north_slope() {
        let mut s = SandboxMap::flat(8, 8, 4);
        let c = TileCoord::new(3, 4);
        apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
        apply_command(&mut s, &Command::LowerLand(c)).unwrap();
        let (tileh, _) = tile_slope_and_z(&s.map, c).unwrap();
        assert_eq!(tileh, 0, "bajar debe revertir la pendiente creada");
        assert_eq!(s.map.get(c).unwrap().height, 4);
    }

    #[test]
    fn autoslope_noop_near_road_network() {
        let mut s = SandboxMap::flat(12, 12, 1);
        for x in 3..=6 {
            apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(x, 5), 0x0A)).unwrap();
        }
        let c = TileCoord::new(8, 6);
        let (tileh, _) = tile_slope_and_z(&s.map, c).unwrap();
        assert_eq!(tileh, 0);
        super::apply_autoslope_if_needed(&mut s, c).unwrap();
        apply_command(
            &mut s,
            &Command::PlaceRoadBits(c, 0x0A | crate::ROAD_PLACE_FORCE_AXIS),
        )
        .unwrap();
    }

    #[test]
    fn lower_at_sea_level_fails() {
        let s = SandboxMap::flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        assert_eq!(
            check_lower_land(&s.map, c, 0),
            Err(CommandError::TerrainTooLow)
        );
    }

    #[test]
    fn lower_to_sea_creates_water() {
        let mut s = SandboxMap::flat(8, 8, 1);
        let c = TileCoord::new(3, 3);
        apply_command(&mut s, &Command::LowerLand(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
        assert_eq!(s.map.get(c).unwrap().mapt, MAPT_WATER);
    }

    #[test]
    fn raise_from_flat_water_creates_grass() {
        let mut s = SandboxMap::flat(8, 8, 0);
        let c = TileCoord::new(2, 2);
        s.map.set_kind(c, TileKind::Water).unwrap();
        s.map.set_mapt_m5(c, MAPT_WATER, 0).unwrap();
        apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    }

    #[test]
    fn level_rect_flattens_north_corners() {
        let mut s = SandboxMap::flat(8, 8, 4);
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(4, 4);
        apply_command(&mut s, &Command::RaiseLand(TileCoord::new(3, 3))).unwrap();
        apply_command(
            &mut s,
            &Command::LevelLand {
                from: a,
                to: b,
                mode: LevelMode::Level,
            },
        )
        .unwrap();
        for y in 2..=4 {
            for x in 2..=4 {
                assert_eq!(s.map.get(TileCoord::new(x, y)).unwrap().height, 4);
            }
        }
    }

    #[test]
    fn lower_rejects_road_tile() {
        let mut s = SandboxMap::flat(8, 8, 4);
        let c = TileCoord::new(2, 2);
        s.map.set_kind(c, TileKind::Road).unwrap();
        assert_eq!(
            check_lower_land(&s.map, c, 0),
            Err(CommandError::TileNotTerraformable)
        );
    }

    #[test]
    fn raise_rejects_road_tile() {
        let mut s = SandboxMap::flat(8, 8, 4);
        let c = TileCoord::new(2, 2);
        s.map.set_kind(c, TileKind::Road).unwrap();
        assert_eq!(
            check_raise_land(&s.map, c, 0),
            Err(CommandError::TileNotTerraformable)
        );
    }
}
