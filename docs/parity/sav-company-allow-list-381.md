# #381 — conservar `PLYR.allow_list` como struct-list de claves

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte preserva
permisos serializados de compañía; no añade red ni autorización multijugador.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

Desde `SLV_COMPANY_ALLOW_LIST_V2` (341), OpenTTD 15.3 guarda
`CompanyProperties::allow_list` como el struct-list `PLYR.allow_list`; cada
entrada `SlAllowListData::KeyWrapper` contiene el string `key`. Son claves
públicas de clientes autorizados a unirse a la compañía, no nombres de UI.
El modelo/importador/writer de `openttdrs` no declaraba esa estructura, por lo
que un SAV reexportado perdía la lista completa.

## Corrección acotada

`Company.allow_list` y `SavCompany.allow_list` retienen las claves en orden.
El importador decodifica `allow_list[].key`, la hidratación la copia por
`CompanyID` y el writer emite el header anidado `STRUCT | HAS_LENGTH` con el
subcampo `key` `STRING | HAS_LENGTH`, seguido de la longitud gamma y cada
string. Un JSON anterior recibe una lista vacía por `serde(default)`.

El runtime no consume esas claves para admitir, rechazar o expulsar clientes:
esa semántica de `NetworkAuthorizedKeys` y su UI/red queda fuera de este
corte.

## Regresiones y oracle

El round-trip interno verifica el campo `0x1B`, dos claves distintas y la
hidratación de la compañía rival. El fixture rico emite dos claves de prueba;
OpenTTD dedicated las carga y re-guarda, y la regresión externa
`openttd_resaved_preserves_requested_company_allow_list` verifica que ambas
vuelven sin alteración.

Oracle: `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`,
`src/saveload/company_sl.cpp` (`SlAllowListData`) y `src/company_base.h`
(`NetworkAuthorizedKeys`). Artefactos de la corrida final:

- candidato Rust: `358e59dc26500bfadaa7881a74b36ba2bb9ec828e41c7c00df42864964089187`;
- re-guardado por OpenTTD: `54c81e6135c8eb7219bd27eacbe7335d35447efa5d7ddbe30ad2d5e7cab03cfe`.

```bash
allow_list_artifacts=$(mktemp -d /tmp/openttdrs-sav-company-allow-list.XXXXXX)
OPENTTDRS_DUMP_MVP_RICH_SAV="$allow_list_artifacts/company-allow-list.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$allow_list_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$allow_list_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$allow_list_artifacts/company-allow-list.sav"
OPENTTDRS_ROUNDTRIP_REQUIRE_COMPANY_ALLOW_LIST=1 \
  OPENTTDRS_ROUNDTRIP_SAV="$allow_list_artifacts/artifacts/company-allow-list.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_company_allow_list -- --exact
```

## Pendiente real

El formato ya no pierde las claves existentes, pero `openttdrs` no implementa
la autenticación de clientes, comandos de alta/baja, GUI, ni las decisiones de
join de servidor. También siguen abiertos en #328 el takeover, HQ funcional,
contabilidad anual, límites runtime, flags y noticias.
