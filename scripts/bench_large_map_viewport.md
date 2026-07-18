# SP3.6 / perf — mapas grandes y viewport

## Umbral de culling (código)

Fuente de verdad: `crates/openttdrs-client/src/render/viewport.rs`

| Constante / env | Valor |
|-----------------|------:|
| `LARGE_MAP_TILE_THRESHOLD` | **1024** teselas (default) |
| `OPENTTDRS_MAP_VIEWPORT_THRESHOLD` | override 256…65536 |
| `OPENTTDRS_MAP_VIEWPORT_OFF=1` | desactiva culling |
| Span cámara inicial (culling on) | ~64 teselas |
| Margen | 10 teselas |
| Chunks | 16×16 |

Mapas con `width × height ≥ 1024` usan viewport culling (p. ej. 32×32 ya entra; **1024²** y **4096²** siempre).

## Cargar mapas

```bash
# Fixture 256×256
OTTDMAP_FILE=tests/fixtures/stationlist-test.ottdmap cargo run -p openttdrs-client --release

# Nueva partida 1024² / 4096²: menú → Nueva partida → tamaño T1024 / T4096
cargo run -p openttdrs-client --release
```

## Medir FPS (manual)

1. Abrir mapa (256² fixture, o nueva partida 1024² / 4096²).
2. Abrir consola de desarrollo (FPS / `visuales` en overlay).
3. Pan/zoom; anotar FPS estable y picos al panear fuera de chunks.
4. Repetir con `OPENTTDRS_MAP_VIEWPORT_OFF=1` (solo en 256² o con RAM; **no** en 4096² sin culling).

| Escenario | Esperado |
|-----------|----------|
| ≥1024 tiles, culling ON | ~viewport (~84² teselas base + overlays); pan fluido |
| Culling OFF en 1024² | ~1M entidades base → FPS colapsa / carga larga |
| 4096² culling ON | mismo orden de entidades visibles que 1024² (viewport acotado) |

## Relación con sim headless

La simulación se mide aparte (`docs/BENCHMARKS.md`, `sim_profile`, `docs/PERF_LARGE_MAP.md`).
Un tick vacío 4096² temperate ≈ 1,4 ms; un **día de nieve** SubArctic en 4096² ≈ **25 ms** (casi el presupuesto 1×).

## Hallazgo remap (2026-07-18)

En map_shot 256² con culling, el log muestra `↻144 chunks` casi cada frame si hay teselas dirty de landscape/industria. Causa: en `remap.rs`, cualquier `refresh_chunks` no vacío se amplía a **todos** los chunks del viewport. Ver `docs/PERF_LARGE_MAP.md`.

## Checklist de sesión

- [ ] 256² fixture, FPS culling on
- [ ] 1024² nueva partida, FPS + `visuales`
- [ ] 4096² nueva partida, FPS + `visuales`
- [ ] A/B 256² o 1024² con `OPENTTDRS_MAP_VIEWPORT_OFF=1`
- [ ] Comparar sensación con OpenTTD mismo tamaño (misma máquina)
