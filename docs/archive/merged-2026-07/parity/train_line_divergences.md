# Divergencias conocidas openttdrs ↔ OpenTTD

Archivo generado por `parity_runner --divergence-report`.
Estas divergencias son conocidas y NO rompen CI; su corrección es parte de las fases de paridad.

Metadatos de generación:

- Regenerar: `./scripts/regenerate_parity_reports.sh`
- Tick: `OTTD_MILLISECONDS_PER_TICK=27` (~37.04 Hz); `REFERENCE_PROGRESS_STEP=112`
- Pin OpenTTD: [`openttd-reference.json`](openttd-reference.json) (tag **15.3**, `14ec60f24854`)
- openttdrs commit: `1792e0c`

## Falta la penalización de velocidad del 25 % en curvas (`curve_speed_penalty`)

Estado: **no observada en esta traza**

Evidencia medida:

- la traza no contiene giros diagonales del camión

- Referencia OpenTTD: `OpenTTD/src/roadveh_cmd.cpp:1481 (`v->cur_speed -= v->cur_speed >> 2`, AM_ORIGINAL; también :1353 y :1426)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`Vehicle::set_direction_with_curve_penalty`)`
- Fase 2: IMPLEMENTADA (Fase 2): `cur_speed -= cur_speed >> 2` al cambiar `direction` en bus/camión

## El camión se detiene en la carretera de acceso, no dentro de la bahía (`bay_stop_position`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 145: carga iniciada con el camión en TileCoord { x: 1, y: 6 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })

- Referencia OpenTTD: `OpenTTD/src/table/roadveh_movement.h:1087-1093 (`_road_stop_stop_frame`, frames 11-20) y OpenTTD/src/roadveh_cmd.cpp:1496-1502 (chequeo del frame de parada)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → tesela de la bahía; `is_connected_bay_road_stop`)`
- Fase 2: IMPLEMENTADA (Fase 2): bus/camión entra a la tesela de la bahía y carga dentro; pendiente afinar el punto exacto de parada (`_road_stop_stop_frame`) en el render

## Carga/descarga instantánea (OpenTTD la hace gradual por tick) (`instant_loading`)

Estado: **no observada en esta traza**

Evidencia medida:

- transferencia gradual: 1× `loading_started`, 1× `loading_finished` (no en el mismo tick)

- Referencia OpenTTD: `OpenTTD/src/economy.cpp:1609 (`LoadUnloadVehicle`, transfiere por tick)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/sim_step/cargo_transfer.rs (`load_unload_speed` + packets)`
- Fase 2: IMPLEMENTADA (Fase 2): transferencia gradual por tick según `load_unload_speed`; packets con origen/edad

## Tick de simulación alineado con OpenTTD (~37 Hz) (`tick_rate`)

Estado: **no observada en esta traza**

Evidencia medida:

- `SIM_TICKS_PER_SECOND` = 1000/27 (`timer_game_tick.h`); `REFERENCE_PROGRESS_STEP` = 112 (`GetAdvanceSpeed`)

- Referencia OpenTTD: `OpenTTD/src/timer/timer_game_tick.h:77 (`DAY_TICKS = 74`, ~27 ms/tick)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/economy/time.rs (`SIM_TICKS_PER_SECOND`) y engine/physics.rs (`REFERENCE_PROGRESS_STEP`)`
- Fase 2: IMPLEMENTADO: sim a ~37 Hz y paso sub-tesela según `GetAdvanceSpeed`/`GetAdvanceDistance`.

## El tren acelera con la fórmula de carretera (ROAD_ACCEL_ORIGINAL) en lugar de power/weight·4 (`train_road_acceleration`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 11: speed≥2 tras 6 ticks desde parado (carretera ≈1–2; Kirby AM_ORIGINAL ≫2)

- Referencia OpenTTD: `OpenTTD/src/train_cmd.cpp:444-452 (`UpdateAcceleration`) y :3080-3090 (`UpdateSpeed` AM_ORIGINAL, `accel·2`)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`update_movement_speed` → `accelerate_train_speed`)`
- Fase 2: IMPLEMENTADA (Rail 3B): `train_acceleration` + `accel·2` / freno `accel·4`

## Falta el frenado por curva del tren (`_accel_slowdown`) (`train_no_curve_braking`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 59: giro dir 1→3; velocidad 10→6 (OpenTTD esperaría ≤ 8)
- tick 91: giro dir 3→1; velocidad 11→6 (OpenTTD esperaría ≤ 9)
- tick 156: giro dir 1→5; velocidad 1→1 (OpenTTD esperaría ≤ 1)
- tick 321: giro dir 5→3; velocidad 32→16 (OpenTTD esperaría ≤ 24)

- Referencia OpenTTD: `OpenTTD/src/train_cmd.cpp:3147-3152 (`_accel_slowdown`), :3564-3568 (aplicación en locomotora)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`set_direction_with_curve_penalty` para `VehicleKind::Train`)`
- Fase 2: IMPLEMENTADA (Rail 3B): `cur_speed -= turn·cur_speed >> 8` con small_turn=64 / large_turn=128

## El tren carga desde la vía de acceso, no desde la plataforma (`train_platform_stop`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 145: carga iniciada en TileCoord { x: 1, y: 6 } (at_platform=true)

- Referencia OpenTTD: `OpenTTD/src/train_cmd.cpp:266-305 (`GetTrainStopLocation`) y :3097-3123 (`TrainEnterStation`)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → `rail_station_stop_tile`)`
- Fase 2: IMPLEMENTADA (Rail 3C): destino = plataforma; `at_platform: true` en la traza

## La sub-tesela de la traza rail no coincide con la del render (`train_render_subtile_consistency`)

Estado: **no observada en esta traza**

Evidencia medida:

- la traza rail y `vehicle_subtile` coinciden en todos los ticks

- Referencia OpenTTD: `OpenTTD/src/vehicle.cpp:3359 (`_vehicle_subcoord` + progreso)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/parity/tracer.rs + `road_movement::vehicle_subtile``
- Fase 2: IMPLEMENTADA (Rail 3E): regresión traza ↔ render lógico

## Subcoordenadas por pieza: centro de vía en curvas diagonales (`train_diagonal_subcoord_approximation`)

Estado: **CONFIRMADA en la traza**

Evidencia medida:

- tick 312: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 313: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 314: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 315: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 316: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 317: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 318: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 319: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)
- tick 320: pieza diagonal track_bits=0x20 en TileCoord { x: 12, y: 6 } (render ≈ centro de vía)

- Referencia OpenTTD: `OpenTTD/src/vehicle.cpp:3359-3392 (`_vehicle_subcoord` por enterdir×track)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/road_movement.rs (`train_straight_subtile`, `TRAIN_TRACK_CENTER = 8`)`
- Fase 2: DECIDIDO (Rail 3E): divergencia cosmética documentada; X/Y usan el mismo eje que la entrada OpenTTD

