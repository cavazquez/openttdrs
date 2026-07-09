# Características ferroviarias de OpenTTD aún no consideradas en openttdrs

Subsistemas y reglas del original detectados durante la auditoría de la Fase
Rail 0 que hoy no tienen equivalente en la sim Rust. Priorizados por impacto
en la paridad de movimiento y estaciones (los casos más visibles).

Supuesto histórico (Rail 0): tren puntual. **Fase 1 estructural** añadió consist
(`next_unit` / longitud / ocupación multi-tesela). Varios ítems abajo ya están
parcialmente resueltos; se mantienen tachados o anotados.

## Prioridad alta (afectan movimiento y estaciones)

1. ~~**Aceleración de tren `AM_ORIGINAL`**~~ — **Resuelto (Rail 3B)**.
   `engine.rs::train_acceleration` + `accelerate_train_speed` / `decelerate_train_speed`
   (Kirby: 300 HP / 47 t → `accel = 24`).
2. ~~**Frenado por curva `_accel_slowdown`**~~ — **Resuelto (Rail 3B)**.
   `set_direction_with_curve_penalty` y `apply_immediate_train_turnaround`.
3. ~~**Entrada a la plataforma + punto de parada**~~ — **Resuelto (Rail 3C)**.
   `rail_station_stop_tile` + `at_platform` en traza.
4. ~~**Carga/descarga gradual**~~ — **Resuelto (Fase 2 estructural)**:
   `cargo_packet.rs` + `load_unload_speed`; golden `instant_loading=false`.
5. **Espera y frames de depósito** — `CheckTrainStayInDepot`
   (`train_cmd.cpp:2354-2427`, espera ~37 ticks y chequea señales/reserva
   antes de salir), `_fractcoords_enter` / `_vehicle_initial_*_fract`
   (`rail_cmd.cpp:2975-2991`, `train_cmd.cpp:54-56`) y `TicksToLeaveDepot`
   (salida encadenada de vagones). Hoy la salida es inmediata y sin frames.

## Prioridad media

6. ~~**Consist: locomotora + vagones**~~ — **Resuelto (Fase 1 estructural)**:
   `train_consist.rs`, comandos de enganche, save v12, import `.sav` con
   vagones. Pendiente fino: articulados / dual-headed (ítem 17).
7. ~~**Reservas de camino (PBS)**~~ — **MVP (Fase 3 estructural)**:
   `rail_pbs.rs` (TryReserve, path signals, plataforma, huella consist,
   wormholes). Escenario `train_pbs`. Pendiente: golden tick-a-tick vs
   OpenTTD y `FollowTrainReservation` fino.
8. ~~**Semántica de presignals ENTRY/EXIT/COMBO**~~ — **Decidido (Rail 3D)**:
   se codifican en saves pero **no tienen semántica de presignal** en la sim
   v1. `SIGTYPE_ENTRY` se ignora al bloquear (`train_blocked_by_signal`);
   EXIT y COMBO se tratan como BLOCK sin propagación por segmento
   (`entry_signal_does_not_block_train`). Path/PBS: ver ítem 7.
9. **Túneles/puentes en tránsito** — ocultamiento del tren
   (`_tunnel_visibility_frame` {12,8,8,12}, `tunnelbridge_cmd.cpp:1956`),
   límite de velocidad del puente (`cur_speed = min(cur_speed,
   BridgeSpec->speed)`, `:2028-2033` y `train_cmd.cpp:427-429`). Hoy el tramo
   se atraviesa como vía normal y el tren queda visible.
10. **Subcoordenadas por pieza `_vehicle_subcoord`** —
    `vehicle.cpp:3359-3392`: posición (x, y) y dirección exactas al entrar a
    cada track. La sim usa eje central en rectas (`train_straight_subtile`);
    **evaluado en Rail 3E** (`rail_render_evaluation.md`): alineado en X/Y,
    divergencia cosmética documentada en piezas diagonales puras.
11. **Pathfinder YAPF con penalizaciones y reserva** —
    `yapf_rail.cpp`. **MVP (Fase 3):** `pathfinder/yapf.rs` +
    `next_rail_trackdir_yapf` / `extend_rail_path_yapf`; PBS reserva el path.
    Falta: segmentos con caché, penalizaciones de plataforma/curva 90° finas.
12. **Reversa con coste/chequeos** — `ReverseTrainDirection` (news, PBS,
    `reverse_ctr`). Hoy la reversa automática y manual son instantáneas.

## Prioridad baja (dependen de decisiones estructurales o son cosméticas)

13. ~~**Railtypes** (normal/eléctrico)~~ — **MVP (Fase 5)**: `rail_type.rs`,
    `ConvertRail`, compat eléctricos 110–113. Catenaria wires + PCP/PPP +
    túnel/puente; `OPENTTDRS_HIDE_CATENARY` /
    `OPENTTDRS_TRANSPARENT_CATENARY`. Pendiente: pendientes/nieve tipadas
    mono/maglev, UI de transparencia, `curve_speed` por railtype.
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
- Tras Rail 4, una **IA avanzada debe auditar** el trabajo según
  [`RAIL_REVIEW_HANDOFF.md`](RAIL_REVIEW_HANDOFF.md) antes de abrir la siguiente
  oleada ferroviaria.
