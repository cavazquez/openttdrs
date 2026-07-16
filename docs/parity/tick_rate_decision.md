# Decisión: tick de simulación alineado con OpenTTD (~37 Hz)

Estado: **vigente — sim a ~37 Hz** (`OTTD_MILLISECONDS_PER_TICK = 27`,
`SIM_TICKS_PER_SECOND ≈ 37.04`) con `REFERENCE_PROGRESS_STEP = 112`.

ADR canónica: [adr/0003-tick-37hz-openttd.md](../adr/0003-tick-37hz-openttd.md)
(supersede el punto de tick de [adr/0002](../adr/0002-determinismo-tick-referencia.md)).

## Los dos modelos (hoy alineados en frecuencia)

| | OpenTTD | openttdrs |
|---|---|---|
| Frecuencia del tick | ~37 Hz (27 ms/tick, 74 ticks/día — `timer/timer_game_tick.h`) | `SIM_TICKS_PER_SECOND = 1000/27` (`economy/time.rs`); cliente `SIM_TICK_HZ` |
| Avance por tick | `GetAdvanceSpeed` / `GetAdvanceDistance` (`vehicle_base.h`) | `progress` 0–255; `REFERENCE_PROGRESS_STEP = 112` (`engine/physics.rs`) |
| Fluidez visual | nativa | extrapolación entre ticks (`extrapolate_vehicle_pose` + `tick_alpha`) |

## Historia (ya no vigente)

Hasta la Fase 2 temprana la sim corría a baja frecuencia (~cinco ticks/s) con
paso sub-tesela reducido (~51) para abaratar CPU y medir paridad en unidades
relativas. Esa decisión quedó obsoleta al alinear el reloj con OpenTTD; no
debe citarse como estado actual.

## Qué sigue siendo distinto

- Trazas tick-a-tick contra un binario OpenTTD real aún no son el nivel de
  madurez 5 (`status.md`): hay divergencias de comportamiento abiertas
  (p. ej. subcoordenadas diagonales rail).
- Carga/descarga es **gradual** por tick (`load_unload_speed` + packets);
  la divergencia `instant_loading` quedó como chequeo de regresión.

## Referencias

- `OpenTTD/src/timer/timer_game_tick.h` — 74 ticks/día, ~27 ms/tick.
- `openttdrs/crates/openttdrs-core/src/economy/time.rs` — `OTTD_MILLISECONDS_PER_TICK`, `SIM_TICKS_PER_SECOND`.
- `openttdrs/crates/openttdrs-core/src/engine/physics.rs` — `REFERENCE_PROGRESS_STEP = 112`.
- `openttdrs/crates/openttdrs-client/src/simulation.rs` — `SIM_TICK_HZ`.
- Informes regenerados: `./scripts/regenerate_parity_reports.sh`.
