# Formato binario `.ottdmap` (openttdrs)

Archivo **propio** de este proyecto: empaqueta un subconjunto del **estado del mapa** (y metadatos opcionales) de forma densa. **No** es un savegame OpenTTD (`.sav`); tampoco es un volcado bit a bit de los chunks del save.

- **Productor de referencia:** `scripts/parse_sav.py` (desde un `.sav` de OpenTTD).
- **Consumidores de referencia:** `openttdrs_core::Map::from_ottd_binary` y `Map::from_ottd_binary_with_extras` (`crates/openttdrs-core/src/map.rs`, `ottdmap_extras.rs`).

Todas las multibytes son **little-endian** salvo que se indique lo contrario.

## Cabecera versionada (16 bytes)

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

## Planos densos

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

## Footers opcionales

Van concatenados despues del bloque denso.

`dense_payload_end` fija el fin del denso en `base + 12N` para `MAP1`.

Formato de cada footer:

- `INDP`: `magic(4)` + `count(u32)` + `count * (industry_index(u16), industry_type(u8))`
- `STNN`: `magic(4)` + `len(u32)` + `blob`
- `TNBP`: `magic(4)` + `len(u32)` + `blob`
- `STXY`: `magic(4)` + `count(u32)` + `count * (x(u16), y(u16))`

Orden tipico de escritura en `parse_sav.py`: `INDP` (si hay), `STNN`, `TNBP`, `STXY`.

## M3 y tranvía (fidelidad de datos)

En **MAP1 `format_version = 1`** los planos `m3` y `m3hi` van **siempre** en el bloque denso (no hace falta un footer aparte). `scripts/parse_sav.py` rellena `M3LO` / `M3HI` desde el save cuando existen los chunks.

Que el **cliente** use esos bits para dibujar tranvía encima de la carretera es independiente del formato: el dato ya viaja en cada `Tile` (`crates/openttdrs-core/src/map.rs`).

## Compatibilidad

- Lector Rust y export actual usan unicamente `MAP1`.
- Cambios futuros de layout deben incrementar `format_version`.

## Que no incluye

No reemplaza un `.sav`: no incluye empresas, vehiculos, economia, scripts, goals ni gran parte del estado global de partida. Es mapa + metadatos auxiliares.
