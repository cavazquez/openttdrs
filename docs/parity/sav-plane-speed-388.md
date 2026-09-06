# PATS `vehicle.plane_speed` (#388)

Última actualización: 2026-09-06.

## Alcance

`PATS.vehicle.plane_speed` se importa y exporta como `SLE_UINT8`, con default
nativo `4` y rango `1..=4`. El valor vive en `ConstructionSettings` para que
JSON, SAV y simulación compartan la misma preferencia. El movimiento lineal
de aeronaves divide la distancia sub-tile por el ajuste y el motor FTA de
aeropuertos escala su avance de nodos conservando la cadencia histórica para
el default `4`. Las APIs históricas que no reciben `GameState` usan ese mismo
default.

## Oracle OpenTTD 15.3

`reference/openttd-15.3-oracle/src/table/settings/game_settings.ini:324-335`
declara el campo como `SLE_UINT8`, `SLV_90`, default `4`, mínimo `1` y máximo
`4`. `reference/openttd-15.3-oracle/src/aircraft_cmd.cpp:656-699` multiplica
los límites de taxi/despegue, actualiza la velocidad y divide la distancia
recorrida por el divisor. `src/newgrf/newgrf_actd.cpp:70-80` expone el factor a
callbacks NewGRF.

Las regresiones cubren el default, clamp `1..4`, tipo/posición del campo PATS,
mutación tras importar, round-trip binario y la escala determinista de
distancia `1/2/3/4`. El test opcional
`openttd_resaved_preserves_requested_plane_speed` verifica que OpenTTD 15.3
conserve `plane_speed = 2` al re-guardar el fixture rico. La candidata del
smoke tuvo SHA-256
`70207a79e0170f7f3103676fdaa0351a902c0aaea4707d46935e081f98da7a35` (63044
bytes) y OpenTTD produjo un SAV de 8480 bytes con SHA-256
`2cb2f552cec756d82c685ef684a2c45d216b13d8ed7b2154d284dc370b5ac2e5`.

El alcance no afirma todavía que todas las fórmulas de aceleración de
aeronaves, los límites de velocidad de cada nodo FTA, el callback NewGRF
`0x10` ni la API de scripts hayan alcanzado equivalencia bit a bit. Esos
residuales permanecen en #328; aquí sí se evita que el ajuste persistido sea
ignorado por el movimiento del `GameState`.
