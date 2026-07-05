# Plan de debugging de paridad ferroviaria

Fecha: 2026-07-02 · Producto de la Fase Rail 0 (auditoría). Extiende a
ferrocarriles el sistema de paridad de la Fase 1 (trazas JSONL + runner
headless + comparador de primera divergencia + goldens del C++).

Principio rector: **extender, no duplicar**. `TickRecord`/`ParityEvent`
(`parity/record.rs`) ya identifican por `vehicle.id` y el tracer deriva todo
por diff de estado pre/post tick sin hooks en la lógica; el mismo mecanismo
sirve para trenes.

Regla de orden: **no cambiar comportamiento antes de tener trazas**. Las fases
3B/3C (primeros cambios de lógica) requieren la Fase Rail 1 terminada para
medir antes/después.

## Fase Rail 1 — Trazas ferroviarias mínimas — ✅ IMPLEMENTADA

- **Objetivo**: bloque `rail` opcional en la traza + eventos ferroviarios +
  escenario headless `train_line`.
- **Alcance cerrado** (solo instrumentación, cero cambios de comportamiento):
  - `parity/record.rs`: campo `rail: Option<RailRecord>` en `VehicleRecord`
    con `#[serde(skip_serializing_if = "Option::is_none")]` — las trazas de
    camión no cambian ni un byte. `RailRecord`:
    - `parts: Vec<RailPartRecord { part_index, tile, subtile_x, subtile_y }>`
      (hoy siempre 1 entrada: tren puntual; el esquema admite consist futura);
    - `head_tile`, `tail_tile` (== `head_tile` mientras no haya consist);
    - `track_bits_under` (`m5 & 0x3F` de la tesela actual);
    - `blocked_by_signal`, `blocked_by_traffic` (lectura de los helpers de
      `rail_signals.rs`);
    - `in_depot`, `at_platform` (hoy siempre `false`: evidencia de la
      divergencia de entrada a plataforma).
  - Eventos nuevos en `ParityEvent`: `SignalWaitStarted/Finished { vehicle,
    tile }`, `DepotEntry/DepotExit { vehicle, depot }`, `SignalStateChanged
    { tile, track_mask, green }` (diff de `m3hi` entre ticks).
    `reservation_changed` queda reservado sin emisor (PBS no existe).
    `curve_entered/exited` se derivan de `DirectionChanged` (sin evento propio).
  - `parity/tracer.rs`: derivación por diff, igual que los eventos actuales.
  - `parity/scenario.rs`: escenario `train_line` determinístico — depósito,
    tramo recto, una curva (piezas UPPER/LOWER), una señal de bloque, estación
    rail de 1 plataforma; un tren (`ENGINE_TRAIN_KIRBY`) con órdenes
    estación↔estación. Registrar en `scenario_names()` y `parity_runner`.
- **No tocar**: `vehicle.rs::step`, `sim_step.rs`, `rail_signals.rs`,
  `pathfinder.rs` (comportamiento observable intacto); el comparador (Fase 2).
- **Tests**: roundtrip serde de `RailRecord` y eventos nuevos; traza de
  `truck_bay` sin clave `"rail"`; `train_line` emite en ≤600 ticks al menos
  `StationEntry` + un evento de carga + `DepotExit`/`Start`; JSONL parseable
  línea a línea.
- **Comandos**:
  ```bash
  cargo run -p openttdrs-core --bin parity_runner -- \
      --scenario train_line --ticks 600 --out /tmp/train_line.jsonl
  ./scripts/check.sh
  ```
- **Terminado cuando**: check verde; JSONL con bloque `rail` y eventos nuevos;
  no-regresión en `truck_bay` (`parity_diff` contra traza previa → exit 0).
- **Resultado**: implementada tal cual el alcance. La traza de `truck_bay`
  quedó byte-idéntica a la previa (verificado con `diff` y `parity_diff` →
  exit 0). El escenario `train_line` (600 ticks) emite `depot_exit`,
  `station_entry`, `loading_started/finished`, `order_advanced`,
  `direction_changed` (curva de la L) y `signal_state_changed` (la señal
  pasa a rojo/verde cuando el tren ocupa/libera el bloque). `at_platform`
  es siempre `false`: evidencia medible de la divergencia que corrige la
  Fase Rail 3C. `SignalWaitStarted/Finished` se cubren con un test de dos
  trenes (`signal_wait_events_emitted_with_two_trains`).
  Nota de alcance: `ParityEvent::vehicle()` pasó a devolver `Option<u32>`
  porque `SignalStateChanged` es un evento de infraestructura sin vehículo.

## Fase Rail 2 — Comparador ferroviario mínimo — ✅ IMPLEMENTADA

- **Objetivo**: que `parity_diff` responda «¿en qué tick/tren/tile dejó de
  coincidir?» con clasificación ferroviaria.
- **Alcance**: en `parity/diff.rs` + `bin/parity_diff.rs`:
  - subsistemas nuevos: `rail_infrastructure` (`track_bits_under`,
    `SignalStateChanged`), `train_motion` (`speed`/`progress`/`dir`/`tile`),
    `consist_geometry` (`parts`, `head/tail` — trivial hoy, preparado),
    `pathfinding`, `station_entry`, `loading`, `signaling`
    (`blocked_by_signal`, `SignalWait*`), `reservation` (reservado), `depot`;
  - filtros: `--tile x,y` (divergencias que involucren esa tesela: señal,
    estación, segmento) y `--event tipo`, además del `--vehicle` existente;
  - tolerancia float: `--subtile-epsilon` (default 0.51 = medio píxel de 16)
    solo para `subtile_x/y`; el resto se compara exacto;
  - trazas asimétricas: si una tiene `rail` y la otra no → `missing_field`,
    no divergencia (permite comparar trazas pre/post Fase Rail 1);
  - salida `--json out.json`: `{ first_divergence: { tick, vehicle, subsystem,
    field, a, b }, by_subsystem: {...} }`.
- **Tests**: fixtures JSONL artificiales (pares de trazas fabricadas con una
  divergencia conocida por subsistema); primera divergencia con subsistema
  correcto; epsilon respeta el umbral; exit codes.
- **No tocar**: la sim; el esquema de traza (solo lectura).
- **Terminado cuando**: fixtures verdes y `truck_bay` sigue comparando igual.
- **Resultado**: implementada tal cual el alcance. Los campos `rail.*` se
  clasifican por subsistema (`rail.track_bits_under` → `rail_infrastructure`,
  `rail.parts[i].subtile_*` → `train_motion` con epsilon, `rail.blocked_by_*`
  → `signaling`, `rail.in_depot` → `depot`, `rail.at_platform` →
  `station_entry`, resto de partes/cabeza/cola → `consist_geometry`;
  `path_next` pasó de `orders` a `pathfinding`). Las divergencias de eventos
  ahora se reportan por evento faltante/sobrante con campo
  `events.<tipo>` y el subsistema del evento (antes: un solo diff opaco
  `events` por tick). Verificado sobre trazas reales: `truck_bay`
  pre/post → exit 0; traza `train_line` mutada → primera divergencia
  exacta (tick, campo, subsistema) y JSON con `first_divergence` +
  `by_subsystem`; traza sin bloque rail vs con bloque → NOTA
  `missing_field` y exit 0.

## Fase Rail 3A — Infraestructura ferroviaria básica — ✅ IMPLEMENTADA

- **Objetivo**: goldens de infraestructura y huecos de test cerrados.
- **Alcance**:
  - `scripts/extract_train_movement.py` (mismo patrón regex que
    `extract_roadveh_movement.py`, solo lectura de `OpenTTD/`) →
    `tests/fixtures/parity/train_movement_golden.json` con: `_accel_slowdown`
    {64,128,64,2} (`train_cmd.cpp:3147`), `_vehicle_subcoord`
    (`vehicle.cpp:3359`), `_fractcoords_enter` y `_vehicle_initial_*_fract`
    (`rail_cmd.cpp:2975`, `train_cmd.cpp:54`), `_tunnel_visibility_frame`
    {12,8,8,12} (`tunnelbridge_cmd.cpp:1956`), constantes de `UpdateSpeed`;
  - `tests/golden_rail.rs`: tabla de conectividad piezas×lados
    (`rail_bit_for_sides` vs semántica de `track_type.h`), encoding de
    señales, validación estructural del fixture;
  - tests de túnel/puente **rail** (hoy 0): colocación + tren cruza sin
    descarrilar.
- **No tocar**: lógica de trenes (salvo lo mínimo si un test destapa un bug).
- **Comandos**: `python3 scripts/extract_train_movement.py && ./scripts/check.sh`.
- **Terminado cuando**: golden verde y fixture versionado (recordar la
  excepción de `.gitignore` para `tests/fixtures/**/*.json`).
- **Resultado**: implementada tal cual el alcance. `extract_train_movement.py`
  genera `train_movement_golden.json` con `_accel_slowdown`, fractcoords de
  depósito, `_vehicle_subcoord`, `_tunnel_visibility_frame` y constantes de
  `UpdateSpeed` AM_ORIGINAL. Copias en `train_movement.rs`; `golden_rail.rs`
  (11 tests) valida fixture ↔ Rust, conectividad `rail_bit_for_sides`,
  encoding de señales y movimiento: tren atraviesa túnel completo; puente —
  colocación + entrada a rampa (el vano central aún no lo cruza el
  pathfinder; divergencia documentada en el test).

## Fase Rail 3B — Movimiento de trenes

- **Estado**: **Completada** — aceleración `AM_ORIGINAL` y frenado por curva
  portados; chequeos `train_road_acceleration` y `train_no_curve_braking` en
  `parity/report.rs` (regresión sobre `train_line`).
- **Objetivo**: corregir las dos divergencias de movimiento medidas.
- **Alcance** (primer cambio de lógica; trazas de Rail 1 como antes/después):
  1. aceleración AM_ORIGINAL de tren: `acceleration = Clamp(power_hp /
     (weight_t·4), 1, 255)`, avance `accel·2`, freno `accel·−4`
     (`train_cmd.cpp:3080-3090`, `:444-452`) — usa los `power_hp`/`weight_t`
     ya presentes en `EngineDef`;
  2. frenado por curva `_accel_slowdown`: −25 % (`>>8` de 64) en curva corta,
     −50 % (128) en larga, al cambiar de dirección — invertir el test
     `train_keeps_speed_on_direction_change`.
- **No tocar**: señales, estaciones, pathfinding.
- **Tests**: curva de velocidad tick a tick por motor contra la fórmula golden;
  chequeos `train_road_acceleration` y `train_no_curve_braking` de
  `parity/report.rs` pasan de CONFIRMADA a regresión (patrón de la Fase 2 de
  camiones).
- **Terminado cuando**: check verde + `divergences_found.md` regenerado con
  ambas divergencias «no observada».

## Fase Rail 3C — Estaciones y carga/descarga

- **Objetivo**: el tren entra a la plataforma y para en el punto correcto.
- **Alcance**: destino de orden = plataforma (análogo de
  `resolve_order_destination` → bahía de la Fase 2); punto de parada según
  `GetTrainStopLocation` (`train_cmd.cpp:266-305`; con tren puntual y
  plataforma corta, el caso base es «far end»); frenado sub-tile
  `(stop-x)·20−15` (`station_cmd.cpp:3874-3880`) si se porta el detalle;
  `at_platform` pasa a `true` en la traza; **invertir**
  `showcase_train_stays_on_rail_not_station_platform` (hoy asserta la
  divergencia, como pasó con los tests de bahía).
- **No tocar**: señales/PBS.
- **Terminado cuando**: traza muestra `station_entry` con `at_platform: true`
  y carga dentro; check verde.

## Fase Rail 3D — Señales/reservas

- **Objetivo**: validar y cronometrar la semántica de bloqueo con 2 trenes;
  decidir y documentar si ENTRY/EXIT/COMBO ganan semántica o quedan como
  BLOCK. **PBS queda explícitamente fuera** (documentado en
  `rail_unknown_features.md`).
- **Tests**: dos trenes + una señal: quién espera, cuántos ticks, eventos
  `SignalWait*` correctos en la traza.

## Fase Rail 3E — Render/interpolación

- **Objetivo**: comparar posición lógica (traza Rail 1) vs posición visual
  (`OPENTTDRS_RENDER_TRACE`, ya existente) para trenes; detectar stutter o
  saltos; evaluar subcoordenadas `_vehicle_subcoord` por pieza y ocultamiento
  en túnel (`_tunnel_visibility_frame`).
- **Precondición**: 3B terminada (no medir render sobre física divergente).

## Fase Rail 4 — Reportes y documentación final

- **Objetivo**: chequeos rail en `parity/report.rs` integrados a
  `divergences_found.md`; actualizar los cuatro docs `rail_*.md` con el estado
  final (corregidas vs pendientes).
- **Terminado cuando**: `parity_runner --divergence-report` cubre `train_line`
  y la documentación queda consistente con `status.md`.

## Uso de la traza rail (referencia rápida)

```bash
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 600 --out /tmp/train_line.jsonl
```

Línea real del JSONL (tick 35, el tren cruza al empalme y sale del depósito):

```json
{"tick":35,"vehicles":[{"id":1,"tile":{"x":4,"y":6},"progress":6,"dir":3,
 "speed":35,"subspeed":0,"state":"moving","order_index":0,
 "order_kind":"station","dest":{"x":2,"y":6},"path_next":{"x":3,"y":6},
 "cargo":0,"depart_turn":0,"rail":{"parts":[{"part_index":0,
 "tile":{"x":4,"y":6},"subtile_x":14.647,"subtile_y":8.0}],
 "head_tile":{"x":4,"y":6},"tail_tile":{"x":4,"y":6},"track_bits_under":21,
 "blocked_by_signal":false,"blocked_by_traffic":false,"in_depot":false,
 "at_platform":false}}],"events":[
 {"type":"tile_crossed","vehicle":1,"from":{"x":4,"y":5},"to":{"x":4,"y":6}},
 {"type":"direction_changed","vehicle":1,"from":1,"to":3},
 {"type":"depot_exit","vehicle":1,"depot":{"x":4,"y":5}}]}
```

Escenario `train_line` (`parity/scenario.rs`): L ferroviaria (2,6)→(12,6)→
(12,10) con depósito en (4,5), señal de bloque en (7,6), estación A (1,6) con
goods en stock y estación B (13,10); un tren cicla A ↔ B.

Activación: igual que hoy, opt-in vía `enable_parity_trace()`/`parity_runner`;
el modo normal no ejecuta nada del módulo de paridad. Los vehículos de
carretera no llevan la clave `"rail"` (trazas previas siguen parseando).
