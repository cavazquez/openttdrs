# OpenTTD snapshot export (openttdrs #110)

Parche/integración para el commit fijado en [`docs/parity/openttd-reference.json`](../../docs/parity/openttd-reference.json) (tag **15.3**).

Produce JSON compatible con `snapshot_dumper` **desde el motor C++**, sin `parse_sav.py`.

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
