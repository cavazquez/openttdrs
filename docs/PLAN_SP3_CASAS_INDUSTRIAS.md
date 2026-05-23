# Plan SP3 — Casas e industrias en `.ottdmap` (sin fallbacks genéricos)

Documento de **seguimiento** para cerrar el hueco SP3.4 descrito en
[PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md) y [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md).

**Estado (2026-05):** P1–P6 completados (casas + industrias en checklist y tablas upstream).
Validar en partidas reales grandes sigue siendo útil; extensión **gfx 120–174** y NewGRF
(`gfx ≥ 175`) siguen en [ROADMAP_INDUSTRIAS_PARIDAD.md](ROADMAP_INDUSTRIAS_PARIDAD.md).

**Relacionado:** [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) §8–10,
[SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md), [INDUSTRIAS_OPENGFX.md](INDUSTRIAS_OPENGFX.md),
[SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md).

---

## 1. Qué significa “fallback genérico” aquí

En openttdrs **no** se dibuja un bloque de color sustituto. El comportamiento actual es:

| Situación | Efecto visual |
|-----------|----------------|
| Capa con `sprite_id == 0` o PNG no cargado | Capa **omitida** (hierba/rough debajo) |
| Industria `gfx ≥ 120` o sin fila en tabla | ~~Solo **hierba**~~ → **rough** + `warn!` + HUD `⚠gfx≥120` |
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
  spawn_industry_tile  → INDUSTRY_GFX_DATA[gfx*4+stage(m1)]
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

### P1 — Etapas de construcción de casas ✅

**Problema:** `house_draw_data_index_for_tile` usaba etapa **3** fija.

**Hecho:** `house_building_stage_from_tile(m5, m3)` + etapa en índice; fixture y=1/y=8; tests en `sprites.rs`.

---

### P2 — HouseID fuera de 0–127 (climas y NewGRF) ✅

**Problema:** `house_type.min(7)` colapsaba IDs altos.

**Hecho:** `house_id_for_draw_table` → `% 110`; fixture y=0/y=6 con HouseIDs por clima.

---

### P3 — Industrias con `gfx ≥ 120` o sin entrada ✅

**Problema:** `industry_gfx_entry(gfx)` → `None` → tesela de hierba sin aviso en release.

**Hecho (2026-05):**

1. `MP_INDUSTRY` siempre usa terreno **rough** (también gfx≥120 / sin PNG).
2. `log_industry_gfx_once` → `warn!` once en release; HUD muestra `⚠gfx≥120` / `⚠sin sprite`.
3. Fixture checklist **y=10**: gfx 0, 42, 116, 119, 120, 256 — paso **2** en x (1 tile hierba entre casos).
4. Límite documentado: tabla **120** filas (`INDUSTRY_GFX_TABLE_LEN`); extensión NewGRF = trabajo futuro.

**Criterio:** ninguna tesela `MP_INDUSTRY` en checklist/partida típica queda como hierba plana sin explicación.

---

### P4 — Calibración offsets industriales ✅

**Problema:** filas con macro `M(dx,dy,sx,sy)` + PNG podían estar desplazadas (heurística `XREL_PER_W`).

**Hecho (2026-05):**

1. `scripts/nfo_sprite_meta.py` — offsets desde NFO + escala PNG (como paradas).
2. `gen_industry_gfx_data.py` / `gen_house_draw_data.py` regenerados (`macro=0`, `fallback=0`).
3. `IndustryGfxSprite` con campos **`ground_*`** separados del edificio; render/preview usan capa correcta.
4. Tests: mina gfx0 (-16/-33 + suelo -31/0), chimenea gfx7 (-21/-34).

**Criterio:** mina/fábrica/granja alineadas al rombo; validar visual en checklist y partida real.

**Seguimiento manual:** ajustes finos por industria en `CAL` del generador si algún PNG difiere del NFO upstream.

---

### P5 — Etapas de obra industrial (0–2) ✅

**Problema:** `INDUSTRY_GFX_DATA` solo tenía estadio **3** por `gfx` (`gfx * 4 + 3` en upstream).

**Hecho:**

1. `gen_industry_gfx_data.py` genera **480 filas** (`gfx * 4 + stage`, stages 0–3).
2. `industry_construction_stage_from_tile(m1)` + `industry_gfx_entry_for_tile(gfx, m1)` en `spawn_industry_tile`.
3. Precarga automática vía iteración de `INDUSTRY_GFX_DATA` en `assets.rs` (sprites de obra incluidos).
4. Fixture checklist **y=4**: gfx0 etapas 0–2 + terminada (mina carbón).
5. Tras regenerar la tabla: `python3 scripts/crop_missing_industry_pngs.py` (o `./scripts/descargar_graficos.sh --8bpp`).

**Criterio:** industria en construcción en save ya no aparece terminada.

---

### P6 — Casas especiales y huecos menores ✅

**Problema:** `SPR_GRND_STADIUM_*` y otras constantes `SPR_*` en `town_land.h` se parseaban como **0**.

**Hecho:**

1. `gen_house_draw_data.py` resuelve `SPR_*` desde `reference/.../sprites.h` (estadio **1479–1482**, concreto **1420**, toyland **4675–4676**).
2. `descargar_graficos.sh`: alias `house_s1479..1482` + `house_s1420`; loop `house_s{id}` cubre el resto.
3. Fixture **y=4 x=17**: estadio HouseID **20**.
4. Tests: fila 320 suelo estadio; fila parque `s1=s2=0` intencional.
5. §10 de [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) alineado (sin `house_id % 128` obsoleto).

**Nota:** filas `s1==0 && s2==0` en parques siguen siendo correctas (solo hierba).

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
