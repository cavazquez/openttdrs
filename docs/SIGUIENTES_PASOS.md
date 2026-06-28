# Siguientes pasos — openttdrs

Documento vivo con **hallazgos técnicos** y **comandos**. El plan de trabajo está en
[ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md); el inventario completo de paridad en
[PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md). Índice general: [README.md](README.md).

**Hito actual:** 0.1 solitario · **I0–I7** hechos · **I8 (red)** backlog post-0.1.

---

## Prioridad inmediata (Sprint 1) — cerrado 2026-06-22

- [x] Migración save v3→v4 con test
- [x] Test `effective_road_bits` en fixture `.ottdmap`
- [x] Pasada SP2: CI + tests `command` / `preview` (ver § S1 refresh en [SP2_CHECKLIST.md](SP2_CHECKLIST.md))
- [x] `check.sh ci` documentado en [README.md](../README.md)

**Siguiente foco recomendado:** [SP1_CHECKLIST.md](SP1_CHECKLIST.md) (sesión manual) y [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) § Sprint 3 (visual).

---

## Estado de fases SP

| Fase | Estado | Referencia |
|------|--------|------------|
| **SP2** Construcción | ✅ Cerrado 2026-05-22 | [SP2_CHECKLIST.md](SP2_CHECKLIST.md) |
| **SP3** Visual | 🟡 ~90 % | [ROADMAP_PARIDAD_VISUAL.md](ROADMAP_PARIDAD_VISUAL.md), [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) |
| **SP4** Pulido | ✅ Cerrado 2026-06-22 | ROADMAP_SPRINTS S1 |
| **SP1** Ciclo jugable | 🟡 En curso | [SP1_CHECKLIST.md](SP1_CHECKLIST.md), ROADMAP S4 |

**Huecos visuales reales (SP3):** junctions vía en pendiente; depósito carretera; culling global; industrias gfx ≥ 120.

**Terraform (paisaje):** T1–T3 implementados; gen procedural T4 MVP en `world_gen.rs` — [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md).

**Noticias / barra inferior:** N1–N5 implementados — [ROADMAP_NEWS_STATUSBAR.md](ROADMAP_NEWS_STATUSBAR.md).

**Carreteras — drag / orientación (handoff IA):** fixes parciales junio 2026; usuario pidió
dejarlo — ver [ROADMAP_CARRETERAS_DRAG.md](ROADMAP_CARRETERAS_DRAG.md).

**Señales — pick en diagonal (handoff):** fantasma OK, clic en tesela vecina — ver
[SENALES_FERROVIARIAS.md §11](SENALES_FERROVIARIAS.md#11-bug-abierto-fantasma-vs-colocación-en-vía-diagonal-jun-2026).

**Menú de inicio:** pantallas raíz/nueva partida, cargar desde menú, población procedural y lagos — ver
[ROADMAP_MAIN_MENU.md](ROADMAP_MAIN_MENU.md) (fase 2 cerrada; pendiente preferencias resolución/idioma).

**Refactor módulos (jun 2026):** `ui/main_menu/`, `bootstrap/procedural_population/`, `command/transport/`, `command/tests/` — sin cambio de API pública; CI usa perfil nextest `ci` en `.config/nextest.toml`.

**Toolbar rail sin comando:** `RailConvert`, quitar señal → ver ROADMAP_SPRINTS S2 resto.

---

## Hallazgos fijos (no olvidar)

1. **Cruces a nivel** — no usar bits 0–3 de `m5` como road bits; eje en bit 0. Ver [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).
2. **MAPT + `m5`** — byte MAPT crudo necesario para túneles/puentes vs `MP_ROAD`.
3. **`road_tx` ↔ `road_ty`** — intercambio respecto a `RoadDir` para isometría del cliente (validado visualmente).
4. **Sprite coal mine** — ID correcto 2013; verificación en [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md).
5. **Fuente UI** — `static/fonts/DejaVuSansMono.ttf` (no en `assets/` ignorado).
6. **Estación tren** — multi-tesela `PlaceRailStationArea`, ventana selección, cruce X\|Y en intersecciones (save v3).
7. **Vía Horz/Vert/X/Y** — `PlaceRailBits` solo en tesela del cursor (fantasma = colocación); uniones automáticas solo con autoraíl. Ver [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md).
8. **Señales** — pick en vecindario + offset sub-tesela; se conservan al cruzar diagonales. **Bug abierto:** fantasma vs clic en diagonal — [SENALES §11](SENALES_FERROVIARIAS.md#11-bug-abierto-fantasma-vs-colocación-en-vía-diagonal-jun-2026).

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
```

---

## Si algo se pierde

1. [docs/README.md](README.md) — índice completo
2. Comentarios y submódulos en `crates/openttdrs-core/src/command/` (`transport/`, `tests/`) y `crates/openttdrs-client/src/ui/` (`main_menu/`, `toolbar/`)
3. Upstream: `bash scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/`

---

*Última actualización: 2026-06-22 (refactor módulos, menú fase 2, lagos, población procedural)*
