# Características ferroviarias de OpenTTD aún no consideradas en openttdrs

**Madurez canónica:** [rail_status.md](rail_status.md). Índice: [MAPPING.md](MAPPING.md).
Road: [unknown_features.md](unknown_features.md).

Subsistemas y reglas del original detectados durante la auditoría de la Fase
Rail 0. Priorizados por impacto en movimiento y estaciones. Muchos ítems ya
están ~~tachados~~ (implementados).

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
5. ~~**Espera y frames de depósito**~~ ✅ `tick_train_stay_in_depot`
   (~37 ticks + chequeo de boca; `depot_leave_cleared`). Fractcoords de pose
   ya existían. Residual: `TicksToLeaveDepot` encadenado fino de vagones.
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
    Golden tick-a-tick interno: `train_pbs_tick_golden.json` (`golden_pbs_tick`).
    Residual: captura binaria vs OpenTTD; caché de segmentos.
    → [#97](https://github.com/cavazquez/openttdrs/issues/97).
12. ~~**Reversa con coste/chequeos**~~ ✅ `TurnAroundVehicle` pone
    `cur_speed=0`, limpia PBS, reintenta reserva y news `PbsStuck` si aplica.
    → [#98](https://github.com/cavazquez/openttdrs/issues/98).

## Prioridad baja (dependen de decisiones estructurales o son cosméticas)

13. ~~**Railtypes** (normal/eléctrico + residual #99)~~ ✅ MVP Fase 5–6 +
    `ACCEL_SLOWDOWN` por railtype; pendientes tipadas mono/maglev. Nieve plana
    1037/1038 sin asset tipado (queda clásica).
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
17. ~~**Multi-head / dual-headed**~~ ✅ Spawn de cabina trasera +
    `other_multiheaded_part` + potencia½ por cabina (vanilla). Residual:
    articulados NewGRF / Action0 `0x13`.
    → [#100](https://github.com/cavazquez/openttdrs/issues/100).
18. ~~**Pendientes que afectan velocidad (`z_up`/`z_down` de
    `_accel_slowdown`)**~~ ✅ `affect_speed_by_z_change` + `sync_train_slope_speed`
    con Z en píxeles (`slope_pixel_z` ≈ `GetSlopePixelZ`) al avanzar progreso o
    cruzar tesela.
19. ~~**Vagones en depósito / compra / refit**~~ ✅ Comandos + UI compra/refit
    + botón «Desenganchar». Residual: insertar en medio del consist.
    → [#101](https://github.com/cavazquez/openttdrs/issues/101).
20. ~~**Choques (`CheckTrainCollision`)**~~ ✅ MVP (`train_collision.rs`); averías ya parciales.

## Issues de seguimiento (jul 2026)

| Ítem | Issue | Notas |
|------|-------|--------|
| 5 | [#96](https://github.com/cavazquez/openttdrs/issues/96) | ✅ Espera ~37 ticks (+ residual `TicksToLeaveDepot`) |
| 11 (+7 residual) | [#97](https://github.com/cavazquez/openttdrs/issues/97) | ✅ Golden tick interno; residual vs OpenTTD |
| 12 | [#98](https://github.com/cavazquez/openttdrs/issues/98) | ✅ Reversa + PBS/news |
| 13 residual | [#99](https://github.com/cavazquez/openttdrs/issues/99) | ✅ Curvas + pendientes tipadas |
| 17 | [#100](https://github.com/cavazquez/openttdrs/issues/100) | ✅ Dual-headed vanilla |
| 19 | [#101](https://github.com/cavazquez/openttdrs/issues/101) | ✅ Compra/refit + desenganchar UI |

## Cómo detectar regresiones/omisiones nuevas

- Al implementar cualquier ítem: añadir el evento correspondiente a
  `parity/record.rs` y un chequeo en `parity/report.rs` (patrón de
  `curve_speed_penalty`/`bay_stop_position` de carretera: primero divergencia
  CONFIRMADA medida en la traza, después test de regresión).
- Los ítems 1–3 tienen hoy tests que **fijan el comportamiento divergente**
  (`train_keeps_speed_on_direction_change`,
  `showcase_train_stays_on_rail_not_station_platform`): al corregirlos hay que
  invertir esos tests, como se hizo con los de bahía en la Fase 2.
- Plan de fases (archivo): [`rail_debugging_plan.md`](rail_debugging_plan.md);
  estado vivo en [`rail_status.md`](rail_status.md).
- Tras Rail 4, auditar según
  [`RAIL_REVIEW_HANDOFF.md`](RAIL_REVIEW_HANDOFF.md) (stub → archive) antes de
  abrir la siguiente oleada ferroviaria.
