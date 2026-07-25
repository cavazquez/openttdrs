# Oráculo externo airport FTA (#198)

Traza JSONL de aviones normales + bloques de aeropuerto, producida por
OpenTTD 15.3 parcheado (`patches/openttd-15.3-snapshot-export/`) y comparada
con `openttdrs-core`.

## Contrato JSONL v1

```bash
OPENTTDRS_AIRPORT_FTA_TRACE_OUT=/tmp/openttd-airport-fta.jsonl \
OPENTTDRS_AIRPORT_FTA_TRACE_TICKS=80 \
./reference/openttd-upstream/build/openttd -D -g partida.sav
```

Filas: `metadata` → `initial` → N× `tick`. Cada muestra lleva `aircraft[]`
(`pos`, `previous_pos`, `state`=heading, tile/pixel, speed) y `airports[]`
(`type`, `blocks`, footprint).

## Fixture Helidepot

- Save: `crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav`
- Oráculo: `tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl`
- Tests: `tests/airport_fta_openttd_oracle.rs`

2× Helidepot + 1 Tricario A↔B. El `initial` coincide; tras ~14 ticks el
heading puede adelantarse un tick respecto a OpenTTD (dwell FTA no persistido
en el `.sav`).

## Regenerar

```bash
./patches/openttd-15.3-snapshot-export/integrate.sh
cmake --build reference/openttd-upstream/build -j --target openttd

./scripts/export_openttd_airport_fta_trace.sh \
  crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav \
  crates/openttdrs-core/tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl \
  80

cargo run -p openttdrs-core --bin sav_airport_fta_runner -- \
  crates/openttdrs-core/tests/fixtures/helidepot_fta_cycle_15_3.sav \
  --ticks 80 --out /tmp/helidepot-openttdrs.jsonl

python3 scripts/compare_airport_fta_traces.py \
  crates/openttdrs-core/tests/fixtures/parity/helidepot_fta_cycle_15_3_openttd.jsonl \
  /tmp/helidepot-openttdrs.jsonl
```
