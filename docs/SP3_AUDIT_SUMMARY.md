# SP3.0 — Resultado de auditoría visual (assets)

Generado con:

```bash
python3 scripts/audit_sp3_assets.py --json docs/SP3_AUDIT_REPORT.json
```

Re-ejecutar tras `bash scripts/descargar_graficos.sh` o cambios en precarga de sprites.

## Resumen

| Métrica | Valor |
|---------|------:|
| PNG requeridos por `WorldAssets::load` | 519 |
| Presentes | 519 |
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
- **terrain** / **water** / **station** / **industry** / **house** — sin faltantes en esta máquina.
- **transport_object** — túneles, puentes, depósitos OK.

## Fixtures `.ottdmap` (pruebas de mapa real)

| Fixture | Uso |
|---------|-----|
| `crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap` | TNBP en CI |
| `crates/openttdrs-core/tests/fixtures/m3_road_tram_2x2.ottdmap` | Tranvía `m3` |
| `crates/openttdrs-core/tests/fixtures/v5p12_stxy.ottdmap` | Footer estaciones |
| `crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap` | **Checklist visual denso** (12×8) |
| `tests/fixtures/stationlist-test.ottdmap` | Lista estaciones |

## Referencia upstream

Clon presente en `reference/openttd-upstream/` (para leer `road_cmd.cpp`, `rail_cmd.cpp`, etc.).

Si falta: `bash scripts/fetch-openttd-reference.sh`.

## Prueba manual (SP3.0)

Comando:

```bash
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/v5p12_tnbp.ottdmap cargo run -p openttdrs-client
```

### Resultado 2026-05-22 (fixture `v5p12_tnbp.ottdmap`)

Captura: [sp3/manual-v5p12_tnbp-2026-05-22.png](sp3/manual-v5p12_tnbp-2026-05-22.png)

| Comprobación | Resultado |
|--------------|-----------|
| Carga `.ottdmap` sin error | OK |
| Mapa 2×2, minimapa coherente | OK (2 teselas verdes + 2 marrones en minimapa) |
| Footer TNBP en log | OK — `1 túnel(es) JGR`, extremos norte/sur 1/1 |
| Render isométrico (hierba + asfalto en pendiente) | OK — tesela `(1,0)` con `h:1`, `slope:12 (NE)` |
| Sin crash al cerrar ventana | OK |

**Consola (resumen):** `Grass: 2`, `Road: 2` (las dos teselas `MP_TUNNELBRIDGE` con `m5=0` se clasifican como `TileKind::Road` en `ottd_tile_kind`); TNBP vs mapa 1/1; 0 industrias/estaciones/vehículos (esperado en fixture mínimo).

**HUD en `(1,0)`:** `mapt:0x90`, `m5:0x00`, `rb:0x08` — `road_bits_for_render` infiere bits por vecinos al no haber trazado en M5 (fixture vacío en carretera).

**Alcance:** este fixture valida **TNBP + carga + dibujo básico en pendiente NE**; **no** sustituye revisión de cruces, T, estaciones, industrias ni costa. Para eso usar un `.ottdmap` exportado con `scripts/parse_sav.py` desde una partida real (o ampliar fixtures en `crates/openttdrs-core/tests/fixtures/`).

### Fixture checklist denso (`sp3_visual_checklist.ottdmap`)

Regenerar: `python3 scripts/gen_sp3_visual_checklist_ottdmap.py`

```bash
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client
```

| Zona (x,y) | Contenido |
|------------|-----------|
| (1–4, 2) | Carretera Y, X, T (`0x07`), cruce `0x0F` |
| (5–6, 2) | Cruce a nivel eje X (`m5=0x40`) / Y (`0x41`) |
| (7, 2) | Carretera + tranvía (`m3=0x0A`) |
| (1–4, 3) | Vía Y, X, T, cruce (`m5` track bits 2/1/7/3) |
| (8, 3) | Vía con señales (`m5` subtype 1, `m3=0xC0`) |
| (9, 3) | Vía nieve (`m3` bajo = 12 / snow ground) |
| (0, 5) | Casa (`MP_HOUSE`, `m8=0` Tall Office) |
| (1, 5) | Parada camión (`m6` truck) |
| (4, 4) | Estación tren (`m6` rail, plataforma+edificio; junto al cruce de vía) |
| (4, 5) | `MP_INDUSTRY` gfx 0 (coal mine) |
| (2–3, 7) | Agua Clear + Coast (`m5=0x10`), hierba en (1,7) y (4,7) |

Tests: `cargo test -p openttdrs-core --test ottdmap_sp3_visual_fixture`

### Checklist visual (capturas manuales)

- [ ] Cruce carretera/vía y T en carretera plana — filas y=2 y y=3
- [ ] Casa — (0, 5)
- [ ] Estación / parada — (1, 5) y tren — (4, 4)
- [ ] Industria + gfx — (4, 5)
- [ ] Costa / MP_WATER — (2–3, 7) *(automático: fixture + `verify_parse_sav_water_m5.py`)*

## SP3.5 — agua y costa

- `parse_sav.py`: `export_ottdmap_from_chunks`, histograma `water` en `analyze_save`.
- `scripts/verify_parse_sav_water_m5.py`: MAP5 agua == m5 `.ottdmap`; fixture SP3 con Coast `0x10` en (3,7).
- Cliente: `RenderGrid` usa `m5>>4==1` (Coast) sin depender de vecinos; tests `iso` + `grid.rs`.
- Animación mar: `water_sprite_color` — ciclos dark×5 + glitter×15 con interpolación suave y destellos cian (solo teselas Clear, no `shore_*`).

## SP3.6 — rendimiento mapa grande

- `render/viewport.rs`: rectángulo de teselas visibles desde cámara ortográfica + margen.
- `spawn_world_layer` solo itera `MapTileSpawnViewport.bounds` en mapas ≥ 4096 teselas.
- `sync_map_tile_spawn_viewport`: remap al panear fuera del bloque (sin `sync_camera`).
- Bench manual: `scripts/bench_large_map_viewport.md` + `OTTDMAP_FILE=tests/fixtures/stationlist-test.ottdmap`.

## Siguiente fase

**SP4 / I8** — fuera de SP3 visual (multijugador, NewGRF completo, etc.).

Detalle máquina-legible: [SP3_AUDIT_REPORT.json](SP3_AUDIT_REPORT.json) (no versionar si es ruidoso; está en `.gitignore` opcional — por ahora se puede commitear como snapshot).
