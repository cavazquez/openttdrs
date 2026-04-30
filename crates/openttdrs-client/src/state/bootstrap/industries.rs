use openttdrs_core::{
    GameState, Industry, IndustryKind, IndustrySpec, OttdmapExtras, TileCoord, TileKind,
};
use std::collections::{HashSet, VecDeque};

/// Mapea `IndustryType` de OpenTTD (footer `INDP`) a [`IndustryKind`] del core (best-effort).
fn industry_kind_from_ottd_type(t: u8) -> IndustryKind {
    match t {
        2 | 3 | 9 | 14 | 19 | 20 | 24 | 25 => IndustryKind::Forest,
        11..=13 => IndustryKind::Factory,
        16 | 17 | 33 => IndustryKind::OilWell,
        _ => IndustryKind::CoalMine,
    }
}

pub(crate) fn place_industries(
    state: &mut GameState,
    from_ottd_file: bool,
    ottd_extras: Option<&OttdmapExtras>,
) {
    if from_ottd_file {
        place_industries_from_map_components(state, ottd_extras);
        return;
    }

    let (mw, mh) = state.map.dimensions();
    let mut coal_n = 0u32;
    let mut forest_n = 0u32;
    let mut industry_n = 0u32;
    let stride_proc = 4u32;

    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            let Some(tile) = state.map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::CoalField if !from_ottd_file => {
                    if coal_n.is_multiple_of(stride_proc) {
                        state
                            .industries
                            .push(Industry::new(c, IndustryKind::CoalMine));
                    }
                    coal_n += 1;
                }
                TileKind::Forest if !from_ottd_file => {
                    if forest_n.is_multiple_of(stride_proc) {
                        state
                            .industries
                            .push(Industry::new(c, IndustryKind::Forest));
                    }
                    forest_n += 1;
                }
                TileKind::Industry => {
                    if industry_n.is_multiple_of(16) {
                        let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                        let kind = classify_industry_kind_from_gfx(gfx);
                        if let Some(spec) = classify_industry_spec_from_gfx(gfx) {
                            state.industries.push(Industry::with_tiles_spec(
                                c,
                                kind,
                                spec,
                                vec![c],
                            ));
                        } else {
                            state.industries.push(Industry::new(c, kind));
                        }
                    }
                    industry_n += 1;
                }
                _ => {}
            }
        }
    }
}

fn place_industries_from_map_components(
    state: &mut GameState,
    ottd_extras: Option<&OttdmapExtras>,
) {
    for component in industry_components(state) {
        let Some(&origin) = component.first() else {
            continue;
        };
        let Some(tile) = state.map.get(origin) else {
            continue;
        };
        let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
        let kind = if let Some(ex) = ottd_extras {
            ex.industry_type_for_tile_index(tile.m1)
                .map(industry_kind_from_ottd_type)
                .unwrap_or_else(|| classify_industry_kind_from_gfx(gfx))
        } else {
            classify_industry_kind_from_gfx(gfx)
        };
        if let Some(spec) = classify_industry_spec_from_gfx(gfx) {
            state
                .industries
                .push(Industry::with_tiles_spec(origin, kind, spec, component));
        } else {
            state
                .industries
                .push(Industry::with_tiles(origin, kind, component));
        }
    }
}

fn industry_components(state: &GameState) -> Vec<Vec<TileCoord>> {
    let (mw, mh) = state.map.dimensions();
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut out: Vec<Vec<TileCoord>> = Vec::new();

    for y in 0..mh as i32 {
        for x in 0..mw as i32 {
            let start = TileCoord::new(x, y);
            let Some(start_tile) = state.map.get(start) else {
                continue;
            };
            if start_tile.kind != TileKind::Industry {
                continue;
            }
            let start_industry_id = start_tile.m1;
            let require_same_industry_id = start_industry_id != 0;
            let start_gfx = u16::from(start_tile.m5) | (u16::from((start_tile.m6 >> 2) & 1) << 8);
            let start_gfx_group = industry_group_from_gfx(start_gfx);
            let require_same_gfx_group = start_gfx_group != "Unknown gfx";
            if !visited.insert((x, y)) {
                continue;
            }

            let mut queue = VecDeque::from([start]);
            let mut component = Vec::new();
            while let Some(cur) = queue.pop_front() {
                component.push(cur);
                for next in [
                    TileCoord::new(cur.x - 1, cur.y),
                    TileCoord::new(cur.x + 1, cur.y),
                    TileCoord::new(cur.x, cur.y - 1),
                    TileCoord::new(cur.x, cur.y + 1),
                ] {
                    if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                        continue;
                    }
                    let Some(next_tile) = state.map.get(next) else {
                        continue;
                    };
                    if next_tile.kind != TileKind::Industry {
                        continue;
                    }
                    if require_same_industry_id && next_tile.m1 != start_industry_id {
                        continue;
                    }
                    if require_same_gfx_group {
                        let next_gfx =
                            u16::from(next_tile.m5) | (u16::from((next_tile.m6 >> 2) & 1) << 8);
                        if industry_group_from_gfx(next_gfx) != start_gfx_group {
                            continue;
                        }
                    }
                    if visited.insert((next.x, next.y)) {
                        queue.push_back(next);
                    }
                }
            }
            out.push(component);
        }
    }

    out
}

type IndustryGfxRange = (u16, u16, &'static str, Option<IndustryKind>);

const INDUSTRY_GFX_RANGES: [IndustryGfxRange; 19] = [
    (0, 6, "Coal Mine", Some(IndustryKind::CoalMine)),
    (7, 10, "Power Station", None),
    (11, 15, "Sawmill", None),
    (16, 17, "Forest", Some(IndustryKind::Forest)),
    (18, 23, "Oil Refinery", Some(IndustryKind::OilWell)),
    (24, 28, "Oil Rig", Some(IndustryKind::OilWell)),
    (29, 32, "Oil Wells", Some(IndustryKind::OilWell)),
    (33, 38, "Farm", Some(IndustryKind::Forest)),
    (39, 42, "Factory", Some(IndustryKind::Factory)),
    (43, 46, "Printing Works", None),
    (47, 51, "Copper Ore Mine", Some(IndustryKind::CoalMine)),
    (52, 57, "Steel Mill", None),
    (58, 59, "Bank", None),
    (60, 67, "Food Processing Plant", Some(IndustryKind::Factory)),
    (68, 75, "Paper Mill", Some(IndustryKind::Factory)),
    (76, 88, "Gold Mine", Some(IndustryKind::CoalMine)),
    (89, 90, "Bank", None),
    (91, 99, "Diamond Mine", Some(IndustryKind::CoalMine)),
    (100, 115, "Iron Ore Mine", Some(IndustryKind::CoalMine)),
];

fn gfx_range_info(gfx: u16) -> Option<IndustryGfxRange> {
    INDUSTRY_GFX_RANGES
        .iter()
        .copied()
        .find(|(start, end, _, _)| (*start..=*end).contains(&gfx))
}

fn classify_industry_kind_from_gfx(gfx: u16) -> IndustryKind {
    if let Some((_, _, _, kind)) = gfx_range_info(gfx) {
        // Si el grupo existe pero no mapea 1:1 a nuestro modelo simplificado,
        // lo tratamos como industria de procesamiento.
        return kind.unwrap_or(IndustryKind::Factory);
    }
    if gfx.is_multiple_of(2) {
        IndustryKind::CoalMine
    } else {
        IndustryKind::Forest
    }
}

fn classify_industry_spec_from_gfx(gfx: u16) -> Option<IndustrySpec> {
    match gfx {
        0..=6 => Some(IndustrySpec::CoalMine),
        11..=15 => Some(IndustrySpec::Sawmill),
        16..=17 => Some(IndustrySpec::Forest),
        18..=23 => Some(IndustrySpec::OilRefinery),
        29..=32 => Some(IndustrySpec::OilWells),
        33..=38 => Some(IndustrySpec::Farm),
        39..=42 => Some(IndustrySpec::Factory),
        47..=51 => Some(IndustrySpec::CopperOreMine),
        72..=88 => Some(IndustrySpec::GoldMine),
        100..=115 => Some(IndustrySpec::IronOreMine),
        _ => None,
    }
}

pub(crate) fn industry_group_from_gfx(gfx: u16) -> &'static str {
    if let Some((_, _, label, _)) = gfx_range_info(gfx) {
        return label;
    }
    if (100..=119).contains(&gfx) {
        return "Other climates";
    }
    "Unknown gfx"
}

#[cfg(test)]
mod tests {
    use super::{
        classify_industry_kind_from_gfx, classify_industry_spec_from_gfx, industry_group_from_gfx,
        place_industries,
    };
    use openttdrs_core::{GameState, IndustryKind, IndustrySpec, TileCoord, TileKind};

    #[test]
    fn classify_industry_kind_matches_known_ranges() {
        assert_eq!(classify_industry_kind_from_gfx(18), IndustryKind::OilWell);
        assert_eq!(classify_industry_kind_from_gfx(29), IndustryKind::OilWell);
        assert_eq!(classify_industry_kind_from_gfx(40), IndustryKind::Factory);
        assert_eq!(classify_industry_kind_from_gfx(48), IndustryKind::CoalMine);
        assert_eq!(classify_industry_kind_from_gfx(61), IndustryKind::Factory);
        assert_eq!(classify_industry_kind_from_gfx(16), IndustryKind::Forest);
    }

    #[test]
    fn industry_group_labels_known_and_unknown() {
        assert_eq!(industry_group_from_gfx(40), "Factory");
        assert_eq!(industry_group_from_gfx(7), "Power Station");
        assert_eq!(industry_group_from_gfx(255), "Unknown gfx");
    }

    #[test]
    fn industry_group_gold_and_iron_mine_labels_match_ranges() {
        assert_eq!(industry_group_from_gfx(76), "Gold Mine");
        assert_eq!(industry_group_from_gfx(88), "Gold Mine");
        assert_eq!(industry_group_from_gfx(89), "Bank");
        assert_eq!(industry_group_from_gfx(99), "Diamond Mine");
        assert_eq!(industry_group_from_gfx(100), "Iron Ore Mine");
        assert_eq!(classify_industry_kind_from_gfx(76), IndustryKind::CoalMine);
        assert_eq!(classify_industry_kind_from_gfx(99), IndustryKind::CoalMine);
    }

    #[test]
    fn classify_industry_spec_matches_farm_range() {
        assert_eq!(
            classify_industry_spec_from_gfx(33),
            Some(IndustrySpec::Farm)
        );
    }

    #[test]
    fn place_industries_from_file_deduplicates_same_industry_id() {
        let mut state = GameState::new(2, 1);
        let _ = state.map.set_kind(TileCoord::new(0, 0), TileKind::Industry);
        let _ = state.map.set_kind(TileCoord::new(1, 0), TileKind::Industry);

        place_industries(&mut state, true, None);

        // Ambos tiles son adyacentes y forman un único componente de industria.
        assert_eq!(state.industries.len(), 1);
    }

    #[test]
    fn place_industries_from_file_separates_disconnected_components() {
        let mut state = GameState::new(3, 1);
        let _ = state.map.set_kind(TileCoord::new(0, 0), TileKind::Industry);
        let _ = state.map.set_kind(TileCoord::new(2, 0), TileKind::Industry);

        place_industries(&mut state, true, None);

        assert_eq!(state.industries.len(), 2);
    }

    #[test]
    fn place_industries_from_file_separates_adjacent_different_m1() {
        let mut state = GameState::new(2, 1);
        let c0 = TileCoord::new(0, 0);
        let c1 = TileCoord::new(1, 0);
        let mut t0 = state.map.get(c0).expect("tile 0");
        t0.kind = TileKind::Industry;
        t0.m1 = 10;
        t0.m5 = 16; // Forest
        let mut t1 = state.map.get(c1).expect("tile 1");
        t1.kind = TileKind::Industry;
        t1.m1 = 11;
        t1.m5 = 18; // Oil Refinery
        let _ = state.map.set_tile(c0, t0);
        let _ = state.map.set_tile(c1, t1);

        place_industries(&mut state, true, None);

        assert_eq!(state.industries.len(), 2);
    }

    #[test]
    fn place_industries_from_file_separates_adjacent_same_m1_different_gfx_group() {
        let mut state = GameState::new(2, 1);
        let c0 = TileCoord::new(0, 0);
        let c1 = TileCoord::new(1, 0);
        let mut t0 = state.map.get(c0).expect("tile 0");
        t0.kind = TileKind::Industry;
        t0.m1 = 10;
        t0.m5 = 18; // Oil Refinery
        let mut t1 = state.map.get(c1).expect("tile 1");
        t1.kind = TileKind::Industry;
        t1.m1 = 10;
        t1.m5 = 16; // Forest
        let _ = state.map.set_tile(c0, t0);
        let _ = state.map.set_tile(c1, t1);

        place_industries(&mut state, true, None);

        assert_eq!(state.industries.len(), 2);
    }
}
