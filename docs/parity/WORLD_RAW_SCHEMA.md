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
- `trees_replay`: mapa resultante de reanudar únicamente `GenerateTrees` desde
  un `.sav` capturado justo antes de esa fase. El dumper exige los dos campos
  `DATE.random_state`; por eso es un diagnóstico de algoritmo y no una nueva
  partida generada desde una semilla aproximada.
- `landscape`, `clear`, `towns`, `industries`, `objects` y `trees`: fronteras
  de partida nueva emitidas por el fixture de generación. El candidato puede
  detenerse en la misma frontera con `world_raw_dumper --generate-until`;
  `landscape` queda inmediatamente después de `GenerateLandscape`, por lo que
  incluye su conversión de agua, ríos y tile loops, pero precede a
  `GenerateClearTile`.

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

Para investigar una frontera de generación que conserve el RNG de OpenTTD:

```bash
cargo run -p openttdrs-core --bin world_raw_dumper -- \
  --replay-trees /tmp/pre-generate-trees.sav /tmp/trees-replay.jsonl
```

El resultado tiene `stage: "trees_replay"`. Compararlo contra el `.sav` que
OpenTTD guarda inmediatamente después de `GenerateTrees` evita atribuir a los
árboles diferencias que ya existían en clear, pueblos, industrias u objetos.

El harness reproducible añade la traza separada `tree-generation-trace` (no es
una captura raster ni un sustituto de `world-raw`):

```bash
python3 scripts/tree_phase_parity.py --size 64 --seed 1330935378 --climate arctic \
  --out-dir /tmp/openttdrs-tree-phase
```

Además de los diez bytes por tesela y los bloques 4×4, compara en orden cada
llamada de colocación admitida por sustrato (`group`, `random`,
`same_height` o `rainforest`), sus coordenadas, el valor RNG y su padre cuando corresponde.
En tropical una llamada puede ser un no-op si el tipo elegido es inválido en
desierto; la traza lo conserva para localizar el primer cambio de stream. La metadata contiene dimensiones,
clima y el estado RNG inicial; `producer` y las rutas se excluyen a propósito
porque difieren entre el oráculo C++ y Rust. El hook de esta fase requiere el
parche completo del checkout 15.3 fijado, no el modo `world_raw_only`.

Para localizar la primera fase que diverge en un mapa procedural, el harness
por etapas exporta `world-raw` directamente mientras OpenTTD genera el mundo:

```bash
python3 scripts/generation_phase_parity.py --size 64 --seed 1330935378 \
  --out-dir /tmp/openttdrs-generation-phase
```

Compara `landscape`, `clear`, pueblos, industrias, objetos y árboles por bytes
y bloques 4×4. RMAP-143 añadió al reporte de fases v2
además `random_state_0`, `random_state_1`, `town_count` y `town_positions`
(secuencia de pueblos `{id,x,y}` en orden de pool). Desde RMAP-144
(2026-09-04), el reporte v3 exige además `population` y `num_houses` dentro
de cada pueblo y registra `first_town_difference` para localizar la primera
entidad distinta. RMAP-147 (2026-09-05) eleva el reporte a v4: también exige
`industry_count`/`industry_positions` (`id`, tipo, origen y
`selected_layout`) y `object_count`/`object_positions` (`id`, tipo, origen,
huella y `view`), con `first_industry_difference` y
`first_object_difference`. RMAP-151 (2026-09-05) eleva el reporte a v5:
cada industria añade los bits `random`, `random_colour`, `counter`,
`prod_level` y `town_id`; el puntero de pueblo nulo se normaliza a
`u32::MAX`. Las secuencias deben ser únicas y ascender por ID; metadata
ausente, malformada o desordenada falla cerrado. Ambos exportadores generan
estos campos en la cabecera; son opcionales en `world-raw` para no cambiar el
contrato de carga SAV, pero obligatorios para el gate de generación. Si faltan,
el comparador falla y pide reconstruir el exportador. La igualdad de teselas
se conserva en `tiles_exact_match`; `exact_match` requiere también igualdad
del estado observado. No compara todavía todos los campos CITY, INDY u OBJS
ni las trazas de intentos. El candidato se detiene con
`world_raw_dumper --generate-until FASE`, así el informe declara la primera
fase divergente sin intentar cargar un `.sav` que aún no tiene pueblos.

El comparador valida primero contrato, dimensiones, región, SHA de la partida
si ambos lados la informan, cantidad de filas y luego cada campo crudo. Por
default tolera que `producer` y `stage` sean distintos; `--strict-metadata`
agrega tick, clima, commit, ruta y versión del save a la comparación estricta.
