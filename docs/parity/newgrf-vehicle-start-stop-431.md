# CB31 y el motivo textual de start/stop

## Estado

Implementado el 2026-09-06 en el issue
[#431](https://github.com/cavazquez/openttdrs/issues/431). El runtime conserva
la clasificación que OpenTTD obtiene antes de construir el error de comando;
la capa de presentación final sigue siendo un trabajo separado.

## Contrato upstream

`CBID_VEHICLE_START_STOP_CHECK` (`0x31`) recibe `param1=0` y `param2=0`.
`CALLBACK_FAILED` permite la acción. En GRF anteriores a la versión 8, el
byte bajo `0xFF` permite y los demás resultados de ocho bits son
`GRFSTR_MISC_GRF_TEXT + resultado`. En GRF v8 o posterior, `0x400` permite,
`0..0x3FF` apunta al mismo rango genérico, `0x40F` usa el `StringID` de
`register 0x100` y el resto conserva el error genérico.

## Implementación y límites

- `classify_vehicle_start_stop_callback` es una función pura que conserva
  `Allow`, `LocalString(0xD000 + resultado)`, `GrfString(regs100[0])` o
  `GenericDenied(resultado)`.
- `resolve_vehicle_start_stop_callback` ejecuta el Action2, captura
  `register 0x100` y hace writeback de `7C` antes de devolver la clasificación.
- `apply_vehicle_start_stop_callback` mantiene la API booleana existente y
  sólo permite la variante `Allow`, por lo que los comandos actuales no
  cambian su contrato.
- Las regresiones cubren GRF v7/v8, `0xFF`, `CALLBACK_FAILED`, `0x400`,
  `0x40F` con/sin registro y un resultado desconocido.

El mensaje localizado, la expansión de códigos de control y la serialización
del motivo en `CommandError` quedan pendientes; ya no se pierde el `StringID`
en el runtime NewGRF.
