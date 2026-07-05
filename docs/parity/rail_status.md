# Estado de paridad ferroviaria openttdrs ↔ OpenTTD

Fecha: 2026-07-02 · Fase Rail 0 (auditoría y documentación, sin cambios de
código). Usa los mismos niveles de madurez que `status.md`:

1. **Implementado** — existe código que cubre la funcionalidad.
2. **Probado** — tiene tests unitarios/integración propios.
3. **Validado contra OpenTTD** — golden/test contra tablas, constantes o
   comportamiento del C++ de `OpenTTD/`.
4. **Visualmente parecido** — comparación de videos/capturas sin diferencias
   evidentes.
5. **Realmente equivalente** — traza determinística equivalente tick a tick.
   Hoy ningún subsistema ferroviario alcanza los niveles 3–5.

Supuesto estructural documentado: **el tren de la sim es un vehículo puntual**
(una locomotora lógica, un sprite, una tesela). No hay consist, vagones ni
longitud de tren; toda comparación de geometría de consist queda como «no
verificable todavía» hasta que exista el modelo.

## Infraestructura ferroviaria

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Track bits (6 piezas X/Y/UPPER/LOWER/LEFT/RIGHT) | `command/transport/rail.rs` (`RAIL_TB_*`, autorail, merges, refresco de vecinos) | `rail_map.h:136-150` (`GetTrackBits`), `track_type.h:19-52` | 2 · probado (~20 tests de colocación/merge/cruces) | `command/tests/rail.rs` (`autorail_crossing_*`, `parallel_*`, `set_rail_bits_*`) | Bajo: misma semántica de bits en `m5` |
| Pendientes + fundaciones de vía | `map/rail_slope.rs` (`rail_trackbits_valid_on_slope`), `command/terraform.rs` (autoslope) | `rail_cmd.cpp` (foundations), `slope_func.h` | 3 · validado parcial (`computed_tileh_matches_openrtd_sw`) | tests de `map/rail_slope.rs` | Bajo |
| Señales — colocación y encoding | `rail_signals.rs` (`signal_placement_for_track`, `m2`/`m3`/`m3hi`) | `rail_map.h:287-526`, `signal_type.h` | 2 · probado (encoding compatible con saves OpenTTD) | tests de `rail_signals.rs` (`signal_placement_is_single_bit`, `cycle_signal_side_*`) | Medio: variantes ENTRY/EXIT/COMBO se codifican pero sin semántica propia |
| Señales — bloqueo | `rail_signals.rs` (`rail_block_ahead`, `train_blocked_by_signal`, `update_rail_signal_states`) + `sim_step.rs` | `signal.cpp:280-660` (`UpdateSignalsOnSegment`) | 2 · probado (modelo de bloque simplificado «v1»; ENTRY no bloquea) | `sim_train_waits_until_block_ahead_clears`, `block_ahead_stops_at_next_signal` | Alto: sin presignals reales ni PBS; timing de espera sin medir contra OpenTTD |
| Reservas de camino (PBS) | — no existe | `pbs.cpp` (`TryReserveRailTrack`), señales `Path` | 0 · no implementado (el `m2_hi` de saves se conserva sin lógica) | — | Alto (estructural): documentado en `rail_unknown_features.md` |
| Estaciones rail (plataformas 1..=7, waypoints) | `command/transport/station.rs` (`place_rail_station_area`, `rail_station_layout`), `station.rs` | `station_cmd.cpp:1447` (`CmdBuildRailStation`) | 2 · probado (layout gfx compatible; entrada exige vía adyacente) | `place_rail_station_area_*`, `place_rail_waypoint_*` | Medio |
| Depósitos rail | `command/transport/rail.rs` (`place_rail_depot_dir`, `rail_depot_exit_for_dir`), `depot.rs` | `rail_map.h:171-185`, `rail_cmd.cpp:2975-3064` | 2 · probado (boca + empalme automático) | `rail_depot_beside_x_line_connects_exit_tile`, `train_uses_rail_depot_only` | Medio: sin timing de entrada/salida (frames, 37 ticks) |
| Túneles/puentes rail | `command/transport/bridge.rs` (compartido con road), `map/slope.rs` | `tunnelbridge_cmd.cpp:1959-2087` | 1 · implementado (colocación validada; **0 tests específicos rail**, solo road) | tests solo `PlaceRoadBridge` en `command/tests/bridge.rs` | Medio: sin ocultamiento del tren (`_tunnel_visibility_frame`) ni límite de velocidad de puente |
| Pathfinding rail | `pathfinder.rs` (A* direccional `(tile, in_side)` sobre track bits) | `pathfinder/yapf/yapf_rail.cpp`, `follow_track.hpp` | 2 · probado (A* propio, no YAPF) | `astar_rail_*`, `bfs_rail_*`, `jgr_wormhole_*` | Medio: desempates y penalizaciones difieren de YAPF |
| Ocupación/anticolisión | `rail_signals.rs` (`train_blocked_by_traffic`) | (OpenTTD lo resuelve con reservas + señales) | 2 · probado (tile ocupado, frente a frente, tren parado delante) | `trains_block_head_on_without_signal` | Alto: modelo distinto al de OpenTTD (que usa PBS) |
| Railtypes / electrificación / conversión | — no existe (solo variante visual de señal semáforo/eléctrica) | `rail.h:26-525` (`RailTypeInfo`), `CmdConvertRail` | 0 · no implementado | — | Bajo (impacto principalmente visual y de compatibilidad motor↔vía) |
| Ownership por tile de vía | — (`m1` se fuerza a 0 al construir) | `rail_map.h` (`GetTileOwner`) | 0 · no implementado | — | Bajo |
| Serialización (JSON v10, `.sav`, `.ottdmap`) | `save.rs` (migraciones de cruces), `sav/mod.rs`, `map/binary.rs` | formato de mapa OpenTTD | 2 · probado (roundtrip + carga de saves reales) | `tests/sav_load_rail_saves.rs` (`grinnway_sav_has_rail_network`) | Bajo |

## Trenes

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Consist (loco + vagones, longitud) | — no existe (tren puntual; la importación de `.sav` descarta vagones) | `train.h` (`Next()`, `tcache`), `train_cmd.cpp:110-254` (`ConsistChanged`) | 0 · no implementado | test `decodes_front_vehicles_and_skips_wagons` (`sav/entities.rs`) | Alto (estructural) |
| Velocidad máxima por motor | `engine.rs` (`EngineDef::max_speed`, `speed_kmh` sin ÷2 para trenes) | `rail_vehicles` (engine info) | 2 · probado | tests de `engine.rs` | Bajo |
| Aceleración | `vehicle.rs::update_movement_speed` → **reusa `update_road_speed` con `ROAD_ACCEL_ORIGINAL = 256`**; `power_hp`/`weight_t` existen pero no se usan | `train_cmd.cpp:3080-3090` (`UpdateSpeed` AM_ORIGINAL: `accel·2` / freno `accel·−4`), `:444-452` (`UpdateAcceleration`: `Clamp(power/(weight·4), 1, 255)`) | 1 · implementado con la fórmula de carretera (**divergencia estructural medible**) | test relativo `train_moves_slower_than_bus_on_same_path` | Alto |
| Frenado por curva | — **excluido a propósito** en `set_direction_with_curve_penalty` (`kind != Train`) | `train_cmd.cpp:3147-3152` (`_accel_slowdown`: `small_turn=64` → −25 %, `large_turn=128` → −50 %, aplicado `cur_speed -= x·cur_speed >> 8` en `:3564-3568`) | 0 · no implementado (el test `train_keeps_speed_on_direction_change` fija hoy la divergencia) | test que asserta el comportamiento divergente | Alto |
| Paso sub-tesela (`progress` 0–255) | compartido con carretera (`progress_step_for_speed`, 192/256) | `vehicle_base.h:439-454` | 3 · validado (heredado del golden de carretera) | `tests/golden_roadveh.rs` | Medio: escala absoluta distinta (5 Hz) |
| Posición sub-tile / render | `road_movement.rs::train_straight_subtile` (siempre centro de vía, `TRAIN_TRACK_CENTER = 8`) | `vehicle.cpp:3359-3392` (`_vehicle_subcoord` por enterdir×track) | 2 · probado (`train_uses_center_track_not_road_lanes`) | test de `road_movement.rs` | Medio: sin subcoordenadas exactas por pieza de vía |
| Reversa | `vehicle.rs::apply_immediate_train_turnaround` (instantánea) + comando `turn_around_vehicle` | `train_cmd.cpp` (`ReverseTrainDirection`, con chequeos y coste) | 2 · probado | `train_reverses_immediately_when_next_tile_opposite`, `turn_around_vehicle_reverses_train_heading` | Medio |
| Entrada/salida de estación | `station.rs::rail_station_approach_tile` — **el tren para en la vía de acceso, no entra a la plataforma** | `train_cmd.cpp:266-305` (`GetTrainStopLocation`), `station_cmd.cpp:3846-3881` (frenado sub-tile `(stop-x)*20-15`) | 1 · implementado distinto (**divergencia confirmada**; el test showcase `showcase_train_stays_on_rail_not_station_platform` asserta hoy el comportamiento divergente) | tests de `station.rs` y showcase | Alto |
| Carga/descarga | `sim_step.rs` (instantánea, con ventana de carga de 1 tick) | `economy.cpp:1609` (`LoadUnloadVehicle`, gradual) | 2 · probado (misma divergencia `instant_loading` que carretera) | `train_loads_freight_from_rail_station_waiting_cargo` | Alto |
| Entrada/salida de depósito | `refit.rs::vehicle_in_depot` + orden `Depot`; salida inmediata | `train_cmd.cpp:2354-2427` (`CheckTrainStayInDepot`, espera ~37 ticks), `rail_cmd.cpp:2975-2991` (`_fractcoords_enter`) | 2 · probado (sin timing OpenTTD) | tests de `command/tests/rail.rs` y showcase | Medio |
| Órdenes (estación/waypoint/depósito/condicionales) | `vehicle.rs` (`VehicleOrder`), waypoint solo trenes | `order_*.cpp` | 2 · probado | `train_order_through_waypoint_advances_without_full_stop` | Medio |
| Señales en movimiento / forzar paso | `sim_step.rs` (bloqueado → `cur_speed = 0`), `force_proceed` | `train_cmd.cpp:3454-3456` (señal roja: `cur_speed=0`, `progress=255`) | 2 · probado | `force_vehicle_proceed_sets_flag_on_train` | Alto (semántica de bloque simplificada) |
| Render/interpolación | `render/vehicles.rs` (capas `TRAIN_*`, extrapolación genérica aplica a trenes) | (no aplica: OpenTTD corre a ~33 Hz sin interpolar) | 2 · probado | `train_layers_differ_from_bus`, `stopped_train_in_rail_depot_is_hidden_from_pick` | Medio |

## Infraestructura de paridad ferroviaria

| Herramienta | Estado |
|---|---|
| Traza por tick para trenes | **Implementada (Fase Rail 1)** — bloque `rail` en `VehicleRecord` (partes, track bits, bloqueos, depósito, plataforma) + eventos `SignalWait*`, `DepotEntry/Exit`, `SignalStateChanged` |
| Escenario headless de tren | **Implementado (Fase Rail 1)** — `train_line` en `parity/scenario.rs` (depósito, L con curva, señal de bloque, 2 estaciones, órdenes A↔B) |
| Comparador con subsistemas rail | **Implementado (Fase Rail 2)** — subsistemas `rail_infrastructure`/`train_motion`/`consist_geometry`/`pathfinding`/`station_entry`/`loading`/`signaling`/`reservation`/`depot`, filtros `--tile`/`--event`, `--subtile-epsilon` (default 0.51) y `--json` |
| Golden de tablas C++ de tren | **Implementado (Fase Rail 3A)** — `extract_train_movement.py` + `train_movement_golden.json` + `golden_rail.rs` (11 tests) |
| Chequeos de divergencia rail en `parity/report.rs` | **No existen** |

## Top 5 divergencias ferroviarias detectadas en la auditoría

1. **Aceleración de tren usa la fórmula de carretera** (`ROAD_ACCEL_ORIGINAL=256`
   fijo) en lugar de `Clamp(power/(weight·4), 1, 255)` con `accel·2` / freno
   `accel·−4` (`train_cmd.cpp:3080-3090`, `:444-452`). `power_hp`/`weight_t` ya
   están en `EngineDef` sin usar.
2. **Sin frenado por curva**: OpenTTD AM_ORIGINAL aplica `_accel_slowdown`
   (−25 % curva corta, −50 % curva larga, `>>8`) en cada giro; la sim lo excluye
   explícitamente para trenes. Análogo exacto de la penalización de curva ya
   corregida para camiones en la Fase 2.
3. **El tren no entra a la plataforma**: para en la vía de acceso
   (`rail_station_approach_tile`); OpenTTD entra, elige punto de parada por
   `GetTrainStopLocation` y frena sub-tile con `(stop-x)*20-15`. Misma familia
   que la divergencia `bay_stop_position` ya corregida en carretera.
4. **Sin consist**: tren puntual sin vagones ni longitud; sin ocupación
   multi-tesela ni geometría de cola. Divergencia estructural, no medible
   hasta decidir el modelo.
5. **Señales sin PBS ni semántica ENTRY/EXIT/COMBO** y **salida de depósito
   instantánea** (OpenTTD espera ~37 ticks y usa frames de entrada/salida).

## Cómo regenerar la evidencia

Hoy la evidencia ferroviaria es solo estática (tests + este documento). Las
trazas y reportes automáticos llegan con las Fases Rail 1–2 (`rail_debugging_plan.md`).

```bash
# Suite completa (incluye los ~70 tests ferroviarios listados arriba)
./scripts/check.sh
```
