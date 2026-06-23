# Siguientes pasos — openttdrs

Documento vivo con **hallazgos técnicos** y **comandos**. El plan de trabajo está en
[ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md); el inventario completo de paridad en
[PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md). Índice general: [README.md](README.md).

**Hito actual:** 0.1 solitario · **I0–I7** hechos · **I8 (red)** backlog post-0.1.

---

## Prioridad inmediata (Sprint 1)

Ver [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) § Sprint 1:

- [x] Migración save v3→v4 con test
- [x] Test `effective_road_bits` en fixture `.ottdmap`
- [ ] Pasada manual SP2 checklist

---

## Estado de fases SP

| Fase | Estado | Referencia |
|------|--------|------------|
| **SP2** Construcción | ✅ Cerrado 2026-05-22 | [SP2_CHECKLIST.md](SP2_CHECKLIST.md) |
| **SP3** Visual | 🟡 ~90 % | [ROADMAP_PARIDAD_VISUAL.md](ROADMAP_PARIDAD_VISUAL.md), [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) |
| **SP4** Pulido | 🟡 En curso | ROADMAP_SPRINTS S1 |
| **SP1** Ciclo jugable | 🟡 | ROADMAP_SPRINTS S4 |

**Huecos visuales reales (SP3):** junctions vía en pendiente; depósito carretera; culling global; industrias gfx ≥ 120.

**Toolbar rail sin comando:** waypoint, señales, quitar vía, convertir → ROADMAP_SPRINTS S2/S5.

---

## Hallazgos fijos (no olvidar)

1. **Cruces a nivel** — no usar bits 0–3 de `m5` como road bits; eje en bit 0. Ver [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).
2. **MAPT + `m5`** — byte MAPT crudo necesario para túneles/puentes vs `MP_ROAD`.
3. **`road_tx` ↔ `road_ty`** — intercambio respecto a `RoadDir` para isometría del cliente (validado visualmente).
4. **Sprite coal mine** — ID correcto 2013; verificación en [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md).
5. **Fuente UI** — `static/fonts/DejaVuSansMono.ttf` (no en `assets/` ignorado).
6. **Estación tren** — multi-tesela `PlaceRailStationArea`, ventana selección, cruce X\|Y en intersecciones (save v3).

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
2. Comentarios en `crates/openttdrs-core/src/command/` y `crates/openttdrs-client/src/ui/toolbar/`
3. Upstream: `bash scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/`

---

*Última actualización: 2026-06-11*
