# #380 — conservar gastos anuales de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte preserva
historial financiero nativo; no añade una contabilidad anual completa.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

`CompanyProperties::yearly_expenses` de OpenTTD 15.3 es un array de
`3 * 13 = 39` importes `Money`. El descriptor moderno lo expresa como
`SLE_INT64` con longitud y la tabla guarda los tres años por las trece clases
de gasto. El modelo/importador/writer de `openttdrs` no lo exponía, por lo que
una reexportación perdía esos importes y degradaba el gráfico financiero de la
compañía al abrir el resultado en OpenTTD.

## Corrección acotada

`Company.yearly_expenses` conserva las 39 entradas firmadas; JSON anterior se
normaliza a 39 ceros. `SavCompany` decodifica la lista sin perder signo, la
hidratación la copia por `CompanyID` y el writer emite `SLE_INT64 |
SLE_FILE_HAS_LENGTH` con gamma `39`, rellenando sólo entradas ausentes de JSON
histórico con cero. El campo se mantiene separado de `cur_economy` y
`old_economy`, que son trimestres y ya tenían su propia representación.

No se porta aquí la contabilidad: el runtime aún no imputa gastos por categoría
ni hace el rollover de tres años de OpenTTD.

## Regresiones y oracle

El round-trip interno comprueba el tipo `0x17`, las 39 posiciones, valores
negativos, cero y positivos, e hidratación. El fixture rico escribe la serie
`-19000, -18000, …, 19000`; OpenTTD dedicated la carga, la re-guarda y la
regresión externa `openttd_resaved_preserves_requested_company_yearly_expenses`
comprueba los extremos y la posición central.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`, en
`src/saveload/company_sl.cpp`. Artefactos de la corrida final:

- candidato Rust: `3d227eed397bbf32c5dd41a2bf9c00b9b4ae7b7bd4bb9d67d4e241826b39af59`;
- re-guardado por OpenTTD: `d77b41550a3b94a7ca265b3193b5e4abfa03272cc63f960ef6817b78b5dfb591`.

```bash
expenses_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-yearly-expenses.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$expenses_artifacts/company-yearly-expenses.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$expenses_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$expenses_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$expenses_artifacts/company-yearly-expenses.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_YEARLY_EXPENSES=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$expenses_artifacts/artifacts/company-yearly-expenses.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_yearly_expenses -- --exact
```

## Pendiente real

La historia existente ya no se pierde, pero faltan la imputación de gastos por
clase, el rollover anual, los gráficos/UI de categorías y su interacción con
la economía y noticias. El cierre global de compañías y noticias continúa en
#328.
