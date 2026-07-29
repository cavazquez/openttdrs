//! Assert del subconjunto #226 tras re-guardado por OpenTTD oficial.
//!
//! Activado solo si `OPENTTDRS_ROUNDTRIP_SAV` apunta a un `.sav` (típicamente
//! el output de `scripts/roundtrip_sav_openttd.sh`). Sin env → test ignored.

#![allow(clippy::expect_used)]

use openttdrs_core::{SavVehicleKind, sav};

#[test]
fn openttd_resaved_preserves_declared_subset() {
    let Ok(path) = std::env::var("OPENTTDRS_ROUNDTRIP_SAV") else {
        eprintln!("skip: OPENTTDRS_ROUNDTRIP_SAV no definido");
        return;
    };
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));

    let (w, h) = game.map.dimensions();
    assert!(w >= 64 && h >= 64, "dims mapa {w}x{h}");
    assert!(
        !game.stations.is_empty(),
        "subconjunto: ≥1 estación (hay {})",
        game.stations.len()
    );
    assert!(
        game.vehicles
            .iter()
            .any(|v| v.kind == SavVehicleKind::Train),
        "subconjunto: ≥1 tren"
    );
    assert!(game.money.is_some(), "subconjunto: dinero PLYR");
    assert!(game.game_time.is_some(), "subconjunto: tick/DATE presente");
    // Industria y ROAD son parte del fixture rico; no exigir si el .sav es más pobre.
    if let Ok(strict) = std::env::var("OPENTTDRS_ROUNDTRIP_STRICT")
        && (strict == "1" || strict.eq_ignore_ascii_case("true"))
    {
        assert!(!game.industries.is_empty(), "strict: ≥1 industria");
        assert!(
            game.vehicles
                .iter()
                .any(|v| v.kind == SavVehicleKind::RoadVehicle),
            "strict: ≥1 ROAD vehicle"
        );
    }
}
