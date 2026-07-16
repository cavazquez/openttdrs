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
| `house_draw_data` | `gen_house_draw_data.py` | Solo `output_sha256` en CI (OpenGFX no vendorizado) |

`gen_house_draw_data.py --check` sirve en local con PNG; hoy diverge del `.rs` versionado (offsets/NFO vs pin 15.3). Regenerar ese output queda fuera de #119 (cambio de datos de render).

OpenGFX (`assets/opengfx/tiles/`) **no** está vendorizado ni se descarga en CI.

## Comandos

```bash
# Listar inventario
python3 scripts/check_generated_tables.py --list

# Verificar pilots (requiere reference/openttd-upstream)
./scripts/fetch-openttd-reference.sh   # una vez / en CI
python3 scripts/check_generated_tables.py --check

# CI: fetch pin + check
python3 scripts/check_generated_tables.py --check --fetch-upstream

# Regenerar (escribe el .rs)
python3 scripts/gen_house_population.py
python3 scripts/gen_house_draw_data.py   # requiere PNG house_s*.png
```

Tras regenerar `house_draw_data`, actualizá `output_sha256` en el manifiesto:

```bash
sha256sum crates/openttdrs-client/src/sprites/house_draw_data_generated.rs
```

## Licencia

Derivados de headers OpenTTD: **GPL-2.0-only** (ver pin). Offsets/PNG OpenGFX quedan fuera del árbol git.

## Extensión

Nuevas tablas: añadir al `inventory`; cuando tengan `--check` estable, subirlas a `pilots`.
