# Fixtures para tests

- **`stationlist-test.sav`** — Save de regresión de OpenTTD (~94 KiB). Sirve de entrada fija para comprobar que `scripts/parse_sav.py` no cambia estadísticas clave sin querer.
- **`parse_sav_stationlist_golden.json`** — Resultado esperado de `analyze_save()` sobre ese `.sav` (conteos por tipo de tesela e histograma de HouseID en `m8` tras la migración M3HI/M3LO).
- **`crates/openttdrs-core/tests/fixtures/rail_signals_mixed.sav`** — Save sintético 16×16 con señales tipos 0–5 (regenerar con `scripts/gen_rail_signals_sav.py`).
- **`crates/openttdrs-core/tests/fixtures/parity/rail_signals_golden.json`** — Encoding `m2/m3/m5` y IDs de textura esperados para regresión de sprites.

Regenerar el golden tras modificar `parse_sav.py`:

```bash
python3 scripts/emit_parse_sav_golden.py tests/fixtures/stationlist-test.sav \
  > tests/fixtures/parse_sav_stationlist_golden.json
```

Revisar el diff y commitear solo si el cambio es intencionado.
