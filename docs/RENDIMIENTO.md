# Rendimiento

Perfiles de mapas grandes y benchmarks headless (`./scripts/check.sh bench`).

## Índice

- [Mapas grandes](#rendimiento-mapas-grandes)
- [Benchmarks](#benchmarks)

---

## Rendimiento mapas grandes

<!-- fuente: PERF_LARGE_MAP.md -->

Fecha: 2026-07-18 · Actualizado tras tope zoom/spawn + remap dirty  
Hardware: AMD Ryzen 5 9600X, 29 GiB RAM, Linux x86_64  
Presupuesto 1×: **27 000 µs/tick** (~37 Hz, ADR 0003).  
OpenTTD: Flatpak `org.openttd.OpenTTD` **15.3**, dedicated + consola `fps`.

### Resumen ejecutivo

| Área | Veredicto |
|------|-----------|
| Sim vacía temperate 1024² | **OK** (~41 µs/tick) |
| Sim vacía temperate 4096² | **OK** (~1,4 ms/tick) |
| Sim SubArctic 4096² | **OK** tras #196 (~2,0 ms media; max ~2,3 ms; sin pico diario) |
| Memoria `Tile` | +2 B/tile vs OpenTTD (~16,7 %); 4096² = 224 MiB vs ~192 MiB |
| Cliente culling | Activo ≥1024 teselas; spawn acotado a ~192² |
| Cliente zoom mínimo | Tope iso (~0,27× @ 1280×720) — AABB spawn ≤192² sin huecos |
| Cliente remap dirty | Solo chunks dirty ∩ viewport (antes: todo el viewport) |

### Fix #196 — nieve al estilo `TileLoopClearAlps`

**Antes:** `apply_seasonal_snow` barría O(W×H) cada día de tránsito → ~25 ms @ 4096² SubArctic.  
**Ahora:** franja tile-loop (`MapSize/256` teselas/tick), criterio **altura vs `DEF_SNOW_LINE_HEIGHT` (10)**, densidad gradual 0…3 (como OpenTTD `clear_cmd.cpp`).

| 4096² SubArctic | Antes | Después |
|-----------------|------:|--------:|
| media µs/tick | ~1 700 | **~2 024** |
| max µs/tick | **~25 100** (día) | **~2 278** |
| día de tránsito | ~25 ms | ~1,9 ms (sin pico) |

### Herramientas

```bash
cargo bench -p openttdrs-core --bench sim_tick -- large_
cargo run -p openttdrs-core --release --bin sim_profile -- --side 4096 --climate subarctic --ticks 160
cargo run -p openttdrs-core --release --bin map_memory -- --alloc-max 4096
./scripts/bench_openttd_flatpak.sh
MAP_BITS=12 LANDSCAPE=arctic ./scripts/bench_openttd_flatpak.sh
## Cliente: scripts/bench_large_map_viewport.md
```

### Criterion (`sim_tick`, warm 0,5 s / meas 2 s / n=20)

| Benchmark | mean (iter) | ≈ µs/tick |
|-----------|------------:|----------:|
| `large_256_world_gen/50` | 106,7 µs | **2,1** |
| `large_1024_world_gen/50` | 1,93 ms | **38,6** |
| `large_4096_world_gen/20` | 28,4 ms | **1 420** |

### `sim_profile` (tras #196)

#### Temperate

| Lado | µs/tick | max |
|-----:|--------:|----:|
| 256² | 2,6 | 3 |
| 1024² | 40,7 | 83 |
| 4096² | 1 415 | 1 656 |

#### SubArctic

| Lado | µs/tick | max | día tránsito |
|-----:|--------:|----:|-------------:|
| 1024² | 71 | 102 | ~71 |
| 4096² | **2 024** | **2 278** | ~1 936 |

### Memoria (`map_memory`)

| Lado | openttdrs | OpenTTD~ |
|-----:|----------:|---------:|
| 1024² | 14 MiB | 12 MiB |
| 4096² | 224 MiB | 192 MiB |

### Cliente Bevy

Culling ≥1024 teselas. En zoom extremo el viewport ortográfico cubría cientos de miles de teselas (p. ej. **332 928** a 0,05× → ~2–9 FPS). Mitigaciones:

1. **Tope de zoom isométrico** (`MAX_SPAWN_SPAN_TILES = 192`, `clamp_ortho_scale`): el span en teselas es `scale·(w/(2·ISO_HW)+h/(2·ISO_QH))`; a 1280×720 el máximo es ~0,27×. No se recorta el spawn (eso dejaba franjas diagonales vacías).
2. **Remap dirty** (#197): `refresh_chunks` se queda en dirty ∩ viewport (ya no se clona todo `needed`).

### Comparación OpenTTD (Flatpak 15.3)

| Mapa | Clima | openttdrs µs/tick | OpenTTD Game loop | Notas |
|------|-------|------------------:|------------------:|-------|
| 1024² | temperate | 41 | ~150 | |
| 4096² | temperate | 1 415 | ~1 850 | |
| 4096² | arctic | **~2 024** (max ~2,3 ms) | ~4 860 | ambos ≪ 27 ms; pico diario eliminado |

Script: [`scripts/bench_openttd_flatpak.sh`](../scripts/bench_openttd_flatpak.sh).

### Ranking hot paths

1. ~~`apply_seasonal_snow` O(map)/día~~ ✅ #196
2. ~~Remap dirty → viewport completo~~ ✅ (retain dirty ∩ viewport + tope spawn)
3. `tile_animation` stripe ~0,9 ms @ 4096² vacío
4. Densidad Tile +2 B (memoria)
5. CargoDist / YAPF con flota — no medido en vacío
6. LOD / atlas a zoom muy bajo (opcional; el tope de spawn ya evita el colapso)

### Issues

1. ~~[#196](https://github.com/cavazquez/openttdrs/issues/196)~~ — nieve tile-loop
2. [#197](https://github.com/cavazquez/openttdrs/issues/197) — Remap Bevy dirty → viewport (mitigado en cliente; verificar/cerrar)

## Benchmarks

<!-- fuente: BENCHMARKS.md -->

Baseline de rendimiento **sin Bevy**: tick de simulación y pathfinding en `openttdrs-core`.
Los umbrales son **informativos** (comparar distribuciones, no fallar CI por un ms).

### Cómo ejecutar

```bash
## Suite completa (Criterion; escribe target/criterion/)
## Targets Criterion (evitar `cargo bench` sin `--bench`: también corre bins)
cargo bench -p openttdrs-core --bench sim_tick
cargo bench -p openttdrs-core --bench pathfinding

## Cinco corridas + resumen de variabilidad
./scripts/bench_baseline.sh
```

Informes HTML: `target/criterion/*/report/index.html`.

### Escenarios y métricas

| Grupo Criterion | Escenario | Qué mide |
|-----------------|-----------|----------|
| `sim_tick/truck_bay/{100,500}` | parity `truck_bay` (camión + red) | N × `GameState::step` |
| `sim_tick/train_pbs/{100,500}` | parity `train_pbs` | N × tick con PBS |
| `sim_tick/large_256_world_gen/50` | mapa 256×256 + `apply_world_gen` (seed 116) | tick sobre mapa grande sin flota |
| `sim_tick/large_1024_world_gen/50` | mapa 1024×1024 procedural (clon plantilla) | tick mapa grande sin flota |
| `sim_tick/large_4096_world_gen/20` | mapa 4096×4096 (estado estable, sin clon) | tick mapa máximo sin flota |
| `pathfinding/road/truck_bay/cold` | `truck_bay` | `find_path` Road load→deliver |
| `pathfinding/road/truck_bay/hot_cache` | idem + `PathCache` | hit de `find_path_cached` |
| `pathfinding/rail/train_line/cold` | `train_line` | YAPF depósito→estación A |
| `pathfinding/rail/train_line/a_to_b/cold` | `train_line` | YAPF A→B |

Throughput Criterion: elementos = ticks (sim) o 1 ruta (pathfinding).

### Baseline y variabilidad

- Hardware y commit van en el reporte de `./scripts/bench_baseline.sh` (`benches/baselines/latest.md`).
- Cinco ejecuciones independientes; el script calcula media y coeficiente de variación del tiempo medio Criterion.
- **No** se versionan goldens de tiempo (dependen de máquina). Adjuntar `latest.md` al PR cuando se cierre una medición.
- Los benches **no** escriben fixtures ni tablas generadas.

### Perfil por fase del tick

```bash
cargo run -p openttdrs-core --release --bin sim_profile -- --side 1024 --ticks 200
cargo run -p openttdrs-core --release --bin sim_profile -- --side 1024 --climate subarctic --ticks 300
cargo run -p openttdrs-core --release --bin sim_profile -- --side 4096 --ticks 80
```

Informe de investigación mapas grandes: [`PERF_LARGE_MAP.md`](#rendimiento-mapas-grandes).
Comparación OpenTTD Flatpak: [`scripts/bench_openttd_flatpak.sh`](../scripts/bench_openttd_flatpak.sh).

### Fuera de este harness

| Tema | Dónde |
|------|--------|
| Remap / culling viewport (Bevy) | Manual: [`scripts/bench_large_map_viewport.md`](../scripts/bench_large_map_viewport.md) |
| FPS de ventana | Cliente con `OTTDMAP_FILE=…` o nueva partida 1024²/4096² |
| Densidad / RSS de mapa | `cargo run -p openttdrs-core --bin map_memory -- --alloc-max 4096` |
| Optimizaciones | Issues aparte; este harness solo mide |

### Perfil recomendado

Usar el perfil por defecto de `cargo bench` (release). Para smoke local más corto:

```bash
cargo bench -p openttdrs-core --bench sim_tick -- --warm-up-time 0.5 --measurement-time 1.5 --sample-size 40
```
