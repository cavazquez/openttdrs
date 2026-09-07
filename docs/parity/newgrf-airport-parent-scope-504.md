# Variables `F0` y `FA` del `AirportScope` padre (#504)

Actualizado el 2026-09-07.

## Divergencia corregida

El contexto de `AirportTile` ya construía el scope padre para layout (`40`),
badges (`7A`) y PSA (`7C`), pero dejaba sin materializar las dos variables
explícitas de estación que `AirportScopeResolver` atiende antes de delegar el
resto: `F0` (facilities) y `FA` (fecha de construcción relativa). Un grupo
Action2 padre obtenía por ello su valor default aunque la estación fuese un
aeropuerto construido.

## Oráculo nativo

En `src/newgrf_airport.cpp`, `AirportScopeResolver::GetVariable` devuelve:

- `0xF0`: `st->facilities.base()`;
- `0xFA`: `ClampTo<uint16_t>(st->build_date - DAYS_TILL_ORIGINAL_BASE_YEAR)`;
- `0x7C`: el PSA `st->airport.psa`, o cero si todavía no fue creado.

Sólo después delega las demás variables a `Station::GetNewGRFVariable`.

## Contrato implementado

- El parent de `action2_eval_ctx_for_airport_tile...` publica `F0` con el
  `StationFacilities` representable por `StopKind`; para una estación Airport
  es el bit de aeropuerto (`1 << 3`).
- `FA` reutiliza `Station::newgrf_build_date_value`: días desde el año base
  original, saturados al `WORD` nativo.
- `7C` ya copiaba los registros persistentes de la estación al scope padre;
  se conserva esa identidad de PSA, incluida su hidratación desde
  `STNN.normal.airport.psa`/`PSAC`.
- No hace falta otra vía de caché: `runtime_fingerprint` ya mezcla en orden
  determinista `parent_vars`, por lo que los nuevos valores invalidan una
  variante Action2 de renderer cuando cambian.

Las regresiones
`airport_tile_parent_scope_exposes_facilities_and_build_date` y
`built_newgrf_airport_uses_parent_badge_action2_sprite` ejercen respectivamente
la cadena Action2 core (`F0` → `FA`) y el renderer ECS (`F0` → `7A`).

## Límites que continúan abiertos

No se implementó todavía la delegación completa de
`Station::GetNewGRFVariable` desde `AirportScope`; tampoco FTA, paletas,
foundations Action5/rotaciones, sonidos ni la composición raster global. Esos
límites permanecen en #326 y #329.
