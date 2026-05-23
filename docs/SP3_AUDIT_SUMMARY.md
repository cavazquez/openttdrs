# SP3.0 — Resultado de auditoría visual (assets)

Generado con:

```bash
python3 scripts/audit_sp3_assets.py --json docs/SP3_AUDIT_REPORT.json
```

Re-ejecutar tras `bash scripts/descargar_graficos.sh` o cambios en precarga de sprites.

## Resumen

| Métrica | Valor |
|---------|------:|
| PNG requeridos por `WorldAssets::load` | 538 |
| Presentes | 538 |
| Ausentes | 0 |
| **Placeholder 1×1** (NFO sin sprite) | **8** |

Todos los placeholders están en categoría **rail** (sprites de señal del rango PBS/alt generado por `rail_sprite_ids_for_preload`).

## Placeholders detectados

| Archivo | Sprite ID | Nota |
|---------|----------:|------|
| `rail_1438.png` | 1438 | Señal PBS / alt |
| `rail_1439.png` | 1439 | Señal PBS / alt |
| `rail_1530.png` | 1530 | Señal PBS / alt |
| `rail_1532.png` | 1532 | Señal PBS / alt |
| `rail_1540.png` | 1540 | Señal PBS / alt |
| `rail_1542.png` | 1542 | Señal PBS / alt |
| `rail_1546.png` | 1546 | Señal PBS / alt |
| `rail_1548.png` | 1548 | Señal PBS / alt |

**Acción SP3.2 (hecho):** `rail_sprite_ids_for_preload` precarga solo señales alcanzables por `collect_signal_sprite_ids` y excluye `SIGNAL_SPRITE_OPENGFX_GAPS` (los 8 IDs de arriba). Si un save pide un hueco, el cliente omite el sprite ausente en `spawn_rail_tile`.

## Categorías sin problemas

- **road** / **tram** — `road_flat_00..18`, `tram_flat_00..18` presentes (tabla `GetRoadSpriteOffset` ya cableada).
- **terrain** / **water** / **station** (incl. `bus_stop_*_build_*`, `truck_stop_*_build_*`) / **industry** / **house** — sin faltantes en esta máquina.
- **transport_object** — túneles, puentes, depósitos OK.

## Fixtures `.ottdmap` (pruebas de mapa real)

| Fixture | Uso |
|---------|-----|
| `crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap` | TNBP en CI |
| `crates/openttdrs-core/tests/fixtures/m3_road_tram_2x2.ottdmap` | Tranvía `m3` |
| `crates/openttdrs-core/tests/fixtures/v5p12_stxy.ottdmap` | Footer estaciones |
| `crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap` | **Checklist visual** (20×17, escenas separadas) |
| `crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap` | **Laboratorio pendiente/agua** (16×20, mapa dedicado) |
| `tests/fixtures/stationlist-test.ottdmap` | Lista estaciones |

## Referencia upstream

Clon presente en `reference/openttd-upstream/` (para leer `road_cmd.cpp`, `rail_cmd.cpp`, etc.).

Si falta: `bash scripts/fetch-openttd-reference.sh`.

## Prueba manual (SP3.0)

### Fixture mínimo TNBP

```bash
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap cargo run -p openttdrs-client
```

Captura: [sp3/manual-v5p12_tnbp-2026-05-22.png](sp3/manual-v5p12_tnbp-2026-05-22.png) — valida TNBP + pendiente NE en mapa 2×2.

### Fixture checklist denso (SP3.0 cerrado / SP3.1 visual)

Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`

```bash
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client
```

**Mapa 20×17** — cada escena va con **≥1 tesela de hierba** de separación. Pan/zoom para revisar fila a fila.

| Zona (x,y) | Contenido |
|------------|-----------|
| (1,3) | Carretera Y |
| (3,3) | Carretera X |
| (5,3) | T (`0x07`) |
| (7,3) | Cruce `0x0F` |
| (9,3) | Cruce a nivel eje X (`m5=0x40`) |
| (11,3) | Cruce a nivel eje Y (`0x41`) |
| (15,3) | Carretera + tranvía eje X (`m5=0x0A`, `m3=0x0A`) |
| (1,5) | Vía Y |
| (3,5) | Vía X |
| (5,5) | Vía T |
| (7,5) | Cruce vía |
| (9,5) | Vía con señales |
| (11,5) | Vía nieve (`m3` bajo = 12) |
| **(1,7)** | **Carretera en pendiente NE** (SP3.1) |
| **(4,7)** | **Carretera pendiente SE** |
| **(7,7)** | **Carretera pendiente SW** |
| **(10,7)** | **Carretera pendiente NW** |
| **(13,7)** | **Tranvía en pendiente NE** (`m5=0x05`, `m3=0x05` → `road_flat_11` / `tram_flat_11`) |
| **(16,7)** | **Estación tren eje Y en pendiente NE** (vía **1031** bajo plataformas) |
| **(1,9)** | Parada bus **NE** |
| **(3,9)** | Parada bus **SE** |
| **(5,9)** | Parada bus **SW** |
| **(7,9)** | Parada bus **NW** |
| (9,9) | Parada camión SE |
| (11,9) | Estación tren 1×1 |
| **(15,9)** | **Parada bus NE en pendiente NE** (stub → `road_flat_11`) |
| (13,9) | Casa Tall Office |
| **(2,11)–(3,11)** | **Charco Clear** (`m5=0x00`, `h=4`; borde costero inferido vs hierba) |
| **(5,11)** | **Costa explícita** (`m5=0x10`, `h=4`) |
| **(9,11)** | **Vía recta Y en pendiente NE** (sprite **1031**) |
| **(12,11)** | **Cruce X\|Y en pendiente SE** (`m5=0x03` → solo **1032**, como OpenTTD) |
| **(15,11)** | **Cruce X\|Y en pendiente SW** (**1033**) |
| **(18,11)** | **Cruce X\|Y en pendiente NW** (**1034**) |
| **(1,13)** | **T vía en pendiente NE** (`m5=0x07` → solo **1031**; overlays solo en plano) |
| **(4,13)** | **T vía pendiente SE** |
| **(7,13)** | **T vía pendiente SW** |
| **(10,13)** | **T vía pendiente NW** |
| **(1,15)** | **Cruce X\|Y pendiente NE** (comparar con cruce plano **7,5**) |
| **(4,15)** | **Cruce X\|Y pendiente SE** |
| **(7,15)** | **Cruce X\|Y pendiente SW** |
| **(10,15)** | **Cruce X\|Y pendiente NW** |
| (5,5) | **T plano** (referencia: base **1018** + overlays) |
| (7,5) | **Cruce plano** (`m5=0x03` → sprite **1017**) |
| y=12 | Buffer hierba (bajo pendientes y=11) |
| y=14 | Buffer hierba (borde sur del mapa) |

Tests: `cargo test -p openttdrs-core --test ottdmap_sp3_visual_fixture`

### Laboratorio pendiente + agua (`sp3_slope_lab`)

Regenerar: `python3 scripts/gen_sp3_slope_lab_ottdmap.py`

```bash
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_slope_lab.ottdmap cargo run -p openttdrs-client
```

**Mapa 16×20** — referencias planas, lago 3×3, costa y vía en pendiente (recta / cruce / T / HORZ / VERT). Sin casas, industrias ni climas.

| Fila y | Contenido |
|--------|-----------|
| **1** | Vía plana Y, X, T, cruce (x=1,4,7,10) + **HORZ** (13) / **VERT** (15) → **1035** / **1036** |
| **3–5** | Lago Clear 3×3; centro **(3,4)** = mar animado sin tierra en 8-vecinos |
| **(8,4)** | Costa explícita `m5=0x10` |
| **8** | Recta Y en pendiente NE/SE/SW/NW (solo **1031–1034**) |
| **11** | Cruce X\|Y en 4 pendientes (solo sprite inclinado) |
| **14** | T en 4 pendientes (solo sprite inclinado; comparar T plano y=1) |
| **16** | HORZ (`0x0C`) en 4 pendientes (solo sprite inclinado; comparar **1035** en y=1) |
| **18** | VERT (`0x30`) en 4 pendientes (comparar **1036** en y=1; fila y=17 = buffer) |

Tests: `cargo test -p openttdrs-core --test ottdmap_sp3_slope_lab_fixture`

### Checklist visual (capturas manuales)

Marcar tras cargar el fixture checklist:

- [ ] Fila y=3: carretera plana (Y, X, T, cruce, cruces nivel, tranvía X en x=15)
- [ ] Fila y=5: vía (Y, X, T, cruce, señales, nieve)
- [ ] Fila y=7: carretera en 4 pendientes (`road_flat_11..14`) + tranvía NE en (13,7) + estación tren NE en (16,7)
- [x] Fila y=9: bus NE/SE/SW/NW (x=1,3,5,7) + camión + tren + casa + **bus NE en pendiente (15,9)**
- [ ] Fila y=11: charco Clear **(2–3,11)** + costa **(5,11)** + recta Y NE (9,11) + **cruce X\|Y en pendiente** (12,15,18,11)
- [ ] Fila y=13: T vía en 4 pendientes (x=1,4,7,10) — comparar con T plano (5,5) y cruce plano (7,5)
- [ ] Fila y=15: **cruce X\|Y en 4 pendientes** (x=1,4,7,10) — comparar con fila y=11 y cruce plano (7,5)

**Nota:** el mapa procedural por defecto (`cargo run -p openttdrs-client`) mezcla todo en la demo de transporte; para regresión visual usar el fixture checklist.

## SP3.5 — agua y costa

- `parse_sav.py`: `export_ottdmap_from_chunks`, histograma `water` en `analyze_save`.
- `scripts/verify_parse_sav_water_m5.py`: MAP5 agua == m5 `.ottdmap`; fixture SP3 con Coast `0x10` en (5,11).
- Cliente: `RenderGrid` usa `m5>>4==1` (Coast) sin depender de vecinos; tests `iso` + `grid.rs`.
- Animación mar: `water_sprite_color` — ciclos dark×5 + glitter×15 con interpolación suave y destellos cian (solo teselas Clear, no `shore_*`).

## SP3.6 — rendimiento mapa grande

- `render/viewport.rs`: rectángulo de teselas visibles desde cámara ortográfica + margen.
- `spawn_world_layer` solo itera `MapTileSpawnViewport.bounds` en mapas ≥ 4096 teselas.
- `sync_map_tile_spawn_viewport`: remap al panear fuera del bloque (sin `sync_camera`).
- Bench manual: `scripts/bench_large_map_viewport.md` + `OTTDMAP_FILE=tests/fixtures/stationlist-test.ottdmap`.

## Siguiente fase

**SP3.1 en saves reales** — exportar `.ottdmap` con `parse_sav.py` y comparar tramos en pendiente con OpenTTD.

**SP4 / I8** — fuera de SP3 visual (multijugador, NewGRF completo, etc.).

Detalle máquina-legible: [SP3_AUDIT_REPORT.json](SP3_AUDIT_REPORT.json) (regenerar con `audit_sp3_assets.py`).
