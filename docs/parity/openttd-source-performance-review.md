# Revisión de fuentes OpenTTD: rendimiento y diferencias del port

Fecha: **2026-09-30**. Estado del port estudiado: [`89947557`](https://github.com/cavazquez/openttdrs/commit/8994755781092e2e7a6baf463a2126d9125fbebb).
Original: **OpenTTD 15.3**, commit [`14ec60f2`](https://github.com/OpenTTD/OpenTTD/commit/14ec60f248547d4d062a1160f0fc26d742319888).

El problema de las flotas grandes tiene causas concretas: recálculo global de
CargoDist dentro de la descarga, contextos NewGRF preparados con más datos de
los que se consultan y trabajo del host que serializa el mundo. El render
presenta un cuello separado en el compositor de vidrio al alejar el zoom.
Cambiar estos caminos es más prometedor que reducir detalle de vehículos o
frecuencia de simulación.

Esta revisión **no implementa esas optimizaciones ni acredita 30 FPS**.
Registra 32 hallazgos con fuente, prioridad, propuesta y validación.
P1 tiene ocho candidatos con impacto directo en presupuesto o en el diseño que
permite alcanzarlo; P2 tiene diecisiete; P3 tiene siete. Son prioridades de
trabajo, no predicciones de ganancia ni niveles de vulnerabilidad.

## Alcance y método

Se recorrieron las rutas principales de los tres crates: núcleo, cliente y
red. Incluye ticks y relojes, CargoDist/carga, NewGRF, movimiento/ocupación,
rutas/PBS/señales, render/zoom/chunks/ordenación, persistencia, generación,
comandos/preview, assets, HUD/ventanas, audio, IA/GS, economía y verificación.
El inventario contiene 802 archivos Rust (439 core, 352 cliente, 11 net),
incluidos tests y código generado; **no se afirma una inspección línea por
línea de los 802 archivos**, ni compatibilidad completa con todas las features
de OpenTTD por haber recorrido sus subsistemas.

La copia local instrumentada del oráculo tiene cambios previos. Para estudiar
el original se leyó `git show HEAD:src/...` de su commit fijado, sin modificar
esa copia ni retirar instrumentación. Los enlaces nativos de este informe
apuntan a ese commit y no a una rama que pueda cambiar. Los enlaces Rust
también fijan el estado revisado.

Tipos de evidencia:

- **Medido**: el coste de la ruta está observado en perfiles o en la sonda de
  esta revisión. No implica que la mejora propuesta ya se haya medido.
- **Fuente**: se comprobó el comportamiento/diferencia en código. Su ganancia
  potencial aún requiere benchmark.
- **Hipótesis**: hay una estructura o patrón candidato; no se demostró que
  domine el tiempo ni que deba cambiarse.

La evidencia previa de ticks, ventana/GPU, seis zooms y raster sigue en
[`RENDIMIENTO.md`](../RENDIMIENTO.md#flotas-grandes-y-presupuesto-de-30-fps)
y el objetivo abierto en
[`runtime-fleet-performance.md`](runtime-fleet-performance.md).
Aquí se incorpora la medición nueva de costes de estado; no se duplican las
tablas completas ni se reemplazan las fuentes de verdad de paridad.

## Qué enseñan las fuentes originales

El original amortiza trabajo, conserva índices y delimita cuándo publicar
resultados. CargoDist copia un grafo y calcula en un hilo; el resultado se
integra por orden y fecha de join. Las variables NewGRF se consultan a demanda.
YAPF comparte costes de segmentos con invalidación. Los vehículos mantienen
hashes espaciales. Save/load distingue captura coherente, encoding, escritura
y reconstrucción. Las ventanas separan invalidación de datos de dibujo.

No todo es asíncrono o incremental en OpenTTD. `VideoDriver::GameLoop` mantiene
`game_state_mutex` durante el tick; el dibujo también necesita ese lock.
`GameLoopPause` tiene callsites de generación y configuración NewGRF, no una
cesión dentro de cada movimiento de vehículo. `DoSave` serializa a memoria
antes de lanzar el hilo de escritura. `OnTick_Town` y `OnTick_Industry`
iteran sus pools. Copiar sólo “un hilo más” o prohibir todo bucle lineal no
resolvería el problema.

El port ya incorporó decisiones valiosas: LFSR compartido y persistido,
relojes de calendario/economía, acumulado industrial escalonado, índices de
tráfico/señales/terminales/flota, cache exacta de entradas CargoDist,
remap visible incremental, orden canónico de chunks, fast path del sort,
deduplicación de imágenes con igualdad exacta y mixer acotado.
Los benchmarks históricos de mapa **vacío** incluso muestran costes menores
que el original en ciertos tamaños; no describen esta carga de vehículos.

## Costes de estado medidos en esta revisión

Linux, Ryzen 5 9600X; biblioteca core release del estado revisado. Entrada
ignorada `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`.
Estado importado e hidratado con catálogos NewGRF, tick 3.703.074,
3.293 vehículos y 245 estaciones. No se avanza ningún tick.

Sonda reproducible: [`profile_state_costs.rs`](../../scripts/profile_state_costs.rs).
Datos: [`state-costs-20260930.csv`](evidence/state-costs-20260930.csv).
Cada operación tiene una llamada de calentamiento y cinco muestras secuenciales,
sin otro benchmark/compilación concurrente. Se mide la obtención del resultado;
su destrucción queda fuera del reloj. Las asignaciones temporales que una
función crea y destruye internamente sí quedan incluidas. Es una sonda de
latencia, no un benchmark de throughput de red ni una distribución de FPS.

- `GameState::save_json` (pretty): media **73,131 ms**, mínimo 71,777,
  máximo 74,306. Produce **95.428.781 bytes**, 95,43 MB decimales.
- `GameState::canonical_hash` (dominio v4): media **377,051 ms**, mínimo
  370,707, máximo 393,720.
- `GameState::clone` (incluye runtime actual): media **3,667 ms**, mínimo
  3,302, máximo 4,073.
- `GameState::load_json` (incluye `hydrate_runtime`): media **881,155 ms**,
  mínimo 869,038, máximo 890,783.

Estas operaciones no se suman para representar un tick: cargar no sucede por
tick y el hash del host se calcula cada 37. La serialización sí ocurre por
tick del host/dedicated. Su media por sí sola excede 33,33 ms. El JSON completo
se entrega al listener para mantener el snapshot de incorporación/recovery;
los peers reciben normalmente `AdvanceTicks { count }`.

La medida de clone ofrece una vía para explorar snapshots tipados, pero no
prueba su coste bajo ticks concurrentes, con todo el runtime activo o incluyendo
destrucción/publicación. Tampoco convierte la captura en un mecanismo correcto
de late join sin conservar su frontera de secuencia.

Reproducción desde la raíz del repositorio, con un build release actual:

```bash
cargo build --release -p openttdrs-core --lib
CORE_COST_RLIB=$(python3 - <<'PY'
from pathlib import Path
libraries = list(Path("target/release/deps").glob("libopenttdrs_core-*.rlib"))
print(max(libraries, key=lambda path: path.stat().st_mtime))
PY
)
rustc --edition=2024 scripts/profile_state_costs.rs \
  --extern "openttdrs_core=$CORE_COST_RLIB" \
  -L dependency=target/release/deps -C opt-level=3 \
  -o /tmp/openttdrs-state-costs
/tmp/openttdrs-state-costs save/Kale_TitleGame.sav > /tmp/state-costs.csv
```

El path de la partida puede cambiarse; para comparar, mantener exactamente
save, commit, catálogos y estado inicial. La salida no incluye contenido del
save, sólo tamaño, cantidades y tiempos.

## Hallazgos y mejoras propuestas

### F01 — La descarga recalcula Demand + MCF en el tick

P1 · CargoDist · Evidencia: **Medido** · Alcance estimado: grande.

**Port al auditar 89947557:** La descarga invocaba rebuild_station_flows antes de cargar. Ocupaba el 85,1 % del tick hidratado (1.150,7 de 1.352,1 ms); esas cifras describen esa versión inicial.

**Estado actual:** La [etapa 2](performance-implementation.md#etapa-2--solver-en-workers-y-frontera-de-publicación-f01f02) retiró Demand/MCF de descarga y del rollover mensual. unload_vehicles registra estadísticas y conserva los flows hasta JoinNext. Su ganancia y regresiones están en esa etapa; la simulación activa actual (~14–15 ms) no se atribuye a aquel recálculo retirado. El grafo nativo F03 y los oracles de transferencias/join siguen pendientes.

**Original:** OpenTTD acumula estadísticas en LinkGraph y programa su recálculo mediante LinkGraphSchedule; no ejecuta el solver global después de cada descarga.

**Pendiente:** Mantener la frontera ya implementada y completar los oracles de transferencias y estadísticas/componentes nativos de F03. No volver a proponer como pendiente el recálculo inmediato que ya se retiró.

**Cómo comprobarla:** Comparar flows, destino de paquetes, reservas, pagos, RNG y fechas de publicación a lo largo de producción, transferencias y un rollover mensual. Volver a medir la descarga y todo el tick.

Fuentes: [Rust: `sim_step/cargo_transfer.rs`, L1450](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/sim_step/cargo_transfer.rs#L1450);
[OpenTTD: `src/linkgraph/linkgraphschedule.cpp`, L202](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/linkgraph/linkgraphschedule.cpp#L202).

### F02 — Los jobs se calculan al hacer join

P1 · CargoDist · Evidencia: **Fuente** · Alcance estimado: grande.

**Port al auditar 89947557:** Los jobs copiaban estaciones/grafo/catálogos y ejecutaban el pipeline en el join.

**Estado actual:** La [etapa 2](performance-implementation.md#etapa-2--solver-en-workers-y-frontera-de-publicación-f01f02) prepara snapshots de entradas y lanza rayon::spawn desde PendingLinkGraphJob. Un resultado inmutable compartido se publica en la fecha del join; el cliente conserva Update/render durante la espera. Sólo el fallback tras fallo de worker reproduce el solver sincrónicamente. Persisten F03, SAV tras mutaciones y la cancelación/recursos de jobs descartados; no se acredita paridad nativa completa ni 30 FPS.

**Original:** SpawnNext crea un LinkGraphJob que usa SpawnThread; JoinNext publica en orden y en su fecha. Si no terminó, PauseControl pausa el avance autoritativo antes del join. La disponibilidad del worker no decide el tick de publicación.

**Pendiente:** Completar límites/cancelación de recursos de jobs descartados y los oracles nativos de F03/SAV. Conservar los snapshots, la publicación determinista y el render durante espera ya implementados.

**Cómo comprobarla:** Forzar workers rápidos y lentos; obtener idénticos flows y hashes al variar sus tiempos. Cubrir cancelación por carga de mundo, eliminación de estación, cola de jobs y lockstep.

Fuentes: [Rust: `sim_step/landscape.rs`, L172](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/sim_step/landscape.rs#L172);
[OpenTTD: `src/linkgraph/linkgraphjob.cpp`, L55](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/linkgraph/linkgraphjob.cpp#L55).

### F03 — El grafo del port no es el mismo que el nativo

P1 · CargoDist · Evidencia: **Fuente** · Alcance estimado: grande.

**Port:** build_job_for_cargo incorpora todas las estaciones y genera un job por cargo. Usa stock actual como supply, aceptación con demanda fija 8 y, sin supply, capacidad saliente como fallback. La identidad de nodo nace del índice de tiles ordenados.

**Original:** LinkGraph conserva StationID, supply acumulado al generar carga, estadísticas temporales y grafos que se fusionan al conectarse. SpawnNext rota un grafo elegible, en lugar de crear todos los cargos globales juntos.

**Propuesta:** Separar componentes y conservar estadísticas e identidad nativas. Tratarlo como cambio de semántica con beneficio potencial de rendimiento, no como una partición mecánica garantizada del solver actual.

**Cómo comprobarla:** Oracle con dos redes desconectadas del mismo cargo, aceptación desigual, estaciones vacías pero productivas, fusiones, cierres y compresión del grafo; comparar demanda, MCF y flows por etapa.

Fuentes: [Rust: `parity/from_game.rs`, L163](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/cargodist/parity/from_game.rs#L163);
[OpenTTD: `src/station_cmd.cpp`, L4415](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/station_cmd.cpp#L4415).

### F04 — El contexto calcula variables que nadie pidió

P1 · NewGRF · Evidencia: **Medido** · Alcance estimado: grande.

**Port:** action2_eval_ctx_for_unit_indexed materializa scopes propios y vecinos, conteos, mapas y offsets anidados de curvatura. La [etapa 39](performance-implementation.md#etapa-39--lookup-indexado-del-vehículo-para-efectos-visuales-f04f31) reutiliza FleetIndex para evitar otro scan por ID: efectos ~1,9–2,0 → 0,5 ms en dos tandas ABBA. Quedan el resolver a demanda, la caché CB10 y su cadencia nativa.

**Original:** VehicleGetVariable resuelve la variable y parámetro solicitados sobre el vehículo seleccionado. El cache grf_cache tiene entradas específicas para propiedades estables del consist.

**Propuesta:** Dar al evaluador acceso de sólo lectura a un snapshot e índice y resolver variables a demanda. Como paso acotado, evitar contextos completos cuando el motor no puede invocar el callback o preanalizar dependencias conservadoras del Action2.

**Cómo comprobarla:** Procedures 7E, registros 10E/10F, vars 61/62, scopes parent/relative, triggers, persistent registers y callbacks CB10/CB160. Verificar también que se conserve el orden y número de consumos de RNG.

Fuentes: [Rust: `train_consist/newgrf_vars.rs`, L75](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/train_consist/newgrf_vars.rs#L75);
[OpenTTD: `src/newgrf_engine.cpp`, L614](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_engine.cpp#L614).

### F05 — El host serializa el mundo después de cada tick

P1 · Red · Evidencia: **Medido** · Alcance estimado: grande.

**Port:** broadcast_tick_after_step y dedicated llaman a save_json y entregan el snapshot al listener por cada tick. La nueva sonda mide 73,13 ms y 95.428.781 bytes de JSON. El listener guarda ese JSON; el paquete de red habitual es AdvanceTicks { count }, no los 95 MB.

**Original:** La red nativa mantiene sincronización por frames, comandos y seed; el guardado completo se usa para incorporación de clientes y transferencia de mapa.

**Propuesta:** Diseñar checkpoints más un journal ordenado de comandos/ticks, o transferir un snapshot tipado económico al listener. Materializar JSON para Welcome/resync/failover manteniendo la misma frontera atómica de estado y secuencia.

**Cómo comprobarla:** Late join mientras llegan comandos y ticks, reconnect/resync, failover y seq. No reemplazar sin más por broadcast_advance: esa rama hace state.step en la copia del listener y duplicaría el cálculo de simulación.

Fuentes: [Rust: `network/plugin.rs`, L505](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/network/plugin.rs#L505);
[OpenTTD: `src/network/network_server.cpp`, L616](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/network/network_server.cpp#L616).

### F06 — El hash canónico crea todo un árbol JSON

P1 · Red · Evidencia: **Medido** · Alcance estimado: medio.

**Port:** canonical_hash usa serde_json::to_value y recorre ese árbol. La sonda da 377,05 ms por llamada. El host lo calcula cada 37 ticks; produce un pico periódico adicional. Ya se evitó ordenar de nuevo las claves de objetos respaldados por BTreeMap.

**Original:** La sincronización nativa usa estado de RNG y frames; no certifica el mismo dominio de hash completo del port.

**Propuesta:** Implementar una serialización canónica hacia el hasher sin Value intermedio, conservando los tokens, orden y representación numérica de v4. Evaluar incremental sólo después de cubrir todas las mutaciones.

**Cómo comprobarla:** Comparación exacta contra el algoritmo actual sobre fixtures, claves de HashMap en distinto orden, enteros con signo, JSON/SAV, comandos y ticks. Si cambia el dominio, versionar el protocolo y sus tests.

Fuentes: [Rust: `game_state/canonical_hash.rs`, L56](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/game_state/canonical_hash.rs#L56);
[OpenTTD: `src/network/network_server.cpp`, L639](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/network/network_server.cpp#L639).

### F07 — Un tick lento bloquea el frame de Bevy

P1 · Simulación · Evidencia: **Fuente** · Alcance estimado: grande.

**Port:** FixedUpdate ejecuta state.step y reconstruye VehicleIndex antes del Update/render. El max_delta nominal reduce acumulación bajo lag, pero no reduce el coste del tick. El limitador es una política de tiempo, no una optimización de CPU.

**Original:** VideoDriver dispone de relojes de juego y dibujo y puede usar un hilo de juego. Ambos comparten game_state_mutex: copiar ese modelo sin acotar el lock tampoco garantiza FPS.

**Propuesta:** Primero retirar MCF y serializaciones del camino del frame. Si el tick residual no entra en presupuesto, separar simulación y render mediante snapshots de presentación e input/comandos ordenados, con bloqueo corto.

**Cómo comprobarla:** Medir FPS y ticks autoritativos por segundo por separado; pausa, velocidad, interpolación, comandos y red. Objetivos distintos: frame 33,33 ms y tick nativo aproximadamente 27 ms.

Fuentes: [Rust: `src/simulation.rs`, L89](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/simulation.rs#L89);
[OpenTTD: `src/video/video_driver.cpp`, L30](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/video/video_driver.cpp#L30).

### F08 — El compositor de vidrio recorre y replica todos los sprites

P1 · Render · Evidencia: **Medido** · Alcance estimado: medio.

**Port:** sync_rail_glass_mask_proxies reconstruye dos HashMap, construye SpriteMesh por fuente y sincroniza proxies por frame. El perfil congelado atribuye 17,18 ms a esta fase en escala 4 y 28,73 ms en escala 8; el último caso sólo alcanza 18,74 FPS.

**Original:** En el blitter 8bpp, Transparent aplica remap al píxel de destino. Es una operación dependiente del orden y no equivale a agregar alpha a un sprite. El diseño GPU del port necesita preservar ese resultado.

**Propuesta:** Probar primero un índice persistente Entity→proxy que conserve las escrituras y cámaras actuales. Medir después ámbitos visibles y pases auxiliares. La variante anterior que omitía copias con change detection fue retirada por diferencias de raster.

**Cómo comprobarla:** Separar cada optimización y comparar raster y stream de parents/children en los seis zooms, movimiento, vidrio ocluido, catenaria y cambios de chunks. No dar por válido un test ECS que no verifica píxeles.

Fuentes: [Rust: `render/rail_glass_compositor.rs`, L143](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/render/rail_glass_compositor.rs#L143);
[OpenTTD: `src/blitter/8bpp_optimized.cpp`, L101](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/blitter/8bpp_optimized.cpp#L101).

La etapa 39 conserva un fallo Out2x de 406 píxeles/124 bloques, seguido de
dos parejas exactas con el mismo candidato. Fuentes y bytes de assets son
exactos; los valores de máscara por fuente también, pero cambia el orden
ECS. Ese diagnóstico no permite dispensar entradas ordenadas, dar por
probada la causa ni cerrar F08. Véase la evidencia en la etapa 39.

La [etapa 43](performance-implementation.md#etapa-43--reproducción-gpu-del-empate-parentchild-f08f31)
lee el empate en Depth32Float para una pareja parent/child real de In2x:
ambos almacenan 1048597585 y cambia el ganador al invertir el dibujo.
Dos ejecuciones GPU y controles son exactos. Sólo cierra esa reproducción;
no cambia el compositor ni atribuye aún los 204 píxeles o las otras variaciones.

La [etapa 44](performance-implementation.md#etapa-44--solapamiento-texturado-contra-el-blitter-original-f08f31)
incorpora los RGBA reales y ejecuta Draw original con etiquetas de propiedad.
El orden inverso pierde 68 píxeles de vidrio en GPU, idénticos en dos runs;
parent→child coincide con el blitter en los 13.728. El control sin offset
separa Z, pero recortaría sprites negativos y no se instala. Falta una
corrección de rango completo y el oracle de escena; F08 permanece abierto.

La [etapa 45](performance-implementation.md#etapa-45--orden-de-bins-y-localización-del-solapamiento-f08f31)
registra bins en el frame de captura sin alterar seis zooms ni las entradas.
La misma pareja entra parent→child en In2x y child→parent en Out2x. Se localizan
48 cambios de máscara y 40 del PNG histórico en su solapamiento visible; falta
el registro de bins del fallo y no se atribuyen todos sus 204 píxeles.

La [etapa 46](performance-implementation.md#etapa-46--rango-de-profundidad-conservando-clipping-candidato-retirado-f08f31)
separa Z mediante tags GPU sin cambiar clipping ni empates world. La pareja
texturada y controles pasan, pero los seis PNG repiten bit a bit el candidato
31, incluidas 114/44 coincidencias nativas perdidas en Out2x/Out4x. Se retira;
el programa vuelve exactamente a 45. Falta una referencia nativa pausada
con vehículos y metadatos para aislar los límites de la comparación de escena.

La [etapa 47](performance-implementation.md#etapa-47--referencia-nativa-pausada-con-capas-conservadas-f08f31)
obtiene esa referencia en seis zooms y dos repeticiones: tick/pose/flags
registrados estables, CLEAN=0 y todos los PNG idénticos a los históricos.
Las pérdidas 114/44 se mantienen; no se atribuyen a una carrera entre capturas
ni se acepta el candidato. Faltan muestreo/oclusión nativos y el contrato de
importación/compositor completo.

La [etapa 48](performance-implementation.md#etapa-48--muestreo-nearest-frente-al-blitter-en-seis-zooms-f08f31)
reproduce una diferencia independiente de Z: nearest toma texeles distintos
al blitter cuando aleja el zoom. Dos máscaras alineadas difieren en Out2x,
Out4x y Out8x. Ajustar muestra y dimensiones coincide en las doce máscaras
nativas de la sonda, en dos runs. No cambia el compositor ni atribuye las
pérdidas de escena; faltan fase real, atlas y clipping.

Las etapas [49](performance-implementation.md#etapa-49--raíces-nativas-cargadas-y-bancos-de-zoom-reales-f08f31)
y [50](performance-implementation.md#etapa-50--selector-explícito-de-correcciones-de-captura-f08f31)
verifican las raíces cargadas y hacen explícito el alcance de las variantes:
se usan sólo en capturas. El selector conserva seis controles exactos y
permite medir la ruta sin correcciones, con diferencias amplias en los zooms
lejanos. No certifica la sesión interactiva ni acredita FPS nuevos.

La [etapa 49](performance-implementation.md#etapa-49--raíces-nativas-cargadas-y-bancos-de-zoom-reales-f08f31)
confirma las dos raíces reales cargadas y ocho crops de bancos en dos runs.
Out2x ya compensa el paso del blitter en sus RGBA; la sonda 48 no se instala
sobre esas texturas. Las correcciones de atlas/posición están limitadas a
MAP_SHOT: una captura nativa no certifica la ruta visual interactiva. Faltan
posición/clipping y una comparación explícita sin esas correcciones.

### F09 — Var 62 usa crashed para el bit de Hidden

P2 · NewGRF · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** vehicle_relative_curvature establece 0x80 cuando candidate.crashed es true. El modelo y el import distinguen Stopped y Crashed, pero esta función no consulta Hidden.

**Original:** El bit 7 de var 62 corresponde a VehState::Hidden. Un vehículo estrellado visible y uno sano oculto no representan el mismo valor.

**Propuesta:** Modelar/hidratar la visibilidad nativa requerida por el resolver y devolver ese bit. Mantener separada la corrección semántica del refactor de rendimiento.

**Cómo comprobarla:** Oracle con vehículo oculto y sano, visible y estrellado, depósito y túnel. El test del cambio anterior compara con el scan viejo del port y no acredita esta paridad nativa.

Fuentes: [Rust: `train_consist/newgrf_vars.rs`, L706](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/train_consist/newgrf_vars.rs#L706);
[OpenTTD: `src/newgrf_engine.cpp`, L637](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_engine.cpp#L637).

### F10 — Var 62 resta tiles en lugar de coordenadas físicas

P2 · NewGRF · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** Los bytes dx/dy de vehicle_relative_curvature se calculan con Vehicle.pos, que es TileCoord. El modelo ya conserva road_x/road_y y otras coordenadas continuas; la función no las usa.

**Original:** Var 62 resta x_pos/y_pos en píxeles del mundo (16 unidades por tile), y z_pos. Dos unidades dentro de la misma tile pueden tener un desplazamiento distinto de cero.

**Propuesta:** Usar una fuente física unificada por tipo de vehículo, incluyendo la pose de tren, y conservar signo, wrapping y dirección nativos.

**Cómo comprobarla:** Unidades en la misma tile pero distintas subcoordenadas, offsets positivos/negativos, curvas, rampas y túneles. Comparar la palabra zzyyxxFD completa, no sólo su nibble de dirección.

Fuentes: [Rust: `train_consist/newgrf_vars.rs`, L706](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/train_consist/newgrf_vars.rs#L706);
[OpenTTD: `src/newgrf_engine.cpp`, L657](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_engine.cpp#L657).

### F11 — Los scopes relativos quedan limitados a quince vecinos

P2 · NewGRF · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** El builder materializa self y ±15 vecinos. read_action2_var busca var 61/62 en esas tablas y devuelve None cuando falta la entrada; no aplica allí el filtro nativo de callbacks permitidos para 61.

**Original:** Var 62 permite un desplazamiento signed byte completo. Var 61 usa el registro 10F, restringe los callbacks que pueden leerla y devuelve cero disponible cuando Move no encuentra el vehículo.

**Propuesta:** Cubrir todo el dominio del resolver y la distinción entre cero y variable no disponible. El resolver a demanda de F04 permite hacerlo sin expandir todas las tablas.

**Cómo comprobarla:** Offsets 16, -16, 127 y -128 en cadenas largas; destino ausente; var 61 en callbacks admitidos y de cambio de propiedades; prohibición de pedir 61 a través de 61.

Fuentes: [Rust: `newgrf_sprites/action2.rs`, L80](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/newgrf_sprites/action2.rs#L80);
[OpenTTD: `src/newgrf_engine.cpp`, L614](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_engine.cpp#L614).

### F12 — La carga residual ya excede el presupuesto del tick

P2 · Carga · Evidencia: **Medido** · Alcance estimado: medio.

**Port:** Con NewGRF hidratado load_vehicles toma 103,8 ms por tick en la medición previa. Aunque el solver desapareciera del tick, esta fase y los efectos seguirían excediendo el objetivo.

**Original:** CargoList mantiene paquetes, reservas y reglas de LoadUnloadVehicle; origen, tránsito, pagos y next hop tienen significado propio.

**Propuesta:** Perfilar subfases de carga: búsqueda de terminal, listas por next hop, reservas, agregados de stock y contexto de callback. Reutilizar índices y scratch buffers donde el perfil los justifique.

**Cómo comprobarla:** Conservar orden FIFO/MTA, reservas, envejecimiento, cantidades por origen, transfer credit, orden de atención de vehículos y RNG. No reemplazar paquetes físicos por un stock agregado.

Fuentes: [Rust: `sim_step/cargo_transfer.rs`, L1457](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/sim_step/cargo_transfer.rs#L1457);
[OpenTTD: `src/economy.cpp`, L1615](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/economy.cpp#L1615).

### F13 — Quedan builders que reconstruyen FleetIndex por consulta

P2 · Flota · Evidencia: **Fuente** · Alcance estimado: corto.

**Port:** action2_eval_ctx_for_unit reconstruye FleetIndex para toda la flota. La API indexed ya se usa en render y efectos, pero la API de conveniencia continúa disponible y se invoca desde otras rutas core. La reutilización debe medirse por callsite; la presencia del wrapper por sí sola no cuantifica un coste.

**Original:** Las unidades tienen ID de pool y enlaces Previous/Next; consultar una unidad no exige volver a indexar el pool entero.

**Propuesta:** Llevar el índice vigente a los callbacks restantes y delimitar cuándo queda inválido por alta, baja, acople o reordenamiento. Conservar el wrapper para consultas aisladas y previews.

**Cómo comprobarla:** Contar rebuilds por fase y comparar contra el wrapper en flotas con IDs dispersos, altas/bajas y cadenas. No extender la vida de un índice de slots a través de cambios de Vec.

Fuentes: [Rust: `train_consist/newgrf_vars.rs`, L53](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/train_consist/newgrf_vars.rs#L53);
[OpenTTD: `src/vehicle.cpp`, L426](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/vehicle.cpp#L426).

### F14 — Las bahías viales siguen buscando en toda la flota

P2 · Flota · Evidencia: **Hipótesis** · Alcance estimado: corto.

**Port:** RoadTrafficIndex acota RoadVehFindCloseTo y se actualiza secuencialmente durante el movimiento. allocate_bay y bay_entrance_busy todavía recorren vehicles para encontrar usuarios de una parada.

**Original:** El original conserva estructuras de parada y hashes por tile para localizar vehículos. La ocupación local no exige un barrido global.

**Propuesta:** Medir paradas con muchas llegadas y, si domina, reutilizar una consulta de ocupantes por parada/tile para la asignación de bahías.

**Cómo comprobarla:** Mantener primero far/near, lado de circulación, frames de entrada/salida y orden secuencial. El movimiento completo medido sólo toma 8,8 ms: esto no explica el cuello principal actual.

Fuentes: [Rust: `road_movement/controller.rs`, L381](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/road_movement/controller.rs#L381);
[OpenTTD: `src/vehicle.cpp`, L592](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/vehicle.cpp#L592).

### F15 — La caché de rutas se vacía cada tick

P2 · Rutas · Evidencia: **Fuente** · Alcance estimado: grande.

**Port:** PathCache tiene máximo 256 entradas y begin_tick limpia el mapa al cambiar tick. YAPF calcula cachés de coste dentro de cada búsqueda. Hay reutilización y límites; no hay que describir esta ruta como totalmente sin caché.

**Original:** YAPF comparte una caché de segmentos y la invalida con s_rail_change_counter cuando cambia vía o configuración de señales.

**Propuesta:** Explorar caché de topología/segmentos estable entre ticks, separada de ocupación y reservas dinámicas. Añadir métricas de consultas, nodos expandidos, hit ratio y resultados fallidos.

**Cómo comprobarla:** Cambios de vías, señales, puentes/túneles, railtypes y costes navales; probar cached versus uncached. No retener rutas completas cuyo coste depende de reservas sin invalidación suficiente.

Fuentes: [Rust: `pathfinder/cache.rs`, L33](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/pathfinder/cache.rs#L33);
[OpenTTD: `src/pathfinder/yapf/yapf_costcache.hpp`, L131](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/pathfinder/yapf/yapf_costcache.hpp#L131).

### F16 — La búsqueda PBS clona el camino en cada expansión

P2 · Rutas · Evidencia: **Hipótesis** · Alcance estimado: medio.

**Port:** find_path_to_safe_wait_with_wormholes_impl guarda path: Vec en la cola y clona path_so_far para cada vecino. El algoritmo tiene límites de nodos y longitud; aun así puede asignar mucho ante señales bloqueadas.

**Original:** Los nodos YAPF conservan un parent y reconstruyen la ruta, además de datos de segmento y estado direccional.

**Propuesta:** Medir asignaciones y sustituir copias por un arena de nodos/predecesores si el coste lo justifica. Incluir en la clave todo estado que realmente altera las transiciones.

**Cómo comprobarla:** Empates de coste, posición segura, passed_path, reservas cruzadas, ruta preferida, túneles y límite de búsqueda. Mantener exactamente qué camino gana.

Fuentes: [Rust: `rail_pbs/search.rs`, L235](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/rail_pbs/search.rs#L235);
[OpenTTD: `src/pathfinder/yapf/yapf_rail.cpp`, L559](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/pathfinder/yapf/yapf_rail.cpp#L559).

### F17 — El pan produce picos al rematerializar chunks

P2 · Render · Evidencia: **Medido** · Alcance estimado: medio.

**Port:** El remap ya refresca sólo chunks dirty visibles y preserva vehículos/FX. La medición congelada con pan da 42,27 FPS medios, pero máximo 71,30 ms y 6 de 300 frames fuera de presupuesto. Refrescar un chunk implica despawn/spawn de sus estáticos.

**Original:** El viewport usa productores y áreas sucias; no tiene exactamente el mismo coste de materialización persistente de entidades ECS.

**Propuesta:** Actualizar sólo productores alterados dentro de chunks, o preparar chunks nuevos en lotes acotados con publicación coherente. Reutilizar entidades/meshes si el perfil lo confirma.

**Cómo comprobarla:** Mismo raster y orden geométrico canónico al salir/entrar en viewport, construir/demoler, variar zoom y mostrar overlays. No repartir la publicación de modo que aparezca un mapa parcialmente incoherente.

Fuentes: [Rust: `world/remap.rs`, L65](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/render/world/remap.rs#L65);
[OpenTTD: `src/viewport.cpp`, L1811](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/viewport.cpp#L1811).

### F18 — El sort ya tiene caché y no es un Z plano

P2 · Render · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** sort_viewport_sortable_parents usa scope, fingerprints, referencias changed y proxies segmentados. En escena estable congelada sort cuesta aproximadamente 0,1–0,4 ms; con movimiento se midieron unos 4–14 ms.

**Original:** El ordenador del viewport compara bounds de parents y trata children/insertion order. No equivale a ordenar cada sprite sólo por tile o por profundidad flotante.

**Propuesta:** Mantener el fast path estable y medir el cierre de dependencias entre dinámicos y estáticos antes de hacer sort incremental por bandas.

**Cómo comprobarla:** Orden de parents/children, promoción, segmentación, bounds, inserción y seis zooms. No usar una lectura histórica de 43 ms como coste actual estable.

Fuentes: [Rust: `render/house_viewport_sort.rs`, L1241](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/render/house_viewport_sort.rs#L1241);
[OpenTTD: `src/viewport.cpp`, L1596](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/viewport.cpp#L1596).

La [etapa 36](performance-implementation.md#etapa-36--no-invalidar-poses-estables-de-proxies-f18f31)
evita invalidaciones falsas de pose/ancla/visibilidad y conserva reparación y
borrado diferido. Seis zooms y entradas completas coinciden; las dos tandas de
tiempos no acreditan ganancia clara de FPS. La
[etapa 37](performance-implementation.md#etapa-37--conservar-el-único-parent-segmentado-f08f18)
corrige la desaparición de una fuente segmentada sin parents globales:
regresión de seis bandas, sonda geométrica con las funciones nativas y
conservación de Kale en seis zooms. No se cierra F18 ni el contrato nativo
completo. La
[etapa 38](performance-implementation.md#etapa-38--no-invalidar-sprites-iguales-de-proxies-f18f31)
evita flags falsos de Sprite ordinarios reutilizados, con comparación raw y
reparación de cambios externos. Dos tandas no acreditan ganancia sostenida de
FPS; slices conservan el refresco anterior y el resto de F18 sigue abierto.

La [etapa 40](performance-implementation.md#etapa-40--orden-estable-al-retirar-proxies-segmentados-f08f18)
fija el orden del CommandQueue al retirar sobrantes por banda/fuente. La
regresión del sistema falla antes y pasa en ambas ramas; seis zooms y tres
parejas Out2x son exactos. No prueba la causa de la variación 39, no elimina
los empates f32 ni acredita mejora de FPS. F08/F18 permanecen abiertos.

La [etapa 41](performance-implementation.md#etapa-41--conservar-los-grupos-ordenados-de-children-f18f31)
conserva grupos por parent y ordena sólo los afectados, recuperando bajas
con mensajes expirados. Las regresiones diferenciales y seis zooms pasan;
children baja ~0,12–0,13 ms. El frame fijo mejora poco y pan no mejora
consistentemente: se conservan ambas tandas y sus picos. F18 sigue abierto.

La [etapa 42](performance-implementation.md#etapa-42--preparación-compartida-de-recorte-retirada-f18f31)
ensayó preparar geometría de recorte una vez por fuente. Se retiró: dos
ABBA empeoran el frame y no reducen consistentemente sort. El diferencial
CPU pasa, pero In2x inicial falla 204 píxeles y dos repeticiones son exactas.
Se conservan todos los resultados; producción vuelve a la etapa 41.

### F19 — El guardado bloquea UI y codifica dos veces el JSON

P2 · Persistencia · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** Las acciones de persistencia llaman al guardado desde Update. save_with_limit recorre serde una vez para contar bytes y otra para escribir. El reemplazo usa temporal, sync_all y preservación del destino anterior, garantías que conviene conservar.

**Original:** DoSave serializa chunks en memoria y luego desplaza escritura/compresión a ottd:savegame. La captura/serialización inicial también tiene coste en el hilo de juego.

**Propuesta:** Capturar un snapshot coherente y hacer encoding/I/O en un job con progreso y resultado visible. Estudiar un buffer codificado único o spool limitado, preservando cuota y atomicidad.

**Cómo comprobarla:** Disco lleno, cuota, fallo de encoding y permisos; save anterior intacto. Medir serialización e I/O separados. No adjudicar los 73 ms de save_json al guardado versionado completo: es otro camino.

Fuentes: [Rust: `save/io.rs`, L140](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/save/io.rs#L140);
[OpenTTD: `src/saveload/saveload.cpp`, L3046](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/saveload/saveload.cpp#L3046).

### F20 — Cargar JSON incluye reconstrucción de runtime

P2 · Persistencia · Evidencia: **Medido** · Alcance estimado: medio.

**Port:** GameState::load_json parsea y llama hydrate_runtime, que reconstruye flows y sanitiza órdenes. La sonda tarda 881,15 ms. El loader versionado adicionalmente convierte un Value completo a GameState; el camino legacy puede volver a parsear el texto.

**Original:** El SAV usa chunks y una etapa explícita AfterLoad que repara vínculos, caches y versiones; no es sólo leer bytes.

**Propuesta:** Separar lectura/decode/migración/hidratación/publicación y preparar el nuevo mundo fuera de Update. Deserializar el envelope directamente si se conserva la detección legacy y los límites.

**Cómo comprobarla:** Compatibilidad de versiones, órdenes, catálogos NewGRF, runtime, hashes y estado previo ante un error. Publicar el mundo una única vez al terminar.

Fuentes: [Rust: `game_state/mod.rs`, L1257](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/game_state/mod.rs#L1257);
[OpenTTD: `src/saveload/afterload.cpp`, L1](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/saveload/afterload.cpp#L1).

### F21 — Flows y jobs futuros están fuera del estado persistido

P2 · Persistencia · Evidencia: **Fuente** · Alcance estimado: grande.

**Port:** station_flows y pending_linkgraph_jobs pertenecen a SimulationRuntime, excluido de JSON/hash. hydrate_runtime reconstruye flows desde el grafo y crea runtime nuevo. Es una frontera que requiere revisión antes de introducir workers.

**Original:** El original tiene chunks LGRJ/LGRS y controles de pausa afterload para jobs pendientes; además comprueba que estaciones/grafos sigan válidos al integrar.

**Propuesta:** Definir qué es caché derivable y qué condiciona el futuro: snapshot de entrada, orden, join_date y último flow publicado. Persistir o reconstruir de manera equivalente esa información; los handles de hilo deben quedar efímeros.

**Cómo comprobarla:** Comparar continuidad sin guardado versus guardar/cargar entre spawn y join, además de late join/failover. Esto identifica un riesgo de continuidad; no se afirma una desincronización reproducida en esta revisión.

Fuentes: [Rust: `game_state/runtime.rs`, L269](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/game_state/runtime.rs#L269);
[OpenTTD: `src/saveload/linkgraph_sl.cpp`, L289](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/saveload/linkgraph_sl.cpp#L289).

### F22 — Generar/regenerar terreno es síncrono desde la interfaz

P2 · Generación · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** El menú crea SimWorld::from_new_game directamente y el editor aplica RegenerateLandscape dentro de la acción de UI. Ambos realizan trabajo completo antes de devolver el control.

**Original:** GenerateWorld se ejecuta en la ruta de juego y las etapas actualizan progreso; genworld_gui llama GameLoopPause para ceder el lock y permitir dibujo. En 15.3 no es un worker independiente creado por genworld.cpp.

**Propuesta:** Construir un mundo nuevo en job puro o en etapas con presupuesto, progreso y cancelación. Tratar una regeneración autoritativa de red como operación ordenada, con transición de generación explícita.

**Cómo comprobarla:** Semilla, estado final del RNG, fase de población/startup tile loop, rollback de cancelación, comandos durante generación y comparación por teselas/bloques.

Fuentes: [Rust: `systems/session.rs`, L122](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/ui/main_menu/systems/session.rs#L122);
[OpenTTD: `src/genworld_gui.cpp`, L1512](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/genworld_gui.cpp#L1512).

### F23 — La caché de imágenes deduplica, pero puede crecer durante la sesión

P2 · Assets · Evidencia: **Hipótesis** · Alcance estimado: medio.

**Port:** NewGrfTrainSpriteCache deduplica el resultado RGBA por fingerprint y verifica igualdad completa del Image. Action2 se sigue resolviendo cada vez; los handles permanecen hasta clear y no hay cuota visible en esa estructura.

**Original:** SpriteCache administra memoria de sprites y reutiliza representaciones decodificadas; el cache de variables del vehículo es otra capa.

**Propuesta:** Medir bytes/hits/misses/resolutions en sesiones largas con cambios de librea, carga y animación. Separar el coste del resolver del horneado e incorporar límites/evicción sólo si el crecimiento medido lo exige.

**Cómo comprobarla:** Colisiones de fingerprint, reutilización entre motores idénticos, remaps, swap de NewGRF y liberación de assets vivos. No usar el hash de contexto como prueba de igualdad de píxeles.

Fuentes: [Rust: `vehicles/assets.rs`, L237](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/render/vehicles/assets.rs#L237);
[OpenTTD: `src/spritecache.cpp`, L1](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/spritecache.cpp#L1).

### F24 — El HUD evita escribir texto, pero primero construye una clave costosa

P3 · UI · Evidencia: **Fuente** · Alcance estimado: corto.

**Port:** update_tile_info_text tiene early return por TileInfoHudKey. Antes del retorno cuenta vehículos activos y copia/forma varias cadenas; tras invalidación vuelve a calcular ciertos agregados. sync_window_title también evita escrituras si nada cambió.

**Original:** Las ventanas separan invalidación de datos y dibujo. Un dirty de comando se propaga por dominio, sin requerir reconstruir todo el resumen para comprobar si cambió.

**Propuesta:** Cachear resúmenes por tick/revisión y separar estado del HUD global de la tile seleccionada. Medir UI con ventanas de flota/órdenes abiertas.

**Cómo comprobarla:** Dinero, carga, feedback, selección, locale y zoom actualizados en el momento correcto. No reducir frecuencia de input para ahorrar un barrido de resumen.

La [etapa 51](performance-implementation.md#etapa-51--perfil-actual-de-cpu-activa-y-pausada-f07f24f31)
observa VehicleOperationalSummary::analyze en 1,99 % de la CPU muestreada
pausada. El HUD técnico arranca oculto y aun así prepara su texto cada frame.
Se corregirá primero ese trabajo, conservando el refresco inmediato al
mostrarlo; el porcentaje no acredita una ganancia de FPS.

La [etapa 52](performance-implementation.md#etapa-52--evitar-preparar-el-hud-técnico-oculto-f24)
cierra ese subconjunto: el HUD oculto retorna antes de preparar datos y se
refresca al mostrarlo, con regresión de texto/pose/flags y seis zooms exactos.
Dos ABBA dan una mejora global pequeña; la segunda conserva ~24,45 FPS fijo
y ~21,58 pan con muchos frames fuera del presupuesto. El HUD visible y el
objetivo de 30 FPS siguen pendientes; no se atribuye todo el ahorro al HUD.

Fuentes: [Rust: `display/mod.rs`, L328](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/ui/hud/display/mod.rs#L328);
[OpenTTD: `src/window.cpp`, L3237](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/window.cpp#L3237).

### F25 — La planificación de IA puede concentrarse en el cambio de mes

P3 · IA/GS · Evidencia: **Hipótesis** · Alcance estimado: medio.

**Port:** El mantenimiento y drenado ocurren por tick; TransCargo y RoadHaul planifican en new_month. record_build_commands clona GameState y ejecuta la construcción en ese clon. La sonda mide 3,67 ms para clone con runtime, sin destrucción del resultado.

**Original:** La VM de scripts tiene suspensión y presupuesto de opcodes. Controla trabajo por avance, en lugar de asumir que toda decisión mensual es breve.

**Propuesta:** Medir meses con todos los rivales activos. Convertir planificación/búsqueda en etapas con cuota determinista de operaciones, o usar un snapshot de planificación reducido.

**Cómo comprobarla:** Misma política, orden de comandos, costes/revalidación al construir y resultados repetibles al variar FPS. No usar una cuota de milisegundos de pared para decidir resultados de la IA.

Fuentes: [Rust: `ai/build_queue.rs`, L143](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/ai/build_queue.rs#L143);
[OpenTTD: `src/script/script_instance.cpp`, L185](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/script/script_instance.cpp#L185).

### F26 — AI y GameScript-lite tienen un alcance distinto

P3 · IA/GS · Evidencia: **Fuente** · Alcance estimado: grande.

**Port:** CompanyAi genera Commands para políticas Rust; gs implementa objetivos, historias y ligas sin VM Squirrel. Sus costes y APIs no representan la ejecución de scripts nativos de OpenTTD.

**Original:** ScriptInstance ejecuta una VM con callbacks, save/load, límites y suspensión; OpenTTD permite ecosistemas NoAI/GS externos.

**Propuesta:** Documentar esa diferencia de producto y distinguir paridad conceptual de compatibilidad de scripts. Si se amplía la compatibilidad, incorporar un presupuesto determinista desde el diseño.

**Cómo comprobarla:** Contratos de Commands, save/load de objetivos y política de suspensión; benchmarks de scripts reales sólo cuando exista ese runtime.

Fuentes: [Rust: `gs/mod.rs`, L1](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/gs/mod.rs#L1);
[OpenTTD: `src/script/script_instance.cpp`, L223](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/script/script_instance.cpp#L223).

### F27 — El audio está acotado; los eventos aún hacen búsquedas lineales

P3 · Audio · Evidencia: **Fuente** · Alcance estimado: corto.

**Port:** WorldSfxHandles cachea sonidos y SfxMixer tiene ocho canales. El puente de SimEvent conserva FX también con audio desactivado. Helpers de salida/choque buscan vehículos por id en la flota.

**Original:** El mixer nativo también tiene ocho canales. Los callbacks y eventos del juego no se deben eliminar sólo por omitir reproducción.

**Propuesta:** Reutilizar el índice de flota al procesar eventos si su frecuencia lo justifica. Mantener caches de muestras y máximo de canales; audio no está demostrado como cuello de botella.

**Cómo comprobarla:** Prioridad/robo de canal, Action11, audio desactivado, FX visibles y RNG de callbacks. Medir evento y reproducción por separado.

Fuentes: [Rust: `audio/sim_events.rs`, L98](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/audio/sim_events.rs#L98);
[OpenTTD: `src/mixer.cpp`, L41](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/mixer.cpp#L41).

### F28 — El tile loop y los relojes ya amortizan trabajo correctamente

P3 · Mapa/economía · Evidencia: **Fuente** · Alcance estimado: corto.

**Port:** El runtime usa visitas LFSR compartidas, relojes separados de calendario/economía y acumulado industrial por (tick + IndustryID) % DAY_TICKS. El helper antiguo de barrido completo de mapas pequeños no tiene consumidores de producción encontrados, sólo tests.

**Original:** RunTileLoop visita MapSize/256 con secuencia LFSR persistida y trata tile 0 aparte. OnTick_Industry escalona acumulación por ID; OnTick_Town sí itera towns.

**Propuesta:** Conservar estas decisiones. Medir asignaciones de visitas y reutilizar capacidad si aparece en el perfil. No atribuir el problema actual a un full scan de ese helper sin un callsite activo.

**Cómo comprobarla:** Secuencia de tiles, contador persistido, tile 0, RNG por visita, calendario/wallclock y producción mensual. Cambiar cadencia cambia el juego.

Fuentes: [Rust: `map/tile_loop.rs`, L174](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/map/tile_loop.rs#L174);
[OpenTTD: `src/landscape.cpp`, L795](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/landscape.cpp#L795).

### F29 — Preview y ejecución deben compartir reglas sin efectos secundarios

P3 · Comandos · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** command/preview.rs usa comprobaciones de sólo lectura y command/apply.rs ejecuta por dominio. La validación de presupuesto, slope y callbacks tiene muchas rutas; cachear preview sólo por tile sería insuficiente.

**Original:** DoCommand separa prueba y ejecución y preserva su contrato en multiplayer. OnInvalidateData también impone restricciones a consultas de GUI para evitar efectos en el estado.

**Propuesta:** Reutilizar un plan/check de comando y cachear previews por inputs y revisión de dependencias, si el perfil de arrastre lo requiere. Conservar coste y rechazo coherentes.

**Cómo comprobarla:** Preview sin mutar RNG/PSA/dinero, rechazo idéntico en apply y revalidación cuando cambia el mundo. Medir herramientas de terraform/stations durante arrastre.

Fuentes: [Rust: `command/preview.rs`, L90](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/command/preview.rs#L90);
[OpenTTD: `src/window.cpp`, L3278](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/window.cpp#L3278).

### F30 — Paridad del port anterior no equivale a paridad nativa

P2 · Verificación · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** Hay tests, fixtures, oracles y comparadores por fases. La prueba nueva de curvatura valida el reemplazo contra el scan anterior, y detecta regresiones del refactor; no detecta F09/F10 porque ambos usan la misma función de empaquetado.

**Original:** El commit original fijado permite extraer casos y salidas independientes. El oráculo instrumentado local tiene cambios y no debe confundirse con la fuente original pristine.

**Propuesta:** Etiquetar pruebas por contrato: equivalencia interna, nativo semántico, draw stream, raster y continuidad. Añadir casos independientes de 61/62 antes del resolver nuevo.

**Cómo comprobarla:** Contrajemplos con Hidden y subcoordenadas, ticks antes/después de un join y guardado. Registrar versión/GRF/semilla y qué ámbito acredita cada evidencia.

Fuentes: [Rust: `train_consist/newgrf_vars.rs`, L645](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/train_consist/newgrf_vars.rs#L645);
[OpenTTD: `src/newgrf_engine.cpp`, L637](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_engine.cpp#L637).

### F31 — Las fases de CPU no cubren todo el frame

P2 · Verificación · Evidencia: **Fuente** · Alcance estimado: corto.

**Port:** El CSV instrumenta nueve fases y el frame de pared. Phase::Simulation rodea state.step y el índice, pero no incluye save_json/hash del host. El trabajo del render thread y GPU tampoco queda explicado por sumar timers del hilo principal.

**Original:** OpenTTD separa mediciones de game loop, draw loop y categorías; en una comparación se deben alinear escenarios y unidades, no sólo sus nombres.

**Propuesta:** Añadir medición separada de red/serialización/UI, upload/extract/GPU y tamaño de colas cuando corresponda. Mantener sampling de perf como evidencia independiente.

**Cómo comprobarla:** Mismo save/hash, tick inicial, catálogos, cámara, zoom, resolución y calentamiento. Reportar p50/p95/p99/máximo, frames >33,33 ms y ticks/s; no extrapolar un mapa vacío a una flota.

Fuentes: [Rust: `src/performance.rs`, L25](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-client/src/performance.rs#L25);
[OpenTTD: `src/framerate_type.h`, L1](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/framerate_type.h#L1).

La [etapa 35](performance-implementation.md#etapa-35--conservar-la-escena-al-activar-el-colector-f31)
elimina el registro temprano del colector en ECS y conserva la matriz de seis
zooms, con entradas/bytes/máscaras en In2x/Out2x y tres repeticiones In2x.
La medición actual sitúa PostUpdate alrededor de 6,9 ms fijo y 8,0 en pan;
la atribución interna, el coste del diagnóstico completo y 30 FPS siguen
pendientes. No cierra F31 ni acredita paridad por una captura.

La etapa 39 mide dos nuevas tandas ABBA con Main y registro básico alineados:
efectos ~0,5 ms, Update ~17,3–17,9, PostUpdate ~6,9 fijo/8,1 pan, y
~24 FPS fijo/~21,4 pan. Conserva ticks desplazados y todos los picos. El
lookup mejora Update; no explica aún todo el frame ni cumple 30 FPS.

La etapa 41 repite dos tandas: ~24,3 FPS fijo y ~21,4 pan, children
~0,66 ms, simulación ~14–15, Update ~17–18 y PostUpdate ~6,8/8,0.
La reducción de children es consistente; el frame pan no lo es. Todos los
runs, máximos y el control desplazado se conservan en la evidencia enlazada.

La etapa 43 añade lectura real de color/profundidad para dos planos con
la matriz capturada, confirmando el empate que antes sólo acreditaba CPU.
Es una sonda aislada con controles; no un timer de todo el frame ni un oracle
de raster/importación. El programa conserva las cifras de la etapa 41.

La etapa 44 compara lecturas GPU texturadas con Draw original para una
pareja capturada. No mide rendimiento del frame; conserva la producción
y cifras 41 mientras descarta instalar el offset cero por su recorte de Z.

La etapa 45 añade trazas de bins CPU vinculadas a extracción por FrameCount.
No son timestamps GPU ni una nueva mejora de frame. Las comparaciones off/on
son exactas y las cifras activas siguen siendo las de la etapa 41.

La etapa 46 conserva 48 lecturas de una sonda GPU de rangos, pero retira el
candidato de escena por pérdidas nativas. No mide FPS ni cambia las cifras
retenidas. La corrección del orden aislado no cierra el render de todo el mapa.

### F32 — Separar contratos facilita optimizar sin romper paridad

P3 · Mantenimiento · Evidencia: **Fuente** · Alcance estimado: medio.

**Port:** Los tres crates separan núcleo, cliente y red. Las rutas extensas de cargo_transfer, NewGRF, sort y bootstrap mezclan preparación, resolución, publicación y diagnóstico; los comentarios históricos pueden quedar atrás (p. ej., rebuild de flows descrito como sin MCF).

**Original:** El original delimita módulos para solver, scheduler, handlers de save y resolvers; eso permite sustituir el cálculo puro manteniendo sus fronteras de estado.

**Propuesta:** Separar preparación/ejecución/publicación en los hot paths que se cambien, actualizar comentarios y exponer contadores por contrato. No hacer una reestructuración masiva ni presentar modularidad como una ganancia de FPS medida.

**Cómo comprobarla:** Refactors acotados con mismas salidas y coste medido; clippy/fmt/tests apropiados y fuentes de verdad documentadas. Mantener Generated/oracle y artefactos de usuario fuera de cambios.

Fuentes: [Rust: `sim_step/economy.rs`, L149](https://github.com/cavazquez/openttdrs/blob/8994755781092e2e7a6baf463a2126d9125fbebb/crates/openttdrs-core/src/sim_step/economy.rs#L149);
[OpenTTD: `src/linkgraph/linkgraphschedule.cpp`, L33](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/linkgraph/linkgraphschedule.cpp#L33).

## Orden de trabajo recomendado

1. **CargoDist: F01–F03 y F21.** Fijar primero el contrato de estadística,
   componentes, fechas y continuidad con un oracle de transferencias. Luego
   calcular el trabajo puro en workers y publicar de forma determinista.
   Cambiar recálculo inmediato por el scheduler modifica el momento de
   enrutamiento del port actual; no debe presentarse como refactor neutro.
2. **NewGRF: F04 y F09–F13.** Capturar contrajemplos de variables relativas
   en el original y resolver a demanda, usando el índice existente.
   Separar correcciones semánticas y optimización para que los tests acrediten
   claramente cada cambio.
3. **Host: F05/F06.** Eliminar JSON por tick y Value por hash, preservando
   late join/failover/frontera atómica. El ahorro potencial está medido; el
   esquema de checkpoints y la equivalencia del hash aún no están implementados.
4. **Render: F08/F17/F18.** Ensayar primero el índice de proxies sin alterar
   las escrituras. Reducir spikes de remap/pan y trabajo de zoom distante bajo
   oracle raster. No reutilizar como válida la variante de vidrio retirada.
5. **Tick residual: F12/F14–F16/F25.** Tras mover el solver, perfilar carga,
   reservas y decisiones reales. El tiempo restante conocido todavía excede
   el presupuesto; sacar MCF no acredita por sí solo 30 FPS ni 37 ticks/s.
6. **Fluidez de operaciones: F19/F20/F22.** Jobs de guardado, carga y generación,
   con publicación coherente, errores y progreso visibles. Después abordar
   UI/audio/memoria según mediciones de sesiones largas.

No se recomienda una reescritura global ni una GPU para MCF como primer paso:
la mayor oportunidad observada está en cuándo y cuánto trabajo se hace.
Un job asíncrono no aumenta necesariamente el throughput del solver; evita
bloquear presentación mientras hay capacidad de CPU suficiente.

## Aceptación y preguntas que siguen abiertas

Para el objetivo de 30 FPS, registrar **cada frame** y los ticks autoritativos
por segundo. Un promedio superior a 30 no acredita “todo el tiempo”.
Usar la misma partida con carga efectiva, joins, tránsito, seis zooms,
movimiento de cámara, construcción/ventanas, pausa y velocidad. Abarcar el
rollover mensual, incorporación/recovery de red y sesiones prolongadas.

Las mediciones anteriores prueban buen render estable en escalas hasta 2,
pero no todos los frames de pan ni los zooms 4/8. La captura congelada no mide
simulación ni tránsito. Las nueve fases del CSV no agotan render thread/GPU.

Queda sin medir en este corte: cada subfase de load_vehicles; cantidad/coste
de wrappers NewGRF restantes; presión de asignaciones PBS; meses con múltiples
rivales; bytes de caches de assets en sesiones largas; GPU por pase; y FPS de
OpenTTD original sobre **la misma flota y cámara**. No usar su benchmark de
mapa vacío para calcular una supuesta ventaja relativa en este caso.

F09–F11 describen diferencias comprobadas en la ruta del resolver, pero su
impacto sobre cada GRF requiere casos de oracle. F21 señala una frontera de
estado riesgosa para continuidad; no declara un desync reproducido.
Las propuestas de arquitectura quedan anotadas para etapas acotadas.

## Validación de este corte

Se compiló y ejecutó la sonda contra el core release fijado y se conservaron
sus veinte muestras. El script versionado se volvió a compilar con
`rustc -D warnings` y ejecutar: mismas cantidades, tick y tamaño JSON, con
variación pequeña de latencia entre corridas. No se mezclan ambas corridas
en las medias publicadas.

Se verificaron los 32 símbolos/paths Rust, los 64 anchors de fuentes fijadas,
los enlaces locales y el SHA-256 del save. Pasaron
`cargo fmt --all -- --check`, `rustfmt --edition 2024 --check` de la sonda,
`scripts/check_parity_docs_fresh.sh` y `git diff --check`.

La vista interactiva usa el mismo conjunto de hallazgos y pasó la comprobación
TypeScript estricta con el SDK local. La apertura en el panel quedó encolada;
no se afirma una inspección visual del panel en esta sesión.

Los cambios de esta etapa son documentación y sonda diagnóstica; no alteran
las reglas ni el renderer. El contrato de 30 FPS permanece abierto.
