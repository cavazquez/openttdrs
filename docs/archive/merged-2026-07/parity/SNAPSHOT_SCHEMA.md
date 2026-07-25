# Esquema de snapshot de mapa (openttdrs ↔ OpenTTD)

Contrato compartido por:

- **Candidato:** `cargo run -p openttdrs-core --bin snapshot_dumper`
- **Oráculo:** export C++ en OpenTTD pin (#109) vía `OPENTTDRS_SNAPSHOT_OUT` ([`patches/openttd-15.3-snapshot-export/`](../../patches/openttd-15.3-snapshot-export/))

`schema_version`: **1**

## Campos

| Campo | Tipo | Notas |
|-------|------|--------|
| `schema_version` | int | Siempre `1` |
| `producer` | string | `"openttd"` (oráculo) o `"openttdrs"` (candidato) |
| `openttd_commit` | string | SHA del manifiesto (#109); oráculo lo rellena; candidato puede ir vacío |
| `source_path` | string | Path de entrada (`.sav` / `.ottdmap`) |
| `map.width` / `map.height` | int | Dimensiones |
| `map.tile_count` | int | `width * height` |
| `map.tile_kind_counts` | object | Conteos por nombre de `TileKind` |
| `map.min_height` / `max_height` | int | Extremos de altura |
| `hashes.*_fnv1a64` | string hex 16 | FNV-1a 64-bit, orden de tiles `(y,x)` fila-mayor |
| `extras.*` | int | Solo candidato `.ottdmap`; oráculo pone `0` |
| `components.industry_components` | int | Componentes 4-conectados `Industry` |
| `components.station_components` | int | Idem `Station` (no aeropuerto) |

## Hashes (orden de bytes)

Recorrido: `for y in 0..height { for x in 0..width }`

- `height`: 1 byte `TileHeight`
- `kind`: 1 byte código (ver `snapshot_dumper` / `KindCode` C++)
- `mapt`: 1 byte `tile.type()` completo (MAPT)
- `rail_bits` (solo kind Rail=4): `m5&0x3F`, `m3`, `m4` (= m3hi ottdmap)
- `road_bits` (solo kind Road=3): `m5&0x0F`, `m8` u16 LE

Offset FNV: `0xcbf29ce484222325`, prime `0x100000001b3`.

## Comparación

```bash
python3 scripts/compare_snapshots.py oracle.json candidate.json
```

Campos hard: dimensiones + 5 hashes + 2 component counts.  
`extras` **no** se comparan (el oráculo no tiene footers ottdmap).
