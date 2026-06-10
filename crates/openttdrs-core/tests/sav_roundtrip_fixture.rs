//! Validación cruzada del parser `.sav`: los planos de un fixture `.ottdmap`
//! real, reempaquetados como chunks RIFF de un save OTTN, deben producir
//! exactamente el mismo `Map` que `Map::from_ottd_binary`.

#![allow(clippy::expect_used, clippy::cast_possible_truncation)]

use openttdrs_core::{Map, TileCoord, sav};

const FIXTURE: &[u8] = include_bytes!("fixtures/v5p12_stxy.ottdmap");

fn riff_chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = name.to_vec();
    let size = payload.len();
    out.push(((size >> 24) as u8) << 4); // CH_RIFF
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

/// Reconstruye los chunks de mapa de un save desde los tiles del fixture.
fn sav_from_map(map: &Map) -> Vec<u8> {
    let (w, h) = map.dimensions();
    let n = (w * h) as usize;

    let mut planes: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"MAPT", Vec::with_capacity(n)),
        (b"MAPH", Vec::with_capacity(n)),
        (b"MAPO", Vec::with_capacity(n)),
        (b"M3LO", Vec::with_capacity(n)),
        (b"M3HI", Vec::with_capacity(n)),
        (b"MAP5", Vec::with_capacity(n)),
        (b"MAPE", Vec::with_capacity(n)),
        (b"MAP7", Vec::with_capacity(n)),
    ];
    let mut map2 = Vec::with_capacity(n * 2);
    let mut map8 = Vec::with_capacity(n * 2);

    for y in 0..h {
        for x in 0..w {
            let t = map
                .get(TileCoord::new(x as i32, y as i32))
                .expect("tile del fixture");
            planes[0].1.push(t.mapt);
            planes[1].1.push(t.height);
            planes[2].1.push(t.m1);
            planes[3].1.push(t.m3);
            planes[4].1.push(t.m3hi);
            planes[5].1.push(t.m5);
            planes[6].1.push(t.m6);
            planes[7].1.push(t.m7);
            map2.push(t.m2);
            map2.push(t.m2_hi);
            map8.extend_from_slice(&t.m8.to_le_bytes());
        }
    }

    let mut data = Vec::new();
    let mut dims = Vec::new();
    dims.extend_from_slice(&w.to_be_bytes());
    dims.extend_from_slice(&h.to_be_bytes());
    data.extend_from_slice(&riff_chunk(*b"MAPS", &dims));
    for (name, plane) in &planes {
        data.extend_from_slice(&riff_chunk(**name, plane));
    }
    data.extend_from_slice(&riff_chunk(*b"MAP2", &map2));
    data.extend_from_slice(&riff_chunk(*b"MAP8", &map8));
    data.extend_from_slice(&[0, 0, 0, 0]);

    let mut raw = b"OTTN".to_vec();
    raw.extend_from_slice(&350u16.to_be_bytes()); // ≥ 348: sin migración de HouseID
    raw.extend_from_slice(&[0, 0]);
    raw.extend_from_slice(&data);
    raw
}

#[test]
fn sav_map_matches_ottdmap_fixture() {
    let expected = Map::from_ottd_binary(FIXTURE).expect("fixture .ottdmap");
    let raw = sav_from_map(&expected);
    let sav = sav::load(&raw).expect("load .sav sintético");

    assert_eq!(sav.map.dimensions(), expected.dimensions());
    let (w, h) = expected.dimensions();
    for y in 0..h {
        for x in 0..w {
            let c = TileCoord::new(x as i32, y as i32);
            assert_eq!(
                sav.map.get(c),
                expected.get(c),
                "tesela ({x},{y}) difiere entre .sav y .ottdmap"
            );
        }
    }

    // El footer STXY derivado debe listar las mismas teselas MP_STATION.
    let (_, expected_extras) = Map::from_ottd_binary_with_extras(FIXTURE).expect("extras");
    assert_eq!(sav.extras.station_xy, expected_extras.station_xy);
}
