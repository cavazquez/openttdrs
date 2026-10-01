# Flotas grandes: investigación de 30 FPS

Fechas: 2026-09-29/30. Pedido: localizar la caída de FPS con muchos vehículos y
determinar cómo mantener un frame de 33,33 ms, conservando la simulación de
OpenTTD a 27 ms/tick.

Estado del objetivo completo: **abierto**. Las cifras y los comandos de
reproducción tienen una única fuente en
[`RENDIMIENTO.md`](../RENDIMIENTO.md#flotas-grandes-y-presupuesto-de-30-fps).
Una medición con los ticks congelados no acredita el rendimiento en marcha.

## Primera etapa: trabajo repetido sin cambiar las reglas

- **Resuelto — índices de flota:** Action2 reutiliza un índice para los scopes
  de unidad, parent y relativos. El cliente lo actualiza tras carga, tick y
  comandos de red; libreas y el minimapa reutilizan búsquedas por ID o tesela.
  Las partes articuladas de carretera y el cierre de pagos evitan reconstruir
  toda la flota por unidad. Las regresiones comprueban IDs desordenados,
  valores vivos, scopes relativos, fallback sin tick y prioridad de compañía
  en el minimapa.
- **Resuelto — curvatura relativa NewGRF:** los efectos visuales reutilizan
  el índice de flota. La variable 62 anidada en 61 recorre cada dirección una
  sola vez, en lugar de volver a recorrer la cadena y buscar cada enlace en
  toda la flota para cada desplazamiento. La comparación con el scan anterior
  cubre los 256 valores del byte firmado en cadenas de 1, 4 y 260 unidades,
  incluyendo ciclos, sin cambiar valores ni consumo de RNG.
- **Resuelto — búsquedas MCF:** la cola conserva los desempates del scan
  anterior para distancia y capacidad. La adyacencia usa un índice de
  estaciones, conservando orden, duplicados y centinelas. Los orígenes sin
  demanda alcanzable dejan de ejecutar pasadas inútiles. El oracle de tests
  compara paths, demandas, edge flows y shares con el algoritmo anterior,
  además de los goldens existentes de linkgraph.
- **Resuelto — caché por cargo:** reutiliza el resultado sólo si nodos,
  aristas, settings y runtime coinciden exactamente. Un cambio en pasajeros
  no obliga a resolver otra vez correo sin cambios. La publicación de shares
  y el reencaminamiento de paquetes mantienen su frontera anterior.
- **Resuelto — medición reproducible:** el cliente exporta tiempos de frame,
  update y subfases con ventana/GPU real. El modo congelado también fija la
  interpolación. `sav_profile --newgrf` hidrata los catálogos activos como el
  cliente; el perfil sin ese flag tiene un alcance distinto.

Estas optimizaciones conservan las reglas existentes. No cierran la paridad
completa de CargoDist ni la garantía de 30 FPS.

## Cuellos de botella y siguiente etapa

Actualización de implementación: las etapas JSON y workers de CargoDist se
registran en [performance-implementation.md](performance-implementation.md).
Ya se retiró el solver de descarga/mes y se inició en spawn, con pausa de ticks
y render activo si vence sin terminar. La medición del cliente en marcha
posterior a `fc63adef` está en
[RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-workers-e-índices-2026-10-01).
La descripción siguiente conserva el
diagnóstico del baseline `89947557`. El objetivo completo sigue abierto.

La etapa 22 mantiene las invalidaciones recuperadas en la etapa 21 y actualiza
el ascensor vanilla mediante su child, sin reconstruir el chunk por ese único
movimiento. La ventana sostenida pasa de 17,165 / 16,985 a 18,831 / 18,864 FPS;
con cámara en movimiento, de 15,289 / 14,982 a 16,600 / 16,673. Los 459 avisos
se conservan, 61 fases mantienen estado/eventos/teselas y los seis zooms
congelados coinciden exactamente. La variante inicial que fallaba sólo en
Out4x fue rechazada. Evidencia y límites en el registro de implementación.
El límite actual de un tick por frame todavía ralentiza la partida al caer
los FPS; cadencia y 30 FPS siguen abiertos.
La etapa 23 reproduce 30 ticks con 30 frames en un segundo. El reloj aislado
propuesto corrige el conteo, pero fue retirado: el emisor de humo consume RNG
de juego desde el render y omite ticks agrupados, incluso sin NewGRF. La
corrección de cadencia debe integrar esas decisiones y los pasos de red.
La etapa 24 lee stocks por referencia: la sonda aislada mejora alrededor de
tres veces y conserva los 64 slots, pero el tick y los FPS no muestran una
mejora global clara. La última ventana fija mide 18,826 / 18,733 FPS; con
movimiento, 16,620 / 16,893. Estado, avisos y seis zooms permanecen exactos.

La [revisión de fuentes OpenTTD](openttd-source-performance-review.md) registra
32 hallazgos priorizados en los tres crates, diferencias semánticas NewGRF y
una sonda nueva de serialización/hash/carga. Conserva propuestas y criterios
de validación; no acredita mejoras implementadas ni cierra este objetivo.

En el cliente en marcha, `perf record -e cpu-clock:u` atribuyó el **83,55 %**
del tiempo de CPU a `fill_relative_vehicle_vars`, usado por los efectos
visuales. Ese coste no aparecía en el perfil aislado de `GameState::step`.
El CSV incluye ahora `effects_ms` para distinguirlo del tick de simulación.

`unload_vehicles` llama a `rebuild_station_flows` al finalizar cada tick con
aristas modificadas. Eso ejecuta Demand + MCF global en el hilo que también
debe dibujar el cliente. La caché ayuda a cargos sin cambios, pero pasajeros
vuelve a invalidarse continuamente en la partida de estrés.

El port ya implementa la cadencia de `OnTick_LinkGraph` en
`sim_step/landscape.rs`: snapshots en SpawnNext y publicación al vencer
JoinNext. La reconstrucción inmediata desde descarga y el rollover mensual
se superponen a ese scheduler. Además, el scheduler actual calcula el job
sincrónicamente al hacer join. OpenTTD 15.3 ejecuta `SpawnThread` y mantiene
el dibujo durante una espera de CargoDist.

Para cerrar este bloque:

1. Integrar la actualización del grafo con SpawnNext/JoinNext, sin publicar
   rutas nuevas por cada descarga. Verificar ticks, flows y RNG contra el
   oracle nativo con transferencias reales.
2. Calcular jobs sobre snapshots en workers, publicar en orden determinista
   en su fecha de join y conservar el render si un job vence sin terminar.
   Cubrir clonación, comandos, guardado/carga y lockstep; los jobs pendientes
   y los flows runtime actuales no se serializan en JSON.
3. Volver a perfilar carga, reservas/PBS, rutas y callbacks con NewGRF activo
   tras retirar el coste global del tick. El tiempo restante también debe
   entrar en 27 ms; desplazar MCF por sí solo no lo acredita.
   Los efectos visuales corregidos todavía toman unos 33–35 ms por tick en
   esta flota; revisar la construcción de contextos cuando no se invoca un
   callback y conservar las reglas CB10/CB160, persistent registers y RNG.
4. Medir la ordenación del viewport durante movimiento y Out8x, junto con
   pan/zoom. Comprobar raster y stream de parents/children al reducir su
   trabajo, en los seis zooms soportados.
5. Perfilar el compositor de vidrio conservando su raster. La variante que
   reutilizaba proxies y omitía copias según change detection se retiró: los
   tests ECS pasaban, pero el raster real difería. Las mejoras de flota y
   minimapa son independientes de esa variante.

La aceptación requiere partidas en marcha, cargas y joins de CargoDist,
movimiento de cámara y seis zooms. Deben informarse distribución de frame
times y ticks/segundo; un promedio superior a 30 FPS no prueba que cada frame
cumpla el presupuesto.

## Validación de la primera etapa

- `cargo fmt --all -- --check`, clippy de núcleo y cliente para todos los
  targets con `-D warnings`, y `git diff --check`: correctos.
- Núcleo completo: **2.986 tests pasados, 6 ignorados**, en 53 suites.
- Cliente: **1.658 tests pasados, 2 ignorados**; incluye composición de efectos
  y callbacks visuales, minimapa, interpolación y carga de índices.
- Build release de cliente y `sav_profile`: correcto. Perfil en marcha con
  GPU real y `perf`, además del perfil aislado de simulación.
- Raster comparado con el baseline en seis zooms y una captura limpia: el
  resultado y las métricas se conservan en la sección de rendimiento enlazada.
- `scripts/check_parity_docs_fresh.sh`: correcto.
