# CB157: motivos de rechazo al construir objetos

Actualizado: 2026-09-06 · issue [#440](https://github.com/cavazquez/openttdrs/issues/440)

## Alcance

`CBID_OBJECT_LAND_SLOPE_CHECK` (`0x157`) ya no se reduce a un `bool` en la
ruta de construcción. El core conserva el resultado de ubicación que devuelve
el runtime `NewGRF` y el cliente lo consume después de un rechazo de
`BuildObject`.

La semántica sigue `GetErrorMessageFromLocationCallbackResult` de OpenTTD
15.3:

- `CALLBACK_FAILED` y `0x400` permiten la ubicación.
- En GRF anteriores a la versión 8 se invierte el bit 10 antes de clasificar.
- `0..0x3FF` se convierten en `GRF_STRING_GENERIC_BASE + resultado`.
- `0x40F` toma el `StringID` del registro `0x100`; si falta, conserva el código
  genérico.
- Los demás resultados se conservan como `GenericDenied(código)`.

El callback map-aware mantiene el scope de objeto y pueblo usado previamente:
los registros `7C` del pueblo se escriben incluso cuando la tesela es
rechazada. El preflight sigue trabajando sobre una copia de los pueblos; por
eso una consulta o una falta de fondos no deja cambios persistentes. El
diagnóstico sí se guarda de forma efímera en `SimulationRuntime` sólo para el
comando actual y se limpia al iniciar el siguiente comando.

## Feedback y pruebas

La UI consume el diagnóstico una sola vez, resuelve `LocalString` y
`GrfString` contra el catálogo expandido y locale activo, y traduce los códigos
estándar de clima/agua. Si el catálogo no contiene el texto, conserva el
mensaje genérico de callback. Hay regresiones para la inversión GRF<8,
`regs100[0]`/`0x40F`, texto local, atomicidad de `BuildObject`, writeback PSA y
consumo del diagnóstico en el HUD.

Referencias upstream: `src/object_cmd.cpp` (preflight de cada tesela) y
`src/newgrf_commons.cpp` (`GetErrorMessageFromLocationCallbackResult`).
