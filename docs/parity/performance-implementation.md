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
