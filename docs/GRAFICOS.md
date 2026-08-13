# Gráficos y render

Extracción OpenGFX, isometría, locomotoras y handoff de bugs visuales de terreno.

## Índice

- [Sprites OpenGFX](#sprites-opengfx)
- [Bugs terreno](#bugs-visuales-de-terreno-handoff)

---

## Sprites OpenGFX

<!-- fuente: SPRITES_OPENGFX.md -->

Este documento registra todo lo aprendido sobre la extracción y uso de sprites de
[OpenGFX](https://github.com/OpenTTD/OpenGFX) en el renderer isométrico de openttdrs.

**Anexos (catálogos históricos, no mantener en paralelo):**

- [archive/SPRITES_OPENGFX_COMPLETO.md](archive/SPRITES_OPENGFX_COMPLETO.md) — IDs OpenGFX / `sprites.h`
- [archive/INDUSTRIAS_OPENGFX.md](archive/INDUSTRIAS_OPENGFX.md) — gfx industria → sprite (abr 2026; preferir código + [ROADMAP_INDUSTRIAS_PARIDAD.md](PLANIFICACION.md#industrias-paridad))

---

### Estructura del paquete OpenGFX

Descargado con `scripts/descargar_graficos.sh` (versión 8.0 por defecto) en
`assets/opengfx/` (carpeta ignorada por git).

```
assets/opengfx/opengfx-8.0/
├── ogfx1_base.grf          ← Sprite sheet principal (clima templado)
├── ogfxc_arctic.grf        ← Clima ártico
├── ogfxh_tropical.grf      ← Clima tropical
├── ogfxt_toyland.grf       ← Clima toyland
├── ogfxe_extra.grf         ← Sprites extra
├── ogfxi_logos.grf         ← Logos
├── sprites/
│   ├── ogfx1_base.nfo      ← Índice de sprites (coordenadas, offsets)
│   ├── ogfx1_base00.png    ← Hoja de sprites 8bpp (palette PNG, 800×15968)
│   └── ogfx1_base00.32.png ← Hoja de sprites 32bpp (no legible por PIL directamente)
└── tiles/                  ← Sprites individuales extraídos (generados por el script)
```

Para generar la carpeta `sprites/` hay que decodificar el GRF con
[grfcodec](https://github.com/OpenTTD/grfcodec):

```bash
sudo apt install grfcodec
grfcodec -d -p 1 assets/opengfx/opengfx-8.0/ogfx1_base.grf \
         -o assets/opengfx/opengfx-8.0/sprites/
```

#### Side-cache 8bpp en modo OpenGFX2 (`--32bpp`)

Con `./scripts/descargar_graficos.sh --32bpp` el renderer usa OpenGFX2 High Def
(`opengfx2-32ez`). Ese set **aún no** expone el bloque Action5 tipo 05 (elrail)
con iconos GUI de vía eléctrica ni la catenaria indexada como en OpenGFX clásico.

Por eso el pipeline, en `--32bpp`, descarga además OpenGFX 8.x (caché en
`.downloads/openttd/`) y deja un mínimo decodificado en:

```
assets/opengfx/.signal-src-8bpp/
├── ogfxe_extra.grf
├── ogfx1_base.grf
└── sprites/
    ├── ogfxe_extra.nfo + ogfxe_extra*.png
    └── ogfx1_base.nfo (+ hojas; solo se usan pseudo-sprites de recolor)
```

Consumidores actuales:

| Script | Qué saca del side-cache |
|--------|-------------------------|
| `gen_toolbar_rail_icons.py` | `toolbar_rail_electric_{rail_*,tunnel}.png` (slots A5 36..39, 44) |
| `extract_elrail_catenary.py` | wires / postes / entradas de túnel |
| `gen_bridge_structure_palette.py` | tablas `PALETTE_TO_STRUCT_*` (pseudo-sprites 795–801 de `ogfx1_base`) |
| `extract_rail_pbs_palette_sprites.py` | overlays de reserva PBS ya remapeados con la pseudo-sprite `PALETTE_CRASH=804` |

Ese directorio **no** se borra en la limpieza de `opengfx-*` / `opengfx2-*` de cada
corrida; se reutiliza si el NFO ya está.

#### Paleta DOS 8bpp y reservas PBS

El baseset clásico declara `palette = DOS`. Las hojas que genera `grfcodec`
contienen índices DOS aunque el PNG pueda traer una paleta de trabajo distinta;
por eso los extractores 8bpp deben usar `scripts/opengfx_palette.py` y no
`Pillow.convert("RGBA")` directamente.

En particular, OpenTTD dibuja una reserva PBS con `PALETTE_CRASH = 804`, una
pseudo-sprite de recolor por **índice**, no con un tinte naranja. El pipeline
genera `rail_pbs_<id>.png` mediante
`scripts/extract_rail_pbs_palette_sprites.py` antes del atlas. Así se preservan
los casos donde dos píxeles comparten RGB pero la tabla 804 les asigna salidas
distintas. Si esos archivos no están en el atlas, el HUD/traza marca fallback
en vez de declarar una coincidencia visual falsa.

**Cuando implementen el equivalente 32bpp nativo** (Action5 elrail usable en
`ogfx2e_extra_32ez` o sucesor):

1. Hacer que `gen_toolbar_rail_icons.py` y `extract_elrail_catenary.py` lean el
   GRF extra 32bpp (buscar `TODO(32bpp-nativo)` en esos scripts).
2. Quitar `ensure_signal_src_8bpp` de `descargar_graficos.sh` y dejar de
   descargar OpenGFX 8.x en modo `--32bpp`.
3. Borrar esta subsección y el directorio `.signal-src-8bpp/`.

---

### Formato del NFO

El archivo `ogfx1_base.nfo` tiene una línea por sprite:

```
<sprite_id>  sprites/<archivo>.png  <profundidad>  <x>  <y>  <w>  <h>  <xrel>  <yrel>  <flags>
```

| Campo | Descripción |
|-------|-------------|
| `sprite_id` | ID numérico del sprite (referenciado en el código de OpenTTD) |
| `x`, `y` | Posición del recorte en la hoja de sprites (píxeles desde arriba-izquierda) |
| `w`, `h` | Ancho y alto del sprite en píxeles |
| `xrel`, `yrel` | Offsets en pantalla **respecto al punto de referencia** (coordenadas Y-down) |

#### Espacios de IDs: base vs. extra

El número de sprite dentro de un NFO es **local a su GRF**, no un identificador
global entre `ogfx1_base.grf` y `ogfxe_extra.grf`. Las constantes `SPR_*` de
OpenTTD se resuelven contra el baseset (`ogfx1_base.nfo` en 8bpp o
`ogfx21_base_32ez.nfo` en 32bpp). Nunca hay que combinar ambos NFO en un único
diccionario indexado por número: por ejemplo, el ID global de campo `4259` es
un rombo de `64×31` en el base, mientras que el `4259` local del extra es un
sprite distinto de `12×6`.

`scripts/nfo_sprite_meta.py` y el `Cropper` compartido aplican esa regla. Los
sprites que realmente pertenecen a `ogfxe_extra` se extraen sólo desde un
bloque Action5 identificado explícitamente (catenaria, señales/PBS, iconos o
paradas drive-through), sin reutilizar sus IDs locales como `SPR_*`.

#### Conversión xrel/yrel → posición Bevy (Y-up)

El punto de referencia de OpenTTD es el **vértice superior del rombo** de la tesela.
Para calcular el centro del sprite en Bevy:

```
center_x = ref_x + xrel + w / 2
center_y = ref_y - yrel - h / 2      ← invertir Y (Y-down → Y-up)
```

Donde `(ref_x, ref_y)` es la salida de `iso(tx, ty)` en el código Rust.

---

### Transparencia en los sprites 8bpp

La hoja `ogfx1_base00.png` es un PNG con paleta de 256 colores. OpenGFX clásico
declara `palette = DOS` en `opengfx.obg`, por lo que debe decodificarse con
`grfcodec -p 1` (no `-p 2`, que sólo cambia el RGB embebido a paleta Windows).
El índice 0 (azul puro `RGB(0, 0, 255)` en la hoja) es transparente.

No hay que inferir los demás colores por RGB: el índice 1..9 es metal/asfalto
y 215..226 es padding transparente. `scripts/opengfx_palette.py` lee la
paleta DOS canónica desde `third_party/openttd/table/palettes.h` y realiza la
conversión por índice; los extractores deben usarlo antes de recortar sprites.

Para una herramienta puntual que reciba una imagen ya RGBA, la limpieza de
colorkey sigue siendo un fallback aceptable. No debe usarse sobre la hoja
indexada original:

```python
img = Image.open("ogfx1_base00.png")
pal = img.getpalette()
transparent_rgb = tuple(pal[0:3])   # → (0, 0, 255)
img_rgba = img.convert("RGBA")
data = list(img_rgba.getdata())
data = [(0,0,0,0) if (r,g,b)==transparent_rgb else (r,g,b,a)
        for r,g,b,a in data]
img_rgba.putdata(data)
```

#### Recolor de compañía en 8bpp

Los sprites vanilla que OpenTTD dibuja con `PALETTE_MODIFIER_COLOUR` vienen
autorizados por sus índices DOS, no porque cualquier RGB coincida con un tono
de una rampa. Después de convertir a RGBA, el cliente sólo reconoce la rampa
autora `COLOUR_DARK_BLUE` (`0xC6..=0xCD`) y la transforma al color de la
compañía. Es importante no buscar coincidencias en las 16 rampas: tonos de
techos, acero y asfalto comparten esos RGB y terminarían recoloreados por
accidente.

`scripts/gen_company_palette_rust.py` genera desde la paleta DOS canónica las
dos tablas que deben mantenerse idénticas:

```bash
python3 scripts/gen_company_palette_rust.py
python3 scripts/gen_company_palette_rust.py --check
```

La segunda tabla (`openttdrs-core/src/newgrf_company_ramp.rs`) se usa al
hornear máscaras explícitas de sprites NewGRF. En ese camino la máscara, no el
RGB resultante, es la autorización para recolorear.

---

### Sprites de tesela de suelo

Las teselas planas miden **64×31 px** con `xrel=-31, yrel=0`; las pendientes y
algunas costas conservan el `height/yrel` del NFO y por eso se posicionan con
`SLOPE_HALF_H[tileh]` en vez de asumir siempre 15.5 px.

| Archivo extraído | Sprite ID | Descripción |
|-----------------|-----------|-------------|
| `grass.png` | 3981 | Hierba plana (`terrain_grass.png`, alias tras `crop_by_id`) |
| `grass_rough.png` | 4000 | Prado rugoso (`terrain_rough.png`; bosque, carbón, industria) |
| `water.png` | ~3984 | Agua (azul) |
| `shore_0.png` … `shore_7.png` | 4062–4069 (`SPR_SHORE_BASE`) | Costa: **un** sprite según pendiente en teselas Coast, no máscara por vecinos |
| `road_ty.png` | 1332 (`SPR_ROAD_Y`) | Recorte “Y” en OpenGFX; en el cliente se usa para `RoadDir::Tx` |
| `road_tx.png` | 1333 (`SPR_ROAD_X`) | Recorte “X” en OpenGFX; en el cliente se usa para `RoadDir::Ty` |
| `road_cross.png` | 1338 | Cruce de carretera (tx + ty) |
| `road_corner_a.png` | 1335 | Esquina NE-SW |
| `road_corner_b.png` | 1337 | Esquina NW-SE |
| `tram_flat_00.png` … `tram_flat_18.png` | 5990–6008 (`SPR_TRAMWAY_OVERLAY`) | Tranvía sobre asfalto; mismo índice lógico que `road_flat_*` (`GetRoadSpriteOffset`). Se generan con `scripts/descargar_graficos.sh`. |

#### Costa (`shore_*.png`) y `WaterTileType::Coast`

Los archivos `shore_0.png` … `shore_7.png` corresponden aproximadamente a los sprites
**4062–4069** (`SPR_SHORE_BASE` + offset). No son un “set de orientación” que se elige
mirando **vecinos** N/E/S/W sobre agua plana.

En OpenTTD, las teselas **MP_WATER** con tipo **Coast** (`m5` bits 4–7 = 1, ver
`water_map.h`) se dibujan con **`DrawShoreTile(tileh)`** (`water_cmd.cpp`): **un solo
sprite** según la **pendiente** de la tesela (`Slope` / `tileh`), vía la tabla
`tileh_to_shoresprite[]`. No se dibuja `DrawSeaWater` debajo de Coast; el sprite de
costa ya es el ground sprite del rombo. El agua “libre” (Clear) usa el sprite de agua
animada, sin superponer costa por adyacencia a tierra.

En openttdrs, el renderer alinea con eso: si `water_tile_type == 1` (Coast), se elige
`shore_{n}.png` con `shore_png_index(shore_tileh_for_draw_shore(...))` y posición
`tile_pos_half(..., shore_sprite_half_h(tileh))` (ver `crates/openttdrs-client/src/iso.rs`).
Si el `.ottdmap` no conserva el subtipo Coast en `m5` (todo agua como Clear), se aplica el
mismo dibujo de costa cuando la tesela de agua **linda con tierra** en su vecindario
de 8 celdas (un solo criterio geométrico; el sprite sigue siendo el de `tileh`, no una
máscara por vecinos).

#### Atención: convención de nombres de SPR_ROAD_*

En OpenTTD, `SPR_ROAD_Y` (1332) indica una carretera que corre en la **dirección ty**
del mapa (de tile `(tx,ty)` a `(tx,ty+1)`), que visualmente aparece como una diagonal
NW-SE en pantalla. El nombre "Y" se refiere al eje del mapa, no al eje de pantalla.

En mapas cargados desde `.ottdmap`, la orientación sale del **MAPT + m5** (carretera
normal, cruce a nivel, depósito, túnel/puente). Ver
[TILES_Y_SAVEGAMES_OPENTTD.md](MAPA_Y_FERROCARRIL.md#tiles-y-savegames-openttd).

En mapas **generados en código** (`m5 = 0`), el cliente usa un **fallback** mirando
vecinos:

```rust
let has_tx = is_road(pos + (±1, 0));   // vecinos en dirección tx
let has_ty = is_road(pos + (0, ±1));   // vecinos en dirección ty
```

---

### Suelo y sprites de árboles

Una tesela `MP_TREES` no siempre está sobre prado. `DrawTile_Trees` primero dibuja
su suelo y después compone de 1 a 4 árboles. Para una partida `.sav`, la fuente de
verdad es `MAP2` completo (`m2 | m2_hi << 8`), no el clima ni los vecinos:

| Bits de MAP2 | Dato | Uso en OpenTTD/openttdrs |
|--------------|------|--------------------------|
| 6–8 | `TreeGround` | Elige `Grass`, `Rough`, `SnowDesert`, `Shore` o `RoughSnow` |
| 4–5 | `TreeDensity` | Elige una de las cuatro bandas de césped o nieve/desierto |

Los cinco suelos se componen con el offset `SlopeToSpriteOffset(tileh)` (0–18):

| `TreeGround` | Suelo de referencia |
|--------------|---------------------|
| `Grass` | `3924 + densidad × 19 + pendiente` |
| `Rough` | pendiente `4000 + pendiente`; plano con las cinco variantes de `TileHash` |
| `SnowDesert` | `4493 + densidad × 19 + pendiente` |
| `Shore` | tabla de costa `SPR_SHORE_BASE` según la pendiente |
| `RoughSnow` | la misma tabla snow/desert que `SnowDesert` |

Los árboles usan la tabla original a partir de `SPR_TREE_BASE` (1576), posiciones
sub-tesela y orden estable por `x + y`; eso conserva qué copa queda delante. Los
bits de `m5` determinan especie, cantidad y crecimiento. El hash determinista sólo
es un fallback para mapas generados sin datos de partida, no para un `MP_TREES`
cargado.

Los 152 suelos necesarios (4 densidades × 19 pendientes de césped y snow/desert)
se extraen junto con el resto de OpenGFX mediante `descargar_graficos.sh`; para
añadirlos a una instalación ya descargada:

```bash
python3 scripts/crop_tree_ground_sprites.py
python3 scripts/gen_tile_atlas.py
```

---

### Sprites de vehículos de carretera (camiones)

Los camiones tienen 8 vistas (una por dirección), en grupos de 8 sprites consecutivos.
Cada vista mide aproximadamente **20×14 px**.

| Archivo | Sprite ID | Dirección en pantalla | Movimiento en tile |
|---------|-----------|----------------------|--------------------|
| `truck_ne.png` | 3585 | Arriba-derecha (NE) | `ty-1` |
| `truck_se.png` | 3587 | Abajo-derecha (SE) | `tx+1` |
| `truck_sw.png` | 3589 | Abajo-izquierda (SW) | `ty+1` |
| `truck_nw.png` | 3591 | Arriba-izquierda (NW) | `tx-1` |

Los sprites 3585-3592 son el primer modelo de camión. Los modelos siguientes están en
rangos de +8 (3593-3600, 3601-3608, etc.).

En isométrico, un movimiento en `+tx` (tile a la derecha-abajo) se visualiza en
dirección SE, y un movimiento en `-ty` (tile arriba-derecha) se visualiza en NE.

---

### Sprites de locomotoras (trenes)

#### Estado actual (jun 2026)

| Capa | Implementado | Paridad visual OpenTTD |
|------|--------------|------------------------|
| Datos (`EngineDef.train_image_index`) | ✅ cada motor tiene su índice del original | ✅ |
| Agrupación (`train_sprite_group`) | ✅ 5 conjuntos (vapor×2, Kirby, diésel, eléctrico) | 🟡 OpenTTD distingue más modelos |
| Selección en cliente (`train_layers_for`) | ✅ elige el array según grupo | ✅ |
| Ventana de compra (`train_preview`) | ✅ preview por `train_image_index` | ✅ |
| **PNG + paths en `vehicle_gfx_data_generated.rs`** | ✅ | ✅ (jun 2026) |

**Síntoma en juego:** todos los trenes se ven igual (sprite Kirby Paul Tank) aunque
la ventana de compra ya elige el conjunto correcto por motor.

**Causa:** `scripts/gen_vehicle_gfx_data.py` genera cinco arrays
(`TRAIN_VEHICLE_LAYERS`, `T0`, `T1`, `TDIESEL`, `TELECTRIC`) con offsets NFO
distintos, pero si falta el PNG del grupo apunta al fallback Kirby
(`vehicle_train_*.png`). Sin assets extraídos, **los cinco arrays enlazan al mismo
archivo**.

#### Cómo funciona en OpenTTD

Upstream: `GetDefaultTrainSprite(image_index, direction)` (`train_sprites.h`).
Cada `image_index` (0–23 en vanilla) mapea a 8 sprites consecutivos (N…NW).
OpenGFX templado usa rangos como 2905–2928 (Kirby y variantes de vapor),
2949–2956 (diésel), 2965–2972 (eléctrico), etc.

En openttdrs simplificamos a **5 grupos visuales** vía `train_sprite_group()` en
`openttdrs-core/src/engine.rs` — suficiente para distinguir familias (vapor
temprano/tardío, Kirby, diésel, eléctrico) sin exportar los ~24 conjuntos del GRF.

#### Tabla grupo → sprites OpenGFX → PNG

| Grupo | `train_sprite_group` | Motor representativo | `image_index` | Sprites OpenGFX | PNG esperado |
|-------|----------------------|----------------------|---------------|-----------------|--------------|
| T0 | 0 | Chaney Jubilee | 0 | 2905–2912 | `vehicle_train_t0_{n,ne,…,nw}.png` |
| T1 | 1 | Ginzu A4 | 1 | 2913–2920 | `vehicle_train_t1_*.png` |
| Kirby | 2 | Kirby Paul Tank | 2 | 2921–2928 | `vehicle_train_*.png` |
| Diésel | 3 | UU 37, Floss 47, … | 4–19, 22 | 2949–2956 | `vehicle_train_td_*.png` |
| Eléctrico | 4 | AsiaStar | 20–23 | 2965–2972 | `vehicle_train_te_*.png` |

Índice de capa = `Vehicle::render_direction()` (0..7 = N, NE, E, SE, S, SW, W, NW).

#### Paridad visual — trabajo pendiente

Para que cada grupo se vea distinto como en OpenTTD:

1. **Extraer PNG** desde OpenGFX:
   ```bash
   python3 scripts/extract_train_vehicle_sprites.py   # solo trenes, sin borrar tiles
   python3 scripts/extract_ship_vehicle_sprites.py    # solo barcos, sin borrar tiles
   python3 scripts/gen_vehicle_gfx_data.py
   # o bien el flujo completo:
   ./scripts/descargar_graficos.sh --32bpp
   ```
2. **Regenerar metadatos** (offsets `x_offs`/`y_offs` desde NFO):
   ```bash
   python3 scripts/gen_vehicle_gfx_data.py
   ```
   Salida: `crates/openttdrs-client/src/sprites/vehicle_gfx_data_generated.rs`
   (no editar a mano).
3. **Verificar** que el script no imprime `PNG ausentes (fallback Kirby)` y que
   cada `TRAIN_VEHICLE_LAYERS_*` apunta a su prefijo (`t0_`, `t1_`, `td_`, `te_`).
4. **Comprobar en juego:** comprar Chaney vs Kirby vs AsiaStar — sprites distintos
   en mapa y en ventana de compra.

**Costo estimado:** S (1–2 días) si solo se ejecutan scripts existentes; M si hace
falta ampliar grupos (más `image_index` → más recortes) o soportar 32bpp con
recolor por compañía.

**Ampliación opcional:** un PNG por cada `image_index` distinto (como NewGRF) —
requiere ampliar `GFX_SETS` en `gen_vehicle_gfx_data.py`, `train_layers_for` y
`descargar_graficos.sh`; no está planificado en el MVP.

#### Archivos relacionados

| Archivo | Rol |
|---------|-----|
| `openttdrs-core/src/engine.rs` | `train_image_index`, `train_sprite_group()` |
| `openttdrs-client/src/render/vehicles.rs` | `train_layers_for()`, dibujo en mapa |
| `openttdrs-client/src/sprites/vehicle_gfx_data_generated.rs` | Arrays `TRAIN_VEHICLE_LAYERS_*` |
| `scripts/extract_train_vehicle_sprites.py` | Recorte incremental de locomotoras (40 PNG) |
| `scripts/extract_ship_vehicle_sprites.py` | Recorte incremental de barcos (32 PNG) |
| `scripts/gen_vehicle_gfx_data.py` | Generador Rust desde PNG + NFO |
| `scripts/descargar_graficos.sh` | Recorte `crop_by_id` de sprites 2905–2972 |

Ver también [archive/ROADMAP_MENUS_UI.md](archive/ROADMAP_MENUS_UI.md) § limitaciones y
[archive/ROADMAP_PARIDAD_VISUAL.md](archive/ROADMAP_PARIDAD_VISUAL.md) § 14.

---

### Sprites de industrias — clima templado

#### ⚠️ Error frecuente: identificar sprites por posición Y en la hoja

Los sprites de industrias **no tienen etiquetas en el NFO**. Para identificarlos hay que
cruzar el `sprite_id` con la tabla `_industry_draw_tile_data[]` en
`src/table/industry_land.h` del código fuente de OpenTTD.

Esa tabla usa IDs **en hexadecimal**. Ejemplo: `0x7db = 2011`.

Los datos generados de industria no son independientes del perfil gráfico:
`gen_industry_gfx_data.py` toma dimensiones y anclas del NFO **activo** y la
caja 3D `M(dx,dy,sx,sy,sz)` de OpenTTD. Por eso se debe cambiar el perfil con
`scripts/descargar_graficos.sh --8bpp` o `--32bpp` (que regenera la tabla),
nunca editando `.graphics_mode` a mano.

#### Orden de industrias templadas y rango de sprites

| IT | Nombre | Sprite building (aprox.) |
|----|--------|--------------------------|
| 0 | Coal Mine | 2011–2035 |
| 1 | Power Station | 2036–2066 |
| 2 | Sawmill | 2067–2095 |
| 3 | Forest | 2096–2111 |
| 4 | Oil Refinery | 2112–2132 |
| 5 | Oil Rig | 2133–2152 |
| 6 | Factory | 2153–2173 |
| 7 | Printing Works | 2174–2199 ← **aquí está el sprite 2179** |
| 8 | Steel Mill | 2200–2219 |
| 9 | Iron Ore Mine | 2220–… |
| 12 | Oil Wells | ~2079–2095 (con frames animados) |

> **El sprite 2179 (29×43)** que inicialmente se etiquetó como "coal_mine" es en
> realidad parte de la **Printing Works** (imprenta). Se parece a un derrick o
> prensa industrial.

#### Coal Mine — sprites correctos

| Archivo | Sprite ID | Tamaño | xrel | yrel | Descripción |
|---------|-----------|--------|------|------|-------------|
| `coal_mine_hq.png` | 2013 | 58×50 | -16 | -33 | Headframe principal (torre de extracción) |
| `coal_mine_tower.png` | 2028 | 46×53 | -14 | -38 | Torre de extracción alternativa |
| `coal_mine_entry.png` | 2011 | 36×25 | -17 | -7 | Entrada/componente pequeño |

El sprite **2013** (58×50) es el edificio más representativo de la mina de carbón —
el armazón metálico que cubre el pozo de extracción.

#### Identificar sprites de industrias desconocidos

1. Encontrar el `sprite_id` en el NFO.
2. Convertir a hex: `hex(sprite_id)`.
3. Buscarlo en `src/table/industry_land.h` del upstream.
4. Contar las entradas `M(...)` antes de esa posición y dividir por 16
   (cada industria tiene ~16 tiles × 4 estados ≈ 16-32 entradas).
5. Mapear al índice de `IT_*` para identificar la industria.

---

### Sprites de estaciones y puntos de parada

| Sprite ID | Nombre en sprites.h | Descripción |
|-----------|---------------------|-------------|
| 2708 | `SPR_TRUCK_STOP_NE_GROUND` | Suelo de parada de camiones (NE) |
| 2712 | `SPR_TRUCK_STOP_NE_BUILD_A` | Edificio de parada (NE) |
| 1313 | `SPR_ROAD_PAVED_STRAIGHT_Y` | Carretera pavimentada Y |
| 1314 | `SPR_ROAD_PAVED_STRAIGHT_X` | Carretera pavimentada X |

---

### Proyección isométrica

OpenTTD usa una proyección isométrica **2:1** (el ancho del rombo es el doble que su
alto). Las teselas miden 64×31 px (ligeramente menos que los 32 px teóricos del
ratio 2:1, lo que produce un gap de 1 px entre tiles).

#### Fórmulas de conversión tile → pantalla (Bevy Y-up)

```rust
const ISO_HW: f32 = 32.0;  // mitad del ancho de la tesela
const ISO_QH: f32 = 16.0;  // cuarto del alto teórico (32/2)

fn iso(tx: i32, ty: i32) -> Vec2 {
    Vec2::new(
        (tx - ty) as f32 * ISO_HW,    // X: diferencia de ejes
        (tx + ty) as f32 * -ISO_QH,   // Y: suma de ejes (negativo = Y-up)
    )
}
```

#### Referencia de posicionamiento

En OpenTTD, el **punto de referencia** de cada sprite es el **vértice superior** del
rombo de la tesela (el píxel más alto del diamante). Esto se confirma porque todas las
teselas de suelo tienen `xrel=-31, yrel=0` en el NFO, lo que coloca el vértice superior
exactamente en el punto de referencia.

Para centrar un sprite con `Anchor::Center` en Bevy equivalente al anchor top-center:

```rust
// Desplazar el centro del sprite 15.5 px por debajo del vértice superior
fn tile_pos(tx: i32, ty: i32, layer: f32) -> Vec3 {
    let p = iso(tx, ty);
    Vec3::new(p.x, p.y - 15.5, (tx + ty) as f32 * 0.01 + layer)
}
```

#### Z-ordering (painter's algorithm)

Los sprites con mayor `tx + ty` están más cerca del espectador y deben renderizarse
encima. Se usa un incremento pequeño de Z:

```rust
z = (tx + ty) as f32 * 0.01 + layer
```

Donde `layer` diferencia entre suelo (0.0), overlays naturales (0.3), edificios (0.6)
y vehículos (1.0).

El suelo natural no usa la elevación como parte de esa profundidad. OpenTTD lo
emite primero como `DrawGroundSprite` y conserva el orden de inserción del
barrido diagonal `row = tx + ty`, `column = ty - tx`; la altura sólo cambia su
coordenada Y de pantalla. `ground_tile_pos_half` y
`TileViewportBounds::iter_coords` conservan ese contrato para evitar que una
parcela elevada pinte sobre una fila posterior. Las fundaciones y overlays
especiales siguen usando su orden local, porque OpenTTD los asocia a un sprite
padre/sortable en vez del pase genérico de suelo.

Este orden es independiente del baseset: se aplica igual con OpenGFX 8bpp y
OpenGFX2 32bpp. En este último, un mismo SpriteID tiene fila indexada y una
continuación `|` RGBA en el NFO; `parse_global_sprite_rects` selecciona la
alternativa `32bpp` de zoom normal con **sus propias** coordenadas y hoja
`.32.png`. Nunca se puede combinar el rectángulo 8bpp con la hoja 32bpp.

---

### Pendiente de terreno y cimientos (foundations)

**Terreno (MP_CLEAR, bosques, etc.):** el cliente calcula `tileh` con las alturas de las cuatro esquinas (`compute_tileh` / `iso.rs`) y elige `terrain_grass_slope_01..14` o `terrain_rough_slope_01..14` junto con `SLOPE_HALF_H` para posicionar el sprite.

**Cimientos de hormigón:** en OpenTTD se dibujan bajo carreteras, raíles y edificios cuando la tesela no está nivelada; los IDs de sprite dependen del NewGRF base (internamente hay macros tipo `SPR_FOUNDATION_*`). Para reproducirlos hay que localizar el bloque correspondiente en `ogfx1_base.nfo`, añadir recortes en `descargar_graficos.sh` y pintar una capa intermedia entre el terreno y el overlay — trabajo pendiente.

---

### Centro de la cámara para el mapa 24×18

Para que la cámara muestre el mapa completo centrado en una ventana de 1280×720:

```
cam_x = ((MAP_W - 1) - (MAP_H - 1)) / 2 × ISO_HW  =  96.0
cam_y = -((MAP_W - 1) + (MAP_H - 1)) / 2 × ISO_QH - 15.5  =  -335.5
```

El mapa isométrico ocupa aproximadamente 1280×671 px, encajando bien en 1280×720.

---

### Texture atlas (batching Bevy)

Tras extraer PNGs en `tiles/`, `scripts/gen_tile_atlas.py` empaqueta las imágenes
únicas en páginas bajo `assets/opengfx/atlas/tiles_atlas_{p}.png` y genera la tabla
compilada:

- **En el repo (sí):** `crates/openttdrs-client/src/sprites/tile_atlas_generated.rs`
- **En el repo (sí):** metadatos de draw (`*_draw_data_generated.rs`, p. ej. `effect_vehicle_draw_data_generated.rs`)
- **Gitignored (no):** `assets/opengfx/tiles/*.png` (~11 MB)
- **En el repo (sí):** `assets/opengfx/atlas/tiles_atlas_*.png` (~2–3 MB por página)

#### Atlas PNG en el repositorio

Los PNG del atlas **sí se versionan** (excepción en `.gitignore`). Tras cambiar sprites en
`tiles/`, regenerar **ambos**:

```bash
python3 scripts/gen_tile_atlas.py
git add assets/opengfx/atlas/ crates/openttdrs-client/src/sprites/tile_atlas_generated.rs
```

Los PNG sueltos en `tiles/` siguen ignorados; hace falta `descargar_graficos.sh` solo para
regenerar el atlas o añadir sprites nuevos.

La comprobación no destructiva también valida que cada página PNG versionada sea
idéntica píxel a píxel al empaquetado reproducido desde `tiles/`:

```bash
python3 scripts/gen_tile_atlas.py --check
```

---

### Scripts de utilidad

| Script | Descripción |
|--------|-------------|
| `scripts/descargar_graficos.sh` | Descarga OpenGFX y extrae sprites a `assets/opengfx/tiles/` |
| `scripts/crop_tree_ground_sprites.py` | Extrae incrementalmente los 152 suelos que usa `DrawTile_Trees` |
| `scripts/gen_effect_vehicle_sprites.py` | Humo tren, chispas, explosión, avería → `effect_vehicle_draw_data_generated.rs` |
| `scripts/gen_tile_atlas.py` | Empaqueta `tiles/` en atlas + `tile_atlas_generated.rs`; `--check` compara también los PNG píxel a píxel |
| `scripts/descargar_sonidos.sh` | Descarga OpenSFX a `assets/opensfx/` |
| `scripts/fetch-openttd-reference.sh` | Clona el código fuente de OpenTTD en `reference/` |

Los sprites extraídos se guardan en `assets/opengfx/tiles/` (ignorado por git).
El script de descarga incluye extracción automática con PIL si `grfcodec` está disponible.

## Bugs visuales de terreno (handoff)

<!-- fuente: HANDOFF_BUGS_VISUALES_TERRAIN.md -->

**Audiencia:** agentes / mantenedores.  
**Detalle histórico** (intentos, diffs, hipótesis largas):
[archive/HANDOFF_BUGS_VISUALES_TERRAIN.md](archive/HANDOFF_BUGS_VISUALES_TERRAIN.md).

Waypoints rail (cerrado): [HANDOFF_WAYPOINTS_RAIL.md](MAPA_Y_FERROCARRIL.md#waypoints-rail-handoff).

---

### Abiertos / a verificar

| # | Síntoma | Estado |
|---|---------|--------|
| C | Rectángulo verde semitransparente al iniciar (ghost construcción) | Abierto |
| D | Casas Toyland en pueblo templado | Abierto |
| E | Orillas de agua con artefactos blancos | Abierto (menor) |
| A | Tablero marrón/verde al iniciar | Parcialmente mitigado |
| B | Rombo de teselas oscuras al construir carretera | Parcialmente mitigado |

### Reproducción rápida

```bash
cd openttdrs
OPENTTDRS_JSON_SAVE=save/partida_2026-06-22_0942.json cargo run -p openttdrs-client
```

Ghost (C): entrar con herramienta paisaje (p. ej. Plantar bosque) activa; el
preview sigue al cursor desde el primer frame.

### Pistas de código

- Ghost: `update_build_ghost_preview` / toolbar landscape.
- Remap / culling: `render/world.rs`, `ui/toolbar/build_input/click.rs`.
- Casas / clima: `HOUSE_DRAW_DATA`, gen procedural; ver archive § D.
- Bytes tesela: [TILES_Y_SAVEGAMES_OPENTTD.md](MAPA_Y_FERROCARRIL.md#tiles-y-savegames-openttd).

Al cerrar un bug: marcar aquí y, si aporta fórmulas, una línea en TILES o
`SPRITES_OPENGFX.md` — no reexpandir este handoff.
