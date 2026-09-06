# CB149: motivo de pendiente de estación en el feedback (#437)

Actualizado: 2026-09-06.

La ruta ferroviaria de `PlaceRailStation` y `PlaceRailStationArea` conserva
ahora la semántica de `PerformStationTileSlopeCheck` de OpenTTD 15.3 y el
motivo del primer rechazo antes de mutar el mapa:

- `CALLBACK_FAILED` y `0x400` permiten;
- los resultados `0..0x3FF` se convierten a `GRF_STRING_GENERIC_BASE + result`;
- `0x40F` usa `register 0x100` cuando el callback lo publicó;
- `0x401..0x408` y resultados desconocidos quedan como rechazo genérico;
- GRF menores que la versión 8 invierten el bit 10 antes de clasificar.

El diagnóstico `(GRFID, outcome)` es efímero, se limpia al iniciar cada
comando y se consume desde el HUD. Cuando el catálogo Action4/Action13 tiene
la cadena, el locale activo muestra el texto expandido; si falta, se conserva
`NewGrfCallbackDenied`. El preflight recorre toda la huella antes de escribir
teselas, por lo que una estación multiplaforma no queda parcialmente creada.

La regresión `rail_station_cb149_rejection_keeps_map_unchanged_and_diagnostic`
comprueba rechazo, atomicidad y `StringID`; la prueba de callback conserva los
parámetros `param1`/`param2` y cubre allow, texto local y código estándar. Los
scopes de estación/vecinos y los callbacks de otros tipos siguen siendo
parciales en #329.
