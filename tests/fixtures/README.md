# Fixtures para tests

- **`stationlist-test.sav`** — Save de regresión de OpenTTD (~94 KiB). Sirve de entrada fija para comprobar que `scripts/parse_sav.py` no cambia estadísticas clave sin querer.
- **`parse_sav_stationlist_golden.json`** — Resultado esperado de `analyze_save()` sobre ese `.sav` (conteos por tipo de tesela e histograma de HouseID en `m8` tras la migración M3HI/M3LO).

Regenerar el golden tras modificar `parse_sav.py`:

```bash
python3 scripts/emit_parse_sav_golden.py tests/fixtures/stationlist-test.sav \
  > tests/fixtures/parse_sav_stationlist_golden.json
```

Revisar el diff y commitear solo si el cambio es intencionado.
