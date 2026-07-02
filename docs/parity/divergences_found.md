# Divergencias conocidas openttdrs ↔ OpenTTD

Archivo generado por `parity_runner --divergence-report` sobre el escenario `truck_bay`.
Estas divergencias son conocidas y NO rompen CI; su corrección es parte de la Fase 2.

## Falta la penalización de velocidad del 25 % en curvas (`curve_speed_penalty`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 90: giro dir 1→7; velocidad 89→68 (OpenTTD esperaría ≤ 68)
- tick 130: giro dir 7→1; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 168: giro dir 1→7; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 178: giro dir 7→3; velocidad 79→60 (OpenTTD esperaría ≤ 60)
- tick 195: giro dir 3→5; velocidad 76→58 (OpenTTD esperaría ≤ 58)
- tick 238: giro dir 5→3; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 277: giro dir 3→5; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 315: giro dir 5→7; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 325: giro dir 7→3; velocidad 79→60 (OpenTTD esperaría ≤ 60)
- tick 342: giro dir 3→1; velocidad 76→58 (OpenTTD esperaría ≤ 58)
- tick 385: giro dir 1→7; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 424: giro dir 7→1; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 462: giro dir 1→7; velocidad 96→72 (OpenTTD esperaría ≤ 73)
- tick 472: giro dir 7→3; velocidad 79→60 (OpenTTD esperaría ≤ 60)
- tick 489: giro dir 3→5; velocidad 76→58 (OpenTTD esperaría ≤ 58)

- Referencia OpenTTD: `OpenTTD/src/roadveh_cmd.cpp:1481 (`v->cur_speed -= v->cur_speed >> 2`, AM_ORIGINAL; también :1353 y :1426)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`Vehicle::set_direction_with_curve_penalty`)`
- Fase 2: IMPLEMENTADA (Fase 2): `cur_speed -= cur_speed >> 2` al cambiar `direction` en bus/camión

## El camión se detiene en la carretera de acceso, no dentro de la bahía (`bay_stop_position`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 169: carga iniciada con el camión en TileCoord { x: 4, y: 5 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })
- tick 463: carga iniciada con el camión en TileCoord { x: 4, y: 5 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })

- Referencia OpenTTD: `OpenTTD/src/table/roadveh_movement.h:1087-1093 (`_road_stop_stop_frame`, frames 11-20) y OpenTTD/src/roadveh_cmd.cpp:1496-1502 (chequeo del frame de parada)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → tesela de la bahía; `is_connected_bay_road_stop`)`
- Fase 2: IMPLEMENTADA (Fase 2): bus/camión entra a la tesela de la bahía y carga dentro; pendiente afinar el punto exacto de parada (`_road_stop_stop_frame`) en el render

## Carga/descarga instantánea (OpenTTD la hace gradual por tick) (`instant_loading`)

Estado: **CONFIRMADA en la traza**

Evidencia medida:

- tick 169: `loading_started` y `loading_finished` en el mismo tick (carga instantánea)
- tick 463: `loading_started` y `loading_finished` en el mismo tick (carga instantánea)

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

