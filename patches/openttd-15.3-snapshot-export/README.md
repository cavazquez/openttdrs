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
