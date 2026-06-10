# Roadmap: paridad visual con OpenTTD oficial

Comparación del mismo sector del save `openttd_regression_test.sav` (zona de
Nuntburg, costa NW) entre nuestro cliente y OpenTTD 15.3 oficial.

| Referencia | Captura |
|---|---|
| openttdrs | `docs/img/paridad_openttdrs.png` |
| OpenTTD 15.3 | `docs/img/paridad_openttd_oficial.png` |

Diferencias detectadas, ordenadas por impacto visual. Marcar al completar.

## 1. Puente de carretera sobre el agua — falta el tablero

- [x] Hecho (`render/tiles/bridge.rs`)

En el oficial se ve el puente de madera completo cruzando el agua al oeste de
Nuntburg (rampas + tramos intermedios elevados sobre pilotes). En el nuestro
las rampas ya se clasifican bien (`RoadBridge`, fix de TransportType) pero
**no se dibujaba el tablero**: las teselas intermedias son `MP_WATER` con
`IsBridgeAbove` en bits 2–3 de `mapt` (eje X/Y). Ahora `spawn_bridge_middle`
detecta el vano, busca la rampa por el eje para obtener altura (`z rampa + 1`,
como `GetBridgeHeight`) y tipo (road/rail), y dibuja tablero + barandilla
frontal + pilares con los sprites `bridge_wood_*`.

## 2. Árboles — densidad, especies y colores

- [x] Hecho (`gen_tree_draw_data.py` + `push_forest_tree`)

Oficial: vegetación densa con muchas especies y colores (verdes, naranjas,
rojos otoñales), varios árboles por tesela y en acantilados de la costa.
Nuestro dibujaba 1 conífera (sprite de retoño) por tesela. Ahora:
sprites completos (19 especies × 7 etapas, `tree_00..132.png` extraídos por
`scripts/gen_tree_draw_data.py`), y `push_forest_tree` replica
`DrawTile_Trees`: 1–4 árboles según bits 6–7 de m5, posiciones de
`_tree_layout_xy`, especies por árbol de `_tree_layout_sprite` (tipo m3 ×
variante `CountBits`), etapa de crecimiento (bits 0–2 de m5) en el último
árbol. El suelo de `MP_TREES` pasa de rough teñido a hierba normal.

## 3. Campos de cultivo — sin sprites de surcos ni cercas

- [x] Hecho (`gen_field_draw_data.py` + `spawn_field_fences`)

Oficial: parcelas aradas en distintos tonos de marrón con surcos en ambas
diagonales y cercas en los bordes. Nuestro dibujaba rough teñido genérico.
Ahora: sprites completos de farmland (9 estados × 19 pendientes,
`field_{estado}_{off}.png`, sprites 4126..4296) y de cercas (6 tipos × 6
variantes, `fence_{tipo}_{var}.png`, sprites 4090..4125) extraídos por
`scripts/gen_field_draw_data.py`. `spawn_generic_land_tile` elige el suelo
por `GetFieldType` (bits 0–3 de m3) + pendiente, y `spawn_field_fences`
replica `DrawClearLandFence`: tipo de cerca por lado (SE/SW en m4, NE en m3,
NW en m6, 3 bits c/u), variante por pendiente (`_fence_mod_by_tileh_*`) y
posición en el borde correspondiente con la z de la esquina.

## 4. Nombres de ciudades sobre el viewport

- [x] Hecho (`render/town_labels.rs`)

Oficial muestra la etiqueta "Nuntburg (738)" flotando sobre la ciudad (nombre
+ población, con fondo translúcido). Ahora `spawn_town_labels` crea un cartel
`Text2d` («Nombre (población)», blanco sobre fondo oscuro translúcido) por
cada `Town` del estado, anclado sobre la tesela central (`pos` del save) con
su altura de terreno, en una capa z por encima de todos los sprites del mapa.
Se regenera junto al resto de `MapVisualLayer` en cada remap/carga de save.

## 5. Agua — color/textura y animación

- [x] Hecho (`gen_water_anim_frames.py` + `render/water.rs`)

Oficial: azul más oscuro y uniforme, con animación de oleaje por paleta.
Nuestro tenía un tinte por tesela (rombos marcados, azul más claro) y los
píxeles glitter horneados estáticos (puntos blancos). Ahora se replica
`DoPaletteAnimations` (`palette.cpp`): los colores base de los índices
245–249 (dark water) y 250–254 (glitter water) se invierten por RGB y
`scripts/gen_water_anim_frames.py` hornea los 15 frames del ciclo
(`water_anim_*` y `shore_{i}_anim_*`). `animate_water` intercambia la imagen
global cada ~150 ms (sin tinte, mismo frame en todas las teselas, como la
paleta del original); las "estrellitas" ahora destellan y se apagan igual
que el oleaje oficial.

## 6. Casas — variedad de sprites según HouseID

- [x] Hecho (`TileHash2Bit` oficial en `sprites.rs`)

La cadena de datos ya estaba completa: `HOUSE_DRAW_DATA` cubre las 110 casas
originales × 16 filas (4 variantes × 4 etapas de obra, de
`_town_draw_tile_data`), los 410 sprites están extraídos y el save aporta 16
HouseIDs distintos en m8 (iglesia, comercios, hotel 2×2, etc.) con la etapa
en m3/m5. El hueco de paridad real era la **variante por tesela**: usábamos
un hash inventado (`tx·5787 + ty·3781`) en vez del `TileHash2Bit` de OpenTTD,
así que cada tesela elegía una fila distinta a la del oficial. Ahora
`tile_hash_2bit` replica `TileHash(16·tx, 16·ty) & 3` (`tile_map.h`) y la
casa dibujada coincide 1:1 con el cliente oficial.

## 7. Industrias — sprites por tipo y humo animado

- [x] Hecho (`gen_chimney_smoke.py` + `render/smoke.rs`)

Auditoría previa: los 59 gfx de industria presentes en el save (795 teselas)
ya resolvían a filas calibradas de `INDUSTRY_GFX_DATA` con sus PNGs en disco
— el mapeo por tile quedó completo en iteraciones anteriores. Lo que faltaba
era el **humo de la chimenea** de la central: OpenTTD crea un `EffectVehicle`
`EV_CHIMNEY_SMOKE` en la tesela `GFX_POWERPLANT_CHIMNEY` (gfx 8) anclado en
`(x+15, y+14, z+59)` que cicla `SPR_CHIMNEY_SMOKE_0..7` cada 8 ticks. Ahora
`scripts/gen_chimney_smoke.py` extrae los 8 frames (sprites 3701–3708) y
`spawn_chimney_smoke` + `IndustrySmokePlugin` replican ese penacho (fase
inicial pseudoaleatoria por tesela, ~0.22 s por frame, reposicionado por los
NFO offsets de cada frame) solo en chimeneas de industrias terminadas.

## 8. Costa — verificación fina contra el original

- [x] Completado

Se reemplazaron las 8 orillas legacy (4062..4069) por el **set completo de 18
sprites** de `SPR_SHORE_BASE`, extraído del GRF *extra* de OpenGFX
(`ogfx2e_extra_32ez.grf`, Action5 tipo 0x0D, clima templado) con el nuevo
`scripts/gen_shore_full_set.py`:

- Bloque de 16 sprites (A5BLOCK_FIXED) → slots 0..15; bloque de 10 sprites
  ("missing shore sprites", `newgrf_act5.cpp`) → slots 16/17 (WE/NS).
- `shore_png_index` usa la tabla oficial `tileh_to_shoresprite`
  (`water_cmd.cpp`), portada en `shore_draw_data_generated.rs`; el ancla se
  deriva de los offsets NFO (`h/2 + yrel`, coincide con `SLOPE_HALF_H`).
- `shore_tileh_for_draw_shore` ya no cae a inferencia en pendientes WE/NS ni
  de 3 esquinas: en el save real hay ~660 teselas de costa con `tileh`
  7/11/13/14 que ahora usan su sprite correcto.
- Animación: `gen_water_anim_frames.py` genera los 15 frames de paleta para
  las 18 orillas (`shore_full_{i:02}_anim_{f:02}.png`).

## 9. Carreteras — textura y bordes

- [ ] Pendiente

Diferencia menor: el pavimento oficial tiene otro tono y marcas claras de
borde/acera en town roads. Revisar sprites de carretera OpenGFX (con y sin
aceras según está dentro de ciudad).
