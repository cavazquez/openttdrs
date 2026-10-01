# Implementación de la revisión de rendimiento

Solicitud activa: implementar las mejoras de
[la revisión de fuentes](openttd-source-performance-review.md), con el objetivo
abierto de [30 FPS en flotas grandes](runtime-fleet-performance.md).
El informe conserva la evidencia anterior; este registro distingue los cambios
implementados de las hipótesis y de las diferencias que necesitan más trabajo.

## Etapa 1 — Frontera persistente de CargoDist (F21)

Estado: implementada y validada para JSON. Los snapshots conservan los flows publicados, el
orden de shares, las fechas de join y las entradas inmutables de jobs pendientes.
El scratch de MCF, los catálogos e índices siguen siendo efímeros. Rehidratar un
snapshot nuevo no ejecuta el solver ni reencamina paquetes ni consume RNG.
Los JSON antiguos conservan la migración anterior cuando falta esta frontera.

Regresiones: guardar antes del join, mutar el grafo vivo y comparar publicación,
hash y RNG con la ejecución continua; conservar el orden de selección ponderada
de un flow ya publicado. El formato SAV de jobs mutados sigue pendiente.

Validación: suite completa core (2.988 tests aprobados, seis ignorados), 36 tests
de red TCP y 1.658 del cliente (dos ignorados); Clippy en todos los targets de
core, red y cliente, formato, diff y frescura de docs. El protocolo pasa a v6
para separar clientes que reconstruían esta frontera al cargar.

## Compilación — símbolos y trabajo incremental

Se compararon seis builds con Rust 1.98, `clang` + `mold`, dependencias cacheadas
y el mismo nivel de optimización. La configuración de desarrollo ahora genera
tablas de líneas: los backtraces conservan archivo/línea y el binario medido
ocupa unos 371 MiB en lugar de 725 MiB. Inspeccionar variables locales en un
debugger requiere `cargo build --config profile.dev.debug=2`.

La primera comparación dio 89,50 s con símbolos completos y 85,14 s con tablas
de líneas, con distinto calentamiento del incremental. Tras una edición pequeña
del diagnóstico de topología: 56,57 s y 28,52 s. Otra edición, invirtiendo el
orden y con ambos caches calientes: 16,23 s y 14,73 s. La ganancia depende del
trabajo invalidado y del caché; no se extrapola a un checkout frío ni a cualquier
cambio. Los datos por crate están en
[build-times-20260930.json](evidence/build-times-20260930.json).

El build inicial concentró 25,12 s en core y 64,03 s en el cliente. Las
dependencias ya estaban frescas. `mold` y el enlace dinámico opcional ya existían;
activar un linker que ya se usa no resolvería ese coste. Los aliases
`cargo check-client` y `cargo check-core` permiten verificar el target editado
durante la iteración; los gates completos se mantienen al publicar.

La elección de símbolos sigue la
[documentación de perfiles Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html#debug).
Las unidades y sus intervalos se obtuvieron de los
[reportes de Cargo](https://doc.rust-lang.org/cargo/reference/timings.html).
Las optimizaciones de Bevy y el perfil release conservan su configuración.

Una validación posterior agotó el disco: `target/debug/incremental` tenía
50 GiB y quedaban menos de 1 GiB libres. LLVM informó `No space left on device`
antes del SIGSEGV. Se archivó y verificó con zstd un único caché incremental
del cliente del 26/09 (7,15 GiB); luego se retiró ese directorio derivable.
Las dependencias y los caches actuales se conservaron. Quedaron 8 GiB libres
y el build de tests del cliente terminó correctamente. El archivo recuperable
vive temporalmente en `/tmp/openttdrs-client-cache-20260926.tar.zst`.

## Etapa 2 — Solver en workers y frontera de publicación (F01/F02)

La descarga registra usage/capacity sin resolver Demand + MCF. El cierre de
mes conserva los flows publicados y sólo rota el contador mensual. Spawn
prepara entradas propias sin copiar todas las estaciones/catálogos ni reservar
el scratch denso; el worker asigna ese scratch y ejecuta el pipeline entero.
Join consume la cabeza de la cola en su fecha, con el mismo orden de shares.

El cliente detiene los ticks ante una cabeza vencida no terminada a partir de
la fracción nativa 19. La pausa de AfterLoad cubre también una partida cargada
fuera de esa ventana. Update/render continúan; el host envía heartbeat durante
la espera sin enviar AdvanceTicks. Los resultados y handles del worker quedan
fuera de JSON y hash. Clonar comparte el resultado inmutable; cargar vuelve a
iniciar el cálculo sobre las entradas persistidas. Un fallo del worker señala
la finalización y se reproduce sincrónicamente para informar el mismo fallo.

Oracle de fuente: OpenTTD 15.3, `14ec60f2`, `linkgraphschedule.cpp`
SpawnNext/Run/JoinNext y StateGameLoop/AfterLoad_LinkGraphPauseControl.
Regresiones: entradas sin annotations en spawn, worker frente al pipeline
sincrónico, finalización sin publicación/RNG/hash, FIFO con cabeza retenida,
fracción 19 y AfterLoad, render activo durante espera, descarga múltiple y mes
sin reemplazar rutas, más los roundtrips JSON de la etapa 1.

Medición release aislada: la misma Kale_TitleGame.sav, NewGRF activo, cuatro
ticks por ejecución, orden antes/después/después/antes y sin compilaciones
concurrentes. Tick medio anterior: 1.356,86 / 1.348,97 ms; después:
536,88 / 541,78 ms. Descarga: 1.153,93 / 1.146,91 → 331,02 / 333,58 ms.
La mejora del tick es aproximadamente 2,5 veces; aún excede los 27 ms.
Carga conserva 106–108 ms y las rutas 67–69 ms en esta ventana inicial.
Los datos por fase están en
[cargo-workers-20260930.csv](evidence/cargo-workers-20260930.csv).
Esto mide simulación, no FPS. Se comparó el ejecutable previo de `89947557`
(SHA256 `ca88d838af0e8cc236793c9592fc08a2adb979e289f78019f30f1face9db54f9`)
con el candidato de esta etapa (`be6e16c0088288f23db02a8d1cf20e08716913b26b817bafeceb14ba160028c4`).

Validación: núcleo completo 2.992 aprobados/seis ignorados; cliente 1.659/dos;
36 tests de red. Clippy de core/cliente en todos los targets, formato, diff y
frescura de docs. Persisten F03 (componentes, supply e identidad nativos),
SAV tras mutaciones y la cancelación del trabajo pendiente al descartar un
mundo. Los workers son puros y no pueden publicar sobre un mundo reemplazado.

## Etapa 3 — Caché de rutas e invalidación (sub-issue F15)

Las búsquedas de carretera, tranvía y agua se conservan entre ticks, incluidas
las búsquedas sin ruta. El mapa identifica cambios de infraestructura y altura
con una revisión efímera distinta de las reservas/señales ferroviarias. Un
mundo nuevo o un clon divergente tiene otra identidad; cambiar enlaces de
túneles invalida también las entradas que los usan. Los costes navales y el
conjunto de direcciones de origen forman parte de la clave. Ninguna restricción
y una máscara vacía son casos diferentes.

El lote paralelo anterior evitaba la caché cuando había 32 solicitudes. Ahora
consulta primero las entradas persistidas y resuelve sólo las solicitudes
distintas que faltan. Publica por orden de flota, independientemente del orden
de finalización. La caché sigue acotada a 256 entradas y expulsa la más antigua,
sin vaciar todas las entradas al alcanzar el límite. `sav_profile` informa
hits, negativos, misses, búsquedas distintas e invalidaciones.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`src/pathfinder/yapf/yapf_costcache.hpp`, `stGetGlobalCache` y
`PfNodeCacheFetch`: invalidación por layout y separación del coste que no puede
usar la caché global. Las rutas ferroviarias completas del port conservan la
caducidad por tick y observan cambios del mapa dentro del tick; implementar una
caché nativa de segmentos de tren sigue pendiente.

Regresiones: carretera eliminada/restaurada, resultado negativo reutilizado,
cambios irrelevantes de paisaje y reservas, clones de mapa divergentes con
igual revisión, enlaces de túneles cambiados sin cambiar tiles, agua eliminada,
costes/direcciones navales y 40 camiones con una búsqueda y publicación estable.
La comparación cacheada/sin caché cubre los cambios de conectividad. Los
contadores y revisiones no cambian la representación persistida del mapa.

Medición release de Kale_TitleGame.sav con NewGRF, sin compilaciones concurrentes,
orden antes/después/después/antes en cada ventana. En los primeros cuatro ticks,
rutas: 66,88 / 67,25 → 62,61 / 61,88 ms. El tick completo quedó en
528,15 / 523,41 → 526,64 / 522,16 ms: no se atribuye una mejora global clara.
En 24 ticks, el total fue 268,10 / 265,62 → 269,25 / 268,99 ms, también sin
ganancia global. La flota consulta sobre todo orígenes/destinos distintos y
ya conserva caminos individuales: 23 hits, 1.826 misses, 1.616 búsquedas
distintas y 14 invalidaciones en 24 ticks. No es una búsqueda completa por
vehículo en cada tick. Los datos por fase están en
[route-cache-20260930.csv](evidence/route-cache-20260930.csv).
Se compara `f50cf50d` (ejecutable SHA256 `be6e16c0088288f23db02a8d1cf20e08716913b26b817bafeceb14ba160028c4`)
con esta etapa (`d24653ec45b08724401d34d00adae4112ae95d6822fc28f234ddf7380c91c9dc`).

Validación: 2.998 tests core aprobados/seis ignorados, 1.659 cliente/dos;
Clippy de core/cliente en todos los targets, formato, diff y frescura de docs.

La invalidación es conservadora: un cambio de bytes de estación puede invalidar
las rutas de carretera/agua aunque no altere su conectividad. La aplicación de
comandos todavía descarta las rutas individuales ante cambios de mapa. Reducir
estas invalidaciones y cachear segmentos ferroviarios mantiene F15 abierto.

## Etapa 4 — Referencias a catálogos durante la carga (sub-issue F12)

Se retiraron copias de `EngineDef` en cantidad de carga, capacidad por unidad,
refresco de capacidad, selección/aplicación de refit y la copia de
`IndustrySpecDef` en entrega a industrias. Los callbacks reciben referencias
al mismo catálogo y siguen escribiendo sobre el vehículo/industria en el mismo
orden. La consulta de refit conserva la copia de vehículo necesaria para que
los registros persistentes de una prueba no modifiquen el vehículo real.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `economy.cpp::GetLoadAmount` consulta
`const Engine *`; `newgrf_engine.cpp::GetVehicleProperty/GetEngineProperty`
evalúan callbacks sin copiar la definición de motor. No se cambian resultados,
clamping ni fallbacks de estos callbacks.

Los ocho runs release sobre Kale_TitleGame.sav con NewGRF no muestran una
ganancia global clara. Cuatro ticks: total 522,30 / 517,88 → 513,92 / 519,20 ms;
24 ticks: 268,80 / 265,41 → 282,82 / 265,60 ms. Carga sigue cerca de 100 ms.
Esta reducción de copias no explica el cuello principal de la fixture. Todos
los runs, incluido el pico de 282,82 ms, están en
[catalog-borrows-20260930.csv](evidence/catalog-borrows-20260930.csv).
El baseline es `8f688650` (SHA256 `d24653ec45b08724401d34d00adae4112ae95d6822fc28f234ddf7380c91c9dc`)
y el candidato `15ee18b001d076d44795d2ca99f980f22284a7d1cfe60df8c61155fd592febc6`.

Validación: 2.998 core/seis ignorados, 1.659 cliente/dos; Clippy de ambos en
todos los targets, formato, diff y frescura de docs. Las regresiones existentes
de capacidad dinámica CB36, cantidad de carga, refit, capacidad nativa por
unidad y entrega a industrias conservan su resultado. F12 sigue abierto para
perfilar terminales, paquetes y agregados; F13 para builders repetidos.

## Etapa 5 — Índice vigente al refrescar formaciones (sub-issue F13)

El refresco de capacidad/peso de cada cabeza usaba el wrapper que reconstruye
`FleetIndex`, seguido por más wrappers al propagar poses. Ahora recibe el
índice del tick, resuelve los slots en O(1) y usa también la propagación indexada.
Se conservan las pasadas y el orden de callbacks sobre cada unidad. El wrapper
de consultas aisladas conserva su implementación; la nueva API tiene un
fallback cuando falta un índice preparado. Un cambio de IDs, slots o enlaces
requiere reconstruir el índice antes de usar la nueva API.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `train_cmd.cpp::Train::ConsistChanged`
recorre `Next()` y consulta las unidades/motores existentes. El port conserva
sus reglas actuales de capacidad y pose; esta etapa reduce las búsquedas de
entidades y los builders, sin ampliar por sí sola la paridad de esas reglas.

Regresión: bytes completos de vehículos iguales al wrapper en flota con IDs
desordenados, NewGRF CB36, registros/bits aleatorios, carga custom, multiplicador
de freight, ambos ajustes de velocidad de vagones, formación apilada y posiciones
distintas. Se comprueba también el fallback de preview y la generación posterior
a reordenar el Vec y reconstruir el índice.

Medición release sobre Kale_TitleGame.sav con NewGRF, orden
antes/después/después/antes, sin compilaciones concurrentes. En los primeros
cuatro ticks: carga 105,98 / 104,63 → 38,29 / 39,37 ms; tick completo
512,98 / 515,68 → 442,78 / 458,85 ms. En 24 ticks: carga
96,87 / 96,70 → 30,75 / 30,86 ms; total
265,98 / 267,06 → 202,57 / 201,82 ms, una reducción aproximada del 24 %.
Descarga sigue alrededor de 83 ms en esa ventana; el objetivo de FPS sigue
abierto. Los datos por fase están en
[consist-index-20260930.csv](evidence/consist-index-20260930.csv).
Baseline `1fba1491` (SHA256 `15ee18b001d076d44795d2ca99f980f22284a7d1cfe60df8c61155fd592febc6`),
candidato `a859f5f15fd62facfd8a8d2d4ad8d392af9ec7caf211b2063ee6ac8af96ac495`.

Validación: 2.999 core/seis ignorados, 1.659 cliente/dos; Clippy de ambos en
todos los targets, formato, diff y frescura de docs. Se cierra este callsite
acotado de F13; los demás builders y subfases de carga siguen en revisión.

## Etapa 6 — Huellas derivadas al consultar una estación (sub-issue F12)

El índice de terminales conserva la huella conectada que antes se reconstruía
para cada consulta ferroviaria de carga/descarga. Se reutiliza únicamente si
la identidad/revisión del mapa sigue vigente. Ante un cambio durante el tick
se vuelve a la consulta viva hasta refrescar el índice. Cambiar entre Station
y Airport invalida también cuando MAP2 conserva el mismo ID: la pertenencia
al flood-fill ha cambiado. Consultar una tesela que no es plataforma descarta
el caso antes de construir una huella.

Se conserva la geometría legacy del port, su orden y su límite de recorrido.
Esta etapa no amplía la paridad de propiedad de estaciones adyacentes ni
cambia el límite de 64 tiles. OpenTTD 15.3 `14ec60f2`, `train_cmd.cpp`,
`TrainEnterStation` y consultas `GetStationIndex/Station::Get`, confirma que
el servicio usa la identidad de la estación; el índice nativo del port por
MAP2 sigue separado del fallback legacy que aquí se cachea.

Regresiones: consulta cacheada frente a viva en plataformas/adyacencias y fuera
de estación; mismo ID con cambio de tipo de tesela, mapa clonado, recorrido
conectado mayor que el límite y bytes del mapa intactos al preparar la caché.

El perfil CPU previo capturó 798 muestras sin pérdidas, evento `cpu-clock:u`
a 99 Hz, DWARF con stack de 65.528 bytes. Incluye carga de SAV/NewGRF, workers y
24 ticks, no sólo la descarga. Muestra asignaciones y operaciones sobre sets de
TileCoord, pero los callgraphs están incompletos y hay símbolos genéricos que
LLVM puede compartir. No se atribuye todo ese porcentaje al flood-fill ni a
terraformación. La ganancia de esta etapa se decide por tiempos de fases.

Ocho runs release con la misma fixture/NewGRF, orden antes/después/después/antes
por ventana y sin compilaciones concurrentes. Cuatro ticks: total
449,10 / 463,29 → 451,51 / 438,11 ms; carga
38,87 / 39,18 → 32,10 / 31,92 ms. En 24 ticks: total
201,39 / 198,55 → 184,56 / 186,70 ms (aproximadamente 7 % menos), carga
31,17 / 30,40 → 22,63 / 22,79 ms y descarga
83,34 / 82,37 → 75,75 / 77,45 ms. La ventana inicial tiene más variación y
continúa dominada por descarga. Datos:
[footprint-cache-20261001.csv](evidence/footprint-cache-20261001.csv).
Baseline `2ac6ab07` (SHA256 `a859f5f15fd62facfd8a8d2d4ad8d392af9ec7caf211b2063ee6ac8af96ac495`),
candidato `bb1f331679eb6c07d4378f853ec3213f4d9b91efd087e03a532d64b557d6fd53`.

Validación: 3.001 core/seis ignorados, 1.659 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Se cierra la reutilización
de estas huellas; el resto de F12 y el objetivo de FPS siguen abiertos.

## Etapa 7 — Enlaces de vehículos en órdenes y eventos NewGRF (F12/F13)

El recorrido hacia la cabeza de una orden, el cierre de carga/descarga de una
formación y la propagación de eventos aleatorios NewGRF usan ahora el índice
vigente de vehículos. Se conserva el recorrido por enlaces, su orden y las
guardas de ciclos/límite de visitas. Las consultas aisladas, slots reordenados
e IDs duplicados mantienen la búsqueda legacy del primer vehículo coincidente.
El índice detecta duplicados al reconstruirse; esa marca y los slots son
efímeros. Los eventos también consultan el motor por referencia, sin copiar
su runtime NewGRF.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`newgrf_engine.cpp::DoTriggerVehicleRandomisation`, líneas 1260–1324:
propagación mediante `First()/Next()`, palabra base compartida en Empty y
AnyNewCargo, evento NewCargo seguido por AnyNewCargo desde la cabeza,
randomización independiente por unidad en Depot y ausencia de recursión en
Callback32. Esta etapa conserva la implementación del port de esos eventos;
no amplía por sí sola la paridad del resolver o de su RNG.

Regresiones: lookup del primer ID, índice ausente/reordenado/duplicado y bytes
completos de todos los vehículos frente al wrapper de eventos, para cinco
triggers en cadenas desordenadas, enlaces faltantes, ciclos e IDs duplicados.
Se conserva el número de reconstrucciones del índice.

Ocho runs release de Kale_TitleGame.sav con NewGRF, orden
antes/después/después/antes, sin compilaciones concurrentes. Cuatro ticks:
total 435,10 / 437,81 → 431,54 / 440,32 ms, sin ganancia global clara;
carga 31,92 / 31,98 → 26,43 / 27,72 ms. En 24 ticks:
total 186,19 / 184,75 → 177,25 / 176,47 ms (aproximadamente 5 % menos),
carga 23,27 / 22,98 → 17,74 / 17,58 ms y descarga
76,48 / 76,08 → 72,67 / 71,13 ms. Datos:
[chain-lookups-20261001.csv](evidence/chain-lookups-20261001.csv).
Baseline `0b442152` (SHA256 `bb1f331679eb6c07d4378f853ec3213f4d9b91efd087e03a532d64b557d6fd53`),
candidato `6050236782d36d6cf3f03cbafb510cee32d4b64c807d0102908c7579bbaa0330`.

Validación: 3.003 core/seis ignorados, 1.659 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. F12/F13 permanecen abiertos;
descarga sigue siendo la mayor fase residual en esta fixture.

## Trabajo restante

- F03/F21: grafos por componente, estadísticas nativas de producción y SAV
  de flows/jobs tras mutaciones; acotar/cancelar trabajo pendiente (F02).
- F04, F09–F11, F13: resolver NewGRF a demanda y corregir variables relativas.
- F05–F06: snapshots tipados de red y hash sin árbol JSON completo.
- F07–F08, F17–F18: medir tick residual, compositor y cámara con raster en seis zooms.
- F12, F14–F16: perfilar carga, ocupación y rutas; retirar copias PBS innecesarias.
- F19–F20, F22: encoding/carga/generación sin bloquear el cliente.
- F23–F25, F27–F29: caches/invalidación de imágenes, HUD, IA, audio y previews;
  preservar los comportamientos que la revisión ya verificó correctos.
- F26: documentar el alcance de IA/GS; una VM Squirrel completa requiere un
  proyecto de compatibilidad propio y no es una optimización del hot path.
- F30–F32: pruebas diferenciales, métricas por fase y límites claros de publicación.

No se declara alcanzado el presupuesto de 33,33 ms sin una nueva medición de la
partida grande en ejecución, cámara y los seis niveles de zoom.
