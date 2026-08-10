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

### Perfil SAV real: `Kale_TitleGame.sav`

Medición de estrés reproducible (2026-08-10) con la partida ignorada
`save/Kale_TitleGame.sav`: mapa 256×256, 3.293 vehículos y 245 estaciones.
No se versiona la partida; el perfil describe el caso, no es un golden de
tiempo.

```bash
RUSTC_WRAPPER= CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true \
  cargo run -p openttdrs-core --release --bin sav_profile -- \
  save/Kale_TitleGame.sav --ticks 148
```

`sav_profile` separa lectura, decode/import y cada subfase del tick, además de
informar rutas pendientes, fuentes de carga y los deltas visuales core →
cliente. El presupuesto sigue siendo 27.000 µs/tick (≈37 Hz).

| Métrica | Resultado del perfil |
|---|---:|
| Decode / import | ~149 ms / ~233 ms |
| Primer tick (rutas importadas pendientes) | ~47,8 ms |
| Media de 148 ticks | **~26,0 ms** |
| Pico periódico del día de tránsito | eliminado |
| `cargo_load` medio | ~8,1 ms (carga real importada) |

El primer tick resuelve 1.637 rutas importadas pendientes; no se limita ni se
detiene a los vehículos ya en marcha. Las búsquedas independientes se calculan
en paralelo y se aplican en el orden estable de la flota. Los trenes de
estación reutilizan el índice de ocupación de andenes y PBS reutiliza el índice
de ocupación de consistes/reservas.

El servicio automático de vehículos de carretera sigue ahora el reparto de
`RunEconomyVehicleDayProc` de OpenTTD: cada slot `index % DAY_TICKS` revisa su
fracción de la flota. Antes se lanzaba un barrido completo de depósitos y A*
al comenzar el día, lo que concentraba un pico de ~2,48 s en Kale.

Desde #311 el perfil decodifica el pool denso de `INDY`/`CAPA` igual que
`SlIterateArray()` de OpenTTD: las filas vacías (`length = 1`) avanzan el
índice del pool y no abortan el chunk. Kale informa ahora **59 industrias
(`INDY`)**, **218 estaciones con carga en espera**, **34.044 paquetes / 792.188
unidades** enlazados desde `STNN.goods` y **48.096 paquetes físicos (`CAPA`)**
decodificados. La diferencia corresponde a paquetes que no están referidos por
una cola de estación importada.

Con esa carga real, `load_vehicles` ya visita sus fuentes y el coste medio de
`cargo_load` es ~8,1 ms; el total medio sigue dentro del presupuesto de
27.000 µs/tick (≈37 Hz), aunque el primer tick conserva el pico de rutas
pendientes. La importación semántica vive en core, de modo que cliente,
herramientas y servidor parten de las mismas industrias, stock y paquetes de
estación.

#### Deltas visuales y etiquetas

Las listas `signal_tile_dirty` y `reservation_tile_dirty` son deltas de un solo
tick: se vacían al iniciar el siguiente sin tocar las colas que deben cruzar
ticks (`tile_loop_visited`, `signal_globset`). En Kale, al final de una ventana
de 148 ticks quedan 2 señales y 14 reservas, en vez de acumular todo el
historial desde la carga.

El remap incremental de Bevy sólo vuelve a crear etiquetas de pueblo, estación
y cartel cuando cambia el viewport (pan/zoom) o una construcción puede haber
cambiado una etiqueta. Un cambio de señal, catenaria o reserva PBS refresca
sus chunks, pero no hace despawn/spawn de todas las etiquetas. Si el overlay
PBS está oculto, sus reservas tampoco provocan remap.

El FPS final debe medirse en una sesión con GPU real mediante el título/HUD del
cliente. Xvfb sin adaptador WGPU no es una medición válida de render.

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

### Señales rail — índice espacial + globset acotado (#214)

La simulación construye una vez un índice ordenado de teselas con señales y lo
mantiene desde `_globset`. Los drenados posteriores recorren ese índice y solo
calculan estados/combos dentro del cierre de dependencias afectado: no vuelven a
barrer la grilla completa. Los goldens PBS/señales comparan el resultado con el
update global y Criterion cubre mapas señalizados de 1024² y 4096².

| Benchmark incremental | Señales | Tiempo medio |
|-----------------------|--------:|-------------:|
| `dense_1024` | 128 | **~271 µs** |
| `dense_4096` | 2.048 | **~4,37 ms** |

Medición local 2026-07-25 (Ryzen 5 9600X); el barrido único de inicialización
queda fuera de la iteración de Criterion.

```bash
cargo bench -p openttdrs-core --bench sim_tick -- signal_glob_indexed
```

### CargoDist — reconstrucción agrupada por tick (#215)

Las descargas actualizan primero todas las aristas de `link_graph` y ejecutan
Demand + MCF **una sola vez al final de la fase de descarga**, antes de cargar.
El runtime expone `station_flow_rebuilds` como contador diagnóstico no persistido,
y Criterion cubre una ráfaga de 128 vehículos con CargoDist asimétrico.

```bash
cargo bench -p openttdrs-core --bench sim_tick -- cargodist/unload_burst_128
```

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
| `sim_tick/cargodist/unload_burst_128` | 128 camiones descargan con CargoDist asimétrico | una reconstrucción Demand + MCF por tick |
| `signal_glob_indexed/dense_{1024,4096}` | corredores señalizados + un tren por corredor | drain incremental sin barrido completo de mapa |
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
