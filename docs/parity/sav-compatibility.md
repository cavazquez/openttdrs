# Compatibilidad `.sav` OpenTTD ↔ openttdrs

Estado vigente de compatibilidad del formato `.sav`. Corte: **2026-08-21**,
`main` `29f5d7a0`; referencia: **OpenTTD
15.3**, commit `14ec60f248547d4d062a1160f0fc26d742319888`.

Esta es la única matriz de capacidad para importación y exportación `.sav`.
[`PARIDAD.md`](../PARIDAD.md) sólo resume su madurez; la guía de wire format en
[`PLANIFICACION.md`](../PLANIFICACION.md#export-sav) y el pipeline de mapa en
[`MAPA_Y_FERROCARRIL.md`](../MAPA_Y_FERROCARRIL.md) no deben repetir ni
contradecir esta tabla. Un cambio de capacidad actualiza esta matriz y, si
cambia la prioridad global, la fila resumida de `PARIDAD.md`.

`✅` cubierto en el corte indicado; `🟡` best-effort o subconjunto; `❌` no se
preserva. Importar un dato no implica que el exportador lo escriba.

| Área | Importar `.sav` de OpenTTD | Exportar `.sav` para OpenTTD | Límite y evidencia |
|---|---|---|---|
| Mapa y tiles | ✅ planos MAP*, túneles/puentes y metadatos de mapa | ✅ `MAPS` + planos RIFF | [`sav/build.rs`](../../crates/openttdrs-core/src/sav/build.rs), [`sav/write/map.rs`](../../crates/openttdrs-core/src/sav/write/map.rs) |
| Mundo base | ✅ ciudades, estaciones/waypoints, industrias, fecha y primera compañía best-effort | ✅ `CITY`, `STNN`, `INDY`, `DATE`, `PLYR` básicos | No equivale a estado completo de compañías o de economía |
| Link graph y carga | 🟡 lee `LGRP`, `CAPA` y `CAPY` cuando están presentes | 🟡 escribe `LGRP` observado, `ECMY` y `CAPY` preservado; el runtime aún no crea pagos activos | [`sav/linkgraph.rs`](../../crates/openttdrs-core/src/sav/linkgraph.rs), [`sav/entities.rs`](../../crates/openttdrs-core/src/sav/entities.rs), [`sav/economy.rs`](../../crates/openttdrs-core/src/sav/economy.rs) |
| Tren y consist | 🟡 lee cabezas, vagones y `next`; recompone el consist best-effort | 🟡 escribe `next` y subtipos de cabeza/vagón para cadenas modeladas | Motores, runtime y geometría siguen siendo best-effort; no es equivalencia dinámica completa |
| Road y tranvía | 🟡 road se convierte a bus/camión; no identifica tranvía | 🟡 bus/camión sobre road/depot válido; no tranvía | [`sav/mod.rs`](../../crates/openttdrs-core/src/sav/mod.rs), [`sav/write/vehicles.rs`](../../crates/openttdrs-core/src/sav/write/vehicles.rs) |
| Barcos | 🟡 `VEH_SHIP` se hidrata como `Ship` | 🟡 sólo sobre agua o ship depot | Motor y estado dinámico son best-effort |
| Aviones y helicópteros | 🟡 aeronaves y FTA se hidratan; reconoce helicóptero | 🟡 avión de ala fija + sombra; no rotor de helicóptero ni runtime FTA completo | [`sav/entities.rs`](../../crates/openttdrs-core/src/sav/entities.rs), [`sav/write/vehicles.rs`](../../crates/openttdrs-core/src/sav/write/vehicles.rs) |
| Órdenes | 🟡 estación, waypoint, depósito, condicionales y flags soportados | 🟡 mismo subconjunto, una lista `ORDL` por vehículo | Refit se escribe pero no se restaura al importar; destinos/contextos no soportados se degradan |
| Horarios | 🟡 lee `wait_time`, `travel_time`, límite de velocidad por orden, inicio, tiempo de orden, lateness, muestras derivadas y flags nativos | 🟡 escribe esos campos por orden, `service_interval` y el bitset de `VehicleFlags` (con bits de horario sincronizados) | La espera activa es estado efímero; reparto `timetable_all`, livery y metadatos de órdenes avanzadas siguen reducidos |
| Shared orders | ✅ reconstruye `shared_order_id` agrupando los vehículos por su índice `ORDL` | ✅ reutiliza una única `ORDL` para vehículos que comparten lista | Persisten limitaciones de horarios/órdenes avanzadas, pero la identidad compartida se conserva |
| Grupos y autoreplace | 🟡 lee campos modelados de `GRPS` / `ERNW` | 🟡 escribe grupos, referencias `VEHS.group_id` y reglas de renovación | Livery/cadenas nativas avanzadas y semántica completa de autoreplace siguen reducidas |
| Objetos | 🟡 usa `OBJS` para traducir tipos de objeto del mapa y conserva el chunk nativo | 🟡 reemite `OBJS`/`OBID` como passthrough cuando provienen del save | El runtime no ejecuta todavía todas las specs/callbacks de objetos |
| Ajustes | 🟡 lee el subconjunto ejecutado por el core de `PATS`/`OPTS`: construcción, pathfinding, averías, subsidios, desastres, autoridad, inflación/recesiones y unidades de tiempo | 🟡 escribe ese subconjunto en `PATS` y conserva `ENGN`/`SRND` nativos como passthrough | [`sav/settings.rs`](../../crates/openttdrs-core/src/sav/settings.rs), [`sav/landscape.rs`](../../crates/openttdrs-core/src/sav/landscape.rs) |
| Compañías y noticias | 🟡 dinero/color básico de `PLYR`; no pools completos | 🟡 `PLYR` básico | El historial de noticias no es persistencia nativa de OpenTTD `.sav`; la cola propia completa queda en JSON |
| NewGRF | 🟡 conserva `NGRF`, `ENGN`, `EIDS` y storage/mappings nativos como chunks opacos | 🟡 reemite esos chunks sin interpretar | El runtime todavía no ejecuta todos los callbacks ni valida que los `.grf` estén instalados |

## Qué usar en cada caso

- Elegir `.sav` para intercambio del subconjunto anterior con OpenTTD 15.3.
- Elegir `.json` para reanudar una partida de openttdrs sin perder el estado
  propio, incluidos grupos, shared orders, autoreplace y el estado completo de
  horarios.
- No presentar una carga satisfactoria como equivalencia de round-trip: los
  campos que no se escriben pueden perderse al volver a guardar.

## Validación

El writer tiene pruebas de chunks, vehículos y órdenes en
[`sav/write`](../../crates/openttdrs-core/src/sav/write/). La matriz de release
usa [`validate_sav_openttd_matrix.sh`](../../scripts/validate_sav_openttd_matrix.sh)
contra OpenTTD 15.3 sin `SKIP`; el smoke local y el round-trip se ejecutan con
`./scripts/check.sh openttd-smoke` cuando hay un binario de referencia.

La matriz no garantiza compatibilidad binaria general, multijugador ni
ejecución de NewGRF. Para runtime de NewGRF, usar las matrices de
[Action0/3/5](newgrf-action0-matrix.md) y de
[callbacks](newgrf-callback-matrix.md).
