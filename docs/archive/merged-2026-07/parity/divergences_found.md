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

- tick 60: giro dir 1→7; velocidad 59→45 (OpenTTD esperaría ≤ 45)
- tick 87: giro dir 7→1; velocidad 71→54 (OpenTTD esperaría ≤ 54)
- tick 110: giro dir 1→7; velocidad 76→58 (OpenTTD esperaría ≤ 58)
- tick 138: giro dir 7→3; velocidad 22→18 (OpenTTD esperaría ≤ 18)
- tick 157: giro dir 3→5; velocidad 36→28 (OpenTTD esperaría ≤ 28)
- tick 192: giro dir 5→3; velocidad 62→48 (OpenTTD esperaría ≤ 48)
- tick 217: giro dir 3→5; velocidad 72→55 (OpenTTD esperaría ≤ 55)
- tick 240: giro dir 5→7; velocidad 77→59 (OpenTTD esperaría ≤ 59)
- tick 268: giro dir 7→3; velocidad 22→18 (OpenTTD esperaría ≤ 18)
- tick 287: giro dir 3→1; velocidad 36→28 (OpenTTD esperaría ≤ 28)
- tick 322: giro dir 1→7; velocidad 62→48 (OpenTTD esperaría ≤ 48)
- tick 347: giro dir 7→1; velocidad 72→55 (OpenTTD esperaría ≤ 55)
- tick 370: giro dir 1→7; velocidad 77→59 (OpenTTD esperaría ≤ 59)
- tick 398: giro dir 7→3; velocidad 22→18 (OpenTTD esperaría ≤ 18)
- tick 417: giro dir 3→5; velocidad 36→28 (OpenTTD esperaría ≤ 28)
- tick 452: giro dir 5→3; velocidad 62→48 (OpenTTD esperaría ≤ 48)
- tick 477: giro dir 3→5; velocidad 72→55 (OpenTTD esperaría ≤ 55)
- tick 500: giro dir 5→7; velocidad 77→59 (OpenTTD esperaría ≤ 59)

- Referencia OpenTTD: `OpenTTD/src/roadveh_cmd.cpp:1481 (`v->cur_speed -= v->cur_speed >> 2`, AM_ORIGINAL; también :1353 y :1426)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/vehicle.rs (`Vehicle::set_direction_with_curve_penalty`)`
- Fase 2: IMPLEMENTADA (Fase 2): `cur_speed -= cur_speed >> 2` al cambiar `direction` en bus/camión

## El camión se detiene en la carretera de acceso, no dentro de la bahía (`bay_stop_position`)

Estado: **no observada en esta traza**

Evidencia medida:

- tick 111: carga iniciada con el camión en TileCoord { x: 4, y: 5 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })
- tick 371: carga iniciada con el camión en TileCoord { x: 4, y: 5 } (bahía = TileCoord { x: 4, y: 5 }, acceso = TileCoord { x: 4, y: 6 })

- Referencia OpenTTD: `OpenTTD/src/table/roadveh_movement.h:1087-1093 (`_road_stop_stop_frame`, frames 11-20) y OpenTTD/src/roadveh_cmd.cpp:1496-1502 (chequeo del frame de parada)`
- Referencia Rust: `openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → tesela de la bahía; `is_connected_bay_road_stop`)`
- Fase 2: IMPLEMENTADA (Fase 2): bus/camión entra a la tesela de la bahía y carga dentro; pendiente afinar el punto exacto de parada (`_road_stop_stop_frame`) en el render

## Carga/descarga instantánea (OpenTTD la hace gradual por tick) (`instant_loading`)

Estado: **no observada en esta traza**

Evidencia medida:

- transferencia gradual: 2× `loading_started`, 2× `loading_finished` (no en el mismo tick)

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

