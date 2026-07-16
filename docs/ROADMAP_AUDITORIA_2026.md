# Roadmap auditoría Rust + Bevy (issue #121)

Documento vivo sincronizado con [#121](https://github.com/cavazquez/openttdrs/issues/121).
Baseline original: `main` @ `7e731b32` (2026-07-15). Actualizado: 2026-07-16.

## Camino crítico

```text
#107 ✅ → #108 ✅ → #115 ✅ → #114 ✅ → #21 ✅
```

Arquitectura red v1 (listen-server + dedicated headless, sin host migration): [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md).

Inventarios: [INVENTARIO_HASHMAP_DETERMINISMO.md](INVENTARIO_HASHMAP_DETERMINISMO.md) (#115) · [INVENTARIO_MUTACIONES_CLIENTE.md](INVENTARIO_MUTACIONES_CLIENTE.md) (#114).

En paralelo (ya avanzado): `#104 → #106` ✅ · save/load `#122`/`#123`/`#118` ✅ · UI `#113`/`#145` ✅.

## Estado por fase

### Fase 0 — Baseline y reproducibilidad

| Issue | Estado | Notas |
|-------|--------|-------|
| #103 rustdoc CI | ✅ | Cerrado |
| #108 hash por tick | ✅ | `GameState::canonical_hash` (FNV-1a, dominio `openttdrs-gs-v1`) |
| #109 pin OpenTTD | ✅ | `docs/parity/openttd-reference.json` + fetch por SHA |
| #110 oráculo independiente | ✅ | Export C++ 15.3; `road_bits_hash` alineado (`SLV_ROAD_TYPES`) |
| #119 tablas generadas | ✅ | `generated_tables_manifest.json` + check pilots |
| #125 docs tick/carga | ⬜ | Idealmente tras #109 |

### Fase 1 — Gobierno

| Issue | Estado |
|-------|--------|
| #117 contribución / ADRs | ✅ | CONTRIBUTING/SECURITY/ARCHITECTURE + ADR 0001–0002 + PR template |
| #120 alinear `check.sh ci` ↔ GHA | ✅ | `ci_python_manifest.json` + `check.sh ci-python` |
| #106 cargo-audit / deny | ✅ |

### Fase 2 — Límites arquitectónicos

| Issue | Estado |
|-------|--------|
| #114 mutaciones cliente fuera de Command | ✅ | [INVENTARIO_MUTACIONES_CLIENTE.md](INVENTARIO_MUTACIONES_CLIENTE.md) |
| #111 estado persistido vs runtime | ✅ |
| #112 fases de `sim_step` | ✅ |

### Fase 3 — Tiempo y determinismo

| Issue | Estado |
|-------|--------|
| #107 persistir RNG CargoDist | ✅ |
| #108 hash global | ✅ |
| #115 orden HashMap | ✅ | [INVENTARIO_HASHMAP_DETERMINISMO.md](INVENTARIO_HASHMAP_DETERMINISMO.md); sin BTreeMap masivo |

### Fase 4 — Subsistemas funcionales

| Issue | Estado |
|-------|--------|
| #118 errores bootstrap load | ✅ |
| #122 sanitizar `current_order` | ✅ |
| #123 gamma SAV tipado | ✅ |
| #149 codec ORDL simétrico | ✅ |
| #21 red multiplayer | ✅ | `openttdrs-net` + dedicated + `--server`/`--client` (ADR 0001) |

### Fase 5 — Bevy / presentación

| Issue | Estado |
|-------|--------|
| #113 partir `ClientUiPlugin` | ✅ |
| #145 teardown InGame declarativo | ✅ |
| #124 schedules / sets vacíos | ✅ |

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
| #156 engine catalog | ✅ |
| #157 prelude API raíz | ✅ |
| #138 escenarios parity | ✅ |
| #135 sprites NewGRF | ✅ |

## Definition of Done global (#121)

- [x] Fmt/check/clippy/tests verdes en el baseline actual (suite ampliada desde la auditoría).
- [x] Auditoría de dependencias con política (`#104`/`#106`).
- [x] Mismo seed/comandos/ticks → hashes idénticos tras save/load (`#108`, refuerzo `#107` ✅).
- [x] Referencia OpenTTD fijada (`#109`) y oráculo independiente (`#110`, ver `parity/SNAPSHOT_FIRST_DIVERGENCE.md`).
- [x] Refactors con equivalencia tick-a-tick (`#108`).
- [ ] Benchmarks con baseline (`#116`).
- [x] ADRs / gobierno (`#117`).
- [x] Bloqueantes de determinismo para `#21` cerrados (`#108`/`#114`/`#115`).
- [x] `#21` transporte TCP + flags cliente / dedicated (ADR 0001).

## Próximo foco recomendado

1. Deuda I8 de settings cliente → `Command` (lista en inventario #114).
2. Host migration post-v1 (fuera de ADR 0001).
3. Benchmarks headless (#116).
4. Regenerar pilots OpenGFX con drift (house_draw / vehicle_gfx) en PR de datos.
