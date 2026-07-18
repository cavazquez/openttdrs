# Rendimiento mapas grandes vs OpenTTD

Fecha: 2026-07-18 · Commit: `2112f86` (+ comparación Flatpak OpenTTD 15.3)  
Hardware: AMD Ryzen 5 9600X, 29 GiB RAM, Linux x86_64  
Presupuesto 1×: **27 000 µs/tick** (~37 Hz, ADR 0003).  
OpenTTD: Flatpak `org.openttd.OpenTTD` **15.3**, dedicated + consola `fps`.

## Resumen ejecutivo

| Área | Veredicto |
|------|-----------|
| Sim vacía temperate 1024² | **OK** (~41 µs/tick ≪ 27 ms); mejor que OTTD (~150 µs) |
| Sim vacía temperate 4096² | **OK** (~1,4 ms); similar a OTTD (~1,85 ms) |
| Sim SubArctic **día de nieve** 4096² | **CRÍTICO** (~25 ms); OTTD arctic vacío promedia ~4,9 ms y mantiene 37 fps |
| Memoria `Tile` | +2 B/tile vs OpenTTD (~16,7 %); 4096² = 224 MiB vs ~192 MiB |
| Cliente culling | Activo ≥1024 teselas; en 256² ~30k teselas visibles |
| Cliente remap dirty | **CRÍTICO**: cualquier tesela dirty refresca **todo** el viewport (~144 chunks/frame) |
| OpenTTD head-to-head (sim) | Medido vía Flatpak dedicated — ver § Comparación |

## Herramientas

```bash
cargo bench -p openttdrs-core --bench sim_tick -- large_
cargo run -p openttdrs-core --release --bin sim_profile -- --side 1024 --ticks 200
cargo run -p openttdrs-core --release --bin sim_profile -- --side 4096 --climate subarctic --ticks 160
cargo run -p openttdrs-core --release --bin map_memory -- --alloc-max 4096
# Cliente: scripts/bench_large_map_viewport.md
# OpenTTD Flatpak:
./scripts/bench_openttd_flatpak.sh
MAP_BITS=12 LANDSCAPE=arctic ./scripts/bench_openttd_flatpak.sh
```

## Criterion (`sim_tick`, warm 0,5 s / meas 2 s / n=20)

| Benchmark | mean (iter) | ≈ µs/tick |
|-----------|------------:|----------:|
| `large_256_world_gen/50` | 106,7 µs | **2,1** |
| `large_1024_world_gen/50` | 1,93 ms | **38,6** |
| `large_4096_world_gen/20` | 28,4 ms | **1 420** |

## `sim_profile` (media por fase)

### Temperate (sin nieve)

| Lado | µs/tick | economy | tile_anim | max |
|-----:|--------:|--------:|----------:|----:|
| 256² | 2,6 | 1,1 | 1,2 | 3 |
| 1024² | 40,7 | 19,6 | 20,7 | 83 |
| 4096² | 1 415 | 498 | 916 | 1 656 |

Escalado ~lineal con teselas/256 (stripe). Dominan `economy_and_world` (árboles/franja) y `tile_animation` (industrias stripe).

### SubArctic — ticks de día de tránsito (nieve O(W×H))

| Lado | n días | economy µs | total µs | vs 27 ms |
|-----:|-------:|-----------:|---------:|---------:|
| 1024² | 4 | 1 537 | 1 561 | 6 % |
| 4096² | 2 | **24 068** | **25 099** | **93 %** |

Código: `apply_seasonal_snow` en [`tree_tile_loop.rs`](../crates/openttdrs-core/src/map/tree_tile_loop.rs) — barrido completo deliberado cada día.

## Memoria (`map_memory`)

| Lado | openttdrs Vec\<Tile\> | OpenTTD~ | ΔRSS alloc |
|-----:|----------------------:|---------:|-----------:|
| 1024² | 14 MiB | 12 MiB | 14 MiB |
| 4096² | 224 MiB | 192 MiB | 224 MiB |

`size_of::<Tile>() = 14` vs 12 B OpenTTD.

## Cliente Bevy (map_shot 256² fixture)

| Modo | Teselas visibles (log remap) | Notas |
|------|-----------------------------:|-------|
| Culling ON (≥1024 tiles) | **29 929** | ~144 chunks 16×16 |
| Culling OFF | (path full rebuild; RSS ~1,1 GiB en sesión) | No spamear logs `teselas visibles` igual |

Hallazgo: con culling ON, el log muestra `↻144 chunks` **cada frame** mientras la sim ensucia landscape/industria. En [`remap.rs`](../crates/openttdrs-client/src/render/world/remap.rs), si `refresh_chunks` no está vacío se reemplaza por **todos** los chunks del viewport — coste O(viewport) por tick sucio, no O(tiles dirty).

Protocolo FPS interactivo (1024²/4096²): [`scripts/bench_large_map_viewport.md`](../scripts/bench_large_map_viewport.md).

## Comparación OpenTTD (Flatpak 15.3, misma máquina)

Método: servidor dedicado (`-D`), mapa nuevo vacío (0 towns / 0 industries), seed 116, consola `fps`.
Script: [`scripts/bench_openttd_flatpak.sh`](../scripts/bench_openttd_flatpak.sh).
OpenTTD reporta **Game loop times** en ms/tick (media corta); rate esperado 37,04 fps.

| Mapa | Clima | openttdrs µs/tick | OpenTTD Game loop | OTTD rate | Notas |
|------|-------|------------------:|------------------:|-----------|-------|
| 256² | temperate | 2,6 | **30 µs** (0,03 ms) | 37,04 fps | ambos holgados |
| 1024² | temperate | 41 | **150 µs** (0,15 ms) | 37,04 fps | openttdrs más barato |
| 1024² | arctic / SubArctic | 62 (día nieve **1 560**) | **280 µs** (0,28 ms) | 37,04 fps | pico diario nuestro ≫ media OTTD |
| 4096² | temperate | 1 415 | **1 850 µs** (1,85 ms) | 37,04 fps | mismo orden de magnitud |
| 4096² | arctic / SubArctic | 1 697 (día nieve **25 100**) | **4 860 µs** (4,86 ms) | 37,04 fps | media OTTD OK; **nuestro pico diario ≈ presupuesto 1×** |

Conclusión sim: en mapas vacíos temperate/arctic **promedio**, openttdrs ya está a la par o mejor que OpenTTD 15.3. El gap de “mapa más grande igual de eficiente” no es el tick base, sino:

1. Picos de `apply_seasonal_snow` (#196) — OTTD no paga ~25 ms en un solo tick en 4096² arctic vacío.
2. Remap Bevy (#197) — fuera del game loop dedicated; afecta FPS del cliente.

FPS gráfico GUI (pan/zoom) sigue siendo medición manual en ambos clientes.

## Ranking de hot paths (con evidencia)

1. **`apply_seasonal_snow` O(map)/día** — 25 ms @ 4096² SubArctic → issue P0 sim.
2. **Remap dirty → refresh viewport completo** — ↻144 chunks/frame @ 256² → issue P0 client.
3. **`tile_animation` stripe** — ~0,9 ms @ 4096² vacío (aceptable 1×; vigilar con muchas industrias).
4. **Densidad Tile +2 B** — memoria, no CPU del tick.
5. CargoDist / YAPF con flota — **no medido** en esta pasada (mapa vacío).

## Issues

1. [#196](https://github.com/cavazquez/openttdrs/issues/196) — `apply_seasonal_snow` O(map)/día (~25 ms @ 4096² SubArctic).
2. [#197](https://github.com/cavazquez/openttdrs/issues/197) — Remap Bevy: dirty → refresh de todo el viewport cada tick.
