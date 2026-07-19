# Snapshot oracle workflow (#110)

Hay **dos productores independientes**:

| Rol | Productor | Entrada |
|-----|-----------|---------|
| **Oráculo** | OpenTTD C++ (commit pin [#109](parity/openttd-reference.json)) | `.sav` |
| **Candidato** | `parse_sav.py` + `snapshot_dumper` (openttdrs) | `.sav` → `.ottdmap` |

Esquema: [parity/SNAPSHOT_SCHEMA.md](parity/SNAPSHOT_SCHEMA.md).

> **No es oráculo** el flujo antiguo que envolvía `parse_sav.py` dentro de un “fork” OpenTTD: ambos lados usaban el parser bajo prueba. Ese script quedó reemplazado.

## 1) Oráculo (OpenTTD real)

```bash
./scripts/fetch-openttd-reference.sh
./patches/openttd-15.3-snapshot-export/integrate.sh
cmake -B reference/openttd-upstream/build -S reference/openttd-upstream -DOPTION_DEDICATED=ON
cmake --build reference/openttd-upstream/build -j

./scripts/export_openttd_oracle_snapshot.sh \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/openttd.oracle.json
```

El JSON debe tener `"producer": "openttd"`.

## 2) Candidato (openttdrs)

```bash
python3 scripts/parse_sav.py \
  crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav \
  /tmp/candidate.ottdmap
cargo run -p openttdrs-core --bin snapshot_dumper -- \
  /tmp/candidate.ottdmap /tmp/openttdrs.candidate.json
```

## 3) Comparación

```bash
python3 scripts/compare_snapshots.py \
  /tmp/openttd.oracle.json \
  /tmp/openttdrs.candidate.json
```

La primera divergencia se imprime y el exit code es `1`.  
Mutación sintética (CI local):

```bash
python3 scripts/test_compare_snapshots_mutation.py
```

## Spike / fixtures

- Save pequeño versionado: `crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav`
- Ottdmap 2×2 (solo candidato): `m3_road_tram_2x2.ottdmap`

Sin binario OpenTTD compilado el paso 1 no corre; el resto del tooling y el parche sí están versionados.

## Trazas PBS por tick

Este workflow compara snapshots de mapa al cargar un save. Las reservas PBS son
dinámicas y usan un productor separado: [PBS_EXTERNAL_ORACLE.md](PBS_EXTERNAL_ORACLE.md).
El parche comparte integración, pero el export PBS emite JSONL post-tick y
finaliza automáticamente tras el número de filas solicitado.
