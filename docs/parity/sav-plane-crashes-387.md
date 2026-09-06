# PATS `vehicle.plane_crashes` (#387)

Última actualización: 2026-09-06.

## Alcance

`PATS.vehicle.plane_crashes` se importa y exporta como `SLE_UINT8`, con
default nativo `2` y valores `0` (ninguno), `1` (reducido) y `2` (normal).
El valor vive en `ConstructionSettings` para que el estado JSON y el SAV
compartan el mismo ajuste. El camino de accidentes FTA del core aplica `0` a
la desactivación y los umbrales nativos a `1/2`; el caso especial
jet+pista corta mantiene su probabilidad fija `3276/2²²` y la excepción
`no_jetcrash`, como en OpenTTD.

## Oracle OpenTTD 15.3

`reference/openttd-15.3-oracle/src/table/settings/game_settings.ini:338-353`
declara el campo, su rango y default. `src/aircraft_cmd.cpp:1390-1402`
calcula la probabilidad general como
`(0x4000 << plane_crashes) / 1500`, mientras conserva el umbral fijo de jet
en pista corta.

Las regresiones cubren default/parser y clamp `0..2`, tipo/posición del campo
PATS, round-trip por mutación, y los límites deterministas de probabilidad.
El smoke dedicado usa el fixture rico con valor `1` y el test opcional
`openttd_resaved_preserves_requested_plane_crashes` verifica que OpenTTD 15.3
lo conserve al re-guardar. La candidata tuvo SHA-256
`1c016e070d8973785d3be9f84c5a50c650867b323bcefed17d76092180ff4b63` y
OpenTTD produjo un SAV de 8480 bytes con SHA-256
`5d9a2286e17a766808bd28413aa1ae92585be7222d5ac3f0f9d95be028468dbb`.

El alcance no afirma que el runtime propio reproduzca todas las rutas de
accidente de OpenTTD (por ejemplo, fallos fuera del FTA implementado) ni la
UI nativa completa de settings. #328 continúa abierto por esos residuales,
además de pools y runtime SAV restantes.
