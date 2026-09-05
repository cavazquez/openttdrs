# #377 — conservar coordenadas de HQ y última construcción de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte preserva
metadata de coordenadas; no implementa edificios de sede ni comandos de
compañía.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El descriptor de OpenTTD 15.3 guarda `PLYR.location_of_HQ` y
`PLYR.last_build_coordinate` como `SLE_UINT32` desde SLV 6. Antes de este
corte, el descriptor canónico de `openttdrs` omitía ambas columnas: al abrir y
volver a exportar un save se descartaba tanto la tesela norte de HQ como la
referencia usada por OpenTTD para la última construcción.

`location_of_HQ` no usa cero para indicar ausencia: el valor nativo es
`INVALID_TILE = 0xffffffff`. `last_build_coordinate`, en cambio, se inicia en
cero. Por eso los dos valores se conservan como índices crudos, no como
`TileCoord` que pudiera reinterpretar el centinela.

## Corrección acotada

`Company` usa `hq_tile` y `last_build_tile`; el primero aplica
`INVALID_COMPANY_HQ_TILE` al deserializar JSON anterior. `SavCompany` decodifica
los dos nombres de tabla, la hidratación los asigna por `CompanyID` y el writer
los emite como `SLE_UINT32` entre `block_preview` e `inaugurated_year`, que es
el orden exacto de `_company_desc` de OpenTTD. El writer sigue usando SLV 358,
posterior al umbral SLV 6.

Esto no convierte una tesela cualquiera en HQ: construir, eliminar, mover,
dibujar y actualizar la sede, además de actualizar la última construcción
desde comandos propios, siguen pendientes en #328.

## Regresiones y oracle

El round-trip interno verifica tipos de ambas columnas, coordenadas no nulas
`1038`/`1300`, hidratación y el caso de jugador nuevo
`location_of_HQ = INVALID_TILE`. El fixture rico de 64×64 usa ambos índices
dentro del mapa. OpenTTD dedicated lo carga, re-guarda y la regresión externa
`openttd_resaved_preserves_requested_company_location_metadata` confirma que
los dos valores sobreviven.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`, en
`src/saveload/company_sl.cpp` y `src/company_base.h`. Artefactos de la corrida
final:

- candidato Rust: `fb1a7a183c5e7a3fa567504e43fa0fd5cf2a23ee5b40d1bf80f1e55e255bb833`;
- re-guardado por OpenTTD: `609cc3dfd15fe749d5de86f8a54f4f6d75286b3eb84e755a513e851bb5c28424`.

```bash
location_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-location.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$location_artifacts/company-location.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$location_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$location_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$location_artifacts/company-location.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_LOCATION=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$location_artifacts/artifacts/company-location.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_location_metadata -- --exact
```

## Pendiente real

La metadata ya no se pierde, pero la equivalencia funcional de HQ y el ciclo
de vida de empresas no se ha alcanzado. Continúan pendientes las operaciones
de HQ, bancarrota, gastos, límites, flags y consumidores de noticias en #328.
