use openttdrs_core::{GameState, Industry, IndustryKind, OttdmapExtras, TileCoord, TileKind};
use std::collections::HashSet;

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
    let (mw, mh) = state.map.dimensions();
    let mut coal_n = 0u32;
    let mut forest_n = 0u32;
    let mut industry_n = 0u32;
    let mut seen_ottd_industry_ids: HashSet<u8> = HashSet::new();
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
                    if from_ottd_file {
                        let industry_id = tile.m1 & 0x7F;
                        if !seen_ottd_industry_ids.insert(industry_id) {
                            industry_n += 1;
                            continue;
                        }
                        let kind = if let Some(ex) = ottd_extras {
                            ex.industry_type_for_tile_index(tile.m1)
                                .map(industry_kind_from_ottd_type)
                                .unwrap_or_else(|| {
                                    let gfx =
                                        u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                                    classify_industry_kind_from_gfx(gfx)
                                })
                        } else {
                            let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                            classify_industry_kind_from_gfx(gfx)
                        };
                        state.industries.push(Industry::new(c, kind));
                    } else if industry_n.is_multiple_of(16) {
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
