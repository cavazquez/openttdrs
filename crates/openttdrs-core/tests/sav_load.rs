//! Pipeline completo `.sav` → `SavGame` → `GameState` con un save sintético
//! (contenedor OTTN/OTTZ + chunks MAPS/MAPT/MAPH/STNN/CITY/INDY/PLYR/VEHS).

#![allow(clippy::expect_used, clippy::cast_possible_truncation)]

use openttdrs_core::{
    GameState, SavVehicleKind, StopKind, TileCoord, TileKind, VehicleKind, VehicleOrder, sav,
};

const CH_RIFF: u8 = 0;
const CH_TABLE: u8 = 3;
const CH_SPARSE_TABLE: u8 = 4;

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

    // MAPT: pradera con una estación en (5,2), una vía en (6,2) y casas para
    // reconstruir la población (OpenTTD no la guarda en el save).
    let mut mapt = vec![0u8; n];
    let station_tile = 2 * MAP_W as usize + 5;
    mapt[station_tile] = 5 << 4; // MP_STATION
    mapt[2 * MAP_W as usize + 6] = 1 << 4; // MP_RAILWAY
    let house_a = 10 * MAP_W as usize + 10; // town 0, HouseID 0 (pop 187)
    let house_b = 10 * MAP_W as usize + 11; // town 0, HouseID 1 (pop 85)
    let house_c = 20 * MAP_W as usize + 20; // town 1, HouseID 3 (pop 5)
    let house_d = 10 * MAP_W as usize + 12; // town 0, en construcción: no suma
    for i in [house_a, house_b, house_c, house_d] {
        mapt[i] = 3 << 4; // MP_HOUSE
    }
    data.extend_from_slice(&riff_chunk(b"MAPT", &mapt));
    data.extend_from_slice(&riff_chunk(b"MAPH", &vec![1u8; n]));

    // M3LO bit 7 = casa completada; M3HI = HouseID (encoding pre-348);
    // MAP2 = TownID u16 big-endian.
    let mut m3lo = vec![0u8; n];
    let mut m3hi = vec![0u8; n];
    let mut map2 = vec![0u8; n * 2];
    for (i, hid, town) in [
        (house_a, 0u8, 0u16),
        (house_b, 1, 0),
        (house_c, 3, 1),
        (house_d, 0, 0),
    ] {
        if i != house_d {
            m3lo[i] = 0x80;
        }
        m3hi[i] = hid;
        map2[i * 2..i * 2 + 2].copy_from_slice(&town.to_be_bytes());
    }
    data.extend_from_slice(&riff_chunk(b"M3LO", &m3lo));
    data.extend_from_slice(&riff_chunk(b"M3HI", &m3hi));
    data.extend_from_slice(&riff_chunk(b"MAP2", &map2));

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

    // CITY: una ciudad con nombre custom y otra con nombre por defecto.
    let mut t1 = Vec::new();
    t1.extend_from_slice(&((10u32 * MAP_W) + 10).to_be_bytes());
    write_str("Bahía Blanca", &mut t1);
    let mut t2 = Vec::new();
    t2.extend_from_slice(&((20u32 * MAP_W) + 20).to_be_bytes());
    write_str("", &mut t2);
    data.extend_from_slice(&table_chunk(
        b"CITY",
        &[(6, "xy"), (0x0A | 0x10, "name")],
        &[t1, t2],
    ));

    // INDY: mina 2×2 en (30,30) (las teselas MP_INDUSTRY no hacen falta para
    // el parser; el cliente las usa para refinar las teselas de render).
    let mut ind = Vec::new();
    ind.extend_from_slice(&((30u32 * MAP_W) + 30).to_be_bytes());
    ind.push(2); // location.w
    ind.push(2); // location.h
    ind.push(0); // type 0 = coal mine
    data.extend_from_slice(&table_chunk(
        b"INDY",
        &[
            (6, "location.tile"),
            (2, "location.w"),
            (2, "location.h"),
            (2, "type"),
        ],
        &[ind],
    ));

    // PLYR: dinero y color de la primera empresa.
    let mut pl = Vec::new();
    pl.extend_from_slice(&777_000i64.to_be_bytes());
    pl.push(6);
    data.extend_from_slice(&table_chunk(b"PLYR", &[(7, "money"), (2, "colour")], &[pl]));

    // DATE: calendario + contador de ticks.
    let mut date_rec = Vec::new();
    date_rec.extend_from_slice(&737_790i32.to_be_bytes());
    date_rec.extend_from_slice(&42_000u64.to_be_bytes());
    data.extend_from_slice(&table_chunk(
        b"DATE",
        &[(5, "date"), (8, "tick_counter")],
        &[date_rec],
    ));

    // ORDL: una orden «ir a estación 0» (Terminal Sur en STNN índice 0).
    {
        let mut header = Vec::new();
        header.push(0x1B);
        write_str("orders", &mut header);
        header.push(0);
        header.push(2);
        write_str("type", &mut header);
        header.push(2);
        write_str("flags", &mut header);
        header.push(4);
        write_str("dest", &mut header);
        header.push(2);
        write_str("refit_cargo", &mut header);
        header.push(4);
        write_str("wait_time", &mut header);
        header.push(4);
        write_str("travel_time", &mut header);
        header.push(4);
        write_str("max_speed", &mut header);
        header.push(0);

        let mut order = Vec::new();
        order.push(1); // OT_GOTO_STATION
        order.push(0);
        order.extend_from_slice(&0u16.to_be_bytes()); // StationID 0
        order.push(0xFF);
        order.extend_from_slice(&0u16.to_be_bytes());
        order.extend_from_slice(&0u16.to_be_bytes());
        order.extend_from_slice(&0u16.to_be_bytes());

        let mut rec = Vec::new();
        rec.push(1); // orders ×1
        rec.extend_from_slice(&order);

        let mut ordl = b"ORDL".to_vec();
        ordl.push(CH_TABLE);
        write_gamma(header.len() as u32 + 1, &mut ordl);
        ordl.extend_from_slice(&header);
        write_gamma(rec.len() as u32 + 1, &mut ordl);
        ordl.extend_from_slice(&rec);
        write_gamma(0, &mut ordl);
        data.extend_from_slice(&ordl);
    }

    // VEHS (sparse): un tren cabeza de convoy sobre la vía de (6,2).
    let mut vehs_header = Vec::new();
    vehs_header.push(2);
    write_str("type", &mut vehs_header);
    vehs_header.push(11 | 0x10);
    write_str("train", &mut vehs_header);
    vehs_header.push(0);
    // Sub-lista de train: struct common con tile/subtype/cargo_type.
    vehs_header.push(11 | 0x10);
    write_str("common", &mut vehs_header);
    vehs_header.push(0);
    vehs_header.push(6);
    write_str("tile", &mut vehs_header);
    vehs_header.push(2);
    write_str("subtype", &mut vehs_header);
    vehs_header.push(2);
    write_str("cargo_type", &mut vehs_header);
    vehs_header.push(6);
    write_str("orders", &mut vehs_header);
    vehs_header.push(2);
    write_str("cur_real_order_index", &mut vehs_header);
    vehs_header.push(2);
    write_str("vehstatus", &mut vehs_header);
    vehs_header.push(0);

    let mut v0 = vec![0u8]; // índice sparse 0
    v0.push(0); // type 0 = tren
    v0.push(1); // train presente
    v0.push(1); // common presente
    v0.extend_from_slice(&((2u32 * MAP_W) + 6).to_be_bytes());
    v0.push(0x01); // GVSF_FRONT
    v0.push(1); // cargo carbón
    v0.extend_from_slice(&1u32.to_be_bytes()); // OrderList ref (índice 0 + 1)
    v0.push(0); // cur_real_order_index
    v0.push(0); // vehstatus: running

    let mut vehs = b"VEHS".to_vec();
    vehs.push(CH_SPARSE_TABLE);
    write_gamma(vehs_header.len() as u32 + 1, &mut vehs);
    vehs.extend_from_slice(&vehs_header);
    write_gamma(v0.len() as u32 + 1, &mut vehs);
    vehs.extend_from_slice(&v0);
    write_gamma(0, &mut vehs);
    data.extend_from_slice(&vehs);

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
    assert_eq!(
        sav.towns[0].population,
        187 + 85,
        "casas completas de la town 0 (la casa en construcción no suma)"
    );
    assert_eq!(sav.towns[1].name, "Ciudad 2");
    assert_eq!(sav.towns[1].population, 5);

    assert_eq!(sav.industries.len(), 1);
    assert_eq!(sav.industries[0].pos, TileCoord::new(30, 30));
    assert_eq!((sav.industries[0].width, sav.industries[0].height), (2, 2));
    assert_eq!(sav.industries[0].industry_type, 0);

    assert_eq!(sav.money, Some(777_000));
    assert_eq!(sav.company_colour, Some(6));
    assert_eq!(sav.game_time.map(|t| t.tick), Some(42_000));

    assert_eq!(sav.vehicles.len(), 1);
    assert_eq!(sav.vehicles[0].kind, SavVehicleKind::Train);
    assert_eq!(sav.vehicles[0].pos, TileCoord::new(6, 2));
    assert_eq!(sav.vehicles[0].orders.len(), 1);
    assert!(sav.vehicles[0].running);

    let state = GameState::from_sav_game(sav);
    assert_eq!(state.stations.len(), 1);
    assert_eq!(state.stations[0].stop_kind, StopKind::RailStation);
    assert_eq!(state.towns.len(), 2);
    assert_eq!(state.economy.money, 777_000);
    assert_eq!(state.company_colour, 6);
    assert_eq!(state.tick.get(), 42_000);
    assert_eq!(state.vehicles.len(), 1);
    assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
    assert_eq!(state.vehicles[0].pos, TileCoord::new(6, 2));
    assert_eq!(state.vehicles[0].orders.len(), 1);
    assert!(matches!(
        state.vehicles[0].orders[0],
        VehicleOrder::Station { .. }
    ));
}

#[test]
fn imported_train_moves_toward_station_order() {
    let raw = wrap_ottn(&synthetic_sav_payload(), 354);
    let mut state = GameState::from_sav_game(sav::load(&raw).expect("load"));
    assert_eq!(state.vehicles.len(), 1);
    assert!(!state.vehicles[0].orders.is_empty());
    let start = state.vehicles[0].pos;
    let progress0 = state.vehicles[0].progress;
    for _ in 0..200 {
        state.step();
    }
    let v = &state.vehicles[0];
    assert!(
        v.pos != start || v.progress != progress0,
        "el tren debería avanzar con órdenes importadas (pos={:?} progress={})",
        v.pos,
        v.progress
    );
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
