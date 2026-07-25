# Paridad OpenTTD ↔ openttdrs

Madurez, mapeos C++↔Rust, gaps, UI, divergencias y oráculos. Roadmaps de producto: [PLANIFICACION.md](PLANIFICACION.md). Pin JSON y capturas siguen en `docs/parity/`.

## Estado canónico actual

**Fecha de corte: 2026-07-25. Referencia: OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`.** Esta tabla es la fuente de
verdad para el estado vigente. Las tablas detalladas posteriores conservan el
mapeo y la evidencia de auditorías anteriores; ante una contradicción prevalece
este bloque y debe corregirse la fila antigua en el mismo cambio.

Leyenda: **alta** = jugable y ampliamente probado; **media** = funcional con
semántica parcial; **inicial** = primer corte utilizable; **ausente** = todavía
no existe. Ningún nivel implica compatibilidad binaria o de red con OpenTTD.

| Área | Estado vigente | Evidencia y límite principal |
|---|---|---|
| Tick y determinismo | **Alta** | Tick de 27 ms, RNG/orden autoritativo, hash canónico, replay y save/load deterministas |
| Carretera | **Alta funcional / media exacta** | Construcción, depósitos, paradas, overtaking y tablas de movimiento; quedan RVSB/dársenas y escala |
| Ferrocarril | **Alta funcional / media exacta** | Consists, railtypes, señales, PBS/YAPF, túneles/puentes y plataformas; los oráculos externos cubren escenarios acotados |
| Economía y carga | **Media** | 11 cargas temperate, packets, transfer/deliver, CargoDist y ratings; faltan reglas completas por clima/NewGRF |
| Pueblos e industrias | **Media** | Crecimiento, casas/industrias vanilla y producción; contenido de Arctic/Tropic/Toyland incompleto |
| Órdenes y horarios | **Media-alta en core / media en UI** | Full-load all/any, no-load/no-unload, transfer, non-stop/go-via, stop-location, refit de depósito, condicionales y timetable-start; la UI no expone todo |
| Fiabilidad y servicio | **Media en core / inicial en UI** | Averías, intervalos días/porcentaje, servicio y autoenvío a depósito; falta el editor completo de intervalos/unbunch |
| Aviones | **Media** | Aeropuertos FTA, compra, vuelo, ruido y crashes; presentación y casos límite incompletos |
| Barcos | **Inicial** | Depósitos, docks, boyas, locks, compra y A* acuático; movimiento y órdenes todavía simplificados |
| Guardado propio JSON | **Alta** | Formato versionado con migraciones y determinismo mid-run |
| Compatibilidad `.sav` | **Inicial-media** | Import/export parcial; no es round-trip completo ni garantía de compatibilidad histórica |
| NewGRF | **Media de parseo / inicial-media de runtime** | Actions 0–14 reconocidas y varios paths Action 1/2/3/5; callbacks y semántica total incompletos |
| Multijugador | **Inicial** | Lockstep TCP, dedicated, late join y host migration; protocolo propio sin lobby, auth, cifrado ni interoperabilidad |
| IA / GameScript / editor | **Inicial-media** | TransCargo/RoadHaul, GS-lite y editor propios; Squirrel compatible ausente |
| Render/UI vanilla | **Media-alta visual / media funcional** | Cobertura OpenGFX amplia; no hay oracle visual total ni internacionalización completa |
| Plataformas y release | **Preparada** | Checks Windows/macOS + paquetes reproducibles Linux x86_64, Windows x86_64 y macOS arm64; `0.1.0-alpha.1` aún sin tag/publicación |

## Índice

- [Estado canónico](#estado-canónico-actual)
- [Tick](#tick-de-simulación)
- [Madurez road](#madurez-road--tick)
- [Madurez rail](#madurez-rail)
- [Mapeos](#mapeo-openttd-road)
- [Gaps](#gaps-desconocidos-road)
- [UI](#paridad-ventanas-ui)
- [Divergencias](#divergencias-encontradas)
- [Referencia / pin](#referencia-openttd-clonpin)
- [Snapshots / oráculos](#workflow-snapshot-oráculo)
- [PBS / Airport](#oráculo-pbs-externo)

---

## Tick de simulación

Fuente de verdad: [ADR 0003](adr/0003-tick-37hz-openttd.md) (`OTTD_MILLISECONDS_PER_TICK = 27`, `SIM_TICKS_PER_SECOND ≈ 37.04`). Complementa [ADR 0002](adr/0002-determinismo-tick-referencia.md). Código: `economy/time.rs`, `engine/physics.rs` (`REFERENCE_PROGRESS_STEP = 112`), cliente `simulation.rs` (`SIM_TICK_HZ`).

## Handoffs rail (cerrados)

Fases Rail 0–4 implementadas. Checklist histórico: [archive/RAIL_REVIEW_HANDOFF.md](archive/RAIL_REVIEW_HANDOFF.md), plan de debugging: [archive/rail_debugging_plan.md](archive/rail_debugging_plan.md). Principio vigente: extender `TickRecord`/`ParityEvent`, no duplicar trazas.

## Assets en `parity/`

- Manifiesto pin: [`parity/openttd-reference.json`](parity/openttd-reference.json)
- Screenshots: [`parity/screenshots/`](parity/screenshots/)
- Trazas HTML (p. ej. PBS): archivos `parity/*.html`

## Madurez road / tick

<!-- fuente: parity/status.md -->

Fecha: 2026-07-16 · Fase 1 del sistema de paridad (trazas + runner headless +
comparador de primera divergencia + golden de tablas C++). Tick ~37 Hz y carga
gradual alineados con el código (#125); regenerar informes con
`./scripts/regenerate_parity_reports.sh`.

### Niveles de madurez

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

### Tabla de estado

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
| Tick lógico | `economy/time.rs` + cliente `simulation.rs` (~37 Hz) | `timer_game_tick.h` (74 ticks/día, ~27 ms) | 3 · validado (mismas constantes; ADR 0003) | `PARIDAD.md` § tick, `divergences_found.md` (`tick_rate`) | Bajo |

### Infraestructura de paridad (Fase 1)

| Herramienta | Ruta | Estado |
|---|---|---|
| Traza por tick (JSONL) | `openttdrs-core/src/parity/` (`TickRecord`, `ParityTracer`) | Probado (`tests/parity_system.rs`) |
| Runner headless | `openttdrs-core/src/bin/parity_runner.rs` (`--scenario truck_bay|train_line|train_signal`, `--divergence-report`) | Probado |
| Comparador primera divergencia | `openttdrs-core/src/bin/parity_diff.rs` (`--vehicle`, `--subsystem`, exit code 1 si diverge) | Probado |
| Golden tablas C++ | `scripts/extract_roadveh_movement.py` + `tests/golden_roadveh.rs` | Verde |
| Traza de render | `openttdrs-client/src/render_trace.rs` (`OPENTTDRS_RENDER_TRACE=f.csv`) | Probado |

### Cómo regenerar la evidencia

```bash
## Traza del caso de los videos + reporte de divergencias conocidas
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario truck_bay --ticks 500 --out /tmp/truck_bay.jsonl \
    --divergence-report docs/parity/divergences_found.md

## Reporte ferroviario (escenario train_line, 600 ticks)
./scripts/regenerate_parity_reports.sh

## Comparar dos trazas (0 = idénticas, 1 = divergen)
cargo run -p openttdrs-core --bin parity_diff -- /tmp/a.jsonl /tmp/b.jsonl --vehicle 1

## Regenerar fixture golden desde el C++ (solo lectura de OpenTTD/)
python3 scripts/extract_roadveh_movement.py
```

### Paridad ferroviaria (Rail 0–4)

| Documento | Uso |
|-----------|-----|
| [RAIL_REVIEW_HANDOFF.md](archive/RAIL_REVIEW_HANDOFF.md) | Stub → archive; revisión post Rail 4 |
| [rail_debugging_plan.md](archive/rail_debugging_plan.md) | Stub → archive; fases 0–4 hechas |
| [rail_status.md](#madurez-rail) | Estado vivo por subsistema |
| [train_line_divergences.md](parity/train_line_divergences.md) | Reporte generado (`regenerate_parity_reports.sh`) |

## Madurez rail

<!-- fuente: parity/rail_status.md -->

Fecha: 2026-07-19 · Fases Rail 0–4 + **consist Fase 1** + física subtesela exacta. Usa los mismos niveles de
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

### Infraestructura ferroviaria

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Track bits (6 piezas X/Y/UPPER/LOWER/LEFT/RIGHT) | `command/transport/rail.rs` (`RAIL_TB_*`, autorail, merges, refresco de vecinos) | `rail_map.h:136-150` (`GetTrackBits`), `track_type.h:19-52` | 2 · probado (~20 tests de colocación/merge/cruces) | `command/tests/rail.rs` (`autorail_crossing_*`, `parallel_*`, `set_rail_bits_*`) | Bajo: misma semántica de bits en `m5` |
| Pendientes + fundaciones de vía | `map/rail_slope.rs` (`rail_trackbits_valid_on_slope`), `command/terraform.rs` (autoslope) | `rail_cmd.cpp` (foundations), `slope_func.h` | 3 · validado parcial (`computed_tileh_matches_openrtd_sw`) | tests de `map/rail_slope.rs` | Bajo |
| Señales — colocación y encoding | `rail_signals.rs` (`signal_placement_for_track`, `m2`/`m3`/`m3hi`) | `rail_map.h:287-526`, `signal_type.h` | 2 · probado (encoding compatible con saves OpenTTD) | tests de `rail_signals.rs` (`signal_placement_is_single_bit`, `cycle_signal_side_*`) | Medio: ENTRY/EXIT/COMBO degradados a BLOCK (Rail 3D) |
| Señales — bloqueo | `rail_signals.rs` (`rail_block_ahead`, `train_blocked_by_signal`, `update_rail_signal_states`) + `sim_step.rs` | `signal.cpp:280-660` (`UpdateSignalsOnSegment`) | 3 · validado (Rail 3D: bloque v1 + escenario `train_signal`) | `sim_train_waits_until_block_ahead_clears`, `train_signal_divergences_are_absent_after_rail_3d`, `signal_wait_events_emitted_with_two_trains` | Alto: sin presignals reales ni PBS; timing sin golden contra OpenTTD |
| Reservas de camino (PBS) | `rail_pbs.rs` (TryReserve, `follow_train_reservation`, path signals, plataforma) | `pbs.cpp` (`TryReserveRailTrack`, `FollowTrainReservation`) | 5 · equivalente (fixture PBS 15.3, 40 ticks) | `pbs_openttd_oracle.rs`, `golden_pbs.rs`, `follow_train_reservation_*` | Bajo en el escenario del oráculo; multi-tren / consist largo aún no golden externo |
| Estaciones rail (plataformas 1..=7, waypoints) | `command/transport/station.rs` (`place_rail_station_area`, `rail_station_layout`), `station.rs` | `station_cmd.cpp:1416-1433`, `CmdBuildRailStation` | 2 · probado (layout + flags catenaria m3 compatibles; entrada exige vía adyacente) | `place_rail_station_area_*`, `place_rail_waypoint_*`, `station_*catenary*` | Medio |
| Depósitos rail | `depot.rs` (`Has/SetDepotReservation`), `depot_leave.rs` (`CheckTrainStayInDepot` + `TryPathReserve` + `TicksToLeaveDepot`) | `rail_map.h:256-272`, `train_cmd.cpp:2354-2427`, `rail_cmd.cpp:2999-3044` | 4 · paridad PBS leave | `depot_leave::*`, `two_trains_leave_same_rail_depot_sequentially` | Medio: sin enum `Track` completo; `depot_leave_cleared` es el proxy |
| Túneles/puentes rail | `command/transport/bridge.rs` (compartido con road), `map/slope.rs` | `tunnelbridge_cmd.cpp:1959-2087` | 1 · implementado (colocación validada; **0 tests específicos rail**, solo road) | tests solo `PlaceRoadBridge` en `command/tests/bridge.rs` | Medio: sin ocultamiento del tren (`_tunnel_visibility_frame`) ni límite de velocidad de puente |
| Pathfinding rail | `pathfinder/yapf.rs` (trackdir + señales/reservas) | `pathfinder/yapf/yapf_rail.cpp`, `follow_track.hpp` | 2 · probado (+ golden rutas estáticas) | `yapf_*`, `golden_yapf.rs` | Medio: sin golden tick-a-tick vs OpenTTD; desempates difieren |
| Ocupación/anticolisión | `rail_signals.rs` (`train_blocked_by_traffic`) | (OpenTTD lo resuelve con reservas + señales) | 2 · probado (tile ocupado, frente a frente, tren parado delante) | `trains_block_head_on_without_signal` | Alto: modelo distinto al de OpenTTD (que usa PBS) |
| Railtypes / electrificación / conversión | `rail_type.rs` + `ConvertRail` + catenaria | `rail.h`, `elrail.cpp` / `elrail_data.h` | 2 · probado (Fase 5–6 + catenaria) | `convert_rail_*`, `collect_catenary_*`, `*_engine_requires_*` | Medio: wires PCP + postes PPP + estación/túnel/puente; TO_CATENARY persistente + env; tranvía = RoadType |
| Ownership por tile de vía | `m1` = compañía activa en `PlaceRail` / depósito / túnel / puente (`bridge.rs`, `rail.rs`) | `rail_map.h` (`GetTileOwner` / `MAPO`) | 2 · probado | `place_rail_and_road_write_active_company_owner_m1`, `place_rail_tunnel_and_bridge_write_active_company_owner_m1` | Bajo |
| Serialización (JSON v10, `.sav`, `.ottdmap`) | `save.rs` (migraciones de cruces), `sav/mod.rs`, `map/binary.rs` | formato de mapa OpenTTD | 2 · probado (roundtrip + carga de saves reales) | `tests/sav_load_rail_saves.rs` (`grinnway_sav_has_rail_network`) | Bajo |

### Trenes

| Subsistema | Módulo Rust | Referencia OpenTTD | Nivel alcanzado | Evidencia | Riesgo divergencia |
|---|---|---|---|---|---|
| Consist (loco + vagones, longitud) | `train_consist.rs` + campos en `vehicle.rs`; save JSON v12; import `.sav` conserva vagones | `train.h` (`Next()`, `tcache`), `train_cmd.cpp:110-254` (`ConsistChanged`) | 2 · probado | `attach_and_detach_wagon`, `consist_tile_span_grows_with_units`, `train_consist_*`, `decodes_front_vehicles_and_train_wagons` | Medio: sin insertar en medio ni golden longitud vs OpenTTD |
| Velocidad máxima por motor | `engine.rs` (`EngineDef::max_speed`, `speed_kmh` sin ÷2 para trenes) | `rail_vehicles` (engine info) | 2 · probado | tests de `engine.rs` | Bajo |
| Aceleración | `engine/physics.rs` (`AM_ORIGINAL` + `AM_REALISTIC`/`GetAcceleration`); SAV → Realistic | `train_cmd.cpp` `UpdateSpeed`, `ground_vehicle.cpp` `GetAcceleration` | 5 · equivalente (oráculo Ginzu A4) | `ginzu_realistic_accel_*`, `pbs_openttd_oracle.rs` | Bajo en llano; pendientes/maglev aún no |
| Límite de curva Realistic | `get_curve_speed_limit` + `cached_max_curve_speed` / tilt / `curve_speed_mod` | `train_cmd.cpp:312-381` (`GetCurveSpeedLimit`) | 3 · validado (función + techo) | `curve_speed_limit_*`, integración techo Realistic | Medio: dirs de vagón con lag; sin `railtype.curve_speed` |
| Frenado plataforma Realistic | `train_realistic_station_max_speed` | `train_cmd.cpp:394-415` | 2 · probado (MVP distancia en teselas) | `station_approach_max_speed_*` | Medio: sin píxeles de `GetTrainStopLocation` |
| Frenado por curva Original | `set_direction_with_curve_penalty` (`ACCEL_SLOWDOWN`); **omitido si Realistic** | `train_cmd.cpp:3147-3152`, `:3564-3568` (solo `AM_ORIGINAL`) | 3 · validado (Rail 3B) | `train_loses_speed_on_direction_change`, chequeo `train_no_curve_braking` | Bajo |
| Pendiente → velocidad | `slope_pixel_z` + `vehicle.rs::sync_train_slope_speed` (`z_pos`, progreso y cruce) | `ground_vehicle.hpp` (`UpdateInclination`), `train_cmd.cpp:3140-3152` | 2 · probado | `slope_pixel_z_combines_tile_z_and_partial`, `train_applies_z_change_while_progressing_on_inclined_tile`, climb/descend | Medio: sin bits GoingUp/Down ni paso por píxel de mapa |
| Paso sub-tesela rail | `progress` = remanente `DoUpdateSpeed`; `rail_pixel` 0..15; 2× loco/tick; umbral 192/256 | `ground_vehicle.hpp` `DoUpdateSpeed`, `vehicle_base.h` `GetAdvanceDistance`, `Train::Tick` | 5 · equivalente (oráculo 40 ticks) | `pbs_openttd_oracle.rs`, `axial_and_corner_advance_distances_*`, `pbs_fixture_first_tick_*` | Bajo en el fixture; carretera sigue en modelo 0–255 |
| Posición sub-tile / render | `rail_pixel/16` → visual 0..=255 + `train_subtile_on_rail` | `vehicle.cpp:3359-3392` (`_vehicle_subcoord` por enterdir×track) | 3 · validado (Rail 3E + proyección visual) | `train_render_subtile_consistency`, `train_visual_progress_from_pixel` | Medio: piezas diagonales ≈ centro de vía |
| Reversa | `vehicle.rs::apply_immediate_train_turnaround` (instantánea) + comando `turn_around_vehicle` | `train_cmd.cpp` (`ReverseTrainDirection`, con chequeos y coste) | 2 · probado | `train_reverses_immediately_when_next_tile_opposite`, `turn_around_vehicle_reverses_train_heading` | Medio |
| Entrada/salida de estación | `station.rs::rail_station_stop_tile` + `resolve_order_destination` → plataforma; `vehicle_physically_at_station` en plataforma | `train_cmd.cpp:266-305` (`GetTrainStopLocation`), `station_cmd.cpp:3846-3881` (frenado sub-tile) | 3 · validado (Rail 3C) | `train_line_emits_rail_block_and_events`, `showcase_train_enters_rail_station_platform`, chequeo `train_platform_stop` | Bajo |
| Carga/descarga | `sim_step.rs` + `cargo_packet.rs` (gradual por tick, packets) | `economy.cpp:1609` (`LoadUnloadVehicle`, gradual) | 2 · probado (Fase 2; `instant_loading` cerrado) | `train_loads_freight_from_rail_station_waiting_cargo`, golden `instant_loading=false` | Medio: velocidades MVP, no tablas NewGRF |
| Entrada/salida de depósito | `depot_leave.rs` (37 ticks, bit `m5`, `try_path_reserve`, reentrada, stagger) + `refit` Hidden | `train_cmd.cpp:2354-2427`, `rail_cmd.cpp:2999-3044` | 4 · paridad leave | `train_waits_37_ticks_*`, `consist_followers_activate_*`, `same_depot_order_reenters_*` | Medio: activación por `TicksToLeaveDepot` aproximada con fractcoords/`rail_pixel` |
| Órdenes (estación/waypoint/depósito/condicionales) | `vehicle.rs` (`VehicleOrder`), waypoint solo trenes | `order_*.cpp` | 2 · probado | `train_order_through_waypoint_advances_without_full_stop` | Medio |
| Señales en movimiento / forzar paso | `sim_step.rs` (bloqueado → `cur_speed = 0`), `force_proceed` | `train_cmd.cpp:3454-3456` (señal roja: `cur_speed=0`, `progress=255`) | 2 · probado | `force_vehicle_proceed_sets_flag_on_train` | Alto (semántica de bloque simplificada) |
| Render/interpolación | `render/vehicles.rs` + `render_trace.rs` (sub-teselas en CSV) | (no aplica: OpenTTD corre a ~33 Hz sin interpolar) | 3 · validado (Rail 3E) | `train_line_extrapolation_subtile_is_monotonic`, `sprite_selection_uses_extrapolated_pose_for_train` | Bajo en rectas X/Y |

### Infraestructura de paridad ferroviaria

| Herramienta | Estado |
|---|---|
| Traza por tick para trenes | **Implementada (Fase Rail 1 + #54)** — bloque `rail` (partes, bloqueos, depósito, plataforma, `reserved_len` / `blocked_by_reservation` / `reservation_end`) + eventos `SignalWait*`, `DepotEntry/Exit`, `SignalStateChanged` |
| Escenario headless de tren | **Implementado (Fase Rail 1)** — `train_line` en `parity/scenario.rs` (depósito, L con curva, señal de bloque, 2 estaciones, órdenes A↔B) |
| Comparador con subsistemas rail | **Implementado (Fase Rail 2)** — subsistemas `rail_infrastructure`/`train_motion`/`consist_geometry`/`pathfinding`/`station_entry`/`loading`/`signaling`/`reservation`/`depot`, filtros `--tile`/`--event`, `--subtile-epsilon` (default 0.51) y `--json` |
| Golden de tablas C++ de tren | **Implementado (Fase Rail 3A)** — `extract_train_movement.py` + `train_movement_golden.json` + `golden_rail.rs` (11 tests) |
| Chequeos de divergencia rail en `parity/report.rs` | **Implementados (Rail 3B–3E)** — `train_road_acceleration`, `train_no_curve_braking`, `train_platform_stop`, `train_signal_wait`, `train_render_subtile_consistency`, `train_diagonal_subcoord_approximation` |
| Reporte `train_line_divergences.md` | **Implementado (Rail 4)** — `parity_runner --scenario train_line --divergence-report` |
| Escenarios headless | `truck_bay`, `train_line`, `train_signal`, `train_pbs`, … |
| Golden PBS / YAPF interno | **#54/#53 slices** — `train_pbs_golden.json`, `yapf_routes_golden.json` (no vs OpenTTD) |
| Oráculo PBS externo | Fixture `train_pbs_15_3.sav` + 40 ticks OpenTTD — [PBS_EXTERNAL_ORACLE.md](#oráculo-pbs-externo); cinemática + reservas alineadas (`rail_pixel` + `DoUpdateSpeed` + 2× loco) |

### Top 5 divergencias ferroviarias detectadas en la auditoría

1. ~~**Aceleración de tren usa la fórmula de carretera**~~ **Corregida (Rail 3B)** —
   `train_acceleration` + `accel·2` / freno `accel·4`.
2. ~~**Sin frenado por curva**~~ **Corregida (Rail 3B)** — `_accel_slowdown` en
   giros y reversas inmediatas.
3. ~~**El tren no entra a la plataforma**~~ **Corregida (Rail 3C)** —
   `rail_station_stop_tile` + carga con `at_platform: true`.
4. ~~**Sin consist**~~ **Mitigado (Fase 1 estructural)** — cadena
   loco+vagones, longitud cacheada, ocupación multi-tesela básica; falta
   paridad fina de geometría/PBS (Fase 3).
5. ~~**Sin PBS y salida de depósito instantánea**~~ **Mitigado** — hay PBS
   parcial con oráculos externos y espera/salida de depósito de 37 ticks. La
   divergencia vigente es la semántica completa ENTRY/EXIT/COMBO, los costes
   YAPF y la geometría fina fuera de los fixtures cubiertos.

### Cómo regenerar la evidencia

```bash
## Suite completa (tests ferroviarios + carretera)
./scripts/check.sh

## Reportes markdown de divergencias (carretera + ferrocarril)
./scripts/regenerate_parity_reports.sh
```

El reporte ferroviario queda en `docs/parity/train_line_divergences.md` (600
ticks de `train_line`). El de carretera en `docs/parity/divergences_found.md`.

### Revisión por IA avanzada (pendiente)

El plan Rail 0–4 está cerrado en código y tests, pero **debe revisarse** por
una IA avanzada con el checklist de
[`RAIL_REVIEW_HANDOFF.md`](archive/RAIL_REVIEW_HANDOFF.md) (stub → archive) antes de considerar la
paridad ferroviaria «auditada».

## Mapeo OpenTTD (road)

<!-- fuente: parity/openttd_mapping.md -->

Índice: [MAPPING.md](#). Complemento rail: [rail_openttd_mapping.md](#mapeo-openttd-rail).
Madurez: [status.md](#madurez-road--tick).

Correspondencia entre el código C++ de referencia (`OpenTTD/`, solo lectura) y
los módulos Rust, con el mecanismo de validación disponible para cada pieza.

### Movimiento de vehículos de carretera

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

### Estaciones y paradas

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| Bahía (bay stop) bus/camión — el vehículo ENTRA a la tesela | `roadveh_cmd.cpp:1311-1330`, tablas `_rv_station_left_*` (`roadveh_movement.h:458-737`, punteros `:1052-1067`) | Fase 2: destino = tesela de la bahía (`station.rs::resolve_order_destination`); render con las 8 tablas exactas del lado izquierdo (`road_movement.rs::bay_station_table` + `bay_subtile`: entrada, lazo y salida). Sin portar: `_rv_station_right_*` y dársena `near` | golden `bay_station_tables_match_rust_copies` (punto a punto + stop frame); chequeo `bay_stop_position` como regresión |
| Frame exacto de parada en bahía | `_road_stop_stop_frame` (`roadveh_movement.h:1087-1093`, valores 11–20) + chequeo `roadveh_cmd.cpp:1496-1502` | `BayStationTable::stop` por tabla (copiado del upstream); el vehículo se detiene y carga en ese punto | golden verifica valor y que sea el vértice del lazo |
| `StationType` en `m6` (bits 3–6) | `station_map.h` | `station.rs::station_type_from_m6`, `stop_kind_from_m6` | tests `station.rs` |
| Orientación de la boca de la parada (`m5 & 3`) | `station_map.h` (`GetBayRoadStopDir`) | `command/transport/station.rs::road_stop_m5` + `road_stop_approach_tile` | tests de comandos |
| Carga/descarga gradual por tick | `economy.cpp:1609` (`LoadUnloadVehicle`) | `sim_step/cargo_transfer.rs` + `load_unload_speed` (gradual) | regresión `instant_loading` |
| Cobertura de estación (radio) | `station_cmd.cpp` (catchment) | `station.rs` (`STATION_COVERAGE_RADIUS = 4`, cuadrado) | tests `station.rs` |

### Tiempo

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| 74 ticks/día, ~37 ticks/s (27 ms) | `timer/timer_game_tick.h` | `OTTD_MILLISECONDS_PER_TICK` / `SIM_TICKS_PER_SECOND` + cliente `SIM_TICK_HZ` | ADR 0003; `tick_rate` resuelto |
| `Vehicle::frame` avanza N pasos por tick según `GetAdvanceSpeed` | `roadveh_cmd.cpp` (`RoadVehController`) | `Vehicle::step` suma `progress_step` por tick (puede cruzar tesela en bucle) | tests `vehicle.rs` |

### Trazabilidad (nuevo en Fase 1)

| Pieza | Ruta |
|---|---|
| Esquema de traza (registros + eventos) | `openttdrs-core/src/parity/record.rs` |
| Tracer por diff de estado (cero hooks en la lógica) | `openttdrs-core/src/parity/tracer.rs`, único punto de enganche en `sim_step.rs` (última línea de `step`) |
| Escenario `truck_bay` | `openttdrs-core/src/parity/scenario.rs` |
| Comparador | `openttdrs-core/src/parity/diff.rs` + bin `parity_diff` |
| Divergencias conocidas | `openttdrs-core/src/parity/report.rs` → `docs/parity/divergences_found.md` |
| Traza de render por frame | `openttdrs-client/src/render_trace.rs` (`OPENTTDRS_RENDER_TRACE`) |

## Mapeo OpenTTD (rail)

<!-- fuente: parity/rail_openttd_mapping.md -->

Índice: [MAPPING.md](#). Complemento road: [openttd_mapping.md](#mapeo-openttd-road).
Madurez: [rail_status.md](#madurez-rail).

Correspondencia entre el código C++ de referencia (`OpenTTD/`, solo lectura) y
los módulos Rust ferroviarios, con el mecanismo de validación disponible para
cada pieza.

### Vías y mapa

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `TrackBits` 6 piezas (X/Y/Upper/Lower/Left/Right) en `m5[0:6]` | `track_type.h:43-52`, `rail_map.h:136-150` (`GetTrackBits`) | `command/transport/rail.rs` (`RAIL_TB_*`, mismos valores 0x01–0x20) y `map/rail_slope.rs` (`TRACK_BIT_*`) | tests de colocación (`command/tests/rail.rs`); falta golden piezas×lados |
| `RailTileType` (Normal=0, Signals=1, Depot=3) en `m5[6:2]` | `rail_map.h:23-40` | `map/types.rs` (`TileKind::Rail`/`RailDepot`; `RAIL_TILE_SIGNALS` en `rail_signals.rs`) | tests de mapeo binario (`map/mod.rs`) |
| Autorail / merges de piezas | `rail_cmd.cpp` (`CmdBuildSingleRail`) | `rail_trackbits_from_neighbors`, `merge_rail_trackbits`, `junction_merge_for_neighbor` (`command/transport/{rail,shared}.rs`) | `autorail_crossing_two_lines_yields_clean_x_y_cross` y afines |
| Fundaciones y vía en pendiente | `rail_cmd.cpp` (`CheckRailSlope`), `slope_func.h` | `map/rail_slope.rs` (`rail_trackbits_valid_on_slope`, `rail_foundation_for_trackbits`) + autoslope (`command/terraform.rs`) | `computed_tileh_matches_openrtd_sw` y tests de `rail_slope.rs` |
| `RailTypeInfo` (railtypes, `curve_speed`, electrificación, conversión) | `rail.h:26-525`, `CmdConvertRail` | `rail_type.rs` + `RailConvert`: rail/electric/monorail/maglev, compatibilidad, velocidad de curva y catenaria; NewGRF parcial | tests de `rail_type.rs`, conversión y consist |
| Ownership de tile de vía | `rail_map.h` / `tile_map.h` (`GetTileOwner` en `m1`) | `PlaceRail`, depósito, túnel y puente escriben la compañía activa en `m1` | `place_rail_and_road_write_active_company_owner_m1`, `place_rail_tunnel_and_bridge_write_active_company_owner_m1` |

### Movimiento de trenes

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `Train::UpdateSpeed` `AM_ORIGINAL` (`accel·2`, freno `accel·−4`) | `train_cmd.cpp:3080-3090` | **Paridad (Rail 3B)**: `engine.rs::accelerate_train_speed` / `decelerate_train_speed`; `vehicle.rs::update_movement_speed` rama `Train` | `kirby_acceleration_formula_matches_golden`, `train_line_divergences_are_absent_after_rail_3b` |
| `Train::UpdateAcceleration` = `Clamp(power/weight·4, 1, 255)` | `train_cmd.cpp:444-452` | **Paridad (Rail 3B)**: `engine.rs::train_acceleration` | `kirby_train_acceleration_matches_upstream` |
| Frenado por curva `_accel_slowdown` {64, 128, 64, 2} (`cur_speed -= x·cur_speed >> 8`) | `train_cmd.cpp:3147-3152` (tabla), `:3564-3568` (aplicación) | **Paridad (Rail 3B)**: `set_direction_with_curve_penalty` + `apply_immediate_train_turnaround` | `train_loses_speed_on_direction_change`, chequeo `train_no_curve_braking` |
| `GetCurveSpeedLimit` (61 / 88 / `232-(13-n)²`; solo AM_REALISTIC) | `train_cmd.cpp:312-381` | **Parcial**: `get_curve_speed_limit` + caché consist; lag de dirs en vagones | tests en `engine/physics.rs` (`curve_speed_limit_*`) |
| `GetCurrentMaxSpeed` estación (`st_max_speed`) | `train_cmd.cpp:394-415` | **MVP** en `train_realistic_station_max_speed` | `station_approach_max_speed_*` |
| `GetAdvanceSpeed = speed·3/4`, distancias 192/256 | `vehicle_base.h:439-454` | `engine.rs::progress_step_for_speed` + `tile_progress_length` (compartido con carretera) | golden `advance_constants_match_upstream` |
| Subcoordenadas por pieza `_vehicle_subcoord[enterdir][track]` | `vehicle.cpp:3359-3392` | **Evaluado (Rail 3E)**: golden 3A; render usa `train_straight_subtile` (centro); divergencia en piezas diagonales | `vehicle_subcoord_matches_rust_copy`, `train_diagonal_subcoord_approximation` |
| `TrainController` (bucle de avance del frente) | `train_cmd.cpp:3359-3656` | `Vehicle::step` + `advance_one_tile` (pasos cardinales entre teselas; curvas solo vía track bits del pathfinder) | tests de `vehicle.rs` |
| `ReverseTrainDirection` / dar la vuelta | `train_cmd.cpp` | `apply_immediate_train_turnaround` (automática) + `command/vehicles.rs::turn_around_vehicle` (manual) | `train_reverses_immediately_when_next_tile_opposite` |
| Consist: `Next()`, `ConsistChanged`, `cached_total_length` | `train.h:74-188`, `train_cmd.cpp:110-254` | `train_consist/`: cadena loco+vagones, longitud/potencia/peso cacheados, pose por unidad e import `.sav`; faltan algunas operaciones finas de composición | `train_consist::*`, `decodes_front_vehicles_and_train_wagons`, oráculo consist+PBS v2 |

### Señales y reservas

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| Señales por track en `m2`/`m3`/`m3hi` (presencia, estado, tipo, variante) | `rail_map.h:287-526`, `signal_type.h` | `rail_signals.rs` (`signal_placement_for_track`, `signal_on_track_mask`, `signal_is_green`) — mismo encoding | tests de `rail_signals.rs` |
| `UpdateSignalsOnSegment` (propagación por segmento) | `signal.cpp:280-660` | `update_rail_signal_states` + `rail_block_ahead` (modelo de bloque simplificado «v1») | `block_ahead_stops_at_next_signal`, `sim_train_waits_until_block_ahead_clears`, `train_signal_divergences_are_absent_after_rail_3d` |
| Semántica ENTRY/EXIT/COMBO (presignals) | `signal_type.h`, `signal.cpp` | **Decidido (Rail 3D)**: encoding en saves; ENTRY ignorado al bloquear; EXIT/COMBO sin propagación | `entry_signal_does_not_block_train`, escenario `train_signal` |
| Señal roja detiene el tren (`cur_speed=0`, `progress=255`) | `train_cmd.cpp:3454-3456` | `sim_step.rs` (tren bloqueado → `cur_speed = 0`, no avanza) | tests de integración de `rail_signals.rs` |
| PBS: `TryReserveRailTrack`, `FollowTrainReservation`, señales `Path` | `pbs.cpp/h` | Implementación parcial en `pathfinder/yapf.rs`, reservas por track y señales path; no cubre toda la semántica/escala de OpenTTD | `pbs_openttd_oracle`, `pbs_dual_curve_oracle`, oráculo consist+PBS v2 |

### Estaciones, depósitos, túneles

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| `CmdBuildRailStation` (plataformas × longitud, layout gfx) | `station_cmd.cpp:1447` | `command/transport/station.rs` (`place_rail_station_area`, `rail_station_layout`, 1..=7) | `place_rail_station_area_*` |
| Entrada del tren a plataforma + `GetTrainStopLocation` (OSL near/middle/far) | `train_cmd.cpp:266-305`, `order_type.h:97-102` | **Paridad (Rail 3C)**: `rail_station_stop_tile` (Middle por defecto); `resolve_order_destination` → plataforma | `showcase_train_enters_rail_station_platform`, `train_platform_stop` |
| Frenado y punto de parada en plataforma | `station_cmd.cpp:3846-3881`, `GetTrainStopLocation` | Implementación simplificada por distancia, longitud del consist y `OrderStopLocation`; no reproduce aún cada píxel del controlador C++ | `station::geometry` y tests `station_approach_max_speed_*` |
| Waypoints | `waypoint_cmd.cpp` | `place_rail_waypoint` (solo vía recta X/Y) + orden `Waypoint` sin parada completa | `train_order_through_waypoint_advances_without_full_stop` |
| Depósito: boca, reserva `m5` bit 4, frames leave | `rail_map.h:256-272`, `rail_cmd.cpp:2975-3044` (`TicksToLeaveDepot`) | `has/set_depot_reservation` + `ticks_to_leave_depot` + stagger de units | `depot_leave::*`, `depot_reservation_bit_roundtrips` |
| Espera en depósito (`CheckTrainStayInDepot`, ~37 ticks + PBS) | `train_cmd.cpp:2354-2427` | `tick_train_stay_in_depot` + `try_path_reserve` + reentrada/force | `train_waits_37_ticks_*`, `second_train_waits_while_depot_reserved` |
| Túnel/puente: wormhole, ocultamiento (`_tunnel_visibility_frame` {12,8,8,12}), límite de velocidad de puente | `tunnelbridge_cmd.cpp:1956-2087`, `train_cmd.cpp:427-429` | Colocación, tránsito, ocultamiento de sprite por frame y límite de velocidad por tipo; el wormhole sigue simplificado | `tunnel_hides_train_matches_visibility_frame`, `train_on_wooden_bridge_is_speed_capped` |

### Pathfinding

| Concepto OpenTTD | Referencia C++ | Equivalente Rust | Validación |
|---|---|---|---|
| YAPF rail (`CYapfRail`, reserva durante pathfind) | `pathfinder/yapf/yapf_rail.cpp:36-604` | `pathfinder/yapf.rs`: trackdir, penalizaciones y reservas parciales; desempates/costes no son todavía equivalentes en todos los mapas | `golden_yapf`, PBS externos y tests `yapf_*` |
| `CFollowTrackRail` (seguidor de vías) | `pathfinder/follow_track.hpp:27-507` | `rail_bit_for_sides`, `rail_bits_touching_side`, `rail_traversal_bits` (`pathfinder.rs:218-270`) | tests de conectividad; falta golden piezas×lados |
| Depósito solo por la boca | `train_cmd.cpp` + `depot_map.h` | `rail_depot_mouth` (`pathfinder.rs:274-278`) | `rail_depot_beside_x_line_connects_exit_tile` |
| Estación no transitable salvo origen/destino | `yapf_rail.cpp` (penalización plataforma) | trenes no rutean a través de plataformas (`astar_rail_station_reaches_track_below_entrance`) | test citado |

### Trazabilidad (Fases Rail 1–4)

| Pieza | Estado |
|---|---|
| Bloque `rail` en `VehicleRecord` + eventos ferroviarios | **Implementado (Rail 1)** — `parity/tracer.rs`, `parity/record.rs` |
| Escenarios `train_line` / `train_signal` en `parity/scenario.rs` | **Implementados (Rail 1, 3D)** |
| Subsistemas rail en `parity_diff` | **Implementado (Rail 2)** |
| Golden `train_movement_golden.json` | **Implementado (Rail 3A)** |
| Chequeos rail en `parity/report.rs` | **Implementado (Rail 3B–3E)** → `train_line_divergences.md` vía `regenerate_parity_reports.sh` |
| Revisión independiente | **Pendiente** — ver [`RAIL_REVIEW_HANDOFF.md`](archive/RAIL_REVIEW_HANDOFF.md) (IA avanzada) |

## Gaps desconocidos (road)

<!-- fuente: parity/unknown_features.md -->

**Madurez canónica:** [status.md](#madurez-road--tick). Índice de mapeos: [MAPPING.md](#).
Rail: [rail_unknown_features.md](#gaps-desconocidos-rail).

Subsistemas y reglas del original detectados durante el análisis de paridad
(Fase 1) que hoy no tienen equivalente en la sim Rust. Priorizados por impacto
en el caso «camión entra a playa de carga» y en la paridad general de
vehículos de carretera. Muchos ítems ya están ~~tachados~~ (implementados).

### Prioridad alta (afectan el caso de los videos)

1. ~~**Penalización de velocidad en giros (−25 %)**~~ — `roadveh_cmd.cpp:1481`
   (`cur_speed -= cur_speed >> 2`, modelo AM_ORIGINAL). **IMPLEMENTADA en la
   Fase 2** (`vehicle.rs::set_direction_with_curve_penalty`, test
   `road_vehicle_loses_quarter_speed_on_turn`); el chequeo `curve_speed_penalty`
   del reporte quedó como test de regresión.
2. ~~**Entrada a la tesela de la bahía**~~ — **IMPLEMENTADA en la Fase 2**:
   `resolve_order_destination` apunta a la bahía, el pathfinder entra por la
   boca (`m3`) y el vehículo carga dentro (`bay_stop_position` «no observada»).
   Las 8 tablas `_rv_station_left_*` (lado izquierdo, el que usa el port) están
   copiadas en `road_movement.rs::bay_station_table` y validadas punto a punto
   por el golden `bay_station_tables_match_rust_copies`. Sin portar: las 8
   `_rv_station_right_*` (conducción por la derecha) y la dársena `near`
   (la sim modela una dársena por bahía y usa siempre la `far`).
3. ~~**Frame de parada `_road_stop_stop_frame` exacto**~~ — **IMPLEMENTADO en
   la Fase 2**: cada tabla de bahía lleva su stop frame upstream (valores
   11–20, `roadveh_movement.h:1087-1093`); el vehículo se dibuja detenido en
   ese punto y el golden verifica que sea el vértice del lazo.
4. **Retardo de un frame por dirección de giro** — `roadveh_cmd.cpp:1483-1487`:
   en curvas el vehículo «pierde» un frame extra por cada cambio de dirección
   (la curva corta de 8 frames pasa a 10). Afecta el timing en esquinas.
5. **Carga/descarga gradual (hecho)** — `economy.cpp:1609` (`LoadUnloadVehicle`):
   la sim ya transfiere por tick (`load_unload_speed` + packets). Queda afinar
   cantidades frente a tablas NewGRF del motor.

### Prioridad media

6. **Ocupación/bloqueo de bahías (`RoadStop::Enter/Leave`)** —
   `src/roadstop.cpp`: una bahía tiene 2 plazas; un tercer camión espera fuera.
   Hoy no hay exclusión ni cola.
7. **Adelantamiento (`overtaking`)** — `roadveh_cmd.cpp:821-860`: los
   vehículos se adelantan en rectas (con aceleración 512 en vez de 256).
8. **Colisión/seguimiento entre vehículos de carretera**
   (`RoadVehFindCloseTo`, `roadveh_cmd.cpp:1454`): frenado detrás de otro
   vehículo. La sim actual no considera tráfico en carretera.
9. **`GetCurrentMaxSpeed` con límites por tramo** (`roadveh_cmd.cpp`):
   velocidad máxima reducida dentro de bahías/curvas cerradas y por
   `RoadZPosAffectSpeed` (pendientes).
10. **Drive-through stops** — estado `RVSB_IN_DT_ROAD_STOP`, frame de parada
    `RVC_DRIVE_THROUGH_STOP_FRAME`; hoy solo hay bahías 1×1 con acceso.
11. **`last_station_visited` / `ShouldStopAtStation`** — evita re-parar en la
    misma estación y regula paradas de paso; el equivalente Rust usa
    comparación de orden actual solamente.

### Prioridad baja (para fases posteriores)

12. **Averías (`HandleBreakdown`)** y humo/efectos asociados.
13. **`reverse_ctr` y giros en U forzados** fuera de paradas.
14. **Tranvías** (tablas `_roadveh_tram_turn_*`, `roadveh_movement.h:1095+`).
15. **Articulados** (`HasArticulatedPart`): trailers que siguen al frontal.
16. **Aceleración realista (AM_REALISTIC)** — modelo alternativo por potencia y
    peso (`ground_vehicle.hpp::GetAcceleration`).
17. **Días económicos** (`TimerGameEconomy`, `vehicle.cpp:951`): procesamiento
    de vehículos escalonado por `index % DAY_TICKS` (edad, costes, fiabilidad).
18. **Pathfinder YAPF con penalizaciones** (curvas, slopes, drive-through):
    el A* propio no replica los desempates ni los costes de YAPF.

### Cómo detectar regresiones/omisiones nuevas

- Correr `parity_runner --divergence-report` tras cada cambio de la sim: las
  divergencias conocidas se re-verifican contra la traza.
- El test `golden_roadveh::known_divergences_are_confirmed_by_trace` falla si
  una divergencia documentada deja de existir (recordatorio de actualizar docs)
  o si las tablas copiadas dejan de coincidir con el C++.
- Para features de esta lista: al implementarlas, añadir el evento
  correspondiente a `parity/record.rs` y un chequeo en `parity/report.rs`.

## Gaps desconocidos (rail)

<!-- fuente: parity/rail_unknown_features.md -->

**Madurez canónica:** [rail_status.md](#madurez-rail). Índice: [MAPPING.md](#).
Road: [unknown_features.md](#gaps-desconocidos-road).

Subsistemas y reglas del original detectados durante la auditoría de la Fase
Rail 0. Priorizados por impacto en movimiento y estaciones. Muchos ítems ya
están ~~tachados~~ (implementados).

Supuesto histórico (Rail 0): tren puntual. **Fase 1 estructural** añadió consist
(`next_unit` / longitud / ocupación multi-tesela). Varios ítems abajo ya están
parcialmente resueltos; se mantienen tachados o anotados.

### Prioridad alta (afectan movimiento y estaciones)

1. ~~**Aceleración de tren `AM_ORIGINAL`**~~ — **Resuelto (Rail 3B)**.
   `engine.rs::train_acceleration` + `accelerate_train_speed` / `decelerate_train_speed`
   (Kirby: 300 HP / 47 t → `accel = 24`).
2. ~~**Frenado por curva `_accel_slowdown`**~~ — **Resuelto (Rail 3B)**.
   `set_direction_with_curve_penalty` y `apply_immediate_train_turnaround`.
3. ~~**Entrada a la plataforma + punto de parada**~~ — **Resuelto (Rail 3C)**.
   `rail_station_stop_tile` + `at_platform` en traza.
4. ~~**Carga/descarga gradual**~~ — **Resuelto (Fase 2 estructural)**:
   `cargo_packet.rs` + `load_unload_speed`; golden `instant_loading=false`.
5. ~~**Espera y frames de depósito**~~ ✅ `tick_train_stay_in_depot`
   (~37 ticks + chequeo de boca; `depot_leave_cleared`). Fractcoords de pose
   ya existían. Residual: `TicksToLeaveDepot` encadenado fino de vagones.
   → [#96](https://github.com/cavazquez/openttdrs/issues/96).

### Prioridad media

6. ~~**Consist: locomotora + vagones**~~ — **Resuelto (Fase 1 estructural)**:
   `train_consist.rs`, comandos de enganche, save v12, import `.sav` con
   vagones. Pendiente fino: articulados / dual-headed (ítem 17).
7. ~~**Reservas de camino (PBS)**~~ — **MVP (Fase 3 + #54)**:
   `rail_pbs.rs` + `follow_train_reservation` + traza/golden interno
   `train_pbs_golden.json`. Pendiente: golden tick-a-tick vs OpenTTD (ítem 11).
8. ~~**Semántica de presignals ENTRY/EXIT/COMBO**~~ — **Decidido (Rail 3D)**:
   se codifican en saves pero **no tienen semántica de presignal** en la sim
   v1. `SIGTYPE_ENTRY` se ignora al bloquear (`train_blocked_by_signal`);
   EXIT y COMBO se tratan como BLOCK sin propagación por segmento
   (`entry_signal_does_not_block_train`). Path/PBS: ver ítem 7.
9. ~~**Túneles/puentes en tránsito**~~ ✅ Ocultamiento
   (`tunnel_hides_train_at_progress` + `vehicle_hidden_in_tunnel` / cliente) y
   tope de puente (`bridge_max_speed_for_tile` en `update_movement_speed`).
10. **Subcoordenadas por pieza `_vehicle_subcoord`** —
    `vehicle.cpp:3359-3392`: posición (x, y) y dirección exactas al entrar a
    cada track. La sim usa eje central en rectas (`train_straight_subtile`);
    **evaluado en Rail 3E** (`rail_render_evaluation.md`): alineado en X/Y,
    divergencia cosmética documentada en piezas diagonales puras.
    → no abrir issue salvo que se priorice estética diagonal.
11. ~~**Pathfinder YAPF con penalizaciones y reserva**~~ — **MVP (Fase 3 + #53 slice)**:
    `pathfinder/yapf.rs` + golden estático `yapf_routes_golden.json`.
    Golden tick-a-tick interno: `train_pbs_tick_golden.json` (`golden_pbs_tick`).
    Residual: captura binaria vs OpenTTD; caché de segmentos.
    → [#97](https://github.com/cavazquez/openttdrs/issues/97).
12. ~~**Reversa con coste/chequeos**~~ ✅ `TurnAroundVehicle` pone
    `cur_speed=0`, limpia PBS, reintenta reserva y news `PbsStuck` si aplica.
    → [#98](https://github.com/cavazquez/openttdrs/issues/98).

### Prioridad baja (dependen de decisiones estructurales o son cosméticas)

13. ~~**Railtypes** (normal/eléctrico + residual #99)~~ ✅ MVP Fase 5–6 +
    `ACCEL_SLOWDOWN` por railtype; pendientes tipadas mono/maglev. Nieve plana
    1037/1038 sin asset tipado (queda clásica).
    → [#99](https://github.com/cavazquez/openttdrs/issues/99).
14. ~~**Ownership por tile de vía**~~ ✅ `m1` = compañía activa en
    `PlaceRail` / depósito / túnel / puente.
15. ~~**AM_REALISTIC + `GetCurveSpeedLimit`**~~ — **Parcial (2026-07-19)**:
    `GetAcceleration` llano + oráculo PBS `train_pbs_15_3`; `GetCurveSpeedLimit`
    (61 / 88 / `232-(13-n)²`, tilt +20 %, `curve_speed_mod`) + techo en
    `train_do_update_speed`; en Realistic no se aplica `_accel_slowdown`.
    Residual: `RailTypeInfo::curve_speed`; geometría de consist (dirs por
    vagón) aún aproximada con lag de dirección.
16. ~~**Frenado anticipado en plataforma (AM_REALISTIC)**~~ — **MVP**:
    `st_max_speed` (`25·distance_to_go`, techo 120) al estar en plataforma
    con destino en la misma. Residual: `GetTrainStopLocation` exacto
    (ahead/length/stop_at en píxeles).
17. ~~**Multi-head / dual-headed**~~ ✅ Spawn de cabina trasera +
    `other_multiheaded_part` + potencia½ por cabina (vanilla). Residual:
    articulados NewGRF / Action0 `0x13`.
    → [#100](https://github.com/cavazquez/openttdrs/issues/100).
18. ~~**Pendientes que afectan velocidad (`z_up`/`z_down` de
    `_accel_slowdown`)**~~ ✅ `affect_speed_by_z_change` + `sync_train_slope_speed`
    con Z en píxeles (`slope_pixel_z` ≈ `GetSlopePixelZ`) al avanzar progreso o
    cruzar tesela.
19. ~~**Vagones en depósito / compra / refit**~~ ✅ Comandos + UI compra/refit
    + botón «Desenganchar». Residual: insertar en medio del consist.
    → [#101](https://github.com/cavazquez/openttdrs/issues/101).
20. ~~**Choques (`CheckTrainCollision`)**~~ ✅ MVP (`train_collision.rs`); averías ya parciales.

### Issues de seguimiento (jul 2026)

| Ítem | Issue | Notas |
|------|-------|--------|
| 5 | [#96](https://github.com/cavazquez/openttdrs/issues/96) | ✅ Espera ~37 ticks (+ residual `TicksToLeaveDepot`) |
| 11 (+7 residual) | [#97](https://github.com/cavazquez/openttdrs/issues/97) | ✅ Golden tick interno; residual vs OpenTTD |
| 12 | [#98](https://github.com/cavazquez/openttdrs/issues/98) | ✅ Reversa + PBS/news |
| 13 residual | [#99](https://github.com/cavazquez/openttdrs/issues/99) | ✅ Curvas + pendientes tipadas |
| 17 | [#100](https://github.com/cavazquez/openttdrs/issues/100) | ✅ Dual-headed vanilla |
| 19 | [#101](https://github.com/cavazquez/openttdrs/issues/101) | ✅ Compra/refit + desenganchar UI |

### Cómo detectar regresiones/omisiones nuevas

- Al implementar cualquier ítem: añadir el evento correspondiente a
  `parity/record.rs` y un chequeo en `parity/report.rs` (patrón de
  `curve_speed_penalty`/`bay_stop_position` de carretera: primero divergencia
  CONFIRMADA medida en la traza, después test de regresión).
- Los ítems 1–3 tienen hoy tests que **fijan el comportamiento divergente**
  (`train_keeps_speed_on_direction_change`,
  `showcase_train_stays_on_rail_not_station_platform`): al corregirlos hay que
  invertir esos tests, como se hizo con los de bahía en la Fase 2.
- Plan de fases (archivo): [`rail_debugging_plan.md`](archive/rail_debugging_plan.md);
  estado vivo en [`rail_status.md`](#madurez-rail).
- Tras Rail 4, auditar según
  [`RAIL_REVIEW_HANDOFF.md`](archive/RAIL_REVIEW_HANDOFF.md) (stub → archive) antes de
  abrir la siguiente oleada ferroviaria.

## Evaluación render rail

<!-- fuente: parity/rail_render_evaluation.md -->

Fecha: 2026-07-04 · Complementa `rail_status.md` y
[`rail_debugging_plan.md`](archive/rail_debugging_plan.md) (stub → archive).

### Objetivo

Comparar posición lógica (traza `train_line` / `parity_runner`) con lo que dibuja
el cliente (`OPENTTDRS_RENDER_TRACE`), medir saltos de interpolación y documentar
gaps frente a `_vehicle_subcoord` y `_tunnel_visibility_frame`.

### Hallazgos

| Aspecto | Traza lógica (`parity`) | Render (`vehicle_subtile` + cliente) | Estado |
|---|---|---|---|
| Sub-tesela en JSONL | `rail.parts[0].subtile_x/y` | Misma función `vehicle_subtile` a `tick_alpha=0` | **Alineado** — chequeo `train_render_subtile_consistency` |
| Interpolación entre ticks | Sim ~37 Hz; render extrapola | `extrapolate_vehicle_pose` + `tick_alpha` | **Sin retrocesos** en recta (`train_line_extrapolation_subtile_is_monotonic`) |
| Sprite del tren | `dir` lógico | Capa según pose extrapolada | **Alineado** — `sprite_selection_uses_extrapolated_pose_for_train` |
| CSV de render | — | Columnas `logical_subtile_*` / `extrap_subtile_*` añadidas | **Listo** para diff manual vs JSONL |
| `_vehicle_subcoord` por pieza | Golden 3A (`vehicle_subcoord_matches_rust_copy`) | Render usa `train_straight_subtile` (eje central) | **Divergencia cosmética** en piezas diagonales puras (`train_diagonal_subcoord_approximation`) |
| Ocultamiento en túnel | Constante `{12,8,8,12}` portada | Sin ocultar sprite en túnel | **Pendiente** — `tunnel_hides_train_at_progress` solo evalúa umbral |

### Cómo reproducir

```bash
## Traza lógica
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 300 --out /tmp/train_line.jsonl

## Traza de render (cliente con escenario cargado o mapa propio)
OPENTTDRS_RENDER_TRACE=/tmp/render_trace.csv cargo run -p openttdrs-client
```

Comparar `rail.parts[0].subtile_*` del JSONL (por tick) con
`logical_subtile_*` del CSV en el mismo tick (columnas `tick` + `vehicle`).

Tolerancia recomendada: `0.51` (medio píxel), igual que `parity_diff --subtile-epsilon`.

### Decisiones (Rail 3E)

1. **No portar** `_vehicle_subcoord` completo al render en esta fase: en vías `X`/`Y`
   el eje central coincide con la entrada OpenTTD; en `UPPER`/`LOWER`/`LEFT`/`RIGHT`
   el sprite puede desplazarse ~1 px respecto al original.
2. ~~**Ocultamiento por `_tunnel_visibility_frame`**~~ ✅
   `tunnel_hides_train_at_progress` / `vehicle_hidden_in_tunnel` (ítem 9 cerrado).
3. **Mantener** extrapolación genérica de carretera para trenes: sin stutter medible
   en `train_line` con física Rail 3B.

### Tests de regresión

- `train_render_subtile_consistency` en `parity/report.rs`
- `train_line_divergences_are_absent_after_rail_3b` (incluye consistencia render)
- `train_line_extrapolation_subtile_is_monotonic`
- `sprite_selection_uses_extrapolated_pose_for_train`
- `tunnel_hides_train_matches_visibility_frame`

## Paridad ventanas UI

<!-- fuente: parity/ui_windows_parity.md -->

Fecha: 2026-07-09 · Actualizado tras Fase 1 consist (core + UI MVP).
Compara las ventanas/paneles del cliente Bevy (`openttdrs-client/src/ui/`)
contra las ventanas reales de OpenTTD (`depot_gui.cpp`, `vehicle_gui.cpp`,
`train_gui.cpp`, `order_gui.cpp`, `timetable_gui.cpp`, `build_vehicle_gui.cpp`,
`group_gui.cpp`).

> Este documento profundiza en flota y conserva un snapshot histórico.
> El roadmap global y su baseline actualizado están en
> [ROADMAP_PARIDAD_UI_GLOBAL.md](PLANIFICACION.md#paridad-ui-global).

### Clasificación de cercanía alcanzable

Para cada feature se indica qué tan cerca podemos llegar y qué lo limita:

- **✔** — ya hay paridad funcional (la acción existe y hace lo mismo, aunque
  el layout difiera).
- **A (solo UI)** — alcanzable únicamente tocando el cliente; el comando o el
  dato ya existen en `openttdrs-core`.
- **B (comando chico)** — requiere agregar un comando o campo pequeño en la
  sim, sin cambios estructurales.
- **C (bloqueado por la sim)** — depende de una carencia estructural
  (p. ej. PBS multi-tesela fina, averías/servicio, beneficio por vehículo).
  El **consist ya existe** en core (Fase 1); lo que falta es pulido de UI.

Conclusión: comandos de flota siguen cerca; depósito/compra ya enganchan
vagones (MVP). Falta matriz horizontal con sprites por unidad y drag nativo.

### 1. Ventana de depósito

OpenTTD: `DepotWindow` (`depot_gui.cpp:261-1166`). Cliente:
`ui/toolbar/depot_panel.rs` (ventana flotante `FloatingWindowId::Depot`,
abre con clic en tile `RoadDepot`/`RailDepot`).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Matriz de vehículos con sprites (`WID_D_MATRIX`, `DrawTrainImage`) | Filas de texto (8 slots): nombre, grupo, edad, carga | **A** — dibujar el sprite del vehículo en la fila es solo UI |
| 1 fila = 1 consist (loco + vagones, scroll horizontal) | 1 fila = cabeza; label `[Nu]` unidades | **A** — falta scroll horizontal con sprites por vagón |
| **Drag & drop de vagones** (`MoveRailVehicle`, formar/partir trenes, Ctrl = cadena) | Drag sprites/filas + Ctrl=`move_chain`; clic A→B también | ✔ |
| Ctrl+soltar sobre sí mismo = `ReverseTrainDirection` en depósito | Botón «Dar la vuelta» en ventana de vehículo | ✔ funcional (gesto distinto) |
| Vender arrastrando a `WID_D_SELL` / vender cadena | Zonas drop «Vender»/«Cadena» + ✕ por fila; Ctrl en drop = cadena | ✔ |
| Vender todo (`DepotMassSell`) | Botón «Vender todo» (`SellAllVehiclesAtDepot`) | ✔ |
| Comprar (`WID_D_BUILD` → `BuildVehicleWindow`) | Botón «Nuevos vehículos» → `buy_window` | ✔ |
| Clonar (`CloneVehicle`, Ctrl = compartir órdenes) | Botones «Clonar» (`CloneVehicleAtDepot`) y «Compartir órdenes» separados | ✔ (la variante Ctrl es A) |
| Parar/arrancar todos (`MassStartStop`) | Botones «Parar todos»/«Arrancar todos» (`SetDepotVehiclesRunning`) | ✔ |
| Autoreemplazo masivo (`DepotMassAutoreplace`) | Botones autoreemplazo + regla + «solo viejos» | ✔ (el cliente incluso expone más que la ventana de depósito de OpenTTD) |
| Bandera start/stop por celda | ▶/■ por fila (`ToggleVehicleRunning`) | ✔ |
| Renombrar depósito (`RenameDepot`) | No existe | **B** — falta nombre de depósito en core |
| Tooltip de carga con clic derecho | No existe | A (bajo valor) |
| Lista de vehículos del depósito (`WID_D_VEHICLE_LIST`) | Las 8 filas cumplen ese rol | ✔ parcial (sin scroll: >8 vehículos quedan ocultos → **A**) |
| Ir al tile (`WID_D_LOCATION`) | Botón «Centrar» | ✔ |

Extras del cliente sin equivalente en la ventana de OpenTTD: reordenar slots
(↑/↓), «Copiar órdenes» por fila, ciclo de grupo. No son divergencias: son
azúcar propio.

### 2. Ventana de vehículo (vista)

OpenTTD: `VehicleViewWindow` (`vehicle_gui.cpp:3007-3503`). Cliente:
`ui/vehicle_window.rs` (flotante, abre con clic en el vehículo en el mapa).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Viewport siguiendo al vehículo (`WID_VV_VIEWPORT`, zoom) | Cámara render-target 280×120 (preview real del mundo) | ✔ esencial (seguir con doble clic es A) |
| Barra de estado (`GetVehicleStatusString`): velocidad + destino + «parado» + averiado + atascado | Status corto bajo viewport (#174): Detenido / En marcha a X km/h → destino / Sin ruta / Averiado / PBS | ✔ esencial |
| Start/stop (`StartStopVehicle`) | Icono ▶/■ + tooltip (`ToggleVehicleRunning`) | ✔ |
| Toolbar de iconos (vista) | Fila de iconos + tooltips (#174); Horario/Detalles/Depósito/… | ✔ chrome; sprites GUI nativos OpenTTD opcionales |
| Ir a depósito (`SendVehicleToDepot`, Ctrl = servicio) | «Depósito» (`AppendGotoNearestDepot`) | ✔ funcional; core soporta servicio por intervalo y autoenvío road, pero falta exponer el modificador/acción completa en UI |
| Refit (`ShowVehicleRefitWindow`) | `RefitWindow` lista + coste/cap.; View y Details; parcial por unidad | ✔ (#178); `OrderRefit` sigue **B** |
| Clonar desde la ventana | Solo desde el depósito | **A** |
| Dar la vuelta (`ReverseTrainDirection`/`TurnRoadVehicle`) | «Dar la vuelta» (`TurnAroundVehicle`, solo tren) | ✔ tren; road es **B** |
| Forzar paso (`ForceTrainProceed`) | «Forzar paso» (`ForceVehicleProceed`, solo tren) | ✔ |
| Órdenes / horario (Ctrl) | Botones «Órdenes» y «Horario» separados | ✔ |
| Detalles (`ShowVehicleDetailsWindow`) | Ventana `VehicleDetails` (#173/#175); filas por unidad + sprites | ✔ |
| Ir al destino de la orden (`WID_VV_ORDER_LOCATION`) | Botón «Ir a orden» | ✔ |
| Renombrar (`RenameVehicle`) | Campo de renombrado inline | ✔ |

### 3. Ventana de detalles del vehículo

OpenTTD: `VehicleDetailsWindow` (`vehicle_gui.cpp:2436-3006`) +
`DrawTrainDetails` (`train_gui.cpp:359-471`). Cliente: `ui/vehicle_details_window/`
(`FloatingWindowId::VehicleDetails`, #173/#175) con tabs Info/Carga/Capacidad/Totales
y **una fila por unidad** (sprite lateral + texto según tab; scroll).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Edad + vida útil | Filas Details (Info) + depósito | **A** |
| Beneficio este año / anterior | Resumen tab Totales | ✔ (campo en vehículo) |
| Peso/potencia/esfuerzo tractor (TE) | Peso/potencia por unidad y consist | **A** para peso/potencia; TE es **B** |
| Fiabilidad + nº de averías | Fiabilidad en fila Info; averías no | Fiabilidad ✔; averías **C** |
| Intervalo de servicio (`ChangeServiceInterval`, dropdown días/%/min) | No hay editor UI | La sim soporta intervalo en días o porcentaje, revisión y autoenvío road; falta el comando/editor por vehículo y la opción minutos |
| **Lista de vagones con 4 pestañas** (cargo/info/capacidad/totales por vagón) | Filas con sprite + datos por tab (#175) | ✔ |

Con tren puntual, lo máximo alcanzable hoy es una ventana de detalles de
«una unidad»: edad, peso/potencia, coste, fiabilidad, carga — todo A/B.

### 4. Ventana de órdenes

OpenTTD: `OrdersWindow` (`order_gui.cpp:499-1755`). Cliente:
`ui/toolbar/order_panel/` como **ventana flotante** (`FloatingWindowId::Orders`, #176);
copia local editable + `SetVehicleOrderList`. Se abre desde View / estación /
picker; no ocupa el borde derecho de forma permanente.

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista de órdenes con orden activa | Sí (32 slots, resaltado, marcador `>`) | ✔ |
| Insertar por clic en mapa (`GetOrderCmdFromTile`) | Sí: picker de destino + clic en mapa + `destination_window` | ✔ |
| Skip / delete / reordenar (drag) | Saltar, Borrar, ↑/↓ + drag nativo (#194) | ✔ |
| Full load (variantes any/all) | UI alterna «Carga compl.» all; core y codec `.sav` distinguen all/any/no-load | ✔ básico; falta selector completo en UI |
| Unload / **transfer** / no unload | UI expone «No descargar»; core implementa transfer con feeder share y no-load/no-unload | Falta exponer transfer/no-load y unload forzado en UI |
| **Non-stop / go via** | Core y codec implementan `OrderNonStop`; no hay control UI | Semántica parcial de estaciones intermedias; falta selector y más escenarios oracle |
| Acción de depósito en orden (always/service/halt/unbunch) | UI alterna parada; core soporta servicio si hace falta | ✔ halt/service parcial; unbunch ausente |
| **Refit en orden** (`OrderRefit`) | Refit en orden de depósito (`refit_cargo`) y botón de ciclo | ✔ para depósito; faltan variantes/selección completa por unidad |
| Condicionales (variable+comparador+valor) | Sí, limitado (carga >50 %, salto fijo) | **B** para más variables/comparadores (el core ya tiene `Conditional`) |
| **Stop location de trenes (near/middle/far)** (`MOF_STOP_LOCATION`, doble clic) | Core y codec implementan near/middle/far; sin control UI | ✔ semántica simplificada; falta selector y oráculo por píxel |
| Órdenes compartidas (lista de vehículos, stop sharing) | Crear/desvincular desde depósito; sin lista de compartidos | **A** para la lista; la mecánica ya existe |
| Ir a depósito más cercano (dropdown GOTO) | `AppendGotoNearestDepot` desde ventana vehículo | ✔ |
| Waypoints en órdenes | Sí (solo trenes, sin parada completa) | ✔ |

Extras del cliente: tiempos de espera/viaje editables inline, «Poner en
hora», vaciar lista — equivalentes a piezas de la ventana de horarios de
OpenTTD.

### 5. Ventana de horarios (timetable)

OpenTTD: `TimetableWindow` (`timetable_gui.cpp:174-863`). Cliente:
`ui/timetable_window.rs` (**sí existe**, flotante).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Tiempos de espera/viaje por orden | Sí (8 filas) | ✔ |
| Autofill | Sí | ✔ |
| Reset de retraso (`SetVehicleOnTime`) | «Poner en hora» | ✔ |
| Resumen retraso/adelanto | Sí | ✔ |
| **Velocidad máxima por tramo** | No existe | **B** (campo por orden + clamp en `update_movement_speed`) |
| Fecha de inicio (`SetTimetableStart`) | Core/Command implementados; no expuesto en la ventana | **A/B**: falta control UI y validación de calendario |
| Llegada/salida esperadas por orden | No existe | **A** (derivable de los tiempos) |

### 6. Ventana de refit

OpenTTD: `RefitWindow` (`vehicle_gui.cpp:753-1358`) con selección parcial del
consist por drag. Cliente: `ui/refit_window.rs` (`FloatingWindowId::Refit`).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista de cargas con coste/capacidad | Filas `nombre · cap. N · gratis`; coste real **B** | ✔ UI (#178) |
| Abrir desde View / Details | Botón View + «Refit» en Details | ✔ |
| Selección parcial del consist | Toggle de unidades + `unit_ids` en `RefitVehicle` | ✔ (sin drag nativo) |
| Refit como orden (`OrderRefit`) | No existe | **B** (ver §4) |

### 7. Compra de vehículos

OpenTTD: `BuildVehicleWindow` (`build_vehicle_gui.cpp:1216+`). Cliente:
`ui/buy_window.rs`.

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista con orden asc/desc y ~11 criterios | Orden por nombre/precio/velocidad/año | ✔ básico; más criterios **A** |
| Matriz con sprite por fila | Sprite + nombre/precio por fila (#179) | ✔ chrome; preview grande sigue abajo |
| Filtro por cargo / texto / motores ocultos / badges | Filtro todos/buses/camiones (solo road); rail lista loco+vagón | **A** |
| Panel de detalle (coste, peso, velocidad, potencia, **TE**, running cost, refit) | Sí salvo TE | ✔ (TE es **B**) |
| **Comprar vagones** (`CcBuildWagon` acopla a la loco) | Compra `ENGINE_WAGON_*` + auto-`AttachWagonToConsist` | ✔ MVP |
| Ocultar/renombrar motor (`SetVehicleVisibility`, `RenameEngine`) | No existe | **B**, bajo valor |

### 8. Lista de vehículos y grupos

OpenTTD: `VehicleListWindow` (`vehicle_gui.cpp:1923-2319`) y
`VehicleGroupWindow` (`group_gui.cpp:208-1244`). Cliente: **`VehicleList`
existe** (UI-2, `vehicle_list.rs` / `FloatingWindowId::VehicleList`) con filtro
por tipo y acciones básicas. Grupos dedicados (`VehicleGroupWindow`) siguen
parciales (ciclo de grupo en depósito + HUD).

- Ventana de lista de flota con ordenamiento y acciones masivas
  (`MassStartStop`, enviar todos a depósito): **A/B** — los comandos masivos
  por depósito ya existen; falta la vista global y un `SendAllToDepot`.
- Ventana de grupos (crear/renombrar/borrar, drag de vehículos): **B** — el
  core ya tiene `CreateVehicleGroup`/`AssignVehicleToGroup`; faltan renombrar
  y borrar grupo.

### 9. Auditoría layout entidad (#179)

Checklist OpenTTD vs openttdrs vs acción (epic UI-Layout #172).

| Superficie | OpenTTD | openttdrs | Acción |
|---|---|---|---|
| Estación | `station_gui` viewport + iconos | Panel fijo; barra Ruta/Órd./Loc… (#183) | ✔ chrome; viewport **A** residual |
| Industria | `industry_gui` | `FloatingWindowId::Industry` + preview RT + Loc | ✔ chrome (#179); Authority/catchment **B/C** |
| Pueblo | `town_gui` viewport + iconos | Flotante; barra Loc/Pub/Fondos | ✔ chrome (#179); Authority completa **B** |
| Compra | matriz sprites | Filas con sprite + stats | ✔ chrome (#179); TE/ocultar motor **B** |
| Lista flota | sprites + mass actions | Filas con sprite + start/stop (#182) | ✔ chrome; grupos/mass **A/B** |

Parcial/OOS: callbacks NewGRF completos, cheats, multi-instance y UI completa de servicio/averías. El core ya modela fiabilidad, averías e intervalos.

### Resumen: qué tan cerca podemos llegar

| Categoría | Ítems | Veredicto |
|---|---|---|
| Ya en paridad funcional (✔) | start/stop, vender, vender todo, clonar, autoreemplazo, comprar, órdenes básicas + condicionales + skip + reorden, waypoints, horarios con autofill, reversa/forzar paso de tren, centrar/ir a destino, renombrar vehículo | La mecánica de comandos está prácticamente completa para vehículos puntuales |
| Alcanzable solo con UI (A) | sprites en filas de depósito, scroll >8 vehículos, string de estado con destino, edad/peso/potencia en detalles, ventana de refit con lista, lista de órdenes compartidas, drag para reordenar órdenes, llegada/salida esperadas, más criterios de orden en compra, clonar desde ventana de vehículo | Un paquete de trabajo de cliente sin tocar core |
| Comando chico en core (B) | unload forzado, más condicionales, velocidad máx. por tramo de horario, renombrar depósito/grupo, TE en `EngineDef`, dar la vuelta para road | Cambios acotados; transfer, full-load variants, refit de depósito y timetable-start ya existen en core |
| Requiere ampliar la sim (C) | PBS/reservas finas, unbunch y paradas intermedias avanzadas | Consist, servicio/averías y beneficio por vehículo ya no bloquean; falta profundidad y UI |

**El techo actual**: Fase 1 desbloqueó consist en core y un MVP de UI
(compra+enganche, reorden clic A→B, render de trailers, venta de cadena).
El salto siguiente es pulido A (matriz horizontal, drag nativo, pestañas por
vagón) más Fases 2–3 de sim (packets, PBS).

### Orden recomendado si se ataca la UI

1. Paquete A de depósito + vehículo (sprites en filas, scroll, string de
   estado, detalles con edad/peso/potencia) — máxima paridad visible sin
   tocar core.
2. Exponer en UI lo ya presente en core (full-load any/no-load, transfer,
   non-stop, stop-location y timetable-start) y completar unload forzado.
3. Ventana de flota + grupos (A/B) — único subsistema de gestión ausente.
4. Afinar stop-location y consist contra oráculos más largos; la estructura
   base de ambos ya existe.

### Tests hoy y huecos

Cubierto: labels de órdenes (`order_row_labels_depots`), sync del panel de
órdenes, pick de destino, añadir estación a ruta, conversión km/h, drag de
ventanas flotantes. Sin tests: `depot_panel`, `buy_window`,
`destination_window`, `timetable_window` y los handlers de botones de la
ventana de vehículo — si se encara el paquete A, agregar tests de sync/handler
por ventana al estilo `setup_order_panel_then_sync_order_panel`.

## Inventario rutas UI

<!-- fuente: parity/ui_route_inventory.md -->

Checklist versionado de superficies de UI. Los conteos deben coincidir con
`FloatingWindowId::ALL` / `BuildMenuAction::ALL` / etc. (test
`ui_enum_inventory_counts`).

**Fecha:** 2026-07-17 · **FloatingWindowId:** 43 · **BuildMenuAction:** 66 ·
**SaveMenuAction:** 22 · **ToolbarGroup:** 8

### Ventanas flotantes (`FloatingWindowId`)

| Id | Apertura típica | Notas |
|----|-----------------|-------|
| Town | clic pueblo / menú | Chrome compacto (#179) |
| TownDirectory | menú Info / `UiRoute` | |
| IndustryDirectory | menú Info | |
| Industry | clic industria | Viewport RT + Loc (#179) |
| StationDirectory | menú Info | |
| VehicleList | menú Info / flota | |
| SubsidyList | menú Economía | |
| Depot | clic depósito | |
| BuyVehicle | depósito → comprar | |
| Vehicle | clic vehículo / depósito | Vista (`VehicleView`) |
| VehicleDetails | View → Detalles | Tabs Info/Carga/Capacidad/Totales (#173) |
| RailStationPicker | herramienta estación rail | |
| AirportPicker | herramienta aeropuerto | |
| BridgePicker | tras tramo de puente | |
| DestinationPicker | órdenes → destino | |
| NewsHistory | barra de noticias | |
| Finances | menú Economía | |
| NewsSettings | Ajustes | |
| PathfindingSettings | Ajustes | |
| CargoDistSettings | Ajustes | Manual / Asimétrica / Simétrica |
| AiSettings | Ajustes / Finanzas «IA…» | |
| NewGrf | Ajustes | |
| SoundMusic | toolbar audio | |
| Timetable | vehículo / F4 | |
| Orders | View → Órdenes / estación (#176) | Flotante; ya no dock fijo |
| Refit | depósito | |
| SharedOrders | vehículo | |
| Autoreplace | depósito / flota | |
| Graphs | menú Economía | |
| CargoPaymentRates | menú Economía | |
| DisplayOptions | Ajustes | |
| ExtraViewport | Ajustes | |
| SignList | menú Info | |
| LinkGraphLegend | menú Economía | |
| SignalPicker | herramienta señales | |
| Help | Ajustes / F1 | |
| DevConsole | Ajustes / F3 | |
| TileInspector | Ajustes / F2 | |
| CheatWindow | Ajustes / Ctrl+Alt+C | |
| GenLand | Editor → Terreno | |
| Goals | menú Economía | |
| Story | menú Mundo | |
| League | menú Economía | |

### Paneles no flotantes (fijos)

| Superficie | Apertura |
|------------|----------|
| StationCargoPanel | clic estación |
| SaveWindow | Guardar/Cargar |
| Minimap | HUD |
| Build toolbar groups | barra superior |

### Toolbar

- **ToolbarGroup (8):** Rail, Road, Water, Air, Economy, Landscape, Info, Settings
- **BuildMenuAction (66):** ver `BuildMenuAction::ALL` en `toolbar/mod.rs`
- **SaveMenuAction (18):** ver `SaveMenuAction::ALL`

### Mantenimiento

1. Añadir variante al enum.
2. Actualizar `ALL` y este checklist.
3. Ajustar constantes en `ui_enum_inventory_test.rs`.
4. `cargo test -p openttdrs-client --bin openttdrs-client ui_enum_inventory`.

## Entrada vehículo–estación

<!-- fuente: parity/vehicle_station_entry.md -->

Contrasta la timeline generada por `parity_runner --scenario truck_bay` con las
observaciones de los videos de referencia:

- `openttd.webm` — comportamiento esperado (OpenTTD original).
- `opentddrs.webm` — estado actual del cliente Rust + Bevy.

### Observaciones de los videos

Del video de OpenTTD (camión aproximándose a una bahía de carga):

1. **Desaceleración por curvas**: la velocidad cae 48 → 33 → 31 km/h en las dos
   curvas de 90° previas a la bahía (penalización de −25 % por giro, dos giros
   próximos). Recupera al salir de cada curva.
2. **Detención DENTRO de la bahía**: el camión entra a la tesela de la estación
   y frena en el frame de parada (tabla `_road_stop_stop_frame`, frames 11–20),
   quedando visualmente dentro de la dársena.
3. **Movimiento continuo**: sin saltos; el sprite cambia de orientación en
   sincronía con la trayectoria curva pixel a pixel.

Del video de openttdrs:

1. La velocidad NO baja en las curvas.
2. El camión se detiene en la carretera frente a la parada (nunca entra a la
   tesela de la estación).
3. El movimiento presenta tirones y la orientación del sprite cambia tarde
   respecto a la posición dibujada en las curvas.

### Timeline del runner (traza tras la Fase 2, 500 ticks)

Camión id 1, motor MPS (velocidad interna máx. 96 = 48 km/h). Ruta con dos
curvas de 90° y bahías `TruckStop` en ambos extremos.

| Tick | Evento / estado | Detalle |
|---|---|---|
| 1 | `start` | arranca desde parado (aceleración AM_ORIGINAL) |
| 35 | primer `tile_crossed` | aún acelerando (≈14 ticks/tesela) |
| 90, 130 | `direction_changed` (curvas 90°) | **velocidad 96→72** (−25 %, Fase 2) y recupera acelerando |
| 168 | `tile_crossed` + `station_entry` | **entra a la tesela de la bahía** (4,5) desde el acceso (4,6) (Fase 2) |
| 169 | `loading_started` + `loading_finished` + `order_advanced` | carga 0→20 **en un solo tick** (OpenTTD: gradual — pendiente) |
| 170–178 | `depart_turn_started` … `depart_turn_ended` | media vuelta animada dentro de la bahía |
| 238, 277 | curvas de vuelta | con penalización −25 % en cada giro |
| 315–316 | `station_entry` + `unloading_started/finished` | descarga gradual dentro de la bahía destino |
| 462–463 | segundo ciclo de carga | el ciclo es estable y determinístico |

### Qué divergencia del reporte explica cada diferencia visual

| Diferencia visual (videos) | Divergencia (`docs/parity/divergences_found.md`) | Estado |
|---|---|---|
| No frena en las curvas (48 km/h constantes vs 48→33→31) | `curve_speed_penalty` | **CORREGIDA en Fase 2**: `Vehicle::set_direction_with_curve_penalty` aplica −25 % en cada giro (ticks 90/130/238/277 de la traza: 96→72) |
| Se detiene fuera de la dársena | `bay_stop_position` | **CORREGIDA en Fase 2**: el destino es la tesela de la bahía; carga con el camión en (4,5). El render sigue las tablas exactas `_rv_station_left_*` (entrada por la boca, lazo, parada en el stop frame 11–20 y salida), validadas punto a punto por el golden |
| La pausa de carga parece un frenazo inmediato | `instant_loading` — ya gradual por tick (`load_unload_speed`) | Resuelto (regresión en reportes) |
| Tirones / baja fluidez general | `tick_rate` — sim ~37 Hz, `REFERENCE_PROGRESS_STEP=112` | Resuelto — ver `docs/PARIDAD.md` / ADR 0003 |
| Sprite gira tarde en las curvas | corregido en Fase 1: el selector de textura ahora usa la pose extrapolada (`render/vehicles.rs::for_vehicle`); antes usaba `v.render_direction()` lógico | test `sprite_selection_uses_extrapolated_pose_not_logical_direction`; verificable con `OPENTTDRS_RENDER_TRACE` |

### Cómo verificar la parte visual (render vs sim)

```bash
OPENTTDRS_RENDER_TRACE=/tmp/render_trace.csv cargo run -p openttdrs-client
```

El CSV registra por frame: pose lógica (tesela + progress del último tick de
sim), pose extrapolada (lo que se dibuja), `tick_alpha` y `sprite_dir`. Si la
columna extrapolada avanza suave mientras la lógica salta cada ~27 ms, la
extrapolación solo suaviza el render; el tick lógico ya está a ~37 Hz.

## Divergencias train line

Reporte regenerable: [`parity/train_line_divergences.md`](parity/train_line_divergences.md) (`./scripts/regenerate_parity_reports.sh`).


## Divergencias encontradas

Reporte regenerable: [`parity/divergences_found.md`](parity/divergences_found.md) (`./scripts/regenerate_parity_reports.sh`).


## CargoDist MCF

<!-- fuente: parity/cargodist_mcf_parity.md -->

**Estado:** implementado en `openttdrs-core::linkgraph_parity`  
**MVP previo:** #49 (Manual + stub `CapacityScaled`)  
**Seguimiento:** [#102](https://github.com/cavazquez/openttdrs/issues/102) ✅ cerrado  
**LGRP MVP:** load/save del grafo observado (`sav/linkgraph.rs`) ✅ — `LGRJ`/`LGRS` vacíos (OOS consciente).  
**Overlay mapa:** ✅ gizmos al abrir Link Graph o con «Overlay Link Graph» en Opciones de visualización.  
**Dumps C++ byte-igual (MCF):** fixtures JSON en `tests/fixtures/linkgraph/*.json` (`OPENTTD_DUMP_LINKGRAPH=1`).  
**Dumps C++ byte-igual (LGRP wire):** `lgrp_empty.bin` / `lgrp_two_node_goods.bin` (`OPENTTD_DUMP_LGRP=1`).

### Pipeline

1. **Ingesta** (`from_game`): nodos `supply` (waiting) / `demand` (acceptance); aristas `capacity` / `usage` / `travel_time` desde `LinkGraphStats`.
2. **DemandCalculator** — Asymmetric / Symmetric reales (geografía + supply; no espejo de aristas).
3. **MCF1** — Dijkstra `DistanceAnnotation` + `FlowMapper(false)` + eliminación de ciclos.
4. **MCF2** — Dijkstra `CapacityAnnotation` + `FlowMapper(true)` + scale mensual.
5. **GetVia** — `RandomRange` sobre shares con `Randomizer` alineado a OpenTTD (`core/random_func`).

El stub BFS en `mcf.rs` queda **legado** (solo tests de regresión). El camino de juego (`GameState::rebuild_station_flows`) usa el pipeline nuevo.

### Fixtures

`crates/openttdrs-core/tests/fixtures/linkgraph/`:

| Fixture | Escenario |
|---------|-----------|
| `asymmetric_two_node` | 1-hop Asymmetric |
| `symmetric_mirror_nodes` | Symmetric Demand |
| `three_node_linear` | 2-hop |
| `three_node_cycle` | ciclo dirigido |
| `express_vs_local` | express vs local (travel_time) |

Tests: `linkgraph_parity_fixtures` (demands + flows byte-igual; golden GetVia 16 draws + checksum 10k).

#### Oráculo C++

Harness Catch2: `OpenTTD/src/tests/linkgraph_parity_fixtures.cpp`.

```bash
cd OpenTTD/build
cmake .. -GNinja -DOPTION_DEDICATED=ON -DCMAKE_BUILD_TYPE=RelWithDebInfo
ninja openttd_test
OPENTTD_DUMP_LINKGRAPH=1 ./openttd_test "[linkgraph][parity]"
## Pegar cada bloque ===DUMP name=== en
## openttdrs/crates/openttdrs-core/tests/fixtures/linkgraph/<name>.json
```

Nota de paridad: `Path::GetCapacityRatio` en OpenTTD hace `(int * 16) / uint`; con `free < 0` el cociente se promociona a unsigned y el ratio queda enorme positivo. MCF2 usa eso al sobrecargar aristas (`express_vs_local`: via express 60 / local 40).

#### Oráculo LGRP (bytes del chunk)

Harness Catch2: `OpenTTD/src/tests/lgrp_byte_fixtures.cpp` (serializa el grafo en memoria con el layout de `GetLinkGraphDesc`).

```bash
cd OpenTTD/build
OPENTTD_DUMP_LGRP=1 ./openttd_test "[linkgraph][lgrp]"
## Guardar hex → tests/fixtures/linkgraph/lgrp_*.bin
```

Asserts en `sav/linkgraph.rs` (`lgrp_*_matches_openttd_dump`). `LGRJ`/`LGRS` quedan fuera del golden (chunks aparte; Rust los emite vacíos).

### Manual

`DistributionType::Manual` sigue resolviendo `next_hop` solo desde órdenes (sin MCF).

## Referencia OpenTTD (clon/pin)

<!-- fuente: parity/OPENTTD_REFERENCE.md -->

Fuente de verdad machine-readable: [`openttd-reference.json`](parity/openttd-reference.json).

Todos los flujos de paridad / extractores / lectura de C++ deben usar el **mismo commit**. No clonar `master` ni hacer `pull` a HEAD móvil.

### Uso

```bash
./scripts/fetch-openttd-reference.sh
git -C reference/openttd-upstream rev-parse HEAD   # debe == commit del manifiesto
```

El script imprime tag, SHA, URL y licencia. Overrides opcionales (solo depuración):

- `OPENTTD_UPSTREAM_URL`
- `OPENTTD_UPSTREAM_COMMIT` (debe ser SHA completo de 40 hex)

### Actualizar la referencia (deliberado)

1. Elegí un tag/release o SHA estable de [OpenTTD/OpenTTD](https://github.com/OpenTTD/OpenTTD).
2. Actualizá `commit`, `tag` y `pinned_at` en `openttd-reference.json`.
3. Corré `./scripts/fetch-openttd-reference.sh` y verificá el SHA.
4. Revisá impacto en docs/parity, extractores Python y citas `archivo:línea`.
5. Abrí un PR que mencione el SHA anterior → nuevo y el motivo (API upstream, bugfix, release).

No regenerar goldens “de pasada” sin documentar el cambio de referencia.

### Oráculo / fork auxiliar

`scripts/setup_openttd_oracle_fork.sh` también clona el commit del manifiesto (no HEAD).

### Licencia

OpenTTD es **GPL-2.0-only** (`license_spdx` en el manifiesto). El clon vive en `reference/openttd-upstream/` (gitignored); no se vendoriza en este repo.

## Esquema snapshot oráculo

<!-- fuente: parity/SNAPSHOT_SCHEMA.md -->

Contrato compartido por:

- **Candidato:** `cargo run -p openttdrs-core --bin snapshot_dumper`
- **Oráculo:** export C++ en OpenTTD pin (#109) vía `OPENTTDRS_SNAPSHOT_OUT` ([`patches/openttd-15.3-snapshot-export/`](../../patches/openttd-15.3-snapshot-export/))

`schema_version`: **1**

### Campos

| Campo | Tipo | Notas |
|-------|------|--------|
| `schema_version` | int | Siempre `1` |
| `producer` | string | `"openttd"` (oráculo) o `"openttdrs"` (candidato) |
| `openttd_commit` | string | SHA del manifiesto (#109); oráculo lo rellena; candidato puede ir vacío |
| `source_path` | string | Path de entrada (`.sav` / `.ottdmap`) |
| `map.width` / `map.height` | int | Dimensiones |
| `map.tile_count` | int | `width * height` |
| `map.tile_kind_counts` | object | Conteos por nombre de `TileKind` |
| `map.min_height` / `max_height` | int | Extremos de altura |
| `hashes.*_fnv1a64` | string hex 16 | FNV-1a 64-bit, orden de tiles `(y,x)` fila-mayor |
| `extras.*` | int | Solo candidato `.ottdmap`; oráculo pone `0` |
| `components.industry_components` | int | Componentes 4-conectados `Industry` |
| `components.station_components` | int | Idem `Station` (no aeropuerto) |

### Hashes (orden de bytes)

Recorrido: `for y in 0..height { for x in 0..width }`

- `height`: 1 byte `TileHeight`
- `kind`: 1 byte código (ver `snapshot_dumper` / `KindCode` C++)
- `mapt`: 1 byte `tile.type()` completo (MAPT)
- `rail_bits` (solo kind Rail=4): `m5&0x3F`, `m3`, `m4` (= m3hi ottdmap)
- `road_bits` (solo kind Road=3): `m5&0x0F`, `m8` u16 LE

Offset FNV: `0xcbf29ce484222325`, prime `0x100000001b3`.

### Comparación

```bash
python3 scripts/compare_snapshots.py oracle.json candidate.json
```

Campos hard: dimensiones + 5 hashes + 2 component counts.  
`extras` **no** se comparan (el oráculo no tiene footers ottdmap).

## Primera divergencia snapshot

<!-- fuente: parity/SNAPSHOT_FIRST_DIVERGENCE.md -->

Fixture: `tests/fixtures/stationlist-test.sav`  
Oráculo: OpenTTD **15.3** (`14ec60f248547d4d062a1160f0fc26d742319888`) + [`patches/openttd-15.3-snapshot-export/`](../../patches/openttd-15.3-snapshot-export/)  
Candidato: `parse_sav.py` → `snapshot_dumper`  
Artefacto: [`crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json`](../../crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json)

```bash
./scripts/export_openttd_oracle_snapshot.sh \
  tests/fixtures/stationlist-test.sav \
  crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json
python3 scripts/parse_sav.py tests/fixtures/stationlist-test.sav /tmp/cand.ottdmap
cargo run -p openttdrs-core --bin snapshot_dumper -- /tmp/cand.ottdmap /tmp/cand.json
python3 scripts/compare_snapshots.py \
  crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json /tmp/cand.json
```

### Estado: resuelta

Tras aplicar en `parse_sav.py` / `sav::build` la migración `SLV_ROAD_TYPES` (save &lt; 214), el comparador reporta **OK** en campos hard.

| Campo | Valor |
|-------|--------|
| `map.width` / `height` | 256 × 256 |
| `hashes.height_hash_fnv1a64` | `491f3424ae6844b5` |
| `hashes.mapt_hash_fnv1a64` | `4298ad417a195769` |
| `hashes.kind_hash_fnv1a64` | (igual tras alinear KindCode a `ottd_tile_kind`) |
| `hashes.rail_bits_hash_fnv1a64` | `d0a3931867272a40` |
| `hashes.road_bits_hash_fnv1a64` | `cc1c08d5ec5b4d7f` |
| `components.industry_components` | 73 |
| `components.station_components` | 8 |

### Causa raíz (histórica)

**`hashes.road_bits_hash_fnv1a64`** divergía porque el oráculo hashea `m8` post-`AfterLoadGame` y el candidato copiaba `MAP8` crudo (todo 0 en este save v211).

En saves &lt; 214, OpenTTD mueve el RoadType desde bits 6–7 de `m7` a `m4` (road) y `m8` bits 6–11 (tram). Sin tram: `m8 = INVALID_ROADTYPE << 6` (`0xFC0`).

### Notas

- Saves sintéticos (`rail_signals_mixed.sav`, `demo_openttd.sav`) no cargan en 15.3 (`MAPS` ya no es RIFF simple).
- Dedicated + `-g` dispara dos `AfterLoadGame` (new-game luego load); el export usa `OPENTTDRS_SNAPSHOT_MIN_CALL=2`.
- El oráculo **no** invoca `parse_sav.py` ni `snapshot_dumper`.

## Tablas generadas

<!-- fuente: parity/GENERATED_TABLES.md -->

Inventario y verificación de reproducibilidad de `*_generated.rs`.

### Fuente de verdad

| Artefacto | Rol |
|-----------|-----|
| [`scripts/generated_tables_manifest.json`](../../scripts/generated_tables_manifest.json) | Inventario + pilots + `output_sha256` |
| [`docs/parity/openttd-reference.json`](parity/openttd-reference.json) | Pin OpenTTD (#109) |
| [`scripts/check_generated_tables.py`](../../scripts/check_generated_tables.py) | Orquestador `--check` |

### Pilots (verificados en CI)

| id | Generador | Check |
|----|-----------|-------|
| `house_population` | `gen_house_population.py` | Regenera vs `town_land.h` del pin; si no hay upstream, `output_sha256` |
| `house_draw_data` | `gen_house_draw_data.py` | Solo `output_sha256` (OpenGFX no vendorizado) |
| `vehicle_gfx_data` | `gen_vehicle_gfx_data.py` | Solo `output_sha256`; `--check` local con PNG |
| `tile_atlas` | `gen_tile_atlas.py` | Solo `output_sha256` del `.rs`; `--check` no escribe PNG |

Los generadores OpenGFX tienen `--check` (exit 2 si faltan assets). Tras regenerar con el set local, actualizá `output_sha256` en el manifiesto (PR de datos de render).

OpenGFX (`assets/opengfx/tiles/`) **no** está vendorizado ni se descarga en CI.

### Comandos

```bash
python3 scripts/check_generated_tables.py --list
./scripts/fetch-openttd-reference.sh   # para house_population regen
python3 scripts/check_generated_tables.py --check
python3 scripts/check_generated_tables.py --check --fetch-upstream   # CI

## Regenerar (escribe)
python3 scripts/gen_house_population.py
python3 scripts/gen_house_draw_data.py
python3 scripts/gen_vehicle_gfx_data.py
python3 scripts/gen_tile_atlas.py   # también reescribe assets/opengfx/atlas/*.png
```

Tras regenerar un piloto con `check: hash`, actualizá `output_sha256`:

```bash
sha256sum crates/openttdrs-client/src/sprites/<archivo>_generated.rs
```

### Licencia

Derivados de headers OpenTTD: **GPL-2.0-only** (ver pin). Offsets/PNG OpenGFX quedan fuera del árbol git.

### Extensión

Nuevas tablas: añadir al `inventory`; con `--check` + hash estable → `pilots`.

## Workflow snapshot oráculo

<!-- fuente: SNAPSHOT_ORACLE_WORKFLOW.md -->

Hay **dos productores independientes**:

| Rol | Productor | Entrada |
|-----|-----------|---------|
| **Oráculo** | OpenTTD C++ (commit pin [#109](parity/openttd-reference.json)) | `.sav` |
| **Candidato** | `parse_sav.py` + `snapshot_dumper` (openttdrs) | `.sav` → `.ottdmap` |

Esquema: [parity/SNAPSHOT_SCHEMA.md](#esquema-snapshot-oráculo).

> **No es oráculo** el flujo antiguo que envolvía `parse_sav.py` dentro de un “fork” OpenTTD: ambos lados usaban el parser bajo prueba. Ese script quedó reemplazado.

### 1) Oráculo (OpenTTD real)

```bash
./scripts/fetch-openttd-reference.sh
./patches/openttd-15.3-snapshot-export/integrate.sh
cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON
cmake --build reference/openttd-upstream/build -j

./scripts/export_openttd_oracle_snapshot.sh \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/openttd.oracle.json
```

El JSON debe tener `"producer": "openttd"`.

### 2) Candidato (openttdrs)

```bash
python3 scripts/parse_sav.py \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/candidate.ottdmap
cargo run -p openttdrs-core --bin snapshot_dumper -- \
  /tmp/candidate.ottdmap /tmp/openttdrs.candidate.json
```

### 3) Comparación

```bash
python3 scripts/compare_snapshots.py \
  /tmp/openttd.oracle.json \
  /tmp/openttdrs.candidate.json
```

La primera divergencia se imprime y el exit code es `1`.  
Mutación sintética (CI local):

```bash
python3 scripts/test_compare_snapshots_mutation.py
```

### Spike / fixtures

- Save pequeño versionado: `crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav`
- Ottdmap 2×2 (solo candidato): `m3_road_tram_2x2.ottdmap`

Sin binario OpenTTD compilado el paso 1 no corre; el resto del tooling y el parche sí están versionados.

### Trazas PBS por tick

Este workflow compara snapshots de mapa al cargar un save. Las reservas PBS son
dinámicas y usan un productor separado: [PBS_EXTERNAL_ORACLE.md](#oráculo-pbs-externo).
El parche comparte integración, pero el export PBS emite JSONL post-tick y
finaliza automáticamente tras el número de filas solicitado.

## Oráculo PBS externo

<!-- fuente: PBS_EXTERNAL_ORACLE.md -->

El golden `train_pbs_golden.json` y la traza
`train_pbs_tick_golden.json` son regresiones **internas** de openttdrs. Este
documento define el segundo productor independiente: OpenTTD 15.3, fijado en
[`parity/openttd-reference.json`](parity/openttd-reference.json).

### Contrato JSONL v1

El parche `patches/openttd-15.3-snapshot-export/` añade un exportador que se
activa con:

```bash
OPENTTDRS_PBS_TRACE_OUT=/tmp/openttd-pbs.jsonl \
OPENTTDRS_PBS_TRACE_TICKS=40 \
./reference/openttd-upstream/build/openttd -D -g partida.sav
```

La primera fila es metadata (`producer: "openttd"`). Sigue una muestra
`initial` tras cargar el save y antes de avanzar; las filas `tick` se capturan
después de `StateGameLoop`:

```json
{"kind":"initial","tick":122,"trains":[{"vehicle":17,"x":2,"y":2,"progress":51,"speed":73,"subspeed":52,"direction":1}],"rail_reservations":[{"x":3,"y":2,"track_bits":1}]}
```

Los IDs de vehículo son locales a cada motor y el comparador no los usa. El
contrato compara la colección ordenada de `(x, y, progress, speed, subspeed,
direction)` de trenes y las reservas `(x, y, track_bits)` por muestra.
`track_bits` corresponde a
`GetRailReservationTrackBits` en OpenTTD y a la reserva `m2_hi` decodificada
en openttdrs.

### Contrato JSONL v2

`schema_version: 2` añade `units[]` por cabeza de tren (recorrido `Next()`),
sin romper fixtures v1: el comparador solo exige unidades cuando el oráculo
las declara.

Cada unidad exporta `index` (0 = cabeza), tile `x`/`y`, `rail_pixel` (misma
convención que `rail_pixel_from_openttd_pos` en Rust) y `direction`. La cabeza
conserva los campos v1 de velocidad/progreso.

```json
{"kind":"initial","tick":4685,"trains":[{"vehicle":2,"x":46,"y":37,"progress":51,"speed":73,"subspeed":52,"direction":1,"units":[{"index":0,"x":46,"y":37,"rail_pixel":5,"direction":1},{"index":1,"x":47,"y":37,"rail_pixel":13,"direction":1},{"index":2,"x":47,"y":37,"rail_pixel":5,"direction":1}]}],"rail_reservations":[{"x":43,"y":37,"track_bits":1}]}
```

### Generación reproducible

1. Obtener e integrar OpenTTD 15.3:

   ```bash
   ./scripts/fetch-openttd-reference.sh
   ./patches/openttd-15.3-snapshot-export/integrate.sh
   cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON
   cmake --build reference/openttd-upstream/build -j
   ```

2. El fixture versionado
   `crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav` fue creado desde
   **OpenTTD 15.3**. Tiene un tren, una path signal eléctrica unidireccional,
   una recta y una estación de destino. No incluye NewGRF, cruces ni más
   vehículos. Para reemplazarlo, validar primero:

   ```bash
   ./scripts/validate_sav_openttd.sh crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav
   ```

3. Exportar el oráculo y el candidato desde **el mismo save**:

   ```bash
   ./scripts/export_openttd_pbs_trace.sh \
     crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav \
     crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl 40

   cargo run -p openttdrs-core --bin sav_pbs_runner -- \
     crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav \
     --ticks 40 --out /tmp/train_pbs_openttdrs.jsonl
   python3 scripts/compare_pbs_traces.py \
     crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl \
     /tmp/train_pbs_openttdrs.jsonl
   ```

### Viewer de traza (tiles + reservas + señal)

Para ver por dónde pasa el tren y qué reservas PBS hay en cada muestra:

```bash
## Oráculo corto (paridad, 40 ticks)
python3 scripts/view_pbs_trace.py \
  crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_openttd.jsonl \
  /tmp/pbs_trace.html

## Recorrido completo del mismo save (400 ticks → estación destino)
python3 scripts/view_pbs_trace.py \
  crates/openttdrs-core/tests/fixtures/parity/train_pbs_15_3_400_openttd.jsonl \
  /tmp/pbs_trace_400.html
## HTML versionado: docs/parity/train_pbs_15_3_400_trace.html
```

Abre el HTML en el navegador: scrubber de muestras, mapa del corredor,
lista de tiles visitados y anotación de señal (`--signal X,Y,label`; el
fixture `train_pbs_15_3` usa por defecto la path signal en `(46,37)`).

La traza de **400 ticks** es solo para inspección visual (path
`47,37 → … → 42,37`). El golden de paridad sigue siendo la de **40 ticks**.

### Estado

El exportador, normalizador, validador y comparador están implementados y el
exportador fue compilado contra el commit OpenTTD 15.3 fijado. El fixture y su
oráculo de 40 ticks están versionados.

**Paridad cerrada** para este escenario (un tren, path signal, `AM_REALISTIC`):
`initial` y los 40 ticks coinciden en tesela, `progress` físico, `cur_speed`,
`subspeed`, dirección y reservas PBS (`tests/pbs_openttd_oracle.rs`).

#### Fixture dual (curva + PBS + plataformas)

- Save: `crates/openttdrs-core/tests/fixtures/train_dual_pbs_curve_15_3.sav`
- Oráculo: `tests/fixtures/parity/train_dual_pbs_curve_15_3_openttd.jsonl`
- Tests: `tests/pbs_dual_curve_oracle.rs`

Contenido: 2 trenes Ginzu A4, 2 estaciones duales, path / path-oneway, curva en
`(25–26, 8)`, depósito `(24, 9)`. **Paridad cerrada** (`initial` + 40 ticks:
cinemática y reservas PBS) en `tests/pbs_dual_curve_oracle.rs`.

#### Fixture multi-vagón (consist + PBS, schema v2)

- Save: `crates/openttdrs-core/tests/fixtures/train_consist_2wagon_pbs_15_3.sav`
- Oráculo: `tests/fixtures/parity/train_consist_2wagon_pbs_15_3_openttd.jsonl`
- Tests: `tests/consist_pbs_openttd_oracle.rs`

Contenido: locomotora Ginzu A4 + 2 Goods Van sobre la recta PBS de
`train_pbs_15_3`, sin NewGRF. La cola ocupa otra tesela/píxel que la cabeza.
Regeneración:

```bash
./scripts/gen_consist_2wagon_fixture.sh 40
```

El generador engancha vagones en AfterLoad (`OPENTTDRS_FIXTURE_ATTACH_WAGONS`)
sobre `train_pbs_15_3.sav`, guarda el `.sav` materializado y exporta el JSONL
desde ese save (sin re-enganchar).

Contrato rail (solo trenes):

- `DoUpdateSpeed` devuelve distancia (`GetAdvanceSpeed` + remanente).
- Umbral `GetAdvanceDistance` (192 axial / 256 corner); sobrante en `progress`.
- Un tick de juego = 2× `TrainLocoHandler`; 16 pasos de píxel por tesela.
- Aceleración realista al importar `.sav` (`train_acceleration_model = Realistic`).
- Render: `rail_pixel / 16` → progreso visual 0..=255 (no usar el remanente físico).
- Import: no teletransportar vehículos ya sobre su red aunque YAPF falle (path signal).

## Oráculo Airport FTA

<!-- fuente: AIRPORT_FTA_ORACLE.md -->

Traza JSONL de aviones normales + bloques de aeropuerto, producida por
OpenTTD 15.3 parcheado (`patches/openttd-15.3-snapshot-export/`) y comparada
con `openttdrs-core`.

### Contrato JSONL v1

```bash
OPENTTDRS_AIRPORT_FTA_TRACE_OUT=/tmp/openttd-airport-fta.jsonl \
OPENTTDRS_AIRPORT_FTA_TRACE_TICKS=80 \
./reference/openttd-upstream/build/openttd -D -g partida.sav
```

Filas: `metadata` → `initial` → N× `tick`. Cada muestra lleva `aircraft[]`
(`pos`, `previous_pos`, `state`=heading, tile/pixel, speed) y `airports[]`
(`type`, `blocks`, footprint).

### Fixture Helidepot

- Save: `crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav`
- Oráculo: `tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl`
- Tests: `tests/airport_fta_openttd_oracle.rs`

2× Helidepot + 1 Tricario A↔B. El `initial` coincide; tras ~14 ticks el
heading puede adelantarse un tick respecto a OpenTTD (dwell FTA no persistido
en el `.sav`).

### Regenerar

```bash
./patches/openttd-15.3-snapshot-export/integrate.sh
cmake --build reference/openttd-upstream/build -j --target openttd

./scripts/export_openttd_airport_fta_trace.sh \
  crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav \
  crates/openttdrs-core/tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl \
  80

cargo run -p openttdrs-core --bin sav_airport_fta_runner -- \
  crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav \
  --ticks 80 --out /tmp/helidepot-openttdrs.jsonl

python3 scripts/compare_airport_fta_traces.py \
  crates/openttdrs-core/tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl \
  /tmp/helidepot-openttdrs.jsonl
```
