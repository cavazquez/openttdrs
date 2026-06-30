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

- [x] Completado

Se implementó `Roadside` (`m6` bits 3–5, `road_map.h`) para carreteras
normales, replicando `GetRoadGroundSprite` (`road_cmd.cpp`):

- **Acera pavimentada**: Paved (2), StreetLights (3), Trees (5) y
  PavedRoadWorks (7) usan el set `SPR_ROAD_Y − 19` (sprites 1313..1331,
  `road_paved_{i:02}.png`), con el mismo orden/offsets que `road_flat`.
  Barren/Grass/GrassRoadWorks mantienen el set sobre pasto; nieve/desierto
  conserva el tinte actual.
- **Faroles**: `Roadside::StreetLights` dibuja los faroles de
  `_roadside_lamps` (`table/road_land.h`, sprites 0x57E/0x57F) en sus
  subcoordenadas de mundo, solo con 2+ road bits como upstream.
- En el save real: 576 tiles Grass, 139 Paved y 31 StreetLights.

## 10. Población de ciudades en las etiquetas

- [x] Completado

Las etiquetas mostraban «Nuntburg (0)» en vez de «Nuntburg (737)». Dos causas:

- **OpenTTD no guarda la población en el save**: la reconstruye al cargar
  (`RebuildTownCaches`, `town_sl.cpp`) sumando `HouseSpec::population` de
  cada tesela `MP_HOUSE` completada (bit 7 de `m3`). Se replicó en
  `sav::rebuild_town_populations` con la tabla `HOUSE_POPULATION`
  (generada por `gen_house_population.py` desde `table/town_land.h`).
- **Endianness de `MAP2`/`MAP8`**: ambos chunks son `SLE_UINT16`
  big-endian en el save, pero `build.rs` los dividía/copiaba como
  little-endian. El TownID (`m2`) quedaba con los bytes cruzados (todas
  las casas parecían de la town 0, y los tipos de señal ferroviaria leían
  el byte equivocado). Corregido en `export_ottdmap`/`build_m8_le`.

Verificado contra el save real: las 28 ciudades coinciden con el oficial
(p. ej. Nuntburg 737, Planfield 787).

## 11. Puente de madera — altura del tablero, offsets y pilares

- [x] Completado (pendiente verificación visual in-game)

Réplica de `DrawBridgeMiddle` (`tunnelbridge_cmd.cpp`):

- **Altura del tablero**: `bridge_deck_z` porta `GetBridgeHeight`
  (`bridge_map.cpp`) con la fundación del cabezal (`GetBridgeFoundation` +
  `ApplyFoundationToSlope`): rampa plana/inclinada según el eje → `z+1`;
  una esquina elevada → fundación inclinada (`z+1`); resto → fundación
  niveladora (`z+2`). Antes se usaba `min_z+1` a secas y el tablero quedaba
  un nivel bajo cuando la rampa apoya en pendiente (p. ej. costa).
- **Offsets NFO**: `gen_bridge_draw_data.py` genera
  `bridge_draw_data_generated.rs` (rear/front/pillar por eje, sprites
  2545–2552) y el render ancla cada sprite con su xrel/yrel a
  `z = tablero − BRIDGE_Z_START (3 px)`, eliminando el centrado con
  `TILE_OVERLAP_SCALE` que producía el efecto «empalizada».
- **Front y pilares como upstream**: barandilla frontal a +12 unidades de
  mundo perpendiculares; columna frontal de pilares cada `TILE_HEIGHT` px
  hasta el suelo (máximo de las esquinas del borde, ~`GetSlopePixelZOnEdge`)
  y columna trasera a −9 unidades saltando los dos tramos tapados.
- El arte (madera OpenGFX vs TTD original) difiere por baseset: no es bug.

## 12. Costa — cabo «desprendido» al oeste de Nuntburg

- [x] Hecho (pendiente verificación visual)

En el nuestro había un triángulo de césped con arena flotando en el agua,
separado de la costa por una franja de agua; en el oficial esa tesela es
agua plana.

Causa: `use_shore` (`render/grid.rs`) dibujaba orilla en **cualquier**
agua lisa (`WATER_TILE_CLEAR`) que tocara tierra en el vecindario 8.
Para teselas que solo tocan tierra en diagonal (p. ej. (47,23) junto a
Nuntburg), `infer_coast_tileh_when_flat` devolvía una pendiente de una
esquina y aparecía el triángulo flotante. OpenTTD solo ejecuta
`DrawShoreTile` en teselas marcadas `WATER_TILE_COAST` en `m5`
(`water_cmd.cpp`), y el save trae esa marca (m5 = 0x1X).

Arreglo: la heurística de vecinos queda restringida a mapas generados
sin `MAPT` (demo); con save real manda `m5`. Esto también elimina la
orilla espuria en ~70 teselas de agua lisa ortogonales a tierra que
upstream dibuja como agua plana. Test de regresión:
`sav_plain_water_near_land_does_not_use_shore`.

## 13. Estación de tren — techos y plataformas (gfx 4–7)

- [x] Hecho (pendiente verificación visual)

En el save de Grinnway las teselas de estación usan `StationGfx` 0, 4 y 6;
nosotros solo implementábamos 0–3, así que las variantes con techo (4–7,
`_station_display_datas_rail` en `station_land.h`) caían al fallback de
plataformas planas: faltaban los techos rojos y los muros con arcos que
se ven en el oficial. Además `rail_station_overlay_rel` escalaba los
offsets `TILE_SEQ` al doble (4 px por unidad en vez de 2) y no aplicaba
los offsets NFO de cada sprite, separando las plataformas rear/front.

Implementación:
- `scripts/gen_rail_station_draw_data.py` genera metadata NFO
  (`w/h/xrel/yrel`) de los sprites 1069–1082 en
  `sprites/rail_station_draw_data_generated.rs`.
- `station.rs`: secuencias completas gfx 0–7 con origen `TILE_SEQ`
  (`dx/dy/dz`, techos con `dz = 16`) + `rail_station_overlay_rel` que
  remapea a `remap_tile_offset × 0.5` y suma offsets NFO. Los childsprites
  de vidrio del techo (1083–1086, `PALETTE_TO_TRANSPARENT`) se omiten.
- `objects.rs`: dibuja cada capa con `overlay_pos` y metadata NFO, sin
  `TILE_OVERLAP_SCALE`.
- Aliases `rail_1075..1082.png` (pilares y techos) en
  `descargar_graficos.sh` / `alias_rail_station_sprites.sh`.

Diferencias restantes detectadas en la misma zona (no bugs nuestros):
- Suelo marrón bajo los cruces de vía: OpenGFX dibuja los sprites de
  suelo de cruce (1018–1022) mucho más áridos que el TTD original;
  la lógica (1018 + offset de `GetJunctionGroundSpriteOffset`) coincide
  con upstream.
- Textura de campos/cercas y tono de árboles: arte del baseset.

## 14. Toolbar de construcción ferroviaria — paridad con upstream

- [x] Hecho (pendiente verificación visual)

El panel tenía 7 botones con sprites de teselas como iconos; el oficial
(`_nested_build_rail_widgets`, `rail_gui.cpp`) tiene 14 con iconos GUI
propios: NS/NE-SW/EO/NW-SE, autorail | dinamita, depósito, waypoint,
estación, señales, puente, túnel, quitar y convertir.

Implementación:
- `scripts/gen_toolbar_rail_icons.py` extrae los iconos de OpenGFX a
  `assets/opengfx/tiles/toolbar_rail_*.png` (lienzo 63×51, fondo azul →
  alfa, escala ×2 sin deformar). Los del set base (703, 714, 1251–1254,
  1291, 1294, 1298, 2430, 2594) salen del NFO base; autorail (+53),
  convertir (+55) y waypoint (+76, `SPR_OPENTTD_BASE + n`) se mapean
  desde los bloques Action 5 tipo `95` del GRF extra de OpenGFX2.
- Acciones nuevas: `RailX` (0x01) y `RailY` (0x02) en `PlaceRailBits` con
  soporte de arrastre; el botón autorail usa el `Rail` autodireccional.
- Waypoint, señales, quitar y convertir se muestran (mismo orden e icono
  que upstream) pero aún sin comando en el simulador: tooltip
  «(no implementado)».
- Panel con separador entre grupos y título «Construccion de Ferrocarril».

## 14. Locomotoras — un sprite distinto por grupo

- [x] Hecho (`extract_train_vehicle_sprites.py` + `gen_vehicle_gfx_data.py`, jun 2026)

En OpenTTD cada familia de locomotora tiene su aspecto (vapor pequeño, A4, diésel,
eléctrico de alta velocidad, etc.). En openttdrs la **simulación y la ventana de
compra** ya distinguen cinco grupos vía `train_image_index` → `train_sprite_group`,
pero **en mapa todos se dibujan como Kirby Paul Tank** mientras falten los PNG
de los otros grupos: `gen_vehicle_gfx_data.py` cae al fallback Kirby si no
encuentra `vehicle_train_t0_*.png`, `vehicle_train_t1_*.png`, etc.

**Pasos para cerrar el gap (costo S):**

1. `./scripts/descargar_graficos.sh` — recorta sprites 2905–2972 a
   `assets/opengfx/tiles/`.
2. `python3 scripts/gen_vehicle_gfx_data.py` — regenera
   `vehicle_gfx_data_generated.rs` con paths únicos por grupo.
3. Comparar lado a lado compra + mapa: Chaney (T0), Ginzu (T1), Kirby, diésel,
   AsiaStar (eléctrico).

Detalle de IDs, tablas y archivos: [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md)
§ «Sprites de locomotoras».
