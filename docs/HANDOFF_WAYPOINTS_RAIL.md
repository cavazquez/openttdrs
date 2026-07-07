# Handoff: waypoints ferroviarios — render incorrecto

**Audiencia:** IA con más contexto / modelo más capaz.  
**Estado:** trabajo **sin commit** en `main` (cambios locales + atlas regenerado).  
**Prioridad:** alta — el usuario confirma que **sigue viéndose mal** tras los intentos de esta sesión.  
**Última actualización:** jul 2026.

---

## 1. Resumen ejecutivo

Los **waypoints ferroviarios** (`StationType::RailWaypoint`) no se renderizan como en OpenTTD con OpenGFX2+ Stations. El usuario reportó la evolución en tres capturas:

| Fase | Síntoma | Hipótesis de causa |
|------|---------|-------------------|
| 1 | Dos toldos **morados/planos** flotando a los lados de la vía | Sprites del GRF *extra* (4974–4977 vanilla) + rampa CC púrpura sin hornear |
| 2 | Dos casetas **cyan/azul** separadas a los lados (no integradas) | Sprites ogfx2 correctos pero `TILE_SEQ dy=11` duplicando offset |
| 3 | Forma en **L** sobre la vía, vía **cortada** en la tesela del waypoint | `dy=11` **o** ancla/offset incorrecto; tras poner `dy=0` el usuario **aún** ve mal |

**Objetivo de paridad:** una sola caseta con ballast + vía continua + techo recoloreado por compañía (como OpenTTD con `ogfx2_stations.grf` activo).

---

## 2. Cómo reproducir

```bash
cd openttdrs

# Regenerar assets (si hace falta)
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py

# Compilar y ejecutar (obligatorio tras cambios en Rust/assets)
cargo run -p openttdrs-client

# En partida: construir vía recta (eje X o Y), colocar waypoint desde toolbar ferroviario.
```

**Tests relevantes:**

```bash
cargo test -p openttdrs-client rail_waypoint
./scripts/check.sh
```

**Partida de prueba del usuario (otros bugs):** `save/partida_2026-06-22_0942.json`  
(El bug de waypoint es reproducible en mapa nuevo con una sola vía recta.)

---

## 3. Comportamiento de referencia (OpenTTD + OpenGFX2)

### 3.1 Vanilla (`station_land.h`)

```c
// third_party/openttd/station_land.h
static const DrawTileSeqStruct _station_display_datas_waypoint_X[] = {
    TILE_SEQ_LINE( 0,  0,  0, 16,  5, 16, SPR_WAYPOINT_X_1 | PALETTE_MODIFIER_COLOUR)
    TILE_SEQ_LINE( 0, 11,  0, 16,  5, 16, SPR_WAYPOINT_X_2 | PALETTE_MODIFIER_COLOUR)
};
static const DrawTileSeqStruct _station_display_datas_waypoint_Y[] = {
    TILE_SEQ_LINE( 0,  0,  0,  5, 16, 16, SPR_WAYPOINT_Y_1 | PALETTE_MODIFIER_COLOUR)
    TILE_SEQ_LINE(11,  0,  0,  5, 16, 16, SPR_WAYPOINT_Y_2 | PALETTE_MODIFIER_COLOUR)
};
```

- Eje X: segunda capa con **`dy=11`**.
- Eje Y: segunda capa con **`dx=11`**.
- Ground: `SPR_RAIL_TRACK_X` (1012) o `SPR_RAIL_TRACK_Y` (1011) **por separado** bajo los overlays.

### 3.2 OpenGFX2+ Stations (`ogfx2_stations.grf`)

Con el NewGRF activo, OpenTTD **no usa** los PNG vanilla de 4974–4977: Action 1 reemplaza por sprites internos del GRF:

| Sprite ogfx2 | Alias OpenTTD | Tamaño | xrel | yrel | Uso |
|--------------|---------------|--------|------|------|-----|
| 19 | 4974 (X oeste) | 40×29 | -30 | -9 | Mitad oeste + ballast eje X |
| 20 | 4975 (X este) | 40×29 | -8 | -9 | Mitad este |
| 21 | — | 23×14 | -23 | -5 | **Toldo CC** (overlay compañía, eje X) |
| 22 | — | 23×14 | 2 | -5 | **Toldo CC** este |
| 23 | 4976 (Y oeste) | 38×28 | -28 | -8 | Mitad oeste eje Y |
| 24 | 4977 (Y este) | 38×28 | -8 | -8 | Mitad este |
| 25–26 | — | 23×14 | … | … | Toldos CC eje Y |

NFO decodificado en: `assets/opengfx/.ogfx2_stations_decode/sprites/ogfx2_stations.nfo`  
(líneas 42–57; generado por `gen_rail_waypoint_sprites.py` vía `grfcodec`).

**Importante:** el GRF define **Action 2** (layout avanzado, ~línea 32 del NFO) con ground `1012`/`1011` y **hasta 4 child sprites** por layout. OpenTTD con el GRF activo puede ignorar `station_land.h` y usar ese layout. **Nuestro cliente solo imita `station_land.h` con 2 capas (19/20 o 23/24), sin 21/22 ni el layout Action 2 completo.**

---

## 4. Implementación actual en openttdrs

### 4.1 Simulación (colocación)

`crates/openttdrs-core/src/command/transport/station.rs`:

```rust
let axis_y = rail_waypoint_axis_from_trackbits(tile.m5).unwrap_or(false);
out.m5 = u8::from(axis_y);  // bit 0 = eje Y; 0 = eje X
out.m6 = apply_station_m6(out.m6, StopKind::RailWaypoint);
```

- `RAIL_TB_X` (1) → vía diagonal **abajo-izq → arriba-der** → sprite 1012 → `m5=0` → secuencia X.
- `RAIL_TB_Y` (2) → vía **arriba-izq → abajo-der** → sprite 1011 → `m5=1` → secuencia Y.

### 4.2 Render

`crates/openttdrs-client/src/render/tiles/objects.rs` → `spawn_station_tile`:

1. Hierba bajo estación (si `tileh==0`).
2. `spawn_rail_foundation` (cimiento en pendiente).
3. **No** dibuja vía de fondo si `StationTileClass::RailWaypoint` (los sprites ogfx2 ya incluyen ballast).
4. Dos capas de `rail_waypoint_draw_layers(m5)` en posiciones de `rail_waypoint_sprite_center`.

### 4.3 Secuencias y posicionamiento

`crates/openttdrs-client/src/sprites/station.rs`:

```rust
static RAIL_WAYPOINT_SEQ_X: [RailStationLayer; 2] = [
    layer(4974, 0.0, 0.0, 0.0, 0.05),
    layer(4975, 0.0, 0.0, 0.0, 0.06),  // sin dy=11
];
static RAIL_WAYPOINT_SEQ_Y: [RailStationLayer; 2] = [
    layer(4976, 0.0, 0.0, 0.0, 0.05),
    layer(4977, 0.0, 0.0, 0.0, 0.06),  // sin dx=11
];
```

Posición:

```rust
pub fn rail_waypoint_sprite_center(...) -> Vec3 {
    let (xrel, yrel) = rail_station_overlay_rel(seq, nfo_xrel, nfo_yrel);
    overlay_pos(ref_pos, xrel, yrel, w, h, base_z, layer_z, tx, ty)
}
```

- `ref_pos` = `ctx.iso_pos` = vértice superior del rombo (`iso(tx, ty)`).
- `overlay_pos` coloca el **centro** del sprite (ancla Bevy por defecto).
- `remap_tile_offset(dx,dy,dz) * 0.5` escala offsets `TILE_SEQ` al rombo ~64×31 px.

### 4.4 Metadata NFO generada

`crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs`:

```
4974: 40×29, xrel=-30, yrel=-9
4975: 40×29, xrel=-8,  yrel=-9
4976: 38×28, xrel=-28, yrel=-8
4977: 38×28, xrel=-8,  yrel=-8
```

### 4.5 Assets

- Script: `scripts/gen_rail_waypoint_sprites.py` — descarga `ogfx2_stations.grf`, extrae 19/20/23/24, hornea paleta a `DARK_BLUE`.
- PNG: `assets/opengfx/tiles/rail_4974.png` … `rail_4977.png`
- Atlas: `assets/opengfx/atlas/tiles_atlas_0.png` + `tile_atlas_generated.rs`

### 4.6 Paleta compañía

`company_palette.rs`: remapeo **siempre activo** (incluso `DarkBlue`) para convertir rampas CC ajenas en runtime. Los PNG están horneados a `DARK_BLUE` en el script.

---

## 5. Cambios intentados (sin commit)

| Cambio | Archivos | Resultado según usuario |
|--------|----------|-------------------------|
| Extraer sprites de `ogfx2_stations` (no extra 4974 vanilla) | `gen_rail_waypoint_sprites.py`, PNG, atlas | Mejor color, sigue mal posición |
| Metadata NFO ogfx2 (40×29, xrel -30/-8) | `gen_rail_station_draw_data.py`, `rail_station_draw_data_generated.rs` | — |
| Hornear rampa CC → DARK_BLUE en script | `gen_rail_waypoint_sprites.py` | Fin de morado plano |
| Remapeo compañía siempre on | `company_palette.rs` | Cyan/azul compañía |
| Quitar `dy=11` / `dx=11` en `RAIL_WAYPOINT_SEQ_*` | `station.rs` | **Usuario: sigue mal** |
| No dibujar vía duplicada bajo waypoint | `objects.rs`, `preview/rail_waypoint.rs` | Vía cortada en captura (esperado si sprites no alinean) |
| Tests regresión `rail_waypoint_*` (4 tests) | `station.rs` | Pasan en CI local |

**`git status`:** 9 archivos modificados (ver `git diff --stat`).

---

## 6. Evidencia: composición offline (ground truth)

Se validó offline que con la matemática de `rail_station_overlay_rel` + metadata ogfx2:

| Composición | Eje | TILE_SEQ 2ª capa | Resultado visual |
|-------------|-----|------------------|------------------|
| `wp_on_track_x0` | X (1012) | `dy=0` | ✅ Caseta integrada, vía continua |
| `wp_on_track_x11` | X (1012) | `dy=11` | ❌ Forma en **L** (coincide con captura fase 3 del usuario) |
| `wp_on_ytrack_y0` | Y (1011) | `dx=0` | ✅ Caseta integrada |
| `wp_on_ytrack_y11` | Y (1011) | `dx=11` | ❌ Piezas separadas / fuera de tesela |

**Script de reproducción** (ejecutar en repo):

```python
# Pegar en python3 desde openttdrs/
from PIL import Image
from pathlib import Path
TILES = Path("assets/opengfx/tiles")

def remap(dx, dy, dz=0):
    return (dy - dx) * 2.0, -(dx + dy - dz) * 1.0  # ×0.5 ya aplicado

def overlay_rel(nfo_xr, nfo_yr, dx, dy):
    ox, oy = remap(dx, dy)
    return nfo_xr + ox, nfo_yr - oy

def compose(track, halves, layers, out):
    bg = Image.open(TILES / track).convert("RGBA")
    pad = 40
    canvas = Image.new("RGBA", (bg.width + 2*pad, bg.height + 2*pad), (0,0,0,0))
    canvas.alpha_composite(bg, (pad, pad))
    for (name, xr, yr), (dx, dy) in zip(halves, layers):
        x, y = overlay_rel(xr, yr, dx, dy)
        canvas.alpha_composite(Image.open(TILES/name).convert("RGBA"), (pad+int(x), pad+int(y)))
    canvas.save(out)

compose("rail_1012.png",
    [("rail_4974.png",-30,-9),("rail_4975.png",-8,-9)],
    [(0,0),(0,0)], "/tmp/wp_x_correct.png")
compose("rail_1012.png",
    [("rail_4974.png",-30,-9),("rail_4975.png",-8,-9)],
    [(0,0),(0,11)], "/tmp/wp_x_bug_L.png")
```

**Conclusión de la evidencia:** con el código actual (`dy=dx=0`), la composición offline es **correcta**. Si el juego sigue mostrando la forma en L, o bien (a) no se está ejecutando el binario recompilado, o bien (b) hay **otra discrepancia** entre la composición offline y el pipeline Bevy (ancla, atlas, capas faltantes, eje mal detectado).

---

## 7. Hipótesis priorizadas para la siguiente IA

### H1 — Binario / atlas desactualizado (verificar primero)

- Confirmar que `cargo run -p openttdrs-client` se ejecutó **después** de los cambios.
- Borrar `target/` y regenerar atlas si hace falta.
- Añadir log temporal en `spawn_station_tile` imprimiendo `layer.dx`, `layer.dy`, `m5`, `pos3` para waypoint.

### H2 — Faltan capas 21/22 (toldos CC) del layout Action 2

OpenGFX2 dibuja **4** sprites en eje X: cuerpo 19+20 + toldos 21+22 con `PALETTE_MODIFIER_COLOUR`. Sin 21/22 la caseta puede verse incompleta o “dos piezas sueltas”. **No explica sola la forma en L**, pero es necesario para paridad.

**Acción:** extender `RAIL_WAYPOINT_SEQ_X` con sprites 21/22 (exportar en script o referenciar por ID interno), mismo para 25/26 en eje Y.

### H3 — Implementar layout Action 2 de `ogfx2_stations.grf` en lugar de `station_land.h`

Parchear solo `TILE_SEQ` puede ser insuficiente: OpenTTD con el GRF usa `DrawRailTileSeq` con layout del NewGRF (ground 1012 + children con offsets del Action 2, no necesariamente `dy=11`). Decodificar bytes en `ogfx2_stations.nfo` líneas 32–33 y replicar en Rust o JSON generado.

### H4 — Ancla `iso()` vs ancla OpenTTD para child sprites

- `iso(tx,ty)` = vértice **superior** del rombo.
- OpenTTD `DrawCommonTileSeq` usa esquina de tesela + `RemapCoords` + `xrel/yrel` como **esquina superior izquierda** del sprite.
- `overlay_pos` convierte a **centro** Bevy: `ref + (xrel + w/2, -yrel - h/2)`.
- Las **estaciones rail normales** (1070–1082) usan el mismo pipeline y **funcionan** → la ancla debería ser coherente, salvo que waypoints tengan bounding box distinto.

**Acción:** comparar posición real en pantalla de un waypoint vs una plataforma rail en la misma tesela (gizmo o `println!`).

### H5 — Detección de eje invertida o `m5` corrupto

- Verificar en runtime: `tile.m5 & 1` vs orientación visual del riel adyacente.
- Saves importados: ¿`m5` del waypoint preserva bit 0?
- Test: colocar waypoint en vía **Y** (1011) y confirmar que usa 4976/4977.

### H6 — Atlas UV / tamaño de sprite en GPU

- Si el atlas recorta mal 4974/4975, las mitades podrían desalinearse.
- Comparar dimensiones en `tile_atlas_generated.rs` vs PNG fuente (40×29).
- Probar render forzando PNG sueltos (`assets/opengfx/tiles/rail_4974.png`) sin atlas.

### H7 — `CompanyColoredSprites` con textura distinta al atlas

` sprite_from_atlas_or_company_white` prefiere textura recoloreada. Si el recolor altera bbox o hay bug en caché, podría desalinear. Probar desactivando compañía (`company: None`) temporalmente.

### H8 — Z-order / hierba / cimiento tapando ballast

`spawn_rail_foundation` + hierba bajo waypoint podría ocultar parte del ballast y dar sensación de “vía cortada”. Menos probable que la forma en L.

---

## 8. Plan de ataque sugerido

1. **Instrumentar** `spawn_station_tile` (solo waypoints): log de `m5`, `class`, cada `layer.sprite_id`, `(dx,dy)`, `nfo xrel/yrel`, `pos3`, y si usa atlas o company texture.
2. **Captura de referencia:** guardar screenshot del juego + overlay de gizmos en centros calculados.
3. **Comparar** con `/tmp/wp_x_correct.png` (script §6).
4. Si posiciones coinciden con composición correcta pero se ve mal → problema de **textura/atlas/capas faltantes** (H2, H6, H7).
5. Si posiciones coinciden con `dy=11` → buscar **otro** origen del offset (código viejo, ruta de render duplicada, preview superpuesto).
6. Implementar **layout Action 2** completo (H3) + capas 21/22 (H2).
7. Añadir test de integración visual o snapshot PNG en CI (render headless una tesela waypoint).

---

## 9. Mapa de archivos

| Archivo | Rol |
|---------|-----|
| `crates/openttdrs-client/src/sprites/station.rs` | Secuencias `RAIL_WAYPOINT_SEQ_*`, `rail_waypoint_draw_layers`, posicionamiento |
| `crates/openttdrs-client/src/render/tiles/objects.rs` | `spawn_station_tile` — spawn en mapa |
| `crates/openttdrs-client/src/ui/toolbar/preview/rail_waypoint.rs` | Fantasma de colocación |
| `crates/openttdrs-client/src/iso/coords.rs` | `iso`, `overlay_pos`, `remap_tile_offset` |
| `crates/openttdrs-client/src/sprites/rail_station_draw_data_generated.rs` | Metadata NFO (w,h,xrel,yrel) |
| `scripts/gen_rail_waypoint_sprites.py` | Extrae ogfx2 → PNG |
| `scripts/gen_rail_station_draw_data.py` | Genera metadata Rust |
| `scripts/gen_tile_atlas.py` | Atlas GPU |
| `assets/opengfx/tiles/rail_4974.png` … `4977.png` | Sprites fuente |
| `assets/opengfx/.ogfx2_stations_decode/` | NFO decodificado (gitignored parcialmente) |
| `third_party/openttd/station_land.h` | Referencia vanilla TILE_SEQ |
| `crates/openttdrs-core/src/command/transport/station.rs` | `place_rail_waypoint`, `m5` |
| `crates/openttdrs-client/src/sprites/company_palette.rs` | Recolor compañía |

---

## 10. Tests existentes (deben seguir pasando)

```
sprites::station::tests::rail_waypoint_ogfx2_uses_zero_tile_seq_offsets
sprites::station::tests::rail_waypoint_halves_offset_by_nfo_xrel
sprites::station::tests::rail_waypoint_y_halves_share_screen_row
sprites::station::tests::rail_waypoint_meta_covers_layer_sprites
```

Core: `place_rail_waypoint_on_straight_track`, `place_rail_waypoint_rejects_curved_track`.

---

## 11. Comandos útiles

```bash
cd openttdrs
./scripts/check.sh
cargo test -p openttdrs-client rail_waypoint -- --nocapture
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py
git diff crates/openttdrs-client/src/sprites/station.rs
```

Decodificar GRF (requiere `grfcodec`):

```bash
# Lo hace automáticamente gen_rail_waypoint_sprites.py
ls assets/opengfx/.ogfx2_stations_decode/sprites/ogfx2_stations.nfo
```

---

## 12. Criterio de “resuelto”

- [ ] Waypoint en vía recta **eje X**: una caseta, ballast continuo, vía alineada con teselas vecinas.
- [ ] Waypoint en vía recta **eje Y**: igual en orientación Y.
- [ ] Techo con color de compañía (como OpenTTD).
- [ ] Fantasma de colocación coincide con resultado final.
- [ ] `./scripts/check.sh` verde.
- [ ] (Opcional) Screenshot comparativo lado a lado con OpenTTD + ogfx2_stations.

---

## 13. Contexto de sesión anterior

- Commit previo en `main`: sonido de clic en menú (`d6173a9`).
- Waypoints: **sin commit**.
- Otros bugs documentados en `docs/HANDOFF_BUGS_VISUALES_TERRAIN.md` (fantasma verde, casas Toyland, etc.) — **distintos** de este handoff.

---

*Fin del handoff waypoints — jul 2026. Priorizar H1 (verificación runtime) y H2/H3 (layout Action 2 completo).*
