# Handoff: bugs visuales de terreno, ghost y casas

**Audiencia:** IA con más contexto / modelo más capaz.  
**Estado:** trabajo **sin commit** en `main` (2 commits previos de audio sin push).  
**Última captura del usuario:** `save/partida_2026-06-22_0942.json`, mapa procedural grande, clima templado.

---

## 1. Resumen ejecutivo

El usuario reportó tres problemas relacionados:

| # | Síntoma | Estado tras intentos locales |
|---|---------|------------------------------|
| A | Tablero marrón/verde al **iniciar** partida | Parcialmente mitigado (ver §3) |
| B | Rombo de teselas oscuras al **construir** carretera | Parcialmente mitigado (remap incremental) |
| C | Rectángulo verde semitransparente al iniciar (fantasma de construcción) | **Abierto** |
| D | Casas Toyland (cerdos, osos) en pueblo templado | **Abierto** |
| E | Orillas de agua con artefactos blancos | **Abierto** (menor) |

El usuario pidió **dejar documentado** y no seguir iterando con el modelo actual.

---

## 2. Cómo reproducir

```bash
cd openttdrs
# Partida del usuario (JSON en save/)
OPENTTDRS_JSON_SAVE=save/partida_2026-06-22_0942.json cargo run -p openttdrs-client

# O cargar desde menú / F9 tras arrancar
```

**Secuencia que dispara el ghost verde (hipótesis C):**

1. Entrar en partida con herramienta de paisaje activa (panel **Paisaje** → «Plantar bosque» / `BuildForest`).
2. El fantasma sigue al cursor desde el primer frame (`update_build_ghost_preview`).
3. Si el cursor está sobre el mapa al cargar, aparece un bloque ~2×N teselas sin que el usuario haya clicado.

**Secuencia rombo oscuro (hipótesis B):**

1. Mapa ≥ 64×64 con culling de viewport (`large_map_viewport_cull_enabled`).
2. Colocar un tramo de carretera.
3. Antes: rombo ~7×9 de hierba oscura/duplicada alrededor del tramo.

---

## 3. Cambios locales sin commit (verificar con `git diff`)

### 3.1 Render / remap incremental

**Archivos:** `crates/openttdrs-client/src/render/world.rs`, `crates/openttdrs-client/src/ui/toolbar/build_input/click.rs`

- Evitar doble `spawn_map_chunk` cuando un chunk está en `to_add` y `refresh_chunks`.
- Despawn de chunks a refrescar en batch antes de respawn.
- `road_action_refreshes_neighbors()` — remap incluye vecinos ortogonales (como rail).
- En construcción con culling: si `refresh_chunks` no está vacío → expandir a **todo el viewport visible** (`needed`).

**Riesgo:** refrescar todo el viewport en cada clic puede ser caro en mapas 256²; validar FPS.

### 3.2 Densidad de hierba

**Archivos:** `render/tiles/land.rs`, `render/assets.rs`, `world_gen.rs`, `save.rs`, `map/mod.rs`

- Sprites: `terrain_bare.png`, `terrain_grass_1_3.png`, `terrain_grass_2_3.png`, `terrain_rocky_1/2.png`.
- Densidad solo en `MP_CLEAR` (`ottd_type == 0`) + `CLEAR_GROUND_GRASS`.
- **`m5 == 0` → hierba completa** en render (compatibilidad con `Map::new_flat` y saves viejos).
- Migración save **v11**: teselas `Grass` + `mapt == 0` + `m5 == 0` → `m5 = 3`.
- `world_gen::grass_density`: ya no devuelve `0` (bare); solo 1/2/3.

**Conflicto no resuelto:** no se pudo cambiar `new_flat` a `m5 = 3` porque rompe pathfinder de carretera (`effective_road_bits`: `m5 & 0x0F == 0` → `0x0F`; con `m5 = 3` devuelve bits `3` y el BFS falla). Ver `pathfinder.rs:207-218` y tests `bfs_finds_path_on_straight_road`.

**Solución correcta a largo plazo:** al `set_kind(Road)` inicializar `m5` con bits de carretera, no depender del default de hierba.

### 3.3 Viewport al entrar en partida

**Archivo:** `render/world.rs`

- `setup()` usa `resolve_spawn_viewport_at(cam_pos, cam_scale)` en lugar de consultar la cámara antes de que exista en ECS (evitaba fallback a `full(mw,mh)`).

---

## 4. Bugs abiertos — guía para la IA siguiente

### 4.1 [C] Fantasma verde al iniciar (prioridad alta)

**Síntoma:** rectángulo verde semitransparente fijo o siguiendo el cursor al cargar partida; en captura el panel **Paisaje** está abierto con herramienta de árbol/bosque seleccionada.

**Archivos clave:**

| Archivo | Rol |
|---------|-----|
| `ui/toolbar/preview/mod.rs` | `update_build_ghost_preview` — spawnea `BuildGhostPreview` cada frame |
| `ui/toolbar/preview/industry.rs` | `spawn_industry_template_preview` — suelo `grass_rough` teñido (herramientas `BuildForest`, etc.) |
| `ui/toolbar/preview/station_coverage.rs` | Halo amarillo/rojo (menos probable si no hay estación activa) |
| `ui/toolbar/mod.rs` | `UiToolState`, `DragBuildState` |
| `persistence.rs` | `apply_loaded_state` — **no resetea** `UiToolState` ni `DragBuildState` |

**Hipótesis ordenadas:**

1. **Herramienta sigue activa** tras cargar partida o al entrar en `InGame` (`UiToolState` persiste entre sesiones de la misma ejecución; no hay `OnEnter` que la limpie).
2. **`DragBuildState.pending_tiles`** no vacío tras un arrastre interrumpido → preview multi-tesela (`preview/mod.rs:223-227`).
3. Fantasma **intencional** siguiendo cursor en centro de pantalla al inicio — UX: no mostrar preview hasta primer movimiento de ratón o clic en herramienta.

**Fix sugerido (mínimo):**

```rust
// En apply_loaded_state o OnEnter(InGame):
tool_state.active_tool = None;
drag_state.reset(); // armed=false, pending_tiles.clear(), etc.
```

**Fix sugerido (UX):**

- No ejecutar ghost si `!mouse_moved_since_tool_select` o si `cursor_position` es `None`.
- Cerrar panel de toolbar al cargar partida.

**Tests:** test de integración que cargue estado y assert `UiToolState::default()` + cero entidades `BuildGhostPreview`.

---

### 4.2 [D] Casas Toyland en clima templado (prioridad media)

**Síntoma:** en «Santa Cruz» aparecen estatuas de cerdo rosa y osos (sprites Toyland) en partida templada.

**Cadena de render:**

```
spawn_house_tile (land.rs)
  → clean_house_id = m8 & 0xFFF
  → house_draw_data_index_for_tile (sprites.rs)
  → HOUSE_DRAW_DATA[idx] → assets.houses[s1/s2]
```

**Hipótesis:**

1. **`m8` incorrecto** en bootstrap procedural de pueblos (`state/bootstrap/procedural_population/`) — IDs Toyland en mapa temperate.
2. **`house_id_for_draw_table`** hace `% 110` y mezcla tablas de clima.
3. Sprites **no filtrados por clima** al poblar `WorldAssets::houses` (carga todo el atlas).

**Referencias:**

- `docs/TILES_Y_SAVEGAMES_OPENTTD.md` § casas (`m8`, `HOUSE_DRAW_DATA`).
- `docs/ROADMAP_PARIDAD_VISUAL.md` — casas 110 originales.
- IDs Toyland típicos: rango alto en `town_land.h` / entradas > 110 en tabla extendida.

**Fix sugerido:**

- Al generar casas en población procedural, usar `HouseID` válidos para `Climate::Temperate`.
- En render: `house_id_for_draw_table` con filtro por `sim.state.climate` o sustituto de clima en `GameState`.
- Comparar `m8` del save del usuario tesela a tesela en Santa Cruz.

---

### 4.3 [B] Rombo oscuro al construir (prioridad media — verificar si cerrado)

**Causa raíz original:** remap incremental en mapas grandes dejaba capas de hierba superpuestas.

**Verificación pendiente:** probar en mapa 64×64+ del usuario tras los cambios de §3.1; si persiste:

- Inspeccionar si `apply_remap_map_visuals` corre **dos veces** el mismo frame (`setup` + `pending` en primer `Update`).
- `sync_map_tile_spawn_viewport` dispara `pending` al primer frame si bounds ≠ viewport insertado en `setup`.
- Log: contar entidades `MapVisualLayer` por tesela tras construir (debe ser 1 suelo).

**Archivos:** `render/world.rs`, `simulation.rs` (`flag_map_tile_dirty_remap`).

---

### 4.4 [A] Tablero de hierba al iniciar (prioridad baja si migración v11 OK)

Si tras **recargar** el save sigue habiendo patrón ajedrezado:

1. Confirmar que `save::load_from_str` aplica migración v10→v11.
2. Partidas guardadas ya en v11 con `m5` mezclados del world gen (densidades 1–3) — es **esperado** pero debe ser sutil; si es fuerte, ajustar sprites o mezclar con tint.
3. Teselas no-`MP_CLEAR` no deben usar densidad (ya corregido en `land.rs` para `Forest` y `Grass` genérico).

---

### 4.5 [E] Orillas de agua (prioridad baja)

Artefactos blancos en transición agua/tierra. Revisar:

- `push_water_tile` / `shore_*` sprites y animación (`WaterAnimFrames`).
- `mark_water_coasts` en `world_gen.rs` (`WATER_COAST_M5 = 0x10`).
- `ctx.info.use_shore` en spawn de agua.

---

## 5. Commits existentes (ya en `main`, sin push)

| Hash | Contenido |
|------|-----------|
| `ffdab02` | Jukebox + ventana «Sonido y música» + botón toolbar |
| `d2d7442` | Sin SFX de construcción al bootstrap del mapa |

**Sin commit:** todos los cambios de §3 (`git diff` ~8 archivos).

---

## 6. Comandos útiles

```bash
bash scripts/check.sh          # formato + clippy + tests
bash scripts/check.sh ci       # CI completo

# Tests remap / tiles
cargo test -p openttdrs-client apply_remap
cargo test -p openttdrs-core grass_density
cargo test -p openttdrs-core bfs_finds_path_on_straight_road
```

---

## 7. Decisiones de producto pendientes

1. ¿Mostrar fantasma de construcción **solo** con herramienta explícitamente seleccionada en ese frame (reset al cargar)?
2. ¿Densidad de hierba bare (`m5 & 3 == 0`) solo en mapas con flag `world_gen` en `GameState`? (requiere campo nuevo en save v12).
3. ¿Refresco de viewport completo en construcción o solo anillo de chunks alrededor del tramo?

---

## 8. Capturas de referencia (conversación usuario)

Las imágenes del hilo muestran la evolución:

1. Teselas oscuras dispersas (**antes** de construir).
2. Rombo grande tras colocar carretera.
3. Tablero marrón/verde al iniciar con save JSON.
4. Última: hierba más uniforme pero **ghost verde** + casas Toyland en Santa Cruz.

Rutas en workspace Cursor: `assets/image-*.png` del proyecto (no versionadas en git).

---

## 9. Checklist para cerrar el handoff

- [ ] Recargar `partida_2026-06-22_0942.json` y confirmar terreno sin tablero marrón
- [ ] Construir carretera en mapa grande sin rombo oscuro
- [ ] Al entrar en partida: **sin** ghost hasta elegir herramienta
- [ ] Santa Cruz: solo casas del clima del mapa
- [ ] `check.sh` verde
- [ ] Commit único o dos: «fix visual terrain/remap» + «fix ghost/houses on load»
- [ ] Bump doc `CURRENT_SAVE_VERSION` en README si se publica v11

---

*Fin del handoff — jul 2026.*
