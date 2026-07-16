# Benchmarks headless (#116)

Baseline de rendimiento **sin Bevy**: tick de simulación y pathfinding en `openttdrs-core`.
Los umbrales son **informativos** (comparar distribuciones, no fallar CI por un ms).

## Cómo ejecutar

```bash
# Suite completa (Criterion; escribe target/criterion/)
# Targets Criterion (evitar `cargo bench` sin `--bench`: también corre bins)
cargo bench -p openttdrs-core --bench sim_tick
cargo bench -p openttdrs-core --bench pathfinding

# Cinco corridas + resumen de variabilidad
./scripts/bench_baseline.sh
```

Informes HTML: `target/criterion/*/report/index.html`.

## Escenarios y métricas

| Grupo Criterion | Escenario | Qué mide |
|-----------------|-----------|----------|
| `sim_tick/truck_bay/{100,500}` | parity `truck_bay` (camión + red) | N × `GameState::step` |
| `sim_tick/train_pbs/{100,500}` | parity `train_pbs` | N × tick con PBS |
| `sim_tick/large_256_world_gen/50` | mapa 256×256 + `apply_world_gen` (seed 116) | tick sobre mapa grande sin flota |
| `pathfinding/road/truck_bay/cold` | `truck_bay` | `find_path` Road load→deliver |
| `pathfinding/road/truck_bay/hot_cache` | idem + `PathCache` | hit de `find_path_cached` |
| `pathfinding/rail/train_line/cold` | `train_line` | YAPF depósito→estación A |
| `pathfinding/rail/train_line/a_to_b/cold` | `train_line` | YAPF A→B |

Throughput Criterion: elementos = ticks (sim) o 1 ruta (pathfinding).

## Baseline y variabilidad

- Hardware y commit van en el reporte de `./scripts/bench_baseline.sh` (`benches/baselines/latest.md`).
- Cinco ejecuciones independientes; el script calcula media y coeficiente de variación del tiempo medio Criterion.
- **No** se versionan goldens de tiempo (dependen de máquina). Adjuntar `latest.md` al PR cuando se cierre una medición.
- Los benches **no** escriben fixtures ni tablas generadas.

## Fuera de este harness

| Tema | Dónde |
|------|--------|
| Remap / culling viewport (Bevy) | Manual: [`scripts/bench_large_map_viewport.md`](../scripts/bench_large_map_viewport.md) |
| FPS de ventana | Cliente con `OTTDMAP_FILE=…` |
| Optimizaciones | Issues aparte; este harness solo mide |

## Perfil recomendado

Usar el perfil por defecto de `cargo bench` (release). Para smoke local más corto:

```bash
cargo bench -p openttdrs-core --bench sim_tick -- --warm-up-time 0.5 --measurement-time 1.5 --sample-size 40
```
