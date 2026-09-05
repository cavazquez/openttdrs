# #376 — conservar años de inauguración de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte conserva
metadata de compañía; no implementa el ciclo de vida completo de compañías.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El descriptor de OpenTTD 15.3 define `PLYR.inaugurated_year` como
`SLE_INT32` y, desde `SLV_COMPANY_INAUGURATED_PERIOD_V2 = 349`, guarda además
`PLYR.inaugurated_year_calendar` como otro `SLE_INT32`. El exportador ya usa
SLV 358, pero el modelo, importador y writer de `openttdrs` omitían ambos
nombres. Al reconstruir `PLYR`, se perdían el año económico visible en la
compañía y el año de calendario que OpenTTD muestra en modo wallclock.

## Corrección acotada

`Company` conserva los dos años firmados, con cero como valor nativo no
inicializado para JSON anterior. `SavCompany` los decodifica por nombre, la
hidratación los asigna por `CompanyID` y el writer los emite como `SLE_INT32`
entre `block_preview` e `is_ai`. No se sube `EXPORT_SAVE_VERSION`: 358 ya es
posterior a 349.

## Regresiones y oracle

El round-trip interno comprueba los dos tipos wire, importación e hidratación
de una compañía rival con `1967`/`2067`. El fixture rico del writer incorpora
los mismos años; OpenTTD dedicated lo carga, re-guarda y la regresión externa
`openttd_resaved_preserves_requested_company_inauguration_years` verifica
ambos valores en el archivo re-guardado.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`. Artefactos de la corrida final:

- candidato Rust: `1e9677f8c1987dd8d469d852e8c64a8785e2c8e3541c00c4e9014030c98b1ba6`;
- re-guardado por OpenTTD: `0014844de405b06694196f5290c0cf5eda7d21a4eb529862b3674e353165b0b4`.

```bash
inauguration_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-inauguration.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$inauguration_artifacts/company-inauguration.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$inauguration_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$inauguration_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$inauguration_artifacts/company-inauguration.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_INAUGURATION=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$inauguration_artifacts/artifacts/company-inauguration.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_inauguration_years -- --exact
```

## Pendiente real

La creación runtime de empresas aún no asigna estos años con la misma política
de OpenTTD, y siguen fuera HQ, última construcción, bancarrota, gastos,
límites y flags restantes. El cierre global de interoperabilidad continúa en
#328.
