# CB149: scope de terreno durante la construcción (#439)

Actualizado: 2026-09-06.

OpenTTD 15.3 conserva la tesela actual en `StationResolverObject` aunque la
estación todavía no exista. Si un Action2 de `CBID_STATION_LAND_SLOPE_CHECK`
consulta `0x67[param]`, `GetNearbyTile` interpreta los nibbles como offsets
firmados, intercambia X/Y para el eje `AXIS_Y` y envuelve en el mapa. El valor
devuelto tiene el formato `0czzbbss`: tipo de tesela, altura, clase de agua y
terreno, y pendiente orientada al eje.

OpenTTDRS ahora materializa esos valores desde el `Map` real antes de cada
callback CB149. La altura usa unidades de píxel para GRF anteriores a la
versión 8 y unidades de tesela para GRF modernos; la clase de agua y el tipo
de tesela se codifican con los mismos bits que `GetNearbyTileInformation`.

El resolver legacy sin mapa se conserva para APIs que sólo empaquetan
`param1`/`param2`. Los comandos de construcción usan la variante map-aware y
mantienen el preflight atómico: una denegación no escribe teselas, estaciones
ni fondos.

Fuentes upstream: `reference/openttd-15.3-oracle/src/newgrf_station.cpp`
(variables `0x67` y `PerformStationTileSlopeCheck`) y
`reference/openttd-15.3-oracle/src/newgrf_commons.cpp`
(`GetNearbyTile`/`GetNearbyTileInformation`).
