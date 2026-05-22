# Plan SP3 — Casas e industrias en `.ottdmap` (sin fallbacks genéricos)

Documento de **seguimiento** para cerrar el hueco SP3.4 descrito en
[PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md) y [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md).

**Estado (2026-05):** tablas y PNG base listos (templado + 120 gfx industriales estadio 3).
Queda **fidelidad en mapas reales**: etapas de obra, HouseID altos, gfx fuera de rango y
calibración fina.

**Relacionado:** [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) §8–10,
[SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md), [INDUSTRIAS_OPENGFX.md](INDUSTRIAS_OPENGFX.md),
[SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md).

---

## 1. Qué significa “fallback genérico” aquí

En openttdrs **no** se dibuja un bloque de color sustituto. El comportamiento actual es:

| Situación | Efecto visual |
|-----------|----------------|
| Capa con `sprite_id == 0` o PNG no cargado | Capa **omitida** (hierba/rough debajo) |
| Industria `gfx ≥ 120` o sin fila en tabla | Solo **hierba** (sin overlay industrial) |
| Fila industrial con dims `64×48/-32/-32` sin PNG | “Fallback genérico” lógico (**0 filas** así hoy en `INDUSTRY_GFX_DATA`) |
| Casa: etapa ignorada (siempre 3) | Obras en construcción se ven **terminadas** |
| Casa: `HouseID ≥ 128` | Tipo clamp al 7 → edificio **incorrecto** (no templado/árctico/NewGRF) |

Objetivo: que un `.ottdmap` exportado de un save real use el **mismo criterio** que OpenTTD
(`m8`, `m5`, `gfx` 9 bits) sin degradar a “ciudad genérica” o tesela vacía.

---

## 2. Pipeline actual (referencia rápida)

```text
.ottdmap → Map::from_ottd_binary
  MP_HOUSE:  m8 = HouseID (u16)
  MP_INDUSTRY: gfx = m5 | ((m6 >> 2) & 1) << 8

Cliente (segunda pasada, tras agua — Z-order):
  spawn_house_tile     → HOUSE_DRAW_DATA[índice]
  spawn_industry_tile  → INDUSTRY_GFX_DATA[gfx]
```

| Componente | Archivo |
|------------|---------|
| Tabla casas (128 filas) | `crates/openttdrs-client/src/sprites.rs` — `HOUSE_DRAW_DATA` |
| Índice por tesela | `house_draw_data_index_for_tile(m8, tx, ty)` — etapa **3 fija** |
| Tabla industrias (120 filas) | `crates/openttdrs-client/src/sprites/industry_gfx_data_generated.rs` |
| Lookup + debug fallback | `crates/openttdrs-client/src/sprites/industry.rs` |
| Generador industria | `scripts/gen_industry_gfx_data.py` |
| Render | `crates/openttdrs-client/src/render/tiles/land.rs` |
| Precarga PNG | `crates/openttdrs-client/src/render/assets.rs` |
| Auditoría assets | `scripts/audit_sp3_assets.py` → [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) |

OpenTTD upstream: `table/town_land.h`, `table/industry_land.h` (copia en
`third_party/openttd/station_land.h` solo estaciones; casas/industria en referencia
`reference/openttd-upstream/` vía `scripts/fetch-openttd-reference.sh`).

---

## 3. Estado hecho (no repetir)

Marcado en SP3.4 como base; **no** implica mapas reales perfectos:

- [x] `INDUSTRY_GFX_DATA` generado; 116/120 filas con PNG calibrado (no `64×48` genérico).
- [x] `HOUSE_DRAW_DATA` 128 filas (8 tipos × hash × 4 etapas en tabla; render usa solo etapa 3).
- [x] `debug_log_industry_gfx_once` (builds debug) para gfx sin entrada o fallback lógico.
- [x] Z-order: casas/industrias tras agua (`world.rs` / `flush_map_batches`).
- [x] Auditoría SP3: sin PNG faltantes de casa/industria en repo (riesgo: placeholder 1×1 si falla NFO al descargar).

---

## 4. Roadmap por prioridad (seguir en este orden)

### P1 — Etapas de construcción de casas (mayor impacto / menor alcance)

**Problema:** `house_draw_data_index_for_tile` usa `FINISHED_STAGE = 3` siempre.

**OpenTTD:** `house_id * 16 + TileHash2Bit(x,y) * 4 + GetHouseBuildingStage()`.

**Tareas:**

1. Documentar en [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) §10 qué bits de `m5`
   en `MP_HOUSE` codifican la etapa (consultar `house_map.h` en upstream).
2. Pasar `m5` (o etapa extraída) a `house_draw_data_index_for_tile` desde `spawn_house_tile`
   (`land.rs` tiene acceso a `ctx.tile`).
3. Clamp etapa 0..3; mantener hash 2-bit y `house_type = house_id / 16`.
4. Tests en `sprites.rs` (`house_draw_index_tests`): etapas 0, 1, 2, 3 distintas para mismo `(tx,ty)`.

**Criterio de aceptación:** en ciudad con obras en save real, andamios / etapas intermedias visibles;
edificios terminados siguen igual (etapa 3).

**PR sugerido:** 1 PR pequeño solo cliente + doc + test.

---

### P2 — HouseID fuera de 0–127 (climas y NewGRF)

**Problema:** `house_type.min(7)` → IDs ≥ 128 se dibujan como tipo 7 (Large Office moderno).

**Tareas:**

1. Inventariar HouseIDs en saves de prueba (`parse_sav.py` + histograma `m8` en `MP_HOUSE`).
2. Opción A: ampliar `HOUSE_DRAW_DATA` con filas ártico/tropical desde `town_land.h`.
3. Opción B (interina): mapeo explícito `house_id % 128` por **clima** documentado (peor que A).
4. Actualizar `descargar_graficos.sh` si faltan `house_s*.png` de otros climas.

**Criterio:** save ártico/tropical no muestra solo rascacielos del tipo 7.

**PR sugerido:** 1–2 PR (datos + script + tabla).

---

### P3 — Industrias con `gfx ≥ 120` o sin entrada

**Problema:** `industry_gfx_entry(gfx)` → `None` → tesela de hierba sin aviso en release.

**Tareas:**

1. Contar `gfx` en `.ottdmap` reales por encima de 119.
2. Extender generador/tablas o documentar límite y degradar a `rough` + log HUD (no silencio).
3. Revisar filas 116–119 (muchas solo `ground_sprite_id` en trópico — válido en footprint).

**Criterio:** ninguna tesela `MP_INDUSTRY` en checklist/partida típica queda como hierba plana sin explicación.

---

### P4 — Calibración offsets industriales

**Problema:** filas con macro `M(dx,dy,sx,sy)` + PNG pueden estar desplazadas (Farm, Factory, Coal Mine).

**Tareas:**

1. `bash scripts/descargar_graficos.sh` (OpenGFX actual).
2. `python3 scripts/gen_industry_gfx_data.py` — revisar salida `fallback=0`.
3. Validación visual en checklist SP3 y en `assets/maps/mapa.ottdmap` de partida real.
4. Ajuste manual por industria si hace falta (como paradas: delta por fila en generador).

**Criterio:** mina/fábrica/granja alineadas al rombo; sin “flotar” en tesela vecina.

---

### P5 — Etapas de obra industrial (0–2)

**Problema:** `INDUSTRY_GFX_DATA` solo estadio **3** por tile (`gfx * 4 + 3` en upstream).

**Tareas:**

1. Extender `gen_industry_gfx_data.py` para filas `gfx*4+0..2` (como `industry_land.h`).
2. Leer etapa desde `m5`/`m6` en `spawn_industry_tile` (misma fuente que OpenTTD).
3. Precarga de sprites de obra en `assets.rs`.

**Criterio:** industria en construcción en save no aparece ya terminada.

---

### P6 — Casas especiales y huecos menores

**Tareas:**

- Estadio y sprites 1479–1482 (en `descargar_graficos.sh` pero no en `HOUSE_DRAW_DATA`).
- Filas `s2 == 0` / `s1 == 0` intencionales (parques) — documentar, no “arreglar”.
- Alinear doc obsoleta en §10 de `TILES_Y_SAVEGAMES_OPENTTD.md` (`house_id % 128` en main.rs ya no aplica).

---

## 5. Validación (checklist repetible)

```bash
# CI
bash scripts/check.sh ci

# Auditoría PNG
python3 scripts/audit_sp3_assets.py

# Mapa checklist 20×12 (casa + industria base)
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap \
  cargo run -p openttdrs-client

# Partida real
python3 scripts/parse_sav.py partida.sav assets/maps/mapa.ottdmap
OTTDMAP_FILE=assets/maps/mapa.ottdmap cargo run -p openttdrs-client

# Regenerar tabla industria tras cambiar NFO/PNG
python3 scripts/gen_industry_gfx_data.py
```

**Sesión manual (~10 min) tras cada PR de esta lista:**

1. Zoom a zona urbana: ¿obras vs terminados? (P1)
2. ¿Casas coherentes con el clima del save? (P2)
3. ¿Industrias con suelo + edificio donde corresponde? (P3–P4)
4. F5/F9: mismas teselas tras recargar.

Marcar progreso en la tabla §4 moviendo `[ ]` → `[x]` por ítem completado.

---

## 6. Fuera de este plan (no mezclar)

| Tema | Dónde |
|------|--------|
| Paradas bus/camión otras direcciones (NE/SW/NW, capas B/C) | [PLAN_PARADAS_REMAPCOORDS.md](PLAN_PARADAS_REMAPCOORDS.md) — SE truck `build_a` calibrado |
| Pendientes carretera/vía en slope | [PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md) SP3.1 |
| SP2.6 sesión manual construcción | [SP2_CHECKLIST.md](SP2_CHECKLIST.md) |
| Economía / órdenes | SP1 |

---

## 7. Registro de decisiones

| Fecha | Decisión |
|-------|----------|
| 2026-05 | SP3.4 “hecho” en código = tablas base; este doc define el **restante** por prioridad P1–P6. |
| 2026-05 | P1 (etapa casa desde `m5`) es el siguiente paso recomendado por impacto/alcance. |
