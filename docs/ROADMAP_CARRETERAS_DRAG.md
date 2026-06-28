# Handoff — construcción de carreteras (drag / orientación)

**Estado (2026-06-22):** parcialmente mejorado; el usuario reporta que **sigue sin sentirse
correcto** en juego. Se deja así a petición suya. Este documento es para que otra sesión de
IA (o un humano) retome el trabajo sin re-descubrir el contexto.

**Relacionado:** [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) § road bits,
[SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) § orientación isométrica, [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md)
T3 autoslope, [SP2_CHECKLIST.md](SP2_CHECKLIST.md) § drag carretera.

---

## 1. Síntoma reportado por el usuario

Capturas y GIF (sesión 2026-06-22):

1. Al **arrastrar** o **clic** para extender una carretera existente (eje NE–SW en pantalla,
   `0x0A` en mapa), aparecen teselas **perpendiculares** (NW–SE, `0x05`), **desconectadas**
   o en “escalera” (misma fila de teselas pero sprite girado 90°).
2. El **fantasma** a veces coincidía con lo colocado, pero el usuario quiere una **línea
   continua** alineada con la red, no una tesela aislada mal orientada.
3. Tras varios fixes dijo «sigue igual» y luego «empeoró» (inferencia de eje anulaba la
   herramienta `RoadX`). Último mensaje: **dejarlo así** y documentar.

---

## 2. Modelo OpenTTD (referencia obligatoria)

### Road bits (`m5` nibble bajo, tesela `MP_ROAD` normal)

| Valor | Nombre upstream | Eje en grilla | Apariencia isométrica típica |
|-------|-----------------|---------------|------------------------------|
| `0x0A` | `ROAD_X` (SW\|NE) | Misma **Y**, varía **X** | Diagonal **NE–SW** |
| `0x05` | `ROAD_Y` (NW\|SE) | Misma **X**, varía **Y** | Diagonal **NW–SE** |
| `0x0F` | Cruce | Ambos ejes | Cruz |

Constantes en cliente: `BuildMenuAction::RoadX` → `0x0A`, `RoadY` → `0x05`, `Road` → genérica.

### Herramientas en toolbar (`layout/sections.rs`)

| Botón UI | Acción | Icono PNG |
|----------|--------|-----------|
| Carretera NW–SE | `RoadY` | `road_flat_00.png` |
| Carretera NE–SW | `RoadX` | `road_flat_01.png` |
| Cruce de carretera | `Road` | `road_flat_02.png` |

**Confusión habitual:** en isométrico, el usuario elige el botón que “se ve” como la carretera
en pantalla; si la red existente es `RoadX` y tiene activo `RoadY`, coloca perpendicular.

OpenTTD **no** reorienta automáticamente `RoadY` hacia `RoadX` al arrastrar paralelo (la tool
bloquea eje). La mejora deseada en openttdrs sería UX extra, no paridad estricta.

### Render en pendiente

`GetRoadSpriteOffset` en OpenTTD (`road_cmd.cpp`): en `SLOPE_NE/SE/SW/NW` **ignora** `road_bits`
y usa sprites 11–14. Mismo comportamiento en `road_flat_sprite_index` (cliente). En colina,
una recta plana puede **verse** “girada” aunque `m5` sea correcto → hace falta **autoslope**
([ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md) T3) o cimientos.

---

## 3. Flujo actual en openttdrs

```
Toolbar → click.rs (drag arm / release)
       → drag_line_tiles(map, action, from, to)   // teselas de la línea
       → apply_drag_action → Command::PlaceRoadBits(c, axis | ROAD_PLACE_FORCE_AXIS)
       → transport::place_road_bits → merge_road_bits_with_neighbors → m5
       → render: road_bits_for_render + road_flat_sprite_index(tileh, bits)
```

Preview: `preview/mod.rs` → `road_preview_at` (sprite según `tileh` + bits inferidos).

### Flag `ROAD_PLACE_FORCE_AXIS` (`0x10`)

Bit alto en el parámetro `bits` de `PlaceRoadBits` (no se guarda en `m5`). Indica arrastre en
línea: `merge` debe usar `connect | requested` y **no** girar 90° por un vecino cardinal suelto.

---

## 4. Cambios aplicados (sin commit pedido; junio 2026)

Archivos principales:

| Archivo | Qué se tocó |
|---------|-------------|
| `crates/openttdrs-core/src/command/transport.rs` | `merge_road_bits_with_neighbors`, `propagate_road_bits_to_neighbors`, `ROAD_PLACE_FORCE_AXIS`, `road_bits_for_autoroute`, `preview_road_bits_at`, `infer_road_drag_axis`, `road_locked_tool_axis`, `road_drag_line_tiles`, `finalize_road_drag_line`, `road_axis_from_colinear_neighbor` |
| `crates/openttdrs-client/.../build_input/drag.rs` | Arrastre con eje; `road_drag_axis()`; línea solo con inferencia para `Road` genérica |
| `crates/openttdrs-client/.../build_input/commands.rs` | Clic suelto `Road` usa `road_bits_for_autoroute` (no `0x0F` aislado) |
| `crates/openttdrs-client/.../preview/mod.rs` | Fantasma con PNG según pendiente + bits efectivos |

Tests añadidos/actualizados: `command/tests.rs`, `drag.rs` (cliente), `ui_command_integration.rs`.

CI: `bash scripts/check.sh ci` pasaba tras el último cambio.

---

## 5. Comportamiento **actual** (post-fixes)

| Herramienta | Línea de arrastre | Eje de bits |
|-------------|------------------|-------------|
| **RoadX** | Siempre fila constante **Y** (como OpenTTD) | `road_locked_tool_axis` → casi siempre `0x0A`; rama si arrancas **sobre** tesela recta y arrastras perpendicular |
| **RoadY** | Siempre columna constante **X** | Igual con `0x05` |
| **Road** (genérica) | `road_drag_line_tiles` + `infer_road_drag_axis` (vecinos, colinear ±1 tesela, ratón) | Inferido |

**Clic suelto** (sin arrastre): una tesela vía `apply_drag_action` con `pending_tiles.len()==1`.

**Merge sin arrastre** (`PlaceRoadBits` sin `FORCE`): vecino cardinal E/O fuerza horizontal;
vecino N/S fuerza vertical (test `place_road_bits_extends_horizontal_when_neighbor_west`).

---

## 6. Problemas **no resueltos** / hipótesis para la próxima IA

### P1 — UX tool vs geometría isométrica (muy probable)

El usuario construye una línea NE–SW (`RoadX`) pero tiene seleccionado el **primer** botón
(`RoadY`) o arrastra con ratón en dirección que no coincide con el eje bloqueado de la tool.

**Prueba manual:** con **segundo** botón (NE–SW) activo, arrastrar **desde** el extremo de la
carretera en la misma fila Y.

**Posible mejora:** resaltar en UI qué eje está activo; al acercar el cursor a una red
existente, **sugerir** tool (`RoadX`/`RoadY`) en el fantasma o con mutación temporal de eje
solo para tool genérica.

### P2 — Inferencia demasiado agresiva (regresión «empeoró»)

Versión intermedia aplicaba `infer_road_drag_axis` también a `RoadX`/`RoadY`, cambiando eje
y sprites a 90°. **Revertido:** tools bloqueadas usan `road_locked_tool_axis`.

No reintroducir colinear/cardinal override en tools bloqueadas sin tests de regresión.

### P3 — Pendientes sin autoslope

Colocar en tesela inclinada: `m5` puede ser correcto pero sprite 11–14 no coincide con
recta plana vecina. Requiere T3 terraform o `CheckRoadSlope` + foundation como OpenTTD.

### P4 — Fantasma sin textura en algunos casos

Informe usuario: a veces solo cuadrado verde (`tile_select` fallback no — carretera usa
`road_flat_XX`). Verificar que `road_preview_at` devuelve `Some` para todas las tools road y
que el asset existe.

### P5 — Arrastre que no coloca (micro-movimiento)

Si entre press/release el ratón mueve 2 teselas, `just_released` con `len>1` **no** coloca
hasta un segundo click (`click.rs`). Puede parecer “clic no hace nada” o línea inesperada.

### P6 — Huecos en la línea

Si una tesela del arrastre falla (`CannotPlaceRoadOnWater`, industria, etc.), se omiten
teselas intermedias → segmentos desconectados con orientación correcta pero gap.

### P7 — Paridad OpenTTD `CmdBuildRoad` drag

Revisar `OpenTTD/src/road_cmd.cpp` + GUI autoroute para: snap a red, coste por pieza,
`CheckRoadSlope`, actualización de vecinos tras línea completa. openttdrs coloca tesela a
tesela sin pasada global de normalización (salvo `finalize_road_drag_line`).

---

## 7. Funciones clave (punto de entrada)

```text
Core
  place_road_bits(state, c, bits)           — bits & 0x10 → force axis
  merge_road_bits_with_neighbors(...)       — lógica de eje + cruces
  propagate_road_bits_to_neighbors(...)     — enlace recíproco vecinos
  infer_road_drag_axis(map, start, end, tool_axis)  — SOLO tool genérica
  road_locked_tool_axis(map, start, end, tool_axis) — RoadX / RoadY
  road_drag_line_tiles(map, from, to, tool_axis)    — SOLO invocada para Road genérica
  road_bits_for_autoroute(map, c)           — clic suelto genérico
  preview_road_bits_at(map, c, requested, force_axis)

Cliente
  drag_line_tiles(map, action, from, to)     — RoadX/Y: eje fijo; Road: inferida
  apply_drag_action(...)                    — bucle PlaceRoadBits + finalize_road_drag_line
  road_preview_at(...)                      — ghost PNG
  command_for_action(..., map)              — Road → autoroute
```

Constantes: `ROAD_PLACE_FORCE_AXIS = 0x10` exportada en `openttdrs_core`.

Tabla sprites planos: `ROAD_FLAT_OFFSET_TBL` en `sprites/road.rs` (golden tests bits 1–15).

---

## 8. Tests útiles para no romper

```bash
# Core
cargo test -p openttdrs-core place_road_bits
cargo test -p openttdrs-core infer_road_drag_axis
cargo test -p openttdrs-core road_locked

# Cliente drag
cargo test -p openttdrs-client drag_road
cargo test -p openttdrs-client road_x_drag_keeps
cargo test -p openttdrs-client generic_road_drag

# CI completo
bash scripts/check.sh ci
```

Casos que **deben** seguir pasando:

- `drag_road_merge_bits_at_perpendicular_intersection` — cruce X luego Y → `0x0F`.
- `road_x_drag_keeps_horizontal_axis_near_vertical_road` — RoadX no gira por vía vertical cercana.
- `place_road_bits_links_perpendicular_neighbor` — T correcto en vecino horizontal.

---

## 9. Enfoques recomendados (orden sugerido)

1. **Reproducir con save concreto** del usuario (`save/partida_2026-06-22_0942.json` citado en
   capturas): log `m5` tras colocar vs tool activa y teselas de `pending_tiles`.
2. **Decidir política de producto:** ¿paridad estricta OpenTTD (tool bloquea eje) o autoroute
   amigable (tercer botón / snap)?
3. **Tool genérica como default** al abrir panel carretera (hoy atajos pueden dejar `RoadY`).
4. **Autoslope T3** si el bug es solo visual en pendiente.
5. **Pasada post-drag** estilo `normalize_rail_trackbits_from_neighbors` pero para carreteras
   en la polilínea colocada (re-merge todos los tiles de la línea + vecinos).
6. **Comparar** con `OpenTTD/src/road_cmd.cpp` `CmdBuildRoad` y drag en `road_gui.cpp`.

---

## 10. Commits

El usuario **no pidió commit** de estos cambios. Verificar `git status` antes de asumir qué
está en el árbol de trabajo.

---

## 11. Historial breve de la conversación

| Intento | Idea | Resultado |
|---------|------|-----------|
| 1 | `PlaceRoadBits` en drag OR `propagate_road_bits_to_neighbors` | Mejor en cruces; usuario: «sigue igual» |
| 2 | `merge_road_bits_with_neighbors` alinea eje cardinal | Mejor al continuar desde vecino |
| 3 | `ROAD_PLACE_FORCE_AXIS` + primera tesela arrastre | Mejor en línea aislada |
| 4 | `road_bits_for_autoroute` (no cruce 0x0F en hierba) | Clic genérico más sensato |
| 5 | Fantasma con `road_flat_sprite_index(tileh, bits)` | Preview alineado con render |
| 6 | `infer_road_drag_axis` en **todas** las tools | **Empeoró** — sprites 90° con RoadX |
| 7 | Split: `road_locked_tool_axis` (X/Y) vs `infer_*` (solo Road) + merge force antes de vecino | Estado actual; usuario: dejarlo |

---

*Fin del handoff.*
