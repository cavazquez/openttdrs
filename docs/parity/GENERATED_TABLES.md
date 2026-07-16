# Tablas Rust generadas (#119)

Inventario y verificación de reproducibilidad de `*_generated.rs`.

## Fuente de verdad

| Artefacto | Rol |
|-----------|-----|
| [`scripts/generated_tables_manifest.json`](../../scripts/generated_tables_manifest.json) | Inventario + pilots + `output_sha256` |
| [`docs/parity/openttd-reference.json`](openttd-reference.json) | Pin OpenTTD (#109) |
| [`scripts/check_generated_tables.py`](../../scripts/check_generated_tables.py) | Orquestador `--check` |

## Pilots (verificados en CI)

| id | Generador | Check |
|----|-----------|-------|
| `house_population` | `gen_house_population.py` | Regenera vs `town_land.h` del pin; si no hay upstream, `output_sha256` |
| `house_draw_data` | `gen_house_draw_data.py` | Solo `output_sha256` (OpenGFX no vendorizado) |
| `vehicle_gfx_data` | `gen_vehicle_gfx_data.py` | Solo `output_sha256`; `--check` local con PNG |
| `tile_atlas` | `gen_tile_atlas.py` | Solo `output_sha256` del `.rs`; `--check` no escribe PNG |

Los generadores OpenGFX tienen `--check` (exit 2 si faltan assets). Hoy `house_draw_data` y `vehicle_gfx_data` pueden **divergir** al regenerar con el set local frente al `.rs` versionado; regenerar esos outputs es un PR de datos de render aparte.

OpenGFX (`assets/opengfx/tiles/`) **no** está vendorizado ni se descarga en CI.

## Comandos

```bash
python3 scripts/check_generated_tables.py --list
./scripts/fetch-openttd-reference.sh   # para house_population regen
python3 scripts/check_generated_tables.py --check
python3 scripts/check_generated_tables.py --check --fetch-upstream   # CI

# Regenerar (escribe)
python3 scripts/gen_house_population.py
python3 scripts/gen_house_draw_data.py
python3 scripts/gen_vehicle_gfx_data.py
python3 scripts/gen_tile_atlas.py   # también reescribe assets/opengfx/atlas/*.png
```

Tras regenerar un piloto con `check: hash`, actualizá `output_sha256`:

```bash
sha256sum crates/openttdrs-client/src/sprites/<archivo>_generated.rs
```

## Licencia

Derivados de headers OpenTTD: **GPL-2.0-only** (ver pin). Offsets/PNG OpenGFX quedan fuera del árbol git.

## Extensión

Nuevas tablas: añadir al `inventory`; con `--check` + hash estable → `pilots`.
