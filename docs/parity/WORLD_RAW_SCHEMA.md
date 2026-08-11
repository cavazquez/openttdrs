# Contrato `world-raw` v2

`world-raw` es el oráculo de bajo nivel para investigar una partida `.sav`
que se ve distinta en `openttdrs`. En vez de resumir el mapa en hashes,
exporta una línea JSON por tesela con los bytes que intervienen en el tipo,
subtipo, orientación y sprite.

El archivo es JSONL: la primera línea es `metadata`; el resto, `tile_raw`.
Las filas se escriben en orden fila-mayor y no requieren cargar el mapa entero
en el comparador.

## Metadatos

```json
{
  "kind": "metadata",
  "schema_version": 2,
  "contract": "world-raw",
  "producer": "openttd",
  "stage": "after_load_game",
  "tick": 3703074,
  "climate": 0,
  "openttd_commit": "…",
  "source_path": "/ruta/partida.sav",
  "save_sha256": "…",
  "save_version": 350,
  "width": 256,
  "height": 256,
  "tile_count": 65536,
  "emitted_tile_count": 65536,
  "region": null
}
```

`stage` identifica el momento exacto del mapa observado:

- `after_load_game`: mapa vivo de `OpenTTD`, al final de `AfterLoadGame`.
- `sav_map`: bytes reconstruidos por `openttdrs_core::sav::load`, antes de
  normalizaciones del estado jugable.
- `game_state_map`: mapa tras `GameState::from_sav_game`; sirve para detectar
  una normalización local que modifica un byte o retaguea una tesela.

`climate` conserva el valor de `LandscapeType` de OpenTTD: 0 temperate,
1 arctic, 2 tropic, 3 toyland. `region` es inclusiva y conserva coordenadas
absolutas; si queda parcialmente fuera del mapa, `emitted_tile_count` refleja
solamente la intersección.

## Fila de tesela

```json
{
  "kind": "tile_raw",
  "index": 1234,
  "x": 210,
  "y": 4,
  "height": 7,
  "type": 144,
  "m1": 0,
  "m2": 513,
  "m3": 16,
  "m4": 128,
  "m5": 134,
  "m6": 32,
  "m7": 0,
  "m8": 2
}
```

`index = y * width + x`. `type` es el byte MAPT completo de OpenTTD, no sólo
su nibble de tipo. `m2` y `m8` son enteros little-endian de 16 bits. En
`openttdrs`, `m4` se reconstruye desde el campo interno `m3hi` y `m2` desde
`m2 | (m2_hi << 8)`.

Por eso una diferencia puede señalar directamente casos como:

- `type` o `m5`: clase/subtipo de rail, road, bridge, tunnel o station;
- `m3`/`m4`: orientación, señales, estado de objeto o variación de árbol;
- `m2`/`m8`: índice de estación, tile de aeropuerto, house ID o road/tram;
- `height`: cimiento, pendiente o el orden vertical de capas.

En `MP_OBJECT`, `m2 | (m5 << 16)` es el `ObjectID` crudo. El `ObjectType`
visible no se deduce de `m5`: proviene del pool `OBJS` y se compara en la capa
`world-semantic` como `details.object_type`.

## Flujo reproducible

Primero integrar y compilar la referencia. El checkout oficial fijado a 15.3
usa el exportador completo; para otro árbol local, el opt-in mantiene sólo el
exportador mínimo de bytes, sin portar las trazas PBS/FTA históricas.

```bash
./patches/openttd-15.3-snapshot-export/integrate.sh
cmake --build reference/openttd-upstream/build -j

# Árbol local no pinneado, por ejemplo OpenTTD 16 beta.
OPENTTDRS_ALLOW_UNPINNED=1 \
  ./patches/openttd-15.3-snapshot-export/integrate.sh ../OpenTTD
cmake --build ../OpenTTD/build -j
```

Exportar ambos lados (mismo `.sav` y mismo filtro si lo hubiera):

```bash
./scripts/export_openttd_world_raw.sh save/Kale_TitleGame.sav /tmp/openttd.jsonl \
  ../OpenTTD/build/openttd

./scripts/export_openttdrs_world_raw.sh \
  save/Kale_TitleGame.sav /tmp/openttdrs-sav-map.jsonl

python3 scripts/compare_world_raw.py /tmp/openttd.jsonl /tmp/openttdrs-sav-map.jsonl \
  --json-report /tmp/world-raw-report.json
```

Para aislar una zona grande:

```bash
# Rectángulo inclusivo.
./scripts/export_openttd_world_raw.sh partida.sav /tmp/openttd.jsonl ../OpenTTD/build/openttd 80,120,112,152
./scripts/export_openttdrs_world_raw.sh \
  partida.sav /tmp/openttdrs.jsonl 80,120,112,152

# Una tesela y su vecindario local, útil tras la primera divergencia.
cargo run -p openttdrs-core --bin world_raw_dumper -- \
  partida.sav /tmp/openttdrs.jsonl --tile 96,134 --radius 2
```

El comparador valida primero contrato, dimensiones, región, SHA de la partida
si ambos lados la informan, cantidad de filas y luego cada campo crudo. Por
default tolera que `producer` y `stage` sean distintos; `--strict-metadata`
agrega tick, clima, commit, ruta y versión del save a la comparación estricta.
