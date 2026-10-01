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
