# Divergencias conocidas openttdrs ↔ OpenTTD

Archivo generado por `parity_runner --divergence-report`.
Estas divergencias son conocidas y NO rompen CI; su corrección es parte de las fases de paridad.

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

- tick 213: carga iniciada con el camión en TileCoord { x: 1, y: 6 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })

- Referencia OpenTTD: `OpenTTD/src/table/roadveh_movement.h:1087-1093 (`_road_stop_stop_frame`, frames 11-20) y OpenTTD/src/roadveh_cmd.cpp:1496-1502 (chequeo del frame de parada)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → tesela de la bahía; `is_connected_bay_road_stop`)`
- Fase 2: IMPLEMENTADA (Fase 2): bus/camión entra a la tesela de la bahía y carga dentro; pendiente afinar el punto exacto de parada (`_road_stop_stop_frame`) en el render

## Carga/descarga instantánea (OpenTTD la hace gradual por tick) (`instant_loading`)

Estado: **CONFIRMADA en la traza**

Evidencia medida:

- tick 213: `loading_started` y `loading_finished` en el mismo tick (carga instantánea)

- Referencia OpenTTD: `OpenTTD/src/economy.cpp:1609 (`LoadUnloadVehicle`, transfiere por tick)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/sim_step.rs:205-241 (`try_load_from_industry` carga la capacidad completa en un tick)`
- Fase 2: modelar carga gradual con velocidad de carga por tipo de cargo

## Tick de simulación a 5 Hz frente a ~33,3 Hz de OpenTTD (`tick_rate`)

Estado: **CONFIRMADA en la traza**

Evidencia medida:

- constante: `SIM_TICK_HZ = 5.0` con `REFERENCE_PROGRESS_STEP = 51` (5 ticks/tesela); OpenTTD avanza `frame` cada tick a 74 ticks/día

- Referencia OpenTTD: `OpenTTD/src/timer/timer_game_tick.h:77 (`DAY_TICKS = 74`)`
- Referencia Rust: `openttdrs/crates/openttdrs-client/src/simulation.rs:12 (`SIM_TICK_HZ = 5.0`) y openttdrs/crates/openttdrs-core/src/engine.rs:71 (`REFERENCE_PROGRESS_STEP = 51`)`
- Fase 2: DECIDIDO (Fase 2): se mantiene 5 Hz y la paridad se valida en unidades relativas; criterios de revisión en docs/parity/tick_rate_decision.md

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

- tick 88: giro dir 1→3; velocidad 16→8 (OpenTTD esperaría ≤ 12)
- tick 138: giro dir 3→1; velocidad 17→9 (OpenTTD esperaría ≤ 13)
- tick 215: giro dir 1→5; velocidad 22→11 (OpenTTD esperaría ≤ 17)
- tick 436: giro dir 5→3; velocidad 52→27 (OpenTTD esperaría ≤ 39)
- tick 493: giro dir 3→7; velocidad 37→19 (OpenTTD esperaría ≤ 28)
- tick 598: giro dir 7→1; velocidad 38→19 (OpenTTD esperaría ≤ 29)

- Referencia OpenTTD: `OpenTTD/src/train_cmd.cpp:3147-3152 (`_accel_slowdown`), :3564-3568 (aplicación en locomotora)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`set_direction_with_curve_penalty` para `VehicleKind::Train`)`
- Fase 2: IMPLEMENTADA (Rail 3B): `cur_speed -= turn·cur_speed >> 8` con small_turn=64 / large_turn=128

## El tren carga desde la vía de acceso, no desde la plataforma (`train_platform_stop`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 213: carga iniciada en TileCoord { x: 1, y: 6 } (at_platform=true)

- Referencia OpenTTD: `OpenTTD/src/train_cmd.cpp:266-305 (`GetTrainStopLocation`) y :3097-3123 (`TrainEnterStation`)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → `rail_station_stop_tile`)`
- Fase 2: IMPLEMENTADA (Rail 3C): destino = plataforma; `at_platform: true` en la traza

