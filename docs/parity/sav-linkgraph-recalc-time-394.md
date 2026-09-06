# #394 — latencia nativa de `linkgraph.recalc_time`

Actualizado: **2026-09-06**. Sub-issue de [#328][parent].

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El corte #383 ya conservaba los diez valores `PATS.linkgraph.*` y usaba
`recalc_interval` para decidir las marcas diarias. Sin embargo, el scheduler
Rust ejecutaba el pipeline completo en el tick de spawn y lo repetía en la
marca fija de mitad de intervalo. `recalc_time` quedaba como un byte
reexportable, pero no afectaba cuándo aparecían los nuevos `FlowStat`.

En OpenTTD 15.3, `LinkGraphJob` copia el grafo al hacer spawn y calcula
`join_date = date + recalc_time / EconomyTime::SECONDS_PER_DAY`. La marca de
mitad de `recalc_interval` sólo intenta unir el primer job si esa fecha ya
venció; un job largo puede esperar varias marcas.

## Corrección acotada

`SimulationRuntime` contiene ahora una cola efímera de jobs pendientes. En
`offset == 0`, el core clona estaciones, grafo, catálogo y ajustes y guarda los
jobs junto con su `join_date`. En `offset == interval / 2`, integra como máximo
la cabeza vencida; mientras tanto conserva los flows anteriores. La división
de segundos a días mantiene el entero nativo de OpenTTD, incluido un
`recalc_time` menor que un día.

El estado de esa cola no se serializa en `GameState`/JSON. OpenTTD sí guarda
sus jobs y listas de schedule en `LGRJ`/`LGRS`; la rehidratación ejecutable de
esos snapshots se implementa en el corte SAV
[`sav-linkgraph-jobs-395.md`](sav-linkgraph-jobs-395.md). `rebuild_station_flows`
continúa siendo una reconstrucción inmediata para cargas/comandos que deben
invalidar flows en el mismo tick.

## Regresiones

- `linkgraph_spawn_keeps_snapshot_until_recalc_time_join_date` comprueba que
  un job con `recalc_time = 9 s` (4 días económicos) no cambia los flows en la
  primera marca de día 2 y sí lo hace en la marca posterior al día 4.
- `linkgraph_pending_job_preserves_graph_snapshot` muta el grafo después del
  spawn y verifica que el job conserva sólo la copia inicial.
- `linkgraph_test_job_has_stable_join_pipeline` mantiene un pipeline sintético
  no vacío para detectar regresiones en la integración de shares.

## Oracle y límites

Oracle OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`:

- `src/linkgraph/linkgraphjob.cpp` (`join_date`);
- `src/linkgraph/linkgraphschedule.cpp` (`SpawnNext`, `JoinNext` y la marca
  `SPAWN_JOIN_TICK`);
- `src/linkgraph/linkgraphschedule.h`.

Esto corrige la latencia observable de jobs en la simulación determinista, no
la equivalencia completa de CargoDist. Threads, presupuesto de CPU real,
pausa multiplayer, compresión, merge/eliminación de nodos y la validación de
jobs contra estaciones borradas siguen pendientes en #328.
