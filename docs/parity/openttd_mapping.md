# Mapa OpenTTD → openttdrs (vehículos de carretera y estaciones)

Correspondencia entre el código C++ de referencia (`OpenTTD/`, solo lectura) y
los módulos Rust, con el mecanismo de validación disponible para cada pieza.

## Movimiento de vehículos de carretera

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `RoadVehicle::UpdateSpeed` (AM_ORIGINAL, accel 256) | `roadveh_cmd.cpp:742-748` | `engine.rs::update_road_speed` (`ROAD_ACCEL_ORIGINAL = 256`) | golden `advance_constants_match_upstream` + tests de `engine.rs` |
| `GroundVehicleBase::DoUpdateSpeed` (subspeed u8, tempmax) | `ground_vehicle.hpp` | `engine.rs::update_road_speed` (misma aritmética con truncado a u8) | tests `engine.rs` |
| `GetAdvanceSpeed = speed * 3 / 4` | `vehicle_base.h:439-442` | `engine.rs::progress_step_for_speed` (numerador/denominador 3/4) | golden |
| `GetAdvanceDistance` 192 diagonal / 256 cardinal | `vehicle_base.h:451-454` | `engine.rs::tile_progress_length` (`TILE_AXIAL_DISTANCE`/`TILE_CORNER_DISTANCE`) | golden |
| `frame` dentro de la tesela (0..15 por entrada de tabla) | `roadveh.h` (`RoadVehicle::frame`) | `Vehicle::progress` 0–255 lineal (reescalado; sin tabla por frame en la sim) | — (divergencia estructural documentada) |
| Penalización de giro `cur_speed -= cur_speed >> 2` | `roadveh_cmd.cpp:1481` (también `:1353`, `:1426`) | `vehicle.rs::set_direction_with_curve_penalty` (Fase 2; bus/camión, no trenes) | test `road_vehicle_loses_quarter_speed_on_turn`; chequeo `curve_speed_penalty` como regresión |
| Tablas `_roadveh_drive_data_*` (trayectorias por tesela) | `table/roadveh_movement.h:10-1084` | `road_movement.rs` (`STRAIGHT`, `CURVE_*`, `U_TURN_*`) — solo para render | golden compara data_0/2/3 punto a punto |
| `Direction` 0..7 (N=0 … NW=7) | `direction_type.h` | `vehicle.rs` (`DIR_N`..`DIR_NW`) | tests `vehicle.rs` |
| Media vuelta en parada (`RVSB_IN_ROAD_STOP` + reversing) | `roadveh_cmd.cpp:1306-1330` | `Vehicle::depart_turn` + `U_TURN_*` | tests `road_movement.rs` |

## Estaciones y paradas

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| Bahía (bay stop) bus/camión — el vehículo ENTRA a la tesela | `roadveh_cmd.cpp:1311-1330`, tablas `_rv_station_left_*` (`roadveh_movement.h:458-737`, punteros `:1052-1067`) | Fase 2: destino = tesela de la bahía (`station.rs::resolve_order_destination`); render con las 8 tablas exactas del lado izquierdo (`road_movement.rs::bay_station_table` + `bay_subtile`: entrada, lazo y salida). Sin portar: `_rv_station_right_*` y dársena `near` | golden `bay_station_tables_match_rust_copies` (punto a punto + stop frame); chequeo `bay_stop_position` como regresión |
| Frame exacto de parada en bahía | `_road_stop_stop_frame` (`roadveh_movement.h:1087-1093`, valores 11–20) + chequeo `roadveh_cmd.cpp:1496-1502` | `BayStationTable::stop` por tabla (copiado del upstream); el vehículo se detiene y carga en ese punto | golden verifica valor y que sea el vértice del lazo |
| `StationType` en `m6` (bits 3–6) | `station_map.h` | `station.rs::station_type_from_m6`, `stop_kind_from_m6` | tests `station.rs` |
| Orientación de la boca de la parada (`m5 & 3`) | `station_map.h` (`GetBayRoadStopDir`) | `command/transport/station.rs::road_stop_m5` + `road_stop_approach_tile` | tests de comandos |
| Carga/descarga gradual por tick | `economy.cpp:1609` (`LoadUnloadVehicle`) | `sim_step.rs::load_vehicles` (instantánea) | divergencia `instant_loading` |
| Cobertura de estación (radio) | `station_cmd.cpp` (catchment) | `station.rs` (`STATION_COVERAGE_RADIUS = 4`, cuadrado) | tests `station.rs` |

## Tiempo

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| 74 ticks/día, ~33,3 ticks/s | `timer/timer_game_tick.h:77` | `GameTick` + cliente a 5 Hz (`simulation.rs:12`), un día ≠ mismo tiempo real | divergencia `tick_rate` |
| `Vehicle::frame` avanza N pasos por tick según `GetAdvanceSpeed` | `roadveh_cmd.cpp` (`RoadVehController`) | `Vehicle::step` suma `progress_step` por tick (puede cruzar tesela en bucle) | tests `vehicle.rs` |

## Trazabilidad (nuevo en Fase 1)

| Pieza | Ruta |
|---|---|
| Esquema de traza (registros + eventos) | `openttdrs-core/src/parity/record.rs` |
| Tracer por diff de estado (cero hooks en la lógica) | `openttdrs-core/src/parity/tracer.rs`, único punto de enganche en `sim_step.rs` (última línea de `step`) |
| Escenario `truck_bay` | `openttdrs-core/src/parity/scenario.rs` |
| Comparador | `openttdrs-core/src/parity/diff.rs` + bin `parity_diff` |
| Divergencias conocidas | `openttdrs-core/src/parity/report.rs` → `docs/parity/divergences_found.md` |
| Traza de render por frame | `openttdrs-client/src/render_trace.rs` (`OPENTTDRS_RENDER_TRACE`) |
