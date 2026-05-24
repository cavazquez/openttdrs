# Plan: depósito de carretera y alineación visual

Documento de **retomada** tras revertir el trabajo experimental en depósito (2026-05).
Estado del código en `main` (commit `6641239`): render **legacy** — un PNG 60×47 centrado
en el rombo (`spawn_object_sprite` + `ROAD_DEPOT_BUILDING_BY_DIR`).

**Relacionado:** [PLAN_PARADAS_REMAPCOORDS.md](PLAN_PARADAS_REMAPCOORDS.md),
[SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md),
[TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) § depósito carretera,
[PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md).

---

## 1. Problema reportado

| Síntoma | Causa en `main` |
|---------|-----------------|
| Depósito “desalineado” con la rejilla | Un solo sprite centrado en `tile_pos_half`; ignora `TILE_SEQ` + NFO de OpenTTD |
| Parece en “otra tesela” vs la boca/carretera | Mismo motivo: no hay capas ni tramo de carretera en la tesela del depósito |

OpenTTD **no** centra el edificio en el rombo: pinta **suelo 2634** + **road_flat**
(`DiagDirToRoadBits`) + **1–2 capas BUILD** con `RemapCoords` desde `road_land.h`.

---

## 2. Referencia upstream

| Recurso | Contenido |
|---------|-----------|
| `third_party/openttd/road_land.h` (copiar de upstream si falta) | `_road_depot_NE/SE/SW/NW`, suelo `0xA4A` (2634) |
| `road_cmd.cpp` → `DrawTile_Road` caso `RoadTileType::Depot` | `DrawGroundSprite` + overlay carretera + `DrawRailTileSeq` |
| `road_func.h` → `DiagDirToRoadBits` | Bit de acceso en la tesela del depósito |
| Sprites OpenGFX | 1408–1411 bocas 12×12; 1412/1413 edificios; 2634 losa |

Secuencia por dirección (TILE_SEQ):

| Dir | Capas |
|-----|-------|
| NE | 1412 @ (0,15) |
| SE | 1408 @ (0,0) + 1409 @ (15,0) |
| SW | 1410 @ (0,0) + 1411 @ (0,15) |
| NW | 1413 @ (15,0) |

Salida lógica: `road_depot_exit_for_dir` en `transport.rs` (tesela **vecina** con road bits).

---

## 3. Estado actual del cliente (`main`)

```text
TileKind::RoadDepot → spawn_object_sprite(ROAD_DEPOT_BUILDING_BY_DIR[dir])
```

- Sin suelo 2634, sin `road_flat` en la tesela, sin bocas 12×12.
- Preview: un fantasma centrado (`preview/sprites.rs`).
- Assets: `road_depots` = 4 PNG grandes; `descargar_graficos.sh` exporta 1408–1413 + 1412/1413.

Paradas bus/camión **sí** usan el pipeline correcto (`gen_road_stop_gfx_data.py`,
`spawn_road_stop_buildings`, preview multi-capa). El depósito es el hueco pendiente.

---

## 4. Trabajo experimental revertido (lecciones)

Se probó en rama local (no mergeado) aproximadamente:

1. **`scripts/gen_road_depot_gfx_data.py`** — generar `road_depot_gfx_data_generated.rs` desde
   `road_land.h` + NFO (como paradas).
2. **Render:** suelo + `road_depot_entrance_road_bits` + capas BUILD con `road_stop_build_sprite_center`.
3. **Preview** multi-capa (`preview/road_depot.rs`).
4. **Calibración:** primero corrección “centro del rombo” (empeoró encaje con carretera);
   luego anclaje a centro de losa 2634 + offset de boca por dirección.

### Resultados visuales (sesión)

| Iteración | Resultado |
|-----------|-----------|
| PNG único centrado (`main`) | Tesela correcta, **desalineado** con rombo |
| RemapCoords puro | Boca + edificio coherentes entre sí, pero edificio “en otra tesela” |
| Centrar edificio 60×47 en rombo | Vuelve a tesela lógica, **rompe** boca y carretera |
| Losa OK + calibración por capa | Mejor integración suelo/carretera; boca/edificio aún sensibles a `MOUTH_REL` |

### Confusión frecuente en capturas

- **`COAL 0/20`** flotante = etiqueta de **carga del camión**, no nombre de estación.
- **Depósito con calzada bajo el edificio** = comportamiento OpenTTD (no es parada).
- **Paradas “en medio de la calle”** en mapa demo = bug de bootstrap (ver §5).

---

## 5. Paradas demo sobre la carretera — **corregido (2026-05-22)**

En `demo_layout.rs`, las paradas del ciclo económico estaban en la **misma Y que la carretera**:

```rust
// Antes (incorrecto):
DEMO_ECONOMY_LOAD_STATION = (3, DEMO_ROAD_Y)
```

`place_demo_truck_station_tile` forzaba `TileKind::Station` **sin** `PlaceStationDir` ni
`check_station_placement` (que exige **hierba** contigua a la red, no encima).

**Fix aplicado:** paradas en hierba al norte de la vía (`y = DEMO_ROAD_Y - 1`) con
`Command::PlaceStationDir` y entrada `DIAGDIR_SE` hacia la carretera. Mismo flujo que la
toolbar del jugador.

---

## 6. Roadmap de implementación (cuando se retome)

### Fase A — Infraestructura (paridad paradas)

- [ ] Añadir `third_party/openttd/road_land.h` (ya usado en sesión; volver a copiar).
- [ ] `scripts/gen_road_depot_gfx_data.py` → `road_depot_gfx_data_generated.rs`.
- [ ] `road_depot_build_layers`, `road_depot_ground_layer`, `road_depot_entrance_road_bits`.
- [ ] `objects.rs`: suelo + road_flat + capas BUILD.
- [ ] Preview alineado con mapa.
- [ ] Tests: conteos de capas, `DiagDirToRoadBits`, centros vs losa (tolerancia px).

### Fase B — Calibración visual SP3

- [ ] Capturas 4 direcciones en mapa plano con carretera en las 4 diagonales.
- [ ] Ajustar `MOUTH_REL_FROM_GROUND` / `remap_x_adj` en el generador (patrón
      `compute_layer_corrections` de `gen_road_stop_gfx_data.py`).
- [ ] Entrada en checklist: `sp3_visual_checklist.ottdmap` o fixture dedicado `road_depot_4dir`.

### Fase C — Demo y UX

- [x] Corregir paradas demo (§5): hierba adyacente + `PlaceStationDir`.
- [ ] Documentar en HUD/tooltip: depósito vs parada de carga.

### Fase D — Opcional

- [ ] Depósito en pendiente (`DrawFoundation` upstream).
- [ ] Paleta compañía en sprites depot (`PALETTE_MODIFIER_COLOUR` en `road_land.h`).

---

## 7. Archivos tocados en el experimento (referencia)

| Área | Archivos |
|------|----------|
| Generador | `scripts/gen_road_depot_gfx_data.py` |
| Datos | `crates/.../sprites/road_depot_gfx_data_generated.rs` |
| Sprites API | `sprites/station.rs`, `sprites/road.rs`, `sprites.rs` |
| Render | `render/tiles/objects.rs`, `render/assets.rs` |
| Preview | `ui/toolbar/preview/road_depot.rs`, `preview/mod.rs` |
| Assets | `scripts/descargar_graficos.sh` (`road_depot_ground.png`) |
| Upstream | `third_party/openttd/road_land.h` |

Comando regeneración:

```bash
python3 scripts/gen_road_depot_gfx_data.py
bash scripts/check.sh ci
```

---

## 8. Criterios de “hecho”

1. Depósito en las **4 orientaciones** alineado con losa + tramo de carretera + boca visible.
2. Preview de construcción coincide con tesela colocada.
3. No regresión en paradas bus/camión (`PLAN_PARADAS_REMAPCOORDS.md`).
4. Demo sin paradas encima del trazado de carretera.
5. `check.sh ci` en verde.

---

*Última actualización: 2026-05-22 — trabajo de depósito revertido; doc para retomada.*
