# Mapa OpenTTD → openttdrs (ferrocarriles)

Correspondencia entre el código C++ de referencia (`OpenTTD/`, solo lectura) y
los módulos Rust ferroviarios, con el mecanismo de validación disponible para
cada pieza. Complementa `openttd_mapping.md` (vehículos de carretera).

## Vías y mapa

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `TrackBits` 6 piezas (X/Y/Upper/Lower/Left/Right) en `m5[0:6]` | `track_type.h:43-52`, `rail_map.h:136-150` (`GetTrackBits`) | `command/transport/rail.rs` (`RAIL_TB_*`, mismos valores 0x01–0x20) y `map/rail_slope.rs` (`TRACK_BIT_*`) | tests de colocación (`command/tests/rail.rs`); falta golden piezas×lados |
| `RailTileType` (Normal=0, Signals=1, Depot=3) en `m5[6:2]` | `rail_map.h:23-40` | `map/types.rs` (`TileKind::Rail`/`RailDepot`; `RAIL_TILE_SIGNALS` en `rail_signals.rs`) | tests de mapeo binario (`map/mod.rs`) |
| Autorail / merges de piezas | `rail_cmd.cpp` (`CmdBuildSingleRail`) | `rail_trackbits_from_neighbors`, `merge_rail_trackbits`, `junction_merge_for_neighbor` (`command/transport/{rail,shared}.rs`) | `autorail_crossing_two_lines_yields_clean_x_y_cross` y afines |
| Fundaciones y vía en pendiente | `rail_cmd.cpp` (`CheckRailSlope`), `slope_func.h` | `map/rail_slope.rs` (`rail_trackbits_valid_on_slope`, `rail_foundation_for_trackbits`) + autoslope (`command/terraform.rs`) | `computed_tileh_matches_openrtd_sw` y tests de `rail_slope.rs` |
| `RailTypeInfo` (railtypes, `curve_speed`, electrificación, conversión) | `rail.h:26-525`, `CmdConvertRail` | **No implementado** (sin `RailType` en core) | — (documentado en `rail_unknown_features.md`) |
| Ownership de tile de vía | `rail_map.h` / `tile_map.h` (`GetTileOwner` en `m1`) | **No implementado** (`m1` se fuerza a 0 en `write_normal_rail_tile`) | — |

## Movimiento de trenes

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `Train::UpdateSpeed` `AM_ORIGINAL` (`accel·2`, freno `accel·−4`) | `train_cmd.cpp:3080-3090` | **Paridad (Rail 3B)**: `engine.rs::accelerate_train_speed` / `decelerate_train_speed`; `vehicle.rs::update_movement_speed` rama `Train` | `kirby_acceleration_formula_matches_golden`, `train_line_divergences_are_absent_after_rail_3b` |
| `Train::UpdateAcceleration` = `Clamp(power/weight·4, 1, 255)` | `train_cmd.cpp:444-452` | **Paridad (Rail 3B)**: `engine.rs::train_acceleration` | `kirby_train_acceleration_matches_upstream` |
| Frenado por curva `_accel_slowdown` {64, 128, 64, 2} (`cur_speed -= x·cur_speed >> 8`) | `train_cmd.cpp:3147-3152` (tabla), `:3564-3568` (aplicación) | **Paridad (Rail 3B)**: `set_direction_with_curve_penalty` + `apply_immediate_train_turnaround` | `train_loses_speed_on_direction_change`, chequeo `train_no_curve_braking` |
| `GetCurveSpeedLimit` (61 / 88 / `232-(13-n)²`; solo AM_REALISTIC) | `train_cmd.cpp:312-381` | **No implementado** (aplicaría si se adopta AM_REALISTIC) | — |
| `GetAdvanceSpeed = speed·3/4`, distancias 192/256 | `vehicle_base.h:439-454` | `engine.rs::progress_step_for_speed` + `tile_progress_length` (compartido con carretera) | golden `advance_constants_match_upstream` |
| Subcoordenadas por pieza `_vehicle_subcoord[enterdir][track]` | `vehicle.cpp:3359-3392` | **Evaluado (Rail 3E)**: golden 3A; render usa `train_straight_subtile` (centro); divergencia en piezas diagonales | `vehicle_subcoord_matches_rust_copy`, `train_diagonal_subcoord_approximation` |
| `TrainController` (bucle de avance del frente) | `train_cmd.cpp:3359-3656` | `Vehicle::step` + `advance_one_tile` (pasos cardinales entre teselas; curvas solo vía track bits del pathfinder) | tests de `vehicle.rs` |
| `ReverseTrainDirection` / dar la vuelta | `train_cmd.cpp` | `apply_immediate_train_turnaround` (automática) + `command/vehicles.rs::turn_around_vehicle` (manual) | `train_reverses_immediately_when_next_tile_opposite` |
| Consist: `Next()`, `ConsistChanged`, `cached_total_length` | `train.h:74-188`, `train_cmd.cpp:110-254` | **No implementado** (tren puntual; `.sav` descarta vagones) | `decodes_front_vehicles_and_skips_wagons` |

## Señales y reservas

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| Señales por track en `m2`/`m3`/`m3hi` (presencia, estado, tipo, variante) | `rail_map.h:287-526`, `signal_type.h` | `rail_signals.rs` (`signal_placement_for_track`, `signal_on_track_mask`, `signal_is_green`) — mismo encoding | tests de `rail_signals.rs` |
| `UpdateSignalsOnSegment` (propagación por segmento) | `signal.cpp:280-660` | `update_rail_signal_states` + `rail_block_ahead` (modelo de bloque simplificado «v1») | `block_ahead_stops_at_next_signal`, `sim_train_waits_until_block_ahead_clears`, `train_signal_divergences_are_absent_after_rail_3d` |
| Semántica ENTRY/EXIT/COMBO (presignals) | `signal_type.h`, `signal.cpp` | **Decidido (Rail 3D)**: encoding en saves; ENTRY ignorado al bloquear; EXIT/COMBO sin propagación | `entry_signal_does_not_block_train`, escenario `train_signal` |
| Señal roja detiene el tren (`cur_speed=0`, `progress=255`) | `train_cmd.cpp:3454-3456` | `sim_step.rs` (tren bloqueado → `cur_speed = 0`, no avanza) | tests de integración de `rail_signals.rs` |
| PBS: `TryReserveRailTrack`, `FollowTrainReservation`, señales `Path` | `pbs.cpp/h` | **No implementado** (`m2_hi` se conserva sin lógica; anticolisión propia vía `train_blocked_by_traffic`) | `trains_block_head_on_without_signal` |

## Estaciones, depósitos, túneles

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `CmdBuildRailStation` (plataformas × longitud, layout gfx) | `station_cmd.cpp:1447` | `command/transport/station.rs` (`place_rail_station_area`, `rail_station_layout`, 1..=7) | `place_rail_station_area_*` |
| Entrada del tren a plataforma + `GetTrainStopLocation` (OSL near/middle/far) | `train_cmd.cpp:266-305`, `order_type.h:97-102` | **Paridad (Rail 3C)**: `rail_station_stop_tile` (Middle por defecto); `resolve_order_destination` → plataforma | `showcase_train_enters_rail_station_platform`, `train_platform_stop` |
| Frenado sub-tile en plataforma `cur_speed = max(0, (stop-x)·20 − 15)` | `station_cmd.cpp:3874-3880` | **No implementado** | — |
| Waypoints | `waypoint_cmd.cpp` | `place_rail_waypoint` (solo vía recta X/Y) + orden `Waypoint` sin parada completa | `train_order_through_waypoint_advances_without_full_stop` |
| Depósito: dirección de boca, entrada/salida con frames | `rail_map.h:171-185`, `rail_cmd.cpp:2975-3064` (`_fractcoords_enter`, `TicksToLeaveDepot`) | `place_rail_depot_dir` + `rail_depot_exit_for_dir`; **sin frames ni timing** | `rail_depot_beside_x_line_connects_exit_tile` |
| Espera en depósito (`CheckTrainStayInDepot`, ~37 ticks) | `train_cmd.cpp:2354-2427` | **No implementado** (salida inmediata) | — |
| Túnel/puente: wormhole, ocultamiento (`_tunnel_visibility_frame` {12,8,8,12}), límite de velocidad de puente | `tunnelbridge_cmd.cpp:1956-2087`, `train_cmd.cpp:427-429` | Colocación en `command/transport/bridge.rs`; tránsito como vía normal (bits X\|Y); **sin ocultar tren ni límite de puente** | 0 tests rail de túnel/puente (hueco detectado) |

## Pathfinding

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| YAPF rail (`CYapfRail`, reserva durante pathfind) | `pathfinder/yapf/yapf_rail.cpp:36-604` | `pathfinder.rs` A* propio direccional `(tile, in_side)`; sin reservas ni penalizaciones YAPF | `astar_rail_requires_matching_axis`, `astar_rail_no_turn_at_plain_crossing` |
| `CFollowTrackRail` (seguidor de vías) | `pathfinder/follow_track.hpp:27-507` | `rail_bit_for_sides`, `rail_bits_touching_side`, `rail_traversal_bits` (`pathfinder.rs:218-270`) | tests de conectividad; falta golden piezas×lados |
| Depósito solo por la boca | `train_cmd.cpp` + `depot_map.h` | `rail_depot_mouth` (`pathfinder.rs:274-278`) | `rail_depot_beside_x_line_connects_exit_tile` |
| Estación no transitable salvo origen/destino | `yapf_rail.cpp` (penalización plataforma) | trenes no rutean a través de plataformas (`astar_rail_station_reaches_track_below_entrance`) | test citado |

## Trazabilidad (pendiente, Fases Rail 1–2)

| Pieza | Estado |
|---|---|
| Bloque `rail` en `VehicleRecord` + eventos ferroviarios | Diseñado en `rail_debugging_plan.md`, sin implementar |
| Escenario `train_line` en `parity/scenario.rs` | Sin implementar (hoy solo `truck_bay`) |
| Subsistemas rail en `parity_diff` | Sin implementar |
| Golden `train_movement_golden.json` (`scripts/extract_train_movement.py`) | Sin implementar |
| Chequeos rail en `parity/report.rs` → `divergences_found.md` | Sin implementar |
