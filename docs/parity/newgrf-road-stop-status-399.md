# Paridad NewGRF: estado de RoadStop (issue #399)

Actualizado: 2026-09-06.

OpenTTD expone el estado de cada entrada de `RoadStop` mediante las variables
de estación `0xF2` (truck stop) y `0xF3` (bus stop). El valor es el byte base
de `RoadStop::status`:

| Bit | Nombre upstream | Significado |
| --- | --- | --- |
| 0 | `Bay0Free` | la primera bahía está libre |
| 1 | `Bay1Free` | la segunda bahía está libre |
| 6 | `BaseEntry` | la parada es drive-through |
| 7 | `EntryBusy` | una entrada está ocupada |

## Implementación

`Station.road_stop_status` conserva el byte en el JSON propio, con valor
compatible para saves anteriores (ambas bahías libres). `StationScope` sólo
devuelve el byte cuando el contexto y el tipo de parada coinciden: `0xF2` para
truck y `0xF3` para bus; rail, docks, aeropuertos y el contexto cruzado
devuelven cero, igual que OpenTTD.

El paso de simulación reconstruye el byte en los mismos límites de tick a
partir de la geometría de la parada y de los vehículos primarios. Las paradas
bay parten con las dos bahías libres; una unidad vial en estado de bahía
consume la bahía correspondiente y marca `EntryBusy`. Las paradas
drive-through añaden `BaseEntry`; los vehículos de otras categorías o las
unidades secundarias no alteran el estado.

La reconstrucción es deliberada: OpenTTD no serializa un estado de ocupación
transitorio en `STNN`; lo deriva de su pool `RoadStop` al cargar y durante el
tick. El byte JSON sólo evita perder el valor observado entre snapshots
propios antes de que el siguiente tick lo recalcule.

## Regresiones y límites

- `station_action2::station_road_stop_status_vars_follow_native_stop_kind`
  comprueba F2/F3 para truck, bus y rail.
- `sim_step::road_stop_status_replays_base_and_bay_occupancy` comprueba las
  dos bahías, `EntryBusy` y una parada drive-through.
- Se mantienen fuera de este issue los pools separados de `RoadStop`, las
  colas físicas completas de drive-through, las animaciones CB140--142 y los
  scopes de estación que no consultan el estado de una parada vial.

Gates ejecutados para cerrar #399: `cargo test -p openttdrs-core`, ambos
`clippy` estrictos, formato, checker de documentación y `git diff --check`.
