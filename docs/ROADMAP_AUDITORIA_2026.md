# Roadmap auditoría Rust + Bevy (issue #121)

Documento vivo sincronizado con [#121](https://github.com/cavazquez/openttdrs/issues/121).
Baseline original: `main` @ `7e731b32` (2026-07-15). Actualizado: 2026-07-16.

## Camino crítico

```text
#107 ✅ → #108 ⬜ → #115 ⬜ → #114 ⬜ → #21 ⬜
```

En paralelo (ya avanzado): `#104 → #106` ✅ · save/load `#122`/`#123`/`#118` ✅ · UI `#113`/`#145` ✅.

## Estado por fase

### Fase 0 — Baseline y reproducibilidad

| Issue | Estado | Notas |
|-------|--------|-------|
| #103 rustdoc CI | ✅ | Cerrado |
| #108 hash por tick | ⬜ | Bloqueante de equivalencia / red |
| #109 pin OpenTTD | ⬜ | Bloquea #110 / #119 |
| #110 oráculo independiente | ⬜ | Tras #109 |
| #119 tablas generadas | ⬜ | Tras #109 |
| #125 docs tick/carga | ⬜ | Idealmente tras #109 |

### Fase 1 — Gobierno

| Issue | Estado |
|-------|--------|
| #117 contribución / ADRs | ⬜ |
| #120 alinear `check.sh ci` ↔ GHA | ⬜ |
| #106 cargo-audit / deny | ✅ |

### Fase 2 — Límites arquitectónicos

| Issue | Estado |
|-------|--------|
| #114 mutaciones cliente fuera de Command | ⬜ |
| #111 estado persistido vs runtime | ✅ |
| #112 fases de `sim_step` | ✅ |

### Fase 3 — Tiempo y determinismo

| Issue | Estado |
|-------|--------|
| #107 persistir RNG CargoDist | ✅ |
| #108 hash global | ⬜ |
| #115 orden HashMap | ⬜ (tras #108) |

### Fase 4 — Subsistemas funcionales

| Issue | Estado |
|-------|--------|
| #118 errores bootstrap load | ✅ |
| #122 sanitizar `current_order` | ✅ |
| #123 gamma SAV tipado | ✅ |
| #149 codec ORDL simétrico | ✅ |
| #21 red multiplayer | ⬜ (bloqueada por #108/#114/#115) |

### Fase 5 — Bevy / presentación

| Issue | Estado |
|-------|--------|
| #113 partir `ClientUiPlugin` | ✅ |
| #145 teardown InGame declarativo | ✅ |
| #124 schedules / sets vacíos | ⬜ |

### Fase 6 — Rendimiento

| Issue | Estado |
|-------|--------|
| #116 benchmarks headless | ⬜ |

### Fase 7 — Endurecimiento

| Issue | Estado |
|-------|--------|
| #104 quick-xml | ✅ |
| #105 límites save/input | ✅ |
| #106 gates dependencias | ✅ |

## Refactors de modularización relacionados (#144)

| Issue | Estado |
|-------|--------|
| #139 partir `command/tests/rail` | ✅ |
| #151 `SandboxMap` | ✅ |
| #152 `SimHarness` | ✅ |
| #156 engine catalog | ⬜ |
| #157 prelude API raíz | ⬜ |
| #138 escenarios parity | ⬜ |
| #135 sprites NewGRF | ⬜ |

## Definition of Done global (#121)

- [x] Fmt/check/clippy/tests verdes en el baseline actual (suite ampliada desde la auditoría).
- [x] Auditoría de dependencias con política (`#104`/`#106`).
- [ ] Mismo seed/comandos/ticks → hashes idénticos tras save/load (`#108`, refuerzo `#107` ✅).
- [ ] Referencia OpenTTD y oráculo independientes (`#109`/`#110`).
- [ ] Refactors con equivalencia tick-a-tick (`#108`).
- [ ] Benchmarks con baseline (`#116`).
- [ ] ADRs / gobierno (`#117`).
- [ ] `#21` solo con bloqueantes de determinismo cerrados.

## Próximo foco recomendado

1. **#108** — hash estable + repetibilidad (desbloquea #115 y equivalencia de refactors).
2. **#109** — pin/manifiesto OpenTTD (desbloquea paridad tooling).
3. **#114** — inventario mutaciones cliente (camino a #21).
4. **#120** / **#117** — quick wins de CI y gobierno.
