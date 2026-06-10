//! Pipeline completo `.sav` → `SavGame` → `GameState` con un save sintético
//! (contenedor OTTN/OTTZ + chunks MAPS/MAPT/MAPH/STNN/CITY estilo CH_TABLE).

#![allow(clippy::expect_used, clippy::cast_possible_truncation)]

use openttdrs_core::{GameState, StopKind, TileCoord, TileKind, sav};

const CH_RIFF: u8 = 0;
const CH_TABLE: u8 = 3;

const MAP_W: u32 = 64;
const MAP_H: u32 = 64;

fn write_gamma(v: u32, buf: &mut Vec<u8>) {
    assert!(v < (1 << 14), "el test usa gammas pequeños");
    if v < (1 << 7) {
        buf.push(v as u8);
    } else {
        buf.push(0x80 | ((v >> 8) as u8));
        buf.push((v & 0xFF) as u8);
    }
}

fn write_str(s: &str, buf: &mut Vec<u8>) {
    write_gamma(s.len() as u32, buf);
    buf.extend_from_slice(s.as_bytes());
}

fn riff_chunk(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    let size = payload.len();
    out.push((((size >> 24) as u8) << 4) | CH_RIFF);
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

/// Chunk CH_TABLE: header plano `(tipo, clave)` + registros.
fn table_chunk(name: &[u8; 4], fields: &[(u8, &str)], records: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    for (ftype, key) in fields {
        header.push(*ftype);
        write_str(key, &mut header);
    }
    header.push(0);

    let mut out = name.to_vec();
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(&header);
    for r in records {
        write_gamma(r.len() as u32 + 1, &mut out);
        out.extend_from_slice(r);
    }
    write_gamma(0, &mut out);
    out
}

fn synthetic_sav_payload() -> Vec<u8> {
    let n = (MAP_W * MAP_H) as usize;
    let mut data = Vec::new();

    // MAPS: dimensiones.
    let mut dims = Vec::new();
    dims.extend_from_slice(&MAP_W.to_be_bytes());
    dims.extend_from_slice(&MAP_H.to_be_bytes());
    data.extend_from_slice(&table_chunk(
        b"MAPS",
        &[(6, "dim_x"), (6, "dim_y")],
        &[dims],
    ));

    // MAPT: pradera con una estación en (5,2) y una vía en (6,2).
    let mut mapt = vec![0u8; n];
    let station_tile = 2 * MAP_W as usize + 5;
    mapt[station_tile] = 5 << 4; // MP_STATION
    mapt[2 * MAP_W as usize + 6] = 1 << 4; // MP_RAILWAY
    data.extend_from_slice(&riff_chunk(b"MAPT", &mapt));
    data.extend_from_slice(&riff_chunk(b"MAPH", &vec![1u8; n]));

    // STNN: estación con nombre + un waypoint que debe ignorarse.
    let mut st = Vec::new();
    st.extend_from_slice(&(station_tile as u32).to_be_bytes());
    write_str("Terminal Sur", &mut st);
    st.push(0x01); // FACIL_TRAIN
    let mut wp = Vec::new();
    wp.extend_from_slice(&100u32.to_be_bytes());
    write_str("", &mut wp);
    wp.push(0x80); // waypoint
    data.extend_from_slice(&table_chunk(
        b"STNN",
        &[(6, "xy"), (0x0A | 0x10, "name"), (2, "facilities")],
        &[st, wp],
    ));

    // CITY: una ciudad con nombre custom y otra con nombre generado.
    let mut t1 = Vec::new();
    t1.extend_from_slice(&((10u32 * MAP_W) + 10).to_be_bytes());
    write_str("Bahía Blanca", &mut t1);
    t1.extend_from_slice(&2500u32.to_be_bytes());
    let mut t2 = Vec::new();
    t2.extend_from_slice(&((20u32 * MAP_W) + 20).to_be_bytes());
    write_str("", &mut t2);
    t2.extend_from_slice(&80u32.to_be_bytes());
    data.extend_from_slice(&table_chunk(
        b"CITY",
        &[(6, "xy"), (0x0A | 0x10, "name"), (6, "cache.population")],
        &[t1, t2],
    ));

    // Terminador de stream de chunks.
    data.extend_from_slice(&[0, 0, 0, 0]);
    data
}

fn wrap_ottn(payload: &[u8], version: u16) -> Vec<u8> {
    let mut raw = b"OTTN".to_vec();
    raw.extend_from_slice(&version.to_be_bytes());
    raw.extend_from_slice(&[0, 0]);
    raw.extend_from_slice(payload);
    raw
}

#[test]
fn loads_synthetic_sav_with_map_stations_and_towns() {
    let raw = wrap_ottn(&synthetic_sav_payload(), 300);
    let sav = sav::load(&raw).expect("load .sav");

    assert_eq!(sav.version, 300);
    assert_eq!(sav.map.dimensions(), (MAP_W, MAP_H));
    assert_eq!(
        sav.map.get_kind(TileCoord::new(5, 2)),
        Some(TileKind::Station)
    );
    assert_eq!(sav.map.get_kind(TileCoord::new(6, 2)), Some(TileKind::Rail));
    assert_eq!(sav.extras.station_xy, vec![(5, 2)]);

    assert_eq!(sav.stations.len(), 1, "el waypoint no cuenta");
    assert_eq!(sav.stations[0].pos, TileCoord::new(5, 2));
    assert_eq!(sav.stations[0].name.as_deref(), Some("Terminal Sur"));

    assert_eq!(sav.towns.len(), 2);
    assert_eq!(sav.towns[0].name, "Bahía Blanca");
    assert_eq!(sav.towns[0].population, 2500);
    assert_eq!(sav.towns[1].name, "Ciudad 2");

    let state = GameState::from_sav_game(sav);
    assert_eq!(state.stations.len(), 1);
    assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
    assert_eq!(state.towns.len(), 2);
}

#[test]
fn loads_zlib_compressed_sav() {
    let payload = synthetic_sav_payload();
    let mut enc = flate2_test::compress(&payload);
    let mut raw = b"OTTZ".to_vec();
    raw.extend_from_slice(&310u16.to_be_bytes());
    raw.extend_from_slice(&[0, 0]);
    raw.append(&mut enc);

    let sav = sav::load(&raw).expect("load .sav comprimido");
    assert_eq!(sav.version, 310);
    assert_eq!(sav.map.dimensions(), (MAP_W, MAP_H));
}

#[test]
fn state_with_towns_survives_json_roundtrip() {
    let raw = wrap_ottn(&synthetic_sav_payload(), 300);
    let state = GameState::from_sav_game(sav::load(&raw).expect("load"));
    let json = state.save_json().expect("save_json");
    let restored = GameState::load_json(&json).expect("load_json");
    assert_eq!(restored.towns.len(), 2);
    assert_eq!(restored.stations[0].name.as_deref(), Some("Terminal Sur"));
}

/// zlib helper usando la dependencia de openttdrs-core (flate2 reexportado no existe;
/// el dev-test usa el crate directamente).
mod flate2_test {
    pub(super) fn compress(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(data).expect("zlib write");
        enc.finish().expect("zlib finish")
    }
}
