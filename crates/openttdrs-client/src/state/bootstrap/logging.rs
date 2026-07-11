use bevy::prelude::*;
use openttdrs_core::{
    GameState, IndustryKind, OttdmapExtras, TileCoord, TileKind, TnbpDecoded, VehicleKind,
    jgr_tunnels_from_decoded,
};
use std::collections::BTreeMap;

use super::industries::industry_group_from_gfx;

pub(crate) fn log_detection_summary(
    state: &GameState,
    loaded_from_file: bool,
    extras: Option<&OttdmapExtras>,
) {
    let (mw, mh) = state.map.dimensions();
    info!("Resumen deteccion: mapa {mw}x{mh} ({} teselas)", mw * mh);

    let mut tiles: BTreeMap<String, u32> = BTreeMap::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            let Some(kind) = state.map.get_kind(c) else {
                continue;
            };
            let key = match kind {
                TileKind::Grass => "Grass".to_string(),
                TileKind::Water => "Water".to_string(),
                TileKind::Forest => "Forest".to_string(),
                TileKind::CoalField => "CoalField".to_string(),
                TileKind::Road => "Road".to_string(),
                TileKind::Rail => "Rail".to_string(),
                TileKind::RoadDepot => "RoadDepot".to_string(),
                TileKind::RailDepot => "RailDepot".to_string(),
                TileKind::RoadTunnel => "RoadTunnel".to_string(),
                TileKind::RailTunnel => "RailTunnel".to_string(),
                TileKind::RoadBridge => "RoadBridge".to_string(),
                TileKind::RailBridge => "RailBridge".to_string(),
                TileKind::House => "House".to_string(),
                TileKind::Station => "Station".to_string(),
                TileKind::Industry => "Industry".to_string(),
                TileKind::ShipDepot => "ShipDepot".to_string(),
                TileKind::Airport => "Airport".to_string(),
                TileKind::Void => "Void".to_string(),
                TileKind::Unknown(v) => format!("Unknown({v})"),
            };
            *tiles.entry(key).or_insert(0) += 1;
        }
    }

    info!("Teselas detectadas por tipo:");
    for (kind, count) in tiles {
        info!("  - {kind}: {count}");
    }

    if loaded_from_file {
        let mut industry_groups: BTreeMap<&'static str, u32> = BTreeMap::new();
        for y in 0..mh {
            for x in 0..mw {
                let c = TileCoord::new(x as i32, y as i32);
                let Some(tile) = state.map.get(c) else {
                    continue;
                };
                if tile.kind != TileKind::Industry {
                    continue;
                }
                let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
                let group = industry_group_from_gfx(gfx);
                *industry_groups.entry(group).or_insert(0) += 1;
            }
        }
        info!("Teselas de industria por grupo OpenTTD (gfx):");
        for (group, count) in industry_groups {
            info!("  - {group}: {count}");
        }
    }

    if let Some(ex) = extras {
        if !ex.industry_types.is_empty() {
            info!(
                "Footers .ottdmap: INDP con {} pares (indice industria -> tipo OpenTTD)",
                ex.industry_types.len()
            );
        }
        if let Some(b) = ex.stnn_blob.as_ref() {
            info!(
                "Footers .ottdmap: STNN blob {} bytes (pool serializado OpenTTD; usar STXY o MP_STATION para sim)",
                b.len()
            );
        }
        if !ex.station_xy.is_empty() {
            info!(
                "Footers .ottdmap: STXY con {} teselas MP_STATION (x,y) desde export",
                ex.station_xy.len()
            );
        }
        let tnbp_len = ex.tnbp_blob_len();
        if tnbp_len > 0 {
            match ex.decode_tnbp() {
                Some(Err(e)) => {
                    info!("Footers .ottdmap: TNBP {tnbp_len} bytes (decode fallo: {e:?})");
                }
                Some(Ok(dec)) => {
                    let jgr = jgr_tunnels_from_decoded(&dec);
                    if !jgr.is_empty() {
                        info!(
                            "Footers .ottdmap: TNBP {tnbp_len} bytes -> {} tunel(es) JGR (`tile_n`/`tile_s`)",
                            jgr.len()
                        );
                    } else {
                        match &dec {
                            TnbpDecoded::ChTable {
                                fields,
                                rows,
                                skipped_rows,
                            } => {
                                info!(
                                    "Footers .ottdmap: TNBP {tnbp_len} bytes -> tabla Sl ({} campos, {} filas, {} omitidas)",
                                    fields.len(),
                                    rows.len(),
                                    skipped_rows
                                );
                            }
                            TnbpDecoded::RawGammaSegments { segments } => {
                                info!(
                                    "Footers .ottdmap: TNBP {tnbp_len} bytes -> {} segmento(s) gamma (sin tabla Sl)",
                                    segments.len()
                                );
                            }
                        }
                    }
                }
                None => {}
            }
            let tunnels = ex.jgr_tunnels_from_tnbp();
            if !tunnels.is_empty() {
                let (n_ok, s_ok, tot) = state.map.jgr_tunnel_endpoint_match_stats(&tunnels);
                info!(
                    "TNBP vs mapa: {tot} registro(s) JGR; extremos en teselas MP_TUNNELBRIDGE: norte {n_ok}/{tot}, sur {s_ok}/{tot}"
                );
            }
            if let Ok(v) = std::env::var("OTTDMAP_TNBP_JSON")
                && (v == "1" || v.eq_ignore_ascii_case("true"))
                && let Some(j) = ex.tnbp_json_summary()
            {
                info!("TNBP JSON: {j}");
            }
        }
    }

    let mut industries: BTreeMap<&'static str, u32> = BTreeMap::new();
    for ind in &state.industries {
        let key = match ind.kind {
            IndustryKind::CoalMine => "CoalMine",
            IndustryKind::Forest => "Forest",
            IndustryKind::OilWell => "OilWell",
            IndustryKind::Factory => "Factory",
        };
        *industries.entry(key).or_insert(0) += 1;
    }
    info!("Industrias detectadas: {}", state.industries.len());
    for (kind, count) in industries {
        info!("  - Industria {kind}: {count}");
    }

    info!("Estaciones detectadas: {}", state.stations.len());
    if loaded_from_file && state.stations.is_empty() {
        info!("  - Nota: no hay teselas MP_STATION ni estaciones junto a industrias simuladas.");
    }

    let mut vehicles: BTreeMap<&'static str, u32> = BTreeMap::new();
    for v in &state.vehicles {
        let key = match v.kind {
            VehicleKind::Truck => "Truck",
            VehicleKind::Bus => "Bus",
            VehicleKind::Tram => "Tram",
            VehicleKind::Train => "Train",
            VehicleKind::Ship => "Ship",
            VehicleKind::Aircraft => "Aircraft",
        };
        *vehicles.entry(key).or_insert(0) += 1;
    }
    info!("Vehiculos detectados: {}", state.vehicles.len());
    if loaded_from_file && state.vehicles.is_empty() {
        info!("  - Nota: no hay vehiculos (hace falta al menos una industria y una estacion).");
    }
    for (kind, count) in vehicles {
        info!("  - Vehiculo {kind}: {count}");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod logging_coverage_tests {
    use super::log_detection_summary;
    use openttdrs_core::{
        GameState, Industry, IndustryKind, OttdmapExtras, TileCoord, TileKind, Vehicle, VehicleKind,
    };

    #[test]
    fn log_detection_summary_runs_on_tiny_map() {
        let state = GameState::new(4, 4);
        log_detection_summary(&state, false, None);
    }

    #[test]
    fn log_detection_summary_loaded_file_with_extras_and_entities() {
        let mut state = GameState::new(4, 4);
        state
            .map
            .set_kind(TileCoord::new(0, 0), TileKind::Industry)
            .unwrap();
        state
            .map
            .set_kind(TileCoord::new(1, 0), TileKind::Station)
            .unwrap();
        state
            .map
            .set_kind(TileCoord::new(2, 0), TileKind::Road)
            .unwrap();
        state
            .map
            .set_kind(TileCoord::new(3, 0), TileKind::Rail)
            .unwrap();
        state
            .map
            .set_kind(TileCoord::new(0, 1), TileKind::Unknown(7))
            .unwrap();

        state.industries.push(Industry {
            pos: TileCoord::new(0, 0),
            tiles: vec![TileCoord::new(0, 0)],
            spec: None,
            kind: IndustryKind::OilWell,
            stock: 10,
            capacity: 100,
            random_colour: 0,
            ..Default::default()
        });
        state
            .stations
            .push(openttdrs_core::Station::new(TileCoord::new(1, 0)));
        state.vehicles.push(Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ));

        let extras = OttdmapExtras {
            industry_types: vec![(0, 12)],
            stnn_blob: Some(vec![1, 2, 3]),
            tnbp_blob: Some(vec![9, 9, 9, 9]),
            station_xy: vec![(1, 0)],
        };
        log_detection_summary(&state, true, Some(&extras));
    }
}
