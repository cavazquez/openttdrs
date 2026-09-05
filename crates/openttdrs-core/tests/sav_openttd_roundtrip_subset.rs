//! Assert del subconjunto #226 tras re-guardado por OpenTTD oficial.
//!
//! Activado solo si `OPENTTDRS_ROUNDTRIP_SAV` apunta a un `.sav` (típicamente
//! el output de `scripts/roundtrip_sav_openttd.sh`). Sin env → test ignored.

#![allow(clippy::expect_used)]

use openttdrs_core::{
    COMPANY_LIVERY_FLAG_PRIMARY, COMPANY_LIVERY_FLAG_SECONDARY, CompanyLivery,
    INDUSTRY_HISTORY_RECORDS, SavVehicleKind, sav,
};

/// #371: ejecutar sobre el SAV que OpenTTD re-guardó después del renombrado
/// producido por `native_company_rename_preserves_other_plyr_fields`.
#[test]
#[ignore = "requiere OPENTTDRS_ROUNDTRIP_SAV re-guardado por OpenTTD dedicado"]
fn openttd_resaved_preserves_renamed_company() {
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV").expect("SAV re-guardado requerido");
    let bytes = std::fs::read(path).expect("leer SAV re-guardado");
    let game = sav::load(&bytes).expect("cargar SAV re-guardado");
    assert_eq!(game.companies.len(), 1);
    assert_eq!(
        game.companies[0].name.as_deref(),
        Some("Transportes del Sur y del Litoral")
    );
}

/// #372: ejecutar sobre el SAV que OpenTTD re-guardó después de añadir un
/// `PersistentStorage` de pueblo y su referencia `CITY.psa_list`.
#[test]
#[ignore = "requiere OPENTTDRS_ROUNDTRIP_SAV re-guardado por OpenTTD dedicado"]
fn openttd_resaved_preserves_added_town_psa_list() {
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV").expect("SAV re-guardado requerido");
    let bytes = std::fs::read(path).expect("leer SAV re-guardado");
    let game = sav::load(&bytes).expect("cargar SAV re-guardado");
    let storage = game
        .persistent_storages
        .iter()
        .find(|storage| storage.grfid == 0xD1CE_BA5E)
        .expect("PSAC nuevo preservado");
    assert_eq!(storage.storage.get(7), Some(&0xCAFE_BABE));
    assert!(
        game.town_persistent_storage_ids
            .values()
            .any(|ids| ids.contains(&storage.storage_id)),
        "algún CITY.psa_list conserva la referencia al PSAC nuevo"
    );
}

/// #373: ejecutar sobre el SAV que OpenTTD re-guardó después de añadir una
/// entrada `CITY.supplied` con su lista interna de historial.
#[test]
#[ignore = "requiere OPENTTDRS_ROUNDTRIP_SAV re-guardado por OpenTTD dedicado"]
fn openttd_resaved_preserves_added_town_supplied_entry() {
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV").expect("SAV re-guardado requerido");
    let bytes = std::fs::read(path).expect("leer SAV re-guardado");
    let game = sav::load(&bytes).expect("cargar SAV re-guardado");
    let supplied = game
        .towns
        .iter()
        .flat_map(|town| &town.supplied_cargo)
        .map(|entry| {
            (
                entry.cargo,
                entry
                    .history
                    .iter()
                    .map(|sample| (sample.production, sample.transported))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        supplied.iter().any(|(_, history)| {
            history.len() == 61 && history.starts_with(&[(123, 45), (678, 90)])
        }),
        "OpenTTD conserva y normaliza los 61 registros de CITY.supplied: {supplied:?}"
    );
}

/// #374: ejecutar sobre el SAV que OpenTTD re-guardó después de normalizar
/// ambos historiales de carga de `INDY` a los 61 registros nativos.
#[test]
#[ignore = "requiere OPENTTDRS_ROUNDTRIP_SAV re-guardado por OpenTTD dedicado"]
fn openttd_resaved_preserves_normalized_indy_histories() {
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV").expect("SAV re-guardado requerido");
    let bytes = std::fs::read(path).expect("leer SAV re-guardado");
    let game = sav::load(&bytes).expect("cargar SAV re-guardado");
    assert!(game.industries.iter().any(|industry| {
        industry.accepted.iter().any(|entry| {
            entry.history.len() == INDUSTRY_HISTORY_RECORDS
                && entry
                    .history
                    .first()
                    .is_some_and(|sample| sample.accepted == 123 && sample.waiting == 0)
        })
    }));
    assert!(game.industries.iter().any(|industry| {
        industry.produced.iter().any(|entry| {
            entry.history.len() == INDUSTRY_HISTORY_RECORDS
                && entry
                    .history
                    .first()
                    .is_some_and(|sample| sample.production == 456 && sample.transported == 78)
        })
    }));
}

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

/// Contrato opcional de estado operativo de compañía. `money_fraction` es el
/// residuo monetario sub-entero y `block_preview` impide previews exclusivas
/// durante una cantidad de trimestres; ambos son bytes nativos de `PLYR`, no
/// valores visuales derivados.
#[test]
fn openttd_resaved_preserves_requested_company_preview_state() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_PREVIEW_STATE").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke PLYR");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let company = game
        .companies
        .first()
        .expect("PLYR de compañía activa tras re-guardado OpenTTD");
    assert_eq!(company.money_fraction, Some(197));
    assert_eq!(company.block_preview, Some(19));
}

/// Contrato opcional de años de inauguración. El año económico y el calendario
/// pueden diferir en el modo wallclock, por lo que el writer debe conservar
/// ambos `SLE_INT32` de `PLYR` de forma independiente.
#[test]
fn openttd_resaved_preserves_requested_company_inauguration_years() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_INAUGURATION").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke PLYR");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let company = game
        .companies
        .first()
        .expect("PLYR de compañía activa tras re-guardado OpenTTD");
    assert_eq!(company.inaugurated_year, Some(1967));
    assert_eq!(company.inaugurated_year_calendar, Some(2067));
}

/// Contrato opcional para el límite de préstamo individual. OpenTTD representa
/// el default con `INT64_MIN`, pero una compañía marcada por deity conserva un
/// valor concreto incluso si cambia el límite global por inflación.
#[test]
fn openttd_resaved_preserves_requested_company_max_loan_override() {
    if std::env::var("OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_MAX_LOAN").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido para el smoke de max_loan");
    let raw = std::fs::read(&path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let game = sav::load(&raw).unwrap_or_else(|e| panic!("import openttdrs: {e}"));
    let company = game
        .companies
        .first()
        .expect("PLYR de compañía activa tras re-guardado OpenTTD");
    assert_eq!(company.max_loan, Some(450_000));
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
