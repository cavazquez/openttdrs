//! Assert del subconjunto #226 tras re-guardado por OpenTTD oficial.
//!
//! Activado solo si `OPENTTDRS_ROUNDTRIP_SAV` apunta a un `.sav` (típicamente
//! el output de `scripts/roundtrip_sav_openttd.sh`). Sin env → test ignored.

#![allow(clippy::expect_used)]

use openttdrs_core::{
    COMPANY_LIVERY_FLAG_PRIMARY, COMPANY_LIVERY_FLAG_SECONDARY, CompanyLivery, SavVehicleKind, sav,
};

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

/// Contrato opcional usado por el smoke de `PLYR.liveries`: la entrada de
/// prueba se exporta como bus de dos colores, `OpenTTD` la re-guarda y el
/// importador debe verla sin degradarla.
#[test]
fn openttd_resaved_preserves_requested_company_livery() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_LIVERY").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke de libreas");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let livery = game
        .companies
        .first()
        .and_then(|company| company.liveries.get(14))
        .copied();
    assert_eq!(
        livery,
        Some(CompanyLivery {
            in_use: COMPANY_LIVERY_FLAG_PRIMARY | COMPANY_LIVERY_FLAG_SECONDARY,
            colour1: 7,
            colour2: 11,
        })
    );
}

/// Contrato opcional para la identidad visual nativa de compañía. El fixture
/// rico escribe una presidenta y un rostro válido; OpenTTD debe poder
/// re-guardarlos sin randomizarlos ni perder sus bits.
#[test]
fn openttd_resaved_preserves_requested_company_manager_identity() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_MANAGER_IDENTITY").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke de identidad");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let company = game
        .companies
        .first()
        .expect("PLYR de compañía activa tras re-guardado OpenTTD");
    assert_eq!(company.president_name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(company.manager_face, Some(1 << 7));
    assert_eq!(company.manager_face_style.as_deref(), Some("modern"));
}

/// Contrato opcional para `PLYR.cur_economy`/`old_economy`. El fixture mínimo
/// de writer contiene un trimestre abierto y uno cerrado; `OpenTTD` debe
/// re-guardarlos sin perder importe, desglose de carga ni orden de historial.
#[test]
fn openttd_resaved_preserves_requested_company_quarterly_history() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_QUARTERLY_HISTORY").as_deref() != Ok("1")
    {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke de historial");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let company = game
        .companies
        .first()
        .expect("PLYR de compañía activa tras re-guardado OpenTTD");
    let current = company.cur_economy.as_ref().expect("cur_economy presente");
    assert_eq!(current.income, 900);
    assert_eq!(current.expenses, -400);
    assert_eq!(&current.delivered_cargo[..2], &[3, 4]);
    assert_eq!(company.old_economy.len(), 1);
    assert_eq!(company.old_economy[0].income, 1_200);
    assert_eq!(company.old_economy[0].performance_history, 456);
    assert_eq!(&company.old_economy[0].delivered_cargo[..2], &[4, 5]);
}
