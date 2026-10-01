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

## Trabajo restante

- F03/F21: grafos por componente, estadísticas nativas de producción y SAV
  de flows/jobs tras mutaciones; acotar/cancelar trabajo pendiente (F02).
- F04, F09–F11, F13: resolver NewGRF a demanda y corregir variables relativas.
- F05–F06: snapshots tipados de red y hash sin árbol JSON completo.
- F07–F08, F17–F18: medir tick residual, compositor y cámara con raster en seis zooms;
  aislar la variación Out8x entre ejecuciones del mismo cliente con flota.
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
