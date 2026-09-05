# #378 — conservar estado pasivo de bancarrota de `PLYR`

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte conserva
el estado serializado de una posible adquisición; no implementa el takeover.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El descriptor de OpenTTD 15.3 guarda, después de
`PLYR.months_of_bankruptcy`, tres campos que el writer canónico omitía:

- `bankrupt_asked`: `SLE_UINT16`, máscara de compañías ya consultadas;
- `bankrupt_timeout`: `SLE_INT16`, espera restante para responder una oferta;
- `bankrupt_value`: `SLE_INT64`, valor fijado para la adquisición.

Al importar y exportar un `.sav`, esos valores se descartaban. No son valores
decorativos: cuando la máscara tiene bits, `Company::Tick` de OpenTTD puede
avanzar la negociación y terminar adquiriendo o eliminando una compañía.

## Corrección acotada

`Company` ahora conserva `bankruptcy_asked`, `bankruptcy_timeout` y
`bankruptcy_value`; `SavCompany` los decodifica por nombre, la hidratación los
asigna por `CompanyID` y el writer los emite en el orden y ancho nativos entre
`months_of_bankruptcy` y `settings`. JSON anterior recibe ceros, igual que el
estado pasivo nativo.

El runtime propio no interpreta el mask, no descuenta el timeout, no calcula
el valor de adquisición y no crea eventos ni transfiere/elimina empresas. Por
eso esta etapa es de persistencia, no de paridad funcional de bancarrota.

## Regresiones y oracle

El round-trip interno afirma los tres tipos wire, importación e hidratación de
una rival. El fixture rico usa `bankrupt_asked = 0`,
`bankrupt_timeout = -17` y `bankrupt_value = 87654321`. La máscara vacía es
deliberada: permite probar los tres campos sin activar
`HandleBankruptcyTakeover` durante los segundos del smoke dedicated.

OpenTTD dedicated carga y re-guarda el fixture; la regresión externa
`openttd_resaved_preserves_requested_company_passive_bankruptcy_state` comprueba
que los mismos valores sobreviven.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`, en
`src/saveload/company_sl.cpp`, `src/company_base.h` y `src/company_cmd.cpp`.
Artefactos de la corrida final:

- candidato Rust: `0aa55305356b8e788a983f16435596d9b3e2786fc4840a7a3d44772fadf41f8e`;
- re-guardado por OpenTTD: `05795f768cdf46fce6e49ee1cd2b1b21345f035a778b16780f8f88fe5071e687`.

```bash
bankruptcy_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-bankruptcy.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$bankruptcy_artifacts/company-bankruptcy.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$bankruptcy_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$bankruptcy_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$bankruptcy_artifacts/company-bankruptcy.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_BANKRUPTCY=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$bankruptcy_artifacts/artifacts/company-bankruptcy.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_passive_bankruptcy_state -- --exact
```

## Pendiente real

El save ya no pierde el estado pasivo, pero el ciclo de bancarrota todavía no
es equivalente: faltan las ofertas, el contador, la valoración, el takeover,
eventos/noticias y los efectos sobre vehículos, estaciones e infraestructura.
Todo eso permanece abierto en #328.
