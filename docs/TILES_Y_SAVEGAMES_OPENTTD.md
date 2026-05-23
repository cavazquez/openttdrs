# Teselas, mapas y savegames de OpenTTD — Referencia técnica para openttdrs

Este documento recoge **todo lo aprendido** al cargar mapas reales (`.sav` → `.ottdmap`) y
al renderizar terreno, carreteras, vías, industrias y objetos. Complementa
[SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) y [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md).

Código relevante:
- `scripts/parse_sav.py` — conversor `.sav` → `.ottdmap`
- `crates/openttdrs-core/src/map.rs` — struct `Tile`, `Map::from_ottd_binary`
- `crates/openttdrs-client/src/iso.rs` — proyección isométrica y pendientes
- `crates/openttdrs-client/src/sprites.rs` — constantes y lógica de sprites

---

## Índice

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
17. [Referencias en el código fuente de OpenTTD](#17-referencias-en-el-código-fuente-de-openttd)
18. [Resumen de archivos del proyecto](#18-resumen-de-archivos-del-proyecto)

---

## 1. Byte MAPT (tipo de tesela)

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

### Por qué guardamos `mapt` en `Tile`

El byte `m5` tiene significados completamente distintos según el tipo de tesela.
Sin el MAPT crudo no se puede decodificar `m5` correctamente (road bits, TrackBits,
gfx de industria, etc. son campos incompatibles según el tipo). Por eso `Tile` incluye
`mapt: u8` además de `kind`.

---

## 2. Formato .ottdmap v1→v5

Binario producido por `scripts/parse_sav.py` y consumido por `Map::from_ottd_binary`.

### Cabecera común (todas las versiones)

| Offset | Tamaño | Contenido              |
|--------|--------|------------------------|
| 0–3    | 4 B    | Magic `MAPO` (0x4D41504F, LE) |
| 4–7    | 4 B    | `width` u32 LE         |
| 8–11   | 4 B    | `height` u32 LE        |

Tras el header siguen secciones de `W×H` bytes/palabras en orden `i = y*width + x`
(x varía rápido, igual que en los chunks MAPT/MAPH de OpenTTD).

### Secciones por versión

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

### Footers opcionales (v5+, tras los planos denses)

Orden: **INDP** (si hay datos de industrias), **STNN** (si hay blob), **TNBP** (si hay blob de túnel/puente), **STXY** (lista de teselas `MP_STATION` derivada del mapa en `parse_sav.py`). Cada footer es: 4 bytes ASCII de magic + `u32` LE `len` + `len` bytes de payload (excepto **STXY**, donde `len` es el número de pares y el cuerpo es `len` × 4 bytes: `u16` x, `u16` y).

| Magic | Contenido |
|-------|-----------|
| `INDP` | `u32` count; luego `count` × (`u16` industry_index, `u8` industry_type) |
| `STNN` | Blob crudo del chunk `STNN` (CH_TABLE o CH_ARRAY según versión del save) |
| `TNBP` | Blob del primer chunk entre TNBP, TBUS o TUNN presente en el save |
| `STXY` | `u32` count; luego `count` × (`u16` x, `u16` y) — teselas con tipo `MP_STATION` en MAPT |

`Map::from_ottd_binary` en **openttdrs-core** solo lee los planos densos hasta v5; ignora cualquier byte extra (footers).

### NewGRF y alcance del export

Exponer MAP7, MAP8, M3HI/M3LO y blobs **no** sustituye un stack NewGRF completo: siguen siendo necesarios los archivos `.grf`, definiciones de acciones/sprites y la lógica del cliente para interpretar bits y tablas fuera del mapa denso.

### Evolución v1 → v2 → v3

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

### Detección de versión en el lector Rust

```rust
// from_ottd_binary (map.rs)
let has_m1 = data.len() >= 12 + n * 4;   // v2+
let has_m6 = data.len() >= 12 + n * 5;   // v3+
let has_m8 = data.len() >= 12 + n * 5 + n * 2; // v3+
let has_m3 = data.len() >= 12 + n * 8;   // v4+ (hasta fin de sección M3LO inclusive)
let has_v5_planes = data.len() >= 12 + n * 11; // v5+ (MAP2, MAP7, M3HI tras M3LO)
```

### MP_WATER, MAP5 y costa en el cliente

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

## 3. Nombres reales de chunks en OpenTTD

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

### Por qué m1 era todo ceros antes de v2

El script en v1 buscaba `MAP1` como nombre de chunk, pero el nombre real es `MAPO`.
Al no encontrarlo, el chunk quedaba vacío y `m1` se rellenaba de ceros. La corrección
fue buscar `chunks.get('MAPO')` en lugar de `'MAP1'`.

```python
# parse_sav.py (corrección v2)
map1 = chunks.get('MAPO', b'')  # MAPO = MAP1 (owner/datos de tesela 1)
map6 = chunks.get('MAPE', b'')  # MAPE = MAP6
map8 = chunks.get('MAP8', b'')  # MAP8 = HouseID (2 bytes/tile)
```

---

## 4. Struct Tile en Rust

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

## 5. Byte m5 en MP_ROAD

Fuente: `reference/openttd-upstream/src/road_map.h`.

### Subtipo (bits 6–7)

```text
RoadTileType = (m5 >> 6) & 0x3
```

| Valor | `RoadTileType` | Significado            |
|------:|----------------|------------------------|
| 0     | `Normal`       | Carretera normal        |
| 1     | `Crossing`     | Cruce a nivel (vía + carretera) |
| 2     | `Depot`        | Depósito de carreteras  |

### Caso Normal (subtipo 0)

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

### Caso Crossing (subtipo 1) — error frecuente

En un cruce a nivel, los bits 0–3 **no** son road bits estándar.
El eje de la carretera está en **bit 0** (`GetCrossingRoadAxis`):

- `AXIS_X` (0) → carretera como `ROAD_X` (0x0A).
- `AXIS_Y` (1) → carretera como `ROAD_Y` (0x05).

**Bug histórico en openttdrs:** tratar el cruce como carretera normal y leer road bits
en 0–3 producía valores absurdos; el código caía en fallback por vecinos, rompiendo
trazados rectos en mapas con cruces a nivel.

### Caso Depot (subtipo 2)

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

## 6. MP_TUNNELBRIDGE

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

## 7. MP_RAILWAY: TrackBits en m5

Fuente: `reference/openttd-upstream/src/rail_map.h`.

### RailTileType (bits 6–7 de m5)

```text
RailTileType = (m5 >> 6) & 0x3
```

| Valor | Constante           | Significado          |
|------:|---------------------|----------------------|
| 0     | `RAIL_TILE_NORMAL`  | Vía normal           |
| 1     | `RAIL_TILE_SIGNALS` | Vía con señales      |
| 3     | `RAIL_TILE_DEPOT`   | Depósito de trenes   |

### TrackBits (bits 0–5 de m5)

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

---

## 8. MP_INDUSTRY: gfx de 9 bits

Fuente: `reference/openttd-upstream/src/industry_map.h`, `GetCleanIndustryGfx`.

### Por qué m5 solo (8 bits) no alcanza

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

### Tabla de rangos por industria

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

### m1 en MP_INDUSTRY

Bits 0–6 de m1 = índice de la industria en el array global (`IndustryID`), útil para
agrupar tiles de la misma planta.

---

## 9. MP_OBJECT: resolución de ObjectType desde OBJS

### Problema en savegames modernos (v300+)

En OpenTTD moderno, el chunk `MAP5` para teselas `MP_OBJECT` **no contiene el ObjectType**;
contiene los bits altos del `ObjectID` (instancia específica del objeto). El `ObjectType`
real (qué clase de objeto es: transmisor, faro, HQ…) está almacenado en el chunk `OBJS`.

Si se lee `m5` directamente para MP_OBJECT se obtiene un valor incorrecto que no corresponde
a ningún tipo semántico útil.

### Estructura del chunk OBJS (CH_TABLE / CH_SPARSE_TABLE)

`OBJS` es un array disperso donde cada elemento representa un objeto colocado en el mapa.
Los campos relevantes parseados en `parse_sav.py`:

| Campo            | Tipo  | Significado                              |
|------------------|-------|------------------------------------------|
| `location.tile`  | U32   | Índice lineal del tile base del objeto   |
| `type`           | U16   | ObjectType (0 = Transmisor, 1 = Faro…)  |

### Tipos conocidos de ObjectType

| ObjectType | Nombre              |
|------------|---------------------|
| 0          | `OBJECT_TRANSMITTER` (antena) |
| 1          | `OBJECT_LIGHTHOUSE`  (faro)   |
| 2+         | HQ de empresa, estatuas, etc. |

### Cómo parse_sav.py resuelve el ObjectType

```python
# parse_sav.py
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

## 10. MP_HOUSE: HouseID en m8

### Por qué m5 no alcanza

`MP_HOUSE` tiene más de 255 tipos de casas en OpenTTD (especialmente con NewGRF).
El `HouseID` es un **u16** y se almacena en el chunk `MAP8` (2 bytes little-endian
por tesela).

```text
HouseID = m8  (u16, little-endian)
```

El byte `m5` en MP_HOUSE guarda otras cosas (etapa de construcción, etc.), **no** el
HouseID.

### Etapa de construcción (`m3` + `m5`)

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

### Implementación en openttdrs

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

### Tabla de tipos de casa temperate (stage 3)

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

## 11. MP_CLEAR: ClearGround en m5

Fuente: `reference/openttd-upstream/src/clear_map.h`.

### ClearGround (bits 2–4 de m5)

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

### Densidad de hierba (bits 0–1 de m5 cuando ClearGround = GRASS)

```text
grass_density = m5 & 0x3   // 0=bare, 1=1/3, 2=2/3, 3=full
```

Esto determina qué sprite base usar:
- 0 → `SPR_FLAT_BARE_LAND` (3924)
- 1 → `SPR_FLAT_1_THIRD_GRASS_TILE` (3943)
- 2 → `SPR_FLAT_2_THIRD_GRASS_TILE` (3962)
- 3 → `SPR_FLAT_GRASS_TILE` (3981)

---

## 12. Sistema de pendientes (slopes)

### Concepto: tileh como bitmask de esquinas elevadas

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

### Valores de tileh válidos (0–14)

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

### SLOPE_HALF_H: ajuste vertical por pendiente

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

## 13. Sprites OpenGFX para terreno

### Terreno plano y pendientes (MP_CLEAR, hierba)

```text
flat_sprite    = SPR_FLAT_GRASS_TILE = 3981
slope_sprite   = 3981 + tileh         (tileh 1–14 → sprites 3982–3995)
```

### Terreno rough (CLEAR_ROUGH)

```text
flat_sprite   = SPR_FLAT_ROUGH_LAND = 4000
slope_sprite  = 4000 + tileh          (slopes 4001–4014)
```

Variantes adicionales:
- `SPR_FLAT_ROUGH_LAND_1..4` = 4019–4022 (variación aleatoria en suelo plano)

### Terreno rocoso (CLEAR_ROCKY)

- `SPR_FLAT_ROCKY_LAND_1` = 4023  (rocas tipo 1, plano)
- `SPR_FLAT_ROCKY_LAND_2` = 4042  (rocas tipo 2, plano)

### Agua (MP_WATER)

- `SPR_FLAT_WATER_TILE` = 4061 (agua plana)
- Costas: 4062–4069 (`SPR_ORIGINALSHORE_START`)

---

## 14. De road bits a sprites

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

### Sprites de vía férrea (TrackBits → sprite)

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

## 15. Relieve (altura) en pantalla

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

## 16. Carga de partidas (.sav)

### Formatos de compresión

| Magic | Compresión | Notas                          |
|-------|------------|--------------------------------|
| `OTTZ`| zlib       | Más común en versiones modernas |
| `OTTX`| lzma/xz    | Compresión alternativa         |
| `OTTN`| ninguna    | Para debug                     |
| `OTTD`| LZO        | Formato antiguo; no soportado  |

La versión del savegame está en los bytes 4–5 (big-endian u16) del header sin comprimir.
Los savegames modernos usan versión ≥ 300.

### Estructura del stream de chunks

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

#### Gamma encoding (SlReadSimpleGamma)

Utilizado para longitudes de elementos en todos los tipos array/table:

```
0xxxxxxx         → 7 bits (1 byte)
10xxxxxx xx      → 14 bits (2 bytes)
110xxxxx xx xx   → 21 bits (3 bytes)
1110xxxx xx xx xx → 28 bits (4 bytes)
11110000 xx xx xx xx → 32 bits (5 bytes)
```

### Parseo de MAPS (dimensiones)

`MAPS` es un `CH_TABLE` con campos `dim_x` y `dim_y` (SLE_FILE_U32, big-endian).
Si `MAPS` no se encuentra (savegames muy antiguos), se infieren las dimensiones
asumiendo mapa cuadrado de potencia de 2 desde el tamaño de `MAPT`.

---

## 17. Referencias en el código fuente de OpenTTD

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

### Nota sobre town_land.h y los sprites de casas

El array `_town_draw_tile_data` tiene una entrada por `(HouseID * 4 + stage)`.
Los sprite IDs están en hexadecimal (p.ej. `0x58d` = 1421). Los ground sprites especiales:

| Constante             | ID decimal | Descripción              |
|-----------------------|------------|--------------------------|
| `SPR_FLAT_BARE_LAND`  | 3924       | Hierba plana (grass)     |
| `SPR_FLAT_GRASS_TILE` | 3943       | Hierba con flores        |
| `SPR_CONCRETE_GROUND` | 1311       | Suelo de concreto        |

---

## 18. Resumen de archivos del proyecto

| Archivo | Rol |
|---------|-----|
| `scripts/descargar_graficos.sh` | Descarga OpenGFX y extrae sprites a `assets/opengfx/tiles/`; soporta `ogfx1_base00.png` y `ogfx1_base01.png` |
| `scripts/parse_sav.py` | `.sav` → `.ottdmap` v3; resuelve OBJS para MP_OBJECT |
| `crates/openttdrs-core/src/map.rs` | `Tile`, `Map`, `from_ottd_binary` |
| `crates/openttdrs-client/src/iso.rs` | Proyección isométrica, `compute_tileh`, `SLOPE_HALF_H` |
| `crates/openttdrs-client/src/sprites.rs` | `HOUSE_DRAW_DATA` (128 casas), `INDUSTRY_GFX_DATA`, road/rail bits |
| `crates/openttdrs-client/src/main.rs` | Sistema de render Bevy: teselas, overlays, cámara |
| `docs/SPRITES_OPENGFX_COMPLETO.md` | Catálogo completo de IDs de sprites OpenGFX |
| `docs/INDUSTRIAS_OPENGFX.md` | Detalle de sprites de industrias |
| `docs/INFORME_ARQUITECTURA_OPENTTD.md` | Arquitectura general del proyecto |
