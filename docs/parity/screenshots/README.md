# Capturas de referencia UI

Referencias visuales para UI-0 (#33). Generar con display activo:

```bash
bash scripts/capture_ui_reference.sh
# o con partida fija:
bash scripts/capture_ui_reference.sh save/partida_2026-06-22_0942.json
```

Variables:

| Env | Uso |
|-----|-----|
| `OPENTTDRS_WINDOWS_SHOT` | Ruta PNG de salida |
| `OPENTTDRS_SHOT_RES` | `1280x720` o `1920x1080` |
| `OTTDJSON_LOAD` | Partida JSON opcional |

El harness abre Town / Depot / Buy / Vehicle / Finances / TownDirectory
antes de capturar (`windows_shot.rs`).
