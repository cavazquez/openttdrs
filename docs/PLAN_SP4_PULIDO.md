# Plan SP4 — Pulido y deuda técnica

Documento operativo para cerrar **SP4** antes de retomar **SP3** (visual) y **SP1** (gameplay).
Orden acordado: **SP4 → SP3 → SP1** (hito 0.1 en solitario).

**Relacionado:** [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md), [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md),
[PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md), [PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md](PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md).

---

## 1. Objetivo SP4

Estabilizar el repo **sin** abrir features grandes: deuda modular, saves, tests de regresión
sin Bevy, y docs al día. Tras SP4 el mapa demo y CI deben seguir verdes mientras SP3/SP1 avanzan.

---

## 2. Checklist SP4

### SP4.1 — CI y calidad (hecho)

- [x] `scripts/check.sh ci` alineado con CI (fmt, clippy, tests, TNBP, golden, py_compile).
- [x] Bootstrap demo: paradas con `PlaceStationDir` en hierba (no sobre carretera).

### SP4.2 — Cliente modular

- [x] Extraer comprobación de assets de `main.rs` → `startup/assets_check.rs`.
- [ ] Hotkeys / sim tick: ya en `persistence.rs` y `simulation.rs` (sin mover salvo necesidad).
- [ ] Debug gizmos: `debug_gizmos.rs` (OK separado).
- [ ] Animación agua: `render/water.rs` (OK separado).

### SP4.3 — Saves JSON (I7)

- [x] API pública `CURRENT_SAVE_VERSION` + hook `migrate_loaded_state` en `save.rs`.
- [ ] Cuando el esquema cambie: bump versión, implementar rama `n → n+1`, test de migración.
- [x] Carga legado: JSON plano sin envoltorio + campos serde `default` (tests existentes).

### SP4.4 — Tests core sin Bevy

- [x] Fixtures `.ottdmap`: m3, SP3 checklist, slope lab, TNBP, STXY (tests en `openttdrs-core/tests/`).
- [ ] Opcional: test `effective_road_bits` sobre fixture real (hoy en `sprites/road.rs` del cliente).

### SP4.5 — Documentación

- [x] Este plan + orden SP4→SP3→SP1 en [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md).
- [x] Depósito carretera documentado para SP3 ([PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md](PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md)).

---

## 3. Qué sigue: SP3 (después de SP4)

Prioridad visual según huecos reales en [PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md):

| PR | Tema | Doc |
|----|------|-----|
| **SP3-depot** | Depósito carretera `RemapCoords` (fase A–B) | [PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md](PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md) |
| SP3-industry | gfx ≥ 120, HUD aviso | [PLAN_SP3_CASAS_INDUSTRIAS.md](PLAN_SP3_CASAS_INDUSTRIAS.md) |
| SP3-capture | Captura manual checklist (opcional) | [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) |

Fases SP3.0–SP3.6 del plan visual están **cerradas en código** salvo depósito e industrias extendidas.

---

## 4. Qué sigue: SP1 (después de SP3)

| PR | Tema |
|----|------|
| **SP1-tests** | Integración UI↔`apply_command`: bus stop, rail station, industria, depósito+vehículo |
| SP1-hud | Tooltip depósito vs parada; alertas órdenes (ampliar HUD existente) |
| SP1-flow | Revisión manual toolbar → órdenes → sim en partida nueva |

---

## 5. Comandos de regresión

```bash
bash scripts/check.sh ci
cargo run -p openttdrs-client
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client
```

---

*Última actualización: 2026-05-24 — SP4 en curso; SP3/SP1 en cola.*
