# #395 — rehidratación de jobs `LGRJ`/`LGRS`

Actualizado: **2026-09-06**. Sub-issue de [#328][parent].

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El core ya respetaba la latencia de `linkgraph.recalc_time`, pero al abrir un
`.sav` sólo retenía los cuerpos de `LGRJ` y `LGRS` como bytes opacos. La cola
efímera de `SimulationRuntime` empezaba vacía, de modo que un job que
`OpenTTD` había dejado entre `SpawnNext` y `JoinNext` no podía continuar.

El oracle OpenTTD 15.3 confirma que `GetLinkGraphJobDesc()` guarda los diez
ajustes `linkgraph.*`, `join_date`, `link_graph.index` y un snapshot completo
del grafo (`cargo`, nodos y aristas), mientras `GetLinkGraphScheduleDesc()`
guarda las listas `schedule` y `running`.

## Corrección

`sav/linkgraph.rs` decodifica esas columnas conocidas, convierte cada snapshot
a un `cargodist::parity::Job`, valida coordenadas y destinos antes de aceptarlo
y conserva `capacity`, `usage`, tiempo medio y `join_date`. Las referencias
`LGRS.running` ordenan la cola; si `LGRS` no está disponible se usa un orden
estable por `link_graph.index` y slot de pool. Una referencia imposible sólo se
descarta.

`GameState::from_sav_game` instala los jobs agrupados por fecha en la cola
determinista existente. Al llegar a `JoinNext`, los flows se integran y el
passthrough opaco se invalida; antes de esa marca el writer puede reemitir
`LGRJ`/`LGRS` byte a byte, incluidos campos futuros no modelados.

## Regresiones

- `runtime_jobs_decode_snapshot_and_schedule_order` valida settings, cargo,
  nodos, aristas, tiempo medio, `edge_flow` y orden `LGRS`.
- `runtime_jobs_reject_out_of_bounds_node_without_aborting_save` comprueba que
  una coordenada corrupta no aborta toda la carga.
- Las pruebas existentes de passthrough verifican que un save sin mutaciones
  conserva los cuerpos nativos.

## Límites restantes

La cola sigue siendo determinista y síncrona: no se recrean threads, presupuesto
de CPU, pausa multiplayer ni compresión/merge de nodos. `schedule` se conserva
para el contrato SAV y `running` controla la reanudación actual; el planificador
de nuevos graphs sigue siendo el scheduler Rust ya existente. Las mutaciones
del grafo invalidan los chunks runtime para evitar exportar jobs obsoletos.
