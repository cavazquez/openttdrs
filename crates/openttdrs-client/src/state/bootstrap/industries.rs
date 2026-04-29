use openttdrs_core::{GameState, Industry, IndustryKind, OttdmapExtras, TileCoord, TileKind};
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
                        state.industries.push(Industry::new(c, IndustryKind::CoalMine));
                    }
                    coal_n += 1;
                }
                TileKind::Forest if !from_ottd_file => {
                    if forest_n.is_multiple_of(stride_proc) {
                        state.industries.push(Industry::new(c, IndustryKind::Forest));
                    }
                    forest_n += 1;
                }
                TileKind::Industry => {
                    if industry_n.is_multiple_of(16) {
                        let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                        let kind = classify_industry_kind_from_gfx(gfx);
                        state.industries.push(Industry::new(c, kind));
                    }
                    industry_n += 1;
                }
                _ => {}
            }
        }
    }
}

fn place_industries_from_map_components(state: &mut GameState, ottd_extras: Option<&OttdmapExtras>) {
    for component in industry_components(state) {
        let Some(&origin) = component.first() else {
            continue;
        };
        let Some(tile) = state.map.get(origin) else {
            continue;
        };
        let kind = if let Some(ex) = ottd_extras {
            ex.industry_type_for_tile_index(tile.m1)
                .map(industry_kind_from_ottd_type)
                .unwrap_or_else(|| {
                    let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                    classify_industry_kind_from_gfx(gfx)
                })
        } else {
            let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
            classify_industry_kind_from_gfx(gfx)
        };
        state.industries.push(Industry::new(origin, kind));
    }
}

fn industry_components(state: &GameState) -> Vec<Vec<TileCoord>> {
    let (mw, mh) = state.map.dimensions();
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut out: Vec<Vec<TileCoord>> = Vec::new();

    for y in 0..mh as i32 {
        for x in 0..mw as i32 {
            let start = TileCoord::new(x, y);
            if state.map.get_kind(start) != Some(TileKind::Industry) {
                continue;
            }
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
                    if state.map.get_kind(next) != Some(TileKind::Industry) {
                        continue;
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

fn classify_industry_kind_from_gfx(gfx: u16) -> IndustryKind {
    match gfx {
        43..=46 => IndustryKind::Factory,
        47..=51 => IndustryKind::OilWell,
        0..=6 | 60..=71 | 89..=90 | 91..=99 => IndustryKind::CoalMine,
        24..=28 | 52..=57 | 72..=88 => IndustryKind::Forest,
        _ => {
            if gfx.is_multiple_of(2) {
                IndustryKind::CoalMine
            } else {
                IndustryKind::Forest
            }
        }
    }
}

pub(crate) fn industry_group_from_gfx(gfx: u16) -> &'static str {
    match gfx {
        0..=6 => "Coal Mine",
        7..=10 => "Power Station",
        11..=15 => "Sawmill",
        16..=23 => "Oil Refinery",
        24..=28 => "Forest",
        29..=32 => "Printing Works",
        33..=38 => "Oil Rig",
        39..=42 => "Steel Mill",
        43..=46 => "Factory",
        47..=51 => "Oil Wells",
        52..=57 => "Farm",
        58..=59 => "Bank",
        60..=71 => "Copper Ore Mine",
        72..=88 => "Plantations/Others",
        89..=90 => "Gold Mine",
        91..=99 => "Iron Ore Mine",
        100..=119 => "Other climates",
        _ => "Unknown gfx",
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_industry_kind_from_gfx, industry_group_from_gfx, place_industries};
    use openttdrs_core::{GameState, IndustryKind, TileCoord, TileKind};

    #[test]
    fn classify_industry_kind_matches_known_ranges() {
        assert_eq!(classify_industry_kind_from_gfx(44), IndustryKind::Factory);
        assert_eq!(classify_industry_kind_from_gfx(48), IndustryKind::OilWell);
        assert_eq!(classify_industry_kind_from_gfx(61), IndustryKind::CoalMine);
        assert_eq!(classify_industry_kind_from_gfx(26), IndustryKind::Forest);
    }

    #[test]
    fn industry_group_labels_known_and_unknown() {
        assert_eq!(industry_group_from_gfx(43), "Factory");
        assert_eq!(industry_group_from_gfx(7), "Power Station");
        assert_eq!(industry_group_from_gfx(255), "Unknown gfx");
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
}
