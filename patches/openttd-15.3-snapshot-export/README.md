# OpenTTD snapshot and world-raw export (openttdrs #110, #305)

Parche/integración para el commit fijado en [`docs/parity/openttd-reference.json`](../../docs/parity/openttd-reference.json) (tag **15.3**).

Produce JSON compatible con `snapshot_dumper` **desde el motor C++**, sin `parse_sav.py`.

Además exporta `world-raw` v2, un JSONL por tesela con `type`, `height` y
`m1..m8`. Es el diagnóstico para diferencias de orientación, túneles, puentes,
monorriel/maglev, estaciones y objetos de una partida real. El contrato está en
[`docs/parity/WORLD_RAW_SCHEMA.md`](../../docs/parity/WORLD_RAW_SCHEMA.md).

`world-semantic` v2 toma esos mismos bytes y emite la interpretación de ambos
motores: pendiente/base Z, familia de tesela, tipos de vía, orientaciones,
estaciones, agua/depósitos y la otra punta de cada túnel o puente. Es el paso
siguiente cuando `world-raw` coincide. La v2 también resuelve `object_type`
desde el pool vivo de objetos, en vez de confundirlo con bytes MAP*. Contrato:
[`docs/parity/WORLD_SEMANTIC_SCHEMA.md`](../../docs/parity/WORLD_SEMANTIC_SCHEMA.md).

## Integrar en el clon local

```bash
./scripts/fetch-openttd-reference.sh
./patches/openttd-15.3-snapshot-export/integrate.sh
```

## Build (dedicated)

Requiere CMake ≥ 3.17 y deps: zlib, liblzma, lzo, libpng, curl (y toolchain C++20).

```bash
cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON
cmake --build reference/openttd-upstream/build -j
```

## Exportar

```bash
export OPENTTDRS_SNAPSHOT_OUT=/tmp/openttd.oracle.json
export OPENTTDRS_OPENTTD_COMMIT="$(python3 -c 'import json;print(json.load(open("docs/parity/openttd-reference.json"))["commit"])')"
./reference/openttd-upstream/build/openttd -D -g crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav
# el proceso puede quedarse en dedicated; el JSON se escribe al terminar AfterLoadGame
```

O vía helper:

```bash
./scripts/export_openttd_oracle_snapshot.sh \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/openttd.oracle.json
```

## Comparar con candidato openttdrs

```bash
python3 scripts/parse_sav.py crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav /tmp/cand.ottdmap
cargo run -p openttdrs-core --bin snapshot_dumper -- /tmp/cand.ottdmap /tmp/openttdrs.json
python3 scripts/compare_snapshots.py /tmp/openttd.oracle.json /tmp/openttdrs.json
```

## Diagnóstico `world-raw`

```bash
./scripts/export_openttd_world_raw.sh \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/openttd-world-raw.jsonl

./scripts/export_openttdrs_world_raw.sh \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/openttdrs-world-raw.jsonl

python3 scripts/compare_world_raw.py \
  /tmp/openttd-world-raw.jsonl /tmp/openttdrs-world-raw.jsonl
```

## Matriz de mapas aleatorios

Para generar una partida nueva de OpenTTD y conservar tanto el `.sav` como el
stream `world-raw` se pueden definir estos overrides al ejecutar el binario
dedicated:

```bash
OPENTTDRS_RANDOM_MAP_RAW_OUT=/tmp/random.reference.jsonl \
OPENTTDRS_RANDOM_MAP_SAVE_OUT=/tmp/random.reference.sav \
OPENTTDRS_RANDOM_MAP_SOURCE=random:64x64:seed=123 \
./reference/openttd-upstream/build/openttd -X -c /tmp/map.cfg \
  -I opengfx -v null -s null -m null -b null -D -G 123 -g
```

La matriz completa y la comparación por tesela/bloques 4×4 se ejecutan con
[`scripts/random_map_parity.py`](../../scripts/random_map_parity.py); el hook
se activa en el primer tick de una partida nueva, no sólo después de cargar un
`.sav` existente.

## Frontera `GenerateTrees`

El parche completo también puede guardar una partida justo antes y después de
`GenerateTrees`, junto con cada colocación efectiva de árbol. El harness evita
comparar una captura enorme: valida los bytes de tesela, bloques 4×4, stream
RNG y la secuencia `PlaceTree`.

```bash
python3 scripts/tree_phase_parity.py --size 64 --seed 1330935378 --climate arctic \
  --out-dir /tmp/openttdrs-tree-phase
```

Los artefactos del directorio elegido incluyen `trees.pre.sav`,
`trees.post.sav`, ambas trazas JSONL y el informe. Para ejecutar sólo el hook
manualmente se usan `OPENTTDRS_TREE_PRE_SAVE_OUT`,
`OPENTTDRS_TREE_POST_SAVE_OUT` y `OPENTTDRS_TREE_TRACE_OUT`. Esta instrumentación
se instala sólo sobre el checkout 15.3 completo; el modo no pinneado conserva
únicamente `world-raw`.

Para un checkout de OpenTTD que no sea el pin 15.3 se requiere declarar la
decisión explícitamente:

```bash
OPENTTDRS_ALLOW_UNPINNED=1 \
  ./patches/openttd-15.3-snapshot-export/integrate.sh /ruta/a/OpenTTD
```

En ese modo se compila solamente `world_raw_export.cpp`; el snapshot y las
trazas PBS/FTA del parche histórico siguen reservados al árbol pinneado.

El exportador semántico también se compila en ese modo minimal:

```bash
./scripts/export_openttd_world_semantic.sh \
  save/Kale_TitleGame.sav /tmp/openttd-world-semantic.jsonl \
  /ruta/a/OpenTTD/build/openttd

./scripts/export_openttdrs_world_semantic.sh \
  save/Kale_TitleGame.sav /tmp/openttdrs-world-semantic.jsonl

python3 scripts/compare_world_semantic.py \
  /tmp/openttd-world-semantic.jsonl /tmp/openttdrs-world-semantic.jsonl \
  --json-report /tmp/world-semantic-report.json --show-inventory
```

Para enfocar una anomalía visual se puede restringir la comparación, por
ejemplo `--only railway,tunnel_bridge` o `--where 123,87`.
