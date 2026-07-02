# Decisión: tick de simulación a 5 Hz frente a ~33,3 Hz de OpenTTD

Estado: **decidido — se mantiene 5 Hz en la Fase 2**, con extrapolación de
render para la fluidez y unidades relativas para la paridad. Revisable cuando
el resto de divergencias de comportamiento estén cerradas.

## Los dos modelos

| | OpenTTD | openttdrs |
|---|---|---|
| Frecuencia del tick | ~33,3 Hz (30 ms/tick, 74 ticks/día — `timer/timer_game_tick.h:77`) | 5 Hz (`openttdrs-client/src/simulation.rs::SIM_TICK_HZ`) |
| Avance por tick | `frame` 0–15 por entrada de tabla; 1 frame ≈ 1 paso de píxel (`GetAdvanceSpeed`) | `progress` 0–255 lineal por tesela; `REFERENCE_PROGRESS_STEP = 51` (~5 ticks/tesela a velocidad de crucero) |
| Fluidez visual | nativa (la sim ya corre a frecuencia de animación) | extrapolación entre ticks en el cliente (`extrapolate_vehicle_pose` + `tick_alpha`) |

Ambos modelos usan la misma aritmética de velocidad (AM_ORIGINAL portada tal
cual: `update_road_speed`, `progress_step_for_speed`), así que la *proporción*
de avance por unidad de tiempo simulado coincide; lo que difiere es la
granularidad temporal.

## Por qué se mantiene 5 Hz (por ahora)

1. **La paridad que buscamos hoy es de comportamiento, no de timing absoluto.**
   Orden de eventos, teselas recorridas, velocidades relativas, puntos de
   parada y estados: todo eso se compara bien en unidades relativas
   (ticks/tesela, % de velocidad) y así están escritos el runner, el
   comparador y los golden tests.
2. **Cambiar a 33,3 Hz ahora invalidaría toda la evidencia acumulada** (trazas,
   timelines documentadas, tests con presupuestos de ticks) mientras aún hay
   divergencias de comportamiento abiertas (`instant_loading`, trayectorias
   `_rv_station_*` exactas). Primero cerrar comportamiento, después escalar
   tiempo.
3. **El coste visual ya está mitigado**: el cliente extrapola la pose entre
   ticks y el selector de sprites usa la pose extrapolada (Fase 1), de modo que
   la fluidez percibida no depende de la frecuencia del tick lógico.
4. **Costo de CPU**: 5 Hz mantiene la sim barata en mapas grandes; subir a
   33,3 Hz multiplica ~6,7× el trabajo por segundo sin ganancia de paridad de
   comportamiento.

## Qué NO se puede comparar mientras tanto (asumido)

- Duraciones absolutas (un día de juego no dura lo mismo en tiempo real).
- Timing fino intra-tesela: OpenTTD resuelve 16 frames por entrada de tabla;
  aquí una tesela son ~5 ticks. Efectos de 1–2 frames de OpenTTD (p. ej. el
  retardo extra por giro de `roadveh_cmd.cpp:1483-1487`) se aproximan o se
  posponen: a 5 Hz un «frame perdido» equivaldría a ~0,2 s visibles, peor que
  omitirlo.
- Cualquier diff tick a tick contra una traza capturada de OpenTTD real; el
  comparador (`parity_diff`) solo sirve entre corridas de openttdrs.

## Criterios para revisar la decisión

Migrar la sim a ~33,3 Hz (o a un múltiplo con `frame` 0–15 como en el
original) cuando se cumpla alguna de estas condiciones:

1. Se quiera validar contra **trazas reales de OpenTTD** (nivel 5 de madurez en
   `status.md`): ahí el eje temporal debe coincidir.
2. Se porten las trayectorias `_rv_station_*` exactas por frame: esas tablas
   están indexadas por `frame` 0–15+ y encajan mal en `progress` 0–255 lineal.
3. La extrapolación de render deje de ser suficiente (p. ej. adelantamientos o
   seguimiento entre vehículos, que necesitan resolución temporal fina en la
   propia sim).

La migración prevista: mantener las fórmulas ya validadas (son las mismas),
cambiar `SIM_TICK_HZ` a 33,3, recalibrar `REFERENCE_PROGRESS_STEP` (÷6,7) y
regenerar los golden de timing. El diseño actual concentra la constante en un
único punto (`engine.rs` + `simulation.rs`) para que ese cambio sea acotado.

## Referencias

- `OpenTTD/src/timer/timer_game_tick.h:77` — 74 ticks/día.
- `openttdrs/crates/openttdrs-client/src/simulation.rs` — `SIM_TICK_HZ = 5.0`.
- `openttdrs/crates/openttdrs-core/src/engine.rs` — `REFERENCE_PROGRESS_STEP = 51`.
- `docs/parity/divergences_found.md` — divergencia `tick_rate` (CONFIRMADA, aceptada).
- `docs/parity/status.md` — fila «Tick lógico».
