# #375 — conservar estado fraccional y bloqueo de preview de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte sólo
amplía la conservación binaria de compañía; no declara que el runtime de
economía o previews sea equivalente a OpenTTD.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El descriptor nativo de OpenTTD 15.3 (`src/saveload/company_sl.cpp`) guarda,
inmediatamente después de `PLYR.colour`, los bytes `money_fraction` y
`block_preview`. El primero es el residuo de operaciones monetarias de
precisión sub-entera; el segundo cuenta los trimestres durante los cuales una
compañía no puede recibir una preview exclusiva de motor.

El parser y el writer de `openttdrs` saltaban ambos nombres. Por ello, una
partida que requiriese reconstruir `PLYR` podía volver a OpenTTD con esos dos
valores convertidos silenciosamente en sus defaults.

## Corrección acotada

`Company` y `SavCompany` retienen ambos `u8`, con default `0` para JSON
anterior. El importador los lee por sus nombres nativos; la hidratación los
mantiene por `CompanyID`; y el writer los emite entre `colour` e `is_ai` con
tipo `SLE_UINT8` (`0x02`), igual que el descriptor del oracle.

El core todavía no reproduce cálculos de saldo fraccional ni la selección
completa de previews exclusivas. Conservar el estado evita dañarlo mientras
esas reglas sigan fuera del runtime.

## Regresiones y oracle

`ottn_roundtrip_preserves_company_pool_money_and_colour` verifica tipos wire,
parser e hidratación para una compañía rival con `money_fraction=197` y
`block_preview=19`. El fixture rico de writer lleva los mismos valores y la
prueba externa opcional
`openttd_resaved_preserves_requested_company_preview_state` los comprueba
después de que OpenTTD dedicado cargue y re-guarde el archivo.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`. Artefactos de la corrida final:

- candidato Rust: `1105eceb83797cfee9d37799025dde3d754b02663102099d1b6b8009291f6654`;
- re-guardado por OpenTTD: `4a3ab38b35d4b129153a67c9390038046c62b1cd21960533494fe862579b1fe3`.

```bash
preview_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-preview.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$preview_artifacts/company-preview.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$preview_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$preview_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$preview_artifacts/company-preview.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_PREVIEW_STATE=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$preview_artifacts/artifacts/company-preview.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_preview_state -- --exact
```

## Pendiente real

Siguen fuera los demás campos de ciclo de vida de compañía (HQ, última
construcción, períodos de bancarrota, gastos anuales y límites de
construcción), además de la ejecución de las reglas que consumen estos bytes.
Los restantes pools/configuración y la compatibilidad SAV general siguen en
#328.
