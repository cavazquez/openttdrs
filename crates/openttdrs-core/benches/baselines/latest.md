# Baseline headless (#116)

- Generado (UTC): `20260716T223358Z`
- Commit: `38ec06e`
- Host: `Linux 7.0.0-27-generic x86_64 GNU/Linux`
- CPU: `AMD Ryzen 5 5600G with Radeon Graphics` (12 hilos)
- Corridas: 5
- Criterion: warm-up=0.3s measurement=0.5s sample-size=10

## Tiempos medios por benchmark (ns/iter o según Criterion)

| Benchmark | mean (ns) | CV % | n |
|-----------|----------:|-----:|--:|
| `pathfinding/rail/train_line/a_to_b/cold` | 10889 | 11.67 | 5 |
| `pathfinding/rail/train_line/cold` | 6176 | 12.20 | 5 |
| `pathfinding/road/truck_bay/cold` | 6227 | 8.40 | 5 |
| `pathfinding/road/truck_bay/hot_cache` | 3521 | 19.07 | 5 |
| `sim_tick/large_256_world_gen/50` | 170878 | 4.06 | 5 |
| `sim_tick/train_pbs/100` | 1626540 | 1.98 | 5 |
| `sim_tick/train_pbs/500` | 7512880 | 6.58 | 5 |
| `sim_tick/truck_bay/100` | 48565 | 4.73 | 5 |
| `sim_tick/truck_bay/500` | 249416 | 4.16 | 5 |

## Notas

- CV% = desviación típica poblacional / media × 100 (sobre las medias Criterion de cada corrida).
- Umbrales CI agresivos: fuera de alcance; usar este informe para regresiones manuales.
- Logs crudos: `crates/openttdrs-core/benches/baselines/raw-20260716T223358Z`
