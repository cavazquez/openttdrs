# Características ferroviarias de OpenTTD aún no consideradas en openttdrs

Subsistemas y reglas del original detectados durante la auditoría de la Fase
Rail 0 que hoy no tienen equivalente en la sim Rust. Priorizados por impacto
en la paridad de movimiento y estaciones (los casos más visibles).

Supuesto explícito: el tren de la sim es un **vehículo puntual** (sin consist).
Varios ítems de esta lista dependen de esa decisión estructural y se marcan
como tales.

## Prioridad alta (afectan movimiento y estaciones)

1. ~~**Aceleración de tren `AM_ORIGINAL`**~~ — **Resuelto (Rail 3B)**.
   `engine.rs::train_acceleration` + `accelerate_train_speed` / `decelerate_train_speed`
   (Kirby: 300 HP / 47 t → `accel = 24`).
2. ~~**Frenado por curva `_accel_slowdown`**~~ — **Resuelto (Rail 3B)**.
   `set_direction_with_curve_penalty` y `apply_immediate_train_turnaround`.
3. ~~**Entrada a la plataforma + punto de parada**~~ — **Resuelto (Rail 3C)**.
   `rail_station_stop_tile` + `at_platform` en traza.
4. **Carga/descarga gradual** — `economy.cpp:1609` (`LoadUnloadVehicle`):
   idéntica divergencia `instant_loading` ya documentada para carretera;
   aplica también a trenes.
5. **Espera y frames de depósito** — `CheckTrainStayInDepot`
   (`train_cmd.cpp:2354-2427`, espera ~37 ticks y chequea señales/reserva
   antes de salir), `_fractcoords_enter` / `_vehicle_initial_*_fract`
   (`rail_cmd.cpp:2975-2991`, `train_cmd.cpp:54-56`) y `TicksToLeaveDepot`
   (salida encadenada de vagones). Hoy la salida es inmediata y sin frames.

## Prioridad media

6. **Consist: locomotora + vagones** — `train.h` (`Next()`,
   `GetNextUnit`, `tcache`), `ConsistChanged` (`train_cmd.cpp:110-254`,
   `cached_total_length` con `VEHICLE_LENGTH = 8` por unidad). Estructural:
   sin consist no hay longitud, posición de cola, ocupación multi-tesela ni
   carga por vagón. La importación de `.sav` descarta vagones
   (`decodes_front_vehicles_and_skips_wagons`). Requiere decisión de alcance
   antes de planificar (no entra en Rail 0–4).
7. **Reservas de camino (PBS)** — `pbs.cpp/h` (`TryReserveRailTrack`,
   `FollowTrainReservation`, `SetRailStationPlatformReservation`) y señales
   `Path`/`PathOneWay`. La sim usa anticolisión propia
   (`train_blocked_by_traffic`) y conserva `m2_hi` de los saves sin lógica.
   Excluido explícitamente del plan Rail 0–4 (Fase 3D solo valida el bloque
   simple).
8. **Semántica de presignals ENTRY/EXIT/COMBO** — `signal.cpp` (propagación
   por segmento con presignals). La sim codifica los tipos pero ENTRY no
   bloquea (`entry_signal_does_not_block_train`). Decisión pendiente en la
   Fase Rail 3D: darles semántica o degradarlos documentadamente a BLOCK.
9. **Túneles/puentes en tránsito** — ocultamiento del tren
   (`_tunnel_visibility_frame` {12,8,8,12}, `tunnelbridge_cmd.cpp:1956`),
   límite de velocidad del puente (`cur_speed = min(cur_speed,
   BridgeSpec->speed)`, `:2028-2033` y `train_cmd.cpp:427-429`). Hoy el tramo
   se atraviesa como vía normal y el tren queda visible.
10. **Subcoordenadas por pieza `_vehicle_subcoord`** —
    `vehicle.cpp:3359-3392`: posición (x, y) y dirección exactas al entrar a
    cada track. La sim usa siempre el centro de vía
    (`train_straight_subtile`, `TRAIN_TRACK_CENTER = 8`); afecta solo render.
    Plan: golden en 3A, evaluación en 3E.
11. **Pathfinder YAPF con penalizaciones y reserva** —
    `yapf_rail.cpp` (penaliza curvas, señales rojas, plataformas ocupadas;
    reserva el camino elegido). El A* propio no replica desempates ni costes.
12. **Reversa con coste/chequeos** — `ReverseTrainDirection` (news, PBS,
    `reverse_ctr`). Hoy la reversa automática y manual son instantáneas.

## Prioridad baja (dependen de decisiones estructurales o son cosméticas)

13. **Railtypes** (normal/eléctrico/monorail/maglev), compatibilidad
    motor↔vía, `CmdConvertRail`, `curve_speed` por railtype y catenaria —
    `rail.h:26-525`. Sin `RailType` en core; el único rastro es la variante
    visual semáforo/eléctrica de señales.
14. **Ownership por tile de vía** — `m1` se fuerza a 0 al construir.
15. **AM_REALISTIC + `GetCurveSpeedLimit`** — `train_cmd.cpp:312-381`
    (límites 61 / 88 / `232-(13-n)²`, tilt +20 %, `curve_speed_mod` por motor)
    y `GetAcceleration` física completa (`ground_vehicle.cpp:105-183`). Solo
    relevante si se adopta el modelo realista.
16. **Frenado anticipado en plataforma (AM_REALISTIC)** —
    `train_cmd.cpp:394-415` (`st_max_speed`, mínimo `25·distance_to_go`).
17. **Multi-head / articulados / dual-headed** — `IsArticulatedPart`,
    `GetNextUnit`. Depende del ítem 6.
18. **Pendientes que afectan velocidad (`z_up`/`z_down` de
    `_accel_slowdown`)** — la parte de curvas es el ítem 2; la de pendientes
    requiere que el movimiento consulte `z`.
19. **Vagones en depósito / compra de vagones / refit de consist** — UI y
    comandos; depende del ítem 6.
20. **Choques (`CheckTrainCollision`) y averías** de trenes.

## Cómo detectar regresiones/omisiones nuevas

- Al implementar cualquier ítem: añadir el evento correspondiente a
  `parity/record.rs` y un chequeo en `parity/report.rs` (patrón de
  `curve_speed_penalty`/`bay_stop_position` de carretera: primero divergencia
  CONFIRMADA medida en la traza, después test de regresión).
- Los ítems 1–3 tienen hoy tests que **fijan el comportamiento divergente**
  (`train_keeps_speed_on_direction_change`,
  `showcase_train_stays_on_rail_not_station_platform`): al corregirlos hay que
  invertir esos tests, como se hizo con los de bahía en la Fase 2.
- El plan de fases con criterios de terminado está en
  `rail_debugging_plan.md`; el estado por subsistema en `rail_status.md`.
