# Estado de paridad openttdrs ↔ OpenTTD

Fecha: 2026-07-16 · Fase 1 del sistema de paridad (trazas + runner headless +
comparador de primera divergencia + golden de tablas C++). Tick ~37 Hz y carga
gradual alineados con el código (#125); regenerar informes con
`./scripts/regenerate_parity_reports.sh`.

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
| Paso sub-tesela (`progress` 0–255) | `engine/physics.rs` (`progress_step_for_speed`), `vehicle` (`step`) | `vehicle_base.h:439-454` (`GetAdvanceSpeed`, `GetAdvanceDistance`) | 3 · validado (proporcional a `speed*3/4` con 192/256; `REFERENCE_PROGRESS_STEP=112`) | `tests/golden_roadveh.rs` | Bajo–medio: escala alineada a ~37 Hz |
| Tablas de trayectoria sub-tesela (render) | `road_movement.rs` (`STRAIGHT`, `CURVE_*`, `U_TURN_*`) | `src/table/roadveh_movement.h` | 3 · validado (golden compara data_0/2/3 punto a punto) | `tests/golden_roadveh.rs`, fixture `tests/fixtures/parity/roadveh_movement_golden.json` | Bajo en recta/curva; alto en bahías (`_rv_station_*` no portadas) |
| Entrada a playa de carga (bahía) | `station.rs` (`resolve_order_destination` → bahía, `is_connected_bay_road_stop`), `road_movement.rs` (`bay_station_table`, `bay_subtile`) | `roadveh_cmd.cpp:1496-1502`, `roadveh_movement.h:458-737` (`_rv_station_left_*`) y `:1087` (`_road_stop_stop_frame`) | 3 · validado (Fase 2: entra a la tesela, recorre el lazo `_rv_station_left_*` exacto, para en el stop frame y carga dentro; golden punto a punto) | `divergences_found.md` (`bay_stop_position` «no observada»); golden `bay_station_tables_match_rust_copies`; tests de `road_movement.rs` | Bajo: queda la dársena `near` sin usar (una sola dársena por bahía en la sim) |
| Carga/descarga | `sim_step/cargo_transfer.rs` + `load_unload_speed` | `economy.cpp:1609` (`LoadUnloadVehicle`) | 3 · validado (gradual por tick; regresión `instant_loading`) | `divergences_found.md`, `tests/golden_roadveh.rs` | Bajo |
| Órdenes (estación/full load/no unload/depósito/condicionales) | `vehicle.rs` (`VehicleOrder`) | `src/order_*.cpp` | 2 · probado | tests de `vehicle.rs`, `command/` | Medio |
| Pathfinding carretera | `pathfinder.rs` (A*) | `src/pathfinder/yapf` | 2 · probado (A* propio, no YAPF) | tests de `pathfinder.rs` | Medio: desempates pueden diferir |
| Giro de media vuelta en parada (`depart_turn`) | `vehicle.rs`, `road_movement.rs` (`U_TURN_*`) | `roadveh_cmd.cpp:1306-1330`, tablas 6/7/14/15 | 2 · probado | tests `road_movement.rs` | Medio |
| Interpolación visual entre ticks | `openttdrs-client/src/render/vehicles.rs` + `road_movement.rs` (`extrapolate_vehicle_pose`) | (OpenTTD no interpola; sim ~37 Hz) | 2 · probado (sprite usa pose extrapolada; traza CSV opt-in) | test `sprite_selection_uses_extrapolated_pose_not_logical_direction`; `OPENTTDRS_RENDER_TRACE` | Medio |
| Velocidad/subspeed persistidos | `vehicle.rs` (`cur_speed`, `subspeed`) | `Vehicle::cur_speed/subspeed` | 3 · validado (misma semántica de truncado a u8) | `engine` tests + golden | Bajo |
| Tick lógico | `economy/time.rs` + cliente `simulation.rs` (~37 Hz) | `timer_game_tick.h` (74 ticks/día, ~27 ms) | 3 · validado (mismas constantes; ADR 0003) | `tick_rate_decision.md`, `divergences_found.md` (`tick_rate`) | Bajo |

## Infraestructura de paridad (Fase 1)

| Herramienta | Ruta | Estado |
|---|---|---|
| Traza por tick (JSONL) | `openttdrs-core/src/parity/` (`TickRecord`, `ParityTracer`) | Probado (`tests/parity_system.rs`) |
| Runner headless | `openttdrs-core/src/bin/parity_runner.rs` (`--scenario truck_bay|train_line|train_signal`, `--divergence-report`) | Probado |
| Comparador primera divergencia | `openttdrs-core/src/bin/parity_diff.rs` (`--vehicle`, `--subsystem`, exit code 1 si diverge) | Probado |
| Golden tablas C++ | `scripts/extract_roadveh_movement.py` + `tests/golden_roadveh.rs` | Verde |
| Traza de render | `openttdrs-client/src/render_trace.rs` (`OPENTTDRS_RENDER_TRACE=f.csv`) | Probado |

## Cómo regenerar la evidencia

```bash
# Traza del caso de los videos + reporte de divergencias conocidas
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario truck_bay --ticks 500 --out /tmp/truck_bay.jsonl \
    --divergence-report docs/parity/divergences_found.md

# Reporte ferroviario (escenario train_line, 600 ticks)
./scripts/regenerate_parity_reports.sh

# Comparar dos trazas (0 = idénticas, 1 = divergen)
cargo run -p openttdrs-core --bin parity_diff -- /tmp/a.jsonl /tmp/b.jsonl --vehicle 1

# Regenerar fixture golden desde el C++ (solo lectura de OpenTTD/)
python3 scripts/extract_roadveh_movement.py
```

## Paridad ferroviaria (Rail 0–4)

| Documento | Uso |
|-----------|-----|
| [RAIL_REVIEW_HANDOFF.md](RAIL_REVIEW_HANDOFF.md) | Stub → archive; revisión post Rail 4 |
| [rail_debugging_plan.md](rail_debugging_plan.md) | Stub → archive; fases 0–4 hechas |
| [rail_status.md](rail_status.md) | Estado vivo por subsistema |
| [train_line_divergences.md](train_line_divergences.md) | Reporte generado (`regenerate_parity_reports.sh`) |
