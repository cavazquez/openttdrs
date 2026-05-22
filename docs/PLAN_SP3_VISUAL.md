# Plan SP3 — Presentación del mapa (solitario)

Documento de planificación tras revisar:

- Código **openttdrs** (`sprites/`, `render/tiles/`, `descargar_graficos.sh`).
- Documentación derivada del upstream (`TILES_Y_SAVEGAMES_OPENTTD.md`, `SPRITES_OPENGFX*.md`, `INFORME_ARQUITECTURA_OPENTTD.md`).
- Flujo de dibujo OpenTTD: `road_map.h` + `road_cmd.cpp`, `rail_map.h` + `rail_cmd.cpp`, `station_map.h`, `water_cmd.cpp`, tablas en `industry_land.h`.

Para leer el C++ en vivo: `bash scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/` (gitignored).

**Prioridad:** SP3 es **media**; va **después** de SP1–SP2 (jugabilidad y construcción). **I8 (red)** sigue en backlog.

---

## 1. Estado real (no confundir con docs viejas)

Varios ítems de `SIGUIENTES_PASOS` (2026-04) ya están **hechos** en `main`:

| Tema | En upstream | En openttdrs hoy |
|------|-------------|------------------|
| Carretera plana esquina/T/cruce | `GetRoadSpriteOffset` → `SPR_ROAD_Y` + tabla 16 entradas | `ROAD_FLAT_OFFSET_TBL` + `road_flat_00..18` + `spawn_road_tile` |
| Tranvía en carretera | `GetRoadBits(Tram)` / `m3` bajo | `tram_flat_*` + `tram_flat_sprite_index(m3)` |
| Cruce a nivel | `RoadTileType::Crossing` + sprite vía 1370+ | `is_road_level_crossing`, `level_crossing_rail_sprite_id` |
| Vía por `TrackBits` | `DrawRailTile` / junction overlays 1005–1018 | `collect_rail_sprites` + `rail_<id>.png` |
| Señales | `DrawSignals` en `rail_cmd.cpp` | `collect_signal_sprite_ids` |
| Decodificación `m5` carretera | `road_map.h` subtipos Normal/Crossing/Depot | `effective_road_bits` |
| Agua animada (aprox.) | Paleta cíclica mar | `WaterAnimationPlugin` + culling en entidades `WaterTile` |
| Casas con sprite | `_house_draw_tile_data` | `HOUSE_DRAW_DATA` + precarga por `m8` |
| Industrias | `_industry_draw_tile_data` | `INDUSTRY_GFX_DATA` + plantillas construcción |

**Huecos reales** (donde el mapa aún no “se lee” como OpenTTD):

1. **Vías y carreteras en pendiente** — upstream usa familias `SPR_ROAD_SLOPE_*` / raíles en slope; aquí muchas teselas inclinadas siguen con suelo genérico + pieza plana o solo 4 variantes de cruce en `0x0F`.
2. **Estaciones** — tren: plataformas 1069–1074 (SP2). Paradas bus/camión: suelo + `BUILD_A/B/C` (offsets `station_land.h`). Pendiente: ghost preview, multi-tesela tren. Ver [SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md).
3. **Cobertura de casas** — solo un subconjunto de `CleanHouseType`; el resto cae en specs repetidas o sin `s2`.
4. **Industrias / casas en `.ottdmap`** — ver roadmap [PLAN_SP3_CASAS_INDUSTRIAS.md](PLAN_SP3_CASAS_INDUSTRIAS.md) (P1–P6).
5. **Assets faltantes** — `descargar_graficos.sh` puede dejar `rail_*.png` como **placeholder** si el crop falla; conviene auditoría `assets/opengfx/tiles/`.
6. **Culling del mapa completo** — el agua culling sí; el respawn de **todas** las teselas al mover cámara no (coste en 256×256+).
7. **Túneles/puentes** — un sprite por tipo; upstream varía eje y material.
8. **Coast en `.ottdmap`** — pipeline verificado (`verify_parse_sav_water_m5.py`); en saves sin Coast en MAP5, fallback por vecinos en `RenderGrid`.

---

## 2. Cómo dibuja OpenTTD (referencia para portar)

### Carretera (`road_cmd.cpp`, `road_map.h`)

1. Suelo del rombo (`DrawGroundSprite` / pendiente).
2. Si `MP_ROAD` **Normal**: `road_bits = GetRoadBits(Road)` → sprite **`SPR_ROAD_Y + GetRoadSpriteOffset(road_bits, slope)`**.
3. Si **Crossing**: eje en bit 0, no road bits 0–3; superponer vía (`GetCrossingRailAxis`).
4. Si **Depot**: orientación `DiagDirection` en bits 0–1.
5. Tranvía: capa aparte con la **misma** tabla de offsets sobre `SPR_TRAMWAY_OVERLAY`.
6. Nieve/desierto en carretera: tinte vía `IsOnSnowOrDesert` (MAP7 bit 5) — **ya** `road_flat_sprite_color`.

**Implicación:** no hace falta reintroducir `road_tx.png` / `road_ty.png` sueltos; hay que **completar pendientes** y asegurar bits correctos en mapas reales.

### Vía (`rail_cmd.cpp`, `rail_map.h`, `track_type.h`)

1. `trackbits = GetTrackBits()` (6 bits en `m5` para vía normal).
2. Casos simples: un sprite compuesto (`1011` Y, `1012` X, `1017` cruce, `1035`/`1036` horz/vert).
3. Junctions: `SPR_RAIL_TRACK_BASE + junction_off` + overlays `SPR_RAIL_SINGLE_*` por bit — **ya** `collect_rail_sprites`.
4. Señales: capa extra desde `m2`/`m3`/`m3hi` — **ya** portado.
5. Pendiente / nieve en suelo de vía: variantes `1037+` y `RailGroundType` en `m3` — **parcial** (`rail_track_base_color`).

### Estación (`station_cmd.cpp`, `station_map.h`)

- Tesela `MP_STATION`: estación de tren = varias capas (plataforma, techo, edificio) según tamaño y dirección.
- Bus stop / truck stop: sprites dedicados (`SPR_BUS_STOP`, etc.) — openttdrs usa 4 orientaciones de suelo genérico.

### Agua (`water_cmd.cpp`, `water_map.h`)

- **Clear:** mar animado (paleta).
- **Coast:** un sprite por **pendiente** (`DrawShoreTile`), no por vecinos — openttdrs alineado; verificar datos de entrada.

### Industria / casa

- Tablas `_industry_draw_tile_data[]` / `_house_draw_tile_data[]` en `industry_land.h` / `house_land.h`: cada tesela del footprint lista `(sprite, xrel, yrel, …)`.
- openttdrs: tabla generada + ghost de construcción compartiendo plantilla — bien para sandbox; falta **completar filas** usadas en `.ottdmap`.

---

## 3. Fases de trabajo propuestas

Orden sugerido: primero **datos + mapas reales**, luego **estaciones/industrias**, al final **rendimiento**.

### SP3.0 — Auditoría (1 PR, sin gameplay)

- [x] Clonar referencia: `scripts/fetch-openttd-reference.sh` (opcional; ver `reference/openttd-upstream/`).
- [x] Inventario automatizado: `python3 scripts/audit_sp3_assets.py` o `./scripts/check.sh audit`.
- [x] Informe: [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) (JSON local: `docs/SP3_AUDIT_REPORT.json`, ignorado por `*.json` en git).
- [x] Partida de prueba **manual** con fixture TNBP: `v5p12_tnbp.ottdmap` — captura [sp3/manual-v5p12_tnbp-2026-05-22.png](sp3/manual-v5p12_tnbp-2026-05-22.png), detalle en [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md).
- [x] Fixture **denso** `sp3_visual_checklist.ottdmap` (20×12, **1 tesela de hierba** entre escenas) — `scripts/gen_sp3_visual_checklist_ottdmap.py`, layout en [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md).
- [ ] Captura manual del checklist (una sesión con el comando de abajo).

**Criterio:** documento de gaps con lista de sprite IDs — **cumplido**. Fixture checklist listo para captura — **cumplido**; captura PNG opcional en `docs/sp3/`.

**Hallazgo SP3.0 (esta máquina):** 519 PNG requeridos presentes; **8 placeholders** `rail_1438`…`rail_1548` (señales PBS sin recorte en OpenGFX). Ver resumen.

### SP3.1 — Carretera y tranvía en mapas reales (1–2 PR)

Upstream: `GetRoadSpriteOffset`, `DrawRoadTile`.

- [x] Test de regresión: cada `road_bits` 1..15 en tesela plana → índice `road_flat_*` (`road.rs` + `sprites.rs`).
- [x] Pendientes diagonales NE/SE/SW/NW: `road_flat_sprite_index` alineado con `GetRoadSpriteOffset` (sprites 11–14 = `road_flat_11..14`, mismo grupo `SPR_ROAD_Y`).
- [x] `effective_road_bits` en fixture `m3_road_tram_2x2.ottdmap` + subtipos cruce/depósito (tests unitarios).
- [x] Tranvía: fixture core + índice overlay en cliente (`m3_fixture_effective_bits_and_tram_overlay_index`).
- [x] Fixture checklist: carretera en pendientes NE/SE/SW/NW (fila y=7) + test `ottdmap_sp3_visual_fixture`.

**Criterio:** trazado de carretera en save real coincide con OpenTTD en tramos rectos, T y cruce; tranvía visible donde el save lo tenga; pendientes diagonales usan `road_flat_11..14` en checklist.

### SP3.2 — Vía férrea: suelo, pendiente, señales (1–2 PR)

Upstream: `rail_cmd.cpp`, `DrawRailTile`.

- [x] Placeholders: precarga acotada + denylist `SIGNAL_SPRITE_OPENGFX_GAPS` (no exige `rail_1438`… en audit).
- [x] Precarga: `signal_sprite_ids_for_preload` + `RAIL_SPRITE_IDS` (incl. 1037/1038).
- [x] Nieve en vía: `collect_rail_sprites(..., snow_ground)` usa `1037`/`1038` cuando `m3` bajo = `RAIL_GROUND_SNOW_OR_DESERT`.
- [x] `rail_trackbits_for_render`: tesela `Rail` con `m5=0` no usa vecinos sintéticos.

**Criterio:** T y cruce de vía en mapa real; señales visibles en tiles con `RailTileType::Signals`; sin PNG rosa/placeholder en IDs usados.

### SP3.3 — Estaciones y paradas (1 PR)

Upstream: `station_cmd.cpp`, sprites 1069–1086, bus stops.

- [x] Diferenciar `MP_STATION` tren / bus / truck (`station_tile_class`, suelos distintos).
- [x] Estación tren 1×1: plataforma + edificio (`rail_platform_*` + `rail_station_sprite_layers`).
- [x] Orientación en `m5` (eje Y = bit 0); herramienta **Estación de tren** en panel vía (`PlaceRailStation`).
- [x] Paradas bus/camión: capas `BUILD_A/B/C` con `RemapCoords` (código; validar visual en checklist).

**Criterio:** paradas con edificio visible; tren 1×1 distinguible de hierba y de parada de carretera.

### SP3.4 — Casas e industrias en mapas reales (1–2 PR)

Upstream: `industry_land.h`, `house_land.h`.

- [x] Ampliar `INDUSTRY_GFX_DATA`: `gen_industry_gfx_data.py` calibra w/h/xrel/yrel desde PNG + macro `M(dx,dy,sx,sy)`.
- [x] Reducir fallback genérico: 116/120 filas con PNG; `debug_log_industry_gfx_once` en builds debug.
- [x] Casas: `house_draw_data_index_for_tile` usa tipo (`m8/16`) + `TileHash2Bit` + etapa 3.
- [x] Z-order: casas, industrias y estaciones tras `flush_map_batches` (encima del agua).

**Criterio:** cargar un `.ottdmap` con mina/fábrica/casa no trivial y que no parezcan “bloques de color”.

**Seguimiento (pendiente en mapas reales):** [PLAN_SP3_CASAS_INDUSTRIAS.md](PLAN_SP3_CASAS_INDUSTRIAS.md)
— prioridad **P1** etapas de obra desde `m5`, luego HouseID ≥128, gfx≥120, calibración, etapas industria.

### SP3.5 — Agua y costa (0–1 PR)

Upstream: `water_cmd.cpp`.

- [x] Confirmar conservación de subtipo Coast en pipeline `parse_sav` → `.ottdmap` → cliente (`verify_parse_sav_water_m5.py`, `map.rs`, fixture SP3 `(5,11)` en checklist 20×12).
- [x] Afinar animación mar (interpolación suave dark×5 + glitter×15, brillo/cian en picos).
- [x] Costas: sin regresión en `shore_*` + `tileh` (tests en `iso`/`RenderGrid`; `water_with_coast_m5_uses_shore_without_land_neighbors`).

**Criterio:** bahía en save real muestra costa coherente con OpenTTD; agua libre animada.

**Notas:** el fixture `stationlist-test.sav` no tiene teselas Coast en MAP5 (todo Clear + 2 «otro»); la orilla se infiere con `water_tile_touches_land` en `RenderGrid`. Saves con bahías reales deben conservar `m5=0x10` en export.

### SP3.6 — Rendimiento mapa grande (1 PR)

Upstream: viewport k-d tree (idea, no copiar).

- [x] Culling al **generar** sprites de tesela: `render/viewport.rs` + `MapTileSpawnViewport` (mapas ≥ 4096 teselas).
- [x] Evitar respawn cada frame: `RemapMapVisualsPending` solo en F9/construcción/pan fuera del bloque (`sync_map_tile_spawn_viewport`).
- [x] Medir FPS en 256×256: ver `scripts/bench_large_map_viewport.md` + `tests/fixtures/stationlist-test.ottdmap`.

**Criterio:** pan/zoom fluido en mapa grande de prueba.

**Notas:** `OPENTTDRS_MAP_VIEWPORT_OFF=1` desactiva el culling. El agua animada sigue con culling por cámara en `animate_water`.

---

## 4. Qué no entra en SP3

- Multijugador / log de comandos (**I8**).
- NewGRF / 32 bpp completo.
- Paridad total de todos los modelos de vehículo/industria.
- Pathfinding o economía (SP1/SP2).

---

## 5. Enlaces de lectura en el clon upstream

| Objetivo | Archivos |
|----------|----------|
| Carretera plana + slope | `src/road_map.h`, `src/road_cmd.cpp` (`DrawRoadTile`, `GetRoadSpriteOffset`) |
| Vía + señales | `src/rail_map.h`, `src/rail_cmd.cpp` (`DrawRailTile`, `DrawSignals`) |
| Cruce | `src/road_cmd.cpp`, `src/road_func.h` (`GetCrossingRoadAxis`) |
| Estación | `src/station_map.h`, `src/station_cmd.cpp` |
| Agua | `src/water_map.h`, `src/water_cmd.cpp` (`DrawShoreTile`, `DrawSeaWater`) |
| Industria/casa | `src/table/industry_land.h`, `src/table/house_land.h` |
| Sprites IDs | `src/table/sprites.h` |

---

## 6. Relación con otros docs

- Prioridad global: [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) § Roadmap.
- Checklist operativo: [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) § SP3.
- Extracción PNG: [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md), [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md).
- Bytes de tesela: [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).
