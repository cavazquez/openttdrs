# #379 — conservar cupos de paisajismo de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte conserva
créditos serializados de compañía; no porta el limitador de comandos.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

OpenTTD 15.3 serializa los siguientes créditos por compañía:

- `PLYR.terraform_limit`: `SLE_UINT32` desde SLV 156;
- `PLYR.clear_limit`: `SLE_UINT32` desde SLV 156;
- `PLYR.tree_limit`: `SLE_UINT32` desde SLV 175.

Los valores son contadores fijos 16.16. El modelo y writer de `openttdrs` los
omitían, por lo que una reexportación descartaba el crédito restante de cada
clase de paisajismo.

## Corrección acotada

`Company` conserva los tres `u32`; JSON anterior y compañías recién creadas
usan `4096 << 16`, el burst inicial de los settings nativos por defecto.
`SavCompany` los decodifica, la hidratación los conserva por `CompanyID` y
`PLYR` los reemite como `SLE_UINT32` antes de `settings`.

No se afirma paridad runtime: `UpdateLandscapingLimits` de OpenTTD añade
crédito cada tick y los comandos lo descuentan. El core aún no aplica ninguno
de esos dos efectos ni adapta el valor si cambian las tasas/ráfagas de
settings.

## Regresiones y oracle

El round-trip interno comprueba los tres tipos, importación e hidratación. El
fixture rico usa el máximo del burst por defecto, `4096 << 16 = 0x10000000`,
para los tres campos. Es deliberado: `UpdateLandscapingLimits` ejecuta durante
el smoke dedicado y conserva un contador que ya está saturado. Por eso el hash
re-guardado coincide con el de un estado nativo por defecto; la regresión
externa lee las tres columnas para distinguirlo de una omisión accidental.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`, en
`src/saveload/company_sl.cpp`, `src/company_cmd.cpp` y
`src/table/settings/world_settings.ini`. Artefactos de la corrida final:

- candidato Rust: `1647d20a16b100f47238c7bcd7cabbc846730ad60286236e9e7d8c7298b78a95`;
- re-guardado por OpenTTD: `05795f768cdf46fce6e49ee1cd2b1b21345f035a778b16780f8f88fe5071e687`.

```bash
landscaping_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-landscaping.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$landscaping_artifacts/company-landscaping.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$landscaping_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$landscaping_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$landscaping_artifacts/company-landscaping.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_LANDSCAPING=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$landscaping_artifacts/artifacts/company-landscaping.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_landscaping_limits -- --exact
```

## Pendiente real

La reexportación ya no pierde el crédito, pero aún faltan el límite por
comando, el descuento exacto, la recarga por tick y los settings de tasa/burst
asociados. Esas reglas, junto con los otros datos de ciclo de vida de compañía,
permanecen abiertas en #328.
