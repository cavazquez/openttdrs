# Características de OpenTTD aún no consideradas en openttdrs

Subsistemas y reglas del original detectados durante el análisis de paridad
(Fase 1) que hoy no tienen equivalente en la sim Rust. Priorizados por impacto
en el caso «camión entra a playa de carga» y en la paridad general de
vehículos de carretera.

## Prioridad alta (afectan el caso de los videos)

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
5. **Carga/descarga gradual** — `economy.cpp:1609` (`LoadUnloadVehicle`): mueve
   cargo por tick con `load_amount` del motor; hoy la sim carga la capacidad
   completa en un tick.

## Prioridad media

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

## Prioridad baja (para fases posteriores)

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

## Cómo detectar regresiones/omisiones nuevas

- Correr `parity_runner --divergence-report` tras cada cambio de la sim: las
  divergencias conocidas se re-verifican contra la traza.
- El test `golden_roadveh::known_divergences_are_confirmed_by_trace` falla si
  una divergencia documentada deja de existir (recordatorio de actualizar docs)
  o si las tablas copiadas dejan de coincidir con el C++.
- Para features de esta lista: al implementarlas, añadir el evento
  correspondiente a `parity/record.rs` y un chequeo en `parity/report.rs`.
