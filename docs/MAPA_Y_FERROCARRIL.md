# Mapa, saves y ferrocarril

Flujo save→cliente, formato `.ottdmap`, chunks/tiles OpenTTD, señales, colocación de vías y waypoints. Oráculos PBS/snapshot: [PARIDAD.md](PARIDAD.md).

## Índice

- [Flujo mapa y cliente](#flujo-mapa-y-cliente)
- [OTTDMAP](#formato-ottdmap)
- [Tiles y savegames](#tiles-y-savegames-openttd)
- [Señales](#señales-ferroviarias)
- [Vías](#colocación-de-vías)
- [Waypoints](#waypoints-rail-handoff)

---

## Flujo mapa y cliente

<!-- fuente: FLUJO_MAPA_Y_CLIENTE.md -->

Guía única que enlaza el pipeline principal del repo. Para detalle binario de teselas y chunks, ver **[TILES_Y_SAVEGAMES_OPENTTD.md](#tiles-y-savegames-openttd)**. Para diseño incremental del proyecto, ver **[DISENO_INCREMENTAL.md](ARCHITECTURE.md#diseño-incremental-i0i8)**.

### 1. De `.sav` a `.ottdmap`

1. Tener un save de OpenTTD (`.sav` o comprimido OTTZ/OTTX según soporte en `scripts/parse_sav.py`).
2. Ejecutar:
   ```bash
   python3 scripts/parse_sav.py ruta/partida.sav salida.ottdmap
   ```
3. El binario `MAP1` incluye planos densos v5/v5+12 y footers opcionales (**INDP**, **STNN**, **TNBP**, **STXY**). Ver doc de teselas para el layout exacto.

**CI:** el golden se valida con `python3 scripts/verify_parse_sav_reference.py` (fixture en `tests/fixtures/`).

### 2. Cliente con mapa real

1. Generar assets OpenGFX: `./scripts/descargar_graficos.sh --8bpp` (requiere `grfcodec`; salida bajo `assets/`, ignorada por git).
2. Arrancar:
   ```bash
   OTTDMAP_FILE=salida.ottdmap cargo run -p openttdrs-client
   ```

### 3. Simulación y persistencia JSON

- El núcleo expone `openttdrs_core::save` (`save` / `load` / `load_from_str`): JSON con `version` + `state`, o legado sin envoltorio (sigue cargando).
- `GameState::save_json` / `load_json` siguen disponibles para tests y serialización en memoria.
- **Arranque desde JSON:** `OTTDJSON_LOAD=estado.json cargo run -p openttdrs-client`.
- **En ventana:** **F5** o **Ctrl+S** guardan; **F9** o **Ctrl+L** cargan y **redibujan** suelo/vías/vehículos. Ruta por defecto `save/openttdrs_sim.json` o `OPENTTDRS_JSON_SAVE`; **F4** alterna entre `save/openttdrs_sim.json` y `save/openttdrs_autosave.json`. La cámara no usa **S** para moverse cuando va **Ctrl+S**.
- **P** pausa el avance de ticks de simulación.
- **Ctrl+H** alterna el HUD informativo de la esquina superior izquierda (datos de
  mapa, assets y diagnóstico); arranca oculto y no afecta toolbar, minimapa ni
  barra de estado. Para arrancar mostrándolo: en bash
  `OPENTTDRS_SHOW_HUD=1 cargo run -p openttdrs-client`; en fish
  `env OPENTTDRS_SHOW_HUD=1 cargo run -p openttdrs-client`.
- El cliente arranca con zoom **fijo OpenTTD**. **Ctrl+Alt+Z** alterna entre
  ese modo y el zoom **libre**. El modo fijo usa los seis niveles discretos del
  original (4×, 2×, normal, ½×, ¼× y ⅛×); en mapas grandes sólo deja
  disponibles los niveles que respetan el tope de culling. `+`/`−` y la rueda
  avanzan por niveles en ese modo.
  El atajo se puede reasignar con `toggle_zoom_mode=Ctrl+Alt+Z` dentro de
  `toolbar_hotkeys`.

### 4. Qué simula el core hoy

- Industrias: varios `IndustryKind` (mina, bosque, pozo de petróleo, fábrica con producción más lenta); vehículos **camión** y **tren** (misma lógica de movimiento; el tren conviene asociarlo a rutas con vía).
- Footers **STNN** / **TNBP** en `OttdmapExtras`; **TNBP**: decode Sl/gamma (`tnbp_decode`), JSON (`tnbp_blob_to_json_value` / `OttdmapExtras::tnbp_json_summary`), túneles JGR en `GameState::jgr_tunnels_from_footer`, cruce con mapa (`Map::jgr_tunnel_endpoint_match_stats`). Fixture `tests/fixtures/v5p12_tnbp.ottdmap`; regenerar con `scripts/gen_tnbp_fixture_ottdmap.py`. Saves reales: `parse_sav.py` + `OTTDMAP_FILE`; depuración `OTTDMAP_TNBP_JSON=1`. Validación CLI de un `.sav` → `.ottdmap` → resumen TNBP: `scripts/validate_sav_tnbp.sh partida.sav` (o `cargo run -p openttdrs-core --example validate_ottdmap_tnbp -- mapa.ottdmap`).

### 5. Enlaces rápidos

| Documento | Contenido |
|-----------|-----------|
| [README.md](../README.md) | Cómo correr, CI, stack |
| [TILES_Y_SAVEGAMES_OPENTTD.md](#tiles-y-savegames-openttd) | MAPT, planos, footers, OpenTTD vs export |
| [archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md) | Notas de sesión v5+12 / señales (histórico) |

## Formato OTTDMAP

<!-- fuente: OTTDMAP_FORMAT.md -->

Archivo **propio** de este proyecto: empaqueta un subconjunto del **estado del mapa** (y metadatos opcionales) de forma densa. **No** es un savegame OpenTTD (`.sav`); tampoco es un volcado bit a bit de los chunks del save.

- **Productor de referencia:** `scripts/parse_sav.py` (desde un `.sav` de OpenTTD).
- **Consumidores de referencia:** `openttdrs_core::Map::from_ottd_binary` y `Map::from_ottd_binary_with_extras` (`crates/openttdrs-core/src/map.rs`, `ottdmap_extras.rs`).

Todas las multibytes son **little-endian** salvo que se indique lo contrario.

### Cabecera versionada (16 bytes)

| Offset | Tamano | Contenido |
|--------|--------|-----------|
| 0 | 4 | Magic ASCII: `MAP1` (`0x4D 0x41 0x50 0x31`) |
| 4 | 4 | `width` (`u32`) |
| 8 | 4 | `height` (`u32`) |
| 12 | 2 | `format_version` (`u16`) |
| 14 | 2 | `flags` (`u16`) |

- Version actual emitida por `parse_sav.py`: `format_version = 1`.
- `flags` reservado para compatibilidad futura. Actualmente usa bit 0 (`HAS_M2_HI`), y el plano `m2_hi` se serializa siempre en v1.

Sea `N = width x height` (numero de teselas). Orden de teselas en todos los planos: `i = y * width + x` (fila a fila, `x` crece primero).

### Planos densos

Despues de la cabecera (`base = 16`) siguen los planos.

| Orden | Tamano | Nombre | Origen tipico | Notas |
|------|--------|--------|---------------|-------|
| 1 | `N` | `tile_types` | `MAPT` | Nibble alto `(byte >> 4) & 0xF` = `TileType` OpenTTD (`MP_*`). |
| 2 | `N` | `heights` | `MAPH` | Altura por tesela. |
| 3 | `N` | `m1` | `MAPO` chunk | Owner/indice de industria. |
| 4 | `N` | `m2` | `MAP2` bajo | Byte bajo de `MAP2`. |
| 5 | `N` | `m2_hi` | `MAP2` alto | Byte alto de `MAP2`. |
| 6 | `N` | `m3` | `M3LO` | Byte bajo de `m3()` (siempre presente en v1). En `MP_ROAD` normal, bits 0–3 = **tranvía** (`TrackBits`), 4–7 = owner tranvía; ver `road_map.h` / `TILES_Y_SAVEGAMES_OPENTTD.md`. |
| 7 | `N` | `m3hi` | `M3HI` | Corresponde a `m4()` en OpenTTD (señales, etc.). |
| 8 | `N` | `m5` | `MAP5` | Vias/carretera/industria/object según tipo de tesela. |
| 9 | `N` | `m6` | `MAPE` | Estacion/industria según tipo de tesela. |
| 10 | `N` | `m7` | `MAP7` | Reserva/NewGRF en mapa. |
| 11 | `2N` | `m8` | `MAP8` | `u16` LE por tesela. |

Longitud del bloque denso en `format_version = 1`: `base + 12N`.

### Footers opcionales

Van concatenados despues del bloque denso.

`dense_payload_end` fija el fin del denso en `base + 12N` para `MAP1`.

Formato de cada footer:

- `INDP`: `magic(4)` + `count(u32)` + `count * (industry_index(u16), industry_type(u8))`
- `STNN`: `magic(4)` + `len(u32)` + `blob`
- `TNBP`: `magic(4)` + `len(u32)` + `blob`
- `STXY`: `magic(4)` + `count(u32)` + `count * (x(u16), y(u16))`

Orden tipico de escritura en `parse_sav.py`: `INDP` (si hay), `STNN`, `TNBP`, `STXY`.

### M3 y tranvía (fidelidad de datos)

En **MAP1 `format_version = 1`** los planos `m3` y `m3hi` van **siempre** en el bloque denso (no hace falta un footer aparte). `scripts/parse_sav.py` rellena `M3LO` / `M3HI` desde el save cuando existen los chunks.

Que el **cliente** use esos bits para dibujar tranvía encima de la carretera es independiente del formato: el dato ya viaja en cada `Tile` (`crates/openttdrs-core/src/map.rs`).

### Compatibilidad

- Lector Rust y export actual usan unicamente `MAP1`.
- Cambios futuros de layout deben incrementar `format_version`.

### Que no incluye

No reemplaza un `.sav`: no incluye empresas, vehiculos, economia, scripts, goals ni gran parte del estado global de partida. Es mapa + metadatos auxiliares.

## Tiles y savegames OpenTTD

<!-- fuente: TILES_Y_SAVEGAMES_OPENTTD.md -->

Referencia **vigente** de bytes MAPT/`m5`/chunks y pipeline `.sav` → `.ottdmap`.
Complementa [SPRITES_OPENGFX.md](GRAFICOS.md#sprites-opengfx) (catálogos en
[archive/SPRITES_OPENGFX_COMPLETO.md](archive/SPRITES_OPENGFX_COMPLETO.md) y
[archive/INDUSTRIAS_OPENGFX.md](archive/INDUSTRIAS_OPENGFX.md)).

Código relevante:
- `scripts/parse_sav.py` — conversor `.sav` → `.ottdmap`
- `crates/openttdrs-core/src/map.rs` — struct `Tile`, `Map::from_ottd_binary`
- `crates/openttdrs-client/src/iso.rs` — proyección isométrica y pendientes
- `crates/openttdrs-client/src/sprites.rs` — constantes y lógica de sprites

#### Lectura rápida (vigente)

| Necesitas | Ir a |
|-----------|------|
| Tipo de tesela / planos | §1–4 |
| Road / rail / señales en tile | §5–7 |
| Industrias / objetos / casas | §8–10 |
| Slopes + sprites terreno | §12–13 |
| Import `.sav` / limitaciones | §16–17 |
| Terraform T1–T3 | §20 |
| Spec binaria corta | [OTTDMAP_FORMAT.md](#formato-ottdmap) |
| Flujo cliente | [FLUJO_MAPA_Y_CLIENTE.md](#flujo-mapa-y-cliente) |

Las secciones siguientes son el cuerpo de referencia (fórmulas no repetidas en
otros docs). Preferir código si hay divergencia.

---

### Índice

1. [Byte MAPT (tipo de tesela)](#1-byte-mapt-tipo-de-tesela)
2. [Formato .ottdmap v1→v5](#2-formato-ottdmap-v1v5)
   - [MP_WATER, MAP5 y costa en el cliente](#mp-water-map5-y-costa-en-el-cliente)
3. [Nombres reales de chunks en OpenTTD](#3-nombres-reales-de-chunks-en-openttd)
4. [Struct Tile en Rust](#4-struct-tile-en-rust)
5. [Byte m5 en MP_ROAD](#5-byte-m5-en-mp_road)
6. [MP_TUNNELBRIDGE](#6-mp_tunnelbridge)
7. [MP_RAILWAY: TrackBits en m5](#7-mp_railway-trackbits-en-m5)
8. [MP_INDUSTRY: gfx de 9 bits](#8-mp_industry-gfx-de-9-bits)
9. [MP_OBJECT: resolución de ObjectType desde OBJS](#9-mp_object-resolución-de-objecttype-desde-objs)
10. [MP_HOUSE: HouseID en m8](#10-mp_house-houseid-en-m8)
11. [MP_CLEAR: ClearGround en m5](#11-mp_clear-clearground-en-m5)
12. [Sistema de pendientes (slopes)](#12-sistema-de-pendientes-slopes)
13. [Sprites OpenGFX para terreno](#13-sprites-opengfx-para-terreno)
14. [De road bits a sprites](#14-de-road-bits-a-sprites)
15. [Relieve (altura) en pantalla](#15-relieve-altura-en-pantalla)
16. [Carga de partidas (.sav)](#16-carga-de-partidas-sav)
17. [Import nativo en openttdrs (`openttdrs-core/src/sav/`)](#17-import-nativo-en-openttdrs)
18. [Referencias en el código fuente de OpenTTD](#18-referencias-en-el-código-fuente-de-openttd)
19. [Resumen de archivos del proyecto](#19-resumen-de-archivos-del-proyecto)
20. [Terraform manual y autoslope (T1–T3)](#20-terraform-manual-y-autoslope-t1t3)

---

### 1. Byte MAPT (tipo de tesela)

En el stream descomprimido, el chunk `MAPT` tiene un byte por tesela.
El **tipo principal** está en los **bits 4–7**:

```text
TileType = (mapt_byte >> 4) & 0xF
```

| Valor | Nombre upstream    | Uso típico                           |
|------:|--------------------|--------------------------------------|
| 0     | `MP_CLEAR`         | Prado, rocas, campos, desierto, nieve |
| 1     | `MP_RAILWAY`       | Vías de tren (normal, señales, depósito) |
| 2     | `MP_ROAD`          | Carretera, cruce a nivel, depósito   |
| 3     | `MP_HOUSE`         | Casas / urbano                       |
| 4     | `MP_TREES`         | Árboles / bosque                     |
| 5     | `MP_STATION`       | Estaciones, paradas, aeropuertos     |
| 6     | `MP_WATER`         | Agua, canales                        |
| 7     | `MP_VOID`          | Borde del mapa                       |
| 8     | `MP_INDUSTRY`      | Industrias                           |
| 9     | `MP_TUNNELBRIDGE`  | Entrada túnel o rampa de puente      |
| 10    | `MP_OBJECT`        | Objetos (transmisores, faros, HQ…)   |

Los **bits 0–3** del MAPT guardan datos auxiliares (zona trópico, etc.).

#### Por qué guardamos `mapt` en `Tile`

El byte `m5` tiene significados completamente distintos según el tipo de tesela.
Sin el MAPT crudo no se puede decodificar `m5` correctamente (road bits, TrackBits,
gfx de industria, etc. son campos incompatibles según el tipo). Por eso `Tile` incluye
`mapt: u8` además de `kind`.

---

### 2. Formato .ottdmap v1→v5

Binario producido por `scripts/parse_sav.py` y consumido por `Map::from_ottd_binary`.

#### Cabecera común (todas las versiones)

| Offset | Tamaño | Contenido              |
|--------|--------|------------------------|
| 0–3    | 4 B    | Magic `MAPO` (0x4D41504F, LE) |
| 4–7    | 4 B    | `width` u32 LE         |
| 8–11   | 4 B    | `height` u32 LE        |

Tras el header siguen secciones de `W×H` bytes/palabras en orden `i = y*width + x`
(x varía rápido, igual que en los chunks MAPT/MAPH de OpenTTD).

#### Secciones por versión

| Sección     | Tamaño      | Versión | Contenido                                              |
|-------------|-------------|---------|--------------------------------------------------------|
| `tile_type` | W×H bytes   | v1+     | Byte MAPT original (nibble alto = TileType)            |
| `height`    | W×H bytes   | v1+     | Altura base de la tesela (0–255)                       |
| `m5`        | W×H bytes   | v1+     | Ver secciones 5–11; modificado para MP_OBJECT (v3+)    |
| `m1`        | W×H bytes   | v2+     | Byte MAP1/MAPO: owner, índice de industria             |
| `m6`        | W×H bytes   | v3+     | Byte MAP6/MAPE: bit 2 = bit 8 del gfx industria; StationType |
| `m8`        | W×H×2 bytes | v3+     | Bytes MAP8 LE: HouseID en MP_HOUSE (u16)               |
| `m3`        | W×H bytes   | v4+     | Byte M3LO del save (tram track bits 0–3 en MP_ROAD normal; ver `road_map.h`) |
| `m2`        | W×H bytes   | v5+     | Byte MAP2 (índices town/station/industry según tipo de tesela) |
| `m7`        | W×H bytes   | v5+     | Byte MAP7 (reservas, NewGRF en mapa, etc.) |
| `m3hi`      | W×H bytes   | v5+     | Byte M3HI (alto de `m3` / “m4” en `map_sl.cpp`) |

#### Footers opcionales (v5+, tras los planos denses)

Orden: **INDP** (si hay datos de industrias), **STNN** (si hay blob), **TNBP** (si hay blob de túnel/puente), **STXY** (lista de teselas `MP_STATION` derivada del mapa en `parse_sav.py`). Cada footer es: 4 bytes ASCII de magic + `u32` LE `len` + `len` bytes de payload (excepto **STXY**, donde `len` es el número de pares y el cuerpo es `len` × 4 bytes: `u16` x, `u16` y).

| Magic | Contenido |
|-------|-----------|
| `INDP` | `u32` count; luego `count` × (`u16` industry_index, `u8` industry_type) |
| `STNN` | Blob crudo del chunk `STNN` (CH_TABLE o CH_ARRAY según versión del save) |
| `TNBP` | Blob del primer chunk entre TNBP, TBUS o TUNN presente en el save |
| `STXY` | `u32` count; luego `count` × (`u16` x, `u16` y) — teselas con tipo `MP_STATION` en MAPT |

`Map::from_ottd_binary` en **openttdrs-core** solo lee los planos densos hasta v5; ignora cualquier byte extra (footers).

#### NewGRF y alcance del export

Exponer MAP7, MAP8, M3HI/M3LO y blobs **no** sustituye un stack NewGRF completo: siguen siendo necesarios los archivos `.grf`, definiciones de acciones/sprites y la lógica del cliente para interpretar bits y tablas fuera del mapa denso.

#### Evolución v1 → v2 → v3

**v1** (inicial): solo `mapt`, `height`, `m5`. Suficiente para renderizar suelo y
carreteras básicas.

**v2**: añade `m1` (byte MAPO del savegame). Permite obtener el owner de cada tesela
y el índice de industria en MP_INDUSTRY (bits 0–6 de m1). Antes `m1` era todo ceros
porque no se exportaba — los que cargaban partidas veían cero en ese campo siempre.

**v3**: añade `m6` (byte MAPE) y `m8` (2 bytes LE por tesela, MAP8). Con `m6` se
pueden leer los gfx de industria de 9 bits correctamente. Con `m8` se obtiene el
HouseID real de cada casa urbana.

**v4**: añade `m3` (byte M3LO por tesela, chunk `M3LO` del save sin transformar).
Sirve para tranvía en carretera (`GetRoadBits(..., Tram)` usa `GB(m3,0,4)` en OpenTTD).

**v5**: añade `m2`, `m7`, `m3hi` (chunks `MAP2`, `MAP7`, `M3HI` del save, padding a W×H) y los footers anteriores.

#### Detección de versión en el lector Rust

```rust
// from_ottd_binary (map.rs)
let has_m1 = data.len() >= 12 + n * 4;   // v2+
let has_m6 = data.len() >= 12 + n * 5;   // v3+
let has_m8 = data.len() >= 12 + n * 5 + n * 2; // v3+
let has_m3 = data.len() >= 12 + n * 8;   // v4+ (hasta fin de sección M3LO inclusive)
let has_v5_planes = data.len() >= 12 + n * 11; // v5+ (MAP2, MAP7, M3HI tras M3LO)
```

#### MP_WATER, MAP5 y costa en el cliente

En OpenTTD, en teselas `MP_WATER`, el byte `MAP5` (`m5`) codifica el subtipo en **bits
4–7** (`WaterTileType` en `water_map.h`):

| Valor (bits 4–7) | Nombre        | Uso típico                          |
|----------------:|---------------|-------------------------------------|
| 0               | `Clear`       | Mar, río, canal (agua animada)      |
| 1               | `Coast`       | Orilla: sprite `SPR_SHORE_*` + pendiente |
| 2               | `Lock`        | Esclusa                           |
| 3               | `Depot`       | Depósito naval                    |

**Export `.ottdmap`:** `scripts/parse_sav.py` copia el chunk `MAP5` del save **tal
cual** a la sección `m5` del binario (salvo la sustitución de `MP_OBJECT` con datos
`OBJS`). En saves normales de OpenTTD, las teselas de costa suelen llevar **`Coast`
(`0x10` en bits 4–7, es decir `m5 & 0xF0 == 0x10`)** cuando el juego las guardó
correctamente.

**Cliente (`openttdrs-client`):** se dibujan sprites de costa (`shore_*.png` vía
`shore_tileh_for_draw_shore` + `shore_png_index`) si:

1. **`(m5 >> 4) & 0xF == 1`** (Coast explícito), **o**
2. **`(m5 >> 4) & 0xF == 0`** (Clear) **y** la tesela de agua **linda con tierra** en
   el vecindario de 8 celdas (no `Water`, no `Void`) — función
   `water_tile_touches_land` en `render/grid.rs`.

El caso (2) cubre mapas o pipelines donde el agua en la orilla queda como **Clear**
con `m5 = 0` pero el relieve sigue siendo válido: **no** se usa una máscara N/E/S/W
para elegir el PNG de costa; solo decide *si* entramos en el modo costa; el índice
del sprite sigue siendo el de la **pendiente** (`tileh`). En modo costa se dibuja
solo `DrawShoreTile`, sin agua animada debajo; el agua abierta sin tierra vecina
sigue siendo solo agua animada.

Si el `tileh` crudo del bloque 2×2 es **0** en una tesela que igual debe dibujarse como
costa (típico cuando la tierra solo está **fuera** de ese bloque, p. ej. costa recta
con tierra al norte), OpenTTD no usa agua plana en Coast. El cliente aplica
`infer_coast_tileh_when_flat` (`iso.rs`) antes de elegir `shore_*.png`.

Tests unitarios de `compute_tileh` (mapas 2×2, borde 1×1, franja 2×1): `iso.rs`,
módulo `compute_tileh_tests`.

Tests de carga `.ottdmap` con `MP_WATER` y `m5` (Coast `0x10` vs Clear `0`): `map.rs`,
módulo `ottdmap_binary_tests` (`minimal_ottdmap_water_coast_v1`,
`minimal_ottdmap_mixed_water_v1`).

---

### 3. Nombres reales de chunks en OpenTTD

El código fuente de OpenTTD (`saveload/map_sl.cpp`) registra los chunks del mapa con
nombres que **no siempre coinciden** con el nombre lógico del buffer. Este fue uno
de los descubrimientos críticos del proyecto:

| Nombre chunk (savegame) | Buffer lógico | Contenido                              |
|------------------------|---------------|----------------------------------------|
| `MAPT`                 | —             | TileType por tesela (nibble 4–7)       |
| `MAPH`                 | —             | Altura base                            |
| `MAPO`                 | MAP1          | Owner / datos de tesela 1              |
| `MAP2`                 | MAP2          | Datos misceláneos                      |
| `M3LO`                 | MAP3 low      | Datos de tesela 3 (bits 0–7)           |
| `M3HI`                 | MAP3 high     | Datos de tesela 3 (bits 8–15)          |
| `MAP5`                 | MAP5          | Byte m5 principal                      |
| `MAPE`                 | MAP6          | Byte m6                                |
| `MAP7`                 | MAP7          | Datos extra (reservado/NewGRF)         |
| `MAP8`                 | MAP8          | HouseID y datos NewGRF (2 bytes/tile)  |
| `MAPS`                 | —             | Dimensiones (CH_TABLE con dim_x, dim_y)|
| `OBJS`                 | —             | Array de objetos (CH_TABLE sparse)     |

#### Por qué m1 era todo ceros antes de v2

El script en v1 buscaba `MAP1` como nombre de chunk, pero el nombre real es `MAPO`.
Al no encontrarlo, el chunk quedaba vacío y `m1` se rellenaba de ceros. La corrección
fue buscar `chunks.get('MAPO')` en lugar de `'MAP1'`.

```python
## parse_sav.py (corrección v2)
map1 = chunks.get('MAPO', b'')  # MAPO = MAP1 (owner/datos de tesela 1)
map6 = chunks.get('MAPE', b'')  # MAPE = MAP6
map8 = chunks.get('MAP8', b'')  # MAP8 = HouseID (2 bytes/tile)
```

---

### 4. Struct Tile en Rust

```rust
// crates/openttdrs-core/src/map.rs
pub struct Tile {
    pub height: u8,
    pub kind:   TileKind,
    pub mapt:   u8,   // byte MAPT original
    pub m5:     u8,   // byte MAP5
    pub m1:     u8,   // byte MAPO (v2+)
    pub m6:     u8,   // byte MAPE (v3+)
    pub m8:     u16,  // bytes MAP8 LE (v3+)
    pub m3:     u8,   // M3LO (v4+)
    pub m2:     u8,   // MAP2 (v5+)
    pub m7:     u8,   // MAP7 (v5+)
    pub m3hi:   u8,   // M3HI (v5+)
}
```

Correspondencia `TileType` → `TileKind`:

| TileType | Nombre          | TileKind            |
|----------|-----------------|---------------------|
| 0        | `MP_CLEAR`      | `Grass`             |
| 1        | `MP_RAILWAY`    | `Rail`              |
| 2        | `MP_ROAD`       | `Road`              |
| 3        | `MP_HOUSE`      | `House`             |
| 4        | `MP_TREES`      | `Forest`            |
| 5        | `MP_STATION`    | `Station`           |
| 6        | `MP_WATER`      | `Water`             |
| 7        | `MP_VOID`       | `Void`              |
| 8        | `MP_INDUSTRY`   | `Industry`          |
| 9        | `MP_TUNNELBRIDGE`| `Rail` o `Road` según m5 bit 2 |
| 10       | `MP_OBJECT`     | `Grass` (placeholder) |

---

### 5. Byte m5 en MP_ROAD

Fuente: `reference/openttd-upstream/src/road_map.h`.

#### Subtipo (bits 6–7)

```text
RoadTileType = (m5 >> 6) & 0x3
```

| Valor | `RoadTileType` | Significado            |
|------:|----------------|------------------------|
| 0     | `Normal`       | Carretera normal        |
| 1     | `Crossing`     | Cruce a nivel (vía + carretera) |
| 2     | `Depot`        | Depósito de carreteras  |

#### Caso Normal (subtipo 0)

Road bits en **bits 0–3**:

| Bit | RoadBit | Dirección          |
|----:|---------|--------------------|
| 0   | `NW`    | hacia (x, y-1)     |
| 1   | `SW`    | hacia (x+1, y)     |
| 2   | `SE`    | hacia (x, y+1)     |
| 3   | `NE`    | hacia (x-1, y)     |

Combinaciones comunes:

| Constante  | Bits    | Valor | Descripción                |
|------------|---------|-------|----------------------------|
| `ROAD_X`   | SW+NE   | 0x0A  | Diagonal / (NE↔SW)         |
| `ROAD_Y`   | NW+SE   | 0x05  | Diagonal \ (NW↔SE)         |
| `ROAD_ALL` | todos   | 0x0F  | Cruce 4 vías               |

#### Caso Crossing (subtipo 1) — error frecuente

En un cruce a nivel, los bits 0–3 **no** son road bits estándar.
El eje de la carretera está en **bit 0** (`GetCrossingRoadAxis`):

- `AXIS_X` (0) → carretera como `ROAD_X` (0x0A).
- `AXIS_Y` (1) → carretera como `ROAD_Y` (0x05).

**Bug histórico en openttdrs:** tratar el cruce como carretera normal y leer road bits
en 0–3 producía valores absurdos; el código caía en fallback por vecinos, rompiendo
trazados rectos en mapas con cruces a nivel.

#### Caso Depot (subtipo 2)

Dirección diagonal de la salida en **bits 0–1** (`DiagDirection`):

```text
road_bits = (1 << (3 ^ d)) & 0xF   // d = DiagDirection 0..3
```

| d | DiagDirection | En pantalla    |
|---|--------------|----------------|
| 0 | NE           | Arriba-derecha |
| 1 | SE           | Abajo-derecha  |
| 2 | SW           | Abajo-izquierda|
| 3 | NW           | Arriba-izquierda|

---

### 6. MP_TUNNELBRIDGE

`GetTunnelBridgeDirection`: dirección diagonal en bits **0–1** de `m5`.
`GetTunnelBridgeTransportType`: bits **2–3** (0 = carretera, 1 = raíl).

```rust
// map.rs: MP_TUNNELBRIDGE → Rail o Road
9 => {
    if m5 & 0x04 != 0 { TileKind::Rail } else { TileKind::Road }
}
```

Para obtener el tramo de carretera en la boca del túnel/rampa se aplica la misma
fórmula que el depósito: `(1 << (3 ^ d)) & 0xF`.

---

### 7. MP_RAILWAY: TrackBits en m5

Fuente: `reference/openttd-upstream/src/rail_map.h`.

#### RailTileType (bits 6–7 de m5)

```text
RailTileType = (m5 >> 6) & 0x3
```

| Valor | Constante           | Significado          |
|------:|---------------------|----------------------|
| 0     | `RAIL_TILE_NORMAL`  | Vía normal           |
| 1     | `RAIL_TILE_SIGNALS` | Vía con señales      |
| 3     | `RAIL_TILE_DEPOT`   | Depósito de trenes   |

#### TrackBits (bits 0–5 de m5)

Para tipos `Normal` y `Signals`, los **6 bits** bajos de m5 son el bitmask de vías:

| Bit | Constante      | Valor | Descripción                     |
|----:|----------------|------:|---------------------------------|
| 0   | `TRACK_BIT_X`  | 1     | Diagonal X (NE↔SW)              |
| 1   | `TRACK_BIT_Y`  | 2     | Diagonal Y (NW↔SE)              |
| 2   | `TRACK_BIT_UPPER` | 4  | Tramo superior (horizontal N)   |
| 3   | `TRACK_BIT_LOWER` | 8  | Tramo inferior (horizontal S)   |
| 4   | `TRACK_BIT_LEFT`  | 16 | Tramo izquierdo (vertical W)    |
| 5   | `TRACK_BIT_RIGHT` | 32 | Tramo derecho (vertical E)      |

> **Crítico**: son **6 bits**, no 4. La máscara correcta es `m5 & 0x3F`.
> Si se usa `m5 & 0x0F` se pierde LEFT y RIGHT, rompiendo vías en esos ejes.

Para tipo `Depot`, la dirección está en bits 0–1 (igual que depósito de carretera).

#### 7.1 MP_RAILWAY — teselas con señales

Cuando `RailTileType = RAIL_TILE_SIGNALS` (`m5` bits 6–7 = `01`), la tesela puede
albergar hasta **cuatro señales** (bits 0–3), cada una asociada a direcciones concretas
según el `Track` presente en `m5 & 0x3F`.

| Campo en `.ottdmap` | Equivalente OpenTTD | Uso en señales |
|--------------------|---------------------|----------------|
| `m3` bits 7–4 | `m3()` bits 7–4 | Máscara **presente** (bit = señal existe) |
| `m3hi` bits 7–4 | `m4()` bits 7–4 | Máscara **estado** (bit 1 = verde) |
| `m2` bits 2–0, 6–4 | `m2()` | **Tipo** (`SignalType` 0–5) por par de señales |
| `m2` bits 3, 7 | `m2()` | **Variante** eléctrico (0) / semáforo (1) |
| `m2` bits 8–11 | `m2()` | Reserva **PBS** (path signals) |
| `m5` bit 4 | `m5()` bit 4 | Reserva PBS en **cruce a nivel** (`HasCrossingReservation`); vía plana usa `m2` bits 8–11 |

Tipos (`SignalType` en `signal_type.h`): `0` block, `1` entry, `2` exit, `3` combo,
`4` path, `5` path one-way. Valores en `m2` como enteros 3-bit (ver tabla en
[landscape.html](https://github.com/OpenTTD/OpenTTD/blob/master/docs/landscape.html)).

En doble vía Horz/Vert, **cada clic coloca una señal en un solo carril**; la pieza
se elige con `fract_x` / `fract_y` (`GenericPlaceSignals` / `resolve_signal_track`).
El cliente usa `world_pos_to_rail_signal_pick` para snap al riel vecino y
`rail_signal_track_offset` al dibujar (paridad overlays 1007–1010).

**Colocación de vía** (Horz/Vert vs autoraíl, sin robar tesela vecina): [VIAS_FERROVIARIAS_COLOCACION.md](#colocación-de-vías).

**Guía completa** (comportamiento jugable, presignals, PBS, plan de fases en openttdrs):
[SENALES_FERROVIARIAS.md](#señales-ferroviarias).

---

### 8. MP_INDUSTRY: gfx de 9 bits

Fuente: `reference/openttd-upstream/src/industry_map.h`, `GetCleanIndustryGfx`.

#### Por qué m5 solo (8 bits) no alcanza

OpenTTD tiene más de 255 tipos de gfx de industria en total. El campo `gfx` es
conceptualmente de 9 bits. El bit 8 (el noveno) está almacenado en **bit 2 de m6**:

```text
gfx = m5 | (((m6 >> 2) & 1) << 8)
```

Equivalente en Python (parse_sav.py) y Rust (sprites.rs):

```rust
// sprites.rs
pub fn industry_sprite_for_gfx(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    let entry = INDUSTRY_GFX_DATA.get(usize::from(gfx))?;
    ...
}

// Llamada en main.rs:
let gfx = u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8);
```

#### Tabla de rangos por industria

| gfx     | Industria               |
|---------|-------------------------|
| 0–6     | Coal Mine               |
| 7–10    | Power Station           |
| 11–15   | Sawmill                 |
| 16–23   | Oil Refinery            |
| 24–28   | Forest                  |
| 29–32   | Printing Works          |
| 33–38   | Oil Rig                 |
| 39–42   | Steel Mill              |
| 43–46   | Factory                 |
| 47–51   | Oil Wells               |
| 52–57   | Farm                    |
| 58–59   | Bank (Templado)         |
| 60–71   | Copper Ore Mine         |
| 72–88   | Plantaciones/otros      |
| 89–90   | Gold Mine               |
| 91–99   | Iron Ore Mine           |
| 100–119 | Otros climas (trópico…) |

La tabla completa está en `crates/openttdrs-client/src/sprites.rs` (`INDUSTRY_GFX_DATA`).

#### m1 y m2 en MP_INDUSTRY

- **`m2`** = `IndustryID` (`GetIndustryIndex` en OpenTTD): identifica la instancia de
  planta; openttdrs agrupa teselas adyacentes con el mismo `m2`.
- **`m1` bit 7** = industria terminada (`IsIndustryCompleted`); bits 0–1 = etapa de obra
  si no está terminada.
- Footer **`INDP`**: pares `(industry_index, industry_type)` indexados por **`m2`**, no por `m1`.

---

### 9. MP_OBJECT: resolución de ObjectType desde OBJS

#### Problema en savegames modernos (v300+)

En OpenTTD moderno, el chunk `MAP5` para teselas `MP_OBJECT` **no contiene el ObjectType**;
contiene los bits altos del `ObjectID` (instancia específica del objeto). El `ObjectType`
real (qué clase de objeto es: transmisor, faro, HQ…) está almacenado en el chunk `OBJS`.

Si se lee `m5` directamente para MP_OBJECT se obtiene un valor incorrecto que no corresponde
a ningún tipo semántico útil.

#### Estructura del chunk OBJS (CH_TABLE / CH_SPARSE_TABLE)

`OBJS` es un array disperso donde cada elemento representa un objeto colocado en el mapa.
Los campos relevantes parseados en `parse_sav.py`:

| Campo            | Tipo  | Significado                              |
|------------------|-------|------------------------------------------|
| `location.tile`  | U32   | Índice lineal del tile base del objeto   |
| `type`           | U16   | ObjectType (0 = Transmisor, 1 = Faro…)  |

#### Tipos conocidos de ObjectType

| ObjectType | Nombre              |
|------------|---------------------|
| 0          | `OBJECT_TRANSMITTER` (antena) |
| 1          | `OBJECT_LIGHTHOUSE`  (faro)   |
| 2+         | HQ de empresa, estatuas, etc. |

#### Cómo parse_sav.py resuelve el ObjectType

```python
## parse_sav.py
obj_types: dict[int, int] = chunks.get('OBJS', {})  # {tile_index: object_type}

m5_list = bytearray(map5[:expected])
if obj_types:
    for i in range(expected):
        if (mapt[i] >> 4) & 0xF == 10:  # MP_OBJECT
            t = obj_types.get(i, 0xFF)
            if t != 0xFF:
                m5_list[i] = t  # sobrescribir con ObjectType real
m5_data = bytes(m5_list)
```

Tras este paso, `m5` en el `.ottdmap` contiene el `ObjectType` real (no el `ObjectID`)
para los tiles `MP_OBJECT`. Así el renderer puede distinguir transmisor de faro
sin acceder al chunk OBJS en tiempo de ejecución.

---

### 10. MP_HOUSE: HouseID en m8

#### Por qué m5 no alcanza

`MP_HOUSE` tiene más de 255 tipos de casas en OpenTTD (especialmente con NewGRF).
El `HouseID` es un **u16** y se almacena en el chunk `MAP8` (2 bytes little-endian
por tesela).

```text
HouseID = m8  (u16, little-endian)
```

El byte `m5` en MP_HOUSE guarda otras cosas (etapa de construcción, etc.), **no** el
HouseID.

#### Etapa de construcción (`m3` + `m5`)

OpenTTD (`town_map.h`: `IsHouseCompleted`, `GetHouseBuildingStage`):

| Campo | Uso en `MP_HOUSE` |
|-------|-------------------|
| `m3` bit 7 | **1** = edificio terminado; **0** = en obra |
| `m5` (terminado) | Edad de la casa en años (0–255) |
| `m5` bits 4..3 (en obra) | Etapa de construcción **0..3** para el sprite |
| `m5` bits 2..0 (en obra) | Contador de obra (avance entre etapas) |

Índice en `_town_draw_tile_data`:

```text
house_id * 16 + TileHash2Bit(x, y) * 4 + GetHouseBuildingStage()
```

En openttdrs: [`house_building_stage_from_tile`](../../crates/openttdrs-client/src/sprites.rs)
decodifica la etapa; si `m3 & 0x80 != 0` devuelve **3** aunque `m5` sea la edad.

#### Implementación en openttdrs

El campo `m8: u16` en `Tile` se lee desde el formato `.ottdmap` v3. La tabla
`HOUSE_DRAW_DATA` (generada desde `town_land.h`) cubre las **110** casas originales
(0..109) × **16** filas cada una (`scripts/gen_house_draw_data.py`).

Cada entrada tiene dos componentes (stage 3 = edificio completado):
- **`s1`**: sprite de suelo/base (`0` = usar grass por defecto)
- **`s2`**: sprite del edificio overlay (`0` = sin edificio)

Los sprites se cargan como `house_s{sprite_id}.png` (67 sprites únicos, IDs 1311–1575 + 4569).

```rust
// Índice OpenTTD (110 casas × 16 filas):
let stage = house_building_stage_from_tile(tile.m5, tile.m3);
let idx = house_draw_data_index_for_tile(tile.m8 & 0xFFF, tx, ty, stage);
let spec = &HOUSE_DRAW_DATA[idx];
// s1 = suelo/base (0 = hierba del mapa); s2 = edificio (0 = sin overlay)
```

Los sprites se cargan como `house_s{sprite_id}.png`. Constantes `SPR_*` de `town_land.h`
(estadio `1479–1482`, concreto `1420`, toyland `4675–4676`) se resuelven vía `sprites.h`
en `scripts/gen_house_draw_data.py`.

**Parques / suelo-only:** filas con `s1 == 0` y `s2 == 0` son intencionales (solo hierba).

**Estadio:** HouseID **20–23** (climas); suelo N/E/W/S = sprites **1479–1482**.

Para HouseIDs **≥ 110** (NewGRF) el cliente aplica `house_id % 110` como sustituto
visual hasta cargar specs NewGRF (`subst_id` en OpenTTD).

#### Tabla de tipos de casa temperate (stage 3)

| HouseIDs | s1 (ground)         | s2 (building)            | Descripción              |
|----------|---------------------|--------------------------|--------------------------|
| 0        | 1424 (ground)       | 1423 (tall office)       | Tall Office Block        |
| 1–3      | 1424                | 1425                     | Office Block variants    |
| 4–7      | 1429                | 1428                     | Large Office Block       |
| 8–11     | 1433                | 1432                     | Small Block of Flats     |
| 12–15    | 1437                | 1436                     | Church                   |
| 16–19    | 1311 (concrete)     | 1442                     | Large Office (concrete)  |
| 20–23    | 1311                | 4569 (ogfx1_base01.png)  | Large Office v2          |
| 24–25    | 1447                | 1446                     | Townhouse V1             |
| 26–27    | 1505                | 1506                     | Townhouse V2             |
| 28–31    | 1311                | 1450                     | Hotel NW                 |
| 32–35    | 1311                | 1453                     | Hotel SE                 |
| 36–79    | 1311 / 0(grass)     | 1454–1478                | Decorativos, torres, etc.|
| 80–95    | 0 (grass)           | 1483–1486                | Casas pequeñas, cottages |
| 96–127   | 1487–1574           | 1488–1575                | Tiendas, shops, townhouses|

> **Nota**: el sprite 4569 está en `ogfx1_base01.png`, no en `ogfx1_base00.png`. El script
> `descargar_graficos.sh` ahora carga ambos sheets automáticamente.

---

### 11. MP_CLEAR: ClearGround en m5

Fuente: `reference/openttd-upstream/src/clear_map.h`.

#### ClearGround (bits 2–4 de m5)

```text
ClearGround = (m5 >> 2) & 0x7
```

| Valor | `ClearGround`        | Descripción               |
|------:|----------------------|---------------------------|
| 0     | `CLEAR_GRASS`        | Hierba (densidad en 0–1)  |
| 1     | `CLEAR_ROUGH`        | Terreno irregular         |
| 2     | `CLEAR_ROCKY`        | Rocoso                    |
| 3     | `CLEAR_FIELDS`       | Campos de cultivo         |
| 4     | `CLEAR_SNOW`         | Nieve (ártico)            |
| 5     | `CLEAR_DESERT`       | Desierto (tropical)       |

#### Densidad de hierba (bits 0–1 de m5 cuando ClearGround = GRASS)

```text
grass_density = m5 & 0x3   // 0=bare, 1=1/3, 2=2/3, 3=full
```

Esto determina qué sprite base usar:
- 0 → `SPR_FLAT_BARE_LAND` (3924)
- 1 → `SPR_FLAT_1_THIRD_GRASS_TILE` (3943)
- 2 → `SPR_FLAT_2_THIRD_GRASS_TILE` (3962)
- 3 → `SPR_FLAT_GRASS_TILE` (3981)

---

### 12. Sistema de pendientes (slopes)

#### Concepto: tileh como bitmask de esquinas elevadas

Cada tesela tiene cuatro esquinas. La pendiente (`tileh`) es un bitmask de 4 bits
donde cada bit indica si esa esquina está por **encima del mínimo** entre las cuatro:

```text
hnorth = height(tx,   ty  )
hwest  = height(tx+1, ty  )
heast  = height(tx,   ty+1)
hsouth = height(tx+1, ty+1)

min_h = min(hnorth, hwest, heast, hsouth)
tileh = 0
if hwest  > min_h: tileh |= 1  (SLOPE_W)
if hsouth > min_h: tileh |= 2  (SLOPE_S)
if heast  > min_h: tileh |= 4  (SLOPE_E)
if hnorth > min_h: tileh |= 8  (SLOPE_N)
```
(Misma convención que `GetTileSlopeGivenHeight` en `tile_map.cpp` de OpenTTD.)

Implementado en `crates/openttdrs-client/src/iso.rs` → `compute_tileh`.

#### Valores de tileh válidos (0–14)

`tileh = 15` (todas las esquinas elevadas) es una **pendiente empinada** que requiere
sprites especiales; actualmente se trata como `min(tileh, 14)`.

| tileh | Esquinas elevadas | Descripción          |
|------:|-------------------|----------------------|
| 0     | ninguna           | Plano                |
| 1     | W                 | Pendiente W          |
| 2     | S                 | Pendiente S          |
| 3     | WS                | Esquina doble WS     |
| 4     | E                 | Pendiente E          |
| 5     | WE                | Pendiente WE (doble) |
| 6     | SE                | Esquina doble SE     |
| 7     | WSE               | Tres esquinas        |
| 8     | N                 | Pendiente N          |
| 9     | NW                | Esquina doble NW     |
| 10    | NS                | Pendiente NS (doble) |
| 11    | NWS               | Tres esquinas        |
| 12    | NE                | Esquina doble NE     |
| 13    | NWE               | Tres esquinas        |
| 14    | NSE               | Tres esquinas        |

#### SLOPE_HALF_H: ajuste vertical por pendiente

Para renderizar cada tesela en la posición vertical correcta se usa `tile_pos_half`
con una constante `half_h` que varía según `tileh`. Derivada del campo `height` y
`yrel` del NFO de OpenGFX:

```rust
// iso.rs
pub const SLOPE_HALF_H: [f32; 15] = [
    15.5, // 0:  flat
    15.5, // 1:  W
    11.5, // 2:  S
    11.5, // 3:  WS
    15.5, // 4:  E
    15.5, // 5:  WE
    11.5, // 6:  SE
    11.5, // 7:  WSE
    11.5, // 8:  N
    11.5, // 9:  NW
    7.5,  // 10: NS
    7.5,  // 11: NWS
    11.5, // 12: NE
    11.5, // 13: NWE
    7.5,  // 14: NSE
];
```

La fórmula completa de posición de una tesela:

```rust
// iso.rs: tile_pos_half(tx, ty, base_z, layer, half_h) — base_z = mínimo de esquinas (GetTileZ)
let p = iso(tx, ty);          // proyección isométrica plana
let elev = base_z as f32 * HEIGHT_PX;  // HEIGHT_PX = 8.0
Vec3::new(
    p.x,
    p.y - half_h + elev,
    (tx + ty) as f32 * 0.01 + base_z as f32 * 0.001 + layer,
)
```

---

### 13. Sprites OpenGFX para terreno

#### Terreno plano y pendientes (MP_CLEAR, hierba)

```text
flat_sprite    = SPR_FLAT_GRASS_TILE = 3981
slope_sprite   = 3981 + tileh         (tileh 1–14 → sprites 3982–3995)
```

#### Terreno rough (CLEAR_ROUGH)

```text
flat_sprite   = SPR_FLAT_ROUGH_LAND = 4000
slope_sprite  = 4000 + tileh          (slopes 4001–4014)
```

Variantes adicionales:
- `SPR_FLAT_ROUGH_LAND_1..4` = 4019–4022 (variación aleatoria en suelo plano)

#### Terreno rocoso (CLEAR_ROCKY)

- `SPR_FLAT_ROCKY_LAND_1` = 4023  (rocas tipo 1, plano)
- `SPR_FLAT_ROCKY_LAND_2` = 4042  (rocas tipo 2, plano)

#### Agua (MP_WATER)

- `SPR_FLAT_WATER_TILE` = 4061 (agua plana)
- Costas: 4062–4069 (`SPR_ORIGINALSHORE_START`)

---

### 14. De road bits a sprites

Solo tenemos tres gráficos de carretera base extraídos del NFO:

| `RoadDir` | Bits    | Sprite           |
|-----------|---------|------------------|
| `Tx` (eje X del mapa, `0x0A`) | SW+NE | `road_ty.png` |
| `Ty` (eje Y del mapa, `0x05`) | NW+SE | `road_tx.png` |
| `Both` (ambos ejes) | 0x0F | `road_cross.png` |

> **Intercambio intencional**: `ROAD_X` → sprite `road_ty.png` y viceversa.
> Verificado con capturas sobre `.ottdmap` de partidas reales.

Para la tabla completa `road_bits → offset` en OpenGFX (19 variantes):

```rust
// sprites.rs
pub const ROAD_FLAT_OFFSET_TBL: [u8; 16] =
    [0, 18, 17, 7, 16, 0, 10, 5, 15, 8, 1, 4, 9, 3, 6, 2];
// Sprite final = SPR_ROAD_Y (1332) + ROAD_FLAT_OFFSET_TBL[road_bits & 0x0F]
```

#### Sprites de vía férrea (TrackBits → sprite)

| TrackBits | Sprite ID |
|-----------|-----------|
| Y (2)     | 1011      |
| X (1)     | 1012      |
| UPPER (4) | 1013      |
| LOWER (8) | 1014      |
| RIGHT (32)| 1015      |
| LEFT (16) | 1016      |
| X+Y cruce | 1017      |
| HORZ (UPPER+LOWER) | 1035 |
| VERT (LEFT+RIGHT)  | 1036 |
| Junctions (3+ vías)| 1018–1022 (base) + overlays 1005–1010 |

---

### 15. Relieve (altura) en pantalla

OpenTTD usa **8 píxeles** de desplazamiento vertical por unidad de altura (`TILE_HEIGHT`
en el upstream). En el cliente, `HEIGHT_PX = 8.0` separa visualmente mesetas sin
renderizar pendientes reales.

La **cota** que entra en `tile_pos_half` / `tile_pos` / `overlay_pos` para suelo,
agua, costa y overlays sobre tesela debe ser el **mínimo de las cuatro esquinas**
muestreadas igual que en `GetTileSlopeZ` — en upstream eso es `GetTileZ` (`tile_map.cpp`).
Usar solo `Tile.height` (la altura almacenada en `MAPH` para esa celda, esquina N)
desplaza los sprites en pendiente respecto a los vecinos y produce **huecos**,
costas rotas o teselas en “V”. En openttdrs: `tile_min_corner_height` y
`tile_min_z` en `iso.rs`; el bucle de `setup` en `main.rs` usa `base_z` derivado de
ese mínimo.

El orden Z mezcla `(tx + ty)` con un término en la cota base (`base_z`) para reducir parpadeos
entre teselas a distinta cota:

```rust
z = (tx + ty) as f32 * 0.01 + base_z as f32 * 0.001 + layer
```

`layer` es un pequeño offset usado para priorizar overlays sobre el suelo.

---

### 16. Carga de partidas (.sav)

#### Formatos de compresión

| Magic | Compresión | Notas                          |
|-------|------------|--------------------------------|
| `OTTZ`| zlib       | Más común en versiones modernas |
| `OTTX`| lzma/xz    | Compresión alternativa         |
| `OTTN`| ninguna    | Para debug                     |
| `OTTD`| LZO        | Formato antiguo; no soportado  |

La versión del savegame está en los bytes 4–5 (big-endian u16) del header sin comprimir.
Los savegames modernos usan versión ≥ 300.

#### Estructura del stream de chunks

```
[chunk_id: 4 bytes BE ASCII]
[m: 1 byte]  → chunk_type = m & 0x0F
  CH_RIFF          = 0 → tamaño y datos inline
  CH_ARRAY         = 1 → secuencia gamma+datos, termina con gamma=0
  CH_SPARSE_ARRAY  = 2 → igual, con índice disperso
  CH_TABLE         = 3 → igual que ARRAY pero primer elemento = header
  CH_SPARSE_TABLE  = 4 → igual que SPARSE_ARRAY con header
[datos del chunk...]
```

##### Gamma encoding (SlReadSimpleGamma)

Utilizado para longitudes de elementos en todos los tipos array/table:

```
0xxxxxxx         → 7 bits (1 byte)
10xxxxxx xx      → 14 bits (2 bytes)
110xxxxx xx xx   → 21 bits (3 bytes)
1110xxxx xx xx xx → 28 bits (4 bytes)
11110000 xx xx xx xx → 32 bits (5 bytes)
```

#### Parseo de MAPS (dimensiones)

`MAPS` es un `CH_TABLE` con campos `dim_x` y `dim_y` (SLE_FILE_U32, big-endian).
Si `MAPS` no se encuentra (savegames muy antiguos), se infieren las dimensiones
asumiendo mapa cuadrado de potencia de 2 desde el tamaño de `MAPT`.

---

### 17. Import nativo en openttdrs (`openttdrs-core/src/sav/`)

Además de `parse_sav.py` → `.ottdmap`, el cliente puede cargar `.sav` directamente
(`sav::load` → `GameState::from_sav_game`). Esto importa mapa, estaciones, industrias,
ciudades, vehículos con órdenes y reloj (`DATE`).

**Export:** `sav::save` / `sav::save_to_bytes` escriben un `.sav` mínimo (OTTZ,
versión 350: planos de mapa + `DATE` + `PLYR`). La UI guarda `.sav` por defecto;
usar sufijo `.json` para el save nativo completo. Detalle y handoff:
[ROADMAP_SAV_EXPORT.md](PLANIFICACION.md#export-sav).

#### Qué se importa hoy

| Chunk / dato | Estado |
|--------------|--------|
| MAP* (mapa completo) | ✅ |
| STNN (estaciones, waypoints) | ✅ |
| CITY, INDY, PLYR, DATE | ✅ |
| VEHS (tren/bus/camión cabeza) | ✅ |
| ORDL / ORDR (órdenes goto estación/waypoint) | ✅ |
| Flags **carga completa** / **no descargar** en órdenes | ✅ (bits `Order::flags`) |
| Barcos, aviones, efectos | ❌ omitidos |
| Tipos de señal en MAP* (`m2`, `m3`, `m3hi`, `RAIL_TILE_SIGNALS`) | ✅ block/entry/exit/combo/path/oneway |
| Reservas PBS en runtime tras import | ❌ no se reconstruyen (solo bits en mapa) |
| Lógica presignal completa (`UpdateSignalsOnSegment`) | ❌ parcial |
| Condicionales, depósito en órdenes, refit | ❌ omitidos |
| Dinero `PLYR` en saves muy antiguos (v211) | ⚠️ puede salir `0` |
| Vehículos en depósito sin vía contigua | ⚠️ se fuerzan `running` y snap a red cercana |

#### Post-import (normalización)

Tras cargar, `from_sav_game` puede:

- Corregir `RailDepot` mal tipados en MAPT antiguo.
- Normalizar `TrackBits` y puentear huecos colineales en vía.
- Poner `running=true` en vehículos con órdenes (saves parados en depósito).
- Reubicar (`snap`) vehículos a la red ferroviaria/carretera más cercana.

#### Órdenes y flags (`order_base.h`)

En ORDL/ORDR cada orden tiene `type`, `dest` y `flags`:

- **Unload** (bits 0–2): `NoUnload = 4` → el vehículo no descarga en esa parada.
- **Load** (bits 4–6): `FullLoad = 2`, `FullLoadAny = 3` → espera llenar antes de ir a la siguiente orden.

#### Tests de regresión

```bash
cargo test -p openttdrs-core --test sav_load_stationlist
cargo test -p openttdrs-core --test sav_load_rail_saves
cargo test -p openttdrs-core --test golden_rail_signals
cargo test -p openttdrs-core sav::orders::
```

Fixtures: `crates/openttdrs-core/tests/fixtures/demo_openttd.sav` (sintético con ORDL;
regenerar con `scripts/gen_demo_sav.py`). `rail_signals_mixed.sav` (señales 0–5 en vía;
regenerar con `scripts/gen_rail_signals_sav.py`). Golden de encoding/sprites:
`crates/openttdrs-core/tests/fixtures/parity/rail_signals_golden.json`. Partidas reales bajo `save/` son opcionales en local.

#### Limitaciones conocidas (señales al importar)

- Los **tipos** (`SignalType` 0–5) y máscaras presente/estado se leen del mapa; el **render** del cliente los respeta.
- Las **reservas PBS** guardadas en `m2_hi` (bits 8–11 del `m2()` de 16 bits) **no** se aplican como estado de simulación al cargar; `update_train_reservations` las recalcula en juego.
- **Presignals** (entry/exit/combo): el tipo se importa, pero la lógica de segmento upstream no replica aún `UpdateSignalsOnSegment` de OpenTTD.
- Colocación vía `PlaceRailSignal` solo expone block/path/path oneway; entry/exit/combo en saves vienen del `.sav`, no del menú de construcción.

- Saves sin red conectada a destinos de órdenes: trenes no se mueven (esperado).
- `stationlist-test.sav`: casi sin vía; sirve para buses/órdenes, no para trenes en vía.
- Partidas con muchos vehículos no parseados (SLV reciente, tipos omitidos) pueden quedar vacías de tráfico.
- El import no sustituye a `.ottdmap` para TNBP/JGR ni para render 100 % fiel; usar ambos pipelines según el caso.

---

### 18. Referencias en el código fuente de OpenTTD

Bajo `reference/openttd-upstream/src/`:

| Archivo              | Contenido relevante                                  |
|----------------------|------------------------------------------------------|
| `tile_map.h`         | `GetTileType`, acceso a altura                       |
| `road_map.h`         | `GetRoadBits`, `GetRoadTileType`, `GetCrossingRoadAxis` |
| `road_func.h`        | `DiagDirToRoadBits`, `AxisToRoadBits`                |
| `road_type.h`        | `RoadBit`, `ROAD_X`, `ROAD_Y`                        |
| `rail_map.h`         | `GetTrackBits`, `GetRailTileType`, TrackBits         |
| `track_type.h`       | Constantes `TRACK_BIT_*`                             |
| `industry_map.h`     | `GetCleanIndustryGfx` (9 bits desde m5+m6)          |
| `clear_map.h`        | `GetClearGround`, `GetClearDensity`                  |
| `town_map.h`         | `GetCleanHouseType`, `GetHouseBuildingStage`, `IsHouseCompleted` |
| `object_map.h`       | `GetObjectType` (ObjectType desde array OBJS)        |
| `tunnelbridge_map.h` | Dirección y tipo de transporte                       |
| `table/town_land.h`  | `_town_draw_tile_data`: sprite por HouseID y stage   |
| `saveload/map_sl.cpp`| Registro de chunks `MAPT`, `MAPH`, `MAPO`→MAP1, etc. |

#### Nota sobre town_land.h y los sprites de casas

El array `_town_draw_tile_data` tiene una entrada por `(HouseID * 4 + stage)`.
Los sprite IDs están en hexadecimal (p.ej. `0x58d` = 1421). Los ground sprites especiales:

| Constante             | ID decimal | Descripción              |
|-----------------------|------------|--------------------------|
| `SPR_FLAT_BARE_LAND`  | 3924       | Hierba plana (grass)     |
| `SPR_FLAT_GRASS_TILE` | 3943       | Hierba con flores        |
| `SPR_CONCRETE_GROUND` | 1311       | Suelo de concreto        |

---

### 20. Terraform manual y autoslope (T1–T3)

Implementación en `crates/openttdrs-core/src/command/terraform.rs` y toolbar **Paisaje**
del cliente. Referencia upstream: `terraform_cmd.cpp`, `terraform_gui.cpp`.

#### Comandos

| Comando | Efecto |
|---------|--------|
| `RaiseLand` | Sube la esquina norte de la tesela clicada (+ propagación diagonal) |
| `LowerLand` | Baja la esquina; a `z=0` en hierba/bosque → `TileKind::Water` |
| `LevelLand` | Rectángulo (`from`→`to`) con modo `Level` / `Raise` / `Lower` |

#### Coste

- Base: `TERRAFORM_BASE_PRICE` (= `Price::Terraform` normalizado, **500** £/esquina en tick 0).
- Inflación de construcción: [`terraform_cost_per_corner(tick)`](../crates/openttdrs-core/src/economy.rs)
  con `inflation_prices_factor` (~0,3 %/año simulado).

#### Qué se puede terraformar (manual)

| Tesela | Manual (herramienta paisaje) | Autoslope al construir |
|--------|------------------------------|-------------------------|
| Hierba / bosque | ✅ | ✅ (nivela a `GetTileZ`) |
| Agua lisa (`z=0`, plana) | Elevar → hierba | ❌ |
| Carretera / vía / estación / industria / casa | ❌ `TileNotTerraformable` | ❌ (no aplana encima de infra) |

**Política T3.2:** el terraform manual **no** demuele vías ni carreteras; hay que usar
dinamita / quitar vía antes. El **autoslope** solo actúa en hierba/bosque pendiente al
colocar `PlaceRoad*` / `PlaceRail*`, cobrando el terraform antes del coste de la vía.

#### Autoslope (T3.3)

Al colocar carretera o vía en tesela inclinada de hierba/bosque, el core:

1. Iguala las cuatro esquinas al mínimo (`FOUNDATION_LEVELED` / `GetTileZ`).
2. Cobra terraform por esquina modificada.
3. Coloca la infraestructura en tesela plana.

Preview HUD usa `check_rail_trackbits_with_autoslope` para vías que serían inválidas en
pendiente pero válidas tras nivelar.

#### Buy land (T3.4 / T4.1)

Comando **`BuyLand`** / **`BuyLandArea`** en `command/buy_land.rs`. Marca la tesela como
objeto de mapa (`mapt = MP_OBJECT`, `m5 = OBJECT_TYPE_OWNED_LAND` = 2), sprite
`object_bought_land.png` en el cliente.

| Regla | Comportamiento |
|-------|----------------|
| Terreno válido | Hierba o bosque sin otro objeto |
| Coste | `BUY_LAND_BASE_PRICE` (50 £) × inflación de construcción |
| Área | Arrastre en toolbar **Paisaje → Comprar terreno**; solo compra teselas válidas |
| Errores | `LandAlreadyOwned`, `CannotBuyLandHere`, `InsufficientFunds` |

No impide construir encima (paridad parcial con OpenTTD; reserva de terreno completa pendiente).

#### Limitaciones conocidas

- Climas / desierto / nieve: conversión de tipos MVP (hierba/agua); saves importados
  conservan `mapt`/`m5` originales.
- Túneles/puentes: terraform rechazado si la tesela es boca (validación transporte).

---

### 19. Resumen de archivos del proyecto

| Archivo | Rol |
|---------|-----|
| `scripts/descargar_graficos.sh` | Descarga OpenGFX y extrae sprites a `assets/opengfx/tiles/`; soporta `ogfx1_base00.png` y `ogfx1_base01.png` |
| `scripts/parse_sav.py` | `.sav` → `.ottdmap` v3; resuelve OBJS para MP_OBJECT |
| `crates/openttdrs-core/src/map.rs` | `Tile`, `Map`, `from_ottd_binary` |
| `crates/openttdrs-client/src/iso.rs` | Proyección isométrica, `compute_tileh`, `SLOPE_HALF_H` |
| `crates/openttdrs-client/src/sprites.rs` | `HOUSE_DRAW_DATA` (128 casas), `INDUSTRY_GFX_DATA`, road/rail bits |
| `crates/openttdrs-client/src/main.rs` | Sistema de render Bevy: teselas, overlays, cámara |
| `docs/GRAFICOS.md` | Extracción / isometría (+ anexos archive) |
| `docs/archive/SPRITES_OPENGFX_COMPLETO.md` | Catálogo IDs OpenGFX (histórico) |
| `docs/archive/INDUSTRIAS_OPENGFX.md` | Sprites industrias (histórico; preferir código) |
| `docs/ARCHITECTURE.md` | Arquitectura general del proyecto |

## Señales ferroviarias

<!-- fuente: SENALES_FERROVIARIAS.md -->

Guía de **tipos, comportamiento, codificación en mapa y plan de paridad** para señales ferroviarias. Fuentes oficiales y código upstream; estado actual del crate `openttdrs-core` / cliente.

**Referencias oficiales**

| Fuente | URL / ruta |
|--------|------------|
| Manual jugador — tipos y uso | [wiki.openttd.org/Manual/Signals](https://wiki.openttd.org/en/Manual/Signals) |
| Tutorial construcción (toolbar, arrastre, convertir) | [wiki.openttd.org/Manual/Building signals](https://wiki.openttd.org/en/Manual/Building%20signals) |
| Codificación en tesela (`m2`–`m5`) | [docs/landscape.html](https://github.com/OpenTTD/OpenTTD/blob/master/docs/landscape.html) (clase Railway) |
| Tipos y estados (`SignalType`, `SignalVariant`, `SignalState`) | [`src/signal_type.h`](https://github.com/OpenTTD/OpenTTD/blob/master/src/signal_type.h) |
| API mapa ferroviario | [`src/rail_map.h`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_map.h) |
| Actualización rojo/verde (bloques y presignals) | [`src/signal.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/signal.cpp) |
| Colocación bajo cursor | [`src/rail_gui.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_gui.cpp) (`GenericPlaceSignals`) |
| Dibujo sprites | [`src/rail_cmd.cpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/rail_cmd.cpp) (`DrawSignals`) |
| PBS en pathfinder | [`src/pathfinder/yapf/yapf_costrail.hpp`](https://github.com/OpenTTD/OpenTTD/blob/master/src/pathfinder/yapf/yapf_costrail.hpp) |
| Histórico YAPP (conceptos PBS) | [wiki Yet Another PBS Patch](https://wiki.openttd.org/en/Archive/Manual/Yet%20Another%20PBS%20Patch) |

> El manual de jugador advierte que **path signals son el estándar desde hace años**; block y presignals se mantienen por compatibilidad con saves antiguos ([Signals](https://wiki.openttd.org/en/Manual/Signals)).

---

### 1. Resumen para el jugador (OpenTTD oficial)

Las señales evitan colisiones y ayudan a elegir ramas hacia el destino. **Waypoints** sirven para forzar rutas concretas; las señales no sustituyen órdenes de ruta.

| Familia | Tipos | Idea central |
|---------|-------|--------------|
| **Path (PBS)** — recomendado | Path, PathOneWay | Reserva un **camino** hasta la siguiente posición segura de espera; varios trenes pueden compartir el mismo “bloque” si sus rutas no chocan. |
| **Block** — legado TTD | Block (two-way / one-way) | Un **bloque** = todo el tramo alcanzable hasta la siguiente señal; rojo si **cualquier** parte del bloque está ocupada. |
| **Presignal** — legado | Entry, Exit, Combo | Block + lógica extra: la **Entry** solo verde si hay al menos una **Exit** verde en el bloque siguiente. |

**Posiciones seguras de espera** (path signals): delante de otra señal, depósito o fin de vía — **no** inmediatamente detrás de un cruce (bloquearía la unión).

**Variante visual** (no cambia la lógica): eléctrica vs semáforo (`SignalVariant`); el juego puede colocar semáforos automáticamente antes de cierto año.

---

### 2. Tipos en código (`signal_type.h`)

#### 2.1 `SignalType` — comportamiento

| Valor | Nombre | `m2` (3 bits) | Función |
|------:|--------|---------------|---------|
| 0 | **Block** | `000` | Señal de bloque clásica. Verde si el bloque **posterior** (hasta la siguiente señal en esa vía) está libre. Soporta **two-way** (bidireccional) y **one-way** (solo entra por el lado que mira). |
| 1 | **Entry** | `001` | Presignal de **entrada**: verde solo si el bloque siguiente tiene al menos una **Exit/Combo** verde además de estar libre según reglas de bloque. |
| 2 | **Exit** | `010` | Presignal de **salida**: como block hacia delante, pero su estado **alimenta** entradas/combos aguas arriba. |
| 3 | **Combo** | `011` | Presignal **combo**: actúa como exit del bloque anterior y entry del siguiente; permite árboles de presignals en estaciones ramificadas. |
| 4 | **Path** | `100` | Path signal **bidireccional por detrás**: por la espalda se ignora o se penaliza en pathfinder; roja hasta reservar camino. |
| 5 | **PathOneWay** | `101` | Path signal **de sentido único**: no se puede pasar por detrás (equivalente a señal permanente en rojo en sentido contrario). |

Helpers en `rail_map.h`:

- `IsPbsSignal` → Path o PathOneWay  
- `IsPresignalEntry` → Entry o Combo  
- `IsPresignalExit` → Exit o Combo  
- `IsOnewaySignal` → todo excepto Path (block one-way, presignals, path one-way)

#### 2.2 `SignalVariant` — apariencia

| Valor | Nombre | Bits `m2` |
|------:|--------|-----------|
| 0 | Electric | bit 3 (señales 2–3) o bit 7 (señales 0–1): **0** |
| 1 | Semaphore | mismo bit: **1** |

En teselas Horz/Vert hay **dos grupos** de tipo/variante: pistas Upper/Left/X/Y usan bits 0–3; Lower/Right usan bits 4–7 (`GetSignalType` / `GetSignalVariant` en `rail_map.h`).

En el cliente, la ventana **Señales** permite elegir explícitamente
**Eléctrica** o **Semáforo** antes de colocar o arrastrar señales. El cambio
solo afecta su aspecto: conserva el mismo tipo de señal y lógica PBS. Sobre
una señal existente, `Ctrl+Shift+clic` alterna la variante sin modificar su
orientación, tipo o estado rojo/verde. La preferencia
`semaphore_build_before` define la variante inicial al abrir la herramienta:
antes de ese año usa semáforos y desde ese año, señales eléctricas.

#### 2.3 `SignalState`

| Valor | Significado en simulación |
|------:|---------------------------|
| Red | El tren no puede cruzar la señal en ese sentido. |
| Green | Puede cruzar (block: bloque libre; presignal: condiciones cumplidas; path: reserva válida). |

---

### 3. Comportamiento detallado por tipo

#### 3.1 Block signal (`SignalType::Block`)

**Bloque:** todas las teselas de vía **alcanzables** desde la señal, siguiendo la vía, hasta la **próxima señal** en esa dirección (incluye ramas del bloque aunque el tren vaya por otra rama).

- **Verde:** ningún tren ocupa ninguna tesela del bloque.  
- **Rojo:** cualquier tren en cualquier rama del bloque.  
- **Limitación:** en un cruce, aunque la rama que tomará el tren esté libre, si otra rama del mismo bloque está ocupada → rojo (motivo principal de path signals).

**Two-way vs one-way** (solo block y presignals, no path):

- Clic repetido sobre señal existente (sin Ctrl): alterna two-way → one-way → one-way invertida → two-way ([Building signals](https://wiki.openttd.org/en/Manual/Building%20signals)).  
- Codificado en `m3` con `CycleSignalSide` (`rail_map.h`): path signals solo 2 lados; block/presignal 3 (ambos sentidos posibles).

**One-way block:** tren que llega por el lado “prohibido” se para y puede invertir (configurable en advanced settings).

#### 3.2 Presignals (Entry / Exit / Combo)

Flujo típico en estación multivía ([Signals § Legacy Pre-signals](https://wiki.openttd.org/en/Manual/Signals)):

```text
[Entry] ──► bifurcación ──► [Exit] rama A
                         └──► [Exit] rama B
```

| Tipo | Regla de color |
|------|----------------|
| **Exit** | Igual que block hacia adelante; además propaga su estado a entries/combos anteriores. |
| **Entry** | Verde si el bloque inmediato posterior permite entrada **y** existe al menos un exit verde en ese bloque posterior. Si no hay exits designados, se comporta como block normal. |
| **Combo** | Exit para bloques anteriores + entry para bloques posteriores; encadena árboles (entry → combos → exits). |

**Limitaciones oficiales:**

- Un exit verde puede hacer verde la entry aunque **topológicamente** el tren no pueda llegar a esa exit (layout en T).  
- Trenes “perdidos” ignoran exit signals (bug conocido “will not be solved”).  
- En la práctica moderna se sustituye por **path signals**.

Implementación upstream: barrido de bloque en `signal.cpp` (`ProbeSigSeg`, flags `Exit`, `MultiExit`, `Green`, `Train`, …) y `UpdateSignalsAroundSegment` — entry roja si hay exit pero ninguna verde.

#### 3.3 Path signals (`Path`, `PathOneWay`) — PBS / YAPP

**Reserva de camino:** antes de entrar, el tren reserva teselas hasta la siguiente **posición segura** (señal, depósito, fin de vía). Otro tren puede entrar al mismo “bloque” si reserva un camino **disjunto**.

| Aspecto | Path | PathOneWay |
|---------|------|------------|
| Pasar por detrás | Permitido (pathfinder penaliza) | Prohibido (bloqueo duro) |
| Color por defecto | Rojo hasta reserva exitosa | Igual |
| Uso recomendado | Casi todo | Cuando hay que prohibir sentido contrario (p. ej. salida estación) |

**Datos extra en tesela** (`landscape.html`):

- `m2` bits 8–10: pista reservada para PBS (`GetRailReservationTrackBits`).  
- `m2` bit 11: reserva también la pista opuesta (Horz/Vert).  
- `m5` bit 4: reserva PBS en **cruces a nivel** (`HasCrossingReservation`); en vía plana la reserva vive solo en `m2`.

**Pathfinder (YAPF):** penalizaciones por cruzar reserva ajena, pasar path signal por detrás, estación reservada (`yapf_costrail.hpp`: `ReservationCost`, `SignalCost`, `rail_pbs_cross_penalty`, …).

**Advanced settings** (wiki Signals / YAPP): resaltar rutas reservadas, tipo por defecto al construir, ciclo Ctrl+clic, `wait_for_pbs_path`, intervalos de reintento, etc.

---

### 4. Codificación en el mapa (paridad save / `.ottdmap`)

Tesela `MP_RAILWAY` con `RailTileType::Signals` (`m5` bits 6–7 = `01`).

#### 4.1 Hasta 4 señales por tesela

Cada **signal bit** 0..3 corresponde a direcciones concretas según el `Track` (X, Y, Upper, Lower, Left, Right). Tabla completa en [landscape.html — Railway signals](https://github.com/OpenTTD/OpenTTD/blob/master/docs/landscape.html) y en `collect_signal_sprite_ids` (`crates/openttdrs-client/src/sprites/rail.rs`).

| Campo | Bits | Significado |
|-------|------|-------------|
| `m5` | 0–5 | `TrackBits` — qué piezas de vía hay |
| `m5` | 6–7 | `RailTileType` (= Signals) |
| `m5` | 4 | reserva PBS **solo en cruce a nivel** (`HasCrossingReservation`) |
| `m3` | 7–4 | **presente** — bit 1 = señal `n` existe (`GetPresentSignals`) |
| `m4` / `m3hi` | 7–4 | **estado** — bit 1 = verde (`GetSignalStates`; en `.ottdmap` el chunk `M3HI` carga en `m4()`) |
| `m2` | 2–0 | tipo señales 2 y 3 |
| `m2` | 6–4 | tipo señales 0 y 1 |
| `m2` | 3, 7 | variante semáforo/eléctrico (grupos 2–3 y 0–1) |
| `m2` | 8–10 | reserva PBS (track reservado) |
| `m2` | 11 | reserva pista opuesta |

En openttdrs, `Tile.m3hi` = nibble alto de estados (equivalente a `m4()` para señales).

#### 4.2 Colocación: una señal, un carril

En doble vía **Horz** (`UPPER|LOWER`) o **Vert** (`LEFT|RIGHT`), **un clic = una señal** en el carril bajo el cursor. OpenTTD elige la pieza con `GenericPlaceSignals` según `fract_x`, `fract_y`:

| Layout | Regla |
|--------|--------|
| Vert | `RIGHT` si `fract_x <= fract_y`, si no `LEFT` |
| Horz | `UPPER` si `fract_x + fract_y <= 256`, si no `LOWER` |
| X / Y / pieza única | esa pieza directamente |

En openttdrs: `resolve_signal_track` en `rail_signals.rs` (misma lógica). Comando: `PlaceRailSignal(coord, orientation, fract_x, fract_y)`.

**No señales** en: cruces con más de Horz o Vert mezclado incompatible (`tracks_overlap`), puentes, túneles, pasos a nivel (OpenTTD).

#### 4.3 Sprites

`DrawSingleSignal` → IDs OpenGFX; bases `1275` (block eléctrico clásico) y `SPR_SIGNALS_BASE - 16` = `5072` (`OPENTTDRS_SIGNAL_ALT_BASE`) para presignals/PBS (PNG Action5 `rail_5088..5327`). Cliente: `signal_sprite_id`, `collect_signal_sprite_ids`, precarga en `rail_sprite_ids_for_preload`.

---

### 5. Herramientas de construcción (OpenTTD)

| Acción | Comportamiento oficial |
|--------|------------------------|
| Clic | Coloca señal del tipo seleccionado en toolbar |
| Clic en existente | Block: cicla two-way / one-way / dirección; con Ctrl: cicla **tipo** (block → entry → exit → combo → path → …) |
| Arrastre | Línea de señales espaciadas (`signal density`, default cada 4 teselas); desde presignal arrastrando → block en la misma dirección |
| Ctrl + arrastre | Autocolocación hasta estación/señal/bifurcación |
| Bulldozer / R | Quitar señales |
| Signal convert | Convierte tipo al seleccionado en toolbar |
| RMB (openttdrs) | Rotar orientación de colocación (`cycle_signal_facing`) |

Toolbar avanzada vs simplificada: la simplificada solo muestra path signals ([Building signals](https://wiki.openttd.org/en/Manual/Building%20signals)).

---

### 6. Estado actual en openttdrs

| Capacidad | Estado | Módulo |
|-----------|--------|--------|
| Colocar / quitar señal **block eléctrica** unidireccional | ✅ | `command/transport.rs`, `PlaceRailSignal` |
| Preview + toolbar + RMB dirección | ✅ | Pick diagonal cerrado jul 2026 — §11 |
| Carriles X/Y/Upper/Lower/Left/Right | ✅ | `resolve_signal_track`, `fract_x/y` |
| Render presente + rojo/verde | ✅ | `sprites/rail.rs`, `collect_signal_sprite_ids` |
| Sim block simple (bloque hasta siguiente señal, 1 ocupación) | ✅ | `rail_signals.rs` — X/Y + Horz/Vert (exit por carril) |
| Two-way / one-way block | ✅ | `cycle_signal_side_m3` vía `PlaceRailSignal` (clic en señal existente) |
| Semaphore vs electric | ✅ | `default_signal_variant` por año (`SEMAPHORE_BUILD_BEFORE_YEAR` = 1950); setting `gui.semaphore_build_before` no expuesto |
| Tipos Entry / Exit / Combo | ✅ | Colocación + Ctrl ciclo 6 tipos; sim entry exige bloque propio libre **y** algún exit/combo verde |
| Path / PathOneWay + reserva PBS | ✅ | Safe wait, wait/giro, UI `PBS...`, TryReserve BFS. YAPF nativo completo opcional |
| Presignal `UpdateSignalsOnSegment` | ✅ | `_globset` only en sim (`drain` pre/post movimiento); barrido global solo API tests/parity |
| Arrastre línea + densidad | ✅ | `signal_density` default 4; Shift+RMB cicla |
| Bulldozer quita señal (conserva vía) | ✅ | `RemoveRailSignal` vía herramienta Demoler |
| Signal convert + Ctrl ciclo tipos | ✅ | Ctrl+clic: block→entry→exit→combo→path→path oneway (`CycleRailSignalType`) |
| Import `.sav` con PBS/presignals | 🟡 | Encoding y render OK; reservas PBS runtime se recalculan; árboles combo multi-nivel frágiles |

---

### 7. Plan de implementación por fases

Orden sugerido alineado con [ROADMAP_SPRINTS.md](PLANIFICACION.md#sprints-hito-01) y [PARIDAD_OPENTTD.md](PLANIFICACION.md#vista-corta-de-gaps).

#### Fase A — Block completo (S5 cierre) ✅

**Objetivo:** paridad jugable en líneas doble vía con block signals.

1. **Sim en Horz/Vert:** `signal_bits_for_exit` / `signal_exit_dir` por carril; bloque sigue el corredor HORZ/VERT (ocupación por tesela).  
2. **Two-way / one-way:** `cycle_signal_side_m3` embebido en `PlaceRailSignal` (clic en señal del mismo carril).  
3. **Merge `m2` al añadir 2.ª señal** en misma tesela (otro carril).  
4. Tests: Upper≠Lower en Horz, Vert Left, ciclo one/two-way, two-way terminal.

**Archivos:** `rail_signals.rs`, `command/transport.rs`, `build_input/click.rs`, tests en `rail_signals.rs` / `command/tests.rs`.

#### Fase B — UX construcción ✅

1. Arrastre con densidad N (`StationBuildState.signal_density`, default 4; Shift+RMB cicla 1/2/4/8/12/16).  
2. Bulldozer (`Clear`) sobre tesela con señal → `RemoveRailSignal` (conserva vía).  
3. Semaphore automático por año — ✅ `default_signal_variant` (1950); setting GUI opcional pendiente.  
4. Ctrl+clic ciclo de tipo (6 tipos OpenTTD) — ✅.

#### Fase C — Presignals 🟡 (jul 2026)

**Objetivo:** saves antiguos y estaciones legacy.

1. Codificar `SignalType` 1–3 en `m2` al colocar/convertir — ✅ `PlaceRailSignal` acepta 0–5; Ctrl cicla los 6.  
2. Port simplificado de `signal.cpp`:  
   - `ProbeSigSeg` / flags de bloque — 🟡 `explore_sig_segment` (`Exit`/`MultiExit`/`Green`/`MultiGreen`; estación/túnel/puente + wormholes JGR)  
   - `UpdateSignalsOnSegment` + buffer `_globset` — ✅ `enqueue_trains` + reservas PBS + `drain` pre/post movimiento (sin barrido global en tick)  


   - Regla entry: rojo si bloque propio ocupado **o** ningún exit verde — ✅ (cadenas combo + bifurcaciones MultiExit)  
   - Combo bidir MultiExit/MultiGreen — ✅ (`stabilize_combo_presignal_greens`)  
3. Sprites entry/exit/combo (ya en OpenGFX vía `signal_type > 3`) — ✅.  
4. Tests: ciclo 6 tipos + colocación entry/exit/combo; demo estación 2 vías — ✅ encoding; dinámico wiki + combo/túnel/estación + globset.

**No replicar** bugs upstream (lost train ignora exit) salvo paridad explícita.

#### Fase D — Path signals (PBS) — Hito 0.2 🟡 (parcial, jul 2026)

**Objetivo:** comportamiento moderno por defecto.

1. **Reserva:** ✅ estructura por tren (`reserved_steps` + track bits); `m2` bits 8–11 vía `m2_hi`; `m5` bit 4 en **cruces a nivel** (`HasCrossingReservation`). En vía plana la reserva no usa `m5` bit 4 (paridad OpenTTD: ahí va en `m2`).  
2. **Antes de mover tren:** ✅ extensión de reserva a lo largo del `path` hasta **posición segura** (`is_safe_waiting_position`: depósito, block, delante de path, fin de vía) o conflicto.  
3. **Estado señal path:** ✅ verde solo con reserva completa hasta safe wait (`pbs_exit_has_complete_reservation`); path **no** exige verde previa para extender reserva; movimiento exige reserva completa.  
4. **Pathfinder:** ✅ penalización PBS por detrás (`YAPF_PBS_BEHIND_PENALTY`); cruce de reserva (`YAPF_RESERVATION_CROSS_PENALTY`).  
5. **Cliente:** ✅ overlay rutas reservadas (tecla R / `show_pbs_reservations`); default toolbar `SIGTYPE_PATH`.  
6. **PathOneWay:** ✅ bloqueo sentido contrario (`DeadEnd` / `train_blocked_by_signal`).  
7. **Espera / giro:** ✅ `PathfindingSettings` (`wait_for_pbs_path` default 30 días, `path_backoff_interval` 20, `reverse_at_signals`); stuck + giro al timeout (`tick_pbs_wait_and_maybe_reverse`).  
8. **UI settings:** ✅ toolbar **engranaje** (Ajustes) → `Pathfinding / PBS...` (`pathfinding_settings_window.rs`).  
9. **TryReservePath:** ✅ Dijkstra con costes YAPF (`find_path_to_safe_wait`: tile + `YAPF_RESERVATION_CROSS_PENALTY` + sesgo off-path) hasta safe wait; reintento acotado por `path_backoff_interval` vía `should_retry_reservation(wait_counter)` en `compute_train_reservation_with_settings` (255 = off).

**Pendiente:** — (ninguno en Fase C/D señales; barrido global retirado del tick).

Dependencias: pathfinder trenes más fiel (YAPF simplificado o extensión de `pathfinder.rs`).

#### Fase E — Import y regresión ✅ (jul 2026)

1. Fixture save OpenTTD con mezcla block + path + presignals → `rail_signals_mixed.sav` (`scripts/gen_rail_signals_sav.py`).  
2. Golden render/encoding señales → `tests/fixtures/parity/rail_signals_golden.json` + `golden_rail_signals.rs` + test cliente `golden_rail_signal_sprite_texture_ids`.  
3. Limitaciones § import documentadas en [TILES_Y_SAVEGAMES_OPENTTD.md](#tiles-y-savegames-openttd) (§17).  
4. Escenario parity `rail_signals_mixed` + roundtrip JSON en `parity/scenario.rs`.

---

### 8. Mapa código openttdrs ↔ upstream

| Responsabilidad | openttdrs | OpenTTD |
|-----------------|-----------|---------|
| Tipos | (implícito `sig_type=0`) | `signal_type.h` |
| Colocación | `resolve_signal_track`, `place_rail_signal` | `GenericPlaceSignals`, `CmdBuildSignal` |
| Codificación tile | `signal_placement_for_track`, `m2_for_signal` | `SetSignalType`, `SetPresentSignals` |
| Estado verde/rojo block | `update_rail_signal_states` | `UpdateSignalsOnSegment` |
| Bloqueo tren | `train_blocked_by_signal` | `CheckTrainOwnership`, PBS checks |
| Dibujo | `collect_signal_sprite_ids` | `DrawSignals` |
| PBS | `rail_pbs.rs` + `sim_step` | `yapf`, reserva en `train_cmd.cpp` / PBS core |

---

### 9. Criterios de aceptación (definición de “hecho”)

| Nivel | Criterio |
|-------|----------|
| **S5** | Block unidireccional en todos los track bits; sim evita entrar a bloque ocupado; preview/colocación/quitar en Horz/Vert. |
| **Block full** | Two-way en terminal; arrastre densidad 4; semáforo/eléctrico visible. |
| **Presignal** | Entry/exit/combo cambian color como en OpenTTD en layout wiki estación; CI con save fixture. |
| **PBS** | Cruce doble vía con trenes en paralelo sin deadlock; reserva visible; path one-way bloquea sentido contrario. |

---

### 11. Fantasma vs colocación en vía diagonal (cerrado jul 2026)

**Estado:** ✅ cerrado jul 2026 · tap ancla al press + preferencia seed en pick.

#### Síntoma (reporte usuario)

En vías **X/Y diagonales** (tesela plana):

1. El **fantasma** (preview) aparece **sobre el riel**, donde el jugador espera colocar.
2. Al **clic**, la señal queda en una **tesela vecina** (a menudo una casilla al este/sudeste en pantalla), a veces en hierba o con apariencia de “vía nueva”.
3. En casos extremos parece colocarse **doble** (fantasma correcto + resultado en vecino).

OpenTTD usa `GetTileBelowCursor()` + `GenericPlaceSignals` sobre **esa** tesela (`rail_gui.cpp`); no hay búsqueda 5×5.

#### Fix aplicado

1. **Tap vs arrastre:** en `RailSignals`, si el cursor se movió ≤10 px entre press y release, se coloca en `start_tile` + `signal_drag_fract` del press (misma fuente que el fantasma), sin re-pickear el vecino isométrico.
2. **Pick:** si el seed geométrico ya es vía válida y el cursor está cerca del ancla, se prefiere ese seed; desempate favorece el seed sobre vecinos.
3. Test: `pick_mid_diagonal_rail_segment_stays_on_track_tile` en `iso/coords.rs`.

#### Intentos previos (insuficientes solos)

| Cambio | Archivos | Resultado |
|--------|----------|-----------|
| Snap al riel vecino | `world_pos_to_rail_signal_pick` (`iso/coords.rs`) | Mejor cerca del borde; sigue desalineado |
| Offset sub-tesela OpenTTD | `rail_signal_subtile_offset`, `signal_draw_pos` (`sprites/rail.rs`) | Fantasma más alineado al riel; clic sigue en otra tesela |
| Fuente única hover | `HoveredTileCoord` en cursor + preview + click | Misma tesela en teoría; usuario confirma bug persiste |
| Orden ECS | `cursor → ghost → click` (`ui.rs`) | Evita frame distinto; no corrige pick erróneo |
| Desempate vecinos | `rail_signal_pick_better` en pick 5×5 | Empates por métrica isométrica |

#### Hipótesis para la próxima sesión

1. **Pick isométrico vs OpenTTD** — `world_pos_to_tile_coord` puede devolver tesela A mientras el riel visible está en B; el vecindario 5×5 elige B con métrica similar a C.
2. **Fract en tesela equivocada** — `PlaceRailSignal(coord, …, fract_x, fract_y)` calculado respecto a tesela B pero el jugador apunta a A; el core escribe en B (datos) mientras el fantasma se dibuja bien por offset visual.
3. **Paridad `GetTileFromScreenXY`** — portar lógica exacta de `viewport.cpp` (no solo inversa de `iso` + rombo relajado).
4. **Proyección al carril** — tras elegir tesela, proyectar `world_pos` al segmento X/Y dentro del rombo antes de `resolve_signal_track` (como hace el cliente con `_tile_fract_coords` tras fijar tile).
5. **Regresión visual** — captura `OPENTTDRS_MAP_SHOT` con herramienta señales + test que compare `HoveredTileCoord` vs tile bajo sprite fantasma.

#### Archivos clave

| Rol | Ruta |
|-----|------|
| Pick | `crates/openttdrs-client/src/iso/coords.rs` — `world_pos_to_rail_signal_pick` |
| Hover unificado | `crates/openttdrs-client/src/ui/toolbar/build_input/cursor.rs` |
| Preview | `crates/openttdrs-client/src/ui/toolbar/preview/rail_signal.rs`, `preview/mod.rs` |
| Clic | `crates/openttdrs-client/src/ui/toolbar/build_input/click.rs` |
| Comando | `crates/openttdrs-core/src/command/transport.rs` — `place_rail_signal` |
| Dibujo | `crates/openttdrs-client/src/sprites/rail.rs`, `render/tiles/transport.rs` |
| Upstream | `OpenTTD/src/viewport.cpp` (`GetTileFromScreenXY`), `rail_gui.cpp` (`GenericPlaceSignals`) |

#### Criterio de cierre

- Fantasma y señal colocada en la **misma tesela** y **misma posición en pantalla** al clicar sobre un tramo X o Y diagonal (caso GIF usuario jun 2026).
- Test cliente: `pick_mid_diagonal_rail_segment_stays_on_track_tile` (+ anclas) en `iso/coords.rs` — pick del centro del riel = tesela del track.

#### Repro local

```bash
cargo run -p openttdrs-client
## Toolbar → Señales → vía X o Y en diagonal → hover sobre riel → clic
## Comparar tesela del fantasma vs tesela donde aparece el sprite sólido
```

---

### 12. Enlaces internos

- Codificación tiles vía: [TILES_Y_SAVEGAMES_OPENTTD.md §7.1](#tiles-y-savegames-openttd)  
- Sprint plan: [ROADMAP_SPRINTS.md § S5](PLANIFICACION.md#sprints-hito-01)  
- Render / assets: [archive/SP3_AUDIT_SUMMARY.md](archive/SP3_AUDIT_SUMMARY.md), [archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md)  
- Paridad global: [PARIDAD_OPENTTD.md](PLANIFICACION.md#vista-corta-de-gaps)

## Colocación de vías

<!-- fuente: VIAS_FERROVIARIAS_COLOCACION.md -->

Referencia rápida para **Horz / Vert / X / Y**, **autorraíl**, **uniones** y **señales** (junio 2026).

Fuentes OpenTTD: `rail_gui.cpp` (`GenericPlaceSignals`, `GenericPlaceSignals`), `viewport.cpp` (`_tile_fract_coords`).

---

### Herramientas y comandos

| Toolbar | Comando | Comportamiento |
|---------|---------|----------------|
| **Autorraíl** | `PlaceRail` | Infiere pieza con `rail_trackbits_from_neighbors`; **refresca vecinos** (`refresh_rail_neighbors`). |
| **X / Y** | `PlaceRailBits` | Coloca bits fijos (`0x01` / `0x02`) en la **tesela del cursor**. |
| **Horz / Vert** | `PlaceRailBits` | Un carril paralelo por clic (`rail_horz_lane_bit` / `rail_vert_lane_bit` según `fract_x/y`). |
| **Quitar vía** | `RemoveRailBits` | Quita bits; actualiza vecinos y señales compatibles. |

---

### Regla principal (Horz / Vert / X / Y)

**La vía se escribe solo en la tesela bajo el cursor** — la misma que muestra el fantasma.

- **No** se redirige el clic a teselas vecinas con vía existente.
- **Sí** se actualizan uniones en vecinos al colocar:
  - **X / Y** (`PlaceRailBits` con diagonal): cruce perpendicular en la tesela vecinal (`propagate_rail_diag_to_neighbors`).
  - **Horz / Vert** (carril paralelo): empalme T en la vía E–O / N–S vecina (`refresh_rail_neighbors_after_place` + `junction_merge_for_neighbor`).

Las curvas en la **misma tesela** siguen siendo por fusión de bits al clic; ramificar con **autorraíl** infiere la pieza completa.

Implementación: `place_rail_bits` en `crates/openttdrs-core/src/command/transport/rail.rs`, `propagate_rail_diag_to_neighbors` en `shared.rs`.

---

### Autorraíl y vecinos

`PlaceRail` y `RemoveRail` sí ejecutan `refresh_rail_neighbors`:

1. `refresh_rail_neighbors_after_place` — fusiona piezas de unión en vecinos **perpendiculares** cuando corresponde (`junction_merge_for_neighbor`).
2. `refresh_rail_trackbits` — re-infiere X/Y/cruces; **no** toca teselas solo paralelas (Horz/Vert sin diagonal) para no destruir carriles al arrastrar.

---

### Señales (Sprint 5)

| Aspecto | Detalle |
|---------|---------|
| Pick | `world_pos_to_rail_signal_pick` — vecindario 5×5; hover unificado en `HoveredTileCoord` |
| Dibujo | `rail_signal_subtile_offset` — tabla `SignalPositions` (`DrawSingleSignal`, OpenTTD) |
| Colocación en cruce | `write_normal_rail_tile` conserva señales al fusionar Y+X → cruce |

**Pick diagonal (cerrado jul 2026):** fantasma y clic alineados — ver [SENALES_FERROVIARIAS.md §11](#señales-ferroviarias).

Ver [SENALES_FERROVIARIAS.md](#señales-ferroviarias).

---

### Tests de regresión

```bash
cargo test -p openttdrs-core parallel_
cargo test -p openttdrs-core place_rail_bits_preserves_signal
```

Casos cubiertos: extensión Horz en línea; segundo carril en tesela vacía; Vert al este de Y sin modificar la Y; señal conservada al añadir segunda diagonal en la misma tesela.

---

### Backlog

- Junctions en pendiente — ✅ S3 (`sp3_visual_checklist_sloped_junction_sprite_ids`).
- Sim de señales en carriles paralelos (hoy probada sobre todo en X/Y).

## Waypoints rail (handoff)

<!-- fuente: HANDOFF_WAYPOINTS_RAIL.md -->

**Estado (jul 2026):** vía de fondo 1011/1012 + 4 capas ogfx2 (cuerpo + toldos CC).

**Posicionamiento correcto:** TILE_SEQ `dy=13` (eje X) / `dx=13` (eje Y) **y**
mitades este con el **mismo xrel/yrel** que el ancla oeste
(`rail_waypoint_layer_meta`). El tamaño `w`/`h` sigue siendo el del PNG este.

### Por qué no xrel NFO este (−8) + dy=13

Sumar el xrel NFO de la mitad este **y** TILE_SEQ dy=13 duplica el offset →
una caseta en la vía y otra en la hierba.

### Por qué no dy=0 + xrel distintos (−30/−8)

Sin TILE_SEQ las mitades quedan en la misma fila de pantalla pero con anclas
independientes → forma de **V** sobre la vía (capturas jul 2026).

### Regenerar

```bash
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py
cargo test -p openttdrs-client rail_waypoint
```

### Archivos

| Archivo | Rol |
|---------|-----|
| `sprites/station.rs` | `RAIL_WAYPOINT_SEQ_*` + `rail_waypoint_layer_meta` |
| `render/tiles/objects.rs` | ground + overlays |
| `ui/toolbar/preview/rail_waypoint.rs` | Fantasma |
| `scripts/gen_rail_waypoint_sprites.py` | PNG #19–26 |
