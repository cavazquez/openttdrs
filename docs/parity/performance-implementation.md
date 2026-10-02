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

## Etapa 8 — Desglose de descarga y nueva captura del cliente (F12/F31)

`step_profiled`, `sav_profile` y `sim_profile` distinguen selección de terminal,
aceptación, staging, entrega por paquete, commit y cierre de formaciones.
Dentro de commit separan reinserción/agregados y eventos de estación. Estos
dos últimos son subconjuntos de commit: no se suman otra vez al total.
El tick normal usa la variante constante sin relojes; los tiempos no entran
en JSON/hash ni deciden el orden de simulación. La salida informa además los
specs con triggers de carga y las teselas animadas activas.

Regresión: ejecución instrumentada frente a normal en formación con slots
desordenados, descarga/reencolado NewGRF CB140, bodega de mail de aeronave,
vehículo fuera de terminal y early-return por carga previa. Se comparan el
estado persistido completo, flags de descarga, popups, eventos, industria
pendiente y tiles dirty; los contadores normales quedan en cero.

Dos runs release por ventana con Kale_TitleGame.sav/NewGRF. En cuatro ticks,
descarga 308,52 / 310,38 ms y eventos de estación 300,84 / 302,90 ms. En 24,
descarga 71,82 / 71,87 ms y eventos de estación 68,69 / 68,81 ms;
reinserción/agregados 0,025 / 0,024 ms. Estos eventos explican alrededor del
96 % de la descarga residual. La aceptación queda alrededor de 0,7 ms y la
entrega por paquete también. No se atribuye ese coste a escanear stocks.
Datos: [unload-subphases-20261001.csv](evidence/unload-subphases-20261001.csv),
ejecutable SHA256 `c34b669d09925edf2064c0e29429049e1ca8ff74caee80d403277bd918d37b8f`.

La fixture tiene un spec ferroviario, cero triggers de carga habilitados y
cero tiles animados activos. El port prepara geometría/contextos de CB140
antes de comprobar esa máscara. El original `14ec60f2`,
`newgrf_station.cpp::TriggerStationAnimation`, líneas 903–938, descarta primero
los triggers deshabilitados mediante `cached_anim_triggers`. Ésta es la
siguiente divergencia acotada; la optimización debe conservar la limpieza de
tiles inválidos que el port realiza aunque el callback no se ejecute.

La comparación nueva de GPU/ventana en marcha entre `89947557` y `fc63adef`
está en [RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-workers-e-índices-2026-10-01).
No se atribuye esa mejora a esta instrumentación.

Validación: 3.004 core/seis ignorados, 1.659 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. F12/F31 y el presupuesto
de 30 FPS siguen abiertos.

## Etapa 9 — Descartar triggers ferroviarios antes de preparar geometría (F12)

Las animaciones de toda una estación o de un andén comprueban primero si su
spec tiene runtime NewGRF y habilita el trigger solicitado. Si no hay teselas
animadas activas y el trigger no puede ejecutarse, terminan antes del
flood-fill y de construir los contextos. Con teselas activas conservan el
recorrido anterior, incluida la eliminación de entradas inválidas. Los
callbacks habilitados conservan orden, sonidos, registros persistentes,
random bits y dirty tiles. La geometría legacy, sus adyacencias y su límite
de 64 tiles siguen pendientes; el port modela un spec por estación.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`newgrf_station.cpp::TriggerStationAnimation`, líneas 903–938: máscara
`cached_anim_triggers` antes de seleccionar área y evaluar callbacks. El
filtro reproduce ese descarte; no acredita por sí solo la paridad nativa
de la generación de parámetros aleatorios de CB140.

Regresión diferencial de las variantes con/sin filtro: 28 combinaciones de
área, máscara, runtime ausente, spec vanilla/ausente, waypoint y conjunto
activo vacío/no vacío. Compara el mapa y las estaciones serializados, el
orden dirty, sonidos y limpieza de entradas antiguas; incluye callbacks
positivos en estaciones adyacentes.

Ocho runs release de Kale_TitleGame.sav con NewGRF, orden
antes/después/después/antes por ventana, sin compilaciones concurrentes.
Cuatro ticks: total 441,98 / 426,56 → 98,12 / 93,18 ms,
descarga 318,94 / 305,88 → 6,50 / 6,26 ms y carga
27,56 / 26,22 → 1,65 / 1,55 ms. En 24 ticks: total
175,78 / 175,87 → 88,51 / 89,83 ms (aproximadamente 49 % menos),
descarga 72,18 / 73,04 → 2,83 / 2,85 ms y carga
17,83 / 17,94 → 1,64 / 1,62 ms. Los eventos de estación pasan de
68,89 / 69,95 a 0,0225 / 0,0224 ms en esa ventana.
Datos: [station-trigger-filter-20261001.csv](evidence/station-trigger-filter-20261001.csv).
Baseline `00402e6f` (SHA256 `c34b669d09925edf2064c0e29429049e1ca8ff74caee80d403277bd918d37b8f`),
candidato `8c851b70bc205ffd9392ef447c50bc2eb3efcb1d68a9a83fc4ae6c328cadf3ee`.

Se comparan además el estado inicial y 24 ticks **normales**, sin relojes de
perfil, en dos builds independientes. Las 25 fases coinciden en hash canónico
v4, eventos/popups/sonidos, todas las teselas y bloques 4×4: cero diferencias.
Sonda: [trace_sav_sim_state.rs](../../scripts/trace_sav_sim_state.rs).
Datos: [station-trigger-state-20261001.csv](evidence/station-trigger-state-20261001.csv).
Las sondas tienen SHA256 `6a2de05098765db4549adc5bfa617fd4ec5b278e9be418bed7149433ef1b7935`
y `02a0b0b808d50b5536971e4915c9002183e5d2d44dbadbb2c394ac31512a2182`.

La sonda ordena una vez las listas compartidas por ID antes de comenzar.
Sin ese paso, incluso dos imports con el mismo binario difieren en el hash
inicial: `from_sav_game` materializa ese vector desde un HashMap. Dos estados
JSON independientes eran iguales tras ordenar únicamente sus 134 listas.
No se cambia el algoritmo de hash ni se ocultan campos durante los ticks;
corregir ese orden en el importador es el siguiente sub-issue de F06/F20.
La sonda se compila contra cada build mediante `rustc --edition=2024
scripts/trace_sav_sim_state.rs --extern openttdrs_core=<rlib> -L
dependency=target/release/deps -C opt-level=2 -o /tmp/state-trace` y se ejecuta
con `save/Kale_TitleGame.sav 24 <directorio-de-tiles>`.

Validación: 3.005 core/seis ignorados, 1.659 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Se cierra este descarte
acotado. El movimiento de vehículos queda alrededor de 52 ms y las rutas
alrededor de 15 ms en 24 ticks; F12 y los 30 FPS siguen abiertos. Aún no hay
una nueva medición de ventana/GPU para atribuir FPS a esta etapa.

## Etapa 10 — Orden determinista de listas compartidas al importar SAV (F06/F20)

El importador agrupa vehículos por `OrderListID` en un BTreeMap para
materializar las listas en orden ascendente. Conserva IDs, órdenes y
referencias de miembros; la selección del primer vehículo de cada grupo
mantiene el orden de VEHS. No consume RNG. OpenTTD 15.3 `14ec60f2`,
`saveload/order_sl.cpp::ORDLChunkHandler` y
`core/pool_type.hpp::PoolIterator`, recorre el pool por índices ascendentes
y conserva esos índices al cargar.

La regresión primero falló con el HashMap: obtuvo
`[10,31,17,45,38,3,52,24]` frente a los IDs ascendentes esperados. Con el
cambio comprueba ocho IDs sparse, dos miembros por grupo, enlaces y órdenes,
dos imports independientes, JSON, hash y RNG iguales.

Cuatro imports independientes de Kale_TitleGame.sav/NewGRF producen ahora
directamente el hash inicial v4 `b10f09635218f649`. La cuarta ejecución
avanza además 24 ticks normales. Sus 25 fases coinciden con la etapa 9 desde
el mismo orden inicial: hash, eventos/popups/sonidos, todas las teselas y
bloques 4×4; cero diferencias. Las 28 filas están en
[shared-order-state-20261001.csv](evidence/shared-order-state-20261001.csv).
La sonda tiene SHA256 `2747f87f4b52d6980fc1ee4f77d8391707d6b6e9bcb57e9148638250a7807eab`;
se compila como en la etapa 9 y se ejecuta con
`save/Kale_TitleGame.sav 24 <directorio> --keep-order-list-order`.
Ese flag omite la preparación inicial del comparador: la disposición la
produce el importador real.

Validación: 3.006 core/seis ignorados, 1.659 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Se cierra la divergencia
de orden de estas listas. Esta corrección permite comparar imports y no se
presenta como una ganancia de FPS. F06 sigue pendiente por el costo del árbol
JSON y F20 por el resto de carga/rehidratación.

## Etapa 11 — Contexto visual reducido cuando no puede ejecutarse un callback (F04)

Los efectos conservan el contexto de la unidad con registros persistentes,
random bits y variables básicas. Sólo preparan el scope completo, badges,
vecinos y parámetros GRF si hay runtime y CB10 habilitado o un efecto
avanzado de Action0. CB160 puede estar habilitado sin la máscara de CB10;
ese caso conserva el contexto completo. El sentinel de efectos por defecto
no habilita CB160. No cambian el orden de emisión ni el límite de efectos.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`vehicle.cpp::Vehicle::UpdateVisualEffect`, líneas 2636–2674, comprueba la
máscara antes de resolver CB10; `SpawnAdvancedVisualEffect` resuelve CB160
por separado. Es un descarte acotado del trabajo imposible, no el cierre
del resolver a demanda ni de todos los scopes NewGRF.

Regresión diferencial: 480 combinaciones de motor steam/diesel/electric,
contador y runtime/GRFID/máscara/propiedad, comparadas con el contexto completo
anterior. Coinciden especificación, emisiones, vehículo serializado,
registros y RNG; se exige al menos una emisión positiva. Otra regresión
consulta B4 en CB160 sin máscara CB10 y conserva el scope completo. La
regresión existente de CB10 también permanece verde.

Cliente release con Kale_TitleGame.sav/NewGRF: ABBA, 30 frames de warmup y
40 muestras por run, sin compilaciones ni perf concurrentes. Efectos:
31,65 / 31,68 → 2,05 / 2,18 ms por frame; FPS: 5,22 / 5,24 → 6,23 / 6,21.
La simulación aún promedia unos 115 ms por frame. Los 80 frames nuevos
exceden 33,33 ms. Método, colas y ticks/s en
[RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-filtrar-contextos-visuales-2026-10-01);
[datos por frame](evidence/visual-context-client-20261001.csv).
Baseline `1f613aad`, SHA256
`19cad4ea6164c5f4605958dda9f6951b42025fcf496ce60d373889ad3c81d60c`;
candidato `1cf2e9b444835498c2ca3815632ddf88e59f78876ff35bd1e2d548ffbb465b8f`.

Capturas 1280×720, centro 128,128, CLEAN=0 (vehículos incluidos), 180 frames
con ticks e interpolación congelados. Escalas 0.25/0.5/1/2/4: cero píxeles
y bloques 4×4 distintos. En escala 8, la primera pareja difiere en
130 píxeles/81 bloques; el mismo binario anterior reproduce esos 130/81
al repetirse. Dos parejas nuevas antes/después coinciden en PNG y traza
raw. No se borra ni se sustituye la primera evidencia:
[seis zooms](evidence/visual-context-raster-20261001.csv),
[controles y repeticiones Out8x](evidence/visual-context-raster-repeat-20261001.csv).

El stream final de padres/proxies coincide en las seis primeras parejas
al omitir IDs ECS locales, índices de entrada y profundidades de entrada;
se conservan ordinal final, sprite, clave de inserción, profundidad de
salida, bounds, bandas y scopes. El raw difiere en IDs en In4x y además
en 40 índices/dos profundidades de entrada en Out8x. Esa equivalencia no
se usa para ocultar el raster distinto. F17 conserva un sub-issue abierto:
hacer repetible el renderer Out8x con flota; su causa sigue por aislar.
Las capturas son diagnóstico y no certifican paridad nativa universal.

Validación: 3.006 core/seis ignorados y 1.661 cliente/dos; Clippy de todos
los targets de ambos, formato, diff y frescura de docs. Se cierra el descarte
visual cuando no puede ejecutarse callback. F04 y los 30 FPS siguen abiertos.

## Etapa 12 — Ocupación calculada una vez por intento PBS (F13)

`TryPathReserve` prepara un índice de la flota y su ocupación una vez por
intento efectivo de reserva. La búsqueda y las comprobaciones de plataformas
consultan esa misma instantánea de posiciones. Se reconstruye en el siguiente
intento, incluidas las llamadas posteriores a una inversión o movimiento;
no se conserva ocupación de otro tick. Los IDs duplicados usan la variante
anterior. Los retornos tempranos y los segmentos sin PBS evitan construirlo.
Reservas previas, conflictos por track, bit tentativo del depósito, rollback,
flags stuck y orden de mutaciones conservan sus reglas.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`vehicle.cpp::VehiclesOnTile::Iterator`, líneas 495–521, consulta el hash
espacial y filtra la tesela real; `train.cpp` utiliza `VehiclesOnTile` al
comprobar reservas. El port ya tenía `TrainOccupancyIndex` para otras fases
PBS, pero este intento reconstruía cadenas completas dentro de cada consulta.
No se acredita aquí la paridad nativa de todas las búsquedas PBS.

Una captura `cpu-clock:u`, 499 Hz, con frame pointers en core produjo
6.554 muestras y cero perdidas. Al filtrar por el ancestro de movimiento,
2.941 de 3.181 muestras contienen `TryPathReserve` y 2.906 contienen consultas
de huella de formación. Son porcentajes inclusivos que se solapan;
la carga/importación queda fuera de ese denominador.
[Desglose](evidence/pbs-attempt-perf-20261001.csv).
La captura diagnóstica no se mezcla con los tiempos release normales.

Regresiones diferenciales: 32 combinaciones de bloqueo, historial de formación,
vagones, plataforma, reserva ajena, ID duplicado, backoff y mark-stuck; otras
32 después de mover la cabeza que bloqueaba. Más 28 intentos desde depósito
con bit tentativo/previo, entrada bloqueada, rollback, tren detenido y path
vacío. Los 92 intentos comparan resultado, teselas y vehículos serializados;
los escenarios iniciales también comprueban RNG. Incluyen reservas positivas
de plataformas y salidas de depósito, además de los bloqueos.

Release normal, misma Kale_TitleGame.sav/NewGRF, ocho runs en orden
antes/después/después/antes por ventana, sin otras mediciones ni builds.
En 24 ticks: total 88,58 / 87,62 → 41,31 / 40,56 ms;
movimiento 51,57 / 51,25 → 3,44 / 3,42 ms. En 120 ticks:
total 81,40 / 80,40 → 31,86 / 32,27 ms (aproximadamente 60 % menos),
movimiento 52,69 / 52,67 → 4,09 / 4,14 ms. No se comparan ventanas de
longitud diferente como si fueran una mejora.
[Tiempos](evidence/pbs-attempt-timings-20261001.csv).
Baseline `aa2e68ee`, SHA256
`ea37c5395f7a659943c32e0ad47b6db7e7b3f316564fcf2987722dd43b991ccb`;
candidato `51e3764532b7e4e5d7804ca1b1c3e02101ba39b37080ae396808d3b9de1c59c8`.

El estado inicial y 60 ticks normales coinciden sin ordenar las listas en la
sonda: hash canónico v4, eventos/popups/sonidos, todas las teselas y bloques
4×4; cero diferencias en 61 fases.
[Comparación](evidence/pbs-attempt-state-20261001.csv).
Cada sonda se enlaza con el artefacto `--lib` identificado por Cargo JSON de
su versión; se corrigió una primera sonda que había tomado un rlib antiguo
y difería ya antes del primer tick. SHA256 de las sondas correctas:
`374ece09f30486f007fb7bd67a2de2054d5d30a92f68d832ca08c5ed9ebd7fc3`
y `fd844ec5ceeb481a09a5eba179bbd9488b99889f4a16600abccbe10cef13c229`.

Cliente en marcha, ABBA de 40 muestras tras 30 de warmup: simulación
114,23 / 114,37 → 32,76 / 32,77 ms por frame;
FPS 6,22 / 6,22 → 12,68 / 12,66. Los 80 frames nuevos siguen sobre 33,33 ms.
Método y colas en
[RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-indexar-los-intentos-pbs-2026-10-01),
[muestras](evidence/pbs-attempt-client-20261001.csv).
SHA256 de clientes: `1cf2e9b444835498c2ca3815632ddf88e59f78876ff35bd1e2d548ffbb465b8f`
y `6872cdb9f8bd35675ba5294959b56450d531d668f9692ade1ab5cd43602f36b3`.

Validación: 3.008 core/seis ignorados y 1.661 cliente/dos; Clippy en todos
los targets, formato, diff y frescura de docs. Se cierra la reconstrucción
por consulta dentro de este intento. F13, F16 y los 30 FPS siguen abiertos;
las sincronizaciones de órdenes y las comprobaciones de aeronaves son los
siguientes costes de simulación medidos.

## Etapa 13 — Compartir candidatos de depósito al sincronizar órdenes (F07)

Las dos tandas de sincronización de órdenes del tick comparten un
`DepotSpatialIndex` local. Su primer lookup recorre el mapa; los siguientes
consultan únicamente candidatos compatibles. Si ninguna orden requiere esa
búsqueda, no se inicializa. La recursión de órdenes condicionales y servicio
opcional conserva el mismo índice. Cada tanda comienza con uno nuevo y sigue
calculando la elección desde la posición actual; no retiene un destino ni
depende de invalidaciones de otro consumidor. Los callers aislados conservan
la API y la búsqueda anterior.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `order_cmd.cpp::UpdateOrderDest`,
líneas 1949–2068, resuelve depósitos concretos por identidad y usa
`FindClosestDepot` para una orden nearest. `depot_base.h` conserva un pool de
depósitos y `GetByTile`. El port consultaba todas las teselas por cada orden
cuyo depósito concreto ya no existía. Esta etapa elimina ese trabajo
repetido y conserva su selección Manhattan y desempate por fila/columna;
no acredita la paridad de identidad, propiedad o alcanzabilidad del buscador
nativo. Tampoco cambia la aceptación previa de un depósito existente de
otra clase. Esas diferencias de resolución siguen pendientes.

Regresiones: 216 combinaciones de seis clases de vehículo, tres posiciones
y doce listas/estados, más un empate explícito. Comparan todos los campos
serializados con la variante escalar: lista vacía, destino existente o
ausente/fuera del mapa, servicio opcional, condicional/ciclo, implícitas,
índice inválido y espera de carga. Las tandas sin búsqueda hacen cero scans;
las que la necesitan hacen uno aunque consulten varias clases. Otra
regresión compara 32 vehículos en tres tandas sobre un mapa 256²: depósito
demolido/nuevo, cabeza movida y desaparición de todos los candidatos;
elección y avance de orden coinciden y cada tanda hace un scan.

Ocho runs release normales de Kale_TitleGame.sav/NewGRF, ABBA por ventana,
sin builds ni otras capturas concurrentes. En 24 ticks: total
40,25 / 40,57 → 31,08 / 30,86 ms; sincronización previa de órdenes
5,08 / 5,06 → 0,340 / 0,337 ms; cierre del tick
6,49 / 6,44 → 1,68 / 1,66 ms. En 120 ticks: total
35,56 / 31,97 → 22,32 / 22,49 ms (aproximadamente 34 % menos al promediar
ambos runs), sincronización 5,03 / 5,05 → 0,321 / 0,323 ms y cierre
6,34 / 6,37 → 1,61 / 1,63 ms. Se conservan las variaciones entre runs.
[Tiempos](evidence/depot-orders-timings-20261001.csv).
Baseline `334e71c2`, SHA256
`51e3764532b7e4e5d7804ca1b1c3e02101ba39b37080ae396808d3b9de1c59c8`;
candidato `c58160ce047a91ded5fc42bc5b4089c3fb9c3d3d605783590fa3979c4ae6612d`.

Dos imports independientes y 60 ticks normales conservan tick, hash v4,
eventos/popups/sonidos, todas las teselas y bloques 4×4: cero diferencias en
61 fases, sin ordenar listas en la sonda.
[Comparación](evidence/depot-orders-state-20261001.csv).
La biblioteca del candidato se identifica con Cargo JSON `--lib`;
SHA256 de sondas:
`fd844ec5ceeb481a09a5eba179bbd9488b99889f4a16600abccbe10cef13c229`
y `589f77a3a0c6a59df0a5816be03b2faba7b7f300bc3bb8246ca378c5d4c32608`.

Cliente en marcha: ABBA, 40 muestras tras 30 de warmup. Simulación:
32,65 / 32,62 → 23,93 / 23,95 ms por frame;
FPS: 12,69 / 12,68 → 14,45 / 14,29. Los 80 frames nuevos todavía exceden
33,33 ms. Ventanas de ticks y colas en
[RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-compartir-candidatos-de-depósito-2026-10-01),
[muestras](evidence/depot-orders-client-20261001.csv).
SHA256 de clientes:
`6872cdb9f8bd35675ba5294959b56450d531d668f9692ade1ab5cd43602f36b3`
y `5f2583d43d7615c3bd533b709309226e81a89ba29c9a019b561bd4e4385fc0d3`.

Validación: 3.010 core/seis ignorados y 1.661 cliente/dos; Clippy en todos
los targets, formato, diff y frescura de docs. Se cierra el scan por vehículo
en estas dos tandas. Reutilizar candidatos entre tandas/ticks requiere
cubrir su invalidación; F07 y los 30 FPS permanecen abiertos.

## Etapa 14 — Despachar la fase aérea sólo a aeronaves (F07)

Con IDs únicos, la fase aérea omite trenes, buses, camiones, tranvías y
barcos antes de preparar sus contextos y entradas de comprobación de freno.
Conserva el orden de los aviones, incluidos seguidores y detenidos. Comprueba
los IDs vivos mediante un conjunto local: operaciones de depósito pueden
haber cambiado la flota desde la construcción del índice del tick. Con IDs
duplicados mantiene el recorrido anterior; una entrada de otro tipo puede
resolver por ID a un avión anterior y consumir otra tirada de accidente.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`aircraft_cmd.cpp::Aircraft::Tick`, líneas 2134–2154, ejecuta el handler de
esa clase y filtra además `IsNormalAircraft`. El helper aéreo del port ya
descartaba otros tipos, pero el caller seguía encolándolos para la búsqueda
de vehículo/estación y comprobación de freno. Esta etapa elimina ese trabajo;
conserva el modelo actual de unidades aéreas y no cierra toda su paridad nativa.

Regresiones diferenciales: cuatro configuraciones de running/plane-speed
durante 40 fases cada una, con tres aeronaves (incluido helicóptero y
seguidor), cinco clases terrestres/navales y un aterrizaje positivo.
Otras 16 combinaciones de running, velocidad, cheat e ID duplicado verifican
entrada a freno FTA, accidente positivo, vehículo eliminado, noticias,
eventos/sonidos, estaciones, teselas y RNG. Dos casos duplicados con el
cheat comprueban explícitamente que se conservan las dos tiradas. No se
reordena la flota ni se descarta una aeronave por su estado de consist.

Ocho runs release normales de Kale_TitleGame.sav/NewGRF, ABBA por ventana,
sin builds ni otras capturas concurrentes. En 24 ticks: total
30,69 / 31,61 → 27,03 / 27,20 ms; fase previa al movimiento
4,73 / 4,76 → 1,20 / 1,20 ms. En 120 ticks: total
22,12 / 26,17 → 18,90 / 22,72 ms (aproximadamente 14 % menos al promediar
ambos runs); fase previa 4,63 / 4,64 → 1,23 / 1,24 ms.
El componente `vehicle_ops_only` pasa de 3,66 / 3,66 a 0,251 / 0,254 ms.
Landscape varía en ambos binarios: 3,18 / 7,18 y 3,31 / 6,96 ms;
se conserva esa variación y no se atribuye al filtro aéreo.
[Tiempos](evidence/aircraft-phase-timings-20261001.csv).
Baseline `daa8f652`, SHA256
`c58160ce047a91ded5fc42bc5b4089c3fb9c3d3d605783590fa3979c4ae6612d`;
candidato `61e90735174c56e15fe017f6a1d6d3d956701afae26e50c186a7d49821eeb472`.

Dos imports independientes y 60 ticks normales: cero diferencias en tick,
hash v4, eventos/popups/sonidos, todas las teselas y bloques 4×4, en 61 fases
sin ordenar listas en la sonda.
[Comparación](evidence/aircraft-phase-state-20261001.csv).
Cargo JSON `--lib` identifica la biblioteca exacta del candidato;
SHA256 de sondas:
`589f77a3a0c6a59df0a5816be03b2faba7b7f300bc3bb8246ca378c5d4c32608`
y `71c1eec12edcfa42af43be7251ceb47b0a605fbc58fe4a76fbfc834dddbc3261`.

Cliente en marcha, ABBA de 40 muestras tras 30 de warmup: simulación
23,85 / 23,90 → 20,95 / 20,80 ms por frame;
FPS 14,15 / 14,47 → 14,87 / 14,85. Los 80 frames nuevos siguen sobre
33,33 ms. Ventanas de ticks y colas en
[RENDIMIENTO.md](../RENDIMIENTO.md#cliente-en-marcha-tras-filtrar-la-fase-aérea-2026-10-01),
[muestras](evidence/aircraft-phase-client-20261001.csv).
SHA256 de clientes:
`5f2583d43d7615c3bd533b709309226e81a89ba29c9a019b561bd4e4385fc0d3`
y `b1f7707905fd79ff868be682cad2223c10cc0c8170df70ae591d51bdf821104b`.
La revisión automática de permisos venció antes de iniciar la captura GPU;
el único reintento permitido se autorizó y las cuatro corridas finalizaron.

Validación: 3.012 core/seis ignorados y 1.661 cliente/dos; Clippy en todos
los targets, formato, diff y frescura de docs. Se cierra el trabajo de fase
aérea sobre los otros tipos en flotas con IDs únicos. F07 y los 30 FPS siguen
abiertos; la presentación domina el tiempo restante del frame medido.

## Etapa 15 — Conservar los vínculos de máscaras de vidrio (F08)

La fase de vidrio conserva un índice local source→proxy con las generaciones
de entidad de Bevy. Deja de construir dos `HashMap` por frame; comprueba la
existencia del proxy, lo recrea tras un remap y retira entradas cuyo source ya
no participa en el query. Compara los campos prestados antes de clonar el
sprite de máscara. Sigue verificando geometría, visibilidad y clasificación
vidrio/opaco incluso cuando el sprite fuente no tiene cambios registrados.
El índice no forma parte del estado persistente ni de la simulación.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `8bpp_simple.cpp::Draw`: el modo
transparente aplica el remap al píxel de destino donde el sprite tiene
cobertura. Los targets, shader, LUT, muestreo de atlas, profundidad y pases
auxiliares del port conservan ese contrato existente. Esta etapa reduce la
sincronización de proxies; no reemplaza el blitter ni acredita paridad visual
global con el original.

Diagnóstico anterior al cambio: `perf record` de nuestro cliente, evento
`cpu-clock:u`, 199 Hz. DWARF produjo 3.688 muestras sin pérdidas pero sólo 162
líneas de frames decodificadas; no permite atribuir ancestros de forma completa.
Se compiló únicamente el cliente con frame pointers, debuginfo 1 y unwind
tables, sin cambiar la configuración del proyecto ni reconstruir dependencias.
El perfil FP tiene 3.686 muestras sin pérdidas y 7.952 líneas de frames. En la
ventana 6062,000732–6066,500732 s, 111 de 1.110 muestras de todos los threads
incluyen el compositor. Dentro de esas 111, 76 terminan en su closure y 29 en
inserciones/hash de entidades. Core/dependencias no tienen frame pointers:
los ancestros de otras fases siguen incompletos y sus ceros no indican ausencia
de trabajo. [Conteos y denominadores](evidence/glass-compositor-perf-20261001.csv).
El binario diagnóstico SHA256 es
`7783feb95fc604d20e2f2ea31d62d5e4bf4838958b652864ad5c8a6806ead42f`;
sus tiempos con `perf` no se usan como comparación normal de FPS.

Regresiones: dos sistemas ECS independientes comparan 49 estados de 64
fuentes, con cambios de imagen/atlas/modo, geometría, visibilidad, marker de
vidrio, chunk, pertenencia estática/dinámica y borrado/recreación de fuente o
proxy. Se comparan todos los componentes de máscara y su vínculo lógico, con
una máscara por fuente y sin huérfanos. Otra prueba cubre inicialización con
un proxy existente, reparación de una edición externa sin cambio del source
y ausencia de writes de extracción en un frame sin cambios.

Cliente normal: baseline `f384eaaf`, ABBA de cuatro corridas, la misma partida,
NewGRF, 1280×720, escala 2, audio desactivado, 30 frames de warmup y 40 muestras
por corrida, sin `perf` ni compilaciones concurrentes. Vidrio medio:
6,524 / 6,581 → 4,580 / 4,583 ms, aproximadamente 30 % menos. Frame medio:
66,869 / 67,357 → 65,886 / 63,981 ms; FPS por duración media:
14,955 / 14,846 → 15,178 / 15,630. Los p95 conservan variación:
84,037 / 85,142 → 95,538 / 81,664 ms. Los 80 frames de cada versión siguen
superando 33,33 ms. [Muestras](evidence/glass-cache-client-20261001.csv).

Raster: seis zooms, centro 128,128, 180 frames de settle y `CLEAN=0`, con flota.
Las seis parejas tienen PNG SHA256 idéntico, cero píxeles/bloques 4×4 distintos
y trazas completas del sorter byte-idénticas. Se conservan también los conteos
de parents (54 a 37.998) y la comparación semántica.
[Resultados](evidence/glass-cache-raster-20261001.csv).
No se cierra la variación Out8x observada en etapas anteriores: estas seis
parejas no certifican todas las repeticiones posibles.

Cámara en movimiento: otras cuatro corridas ABBA de 40 frames con la misma
simulación activa. Frame medio 73,153 / 73,194 → 70,103 / 70,739 ms; FPS:
13,670 / 13,662 → 14,265 / 14,137. Vidrio 6,915 / 6,994 → 4,973 / 5,020 ms.
p95 106,912 / 104,848 → 96,325 / 102,620 ms, pero máximo/p99
159,488 / 161,614 → 157,690 / 166,074 ms. Los 80 frames de cada versión
exceden el presupuesto. Ticks 3.703.103–3.703.142 salvo el primer run nuevo,
3.703.104–3.703.143. [Muestras](evidence/glass-cache-pan-20261001.csv).

Validación: 3.012 core/seis ignorados y 1.663 cliente/dos, Clippy de ambos en
todos los targets, formato, diff y frescura de docs. Binarios normales antes/
después SHA256
`b1f7707905fd79ff868be682cad2223c10cc0c8170df70ae591d51bdf821104b` y
`334ca38dcde0e710cf988f6cdbdd0f9dd86623c2c76cd0f2faafca91581fbe27`.
Se cierra la reconstrucción de las dos tablas por frame y la clonación de
máscaras iguales. F08 conserva el barrido y los pases auxiliares; F17 y el
presupuesto de 30 FPS siguen abiertos.

## Etapa 16 — Comparar máscaras sólo tras cambios de componentes (F08)

Sobre la etapa 15, el query conserva los change ticks de Sprite, Anchor,
Transform, Visibility y chunk. Cada comparación se realiza cuando cambió el
componente fuente o el del proxy. La clasificación vidrio/opaco se recuerda
por separado para detectar también la retirada del marker; inicializar un
vínculo existente fuerza la comprobación completa. Los escritores actuales
del renderer usan el acceso mutable de Bevy, sin bypass de change detection.
No se modifican shader, targets, cámaras ni pases. El contrato nativo de
transparencia y el oracle de fuente son los de la etapa 15.

La regresión diferencial de 49 estados/64 fuentes sigue comparando todos los
componentes frente al sincronizador anterior a ambas optimizaciones. Incluye
marker retirado sin modificar Sprite, entidades eliminadas/recreadas y
visibilidad/pose cambiadas. La reparación externa se amplió al chunk del
proxy, además de sprite, alpha, anchor, pose y visibilidad. Un frame estable
no marca cambios de extracción. Estas pruebas cubren los cambios que los
indicadores deben detectar; el gate raster se ejecutó de nuevo.

Cliente normal: baseline `bac966bc`, ABBA, la misma partida/NewGRF, escala 2,
1280×720, 30 frames de warmup y 40 muestras por corrida. Sin compilaciones ni
`perf` concurrentes. Vidrio medio 4,537 / 4,594 → 2,968 / 3,018 ms, alrededor
de 34 % menos. Frame medio 65,187 / 64,779 → 63,042 / 63,939 ms; FPS por
duración media 15,341 / 15,437 → 15,862 / 15,640. p95
81,955 / 94,994 → 80,634 / 91,564 ms; máximo/p99
165,316 / 155,480 → 161,705 / 161,515 ms. Los 80 frames de cada versión
siguen sobre 33,33 ms. Ticks 3.703.103–3.703.142 salvo el segundo run anterior,
3.703.104–3.703.143. [Muestras](evidence/glass-changes-client-20261001.csv).

Seis zooms con flota, centro 128,128, 180 frames de settle y `CLEAN=0`:
las seis parejas vuelven a conservar PNG y traza completa byte-idénticos,
cero píxeles/bloques 4×4 distintos y los mismos 54–37.998 parents.
[Raster](evidence/glass-changes-raster-20261001.csv). No se extrapola a todas
las ejecuciones posibles ni se cierra la variación histórica Out8x.

Cámara en movimiento: otras cuatro corridas ABBA. Frame medio
70,903 / 70,958 → 68,663 / 68,971 ms; FPS
14,104 / 14,093 → 14,564 / 14,499. Vidrio
5,001 / 4,957 → 3,260 / 3,303 ms. p95
102,303 / 103,109 → 100,308 / 102,004 ms, máximo/p99
164,431 / 162,349 → 158,410 / 160,519 ms. Todos cubren ticks
3.703.103–3.703.142 y sus 80 frames por versión exceden el presupuesto.
[Muestras](evidence/glass-changes-pan-20261001.csv).

Validación: 3.012 core/seis ignorados, 1.663 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. Binarios normales SHA256
`334ca38dcde0e710cf988f6cdbdd0f9dd86623c2c76cd0f2faafca91581fbe27` y
`01e20e3bf67c84098cf506f5d19851f72136885da30e38c3e3d9b82bc6620f87`.
Se cierra la comparación de valores estables en este sincronizador; el barrido,
la presentación restante, los pases auxiliares y los 30 FPS siguen abiertos.

## Etapa 17 — Compartir conteos globales entre chunks del remap (F04/F17)

Los chunks de una tanda leen el mismo `SimWorld` inmutable. Se preparan los
conteos de casas/objetos una vez, de forma lazy al primer chunk que realmente
se materializa, y se comparten entre chunks añadidos y refrescados. Cada nueva
invocación de remap reconstruye la instantánea; no se guarda entre ticks ni
mundos. El pase completo conserva su preparación local y el orden de
chunks, callbacks y spawns no cambia.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `newgrf_house.cpp::GetNumHouses`
consulta contadores globales/por pueblo y `newgrf_object.cpp` usa
`Object::GetTypeCount`. El port conserva sus builders existentes y la parte
de contexto que ya soportaban; esta etapa no incorpora clases de casas ni
amplía la semántica de variables relativas. La regresión coloca casas y
objetos fuera del chunk visible, verifica vars 44/60 y conteos por pueblo,
cuenta instancias de objetos con distintos footprints y vuelve a comprobar
los valores después de demoler. La instantánea anterior conserva sus valores.

Baseline `516a09c5`, misma partida/NewGRF y hardware, 1280×720, escala 2,
audio desactivado, cuatro corridas ABBA por escenario, 40 muestras cada una,
sin compilaciones ni `perf` concurrentes. Con warmup 30, remap medio
10,846 / 10,556 → 10,019 / 9,870 ms; frame medio
62,806 / 62,183 → 62,350 / 61,557 ms. FPS por duración media:
15,922 / 16,082 → 16,038 / 16,245. p95
80,081 / 80,366 → 78,283 / 78,076 ms; máximo/p99
159,569 / 151,790 → 158,611 / 150,044 ms. Ticks 3.703.103–3.703.142
en los primeros runs de ambas versiones y 3.703.104–3.703.143 en los segundos.
[Muestras](evidence/chunk-scopes-client-20261001.csv).

Con cámara en movimiento y warmup 30: remap
13,533 / 13,689 → 12,469 / 12,537 ms; frame medio
68,711 / 69,132 → 67,772 / 68,020 ms; FPS
14,554 / 14,465 → 14,755 / 14,702. p95
103,413 / 102,231 → 98,491 / 97,806 ms; máximo/p99
159,491 / 161,411 → 159,367 / 157,661 ms. Todos cubren ticks
3.703.103–3.703.142. [Muestras](evidence/chunk-scopes-pan-20261001.csv).

Se detectó que el máximo de las corridas de cámara fija de las etapas 15/16
está en la primera muestra. `begin_frame` cambia la escala en el frame 30;
la primera muestra de `frame_ms` mide el intervalo iniciado en ese frame.
Con warmup 30 incluye por tanto ese cambio de viewport. Se conservan íntegros
los datos anteriores y el protocolo comparable de esta etapa; no se describen
sus máximos como picos periódicos de simulación ni como rendimiento sostenido.

Se añadió otra comparación ABBA con **warmup 120**, mismo cliente activo,
escala 2 y 40 muestras. Remap 11,475 / 11,532 → 10,908 / 10,922 ms;
frame medio 60,205 / 59,791 → 59,522 / 59,795 ms; FPS
16,610 / 16,725 → 16,801 / 16,724. p95
87,800 / 87,723 → 85,653 / 86,355 ms; máximo/p99
95,341 / 95,587 → 93,757 / 95,787 ms. El primer run anterior cubre ticks
3.703.194–3.703.233; los otros tres, 3.703.193–3.703.232. No se compara esta
ventana más avanzada de la partida con warmup 30 para atribuir una ganancia.
La mejora sostenida de FPS es pequeña y no uniforme; el problema de frame
persiste. En los tres escenarios, los 80 frames de cada versión superan
33,33 ms. [Muestras sostenidas](evidence/chunk-scopes-client-steady-20261001.csv).

Raster: las seis parejas con flota, centro 128,128, 180 frames de settle y
`CLEAN=0` conservan PNG y traza completa byte-idénticos, cero píxeles/bloques
4×4 distintos y los mismos 54–37.998 parents.
[Resultados](evidence/chunk-scopes-raster-20261001.csv).

Validación: 3.012 core/seis ignorados, 1.664 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. SHA256 de binarios normales:
`01e20e3bf67c84098cf506f5d19851f72136885da30e38c3e3d9b82bc6620f87` y
`e4ea3e72f545188546769e8f9ff522879c90fb8eaf4641055e4d1352aa57c77e`.
Se cierra la repetición de estos builders dentro de una tanda incremental.
Siguen pendientes su cálculo a demanda, el coste de rematerialización, F17
y el objetivo de 30 FPS.

## Etapa 18 — Descartar la eliminación de máscaras ocultas (F08/F17)

Se ensayó no materializar `SpriteMesh` auxiliares para fuentes con
`Visibility::Hidden`, eliminando su vínculo y reconstruyéndolo al mostrarse.
La hipótesis era reducir entidades que no aportaban fragmentos. La prueba
ECS verificó ocultar/mostrar y cambios de clasificación mientras estaban
ocultas, y pasaron 3.012 tests core/seis ignorados, 1.665 cliente/dos,
Clippy y formato. Esa cobertura de componentes no garantiza el mismo raster.

El gate de seis zooms conserva PNG y traza completa en In4x, In2x, Normal y
Out2x, pero falla en Out4x: **730 píxeles / 239 bloques 4×4 distintos**;
en Out8x: **362 píxeles / 207 bloques**. Las trazas completas de las seis
parejas son byte-idénticas, incluidos los parents y proxies lógicos. Por
tanto el sorter no basta para aceptar un cambio de entidades auxiliares.
[Gate inicial](evidence/hidden-masks-raster-20261001.csv).

Se añadieron dos repeticiones de cada binario en ambos zooms, en orden ABBA.
Las tres capturas nuevas de Out4x tienen exactamente el mismo SHA256 y la
diferencia de 730 píxeles; las tres anteriores coinciden. Las tres capturas
nuevas de Out8x también coinciden entre sí. Una de las dos repeticiones del
cliente anterior vuelve a mostrar su variación histórica de 130 píxeles /
81 bloques y cambia su traza; se conserva, sin atribuirla al experimento.
La regresión reproducible de Out4x es suficiente para rechazarlo.
[Repeticiones](evidence/hidden-masks-raster-repeat-20261001.csv).
No se ha aislado todavía qué desempate del renderer auxiliar causa el cambio.

Medición normal sobre `ace33f52`, misma partida/NewGRF, hardware, audio
desactivado, 1280×720, escala 2, cuatro corridas ABBA por escenario, 40 frames
cada una, sin compilación ni `perf` concurrentes. Cámara fija, warmup 30:
62,514 / 62,270 → 61,798 / 61,943 ms; vidrio
2,912 / 2,923 → 2,901 / 2,915 ms. Se evitan unas 920 máscaras de 46.135.
[Muestras](evidence/hidden-masks-client-20261001.csv).
Cámara en movimiento, warmup 30: 68,035 / 68,037 → 67,757 / 66,642 ms;
se evitan unas 883 máscaras. El segundo run nuevo comienza un tick después.
[Pan](evidence/hidden-masks-pan-20261001.csv).
Cámara fija, warmup 120: 59,116 / 59,465 → 59,376 / 59,019 ms,
16,916 / 16,817 → 16,842 / 16,944 FPS; vidrio
3,281 / 3,394 → 3,356 / 3,332 ms. Los cuatro runs cubren ticks
3.703.193–3.703.232 y se evitan unas 966 máscaras de 46.565.
No hay una mejora sostenida clara. Los 80 frames de cada versión y escenario
siguen sobre 33,33 ms. [Ventana sostenida](evidence/hidden-masks-client-steady-20261001.csv).
Se mantiene la salvedad del intervalo de cambio de zoom con warmup 30.

El experimento se retiró: el código del compositor se restauró desde la
copia propia, comprobada byte a byte contra `ace33f52`, y el ejecutable normal
se repuso desde el baseline inmutable. Se conservan datos y ambos binarios
diagnósticos. SHA256 baseline/candidato rechazado:
`e4ea3e72f545188546769e8f9ff522879c90fb8eaf4641055e4d1352aa57c77e` y
`1890f8c047c6a8a98a27b312b4cea4a2b2a6479766231412cf40ed065083c3af`.
Esta etapa sólo publica evidencia; no cierra F08, F17 ni los 30 FPS.

## Etapa 19 — Descartar stocks sin cambios antes de buscar nuevos cargos (F07/F12)

Una captura del núcleo con `perf record -e cpu-clock:u -F 499 -D 2000
--call-graph dwarf,8192`, 1.200 ticks y NewGRF, atribuye 1.634 de 12.035
muestras propias (13,58 %, cero perdidas) al iterador de
`trigger_station_new_cargo_since`. Cada operación de producción/distribución
comparaba los 64 cargos de las 245 estaciones, aunque casi todas conservaran
el stock. El denominador incluye workers de CargoDist; no es un porcentaje
del frame ni una medición normal de FPS. Los conteos proceden de `perf report`
con columna `sample`, porque el primer frame de la pila DWARF puede atribuir
distinto código inline. [Perfil](evidence/station-stock-delta-perf-20261001.csv).

La detección descarta primero stocks completos iguales y conserva el orden
original estación/cargo para los restantes. No cambia los snapshots, la
distribución, los callbacks ni su despacho. Oracle de fuente: OpenTTD 15.3
`14ec60f2`, `station_cmd.cpp::UpdateStationWaiting`, sólo dispara NewCargo
tras agregar unidades; `newgrf_airporttiles.cpp::TriggerAirportAnimation`
consume la palabra RNG base también en un aeropuerto vanilla, antes de
comprobar los GRF de sus teselas. Por eso no se filtra por ausencia de GRF o
callbacks. Esta etapa conserva la semántica del port anterior; no acredita
paridad completa de todos los triggers con el nativo.

La regresión compara la secuencia exacta con el barrido anterior para los
64 cargos y cinco patrones (320 casos): sin cambios, aumentos, descensos,
aumento con descenso de otro cargo manteniendo el total y cambios en una sola
estación con cargo custom 32. También cubre saturación y lista anterior vacía.
Otra prueba verifica cero palabras para un stock estable o decreciente y dos
palabras para dos cargos nuevos en un aeropuerto vanilla.

Baseline `9f5a4c24`, partida/NewGRF y órdenes importadas sin reordenar:
61 fases, inicial más 60 ticks normales, con hash canónico v4, hash de eventos,
teselas completas y bloques 4×4. **Cero fases diferentes**, incluidos RNG y
entidades persistidas; cero teselas/bloques distintos. Tick final 3.703.134,
hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Comparación](evidence/station-stock-delta-state-20261001.csv).

Núcleo normal, sin `perf`, compilaciones ni GPU concurrentes: ABBA con 24 y
120 ticks. Con 24, paisaje medio 4,265 / 3,561 → 0,269 / 0,272 ms y tick
28,179 / 27,173 → 24,131 / 23,958 ms. Con 120, paisaje
3,574 / 3,162 → 0,290 / 0,289 ms y tick
19,217 / 18,751 → 15,759 / 15,738 ms. Son ventanas desde la carga; incluyen
las rutas iniciales y no certifican el presupuesto de todos los ticks.
[Fases](evidence/station-stock-delta-core-20261001.csv).

Cliente normal, mismo hardware, audio desactivado, 1280×720, escala 2,
cuatro corridas ABBA por escenario y 40 muestras cada una:

- Cámara fija, warmup 30: simulación 20,495 / 20,669 → 16,412 / 16,417 ms;
  frame 62,505 / 62,527 → 58,269 / 58,442 ms; FPS
  15,999 / 15,993 → 17,162 / 17,111. p95
  78,334 / 79,394 → 75,247 / 74,682 ms; máximo/p99
  163,649 / 158,031 → 156,146 / 155,604 ms.
  [Muestras](evidence/station-stock-delta-client-20261001.csv).
- Cámara en movimiento, warmup 30: simulación
  20,487 / 20,644 → 16,683 / 16,485 ms; frame
  67,990 / 68,499 → 64,437 / 64,209 ms; FPS
  14,708 / 14,599 → 15,519 / 15,574. p95
  97,581 / 98,759 → 94,597 / 94,371 ms; máximo/p99
  161,658 / 160,420 → 154,596 / 155,258 ms.
  [Pan](evidence/station-stock-delta-pan-20261001.csv).
- Cámara fija, warmup 120: simulación
  20,003 / 19,795 → 15,807 / 15,872 ms; frame
  60,133 / 59,302 → 55,534 / 55,524 ms; FPS
  16,630 / 16,863 → 18,007 / 18,010. p95
  84,822 / 87,400 → 80,818 / 81,963 ms; máximo/p99
  94,237 / 93,812 → 90,527 / 90,944 ms.
  [Ventana sostenida](evidence/station-stock-delta-client-steady-20261001.csv).

Ambas ventanas de warmup 30 cubren ticks 3.703.103–3.703.142. Con warmup 120,
el primer run anterior cubre 3.703.194–3.703.233 y los otros tres
3.703.193–3.703.232. Se mantiene la salvedad del intervalo de cambio de zoom
de la etapa 17 y no se mezclan ventanas para atribuir una ganancia. En cada
escenario, los 80 frames de cada versión exceden 33,33 ms. Remap permanece
alrededor de 10–11 ms fijo y 12,6 ms en movimiento; las fases no suman el frame.

Las seis parejas congeladas con flota, centro 128,128, settle 180 y `CLEAN=0`
conservan PNG y traza completa byte-idénticos: cero píxeles/bloques 4×4
distintos y los mismos 54–37.998 parents. No cierran la variación histórica
Out8x ni acreditan cada frame activo. [Raster](evidence/station-stock-delta-raster-20261001.csv).

Validación: 3.014 core/seis ignorados y 1.664 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. Binarios normales SHA256:
cliente anterior
`e4ea3e72f545188546769e8f9ff522879c90fb8eaf4641055e4d1352aa57c77e`,
posterior `d809e1117637e0bb8b3e409baee99a4e01d24a0f047f0d0a4f249505dedd57f2`;
`sav_profile` anterior
`61e90735174c56e15fe017f6a1d6d3d956701afae26e50c186a7d49821eeb472`,
posterior `cbab113c3cdb22ae912bf9227091d8df99493e29f5d1d745628e82f7f8e5a19f`.
Se cierra el barrido de cargos para stocks iguales. El redibujado, el tráfico
ferroviario, F07/F12 y los 30 FPS siguen pendientes.

## Etapa 20 — Redibujar campos sólo al cambiar su imagen (F17/F28)

El despacho regular de `CLEAR_FIELDS` marcaba dirty ante cualquier cambio de
tesela. OpenTTD 15.3 `14ec60f2`, `clear_cmd.cpp::TileLoop_Clear`, primero
actualiza cercas y retorna tras incrementar el contador si aún es menor que
siete; sólo marca el cambio de cultivo al completar el ciclo.
`UpdateFences` marca por separado sus cambios. `DrawTile_Clear` usa cultivo,
pendiente y cercas, sin leer los tres bits altos de MAP5 del contador.
El renderer del port consulta esos mismos datos para campos.

El booleano de aviso compara todas las propiedades de la tesela después de
normalizar únicamente esos bits en una copia local. El mapa sigue recibiendo
todas las mutaciones originales; no cambian cultivo, industria, reclamación
de huérfanos, visitas LFSR ni RNG. La cola de generación continúa llamando al
mismo `tile_loop_clear_field`, sin esta decisión de redibujado. No se aplica
el filtro a casas NewGRF: su processing timer sí marca dirty en el nativo.

Las pruebas cubren los nueve cultivos y ocho contadores (72 combinaciones),
las cuatro cercas con un contador intermedio, reclamación de tipos 7/8 y
nieve. La prueba del despacho regular verifica cercas → contador sin aviso
→ cambio de cultivo con aviso y RNG constante.

Desde `dab9bfb9`, 61 fases inicial/60 ticks normales, misma partida/NewGRF y
orden importado: estado canónico v4, eventos y teselas/bloques 4×4 coinciden
exactamente. El tick final conserva `4d3524e48b6bc2e5` y los eventos
`4857641856429973`. [Estado](evidence/field-counter-dirty-state-20261001.csv).
Los avisos de paisaje bajan de **734 a 115**; los 619 retirados corresponden
todos a cambios exclusivos del contador. No se agrega ni retira otro aviso,
y se conserva la secuencia de los restantes. La suma de chunks 16×16
distintos por tick baja de 582 a 102 (no son chunks visibles ni llamadas al
remap); máximos de avisos 22 → 5. La sonda
[`trace_sav_visual_deltas.rs`](../../scripts/trace_sav_visual_deltas.rs) exporta
coordenadas ordenadas y bytes completos para repetir este análisis. No mide
rendimiento. [Deltas](evidence/field-counter-dirty-notifications-20261001.csv).

Núcleo normal, ABBA, sin compilaciones, GPU ni `perf` concurrentes: 24 ticks,
23,776 / 24,076 → 24,597 / 24,316 ms; 120 ticks,
15,831 / 15,856 → 16,092 / 16,215 ms. No se acredita una mejora del núcleo;
el ahorro buscado está en el consumidor visual de los avisos. Estas ventanas
incluyen el primer cálculo de rutas. [Fases](evidence/field-counter-dirty-core-20261001.csv).

Cliente normal, mismo hardware y partida/NewGRF, 1280×720, escala 2, audio
desactivado, ABBA con 40 muestras por corrida y sin otras cargas de medición:

- Cámara fija, warmup 30: remap 9,785 / 10,161 → 9,517 / 9,173 ms;
  frame 57,592 / 59,615 → 55,483 / 54,854 ms; FPS
  17,364 / 16,774 → 18,024 / 18,230. p95
  74,216 / 87,550 → 72,916 / 72,489 ms; máximo/p99
  146,838 / 157,728 → 146,546 / 139,875 ms. Ticks
  3.703.104–3.703.143 en el primer run anterior y segundo posterior;
  3.703.103–3.703.142 en los otros dos. Frames sobre presupuesto: 80/80
  anteriores y 79/80 posteriores. [Muestras](evidence/field-counter-dirty-client-20261001.csv).
- Cámara en movimiento, warmup 30: remap
  12,596 / 12,577 → 11,531 / 11,455 ms; frame
  64,452 / 64,144 → 60,585 / 60,175 ms; FPS
  15,515 / 15,590 → 16,506 / 16,618. p95
  94,322 / 93,608 → 90,105 / 88,823 ms; máximo/p99
  156,499 / 155,149 → 147,298 / 147,856 ms. Todos cubren ticks
  3.703.103–3.703.142; 80/80 frames por versión exceden 33,33 ms.
  [Pan](evidence/field-counter-dirty-pan-20261001.csv).
- Cámara fija, warmup 120: remap
  10,923 / 10,765 → 9,946 / 10,017 ms; frame
  55,631 / 56,137 → 52,638 / 52,580 ms; FPS
  17,975 / 17,814 → 18,998 / 19,019. p95
  81,629 / 82,432 → 74,090 / 76,849 ms; máximo/p99
  92,651 / 90,161 → 84,376 / 84,367 ms. Ticks
  3.703.193–3.703.232 salvo el segundo run anterior,
  3.703.194–3.703.233; 80/80 frames por versión exceden el presupuesto.
  [Sostenido](evidence/field-counter-dirty-client-steady-20261001.csv).

Se mantiene la salvedad del cambio de zoom con warmup 30; no se mezclan
ventanas ni se suman fases para reconstruir el frame.

En el gate congelado de seis zooms, cinco parejas iniciales conservan PNG y
traza completa byte-idénticos. Out4x cambia 319 píxeles / 119 bloques 4×4
en la primera captura del **mismo baseline** que antes producía el PNG
canónico. Sus parents y proxies normalizados coinciden con el candidato;
la traza raw difiere en IDs y veinte índices de entrada. Se conserva esa
captura. [Gate inicial](evidence/field-counter-dirty-raster-20261001.csv).
Las dos repeticiones de cada versión en Out4x/Out8x, en orden ABBA, conservan
PNG y traza completa byte-idénticos respecto del candidato inicial: cero
píxeles/bloques distintos. Out4x vuelve a producir con el baseline el mismo
SHA256 canónico de la etapa 19; el ejecutable baseline es byte-idéntico en
ambas etapas. La captura inicial distinta evidencia una variación preexistente
en este zoom, además de la histórica Out8x. No se atribuye al cambio ni se
elimina del registro. [Controles](evidence/field-counter-dirty-raster-repeat-20261001.csv).
El contador no se ejecuta con ticks congelados, por lo que estas capturas
tampoco acreditan cada frame activo; la variación del renderer sigue abierta.

Validación: 3.017 core/seis ignorados, 1.664 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. SHA256 cliente anterior/posterior:
`d809e1117637e0bb8b3e409baee99a4e01d24a0f047f0d0a4f249505dedd57f2` y
`74560f9bca194437fb3295ecef4bddfc172dd576f9108186ecd796a52a77b9d3`;
`sav_profile`: `cbab113c3cdb22ae912bf9227091d8df99493e29f5d1d745628e82f7f8e5a19f`
y `858d0321d028825f25c96500ad75048ff998c6842c31935485b7ab8ad08b22b3`.
Se cierra el aviso de contador exclusivo de campos. No se cierra F17/F28 ni
el objetivo de 30 FPS; siguen pendientes remap, variación visual y cadencia.

## Etapa 21 — Conservar los avisos de animación hasta el renderer (F17/F28)

`AnimateAnimatedTiles` cambiaba los frames de casas y avisaba al cliente, pero
`phase_tile_loop`, ejecutada después dentro del mismo tick, borraba esos avisos.
La lista de paisaje ahora se limpia al comienzo del tick, junto con señales y
reservas. Las mutaciones de animación y del tile loop llegan juntas al consumidor;
el tick siguiente retira los avisos anteriores. No cambia el orden de fases,
la cola ANIT, la visita LFSR, los eventos ni el consumo de RNG.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`, `openttd.cpp::StateGameLoop`
anima antes de ejecutar `RunTileLoop`; `newgrf_animation_base.h` llama a
`MarkTileDirtyByTile` cuando cambia el frame. `town_cmd.cpp::AnimateTile_Town`
también marca el movimiento de ascensor; `TownDrawHouseLift` dibuja el child en
`14, 60 - GetLiftPosition`. El tile loop no descarta esas invalidaciones.

La regresión de tick completo falla con la limpieza anterior y pasa tanto
con `step` como con `step_profiled`. Una casa NewGRF 64×64, fuera de las dos
primeras franjas LFSR, cambia MAP7 y conserva su aviso; el aviso viejo se retira
y el RNG permanece igual. Al detener ANIT, el tick siguiente no repite el aviso.
La prueba aislada de fase continúa cubriendo el cambio de frame. No se declara
por esta prueba paridad universal de animaciones NewGRF.

Desde `642aa905`, 61 fases normales de la misma partida y orden importado
conservan hash v4, eventos y todos los bytes de teselas/bloques 4×4: cero
diferencias. El tick final mantiene `4d3524e48b6bc2e5`, eventos
`4857641856429973`. [Estado](evidence/house-animation-deltas-state-20261001.csv).
En 60 ticks, los avisos de paisaje pasan de 115 a 459. Los 115 originales y
su orden se conservan; los **344 agregados** corresponden a ascensores vanilla
Large Office, HouseID 4, en 40 coordenadas distintas. La suma de chunks 16×16
distintos por tick pasa de 102 a 420; no equivale a remaps visibles.
[Avisos](evidence/house-animation-deltas-notifications-20261001.csv).

Las seis parejas congeladas con flota, `CLEAN=0`, centro 128,128, settle 180,
1280×720 y escalas 0,25/0,5/1/2/4/8 conservan PNG y traza completa
byte-idénticos: cero píxeles/bloques 4×4 distintos, mismos 54–37.998 parents.
El gate congelado no ejecuta estas animaciones ni cierra la variación histórica
Out4x/Out8x. [Raster](evidence/house-animation-deltas-raster-20261001.csv).

Cliente normal en marcha, mismo hardware/partida/NewGRF, cámara fija en escala
2, warmup 120, ABBA y 40 muestras por corrida, sin otras mediciones o builds:
frame 52,697 / 52,597 → 58,774 / 58,377 ms; FPS
18,977 / 19,012 → **17,014 / 17,130**. Remap
9,883 / 9,950 → 13,197 / 13,029 ms; simulación
15,705 / 15,653 → 15,799 / 15,977 ms. Mediana
51,282 / 51,062 → 56,482 / 55,561 ms; p95
74,136 / 74,489 → 87,509 / 87,104 ms; máximo/p99
86,451 / 86,471 → 94,188 / 92,050 ms. Todos cubren ticks
3.703.193–3.703.232; los 80 frames por versión exceden 33,33 ms.
Tick/s observado: 18,963 / 18,996 → 16,981 / 17,095.
[Muestras](evidence/house-animation-deltas-client-steady-20261001.csv).

Es una corrección visual con coste medido, no una mejora de FPS. La medición
anterior omitía invalidaciones reales. El cliente ya mueve el child del
ascensor directamente; evitar la reconstrucción adicional del chunk requiere
identificar el origen de cada aviso, conservar cambios posteriores del mismo
edificio y comprobar el child después de ordenar. Las casas NewGRF continúan
necesitando el redibujado de sus frames.

Validación: 3.018 core/seis ignorados, 1.664 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. SHA256 cliente anterior/posterior:
`74560f9bca194437fb3295ecef4bddfc172dd576f9108186ecd796a52a77b9d3` y
`d3055e6a745f0b3390655ad96f2c9be7c54d04e8351fb304960eb96032430d8d`;
`sav_profile`: `858d0321d028825f25c96500ad75048ff998c6842c31935485b7ab8ad08b22b3`
y `195d1bee53ea6358daff9f15cf224116477bb5a1de5c42ecee22bfae3fd020aa`.
Se cierra la pérdida de avisos de casas dentro del tick. F17/F28, remap,
cadencia y 30 FPS siguen abiertos.

## Etapa 22 — Actualizar el ascensor sin reconstruir su chunk (F17/F28)

Se conservan todos los avisos recuperados en la etapa 21. La fase de animación
separa el movimiento del ascensor vanilla en `house_lift_animation_dirty`,
consumido por el child que el cliente ya conserva. La clasificación ocurre
antes de otras mutaciones de la casa; un aviso posterior de paisaje, industria,
señal, reserva, construcción o demolición sigue solicitando remap, incluso en
la misma coordenada. Los frames de casas NewGRF mantienen el camino general.
Ambas listas vencen al comienzo del tick siguiente y al limpiar el runtime.
No cambia el esquema persistido, los bytes de mapa, la cola ANIT ni el RNG.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`town_cmd.cpp::TownDrawHouseLift`, child en `14, 60 - GetLiftPosition`;
`AnimateTile_Town` cambia la posición con cadencia de cuatro ticks. El cliente
proyecta las 37 posiciones en la misma entidad. El animador conserva X/Y como
antes y deja Z en manos del ordenador de children; evita escribir `Transform`
si X/Y ya coinciden. Antes restablecía también el Z de creación en cada frame,
lo que invalidaba el child aunque no se hubiese movido.

La regresión del core falla antes de separar los avisos y pasa con `step` y
`step_profiled`; comprueba el aviso NewGRF, el del ascensor, su vencimiento y RNG.
La prueba ECS cubre las 37 posiciones, X desplazado por redondeo de captura,
Z ya ordenado, identidad de entidad y cero escrituras en un update sin cambios.
Ocho casos de remap verifican que el aviso directo no oculte otros cambios.

Desde `68541211`, 61 fases normales, misma partida/NewGRF y orden importado:
hash v4, eventos y teselas/bloques 4×4 idénticos. Tick final
3.703.134, hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Estado](evidence/lift-direct-deltas-state-20261001.csv).
Los **459 avisos siguen siendo 459**: 115 generales y 344 directos, sin
agregados ni descartados. Se conserva el orden relativo de cada lista.
La suma de chunks 16×16 generales distintos por tick baja de 420 a 102;
no equivale a chunks visibles ni a llamadas de remap.
[Avisos](evidence/lift-direct-deltas-notifications-20261001.csv).

Núcleo normal, ABBA, sin otras cargas de medición: 24 ticks,
24,310 / 23,785 → 23,841 / 23,907 ms; 120 ticks,
15,699 / 15,767 → 15,888 / 15,824 ms. No se acredita una mejora del core;
el ahorro está en el consumidor visual. [Fases](evidence/lift-direct-deltas-core-20261001.csv).

Cliente normal, mismo hardware/partida/NewGRF, 1280×720, escala 2, sin audio,
ABBA, 40 muestras por corrida y sin builds o mediciones concurrentes:

- Cámara fija, warmup 120: frame 58,260 / 58,877 → 53,104 / 53,011 ms;
  FPS **17,165 / 16,985 → 18,831 / 18,864**. Remap
  13,111 / 13,219 → 9,984 / 9,941 ms; simulación
  15,676 / 15,989 → 15,983 / 15,896 ms. Mediana
  55,287 / 56,093 → 51,682 / 52,173 ms; p95
  87,641 / 86,612 → 77,222 / 74,731 ms; máximo/p99
  93,183 / 96,395 → 85,415 / 85,888 ms. Todos cubren ticks
  3.703.193–3.703.232; tick/s observado
  17,133 / 16,936 → 18,842 / 18,863.
  [Muestras](evidence/lift-direct-deltas-client-steady-20261001.csv).
- Cámara en movimiento, warmup 30: frame
  65,407 / 66,746 → 60,240 / 59,977 ms; FPS
  **15,289 / 14,982 → 16,600 / 16,673**. Remap
  14,434 / 14,642 → 11,392 / 11,367 ms; simulación
  16,326 / 16,458 → 16,384 / 16,270 ms. Mediana
  64,055 / 62,339 → 55,459 / 55,844 ms; p95
  92,164 / 102,212 → 89,295 / 89,243 ms; máximo/p99
  145,015 / 153,852 → 147,757 / 148,974 ms. El primer run anterior
  cubre ticks 3.703.104–3.703.143 y los otros tres 3.703.103–3.703.142;
  tick/s observado 15,782 / 15,501 → 17,243 / 17,332.
  [Pan](evidence/lift-direct-deltas-pan-20261001.csv).

Los 80 frames por versión y escenario exceden 33,33 ms. Se mantiene la
salvedad del intervalo de cambio de zoom con warmup 30; no se mezclan ventanas
ni se suman fases para reconstruir el frame. Todavía no se alcanza 30 FPS.

Las seis parejas congeladas con flota, `CLEAN=0`, centro 128,128, settle 180,
1280×720 y escalas 0,25/0,5/1/2/4/8 conservan PNG y traza completa
byte-idénticos: cero píxeles/bloques 4×4 distintos, mismos 54–37.998 parents.
[Raster aceptado](evidence/lift-direct-deltas-raster-20261001.csv).
No certifican lectura SAV nativa ni todos los frames activos. La comparación
de estado es entre versiones del port; la fuente nativa aporta la regla del
ascensor. La variación histórica Out4x/Out8x del renderer continúa abierta.

Se rechazó una primera variante que retenía X y sólo cambiaba Y. En Out4x
difería en **88 píxeles / 37 bloques 4×4**, con parents/proxies normalizados
idénticos; los otros cinco zooms coincidían. Dos repeticiones ABBA por versión
reprodujeron la diferencia en todas las capturas del candidato y ninguna del
baseline; Out8x coincidió en todas. El redondeo de captura de sprites añadidos
puede desplazar X; el animador anterior restablecía X en el siguiente update.
La prueba ECS se amplió para cubrirlo y el cambio final conserva esa proyección.
Se guardan el [gate rechazado](evidence/lift-direct-deltas-y-only-raster-20261001.csv),
los [controles](evidence/lift-direct-deltas-y-only-raster-repeat-20261001.csv)
y sus muestras [fijas](evidence/lift-direct-deltas-y-only-client-steady-20261001.csv)
y de [pan](evidence/lift-direct-deltas-y-only-pan-20261001.csv) como evidencia del
experimento; no son las cifras del cambio aceptado.

Validación: 3.019 core/seis ignorados, 1.666 cliente/dos, Clippy en todos los
targets, formato, diff y frescura de docs. Una corrida de tests del cliente
falló por cuota de `/tmp` al copiar el ejecutable en la prueba de assets;
se repitió completa con `TMPDIR` en el directorio de evidencia del workspace,
sin modificar ni omitir esa prueba, y pasó. SHA256 cliente anterior/posterior:
`d3055e6a745f0b3390655ad96f2c9be7c54d04e8351fb304960eb96032430d8d` y
`b93231297b4a5611b3c5d3ee41cfebb7a7c53bbc5d28016545dab6a4f2988f58`;
`sav_profile`: `195d1bee53ea6358daff9f15cf224116477bb5a1de5c42ecee22bfae3fd020aa`
y `ca4d25c8af82dbe3802ea852fa782bc3ab8cb150803ee6162db5674d90f6e62a`.
Se cierra la reconstrucción de chunks por movimiento exclusivo de ascensores
vanilla. F17/F28, la cadencia F07 y el objetivo de 30 FPS siguen abiertos.

## Etapa 23 — Medir la cadencia y rechazar el cambio de reloj aislado (F07)

El `max_delta` actual limita el tiempo real a 27 ms antes de escalarlo por
velocidad. Con `TimePlugin` y `SimulationPlugin` reales, `ManualDuration` y
30 frames de 33,333334 ms, la prueba obtiene **30 ticks en un segundo**,
frente a los 37 que corresponden al intervalo nativo. Una prueba que avanzaba
recursos `Time` manualmente antes de `app.update` no servía para medirlo:
`TimePlugin` vuelve a actualizarlos en `First`. El experimento usa la estrategia
manual del plugin y una inicialización de duración cero.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`video_driver.cpp::GameLoop/GameThread/Tick` mantiene relojes distintos para
juego y dibujo; `video_driver.hpp::GetGameInterval` usa 27 ms a velocidad
normal y `ALLOWED_DRIFT = 5` limita atraso acumulado. El prototipo permite
recuperar hasta cinco ticks por frame y divide el límite real por la velocidad,
pues Bevy 0.19.1 aplica `max_delta` antes de `relative_speed`. Esa cota es una
política propuesta para el cliente, no una reproducción exacta del scheduler
nativo ni una optimización del coste de `GameState::step`.

Cuatro pruebas pasan con el prototipo: 37 ticks en un segundo con
100/60/30/20/10 frames; velocidades 0,25/0,5/1/2/4/8 con
9/18/37/74/148/296 ticks; pausa/reanudación sin deuda de tiempo pausado;
y recuperación de cinco ticks tras diez segundos de bloqueo, con remanente
fraccional previo y sin deuda en el frame siguiente. Los escenarios de tiempo
normal y velocidad conservan hash/RNG frente al mismo número de pasos core.
Update sigue ejecutándose una vez por frame e interpolación queda en [0,1).
Son pruebas de schedules sin compositor, entrada de UI ni peer de red.

Una comprobación adicional del emisor real de humo revela por qué el cambio
aislado aún no es publicable. Un vehículo diesel fijo, incluso sin callbacks
NewGRF, consume **dos valores de RNG** al evaluar ticks 1 y 2 por separado,
y **uno** si el render sólo recibe el tick 2. El fixture asigna los ticks sin
ejecutar física; identifica el acoplamiento del emisor, no acredita una partida
nativa completa. `spawn_train_smoke` se ejecuta en Update y omite ticks
intermedios. OpenTTD llama `ShowVisualEffect` desde el controlador de vehículo
y `Chance16` consume el RNG de juego. En el port, CB10/CB160 también pueden
escribir los registros del vehículo. El problema ya existe al agrupar ticks
con velocidad
acelerada y se extendería a velocidad normal al retirar la cota de un tick.

Se conserva el [experimento completo](evidence/tick-cadence-standalone-experiment-20261001.json),
incluido el patch reproducible sobre `91378bd7`, resultados y alcance. No se
aplicó al producto. El siguiente diseño debe ejecutar las decisiones por cada
tick autoritativo, retener eventos visuales entre frames y conservar comandos,
RNG/registros, sincronización y frontera de snapshots de red. Cambiar sólo el
reloj o mover el emisor al FixedUpdate local no cubre los `AdvanceTicks` que el
cliente de red aplica desde Update.

El prototipo pasa Clippy; tras retirarlo, ambos archivos de producción vuelven
exactamente a `91378bd7`. Se repiten las 1.666 pruebas del cliente/dos ignoradas,
Clippy de todos sus targets, formato, diff y frescura de docs. El core no cambia
y conserva las validaciones de la etapa 22. No se midieron FPS/GPU del prototipo
ni se modifica la última cifra aceptada de 18,831 / 18,864 FPS. F07 y la cadencia
completa continúan abiertos; esta etapa cierra sólo el diagnóstico reproducible
del límite y su dependencia de efectos autoritativos.

## Etapa 24 — Leer un slot de carga por referencia (F12)

`CargoStock::get` recibe ahora `&self` y conserva su condición `const`, las
31 variantes nombradas, los 33 slots custom y el resultado cero para índices
custom fuera de rango. Antes recibía por valor los 256 bytes del stock. No
cambian mutaciones, aritmética, layouts persistidos ni selección de cargo.
Los callers del repositorio usan sintaxis de método; no se encontró un
callsite UFCS o puntero de función que requiriese el argumento por valor.
Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
`newgrf_station.cpp::StationScopeResolver::GetVariable`, consulta por cargo
con `const GoodsEntry *ge = &this->goods[cargo]`, sin copiar todos los stocks.
La equivalencia del cambio se compara con la versión anterior del port.

La sonda [`profile_cargo_stock_get.rs`](../../scripts/profile_cargo_stock_get.rs)
usa los stocks importados de las 245 estaciones y los 64 slots, 512 pasadas,
8.028.160 lecturas por muestra, cinco muestras por corrida y ABBA. Carga,
hidratación y salida quedan fuera del reloj; stock y selector atraviesan
`black_box` y el checksum se comprueba en cada muestra. Los cuatro runs
conservan `405600256`, correspondiente a 512 pasadas de 792.188 unidades.
Media por corrida: **17,9067 / 17,9036 → 5,8392 / 6,0713 ms**.
La inspección del código de esta sonda muestra que el getter anterior copia
el stock a la pila; la variante por referencia consulta el slot directamente.
El optimizador puede eliminar copias en otros contextos: no se extrapola
esa mejora aislada a cada caller ni al juego completo.
[Muestras](evidence/cargo-stock-borrow-getter-20261001.csv).

Núcleo normal, mismas partida/NewGRF y orden importado, ABBA sin otras cargas:
24 ticks, 24,227 / 24,013 → 24,248 / 23,833 ms; 120 ticks,
15,767 / 15,882 → 16,288 / 15,935 ms. **No se acredita una mejora global
del tick**. [Fases](evidence/cargo-stock-borrow-core-20261001.csv).

En 61 fases inicial/60 ticks normales se conservan hash v4, eventos,
todos los bytes de teselas/bloques 4×4, contadores visuales y la secuencia
de avisos generales y de ascensor: cero diferencias. Tick final 3.703.134,
hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Estado](evidence/cargo-stock-borrow-state-20261001.csv).

Cliente normal, mismo hardware/partida/NewGRF, 1280×720, escala 2, sin audio,
ABBA y 40 muestras por corrida, sin builds ni otras mediciones concurrentes:

- Cámara fija, warmup 120: frame 52,647 / 52,675 → 53,119 / 53,382 ms;
  FPS **18,994 / 18,984 → 18,826 / 18,733**. Simulación
  15,649 / 15,765 → 16,045 / 15,830 ms; remap
  9,929 / 9,847 → 9,786 / 9,878 ms. Mediana
  51,613 / 51,029 → 51,914 / 52,360 ms; p95
  74,629 / 74,653 → 76,126 / 77,364 ms; máximo/p99
  86,527 / 86,589 → 87,212 / 86,267 ms. El segundo run posterior
  cubre ticks 3.703.194–3.703.233 y los otros tres 3.703.193–3.703.232;
  tick/s observado 18,984 / 18,983 → 18,820 / 18,590.
  [Muestras](evidence/cargo-stock-borrow-client-steady-20261001.csv).
- Cámara en movimiento, warmup 30: frame
  60,399 / 60,049 → 60,168 / 59,196 ms; FPS
  **16,556 / 16,653 → 16,620 / 16,893**. Simulación
  16,401 / 16,404 → 16,572 / 16,403 ms; remap
  11,412 / 11,419 → 11,262 / 11,190 ms. Mediana
  56,496 / 55,980 → 56,031 / 56,896 ms; p95
  88,153 / 89,326 → 88,175 / 86,243 ms; máximo/p99
  149,818 / 145,445 → 147,595 / 138,929 ms. El segundo run posterior
  cubre ticks 3.703.104–3.703.143 y los otros tres 3.703.103–3.703.142;
  tick/s observado 17,210 / 17,283 → 17,263 / 17,498.
  [Pan](evidence/cargo-stock-borrow-pan-20261001.csv).

Los 80 frames por versión y escenario exceden 33,33 ms. Los resultados
completos son mixtos; no acreditan una mejora de FPS. Se mantiene la salvedad
del cambio de zoom con warmup 30, sin mezclar ventanas ni sumar fases.
Las seis parejas congeladas con flota, centro 128,128, settle 180, `CLEAN=0`,
1280×720 y escalas 0,25/0,5/1/2/4/8 conservan PNG y traza completa
byte-idénticos: cero píxeles/bloques 4×4 distintos, 54–37.998 parents.
[Raster](evidence/cargo-stock-borrow-raster-20261001.csv).
No se certifican por ello lectura SAV nativa, todos los frames activos
ni la resolución de la variación histórica del renderer.

Validación: 3.019 core/seis ignorados, 1.666 cliente/dos, Clippy de todos los
targets, formato, diff y frescura de docs. La sonda se formatea y compila con
`rustc -D warnings -O` contra cada `.rlib` exacta de Cargo JSON. SHA256 cliente
anterior/posterior: `b93231297b4a5611b3c5d3ee41cfebb7a7c53bbc5d28016545dab6a4f2988f58`
y `95ccbe237d695c6f4c5bb04ba4efb3e20445db1abb8dd605e576ac20bdfa827b`;
`sav_profile`: `ca4d25c8af82dbe3802ea852fa782bc3ab8cb150803ee6162db5674d90f6e62a`
y `593dd09d8a11cc4e7e08783ba1ead26dbaf1ce990da81a3094bd38077b15fc8d`.
Se cierra el argumento por valor del getter. F12, tráfico, remap, cadencia
y 30 FPS siguen abiertos.

## Trabajo restante

- F03/F21: grafos por componente, estadísticas nativas de producción y SAV
  de flows/jobs tras mutaciones; acotar/cancelar trabajo pendiente (F02).
- F04, F09–F11, F13: resolver NewGRF a demanda y corregir variables relativas.
- F05–F06: snapshots tipados de red y hash sin árbol JSON completo.
- F07–F08, F17–F18: medir tick residual, compositor y cámara con raster en seis zooms;
  aislar la variación Out4x/Out8x entre ejecuciones del mismo cliente con flota.
  Revisar el límite de un tick por frame y conservar la velocidad de juego.
  Ejecutar decisiones/RNG de efectos de vehículo por tick autoritativo antes
  de agrupar pasos, con eventos retenidos y publicación coherente para red.
  F17/F28: conservar animaciones NewGRF y cambios del edificio al reducir
  reconstrucciones adicionales de chunks.
- F12, F14–F16: perfilar carga, ocupación y rutas; retirar copias PBS innecesarias.
- Resolución de depósitos: identidad/tipo, propiedad y alcanzabilidad frente
  al original; invalidación completa de los índices persistentes.
- F19–F20, F22: encoding/carga/generación sin bloquear el cliente.
- F23–F25, F27–F29: caches/invalidación de imágenes, HUD, IA, audio y previews;
  preservar los comportamientos que la revisión ya verificó correctos.
- F26: documentar el alcance de IA/GS; una VM Squirrel completa requiere un
  proyecto de compatibilidad propio y no es una optimización del hot path.
- F30–F32: pruebas diferenciales, métricas por fase y límites claros de publicación.

No se declara alcanzado el presupuesto de 33,33 ms sin una nueva medición de la
partida grande en ejecución, cámara y los seis niveles de zoom.

## Etapa 25 — Cabezas ferroviarias actuales una vez por consulta (F07)

El look-ahead de tráfico y el predicado de frente a frente filtran ahora las
cabezas de tren una vez por consulta inmutable. Las búsquedas siguientes usan
referencias en el mismo orden del vector de vehículos; no se ordenan por ID ni
se conserva una posición entre consultas o ticks. Los guardas, los depósitos,
las huellas y la selección del primer tren sobre una tesela permanecen iguales.
En la fixture hay 3.293 unidades y 151 cabezas ferroviarias: el look-ahead de
hasta 64 teselas consulta esas cabezas en lugar de filtrar repetidamente toda
la flota. El wrapper público sigue construyendo su índice cuando se usa de
forma aislada; la ruta del tick reutiliza el índice de formaciones existente.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
[`train_cmd.cpp::CheckTrainCollision`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/train_cmd.cpp#L3205),
usa `VehiclesOnTile` / `VehiclesNearTileXY` y referencias a unidades actuales.
Esa consulta espacial orienta la reducción de recorridos; **no certifica la
regla simplificada de 64 teselas del port como equivalente a la colisión
física nativa**, que también considera compañía, coordenadas y altura.
La equivalencia de esta etapa usa los dos predicados anteriores de `aee9238a`
como oracle diferencial. Cinco regresiones ejecutan 250 escenarios: flota
mixta, wagons, límites 2/64, guardas, empates por orden, IDs duplicados,
altas/bajas/reordenamiento, movimiento, depósito, bifurcación y huella mediante
eslabones o historial. Los dos resultados coinciden en cada escenario.

Núcleo normal, misma partida/NewGRF y orden importado, ABBA sin otras cargas:
24 ticks, movimiento de vehículos **3,3288 / 3,5040 → 2,4019 / 2,6026 ms**;
total **23,7651 / 24,4710 → 24,6287 / 25,7748 ms**. En 120 ticks, movimiento
**4,0874 / 4,0612 → 3,1131 / 3,1371 ms** y total
**15,8584 / 15,7824 → 15,3016 / 15,2544 ms**. El ahorro de movimiento aparece
en ambas ventanas; el total inicial empeora y no se oculta ni se atribuye
una mejora global uniforme al núcleo.
[Datos](evidence/train-traffic-view-core-20261001.csv).

Las 61 fases normales conservan hash canónico v4, eventos, teselas raw y
bloques 4×4. También coinciden los contadores y las listas ordenadas de avisos
generales y de ascensor. El oracle anterior reutiliza el dump inmutable de la
etapa 24 posterior, generado por el mismo binario de traza cuyo hash se indica
abajo; no se ordenan las listas compartidas antes de avanzar.
Tick final `3703134`, hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Comparación](evidence/train-traffic-view-state-20261001.csv).
Esto acredita conservación frente al port anterior, dentro de esta fixture;
no acredita importación SAV o simulación nativa universales.

Cliente/GPU real, 1280×720, misma flota/NewGRF, escala 2, ABBA, 40 muestras
por run, sin compilación ni perf concurrentes. Ventana fija tras 120 frames:
frame medio **53,3029 / 53,1672 → 51,2810 / 51,1498 ms**, FPS por duración
media **18,761 / 18,809 → 19,500 / 19,550**; simulación
**15,7480 / 16,0070 → 14,4643 / 14,4341 ms**. Remap permanece alrededor de
9,8 ms. Medianas **52,4621 / 52,1645 → 50,1609 / 50,0222 ms**, p95
**75,6472 / 76,7182 → 72,8220 / 73,6081**, máximos/p99
**86,2844 / 87,2434 → 83,9550 / 85,2077**. TPS observados
**18,630 / 18,817 → 19,488 / 19,544**. El primer run anterior cubre ticks
3703194–3703233; los otros tres, 3703193–3703232. Exceden 33,33 ms 80/80
frames anteriores y 79/80 posteriores; un frame aislado bajo el presupuesto
no acredita 30 FPS sostenidos.
[160 muestras](evidence/train-traffic-view-client-steady-20261001.csv).

Pan con warmup 30, otra comparación ABBA de 40 muestras por run:
frame **60,4526 / 60,2850 → 58,9708 / 59,7966 ms**, FPS
**16,542 / 16,588 → 16,958 / 16,723**; simulación
**16,5981 / 16,4873 → 15,2031 / 15,2946 ms**. Medianas
**56,6379 / 55,5866 → 55,1062 / 56,6691**, p95
**89,0991 / 89,2055 → 87,7315 / 89,4415**, máximos/p99
**151,0657 / 150,7162 → 145,0007 / 146,3740**. TPS
**17,203 / 17,251 → 17,617 / 17,368**. Todos los runs cubren ticks
3703103–3703142, todos los 80 frames por versión exceden el presupuesto y
la primera muestra incluye el cambio de zoom del frame 30. Las fases miden
el frame actual y `frame_ms` el intervalo anterior: no se suman como un
presupuesto exacto del mismo frame.
[160 muestras](evidence/train-traffic-view-pan-20261001.csv).

Las seis parejas congeladas conservan PNG y trazas completas de sort
byte-idénticos, cero píxeles y cero bloques 4×4 distintos, de 54 a 37.998
parents. Escalas ortográficas .25/.5/1/2/4/8, centro 128,128, settle 180 y
CLEAN=0. Ese gate no acredita los FPS activos de los otros cinco zooms ni
cada frame animado.
[Raster](evidence/train-traffic-view-raster-20261001.csv).

Artefactos inmutables en `target/performance/train-traffic-view-20261001`:
cliente anterior/posterior SHA256
`95ccbe237d695c6f4c5bb04ba4efb3e20445db1abb8dd605e576ac20bdfa827b` /
`a759a41d2d03e16c2a2c27ad6c95f0793072b56ab8a4cb7a908ee28d2135ce25`;
`sav_profile` anterior/posterior
`593dd09d8a11cc4e7e08783ba1ead26dbaf1ce990da81a3094bd38077b15fc8d` /
`4b026773977296a92096b9bd8f977492b7139b11258ebfd59dcf14f0e75caf31`;
traza anterior/posterior
`19282da1d0dbade93200d44fa942f44c774b1f8835ff6299d4d9f871d5286751` /
`4b78cd3bfae415dadeb0b9982baab1bcfdfcb3d7446dc6225b5941c0068f66a6`.
La sonda posterior se compiló contra la `.rlib` exacta reportada por Cargo
JSON. Release del perfil: 24,68 s; cliente tras cambio core: 77 s. No son una
comparación controlada de compilación ni una mejora de ese objetivo.

Validación: 3.024 core/seis ignorados, 1.666 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Sólo cierra este sub-issue
de recorridos repetidos de tráfico; F07, paridad ferroviaria completa,
cadencia, variación histórica del renderer y 30 FPS permanecen abiertos.

## Etapa 26 — Hash de visitados en huellas de estación (F17/F31)

`station_footprint_tiles` usa ahora `AHashSet`, ya presente en el núcleo, para
comprobar las coordenadas visitadas. No itera ese conjunto: el vector de
salida sigue el mismo BFS, dirección, guardia de 64 y expansión del último
nodo. No añade una caché ni cambia la regla legacy de estaciones adyacentes.
Dos regresiones conservan la secuencia completa en 2.066 escenarios: las 512
topologías 3×3 con cuatro anclas, límite de expansión, anclas fuera del mapa,
tipos no Station, ediciones posteriores y mapa clonado. La expectativa de
orden explícita usa `map/slope.rs::diag_dir_offset`: oeste, sur, este, norte.
Oracle funcional: el recorrido anterior con `std::HashSet` de `b023f1fb`.

El original OpenTTD 15.3 `14ec60f2` resuelve el identificador de estación en
[`station_map.h::GetStationIndex`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/station_map.h#L28)
mediante MAP2. El BFS del port es un fallback legacy; acelerar su conjunto
no lo convierte en esa consulta nativa ni cierra la geometría ferroviaria.
La caché de terminales existente mantiene un rebuild y un barrido completo
en los 120 ticks de esta fixture. Las consultas que siguen usando el recorrido
legacy merecen reutilización independiente, con su mismo contrato.

Perfil del cliente `b023f1fb`, GPU real, activado por FIFO de perf después de
120 frames de calentamiento y de que se publicaran 60 muestras. Evento
`cpu-clock:u`, 199 Hz, `dwarf,8192`; 3.867 muestras, intervalo de muestras
19425,264211–19439,841908 s. El controlador recibe `ack\n\0`; el primer intento
rechazaba ese NUL y fue descartado, conservando sus artefactos. Las cifras de
frame de esta captura son diagnósticas bajo perf, no una comparación ABBA.
Los símbolos leaf incluyen vidrio 4,50 %, huellas 1,14 %, sort y operaciones
ECS. Sólo 71 muestras tienen algún frame decodificado y 27 más de uno:
**no se usa este perfil para atribución acumulada a callers**, ni se atribuye
a un subsistema cada monomorfización de hash que aparece en la tabla.
[42 filas self ≥0,5 %](evidence/client-residual-self-20261001.csv),
[360 frames diagnósticos](evidence/client-residual-profiled-frames-20261001.csv),
[alcance, hashes e intervalo](evidence/client-residual-scope-20261001.json).

La sonda [`profile_station_footprints.rs`](../../scripts/profile_station_footprints.rs)
recorre las 78 anclas rail/waypoint importadas, 1.254 teselas por pasada,
128 pasadas y 9.984 consultas por muestra. Cinco muestras por run, ABBA;
importación, hidratación y salida fuera del reloj. El checksum de longitudes,
coordenadas y orden permanece `12612797840475784741` en las veinte muestras.
Media por run **9,2999 / 9,3205 → 5,1104 / 5,1569 ms**. Es un coste aislado,
no una estimación de ahorro por tick.
[Datos](evidence/station-footprint-hash-probe-20261001.csv).

Núcleo normal, mismas partida/NewGRF y listas importadas, ABBA sin otras
cargas: 24 ticks **24,5503 / 24,5601 → 22,4886 / 23,2078 ms**; 120 ticks
**15,1610 / 15,1570 → 14,1356 / 14,2266 ms**. Las 61 fases normales mantienen
hash v4, eventos, todas las teselas raw, bloques 4×4, contadores visuales y
listas ordenadas de avisos generales/de ascensor. Se reutiliza la traza
posterior inmutable de la etapa 25 como oracle anterior; no se ordenan listas
compartidas antes de avanzar. Tick final 3703134, hash `4d3524e48b6bc2e5`,
eventos `4857641856429973`.
[Core](evidence/station-footprint-hash-core-20261001.csv),
[estado](evidence/station-footprint-hash-state-20261001.csv).

Cliente sin perf ni compilación concurrentes, GPU real, 1280×720, escala 2,
ABBA, 40 muestras por run. Cámara fija, warmup 120: frame
**51,8772 / 51,4945 → 46,9528 / 47,5605 ms**, FPS por duración media
**19,276 / 19,420 → 21,298 / 21,026**; remap
**9,9803 / 9,8932 → 5,7612 / 5,8041 ms** y simulación
**14,5180 / 14,6803 → 14,1245 / 14,2843 ms**. Medianas
**49,5511 / 50,5736 → 46,2374 / 47,2536**, p95
**74,9694 / 73,1600 → 63,4774 / 64,9008**, máximos/p99
**85,4398 / 83,9247 → 69,6296 / 70,3332**. TPS
**19,128 / 19,422 → 21,294 / 21,035**. El primer run anterior cubre ticks
3703194–3703233; los otros tres, 3703193–3703232. Exceden 33,33 ms 79/80
frames de cada versión; las excepciones aisladas no acreditan 30 FPS sostenidos.
[160 muestras](evidence/station-footprint-hash-client-steady-20261001.csv).

Pan, warmup 30, otra ventana ABBA: frame
**57,9298 / 58,9172 → 53,3477 / 53,4758 ms**, FPS
**17,262 / 16,973 → 18,745 / 18,700**; remap
**11,1499 / 11,4369 → 6,6861 / 6,7169 ms**. Medianas
**55,2001 / 54,9613 → 49,6675 / 49,8933**, p95
**82,7533 / 87,5052 → 73,6594 / 73,7580**, máximos/p99
**139,1268 / 146,6285 → 137,6965 / 136,5556**. TPS
**17,906 / 17,647 → 19,537 / 19,476**. El primer run anterior cubre ticks
3703104–3703143; los demás, 3703103–3703142. Todos los 80 frames por versión
exceden el presupuesto. El primer intervalo incluye el cambio de zoom del
frame 30; los timers de fase y de intervalo pertenecen a frames distintos.
[160 muestras](evidence/station-footprint-hash-pan-20261001.csv).

Las seis parejas congeladas conservan PNG y traza completa de sort
byte-idénticos, cero píxeles/bloques 4×4 distintos, escalas .25/.5/1/2/4/8,
centro 128,128, settle 180 y CLEAN=0. Conservan de 54 a 37.998 parents.
El gate no acredita todos los frames animados ni los FPS activos de cada zoom.
[Raster](evidence/station-footprint-hash-raster-20261001.csv).

Artefactos en `target/performance/station-footprint-hash-20261001`.
Cliente anterior/posterior SHA256
`a759a41d2d03e16c2a2c27ad6c95f0793072b56ab8a4cb7a908ee28d2135ce25` /
`b8c09204241281915a0d13872af6c740fc8b017e73019f68895eeb37ca5cab5d`;
`sav_profile`
`4b026773977296a92096b9bd8f977492b7139b11258ebfd59dcf14f0e75caf31` /
`075c0ff9acdfc6ba7cbe6e732ec43abf2ae90b13bb4f10c0262c35e0aa3ae838`;
traza `4b78cd3bfae415dadeb0b9982baab1bcfdfcb3d7446dc6225b5941c0068f66a6` /
`f805baba1d90dfc2eaffe3e45613539a7fe756082bca4226db2298ee31b52913`.
Sondas compiladas contra las `.rlib` exactas/copiadas del reporte Cargo JSON;
`rustc -O -D warnings` y formato standalone pasan. Release del perfil 24,19 s,
cliente tras cambio core 77 s; no es una mejora controlada de compilación.
Tras comparar bytes, los dumps idénticos de fases 24–26 comparten hardlinks
readonly: se conservan todos los contenidos y se recuperan 1.431.032.577 bytes.
No se enlazan archivos de trabajo de Cargo como oracles inmutables.

Validación: 3.026 core/seis ignorados, 1.666 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Sólo cierra este coste de
visitados; reutilización de consultas legacy, F17/F31 completos, cadencia,
variación histórica del renderer y 30 FPS permanecen abiertos.

## Etapa 27 — Reutilizar huellas vigentes al dibujar estaciones (F17/F31)

`station_at_tile_indexed` conserva el contrato de `station_at_tile`: primero
ancla/joined/aeropuerto en orden del vector vivo; después candidatos rail o
waypoint por huella conectada y distancia Manhattan, con el mismo desempate.
Sólo toma prestadas las huellas legacy de `TerminalSpatialIndex` cuando la
época/revisión terminal corresponde al mapa actual. Un ancla nueva o un índice
viejo ejecutan inmediatamente el recorrido anterior. No usa `at()` como
propietario: ese índice incluye MAP2 nativo y no equivale al fallback legacy.
Las estaciones y sus roles se leen de la lista actual, incluso si se reordenó.

El cliente conserva una copia propia del índice dentro de la caché de sprites
de estación, sin copiarlo por tile ni por tick. Cada tanda de chunks comprueba
el vínculo con el mapa; copia el índice de simulación sólo si está vigente y
la copia anterior dejó de corresponder al mapa. Una carga, demolición o cambio
de MAP2 invalida ese vínculo. Si simulación todavía no lo actualizó, el render
usa el recorrido vivo. La clasificación, selección de spec, cimientos y layout
usan esta consulta; los contextos Action2 internos siguen siendo una mejora
pendiente. No se retiene una estación ni el resultado gráfico entre ticks.

Tres regresiones comparan las referencias exactas elegidas con la función
anterior independiente de `56f252fe`: 25.088 consultas sobre las 512 topologías
3×3, 495 con listas/roles/anclas cambiados y 3.456 con huellas limitadas,
demolición, MAP2 editado, mapa clonado y reemplazado; 29.039 en total. Incluyen
prioridad de cobertura directa, empate por orden, IDs/anclas duplicados y ancla
no cacheada. Otra regresión del cliente cubre índice vacío, copia retenida,
invalidez previa al refresco, nueva época y limpieza de recursos. Esta prueba
conserva el port anterior; no certifica la propiedad nativa ni la importación
SAV. El original obtiene el propietario de MAP2, como registra la etapa 26.

La sonda [`profile_station_tile_lookup.rs`](../../scripts/profile_station_tile_lookup.rs)
consulta los 1.807 tiles Station de Kale con las 245 estaciones importadas,
cuatro pasadas y 7.228 consultas por muestra. Compara también la referencia
exacta de cada estación antes del reloj. Dos runs legacy/indexed en ABBA,
cinco muestras por run, misma biblioteca posterior: media por run
**205,8552 / 206,2285 → 8,5987 / 8,6425 ms**, checksum
`7373120780294979429` en las veinte muestras. Construir el índice una vez
cuesta 0,1992–0,2207 ms; copiarlo, 0,0297–0,0309 ms de media sobre veinte
copias por proceso. La importación, hidratación, validación, construcción,
copia y salida quedan fuera de los samples. Es una sonda de consultas, no
una predicción directa del FPS.
[Datos](evidence/station-render-footprints-lookup-20261001.csv).

Núcleo normal, NewGRF activo, ABBA: 24 ticks
**22,7034 / 24,0315 → 22,8531 / 22,8020 ms**; 120 ticks
**14,4202 / 14,3921 → 14,4582 / 14,1466 ms**. No hay una mejora uniforme del
núcleo; el nuevo caller de caché es del cliente. Las 61 fases normales
conservan hash v4, eventos, teselas raw, bloques 4×4, contadores y listas
ordenadas de avisos. Se toma como oracle la traza posterior inmutable de la
etapa 26; las listas importadas siguen su orden original. Tick final 3703134,
hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Core](evidence/station-render-footprints-core-20261001.csv),
[estado](evidence/station-render-footprints-state-20261001.csv).

Cliente sin perf/compilación concurrentes, GPU real, 1280×720, escala 2, ABBA,
40 muestras por run. Cámara fija, warmup 120: frame
**47,4186 / 46,9689 → 43,9334 / 44,5389 ms**, FPS
**21,089 / 21,291 → 22,762 / 22,452**; remap
**5,7630 / 5,7871 → 2,5245 / 2,5372 ms** y simulación
**14,1166 / 13,9399 → 14,1079 / 14,3681 ms**. Medianas
**47,1591 / 46,3669 → 42,8751 / 44,2462**, p95
**63,6427 / 64,3315 → 58,4180 / 57,5051**, máximos/p99
**70,7034 / 71,4308 → 59,6653 / 59,8982**. TPS
**21,095 / 21,287 → 22,761 / 22,446**. Todos los runs cubren ticks
3703193–3703232; exceden 33,33 ms 79/80 frames anteriores y 80/80 posteriores.
[160 muestras](evidence/station-render-footprints-client-steady-20261001.csv).

Pan, warmup 30, otra ventana ABBA: frame
**53,9393 / 54,9979 → 49,8936 / 49,7865 ms**, FPS
**18,539 / 18,183 → 20,043 / 20,086**; remap
**6,7467 / 6,6994 → 2,9705 / 2,9454 ms**. Medianas
**50,1284 / 52,3085 → 46,7959 / 46,7170**, p95
**74,4675 / 74,2855 → 62,5472 / 63,1213**, máximos/p99
**137,7050 / 136,8933 → 131,7458 / 131,3966**. TPS
**19,308 / 18,904 → 20,923 / 20,967**. Los cuatro runs cubren ticks
3703103–3703142 y los 80 frames por versión exceden el presupuesto. El primer
intervalo incluye el zoom del frame 30. Los timers de fase y de intervalo
corresponden a frames distintos; no se suman como presupuesto exacto.
[160 muestras](evidence/station-render-footprints-pan-20261001.csv).

Las seis parejas congeladas conservan PNG y traza completa de sort
byte-idénticos, cero píxeles/bloques 4×4 distintos, escalas .25/.5/1/2/4/8,
centro 128,128, settle 180 y CLEAN=0. Conservan de 54 a 37.998 parents.
La captura pausada permite fallback si el índice todavía no está vinculado;
la sonda de la partida real verifica las referencias elegidas por el índice
vigente en los 1.807 tiles. Este gate no acredita todos los frames animados
ni los FPS activos de cada zoom.
[Raster](evidence/station-render-footprints-raster-20261001.csv).

Artefactos en `target/performance/station-render-footprints-20261001`.
Cliente anterior/posterior SHA256
`b8c09204241281915a0d13872af6c740fc8b017e73019f68895eeb37ca5cab5d` /
`479772feda3342ee6fd09dcce0664a683d1a5c72e5c5c1f05598b59acab8e7fc`;
`sav_profile`
`075c0ff9acdfc6ba7cbe6e732ec43abf2ae90b13bb4f10c0262c35e0aa3ae838` /
`a6c1faf2ded7de2544e251ccab40a7e1a25f6228d654782822aae55f62ddc1c9`;
traza `f805baba1d90dfc2eaffe3e45613539a7fe756082bca4226db2298ee31b52913` /
`b084788ca88aaee7ab8133bbd6e00de9659a3071dbc0b06ce5ce84130b836278`.
Sonda `321e302cf88a371648d607b34c99608f9a021e326138a24d769258c91f4f7dec`
compilada con `rustc -O -D warnings` contra la biblioteca posterior copiada
del artefacto exacto de Cargo JSON,
`a45f548ceec685e32b3520cd67a20f727594dff23510fc4d87323bde2ee74eb9`.
Release del perfil 24,44 s, cliente tras cambio core 77 s; no es una mejora
controlada de compilación. Las 183 trazas de fase idénticas se conservan en
hardlinks readonly verificados; se recuperan 477.010.859 bytes.

Validación: 3.029 core/seis ignorados, 1.667 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Sólo cierra la reutilización
para estos callers de render. Consultas de Action2 y otros callers legacy,
F17/F31 completos, cadencia, variación histórica del renderer y 30 FPS siguen
abiertos. El ahorro reduce el frame a unos 44 ms; todavía supera 33,33 ms.

## Etapa 28 — Compartir geometría dentro de un contexto de estación (F04/F17/F31)

Cada construcción Action2 de una tesela tiene un `StationGeometryLookup`
efímero sobre las referencias inmutables de mapa y estaciones. Guarda la
referencia elegida por `station_at_tile`, incluido el resultado inexistente,
sólo durante esa llamada. El contexto base, las cuatro variantes de andén,
parent, badges y vecinos comparten las consultas; al terminar se descarta el
memo. Las APIs públicas, filtros de eje/rol, orden de desempate, parámetros,
valores de variables y evaluación de callbacks conservan su contrato anterior.
No retiene un contexto ni sus resultados entre ticks y no cambia el RNG.

El contador de misses sólo existe en tests. Una regresión ejerce 735
consultas repetidas sobre cinco topologías con cobertura directa y resultados
negativos: cada una resuelve 49 tiles únicos una sola vez. La otra construcción
real de las cuatro variables de andén resuelve sólo ocho tiles, mantiene sus
valores packed y vuelve a utilizar esos ocho ante otra evaluación inmutable.
Después de descartar el memo, demolición y traslado del ancla cambian el
contexto como corresponde; los bits aleatorios nuevos son observables.

Oracle de fuente: OpenTTD 15.3 `14ec60f2`,
[`StationScopeResolver::GetVariable`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_station.cpp#L307)
guarda sus variables de andén por resolver;
[`FindRailStationEnd`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/newgrf_station.cpp#L158)
usa MAP2, spec por tile y eje. Aquí sólo se reutiliza la propiedad legacy ya
calculada. La variable 0x47 del port todavía tiene el mismo filtro que 0x46,
porque el modelo conserva una spec lógica; se mantiene esa limitación y no se
presenta esta optimización como paridad nativa de identidad o spec por tile.


Sonda completa de la etapa 28:
[`profile_station_action2.rs`](../../scripts/profile_station_action2.rs)
construye contextos para los 1.807 tiles Station de Kale: versiones GRF 7/8,
con y sin mundo, 7.228 contextos por muestra. Desestructura los 24 campos
sin `..`, ordena las trece tablas y compara el contenido completo fuera del
reloj; incluye parámetros, parent, registros, cargo, capacidades, RNG y
resultados. Importación, hidratación, validación, canonicalización y destrucción
quedan fuera del intervalo. ABBA, cinco muestras por run: media
**6.945,8068 / 6.985,9254 → 1.284,5442 / 1.273,4757 ms**, aproximadamente
5,4 veces más rápido en esta sonda. Las veinte muestras conservan checksum
`9150983015706041655`; las 7.228 líneas completas anteriores/posteriores son
byte-idénticas, SHA256
`cc5f3b5f8ca623d27d341d4ebe753c26465c9c9250e7c2d2c4cc0755e61b6faf`.
Este resultado aislado no equivale a acelerar un frame ni certifica otros
NewGRF. [20 muestras](evidence/station-action2-memo-context-20261001.csv).

Núcleo normal, NewGRF activo, ABBA: 24 ticks
**22,3931 / 22,5590 → 22,5901 / 22,3480 ms**; 120 ticks
**14,1804 / 14,2396 → 14,3299 / 14,3551 ms**. La ventana larga empeora
ligeramente; no se acredita una mejora global del tick. Las 61 fases normales
mantienen hash v4, eventos, bytes raw, bloques 4×4, contadores y listas
ordenadas de avisos frente al artefacto posterior inmutable de la etapa 27.
Tick final 3703134, hash `4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Core](evidence/station-action2-memo-core-20261001.csv),
[estado](evidence/station-action2-memo-state-20261001.csv).

Cliente en GPU real, sin perf ni compilación concurrentes, 1280×720, escala 2,
ABBA y 40 muestras por run. Cámara fija, warmup 120: frame
**44,1289 / 44,0606 → 43,9773 / 44,1791 ms**, FPS
**22,661 / 22,696 → 22,739 / 22,635**; remap
**2,5887 / 2,6540 → 2,5772 / 2,5855 ms**, simulación
**14,1842 / 14,2145 → 14,0785 / 14,1136 ms**. p95
**57,4776 / 58,6366 → 58,0820 / 58,3119**, máximos/p99
**60,6950 / 61,2234 → 58,6952 / 59,5101**. TPS
**22,652 / 22,686 → 22,737 / 22,627**. Los cuatro runs cubren ticks
3703193–3703232; exceden 33,33 ms 78/80 frames anteriores y 79/80 posteriores.
Los FPS permanecen esencialmente iguales.
[160 muestras](evidence/station-action2-memo-client-steady-20261001.csv).

Pan, warmup 30: frame
**49,5371 / 49,8196 → 48,9556 / 49,7172 ms**, FPS
**20,187 / 20,072 → 20,427 / 20,114**; remap
**2,8801 / 2,9264 → 2,8868 / 2,9979 ms**. p95
**61,9962 / 63,8913 → 60,7752 / 61,9070**, máximos/p99
**132,1221 / 130,4160 → 128,6777 / 130,2264**. TPS
**21,088 / 20,941 → 21,317 / 20,985**. El primer run posterior cubre ticks
3703104–3703143; los otros tres, 3703103–3703142. Todos los 80 frames por versión
exceden el presupuesto. El primer intervalo incluye el zoom del frame 30;
los timers de fase y de intervalo corresponden a frames distintos.
[160 muestras](evidence/station-action2-memo-pan-20261001.csv).

Raster congelado, centro 128,128, settle 180, CLEAN=0 y escalas .25/.5/1/2/4/8:
cinco parejas iniciales son PNG exactos. En In2x (.5), el binario anterior
produce inicialmente **204 píxeles / 45 bloques 4×4 distintos** respecto del
posterior canónico, bbox (2,0)–(741,363), con alpha 255. Se conserva ese fallo,
SHA256 anterior
`4e1118f0448939a1ff06af6b151142402a5880541664707ffef680e10be85d54`;
no se amplía la tolerancia ni se atribuye una causa que todavía no se conoce.
[Seis parejas iniciales](evidence/station-action2-memo-raster-20261001.csv).

Se repiten In2x y Out4x tres veces por binario: **las doce capturas nuevas**
coinciden exactamente con sus PNG canónicos, incluidos los seis In2x.
El mismo binario anterior presenta así la variación inicial y luego resultados
exactos; no se reproduce una regresión propia del cambio, pero la variación
del renderer queda abierta. Las trazas raw difieren en algunas capturas.
Se verifican los 18 streams completos (seis anteriores iniciales y doce
repetidos) contra el posterior inicial correspondiente: son exactos tras una
renumeración biyectiva y consistente de entidades. Sólo cambian
`parents.entity`, `local_proxies.original_parent` y `source_child`; se conservan
orden, índices, profundidades, alias, referencias y todos los demás campos.
Esta igualdad del stream de sort no certifica las otras capas raster ni
resuelve los 204 píxeles iniciales.
[Repeticiones](evidence/station-action2-memo-raster-repeats-20261001.csv),
[identidad completa de trazas](evidence/station-action2-memo-trace-identity-20261001.csv).

Artefactos en `target/performance/station-action2-memo-20261001`.
Cliente anterior/posterior SHA256
`479772feda3342ee6fd09dcce0664a683d1a5c72e5c5c1f05598b59acab8e7fc` /
`1ab8dfc2fcf27d3a94d1def6ab60dba9ef2c035e2b33894d4f20cdd9559bb259`;
`sav_profile`
`a6c1faf2ded7de2544e251ccab40a7e1a25f6228d654782822aae55f62ddc1c9` /
`77dfedc8b88eb0e5588361146368fe73383a32e0154baf8a9b8a48bdd5541516`;
traza `b084788ca88aaee7ab8133bbd6e00de9659a3071dbc0b06ce5ce84130b836278` /
`b8d76f2c9738ab7f09d1cfd6df9d9c2603f4bfc8b4e68816f6f42e9f175c2917`.
Las dos sondas se compilan con `rustc -O -D warnings` contra las `.rlib`
exactas copiadas desde Cargo JSON; biblioteca posterior
`ba1255b40f492ab8bd43d39b966b8f0b7ae7593e06ccc2386049848677de016f`.
Release del perfil 25,80 s y cliente tras cambio core 77 s; no es una mejora
controlada de compilación. Las 183 trazas de fase idénticas quedan en hardlinks
readonly verificados y conservan sus bytes; se recuperan 477.010.859 bytes.

Validación: 3.031 core/seis ignorados, 1.667 cliente/dos; Clippy de todos los
targets de ambos, formato, diff y frescura de docs. Sólo cierra las consultas
repetidas dentro de esta construcción inmutable. F04/F17/F31 completos,
identidad/spec nativas, variación In2x/Out4x/Out8x, cadencia y 30 FPS siguen
abiertos. La mejora aislada no reduce de forma apreciable el frame de la
partida medida: todavía ronda 44 ms.


## Etapa 29 — Reservas ferroviarias sin expulsar rutas viales/navales (F15)

La revisión efímera de navegación excluye únicamente una modificación aislada
del bit PBS de `m6` en una tesela Station de tipo Rail (0) o RailWaypoint (7).
La igualdad completa de la tesela con ese único cambio deshecho obliga a que
tipo, propietario, MAP2, eje/gfx, altura y todos los otros campos sean iguales.
El byte original se conserva; `mutation_revision` sigue avanzando y la caché
ferroviaria sigue venciendo ante esa mutación dentro del mismo tick, además
de vencer al cambiar de tick. Road/Tram/Water pueden conservar sus resultados,
positivos o negativos; no se aumenta capacidad ni se cambia búsqueda o coste.

El oracle de fuente OpenTTD 15.3 `14ec60f2`,
[`HasStationRail`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/station_map.h#L135)
delimita estaciones/waypoints ferroviarios y
[`SetRailStationReservation`](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/station_map.h#L571)
escribe sólo el bit 2 de `m6`. La
[caché de segmentos YAPF](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/pathfinder/yapf/yapf_costcache.hpp#L48)
separa los avisos de layout rail. Esto sustenta distinguir el dominio ferroviario;
el cambio aquí conserva el contrato previo del pathfinder del port, sin
certificar rutas idénticas a todos los costes/búsquedas del original.

Cuatro regresiones cubren los 256 valores de `m6`, 192 cambios simultáneos
en los otros doce campos y 30 cambios de subtipo, 478 mutaciones en total.
Verifican revisión de navegación, revisión general, vínculo terminal y bytes
almacenados. Otras 192 comparaciones cached/live ejercen Road/Tram/Water,
perfil naval, resultados sin camino, ocho flips por tipo, cruce de tick,
demolición de estación y eliminación de una calle/agua. El tren conserva la
invalidación en el mismo tick, incluido un andén reservado sobre la ruta.
Las pruebas anteriores de túneles y clones de mapa siguen aplicándose.


El perfil de la etapa 29 usa binarios anteriores/posteriores independientes,
NewGRF activo y ABBA. 24 ticks: **22,3802 / 22,4731 → 22,3423 / 22,1807 ms**;
120 ticks: **14,3521 / 14,4793 → 14,0548 / 14,1403 ms**. A 24 ticks bajan
las invalidaciones 14 → 11, sin cambiar hits (23), misses (1.826) ni búsquedas
(1.616). A 120 ticks bajan **83 → 59** invalidaciones, hits **42 → 59**,
misses **2.213 → 2.196**, búsquedas **2.001 → 1.984**; no hay hits negativos.
Se evitan 17 búsquedas en esa ventana. La mejora del núcleo es modesta y no
acredita por sí sola 30 FPS.
[Ocho perfiles con métricas de rutas](evidence/station-reservation-routes-core-20261001.csv).

Las 61 fases normales conservan hash v4, eventos, bytes raw, bloques 4×4,
contadores y listas ordenadas de avisos frente al posterior inmutable de la
etapa 28. Las revisiones y estadísticas derivadas de rutas pueden diferir;
no forman parte del estado persistido. Tick final 3703134, hash
`4d3524e48b6bc2e5`, eventos `4857641856429973`.
[Estado](evidence/station-reservation-routes-state-20261001.csv).

GPU real, 1280×720, escala 2, sin perf ni compilación concurrentes, ABBA de
40 muestras por run. Cámara fija, warmup 120: frame
**43,9237 / 43,7787 → 43,8533 / 44,0570 ms**, FPS
**22,767 / 22,842 → 22,803 / 22,698**; remap
**2,5070 / 2,5167 → 2,5128 / 2,5284 ms**, simulación
**14,1539 / 14,0988 → 14,0350 / 14,2231 ms**. p95
**58,1476 / 58,0881 → 56,9925 / 56,8958**, máximos/p99
**61,1655 / 58,7326 → 58,6812 / 58,7688**. TPS
**22,745 / 22,852 → 22,793 / 22,694**. Todos los runs cubren ticks
3703193–3703232; 79/80 frames por versión exceden 33,33 ms. Los resultados
no muestran una ganancia clara de FPS.
[160 muestras](evidence/station-reservation-routes-client-steady-20261001.csv).

Pan, warmup 30: frame
**49,4931 / 49,6340 → 49,7770 / 49,6368 ms**, FPS
**20,205 / 20,147 → 20,090 / 20,146**; remap
**2,9179 / 2,9239 → 2,9545 / 3,0034 ms**. p95
**62,8669 / 63,0356 → 61,5024 / 63,2219**, máximos/p99
**129,1138 / 134,3986 → 132,8415 / 131,6295**. TPS
**21,074 / 21,070 → 20,988 / 21,037**. Todos los runs cubren ticks
3703103–3703142 y 80/80 frames por versión exceden el presupuesto. El primer
intervalo incluye el zoom del frame 30; los timers de fase y de intervalo
pertenecen a frames distintos. Hay 46.495–46.681 sprites y otras tantas
copias de oclusión en la ventana fija; durante pan, 45.805–57.455 por clase.
[160 muestras](evidence/station-reservation-routes-pan-20261001.csv).

Raster congelado, seis escalas .25/.5/1/2/4/8, centro 128,128, settle 180,
CLEAN=0: cuatro parejas iniciales son PNG exactos. **In2x difiere 204 píxeles
y 45 bloques 4×4**, esta vez en el posterior: SHA256
`4e1118f0448939a1ff06af6b151142402a5880541664707ffef680e10be85d54` frente
al canónico `60da415eca73a715a5fdd4eb380dad119d1765398913f092e28932f151ab7252`.
Reproduce el mismo PNG alternativo que apareció en el anterior de la etapa 28.
**Out2x difiere un píxel y un bloque** en el anterior, SHA256
`55d5cf0b10fe837912a3083c152c429908358f77dd43a2250fa8ed1a0fea9e05`;
posterior canónico
`7e8cb274e2cbb8144245721d25bf1f6464bc4a0c6305f2c2246a681fef6d9b53`.
Las seis trazas completas coinciden tras renumerar biyectivamente las entidades,
sin eliminar índices, profundidades, orden ni otro campo.
[Seis parejas iniciales](evidence/station-reservation-routes-raster-20261001.csv).

Tres repeticiones por binario en In2x y Out2x dan **doce PNG canónicos exactos**
y doce trazas completas equivalentes con ese mismo mapeo consistente de IDs.
Se preservan los fallos iniciales y no se amplía tolerancia. La variación entre
ejecuciones queda abierta en In2x/Out2x, además de los antecedentes Out4x/Out8x;
no se certifican todas las capas raster a partir del stream de sort.
[Repeticiones](evidence/station-reservation-routes-raster-repeats-20261001.csv).

Artefactos en `target/performance/station-reservation-routes-20261001`.
Cliente anterior/posterior SHA256
`1ab8dfc2fcf27d3a94d1def6ab60dba9ef2c035e2b33894d4f20cdd9559bb259` /
`029bd3a603b9b81da9b179ce37d2843f2ad16883afe02d3a4a891df06e7c0645`;
`sav_profile`
`77dfedc8b88eb0e5588361146368fe73383a32e0154baf8a9b8a48bdd5541516` /
`89522e091f7e3226135ceaa6525a9d0b4220140f5fc316aa29abfd1c12645965`;
traza `b8d76f2c9738ab7f09d1cfd6df9d9c2603f4bfc8b4e68816f6f42e9f175c2917` /
`1efd36eef39da175c34f63a67b2d2f964984b089df57052384cd68f77f412107`.
Biblioteca posterior exacta copiada de Cargo JSON
`8e9bfada5fcb7f92d96762270768f81ddffdd9322c18e7ebf56f6bad8a970376`.
Release del perfil 25,08 s, cliente tras cambio core 77 s; no es una mejora
controlada de compilación. Las 183 trazas de fase idénticas se conservan en
hardlinks readonly verificados; se recuperan 477.010.859 bytes.

Validación: 3.035 core/seis ignorados, 1.667 cliente/dos; Clippy en todos los
targets de ambos, formato, diff y frescura de docs. Sólo cierra la expulsión
innecesaria por este bit ferroviario. F15 completo, el límite de 256 entradas,
otras invalidaciones, reglas nativas de rutas, variación raster, cadencia y
30 FPS siguen abiertos. Las capturas repetidas no cierran la variación inicial.

## Etapa 30 — Capturar todas las entradas y máscaras del mapa (F08/F31)

La variación In2x/Out2x de las etapas 28/29 permanece abierta. El stream de
parents no describía los suelos, todos los children, el atlas vigente ni la
oclusión de vidrio. Esta etapa añade un diagnóstico optativo de esas entradas,
sin modificar las reglas ni el orden de dibujo de producción.

Con `OPENTTDRS_MAP_SPRITE_TRACE_OUT=/ruta/entradas.json`, el driver de
`OPENTTDRS_MAP_SHOT` pide una única exportación en su frame de captura. Se
ejecuta después de propagar transformaciones y visibilidad. Guarda todos los
`Sprite` y `SpriteMesh` en el orden de sus queries, los bits de matrices,
anchors, tamaños y rectángulos, relaciones source/proxy y parent/child,
clasificación, chunks, capas, visibilidad y cámaras. Exporta también los
layouts completos y los bytes CPU de cada imagen, en `entradas.images/`.
La visibilidad registrada es la unión de las vistas; no es una traza del
orden de batches del render world ni una lectura de su buffer de profundidad.

Las dos capturas auxiliares del mismo frame son `entradas.coverage.png` y
`entradas.occlusion.png`. Sus targets son `Rgba8Unorm`: el guardador genérico
de Bevy no los acepta. El diagnóstico guarda sus bytes RGBA directamente;
una regresión comprueba que no introduce conversión sRGB. La primera corrida
fallida, con JSON válido y ambas máscaras ausentes, se conserva en el registro.
Las carpetas de bytes rechazan sobrescrituras de capturas previas.

`scripts/compare_map_sprite_traces.py` compara todo el documento y los bytes
CPU. Sólo renumera identidades temporales mediante mapas separados para cada
mundo y tipo de asset; conserva todos los campos, órdenes, referencias y
alias. Sus tres contrajemplos cubren renumeración válida, sources intercambiados
y una diferencia de un bit de float. Las tres regresiones Rust cubren además
el proxy compartido, precisión, bytes y rechazo de sobrescritura.

Siete corridas con GPU real, Kale, 1280×720, centro 128,128, 180 frames y
`CLEAN=0`: cinco In2x, dos Out2x. Todas conservan PNG principal canónico y
entradas completas tras renumeración; todos los bytes CPU son exactos. Las
seis corridas con máscaras completas también conservan ambas imágenes.
In2x registra 13.623 sprites y 13.623 meshes, 272 imágenes y 29.572.756 bytes;
Out2x, 45.209 de cada componente, 384 imágenes y 30.333.044 bytes. No hay
imágenes de ese conjunto sin datos CPU. Las corridas diagnósticas no se usan
como medición de FPS.
[Capturas y hashes](evidence/glass-raster-variation-captures-20261001.csv).

Con la traza desactivada, los seis zooms conservan PNG, cero píxeles y cero
bloques 4×4 distintos, y el stream completo del sorter tras renumeración,
frente a capturas del binario anterior fijado. Para In2x/Out2x se usan las
repeticiones canónicas de la etapa 29; sus fallos iniciales siguen conservados.
[Seis zooms](evidence/glass-raster-variation-disabled-20261001.csv).

En el píxel In2x `(722,0)`, el parent opaco 1163 tiene profundidad
`2,47322678565979` y su child de vidrio `2,473237991333008`. Una reproducción
**escalar CPU en f32** de la proyección capturada da para ambos
`0,2506432831287384`, bits `1048597585`. La máscara actual tiene rojo 248,
el framebuffer `(65,64,65,255)` y la captura alternativa anterior
`(98,101,98,255)`. Es un empate numérico reproducible que justifica investigar
el pase binned con `GreaterEqual`; no confirma el orden efectivo del GPU ni
explica todavía los otros 203 píxeles o el píxel verde Out2x `(836,245)`.
[Datos y alcance del cálculo](evidence/glass-raster-depth-tie-20261001.csv).

El original dibuja cada parent y después sus children, en orden, en
[`ViewportDrawParentSprites`, L1716](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/viewport.cpp#L1716).
Esa regla sirve de referencia para la siguiente corrección acotada de la
máscara. Esta etapa no acredita paridad raster nativa ni importación SAV
completa: las nuevas repeticiones no reprodujeron el PNG alternativo.

Reproducción, además de las opciones habituales de save/cámara/zoom:

```bash
OPENTTDRS_MAP_SPRITE_TRACE_OUT=/tmp/entradas-a.json \
OPENTTDRS_MAP_SHOT=/tmp/mapa-a.png \
cargo run -p openttdrs-client --release
python3 scripts/compare_map_sprite_traces.py /tmp/entradas-a.json /tmp/entradas-b.json
python3 scripts/compare_map_sprite_traces.py --self-test
```

Artefactos en `target/performance/glass-raster-variation-20261001`.
Cliente anterior `029bd3a603b9b81da9b179ce37d2843f2ad16883afe02d3a4a891df06e7c0645`;
primera sonda `61c7b739fb7d60714230a00346a65bfa3ca8017225198a23f5678c8dddb0ca4d`;
sonda con guardado RGBA `a0a959b4715b41f7471ffa8131b2525aaf6067da499d0365074173600d7d1a26`.
`sav_profile`, traza de estado y biblioteca release son hash-idénticos a la
etapa 29; se conserva su evidencia de 61 fases, sin atribuir un nuevo replay.
Los builds cliente de 54,25 / 53,72 s no prueban una mejora de compilación.

Validación: 3.035 core/seis ignorados, 1.670 cliente/dos, tres tests Python;
Clippy en todos los targets de ambos, formato, diff y frescura de docs.
Sólo cierra el sub-issue de captura reproducible de estas entradas y targets.
Variación raster, orden efectivo de la máscara, F08 completo, cadencia y
30 FPS continúan abiertos; los FPS vigentes siguen siendo los de la etapa 29.


## Etapa 31 — Empates de profundidad y candidato de máscara retirado (F08/F31)

La regla nativa de `ViewportDrawParentSprites` sigue siendo parent y después
children, en orden. La nueva regresión usa la matriz real de Bevy/glam, cámara
`(0,-4104,999.9)` y ortho near/far ±2000: los bits de profundidad mundial
`1075726681` y `1075726728` son distintos y ordenados, pero su proyección f32
es idéntica. Es una reproducción CPU; no es lectura del depth buffer GPU.
El test conserva este contrajemplo a confiar sólo en unicidad de profundidad.

Se ensayó una máscara en la fase ordenada `Transparent2d`, sin escribir
profundidad. El candidato mantenía el shader y uniform Mask, cutoff 0,5 y
`MAY_DISCARD`; sólo cambiaba la cola y el pipeline de materiales render-world.
Los fragmentos supervivientes seguían teniendo alpha 1, evitando acumulación
de oclusores parciales. Los productores actuales de `SpriteMesh` son proxies
de esa máscara; una futura mezcla de productores requeriría acotar materiales.
**El candidato está retirado: esta etapa no lo instala en producción.**

El primer prototipo registraba IDs de materiales en el main world. Aunque el
multiconjunto de descriptores y bytes de imágenes coincidía, el comparador
estricto detectó cambios de orden de queries y ordinales: resultado falso,
conservado. La versión render-world-only eliminó esa interferencia. En In2x y
Out2x conserva todos los campos y órdenes tras renumeración biyectiva de IDs,
los bytes de las 272/384 imágenes y las máscaras de cobertura. Cambia el
resultado de oclusión; no altera el stream completo del sorter en ninguno de
los seis zooms.

Kale, 1280×720, centro 128,128, settle 180, CLEAN=0: diferencias candidato
frente al control canónico, sin ampliar tolerancia:

- In4x: 0 píxeles / 0 bloques 4×4.
- In2x: 3.464 / 421; bbox `(0,0,627,385)`.
- Normal: 346 / 56; bbox `(264,0,386,363)`.
- Out2x: 1.068 / 275; bbox `(163,81,1123,685)`.
- Out4x: 1.407 / 502; bbox `(18,232,1154,683)`.
- Out8x: 219 / 115; bbox `(64,74,1230,684)`.

Se capturó también el oracle OpenTTD fijado en
`14ec60f248547d4d062a1160f0fc26d742319888`, blitter 8bpp-simple/OpenGFX 8.0.
Su hook existente exporta PNG, pero no la traza de sorter solicitada; el primer
wrapper devolvió error por esa ausencia, aunque guardó un PNG válido. Las
cinco capturas siguientes omiten esa opción y terminan correctamente. CLEAN=0
no garantiza que ambos engines hayan avanzado los mismos ticks: las capas
móviles, UI y animaciones no quedan certificadas por esta comparación.

Las coordenadas virtuales nativas equivalen a `(4*x,-4*y)` del mundo del port.
Con las cámaras registradas, In4x/In2x/Normal/Out2x/Out4x requieren traslación
vertical de +16/+8/+4/+2/+1 píxeles, sin desplazamiento horizontal. Se deriva
de coordenadas, no de buscar la mejor coincidencia. En los píxeles modificados,
In2x da 2.618 nuevos exactos, cero antiguos exactos y 846 que no coinciden con
ninguno. Normal da 331 / 0 / 15. Out2x da 147 / 114 / 807; Out4x,
265 / 44 / 1.098. **Hay 114 y 44 coincidencias nativas perdidas**, además de
las diferencias residuales. No se aceptó una corrección general por mejorar
sólo In2x. Quedan por aislar muestreo, composición, capas y sincronización.

Out8x fue además limitado por la cámara nativa: virtual left/top
`(-17736,6244)`, frente al recorte centrado esperado. Su traslado derivado
es `(-85,75,-42,125)` píxeles, fraccionario; no se cuentan coincidencias
nativas exactas ni se oculta ese desalineamiento. No es la misma imagen por
pedir la misma tesela central.
[Seis zooms, hashes, entradas y comparación nativa](evidence/glass-mask-order-raster-20261001.csv).

GPU real sin trazas, perf ni compilación concurrentes, escala 2, ABBA de
40 muestras/run. Cámara fija, warmup 120: frame
44,1518 / 43,8626 → 44,1570 / 44,4278 ms; FPS
22,649 / 22,798 → 22,646 / 22,508. p95
58,0800 / 57,9529 → 58,2078 / 56,8690; máximos/p99
59,2642 / 58,6343 → 58,7312 / 59,1264. TPS
22,653 / 22,799 → 22,647 / 22,410. El segundo posterior cubre
3703194–3703233, un tick desplazado; los otros, 3703193–3703232.
79/80 frames del control y 78/80 del candidato superan 33,33 ms.
[160 muestras fijas](evidence/glass-mask-order-client-steady-20261001.csv).

Pan, warmup 30: frame
50,0419 / 50,2029 → 49,4602 / 50,1625 ms; FPS
19,983 / 19,919 → 20,218 / 19,935. p95
64,3170 / 63,1072 → 61,8304 / 63,6530; máximos/p99
130,1233 / 133,1693 → 129,2287 / 131,6171. TPS
20,838 / 20,801 → 21,090 / 20,801. Todos cubren
3703103–3703142, 80/80 frames por versión exceden el presupuesto. No hay
una mejora clara de FPS; las cifras describen un candidato retirado.
[160 muestras de pan](evidence/glass-mask-order-pan-20261001.csv).

La primera exportación completa Out8x falló por falta de espacio; la repetición
en /tmp falló por cuota. Se conservan logs y JSON parciales comprimidos, no
se aceptan como trazas completas. La captura válida Out8x omite los bytes de
sprites y conserva PNG y sorter. El pan inicial no produjo muestras y venció
su timeout; se conserva por separado y no entra en estadísticas. Se deduplican
1.615 archivos de texturas sólo tras comprobar bytes, con hardlinks readonly
entre artefactos; se recuperan 149.241.528 bytes. El binario inicial retirado y
el JSON parcial se archivan con verificación SHA256 antes de quitar sus copias.
La ejecución nativa escalada inicial venció la revisión automática de permisos;
la alternativa con XDG_DATA_HOME/XDG_CONFIG_HOME propios produjo los PNG.

Artefactos: `target/performance/glass-mask-order-20261001`; segundo intento
parcial Out8x en `/tmp/openttdrs-glass-mask-order-out8-retry` y primer parcial
preservado en `/tmp/openttdrs-glass-mask-order-trace-8-partial.json.gz`.
Cliente control SHA256
`a0a959b4715b41f7471ffa8131b2525aaf6067da499d0365074173600d7d1a26`;
primer prototipo
`e17f752d98edf4a2d233daa7ba5be37c113bd29a8ba49b063e00d60a912707ca`;
candidato render-world-only
`a1204ea30bb6420b7b2335b1d28d7a3d2d9f1e0d31da22930cc694fdb22ac4c8`.
Sus fuentes están conservadas en el directorio de artefactos. `sav_profile`,
sonda de estado y biblioteca release conservan los hashes exactos de la etapa
29: se reutiliza la evidencia de 61 fases, no se atribuye un replay nuevo.
El build del candidato de 53,50 s no acredita mejora de compilación.

Validación de la etapa conservada: 3.035 core/seis ignorados, 1.671 cliente/dos;
Clippy de ambos para todos los targets, formato, diff, frescura de docs y tres
tests del comparador de entradas. Esta etapa cierra sólo la reproducción del
empate f32 y el ensayo documentado. F08/F31, diferencias nativas de máscara,
variaciones históricas, cadencia y 30 FPS permanecen abiertos. El renderer
conserva las reglas de la etapa 30 al retirar los dos prototipos.


## Etapa 32 — Evitar búsquedas completas de proxies sin cambios (F08/F31)

El renderer conserva el pase de profundidad anterior: no instala el candidato
retirado de la etapa 31. Esta etapa sólo reduce búsquedas ECS en
`sync_rail_glass_mask_proxies`. Un scan filtrado recoge los proxies con cambios
en SpriteMesh, Anchor, Transform, Visibility o MapTileChunk en un EntityHashSet
local reutilizado. Si fuente y clasificación no cambiaron, tampoco el proxy,
y la entidad sigue cumpliendo la query, conserva su liveness y evita obtener
la tupla completa de componentes mutables. Las demás fuentes siguen por el
camino existente, en el mismo orden de creación y limpieza.

Se mantienen todos los proxies, incluidos los ocultos, las cámaras, cutoff,
shaders y materiales. El chequeo de existencia cubre despawns y retiro de un
componente requerido; la clasificación cacheada detecta retiro del marker de
vidrio. Un ParamSet separa la query de cambios y la query mutable, evitando
accesos ECS incompatibles. Las regresiones existentes conservan 49 estados
con 64 fuentes y ciclos de vida; la nueva modifica cada uno de los cinco
campos del proxy por separado sin cambiar su fuente y exige reparación exacta.

El perfil previo usa el cliente final conservado de la etapa 31, SHA256
`d3c11e988f75e1b518d5ca886385401e8d0b761df4e38277e88c9888398d7181`.
`perf record -e cpu-clock:u -F 199 --call-graph dwarf,32768` empieza tras
warmup 120 y el primer flush de 60 muestras; el ACK del FIFO confirma la
activación. Guarda 240 frames, con sampling en el intervalo posterior hasta
el final, 7.909,084 ms entre primera y última muestra. Hay cero samples
perdidos y un evento fuera de orden reportado. La pila más amplia sigue sin
reconstruir callers fiables: los porcentajes children casi coinciden con self.
No se atribuyen callers ni tiempos exactos a partir de esa pila.

Las hojas identificadas incluyen 4,66 % + 0,83 % en el sync de máscaras,
2,20 % en una query paralela de visibilidad, 1,54 % en extracción Mesh2d,
1,54 % en sort de parents y 1,05 % en su sorter puro. Los porcentajes son
CPU del proceso y sus threads, no porcentajes del frame ni FPS sin perf.
[Scope y porcentajes de hojas](evidence/glass-proxy-lookup-perf-20261001.csv).
Registro de 77.164.872 bytes (data section 77.149.936), SHA256
`e10f993245423e4e6c2c504b62c57ce0dcc725458450a4850cbace3353c6e175`.
El scope, header, reportes y hojas están en
`target/performance/client-render-residual-20261001`; no se repite el perfil
para presentar tiempos instrumentados como ganancia normal.

GPU real, Kale activa, 1280×720, escala 2, ABBA sin perf, trazas ni
compilación concurrentes, 40 muestras/run. Fijo, warmup 120: vidrio
3,2827 / 3,2905 → 2,6737 / 2,6167 ms. Frame
44,7374 / 44,6381 → 43,3720 / 43,4176 ms; FPS
22,353 / 22,402 → 23,056 / 23,032. p95
58,1447 / 59,0647 → 57,3860 / 56,2535; máximos/p99
60,2406 / 60,7406 → 58,8897 / 59,2344. TPS
22,358 / 22,303 → 23,060 / 23,130. Ambos controles cubren
3703194–3703233, ambos candidatos 3703193–3703232: se conserva ese
**desplazamiento de un tick** y no se presentan como ventanas alineadas.
80/80 frames anteriores y 76/80 posteriores superan 33,33 ms.
[160 muestras fijas](evidence/glass-proxy-lookup-steady-20261001.csv).

Pan, warmup 30: vidrio
3,0916 / 3,1717 → 2,5613 / 2,6043 ms; frame
49,4334 / 50,0977 → 49,0999 / 49,4946 ms; FPS
20,229 / 19,961 → 20,367 / 20,204. p95
62,3890 / 62,6911 → 61,9150 / 62,6842; máximos/p99
129,3509 / 132,7875 → 127,7722 / 127,9446. TPS
21,104 / 20,843 → 21,239 / 21,060. El primer control cubre
3703104–3703143; los otros, 3703103–3703142. 80/80 frames de ambos
exceden el presupuesto. La fase de vidrio baja de forma consistente en estas
cuatro parejas; la ganancia global es pequeña, especialmente en pan, y no
acredita 30 FPS ni elimina los picos.
[160 muestras en movimiento](evidence/glass-proxy-lookup-pan-20261001.csv).

Doce capturas congeladas, seis escalas .25/.5/1/2/4/8, centro 128,128,
settle 180 y CLEAN=0: PNG exactos, cero píxeles y cero bloques 4×4 distintos
en todas las parejas. Los seis streams completos del sorter coinciden tras
renumeración biyectiva de entidades, conservando orden, campos y referencias.
En In2x y Out2x, todas las entradas de sprites/meshes/cámaras y los bytes CPU
de las 272/384 imágenes también coinciden; ambas máscaras son exactas. No se
omite ningún campo para aceptar la comparación ni se amplía tolerancia.
[Seis zooms, entradas y hashes](evidence/glass-proxy-lookup-raster-20261001.csv).
Las capturas prueban conservación del renderer previo en esta fixture;
no acreditan paridad nativa ni resuelven los empates y recortes de la etapa 31.

Artefactos en `target/performance/glass-proxy-lookup-20261001`.
Cliente anterior/posterior SHA256
`d3c11e988f75e1b518d5ca886385401e8d0b761df4e38277e88c9888398d7181` /
`531c7acbe43439a265a2fa2453f86d70a1458d748424d89488f5832314d49182`.
El core usado por ambos builds cliente es el mismo artefacto Cargo fresh,
`libopenttdrs_core-6f345d3271658885.rlib`, SHA256
`09d42185d59accd956373a9f95e5e36de6132065ba7cfcee5528da0dfd8734e0`.
Es una variante de dependencias distinta a la biblioteca fijada de sav_profile,
`8e9bfada5fcb7f92d96762270768f81ddffdd9322c18e7ebf56f6bad8a970376`,
usada por el replay de 61 fases de la etapa 29. El core y las sondas no cambian;
se conserva esa evidencia del perfil aislado sin atribuir un replay nuevo del
cliente ni intercambiar las dos bibliotecas por fecha de modificación.

Los cuatro JSON completos de entradas se comparan antes de archivarlos; sus
.gz se verifican contra SHA256 de los bytes originales. Sus carpetas .images
mantienen los bytes mediante hardlinks readonly verificados, recuperando
119.811.600 bytes de texturas. También se preserva comprimido el JSON válido
Out4x del candidato retirado, recuperando 151.872.246 bytes, y el primer
binario diagnóstico de la etapa 30, recuperando 142.266.520. Release de esta
etapa: 53,38 s; no prueba una mejora de compilación.

La primera suite completa del cliente tuvo 1.668 pasados, cuatro fallos por
`StorageFull` al copiar atlas/ejecutable aislado y dos ignorados. Se conserva
`client-tests.log`; no se cambió código para resolverlos. Con TMPDIR propio
en /tmp, la repetición completa pasa 1.672/dos. La copia propia del caché
antiguo de 5.940.131.840 bytes se compacta en 89 bloques gzip verificados
antes de liberar cada rango; el stream reconstruido conserva SHA256
`f354bba5fa0b145bd64db2de09668e7d1ce6ea8f4a0941fea0ab3cafd9fcfdfe`.
Su manifest de recuperación está en
`/tmp/openttdrs-obsolete-incremental-0g0ynd2erlr29.tar.gzip-chunks/manifest.json`;
se recuperan 4.134.167.949 bytes. El control a0a959b4 retirado se conserva
comprimido y verificado en /tmp, con enlaces .gz desde sus artefactos: se
recuperan otros 187.508.416 bytes en home. No se eliminan archivos ajenos.

Validación: 3.035 core/seis ignorados, 1.672 cliente/dos; Clippy de ambos en
todos los targets, formato, diff, frescura de docs y tres tests del comparador.
Cierra sólo la búsqueda completa innecesaria para estos proxies estables.
F08/F31 completos, muestreo/composición nativos, variaciones históricas,
cadencia y 30 FPS siguen abiertos.

## Etapa 33 — Tablas de entidades en el viewport (F18/F31)

Se sustituyen cuatro índices con claves exclusivamente Entity por las tablas
EntityHashMap/EntityHashSet de Bevy: estados de parents, ventanas de profundidad,
children independientes y agrupación temporal de children. Los tres primeros
se usan sólo para inserción y consulta. La agrupación recorre sus grupos en
orden arbitrario, pero sólo escribe transforms de children distintos: no crea
entidades ni Commands. Cada grupo conserva exactamente el sort por source_depth
y Entity::to_bits, la fórmula de intervalo, fallback y guardia f32 anteriores.
Los mapas de claves compuestas y la limpieza de proxies quedan fuera del cambio.

El contrato nativo sigue siendo `ViewportDrawParentSprites` de OpenTTD
14ec60f248: dibuja cada parent y después su cadena de children, antes de pasar
al siguiente parent. La tabla acelera búsquedas internas del port sin cambiar
ese contrato ni acreditar por sí sola su paridad completa. La regresión nueva
intercala grupos, cambia la profundidad del parent y cubre intervalo estrecho,
fallback del último parent, child independiente y parent inválido; conserva
también los bits XY. Pasan las 26 pruebas del módulo.

GPU real, Kale activa, 1280×720, escala 2, ABBA sin perf, trazas o compilación
concurrentes, 40 muestras/run. Fijo, warmup 120: sort
4,4219 / 4,3932 → 4,1765 / 4,0173 ms; children
1,0943 / 1,1074 → 0,8454 / 0,7917 ms. Frame
43,4028 / 43,4054 → 44,3369 / 42,6202 ms; FPS
23,040 / 23,039 → 22,555 / 23,463. p95
56,8860 / 57,1907 → 56,9709 / 57,1753; máximos/p99
60,5302 / 59,6293 → 102,1578 / 57,6853. TPS
23,056 / 23,040 → 22,534 / 23,475. Los cuatro runs cubren
3703193–3703232. 76/80 frames anteriores y 72/80 posteriores superan
33,33 ms. Se conserva el pico de 102,1578 ms, sin atribuirle una causa probada.
[160 muestras fijas](evidence/viewport-entity-lookups-steady-20261001.csv).

Pan, warmup 30: sort
4,2887 / 4,2144 → 4,0213 / 3,9527 ms; children
1,1293 / 1,1424 → 0,8224 / 0,7910 ms. Frame
49,5200 / 49,0116 → 50,0619 / 48,6307 ms; FPS
20,194 / 20,403 → 19,975 / 20,563. p95
64,1869 / 61,4702 → 66,6261 / 61,1651; máximos/p99
131,9280 / 129,2325 → 131,7661 / 127,9329. TPS
21,094 / 21,297 → 20,848 / 21,460. Los cuatro runs cubren
3703103–3703142. Los 80 frames de cada versión exceden el presupuesto.
[160 muestras en movimiento](evidence/viewport-entity-lookups-pan-20261001.csv).

Las fases afectadas mejoran en las cuatro parejas, pero **no se acredita una
ganancia global de FPS**: el frame medio combinado fijo es 43,4041 → 43,4786 ms
y el de pan 49,2658 → 49,3463. El resto del frame conserva variación; sumar
timers no constituye una atribución completa del hilo de render o GPU. Sigue
pendiente localizar ese trabajo y reducir los picos, junto con la cadencia.

Doce capturas congeladas, seis escalas .25/.5/1/2/4/8, centro 128,128,
settle 180 y CLEAN=0: cero píxeles y bloques 4×4 diferentes, PNG exactos.
Los seis streams completos del sorter coinciden tras renumeración biyectiva
de entidades, sin omitir campos ni alterar su orden. In2x/Out2x conservan
todas las entradas de sprites, meshes y cámaras, bytes CPU de las 272/384
imágenes y ambas máscaras. No cambia la tolerancia; esto acredita conservación
del port anterior en esta fixture, no importación SAV ni raster nativos completos.
[Seis zooms, entradas y hashes](evidence/viewport-entity-lookups-raster-20261001.csv).

Artefactos: `target/performance/viewport-entity-lookups-20261001`.
Cliente anterior SHA256
`531c7acbe43439a265a2fa2453f86d70a1458d748424d89488f5832314d49182`;
posterior `3ac2ff2cf93f4e89bf8ee88eee8bed2e19bb3cf9e4513b398f2d4d925b96f63d`,
conservado readonly en /tmp y enlazado desde el directorio de artefactos.
Cargo identifica fresh la misma biblioteca core del cliente que en la etapa
32, SHA256 `09d42185d59accd956373a9f95e5e36de6132065ba7cfcee5528da0dfd8734e0`.
No cambia el core ni se atribuye otro replay de las 61 fases aisladas. Release
tarda 53,61 s; no acredita una mejora de compilación.

Los cuatro JSON nuevos se comparan completos antes de comprimirse y verificarse
por SHA256; las imágenes se deduplican sólo tras igualdad de bytes, entre
artefactos readonly, recuperando 119.811.600 bytes. Ante 158 MiB libres en home,
doce trazas válidas propias de las etapas 30/31 se preservan como gzip readonly
en `/tmp/openttdrs-preserved-input-traces-stage33`, verificando los bytes
originales antes de quitar sus copias sin comprimir. Conservan enlaces .gz y
el manifiesto `preserved-prior-traces.csv`; se recuperan 352.883.180 bytes en
home. Los tests usan TMPDIR propio en /tmp. No se retiran archivos ajenos.

Validación: 3.035 tests core/seis ignorados, 1.673 cliente/dos; Clippy de
ambos para todos los targets, formato, diff, frescura de docs y tres tests del
comparador de entradas. Esta etapa acota la optimización a estas búsquedas;
F18/F31 completos, variaciones históricas, paridad nativa, cadencia y 30 FPS
siguen abiertos.

## Etapa 34 — Atribución por etapas del loop principal (F31)

`OPENTTDRS_PERF_MAIN_OUT=otro.csv`, junto con `OPENTTDRS_PERF_OUT`, registra
intervalos de pared para cada schedule del MainScheduleOrder vigente, incluyendo
StateTransition y schedules de otros plugins. Intercala observadores entre las
etiquetas originales sin agregar dependencias entre sus sistemas ni modificar
los schedules de Startup. Conserva frame, tick, warmup y flush final. Los timers
se guardan dentro del FrameCapture existente, sin registrar otro recurso ECS.
El CSV normal mantiene su esquema; el detallado incluye main_ms y una columna
por etiqueta, con índice y nombre escapado. Rechaza archivos coincidentes,
incluidos aliases por hardlink en Unix, antes de alterar los schedules.

La regresión añade un schedule propio y exige su orden original durante cinco
frames, tres muestras tras warmup, ticks exactos y suma de intervalos. Comprueba
también que destinos inválidos o coincidentes conservan el orden y los archivos.
Detectó un fallo real del primer intento: insert_before/after no encuentra una
etiqueta internada pasada como wrapper. Se corrige construyendo la lista con
las etiquetas originales intactas. El intento fallido queda en los logs.
OpenTTD 14ec60f248 separa game loop, drawing y video en PerformanceElement;
estas categorías de Bevy son otro contrato y no se equiparan por nombre.

GPU real, Kale activa, 1280×720, escala 2, ABBA del **mismo binario** con el
observador detallado off/on, 40 muestras/run, sin perf o compilación concurrentes.
Fijo, warmup 120: frame off 42,4367 / 42,7722 ms, on
42,7305 / 42,8685; FPS 23,565 / 23,380 frente a 23,402 / 23,327.
p95 off 56,5327 / 56,8575, on 56,7248 / 57,5323; máximos/p99
58,8752 / 60,3485 frente a 59,6329 / 57,9557. TPS
23,570 / 23,366 frente a 23,385 / 23,314. Los cuatro runs cubren
3703193–3703232; exceden 33,33 ms 73/80 frames off y 76/80 on.
El frame combinado aumenta 42,6044 → 42,7995 ms; no acredita una ganancia.
[160 muestras y coste observado](evidence/main-schedule-attribution-stable-steady-observer-20261001.csv).

Con el observador on, main_ms es 40,4268 / 40,5005;
RunFixedMainLoop 14,2132 / 14,3216, Update 18,7290 / 18,7186 y
**PostUpdate 6,9101 / 6,8971 ms**. First, PreUpdate, StateTransition,
SpawnScene y Last completan la suma. Los 80 registros se alinean por frame/tick
con el CSV normal y su suma difiere menos de 0,001 ms por redondeo.
[80 muestras por schedule](evidence/main-schedule-attribution-stable-steady-phases-20261001.csv).

Pan, warmup 30: frame off 48,1724 / 48,3672, on
48,4545 / 48,5860 ms; FPS 20,759 / 20,675 frente a 20,638 / 20,582.
p95 60,5872 / 61,9569 frente a 61,6757 / 61,8302; máximos/p99
130,2161 / 128,3545 frente a 130,3193 / 129,0829. TPS
21,707 / 21,591 frente a 21,572 / 21,495. Los cuatro runs cubren
3703103–3703142; los 80 frames por condición superan el presupuesto.
Frame combinado 48,2698 → 48,5202 ms. Main on 43,0131 / 43,2000;
RunFixedMainLoop 14,8441 / 15,0416, Update 19,4741 / 19,4254 y
**PostUpdate 8,0711 / 8,0824 ms**.
[160 muestras y coste](evidence/main-schedule-attribution-stable-pan-observer-20261001.csv),
[80 registros por schedule](evidence/main-schedule-attribution-stable-pan-phases-20261001.csv).

Son intervalos del hilo principal con dispatch y observadores incluidos.
PostUpdate contiene, entre otras tareas, propagación de transforms, cálculo de
bounds, visibilidad y UI: esta medición no reparte sus 6,9–8,1 ms entre ellas.
Main no incluye el subapp de render ni GPU. frame_ms mide el intervalo anterior
entre begin_frame, mientras main_ms cubre el ciclo actual: restarlos por fila
no constituye un timer de GPU. Cadencia y 30 FPS permanecen abiertos.

El primer prototipo añadía MainScheduleCapture como recurso ECS independiente.
En los seis zooms congelados .25/.5/1/2/4/8 produjo respectivamente
0/1136/1500/245/1033/399 píxeles diferentes y 0/97/247/102/341/231 bloques 4×4.
Los streams completos coincidían sólo en los tres primeros. In2x/Out2x fallaban
la comparación ordenada de entradas y bytes por ordinal; el multiset de
descriptores y payloads era idéntico. No se acepta ese multiset como paridad.
Se retira el recurso adicional; su binario, capturas y cuatro CSV de tiempos
marcados retired_extra_ecs_resource permanecen como
diagnóstico. [Fallos completos](evidence/main-schedule-attribution-raster-first-resource-20261001.csv).

La variante conservada, con timers en FrameCapture, da doce PNG exactos en los
seis zooms, cero píxeles/bloques y seis streams completos exactos. In2x/Out2x
conservan todas las entradas ordenadas, los bytes de 272/384 imágenes y ambas
máscaras. El control usa el colector normal anterior y el candidato añade el
detalle; no se omiten campos ni se amplía tolerancia.
[Seis zooms con colector](evidence/main-schedule-attribution-raster-20261001.csv).

Sin colector, se reutilizan los controles congelados de la etapa 33 y se toman
seis PNG nuevos: cinco zooms son exactos; In2x reproduce el hash alternativo
histórico 4e1118f0, con 204 píxeles/45 bloques y sorter exacto.
[Control desactivado](evidence/main-schedule-attribution-raster-disabled-20261001.csv).
Tres parejas nuevas In2x con entradas completas dan cero diferencias en la
primera/tercera y 204/45 en la segunda. Las tres conservan sorter, bytes CPU
y cobertura; la segunda cambia oclusión y falla entradas completas. En ella,
sprites fuente y cámaras son idénticos, pero 3.333 de 13.623 filas de meshes
aparecen en otro orden. Conservan el multiset de todos sus campos y referencias
de fuente al quitar sólo sus IDs propios temporales para diagnóstico. Esto
acota la permutación de proxies; **no sustituye la comparación ordenada fallida**
ni demuestra por sí solo la causa de cada píxel. No se declara resuelta la
variación histórica ni conservación raster universal.
[Tres parejas In2x](evidence/main-schedule-attribution-raster-disabled-in2-repeats-20261001.csv).

También se reproduce un efecto del colector normal preexistente: mismo binario
3ac2ff2c, off/on, cambia 0/104/1122/1456/2048/546 píxeles y
0/13/174/337/622/275 bloques. Los streams coinciden en .25/.5/1 y divergen
en 2/4/8. Ese colector registra recursos/queries adicionales antes de entrar
en partida. Las mediciones anteriores comparaban la misma instrumentación
entre versiones; esta evidencia no las transforma en FPS de una sesión sin
colector. Se abre el sub-issue de conservar también su forma ECS antes de
usar la instrumentación como referencia del juego normal.
[Efecto del colector básico](evidence/main-schedule-attribution-legacy-observer-20261001.csv).

Artefactos en `target/performance/main-schedule-attribution-20261001`.
Control SHA256 `3ac2ff2cf93f4e89bf8ee88eee8bed2e19bb3cf9e4513b398f2d4d925b96f63d`;
prototipo retirado `0e9167e5fa8d30592d14bb879a46970d283e20196f6efa2e4063eec719ac7a16`;
conservado `6245fe0374e2aab5b8a5fb7ee3c499ed34fe509099895ce512287951d8c5e427`.
Todos mantienen la biblioteca cliente core Cargo fresh, SHA256 09d42185,
fijada completa en la etapa 33; no se atribuye otro replay de las 61 fases.
Los JSON de entradas se comparan antes de archivarse y sus gzip conservan el
SHA256 raw; las imágenes usan hardlinks readonly comprobados byte a byte.

El primer release falla en mold por falta de espacio, sin binario validado.
Se conserva su JSON de error y reporte HTML. Dos binarios propios readonly
se trasladan a /tmp con verificación SHA y enlaces desde sus cuatro rutas,
recuperando 374.118.616 bytes; se preserva también el output Cargo obsoleto
del prototipo antes de liberar sus dos aliases de caché, 187.591.768 bytes.
Los manifiestos están en relocated-verified-binaries.csv y retired-cargo-output.csv.
El release correcto del prototipo tarda 53,13 s; el conservado, 53,09 s.
Cargo --timings identifica sólo la unidad cliente activa, 53,01/52,95 s,
con dependencias fresh y sin desglose frontend/codegen/link. No es un build
frío ni una mejora de compilación.
[Unidades de compilación](evidence/main-schedule-attribution-build-20261001.csv).

Validación: 3.035 tests core/seis ignorados, 1.674 cliente/dos; Clippy de ambos
para todos los targets, formato, diff, frescura de docs y tres tests del
comparador de entradas. Cierra sólo la captura de estos intervalos con el
recurso existente; atribución fina de PostUpdate, colector básico, proxies
variables, paridad nativa, cadencia y 30 FPS siguen abiertos.


## Etapa 35 — Conservar la escena al activar el colector (F31)

El colector básico ya no guarda FrameCapture como recurso ECS. Su estado se
comparte mediante Arc/Mutex en sistemas exclusivos de First/Last y en las
fronteras opcionales del Main. Las consultas de conteo se crean después del
warmup; la consulta de cámara sólo existe cuando se solicita zoom o pan.
Activar el diagnóstico no registra estos componentes antes de construir la
escena. Mantiene los esquemas CSV, warmup, controles de cámara, ticks, errores
de escritura, flush final y rechazo de destinos coincidentes del detalle.

La regresión compara todo el registro de componentes contra una app con
First/Last vacíos durante dos frames de warmup. No aparecen Sprite, SpriteMesh
ni Projection. Después añade una escena pequeña y exige una fila final de
15 columnas, tick cero y conteos 2/1 sin registrar otra cámara. La regresión de
Main de la etapa 34 sigue cubriendo orden, schedule propio, ticks y aliases.
El primer intento de esta prueba contaba registros internos que Bevy crea al
ejecutar cualquier schedule; la comparación con una app sin colector separa
ese comportamiento del diagnóstico. Los intentos y el resultado corregido
se conservan junto a los artefactos.

GPU real, mismo ejecutable, Kale congelada en tick 3703074, centro 128,128,
1280×720, clean=0 y settle=180: **los seis zooms conservan PNG exactos, cero
píxeles/bloques 4×4 distintos y todo el stream de sort**, con colector básico
off/on. In2x/Out2x conservan además todas las entradas ordenadas y referencias
bajo un renombrado biyectivo de identidades, bytes CPU de 272/384 imágenes y
ambas máscaras. No se aceptan permutaciones por igualdad de multiconjuntos.
Los hashes coinciden con los controles sin colector de la etapa 33, incluidos
60da415e en In2x y 7e8cb274 en Out2x.
[Seis zooms](evidence/collector-scene-shape-raster-20261001.csv).

Otras dos parejas completas In2x suman tres repeticiones básico off/on:
todas exactas, incluidas entradas, bytes y máscaras. El detalle off/on con
el básico activo también conserva las dos parejas completas In2x/Out2x.
[Repeticiones In2x](evidence/collector-scene-shape-in2-repeats-20261001.csv),
[detalle](evidence/collector-scene-shape-detail-raster-20261001.csv).
La variación histórica de 204 píxeles de la etapa 34 sigue registrada; estas
capturas no cierran los empates de profundidad ni la paridad nativa general.
El sub-issue de registro ECS del colector se cierra para esta matriz y la
regresión de warmup, sin extenderlo a cualquier estado o plugin.

Kale activa, GPU, escala 2, ABBA del mismo binario, básico siempre activo y
detalle off/on, 40 muestras/run, sin perf, trazas o compilaciones concurrentes.
Fijo, warmup 120: frame off 42,4241 / 42,8248 ms, on
42,9238 / 42,6332; FPS 23,571 / 23,351 frente a 23,297 / 23,456.
p95 off 56,4523 / 56,9531, on 56,4491 / 55,3251; máximos/p99
58,2563 / 58,5612 frente a 60,0798 / 57,7220. TPS
23,559 / 23,340 frente a 23,285 / 23,447. Ticks
3703193–3703232 en los cuatro runs; 73/80 frames off y 76/80 on
exceden 33,33 ms. Frame combinado 42,6245 → 42,7785 ms.
[160 muestras](evidence/collector-scene-shape-steady-observer-20261001.csv).

Main on ocupa 40,6453 / 40,3622 ms: fixed loop 14,3207 / 14,2548,
Update 18,7400 / 18,6507 y **PostUpdate 6,9600 / 6,8432**.
[80 registros por schedule](evidence/collector-scene-shape-steady-phases-20261001.csv).

Pan, warmup 30: frame off 48,1801 / 48,3456, on
48,2878 / 48,3906 ms; FPS 20,755 / 20,684 frente a 20,709 / 20,665.
p95 60,5017 / 60,4193 frente a 61,6738 / 62,0035; máximos/p99
126,9890 / 129,4264 frente a 128,2700 / 127,0399. TPS
21,664 / 21,614 frente a 21,628 / 21,564. Ticks
3703103–3703142 en los cuatro runs; los 80 frames por condición exceden
33,33 ms. Frame combinado 48,2629 → 48,3392 ms. Main on
43,0037 / 43,1353: fixed loop 14,8622 / 14,9659, Update
19,4752 / 19,4345 y **PostUpdate 7,9612 / 8,0507**.
[160 muestras](evidence/collector-scene-shape-pan-observer-20261001.csv),
[80 por schedule](evidence/collector-scene-shape-pan-phases-20261001.csv).

En cada escenario, los cuatro runs conservan exactamente las 40 tuplas de
frame/tick/conteos de sprites y meshes. El detalle alinea sus muestras por
frame/tick y mantiene la suma de intervalos dentro de 0,001 ms por redondeo.
Esto mide el coste observado del detalle, incluido ruido; no el coste total
del colector básico ni FPS de una sesión sin diagnóstico. La comparación
raster congelada valida su conservación de escena en esta matriz. Main mide
el ciclo actual y frame_ms el intervalo anterior: su resta por fila no mide
GPU. Tampoco hay una ganancia de FPS atribuida a esta etapa.

Binario final 15cfe88730e81acea388afceda328892512796c1187b150fafbca26dd0f6698d,
conservado de forma inmutable en target/performance/collector-scene-shape-20261001
mediante enlace a /tmp. Control publicado de la etapa 34: 6245fe03, con sus
fuentes de performance preservadas. Cargo mantiene fresh la biblioteca core
real del cliente 09d42185, sin cambios de núcleo; no se atribuye otro replay
a las 61 fases aisladas. Release incremental: 53,47 s totales y unidad cliente
53,31 s, dependencias frescas, sin separación frontend/codegen/link. No se
acredita una mejora de compilación. El reporte HTML y el JSON de Cargo se
conservan junto a las capturas y trazas gzip verificadas.

Para completar el build se archivan 1.034 archivos de un único caché viejo del
cliente del 30/09, fuera de los caches activos. Se verifican SHA y tamaño de
todos los miembros antes de retirar el directorio exacto. El archivo
recuperable /tmp/openttdrs-preserved-client-cache-2ylz474qere8v-20261001.tar.gz
ocupa 734.267.985 bytes; el manifiesto conserva cada ruta y hash. También se
retiran sólo los dos aliases de un ejecutable mutable de Cargo, comprobados
por inode, bytes y copia inmutable existente. No se modifican archivos de
usuario, snapshots, dependencias ni el oracle nativo.

Validación: 3.035 tests core/seis ignorados y 1.675 del cliente/dos ignorados;
Clippy en todos los targets de ambos, formato, tres self-tests del comparador,
frescura de docs y diff sin errores.
F31 completo, atribución interna de PostUpdate, variaciones históricas,
paridad nativa, cadencia y 30 FPS siguen abiertos.


## Etapa 36 — No invalidar poses estables de proxies (F18/F31)

El sorter ocultaba todos los proxies segmentados antes de calcular la vista y
luego reescribía Visibility::Inherited, Transform y Anchor de los reutilizados.
Ahora sólo oculta los retirados antes de encolar el mismo despawn. Conserva
esa frontera también en la salida temprana de input vacío. Los usados sólo
cambian visibilidad cuando difiere, y pose/ancla cuando difieren sus bits.
La Z del parent original de una promoción se actualiza sólo si sus bits cambian.
No altera EPSILON, intervalos, desempates, candidates, orden de queries ni
órdenes de creación/borrado. Sprite conserva por ahora su escritura anterior.
Las comparaciones de floats son por bits para conservar -0 y otros cambios
que PartialEq puede tratar como iguales.

Una regresión de una fuente que cruza varias bandas ejecuta el sorter real.
Repetir un sort forzado conserva poses/anclas/visibilidad y produce cero
Changed<Transform/Anchor/Visibility> en los proxies. Modificar externamente
escala, ancla con -0 y visibilidad sigue reparándose. Al mover la fuente fuera
de la vista, los proxies quedan Hidden antes del borrado diferido. Se repite
la retirada con input vacío y se exige el mismo contrato antes de apply_deferred.
Pasan las 27 regresiones de viewport. La fixture inicial, sin otro parent,
expuso un bug anterior: la lista global vacía descarta también candidates
segmentados que sí alcanzan la pantalla. Esta etapa conserva ese comportamiento;
su corrección semántica queda como el siguiente sub-issue de F08/F18.

Oracle de fuente: OpenTTD 14ec60f248, viewport.cpp::ViewportSortParentSprites
L1596 deja intacta una lista de menos de dos parents y ViewportDrawParentSprites
L1716 dibuja cada parent y sus children. La optimización de flags ECS conserva
ese contrato del port; no cambia el algoritmo puro ni certifica todas las
composiciones nativas.

GPU real, Kale congelada, centro 128,128, 1280×720, clean=0, settle=180:
**doce PNG en seis zooms exactos**, cero píxeles/bloques 4×4 y seis streams
completos de sort idénticos bajo renombrado biyectivo de identidades.
In2x/Out2x mantienen todas las entradas ordenadas, referencias, bytes CPU de
272/384 imágenes y ambas máscaras. Los controles son los del binario inmutable
de la etapa 35, sin colector; no se cambia tolerancia ni se permite permutar
filas. Es conservación del port, sin cierre de paridad nativa ni de las
variaciones históricas.
[Seis parejas](evidence/viewport-proxy-change-flags-raster-20261001.csv).

Dos tandas independientes ABBA, mismos ejecutables antes/después, Kale activa,
GPU, escala 2, 40 muestras/run. El colector básico y el detalle del Main están
activos en **ambas versiones**, sin perf, trazas ni compilaciones concurrentes.
Fijo usa warmup 120; pan warmup 30. Se conservan todas las muestras, incluidos
el pico y el run desplazado de la primera tanda.

Primera tanda fija: frame anterior 42,6429 / 45,1938 ms, posterior
42,8535 / 42,3951; FPS 23,451 / 22,127 frente a 23,335 / 23,588.
p95 56,3787 / 64,0253 frente a 56,2517 / 55,7843; máximos/p99
57,7510 / 74,1540 frente a 59,2132 / 57,8946. TPS
23,438 / 22,092 frente a 23,345 / 23,591. Ticks
3703193–3703232 en los cuatro runs; 73/80 frames por versión superan
33,33 ms. PostUpdate 6,9214 / 7,4748 → 6,9420 / 6,7884.
Frame combinado 43,9184 → 42,6243 ms; el control 2 aumenta también en
simulación y Update. No se atribuye ese cambio al candidato.
[160 muestras](evidence/viewport-proxy-change-flags-steady-20261001.csv),
[160 registros por schedule](evidence/viewport-proxy-change-flags-steady-phases-20261001.csv).

Primera tanda pan: frame 48,3273 / 48,4615 → 48,2650 / 48,2147;
FPS 20,692 / 20,635 → 20,719 / 20,741. p95
61,7309 / 61,3367 → 61,6340 / 61,9782; máximos/p99
131,8243 / 133,9969 → 128,2967 / 128,8929. TPS
21,651 / 21,613 → 21,639 / 21,670. El segundo posterior cubre
3703104–3703143, los otros 3703103–3703142; no se declara estado activo
idéntico entre esos runs. Superan 33,33 ms 80/80 anteriores y 79/80
posteriores. PostUpdate 7,9914 / 8,1045 → 8,1045 / 7,9954; combinado
8,0480 → 8,0499, sin reducción. Frame combinado 48,3944 → 48,2399.
[160 muestras](evidence/viewport-proxy-change-flags-pan-20261001.csv),
[160 por schedule](evidence/viewport-proxy-change-flags-pan-phases-20261001.csv).

Se repite porque el pico y la diferencia de tick impiden una conclusión clara.
Segunda tanda fija: frame 42,9109 / 42,8949 → 42,6459 / 42,7474;
FPS 23,304 / 23,313 → 23,449 / 23,393. p95
57,1525 / 57,2036 → 56,2162 / 56,2994; máximos/p99
59,8143 / 59,3402 → 57,4272 / 59,6478. TPS
23,302 / 23,308 → 23,445 / 23,402. Todos cubren
3703193–3703232; 75/80 frente a 74/80 frames sobre el presupuesto.
PostUpdate 6,8834 / 6,9234 → 6,9065 / 6,8648; combinado
6,9034 → 6,8856 ms. Frame combinado 42,9029 → 42,6966.
[160 muestras de repetición](evidence/viewport-proxy-change-flags-repeat-steady-20261001.csv),
[160 por schedule](evidence/viewport-proxy-change-flags-repeat-steady-phases-20261001.csv).

Segunda tanda pan: frame 48,3485 / 48,4904 → 48,0353 / 48,1581;
FPS 20,683 / 20,623 → 20,818 / 20,765. p95
60,5994 / 60,5617 → 61,3777 / 61,5183; máximos/p99
130,0980 / 128,6909 → 130,0987 / 130,9486. TPS
21,621 / 21,536 → 21,772 / 21,722. Todos cubren
3703103–3703142 y los 80 frames por versión exceden 33,33 ms.
PostUpdate 8,0828 / 8,1561 → 8,0266 / 7,9075; combinado
8,1194 → 7,9670 ms. Frame combinado 48,4194 → 48,0967.
También baja el tiempo de simulación, cuyo código no se modifica.
[160 muestras de repetición](evidence/viewport-proxy-change-flags-repeat-pan-20261001.csv),
[160 por schedule](evidence/viewport-proxy-change-flags-repeat-pan-phases-20261001.csv).

Los registros detallados conservan alineación frame/tick con el básico y suma
de intervalos dentro de 0,001 ms. Main es el ciclo actual y frame_ms el
intervalo previo: su resta por fila no mide GPU. Se conserva la reducción de
invalidaciones innecesarias probada por la regresión, **sin acreditar una
mejora global clara de FPS ni una reducción uniforme de PostUpdate**.
El resultado continúa alrededor de 23 FPS fijo y 20–21 en pan.

Control 15cfe887 de la etapa 35 y candidato
d1c5847e08d83f71960e2f63ce976185bb6fed888a7cc0b000c88382f54d35db
son inmutables bajo target/performance/viewport-proxy-change-flags-20261001,
con ejecutables en /tmp y copias exactas del sorter antes/después.
Cargo confirma fresh la misma biblioteca core del cliente 09d42185; no cambia
el núcleo ni se atribuye otro replay de las 61 fases. Release incremental
53,27 s total / unidad cliente 53,11, dependencias frescas, sin desglose
frontend/codegen/link. Se conserva el reporte y no se acredita mejora de
compilación. Traces completos gzip y sus bytes CPU se verifican antes de
archivar y deduplicar.

Validación: 3.035 tests core/seis ignorados y 1.676 del cliente/dos ignorados;
27 regresiones de viewport, Clippy de todos los targets de ambos crates,
formato, tres self-tests del comparador, frescura de docs y diff sin errores.
Se cierra sólo el sub-issue de flags de poses reutilizadas, con reparación y
retiro diferido. F08/F18/F31 completos, fuente segmentada sin parents globales,
empates f32, variaciones históricas, cadencia y 30 FPS siguen abiertos.

## Etapa 37 — Conservar el único parent segmentado (F08/F18)

Una fuente representada por proxies de bandas queda oculta en su entidad
original y no ocupa un slot global. La salida temprana del sorter comprobaba
sólo la lista global: si estaba vacía, descartaba también las bandas visibles.
Ahora retorna únicamente cuando ambas listas están vacías. Conserva el sort
local, sus profundidades, queries, desempates y retirada diferida existentes.

La regresión ejecuta el sistema real con un único parent de 100×300 y ningún
parent ordinario. Antes del cambio falla: recibe cero bandas en lugar de las
seis esperadas. Después exige bandas 4–9, fuente original oculta y cada una
de las 300 filas cubierta exactamente una vez. Mover la cámara fuera de la
fuente retira todos los proxies; volver recrea las seis bandas. Pasan las
28 regresiones de viewport. El fallo anterior y las fuentes antes/después
quedan conservados en target/performance/standalone-segmented-parent-20261001.

Oracle nativo: se extraen sin editar las funciones AddSortableSpriteToDraw,
ViewportSortParentSprites y ViewportDrawParentSprites del viewport.cpp
prístino de OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888. La sonda C++
usa esas funciones con un sprite opaco de 100×300, origen cero y 15 bandas
de una ventana 1280×720. Produce un parent y una llamada de dibujo en cada
banda 4–9, cubriendo cada fila una vez. El manifiesto conserva el hash de
fuente y de las tres funciones; compila con Clang C++20, warnings como errores.
El dibujante es un stub que registra rectángulos opacos: valida preservación
del parent y geometría de bandas, sin certificar texturas, transparencia,
SpriteCombine ni composición GPU nativa.
[Salida de las 15 bandas](evidence/standalone-segmented-parent-native-bands-20261001.csv).

GPU real, Kale congelada en tick 3703074, centro 128,128, 1280×720,
clean=0, settle=180 y sin colector en ambas versiones: los doce PNG de los
seis zooms son exactos, con cero píxeles/bloques 4×4 distintos y streams
completos de sort idénticos bajo renombrado biyectivo de identidades.
In2x/Out2x conservan todas las entradas ordenadas y referencias, bytes CPU
de 272/384 imágenes y ambas máscaras. No se modifica tolerancia ni se acepta
reordenar entradas por igualdad de multiconjuntos. Las trazas gzip se verifican
antes de archivar; los bytes de imágenes se verifican antes de deduplicar.
Esta matriz comprueba conservación de Kale, donde ya existen otros parents;
la regresión aislada y la sonda nativa cubren el caso corregido.
[Seis parejas](evidence/standalone-segmented-parent-raster-20261001.csv).

Control inmutable de la etapa 36: d1c5847e. Candidato:
01066a4ddc27fff00d95ff51eac96c0b0464e013047d5889944cb67013475d47,
conservado mediante enlace a /tmp junto a las fuentes exactas. Cargo confirma
fresh la misma biblioteca core real del cliente 09d42185. Release incremental
53,03 s total / unidad cliente 52,87, dependencias frescas, reporte HTML y
JSON preservados. No hay desglose frontend/codegen/link ni mejora de
compilación atribuida. Esta etapa corrige una desaparición; no mide ni
atribuye ganancia de FPS.

Validación: 3.035 tests core/seis ignorados y 1.677 del cliente/dos ignorados;
28 regresiones de viewport, Clippy en todos los targets de ambos crates,
formato, tres self-tests del comparador, frescura de docs y diff sin errores.

Se cierra únicamente el sub-issue del parent segmentado aislado y su salida
vacía. F08/F18/F31 completos, empates f32, variaciones históricas,
paridad nativa general, cadencia y 30 FPS siguen abiertos.

## Etapa 38 — No invalidar sprites iguales de proxies (F18/F31)

El sorter reescribía Sprite en cada proxy reutilizado, incluso con valores
idénticos. Bevy 0.19.1, calculate_bounds_2d, filtra actualizaciones por
Changed<Sprite/Anchor>; el compositor también observa esos cambios. Una
regresión forzando otro sort sin mutaciones reproduce seis Sprite modificados.
Ahora el proxy sólo recibe la asignación cuando difiere algún campo de la
imagen ordinaria: handle, atlas/layout/índice, variante y componentes raw del
color, flips, tamaño, recorte o modo de escala. Los floats se comparan por
bits, sin convertir espacios de color. Auto y los seis modos Scale quedan
cubiertos; Sliced/Tiled conservan su refresco anterior.

La regresión del sistema real exige cero flags de pose/ancla/visibilidad/Sprite
estables y sigue reparando ediciones externas de color, flips, tamaño, recorte,
atlas y modo, junto a la escala/ancla/visibilidad de la etapa 36. Dos pruebas
añaden los diez espacios de color y sus cuatro componentes, -0, NaN idéntico,
todos los campos de imagen y el refresco conservador de slices. Pasan las
30 pruebas de viewport. Se conserva el fallo antes de la corrección.
El cambio no altera el sort, geometría de bandas, creación de entidades,
queries, EPSILON ni desempates; el contrato nativo de parents/children
verificado en la etapa 37 permanece acotado a aquella sonda geométrica.

GPU real, Kale congelada en tick 3703074, centro 128,128, 1280×720,
clean=0, settle=180 y sin colector: los doce PNG de seis zooms son exactos,
cero píxeles/bloques 4×4 distintos y streams completos de sort idénticos bajo
renombrado biyectivo de identidades. In2x/Out2x conservan todas las entradas
ordenadas y referencias, bytes CPU de 272/384 imágenes y ambas máscaras.
No se cambia tolerancia ni se admite permutar filas por multiconjuntos.
[Seis parejas](evidence/viewport-proxy-sprite-flags-raster-20261001.csv).

Dos tandas ABBA de Kale activa, GPU, escala 2, 40 muestras/run, colector
básico y detalle Main activos en ambas versiones. Fijo warmup 120 y pan 30;
sin perf, trazas ni compilaciones concurrentes. Se conservan 640 muestras
básicas y 640 detalladas, incluidos el pico y el control desplazado.

Primera tanda fija: frame antes 42,7915 / 43,1742 ms, después
42,6175 / 44,5804; FPS 23,369 / 23,162 → 23,465 / 22,431.
p95 56,5889 / 56,6507 → 55,6857 / 59,4778; máximos/p99
59,3970 / 59,6249 → 57,8316 / 82,2200. TPS
23,368 / 23,164 → 23,462 / 22,403. Todos cubren 3703193–3703232;
75/80 frames sobre 33,33 ms en ambas versiones. Combinado:
frame 42,9829 → 43,5990; sort 4,2565 → 4,4107; glass
2,5458 → 2,5370; PostUpdate 6,9108 → 7,1053 ms. El run del
pico aumenta también simulación y otras fases; no se atribuye sólo al guard.
[160 muestras](evidence/viewport-proxy-sprite-flags-steady-20261001.csv),
[160 registros por schedule](evidence/viewport-proxy-sprite-flags-steady-phases-20261001.csv).

Primera tanda pan: frame 48,1571 / 47,9608 → 48,3807 / 48,7214;
FPS 20,765 / 20,850 → 20,669 / 20,525. p95
60,1434 / 62,7014 → 60,8964 / 60,3520; máximos/p99
129,0670 / 126,6408 → 129,6761 / 133,1939. TPS
21,700 / 21,766 → 21,600 / 21,480. Todos cubren 3703103–3703142
y exceden el presupuesto. Combinado: frame 48,0589 → 48,5511;
sort 4,0182 → 4,1899; glass 2,4524 → 2,4335; PostUpdate
8,0183 → 8,0487. También aumenta simulación, cuyo código queda idéntico.
[160 muestras](evidence/viewport-proxy-sprite-flags-pan-20261001.csv),
[160 por schedule](evidence/viewport-proxy-sprite-flags-pan-phases-20261001.csv).

La repetición fija da frame 42,4545 / 43,2157 → 42,8006 / 43,0567;
FPS 23,555 / 23,140 → 23,364 / 23,225. p95
55,7518 / 57,4178 → 56,1107 / 57,1153; máximos/p99
59,0712 / 59,5159 → 61,7447 / 57,9602. TPS
23,547 / 23,040 → 23,358 / 23,245. El control 2 cubre
3703194–3703233, los otros 3703193–3703232; no se declara el mismo
estado activo en ese run. Hay 74/80 frames sobre presupuesto por versión.
Combinado: frame 42,8351 → 42,9286; sort 4,2791 → 4,3840;
glass 2,5425 → 2,4574; PostUpdate 6,9254 → 6,8344.
[160 muestras de repetición](evidence/viewport-proxy-sprite-flags-repeat-steady-20261001.csv),
[160 por schedule](evidence/viewport-proxy-sprite-flags-repeat-steady-phases-20261001.csv).

Repetición pan: frame 48,0518 / 48,3255 → 48,2454 / 48,2075;
FPS 20,811 / 20,693 → 20,727 / 20,744. p95
60,4959 / 61,1307 → 60,8058 / 60,4868; máximos/p99
129,0129 / 128,3920 → 129,6018 / 128,1121. TPS
21,751 / 21,611 → 21,664 / 21,664. Todos cubren 3703103–3703142;
80/80 frames por versión superan 33,33 ms. Combinado: frame
48,1887 → 48,2265; sort 4,0537 → 4,1397; glass
2,4326 → 2,3896; PostUpdate 8,0340 → 7,9891 ms.
[160 muestras de repetición](evidence/viewport-proxy-sprite-flags-repeat-pan-20261001.csv),
[160 por schedule](evidence/viewport-proxy-sprite-flags-repeat-pan-phases-20261001.csv).

Las dos tandas conservan alineación frame/tick básico–detalle y suma de
intervalos dentro de 0,001 ms. La primera y la repetición pan conservan
exactamente las tuplas de frame/tick/conteos entre versiones. La repetición
fija conserva el control desplazado sin descartarlo. Main mide el ciclo
actual y frame_ms el intervalo previo: su resta por fila no mide GPU.
El guard añade comparación al sort; sus medias aumentan 0,09–0,17 ms,
mientras glass y PostUpdate bajan ligeramente en la repetición. Se retiene
la reducción de invalidaciones probada, **sin acreditar ganancia sostenida
de FPS ni una reducción uniforme de PostUpdate**. 30 FPS sigue pendiente.

Control 01066a4d de la etapa 37 y candidato
ce75b3217ed5d5d6544dca41cdc515c6ce22b55d08e6b3e605c45b52038f3d38
son inmutables en target/performance/viewport-proxy-sprite-flags-20261001,
con ejecutables en /tmp y fuentes exactas antes/después. Core real del cliente
09d42185 sigue fresh e idéntico; no se atribuye otro replay de las 61 fases.
Release incremental 53,89 s total / unidad cliente 53,73, dependencias frescas,
HTML y JSON preservados, sin desglose frontend/codegen/link ni mejora de
compilación atribuida. Traces gzip y bytes deduplicados se verifican antes
de retirar copias redundantes.

Validación: 3.035 tests core/seis ignorados y 1.679 del cliente/dos ignorados;
30 regresiones de viewport, Clippy en todos los targets de ambos crates,
formato, tres self-tests del comparador, frescura de docs y diff sin errores.
Se cierra sólo el sub-issue de flags de Sprite ordinarios reutilizados.
Slices, F08/F18/F31 completos, paridad nativa, empates f32, variaciones
históricas, cadencia y 30 FPS siguen abiertos.

## Etapa 39 — Lookup indexado del vehículo para efectos visuales (F04/F31)

spawn_train_smoke ya reconstruye FleetIndex una vez por ejecución. Sin embargo,
vehicle_visual_effect_context buscaba cada ID recorriendo otra vez la flota.
Ahora reutiliza lookup_slot del mismo índice, expuesto públicamente sin cambiar
su algoritmo. El slot se valida contra el ID; índice vacío/obsoleto y IDs
duplicados conservan el primer match del scan anterior. No se cambian contexto
completo/reducido, orden de vehículos, callbacks, consumo de RNG ni writeback.
El comentario de cadencia deja explícito que Update todavía observa sólo el
último tick: este cambio no recupera decisiones de ticks omitidos (F07).

Una regresión diferencial cubre 756 combinaciones: tres motores, seis modos
CB10/CB160 y disponibilidad de runtime, siete estados del índice/flota y seis
IDs, incluidos faltantes. Compara los 24 campos Action2, especificación,
emisiones, RNG, registros persistentes y JSON de vehículos contra el primer
match lineal. Cubren índice fresco/vacío, reorder, eliminación, duplicados,
flota vacía y cambio de ID. Pasan las 29 pruebas de train_smoke.

Fuente nativa prístina: OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888,
vehicle.cpp: ShowVisualEffect (L2777) opera sobre this/Next; no busca el ID
repetidamente en el pool. UpdateVisualEffect (L2636) calcula y guarda
cached_vis_effect; CB10 no se reevalúa en cada ShowVisualEffect. El port aún
reevalúa esa especificación y ejecuta efectos después de step en Update.
Esta etapa sólo corrige el lookup del port; no implementa la caché nativa,
no añade un oracle C++ de callbacks ni acredita su cadencia nativa completa.

Kale congelada, tick 3703074, centro 128,128, 1280×720, clean=0,
settle=180 y sin colector en ambas versiones: cinco de las seis parejas
iniciales tienen PNG/sort exactos y cero píxeles/bloques distintos. La primera
Out2x falla con **406 píxeles / 124 bloques 4×4**: PNG candidato
3f48dbf22b08439c427b1ddbe1d0d05e0c4ae15d8e0391b37685b5e3b50dc3d6.
Dos nuevas parejas Out2x con los mismos ejecutables son exactas y recuperan
el PNG 7e8cb274. El fallo original se conserva; no se reporta 6/6 inicial.
[Primera matriz](evidence/visual-effect-slot-lookup-raster-20261001.csv),
[tres parejas Out2x](evidence/visual-effect-slot-lookup-out2-repeats-20261001.csv).

In2x conserva entradas ordenadas/referencias, 272 imágenes CPU y máscaras.
Out2x inicial conserva 384 imágenes y cobertura, pero falla el orden de
entradas y la máscara de oclusión. Sus 45.209 fuentes son exactas; el sort
sólo difiere en dos input_index globales, sin variar el orden final ni z.
Hay 7.638 posiciones de meshes permutadas. Comparar la captura fallida con
la repetición exacta del **mismo candidato** confirma valores bit a bit por
fuente canónica, fuentes únicas, assets y multiconjuntos de meshes/cámaras
idénticos al quitar únicamente las identidades propias. Eso es un diagnóstico,
no una dispensa del contrato de entradas ordenadas. No prueba la causa del
cambio de orden ni paridad de importación; F08 y los empates f32 siguen abiertos.
[Diagnóstico mismo binario](evidence/visual-effect-slot-lookup-same-binary-out2-20261001.csv).

Dos tandas ABBA activas, GPU, escala 2, 40 muestras/run, colector básico y
Main detail en ambas versiones; fijo warmup 120 y pan 30. Sin perf, trazas
ni compilaciones concurrentes. Se conservan 640 muestras básicas y 640
registros Main, incluidos ticks desplazados y máximos. Los valores siguientes
son ms salvo FPS/TPS; p99 coincide con el máximo con 40 muestras/run.

Primera fija: frame 42,4635 / 42,7448 → 41,6466 / 41,6836;
FPS 23,55 / 23,395 → 24,012 / 23,99;
p95 56,0181 / 56,3965 → 55,2899 / 56,9651;
máximos/p99 57,847 / 57,9305 → 57,2774 / 57,1181;
TPS 23,537 / 23,391 → 23,889 / 23,997.
Ticks antes 3703193,,3703232 / 3703193,,3703232;
después 3703194,,3703233 / 3703193,,3703232.
Frames >33,33 ms: 73/80 → 69/80.
frame combinado 42,6042 → 41,6651; efectos 1,8973 → 0,5487; simulación 14,131 → 14,1983; Update 18,6948 → 17,5262; PostUpdate 6,835 → 6,9303;
[160 muestras](evidence/visual-effect-slot-lookup-steady-20261001.csv),
[160 registros Main](evidence/visual-effect-slot-lookup-steady-phases-20261001.csv).

Primera pan: frame 47,9782 / 48,5239 → 46,3845 / 46,8827;
FPS 20,843 / 20,608 → 21,559 / 21,33;
p95 59,4695 / 62,0081 → 59,3269 / 60,4342;
máximos/p99 126,781 / 128,3038 → 126,403 / 131,6651;
TPS 21,759 / 21,515 → 22,557 / 22,367.
Ticks antes 3703104,,3703143 / 3703103,,3703142;
después 3703104,,3703143 / 3703103,,3703142.
Frames >33,33 ms: 80/80 → 78/80.
frame combinado 48,2511 → 46,6336; efectos 2,0534 → 0,5312; simulación 14,7256 → 14,8226; Update 19,6057 → 17,8847; PostUpdate 8,0172 → 7,9687;
[160 muestras](evidence/visual-effect-slot-lookup-pan-20261001.csv),
[160 registros Main](evidence/visual-effect-slot-lookup-pan-phases-20261001.csv).

Repetición fija: frame 42,8917 / 42,501 → 41,4527 / 41,2286;
FPS 23,315 / 23,529 → 24,124 / 24,255;
p95 56,2653 / 55,475 → 54,2291 / 55,6083;
máximos/p99 59,2068 / 58,1015 → 57,3564 / 58,3848;
TPS 23,334 / 23,537 → 24,114 / 24,232.
Ticks antes 3703193,,3703232 / 3703193,,3703232;
después 3703193,,3703232 / 3703193,,3703232.
Frames >33,33 ms: 74/80 → 70/80.
frame combinado 42,6963 → 41,3407; efectos 1,8829 → 0,5107; simulación 14,1395 → 14,2597; Update 18,7244 → 17,2837; PostUpdate 6,8167 → 6,8579;
[160 muestras](evidence/visual-effect-slot-lookup-repeat-steady-20261001.csv),
[160 registros Main](evidence/visual-effect-slot-lookup-repeat-steady-phases-20261001.csv).

Repetición pan: frame 48,282 / 48,6111 → 46,641 / 46,8002;
FPS 20,712 / 20,571 → 21,44 / 21,367;
p95 61,628 / 62,1512 → 58,7253 / 59,9585;
máximos/p99 129,4556 / 129,0127 → 127,4103 / 126,6132;
TPS 21,645 / 21,483 → 22,437 / 22,344.
Ticks antes 3703103,,3703142 / 3703103,,3703142;
después 3703104,,3703143 / 3703103,,3703142.
Frames >33,33 ms: 80/80 → 78/80.
frame combinado 48,4465 → 46,7206; efectos 2,0289 → 0,5473; simulación 14,8741 → 14,8057; Update 19,556 → 17,9136; PostUpdate 8,0044 → 8,0708;
[160 muestras](evidence/visual-effect-slot-lookup-repeat-pan-20261001.csv),
[160 registros Main](evidence/visual-effect-slot-lookup-repeat-pan-phases-20261001.csv).

La reducción de efectos (~71–74 %) y de Update se repite en todas las tandas;
la mejora de frame es menor. La repetición fija alinea los cuatro ticks;
las otras tandas conservan los runs desplazados indicados, sin declarar que
comparan el mismo estado activo. PostUpdate no mejora uniformemente y la
simulación sigue alrededor de 14–15 ms. Main y CSV básico alinean frame/tick
y sus intervalos suman dentro de 0,001 ms. Main mide el ciclo actual y
frame_ms el intervalo previo: restarlos por fila no mide GPU.
Se alcanzan aproximadamente 24 FPS fijos y 21,4 en pan, con mayoría de
frames sobre presupuesto. No hay 30 FPS ni 37 ticks/s nativos certificados.

Control ce75b321 de la etapa 38 y candidato
607b460e69600c2d56ec18c2f477d954752e2054f633a55f05b4af66838ba498
son inmutables, con ejecutable en /tmp y fuentes exactas antes/después en
target/performance/visual-effect-slot-lookup-20261001. Release incremental
78,5 s total: core 25,11, net 5,24 y cliente 53,19, con solapamiento de
metadata/net. La biblioteca core real del cliente se recompila por visibilidad
de API y pasa de 09d42185 a 6240adc809bbd5e419c8bc2294237452f6d62460e9f201868d4cef9edc9375c3;
no se declara fresh ni se atribuye un nuevo replay de las 61 fases. Reportes
HTML/JSON preservados; no hay desglose frontend/codegen/link ni mejora de
compilación atribuida. Traces gzip y bytes CPU se verifican antes de deduplicar.

Validación: 3.035 tests core/seis ignorados y 1.680 del cliente/dos ignorados;
29 de train_smoke, Clippy de ambos crates/todos los targets, formato, tres
self-tests del comparador, frescura de docs y diff correctos. El primer Clippy
falló por conversiones en la fixture; corregidas y rerun correcto. Un intento
de ejecutar la repetición GPU no arrancó por timeout del auto-review; el mismo
comando reintentado una vez fue autorizado y terminó correctamente.

Se cierra sólo el lookup indexado del contexto visual. F04 completo, caché
CB10/cadencia F07, compositor F08, atribución F31, importación/paridad nativa
general y 30 FPS permanecen abiertos. El fallo Out2x inicial no se oculta ni
se acepta ampliando tolerancias.

## Etapa 40 — Orden estable al retirar proxies segmentados (F08/F18)

Las dos salidas del sorter retiraban los proxies sobrantes mediante
HashMap::into_values. Una semilla nueva de HashMap podía cambiar el orden
del CommandQueue, el reciclado de entidades y los swaps de tablas densas.
Ahora los retiros se ordenan por banda y bits de fuente. Sólo se ordena el
conjunto sobrante, conservando el lookup, creación, sort global/local,
campos de imagen, profundidades y ocultación antes del despawn diferido.
Una escena sin retiros no obtiene una nueva lista de dibujo.

La regresión usa el sistema real, 30 proxies creados en orden mezclado para
tres fuentes/diez bandas, y registra RemovedComponents. Exige una secuencia
estable, cero proxies restantes y que otro frame no repita retiros. Ejecuta
ocho mundos para cada rama: salida sin parents ni candidatos y salida con
un parent ordinario. Antes falla con una secuencia mezclada; después pasan
las 31 pruebas de viewport, incluida ocultación previa/retorno por pan de
las etapas 36–38. Fuentes y logs anteriores/posteriores quedan preservados.
El primer comando de test usó --exact con un nombre incompleto y ejecutó
cero pruebas; el rerun con filtro correcto reproduce realmente el fallo.

Fuente nativa: ViewportDoDraw/ViewportDrawParentSprites del viewport.cpp
prístino de OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888 conservan
secuencias explícitas de parents/children. La sonda geométrica de la etapa 37
sigue acreditando seis bandas, sin añadir aquí una sonda nativa nueva.
OpenTTD no tiene estos proxies ECS ni el mismo allocator; el desempate del
retiro es una decisión interna del port para hacer reproducible su ciclo de
vida, no una equivalencia de IDs o de reciclado con el C++.

Kale congelada, tick 3703074, centro 128,128, 1280×720, clean=0,
settle=180 y colector apagado en ambos: los doce PNG de seis zooms son
exactos, cero píxeles/bloques 4×4 distintos, y streams completos de sort
idénticos bajo renombrado biyectivo de entidades. In2x/Out2x conservan todas
las entradas ordenadas y referencias, bytes CPU de 272/384 imágenes y ambas
máscaras. Dos nuevas parejas Out2x de los mismos binarios son también
exactas. Se conservan tres muestras, sin dispensar campos, permutar filas
ni ampliar tolerancias. Traces gzip y bytes CPU se verifican antes de deduplicar.
[Seis parejas](evidence/segment-proxy-retirement-order-raster-20261001.csv),
[tres parejas Out2x](evidence/segment-proxy-retirement-order-out2-repeats-20261001.csv).

Estas muestras no demuestran que el retiro aleatorio causara los 406 píxeles
de la etapa 39, ni eliminan el contrajemplo de proyección f32 de la etapa 31.
El mismo binario 39 ya produjo capturas distintas: su fallo original y las
variaciones históricas permanecen registrados y abiertos. Hace falta aislar
orden efectivo GPU, cuantización, sampling y comparación nativa sincronizada.
La igualdad de una captura no certifica importación SAV ni paridad general.

Control de la etapa 39, SHA256
607b460e69600c2d56ec18c2f477d954752e2054f633a55f05b4af66838ba498;
candidato b999a0b9f1641329fad963dc217ff01a519f7bd2f9ccabbe571e6c8a1b605e6a.
Ejecutables readonly en /tmp, fuentes exactas en
target/performance/segment-proxy-retirement-order-20261001.
Release incremental 54,29 s total / unidad cliente 54,13; core real del cliente
6240adc8 fresh e idéntico, HTML/JSON preservados. No hay desglose
frontend/codegen/link ni mejora de compilación atribuida. No se mide FPS
activo en esta etapa; siguen las cifras de la 39 (~24 fijo/~21,4 pan).

Validación: 3.035 core/seis ignorados, 1.681 cliente/dos ignorados,
31 viewport, Clippy de ambos/todos los targets, formato, tres self-tests del
comparador, frescura de docs y diff correctos. El primer cliente completo
falló en dos copias por cuota de /tmp (1.679 pasaron); se preserva ese log.
Repetido completo con temporales propios en el repositorio: 1.681/0 fallos.
Para liberar espacio de trabajo se archiva exclusivamente una caché cliente
inactiva del 26/09: 1.342 archivos con SHA256/tamaño verificados antes de
retirar cada archivo original. El tar de 1.001.250.840 bytes y manifiesto en
/tmp permiten restauración; no se modifica una caché activa ni datos de usuario.

Se cierra sólo el orden de retiro de proxies segmentados. F08/F18 completos,
empates y variaciones históricas, paridad nativa, cadencia y 30 FPS siguen abiertos.


## Etapa 41 — Conservar los grupos ordenados de children (F18/F31)

El sistema reconstruía un HashMap y ordenaba todos los grupos de children
cada frame, aunque parent/source_depth no cambiaran. Un Local conserva los
índices por child y parent; sólo ordena los parents afectados por altas,
cambios, bajas o promoción independiente. Sigue consultando todos los
children y calculando cada profundidad contra el parent y ventana vivos:
no conserva una pose antigua ni cambia intervalos, desempates o escrituras.
El orden por source_depth::total_cmp y bits de Entity permanece idéntico.

La regresión diferencial usa Query<Ref<_>> y RemovedComponents reales de
Bevy: 64 children/16 parents y 97 fases con estabilidad, profundidades raw
(incluidos signos de cero y NaN), reparenting, despawn/recreación, retirada y
reposición del componente, y promoción. Compara todos los índices y grupos
con un scan/ordenado completo, exige cero grupos reordenados en estabilidad
y conserva las pruebas existentes de ventanas/parents y transformaciones.

La primera caché no recuperaba mensajes expirados cuando el sistema InGame
no se ejecutaba. Una sonda con el helper exacto y ECS real reproduce dos
entradas cacheadas frente a una viva después de borrar y reemplazar con el
mismo número de entidades. El candidato final comprueba la cantidad tras
insertar altas y elimina IDs sin componente vivo si difiere. La regresión
incluye dos actualizaciones omitidas, reemplazo con igual cantidad y baja
sin reemplazo; la sonda y las 32 pruebas de viewport pasan. La sonda aislada
compila en unos 0,47 s reutilizando Bevy ya compilado: es una ayuda para
iterar ese helper, no una mejora de compilación del programa completo.
Se conservan fuentes/logs del prototipo, fallo y reparación en
`target/performance/viewport-child-groups-20261001`.

Fuente nativa: ViewportSortParentSprites/ViewportDrawParentSprites del
viewport.cpp prístino 14ec60f248547d4d062a1160f0fc26d742319888 mantienen
el bloque parent/children. Se revalida el pin/hash y se reutiliza sólo el
alcance geométrico de la sonda 37; esta etapa no añade un oracle C++ de
callbacks, GPU ni importación. La caché Local es interna del port.

Kale congelada, tick 3703074, centro 128,128, 1280×720, clean=0,
settle=180 y colectores apagados: los doce PNG de seis zooms son exactos,
cero píxeles/bloques 4×4 distintos, y streams completos de sort idénticos
bajo renombrado biyectivo de entidades. In2x/Out2x conservan todas las
entradas ordenadas/referencias, bytes de 272/384 imágenes CPU y ambas
máscaras. Los gzip y bytes se verifican antes de deduplicar. Estos seis
pares no resuelven las variaciones históricas ni certifican el lector SAV.
[Seis parejas](evidence/viewport-child-groups-raster-20261001.csv).

Dos tandas ABBA activas, GPU, escala 2, 40 muestras/run, colector básico y
Main detail en ambas versiones; fijo warmup 120 y pan 30. Sin perf, trazas
ni compilaciones concurrentes. Se conservan 640 muestras básicas y 640
registros Main. Los valores siguientes son ms salvo FPS/TPS; p99 coincide
con máximo con 40 muestras/run.

Primera fija: frame 41,4742 / 41,4709 → 41,2161 / 41,1846;
FPS 24,111 / 24,113 → 24,262 / 24,281; medianas 40,3916 / 41,0069 → 40,8537 / 40,959;
p95 55,0193 / 54,7026 → 54,2264 / 54,4642; máximos/p99 57,496 / 59,0858 → 58,0067 / 56,8373;
TPS 24,118 / 24,124 → 24,241 / 24,275. Ticks antes → después: 3703193..3703232 / 3703193..3703232 → 3703193..3703232 / 3703193..3703232.
Frames >33,33 ms: 68/80 → 67/80.
Medias combinadas: frame 41,4725 → 41,2003; children 0,7998 → 0,6771; sort 4,2103 → 4,2272; simulación 14,2738 → 14,2803; efectos 0,5249 → 0,5123; glass 2,5216 → 2,4718;
Main/Update/PostUpdate: Main 39,1917 → 38,9445; Update 17,3869 → 17,1429; PostUpdate 6,8336 → 6,8342.
[160 muestras](evidence/viewport-child-groups-steady-20261001.csv),
[160 registros Main](evidence/viewport-child-groups-steady-phases-20261001.csv).

Primera pan: frame 47,4785 / 46,6549 → 47,0338 / 49,7833;
FPS 21,062 / 21,434 → 21,261 / 20,087; medianas 44,421 / 43,7206 → 43,9668 / 45,5922;
p95 61,2604 / 60,0188 → 60,2076 / 76,8448; máximos/p99 129,9408 / 129,4817 → 132,8733 / 127,7951;
TPS 22,044 / 22,456 → 22,305 / 20,928. Ticks antes → después: 3703103..3703142 / 3703103..3703142 → 3703103..3703142 / 3703103..3703142.
Frames >33,33 ms: 79/80 → 79/80.
Medias combinadas: frame 47,0667 → 48,4086; children 0,8053 → 0,6745; sort 4,0207 → 4,1288; simulación 14,9088 → 15,3138; efectos 0,526 → 0,5501; glass 2,444 → 2,5079;
Main/Update/PostUpdate: Main 41,7124 → 42,8984; Update 17,9292 → 18,4246; PostUpdate 8,0766 → 8,345.
[160 muestras](evidence/viewport-child-groups-pan-20261001.csv),
[160 registros Main](evidence/viewport-child-groups-pan-phases-20261001.csv).

Repetición fija: frame 41,424 / 41,4924 → 41,0653 / 41,178;
FPS 24,141 / 24,101 → 24,351 / 24,285; medianas 40,7081 / 40,7986 → 40,5782 / 40,9558;
p95 54,9619 / 54,5874 → 54,8019 / 55,188; máximos/p99 57,893 / 57,6978 → 57,0188 / 58,191;
TPS 24,13 / 24,075 → 24,337 / 24,284. Ticks antes → después: 3703193..3703232 / 3703193..3703232 → 3703193..3703232 / 3703193..3703232.
Frames >33,33 ms: 69/80 → 67/80.
Medias combinadas: frame 41,4582 → 41,1217; children 0,7889 → 0,6627; sort 4,152 → 4,1981; simulación 14,3417 → 14,2635; efectos 0,5279 → 0,535; glass 2,4655 → 2,4498;
Main/Update/PostUpdate: Main 39,176 → 38,8681; Update 17,248 → 17,0712; PostUpdate 6,8937 → 6,8218.
[160 muestras](evidence/viewport-child-groups-repeat-steady-20261001.csv),
[160 registros Main](evidence/viewport-child-groups-repeat-steady-phases-20261001.csv).

Repetición pan: frame 46,7155 / 46,2724 → 46,6793 / 46,7357;
FPS 21,406 / 21,611 → 21,423 / 21,397; medianas 43,7981 / 43,4618 → 43,1164 / 43,1755;
p95 60,2539 / 59,9648 → 60,7943 / 59,9806; máximos/p99 126,6997 / 127,1697 → 131,6645 / 131,1749;
TPS 22,389 / 22,625 → 22,472 / 22,436. Ticks antes → después: 3703103..3703142 / 3703104..3703143 → 3703103..3703142 / 3703103..3703142.
Frames >33,33 ms: 77/80 → 79/80.
Medias combinadas: frame 46,4939 → 46,7075; children 0,7782 → 0,6546; sort 4,0251 → 4,0499; simulación 14,7549 → 14,8985; efectos 0,553 → 0,533; glass 2,4065 → 2,4117;
Main/Update/PostUpdate: Main 41,3526 → 41,3475; Update 17,8098 → 17,7174; PostUpdate 8,0175 → 7,9519.
[160 muestras](evidence/viewport-child-groups-repeat-pan-20261001.csv),
[160 registros Main](evidence/viewport-child-groups-repeat-pan-phases-20261001.csv).

Children baja unos 0,12–0,13 ms (~15–16 %) en las cuatro comparaciones.
Los frames fijos bajan sólo ~0,27–0,34 ms y siguen cerca de 24 FPS. Pan no
mejora de forma consistente: la primera tanda contiene un after de 49,7833
ms/p95 76,8448; la repetición también conserva medias y máximos peores.
El segundo control pan repetido está desplazado un tick, sin afirmar que
compara el mismo estado activo. No se descartan picos ni se atribuye toda
la diferencia de frame a esta caché. Main/basic alinean frame/tick y los
intervalos suman dentro de 0,001 ms; Main mide el ciclo actual y frame_ms
el intervalo previo, por lo que restarlos por fila no mide GPU.

Control b999a0b9f1641329fad963dc217ff01a519f7bd2f9ccabbe571e6c8a1b605e6a;
candidato final 41d26421c0e79fbf55a43619726c3aef7d2362efe90499ffbaa57d1a1516039c.
Fuentes y ejecutables inmutables preservados; el final es un archivo readonly
en el directorio propio de trabajo. El prototipo 1d41297c se conserva y no
es el candidato medido. Release final incremental 53,71 s total / unidad
cliente 53,54; core real 6240adc8 fresh e idéntico. HTML/JSON preservados,
sin nuevo replay de 61 fases, desglose frontend/codegen/link ni mejora de
compilación atribuida.

Validación: 3.035 core/seis ignorados (antes del guard final, core no cambia),
1.682 cliente/dos ignorados con el guard final, 32 viewport, Clippy de ambos
crates/todos los targets, formato, tres self-tests del comparador, frescura
de docs y diff correctos. Se conservan los fallos iniciales de visibilidad
del tipo y de la sonda de mensajes expirados antes de las correcciones.

Se cierra sólo la reconstrucción/ordenado de grupos estables y su recuperación
tras avisos expirados. F18/F31 completos, compositor F08, cadencia F07,
paridad/importación nativa, 37 ticks/s y 30 FPS siguen abiertos.


## Etapa 42 — Preparación compartida de recorte retirada (F18/F31)

El experimento preparaba tamaño, rectángulo de textura, bounds y coordenadas
locales una vez por fuente/pasada y reutilizaba esos valores al recortar sus
bandas. Conservaba operaciones y orden de los recortes, soporte/fallback de
rotación, flips y geometría, sin nuevo recurso ECS ni estado entre frames.
**Se retiró el candidato completo**: no mejora el frame y no reduce el sort
consistentemente. El código retenido vuelve byte a byte a c56bbaa6; este
experimento no cuenta como optimización implementada ni cierre de F18/F31.

Un diferencial contra el helper anterior exacto compara 16.000 casos con
imagen, rect, atlas existente/ausente, tamaño personalizado, cinco anclas,
posiciones fraccionarias/signos de cero, escalas admitidas/no admitidas,
rotación y ocho tipos de bandas (disjuntas, cruzadas, vacías, invertidas e
ilimitadas). Todos los campos de Sprite y los bits de Transform coinciden.
La sonda usa Bevy real ya compilado y tarda ~0,53 s en compilar; no reemplaza
la validación del cliente ni acredita una mejora del build completo. El test
y helper quedan sólo en la fuente/probe privada retirada, no en producción.

El pin nativo 14ec60f248547d4d062a1160f0fc26d742319888 y viewport.cpp
SHA256 33511385834e72f002b4a2f8c336d23799707f42b9b4b309ce5d91a4dd4aabe9
se revalidan. AddSortableSpriteToDraw prepara los extents contra dpi y
ViewportDrawParentSprites conserva el bloque de children. Se reutiliza sólo
el alcance geométrico de la sonda 37, sin nuevo oracle GPU/callback C++.

Kale congelada, tick 3703074, centro 128,128, 1280×720, clean=0,
settle=180 y colectores apagados: **cinco de seis parejas iniciales exactas**
y los seis streams completos de sort coinciden bajo renombrado biyectivo.
In2x inicial cambia **204 píxeles / 45 bloques 4×4**, PNG candidato
4e1118f0448939a1ff06af6b151142402a5880541664707ffef680e10be85d54.
272 imágenes CPU y cobertura coinciden; entradas ordenadas/oclusión fallan.
Out2x conserva entradas ordenadas, 384 imágenes y ambas máscaras.
Dos nuevas parejas In2x con los mismos binarios son exactas en todos esos
contratos y recuperan 60da415e. El fallo inicial no se oculta ni se convierte
en 6/6. [Matriz inicial](evidence/prepared-band-clipping-raster-20261001.csv),
[tres parejas In2x](evidence/prepared-band-clipping-in2-repeats-20261001.csv).

La captura fallida y una exacta del mismo candidato tienen fuentes/assets
idénticos, fuentes de mesh únicas y valores bit a bit por fuente canónica;
los multiconjuntos de meshes/cámaras coinciden quitando únicamente sus IDs
propios. Cambia el orden de filas. Ese diagnóstico no dispensa el fallo de
entradas ordenadas ni prueba qué orden/empate GPU decide el resultado. La
variante de PNG ya existía históricamente; su identidad no demuestra causa,
paridad nativa ni que el lector SAV sea correcto en todos los zooms.
[Diagnóstico mismo binario](evidence/prepared-band-clipping-same-binary-in2-20261001.csv).
Los gzip/bytes CPU se verifican antes de archivar/deduplicar.

Dos tandas ABBA activas, GPU, escala 2, 40 muestras/run, básico y Main detail
en ambas versiones; fijo warmup 120 y pan 30. Sin perf, trazas ni compilación
concurrentes. Se conservan 640 muestras básicas y 640 registros Main. Ms
salvo FPS/TPS; p99 coincide con máximo con 40 muestras/run.

Primera fija: frame 41,5023 / 41,0024 → 42,1032 / 41,7209;
FPS 24,095 / 24,389 → 23,751 / 23,969; medianas 40,8705 / 40,4029 → 41,8734 / 41,4522;
p95 55,1805 / 55,247 → 54,8956 / 55,386; máximos/p99 58,7297 / 56,7915 → 61,8892 / 57,5956;
TPS 24,154 / 24,379 → 23,738 / 23,978. Ticks antes → después: 3703193..3703232 / 3703193..3703232 → 3703193..3703232 / 3703193..3703232.
Frames >33,33 ms: 66/80 → 70/80.
Medias combinadas: frame 41,2523 → 41,9121; children 0,6584 → 0,6672; sort 4,2872 → 4,169; simulación 14,2646 → 14,7037; efectos 0,5157 → 0,5299; glass 2,4571 → 2,4909;
Main/Update/PostUpdate: Main 38,9006 → 39,5662; Update 17,1507 → 17,1783; PostUpdate 6,7922 → 6,9775.
[160 muestras](evidence/prepared-band-clipping-steady-20261001.csv),
[160 registros Main](evidence/prepared-band-clipping-steady-phases-20261001.csv).

Primera pan: frame 46,6359 / 46,6772 → 47,8474 / 46,7185;
FPS 21,443 / 21,424 → 20,9 / 21,405; medianas 43,5602 / 43,0727 → 44,4579 / 43,2596;
p95 59,0675 / 60,2821 → 61,8528 / 60,0607; máximos/p99 129,7387 / 130,1399 → 134,3396 / 125,0911;
TPS 22,469 / 22,453 → 21,916 / 22,367. Ticks antes → después: 3703103..3703142 / 3703103..3703142 → 3703103..3703142 / 3703104..3703143.
Frames >33,33 ms: 78/80 → 78/80.
Medias combinadas: frame 46,6566 → 47,283; children 0,6438 → 0,6497; sort 4,0427 → 4,0401; simulación 14,7672 → 15,2884; efectos 0,5384 → 0,5709; glass 2,4089 → 2,4215;
Main/Update/PostUpdate: Main 41,2955 → 42,0379; Update 17,7388 → 17,841; PostUpdate 8,0032 → 8,1079.
[160 muestras](evidence/prepared-band-clipping-pan-20261001.csv),
[160 registros Main](evidence/prepared-band-clipping-pan-phases-20261001.csv).

Repetición fija: frame 41,2915 / 41,4149 → 42,0013 / 41,9429;
FPS 24,218 / 24,146 → 23,809 / 23,842; medianas 40,7232 / 40,4881 → 41,3414 / 41,2462;
p95 54,5236 / 55,0978 → 56,2333 / 56,2599; máximos/p99 57,3767 / 57,0011 → 59,0919 / 58,2072;
TPS 24,212 / 24,024 → 23,829 / 23,84. Ticks antes → después: 3703193..3703232 / 3703194..3703233 → 3703193..3703232 / 3703193..3703232.
Frames >33,33 ms: 66/80 → 70/80.
Medias combinadas: frame 41,3532 → 41,9721; children 0,683 → 0,686; sort 4,1859 → 4,2285; simulación 14,3257 → 14,6942; efectos 0,5385 → 0,5487; glass 2,4811 → 2,4816;
Main/Update/PostUpdate: Main 39,0799 → 39,6423; Update 17,2289 → 17,33; PostUpdate 6,8195 → 6,9258.
[160 muestras](evidence/prepared-band-clipping-repeat-steady-20261001.csv),
[160 registros Main](evidence/prepared-band-clipping-repeat-steady-phases-20261001.csv).

Repetición pan: frame 46,7995 / 46,8156 → 47,0786 / 47,0985;
FPS 21,368 / 21,36 → 21,241 / 21,232; medianas 43,4177 / 43,0811 → 43,9641 / 44,7219;
p95 59,8715 / 60,5474 → 58,7443 / 59,1379; máximos/p99 128,9606 / 129,4124 → 126,7012 / 127,6965;
TPS 22,375 / 22,373 → 22,204 / 22,207. Ticks antes → después: 3703103..3703142 / 3703103..3703142 → 3703103..3703142 / 3703103..3703142.
Frames >33,33 ms: 79/80 → 79/80.
Medias combinadas: frame 46,8075 → 47,0885; children 0,6563 → 0,6624; sort 4,0604 → 4,0117; simulación 14,9203 → 15,1867; efectos 0,5393 → 0,5346; glass 2,4116 → 2,3959;
Main/Update/PostUpdate: Main 41,4353 → 41,7552; Update 17,7226 → 17,7308; PostUpdate 8,0054 → 8,0526.
[160 muestras](evidence/prepared-band-clipping-repeat-pan-20261001.csv),
[160 registros Main](evidence/prepared-band-clipping-repeat-pan-phases-20261001.csv).

El frame fijo empeora ~0,66 y ~0,62 ms en las dos tandas; pan ~0,63 y
~0,28 ms. Sort sólo baja 4,2872 → 4,169 en la primera fija y sube
4,1859 → 4,2285 en la repetición; pan apenas cambia. También cambia la
simulación pese a que core es idéntico: no se atribuye causalmente toda esa
variación al helper. La evidencia no justifica retener su coste/complejidad.
Se conserva el after pan inicial desplazado un tick y el segundo control
fijo repetido desplazado; no se afirma igualdad de estados activos para
esos runs. No se descartan máximos ni se acredita 30 FPS/37 ticks/s.
Main/basic alinean frame/tick y suman dentro de 0,001 ms; su diferencia por
fila no mide GPU porque frame_ms corresponde al intervalo previo.

Control 41d26421c0e79fbf55a43619726c3aef7d2362efe90499ffbaa57d1a1516039c;
candidato retirado a71e0386feade44f5d2ac4fb36a2ec97ab5261d2b1d981db6d346169712b9624,
con fuentes, probe, logs y ejecutables inmutables en
`target/performance/prepared-band-clipping-20261001`.
Release candidato 54,09 s total / cliente 53,91; core real 6240adc8 fresh e
idéntico. HTML/JSON preservados, sin nuevo replay de 61 fases ni mejora de
compilación. La fuente retenida coincide con la etapa 41 ya validada.
El rebuild restaurado tarda 53,48 s y produce exactamente el SHA256
41d26421 de la etapa 41; se preserva su HTML/JSON y un ejecutable readonly.
No se confunde ese rebuild con un nuevo candidato optimizado.

Gates del candidato: 3.035 core/seis ignorados, 1.683 cliente/dos ignorados,
33 viewport incluido el diferencial, Clippy de ambos/todos los targets,
formato, docs y diff. Tras la retirada no se retiene ese test ni ese helper;
el código de producción es el validado en 41 (1.682 cliente/32 viewport).
Los self-tests del comparador y docs/diff se repiten al registrar la etapa.

Además se corrigen dos descripciones obsoletas de F01/F02: la descarga ya
conserva los flows y PendingLinkGraphJob lanza el solver en workers desde
la etapa 2. Las cifras del primer perfil se identifican como históricas,
sin cerrar F03, SAV mutado, recursos de jobs ni otros contratos pendientes.
La evaluación de este experimento concluye como retirada; F08/F18/F31,
cadencia, importación/paridad nativa y el objetivo de jugabilidad siguen abiertos.


## Etapa 43 — Reproducción GPU del empate parent/child (F08/F31)

La prueba CPU 31 demostraba dos profundidades distintas con igual proyección,
pero no leía un buffer GPU. La nueva sonda
[scripts/probe_projected_mask_depth.rs](../../scripts/probe_projected_mask_depth.rs)
lee color y Depth32Float de una pasada Vulkan 8×8 en la RX 7600. Usa los bits
exactos de clip_from_view y GlobalTransform de la cámara de oclusión de la
captura In2x fallida 42, y calcula clip_from_world con el mismo glam/Bevy
0.19.1. El shader multiplica ese uniforme por world_position como
mesh2d_position_world_to_clip. Sustituye XY por un triángulo de cobertura
completa para aislar Z; no incorpora las texturas ni todo el render de Kale.

La fuente capturada contiene exactamente un proxy por profundidad
1075726681 / 1075726728, en el mismo XY. La primera fuente es un parent
sprite 1163 negro en la máscara; la segunda es su child RailGlassMaskSource,
atlas 1258 rojo en la máscara, que referencia aquel parent. Conservan
profundidades world distintas, con parent < child. Los datos e imágenes SHA
se registran en [la pareja real](evidence/projected-mask-depth-source-pair-20261002.csv).
Los IDs identifican esa captura, no un contrato de identidad entre mundos.

El pipeline de la sonda usa el estado de profundidad de Bevy 2D Mask:
Depth32Float, escritura activada, GreaterEqual y MSAA 1. Las lecturas reales
almacenan **1048597585 en ambos planos**, como la proyección CPU. Dibujar
parent→child deja rojo; child→parent deja negro, con los mismos bits de
profundidad. El control a igual Z también gana según el último plano. El
control separado por 0,01 deja rojo en ambos órdenes y almacena valores
1048597585 / 1048597669 distintos. Se leen copias GPU de color y profundidad,
no una estimación Python de la matriz. Dos ejecuciones independientes
cumplen todas las aserciones y producen CSV idéntico:
[24 lecturas](evidence/projected-mask-depth-gpu-20261002.csv).

La GPU informada es AMD Radeon RX 7600, RADV NAVI33, Vulkan/Mesa 26.0.8.
La sonda rechaza adaptadores CPU; no certifica otros backends/hardware.
La compilación directa reutiliza las rlib ya construidas y dura ~0,46 s,
con -D warnings y rustfmt correctos. El primer intento no compiló porque
InstanceDescriptor no implementa Default en wgpu 29; se ajustó a su API
local new_without_display_handle y se preservó ese fallo. Fuentes, matrices,
binario, logs y SHA/tamaños están en
`target/performance/projected-mask-depth-gpu-20261002`.

Reproducción en este checkout con dependencias debug ya compiladas:

```bash
CARGO_MANIFEST_DIR="$PWD/crates/openttdrs-client" \
CARGO_PKG_NAME=openttdrs-client \
rustc --edition=2024 scripts/probe_projected_mask_depth.rs \
  -L dependency=target/debug/deps \
  --extern bevy=target/debug/deps/libbevy-dab1181ebfc551e7.rlib \
  --extern wgpu=target/debug/deps/libwgpu-29980cb70bc93dff.rlib \
  -C linker=clang -C link-arg=-fuse-ld=mold -C opt-level=1 -D warnings \
  -o /tmp/openttdrs-projected-mask-depth-probe
/tmp/openttdrs-projected-mask-depth-probe > /tmp/openttdrs-projected-mask-depth.csv
```

Los hashes de rlib corresponden a este perfil/checkout. Si cambian, usar las
rlib de esa misma compilación; no mezclar variantes de dependencias. La sonda
no activa perf ni cambia configuración del kernel. Su build aislado no es
una mejora de compilación del cliente completo.

Fuentes verificadas localmente: Bevy mesh2d/mesh.rs (Depth32Float,
GreaterEqual, escritura del modo Mask), mesh2d_functions.wgsl (producto de
matrices) y OpenTTD viewport.cpp prístino 14ec60f2,
ViewportDrawParentSprites, que dibuja el parent antes de recorrer sus children.
No se añade aquí una sonda nativa completa ni se cambia el compositor.

Se cierra sólo el sub-issue de observar el empate y su dependencia del orden
en GPU para esta pareja/matriz. No prueba que esos dos planos, sin sus
texturas/vecinos, expliquen los 204 píxeles de Kale ni la variación Out2x.
Falta reconstruir el solapamiento real, medir orden efectivo del compositor,
comparar con el blitter nativo y validar cualquier corrección en seis zooms.
No se amplían tolerancias ni se acredita importación o 30 FPS.

El programa y su binario continúan idénticos a la etapa 41; no se recompilan
ni repiten suites core/cliente ya pasadas porque no cambian esos crates.
Validación nueva: compilación de la sonda con warnings estrictos, rustfmt,
dos ejecuciones GPU con controles/aserciones, tres self-tests del comparador,
frescura de docs y diff. Los 3.035 core/1.682 cliente y 32 viewport de la
versión retenida siguen siendo la evidencia previa, no un rerun de esta etapa.

## Etapa 44 — Solapamiento texturado contra el blitter original (F08/F31)

Se reconstruye la pareja de la etapa 43 con sus bytes RGBA reales: parent
66×52 y el recorte 66×52 del child atlas 1258. Sus SHA256 son
772b0a25c8400ecf9dd32e04c70ea699bd2030cb2bd3dee5839a4d3f24d3967f y
db519d732823da6fb30f9b2c66b857d5342eadc9b5bada7ca066f8c029387f20.
Hay 541/504 píxeles opacos; ambos alpha sólo contienen 0/255. Comparten
17 píxeles opacos, que ocupan 68 píxeles con nearest a 2×. El child opaco
tiene RGB 252/248/128, suficiente para el predicado rojo de la máscara.

[probe_mask_pair_native.py](../../scripts/probe_mask_pair_native.py) extrae
la función Draw completa y sin modificar de 8bpp_optimized.cpp prístino
14ec60f248547d4d062a1160f0fc26d742319888. La función tiene 2.278 bytes y SHA256
527ccdb519631f7df96f7d31cb59167b90c0cb1ab1c0dc29c0ba90668e37f2b3.
El scaffold codifica los alpha binarios escalados en RLE y suministra tipos
mínimos; no sustituye Draw. Dibuja parent Normal y child Transparent con
una tabla artificial que etiqueta vidrio sobre parent/fondo. Se verifica
cada uno de los 13.728 píxeles. Son etiquetas de propiedad, no la paleta
nativa, el decoder de sprites ni un oracle SAV completo.

El orden parent→child de ViewportDrawParentSprites conserva 68 etiquetas
de vidrio sobre parent. El control child→parent las pierde. Dos ejecuciones
C++ producen los mismos buffers y [cuatro filas nativas](evidence/textured-mask-depth-native-20261002.csv).

[probe_textured_mask_pair.rs](../../scripts/probe_textured_mask_pair.rs)
sube los RGBA reales como Rgba8UnormSrgb y usa nearest, máscara alpha 0,5,
Depth32Float, GreaterEqual, escritura activada y MSAA 1. Conserva world XY/Z
y la matriz capturada para Z; sustituye XY por un quad 132×104. Usa el recorte
del atlas con UV 0..1, por lo que no certifica su transformación UV completa,
frustum, vecinos, tonemapping ni color final del compositor.

Con la matriz capturada, parent→child coincide con la propiedad nativa en
todos los píxeles. Child→parent difiere exactamente en los 68 de solapamiento;
ambos planos almacenan 1048597585. El control experimental con offset Z cero
almacena 975312349..975312397 y conserva el ganador nativo en ambos órdenes.
Dos ejecuciones GPU producen las mismas ocho copias de RGBA/profundidad y
[ocho filas GPU](evidence/textured-mask-depth-gpu-20261002.csv). La sonda 43
también se repite después de compartir su helper: sus doce lecturas siguen
idénticas al CSV publicado. Hardware: RX 7600/RADV NAVI33, Vulkan/Mesa 26.0.8;
otros backends quedan pendientes.

El offset cero **no se instala** en la cámara. La misma captura contiene
2.775 sprites de mapa con Z negativo; 203 tienen ViewVisibility e
InheritedVisibility activos. Su rango visible es -97,56..902 y el conjunto
de mapa -98,08..902; proxies y fuentes coinciden en estos recuentos.
[Rango y SHA de la captura](evidence/textured-mask-depth-range-20261002.csv).
Las flags visibles pertenecen a la traza ECS, no certifican cobertura de un
píxel. Una proyección sin offset recortaría esos Z negativos. La corrección
debe conservar el dominio original y validarse sobre la escena completa.

Se cierra sólo el sub-issue de reproducir este solapamiento texturado contra
Draw original y medir su dependencia del orden en GPU. No se atribuyen aún
los 204 píxeles de Kale a esta pareja ni se corrige F08. Faltan el orden
efectivo de la pasada, una corrección de rango completo, el oracle nativo de
escena y seis zooms. Importación, cadencia y 30 FPS permanecen abiertos.

Entradas, C++ generado, ejecutables, lecturas, logs y manifiesto con SHA/tamaños
se conservan en `target/performance/textured-mask-depth-pair-20261002`.
Los scripts reciben ese directorio; el GPU requiere ejecutar primero el
oracle nativo. La compilación Rust usa las rlib de la etapa 43, cambiando el
archivo de entrada a scripts/probe_textured_mask_pair.rs. Compila con
-D warnings; ambos scripts Rust pasan rustfmt, Python compila y el C++ usa
-Wall/-Wextra. Son herramientas aisladas, sin mejora de compilación del cliente.

Validación: dos runs C++/GPU con aserciones por píxel y readbacks exactos,
sonda 43 sin cambios de resultados, tres self-tests del comparador, formato,
docs y diff. Core/cliente y el binario permanecen idénticos a la etapa 41;
sus suites previas (3.035 core, 1.682 cliente, 32 viewport) no se presentan
como reejecutadas. No hay nueva medición de FPS.

## Etapa 45 — Orden de bins y localización del solapamiento (F08/F31)

Se añade un diagnóstico opt-in a la traza de sprites. Con
OPENTTDRS_MAP_SPRITE_TRACE_MASK_ORDER presente, captura los bins AlphaMask2d
de la cámara -101 después de PrepareBindGroups y antes de Render. Conserva
el orden de bins y entidades, claves de pipeline/material y bits de la
matriz de cada instancia. Identifica los mundos por main_entity y las vistas
por sus IDs main/render. Es una lectura de las fases CPU preparadas; no
certifica ejecución de draw calls ni orden entre fragmentos de una instancia.

La solicitud se publica después de capturar las entradas, fuera del ECS
principal, y se recoge en ExtractSchedule. Esto evita que un render del
frame anterior consuma la solicitud siguiente durante pipelined rendering.
FrameCount existente identifica la captura: PostUpdate 180 debe corresponder
a extracción 181, tras Last. Se rechazan frames que no coincidan, falta del
contador y errores de locks. El archivo mask-order.json usa create_new y
flush; no sobrescribe un diagnóstico anterior. La resource adicional sólo
vive en RenderApp y sólo se registra con el flag opt-in.

Bevy 0.19.1 queue_material2d_meshes agrupa Mask por pipeline, draw function,
mesh y material; AlphaMask2dBinKey no contiene Z. El render Direct y Dynamic
Uniforms recorre la secuencia de batchable_meshes. La captura In2x contiene
216 bins/674 instancias; Out2x, 853/9.878. Todas sus instancias enlazan con
los proxies de la traza del mismo frame y conservan cada bit de transformación.
No hay bins multidrawable, unbatchable ni non-mesh en estas capturas.

La pareja texturada 44 conserva los mismos Z y referencia parent/child:
In2x coloca parent en bin 74 y child en 103; Out2x coloca child en 809 y
parent en 813. [Cuatro filas de la pareja](evidence/mask-bin-order-pair-20261002.csv).
El zoom cambia el conjunto visible y la secuencia de materiales, aunque no
cambie la simulación ni la profundidad de esos sprites. La lectura GPU 44
demostró que invertir estos planos empatados cambia su ganador. Este registro
de bins corresponde a capturas correctas actuales; no existe un registro
equivalente de bins para el fallo histórico 42.

También se proyecta el solapamiento 44 usando XY (34,-3926), cámara
(0,-4104), escala 0,5, anchor central y 1280×720. El rectángulo 132×104
empieza en (642,-48), sin buscar un ajuste de imagen. De sus 68 píxeles,
48 quedan dentro del viewport. En la captura fallida 42 esos 48 pasan de
(248,0,0,255) a negro en oclusión; la cobertura conserva sus bytes. Cuarenta
cambian también en el PNG final, entre los 204 del fallo. Ocho conservan
RGB 173/156/107 pese al cambio de máscara; no se infiere la causa de esos
ocho sin medir la composición. [48 lecturas por coordenada](evidence/mask-bin-order-screen-overlap-20261002.csv).
Se localiza así parte de la diferencia en un solapamiento real; no se atribuye
todo el fallo ni se afirma su orden efectivo histórico sin la traza faltante.

Control y candidato conservan PNG y stream completo del sorter en los seis
zooms soportados. In2x/Out2x conservan además todos los campos ordenados de
sprites, proxies, cámaras y assets, 272/384 imágenes CPU y ambas máscaras:
[seis comparaciones](evidence/mask-bin-order-raster-20261002.csv).
Activar la traza sobre el mismo binario vuelve a conservar exactamente esos
datos y píxeles en ambos zooms:
[dos comparaciones off/on](evidence/mask-bin-order-enabled-raster-20261002.csv).
Los IDs se renumeran por biyección; no se ignoran campos ni se comparan sólo
multiconjuntos. Estos runs son regresiones del diagnóstico, no certificación
de raster nativo, importación ni escenarios activos de muchos vehículos.

Binario control 41d26421c0e79fbf55a43619726c3aef7d2362efe90499ffbaa57d1a1516039c;
candidato e4900c927783092231ff313f3118bcf794979bbf89cd2a5a55cfce886741d340,
187.232.376 bytes. Release tarda 53,41 s total / Cargo 53,22; core 6240adc8
se reutiliza fresh y bit-idéntico. No se acredita reducción de compilación.
Fuentes, ejecutables readonly, HTML/JSON de build, capturas, datos, traza inicial
y logs están en `target/performance/mask-bin-order-20261002`. El Clippy inicial
rechazó dos expect; se conserva y se corrigió el manejo de locks sin panics.

Se recuperó espacio de la caché cliente inactiva 1pvk2584qwjxa, escrita por
última vez el 30/09. Sus 1.324 archivos, bytes, permisos y fechas en nanosegundos
se verificaron contra un tar.gz de 1.471.388.000 bytes antes de retirar el
directorio recuperable. El manifiesto y archivo permanecen junto a esta etapa;
la caché activa no se modifica. No es una mejora de velocidad de compilación.

Gates: formato, Clippy core/cliente all-targets, 1.682 tests cliente/dos ignorados,
docs, diff y tres self-tests del comparador. Las suites core 3.035/seis ignorados
son evidencia previa sobre el core idéntico; no se reejecutan aquí. El código
normal de máscaras conserva proyección, profundidad, shader y agrupación.
No hay nueva medición de FPS; siguen ~24,3 fijo/~21,4 pan de la etapa 41.

Se cierra sólo el registro alineado de bins y la localización de los 48/40
píxeles descritos. F08/F31 permanecen abiertos. El siguiente candidato debe
separar profundidades world distintas sin recortar el dominio original ni
alterar empates world verdaderos, y comprobar el resultado con GPU, nativo y
seis zooms. Cadencia, importación y el objetivo de jugabilidad siguen pendientes.

## Etapa 46 — Rango de profundidad conservando clipping: candidato retirado (F08/F31)

Se ensaya asignar un rango por Z world distinto en RenderApp, después de
extract_mesh2d. El tag de cada instancia transporta el rango; los empates
world verdaderos, incluidos -0/+0, comparten tag. Un vertex shader compatible
con SpriteMaterial conserva geometría, UV, fragment shader, cámara y bins,
pero escribe un valor Depth32Float representable entre 0,25 y 0,5. Antes de
reemplazar Z conserva la decisión de clipping original. El prototipo deja
todo el pase en su dominio anterior si encuentra planos inclinados o Z no
finito. No crea MeshTag ni otras entidades/resources en el mundo principal.
**El candidato se retira; este código/shader no queda en producción.**

La sonda texturada 44 incorpora la misma función del vertex candidato y
ocho casos en ambos órdenes. Con los Z capturados, almacena 1048576001 y
1048576002: child conserva la propiedad nativa en los 13.728 píxeles, en
ambos órdenes. Repite el resultado con -97,56/-97,5596 y con los rangos
máximos, cuyos bits son 1056964606/1056964607. El control world igual conserva
un solo valor y la dependencia del orden anterior. Los planos bajo y sobre
el clipping original no escriben ningún píxel ni profundidad; sus 4.112
diferencias frente al blit sin frustum son esperadas por el control de recorte.
No se confunden con una regresión nativa de escena.

Dos runs GPU son exactos en sus 32 buffers RGBA/profundidad. Después de
retirar el código se conserva sólo la función experimental en
[probe_mask_rank_mapping.wgsl](../../scripts/probe_mask_rank_mapping.wgsl),
usada por [la sonda](../../scripts/probe_textured_mask_pair.rs). Un tercer
run confirma los mismos 16 casos y buffers; los controles capturado/sin
offset también siguen idénticos a 44. [48 lecturas GPU](evidence/ranked-mask-depth-gpu-20261002.csv).
Hardware: RX 7600/RADV, Vulkan/Mesa 26.0.8. No certifica otros backends,
geometrías, estados o el resultado de todo el compositor.

Una prueba aislada del módulo real del candidato, compilada contra las rlib
de Bevy con -D warnings, verifica la pareja capturada, negativos, los empates
0 y repetición de Z, y orden de los tags. Pasa un test; el módulo/test queda
preservado como experimento y no se agrega a la suite retenida del cliente.
Compilación aislada ~0,39 s y sonda ~0,51; no son tiempos del cliente completo.

La escena completa conserva sorter, todas las entradas ordenadas, 272/384
imágenes CPU y cobertura en In2x/Out2x; cambia la oclusión. Diferencias de
PNG frente al control en seis zooms: 0/3.464/346/1.068/1.407/219 píxeles,
0/421/56/275/502/115 bloques 4×4.
[Seis regresiones del candidato](evidence/ranked-mask-depth-raster-20261002.csv).
Los seis PNG after son **bit-idénticos al candidato Transparent2d retirado
en 31**. Separar los Z mediante depth test reproduce aquel resultado sin
su cola ordenada ni pérdida del dominio de clipping. Esto acota el ensayo;
no demuestra que el orden world del port sea correcto frente al nativo.

La comparación con los PNG nativos históricos 31 repite sus resultados:
In2x gana 2.618 exactos, pierde cero y deja 846 sin coincidencia; normal
gana 331/cero/15. Out2x gana 147, **pierde 114** y deja 807; Out4x,
265/**44**/1.098. La alineación procede de cámaras (+8/+4/+2/+1 píxeles Y),
sin búsqueda de ajuste. Out8x mantiene desplazamiento fraccionario
(-85,75,-42,125) por clamping nativo y no cuenta coincidencias exactas.
[Comparación nativa y alcance](evidence/ranked-mask-depth-native-raster-20261002.csv).
La referencia es CLEAN=0, sin certificar ticks, vehículos, UI o animaciones
sincronizados; se mantiene ese límite. Las pérdidas no se dispensan y el
prototipo no se instala por mejorar sólo In2x.

Control e4900c927783092231ff313f3118bcf794979bbf89cd2a5a55cfce886741d340;
candidato retirado 744eb4d0174dd428e9cb1283a70a1d1d94baedcb8e2d1fc2342f9549d9ff6a53,
187.452.792 bytes. Release candidato tarda 54,38 s total / Cargo 54,20; core
6240adc8 fresh e idéntico. Fuentes, shader, binarios readonly, trazas, buffers,
casos nativos perdidos y logs se conservan en
`target/performance/ranked-mask-depth-20261002`.

Tras verificar las copias, se restauran exactamente las fuentes 45. El rebuild
53,45 s / Cargo 53,07 produce el mismo SHA e4900c92 del control. Se conserva
otro ejecutable readonly y el HTML/JSON. No es una mejora de compilación.
No se ejecutan benchmarks activos de un cambio ya rechazado ni se acredita
30 FPS; permanecen las cifras de la versión 41.

Gates del candidato: Clippy cliente all-targets, formato, test aislado del
módulo, dos runs GPU, seis capturas completas y comparación nativa. No se
ejecuta la suite completa del candidato tras rechazar su raster. Gates de la
sonda retenida: rustc con warnings estrictos, rustfmt, tercer run GPU idéntico,
tres self-tests del comparador, docs y diff. Las 1.682 pruebas cliente/dos
ignorados y 3.035 core/seis ignorados son evidencia previa de las fuentes
restauradas, no reruns de esta etapa.

Se cierra la evaluación como retirada. F08/F31, muestreo/composición/orden
nativo, cadencia, importación y jugabilidad siguen abiertos. El siguiente
paso obtiene una referencia nativa pausada sin activar CLEAN ni ocultar los
vehículos, con metadatos de estado, antes de atribuir las pérdidas restantes.

## Etapa 47 — Referencia nativa pausada con capas conservadas (F08/F31)

El exportador permite `OPENTTDRS_WORLD_SCREENSHOT_FREEZE=1` independiente de
CLEAN. Pausa justo después de cargar el SAV solicitado, antes de los callbacks
que estabilizan la cámara. CLEAN=0 mantiene etiquetas, vehículos y opciones
normales del viewport. El wrapper conserva FREEZE=0 por defecto; este cambio
no altera el cliente ni su compositor.

La referencia registra tres fases: after_load, before_raster_queue y
after_raster. Incluye tick, flags de pausa/display/transparencia/invisibilidad,
conteos de vehículos/ocultos/estaciones y una huella FNV-1a de ID, tipo,
posición XYZ, dirección y vehstatus de cada entrada del pool nativo. **La
huella no es el hash canónico del juego**, no incluye todos los campos del
vehículo ni certifica RNG, importación o todas las animaciones/UI.

[El verificador](../../scripts/check_frozen_screenshot_state.py) rechaza fases
faltantes/duplicadas/fuera de orden, campos inválidos, CLEAN=1, pérdida de
pausa normal, comandos durante pausa o cambios en los campos registrados.
La primera captura conserva un rechazo por comparar todos los bits de pausa:
67→65 es la limpieza nativa del bit SaveLoad, con Normal siempre activo.
Se admite únicamente conservar la máscara inicial o limpiar ese bit; no se
omite el resto del control de estado. Se añaden seis regresiones del parser.

En ocho capturas pausadas —seis zooms más repeticiones In2x/Out2x— las tres
fases mantienen tick **3703074**, display=239, transparencia/invisibilidad=0,
**3505 entradas del pool**, 328 con Hidden y 245 estaciones. La huella es
14730810999729983964 en todas las fases/runs. El pool nativo incluye entradas
auxiliares: no se equipara este total con las 3293 unidades importadas del
fixture. Las dos repeticiones son byte-idénticas. Cuatro controles FREEZE=0
comparan el ejecutable instrumentado con el original en In2x/Out2x y también
son exactos. [Doce capturas y metadatos](evidence/native-frozen-full-scene-captures-20261002.csv).

**Los seis PNG pausados son byte-idénticos a los nativos históricos de 31.**
La comparación con el candidato retirado 46 repite exactamente sus ganancias,
pérdidas y listas de píxeles: In2x 2618/0/846, normal 331/0/15, Out2x
147/**114**/807 y Out4x 265/**44**/1098 (gana/pierde/ninguno). La alineación
se deriva de la cámara; Out8x conserva desplazamiento fraccionario por
clamping y no cuenta coincidencias exactas.
[Seis comparaciones](evidence/native-frozen-full-scene-raster-20261002.csv).
Congelar el estado no elimina las pérdidas: el candidato sigue retirado.
Esta coincidencia de capturas no certifica todo el orden, importación,
composición o gameplay del port.

Se instrumenta temporalmente el checkout nativo existente, preservando sus
fuentes y binario antes del cambio. El build CMake completo tarda 279,85 s:
no es una mejora de compilación del cliente. El ejecutable del ensayo es
2898d5abfe6a143f85bfc2d378a90426a64ced9a53b9ed80568b32abff9a9720;
el original fdae5e3c386733f17a53850b415ee43966ee66ee2dedc45a7e84b4e051379a5a
queda restaurado junto con la fuente byte a byte antes de validar. Sólo se
publican el parche del exportador, herramientas, regresiones y evidencia;
reference/ no se añade. Runtime aislado, fuentes readonly, ejecutables,
rechazo inicial, PNG, logs y drivers permanecen en
`target/performance/native-frozen-full-scene-20261002`.

Gates: seis tests del verificador, cinco contratos de fuente del exportador,
las doce capturas/control reales, comparación de seis zooms, sintaxis shell,
formato, docs y diff. Las suites core/cliente anteriores se reutilizan por
mantener las fuentes del juego sin cambios. No hay benchmark activo nuevo
ni avance acreditado hacia 30 FPS. Se cierra sólo el sub-issue de referencia
pausada con campos registrados estables. F08/F31 siguen abiertos; el siguiente
paso debe aislar muestreo y oclusión de vidrio frente al blitter nativo en
Out2x/Out4x, donde separar Z no alcanza.

## Etapa 48 — Muestreo nearest frente al blitter en seis zooms (F08/F31)

La referencia pausada 47 conserva las pérdidas de 46. Se aísla ahora el
muestreo de las dos máscaras reales 66×52, sin depth test ni otros sprites.
[La sonda nativa](../../scripts/probe_mask_sampling_native.py) extrae Draw
prístino de 8bpp-simple y ScaleByZoom/UnScaleByZoom del pin 14ec60f2.
Un scaffold replica el alpha normal 4× para formar el root; no sustituye Draw.
El canvas es 288×232 y el origen de ambas máscaras está alineado en (8,8).
No ejecuta decoder/Encode/GfxBlitter completo, atlas, offsets/subsprite ni SAV.

El blitter avanza por texeles enteros desde el origen y redondea hacia arriba
el tamaño de destino: 264×208, 132×104, 66×52, 33×26, 17×13 y 9×7.
Dos ejecuciones verifican exactamente los doce mapas de ocupación contra
los índices esperados. [24 mapas nativos](evidence/mask-sampling-native-20261002.csv).
Fuente Draw SHA 7eacfbeed029d3abe38d042c7a56650448c1efc14535b3afce4ff6357d7e36c7;
helpers de zoom b25cfcce253d7987fed5ce43ff9ec865beb1774c35de0333f378e8ddeaa67864.

[La sonda GPU](../../scripts/probe_mask_sampling_gpu.rs) usa las texturas
RGBA capturadas, nearest, Rgba8UnormSrgb, Mask >=0,5 y MSAA=1. No agrega
oclusores, LUT ni Z. El control nearest toma muestras del centro del píxel.
Conserva ocupación exacta en In4x, In2x y normal. En Out2x/Out4x/Out8x,
parent difiere en **128/52/11 píxeles** y child en **93/23/7**.
La diferencia aparece sin empates de profundidad ni orden entre entidades.

Se ensayan dos controles sólo en la sonda. Para escalas s>1 se desplaza la
muestra (1-s)/2 texeles: lleva el centro GPU al centro del texel elegido por
el paso nativo; escalas s<=1 conservan el muestreo inicial. Eso elimina las
diferencias de child y Out2x parent. Quedan 2/3 píxeles parent en Out4x/Out8x:
el borde derecho de su quad fraccionario no llega a los centros finales.
Redondear el tamaño de destino hacia arriba y ampliar las UV proporcionalmente
corrige también esos casos. Esta tercera variante coincide en los 66816
píxeles de cada uno de los doce mapas nativos.

Dos runs de 36 casos GPU conservan exactamente sus 36 buffers RGBA y los
resultados CSV. [72 lecturas GPU](evidence/mask-sampling-gpu-20261002.csv).
RX 7600/RADV, Vulkan/Mesa 26.0.8; se rechaza adaptador CPU. Los inputs y
la cámara artificial alineada no prueban otras geometrías, clipping, fases
fraccionarias, atlas o backends. **No se instala ningún ajuste en el cliente.**
No se atribuyen a esta sonda las 114/44 pérdidas de la escena completa.

La primera invocación referenció un directorio de inputs inexistente y el
scaffold abortó sin raster; sus fuentes/binario se conservan. La herramienta
ahora valida tamaño y alpha binario antes de compilar, y usa los inputs
verificados de 46. Sondas, fuentes readonly, hashes, compilaciones, casos,
buffers y fallo inicial quedan en `target/performance/mask-sampling-20261002`.
Compilación GPU aislada 0,57 s con -D warnings; ambos C++ con warnings como
errores. No son mejoras de compilación del juego.

Gates: dos runs nativos/GPU, controles de ocupación, rustfmt de sondas,
formato de workspace, self-tests del comparador, docs y diff. Cliente/core
sin cambios; se reutilizan las suites anteriores. No hay nuevo benchmark
activo. Se cierra sólo la reproducción de muestreo alineado y dimensiones;
F08/F31, 30 FPS y jugabilidad siguen abiertos. El siguiente sub-issue debe
probar la fase real de las coordenadas/UV y clipping de la escena antes de
combinar muestreo nativo con la separación de profundidad.

## Etapa 49 — Raíces nativas cargadas y bancos de zoom reales (F08/F31)

Antes de aplicar el ajuste aislado 48, se verifican los inputs que realmente
usa 46. Out2x no usa la misma textura que In2x: su atlas es
`tiles_atlas_0_out2.png`, y su imagen parent recoloreada también repite la
reducción nativa. Frente al alpha normal, parent cambia 481 texeles y conserva
684 opacos; child cambia 270 y conserva 516. **Ambos buffers Out2x completos
son exactamente la réplica por pasos de dos de sus RGBA capturados normales.**
Por tanto, la sonda con textura normal 48 no mide directamente el muestreo de
la escena que ya seleccionó un banco preparado.

Se instrumenta temporalmente GetSprite después de cargar Kale, con pausa 47
y 8bpp-simple, para extraer la raíz codificada real de los IDs 1163/1167.
Ambas miden 264×208 (54912 bytes). Los offsets nativos son (-124,-80) y (0,0).
Sus bytes se repiten exactamente en dos runs y ambos PNG coinciden con la
referencia pausada Out2x, sin cambiar las tres muestras del estado registrado.
El alpha de cada raíz cargada coincide en sus 54912 píxeles con la máscara
normal capturada expandida 4×. Se valida así aquella hipótesis del scaffold
48 para estos dos sprites; no para todos los decoders, IDs o paletas.

[La nueva sonda](../../scripts/probe_mask_zoom_assets.py) comprueba además el
atlas normal y Out2x/Out4x/Out8x. Los ocho crops conservan exactamente los
RGBA generados por tomar el primer texel de cada bloque y repetirlo en su
huella. Conteos de alpha opaco normal/2/4/8: parent 541/684/592/528,
child 504/516/656/640. Cada prueba se repite con el segundo dump nativo.
[16 comprobaciones](evidence/mask-native-assets-20261002.csv).
Esto prueba raíz/alpha/bancos, no la posición, UV de atlas en GPU, clipping,
paleta nativa, oclusión ni compositor de todo el mapa. No se instala el ajuste
48 y no se atribuyen las pérdidas 114/44.

La lectura de fuentes acota otra brecha: TileAtlas::build,
native_zoom_image_for_capture y native_zoom_bytes_for_capture seleccionan
estas variantes sólo con OPENTTDRS_MAP_SHOT; sync_native_map_sprite_position
limita también el redondeo a capturas. La función pura devuelve None cuando
capture_requested=false, incluso en 2/4/8. Es una limitación explícita del
código: **la captura con correcciones nativas no certifica por sí sola el
camino visual de una partida interactiva**. La validación de performance
retiene sus resultados runtime; no se convierte una captura en un benchmark
ni se extrapola esta diferencia a todos los FPS.

Native root SHA parent 4a98e8200f12935cd3a1eb3cfd923389f7d7f7875f729b19e572790e21b25773;
child 408c37da2facd68b429111d2860cd11eb762679d22c32c2efc3b3754835915c9.
El rebuild nativo incremental tarda 4,46 s. Se preserva un ejecutable propio
sin símbolos de debug antes de ejecutarlo: 95738344 bytes, SHA
914925c22fe8fe2e70b5342e6f340cfa6e3b1de055eaa51a106381d39c1d1b50.
Fuente y binario originales vuelven byte a byte antes de validar; reference/
no se publica. Dumps, fuentes readonly, ejecutable, banco Out2x capturado,
PNG, estado, logs y hashes se conservan en
`target/performance/mask-native-assets-20261002`.

Gates: dos extracciones nativas reales, dos runs de ocho crops, estado 47,
formato, self-tests del comparador, docs y diff. Cliente/core sin cambios;
se reutilizan las suites anteriores. No se mide FPS nuevo. Se cierra sólo
la comprobación de raíces/bancos de esta pareja. F08/F31 y jugabilidad
permanecen abiertos. El siguiente sub-issue hará explícita y medible la
selección de correcciones nativas de zoom en la captura, conservando el
control actual, para probar también la ruta sin esas correcciones.

## Etapa 50 — Selector explícito de correcciones de captura (F08/F31)

OPENTTDRS_MAP_SHOT_NATIVE_ZOOM=0 desactiva juntos el banco del atlas, las
variantes de recoloreados/NewGRF y el redondeo especial de posición. También
acepta false/off/no; el valor ausente conserva el comportamiento anterior.
El wrapper expone OPENTTDRS_WORLD_SCREENSHOT_NATIVE_ZOOM, predeterminado 1.
El selector requiere MAP_SHOT: no habilita variantes en una partida normal.
No desactiva pausa, cámara, resolución ni las demás reglas del driver.

Veinte capturas con ejecutables inmutables: seis control/candidato,
seis con el candidato desactivado y dos habilitados explícitamente.
Los seis PNG predeterminados y streams completos son exactos tras renumerar
entidades; In2x/Out2x conservan todas las entradas, 272/384 imágenes CPU y
ambas máscaras. Los dos controles explícitos también son exactos.
[14 comparaciones](evidence/capture-native-zoom-mode-raster-20261002.csv).

Desactivar las correcciones conserva los tres zooms cercanos. En Out2x,
Out4x y Out8x cambia **373546/668899/574099 píxeles** y
43588/50806/43910 bloques 4×4. En Out2x cambian imágenes, entradas y ambas
máscaras, conservando el stream del sorter. En Out4x cambian también los
parents incluidos: 12390 frente a 12406; no se afirma un stream igual.
Estos contrastes miden el conjunto de correcciones, no el aporte individual
de textura o posición ni equivalencia con toda la sesión interactiva.
No prueban que una de las dos rutas reproduzca el compositor nativo completo.

Gates: cliente **1683 pasados/2 ignorados**, Clippy estricto de todos los
targets de cliente/core, formato, sintaxis del wrapper, tres self-tests del
comparador, docs y diff. El core no cambia; se reutiliza su suite previa.
Release 54,69 s de pared/54,63 de Cargo; core fresh con SHA retenido 6240adc8.
Cliente SHA ac807968ef7d7a1f7cae9171f1eaef990b9ec0e4c1a307721be9e840f85af606,
187202768 bytes. No se atribuye una mejora de compilación o FPS.

Fuentes, ejecutables, trazas, imágenes, máscaras, logs, timing y hashes se
conservan en `target/performance/capture-native-zoom-mode-20261002`.
El primer driver abortó por SyntaxError antes de lanzar el cliente; se
conserva y se corrige su delimitador. Se cierra sólo el selector y su control
de regresión. F08/F31, importación, 30 FPS y jugabilidad siguen abiertos.
La siguiente etapa vuelve al perfil de CPU de Kale activa con el cliente
vigente, para elegir el siguiente coste medido del frame.

## Etapa 51 — Perfil actual de CPU activa y pausada (F07/F24/F31)

El cliente inmutable 50 (ac807968) se ejecuta con Kale, GPU real,
1280×720 y escala 2, sin MAP_SHOT ni compilación concurrente. Cada run
conserva 240 frames tras warmup 120. Perf cpu-clock:u, 199 Hz y DWARF 32768
empieza sólo después del flush 60; se conserva el ACK del FIFO.
Activo: 2188 samples, 7456,730 ms entre primera/última muestra, cero perdidos.
Pausado: 853, 3023,791 ms, cero perdidos; tick fijo 3703074.
Se conservan 480 frames y 50 filas de hojas IP:
[frames instrumentados](evidence/current-client-residual-frames-20261002.csv),
[CPU por hoja](evidence/current-client-residual-perf-20261002.csv).

Activo incluye máscaras (3,43 % agregado por símbolo), query paralela de
visibilidad (2,93 %), sort de parents (2,38 %) y extracción Mesh2d (2,06 %).
Son porcentajes de CPU muestreada del proceso y sus threads, no del frame.
Las pilas children siguen sin aportar callers fiables; se usa la IP sampleada.
MCF también aparece, pero sus muestras están en TID 529917, distinto del
principal 529899. No se infiere bloqueo del frame por esa presencia.
El thread principal concentra 10,01 %; sus hojas incluyen movimientos de
filas ECS, swap_remove y despawns. No se atribuyen todos a un único productor.

Pausado: visibilidad paralela 6,57 %, sorter puro 2,23 %, máscaras 2,11 %,
Mesh2d 2,11 % y VehicleOperationalSummary::analyze 1,99 %.
El HUD técnico arranca oculto, pero update_tile_info_text no comprueba su
visibilidad antes de construir la clave y analizar la flota. Es el siguiente
coste acotado que se corregirá, conservando la actualización al mostrarlo.
La presencia de layout UI tampoco cuantifica por sí sola una mejora posible.

Frames bajo perf: media activa 41,5846 ms, p95 53,045, máximo 70,5977;
216/240 superan 33,33 ms, ticks 3703193–3703432. Pausada 16,6802 ms,
p95 17,3369, máximo 19,1261; cero excedidos. No son una ABBA sin perf,
una mejora de FPS ni una resta que aísle el coste de simulación: cambian
también movimiento, remap y estado de la partida.

Logs, FIFO, fuentes del driver, trazas con PID/TID/IP, binario y hashes quedan
en `target/performance/current-client-residual-20261002`. Fuentes del juego
sin cambios; se reutilizan los gates 50 y se ejecutan docs/diff.
Se cierra sólo la renovación de este perfil. F07/F24/F31, 30 FPS y
jugabilidad siguen abiertos; continúa la corrección del trabajo del HUD oculto.

## Etapa 52 — Evitar preparar el HUD técnico oculto (F24)

update_tile_info_text retorna antes de consultar texto/cámara o preparar
resúmenes si HudVisibility.visible=false. Al mostrarlo vuelve a leer todos
los datos actuales. Sin ese recurso conserva el comportamiento anterior,
como setup_tile_info_ui. La regresión usa un sistema persistente: ocultar,
mostrar, ocultar y cambiar flota/idioma/cámara, y volver a mostrar. Exige texto
y pose intactos sin flags Changed mientras está oculto, y alerta/pose actuales
en el primer frame visible. Las pruebas existentes conservan el HUD visible.

Dos tandas ABBA, GPU real, Kale activa y escala 2, sin perf ni compilación
concurrentes; ambos clientes usan colector básico y detalle Main. Fijo,
warmup 120: frame combinado **41,6865 → 41,2274 ms** y
**41,6117 → 40,9046 ms**; FPS 23,989 → 24,256 y 24,032 → 24,447.
Update baja 17,2377 → 17,1049 y 17,1677 → 16,9519 ms. También varía la fase
de simulación aunque el core no cambió; no se atribuye toda la reducción del
frame al HUD. Primera candidata 1 cubre 3703194–3703233, los otros runs fijos
3703193–3703232. Sobre 33,33 ms: 69/80 → 66/80 y 68/80 → 68/80.
[Primera fija](evidence/hidden-hud-work-steady-20261002.csv),
[segunda fija](evidence/hidden-hud-work-repeat-steady-20261002.csv).

Pan, warmup 30: **47,3034 → 46,6833 ms** y **47,2270 → 46,3362 ms**;
FPS 21,140 → 21,421 y 21,174 → 21,581. Los ocho runs cubren
3703103–3703142; sobre 33,33 ms permanece 79/80 y 78/80 en cada pareja.
Máximos candidatos 130,337/128,645 y 128,952/129,209 ms. La mejora pequeña
no elimina picos ni acredita 30 FPS o la cadencia nativa. TPS de la segunda
tanda: fijo 24,206/23,841 → 24,469/24,413 y pan 22,166/22,178 →
22,683/22,551; el intervalo de TPS excluye el primer frame de cada run.
[Primera pan](evidence/hidden-hud-work-pan-20261002.csv),
[segunda pan](evidence/hidden-hud-work-repeat-pan-20261002.csv).
Los cuatro CSV de fases asociados conservan 640 intervalos completos Main,
con el mismo frame/tick y suma exacta dentro de 0,001 ms.

Doce capturas: seis PNG y streams completos exactos, cero píxeles/bloques
distintos; In2x/Out2x conservan entradas, 272/384 imágenes CPU y ambas máscaras.
[Oracle de regresión](evidence/hidden-hud-work-raster-20261002.csv).
Esto conserva el render anterior; no cierra la paridad nativa ni certifica
por captura el camino interactivo señalado en 49/50.

Gates: **1684 tests cliente/2 ignorados**, Clippy estricto core/cliente,
formato, comparador, docs y diff. Core sin cambios, suite anterior reutilizada.
Release 53,81 s de pared/53,76 Cargo; core fresh 6240adc8. Cliente readonly
SHA e6aa215cdbf5849b7b76e15ccaeb5a0701a8163a3785e0f3b5eb0aa89a53b4d6,
187207320 bytes. No se atribuye una mejora de compilación.
Artefactos y hashes: `target/performance/hidden-hud-work-20261002`.

Se cierra sólo el trabajo del HUD oculto. F24 visible, F07/F17/F31, 30 FPS
y jugabilidad siguen abiertos. Continúa la atribución del trabajo ECS
diferido, especialmente rematerialización de chunks, antes de elegir su cambio.

## Etapa 53 — Medir la cola diferida del remapeo (F17/F31)

OPENTTDRS_REMAP_TRACE_OUT habilita un CSV exclusivo. Dos comandos sin estado
ECS rodean la cola propia de apply_remap_map_visuals y miden su aplicación.
La preparación registrada empieza después de resolver el viewport; no es
todo el intervalo Remap anterior. Los conteos de bajas cubren sus visuales
directos, no todas las etiquetas/proxies retirados por otros sistemas.
El test exige salida sólo después de aplicar la cola, baja efectiva y ningún
componente registrado por las marcas. El modo ausente no encola marcas.

Kale activa, GPU real, escala 2, dos runs fijos y dos con pan; 40 frames/run,
warmup 120/30. La selección usa el rango de ticks de cada CSV de frames,
no un ID de frame inexistente en la traza. Fijo: 40 remapeos/run,
160839 bajas directas y 108 chunks refrescados, sin entradas/salidas de chunks.
Preparación media 2,5322/2,6268 ms; aplicación **2,5960/2,6533 ms**,
máximos 6,7340/6,9253. Pan: 39 remapeos/run, 174289 bajas, 127 refrescos
y 17 chunks añadidos; preparación 2,9921/3,0085, aplicación
**2,8823/2,8964 ms**, máximos 8,0469/8,5109. Sin rebuilds completos en
las cuatro ventanas. Ese tramo de aplicación quedaba fuera de Phase::Remap.
[446 filas, incluido startup/warmup](evidence/remap-command-cost-commands-20261002.csv),
[160 frames instrumentados](evidence/remap-command-cost-frames-20261002.csv),
[160 intervalos Main](evidence/remap-command-cost-phases-20261002.csv).
Las latencias instrumentadas no son una ABBA de mejora de FPS.

Catorce capturas: seis controles predeterminados exactos y dos off/on
In2x/Out2x exactos, con entradas completas, 272/384 imágenes CPU y ambas
máscaras. Streams completos conservados tras renumerar entidades.
[Seis zooms](evidence/remap-command-cost-raster-20261002.csv),
[selector off/on](evidence/remap-command-cost-enabled-raster-20261002.csv).
Conservan el render previo; no certifican paridad nativa general.

Gates: **1685 tests cliente/2 ignorados**, Clippy estricto core/cliente,
formato, comparador, docs y diff. Se corrige el uso inicial de unwrap/expect
del nuevo test mediante Result; fallo preservado. Core sin cambios, suite
anterior reutilizada. Release 53,80 s de pared/53,76 Cargo; core fresh 6240adc8.
Cliente readonly 187319528 bytes, SHA
adcbd9cd9e21b1fb9666754cb0a9cbbc8e802276db50da6ca3d6b5b64bf9cee1.
Artefactos: `target/performance/remap-command-cost-20261002`.

Para recuperar espacio se comprimen dos snapshots nativos propios de 47,
verificando bytes/hash completos antes de retirar sus copias sin comprimir.
Los gzip y registros de recuperación quedan junto a los originales; se
recuperan 465408955 bytes. El binario de reference/ permanece intacto.
No se atribuye una mejora de compilación o FPS a este mantenimiento.

Se cierra sólo la atribución de esta cola. Continúa una prueba de bajas
agrupadas, conservando su orden, antes de cambiar la granularidad de chunks.
F07/F17/F31, 30 FPS y jugabilidad siguen abiertos.

## Etapa 54 — Bajas agrupadas: variante retirada (F17/F31)

Se agrupan únicamente las bajas directas de chunks incrementales. La variante
conserva EntityCommand::despawn, el handler warn y world.flush tras cada baja,
como CommandQueue de Bevy 0.19.1. Su diferencial cubre cascada de hijos,
entidades ya retiradas, observadores con comandos diferidos, creación posterior
y las mismas entidades/generaciones. Pasa, pero agrupar no evita el trabajo
interno de despawn. Fuentes y binario candidatos quedan preservados; la
producción vuelve a 53, byte por byte, sin una nueva ruta de bajas.

ABBA sin perf ni sonda de comandos, GPU real, Kale activa, escala 2,
40 frames/run y colectores básico/Main en ambas versiones. Fijo warmup 120:
**40,8141 → 40,8976 ms**, 24,501 → 24,451 FPS; 67/80 fuera de 33,33 ms
para ambas. Update 16,8117 → 16,9484 ms. Todos cubren 3703193–3703232.
Pan warmup 30: **46,5574 → 47,1633 ms**, 21,479 → 21,203 FPS;
78/80 → 79/80 fuera de presupuesto, picos candidatos 129,750/128,572 ms.
Todos cubren 3703103–3703142. No hay mejora sostenida que justifique retenerla.
[Frames fijos](evidence/batched-remap-despawn-steady-20261002.csv),
[frames pan](evidence/batched-remap-despawn-pan-20261002.csv); los dos CSV
asociados de fases conservan 320 intervalos completos de Main.

Cuatro runs adicionales con marcas: fijo 40 remapeos y 160839 bajas en
ambos, preparación 2,5425 → 2,6241 ms y aplicación **2,6575 → 2,6719 ms**.
Pan selecciona 39 registros en ambos; aplicación 2,9294 → 2,9478 ms,
pero la candidata empieza un tick después y retira 171821 frente a 174289
visuales. Esa pareja no representa idéntico trabajo por tick y no permite
atribuir su diferencia al agrupado. Sin rebuilds completos.
[447 registros, incluido warmup](evidence/batched-remap-despawn-commands-20261002.csv),
[160 frames instrumentados](evidence/batched-remap-despawn-trace-frames-20261002.csv),
[160 intervalos Main](evidence/batched-remap-despawn-trace-phases-20261002.csv).

Doce capturas: seis PNG y streams completos exactos; In2x/Out2x conserva
entradas completas, 272/384 imágenes CPU y ambas máscaras.
[Oracle de regresión](evidence/batched-remap-despawn-raster-20261002.csv).
Conserva el renderer anterior, sin acreditar paridad nativa general.

Gates candidatos: **1686 tests cliente/2 ignorados**, Clippy estricto
core/cliente, formato, docs y diff. Se corrige el import inicial de un módulo
privado de Bevy; fuente y diagnóstico conservados. Core sin cambios, suite
anterior reutilizada. Release 53,951 s de pared/53,91 Cargo; core fresh
6240adc8. Candidata 187269616 bytes, SHA
34db832654a59c9b20144e395ee8046355e981b507f5c7c8552ca664bc885154.
Fuente y release activo restaurados exactos a adcbd9cd de 53. Artefactos,
drivers, hashes y decisión: target/performance/batched-remap-despawn-20261002.
Cuatro snapshots propios inactivos se conservan como gzip, con recuperación
completa verificada; se recuperan 498188524 bytes en total. No se atribuye
una mejora de FPS o compilación a esa conservación.

Se cierra sólo este ensayo fallido. Continúa la reducción del alcance del
redibujado: una tesela alterada no debería retirar todos los productores de
un chunk cuando sus vecinos no cambiaron. F07/F17/F31, 30 FPS y jugabilidad
siguen abiertos; ningún umbral ni golden cambia.

## Etapa 55 — Señales por tesela y divergencia de redibujado (F17/F31)

Se ensaya sustituir únicamente los chunks afectados por señales por sus
teselas productoras. El filtro admite culling y la base vanilla canónica
ogfx1_base.grf, estática, ff4f5401 y sin parámetros; otros GRF activos
conservan el redibujo anterior. Coalescea causas y no duplica una tesela si
su chunk ya se reconstruye. El test compara Sprite, Transform, parent/child
del productor y exige conservar entidades vecinas. **Variante retirada**:
no mejora sostenida y el diferencial forzado expone una divergencia de
representación. Las siete fuentes y el release activo vuelven exactamente a
53; prototipo, diagnósticos y capturas quedan conservados.

ABBA sin perf ni sonda, GPU real, Kale activa, escala 2 y 40 frames/run,
colectores básico/Main en ambas versiones. Fijo, warmup 120:
**40,5711 → 41,0559 ms**, 24,648 → 24,357 FPS; 66/80 → 68/80 frames
exceden 33,33 ms. Todos cubren 3703193–3703232. Pan, warmup 30:
**45,9180 → 46,5900 ms**, 21,778 → 21,464 FPS; 78/80 en ambas,
picos candidatos 130,831/134,840 ms. Todos cubren 3703103–3703142.
[Frames fijos](evidence/tile-signal-refresh-steady-20261002.csv),
[pan](evidence/tile-signal-refresh-pan-20261002.csv), y sus dos CSV de fases.

Cuatro runs con sonda: fijo 40 remapeos, 160839 → 159449 bajas y
108 → 107 refrescos; aplicación **2,5737 → 2,7417 ms**. Pan 39 registros
seleccionados en 3703103–3703141, 174289 → 171041 bajas y 127 → 125
refrescos, 17 entradas en ambas; aplicación **2,8509 → 2,8912 ms**.
Sin rebuilds completos. La menor cantidad no basta para mejorar tiempo;
la señal aislada cubre poco trabajo en esta ventana.
[446 registros, incluido warmup](evidence/tile-signal-refresh-commands-20261002.csv),
[frames instrumentados](evidence/tile-signal-refresh-trace-frames-20261002.csv),
[intervalos Main](evidence/tile-signal-refresh-trace-phases-20261002.csv).

Las doce capturas normales conservan exactamente seis PNG y streams;
In2x/Out2x conserva inputs, 272/384 imágenes CPU y ambas máscaras.
[Regresión predeterminada](evidence/tile-signal-refresh-raster-20261002.csv).
No se activaba ninguna señal dirty durante esa pausa. Por eso se añaden
doce capturas forzadas del mismo candidato: en frame 90 se encolan todas las
teselas con señales, por chunk o por tesela, sin modificar el mapa desde la
sonda. Las trazas permanecen en tick 3703074; esto no acredita un hash completo
de estado/RNG. Los seis casos fallan la igualdad de stream. PNG cambia en
3680/1908/3815/4511/0/28 píxeles y 230/177/424/730/0/8 bloques 4×4 para
escalas 0,25/0,5/1/2/4/8. La ruta por tesela conserva los seis PNG previos;
la reconstrucción por chunk revela diferencias existentes en esa ruta.
[Diferencial forzado](evidence/tile-signal-refresh-probe-raster-20261002.csv).

En las trazas completas de In2x y Out2x, la ruta por chunk contiene
**451/1564 parents estáticos adicionales**, todos sprites 1406, 1407 o 4626:
faroles y árboles de banquina. Children, vidrio y proxies mantienen cantidades.
Es un indicio concreto para revisar el detalle dependiente de zoom de chunks
reutilizados. Los bytes CPU se comparan por índice en la tabla de assets:
su fallo con orden/cantidad distintos no demuestra que el decoder cambie
los píxeles de una misma imagen. La lectura del sprite y la composición
por zoom siguen siendo contratos separados; un único zoom dentro de tolerancia
no certifica ambos.

La nueva [sonda versionada](../../scripts/profile_tile_dirty_causes.rs) ejecuta
200 ticks del core sin callbacks visuales del cliente. Se compila con
rustc -D warnings contra el rlib fijado 6240adc8; una repetición reproduce
los bytes de sus 1850 filas. Deduplica coordenadas dentro de cada causa/tick,
no entre causas. Totales: industria 295, paisaje 377, señales 246 y reservas
932. En la ventana fija hay 59 avisos de industria, **52 Airport**, frente a
17 señales. El cliente ya anima radar/bandera retenidos, mientras step_airport_tiles
añade esos avisos al remapeo general. Será otra investigación acotada tras
la consistencia del zoom; este conteo headless no mide FPS ni certifica replay.
[Avisos por causa y tesela](evidence/tile-signal-refresh-dirty-causes-20261002.csv).

Gates candidatos: **1687 tests cliente/2 ignorados**, Clippy estricto
core/cliente, formato, docs y diff. Se conservan los fallos iniciales de
comparación de Sprite y del tamaño mínimo de culling y sus correcciones;
ningún umbral de producción cambia. Core sin cambios, suite anterior reutilizada.
Release 53,621 s de pared/53,58 Cargo, core fresh 6240adc8. Candidata
187245320 bytes, SHA 88c0ea0a0b00b435fd76e6fe5afe7e69a954f6bd8b11dc9e88e3e0209a4d0dba.
Restauración guardada por bytes/SHA a adcbd9cd de 53. Fuentes, drivers, HTML,
trazas, hashes y decisión: target/performance/tile-signal-refresh-20261002.
El candidato inactivo de 54 se conserva en gzip con recuperación verificada;
cinco caches cliente anteriores se conservan en tar.gz tras verificar bytes,
modos y mtimes. Sus tamaños lógicos no se presentan como espacio físico
recuperado, porque pueden contener hardlinks compartidos.

Se cierra sólo este ensayo. Continúa la consistencia de detalle al variar zoom,
seguida de la animación retenida de aeropuertos. F07/F17/F31, 30 FPS y
jugabilidad permanecen abiertos.

## Etapa 56 — Detalle vial coherente al cambiar zoom (F08/F17/F31)

Se conserva **sólo la corrección de presencia del detalle vial**. OpenTTD
DrawRoadBits retorna antes de faroles/árboles si zoom > ZoomLevel::Detail,
y Detail = Out2x. El cliente ya implementaba ese corte al crear sprites,
pero reutilizaba chunks completos de otro zoom. El camino sin culling ni
siquiera inspeccionaba el cambio de escala. Ahora cruzar Out2x/Out4x solicita
una reconstrucción de representación; los cambios de escala dentro del mismo
nivel de detalle conservan el camino incremental donde corresponde.
No cambia el decoder, los bytes de mapa ni la simulación.
Oracle de fuente fijado a 14ec60f2: road_cmd.cpp L1669–1671,
zoom_type.h L44 y las tablas de faroles/árboles road_land.h.

La regresión reproduce primero el fallo: mapa 16², escala 4, conserva seis
detalles que deben estar ausentes. Después cubre ida/vuelta en 16² y 256²,
dos faroles/cuatro árboles con road bits 5, bytes completos del mapa intactos
y las mismas entidades de vehículos. En la primera suite paralela otros
tests modifican atomics globales de transparencia y ocultan los árboles;
se conserva el fallo y se ejecuta esta regresión en un proceso propio.
**1686 tests cliente pasados/2 ignorados**, Clippy estricto core/cliente,
formato, docs y diff. Core sin cambios, suite anterior reutilizada.

Doce capturas nuevas, seis por defecto y seis con
OPENTTDRS_MAP_SHOT_REDRAW=full en frame 90, pausadas en tick 3703074,
1280×720, centro 128,128, frame 180, CLEAN=0. El driver sólo pide redibujo;
no se acredita un hash completo del estado/RNG. Frente al renderer 53 los
seis PNG cambian en 3680/4152/3972/5282/0/0 píxeles para
escalas 0,25/0,5/1/2/4/8; el CSV es la fuente de cantidades exactas y bloques.
[Diferencial previo/nuevo](evidence/zoom-road-detail-refresh-raster-20261002.csv).
Es una corrección funcional, no un nuevo golden de paridad nativa completa.

Default/redibujo conserva **124/126/48** parents de sprites 1406/1407/4626
en In2x y **516/514/661** en Out2x: el detalle ya no depende de esa
reconstrucción. Conserva bytes de 265/368 imágenes CPU y ambas máscaras.
In4x/In2x conserva PNG y stream global completo. **El gate general de
redibujado falla**: Normal cambia 8 píxeles/4 bloques pese a stream igual;
Out2x 18/16, Out8x 27/7; Out4x conserva PNG pero cambia stream.
Out2x mantiene sus 3733 parents globales y cambia 76/749 proxies locales,
incluidos bounds Z y profundidad. No se atribuyen esos fallos al decoder.
Los inputs Main estrictos difieren tras reconstruir, incluso en In2x con
PNG exacto; su primera diferencia es Z de un mesh. Estos contratos siguen
abiertos en F31 y **no se certifica estabilidad general del renderer**.
[Redibujado forzado](evidence/zoom-road-detail-refresh-redraw-raster-20261002.csv).

ABBA, Kale activa, GPU real, escala 2, 40 frames/run, básico/Main en ambas.
Fijo warmup 120: **40,8294 → 42,2877 ms**, 24,492 → 23,648 FPS;
67/80 → 71/80 sobre 33,33 ms. Todos cubren 3703193–3703232.
Vidrio 2,4896 → 3,7176 ms: ahora hay más detalle vial correcto presente.
Pan warmup 30: **46,5223 → 53,4220 ms**, 21,495 → 18,719 FPS;
78/80 → 80/80 y pico candidato 250,537 ms. El primer control cubre
3703104–3703143; los otros 3703103–3703142, por lo que no toda pareja
es idéntica por tick. La escala se fija en frame 30, justo en el límite de
warmup pan: el rebuild y trabajo diferido de la transición pueden entrar
en esa ventana. No se presenta como pan ya estabilizado ni mejora de FPS.
[Frames fijos](evidence/zoom-road-detail-refresh-steady-20261002.csv),
[pan](evidence/zoom-road-detail-refresh-pan-20261002.csv), y sus CSV de fases.
El coste extra y la transición permanecen como trabajo de rendimiento.

Release 72,708 s de pared con compilaciones de tests simultáneas; no es una
medición aislada de compilación ni se atribuye una mejora. Core fresh 6240adc8.
Cliente readonly 187371320 bytes, SHA
3895d6940a9d14ab34f2455dd3e45fc3c16c5312468cd9c995a6b708c1682133.
Se reutiliza tras la reparación cfg(test) porque todas las fuentes de
producción permanecen byte idénticas; ambas versiones del test se conservan.
El primer intento GPU sin acceso al compositor falla NoCompositor; se
conserva y se repite la misma prueba con acceso Wayland/GPU. El candidato
retirado de 55 queda gzip con recuperación completa verificada, recuperando
142059684 bytes. Artefactos, fuentes, drivers y decisiones:
target/performance/zoom-road-detail-refresh-20261002.

Se cierra únicamente el subcaso de presencia del detalle al cruzar zoom.
F08/F17/F31, estabilidad tras redibujar, 30 FPS y jugabilidad continúan
abiertos. Sigue separar la animación vanilla de aeropuertos del remapeo
general, conservando otros cambios y callbacks NewGRF.

## Etapa 57 — Animación aeroportuaria sin reconstruir chunks (F17/F28)

Los radares y mangas vanilla ya tienen un overlay retenido que lee `m7`,
actualiza sprite/ancla y conserva la profundidad asignada por el sorter.
Sin embargo, cada frame de animación también pedía reconstruir su chunk.
Ahora la fase original publica `airport_tile_animation_dirty`, separado de
los avisos generales. El cliente aprovecha el overlay sólo con la pila
canónica estática `ogfx1_base.grf` (GRFID ff4f5401, sin parámetros) o vacía.
Una entrada custom activa o parámetros conservan el remap; una entrada
inactiva no lo necesita. Industria, paisaje, señales, reservas visibles,
construcción/demolición y otras causas en la misma tesela o un vecino siguen
solicitando su reconstrucción. No se agregan componentes, recursos ni
parámetros de query ECS.

Oracle de fuente OpenTTD 15.3, pin 14ec60f2: station_cmd.cpp dibuja la
secuencia del frame vivo; AnimateAirportTile delega en AnimationBase, que
marca dirty sólo cuando cambia el frame. Las tablas airporttiles.h tienen
12 frames de radar a velocidad 2 y 4 de manga a velocidad 1.
**Se conserva la cadencia actual del port, un avance cada 3 ticks.**
El original usa 4/2 ticks: es una divergencia independiente abierta en F28,
no corregida ni certificada por esta optimización.

La regresión del núcleo cubre step normal/perfilado, `m7`, delta transitorio,
RNG y serialización: el aviso no entra en el JSON del runtime; flows/jobs
sí conservan su contrato persistido. La regresión cliente cubre las causas
concurrentes y fallback NewGRF. Otra prueba ECS recorre 12 valores de `m7`
en radar, manga y torre legacy; comprueba sprite, ancla, entidad estable,
visibilidad y Z del sorter. Pasan **3036 tests core/6 ignorados**, **1688
cliente/2 ignorados**, 36 pruebas cliente aeroportuarias y ambos Clippy
estrictos. Los dos primeros intentos de suite cliente fallaron por espacio
(OS 28 en /home, cuota OS 122 en /tmp); se conserva su salida. Cinco cachés
core inactivas quedan comprimidas con todos sus bytes, modos y mtimes
verificados antes de recuperar espacio. La misma suite pasa después sin
modificar fuentes.

La sonda headless pública `scripts/profile_tile_state_hashes.rs` compara
11 hashes canónicos muestreados durante 200 ticks y todos coinciden con 56.
Son hashes del estado persistido del núcleo, no replay de callbacks cliente.
La sonda de causas conserva los 1850 avisos y todos sus campos al normalizar
sólo la clasificación nueva: 268 Airport pasan a `airport_animation`, los
27 Industry continúan como generales; señales, paisaje y reservas coinciden.
[Estados muestreados](evidence/retained-airport-animation-state-hashes-20261002.csv)
y [causas](evidence/retained-airport-animation-dirty-causes-20261002.csv).

Ocho capturas nuevas, reutilizando las seis referencias inmutables de 56,
conservan PNG y stream geométrico completo en 0,25/0,5/1/2/4/8, pausadas
en tick 3703074, 1280×720, centro 128,128, frame 180, CLEAN=0.
La petición adicional 0,125 se limita al mínimo real de cámara 0,25; no
acredita un séptimo zoom nativo. En 0,5/2 coinciden todos los inputs Main,
265/368 imágenes CPU y ambas máscaras. Son capturas congeladas; la
regresión ECS cubre el avance de frames. No se reabre como verde el gate
general de reconstrucción que falló en 56.
[Raster y contratos](evidence/retained-airport-animation-raster-20261002.csv).

ABBA Kale activa/GPU real, escala 2, 40 frames/run, básico y Main en ambas
versiones, sin compilaciones concurrentes. Fijo warmup 120:
**42,5100 → 39,8969 ms**, **23,524 → 25,065 FPS**; 73/80 → 70/80 frames
sobre 33,33 ms. Remap 2,5774 → 2,0812 ms; simulación 14,2237 → 14,2053.
El primer control cubre 3703194–3703233, los otros 3703193–3703232.
Pan warmup 30, junto a la transición de escala:
**54,0063 → 50,7618 ms**, **18,516 → 19,700 FPS**, 79/80 → 80/80 fuera
de presupuesto; pico 267,425 → 247,461 ms. Primer control un tick después
que los otros, como en fijo. No se llama pan estabilizado ni éxito a 30 FPS.
[Frames fijos](evidence/retained-airport-animation-steady-20261002.csv),
[pan/transición](evidence/retained-airport-animation-pan-20261002.csv) y
sus CSV de fases conservan los tiempos completos.

La segunda ABBA fija confirma **42,3865 → 40,3266 ms**,
23,592 → 24,798 FPS, 73/80 → 72/80 fuera de presupuesto.
El pan con warmup 120, después de finalizar la transición, registra
**44,9776 → 42,9170 ms**, 22,233 → 23,301 FPS, 78/80 → 78/80 y
pico 64,396 → 63,512 ms. Separa la ventana estable de la transición;
ambas mantienen el objetivo de 30 FPS por frame abierto.
[Confirmación fija](evidence/retained-airport-animation-confirm-steady-20261002.csv)
y [pan estabilizado](evidence/retained-airport-animation-confirm-pan-settled-20261002.csv).

Con la sonda propia de remap, en la misma ventana fija de 40 ticks bajan
161195 → 122637 bajas de visuales; la cola propia suma 113,605 → 82,487 ms.
Continúan 40 remapeos: hay otras causas por tick. Pan registra 201592 →
159709 bajas y 131,678 → 108,340 ms de aplicación en 39 remapeos. Es
atribución instrumentada; preparación comienza tras resolver el viewport y
las bajas cuentan sólo los estáticos directos, no todo el trabajo de ECS.
[Cola propia](evidence/retained-airport-animation-trace-commands-20261002.csv).

Release aislada: 79,264 s de pared, recompilando core/net/cliente.
Core e049d933; cliente readonly 187373048 bytes, SHA
b44751794298419b9c6e54a379810242bf4f073114f5586cc27817e63c0ec822.
Fuentes fijadas, binarios, drivers, fallos y decisiones:
target/performance/retained-airport-animation-20261002.
La ganancia se limita a la clasificación y representación retenida vanilla.
Cadencia nativa, otros remaps, estabilidad general del compositor, 37 ticks/s,
30 FPS por frame y jugabilidad completa siguen abiertos.
