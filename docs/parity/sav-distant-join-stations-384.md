# PATS `station.distant_join_stations` (#384)

Última actualización: 2026-09-05.

## Alcance cerrado

El bool nativo `PATS.station.distant_join_stations` se importa, hidrata en
`ConstructionSettings`, se conserva en JSON y vuelve a escribirse como
`SLE_BOOL`. Su default para partidas y tablas que no contienen el campo es
`true`, igual que OpenTTD desde `SLV_106`.

El comando reducido `Command::JoinStations` consulta el mismo valor: una unión
no adyacente entre paradas road compatibles o estaciones rail compatibles sólo
se permite si el setting está activo. Las restricciones ya modeladas —owner,
tipo de parada y eje rail— siguen siendo obligatorias; activar el setting no
relaja esos contratos.

## Oracle OpenTTD 15.3

`reference/openttd-15.3-oracle/src/table/settings/game_settings.ini` declara
el campo como `SDT_BOOL`, con `from = SLV_106` y `def = true` (líneas 135–141).
Los comandos nativos de estación verifican una unión distante contra ese bool:

- rail: `src/station_cmd.cpp:1465–1469`;
- road stop: `src/station_cmd.cpp:2075–2101`;
- aeropuerto: `src/station_cmd.cpp:2629–2633`;
- muelle: `src/station_cmd.cpp:2894–2898`.

La candidata del writer, con el valor no-default `false`, tuvo SHA-256
`fb4d9787c019caaa6e23e371acdd99067101b5241ead51bfe526f1b2b597495e`.
El dedicated de OpenTTD 15.3 la cargó y re-guardó; el resultado tuvo SHA-256
`3f8b58e2beb154f5d4c5a990f711fb286e34285c7ea26f18a2d9164f7a948030`.
El importador volvió a obtener `false` del SAV re-guardado.

## Regresiones

- Parser y default nativo de una tabla PATS ausente/presente.
- Wire PATS y round-trip `SavGame`/`GameState`, incluido un cambio posterior a
  la importación que invalida el passthrough de PATS.
- `JoinStations` rechaza paradas no adyacentes con el setting desactivado y
  las fusiona con el setting activado.
- `sav_openttd_roundtrip_subset` verifica el valor no-default tras el
  re-guardado del dedicated.

## Límite real

Esto no implementa aún todos los comandos de construcción de estación de
OpenTTD, sus áreas multi-tile, station spread, selector UI ni sus variantes de
aeropuerto/muelle. Es la semántica del setting en el comando propio existente,
no una declaración de paridad global de estaciones ni de SAV. El padre #328
permanece abierto por pools, schemas y runtime restantes.
