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
   → [#96](https://github.com/cavazquez/openttdrs/issues/96).

## Prioridad media

6. ~~**Consist: locomotora + vagones**~~ — **Resuelto (Fase 1 estructural)**:
   `train_consist.rs`, comandos de enganche, save v12, import `.sav` con
   vagones. Pendiente fino: articulados / dual-headed (ítem 17).
7. ~~**Reservas de camino (PBS)**~~ — **MVP (Fase 3 + #54)**:
   `rail_pbs.rs` + `follow_train_reservation` + traza/golden interno
   `train_pbs_golden.json`. Pendiente: golden tick-a-tick vs OpenTTD (ítem 11).
8. ~~**Semántica de presignals ENTRY/EXIT/COMBO**~~ — **Decidido (Rail 3D)**:
   se codifican en saves pero **no tienen semántica de presignal** en la sim
   v1. `SIGTYPE_ENTRY` se ignora al bloquear (`train_blocked_by_signal`);
   EXIT y COMBO se tratan como BLOCK sin propagación por segmento
   (`entry_signal_does_not_block_train`). Path/PBS: ver ítem 7.
9. ~~**Túneles/puentes en tránsito**~~ ✅ Ocultamiento
   (`tunnel_hides_train_at_progress` + `vehicle_hidden_in_tunnel` / cliente) y
   tope de puente (`bridge_max_speed_for_tile` en `update_movement_speed`).
10. **Subcoordenadas por pieza `_vehicle_subcoord`** —
    `vehicle.cpp:3359-3392`: posición (x, y) y dirección exactas al entrar a
    cada track. La sim usa eje central en rectas (`train_straight_subtile`);
    **evaluado en Rail 3E** (`rail_render_evaluation.md`): alineado en X/Y,
    divergencia cosmética documentada en piezas diagonales puras.
    → no abrir issue salvo que se priorice estética diagonal.
11. ~~**Pathfinder YAPF con penalizaciones y reserva**~~ — **MVP (Fase 3 + #53 slice)**:
    `pathfinder/yapf.rs` + golden estático `yapf_routes_golden.json`.
    Falta: tick-a-tick vs OpenTTD; caché de segmentos; penalizaciones finas
    → [#97](https://github.com/cavazquez/openttdrs/issues/97).
12. **Reversa con coste/chequeos** — `ReverseTrainDirection` (news, PBS,
    `reverse_ctr`). Hoy la reversa automática y manual son instantáneas.
    → [#98](https://github.com/cavazquez/openttdrs/issues/98).

## Prioridad baja (dependen de decisiones estructurales o son cosméticas)

13. ~~**Railtypes** (normal/eléctrico)~~ — **MVP (Fase 5)**: `rail_type.rs`,
    `ConvertRail`, compat eléctricos 110–113. Catenaria wires + PCP/PPP +
    estación/túnel/puente; TO_CATENARY persistente desde Ajustes y overrides
    `OPENTTDRS_HIDE_CATENARY` / `OPENTTDRS_TRANSPARENT_CATENARY`. Pendiente:
    pendientes/nieve tipadas mono/maglev, `curve_speed` por railtype
    → [#99](https://github.com/cavazquez/openttdrs/issues/99).
14. ~~**Ownership por tile de vía**~~ ✅ `m1` = compañía activa en
    `PlaceRail` / depósito / túnel / puente.
15. **AM_REALISTIC + `GetCurveSpeedLimit`** — `train_cmd.cpp:312-381`
    (límites 61 / 88 / `232-(13-n)²`, tilt +20 %, `curve_speed_mod` por motor)
    y `GetAcceleration` física completa (`ground_vehicle.cpp:105-183`). Solo
    relevante si se adopta el modelo realista. **Diferido** (sin issue hasta
    decidir `AM_REALISTIC`).
16. **Frenado anticipado en plataforma (AM_REALISTIC)** —
    `train_cmd.cpp:394-415` (`st_max_speed`, mínimo `25·distance_to_go`).
    Depende del ítem 15.
17. **Multi-head / articulados / dual-headed** — `IsArticulatedPart`,
    `GetNextUnit`. Depende del ítem 6.
    → [#100](https://github.com/cavazquez/openttdrs/issues/100).
18. ~~**Pendientes que afectan velocidad (`z_up`/`z_down` de
    `_accel_slowdown`)**~~ ✅ `affect_speed_by_z_change` + `sync_train_slope_speed`
    con Z en píxeles (`slope_pixel_z` ≈ `GetSlopePixelZ`) al avanzar progreso o
    cruzar tesela.
19. **Vagones en depósito / compra de vagones / refit de consist** — UI y
    comandos; depende del ítem 6.
    → [#101](https://github.com/cavazquez/openttdrs/issues/101).
20. ~~**Choques (`CheckTrainCollision`)**~~ ✅ MVP (`train_collision.rs`); averías ya parciales.

## Issues de seguimiento (jul 2026)

| Ítem | Issue | Notas |
|------|-------|--------|
| 5 | [#96](https://github.com/cavazquez/openttdrs/issues/96) | Espera ~37 ticks + frames de salida |
| 11 (+7 residual) | [#97](https://github.com/cavazquez/openttdrs/issues/97) | Golden tick-a-tick YAPF/PBS vs OpenTTD |
| 12 | [#98](https://github.com/cavazquez/openttdrs/issues/98) | Reversa con coste / news / PBS |
| 13 residual | [#99](https://github.com/cavazquez/openttdrs/issues/99) | Mono/maglev nieve + `curve_speed` |
| 17 | [#100](https://github.com/cavazquez/openttdrs/issues/100) | Dual-headed / articulados |
| 19 | [#101](https://github.com/cavazquez/openttdrs/issues/101) | Compra/refit vagones en depósito |

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
