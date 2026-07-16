# Primera divergencia oráculo OpenTTD ↔ candidato (#110)

Fixture: `tests/fixtures/stationlist-test.sav`  
Oráculo: OpenTTD **15.3** (`14ec60f248547d4d062a1160f0fc26d742319888`) + [`patches/openttd-15.3-snapshot-export/`](../../patches/openttd-15.3-snapshot-export/)  
Candidato: `parse_sav.py` → `snapshot_dumper`  
Artefacto: [`crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json`](../../crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json)

```bash
./scripts/export_openttd_oracle_snapshot.sh \
  tests/fixtures/stationlist-test.sav \
  crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json
python3 scripts/parse_sav.py tests/fixtures/stationlist-test.sav /tmp/cand.ottdmap
cargo run -p openttdrs-core --bin snapshot_dumper -- /tmp/cand.ottdmap /tmp/cand.json
python3 scripts/compare_snapshots.py \
  crates/openttdrs-core/tests/fixtures/parity/stationlist_openttd_oracle.json /tmp/cand.json
```

## Coinciden

| Campo | Valor |
|-------|--------|
| `map.width` / `height` | 256 × 256 |
| `hashes.height_hash_fnv1a64` | `491f3424ae6844b5` |
| `hashes.mapt_hash_fnv1a64` | `4298ad417a195769` |
| `hashes.kind_hash_fnv1a64` | (igual tras alinear KindCode a `ottd_tile_kind`) |
| `hashes.rail_bits_hash_fnv1a64` | `d0a3931867272a40` |
| `components.industry_components` | 73 |
| `components.station_components` | 8 |

## Primera divergencia

**`hashes.road_bits_hash_fnv1a64`**

- oráculo: `cc1c08d5ec5b4d7f`
- candidato: `076acdf9406b59e3`

El hash solo incorpora tiles `Road`: `m5 & 0x0F` + `m8` u16 LE. La diferencia apunta a packing/valores de `m8` (RoadType) entre el motor vivo y los planos reconstruidos por `parse_sav`.

## Notas

- Saves sintéticos (`rail_signals_mixed.sav`, `demo_openttd.sav`) no cargan en 15.3 (`MAPS` ya no es RIFF simple).
- Dedicated + `-g` dispara dos `AfterLoadGame` (new-game luego load); el export usa `OPENTTDRS_SNAPSHOT_MIN_CALL=2`.
- El oráculo **no** invoca `parse_sav.py` ni `snapshot_dumper`.
