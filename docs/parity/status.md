# Estado de paridad openttdrs ↔ OpenTTD

Fecha: 2026-07-01 · Fase 1 del sistema de paridad (trazas + runner headless +
comparador de primera divergencia + golden de tablas C++).

## Niveles de madurez

Cada subsistema se clasifica en cinco niveles acumulativos:

1. **Implementado** — existe código que cubre la funcionalidad.
2. **Probado** — tiene tests unitarios/integración propios.
3. **Validado contra OpenTTD** — hay un golden/test que compara contra tablas,
   constantes o comportamiento del código C++ de `OpenTTD/`.
4. **Visualmente parecido** — la comparación de videos/capturas no muestra
   diferencias evidentes.
5. **Realmente equivalente** — traza determinística equivalente a la de OpenTTD
   (mismo resultado tick a tick en las unidades acordadas). Hoy ningún
   subsistema alcanza este nivel.

## Tabla de estado

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Aceleración carretera (AM_ORIGINAL) | `openttdrs-core/src/engine.rs` (`update_road_speed`) | `src/ground_vehicle.hpp` `DoUpdateSpeed`, `src/roadveh_cmd.cpp:742` | 3 · validado (fórmula exacta portada; test `advance_constants_match_upstream`) | `tests/golden_roadveh.rs` | Medio: falta frenada al llegar a parada |
| Penalización de curva −25 % (AM_ORIGINAL) | `vehicle.rs` (`set_direction_with_curve_penalty`) | `roadveh_cmd.cpp:1481` (`cur_speed -= cur_speed >> 2`; también :1353 y :1426) | 3 · validado (Fase 2; el chequeo `curve_speed_penalty` del reporte quedó como regresión) | test `road_vehicle_loses_quarter_speed_on_turn`; `divergences_found.md` («no observada») | Bajo |
| Paso sub-tesela (`progress` 0–255) | `engine.rs` (`progress_step_for_speed`), `vehicle.rs` (`step`) | `vehicle_base.h:439-454` (`GetAdvanceSpeed`, `GetAdvanceDistance`) | 3 · validado (proporcional a `speed*3/4` con 192/256) | `tests/golden_roadveh.rs` | Alto: escala absoluta distinta (5 Hz, `REFERENCE_PROGRESS_STEP=51`) |
| Tablas de trayectoria sub-tesela (render) | `road_movement.rs` (`STRAIGHT`, `CURVE_*`, `U_TURN_*`) | `src/table/roadveh_movement.h` | 3 · validado (golden compara data_0/2/3 punto a punto) | `tests/golden_roadveh.rs`, fixture `tests/fixtures/parity/roadveh_movement_golden.json` | Bajo en recta/curva; alto en bahías (`_rv_station_*` no portadas) |
| Entrada a playa de carga (bahía) | `station.rs` (`resolve_order_destination` → bahía, `is_connected_bay_road_stop`), `road_movement.rs` (`bay_station_table`, `bay_subtile`) | `roadveh_cmd.cpp:1496-1502`, `roadveh_movement.h:458-737` (`_rv_station_left_*`) y `:1087` (`_road_stop_stop_frame`) | 3 · validado (Fase 2: entra a la tesela, recorre el lazo `_rv_station_left_*` exacto, para en el stop frame y carga dentro; golden punto a punto) | `divergences_found.md` (`bay_stop_position` «no observada»); golden `bay_station_tables_match_rust_copies`; tests de `road_movement.rs` | Bajo: queda la dársena `near` sin usar (una sola dársena por bahía en la sim) |
| Carga/descarga | `sim_step.rs` (`load_vehicles`/`unload_vehicles`) | `economy.cpp:1609` (`LoadUnloadVehicle`) | 2 · probado (instantánea; OpenTTD es gradual) | `divergences_found.md` (`instant_loading`) | Alto |
| Órdenes (estación/full load/no unload/depósito/condicionales) | `vehicle.rs` (`VehicleOrder`) | `src/order_*.cpp` | 2 · probado | tests de `vehicle.rs`, `command/` | Medio |
| Pathfinding carretera | `pathfinder.rs` (A*) | `src/pathfinder/yapf` | 2 · probado (A* propio, no YAPF) | tests de `pathfinder.rs` | Medio: desempates pueden diferir |
| Giro de media vuelta en parada (`depart_turn`) | `vehicle.rs`, `road_movement.rs` (`U_TURN_*`) | `roadveh_cmd.cpp:1306-1330`, tablas 6/7/14/15 | 2 · probado | tests `road_movement.rs` | Medio |
| Interpolación visual entre ticks | `openttdrs-client/src/render/vehicles.rs` + `road_movement.rs` (`extrapolate_vehicle_pose`) | (no aplica: OpenTTD no interpola, corre a 33 Hz) | 2 · probado (sprite ahora usa pose extrapolada; traza CSV opt-in) | test `sprite_selection_uses_extrapolated_pose_not_logical_direction`; `OPENTTDRS_RENDER_TRACE` | Medio |
| Velocidad/subspeed persistidos | `vehicle.rs` (`cur_speed`, `subspeed`) | `Vehicle::cur_speed/subspeed` | 3 · validado (misma semántica de truncado a u8) | `engine.rs` tests + golden | Bajo |
| Tick lógico | `tick.rs` + cliente `simulation.rs` (5 Hz) | `timer_game_tick.h:77` (74 ticks/día) | 1 · implementado distinto a propósito (decisión documentada en `tick_rate_decision.md`) | `divergences_found.md` (`tick_rate`) | Alto (afecta toda comparación absoluta) |

## Infraestructura de paridad (Fase 1)

| Herramienta | Ruta | Estado |
|---|---|---|
| Traza por tick (JSONL) | `openttdrs-core/src/parity/` (`TickRecord`, `ParityTracer`) | Probado (`tests/parity_system.rs`) |
| Runner headless | `openttdrs-core/src/bin/parity_runner.rs` (`--scenario truck_bay --ticks N --out f.jsonl [--divergence-report f.md]`) | Probado |
| Comparador primera divergencia | `openttdrs-core/src/bin/parity_diff.rs` (`--vehicle`, `--subsystem`, exit code 1 si diverge) | Probado |
| Golden tablas C++ | `scripts/extract_roadveh_movement.py` + `tests/golden_roadveh.rs` | Verde |
| Traza de render | `openttdrs-client/src/render_trace.rs` (`OPENTTDRS_RENDER_TRACE=f.csv`) | Probado |

## Cómo regenerar la evidencia

```bash
# Traza del caso de los videos + reporte de divergencias conocidas
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario truck_bay --ticks 500 --out /tmp/truck_bay.jsonl \
    --divergence-report docs/parity/divergences_found.md

# Comparar dos trazas (0 = idénticas, 1 = divergen)
cargo run -p openttdrs-core --bin parity_diff -- /tmp/a.jsonl /tmp/b.jsonl --vehicle 1

# Regenerar fixture golden desde el C++ (solo lectura de OpenTTD/)
python3 scripts/extract_roadveh_movement.py
```
