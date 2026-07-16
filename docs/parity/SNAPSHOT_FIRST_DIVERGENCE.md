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

## Estado: resuelta

Tras aplicar en `parse_sav.py` / `sav::build` la migración `SLV_ROAD_TYPES` (save &lt; 214), el comparador reporta **OK** en campos hard.

| Campo | Valor |
|-------|--------|
| `map.width` / `height` | 256 × 256 |
| `hashes.height_hash_fnv1a64` | `491f3424ae6844b5` |
| `hashes.mapt_hash_fnv1a64` | `4298ad417a195769` |
| `hashes.kind_hash_fnv1a64` | (igual tras alinear KindCode a `ottd_tile_kind`) |
| `hashes.rail_bits_hash_fnv1a64` | `d0a3931867272a40` |
| `hashes.road_bits_hash_fnv1a64` | `cc1c08d5ec5b4d7f` |
| `components.industry_components` | 73 |
| `components.station_components` | 8 |

## Causa raíz (histórica)

**`hashes.road_bits_hash_fnv1a64`** divergía porque el oráculo hashea `m8` post-`AfterLoadGame` y el candidato copiaba `MAP8` crudo (todo 0 en este save v211).

En saves &lt; 214, OpenTTD mueve el RoadType desde bits 6–7 de `m7` a `m4` (road) y `m8` bits 6–11 (tram). Sin tram: `m8 = INVALID_ROADTYPE << 6` (`0xFC0`).

## Notas

- Saves sintéticos (`rail_signals_mixed.sav`, `demo_openttd.sav`) no cargan en 15.3 (`MAPS` ya no es RIFF simple).
- Dedicated + `-g` dispara dos `AfterLoadGame` (new-game luego load); el export usa `OPENTTDRS_SNAPSHOT_MIN_CALL=2`.
- El oráculo **no** invoca `parse_sav.py` ni `snapshot_dumper`.
