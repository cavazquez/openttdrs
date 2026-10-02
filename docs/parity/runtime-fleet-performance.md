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
La etapa 25 filtra las cabezas ferroviarias una vez por consulta actual y
conserva su orden. El movimiento baja alrededor de 1 ms por tick; la ventana
fija pasa de 18,761 / 18,809 a 19,500 / 19,550 FPS y el pan de 16,542 / 16,588
a 16,958 / 16,723. El total core de 24 ticks empeora y el de 120 mejora; ambas
ventanas quedan registradas. Los 250 escenarios diferenciales, 61 fases y
seis zooms conservan el port anterior. Cadencia y presupuesto siguen abiertos.
La etapa 26 acelera el conjunto de visitados del recorrido legacy de estaciones,
con 2.066 escenarios que conservan su secuencia. El remap sostenido baja de
unos 9,9 a 5,8 ms; cámara fija 19,276 / 19,420 → 21,298 / 21,026 FPS y pan
17,262 / 16,973 → 18,745 / 18,700. Otras 61 fases y seis zooms son exactos.
El nuevo perfil de CPU excluye el calentamiento, pero no permite atribución
acumulada fiable de callers; se conserva como diagnóstico. La cadencia y
los 30 FPS siguen abiertos.
La etapa 27 reutiliza esas huellas vigentes en la clasificación y selección
gráfica de estaciones; mapa o ancla nuevos usan el recorrido vivo. Conserva
29.039 consultas diferenciales y las referencias de los 1.807 tiles Station
importados; la sonda aislada es unas 24 veces más rápida. Remap sostenido
5,8 → 2,5 ms, cámara fija 21,089 / 21,291 → 22,762 / 22,452 FPS y pan
18,539 / 18,183 → 20,043 / 20,086. Otras 61 fases y seis zooms son exactos;
el núcleo no muestra una mejora uniforme. Action2, cadencia y 30 FPS siguen
abiertos: los 80 frames sostenidos posteriores todavía exceden 33,33 ms.

La etapa 28 comparte consultas de geometría dentro de cada construcción
Action2: los 7.228 contextos completos son idénticos y la sonda mejora unas
5,4 veces. El núcleo largo empeora ligeramente y los FPS permanecen cerca
de 22,6; no se acredita una ganancia global. Las 61 fases conservan el estado.
La primera captura anterior de In2x difiere 204 píxeles; se conserva ese fallo,
aunque doce repeticiones In2x/Out4x dan PNG exactos y las 18 trazas completas
coinciden tras renumerar consistentemente los IDs de entidades. Variación del
renderer, identidad nativa, cadencia y 30 FPS siguen abiertos.

La etapa 29 conserva las rutas viales/navales al modificar sólo la reserva PBS
de una estación o waypoint rail. Mantiene la invalidación ferroviaria dentro
del tick: 478 mutaciones y 192 comparaciones cached/live, sin cambios de estado
en las 61 fases. En 120 ticks baja 83 → 59 invalidaciones y evita 17 búsquedas;
el núcleo mejora modestamente, pero el cliente sigue en 22,7–22,8 FPS. In2x
reproduce el PNG alternativo de la etapa 28 en el otro binario y Out2x varía un
píxel; doce repeticiones son exactas. Los fallos se conservan y la variación
raster, F15 completo, cadencia y 30 FPS siguen abiertos.

La etapa 30 añade una captura optativa de todos los sprites, proxies, cámaras,
layouts y bytes de imágenes, más las dos máscaras de vidrio. Siete corridas
conservan entradas completas y PNG; seis tienen targets auxiliares exactos.
Con el diagnóstico apagado, los seis zooms permanecen exactos. En un píxel
variable In2x, parent y vidrio distintos colapsan al mismo valor proyectado
en una reproducción escalar f32; aún no se confirma su orden efectivo en GPU.
La variación inicial y el píxel verde Out2x siguen abiertos. No cambia el
renderer de producción ni se acredita una mejora de FPS en esta etapa.

La etapa 31 reproduce el empate de profundidad con la matriz real de
Bevy/glam y ensaya una máscara con orden de dibujo. Conserva las entradas y
los seis streams del sorter, pero pierde 114 coincidencias nativas en Out2x y
44 en Out4x: el candidato queda retirado. In2x mejora en 2.618 píxeles exactos
y mantiene 846 sin explicación. Out8x tiene un recorte nativo limitado por la
cámara. Las mediciones del candidato no muestran una ganancia clara de FPS.
Sólo se conserva la regresión CPU y la evidencia; F08/F31 y 30 FPS siguen
abiertos. Los datos completos están en la etapa 31 del registro de implementación.

La etapa 32 evita obtener todos los componentes de proxies estables y conserva
la reparación de cambios externos y su existencia. En GPU real, vidrio baja
3,28–3,29 → 2,62–2,67 ms y fijo mide 22,35–22,40 → 23,03–23,06 FPS,
con ventanas desplazadas un tick. Pan mejora menos y todos sus frames exceden
33,33 ms. Doce PNG en seis zooms, los seis streams y las entradas completas,
bytes y máscaras In2x/Out2x son exactos. Es conservación del port, no cierre
de paridad nativa. Las cifras y limitaciones completas están en la etapa 32;
cadencia y 30 FPS siguen abiertos.

La etapa 33 usa las tablas de entidades de Bevy en el viewport y conserva los
desempates, intervalos y transforms. En fijo, sort baja 4,41 → 4,10 ms y children
1,10 → 0,82; la reducción aparece también en pan. El frame combinado permanece
prácticamente igual: 43,4041 → 43,4786 ms fijo y 49,2658 → 49,3463 en movimiento.
Se conserva el pico de 102 ms del primer candidato; no se acredita una ganancia
global de FPS. Doce PNG y seis streams completos son exactos; In2x/Out2x
conservan todas las entradas, bytes CPU y máscaras. Evidencia y alcance en la
etapa 33; trabajo residual del frame, cadencia y 30 FPS siguen abiertos.

La etapa 34 registra intervalos de los schedules de Bevy: PostUpdate toma
6,9 ms fijo y 8,1 ms en pan, además de Update 18,7/19,4 y fixed loop 14–15 ms.
El detalle conserva doce PNG, seis streams y entradas/bytes/máscaras In2x/Out2x
del colector anterior; el prototipo que añadía un recurso se retira por raster.
Sin colector reaparece la variación histórica In2x de 204 píxeles, acotada a
una permutación de proxies en una pareja, con bytes y fuentes idénticos.
También se documenta que el colector básico cambia capturas del mismo binario;
conservar su forma ECS queda como siguiente sub-issue. Estas mediciones no
acreditan FPS de una sesión sin colector ni cierran cadencia o 30 FPS.

La etapa 35 conserva la escena al activar el colector: estado fuera de ECS y
consultas retrasadas hasta el warmup. Seis zooms básico off/on son exactos;
tres parejas In2x y el detalle In2x/Out2x conservan entradas, bytes y máscaras.
El sub-issue de registro se cierra para esta matriz. La nueva medición sigue
en unos 23,3–23,6 FPS fijo / 20,7 en pan; PostUpdate ocupa 6,8–7,0 / 8,0 ms.
No se atribuye una ganancia de FPS ni el coste total del colector. Evidencia y
límites en la etapa 35; F31, cadencia, paridad nativa y 30 FPS siguen abiertos.

La etapa 36 evita marcar pose/ancla/visibilidad de proxies reutilizados cuando
sus bits no cambian y conserva ocultación previa al borrado diferido. Seis
zooms y entradas/bytes/máscaras In2x/Out2x son exactos. Dos tandas ABBA no
acreditan mejora clara de FPS ni reducción uniforme de PostUpdate; se retiene
la reducción de invalidaciones cubierta por la regresión. Las cifras y el
run desplazado de la primera tanda se conservan en la etapa 36.

La etapa 37 corrige la desaparición de una fuente segmentada sin parents
globales. La regresión exige las seis bandas y cada fila una sola vez; una
sonda de las funciones nativas confirma ese contrato geométrico. Kale
conserva los seis zooms, entradas, bytes y máscaras capturados. No se mide
una ganancia de FPS en esta corrección; continúan las cifras de la etapa 36.

La etapa 38 evita flags falsos de Sprite ordinarios reutilizados y conserva
sus campos raw y la reparación externa. Seis zooms y las entradas completas
capturadas son exactos. Dos tandas ABBA no acreditan mejora sostenida de FPS;
el guard añade comparación al sort, mientras glass/PostUpdate bajan algo en
la repetición. El pico de 82 ms y un control fijo desplazado un tick quedan
en la evidencia. Continúa alrededor de 23 FPS fijo y 20–21 en pan.

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
