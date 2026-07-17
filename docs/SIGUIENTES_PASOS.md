# Siguientes pasos — openttdrs

Documento vivo con **hallazgos técnicos** y **comandos**. El plan de trabajo está en
[ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md); el inventario completo de paridad en
[PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md). Índice general: [README.md](README.md).

**Auditoría #121 (cerrada):** [archive/ROADMAP_AUDITORIA_2026.md](archive/ROADMAP_AUDITORIA_2026.md)
— camino crítico `#108 → #115 → #114 → #21` ✅; host migration `#171` ✅ (ADR 0004).

**Hito actual:** 0.1 solitario · **I0–I7** hechos · **I8 (red)** MVP + host migration listen-server.
Pulido jul 2026: `--client` sin bootstrap local; dedicated isla 64² con pueblos/industrias (`--seed`); tiles `water_lock_*` vía `scripts/gen_water_lock_tiles.py`.
**Siguiente foco:** SP1 ciclo jugable ([SP1_CHECKLIST.md](SP1_CHECKLIST.md)).

---

## Prioridad inmediata (Sprint 1) — cerrado 2026-06-22

- [x] Migración save v3→v4 con test
- [x] Test `effective_road_bits` en fixture `.ottdmap`
- [x] Pasada SP2: CI + tests `command` / `preview` (ver § S1 refresh en [archive/SP2_CHECKLIST.md](archive/SP2_CHECKLIST.md))
- [x] `check.sh ci` documentado en [README.md](../README.md)

**Siguiente foco recomendado:** [SP1_CHECKLIST.md](SP1_CHECKLIST.md) (sesión manual) y [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) § Sprint 3 (visual).

---

## Estado de fases SP

| Fase | Estado | Referencia |
|------|--------|------------|
| **SP2** Construcción | ✅ Cerrado 2026-05-22 | [archive/SP2_CHECKLIST.md](archive/SP2_CHECKLIST.md) |
| **SP3** Visual | ✅ Cerrado en código (jul 2026) | [archive/ROADMAP_PARIDAD_VISUAL.md](archive/ROADMAP_PARIDAD_VISUAL.md), [archive/SP3_AUDIT_SUMMARY.md](archive/SP3_AUDIT_SUMMARY.md) |
| **SP4** Pulido | ✅ Cerrado 2026-06-22 | ROADMAP_SPRINTS S1 |
| **SP1** Ciclo jugable | 🟡 En curso | [SP1_CHECKLIST.md](SP1_CHECKLIST.md), ROADMAP S4 |

**SP3 visual vanilla:** cerrado en código (junctions slope, culling teselas+labels, industrias gfx 0–174). QA manual opcional del checklist y=3/5/7. Waypoints: posicionamiento corregido jul 2026 ([HANDOFF_WAYPOINTS_RAIL.md](HANDOFF_WAYPOINTS_RAIL.md)). Preview estación multi-tesela: sprites reales (jul 2026). Fuera de SP3: NewGRF gfx≥175.

**Terraform (paisaje):** T1–T3 implementados; gen procedural T4 MVP en `world_gen.rs` — [archive/ROADMAP_TERRAFORM.md](archive/ROADMAP_TERRAFORM.md).

**Noticias / barra inferior:** N1–N5 implementados — [archive/ROADMAP_NEWS_STATUSBAR.md](archive/ROADMAP_NEWS_STATUSBAR.md).

**Carreteras — drag / orientación (handoff IA):** fixes parciales junio 2026; usuario pidió
dejarlo — ver [ROADMAP_CARRETERAS_DRAG.md](ROADMAP_CARRETERAS_DRAG.md).

**Export `.sav` (handoff IA):** mapa+STNN+CITY+INDY+ORDL+VEHS+DATE+PLYR; horarios/grupos solo en JSON —
ver [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md).

**Señales — pick en diagonal:** ✅ fix jul 2026 (tap ancla press + seed preferido) —
[SENALES_FERROVIARIAS.md §11](SENALES_FERROVIARIAS.md#11-fantasma-vs-colocación-en-vía-diagonal-cerrado-jul-2026).

**Menú de inicio:** pantallas raíz/nueva partida, cargar desde menú, población procedural y lagos — ver
[archive/ROADMAP_MAIN_MENU.md](archive/ROADMAP_MAIN_MENU.md) (fase 2 cerrada; pendiente preferencias resolución/idioma).

**Refactor módulos (jun 2026):** `ui/main_menu/`, `bootstrap/procedural_population/`, `command/transport/`, `command/tests/` — sin cambio de API pública; CI usa perfil nextest `ci` en `.config/nextest.toml`.

**Toolbar rail:** `RailConvert` es MVP visible (ciclo de railtypes). `RailRemove`, waypoint y señales ya cableados.

---

## Hallazgos fijos (no olvidar)

1. **Cruces a nivel** — no usar bits 0–3 de `m5` como road bits; eje en bit 0. Ver [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).
2. **MAPT + `m5`** — byte MAPT crudo necesario para túneles/puentes vs `MP_ROAD`.
3. **`road_tx` ↔ `road_ty`** — intercambio respecto a `RoadDir` para isometría del cliente (validado visualmente).
4. **Sprite coal mine** — ID correcto 2013; verificación en [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md).
5. **Fuente UI** — `static/fonts/DejaVuSansMono.ttf` (no en `assets/` ignorado).
6. **Estación tren** — multi-tesela `PlaceRailStationArea`, ventana selección, cruce X\|Y en intersecciones (save v3).
7. **Vía Horz/Vert/X/Y** — `PlaceRailBits` solo en tesela del cursor (fantasma = colocación); uniones automáticas solo con autoraíl. Ver [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md).
8. **Señales** — pick en vecindario + offset sub-tesela; se conservan al cruzar diagonales. Pick diagonal tap/seed ✅ jul 2026 — [SENALES §11](SENALES_FERROVIARIAS.md#11-fantasma-vs-colocación-en-vía-diagonal-cerrado-jul-2026).

---

## Comandos útiles

```bash
# Mapa desde save OpenTTD
python3 scripts/parse_sav.py partida.sav assets/maps/mapa.ottdmap
OTTDMAP_FILE=assets/maps/mapa.ottdmap cargo run -p openttdrs-client

# Demo procedural
cargo run -p openttdrs-client

# CI local
bash scripts/check.sh ci

# Checklist visual SP3
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client

# Captura automatizada (ghost / herramientas)
OPENTTDRS_MAP_SHOT=/tmp/shot.png OPENTTDRS_MAP_SHOT_TOOL=rail_station cargo run -p openttdrs-client

# DevBot — ¿cargó, descargó, cuánto ganó? (headless, sin UI)
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_line --vehicle 1 --ticks 12000 --require-delivery
cargo test -p openttdrs-core dev_metrics
# Referencia completa: docs/DEV_BOT.md
```

---

## Si algo se pierde

1. [docs/README.md](README.md) — índice completo
2. Comentarios y submódulos en `crates/openttdrs-core/src/command/` (`transport/`, `tests/`) y `crates/openttdrs-client/src/ui/` (`main_menu/`, `toolbar/`)
3. Upstream (pin #109): `bash scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/` @ [`parity/openttd-reference.json`](parity/openttd-reference.json)

---

*Última actualización: jul 2026 (DevBot / `dev_metrics`; refactor módulos, menú fase 2)*
