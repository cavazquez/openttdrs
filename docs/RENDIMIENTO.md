# Rendimiento

Perfiles de mapas grandes y benchmarks headless (`./scripts/check.sh bench`).

## Índice

- [Mapas grandes](#rendimiento-mapas-grandes)
- [Flotas grandes y presupuesto de 30 FPS](#flotas-grandes-y-presupuesto-de-30-fps)
- [Revisión de fuentes OpenTTD: mejoras y diferencias](parity/openttd-source-performance-review.md)
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
| Cliente culling | Activo ≥1024 teselas; detalle acotado a un AABB de ~192² |
| Cliente zoom fijo | Seis niveles OpenTTD: 4×, 2×, 1×, 0,5×, 0,25× y 0,125× (Out8x) |
| Cliente detalle/overview | En mapas grandes el detalle se limita a ~0,27× @ 1280×720; Out4x/Out8x siguen disponibles con resumen 4×4/8×8 cuando el viewport supera 512² |
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

1. **Tope del camino detallado** (`MAX_SPAWN_SPAN_TILES = 192`, `clamp_ortho_scale`): el span en teselas es `scale·(w/(2·ISO_HW)+h/(2·ISO_QH))`; a 1280×720 la escala máxima detallada es ~3,7, equivalente a ~0,27×. No se recorta el spawn (eso dejaba franjas diagonales vacías).
2. **Out4x/Out8x con overview**: las escalas 4 y 8 no se bloquean; si el viewport supera `OVERVIEW_DETAIL_MAX_VIEWPORT_TILES` se instancian bloques agregados 4×4/8×8. Esto conserva el mapa y las etiquetas, pero no es paridad raster completa para infraestructura y vehículos fuera del recorte detallado.
3. **Remap dirty** (#197): `refresh_chunks` se queda en dirty ∩ viewport (ya no se clona todo `needed`).

La matriz automatizada de zoom valida las seis escalas (`0.25, 0.5, 1, 2, 4, 8`) y la magnificación inversa que se muestra en el HUD. El smoke de render también materializa una capa de mapa en cada nivel:

```bash
./scripts/check.sh zoom-smoke
cargo test -p openttdrs-client --bin openttdrs-client camera::
cargo test -p openttdrs-client --bin openttdrs-client render::viewport::
```

Las capturas raster requieren un compositor WGPU presentable (Weston headless
o una GPU real); Xvfb sin adaptador no es una prueba visual válida y puede
fallar al crear la superficie FIFO.

### Perfil SAV real: `Kale_TitleGame.sav`

Medición de estrés reproducible (2026-08-10) con la partida ignorada
`save/Kale_TitleGame.sav`: mapa 256×256, 3.293 vehículos y 245 estaciones.
No se versiona la partida; el perfil describe el caso, no es un golden de
tiempo.

**Resultado histórico:** los 26 ms de esta sección no describen el checkout
actual. El perfil de flotas del 2026-09-29, más abajo, vuelve a medir esta
partida con el pipeline CargoDist actual y separa el render de la simulación.

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

En esa medición, `load_vehicles` visitaba sus fuentes y el coste medio de
`cargo_load` era ~8,1 ms; el total medio estaba dentro del presupuesto de
27.000 µs/tick (≈37 Hz), aunque el primer tick conservaba el pico de rutas
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

## Flotas grandes y presupuesto de 30 FPS

Fechas locales: 2026-09-29/30. Linux, Ryzen 5 9600X, RX 7600/Vulkan, ventana de
1280×720. Baseline: `4cf5a855`; binarios release preservados antes de los
cambios. Partida ignorada `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`:
256×256, 3.293 unidades de flota, 245 estaciones, 59 industrias y 34.044
paquetes de estación. No se versionan la partida ni las capturas.

El objetivo permanece **abierto**. Frame: 33,33 ms para 30 FPS; simulación:
27 ms/tick, sin reducir la frecuencia nativa. El issue local conserva el
alcance y los siguientes pasos en
[`runtime-fleet-performance.md`](parity/runtime-fleet-performance.md).

### Simulación aislada

Comparación de ocho ticks desde la misma carga, sin compilar ni ejecutar otros
benchmarks a la vez. `sav_profile` sin `--newgrf` mide el estado importado,
incluido el primer tick con rutas pendientes; no equivale al cliente con todos
los catálogos activos:

- Tick medio: **2.946,0 → 1.215,3 ms**, mejora de 2,42×. Sigue por encima de
  27 ms.
- Descarga: **2.555,1 → 1.034,8 ms**; representa el 85,1 % del tick nuevo.
- Carga: **105,3 → 103,9 ms**. Movimiento: **225,3 → 19,3 ms**.
- Primer tick nuevo: 1.684,6 ms, con 241,5 ms de resolución de rutas.

El principal coste es `unload_vehicles → rebuild_station_flows → Demand +
MCF`. La descarga modifica el grafo y obliga a resolver pasajeros casi cada
tick. La caché exacta por cargo evita resolver cargos sin cambios, pero no
retira ese cálculo global del hilo de simulación. El scheduler periódico
existente también ejecuta MCF sincrónicamente al hacer join.

```bash
cargo build --release -p openttdrs-core --bin sav_profile
target/release/sav_profile save/Kale_TitleGame.sav --ticks 8
OPENTTDRS_PERF_CARGODIST=1 target/release/sav_profile \
  save/Kale_TitleGame.sav --ticks 4 --newgrf
```

`--newgrf` hidrata los catálogos activos desde los directorios estándar o
`OPENTTDRS_NEWGRF_DIR`; imprime ese coste de arranque por separado. El flag
debe coincidir al comparar versiones.

La repetición con el binario final dio **1.221,2 ms/tick** sin hidratar
NewGRF (ocho ticks). Con `--newgrf`, cuatro ticks dieron **1.352,1 ms/tick**:
descarga 1.150,7 ms (85,1 %), carga 103,8 ms y movimiento 8,8 ms. La
hidratación inicial tomó 15,3 ms y se informa fuera del tiempo de tick.

### Efectos NewGRF en el cliente en marcha

Una captura de CPU con `perf record -F 99 -e cpu-clock:u --call-graph
dwarf,16384` encontró **83,55 % de tiempo propio** en
`train_consist::newgrf_vars::fill_relative_vehicle_vars`. Para los efectos
visuales se construían scopes relativos y, dentro de cada uno, los 256
desplazamientos de la variable 62: cada desplazamiento volvía a recorrer la
cadena, buscando cada enlace en toda la flota. Esta fase quedaba fuera de
`sav_profile`, que sólo mide la simulación.

Los efectos reutilizan ahora el índice de flota y la curvatura anidada recorre
cada dirección una vez. El oracle compara exactamente las tablas de los 256
desplazamientos, incluidas cadenas largas y cíclicas. El CSV registra esta
fase como `effects_ms`.

La corrección de curvatura redujo el frame medio de **15.146,4 a 1.377,1 ms**
en tres muestras de los mismos ticks `3703077..3703079`, con la cámara inicial
guardada y 125.555 sprites. El CSV posterior registra **34,93 ms de efectos**
y **1.184,24 ms de simulación**. Es una comparación corta para aislar el
bloqueo; no acredita estabilidad ni 30 FPS.

La captura posterior en marcha, con escala 2 aplicada y warmup de 30 frames,
registró diez muestras de los ticks `3703104..3703113`: **1,04 FPS**, frame
medio **964,55 ms**, p95 **1.281,00 ms**, máximo **1.758,46 ms**. Los diez
frames excedieron 33,33 ms; la simulación tomó 731,86 ms de media y los efectos
33,19 ms. Esta ejecución incluyó el muestreo de `perf`; las llamadas de MCF y
su cola reemplazan a la curvatura como mayor consumo de CPU. El objetivo
requiere retirar el solver global del hilo del cliente, reducir el coste de
carga y efectos, y resolver el vidrio de los zooms alejados.

### Ventana real e instrumentación

El modo opt-in registra CSV con `frame_ms` (entre dos entradas a First,
incluye render/present del frame previo), `update_ms` (First→Last, sin el render
posterior), subfases, tick y cantidades de sprites/meshes. Las subfases pueden
solaparse; no deben sumarse para reconstruir el tiempo de frame. Los tiempos
individuales de frame y fase tienen un desfase de un frame.

```bash
cargo build --release -p openttdrs-client
OPENTTDRS_SAV_LOAD=save/Kale_TitleGame.sav \
OPENTTDRS_DISABLE_AUDIO=1 \
OPENTTDRS_PERF_OUT=/tmp/flota-pausada.csv \
OPENTTDRS_PERF_PAUSED=1 OPENTTDRS_PERF_SCALE=2 \
OPENTTDRS_PERF_WARMUP=120 OPENTTDRS_PERF_FRAMES=300 \
target/release/openttdrs-client
```

`OPENTTDRS_PERF_PAUSED=1` congela ticks e interpolación; quitarlo permite
medir la partida en marcha. `OPENTTDRS_PERF_PAN=1` mueve la cámara durante la
ventana de muestras. Las escalas ortográficas `0.25, 0.5, 1, 2, 4, 8` son los
seis zooms soportados, con magnificación inversa en el HUD. La escala se
aplica en el frame 30, por lo que un warmup menor no acredita ese zoom.

Sin `OPENTTDRS_PERF_OUT` no se instalan sistemas de captura. Estas mediciones
no necesitan permisos de kernel para `perf`. Un CSV sin muestras tras un
timeout no es una medición de FPS.

### Cliente en marcha tras workers e índices (2026-10-01)

Comparación de `89947557` con `fc63adef`, misma fixture/NewGRF, hardware,
ventana y cámara en escala 2. Dos runs por versión en orden
antes/después/después/antes, 30 frames de calentamiento y diez muestras por
run, sin compilaciones concurrentes ni `perf record`. Datos por frame:
[client-fleet-active-20261001.csv](parity/evidence/client-fleet-active-20261001.csv).

- Baseline: frame medio 924,76 / 1.006,58 ms; 1,08 / 0,99 FPS.
- Candidato: frame medio 220,21 / 221,60 ms; 4,54 / 4,51 FPS.
  Máximos 749,40 / 739,66 ms. Los 20 frames del candidato excedieron 33,33 ms.
- Mediana de frame: baseline 966,93 / 974,99 ms; candidato
  118,29 / 121,81 ms. Con diez muestras por run, p95/p99 por rango más cercano
  coinciden con el máximo: baseline 1.678,57 / 1.599,94 ms y candidato
  749,40 / 739,66 ms. Tick/s observado entre primera y última muestra:
  baseline 1,19 / 1,00; candidato 4,51 / 4,49.
- Simulación del candidato: 134,98 / 133,85 ms por frame; efectos
  33,13 / 35,43 ms y vidrio 6,45 / 6,46 ms. Son timers de subfases, no una
  descomposición aditiva del frame ni tiempos de GPU.

La ventana es corta y los ticks medidos van de 3.703.103/104 a
3.703.112/113; no acredita una partida larga ni todos los zooms. Se conservaron
las muestras con picos. Los cambios mejoran la flota en marcha, pero el
objetivo de 30 FPS sigue abierto. La implementación y el perfil aislado por
etapas se registran en
[performance-implementation.md](parity/performance-implementation.md).

SHA256 de los ejecutables: baseline
`987aefdf5cf5e3c17839710213c039cea0c91eb073315f1c189d184122c49522`,
candidato `5bdf1260a83b767fd14a32d05d505caf6867b5aaf819da725ee2376decf9639e`.

### Cliente en marcha tras filtrar contextos visuales (2026-10-01)

Comparación aislada de `1f613aad` con el candidato de la etapa 11: misma
Kale_TitleGame.sav/NewGRF, hardware, 1280×720, cámara fija en escala 2 y audio
desactivado. Orden antes/después/después/antes, 30 frames de calentamiento y
40 muestras por run, sin compilaciones ni `perf record` concurrentes.
[Datos por frame](parity/evidence/visual-context-client-20261001.csv).

- Frame medio: 191,62 / 190,66 → 160,51 / 161,04 ms; FPS por duración media:
  5,22 / 5,24 → 6,23 / 6,21.
- Mediana: 119,85 / 120,47 → 91,66 / 90,15 ms. p95 por rango más cercano:
  391,56 / 381,31 → 348,09 / 352,13 ms. p99 coincide con el máximo en estas
  40 muestras: 721,88 / 726,38 → 699,30 / 703,67 ms.
- Efectos: 31,65 / 31,68 → 2,05 / 2,18 ms por frame; simulación:
  115,47 / 114,66 → 114,41 / 114,73 ms; vidrio:
  6,64 / 6,52 → 6,41 / 6,42 ms. Los timers de fases no suman el frame.
- Los cuatro runs avanzan de tick 3.703.103 a 3.703.142. Tick/s observado
  entre primera y última muestra: 5,23 / 5,26 → 6,24 / 6,22. Cada frame de
  ambos ejecutables excede 33,33 ms: 80/80 en cada versión.

Es una ventana corta en marcha, con los picos conservados. Las capturas
congeladas de los seis zooms comprueban otra condición y no acreditan FPS
en esos zooms. Los controles, la variación raster Out8x ya presente en el
baseline, SHA256 de binarios y pruebas de callbacks/RNG se registran en
[la etapa 11](parity/performance-implementation.md#etapa-11--contexto-visual-reducido-cuando-no-puede-ejecutarse-un-callback-f04).
El presupuesto de 30 FPS y el ritmo nativo de simulación siguen abiertos.

### Cliente en marcha tras indexar los intentos PBS (2026-10-01)

Comparación de `aa2e68ee` con la etapa 12, misma partida/NewGRF, hardware,
1280×720, escala 2, cámara fija y audio desactivado. ABBA, 30 frames de
warmup y 40 muestras por run, sin compilaciones ni `perf` concurrentes.
[Datos por frame](parity/evidence/pbs-attempt-client-20261001.csv).

- Frame medio: 160,73 / 160,87 → 78,86 / 79,01 ms; FPS por duración media:
  6,22 / 6,22 → 12,68 / 12,66.
- Mediana: 91,28 / 90,79 → 75,76 / 74,82 ms. p95 por rango más cercano:
  344,56 / 353,36 → 96,95 / 96,79 ms. p99 coincide con el máximo en estas
  40 muestras: 698,25 / 699,66 → 175,38 / 177,96 ms.
- Simulación: 114,23 / 114,37 → 32,76 / 32,77 ms; efectos:
  2,12 / 2,12 → 2,07 / 2,06 ms; vidrio: 6,63 / 6,46 → 6,46 / 6,40 ms.
  Las fases medidas no suman el frame.
- Los cuatro runs avanzan de tick 3.703.103 a 3.703.142. Tick/s observado
  entre primera y última muestra: 6,24 / 6,23 → 13,09 / 13,08.
  Los 80 frames de cada versión exceden 33,33 ms.

El movimiento del core pasa de unos 53 a 4 ms en la ventana independiente de
120 ticks; la comparación de 60 ticks normales conserva estado, eventos y
teselas. Las pruebas, binarios y medición CPU están en
[la etapa 12](parity/performance-implementation.md#etapa-12--ocupación-calculada-una-vez-por-intento-pbs-f13).
Queda coste tanto en la simulación como en el cliente. Esta ventana todavía
no alcanza 30 FPS ni certifica el ritmo nativo o los demás zooms en marcha.

### Cliente en marcha tras compartir candidatos de depósito (2026-10-01)

Comparación de `334e71c2` con la etapa 13, misma partida/NewGRF, hardware,
1280×720, escala 2, cámara fija y audio desactivado. ABBA, 30 frames de
warmup y 40 muestras por run, sin compilaciones ni `perf` concurrentes.
[Datos por frame](parity/evidence/depot-orders-client-20261001.csv).

- Frame medio: 78,80 / 78,88 → 69,22 / 70,01 ms; FPS por duración media:
  12,69 / 12,68 → 14,45 / 14,29.
- Mediana: 75,08 / 75,52 → 63,90 / 65,67 ms. p95 por rango más cercano:
  95,95 / 96,97 → 87,04 / 86,49 ms. p99 coincide con el máximo en estas
  40 muestras: 175,07 / 173,79 → 154,93 / 166,47 ms.
- Simulación: 32,65 / 32,62 → 23,93 / 23,95 ms; efectos:
  2,13 / 2,09 → 2,03 / 2,04 ms; vidrio: 6,49 / 6,49 → 6,47 / 6,47 ms.
  Los timers de fases no suman el frame.
- Los dos runs anteriores y el segundo nuevo cubren ticks
  3.703.103–3.703.142; el primero nuevo cubre 3.703.104–3.703.143.
  Tick/s observado entre primera y última muestra:
  13,10 / 13,08 → 14,92 / 14,81. Los 80 frames de cada versión exceden
  33,33 ms.

La ventana CPU independiente promedia 22,32 / 22,49 ms en 120 ticks con el
candidato. Las 61 fases normales comparadas conservan estado/eventos/teselas.
Pruebas, alcance de resolución de depósitos y SHA256 en
[la etapa 13](parity/performance-implementation.md#etapa-13--compartir-candidatos-de-depósito-al-sincronizar-órdenes-f07).
Quedan las comprobaciones de aeronaves y trabajo de presentación, además
de los demás hallazgos. Los 30 FPS y el ritmo nativo siguen abiertos.

### Cliente en marcha tras filtrar la fase aérea (2026-10-01)

Comparación de `daa8f652` con la etapa 14, misma partida/NewGRF, hardware,
1280×720, escala 2, cámara fija y audio desactivado. ABBA, 30 frames de
warmup y 40 muestras por run, sin compilaciones ni `perf` concurrentes.
[Datos por frame](parity/evidence/aircraft-phase-client-20261001.csv).

- Frame medio: 70,67 / 69,09 → 67,24 / 67,34 ms; FPS por duración media:
  14,15 / 14,47 → 14,87 / 14,85.
- Mediana: 67,11 / 63,88 → 63,71 / 62,96 ms. p95 por rango más cercano:
  87,47 / 87,07 → 83,25 / 83,42 ms. p99 coincide con el máximo en estas
  40 muestras: 167,13 / 155,95 → 162,93 / 166,84 ms.
- Simulación: 23,85 / 23,90 → 20,95 / 20,80 ms; efectos:
  2,25 / 2,04 → 2,06 / 2,05 ms; vidrio: 6,67 / 6,44 → 6,52 / 6,56 ms.
  Los timers de fases no suman el frame.
- El segundo run anterior cubre ticks 3.703.104–3.703.143; los demás,
  3.703.103–3.703.142. Tick/s observado entre primera y última muestra:
  14,66 / 14,96 → 15,43 / 15,44. Los 80 frames de cada versión exceden
  33,33 ms.

La fase previa al movimiento del core baja de unos 4,6 a 1,2 ms en 120 ticks;
el tick completo conserva variaciones en landscape. Las 61 fases normales
mantienen estado/eventos/teselas. Pruebas de accidentes y duplicados, alcance
de unidades aéreas y SHA256 en
[la etapa 14](parity/performance-implementation.md#etapa-14--despachar-la-fase-aérea-sólo-a-aeronaves-f07).
El cliente todavía no alcanza 30 FPS. La siguiente medición debe atribuir
el trabajo de presentación residual y conservar las colas del frame.

### Render congelado, seis zooms y movimiento de cámara

Para comparar ambas versiones se usó el driver de mapshot: centro `128,128`,
escala 2, `CLEAN=0`, 120 frames de warmup y 300 muestras. El límite de captura
de mapshot se fijó en 600, para que la salida del profiler ocurriera primero.
Así ambas versiones congelan ticks e interpolación desde el arranque.
En las capturas raster se verificó además la traza de cámara centrada y el
contenido del recorte: esperar 90 frames no basta si el driver omite el
ajuste de cámara al entrar al juego. Una captura con otro zoom se descartó.

- Cámara fija: **16,51 → 58,82 FPS**; p95 del frame **72,44 → 17,90 ms**.
  Después del cambio, el máximo fue 29,69 ms: 0/300 frames excedieron 33,33 ms.
  Actualizar vehículos pasó de 38,69 a 3,32 ms y el minimapa de 7,25 a 0,16 ms.
- Cámara en movimiento: **42,27 FPS**, p95 25,95 ms y máximo 71,30 ms.
  **6/300** frames excedieron el presupuesto; persisten picos de remap.
- Escalas 0.25, 0.5 y 1: aproximadamente 60 FPS, con **0/120** frames sobre
  el presupuesto en cada zoom.
- Escala 4: **28,33 FPS**, p95 37,75 ms, **116/120** frames sobre presupuesto.
  El compositor de vidrio tomó 17,18 ms de media.
- Escala 8: **18,74 FPS**, p95 55,57 ms, **120/120** sobre presupuesto.
  El compositor de vidrio tomó 28,73 ms de media.

Las capturas finales congeladas coinciden con el baseline en **0 píxeles
diferentes** en los seis zooms, más una captura `CLEAN=1` en escala 2. Las
regresiones de núcleo y cliente aportan oracles de valores, RNG y composición
visual. Una variante de optimización del vidrio se retiró al detectar una
divergencia raster: estas cifras usan el compositor original.

Estos resultados sólo acreditan el render congelado. Los costes de efectos,
simulación, joins de CargoDist y remap deben verificarse en una partida en
marcha antes de afirmar 30 FPS estables.

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
| `terminal_spatial_index/imported_{256,1024}_steady_tick_50` | mapa plano con estación importada (`StationID` MAP2) | 50 ticks estables; el setup hace un scan y la medición comprueba que sigue siendo uno |
| `signal_glob_indexed/dense_{1024,4096}` | corredores señalizados + un tren por corredor | drain incremental sin barrido completo de mapa |
| `pathfinding/road/truck_bay/cold` | `truck_bay` | una consulta `find_path` Road load→deliver; setup y `Drop` fuera del intervalo |
| `pathfinding/road/truck_bay/hot_cache` | idem + `PathCache` | hit de `find_path_cached`; miss de calentamiento, cache y `Drop` fuera del intervalo |
| `pathfinding/rail/train_line/cold` | `train_line` | una consulta YAPF depósito→estación A; setup y `Drop` fuera del intervalo |
| `pathfinding/rail/train_line/a_to_b/cold` | `train_line` | una consulta YAPF A→B; setup y `Drop` fuera del intervalo |

Throughput Criterion: elementos = ticks (sim) o 1 ruta (pathfinding).

### Baseline y variabilidad

- Hardware y commit van en el reporte de `./scripts/bench_baseline.sh` (`benches/baselines/latest.md`).
- Cinco ejecuciones independientes; el script calcula media y coeficiente de variación del tiempo medio Criterion.
- **No** se versionan goldens de tiempo (dependen de máquina). Adjuntar `latest.md` al PR cuando se cierre una medición.
- Los benches **no** escriben fixtures ni tablas generadas.

### Contrato de consulta de pathfinding (#559)

Desde #559 los cuatro workloads usan `iter_batched_ref`: el `GameState` cold,
el `PathCache` hot y la ruta `Option<Vec<TileCoord>>` que devuelve la consulta
viven hasta después de `Measurement::end`. Por tanto se mide la búsqueda,
construcción de la ruta y el hit de caché, pero no la destrucción de fixtures,
cache ni resultado. El miss que precalienta hot sigue en setup.

La regresión `criterion_batched_ref_boundary` combina un `DropProbe` con un
`Measurement` controlado y acredita que Criterion 0.8 libera tanto input como
output sólo después del intervalo. Las rutas de la fixture permanecen fijadas:
road = 18 teselas, depósito→A = 4, A→B = 15, y el hit hot coincide con el
miss inicial.

```bash
cargo bench -p openttdrs-core --bench pathfinding --locked --offline -j2 -- \
  --warm-up-time 0.3 --measurement-time 0.8 --sample-size 20
```

Nuevo baseline release local, Linux 7.0.0-31 / AMD Ryzen 5 9600X, mismo host,
perfil, escenarios y parámetros. La columna histórica fue tomada con el
contrato anterior; no representa una mejora del algoritmo ni es comparable
con la nueva medición.

| Benchmark | Histórico con destrucción | Nuevo contrato de consulta |
|-----------|---------------------------:|---------------------------:|
| `road/truck_bay/cold` | 3,481 µs | 2,213 µs |
| `road/truck_bay/hot_cache` | 1,513 µs | 39,232 ns |
| `rail/train_line/cold` | 3,768 µs | 2,275 µs |
| `rail/train_line/a_to_b/cold` | 6,300 µs | 4,934 µs |

### Terminales importadas (#558)

La fixture de `terminal_spatial_index` separa el setup (un índice ya
materializado con una estación cuyo `StationID` es 42) de 50 ticks de medición.
Ejecutar en perfil release:

```bash
cargo bench -p openttdrs-core --bench sim_tick -- terminal_spatial_index \
  --warm-up-time 0.2 --measurement-time 0.4 --sample-size 10
```

Medición local en Linux 7.0.0-31, AMD Ryzen 5 9600X; el corte previo fue
`49abccc9`. Los tiempos son la estimación central de Criterion y sirven como
evidencia reproducible de esta fixture, no como un umbral portable.

| Fixture | Antes (50 ticks) | Después (50 ticks) | scans completos, setup + 50 ticks |
|---------|-----------------:|-------------------:|----------------------------------:|
| importada 256² | 8,901 ms | 7,364 ms | 51 → 1 |
| importada 1024² | 144,94 ms | 116,85 ms | 51 → 1 |

El workload de 4096² no se incluyó en esta comparación: el criterio del issue
lo deja condicionado al presupuesto de memoria y estas dos fixtures cubren el
baseline obligatorio con la misma cardinalidad de estación importada. El
benchmark verifica además el contador para que una regresión no pueda
presentar un tiempo menor a costa de omitir la comprobación.

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
