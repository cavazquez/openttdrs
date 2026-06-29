use std::collections::{HashSet, VecDeque};

use openttdrs_core::{
    IndustryKind, IndustrySpec, Map, TileCoord, TileKind, industry_tiles_mergeable,
};

use crate::sprites::{IndustryGfxStatus, industry_gfx_status};
use crate::state::SimWorld;
use crate::state::bootstrap::industry_group_from_gfx;

pub(crate) fn industry_gfx(tile: &openttdrs_core::Tile) -> u16 {
    u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8)
}

fn anonymous_gfx_group_match(a_gfx: u16, b_gfx: u16) -> bool {
    let ga = industry_group_from_gfx(a_gfx);
    let gb = industry_group_from_gfx(b_gfx);
    ga == "Unknown gfx" || gb == "Unknown gfx" || ga == gb
}

pub(crate) fn flood_industry_tiles(map: &Map, start: TileCoord) -> Vec<TileCoord> {
    let Some(start_tile) = map.get(start) else {
        return Vec::new();
    };
    if start_tile.kind != TileKind::Industry {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut q = VecDeque::new();
    q.push_back(start);
    while let Some(c) = q.pop_front() {
        if !seen.insert(c) {
            continue;
        }
        out.push(c);
        let Some(cur_tile) = map.get(c) else {
            continue;
        };
        let cur_gfx = industry_gfx(&cur_tile);
        for (dx, dy) in [(0i32, 1), (0, -1), (1, 0), (-1, 0)] {
            let n = TileCoord::new(c.x + dx, c.y + dy);
            let Some(tile) = map.get(n) else {
                continue;
            };
            if tile.kind != TileKind::Industry {
                continue;
            }
            let next_gfx = industry_gfx(&tile);
            if industry_tiles_mergeable(
                &cur_tile,
                &tile,
                anonymous_gfx_group_match(cur_gfx, next_gfx),
            ) {
                q.push_back(n);
            }
        }
    }
    out
}

pub(crate) fn dominant_gfx_for_component(
    map: &Map,
    anchor: TileCoord,
) -> Option<(&'static str, TileCoord, u16)> {
    let tiles = flood_industry_tiles(map, anchor);
    if tiles.is_empty() {
        return None;
    }
    let mut best_label = "Unknown gfx";
    let mut best_count = 0usize;
    let mut best_coord = anchor;
    let mut best_gfx = 0u16;
    for c in &tiles {
        let Some(tile) = map.get(*c) else {
            continue;
        };
        let gfx = industry_gfx(&tile);
        let label = industry_group_from_gfx(gfx);
        let count = tiles
            .iter()
            .filter_map(|coord| map.get(*coord))
            .map(|t| industry_group_from_gfx(industry_gfx(&t)))
            .filter(|l| *l == label)
            .count();
        if count > best_count {
            best_count = count;
            best_label = label;
            best_coord = *c;
            best_gfx = gfx;
        }
    }
    Some((best_label, best_coord, best_gfx))
}

pub(crate) fn industry_stats_for_component(
    map: &Map,
    sim: &SimWorld,
    anchor: TileCoord,
) -> Option<(IndustryKind, Option<IndustrySpec>, u32, u32, TileCoord)> {
    let tiles = flood_industry_tiles(map, anchor);
    let set: HashSet<TileCoord> = tiles.into_iter().collect();
    sim.state
        .industries
        .iter()
        .find(|i| i.tiles.iter().any(|tile| set.contains(tile)) || set.contains(&i.pos))
        .map(|i| (i.kind, i.spec, i.stock, i.capacity, i.pos))
}

pub(crate) fn kind_label(k: IndustryKind) -> &'static str {
    match k {
        IndustryKind::CoalMine => "Carbon",
        IndustryKind::Forest => "Bosque",
        IndustryKind::OilWell => "Petróleo",
        IndustryKind::Factory => "Fábrica",
    }
}

pub(crate) fn spec_label(spec: IndustrySpec) -> &'static str {
    match spec {
        IndustrySpec::CoalMine => "Mina de carbón",
        IndustrySpec::IronOreMine => "Mina de hierro",
        IndustrySpec::CopperOreMine => "Mina de cobre",
        IndustrySpec::GoldMine => "Mina de oro",
        IndustrySpec::Forest => "Bosque",
        IndustrySpec::Farm => "Granja",
        IndustrySpec::OilWells => "Pozos petroleros",
        IndustrySpec::OilRefinery => "Refinería",
        IndustrySpec::Factory => "Fábrica",
        IndustrySpec::Sawmill => "Aserradero",
        IndustrySpec::CottonCandy => "Algodón de azúcar",
        IndustrySpec::CandyFactory => "Fábrica de caramelos",
        IndustrySpec::BatteryFarm => "Granja de baterías",
        IndustrySpec::ColaWells => "Pozo de cola",
        IndustrySpec::ToyFactory => "Fábrica de juguetes",
        IndustrySpec::PlasticFountain => "Fuente de plástico",
        IndustrySpec::FizzyDrinkFactory => "Fábrica de bebidas gaseosas",
        IndustrySpec::BubbleGenerator => "Generador de burbujas",
        IndustrySpec::ToffeeQuarry => "Cantera de toffee",
        IndustrySpec::SugarMine => "Mina de azúcar",
    }
}

pub(crate) fn format_panel_title(map: &Map, sim: &SimWorld, focus: TileCoord) -> String {
    if let Some(tile) = map.get(focus) {
        let gfx = industry_gfx(&tile);
        if industry_gfx_status(gfx) == IndustryGfxStatus::OutOfRange {
            return format!("Industria - gfx {gfx} (sin sprite)");
        }
    }
    if let Some((gfx_label, _coord, _gfx)) = dominant_gfx_for_component(map, focus)
        && gfx_label != "Unknown gfx"
    {
        return format!("Industria - {gfx_label} - GFX");
    }
    if let Some((kind, spec, _, _, origin)) = industry_stats_for_component(map, sim, focus) {
        return if let Some(spec) = spec {
            format!("Industria - {} - Sim", spec_label(spec))
        } else if let Some(tile) = map.get(origin) {
            let gfx = industry_gfx(&tile);
            let gfx_label = industry_group_from_gfx(gfx);
            if gfx_label != "Unknown gfx" {
                format!("Industria - {gfx_label} - GFX")
            } else {
                format!("Industria - {} - Sim", kind_label(kind))
            }
        } else {
            format!("Industria - {} - Sim", kind_label(kind))
        };
    }
    "Industria - Sin datos de simulacion".to_string()
}
