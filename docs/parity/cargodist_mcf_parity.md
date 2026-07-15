# CargoDist / MCF — paridad nivel 2 con OpenTTD

**Estado:** implementado en `openttdrs-core::linkgraph_parity`  
**MVP previo:** #49 (Manual + stub `CapacityScaled`)  
**Seguimiento:** #102  
**LGRP MVP:** load/save del grafo observado (`sav/linkgraph.rs`) ✅ — `LGRJ`/`LGRS` vacíos.  
**Overlay mapa:** ✅ gizmos al abrir Link Graph o con «Overlay Link Graph» en Opciones de visualización.  
**Dumps C++ byte-igual (MCF):** fixtures JSON en `tests/fixtures/linkgraph/*.json` (`OPENTTD_DUMP_LINKGRAPH=1`).  
**Dumps C++ byte-igual (LGRP wire):** `lgrp_empty.bin` / `lgrp_two_node_goods.bin` (`OPENTTD_DUMP_LGRP=1`).

## Pipeline

1. **Ingesta** (`from_game`): nodos `supply` (waiting) / `demand` (acceptance); aristas `capacity` / `usage` / `travel_time` desde `LinkGraphStats`.
2. **DemandCalculator** — Asymmetric / Symmetric reales (geografía + supply; no espejo de aristas).
3. **MCF1** — Dijkstra `DistanceAnnotation` + `FlowMapper(false)` + eliminación de ciclos.
4. **MCF2** — Dijkstra `CapacityAnnotation` + `FlowMapper(true)` + scale mensual.
5. **GetVia** — `RandomRange` sobre shares con `Randomizer` alineado a OpenTTD (`core/random_func`).

El stub BFS en `mcf.rs` queda **legado** (solo tests de regresión). El camino de juego (`GameState::rebuild_station_flows`) usa el pipeline nuevo.

## Fixtures

`crates/openttdrs-core/tests/fixtures/linkgraph/`:

| Fixture | Escenario |
|---------|-----------|
| `asymmetric_two_node` | 1-hop Asymmetric |
| `symmetric_mirror_nodes` | Symmetric Demand |
| `three_node_linear` | 2-hop |
| `three_node_cycle` | ciclo dirigido |
| `express_vs_local` | express vs local (travel_time) |

Tests: `linkgraph_parity_fixtures` (demands + flows byte-igual; golden GetVia 16 draws + checksum 10k).

### Oráculo C++

Harness Catch2: `OpenTTD/src/tests/linkgraph_parity_fixtures.cpp`.

```bash
cd OpenTTD/build
cmake .. -GNinja -DOPTION_DEDICATED=ON -DCMAKE_BUILD_TYPE=RelWithDebInfo
ninja openttd_test
OPENTTD_DUMP_LINKGRAPH=1 ./openttd_test "[linkgraph][parity]"
# Pegar cada bloque ===DUMP name=== en
# openttdrs/crates/openttdrs-core/tests/fixtures/linkgraph/<name>.json
```

Nota de paridad: `Path::GetCapacityRatio` en OpenTTD hace `(int * 16) / uint`; con `free < 0` el cociente se promociona a unsigned y el ratio queda enorme positivo. MCF2 usa eso al sobrecargar aristas (`express_vs_local`: via express 60 / local 40).

### Oráculo LGRP (bytes del chunk)

Harness Catch2: `OpenTTD/src/tests/lgrp_byte_fixtures.cpp` (serializa el grafo en memoria con el layout de `GetLinkGraphDesc`).

```bash
cd OpenTTD/build
OPENTTD_DUMP_LGRP=1 ./openttd_test "[linkgraph][lgrp]"
# Guardar hex → tests/fixtures/linkgraph/lgrp_*.bin
```

Asserts en `sav/linkgraph.rs` (`lgrp_*_matches_openttd_dump`). `LGRJ`/`LGRS` quedan fuera del golden (chunks aparte; Rust los emite vacíos).

## Manual

`DistributionType::Manual` sigue resolviendo `next_hop` solo desde órdenes (sin MCF).
