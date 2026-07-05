# Paridad ferroviaria — handoff para revisión por IA avanzada

**Fecha:** 2026-07-04  
**Estado del plan:** Fases Rail 0–4 **implementadas y pusheadas** en `main`  
**Solicitud explícita del mantenedor:** una **IA avanzada** (agente con acceso al
repo, tests, `OpenTTD/` upstream y capacidad de auditoría multi-archivo) debe
**revisar este trabajo** antes de darlo por cerrado o de abrir la siguiente
oleada (consist, PBS, pathfinder YAPF, etc.).

Este documento es el punto de entrada para esa revisión. Complementa — no
reemplaza — `rail_debugging_plan.md`, `rail_status.md` y los reportes
generados.

---

## 1. Qué se completó (resumen ejecutivo)

| Fase | Entregable principal | Commit de referencia (aprox.) |
|------|----------------------|-------------------------------|
| Rail 0 | Auditoría + 4 docs `rail_*.md` | auditoría previa a Rail 1 |
| Rail 1 | Traza `rail` + escenario `train_line` | `58e5394` |
| Rail 2 | `parity_diff` con subsistemas rail | `b9a13c4` |
| Rail 3A | Golden `train_movement_golden.json` | `723bbd7` |
| Rail 3B | Aceleración AM_ORIGINAL + frenado curva | `b9f1472` |
| Rail 3C | Entrada a plataforma + `at_platform` | `b84bfbb` |
| Rail 3D | `train_signal` + presignals → BLOCK | `f3dee9c` |
| Rail 3E | Render/traza alineados + evaluación subcoord | `6e12bec` |
| Rail 4 | Reportes regenerados + script | `a1276af` |

**Supuesto estructural no negociable en esta oleada:** el tren es un **vehículo
puntual** (una tesela, un sprite). No hay consist ni longitud de tren.

---

## 2. Por qué hace falta una revisión externa (IA avanzada)

La implementación fue incremental, con tests de regresión y reportes automáticos,
pero **no** hubo comparación tick-a-tick contra un save OpenTTD real ni revisión
independiente del diseño. Riesgos que una IA avanzada debe auditar:

1. **Falsos verdes** — Los chequeos en `parity/report.rs` miden la sim Rust
   contra sí misma o contra heurísticas del C++ portado; no capturan divergencia
   frente a una partida OpenTTD grabada.
2. **Cobertura de escenarios** — Solo tres escenarios headless (`truck_bay`,
   `train_line`, `train_signal`). Curvas con piezas diagonales, túneles
   completos, puentes con vano, saves importados y 2+ trenes en red no están en
   el runner.
3. **Decisiones documentadas ≠ paridad** — ENTRY/EXIT/COMBO degradados a BLOCK,
   PBS excluido, subcoordenadas en eje central, túnel sin ocultar: son
   **decisiones conscientes**, no equivalencia con OpenTTD.
4. **Tick rate 5 Hz** — Toda comparación absoluta de velocidad/timing es
   inválida; la revisión debe usar unidades relativas o explicitar el sesgo
   (`tick_rate_decision.md`).
5. **Id compartido camión/tren** — Corregido en Rail 4 (`trace_has_train` usa
   `rail.is_some()`), pero conviene verificar que ningún otro chequeo asuma
   `vehicle.id == 1` como tren.

---

## 3. Comandos de verificación (ejecutar primero)

```bash
cd openttdrs

# Suite completa (obligatorio: debe quedar verde)
./scripts/check.sh

# Regenerar reportes markdown de divergencias
./scripts/regenerate_parity_reports.sh

# Trazas JSONL de referencia
cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario train_line --ticks 600 --out /tmp/train_line.jsonl \
    --divergence-report docs/parity/train_line_divergences.md

cargo run -p openttdrs-core --bin parity_runner -- \
    --scenario train_signal --ticks 50 --out /tmp/train_signal.jsonl
```

Tests de regresión rail más relevantes:

```bash
cargo test -p openttdrs-core --test golden_rail
cargo test -p openttdrs-core --test parity_system
cargo test -p openttdrs-client sprite_selection_uses_extrapolated_pose_for_train
```

---

## 4. Mapa de archivos que la IA debe leer

| Área | Rutas |
|------|--------|
| Plan y estado | `docs/parity/rail_debugging_plan.md`, `rail_status.md`, `rail_unknown_features.md`, `rail_openttd_mapping.md`, `rail_render_evaluation.md` |
| Reportes vivos | `docs/parity/train_line_divergences.md`, `divergences_found.md` |
| Paridad core | `crates/openttdrs-core/src/parity/{record,tracer,scenario,report,diff}.rs` |
| Movimiento tren | `vehicle.rs`, `engine.rs`, `train_movement.rs`, `road_movement.rs`, `sim_step.rs` |
| Señales | `rail_signals.rs` |
| Estaciones | `station.rs` |
| Runner / diff | `bins/parity_runner.rs`, `parity_diff.rs` |
| Golden | `tests/golden_rail.rs`, `tests/fixtures/parity/train_movement_golden.json` |
| Render | `openttdrs-client/src/render_trace.rs`, `render/vehicles.rs` |
| OpenTTD ref | `OpenTTD/src/train_cmd.cpp`, `signal.cpp`, `vehicle.cpp`, `economy.cpp` |

---

## 5. Checklist de revisión para la IA avanzada

Marcar cada ítem con **OK**, **GAP** o **RIESGO** y citar evidencia (archivo:línea,
test, o tick de traza).

### 5.1 Correctitud de la simulación

- [ ] `train_acceleration` / `accelerate_train_speed` / `decelerate_train_speed`
      coinciden con `train_cmd.cpp` AM_ORIGINAL (golden Kirby 300/47 → accel 24).
- [ ] `_accel_slowdown` en giros 45° y reversa (`set_direction_with_curve_penalty`).
- [ ] `rail_station_stop_tile` + carga solo con `at_platform: true` en traza.
- [ ] Bloqueo por señal v1: `train_blocked_by_signal`, `rail_block_ahead`, eventos
      `SignalWait*` en escenario `train_signal`.
- [ ] Decisión ENTRY ignorado / EXIT·COMBO sin propagación está documentada y es
      coherente con `entry_signal_does_not_block_train`.

### 5.2 Instrumentación y reportes

- [ ] `train_line_divergences.md` refleja el output actual de
      `detect_known_divergences` (regenerar si difiere).
- [ ] `divergences_found.md` **no** incluye chequeos `train_*` (solo carretera).
- [ ] `trace_has_train` no dispara en `truck_bay`.
- [ ] Divergencia `train_diagonal_subcoord_approximation` en esquina (12,6) es
      aceptable como cosmética o requiere fix.

### 5.3 Cobertura y huecos

- [ ] Revisar ítems abiertos en `rail_unknown_features.md` (consist, PBS, carga
      gradual, depósito 37 ticks, YAPF, túnel oculto, railtypes).
- [ ] Pathfinder no cruza vano de puente rail (`golden_rail` lo documenta).
- [ ] Comparar traza `train_line` con expectativa manual en curva L y estaciones A/B.

### 5.4 Render e interpolación

- [ ] `train_render_subtile_consistency`: traza JSONL = `vehicle_subtile` a α=0.
- [ ] Extrapolación sin saltos (`train_line_extrapolation_subtile_is_monotonic`).
- [ ] Sprite usa pose extrapolada (`sprite_selection_uses_extrapolated_pose_for_train`).
- [ ] Opcional: correlacionar `OPENTTDRS_RENDER_TRACE` CSV con JSONL (ver
      `rail_render_evaluation.md`).

### 5.5 Documentación

- [ ] Los cuatro `rail_*.md` son consistentes entre sí y con `status.md`.
- [ ] No hay afirmaciones «validado tick-a-tick OpenTTD» donde solo hay golden
      de tablas o tests propios.

---

## 6. Criterios de salida de la revisión

La IA avanzada debe producir un **informe breve** (puede ser un issue, un
comentario en PR o un archivo `docs/parity/RAIL_REVIEW_<fecha>.md`) con:

1. **Veredicto global:** aceptable para continuar / requiere correcciones /
   requiere replanteo (consist/PBS).
2. **Lista priorizada** de GAPs encontrados (P0 bloqueante, P1 paridad visible,
   P2 cosmético/documentación).
3. **Propuesta de siguiente oleada** (si aplica): qué fase Rail 5+ o qué ítem de
   `rail_unknown_features.md` atacar primero.
4. **Confirmación** de que `./scripts/check.sh` y `./scripts/regenerate_parity_reports.sh`
   se ejecutaron en el entorno de revisión.

Si la revisión encuentra regresiones en chequeos que hoy son verdes, debe:

- reproducir con comando exacto;
- proponer fix o ajuste del chequeo (si el test estaba mal planteado);
- **no** asumir que «CONFIRMADA en la traza» en el markdown implica bug sin leer
  `fix_phase2` (muchas entradas son divergencias aceptadas).

---

## 7. Divergencias aceptadas (no reabrir sin decisión del mantenedor)

Estas figuran como **CONFIRMADA** o **DECIDIDO** en los reportes y son
intencionales:

| Id | Motivo |
|----|--------|
| `tick_rate` | Sim a 5 Hz por decisión de producto |
| `instant_loading` | Carga en un tick (carretera y tren) |
| `train_diagonal_subcoord_approximation` | Render en eje central, no `_vehicle_subcoord` completo |
| PBS / presignals reales | Fuera de alcance Rail 0–4 |
| Consist / vagones | Modelo puntual |

---

## 8. Contacto con el resto del proyecto

- Paridad **carretera** (videos `openttd.webm` / `opentddrs.webm`): `status.md`,
  `vehicle_station_entry.md`, escenario `truck_bay`.
- Menús y UI: `ROADMAP_MENUS_UI.md` (otro handoff IA).
- Índice general de docs: `docs/README.md`.

---

*Fin del handoff. La próxima IA que retome ferrocarriles debe leer este archivo
antes de modificar `parity/report.rs`, `scenario.rs` o la física del tren.*
