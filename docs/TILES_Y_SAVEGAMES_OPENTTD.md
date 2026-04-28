# Teselas, mapas y savegames de OpenTTD — Referencia para openttdrs

Este documento recoge lo aprendido al cargar mapas reales (`.sav` → `.ottdmap`) y al
renderizar carreteras y relieve. Complementa [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md)
(gráficos) y el código en `crates/openttdrs-core` / `crates/openttdrs-client`.

---

## Byte MAPT (tipo de tesela en el savegame)

En el mapa descomprimido de OpenTTD, cada tesela tiene un byte en el chunk `MAPT`.
El **tipo principal** está en los **bits 4–7** (4 bits):

```text
TileType = (mapt_byte >> 4) & 0xF
```

Valores habituales (`tile_map.h` / `tile_type.h` del upstream):

| Valor | Nombre upstream   | Uso típico                          |
|------:|-------------------|-------------------------------------|
| 0     | `MP_CLEAR`        | Prado, rocas, campos…               |
| 1     | `MP_RAILWAY`      | Vías                                |
| 2     | `MP_ROAD`         | Carretera, cruce, depósito carretera |
| 3     | `MP_HOUSE`        | Casas / urbano                      |
| 4     | `MP_TREES`        | Árboles                             |
| 5     | `MP_STATION`      | Estaciones / paradas                |
| 6     | `MP_WATER`        | Agua                                |
| 7     | `MP_VOID`         | Borde del mapa                      |
| 8     | `MP_INDUSTRY`     | Industrias                          |
| 9     | `MP_TUNNELBRIDGE` | Entrada túnel o rampa puente       |
| 10    | `MP_OBJECT`       | Objetos NewGRF                      |

Los **bits 0–3** del MAPT guardan otros datos (p. ej. zona trópico en clear), no solo
el tipo.

### Por qué guardamos `mapt` en `Tile` (Rust)

`openttdrs` reduce el tipo a un `TileKind` (`crates/openttdrs-core/src/map.rs`), pero el **byte `m5`**
tiene significados distintos según el tipo real de OpenTTD:

- En `MP_ROAD`, `m5` sigue la convención de carreteras (ver abajo).
- En `MP_TUNNELBRIDGE` con transporte carretera, `m5` codifica dirección y tipo de
  transporte **no** como una carretera normal.

Sin el MAPT crudo no se puede decodificar bien `m5`. Por eso `Tile` incluye `mapt: u8`
además de `kind` y `m5`.

---

## Byte `m5` en teselas `MP_ROAD`

Fuente principal: `reference/openttd-upstream/src/road_map.h`.

### Subtipo de tesela carretera: bits 6–7

```text
RoadTileType = (m5 >> 6) & 0x3
```

| Valor | `RoadTileType` | Significado        |
|------:|----------------|--------------------|
| 0     | `Normal`       | Carretera normal   |
| 1     | `Crossing`     | Cruce a nivel (vía + carretera) |
| 2     | `Depot`        | Depósito de carreteras |

### Caso `Normal` (subtipo 0)

Los **road bits** están en los bits **0–3** de `m5` (`GetRoadBits` para carretera, no tranvía).

Convención `RoadBit` (`road_type.h`):

| Bit | Arista del rombo |
|----:|------------------|
| 0   | NW               |
| 1   | SW               |
| 2   | SE               |
| 3   | NE               |

Constantes útiles:

- **`ROAD_X`** (eje X del mapa): `SW + NE` → valor `0x0A`.
- **`ROAD_Y`** (eje Y del mapa): `NW + SE` → valor `0x05`.
- **`ROAD_ALL`**: `0x0F` (cruce carretera a carretera).

**Tranvía:** en tiles normales, los bits de **tranvía** pueden ir en **`m3`** (no en `m5`).
Nuestro `.ottdmap` actual **no** exporta `m3`; mapas con mucho tranvía pueden verse
incompletos hasta ampliar el formato.

### Caso `Crossing` (subtipo 1) — error frecuente

En un **cruce a nivel**, los bits 0–3 **no** son road bits estándar.

- El **eje de la carretera** está en el **bit 0** (`GetCrossingRoadAxis`):

  - `AXIS_X` (0) → carretera como **`ROAD_X`** (`0x0A`).
  - `AXIS_Y` (1) → carretera como **`ROAD_Y`** (`0x05`).

Otros bits de `m5` en cruces (reservas, barreras, etc.) están documentados en el mismo
`road_map.h`; para orientar el sprite de carretera basta el eje.

**Bug histórico en openttdrs:** tratar el cruce como carretera “normal” y leer road bits
en 0–3 producía valores absurdos y el código caía en un **fallback por vecinos**, rompiendo
trazados rectos y alineación con el mapa original.

### Caso `Depot` (subtipo 2)

La salida del depósito es una **dirección diagonal** en los bits **0–1**:
`GetRoadDepotDirection` → `DiagDirection` (0 = NE, 1 = SE, 2 = SW, 3 = NW).

Un solo “tramo” de carretera en esa arista se obtiene con la misma fórmula que el upstream
`DiagDirToRoadBits` (`road_func.h`):

```text
road_bits = (1 << (3 ^ d)) & 0xF   // d = DiagDirection 0..3
```

---

## `MP_TUNNELBRIDGE` con carretera

`GetTunnelBridgeDirection`: dirección diagonal en bits **0–1** de `m5`.
`GetTunnelBridgeTransportType`: bits **2–3** (carretera vs raíl).

Si el juego mapea la tesela a `TileKind::Road`, la orientación del tramo en la rampa /
boca del túnel sigue la misma idea que `DiagDirToRoadBits` (un solo bit en 0–3).

---

## De road bits a sprites en openttdrs

Solo tenemos tres gráficos de carretera base: `road_tx.png`, `road_ty.png`, `road_cross.png`
(ver [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md)).

Regla en el cliente (`RoadDir` desde road bits / cruces / vecinos):

- `RoadDir::Tx` (eje X del mapa, `ROAD_X`, bits `0x0A`) → sprite **`road_ty.png`**.
- `RoadDir::Ty` (eje Y del mapa, `ROAD_Y`, bits `0x05`) → sprite **`road_tx.png`**.
- `RoadDir::Both` → **`road_cross.png`**.

Es un **intercambio intencional** respecto al nombre del archivo: los recortes OpenGFX y
nuestra proyección isométrica quedan alineados así (~90° respecto a asignar “tx→tx”).
Comprobado en capturas sobre `.ottdmap` de regresión / partidas reales (trazados rectos y
cruces coherentes).

El **cruce** no se invierte.

- Si hay presencia en **ambos** ejes (p. ej. `0x0F`, T, L): sprite **cruce** (aproximación;
  OpenTTD usa más variantes para esquinas y tes).

Las **vías** (`TileKind::Rail`) usan provisionalmente el mismo par intercambiado hasta
tener sprites de rail.

Los **subconjuntos** de un eje (p. ej. solo `NE` sin `SW`) siguen mostrando el sprite de
tramo completo: es una limitación hasta extraer más sprites o recortar en shader.

---

## Formato `.ottdmap` (salida de `scripts/parse_sav.py`)

Binario consumido por `Map::from_ottd_binary`:

| Offset | Contenido |
|--------|-----------|
| 0–3    | Magic `MAPO` |
| 4–7    | `width` u32 LE |
| 8–11   | `height` u32 LE |
| 12 + i | `mapt[i]` — byte MAPT original |
| + W×H  | `height[i]` — altura tesela |
| + W×H  | `m5[i]` |

Índice lineal: `i = y * width + x` (x varía rápido), igual que el orden de los chunks
`MAPT` / `MAPH` / `MAP5` en el savegame.

---

## Relieve (altura) en pantalla

OpenTTD usa **8 píxeles** de desplazamiento vertical por unidad de altura de tesela
(`TILE_HEIGHT` en el upstream). En el cliente, `tile_pos` / `overlay_pos` aplican
`HEIGHT_PX = 8.0` en coordenadas Bevy (Y hacia arriba) para separar visualmente mesetas
sin pintar pendientes reales.

El orden Z mezcla `(tx + ty)` con un pequeño término en `height` para reducir parpadeos
entre teselas a distinta cota.

---

## Carga de partidas (`.sav`)

- Formato binario comprimido (`OTTZ` zlib, `OTTX` xz/lzma, `OTTN` sin comprimir, `OTTD` LZO
  antiguo).
- Tras descomprimir: stream de **chunks** de 4 caracteres; los del mapa incluyen `MAPS`,
  `MAPT`, `MAPH`, `MAP5`, etc. Ver comentarios en `scripts/parse_sav.py` y
  `reference/.../src/saveload/map_sl.cpp`.

---

## Resumen de archivos del proyecto

| Archivo | Rol |
|---------|-----|
| `scripts/parse_sav.py` | `.sav` → `.ottdmap` |
| `crates/openttdrs-core/src/map.rs` | `Tile { height, kind, mapt, m5 }`, `from_ottd_binary` |
| `crates/openttdrs-client/src/main.rs` | `effective_road_bits`, relieve, sprites |
| `docs/SPRITES_OPENGFX.md` | NFO, IDs de sprite, transparencia |

---

## Referencias en el código fuente de OpenTTD (local)

Rutas bajo `reference/openttd-upstream/src/`:

- `road_map.h` — cruces, depósitos, `GetCrossingRoadBits`, `GetRoadBits`
- `road_func.h` — `DiagDirToRoadBits`, `AxisToRoadBits`
- `road_type.h` — `RoadBit`, `ROAD_X`, `ROAD_Y`
- `tunnelbridge_map.h` — dirección y tipo de túnel/puente
- `tile_map.h` — `GetTileType`, altura
- `saveload/map_sl.cpp` — chunks `MAPT`, `MAPH`, `MAP5`
