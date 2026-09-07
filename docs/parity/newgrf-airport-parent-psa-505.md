# Writeback del PSA padre de `AirportTile` NewGRF (#505)

Actualizado el 2026-09-07.

## Divergencia corregida

El contexto runtime de `AirportTile` ya leía `AirportScope 7C[param]` desde el
PSA de la estación y el evaluador Action2 ya encaminaba `\\2psto` de un grupo
parent a `parent_persistent_registers`. Al terminar `CB152`/`CB153`/`CB154`,
sin embargo, el writeback genérico copiaba sólo `persistent_registers`, que
pertenece al scope propio de la tesela. La escritura parent se perdía antes del
siguiente callback.

## Oráculo nativo

`AirportTileResolverObject` usa `AirportTileScopeResolver` como scope propio y
`AirportScopeResolver` como parent. El primero no implementa `StorePSA`; la
implementación base no hace nada. En cambio,
`src/newgrf_airport.cpp::AirportScopeResolver::StorePSA` escribe sobre
`st->airport.psa`, crea esa storage con el GRFID/feature de aeropuerto sólo en
la primera escritura no nula y conserva luego las escrituras de cero.

## Contrato implementado

- `writeback_airport_tile_parent_persistent_registers` persiste exclusivamente
  `parent_persistent_registers` después de un callback runtime de AirportTile.
  Ya no confunde el scope sin PSA propio de la tesela con el PSA de su
  aeropuerto padre.
- Si no había PSA ni registro previo y todos los valores son cero, no se
  materializa el mapa disperso ni una futura fila `PSAC`; si el storage ya
  existía, una escritura de cero se conserva.
- `resolve_airport_animation_callback` usa este writeback en las rutas de
  eventos y scheduler, que cubren `CB152`, `CB153` y `CB154`. El renderer sigue
  siendo de sólo lectura y no muta PSA durante una evaluación visual.

Las regresiones
`airport_parent_psto_persists_from_event_to_scheduler` y
`airport_tile_parent_writeback_keeps_native_zero_allocation_rule` prueban,
respectivamente, una cadena Action2 real `CB152 → \\2psto(parent 7C) → CB153`
y la regla nativa de asignación perezosa.

## Límites que continúan abiertos

Esto no completa la delegación restante de `Station::GetNewGRFVariable`, la
FTA propia, paletas, foundations/rotaciones ni sonidos de aeropuerto. Esos
límites continúan abiertos en #326 y #329.
