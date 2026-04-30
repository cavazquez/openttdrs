#![allow(clippy::expect_used)]

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use openttdrs_core::{Map, TileCoord, TileKind, dense_payload_end};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct Snapshot {
    source_path: String,
    map: SnapshotMap,
    hashes: SnapshotHashes,
    extras: SnapshotExtras,
    components: SnapshotComponents,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotMap {
    width: u32,
    height: u32,
    tile_count: u64,
    tile_kind_counts: BTreeMap<String, u64>,
    max_height: u8,
    min_height: u8,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotHashes {
    height_hash_fnv1a64: String,
    kind_hash_fnv1a64: String,
    mapt_hash_fnv1a64: String,
    rail_bits_hash_fnv1a64: String,
    road_bits_hash_fnv1a64: String,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotExtras {
    dense_payload_end: usize,
    footer_industry_pairs: usize,
    footer_station_xy: usize,
    footer_tnbp_blob_len: usize,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotComponents {
    industry_components: usize,
    station_components: usize,
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone, Copy)]
struct Fnv1a64(u64);

impl Fnv1a64 {
    fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn write_u8(&mut self, v: u8) {
        self.0 ^= u64::from(v);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn write_u16(&mut self, v: u16) {
        for b in v.to_le_bytes() {
            self.write_u8(b);
        }
    }

    fn finish_hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

fn tile_kind_name(kind: TileKind) -> String {
    match kind {
        TileKind::Void => "Void".to_string(),
        TileKind::Grass => "Grass".to_string(),
        TileKind::Water => "Water".to_string(),
        TileKind::Road => "Road".to_string(),
        TileKind::Rail => "Rail".to_string(),
        TileKind::RoadDepot => "RoadDepot".to_string(),
        TileKind::RailDepot => "RailDepot".to_string(),
        TileKind::RoadTunnel => "RoadTunnel".to_string(),
        TileKind::RailTunnel => "RailTunnel".to_string(),
        TileKind::RoadBridge => "RoadBridge".to_string(),
        TileKind::RailBridge => "RailBridge".to_string(),
        TileKind::House => "House".to_string(),
        TileKind::Industry => "Industry".to_string(),
        TileKind::Station => "Station".to_string(),
        TileKind::Forest => "Forest".to_string(),
        TileKind::CoalField => "CoalField".to_string(),
        TileKind::Unknown(n) => format!("Unknown({n})"),
    }
}

fn neighbors4(x: i32, y: i32, w: u32, h: u32) -> impl Iterator<Item = TileCoord> {
    let mut v = Vec::with_capacity(4);
    if x > 0 {
        v.push(TileCoord::new(x - 1, y));
    }
    if y > 0 {
        v.push(TileCoord::new(x, y - 1));
    }
    if (x as u32) + 1 < w {
        v.push(TileCoord::new(x + 1, y));
    }
    if (y as u32) + 1 < h {
        v.push(TileCoord::new(x, y + 1));
    }
    v.into_iter()
}

fn count_components_by_kind(map: &Map, kind: TileKind) -> usize {
    let (w, h) = map.dimensions();
    let n = usize::try_from(w.saturating_mul(h)).expect("map size en usize");
    let mut visited = vec![false; n];
    let mut comps = 0usize;

    for y in 0..h {
        for x in 0..w {
            let idx = usize::try_from(y * w + x).expect("idx");
            if visited[idx] {
                continue;
            }
            let c = TileCoord::new(x as i32, y as i32);
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind != kind {
                visited[idx] = true;
                continue;
            }
            comps += 1;
            visited[idx] = true;
            let mut q = VecDeque::from([c]);
            while let Some(cur) = q.pop_front() {
                for nb in neighbors4(cur.x, cur.y, w, h) {
                    let nidx = usize::try_from((nb.y as u32) * w + (nb.x as u32)).expect("nidx");
                    if visited[nidx] {
                        continue;
                    }
                    let is_same = map.get(nb).is_some_and(|t| t.kind == kind);
                    visited[nidx] = true;
                    if is_same {
                        q.push_back(nb);
                    }
                }
            }
        }
    }

    comps
}

fn usage_and_exit() -> ! {
    eprintln!("Uso: snapshot_dumper <mapa.ottdmap> [salida.json]");
    std::process::exit(2);
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input = args.next().map(PathBuf::from).ok_or_else(|| {
        usage_and_exit();
    });
    let output = args.next().map(PathBuf::from);
    if args.next().is_some() {
        usage_and_exit();
    }
    let input = input.unwrap_or_else(|_| unreachable!());

    let raw =
        std::fs::read(&input).map_err(|e| format!("no se pudo leer {}: {e}", input.display()))?;
    let (map, extras) = Map::from_ottd_binary_with_extras(&raw)
        .map_err(|_| "formato .ottdmap inválido (MAP1)".to_string())?;
    let (w, h) = map.dimensions();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();

    let mut min_height = u8::MAX;
    let mut max_height = u8::MIN;
    let mut h_height = Fnv1a64::new();
    let mut h_kind = Fnv1a64::new();
    let mut h_mapt = Fnv1a64::new();
    let mut h_rail = Fnv1a64::new();
    let mut h_road = Fnv1a64::new();

    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x as i32, y as i32);
            let t = map
                .get(c)
                .ok_or_else(|| "tile fuera de rango".to_string())?;
            *counts.entry(tile_kind_name(t.kind)).or_insert(0) += 1;
            min_height = min_height.min(t.height);
            max_height = max_height.max(t.height);
            h_height.write_u8(t.height);
            h_kind.write_u8(match t.kind {
                TileKind::Void => 0,
                TileKind::Grass => 1,
                TileKind::Water => 2,
                TileKind::Road => 3,
                TileKind::Rail => 4,
                TileKind::RoadDepot => 10,
                TileKind::RailDepot => 11,
                TileKind::RoadTunnel => 12,
                TileKind::RailTunnel => 13,
                TileKind::RoadBridge => 14,
                TileKind::RailBridge => 15,
                TileKind::House => 5,
                TileKind::Industry => 6,
                TileKind::Station => 7,
                TileKind::Forest => 8,
                TileKind::CoalField => 9,
                TileKind::Unknown(n) => 128_u8.wrapping_add(n),
            });
            h_mapt.write_u8(t.mapt);
            if matches!(t.kind, TileKind::Rail) {
                h_rail.write_u8(t.m5 & 0x3F);
                h_rail.write_u8(t.m3);
                h_rail.write_u8(t.m3hi);
            }
            if matches!(t.kind, TileKind::Road) {
                h_road.write_u8(t.m5 & 0x0F);
                h_road.write_u16(t.m8);
            }
        }
    }

    let n = usize::try_from(w.saturating_mul(h)).expect("n");
    let snapshot = Snapshot {
        source_path: input.display().to_string(),
        map: SnapshotMap {
            width: w,
            height: h,
            tile_count: u64::try_from(n).expect("u64"),
            tile_kind_counts: counts,
            max_height,
            min_height,
        },
        hashes: SnapshotHashes {
            height_hash_fnv1a64: h_height.finish_hex(),
            kind_hash_fnv1a64: h_kind.finish_hex(),
            mapt_hash_fnv1a64: h_mapt.finish_hex(),
            rail_bits_hash_fnv1a64: h_rail.finish_hex(),
            road_bits_hash_fnv1a64: h_road.finish_hex(),
        },
        extras: SnapshotExtras {
            dense_payload_end: dense_payload_end(&raw, n),
            footer_industry_pairs: extras.industry_types.len(),
            footer_station_xy: extras.station_xy.len(),
            footer_tnbp_blob_len: extras.tnbp_blob_len(),
        },
        components: SnapshotComponents {
            industry_components: count_components_by_kind(&map, TileKind::Industry),
            station_components: count_components_by_kind(&map, TileKind::Station),
        },
    };

    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| format!("error serializando snapshot: {e}"))?;

    if let Some(out) = output {
        std::fs::write(&out, json)
            .map_err(|e| format!("no se pudo escribir {}: {e}", out.display()))?;
        println!("snapshot escrito en {}", out.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

#[cfg(test)]
mod snapshot_dumper_coverage_tests {
    use super::{Fnv1a64, count_components_by_kind, neighbors4, tile_kind_name};
    use openttdrs_core::{Map, TileKind};

    const M3_ROAD_TRAM: &[u8] = include_bytes!("../../tests/fixtures/m3_road_tram_2x2.ottdmap");

    #[test]
    fn tile_kind_name_variants() {
        assert_eq!(tile_kind_name(TileKind::Grass), "Grass");
        assert!(tile_kind_name(TileKind::Unknown(5)).contains('5'));
    }

    #[test]
    fn fnv_smoke() {
        let mut h = Fnv1a64::new();
        h.write_u8(7);
        h.write_u16(0x1234);
        assert_eq!(h.finish_hex().len(), 16);
    }

    #[test]
    fn neighbors4_smoke() {
        let v: Vec<_> = neighbors4(1, 1, 4, 4).collect();
        assert!(!v.is_empty());
    }

    #[test]
    fn count_components_on_fixture() {
        let (map, _) = Map::from_ottd_binary_with_extras(M3_ROAD_TRAM).expect("fixture");
        let n = count_components_by_kind(&map, TileKind::Grass);
        assert!(n >= 1);
    }
}
