# Estado de paridad ferroviaria openttdrs ↔ OpenTTD

Fecha: 2026-07-09 · Fases Rail 0–4 + **consist Fase 1**. Usa los mismos niveles de
madurez que `status.md`:

1. **Implementado** — existe código que cubre la funcionalidad.
2. **Probado** — tiene tests unitarios/integración propios.
3. **Validado contra OpenTTD** — golden/test contra tablas, constantes o
   comportamiento del C++ de `OpenTTD/`.
4. **Visualmente parecido** — comparación de videos/capturas sin diferencias
   evidentes.
5. **Realmente equivalente** — traza determinística equivalente tick a tick.
   Hoy ningún subsistema ferroviario alcanza los niveles 4–5.

Modelo de tren: **consist** (`next_unit`/`prev_unit`, longitud en unidades de 8,
ocupación multi-tesela). Geometría de cola y PBS multi-unidad siguen siendo
aproximadas (Fases 2–3 del roadmap estructural).

## Infraestructura ferroviaria

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Track bits (6 piezas X/Y/UPPER/LOWER/LEFT/RIGHT) | `command/transport/rail.rs` (`RAIL_TB_*`, autorail, merges, refresco de vecinos) | `rail_map.h:136-150` (`GetTrackBits`), `track_type.h:19-52` | 2 · probado (~20 tests de colocación/merge/cruces) | `command/tests/rail.rs` (`autorail_crossing_*`, `parallel_*`, `set_rail_bits_*`) | Bajo: misma semántica de bits en `m5` |
| Pendientes + fundaciones de vía | `map/rail_slope.rs` (`rail_trackbits_valid_on_slope`), `command/terraform.rs` (autoslope) | `rail_cmd.cpp` (foundations), `slope_func.h` | 3 · validado parcial (`computed_tileh_matches_openrtd_sw`) | tests de `map/rail_slope.rs` | Bajo |
| Señales — colocación y encoding | `rail_signals.rs` (`signal_placement_for_track`, `m2`/`m3`/`m3hi`) | `rail_map.h:287-526`, `signal_type.h` | 2 · probado (encoding compatible con saves OpenTTD) | tests de `rail_signals.rs` (`signal_placement_is_single_bit`, `cycle_signal_side_*`) | Medio: ENTRY/EXIT/COMBO degradados a BLOCK (Rail 3D) |
| Señales — bloqueo | `rail_signals.rs` (`rail_block_ahead`, `train_blocked_by_signal`, `update_rail_signal_states`) + `sim_step.rs` | `signal.cpp:280-660` (`UpdateSignalsOnSegment`) | 3 · validado (Rail 3D: bloque v1 + escenario `train_signal`) | `sim_train_waits_until_block_ahead_clears`, `train_signal_divergences_are_absent_after_rail_3d`, `signal_wait_events_emitted_with_two_trains` | Alto: sin presignals reales ni PBS; timing sin golden contra OpenTTD |
| Reservas de camino (PBS) | `rail_pbs.rs` (TryReserve, path signals, plataforma, consist) | `pbs.cpp` (`TryReserveRailTrack`), señales `Path` | 2 · probado (Fase 3 MVP) | `consist_tail_blocks_*`, `platform_reservation_*`, tests PBS existentes | Medio: sin golden tick-a-tick vs OpenTTD |
| Estaciones rail (plataformas 1..=7, waypoints) | `command/transport/station.rs` (`place_rail_station_area`, `rail_station_layout`), `station.rs` | `station_cmd.cpp:1447` (`CmdBuildRailStation`) | 2 · probado (layout gfx compatible; entrada exige vía adyacente) | `place_rail_station_area_*`, `place_rail_waypoint_*` | Medio |
| Depósitos rail | `command/transport/rail.rs` (`place_rail_depot_dir`, `rail_depot_exit_for_dir`), `depot.rs` | `rail_map.h:171-185`, `rail_cmd.cpp:2975-3064` | 2 · probado (boca + empalme automático) | `rail_depot_beside_x_line_connects_exit_tile`, `train_uses_rail_depot_only` | Medio: sin timing de entrada/salida (frames, 37 ticks) |
| Túneles/puentes rail | `command/transport/bridge.rs` (compartido con road), `map/slope.rs` | `tunnelbridge_cmd.cpp:1959-2087` | 1 · implementado (colocación validada; **0 tests específicos rail**, solo road) | tests solo `PlaceRoadBridge` en `command/tests/bridge.rs` | Medio: sin ocultamiento del tren (`_tunnel_visibility_frame`) ni límite de velocidad de puente |
| Pathfinding rail | `pathfinder/yapf.rs` (trackdir + señales/reservas) | `pathfinder/yapf/yapf_rail.cpp`, `follow_track.hpp` | 2 · probado (YAPF propio; extensión incremental MVP) | `yapf_*`, `next_rail_trackdir_*`, `extend_rail_path_*` | Medio: desempates y penalizaciones difieren de upstream |
| Ocupación/anticolisión | `rail_signals.rs` (`train_blocked_by_traffic`) | (OpenTTD lo resuelve con reservas + señales) | 2 · probado (tile ocupado, frente a frente, tren parado delante) | `trains_block_head_on_without_signal` | Alto: modelo distinto al de OpenTTD (que usa PBS) |
| Railtypes / electrificación / conversión | `rail_type.rs` + `ConvertRail` (Rail/Electric/Mono/Maglev) | `rail.h:26-525` (`RailTypeInfo`), `CmdConvertRail` | 2 · probado (Fase 5–6 MVP) | `convert_rail_*`, `*_engine_requires_*`, `monorail_path_does_not_cross_*` | Medio: tint MVP; sin catenaria/sprites mono-maglev OpenGFX; tranvía = RoadType |
| Ownership por tile de vía | — (`m1` se fuerza a 0 al construir) | `rail_map.h` (`GetTileOwner`) | 0 · no implementado | — | Bajo |
| Serialización (JSON v10, `.sav`, `.ottdmap`) | `save.rs` (migraciones de cruces), `sav/mod.rs`, `map/binary.rs` | formato de mapa OpenTTD | 2 · probado (roundtrip + carga de saves reales) | `tests/sav_load_rail_saves.rs` (`grinnway_sav_has_rail_network`) | Bajo |

## Trenes

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Consist (loco + vagones, longitud) | `train_consist.rs` + campos en `vehicle.rs`; save JSON v12; import `.sav` conserva vagones | `train.h` (`Next()`, `tcache`), `train_cmd.cpp:110-254` (`ConsistChanged`) | 2 · probado | `attach_and_detach_wagon`, `consist_tile_span_grows_with_units`, `train_consist_*`, `decodes_front_vehicles_and_train_wagons` | Medio: sin insertar en medio ni golden longitud vs OpenTTD |
| Velocidad máxima por motor | `engine.rs` (`EngineDef::max_speed`, `speed_kmh` sin ÷2 para trenes) | `rail_vehicles` (engine info) | 2 · probado | tests de `engine.rs` | Bajo |
| Aceleración | `engine.rs::train_acceleration` + `accelerate_train_speed` / `decelerate_train_speed`; `vehicle.rs::update_movement_speed` rama `Train` | `train_cmd.cpp:3080-3090` (`UpdateSpeed` `AM_ORIGINAL`: `accel·2` / freno `accel·−4`), `:444-452` (`UpdateAcceleration`: `Clamp(power/weight·4, 1, 255)`) | 3 · validado (Rail 3B) | `kirby_train_acceleration_matches_upstream`, `train_line_divergences_are_absent_after_rail_3b` | Bajo |
| Frenado por curva | `vehicle.rs::set_direction_with_curve_penalty` + `apply_immediate_train_turnaround` (`ACCEL_SLOWDOWN`, `small_turn=64` / `large_turn=128`) | `train_cmd.cpp:3147-3152` (`_accel_slowdown`), `:3564-3568` (`cur_speed -= x·cur_speed >> 8` en locomotora) | 3 · validado (Rail 3B) | `train_loses_speed_on_direction_change`, chequeo `train_no_curve_braking` | Bajo |
| Paso sub-tesela (`progress` 0–255) | compartido con carretera (`progress_step_for_speed`, 192/256) | `vehicle_base.h:439-454` | 3 · validado (heredado del golden de carretera) | `tests/golden_roadveh.rs` | Medio: escala absoluta distinta (5 Hz) |
| Posición sub-tile / render | `road_movement.rs::train_straight_subtile` (eje central) + `vehicle_subtile` | `vehicle.cpp:3359-3392` (`_vehicle_subcoord` por enterdir×track) | 3 · validado (Rail 3E: traza↔render) | `train_render_subtile_consistency`, `rail_render_evaluation.md` | Medio: piezas diagonales ≈ centro de vía |
| Reversa | `vehicle.rs::apply_immediate_train_turnaround` (instantánea) + comando `turn_around_vehicle` | `train_cmd.cpp` (`ReverseTrainDirection`, con chequeos y coste) | 2 · probado | `train_reverses_immediately_when_next_tile_opposite`, `turn_around_vehicle_reverses_train_heading` | Medio |
| Entrada/salida de estación | `station.rs::rail_station_stop_tile` + `resolve_order_destination` → plataforma; `vehicle_physically_at_station` en plataforma | `train_cmd.cpp:266-305` (`GetTrainStopLocation`), `station_cmd.cpp:3846-3881` (frenado sub-tile) | 3 · validado (Rail 3C) | `train_line_emits_rail_block_and_events`, `showcase_train_enters_rail_station_platform`, chequeo `train_platform_stop` | Bajo |
| Carga/descarga | `sim_step.rs` + `cargo_packet.rs` (gradual por tick, packets) | `economy.cpp:1609` (`LoadUnloadVehicle`, gradual) | 2 · probado (Fase 2; `instant_loading` cerrado) | `train_loads_freight_from_rail_station_waiting_cargo`, golden `instant_loading=false` | Medio: velocidades MVP, no tablas NewGRF |
| Entrada/salida de depósito | `refit.rs::vehicle_in_depot` + orden `Depot`; salida inmediata | `train_cmd.cpp:2354-2427` (`CheckTrainStayInDepot`, espera ~37 ticks), `rail_cmd.cpp:2975-2991` (`_fractcoords_enter`) | 2 · probado (sin timing OpenTTD) | tests de `command/tests/rail.rs` y showcase | Medio |
| Órdenes (estación/waypoint/depósito/condicionales) | `vehicle.rs` (`VehicleOrder`), waypoint solo trenes | `order_*.cpp` | 2 · probado | `train_order_through_waypoint_advances_without_full_stop` | Medio |
| Señales en movimiento / forzar paso | `sim_step.rs` (bloqueado → `cur_speed = 0`), `force_proceed` | `train_cmd.cpp:3454-3456` (señal roja: `cur_speed=0`, `progress=255`) | 2 · probado | `force_vehicle_proceed_sets_flag_on_train` | Alto (semántica de bloque simplificada) |
| Render/interpolación | `render/vehicles.rs` + `render_trace.rs` (sub-teselas en CSV) | (no aplica: OpenTTD corre a ~33 Hz sin interpolar) | 3 · validado (Rail 3E) | `train_line_extrapolation_subtile_is_monotonic`, `sprite_selection_uses_extrapolated_pose_for_train` | Bajo en rectas X/Y |

## Infraestructura de paridad ferroviaria

| Herramienta | Estado |
|---|---|
| Traza por tick para trenes | **Implementada (Fase Rail 1)** — bloque `rail` en `VehicleRecord` (partes, track bits, bloqueos, depósito, plataforma) + eventos `SignalWait*`, `DepotEntry/Exit`, `SignalStateChanged` |
| Escenario headless de tren | **Implementado (Fase Rail 1)** — `train_line` en `parity/scenario.rs` (depósito, L con curva, señal de bloque, 2 estaciones, órdenes A↔B) |
| Comparador con subsistemas rail | **Implementado (Fase Rail 2)** — subsistemas `rail_infrastructure`/`train_motion`/`consist_geometry`/`pathfinding`/`station_entry`/`loading`/`signaling`/`reservation`/`depot`, filtros `--tile`/`--event`, `--subtile-epsilon` (default 0.51) y `--json` |
| Golden de tablas C++ de tren | **Implementado (Fase Rail 3A)** — `extract_train_movement.py` + `train_movement_golden.json` + `golden_rail.rs` (11 tests) |
| Chequeos de divergencia rail en `parity/report.rs` | **Implementados (Rail 3B–3E)** — `train_road_acceleration`, `train_no_curve_braking`, `train_platform_stop`, `train_signal_wait`, `train_render_subtile_consistency`, `train_diagonal_subcoord_approximation` |
| Reporte `train_line_divergences.md` | **Implementado (Rail 4)** — `parity_runner --scenario train_line --divergence-report` |
| Escenarios headless | `truck_bay`, `train_line`, `train_signal` |

## Top 5 divergencias ferroviarias detectadas en la auditoría

1. ~~**Aceleración de tren usa la fórmula de carretera**~~ **Corregida (Rail 3B)** —
   `train_acceleration` + `accel·2` / freno `accel·4`.
2. ~~**Sin frenado por curva**~~ **Corregida (Rail 3B)** — `_accel_slowdown` en
   giros y reversas inmediatas.
3. ~~**El tren no entra a la plataforma**~~ **Corregida (Rail 3C)** —
   `rail_station_stop_tile` + carga con `at_platform: true`.
4. ~~**Sin consist**~~ **Mitigado (Fase 1 estructural)** — cadena
   loco+vagones, longitud cacheada, ocupación multi-tesela básica; falta
   paridad fina de geometría/PBS (Fase 3).
5. **Señales sin PBS ni semántica ENTRY/EXIT/COMBO** y **salida de depósito
   instantánea** (OpenTTD espera ~37 ticks y usa frames de entrada/salida).

## Cómo regenerar la evidencia

```bash
# Suite completa (tests ferroviarios + carretera)
./scripts/check.sh

# Reportes markdown de divergencias (carretera + ferrocarril)
./scripts/regenerate_parity_reports.sh
```

El reporte ferroviario queda en `docs/parity/train_line_divergences.md` (600
ticks de `train_line`). El de carretera en `docs/parity/divergences_found.md`.

## Revisión por IA avanzada (pendiente)

El plan Rail 0–4 está cerrado en código y tests, pero **debe revisarse** por
una IA avanzada con el checklist de
[`RAIL_REVIEW_HANDOFF.md`](RAIL_REVIEW_HANDOFF.md) antes de considerar la
paridad ferroviaria «auditada».
