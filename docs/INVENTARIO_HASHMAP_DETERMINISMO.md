# Inventario HashMap / HashSet — determinismo (#115)

Fecha: 2026-07-16. Depende de [#108](https://github.com/cavazquez/openttdrs/issues/108) (`GameState::canonical_hash`).

## Criterio

- **No** migrar todo a `BTreeMap`.
- El hash canónico (#108) **ordena claves** de objetos JSON; el orden de iteración de `HashMap` en estado persistido **no** afecta el fingerprint.
- Estabilizar iteración en simulación solo si un test de repetibilidad falla por orden de visita.
- Estado en `SimulationRuntime` queda **fuera** del hash.

## Hallazgos (core)

| Área | Uso | Persistido | Riesgo actual |
|------|-----|------------|---------------|
| `game_state/runtime.rs` | `HashSet` señales/PBS/news | No (`runtime`) | Bajo — excluido del hash; PBS se reconstruye |
| `vehicle/model.rs` | `newgrf_persistent_regs: HashMap<u8,u32>` | Sí | Bajo — hash ordena claves |
| `cargodist/legacy/flow_stat.rs` | `by_origin` / `by_cargo` / `by_station` | Sí (vía `station_flows` / settings) | Medio si MCF itera y el orden cambia resultados |
| `cargodist/legacy/mcf.rs` | índices y agrupación temporales | No (locales) | Medio — revisar si fallan tests CargoDist |
| `cargodist/legacy/link_graph.rs` | `edges: HashMap` | Parcial (link graph stats) | Medio — mismo criterio MCF |
| `rail_pbs/*` | reservas, A*, sync mapa | Mayormente runtime / locales | Bajo si parity rail ya es determinista |
| `rail_signals/*` | topología / updates | Locales + mapa | Bajo — goldens/parity existentes |
| `pathfinder/*` | caches / A* | Cache en runtime | Bajo — fuera del hash |
| `command/terraform.rs` | heights/dirty locales | No | Nulo |
| `sav/*` | índices al cargar `.sav` | No (pipeline load) | Nulo para sim en curso |
| `train_collision.rs` | `HashSet` doomed | Local | Nulo |

## Verificación 2026-07-16

Tras #108:

- `canonical_hash` tests (truck_bay ×120 ticks, save/load mid-run) **pasan**.
- No se requirió cambiar contenedores a `BTreeMap`.

## Seguimiento

Si aparece flaky determinismo en CargoDist/MCF o PBS:

1. Reproducir con dos mundos + `canonical_hash` / parity trace.
2. Sustituir solo el mapa culpable por iteración ordenada (`BTreeMap` o `sort` antes del fold).
3. Documentar aquí el fix.

Arquitectura red: [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md).
