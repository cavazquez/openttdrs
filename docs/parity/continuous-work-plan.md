# Plan continuo de paridad OpenTTD ↔ openttdrs

Este es el orden operativo para cerrar las brechas sin declarar paridad por una
prueba interna aislada. El plan se actualiza después de cada bloque publicado.

## Regla de ejecución

Cada bloque debe tener una divergencia reproducible, un cambio acotado, una
prueba diferencial o un fixture que lo cubra y una nota en la matriz de paridad.
Antes de empezar el siguiente bloque se ejecuta:

```bash
cargo fmt --all -- --check
cargo clippy -p openttdrs-core --all-targets -- -D warnings
cargo clippy -p openttdrs-client --all-targets -- -D warnings
cargo test -p openttdrs-core
cargo test -p openttdrs-client --bin openttdrs-client
./scripts/check_parity_docs_fresh.sh
git diff --check
```

El bloque termina sólo con `git commit` y `git push`. La captura raster se usa
cuando hay compositor WGPU; si el entorno no lo permite, se registra el bloqueo
y se conserva la evidencia headless, sin convertirla en una afirmación visual.

## Handoff de issues — 2026-09-08

### Lote acotado solicitado: Road/Rail, SAV y UI

La instrucción más reciente limita esta entrega a tres sub-issues. Después
de validarlos y publicar cada commit se detiene el trabajo; no se activa el
siguiente bloque del ciclo continuo en esta entrega.

1. #532 cerrado y publicado en `d97db526`: techo de velocidad vial en
   curvas/reversa según el modelo de aceleración. Evidencia y límites en
   [road-curve-speed-model.md](road-curve-speed-model.md).
2. #533 cerrado y publicado en `f35b1850`: persistencia y chequeo de
   refinerías de `oil_refinery_limit`.
   Evidencia y límites en [sav-oil-refinery-limit.md](sav-oil-refinery-limit.md).
3. #534 implementado y validado en esta etapa: localización reactiva ES/EN
   de la ventana de horarios y captura reproducible en seis zooms.
   Evidencia y límites en [ui-timetable-window.md](ui-timetable-window.md).

#330, #328 y #331 conservan sus alcances generales abiertos. La fundación
automática de Oil Rig diagnosticada antes de este lote se corrigió después en
#531/RMAP-170: `CHECK_OIL_RIG` usa altura norte y límite persistido, y las 52
teselas especiales de agua se validan sin materializarlas. La traza de 171
jornadas vuelve a ser exacta; los límites del corte están en
[runtime-oil-rig-placement-531.md](runtime-oil-rig-placement-531.md).

RMAP-171 corrige la siguiente frontera: una industria incompleta debe ejecutar
`MakeIndustryTileBigger` durante su propia visita LFSR y tomar el `Random()`
incondicional del cambio de etapa antes de árboles y pueblos posteriores. La
fixture `autosave0.sav` queda exacta durante 181 jornadas; su frontera
histórica de `day[210]` queda resuelta por RMAP-172. La evidencia y límites
están en
[runtime-industry-construction-tile-loop-rmap-171.md](runtime-industry-construction-tile-loop-rmap-171.md).

RMAP-172 materializa `BuildOilRig` en la misma visita LFSR que completa la
pieza norte `GFX_OILRIG_1`; basta que exista su pareja sur de la misma
industria, aunque siga en obra. Así la estación neutral participa en el
`AcceptanceTick` del tick actual y conserva el stream global. Con el oracle
OpenTTD 15.3 y `autosave0.sav`, `initial` y `day[0]`…`day[232]` son exactos;
RMAP-173 resuelve la frontera siguiente: el cierre mensual ordena industrias
por `IndustryID` sparse, igual que `Industry::Iterate()`, en vez de usar el
orden físico de importación. La misma fixture alcanza `day[239]` sin
diferencias. Evidencia y límites en
[runtime-oil-rig-station-timing-rmap-172.md](runtime-oil-rig-station-timing-rmap-172.md)
y [runtime-monthly-industry-pool-order-rmap-173.md](runtime-monthly-industry-pool-order-rmap-173.md).

RMAP-174 descarta en la propagación runtime de árboles las teselas cuyo
fallback semántico sea `Grass` pero cuyo nibble `MAPT` no sea `MP_CLEAR`: el
objeto de `(121,171)` de `autosave0.sav` ya no se convierte en bosque ni
desplaza `day[332]`. RMAP-175 completa la siguiente frontera mensual:
`FindSubsidyPassengerRoute` consume el selector TPE, valida el pueblo origen
antes de sortear destino y usa `RandomRange` sobre el pool, no módulo. El
oracle y Rust alcanzan ahora `initial` + `day[0]`…`day[359]` exactos en reloj,
RNG, `ECMY`, `ITBL`, pool y acciones. La evidencia y límites están en
[runtime-tree-object-fallback-rmap-174.md](runtime-tree-object-fallback-rmap-174.md)
y [runtime-passenger-subsidy-rng-rmap-175.md](runtime-passenger-subsidy-rng-rmap-175.md).

Incidencia de validación ajena al lote: al cerrarlo, #535 registraba que
`test_parity_docs_portability.py` copiaba el baseline raster del 2026-09-05
mientras el checker exigía el del 2026-09-07 desde `021f023b`. La corrección
posterior deriva ahora la fixture desde `check_raster_baseline.BASELINE`, para
que los casos limpio y corrupto sigan el mismo baseline que el gate. El gate
directo y las diez pruebas aisladas —con y sin `rg`— pasan localmente; el
workflow **Parity docs** de esta publicación verificará la misma suite.

Verificación del lote: core 2.287 tests unitarios y sus integraciones
aprobados; cliente 1.190 aprobados/2 opt-in ignorados; formatter y Clippy
core/client sin warnings. Oráculo vial 128/128, re-save SAV nativo con
límite 48 y control de seis fases exacto por teselas/bloques4×4/estado.
Se ejecutó el cliente y se revisaron doce capturas de horarios (dos idiomas,
seis zooms), además del mapa local en los cuatro zooms de alejamiento.
La evidencia detallada y las limitaciones están en los tres documentos
anteriores. Tras publicar esta etapa UI finaliza el lote solicitado.

### Frontera runtime anterior al lote

Actualización runtime: RMAP-163 / #524 deja exactos los 24 cortes iniciales
de `autosave0.sav` al alinear el cierre mensual y la actividad global de
estación. RMAP-165 / #526 reconstruye al cargar las caches urbanas derivadas
de `MP_HOUSE` y aplica la cadencia de `larger_town`: con una corrida nativa
dedicated válida, ciudad 22 alcanza su intento en `1474722` en ambos lados.
RMAP-164 / #525 elimina el gate duplicado de estación/financiación de
`TownTickHandler` y conecta el walker vanilla con el RNG global. RMAP-166 /
#527 elimina la frontera posterior: la cola ordenada `ANIT` y el ciclo bajo de
`MAPE` (`None → Animated → Deleted → None`) preservan el slot si un
`TileLoop_Town` reactiva el ascensor antes del siguiente pase; obra runtime no
toma RNG extra y la renovación reutiliza la misma palabra `r`. La corrida
integrada posterior alcanzó **360 jornadas** en RMAP-175 y la extensión
dedicated de #527 sobre la misma fixture verifica **400** (`initial` +
`day[0]`…`day[399]`). Las regresiones cubren la renovación de una huella
vanilla 2×2 y una Large Office protegida. Para NewGRF, Action0 `0x16` se
parsea y limita a los seis bits nativos, persiste en el catálogo y la ruta
runtime `TryBuildTownHouse` programa el temporizador de cada subtesela nueva;
Action0 `0x19` también se conserva: el crecimiento runtime omite casas
históricas y marca como protegida toda la huella de un spec protegido. Los
flags de sincronización de CB1B ya se ejecutan al vencer el temporizador:
tras la randomización Action2, la rama no sincronizada consume una palabra
sólo cuando su máscara coincide; la sincronizada consume la palabra compartida
por huella y pasa sus 16 bits altos a cada callback, cuyos 16 bits bajos son
propios. `0xFD`/`0xFE`/`0xFF`/frame aplican la semántica nativa de ANIT. Action0
`0x1A`/`0x1B` conserva además frames, loop/no-loop y la velocidad limitada
nativa `2..=16` (con defaults `NoAnimation`/`2`), junto con las máscaras
CB1A/CB1B/CB20. En la pasada urbana compartida `ANIT`, CB20 se evalúa antes
del gate de cadencia y limita su resultado a `0..=16`; al vencer `2^speed`,
CB1A toma una palabra RNG sólo con el flag declarado y aplica su frame,
fallback loop/no-loop o `DeleteAnimatedTile` con limpieza diferida. La
subsecuencia urbana de `ANIT` queda persistida e importada en un vector común:
conserva el orden relativo de ascensores vanilla y casas NewGRF al regrabar un
SAV y los snapshots que aún llaman al campo `active_house_lifts` se leen
mediante alias. El frame NewGRF cambiado entra además en la lista dirty del
cliente sin alterar la secuencia de los ascensores. Al vencer el timer,
`NewHouseTileLoop`
ya resuelve Action2 de
`TileLoop`/`TileLoopNorth`: conserva los triggers pendientes, aplica sólo la
máscara de reseed y comparte el resultado norte en huellas multitile, con el
consumo global por tesela nativo. Luego rearma el período, preserva
`AnimatedTileState` y marca dirty. Incluso cuando ese timer sólo se decrementa,
`TileLoop_Town` continúa una casa NewGRF en obra: cada rollover de etapa marca
la subtesela y, si publicó su máscara, CB1C toma una palabra RNG nueva,
recibe `param2 = 0` y aplica la misma semántica `ChangeAnimationFrame` sobre
ANIT. La llamada inaugural de `BuildTownHouse` (`param2 = 1`) sigue pendiente.
#527 permanece abierto para esa llamada inicial, CB21 de destrucción y sonidos
de animación NewGRF; no amplía el cierre de #512.
La evidencia canónica vive sólo en
[random-map-issues.md](random-map-issues.md#rmap-164--atribuir-la-divergencia-de-expansión-urbana-posterior-al-cierre-mensual).

RMAP-167 / #528 restaura `PATS.economy.type` y evita ejecutar la rama diaria
original sobre una partida `ET_SMOOTH`; su corte, regresiones y la siguiente
frontera pendiente se mantienen únicamente en
[random-map-issues.md](random-map-issues.md#rmap-167--respetar-economytype-al-importar-sav-y-ejecutar-producción-vanilla).
No cierra #512 ni amplía la paridad de runtime fuera de esa fixture.
RMAP-168 / #529 conserva además `PATS.economy.town_growth_rate`, su semántica
`0…4` y la cache `Town::num_houses` al recalcular la cadencia mensual: elimina
la frontera RNG de `day[168]` y deja exactos los cortes hasta `day[169]`. La
nueva frontera de fundación industrial de `day[170]` sigue abierta. RMAP-169 /
#530 restaura además `TileLoop_Clear` de los campos en la partida regular y
reduce el residuo raw previo a esa fundación sin cambiar el RNG; sus conteos,
frontera restante y alcance residual de #527 viven sólo en
[random-map-issues.md](random-map-issues.md#rmap-169--despachar-tileloop_clear-para-campos-durante-una-partida-regular).

Última etapa: RMAP-143 / #346 amplía el gate por fases a RNG y secuencia
ID/posición de pueblos, además de bytes de teselas; las 30 fronteras de la
cohorte 64²→512² pasan. Evidencia y alcance en
[random-map-issues.md](random-map-issues.md#rmap-143--gate-de-estado-rng-y-secuencia-de-pueblos).
#336 completa después la comparación de población/casas con RMAP-144 / #348:
las cuatro semillas temperate/default 512² y el control 64²/128²/256²
coinciden por entidad además de tiles/RNG. Su cierre queda limitado a esa
cohorte; #338 y los padres de worldgen conservan la generalización pendiente.
La evidencia canónica está en `docs/parity/evidence/rmap-144.json` y
`random-map-issues.md`. RMAP-142 / `712ec4ba`
evita candidatos obsoletos y conserva hash/procedencia de los binarios.
RMAP-145 / #360 agrega la cohorte Toyland 512² de la seed `1330935381`: las
seis fronteras son exactas por bytes raw, bloques 4×4, RNG y demografía de
pueblos. El alcance y los pools que aún no observa quedan únicamente en
`random-map-issues.md` y `evidence/rmap-145.json`; #338 sigue abierto.
RMAP-146 / #361 corrige una divergencia de cache que no alteraba el raster:
al sustituir una casa, las industrias `OnlyInTown` ahora usan el clear completo
de pueblo. La cohorte Tropic 512² con settings de río explícitos vuelve a ser
exacta; la evidencia y sus límites están en `random-map-issues.md` y
`evidence/rmap-146.json`.
RMAP-147 / #362 amplía el gate a los pools ordenados de industrias y objetos:
la cohorte Temperate/default 512² de `1330935378` es exacta por tiles, bloques
4×4, RNG, pueblos, 213 industrias y 65 objetos. La evidencia canónica y los
límites (campos restantes, intentos y ticks) están sólo en
`random-map-issues.md` y `evidence/rmap-147.json`; #338 sigue abierto.
RMAP-148 / #363 aplica el mismo gate a la cohorte Tropic 512² con ríos
explícitos de RMAP-146: también es exacta por tiles, bloques 4×4, RNG y pools
ordenados (98 pueblos, 213 industrias y 60 objetos). La configuración,
evidencia y límites están en `random-map-issues.md` y
`evidence/rmap-148.json`; no amplía el cierre de #338.
RMAP-149 / #364 completa el control equivalente Arctic 512² con ríos
explícitos: 96 pueblos, 217 industrias y 61 objetos coinciden ordenados además
de tiles, bloques 4×4 y RNG. La evidencia y límites están en
`random-map-issues.md` y `evidence/rmap-149.json`; #338 conserva la matriz
completa pendiente.
RMAP-150 / #365 extiende el gate a Toyland 512²: 85 pueblos y 203 industrias
coinciden ordenados; el pool de objetos es vacío en ambos lados y se valida
explícitamente. La evidencia y sus límites están en `random-map-issues.md` y
`evidence/rmap-150.json`; no equivale a cobertura no vacía de objetos Toyland
ni cierra #338.
RMAP-151 / #366 eleva el gate a v5 para que cada industria compare, además de
su identidad y layout, los bits `random`, color, contador, nivel de producción
y pueblo asociado. Corrige una pérdida real de los 16 bits iniciales de
`CreateNewIndustry` sin modificar el stream RNG: Temperate/default 512² de la
seed `1330935378` queda exacto en las seis fronteras por tiles, bloques 4×4,
RNG, pueblos, pools y ese estado constructor (96/213/65 desde `objects`). La
evidencia canónica y sus límites están sólo en `random-map-issues.md` y
`evidence/rmap-151.json`; siguen pendientes campos INDY restantes, intentos,
industrias acuáticas, otras matrices y ticks de #338.

RMAP-152 / #367 eleva el gate a v6 con la secuencia ordenada de todos los
intentos de `CreateNewIndustry`: ordinal, tipo, origen, `random_var8f`, los
16 bits iniciales, layout y resultado. La cohorte Temperate/default 512² de
la seed `1330935378` conserva sus 409 intentos, incluidos rechazos, exactos
en las seis fronteras junto a tiles, bloques 4×4, RNG y pools. La evidencia
canónica y sus límites quedan en `random-map-issues.md` y
`evidence/rmap-152.json`; #338 sigue abierto para diagnósticos de rechazo,
campos INDY restantes, agua/OilRig, matriz ampliada y ticks.

RMAP-153 / #493 aplica ese gate v6 a Tropic 512² con ríos explícitos: la seed
`1330935380` conserva exactas las seis fronteras, incluidos sus 39.662
intentos ordenados. La evidencia canónica y los límites —motivos de rechazo,
campos INDY restantes, agua/OilRig, más settings y ticks— están sólo en
`random-map-issues.md` y `evidence/rmap-153.json`; #338 permanece abierto.

RMAP-154 / #494 completa la comprobación equivalente Arctic 512² con ríos:
la seed `1330935379` conserva las seis fronteras exactas y sus 17.039 intentos
ordenados. La evidencia canónica y los mismos límites permanecen sólo en
`random-map-issues.md` y `evidence/rmap-154.json`; #338 no se cierra.

RMAP-155 / #495 completa esta cohorte v6 por clima con Toyland/default 512²:
la seed `1330935381` conserva las seis fronteras y 908 intentos exactos; su
pool de objetos vacío se declara como tal. Evidencia y límites quedan sólo en
`random-map-issues.md` y `evidence/rmap-155.json`; #338 no se cierra.

RMAP-156 / #496 agrega `--compact-report` al oráculo por fases: valida el
reporte completo antes de persistir una huella portable de sus pools, por lo
que habilita casos grandes sin convertir sus trazas en artefactos versionados
gigantes. No sustituye el reporte completo al diagnosticar una divergencia ni
reduce el alcance pendiente de #338.

RMAP-157 / #498 aplica esa evidencia a Tropic/ríos 1024²: la seed
`1330935380` mantiene sus seis fronteras exactas, incluidos 151.843 intentos
industriales. La configuración, timeout, hashes y límites quedan sólo en
`random-map-issues.md` y `evidence/rmap-157.json`; #338 no se cierra.

RMAP-158 / #499 porta la plataforma Oil Rig que faltaba del ciclo de runtime:
tipo nativo 5, layout 2×3, agua por tesela, producción, estación neutral
`Oilrig` (helipuerto y muelle), cierre y round-trip SAV interno. La construcción
terminada conserva la entidad y los bytes vinculados; RMAP-172 precisa que la
estación nace al terminar la pieza norte `GFX_OILRIG_1` si existe la sur, no al
esperar que ambas terminen. La cohorte Temperate
512²/seed `1330935382` sigue exacta en `industries` por teselas, bloques 4×4,
RNG e intentos. No se cierra #499: la fundación vanilla posterior a 1960 está
cubierta por RMAP-161/#502, pero faltan catálogos/callbacks NewGRF, settings
no vanilla y una matriz temporal amplia; el detalle canónico queda sólo en
`random-map-issues.md`.

RMAP-159 / #500 completa el estado persistente previo a esa fundación: `IBLD`
(`wanted_inds` 16.16) e `ITBL` (las 240 filas nativas con probabilidad,
mínimo, objetivo y backoff) entran al modelo, JSON v28 y lectura/escritura SAV.
El selector puro replica `GetIndustryGamePlayProbability` vanilla, incluido
Oil Rig temperate desde 1960 con peso 6 y Oil Wells hasta 1950; `SetupTargetCount`
no reconsume RNG cuando la tabla no cambió. También quedan cubiertos el paso
mensual `0x38000/(10*12)`, la acumulación diaria `ECMY` y la escala 64²/256²/512².
El exportador emite las 240 filas y `OpenTTD` dedicated cargó el SAV rico
canónico que las contiene. RMAP-161/#502 ejecuta después la ruta vanilla;
callbacks NewGRF y settings no vanilla continúan fuera de este corte.

RMAP-160 / #501 cierra sólo el corte de observación temporal: el oracle nativo
emite una muestra post-timer con `ECMY`, `IBLD`/las 240 filas `ITBL`, RNG,
industrias ordenadas y decisiones `Chance16`; el runner termina por jornadas y
el validador falla cerrado ante huecos o cardinalidad distinta. La evidencia
concreta de la fixture y el hash viven canónicamente en
[`random-map-issues.md`](random-map-issues.md); el formato y el comando están
en [`INDUSTRY_SCHEDULER_TRACE_SCHEMA.md`](INDUSTRY_SCHEDULER_TRACE_SCHEMA.md).
RMAP-161/#502 consume este contrato para la fundación vanilla, pero la traza
no sustituye todavía el detalle de cada intento ni cierra #499.

RMAP-161 / #502 cierra el corte ejecutable vanilla del scheduler: conserva el
contador diario `ECMY`, `Chance16(3…9,100)`, selección sparse por `IndustryID`,
`IBLD`/backoff y el límite de 2.000 intentos de la fundación automática. La
fixture 64² desde 1960 funda una Oil Rig sin fundador ni cargo, conserva sus
teselas/estación y verifica `INDY.town` como `REF_TOWN` después de un round-trip
OTTN. El alcance y los límites viven sólo en `random-map-issues.md`: NewGRF,
settings no vanilla y matriz temporal siguen abiertos.

RMAP-162 / #506 completa el puente diferencial: el candidato Rust exporta el
mismo JSONL v1 en el corte post-timer y el comparador exige reloj, RNG, `ECMY`,
`ITBL`, industrias y acciones exactos. #507/#515 restauran el `DATE` completo,
incluido el cursor LFSR del tile loop, y #514/#516 despachan Industry/Town;
#517 Tree/`OnTick_Trees`, #518 el grupo global de animación industrial y #519
las seis animaciones de aeropuerto `AcceptanceTick`. #520/#522 corrigen la
cadencia de fábrica a 256 ticks y #523 recupera la semántica `Chance16` de
`PlantFields`, por lo que la corrida auditada ya iguala diez jornadas; la
medición diaria
canónica se mantiene en
[`random-map-issues.md`](random-map-issues.md#rmap-162--comparar-el-scheduler-industrial-rust-contra-la-traza-diaria-openttd).
RMAP-163/#524 extiende esa frontera a 23 jornadas y concentra su evidencia
actualizada en la fila canónica de RMAP-163; RMAP-164/#525 la lleva a 25 y
RMAP-166/#527 a 35 mediante el tile loop urbano. RMAP-168/#529 y los cortes
posteriores extienden la misma fixture hasta 360 jornadas en RMAP-175; la
extensión dedicated de #527 la verifica hasta 400 jornadas y conserva su
cobertura residual.
#510 impide que una ejecución dedicated sin socket se acepte como oracle aunque
produzca JSONL válida. RMAP-162 cierra instrumentación y regresiones, no afirma
todavía paridad temporal ni reduce los pendientes de #499, RMAP-056 o #338.

Actualizado el 2026-09-09: RMAP-159/#500, RMAP-160/#501, RMAP-161/#502 y
RMAP-162/#506 son sub-issues cerrados de estado/selección, observación nativa,
ejecución vanilla e instrumentación diferencial. #507/#515 conservan la carga
`DATE` incluida la posición LFSR, #510 valida la calidad de la corrida native,
#511 cierra el contador persistido, #513 el sonido ambiental, #514/#516 los
bloques actuales de `TileLoop_Industry`/`TileLoop_Town`, #517
`TileLoop_Trees`/`OnTick_Trees`, #518 el grupo global de animación industrial
y #519 los grupos aeroportuarios de la primera jornada; #520/#522 alinean
la cadencia de fábrica con diez `IndustryTick` y #523 corrige el fallback
`PlantFields` para que `Chance16` no se sustituya por `RandomRange`. #512
permanece abierto por cobertura runtime pendiente; RMAP-163/#524 ya alineó el
cierre mensual, RMAP-164/#525 el primer walker posterior y RMAP-166/#527
alineó el corte inicial de ascensores/obra/renovación; RMAP-168/#529 y los
cortes posteriores llevaron la fixture integrada a 360 jornadas, y la
extensión dedicated de #527 la verifica a 400. Siguen abiertas las variantes
urbanas multitile/protegidas y callbacks no cubiertos.
Los
gates continúan siendo obligatorios y ningún issue padre se considera cerrado
por esta cobertura.

Reparación #347 validada (2026-09-04): las casas sin PNG suelto se recortan
del atlas distribuido y conservan la misma paleta; las páginas se decodifican
una vez por construcción. Se mantiene la aserción de pares completos y se
añaden pruebas de directorio sin PNGs, RGBA/dimensiones, recorte, transparencia,
override y página truncada. Pasan 3249 tests workspace con nextest (3 omitidos),
1074 client con cargo test (2 ignorados), Clippy client, rustdoc completo,
formato y gates documentales. El cliente recompilado abrió Kale y produjo
capturas a 1×/0.5×/0.25×/0.125×, revisadas en
`/tmp/openttdrs-house-atlas-scale-{1,2,4,8}.png`. El mapa se ve en los cuatro
niveles; las capturas usan los assets locales y no sustituyen la prueba
aislada sin PNGs ni certifican raster exacto. Persisten marcas negras pequeñas
en agua al alejar a 0.5×/0.25×, aisladas como el sub-issue visual #349 de
#326. El resultado remoto posterior quedó verde y #333 ya se cerró; #349
mantiene separado el diagnóstico visual pendiente.

Brechas identificadas al verificar el cliente: #349 aísla marcas negras de
agua en 0.5×/0.25×. #350 queda reparado el 2026-09-05: la carga síncrona de
paletas (casas, compañía y estructuras de puente) deriva `tiles_assets_dir`
de `resolve_asset_root`, igual que `AssetServer`, en vez de retener
`CARGO_MANIFEST_DIR`. La regresión se ejecuta en subprocesos con layout de
paquete para cwd, override y ejecutable trasladado, sin mutar el entorno del
proceso de pruebas; los assets realmente ausentes siguen devolviendo fallback.
La corrección del atlas de #347 no sustituye el diagnóstico visual de #349.

Oráculo raster #351 (2026-09-05): el exportador C++ acepta ahora la misma
escala ortográfica fija que el candidato (`0.25`, `0.5`, `1`, `2`, `4`, `8`) y
la traduce a `In4x`…`Out8x` antes de rasterizar. La cámara ajusta sus
coordenadas virtuales con ese zoom y el DPI del raster usa el mismo valor; la
captura normal conservó SHA-256 y cero píxeles distintos frente al binario
anterior, mientras Kale generó los seis zooms a 1280×720. El integrador también
detecta un fork derivado del pin que ya contiene `snapshot_export.cpp`: conserva
esa fuente y sus hooks, sin añadir el `world_raw` duplicado. El contrato y el
comando canónico están en `WORLD_SCREENSHOT_SCHEMA.md`; #349 permanece abierto
hasta aislar agua plana y atribuir una causa concreta. Una corrección posterior
del informe exige esos seis pasos discretos y escribe el `ZoomLevel` nativo
efectivo en `report.json`, de modo que una referencia `Out2x` o `Out4x` ya no
queda falsamente identificada como `normal`.

Agua plana #349 (2026-09-05): en el bloque interior 4×4 `(140,12)`…`(143,15)`
de Kale, el PNG OpenGFX de agua mide 64×31 aunque el rombo lógico mide 64×32.
Al reducir el sprite directamente a `Out2x`/`Out4x` quedaba media fila sin
cobertura entre vecinos y se veía el framebuffer negro. El renderer conserva
64×31 en `In4x`/`In2x`/Normal y aplica el footprint lógico 64×32 sólo a agua
animada en `Out2x`/`Out4x`/`Out8x`, incluidos chunks que aparecen tras el
zoom; las esclusas estáticas no se alteran. Una región de agua pura de 128×128
píxeles en `Out2x` pasó de 384 píxeles negros a 0, y Normal mantiene AE=0
frente a la captura previa. Se revisaron los seis zooms. Esto resuelve sólo la
costura negra de #349; las diferencias de composición, sprites y cámara siguen
abiertas en el padre #326.

Seguimiento histórico #347 / #333 (2026-09-05): el primer workflow limpio encontró un
defecto en el *bootstrap* de la propia regresión, no una ausencia del atlas
versionado. La prueba construía `tiles/../atlas`; como `tiles/` es opcional e
ignorado, el kernel no puede recorrer esa ruta en CI. El loader y la prueba
obtienen ahora el padre léxico de `tiles` antes de añadir `atlas`, y una
regresión exige que funcione cuando `tiles/` no existe. La corrección se
publicó posteriormente, #347 se cerró y el workflow remoto verde de
`eb47e7de` permitió cerrar #333.

Etapa #333 — reparación de gates (2026-09-04): se reprodujeron los fallos de
CI de `b47163d1`. Rustdoc tenía dos enlaces rotos (`IndustryRandomTrigger` y
`[LandscapeType][slot]`); las referencias ya están corregidas y el comando
completo con `RUSTDOCFLAGS="-D warnings"` pasa. El workspace fuzz conservaba
SHA-2 0.10 después de que core pasó a 0.11; su lockfile se sincroniza con
Cargo y vuelve a resolver con `--locked`. Cuatro regresiones ejecutan el gate
documental con y sin `rg`, admiten documentación correcta y rechazan un dato
obsoleto; se integran a CI y Parity docs. El replay sanitizer pasó los 800
inputs y el SAV ancla, con ASan/LSan activos fuera del sandbox (su restricción
de ptrace impedía finalizar LSan). Formato, Clippy workspace, rustdoc completo,
gate documental y pruebas Python focalizadas pasan. La verificación remota
posterior de `eb47e7de` dejó todos los workflows requeridos verdes y #333 se
cerró el 2026-09-05.

Seguimiento #333: el manifiesto Python completo detectó una expectativa vieja
`actions/cache@v5` en el test de release, mientras Dependabot ya actualizó el
workflow a v6. El test comprueba ahora el uso de `actions/cache` conservando
los gates de versión OpenTTD, checksum, matriz estricta y artefactos. La
regresión focalizada de release y el manifiesto completo pasan localmente.

Seguimiento #333 (2026-09-05): los workflows de plataforma, fuzz y Parity docs
de `04c44bbf` están verdes; CI llegó hasta el último bloque Python y falló sólo
porque `test_opengfx_palette.py` importa Pillow sin que el conjunto APT
compartido instale `python3-pil`. El arreglo añade ese paquete a la misma lista
que consume el composite de Rust y una regresión de `test_ci_workflow_parity`
lo exige. `check.sh ci-python` pasa completo fuera del sandbox local (la prueba
de release abre un socket localhost que el sandbox prohíbe). Las correcciones
posteriores, incluido el contrato de estación para el checkout limpio, llevaron
a CI, Parity docs, Fuzz replay y Platform check verdes en `eb47e7de`; #333 se
cerró con esa evidencia.

Sub-issue #352 de #337 (2026-09-05): el gate documental ahora incluye el plan
continuo, la matriz RMAP y su registro de issues; sus pruebas inyectan una
afirmación obsoleta en cada fuente tanto con `rg` como con el fallback
`grep -E`. El manifiesto activo valida fecha, hashes, pin OpenTTD y la política
que cita el último commit ya publicado en vez de auto-referenciar el commit
documental. #337 sigue abierto por la auditoría semántica de las demás matrices.

Sub-issue #368 de #337 (2026-09-05): el corte canónico avanza a `dc3602b5`,
última etapa publicada antes de su commit documental, y actualiza sus conteos
de validación. El gate exige que RMAP-056 y RMAP-082 continúen abiertos como
padres de cobertura aunque sus sub-issues estén cerrados; las pruebas cubren
ambos cierres falsos con `rg` y `grep -E`, además de un hash sintácticamente
válido que no coincide con el bloque canónico. #337 sigue abierto para la
auditoría semántica completa de las fuentes restantes.

Sub-issue #369 de #337 (2026-09-05): la evidencia compacta de la cohorte de
mapas 15/15 ya registra fecha, fixture procedural, pin OpenTTD y commit de la
candidata. El hash del binario histórico no se inventa: queda declarado como
no conservado. El gate ejecuta su prueba pura y rechaza mutaciones de fecha,
fixture o commit. Esto conserva trazabilidad de esa cohorte, no cierra
RMAP-004/#338 ni sustituye una regeneración futura.

Auditoría #337 cerrada (2026-09-05): #352–356 y #368–369 dejan una fuente
canónica por área, separan las mediciones históricas de los baselines vigentes
y cubren en el gate las contradicciones conocidas de corte/backlog, RMAP,
SAV/OBID, NewGRF y raster. El cierre es exclusivamente documental: #326,
#328–331 y #338 siguen abiertos con sus criterios técnicos y de paridad.

Sub-issue #353 de #337 (2026-09-05): la matriz Action0 distingue la FTA custom
todavía bloqueada de los callbacks `AirportTile` ya conectados, y este plan
describe CB36 e historiales `INDY` según sus call sites actuales. El gate
rechaza el regreso de las afirmaciones contradictorias. #329 no se cierra:
quedan compositor de foundations/rotaciones/sonidos, APIs legacy sin catálogo,
propiedades y scopes Action0 restantes, cargos custom y writebacks de teselas.

Sub-issue #354 de #337 (2026-09-05): las fuentes SAV ya distinguen el
passthrough sin mutaciones de `OBJS`, su reconstrucción base tras mutarlo y la
fusión de campos conocidos de `OBID` cuando conserva los IDs. El mapping
importado participa al reaplicar el catálogo de objetos NewGRF. #328 permanece
abierto por columnas no modeladas tras mutación, cambios estructurales,
listas/structs, pools nativos y runtime de objetos.

Sub-issue #355 de #337, cerrado el 2026-09-05: la evidencia raster separa ahora el
baseline global reproducible de `Kale_TitleGame.sav` (`cd3c4241`, OpenTTD 15.3
pin `14ec60f` y oracle `c2661164`) de los diagnósticos focales históricos. La
matriz de seis zooms conserva cámara, hashes y métricas en
`evidence/kale-189-126/baseline-2026-09-05.json`; el baseline normal permanece
distinto, por lo que #326 sigue abierto por composición global, clipping,
pivotes y familias de producers restantes.

Base funcional local y publicada: **`25d026a7`** (`render: project vehicle effects in isometric space`),
encima de `566ce56a` (IDs globales SAV), `933042ca` (documentación de aceptación exacta)
y `67ef8101` (`newgrf: evaluate industry tile cargo acceptance`). Los cuatro commits ya están
en `origin/main`; el rechazo 404 anterior quedó resuelto por el reintento posterior.
Este handoff documental incluye la entrega directa
SAV de campos legacy (`56aa7858`) y los slots vacíos legacy (`9f2ecc31`), y se publica después de las etapas
de rechazo temporal (`65682a42`), cargos dinámicos (`389109c1`) y
`PlantOnBuild` manual (`628d1fb9`); es el punto de reanudación y el código local
coincide con este árbol.

Corrección de este handoff: `470499ea` ya cubre `STNN.base.owner`,
`INDY.neutral_station`, `INDY.exclusive_supplier` y
`station.serve_neutral_industries`, tanto en round-trip como en las rutas de
entrega y transporte. `8be6bbc6` añade la fachada `ScriptCargoMonitor` con
validación de límites y `StopAllMonitoring`; la fila histórica de #329 que
enumera esas dos brechas queda superada. `eb6bd78d` conserva además las filas
`INDY.accepted/produced` de cargos no resolubles como passthrough opaco (slot,
stock, rate y ventana histórica), de modo que un SAV no vuelve a perder esos
datos al reexportarse sin el GRF. `c88518c4` separa además la pasada visual de
frames de los callbacks CB25: `TileLoop` sólo se dispara sobre visitas,
`IndustryTick` al intervalo de producción y `CargoReceived` al confirmar la
entrega. `aa289076` conecta también `CargoDistributed` con el retorno real de
`TransportIndustryGoods` cuando la carga llega a una estación. `ca2939a7` conecta
`ConstructionStageChanged` tanto al alta inicial (con `var 18 |= 0x100`) como a
los cambios de etapa posteriores. `67ef8101` conecta además la aceptación exacta
de carga de teselas de industria (`CBID_INDTILE_CARGO_ACCEPTANCE`/`CBID_INDTILE_ACCEPT_CARGO`)
con la cobertura de estación y la descarga real, manteniendo el fallback legacy.
`bd613e2a` materializa hasta 32 cargos custom como `CargoType::Custom`; `a2a0ce35`
materializa también el último slot nativo (`CargoType` 63), los
asigna de forma estable por `(GRFID, local_id)` y los transporta por stocks,
packets, cobertura, estaciones, industria, producción, pagos, ratings,
cargodist, refit y autoreplace. `566ce56a` corrige además la frontera nativa
de SAV: `SLV<55` se interpreta por slot climático y `SLV≥55` por ID global,
incluidos los cargos custom `31..63`, para `STNN`, `INDY`, `VEHS` y `LGRP`;
el exportador emite siempre la tabla moderna de 64 IDs. La economía completa
de una carga aún requiere su `CargoSpec` para nombre, peso, CTT y callbacks.
`6266171f` completa además la validación de `ScriptCargoMonitor` para cargos
custom cuyo `CargoSpec` está activo: las cuatro consultas aceptan el ID global,
mantienen activación/reset y registran entregas y recogidas. La fachada Squirrel
completa y los cargos sin catálogo siguen fuera del alcance.
`fd573da5` pasa también el catálogo de `CargoSpec` a la física de carretera y
usa `prop 0x0F` para el peso de carga custom; los callers legacy conservan el
fallback vanilla. `b32b87f4` aplica el mismo peso a todas las unidades de un
consist ferroviario y recalcula la caché después de cargar/descargar, sin
alterar el orden de señales. `15c8bfcf` añade `vehicle.freight_trains` con
persistencia `PATS`/JSON, frontera `SLV_39` y aplicación exclusiva a cargas
freight. `5e0938ff` expone presets del setting en Ajustes y refresca los
consist de inmediato. Quedan pendientes la edición arbitraria tipo slider, CTT
completa y el resto de settings económicos. `d6b4c5fc` completa ahora la
primera ruta de CTT de vehículos: default cargo y listas include/exclude de los
cuatro features de vehículos se traducen contra GlobalVar `0x09` y el catálogo
`CargoSpec`; refit y la UI ya consumen esos cargos custom. `97571c10` añade las
clases Action0 `allowed`/`disallowed`/`required` de trenes, vehículos de
carretera, barcos y aeronaves: la máscara se aplica contra las clases vanilla o
el `CargoSpecDef` custom, conserva el XOR de `refit_mask` y deja CTT
include/exclude como última capa. La regresión cubre los cuatro parsers y un
`TOFU` custom. El slot global 63 ya está materializado; el callback de refit se
completa en `61e0c53b`; siguen pendientes UI/variables ilimitadas, scopes
económicos y otras propiedades Action0.

Actualización #329-VEHICLE-CARGO-CTT-075 (2026-09-04, commit `d6b4c5fc`): el
parser Action0 conserva los índices locales de cargo por defecto y las listas
CTT de trenes, carretera, barcos y aeronaves. `apply_newgrf_vehicles_trains`
resuelve esos índices con la versión/tabla del GRF y el catálogo activo, y el
catálogo completo aplica primero `Cargoes` para que un label custom sea
ejecutable. `EngineDef` guarda default, inclusión y exclusión; la consulta de
refit, la compra por carga y la selección de sprites usan la identidad global;
la ventana de refit y el botón de vehículo muestran el nombre del `CargoSpec`.
La regresión `vehicle_ctt_resolves_custom_default_and_refit_cargo` cubre
`TOFU` como default e include. Clases/required se completan en el siguiente
commit publicado `97571c10`.

Actualización #329-VEHICLE-CARGO-CLASS-076 (2026-09-04, commit `97571c10`):
Action0 ya parsea `allowed`, `disallowed` y `required` con el ancho WORD nativo:
trenes `0x28/0x29/0x32`, carretera `0x1D/0x1E/0x29`, barcos `0x18/0x19/0x25` y
aeronaves `0x18/0x19/0x23`. `EngineDef` conserva las tres máscaras y distingue
una declaración explícita vacía del fallback vanilla. Refit calcula la máscara
por clases (`Any(allowed)`, `All(required)`, sin `disallowed`), aplica el XOR de
la máscara legacy cuando corresponde y luego las listas CTT; con catálogo usa
las clases declaradas por cada `CargoSpecDef`, incluidos cargos custom. Las
regresiones `vehicle_cargo_class_properties_parse_for_all_features` y
`vehicle_cargo_classes_filter_custom_catalog` fijan parser, aplicación y filtro
de refit. #329 sigue abierto por GUI/variables ilimitadas, scopes económicos y
otras propiedades Action0.

Actualización #329-VEHICLE-CARGO-SLOT-077 (2026-09-04, commit `a2a0ce35`): el
runtime alinea la frontera de cargos con `NUM_CARGO = 64` de OpenTTD y
materializa `CargoType::Custom(32)` (ID global 63) en stocks, antigüedad de
espera, `StationGoods`, packets, producción y las tablas SAV modernas. El
importador `SLV_55` conserva el ID 63 aun sin catálogo y el writer sigue
emitiendo las 64 filas nativas. El JSON propio sube a v27; sus deserializadores
aceptan arrays custom legacy de 32 entradas y dejan el nuevo slot en cero. Las
regresiones `final_custom_slot_matches_openttd_num_cargo_and_legacy_json`,
`final_custom_time_slot_roundtrips_and_accepts_legacy_json` y la estación SAV
global verifican el límite y el round-trip. No se inventan IDs `64+`: el
GUI/variables ilimitadas y scopes económicos siguen pendientes, por lo que
#329 permanece abierto.

Actualización #329-VEHICLE-CUSTOM-REFIT-078 (2026-09-04, commit `61e0c53b`):
el runtime ejecuta `CBID_VEHICLE_CUSTOM_REFIT` (`0x163`) con el bit 9 de la
máscara Action0. Para cada `CargoSpec` candidato se pasan `CargoClass` en
`param1` y el índice local CTT en `param2`, con fallback climático/`bitnum`
según la versión del GRF. El resultado `0`/`CALLBACK_FAILED` conserva la
selección base, `1` agrega el cargo y `2` lo retira; el resto queda como no-op
diagnosticable. Refit manual, órdenes de depósito, refit pendiente y
autoreplace comparten la función catalogue-aware y tienen regresiones de
inclusión/exclusión y parámetros CTT. #329 sigue abierto por GUI/variables
ilimitadas, scopes económicos y el resto de callbacks/propiedades de vehículos.

Actualización #329-VEHICLE-REFIT-COST-079 (2026-09-04, commit `4a80e6d3`):
Action0 conserva `refit_cost` para tren, road, barco y aeronave. El resolver
`CBID_VEHICLE_REFIT_COST` (`0x15E`) empaqueta `CargoClass`, subtipo y CTT local,
y decodifica el factor signed de 14 bits junto con el permiso de autorefit del
bit 14; `CALLBACK_FAILED` vuelve al factor Action0. El precio usa el índice por
tipo (factor doble en trenes) y se integra en refit manual, órdenes de depósito
y cálculo de autoreplace, con rechazo atómico por fondos insuficientes. Las
regresiones cubren parser, CTT custom, signo/autorefit y diferencia de coste.
La aplicación del permiso de autorefit a cadenas articuladas completas y la UI
siguen abiertas; #329 no se cierra.

Actualización #329-VEHICLE-REFIT-COST-080 (2026-09-04, commit `acbc3675`):
el cálculo de autoreplace recorre el consist que `ReplaceChain` reconstruirá y
agrega el coste de refit de cada unidad que tiene una regla efectiva, además
de la cabeza; la trasera dual-head usa el motor nuevo y las piezas generadas
por CB16 no se cobran porque se recrean. Una regresión de dos camiones fija el
doble coste ante el mismo factor Action0/CB15E. La semántica de auto-refit en
estaciones y la UI siguen pendientes; #329 no se cierra.

Actualización #329-VEHICLE-STATION-REFIT-081 (2026-09-04, commit `92e8aee2`):
las órdenes de estación ahora conservan `refit_cargo` y `auto_refit` en JSON y
en `ORDL`; la importación/exportación usa los sentinels nativos `0xFD` y `0xFF`
y traduce también cargos custom globales `31..63`. `load_vehicles` ejecuta el
refit antes de capacidad, locomotora sin vagón y selección de carga, por lo que
una orden manual puede refitar aun sin stock. El modo automático selecciona el
cargo aceptado con mayor stock, consulta las opciones NewGRF de cada unidad,
aplica CB15E/bit 14, recalcula CB36 y cobra una sola vez de forma atómica. El
marcador `refit_capacity` se conserva asimismo en el comando de depósito y
autoreplace. La cobertura queda deliberadamente parcial: no reproduce todavía
el balanceo de capacidad/reserva de `HandleStationRefit`, la elección de
siguiente estación, todos los consist articulados ni la edición UI. #329 sigue
abierto y el siguiente bloque debe medir uno de esos casos con el oráculo.

Actualización #329-VEHICLE-VISUAL-EFFECT-082 (2026-09-04, commit `b1df2500`):
el core ejecuta `CBID_VEHICLE_SPAWN_VISUAL_EFFECT` (`0x160`) cuando el callback
visual selecciona el modelo avanzado. Se decodifican el contador, los bits de
centrado/rotación y los cuatro registros `0x100..0x103` (tipo y offsets
signed X/Y/Z), con writeback de `7C`; el renderer compartido materializa los
tipos vanilla `F1`/`F2`/`F3`/`FA` para trenes, carretera, barcos y aeronaves,
rota X/Y según la dirección visual, aplica el centro de unidades cortas y
conserva cada spawn como entidad de efecto independiente. Los modelos
reservados/desactivados no invocan CB160 y el fallo no degrada silenciosamente
a humo vanilla; depósitos, túneles, puentes, vehículos ocultos, parados y
trenes que revierten quedan suprimidos como en `ShowVisualEffect`. Sigue siendo
una cobertura parcial: faltan sprites y sonidos locales de GRF, la semántica
completa de consist y la proyección exacta de offsets en todas las escalas del
viewport. #329 permanece abierto.

Actualización #329-VEHICLE-VISUAL-EFFECT-083 (2026-09-04, commit `5682ef1c`):
la ruta estándar de CB10 (`0x10`) comparte ahora la emisión vanilla de vapor,
diésel y chispa para carretera, barcos y aeronaves cuando el GRF la selecciona.
El renderer conserva el offset `0..15`, corrige la longitud de unidades de tren
y respeta la inversión visual; la supresión por velocidad, humo, depósitos,
túneles, puentes y visibilidad queda alineada con `ShowVisualEffect`. Los
valores `VE_DEFAULT` de vehículos no ferroviarios continúan desactivados. El
bloque no cierra #329: quedan sprites/sonidos locales, consist completo y la
proyección de offsets en todos los zooms.

Actualización histórica #329-VEHICLE-VISUAL-EFFECT-084 (commit `25d026a7`):
los offsets `x/y/z` de CB10 y CB160 pasan por la proyección isométrica común del
cliente. `z` modifica ahora la altura visual proyectada y no el tercer
componente sortable de Bevy, por lo que el orden de teselas permanece estable;
la corrección cubre el humo vanilla (`z=10`) y los registros avanzados en todos
los zooms. El issue sigue abierto por sprites/sonidos locales, composición
completa de consist y sorter/viewport.

Corrección vigente (2026-09-04): la auditoría #329-VEHICLE-VISUAL-EFFECT-085
encontró y corrige tres defectos que invalidaban la afirmación de posición
exacta de 082–084, además del desbordamiento signed durante la rotación.
La evidencia y los límites se mantienen únicamente en
[la matriz de callbacks](newgrf-callback-matrix.md#329-vehicle-visual-effect-085--posición-y-continuidad-de-efectos).
El siguiente corte visual debe resolver cadencia/RNG, filtros o altura aérea;
los padres #326/#329 siguen abiertos.

Corrección vigente de este corte: CB160 ya tiene call site compartido para
trenes, carretera, barcos y aeronaves, con auto-centro, rotación y supresión
de estados no visibles alineados al upstream. CB10 estándar también tiene una
ruta compartida para los cuatro tipos cuando el modelo no es `VE_DEFAULT`. La
brecha restante de #326/#329 es la composición exacta (sprites/sonidos locales,
consist y sorter/viewport), no la ausencia de un call site por tipo.

Actualización #326-AIRPORT-LAYOUT-ROTATION (2026-09-07): el picker de
construcción lista los airports NewGRF habilitados y conserva el índice Action0
exacto junto con su id global; query, ghost y execute usan la misma elección y
validan sólo sus teselas declaradas. La evidencia y los límites viven en
[newgrf-airport-layout-rotation-326.md](newgrf-airport-layout-rotation-326.md);
la rotación runtime de sprites/children sigue abierta.

Actualización #503-AIRPORT-PARENT-BADGES (2026-09-07): `AirportTile` ahora
materializa `0x7A[param]` también en el scope padre con la tabla local del GRF
de la tesela y los badges del `AirportSpec` construido; render, CB150 y el
scheduler de animación reciben el mismo catálogo, y el fingerprint incluye los
parámetros parent para que dos aeropuertos no compartan una variante equivocada.
La evidencia y los límites están en
[newgrf-airport-parent-badges-503.md](newgrf-airport-parent-badges-503.md).
Esto no cierra #326 ni #329: FTA, paletas, foundations/rotaciones y sonidos
siguen fuera de este subtramo.

Actualización #504-AIRPORT-PARENT-STATION-VARS (2026-09-07): el
`AirportScope` padre publica ahora `F0` (facilities) y `FA` (fecha relativa
`WORD`) con la misma escala de OpenTTD; `7C` carga el PSA de estación.
La prueba core encadena `F0` → `FA` y la de renderer ECS exige `F0` antes de
resolver el badge padre. La evidencia y los límites están en
[newgrf-airport-parent-scope-504.md](newgrf-airport-parent-scope-504.md).
No se declara completa la delegación de `Station::GetNewGRFVariable` ni los
residuales FTA/raster de #326/#329.

Actualización #505-AIRPORT-PARENT-PSA (2026-09-07): `\\2psto` de un Action2
parent de `AirportTile` ya vuelve al PSA del aeropuerto tras CB152/153/154,
en vez de perderse en el mapa del scope propio sin storage. La escritura inicial
de cero conserva la asignación perezosa nativa; el renderer sigue sin writeback.
La regresión cubre CB152 → PSA → CB153 y la evidencia está en
[newgrf-airport-parent-psa-505.md](newgrf-airport-parent-psa-505.md). Esto no
cierra #326 ni #329: siguen pendientes StationScope, FTA y raster/sonido.

Actualización #560-RASTER-CLEAN-INCOME (2026-09-09): el perfil temporal
`OPENTTDRS_MAP_SHOT_CLEAN=1` excluye ahora también `IncomePopupText`, el texto
efímero `+$N` que nace sobre `MapVisualLayer` al materializar ingresos. El
filtro corre después de `UpdateSet::Ui`, igual que el resto del ocultamiento de
captura; fuera de ese perfil el popup sigue visible. La regresión materializa un
`IncomePopup` real y verifica ambos estados. Kale `(189,126)`, `1280×720`, se
revisó limpio a `0,25×`, `0,5×`, `1×` y `2×`; es higiene del oráculo, no una
afirmación de paridad ni una modificación del compositor. #326 permanece
abierto por pivotes, atlas, orden y framebuffer global.

Actualización #562-RASTER-RAIL-GLASS (2026-09-09): los hijos de techo
ferroviario vanilla `1083`–`1086` siguen siendo una máscara de destino
`PALETTE_TO_TRANSPARENT`; no se recolorea su blob CC amarillo. Como el blitter
8bpp de OpenTTD remapea una paleta y Bevy compone alpha lineal, la equivalencia
del cliente queda calibrada a negro con alpha `0,50`, no al antiguo `0,28` ni a
una copia literal del numerador `154/256` del blitter 32bpp. En Kale
`(189,117)`, `1280×720`, perfil limpio, el delta medio raw baja en las cuatro
escalas: `0,25×` `8,263930→8,162895`; `0,5×` `3,306579→3,281427`; `1×`
`3,856157→3,827999`; `2×` `22,498161→22,468118`. La cantidad de píxeles
distintos no cambia: esta corrección sólo ajusta el tono de la máscara, no
declara paridad total ni corrige orden global. #326 permanece abierto; el
vínculo parent/child global de esas secuencias sigue separado en #561.

Actualización #326-ROADSIDE-DETAILS (2026-09-09): los faroles y árboles
roadside de `DrawRoadDetail` ya emiten parents individuales en el compositor
global con prismas nativos `2×2×16`, altura de superficie e inserción entre
fundaciones y `DrawBridgeMiddle`. La traza Kale scoped añade 313 detalles y la
medición limpia normal mejora de 10,553 % a 10,176 % de píxeles distintos; las
cinco escalas restantes no cambian. Quedan composición segmentada, clipping,
pivotes y framebuffer, por lo que #326 sigue abierto; el detalle cuantitativo
canónico queda en `PARIDAD.md`.

Actualización #326-RAIL-STATION-GLOBAL (2026-09-09): los PPP, cables y capas
`TILE_SEQ_LINE` de estaciones rail vanilla ya son parents reales del
compositor global, y el vidrio `1083`–`1086` se vincula como child de su techo.
Kale scoped suma 195 tuplas parent candidatas sin perder ninguna previa y la
intersección exacta normalizada con OpenTTD pasa de 1.136 a 1.330. Normal
mejora de 10,176 % a 10,110 % de píxeles distintos; la matriz de seis zooms no
es monótona (en `2×` empeora 144 píxeles), por lo que no se declara paridad ni
se cierran #326 o #561. Waypoints rail y layouts NewGRF permanecen fuera hasta
publicar sus contracts completos de parents/children; el detalle cuantitativo
canónico queda en `PARIDAD.md`.

Actualización #326-ROAD-STOP-GLOBAL (2026-09-09): las capas BUILD vanilla
`TILE_SEQ_LINE` de paradas Bus/Truck pasan a ser parents reales del compositor
global, con sus prismas nativos y profundidad fuente. Se reservan los ordinales
de fundación/catenaria, pero catenaria vial, waypoints, depósitos y layouts
NewGRF siguen fuera de este corte. Kale scoped añade 56 registros visibles
`5978`–`5983`, y la intersección exacta normalizada con OpenTTD sube de 1.330 a
1.385; la traza nativa repite algunas geometrías entre segmentos, de modo que
no se infiere cardinalidad cruda uno a uno. El raster normal empeora de 10,110
a 10,116 % (93.173→93.226 píxeles) y cinco de seis zooms también empeoran
levemente; sólo `4×` mejora 15 píxeles. No se declara mejora raster, paridad ni
cierre de #326; el detalle cuantitativo canónico queda en `PARIDAD.md`.

Actualización #326-ROAD-DEPOT-GLOBAL (2026-09-09): las fachadas BUILD vanilla
de depósito vial (`1408`–`1413`) ya son parents globales, con prisma
`TILE_SEQ_LINE`, ancla NFO y profundidad fuente; la foundation conserva el
ordinal 0 en pendiente y el suelo queda como child. Kale scoped añade dos
`1408` y dos `1409`, no pierde tuplas y sube la intersección normalizada de
1.385 a 1.389. Los píxeles distintos no empeoran en los seis zooms: quedan
iguales en `0,25×`/`0,5×` y bajan 19/36/6/2 en `1×`/`2×`/`4×`/`8×`; el delta
medio normal aumenta marginalmente de 3,674282 a 3,674402. No se declara
paridad ni cierre de #326: overlays custom/tranvía, catenaria y waypoints
viales siguen fuera, y el detalle cuantitativo canónico queda en `PARIDAD.md`.

Actualización #326-TUNNEL-CATENARY-GLOBAL (2026-09-09): cada cable de entrada
de túnel ferroviario eléctrico (`5656`/`5658`) es ahora el parent global de su
`SpriteCombine`; la fachada frontal vanilla/Action5 y el posible
`RTSG_TUNNEL_PORTAL` son children del mismo bloque. Si no se puede resolver el
PNG del cable, la fachada conserva su parent propio. En Kale scoped, el runtime
aporta cuatro cajas de cable visibles, no pierde ninguna tupla nativa de esa
familia y la intersección exacta normalizada sube de 1.389 a 1.392. Los seis
zooms no empeoran (`0,25×` sin cambio; `0,5×`/`1×`/`2×`/`4×`/`8×` bajan
12/13/31/5/1 píxeles); el delta medio normal baja de 3,674402 a 3,668558.
No se declara paridad ni se cierra #326 o #561: siguen fuera el conjunto de
producers/children, clipping, pivotes y framebuffer global. La evidencia
cuantitativa canónica queda en `PARIDAD.md`.

Actualización #326-CAPTURE-FREEZE (2026-09-09): el proceso de screenshot crea
`VisualCaptureFreeze` antes de entrar a `InGame`, por lo que los ticks fijos
no pueden mutar el SAV durante el ciclo en que el subestado todavía nace como
`Running`. La pausa visible sigue aplicándose normalmente, pero los frames
animados de aeropuerto ya no avanzan tres ticks antes de la captura: Kale
publica de nuevo `2685`/`2678`, sus cajas nativas, y el PNG limpio de 40 y 180
frames tiene el mismo hash. Cinco zooms bajan los píxeles distintos; `8×`
sube 48 píxeles y queda documentado, sin inferir mejora global. #326 y #561
siguen abiertos; el detalle numérico queda en `PARIDAD.md`.

Actualización #326-PAINT-RECT-CULLING (2026-09-09): el culling preciso de un
parent no vacío usa ahora el rectángulo que Bevy rasteriza
(`custom_size`, atlas/recorte, anchor, escala y rotación), igual que el
clipping previo de `AddSortableSpriteToDraw`; no usa un margen fijo sobre el
prisma 3D. Los `SPR_EMPTY_BOUNDING_BOX` conservan su contrato de prisma y los
assets todavía no resueltos el fallback geométrico. En Kale `(189,126)`,
`1280×720`, `clean-static`, la intersección parent normalizada sube de 1.552 a
1.570, los faltantes nativos bajan 29→11 y los candidatos extra 116→100; el
caso Maglev de borde vuelve a incluir `1235`/`1241`/`1244`/`1246`. La matriz
actual de píxeles distintos es 352.960 / 396.048 / 100.151 / 524.417 /
750.195 / 692.486 en `0,25×`/`0,5×`/`1×`/`2×`/`4×`/`8×`; Normal reduce 31
píxeles pero su delta medio sube marginalmente. Es corrección de selección,
no de paridad raster: #326 y #561 siguen abiertos y el detalle canónico queda
en `PARIDAD.md`.

Actualización #326-ROAD-WAYPOINT-GLOBAL (2026-09-09): los cuatro postes
vanilla `TILE_SEQ_LINE` de road waypoint (`6141`–`6144`) pasan a ser parents
del compositor global en plano, con los prismas de `station_land.h`, profundidad
fuente y ordinales 2/3. En pendiente quedan como children de la foundation
nivelada y conservan el orden local. La regresión cubre el spawn ECS X, las
cajas absolutas X/Y y el vínculo inclinado. Kale no tiene un waypoint vial
focal; no se reclama métrica raster ni se altera la matriz de seis zooms. #326 continúa abierto por
catenaria vial, layouts custom/NewGRF, waypoints rail, clipping, pivotes,
children globales y framebuffer.

Actualización #326-ROAD-CATENARY-GLOBAL (2026-09-09): las calles normales
publican ya los cuatro `AddSortableSpriteToDraw` de cada
`DrawRoadTypeCatenary` como parents globales: tres recortes traseros con sus
columnas `1×1×z_wires` y el frente `16×16×1`. La base efectiva de la
foundation, bounds literales de `road_cmd.cpp`, ancla NFO y profundidad fuente
se conservan tanto para road como para tram; los ordinales 4–11 mantienen el
stream carretera → tranvía antes de roadside. Los grupos
`ROTSG_CATENARY_*` resueltos usan el mismo contrato. La regresión ECS verifica
los ocho parents road+tram y sus prismas, y la unitaria cubre la pendiente. No
se midió raster nuevo: Kale no ofrece foco vial de catenaria para este corte.
Paradas y waypoints viales quedan deliberadamente locales hasta reconciliar
sus ordinales BUILD, por lo que #326 continúa abierto junto con clipping,
pivotes, children y framebuffer global.

Actualización #326-ROAD-WAYPOINT-CATENARY-GLOBAL (2026-09-09): el waypoint
vial vanilla sin `TileLayout` custom usa ahora el mismo stream global de
`DrawRoadTypeCatenary` que una calle: columnas traseras y frente ocupan los
ordinales 4–11 y conservan los bounds literales, la base efectiva y la
profundidad fuente. Sus dos `TILE_SEQ_LINE` BUILD pasan a 12/13, respetando el
orden nativo catenaria → BUILD. En pendiente la catenaria es parent a la
altura nivelada y el suelo/postes siguen como children de la foundation. Las
regresiones ECS cubren ambos casos; Kale no contiene un foco vial reproducible,
por lo que no se reclama métrica raster. Las paradas Bus/Truck y los layouts
custom continúan locales para una etapa posterior. #326 no se cierra.

Actualización #326-ROAD-STOP-CATENARY-GLOBAL (2026-09-09): las paradas
Bus/Truck vanilla sin `TileLayout` custom emiten catenaria road/tram antes de
sus capas BUILD, como `DrawTile_Station` nativo. Cada columna/frente usa el
stream global 4–11 con bounds literales, base efectiva y profundidad fuente;
BUILD empieza en 12. La prueba ECS cubre una drive-through plana y una inclinada
nivelada, incluidos bounds, orden y el child de foundation del suelo. Los
layouts custom de paradas/waypoints conservan sus ordinales locales para una
etapa posterior.
Kale no aporta un foco vial reproducible ni se reclama métrica raster; #326 no
se cierra.

Actualización #326-ROAD-STOP-STATIC-TILELAYOUT-GLOBAL (2026-09-09): una
parada Bus/Truck con `TileLayout` NewGRF completo y materializable ya comparte
el mismo contrato: `DrawRoadCatenary` ocupa 4–11 y su secuencia BUILD empieza
en 12, manteniendo el prisma `TILE_SEQ_LINE`, profundidad fuente y el vínculo
del child con su parent NewGRF. Los layouts incompletos —sprites base o paletas
que el cliente no puede materializar atómicamente— conservan la ruta local y
su fallback previo; los road waypoints con layout NewGRF no forman parte de
este corte. La regresión ECS comprueba el cable road/tram, los bounds exactos,
el ordinal 12 y las tres texturas Action1 ground/parent/child. Kale sigue sin
un foco vial reproducible, por lo que no se atribuye una métrica raster ni se
cierra #326.

Actualización #326-ROAD-WAYPOINT-STATIC-TILELAYOUT-GLOBAL (2026-09-09): el
road waypoint con `TileLayout` NewGRF completo y materializable ya sigue la
misma secuencia nativa: su suelo `WaypGround`, catenaria road/tram 4–11 y
parents BUILD desde 12. Se preservan el prisma `TILE_SEQ_LINE`, profundidad
fuente y child del parent NewGRF; la regresión ECS reutiliza el layout Action1
para comprobar paradas Bus/Truck y un RoadWaypoint con bounds/ordinales exactos. Un
layout incompleto conserva la catenaria directa y el fallback anterior; no se
declara equivalencia raster ni se cierra #326.

| Issue | Situación real al dejar este corte | Próxima brecha acotada |
|---|---|---|
| [#326](https://github.com/cavazquez/openttdrs/issues/326) | La composición raster global sigue abierta. El sorter incorpora `TileLayoutSpriteGroup` de AirportTile; parents globales para PPP/cables/capas `TILE_SEQ_LINE` rail vanilla, paradas Bus/Truck y road waypoints vanilla o con `TileLayout` NewGRF materializable, depósitos viales, cable de entrada de túnel eléctrico y catenaria road/tram de calles normales. El vidrio rail y los postes en pendiente conservan su relación child con el parent correspondiente. `VisualCaptureFreeze` evita falsos positivos animados y el culling usa el rectángulo real de Bevy, pero ninguno equivale a paridad raster. Kale no contiene un foco vial de waypoint/catenaria reproducido, así que esas migraciones sólo tienen evidencia estructural. Foundations/rotaciones aeroportuarias, layouts incompletos de road stop/waypoint, waypoints rail, overlays custom/tranvía de depósitos, sprite-stack, clipping, pivotes y framebuffer siguen sin equivalencia global; el contrato global completo de children queda separado en [#561](https://github.com/cavazquez/openttdrs/issues/561). | Delimitar y reproducir el fallback de `TileLayout` incompleto en road stops/waypoints antes de globalizarlo; repetir seis zooms si se altera viewport, culling u overview. |
| [#328](https://github.com/cavazquez/openttdrs/issues/328) | El round-trip preserva las tablas y campos escalares modelados, `CITY`/`INDY`/`STNN`/`PSAC`, `OBJS`/`OBID`, grupos, órdenes y autoreplace en el subconjunto documentado. Las mutaciones de strings, listas o structs anidados con schema y tamaño codificado idénticos ya se fusionan sobre el payload importado y conservan columnas hermanas desconocidas; `imported_plyr_equal_sized_name_change_keeps_raw_header` ejerce ese camino sobre `PLYR.name` y su SAV resultante carga/re-guarda en OpenTTD dedicado. `legacy_imported_plyr_compatible_colour_change_keeps_raw_header` cubre un `PLYR` histórico que omite campos modernos, y `legacy_imported_city_compatible_name_parts_change_keeps_raw_header` hace lo propio con `CITY.townnameparts`: los snapshots de importación permiten modificar esos campos compatibles, preservan las cabeceras legacy y cargan/re-guardan en OpenTTD dedicado. [#371](sav-rename-371.md) añade renombrados de strings raíz con otra longitud, preservando cabecera y columnas ajenas; incluye regresión nativa y re-guardado por OpenTTD. Una mutación de un campo omitido/incompatible, de longitud de lista/struct anidado o de forma/topología todavía degrada al writer canónico. Pools nativos de casas/objetos y labels/cargos no representables siguen pendientes. CB17 de casas y CB157 de objetos pueden crear/modificar PSA de pueblo y el writer les asigna una fila `PSAC`/referencia `CITY` al exportar. | Elegir una mutación SAV reproducible de lista/struct anidado y comparar bytes OpenTTD→Rust→OpenTTD. |
| [#329](https://github.com/cavazquez/openttdrs/issues/329) | `CITY.received` hidrata crecimiento; la producción de casas escribe `CITY.supplied`; `0xBA`–`0xCB` leen producción/transporte y el PSA de pueblo se selecciona por GRFID en scopes parent de casas/objetos. CB17/CB157 evalúan y escriben el parent real. CB25/26/27 comparten contexto y PSA por huella: `TileLoop` sólo sobre visitas, `IndustryTick` al intervalo de producción, `CargoReceived` al completar la entrega y el avance de frames queda separado por tick. Shape-check `CB2F`, foundations `CB30`, autoslope manual `CB3C`, color `CB14A`, rechazo `CB3D`, cargos dinámicos `CB14B`/`CB14C`, `CargoTypesUnlimited` (hasta 16 slots, con salidas extra procesadas/transportables/exportables), slots vacíos `INVALID_CARGO` del modo legacy, rehidratación de filas `INDY` al aplicar el catálogo NewGRF, efectos especiales `CB3B`, `PlantOnBuild` manual/NewGRF, afterload SAV `<SLV_32`, entrega directa tipo `DeliverGoodsToIndustry` y el monitor runtime `AddCargoDelivery` ya tienen call sites y regresiones. La descarga ordena por `DistanceMax`, excluye la industria de origen, respeta el límite `uint16` de waiting, consulta `CBID_INDUSTRY_REFUSE_CARGO`, actualiza fecha/flag de aceptación y difiere la producción hasta después de `load_vehicles`, con rutas CB1, CB2 exclusivo y matriz vanilla. Los historiales aceptados y producidos por salida giran 61 registros nativos y se reemiten para cargos representables; el monitor empaqueta IDs con el layout nativo, exige activación, satura a `i32` y reinicia al consultar. `STNN.base.owner`, `INDY.neutral_station`/`exclusive_supplier` y `serve_neutral_industries` ya tienen importación, runtime y round-trip. Las filas `INDY` de cargos no resolubles se conservan ahora como passthrough opaco (slot, stock, rate e historial), pero no son ejecutables sin su catálogo. `CargoDistributed`/`ConstructionStageChanged` ya tienen call sites y la aceptación exacta de teselas de industria (`CBID_INDTILE_CARGO_ACCEPTANCE`/`CBID_INDTILE_ACCEPT_CARGO`) ya alimenta la cobertura de estación y la descarga. Las órdenes de estación ya ejecutan refit manual/auto antes de cargar y conservan sentinels SAV; el callback visual avanzado `CB160` ya decodifica registros/flags y se materializa en el renderer de trenes, pero quedan el balanceo/reserva completo de `HandleStationRefit`, siguiente estación, articulados heterogéneos y UI, además de road/ship/air, sprites/sonidos locales y auto-centrado exacto de efectos. Sigue faltando el modelo de cargos custom ejecutables/CTT completo, bindings de GameScript equivalentes a `ScriptCargoMonitor`, reatachación económica cuando falta su catálogo, GUI/variables ilimitadas, autoslope en generación automática, sonido, mutaciones económicas fuera de esos caminos y el resto de callbacks/scope. | Medir con el oráculo un caso de auto-refit con dos unidades/cargos distintos y una siguiente estación; no cerrar #329 por este subconjunto. |
| [#330](https://github.com/cavazquez/openttdrs/issues/330) | Economía básica y movimiento funcionan, pero los oráculos externos todavía son acotados. Tráfico/colisiones/dirección vial exhaustivos, PBS/YAPF/presignals/consist ferroviarios y navegación aire/mar no tienen aún cobertura diferencial completa. | Tomar el primer fixture externo reproducible de movimiento y registrar tick, entidad y estado nativo divergente. |
| [#331](https://github.com/cavazquez/openttdrs/issues/331) | Locale `es`/`en`, etiquetas estáticas, errores de comandos y el panel de órdenes cambian en vivo. Este último cubre controles, título/pool/hint y filas dinámicas (modos, horarios, refit de carga, incompatibilidades y falta de ruta); la lista de Órdenes compartidas también alterna título, hint y contadores sin traducir IDs ni datos de las órdenes. Liga invalida además sus filas cuando sólo cambia el locale, traduciendo sus etiquetas y conservando el nombre de compañía. Subvenciones traduce su chrome y estados Offer/Active, mientras conserva como datos cargos, compañías e industrias. La configuración de Noticias localiza sus ocho categorías y el modo `Newspaper`, sin traducir titulares/cuerpos generados. IA / TransCargo localiza el resumen dinámico, pero conserva nombres de compañía, importes, cargos, rutas y coordenadas. Opciones de visualización traduce toggles, presets y categorías `TO_*`, y su viewport con scrollbar clásico conserva las acciones inferiores en 720 px. CargoDist traduce explicación y modos sin alterar Demand/MCF ni el estado de carga. La configuración NewGRF traduce controles, guía y estado de parámetro, pero preserva nombres, GRFID, paths y reportes técnicos. Señales PBS traduce título y selector de espera; los valores de pathfinding siguen siendo los mismos. Autoreemplazo cubre chrome, hint y flags de regla, sin modificar nombres de motores. El depósito traduce título, botones de chrome y unidad de antigüedad, y conserva nombre, carga, capacidad y coordenadas del vehículo. La vista de estación traduce clases, filtros, resúmenes, botones y tooltips, preservando nombres de estación, empresa, cargos y coordenadas. Sonido y música traduce volúmenes, reproducción y controles, mientras los títulos de pista se conservan literales. El sub-issue #370 añade la entrada, cabecera, controles y estado dinámico de trucos; formatea la fecha de presentación de CheatWindow, statusbar y toolbar del editor según el locale, y materializa siempre el estado vacío de objetivos sin traducir datos de GameScript/jugadores. La ayuda integrada completa también alterna en vivo, conserva comandos/hotkeys literales y usa viewport con scrollbar clásico para no exceder una pantalla baja. Story alterna título, fallback y navegación, pero conserva literalmente títulos/cuerpos de páginas GameScript. El campo persistido acepta además los filenames que OpenTTD 15.3 guarda para esos dos packs (`english*.lng`/`spanish*.lng`), con la misma normalización segura de ISO. Siguen pendientes cuerpos/titulares generados, catálogos upstream completos, settings no modelados y la paridad UI sin colisiones ECS. | Auditar un catálogo/setting guardado contra OpenTTD y añadir una regresión de cambio de idioma. |
| RMAP-004 y padres abiertos | Las cohortes auditadas de mapas (64²→512² y cortes ampliados) son exactas por tesela y bloques 4×4, pero eso no generaliza a todas las semillas, tamaños, climas, settings de ríos ni ticks posteriores. | Ampliar la matriz combinatoria sólo cuando exista una primera divergencia reproducible; no convertir una cohorte exacta en cierre del generador. |

Corrección vigente #371–#374 (2026-09-05): la fila #328 de arriba queda
ampliada: strings, listas escalares raíz y struct-lists raíz con descriptor
recursivamente idéntico pueden cambiar de longitud sin descartar cabecera ni
columnas ajenas. `CITY.psa_list` agrega una fila `PSAC`; `CITY.supplied` añade
un cargo con historia interna, preserva los demás bytes `CITY` y OpenTTD
dedicado vuelve a guardarlo al anunciar SLV 358. La frontera sigue excluyendo
subschemas desconocidos/incompatibles, cambios de filas/índices y topología;
#374 normaliza además `INDY.accepted`/`produced` a sus 61 registros nativos
(salvo historia aceptada aún nula), conservando opacas las filas no
resolubles. #328 permanece abierto. La evidencia de este último caso y su
reproducción están en [sav-indy-history-374.md](sav-indy-history-374.md).

Corrección vigente de la tabla: `566ce56a` resuelve la codificación global de
cargos modernos en SAV y conserva los slots climáticos de saves anteriores a
`SLV_55`. Esta nota prevalece sobre las filas históricas que todavía describen
los cargos custom como exclusivamente opacos.

Corrección vigente adicional (`a2a0ce35`): el rango ejecutable de cargos
custom es `31..63`, alineado con `NUM_CARGO = 64`; el slot 63 ya se hidrata y
se reemite en las tablas SAV modernas. El JSON propio usa v27 y acepta los
arrays de 32 slots de versiones anteriores. Sólo permanecen opacos los IDs
fuera de la tabla nativa (`64+`), que no son CargoType válidos de OpenTTD.

Corrección de la tabla en `b25a2362`: la CTT de cargos custom ya es ejecutable
en las variables parametrizadas de estaciones y paradas viales cuando el
catálogo está instalado. El residual de `#329` queda acotado a callbacks
CB140–142, `AirportTiles`, industria, GUI/variables ilimitadas y otros scopes
que todavía no reciben ese catálogo.

Corrección vigente de la fila visual (`25d026a7`): la frase histórica que
limitaba CB160 a trenes y la ruta estándar CB10 a trenes queda superada. Ambos
call sites comparten ahora la emisión vanilla entre trenes, carretera, barcos y
aeronaves; siguen pendientes la composición de sprites/sonidos locales,
consists y sorter/viewport. La proyección isométrica de `x/y/z` ya se aplica a
CB10 y CB160, con `z` reservado a la altura visual y no al orden sortable.

Actualización #329-CARGO-CTT-067 (2026-09-04, commit `7782568d`): las rutas
runtime de animación de estaciones ferroviarias/waypoints (`CB140`–`CB142`)
reciben ahora el catálogo de `CargoSpec` activo. `param2` de `NewCargo` y
`CargoTaken` traduce cargos custom por la CTT declarada por el GRF, y las
variables de Action2 `60`–`69` usan el mismo catálogo para cada tesela; la
propagación cubre construcción, eventos económicos, carga de vehículos y el
scheduler `TileLoop`. La regresión de `TOFU` fija el índice local 6 en ambas
teselas de una plataforma. AirportTiles, industria y GUI/variables ilimitadas
siguen pendientes y #329 no se cierra.

Actualización #329-CARGO-CTT-068 (2026-09-04, commit `9606544b`): los eventos
de animación `AirportTile` que recorren una estación propagan el catálogo
`CargoSpec` activo. `NewCargo`/`CargoTaken` traducen cargos custom mediante la
CTT del GRF para `param2`; construcción y descarga utilizan la variante
catálogo-aware y la regresión de `TOFU` fija el índice local 6. Las APIs
directas sin catálogo mantienen el fallback legacy. Industria,
GUI/variables ilimitadas, foundations, rotaciones y sonidos aún permanecen
abiertos.

Actualización #329-CARGO-CTT-069 (2026-09-04, commit `b80b8362`): `CB3D`
(`IndustryRefuseCargo`) recibe resolución de labels custom contra el catálogo
`CargoSpec` activo. Se aplica en la descarga a industrias y en el procesamiento
de insumos desde estaciones, incluyendo instancias SAV sin slots de entrada
rehidratados; los wrappers legacy mantienen el fallback sin catálogo. La
regresión `TOFU` fija `param2=6`. CB1/CB2 de producción, tipos dinámicos y
aceptación de `IndustryTile` siguen pendientes de la misma propagación.

Actualización #329-CARGO-CTT-070 (2026-09-04, commit `391b35d9`): las rutas
runtime de producción industrial CB1/CB2 (`IndustryProductionSpriteGroup`) y de
tipos dinámicos CB14B/CB14C reciben el catálogo activo de `CargoSpec`. Labels
custom como `TOFU` se resuelven en slots, multiplicadores y grupos de producción
incluso para instancias SAV sin slots previamente hidratados; los wrappers legacy
mantienen el fallback histórico. La aceptación de `IndustryTile`, GUI/variables
ilimitadas y el resto de callbacks de industria siguen pendientes; #329 continúa
abierto.

Actualización #329-CARGO-CTT-071 (2026-09-04, commit `e67b1171`): la aceptación
exacta de `IndustryTile` (`CB2B`/`CB2C`) y la cobertura/descarga de estación
reciben el catálogo `CargoSpec` activo. Los labels custom del tile y de su
industria parent se resuelven aunque el SAV no haya hidratado los slots de
entrada; los wrappers legacy mantienen el fallback histórico. La regresión
`TOFU` confirma que la ruta catálogo-aware acepta el cargo correcto y evita el
alias vanilla `Mail`. GUI/variables ilimitadas, sonidos y scopes avanzados
siguen pendientes; #329 continúa abierto.

Actualización #329-CARGO-CTT-072 (2026-09-04, commit `85db7852`): las variables
de scope parent de `IndustryTile` (`0x40`–`0x47`, `0x69`–`0x71`, `0x88`–`0x90` y
sus historiales) resuelven labels custom contra el catálogo `CargoSpec` activo,
aunque una instancia importada de SAV todavía no tenga hidratados sus slots.
Renderer, shape-check, autoslope y construcción pasan el catálogo explícito y
la regresión `TOFU` comprueba stock, waiting y cargos producidos/aceptados sin
alias a `Mail`; las APIs legacy siguen con fallback sin catálogo. Animación,
randomización, GUI/variables ilimitadas, sonido y scopes avanzados son el
siguiente bloque; #329 sigue abierto.

Actualización #329-CARGO-CTT-073 (2026-09-04, commit `e1f698d3`): los callbacks
de animación `IndustryTile` (`CB25`/`CB26`/`CB27`) reciben el catálogo activo en
todos los eventos runtime (`TileLoop`, `IndustryTick`, `ConstructionStageChanged`,
`CargoReceived` y `CargoDistributed`) y en el avance de frames visuales. El
resolver descubre las variables parametrizadas usadas por Action2, incluyendo
`0x69`–`0x71`, y pasa la CTT al scope parent aunque el SAV no haya hidratado sus
slots. La regresión `TOFU` comprueba stock=23 en CB26; APIs legacy sin catálogo
mantienen fallback. Randomización, GUI/variables ilimitadas, sonido y scopes
avanzados son el siguiente bloque; #329 sigue abierto.

Actualización #329-CARGO-CTT-074 (2026-09-04, commit `a6f561b6`): la ruta de
randomización `IndustryTile` (`ResolveRerandomisation`) recibe el catálogo
`CargoSpec` en generación, `TileLoop`, `IndustryTick` y los eventos de carga.
El helper descubre las variables parent que usa el grafo, de modo que
`0x69`–`0x71` seleccionan cargos custom por CTT aunque falten slots hidratados en
un SAV. La regresión `TOFU` verifica el reseed parent con catálogo y que el
wrapper legacy sin catálogo no tome la rama custom. GUI/variables ilimitadas,
sonidos, reatachación sin GRF y scopes restantes continúan abiertos.

Actualización #329-CARGO-TRAIN-WEIGHT-063 (2026-09-04, commit `b32b87f4`):
`ConsistChanged` acumula `CargoSpec::weight` por unidad y refresca
`cached_weight_t`/esfuerzo tractor después de `LoadUnloadStation`. Esto cubre
trenes vanilla y NewGRF, incluida carga custom con catálogo activo, y conserva
las APIs legacy que no reciben catálogo. La regresión usa 8 unidades de peso
32 (16 toneladas) y el probe de señales sigue pasando; `freight_trains` y las
propiedades CTT/económicas restantes siguen pendientes.

Actualización #329-CARGO-FREIGHT-SETTING-064 (2026-09-04, commit `15c8bfcf`):
`vehicle.freight_trains` ya forma parte de `GameState` y del JSON propio con
default 1. El parser/escritor `PATS` usa el tipo `UINT8`, respeta `1..=255` y
la compatibilidad `SLV_39`; un save legacy conserva el valor histórico por
default. `CargoSpec::is_freight` limita la escala a cargas freight y todos los
rebuilds de consist reciben el multiplicador, incluido el refresh posterior a
`LoadUnloadStation`. Regresiones cubren parser, round-trip, saves legacy y
peso; la GUI, CTT y otros settings siguen abiertos.

Actualización #329-CARGO-FREIGHT-UI-065 (2026-09-04, commit `5e0938ff`):
`SetFreightTrains` quedó disponible como comando de partida: normaliza a
`1..=255`, recalcula todos los consist ferroviarios y no modifica el mapa. La
toolbar de Ajustes ofrece presets `1/2/4/8/16/32/64/128/255`, etiqueta el valor
actual y vuelve a `1` tras `255`; las regresiones cubren ciclo, wrap y refresco
de peso. Falta la ventana avanzada con edición arbitraria y el resto de
settings económicos.

Actualización #329-CARGO-CTT-066 (2026-09-04, commit `b25a2362`): las CTT
explícitas ya se invierten contra el label real del `CargoSpec` para cargos
custom; `StationScopeResolver` y `RoadStopScopeResolver` recorren esos cargos
en las variables parametrizadas `60`–`65`/`69` cuando el renderer aporta el
catálogo activo. Las APIs legacy sin catálogo mantienen el fallback histórico.
Quedan por propagar el catálogo a CB140–142, `AirportTiles`, industria y la
GUI/variables ilimitadas; #329 sigue abierto y esta etapa no cierra el issue
padre.

Última validación de `a6f561b6`: `cargo fmt --all -- --check`, clippy estricto
de core y cliente, **2.028** tests de core y **1.067** de cliente (2 ignorados); la matriz
documental se actualiza en este corte,
`check_parity_docs_fresh.sh` y `git diff --check` pasan. Las fechas y
afirmaciones históricas inferiores no sustituyen este handoff.

Actualización #329-TOWN-PSA-031 (2026-09-03, commit `bd3ea9c1`): el callback
`CBID_HOUSE_ALLOW_CONSTRUCTION` (`0x17`) de una casa en crecimiento recibe la
tesela candidata y el `TownScopeResolver` parent real. Los grupos Action2
parent (`0x82`/`0x86`/`0x8A`) escriben ahora `\2psto` en
`Town.newgrf_persistent_regs` por GRFID, sin tocar el storage propio de la
casa; el writer ya puede asignar esa fila y su referencia `CITY.psa_list` al
exportar `PSAC`. Las regresiones cubren rechazo de construcción, aislamiento
entre GRFIDs y la dirección correcta del operador. #329 sigue abierto por el
writeback de objetos/teselas, historiales mutables, cargos custom y callbacks
restantes.

Actualización #329-OBJECT-PSA-032 (2026-09-03, commit `9303cf65`): CB157 de
construcción de objetos recibe ahora el `TownScopeResolver` parent del pueblo
más cercano. Los grupos Action2 parent persisten `\\2psto` por GRFID en una
copia de cada pueblo durante query/preview; el execute conserva esa copia sólo
después de comprobar fondos, por lo que un preview o una orden sin dinero no
contamina el estado. La regresión cubre writeback, aislamiento por GRFID y
ambas rutas (financiada/sin fondos). El writeback de callbacks de teselas y
otros callbacks/scope de objetos sigue pendiente; #329 no se cierra.

Actualización #329-INDTILE-PSA-033 (2026-09-03, commit `47afecd7`): los
callbacks `CBID_INDTILE_ANIMATION_TRIGGER/NEXT_FRAME/SPEED` (`0x25`–`0x27`)
de las teselas `NewGRF` se ejecutan ahora en la ruta normal con el
`IndustryTileResolverObject` equivalente: etapa, terreno, posición, vecinos,
badges y scope parent de industria. `\\2psto` del parent se hidrata desde la
instancia viva y se escribe de vuelta a `Industry.newgrf_persistent_regs` tras
cada callback; una regresión directa y otra del scheduler fijan el writeback y
la asociación por `m2`/footprint. La API antigua conserva el fallback sin mundo.
La randomización `CBID_RANDOM_TRIGGER`, foundations de render y callbacks de
sonido/slope/autoslope siguen pendientes; #329 permanece abierto.

Actualización #329-INDTILE-RANDOM-034 (2026-09-03, commit `601e7685`): la
re-randomización `Action2` de `IndustryTile` ya se ejecuta con el parent
`Industry` vivo en la ruta `TileLoop`. El scheduler hidrata el PSA antes de
`ResolveRerandomisation`, persiste `\\2psto` después de evaluar el grupo y
mantiene la asociación por `m2`/footprint incluso cuando varias teselas
comparten una industria. La API histórica sin catálogo/world continúa como
fallback explícito. Siguen pendientes los triggers `IndustryTick` y
`CargoReceived`, además de foundations/sonido/slope/autoslope; #329 no se
cierra por este subconjunto.

Actualización #329-INDTILE-TRIGGERS-035 (2026-09-03, commit `916247a2`): los
call sites económicos de `CargoReceived` y `IndustryTick` dejaron de usar el
fallback vanilla. Cada trigger recorre la huella viva de la industria, hidrata
el PSA parent antes de `ResolveRerandomisation`, conserva los triggers no
consumidos y persiste `\\2psto`; las máscaras `0x83` se agregan y reseedean una
sola vez en `Industry.newgrf_random` después de evaluar toda la huella. La
regresión cubre ambos triggers, el writeback PSA y una huella de dos teselas.
Siguen pendientes foundations/sonido/slope/autoslope, historiales mutables y
cargos custom; #329 continúa abierto.

Actualización #329-INDTILE-ANIMATION-055 (2026-09-03, commit `c88518c4`):
`IndustryAnimationTrigger` modela los cinco ordinales de `industry_type.h` y
CB25 ya recibe el evento correcto en los call sites reales. `TileLoop` se
dispara sólo para las visitas del tile loop, `IndustryTick` sólo al vencer la
producción y `CargoReceived` después de procesar la entrega; la pasada visual
usa una API separada que avanza CB26/CB27 únicamente sobre teselas activas.
Una regresión demuestra que un tick visual no activa CB25. `CargoDistributed`,
`ConstructionStageChanged`, sonido, scopes restantes y cargos custom siguen
pendientes; #329 no se cierra.

Actualización #329-INDTILE-ANIMATION-056 (2026-09-03, commit `aa289076`):
`CargoDistributed` se dispara ahora sólo cuando
`TransportIndustryGoods` devuelve unidades realmente entregadas a estaciones;
el callback recorre la huella de la industria y conserva el mismo contexto
parent/PSA. La máscara y el ordinal son independientes de `IndustryTick`, con
regresión directa para ambos caminos. La nota 057 conecta después
`ConstructionStageChanged`; sonido, scopes restantes y cargos custom siguen
pendientes; #329 no se cierra.

Actualización #329-INDTILE-ANIMATION-057 (2026-09-03, commit `ca2939a7`):
`ConstructionStageChanged` ya tiene call sites en la construcción inicial y en
los cambios de etapa observados por `TileLoop`. La primera llamada conserva el
flag upstream `var 18 |= 0x100`; las transiciones posteriores usan el ordinal
sin extensión. Ambos caminos hidratan el parent/PSA de la industria y tienen
regresión de callback. Quedan sonido, scopes restantes, cargos custom y
mutaciones económicas fuera de estos caminos; #329 no se cierra.

Actualización #329-INDTILE-SLOPE-036 (2026-09-03, commit `9e01c1a9`): el
parser de `IndustryTiles` conserva `prop 0x0D` (`slopes_refused`) y los bits
upstream de shape-check (`0x2F`), foundations (`0x30`) y autoslope (`0x3C`).
La colocación NewGRF ejecuta `CBID_INDTILE_SHAPE_CHECK` por tesela con el tipo
de creación y layout en `param2`, un parent temporal que conserva huella,
tipo, random y fundador, y el fallback `IsSlopeRefused` cuando el callback
falla. La inversión de booleano anterior a GRF v7 y la aceptación exclusiva
de `0x400` desde v7 tienen regresiones; el renderer ya usa el ID correcto
`0x30` para foundations. El call site manual de terraformación/autoslope queda
publicado en `fe70a433`; la generación automática y los callbacks de sonido
siguen pendientes; #329 no se cierra.

Actualización #329-INDTILE-AUTOSLOPE-037 (2026-09-03, commit `fe70a433`):
`CBID_INDTILE_AUTOSLOPE` (`0x3C`) se ejecuta en el preflight de
`raise_land`, `lower_land` y `level_land` cuando la guarda de
`TerraformTile_Industry` conserva el máximo absoluto y ambas pendientes no
son empinadas. `CALLBACK_FAILED`/cero permite conservar la industria; un valor
no nulo deja continuar la limpieza normal. El contexto Action2 usa la
`Industry` viva, asocia por `m2`/huella y persiste el PSA `7C`. La regresión
comprueba una subida de esquina que mantiene la industria y el rechazo por
callback. La generación automática y sonido de `IndustryTile` siguen siendo
la siguiente brecha; #329 continúa abierto.

Actualización #329-INDUSTRY-COLOUR-038 (2026-09-03, commit `63d37f04`):
`CBID_INDUSTRY_DECIDE_COLOUR` (`0x14A`) se ejecuta al fundar una industria
NewGRF, después de inicializar su parent. Sólo un resultado con bits 4..14 en
cero reemplaza el color sorteado por su nibble bajo; `CALLBACK_FAILED` o un
resultado inválido conservan el color vanilla. El callback persiste `7C` y las
regresiones cubren la semántica del resultado y la colocación real. Efectos
especiales, cargos dinámicos, sonido y generación automática siguen pendientes;
#329 permanece abierto.

Actualización #329-INDUSTRY-REFUSE-039 (2026-09-03, commit `65682a42`):
`CBID_INDUSTRY_REFUSE_CARGO` (`0x3D`) se consulta antes de retirar cada lote
de entrada NewGRF desde las estaciones. `param2` recibe el índice local
traducido por CTT; los resultados no nulos aceptan, cero rechaza y
`CALLBACK_FAILED` conserva el fallback. Las regresiones comprueban el índice,
la inversión booleana y la conservación del stock ante rechazo. La cobertura
actual está acoplada al ciclo de procesamiento: falta modelar la entrega
directa/monitor `DeliverGoodsToIndustry` y su temporización, además de efectos
especiales, cargos dinámicos, sonido y generación automática; #329 sigue
abierto.

Actualización #329-INDUSTRY-CARGO-TYPES-040 (2026-09-03, commit `389109c1`):
`CBID_INDUSTRY_INPUT_CARGO_TYPES` (`0x14B`) y
`CBID_INDUSTRY_OUTPUT_CARGO_TYPES` (`0x14C`) se consultan durante la
fundación NewGRF y reemplazan los slots estáticos de la instancia. `param1`
lleva el índice, la CTT valida el cargo local y `0xFF`/`CALLBACK_FAILED`
terminan la secuencia; sin runtime se conserva el fallback estático. Las
regresiones cubren los tres slots de entrada, la matriz de multiplicadores y
la lista de salida vacía. El bloque sigue limitado a 3 entradas/2 salidas:
`CargoTypesUnlimited`, cargos custom y persistencia/rehidratación SAV quedan
pendientes; #329 sigue abierto.

Actualización #329-INDUSTRY-CARGO-TYPES-041 (2026-09-03, commit `36662249`):
`prop 0x1A` de `Industries` se conserva como `IndustrySpecDef.behaviour` y el
bit `CargoTypesUnlimited` amplía `0x14B`/`0x14C` hasta 16 entradas/salidas.
Las salidas desde el tercer slot se conservan en
`newgrf_extra_output_cargos`; sus stocks se transportan y exportan por el
buffer adicional. Las regresiones cubren parseo Action0 y cuatro
entradas/salidas dinámicas. Sigue pendiente el procesamiento normal
multi-output (rates/matriz), slots vacíos legacy, cargos custom y
rehidratación runtime desde SAV; #329 sigue abierto.

Actualización #329-INDUSTRY-CARGO-TYPES-042 (2026-09-03, commit `0fddd2f4`):
la economía ya calcula tasas y multiplicadores para todas las salidas
declaradas. Las procesadoras consumen y depositan cada slot, incluidos los
extras en `newgrf_extra_produced_cargo`; la capacidad considera todos los
stocks y `INDY` exporta espera/tasa desde la tercera salida. La regresión de
cuatro entradas y cuatro salidas verifica 32 unidades por salida en un ciclo.
Quedan pendientes historial por salida, GUI/variables ilimitadas, cargos
custom y rehidratación runtime completa desde SAV; #329 sigue abierto.

Actualización #329-INDUSTRY-SPECIAL-EFFECT-043 (2026-09-03, commit `6e3ad37a`):
`CBID_INDUSTRY_SPECIAL_EFFECT` (`0x3B`) corre en el ciclo de 256 ticks para
`PlantFields` y `CutTrees`, pasando `Random()` y escribiendo `7C`. Se reutiliza
la geometría de campos y la espiral 40×40 de árboles, con fallback vanilla ante
`CALLBACK_FAILED`; `PlantOnBuild`, escalas/sonidos y goldens integrales siguen
pendientes, por lo que #329 permanece abierto.

Actualización #329-INDUSTRY-PLANT-ON-BUILD-044 (2026-09-03, commit `628d1fb9`):
la colocación manual vanilla y la fundación NewGRF con `PlantOnBuild` ejecutan
los 50 intentos de `PlantRandomFarmField` después de crear la industria,
compartiendo geometría, límites climáticos, cercas y RNG global con el resto
del runtime. Los campos quedan asociados al `IndustryID` en MAP2. Falta el
hook de afterload/rehidratación SAV, además de escalas/sonidos y goldens.

Actualización #329-INDUSTRY-PLANT-ON-BUILD-045 (2026-09-03, commit `56aa7858`):
el importador SAV conserva la identidad, posición, tipo y tamaño de las
industrias de versiones `< SLV_32` en un marcador efímero. El afterload limpia
los campos legacy con `MakeClear(CLEAR_GRASS, 3)`, ejecuta 50 intentos por
industria con `PlantOnBuild`, vuelve a ligar los campos al `IndustryID`, marca
las teselas para remap y consume el marcador una sola vez. La resolución de
definiciones custom se difiere hasta aplicar el catálogo NewGRF; si no está
instalado se mantiene el fallback vanilla y queda pendiente la reatachación
económica completa. El issue #329 continúa abierto por slots vacíos legacy,
cargos custom, historiales/GUI, escalas/sonidos y goldens.

Actualización #329-INDUSTRY-CARGO-TYPES-046 (2026-09-03, commit `9f2ecc31`):
CB14B/CB14C conserva la posición de cada slot legacy: `INVALID_CARGO` deja
`None` y permite consultar los slots siguientes, mientras los vectores
económicos compactan sólo cargos válidos y preservan el índice estático para
multiplicadores. El parser y el catálogo mantienen alineados los índices
`0xFF`; `CargoTypesUnlimited` conserva la terminación estricta ante valores
inválidos. La regresión cubre un hueco en el slot 0 seguido por COAL en el slot
1 y verifica el multiplicador 128. #329 sigue abierto por cargos custom,
rehidratación runtime SAV, historiales/GUI, escalas/sonidos y goldens.

Actualización #329-INDUSTRY-SAV-047 (2026-09-03, commit `eaa3473d`):
al aplicar el catálogo NewGRF tras un SAV, las filas `INDY` vuelven a enlazarse
por `IndustryType`/overrides sin ejecutar callbacks de fundación. Las listas
serializadas `accepted`/`produced` son la fuente de verdad, mantienen huecos
`INVALID_CARGO`, y reconstruyen cargos, tasas, multiplicadores por índice
estático, stocks y fechas de espera. La regresión cubre una industria custom
con hueco en la primera salida. Si falta el GRF o el cargo custom se conserva
el fallback y la fila opaca; #329 sigue abierto por esa ausencia,
`DeliverGoodsToIndustry`, historiales/GUI, escalas/sonidos y goldens.

Actualización #329-INDUSTRY-DELIVERY-048 (2026-09-03, commit `12e6c751`):
la descarga final ya materializa `DeliverGoodsToIndustry` antes de contabilizar
la entrega: ordena las industrias cubiertas por la distancia `DistanceMax` de
su tesela más cercana, excluye la industria de origen, recorre varios destinos
hasta agotar la carga o el límite `uint16` de `accepted[].waiting`, consulta
`CBID_INDUSTRY_REFUSE_CARGO` y registra `last_accepted`/`was_cargo_delivered`.
La cola de destinos se consume después de `load_vehicles`, reproduciendo el
orden de `LoadUnloadStation`; sin CB1 se aplica la matriz vanilla a las colas,
con CB1 se ejecuta el callback de llegada y con CB2 exclusivo se difiere al
ciclo de 256 ticks. `CargoReceived` y sus registros PSA se disparan por
huella y la regresión cubre exclusión, fecha, diferimiento y producción. El
monitor `AddCargoDelivery`, `exclusive_supplier`/neutral stations, cargos
custom, historiales por salida y la aceptación exacta de estaciones siguen
pendientes; `#329` permanece abierto.

Actualización #329-INDUSTRY-HISTORY-049 (2026-09-03, commit `a4dba228`):
`INDY.accepted[].history` ya forma parte del estado runtime. Cada entrega
incrementa el registro del mes actual y conserva `last_accepted`; el barrido
diario suma `accepted[].waiting` y el cierre mensual calcula el promedio,
rota hasta los 61 registros nativos y actualiza `valid_history`. El importador
hidrata historial, acumulador y máscara desde SAV, y el writer usa esos valores
cuando la industria fue mutada, manteniendo el passthrough para filas opacas.
La regresión cubre entrega/rollover, hidratación y emisión del chunk `INDY`.
Los historiales de producción por salida, cargos custom y `AddCargoDelivery`
siguen pendientes; `#329` no se cierra.

Corrección del corte canónico: cualquier fila histórica que todavía describa
`accepted[].history`, `accepted[].accumulated_waiting`, `produced[].history` o
`valid_history` como simple passthrough queda superada por `26a915db`. Esos
campos se hidratan, actualizan y reemiten para cargos representables; sólo los
cargos custom y mutaciones fuera de esos caminos siguen parciales.

Actualización #329-INDUSTRY-PRODUCED-HISTORY-050 (2026-09-03, commit
`26a915db`): `INDY.produced[].history` deja de ser passthrough para cargos
representables. Las transferencias por estación registran por salida la tanda
producida y las unidades transportadas; la carga directa registra ambos
contadores, el rollover comparte la ventana nativa de 61 posiciones y el
writer prefiere el estado runtime sobre la fila guardada. Importación,
rehidratación NewGRF y regresiones de transferencia/rollover/chunk cubren el
camino; cargos custom y mutaciones económicas fuera de esas rutas permanecen
parciales y `#329` continúa abierto.

Actualización #329-INDUSTRY-CARGO-MONITOR-051 (2026-09-03, commit
`036fda1f`): el runtime implementa `_cargo_pickups`/`_cargo_deliveries` con el
layout nativo de `CargoMonitorID` (entidad, tipo de cargo y compañía),
contadores saturantes de 32 bits y lecturas que reinician o mantienen la
activación según `keep_monitoring`. `DeliverGoodsToIndustry` registra cada
porción aceptada para la industria y el pueblo de la estación; el remanente
de aceptación se registra por separado y la recogida se acredita sólo al
confirmar la entrega final. `GameState` expone consultas y limpieza equivalentes
para el core. No se persiste el mapa efímero; siguen pendientes los bindings
de GameScript, exclusividad/neutral stations y cargos custom.

## Orden recomendado

| Orden | Bloque | Estado | Criterio de cierre |
|---:|---|---|---|
| 1 | Zoom y viewport | Completado | Seis niveles OpenTTD (`0,25×`…`0,125×`), culling/overview deterministas y smoke de render; la paridad raster global queda separada de la cobertura de zoom. |
| 2 | RMAP-004: generador procedural | Abierto P1 (RMAP-005–017, RMAP-019–023, RMAP-025–026, RMAP-028–029, RMAP-031, RMAP-033, RMAP-035–055, RMAP-057–058, RMAP-060, RMAP-063–064, RMAP-066–081 y RMAP-083–139 cerrados; RMAP-018/RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034/RMAP-056/RMAP-059/RMAP-061/RMAP-062/RMAP-065/RMAP-082 en curso) | Reducir la primera divergencia de TGP/RNG/`FixSlopes`/clear/towns/industries/objects con matriz 64²→512². RMAP-139 añade settings explícitos de ríos/bordes al comparador y deja exactas las combinaciones auditadas; esto no cierra el generador. RMAP-138 amplía el control a cuatro seeds temperate 1024² (`1330935388`–`1330935391`) y deja exactas las seis fronteras (24/24 comparaciones, 0 teselas y 0 bloques 4×4 por frontera); esto no cierra el generador. RMAP-087 completa el stream RNG de árboles de humedal tras `CreateRivers` y deja `landscape`/`clear` exactos en la cohorte temperate 512²; RMAP-088 unifica el perfil y la cola de `RunTileLoop` de Nueva partida entre cliente y oracle. RMAP-084/086 cubren las rampas de llegada e inicio inclinadas de puentes municipales, RMAP-085 reproduce el coste/clear atómico de su terraformación y RMAP-089 completa los túneles municipales y la terraformación de sus bocas; las cuatro seeds 512² de control quedan exactas hasta `towns`. RMAP-090 completa la representación de `landscape`/`clear` para Arctic, Tropic y Toyland en las cuatro seeds 64² de control, incluyendo la nieve canónica en `MAP3` y zonas tropicales en `MAPT`; RMAP-091 conserva el nibble de `TropicZone` en las calles municipales y extiende la frontera `towns` exacta a las cuatro seeds Tropic de 64²; RMAP-092 porta los gates climáticos de `CheckNewIndustry_*`, RMAP-093 completa las tablas de layout vanilla, RMAP-094 propaga la línea de nieve efectiva y la admisión de campos árticos, RMAP-095 alinea la admisión `OnlyInTown` y el reset de MAP8 de `MakeIndustry`, RMAP-096 permite costas durante la plataforma gratuita y RMAP-097 usa la línea de nieve efectiva al seleccionar casas árticas; RMAP-098 pasa el límite de altura y la línea de nieve efectivos a `GenerateTrees`, dejando las cuatro semillas Arctic 64² exactas en las seis fronteras (`landscape`→`trees`); RMAP-099/100 conservan `TropicZone` y respetan `ClearTile_Road` al materializar objetos/industrias, y RMAP-101 completa layouts Toyland y `OnlyNearTown`, dejando las cuatro semillas tropicales y cuatro Toyland 64² exactas en las seis fronteras. RMAP-102 escala el borde de refinerías por eje, RMAP-103 replica el `Execute` parcial de plataformas y RMAP-104 difiere pendientes al pase de plataforma y limpia el `gfx` alto de `MakeIndustry`; RMAP-105 verifica las seis fases completas y deja las cuatro seeds temperate 512² (`1330935378`–`1330935381`) exactas en 24/24 fronteras. RMAP-113/RMAP-114/RMAP-115/RMAP-116 cierran la primera transición de entrega del mundo: la cola `RunTileLoop`, animación inicial, árboles, casas, costas e industrias; RMAP-117 corrige la orientación de las bocas de puente/túnel en la limpieza vial municipal y deja Toyland 256² exacto en las cuatro seeds. RMAP-118 unifica el consumo del RNG global de `TileLoop_Trees` en Toyland y RMAP-119 admite las bocas de puente/túnel existentes durante `IsRoadAllowedHere`; RMAP-120 usa el `GetTileZ` mínimo para el gate de Bubble Generator en pendientes y RMAP-121 replica el despeje completo de casas multitile que `ToyShop` reemplaza mediante `GetHouseNorthPart`/`ClearTownHouse`; RMAP-123 rechaza `MP_VOID` durante `RiverMakeWider` y conserva el `RoughSnow` de 16 bits durante `TileLoopTreesAlps`; RMAP-124 hace que el preflight de puentes municipales aplique `CheckBridgeSlope` y rechace cabezas a distinto nivel efectivo; RMAP-127 replica el despeje `Auto` de la salida municipal de un túnel y rechaza bocas multibit; RMAP-128 separa los topes de puente y túnel, rechaza costas/puentes paralelos y deja exacta la frontera urbana ártica de 1024²; RMAP-129 conserva las entidades de `IndustryPool` cuando el origen de un layout cae dentro de otra huella sin superposición y deja exactas las seis fronteras de la cohorte ártica 1024²; RMAP-130 conserva la asociación de pueblo de las industrias fundadas sobre casas y deja exacta esa cohorte ártica 1024²/seed `1330935381` en las seis fases; RMAP-132 detiene el caminador municipal al entrar en una carretera de otro pueblo y deja exacta la cohorte ártica 1024²/seed `1330935383` en las seis fases; RMAP-134 hace que el preflight de puentes paralelos recorra la espiral nativa y deja exacta la cohorte tropical 1024²/seed `1330935386` en las seis fases; RMAP-135 conserva el `Chance16` de `LevelTownLand` al visitar una tesela ocupada y deja exacta towns en temperate 1024²/seed `1330935387`; RMAP-136 pasa el límite efectivo de altura a las plataformas industriales y RMAP-137 resuelve su valor dinámico para árboles y deja exactas las fronteras `industries`/`trees` de temperate 2048²/seed `1330935404`. Las seeds Toyland 512² `1330935378`–`1330935381` quedan exactas en las seis fronteras (`landscape`→`trees`), con 0 teselas y 0 bloques 4×4 distintos en cada fase auditada; las cuatro seeds árticas 512² `1330935378`–`1330935381` también quedan exactas en las seis fronteras tras RMAP-124. La matriz completa 64²→512² queda exacta en 15/15 cargas y 15/15 mapas mismo-seed para la cohorte canónica; el alcance de clima/configuración, otras semillas/tamaños y ticks posteriores sigue abierto. RMAP-082 conserva la generalización urbana fuera de la cohorte de control. La evidencia detallada y el resto de avances se mantienen únicamente en `random-map-issues.md`, para no duplicar métricas. RMAP-004 sigue abierto mientras haya divergencias en otros tamaños/fases; RMAP-018 conserva configuraciones de río y fases posteriores multiclima, y RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034 la generalización de pueblos. |
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden estable. Paradas, waypoints viales, estaciones rail, objetos e industrias resuelven layouts `TileSeq` completos por Action3/2→Action1, materializan suelo, parents/children y pendientes; AirportTile conserva también el `TileLayoutSpriteGroup` estático, reemplaza el ground fallback y convierte BUILD en parents/children, dejando sólo el ground como child de una fundación CB150. Los aeropuertos construidos conservan el `gfx` por tesela y la vista Action1/3 sigue como fallback atómico. El procesador aplica `DODRAW`, offsets de sprite/caja/child, `var10`, draw mode `0x100` e invalida la caché con registros `7D`/`0x100`. Sprites base y paletas custom siguen fallback atómico. Los callbacks `0x150` de casas y teselas de industria ya se evalúan sólo en pendientes y pueden suprimir `FOUNDATION_LEVELED`; `RTSG_DEPOT` (selector 8) ya reemplaza las seis fachadas relocatables de depósitos ferroviarios con Action2, offsets NFO y children de fundación; quedan por cubrir las variantes ferroviarias de pendiente/túnel, el compositor de foundations/rotaciones de aeropuertos y el sprite-stack/callbacks avanzados de vehículos. La animación AirportTile ya ejecuta metadatos Action0 y callbacks `0x152`/`0x153`/`0x154` con lista persistida, además de `NewCargo`, `CargoTaken`, `AcceptanceTick` y `AirplaneTouchdown` desde los eventos de simulación, traduciendo el cargo por la CTT propia del GRF. Las listas de badges de `AirportTiles`/`Airports` se traducen por GlobalVar `0x18` y `AirportTile` expone `0x7A` con resultado `UINT_MAX` para índices fuera de tabla. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD. `STNN` conserva ahora `airport.type`, `airport.layout` y `airport.rotation` custom, además de la huella `airport.tile/w/h` materializada; el cargador reatacha sus `AirportTile` cuando el layout activo coincide exactamente. `NGRF` y las filas base de `OBJS` ya tienen modelo semántico; un `OBJS` importado se conserva byte a byte hasta que una construcción/demolición lo invalida. `OBID` fusiona ahora los tres campos conocidos sobre la cabecera/filas originales cuando cambia el mapping, manteniendo columnas futuras y huecos densos; si cambia el conjunto de IDs se usa el writer canónico de forma segura. Todas las tablas `CH_TABLE`/`CH_SPARSE_TABLE` que reconstruye el writer reciben el snapshot semántico al fusionar sobre el cuerpo original campos con schema y tamaño codificado idénticos —incluidos strings, listas y structs anidados—, preservando columnas futuras y huecos mientras no cambien filas ni índices. [#371](sav-rename-371.md) permite además otra longitud para strings raíz, reconstruyendo sólo la fila y su longitud gamma. Las regresiones legacy directas de `PLYR` y `CITY` prueban que un campo compatible puede cambiar sin añadir campos modernos intactos; si cambia uno ausente o incompatible cae al writer canónico. `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` ya se decodifican, hidratan sus referencias y se reemiten con índices densos y 256 registros; los registros no nulos de pueblo se exponen por GRFID a los scopes parent de casas y objetos y se conservan también cuando no tienen consumidor. `PLYR.allow_list[].key` ya conserva sus strings como struct-list, pero no activa aún autenticación o permisos de red. CB17 de casas y CB157 de objetos pueden crear/modificar la fila PSA de su pueblo y el writer la referencia desde `CITY.psa_list`; quedan cambios de longitud de listas/structs o de topología, writeback de callbacks de teselas y pools nativos de casas/objetos todavía no modelados. |
| 5 | NewGRF runtime (#329) | Abierto | Vehículos, estaciones, objetos e industrias tienen rutas runtime parciales. `AirportTile` ejecuta `CB152`/`CB153`/`CB154`, eventos de carga y `AirplaneTouchdown` con CTT activa; la regresión #447 cubre además el contrato booleano de `CB150` y la relación parent/child de una fundación inclinada. La FTA propia de layouts NewGRF continúa bloqueada. CB36 ya participa en acortamiento, velocidad, capacidad, potencia, peso, esfuerzo tractor y costes en los call sites con catálogo activo. Los historiales `INDY` representables se hidratan, actualizan en entrega/transferencia/barrido/rollover y se reemiten. Persisten foundations/rotaciones/sonidos de aeropuertos, APIs legacy sin catálogo, propiedades/scopes Action0 restantes, cargos custom, callbacks de teselas y estructura residual `OBJS`/`OBID`; la matriz de callbacks concentra el detalle y la evidencia. |
| 6 | Movimiento y economía diferencial (#330) | Abierto | Oráculos externos para carretera (tráfico/colisiones/dirección), rail (PBS/YAPF/presignals/consist) y aire/mar, incluyendo casos límite. El perfilador de `Kale_TitleGame.sav` ya no aborta cuando un callback devuelve un pago negativo: los contadores `u64` de estación/empresa/estadística saturan ese ajuste a cero y el crédito firmado conserva la penalización; quedan pendientes los oráculos diferenciales y sus casos límite. |
| 7 | Idiomas y settings (#331) | Abierto | Catálogo de idiomas, locale, settings y textos guardados se cargan y se comparan con OpenTTD sin colisiones ECS ni regresiones de UI. Los cortes #448–#451 localizan las frases estáticas de recesión, headlines de desastres y primer vehículo, y el body fijo de cierre de industria; #452 añade plantillas estrictas para entrega de carga y body de primer vehículo, #453 añade oferta/adjudicación de subsidios con coordenadas signed, #454 añade logro rival/quiebra, #455 añade el headline de autoreemplazo fallido, #456 añade ambos headlines de cierre de industria, #457 añade los seis bodies de desastre, #458 añade los cinco avisos dinámicos de vehículo, #459 añade headline/body de choque de trenes, #460 añade el body de accidente aéreo, #461 añade headline/body de choque en paso a nivel, #462 añade el body de vehículo inundado, #463 añade headline/body de compra de compañía y #464 añade headlines de accidente/inundación para nombres seguros, preservando unidades, importes, cargos, empresas, objetivos, meses, límites, víctimas, IDs, índices, precios y coordenadas. Coordenadas, contadores, IDs, índices, víctimas, precios, nombres inseguros o fragmentos malformados, GameScript y familias restantes quedan explícitamente sin traducir; sigue pendiente la migración general de NewsItem y el resto de catálogos `.lng`. |

Actualización #371–#384 (2026-09-05): la fila de interoperabilidad SAV de este
orden permite reencuadrar strings, listas escalares y struct-lists de raíz con
descriptor recursivamente idéntico sin perder columnas importadas. Las pruebas
nativas son `CITY.psa_list` + `PSAC` y `CITY.supplied`; esta última usa SLV 358
y OpenTTD dedicado re-guarda los 61 registros de historial. `INDY` normaliza
ahora sus historiales representables al mismo tamaño (con cero para la entrada
aceptada todavía nula), pero su agregación trimestral/anual runtime sigue
pendiente. No se infiere desde el header si un `HAS_LENGTH` es vector o array
fijo, por lo que los writers mantienen sus tamaños nativos.
Subschemas incompatibles y cambios de filas, índices o topología quedan
pendientes en #328. Ver [evidencia #374](sav-indy-history-374.md).
`PLYR` conserva además `money_fraction`/`block_preview` y los años de
inauguración económico/wallclock; el runtime de esos campos y los demás datos
de ciclo de vida siguen fuera del corte. Ver [#375](sav-company-preview-375.md)
y [#376](sav-company-inauguration-376.md). También conserva los `TileIndex`
de HQ y última construcción como metadata, no como implementación de HQ; ver
[#377](sav-company-location-377.md). El bloque pasivo de bancarrota también
se reemite sin disparar takeover; ver [#378](sav-company-bankruptcy-378.md).
Los cupos saturados de paisajismo se preservan sin afirmar aún su recarga
runtime; ver [#379](sav-company-landscaping-379.md).
La matriz fija de gastos anuales de compañía también se conserva como
historial, sin cálculo/rotación runtime; ver
[#380](sav-company-yearly-expenses-380.md). `PLYR.allow_list[].key` conserva
ahora sus claves públicas en el struct-list moderno; no se infiere de ello
autorización de red runtime. Ver [#381](sav-company-allow-list-381.md).
PATS.order.selectgoods conserva además el bool que el core usa para decidir si
una estación sin visita previa puede recibir carga; la mutación del setting
también invalida el passthrough de PATS. Ver [#382](sav-order-selectgoods-382.md).
PATS.linkgraph conserva también el intervalo y presupuesto en segundos, los
cuatro modos por clase y los knobs del pipeline ya portado; el selector respeta
las clases NewGRF y el scheduler usa la división nativa de dos segundos por
día. El job conserva ahora además su snapshot y espera la fecha de join nativa
derivada de `recalc_time`; una marca de join temprana deja intactos los flows
anteriores. Threads/pausa, compresión y validación de topología siguen fuera
del corte. Ver [#383](sav-linkgraph-settings-383.md) y
[#394](sav-linkgraph-recalc-time-394.md). El bool
`station.distant_join_stations` también se conserva y gobierna el comando
propio de unión remota, con default `true` y smoke de OpenTTD 15.3; no cubre
todos los comandos de estación. Ver [#384](sav-distant-join-stations-384.md).
#328 permanece abierto por los demás pools, schemas y runtime SAV.

Actualización #385 (2026-09-06): `PATS.vehicle.wagon_speed_limits` se
hidrata en `ConstructionSettings`, se reemite como `SLE_BOOL` y gobierna la
velocidad máxima de los consistes en todos los call sites con `GameState`.
OpenTTD sólo aplica el mínimo de una unidad wagon cuando el setting está
activo; el test del core cubre ambas ramas y el smoke dedicado verifica que
OpenTTD re-guarda el valor `false`. `UsesWagonOverride` sigue fuera de este
issue; #328 continúa abierto por ese runtime NewGRF y por las demás brechas.

Actualización #386 (2026-09-06): `PATS.vehicle.disable_elrails` se hidrata en
`ConstructionSettings`, se reemite como `SLE_BOOL` y gobierna la compra de
locomotoras eléctricas sobre rail normal y la conversión Electric → Rail como
no-op. Parser, mutación, comandos y smoke están cubiertos; la ocultación del
overlay de catenaria en el renderer global sigue siendo residual de #326/#329.
Ver [evidencia #386](sav-disable-elrails-386.md).

Actualización #387 (2026-09-06): publicado `3a4065db`. `PATS.vehicle.plane_crashes`
se hidrata en `ConstructionSettings`, se reemite como `SLE_UINT8` y aplica
`0/1/2` al camino FTA existente con el cálculo nativo; el caso fijo de jet en
pista corta permanece separado. Parser, clamp, mutación, umbrales
deterministas y smoke OpenTTD 15.3 están cubiertos. Las rutas de accidente
fuera de FTA y la UI nativa siguen pendientes; ver
[`sav-plane-crashes-387.md`](sav-plane-crashes-387.md).

Actualización #388 (2026-09-06, commit `13bcbc10`): `PATS.vehicle.plane_speed`
se hidrata en `ConstructionSettings`, se reemite como `SLE_UINT8` con rango
`1..=4` y default `4`, y el divisor llega al movimiento lineal y FTA desde
`GameState`. Las APIs históricas conservan el default nativo. El smoke
OpenTTD 15.3 re-guardó la candidata de 63044 bytes en 8480 bytes y confirmó
`plane_speed = 2`; la evidencia incluye hashes y todos los gates. La
aceleración completa y callbacks de velocidad siguen siendo residuales
deliberados de #328. Ver [`sav-plane-speed-388.md`](sav-plane-speed-388.md).

Actualización #329-INDUSTRY-CB28-021 (2026-09-02): CB28 mantiene la semántica
exacta de OpenTTD (sin invertir el bit 10), y el call site de construcción
expone `IACT_USERCREATION` (`param2=2`) y las variables de ubicación
`0x7A`/`0x80`/`0x81`/`0x82`/`0x86`–`0x8B`/`0x8D`/`0x8F` (badges, TileIndex,
pueblo, layout, terreno, zona, distancias, altura y random). Continúan
pendientes flags/fundador/fechas completos, otros tipos de creación y strings
de error; #329 permanece abierto.

Actualización #329-INDUSTRY-CB28-022 (2026-09-02): la construcción NewGRF
acepta el layout sorteado/seleccionado en una variante atómica del comando,
conserva el ordinal uno-based `Industry.selected_layout` y los bits `Industry.random`, y los
reexpone en el scope padre de Action2. `INDY` ahora importa/exporta ambos
campos; las variables WORD de stock, historial y contador ya no pierden sus
bits altos. CB28, fundador/fechas/flags/PSA y mensajes de error siguen siendo
parciales, por lo que #329 y el bloque SAV no se cierran.

Última etapa RMAP-064/069/070/071/072/073/074/075/076/079/080/083: la plantación de Farm replica ahora el orden de RNG,
la geometría y las cercas de OpenTTD, el primer `IndustryID(0)` permanece
vinculado a toda su huella durante la obra y la cohorte de carbón escribe los
bytes nativos de `MakeIndustry`. Así OilWells e IronOreMine vuelven a los sitios
nativos en las dos seeds 64²; RMAP-070 aplica la tabla `appear_creation` y el
sorteo ponderado terrestre; RMAP-071 hace transaccional la admisión de agua,
puentes, vehículos y plataforma; RMAP-072 separa el reparto tierra/agua y deja la
pasada acuática vacía de forma explícita hasta modelar `IT_OIL_RIG`. RMAP-073 corrige
el BFS dinámico de `FlowRiver`, RMAP-074 reproduce el aplanado de lagos pequeños,
RMAP-076 ejecuta YAPF/terminus/ensanchamiento y limpia agua compartida al terraformar,
y RMAP-075 reinicializa MAP4 en cada `MakeIndustry`; RMAP-079 añade el puente
municipal plano sobre río/canal y RMAP-080 propaga la terraformación municipal
entre vértices y conserva `MAP6` de tipo fuera del vano; RMAP-083 conserva el
rough ya preparado al plantar árboles de wetlands y RMAP-081 impide ensanchar
ríos sobre el borde libre `MP_VOID`. La cohorte temperate 64²
(`1330935378`–`1330935381`) queda exacta en paisaje, clear, towns e industrias;
RMAP-094/RMAP-095/RMAP-096/RMAP-097 extienden el mismo corte a las cuatro
semillas árticas `1330935378`–`1330935381`, incluyendo línea de nieve efectiva,
casas por zona, bancos `OnlyInTown`, bytes de `MakeIndustry` y costas de
plataforma; RMAP-098 lleva esos ajustes al refuerzo de árboles del pipeline
completo y deja las seis fronteras (`landscape`→`trees`) exactas en esa cohorte.
Esto no cierra las demás semillas, tamaños, configuraciones ni climas.
RMAP-056 conserva
la admisión acuática. No se declara paridad de industrias ni se
cierran RMAP-004/RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034.

> Actualización SAV (2026-09-02): `OBJS` ya se modela en filas base y sólo se
> reconstruye después de una mutación. `OBID` fusiona los tres campos conocidos
> sobre la cabecera y filas originales cuando el conjunto de IDs no cambia.
> Además, el writer común de tablas fusiona campos escalares de longitud fija
> modificados en `STNN`, `CITY`, `INDY`, `ORDL`, `VEHS`, `CAPA`, `PATS`, `ECMY`,
> `CAPY`, `GRPS`, `ERNW`, `NGRF`, `DATE` y `PLYR`, conservando columnas futuras
> y huecos. Las mutaciones de strings, listas, structs/campos anidados o de la
> forma de la tabla usan el encoder canónico; los pools no modelados siguen
> pendientes.

> El rango compacto de sub-issues de la fila RMAP-004 se mantiene como
> referencia histórica; RMAP-122/RMAP-123/RMAP-124 están cerrados y documentados en la matriz
> detallada. El padre sigue abierto y cualquier nuevo corte debe agregar otro
> sub-issue, no convertir este control en paridad general.

## Cómo se decide el siguiente bloque

Actualización RMAP-123 (2026-08-31): la primera divergencia ártica de 512²
quedó resuelta en `TileLoopTreesAlps` y en el conteo de faros de bordes con
ríos: `RiverMakeWider` ahora respeta el marco `MP_VOID` y el crecimiento de
árboles conserva el `MAP2` completo cuando `RoughSnow` ocupa el byte alto. La
semilla `1330935382` es exacta en `landscape`, `clear`, `towns`, `industries`,
`objects` y `trees` (0 teselas y 0 bloques 4×4). Es un control acotado; no
amplía el cierre de RMAP-004 ni de sus padres a otras semillas, tamaños,
configuraciones o ticks.

Actualización RMAP-124 (2026-08-31): la divergencia ártica de 512²/seed
`1330935380` en `towns` provenía de un puente municipal que Rust aceptaba con
las dos cabezas a distinto nivel. El preflight ahora canoniza los extremos y
aplica las pendientes/alturas efectivas de `CheckBridgeSlope`; si el comando
nativo rechaza el puente, la caminata continúa por carretera sin consumir sus
selecciones de tipo. Las cuatro seeds árticas 512² (`1330935378`–`1330935381`)
quedan exactas en las seis fronteras, comparando teselas y bloques 4×4. El
resultado sigue acotado a la cohorte; RMAP-004 y sus padres mantienen abiertos
otros mapas, configuraciones y ticks.

Actualización RMAP-125 (2026-08-31): la divergencia ártica 1024²/seed
`1330935378` quedó aislada en el identificador de industria. El candidato
truncaba `IndustryID` a `u8` y hacía colisionar el pool desde 256; OpenTTD
conserva el valor completo en `MAP2` bajo/alto. La corrección propaga `u16`
por entidades, campos, vínculos, SAV, comandos y renderer. La comparación por
tesela, bytes y bloques 4×4 queda exacta en `landscape`, `clear`, `towns`,
`industries`, `objects` y `trees` (**0/0** en las seis fronteras). Se cierra
este sub-issue con alcance acotado; RMAP-004/RMAP-056 siguen abiertos para
otras semillas, tamaños, climas, configuraciones y runtime NewGRF.

Actualización RMAP-126 (2026-08-31): se conserva el bit `MAP3` de nieve que
`MakeSnow` deja al limpiar árboles `ROUGH_SNOW` en el tile loop ártico. La
frontera `landscape` 1024²/seed `1330935379` queda exacta por tesela, bytes y
bloques 4×4 (0/0/0). El siguiente corte reproducible es la divergencia de
`towns` de esa misma matriz; el padre RMAP-004 sigue abierto.

Actualización RMAP-127 (2026-08-31): la primera divergencia ártica de
`towns` en 1024²/seed `1330935379` era un túnel municipal que Rust aceptaba
cuando la boca de salida tenía dos bits de carretera. `CmdBuildTunnel` usa
`Auto` y rechaza esa limpieza implícita; el preflight ahora exige un único bit
en la salida, conserva la entrada seleccionada por el walker y deja continuar
la carretera normal. La matriz raw baja de 7.833 teselas/41.011 bytes/1.181
bloques 4×4 a **0/0/0** y coincide en las 1.048.576 teselas; el cierre queda
acotado a esta regla y no generaliza el padre RMAP-004.

Actualización RMAP-128 (2026-09-01): la siguiente divergencia ártica de
`towns` en 1024²/seed `1330935380` estaba en el crecimiento desde puentes y
túneles. El modelo separa ahora el límite de puentes inclinados
(`población/1000 + 5`) del límite de túneles bajo montaña
(`población/1000 + 7`), trata las costas como `MP_WATER` no plano igual que
`IsWaterTile`, rechaza rampas viales paralelas sin consumir RNG y salta las
bocas al extremo opuesto sin sortear otra dirección. La comparación por
tesela y bloques 4×4 queda exacta (**0/0**) en `landscape`, `clear` y `towns`;
el alcance sigue acotado a esta semilla y no cierra RMAP-004 ni la
generalización urbana.

Actualización RMAP-129 (2026-09-01): la primera divergencia ártica de
`industries` en 1024²/seed `1330935380` no era una selección ni una huella
distinta, sino una reutilización de `IndustryID`. La ruta Rust eliminaba una
entidad por el origen de la nueva industria aunque el layout real empezara
con un offset y no tocara ninguna tesela anterior; OpenTTD conserva ambos
elementos del pool. Se eliminó ese `retain` por origen y se agregó una
regresión de dos layouts no superpuestos. La comparación por tesela, bytes y
bloques 4×4 queda exacta en `industries`, `objects` y `trees` (0/0/0), además
de las fronteras previas; RMAP-004/RMAP-056 siguen abiertos para otras
semillas, tamaños, climas y runtime NewGRF.

Actualización RMAP-130 (2026-09-01): la primera divergencia ártica de
`industries` en 1024²/seed `1330935381` era una asociación de pueblo perdida,
no el sorteo de campos. El candidato elegía como pueblo más cercano el `177`
para la banca de `(142,301)`, aunque la tesela `MP_HOUSE` llevaba `MAP2=20`;
OpenTTD usa `Town::GetByTile` y rechazaba el banco porque ya existía uno de ese
tipo en el pueblo `20`. `Industry::town_id` conserva ahora esa relación antes
del despeje y la comprobación de una especie por pueblo reutiliza el valor
persistido (con fallback Manhattan para saves antiguos). La regresión cubre la
distancia engañosa y el rechazo del duplicado. El oráculo se recompiló sin
instrumentación y la comparación raw por tesela, bytes y bloques 4×4 queda
exacta en `landscape`, `clear`, `towns`, `industries`, `objects` y `trees`:
**0/0/0** en las seis fases. El alcance es esta semántica `MP_HOUSE` y la
cohorte; SAV completo, otras semillas/tamaños/climas, `IT_OIL_RIG` y callbacks
runtime NewGRF siguen abiertos.

Actualización de #329: el renderer de vehículos ya resuelve los canales
primario y secundario de las 23 libreas por esquema (incluidas clase de
tracción, DMU/EMU, carga, aviones, barcos y tranvías), la prioridad de librea
explícita del grupo y sus padres, 2CC vanilla/Action5 y crash. Sigue abierta la
invalidación visual completa en consumidores fuera de la caché de vehículos;
esta cobertura no cierra el issue de runtime.

Nota de implementación: la consulta de variable `61` para `0x60` conserva el
parámetro `ExtendedByte` completo (WORD, hasta 14 bits); no se trunca al byte
bajo al resolver IDs locales de vehículos.

El subtramo de badges de vehículos y vías traduce GlobalVar `0x18`, conserva las
listas `ReadBadgeList` por `EngineDef`/`RailType`/`RoadType` y expone
`0x64`/`0x65`/`0x7A`, incluidos offsets relativos; quedan callbacks y variables
secundarias fuera de ese contrato.

1. Ejecutar la matriz o fixture del bloque actual.
2. Localizar la primera divergencia por tile, entidad o tick.
3. Portar la regla de OpenTTD mínima necesaria y añadir el test de regresión.
4. Repetir la matriz completa del bloque y los tests de zoom (0,12×, 0,25×,
   0,50× y 1× como mínimo; la matriz completa usa los seis niveles).
5. Actualizar `docs/PARIDAD.md` y el issue correspondiente.
6. Formatear, lintar, probar, commitear y publicar antes de continuar.

## Estado medible al 2026-09-02

- Carga `.sav`: matriz aleatoria 15/15 exacta, 0 tiles y 0 bloques 4×4
  distintos.
- Generador procedural mismo seed: 15/15 exactos en la matriz canónica 64²→512²,
  incluyendo la transición del primer `StateGameLoop` (animación, `RunTileLoop`
  y `OnTick_Trees`); RMAP-004 sigue abierto para otras configuraciones, climas,
  semillas, tamaños y ticks posteriores.
- RMAP-138 amplía la auditoría a cuatro seeds temperate 1024²
  (`1330935388`–`1330935391`): las seis fronteras por seed quedan exactas
  (24/24 comparaciones, 0 teselas, 0 campos y 0 bloques 4×4). La evidencia
  confirma la cohorte, pero no cierra RMAP-004 ni sus padres para tamaños,
  climas, configuraciones de río o ticks no cubiertos.
- RMAP-139 añade al comparador los ajustes explícitos de ríos y bordes de agua.
  Las combinaciones auditadas (longitudes mínima 2/4, ruta aleatoria 1,
  `amount_of_rivers=0` y `water_borders=0`) quedan exactas en las seis fases;
  la matriz combinatoria completa y otros climas/tamaños siguen abiertos en
  RMAP-018/RMAP-004.
- Sub-issue RMAP-117: la limpieza de carreteras municipales distingue la
  dirección exterior de bocas de puente/túnel; Toyland 256² queda exacto en
  cuatro semillas. RMAP-118 corrige el consumo del RNG global de árboles
  durante `CreateRivers` en Toyland y RMAP-119 admite las bocas de puente/túnel
  ya existentes durante `IsRoadAllowedHere`. Las seeds Toyland 512²
  `1330935378` y `1330935379` quedan exactas en las seis fases
  (`landscape`→`trees`), con 0 teselas y 0 bloques 4×4 distintos por fase;
  esto acota la divergencia anterior sin cerrar la generalización urbana para
  otras semillas, tamaños, climas o configuraciones.
- Zoom: las seis escalas fijas y la transición detalle/overview tienen tests;
  la composición raster completa continúa pendiente y no se confunde con el
  smoke de entidades.
- Economía: el perfilador de `Kale_TitleGame.sav` cubre 3.293 vehículos y
  34.044 paquetes sin abortar por pagos negativos. Los contadores acumulativos
  de ingresos convierten `Money` firmado con saturación (una penalización no se
  registra como ingreso), mientras `credit_company` y el beneficio del vehículo
  mantienen el valor firmado. Esto elimina un crash reproducible, pero no cierra
  #330: aún faltan las comparaciones tick a tick de movimiento, feeder y
  callbacks económicos.
- Composición #326: el bloque publicado de puentes enlaza cabezas de rampa,
  barandillas de vano y pilares al sorter global cuando hay sprite; el vínculo
  usa la misma caja de mundo que `world-draw`. Los overlays de carretera y
  tranvía, incluidos los reemplazos NewGRF, también se cuelgan del parent de
  fundación cuando existe. El overlay vanilla de tranvía del vano usa la rampa
  sur como `head_tile`, aplica los seis offsets de `DrawBridgeRoadBits` y queda
  como child del parent trasero; así no depende de los bits vacíos de la tesela
  de agua intermedia. Los grupos NewGRF `ROTSG_BRIDGE`/`ROTSG_OVERLAY` del
  roadtype se resuelven contra esa misma rampa, aplican los offsets específicos,
  reemplazan el deck Action5 cuando entregan superficie y se adjuntan al parent
  trasero combinado. La mitad frontal de `DrawBridgeRoadBits` ya tiene parent
  propio en los vanos y recibe `ROTSG_CATENARY_FRONT`; la trasera se vincula al
  parent posterior junto con `ROTSG_CATENARY_BACK`. Cuando no hay grupos
  específicos, el fallback vanilla de esos sprites (incluidos los assets
  `SPR_TRAMWAY_BASE`) ya se materializa para rampas y vanos con sus seis cajas
  de puente; los layouts/children NewGRF de estación, industria, objeto y
  casa ya tienen consumidores parciales; siguen pendientes sus scopes avanzados;
  cables y postes de catenaria ferroviaria ya participan como parents
  `sortable` con esa misma caja, y los overlays Action5/PBS/tranvía como
  children del parent trasero combinado.
- Vehículos: cada cuerpo y unidad de consist recibe la caja `Vehicle::bounds`
  equivalente (tren diagonal según `unit_length`, orientación de barco y fase
  de aeronave), clave estable de `ViewportAddVehicles` y profundidad fuente;
  sombra/rotor se conservan como children del cuerpo. Las vistas Action1/2
  aplican sus offsets NFO a carretera, barcos y aeronaves además de trenes.
  Los grupos Action2 real distinguen ahora listas loaded/loading según carga y
  capacidad. Cuando Action0 activa el bit de sprite-stack, el renderer crea
  hasta ocho children por unidad, reevalúa la variable `0x10` y conserva offsets
  NFO por capa. Los wagon overrides conservan los IDs extendidos de la cadena
  Action3 anterior, aplican primero el cargo específico y luego el grupo
  default, y sólo cruzan motores del mismo GRFID. El registro `0x100` ya
  termina explícitamente las secuencias SpriteStack (bit 31); quedan
  cubiertas las paletas especiales de vehículos (2CC/crash); siguen pendientes callbacks y los scopes
  parent/relative avanzados (el padre inmediato y los offsets básicos ya se
  resuelven en el contexto de consist).
- Estaciones rail NewGRF: los tiletypes Action1/2/3 se dibujan también en
  pendientes y el overlay queda como child de la fundación nivelada, igual que
  la vía/PBS. Los layouts `TileSeq` completos reemplazan el suelo, emiten
  parents con cajas `M(...)` y children relativos después de la catenaria, y
  comparten la huella de registros con la selección Action2. Sprites base,
  paletas custom y layouts incompletos conservan fallback vanilla atómico.
- Waypoints viales: el suelo vanilla ya respeta `m5` (eje), `m3` (Roadside),
  tranvía y catenaria, y en pendientes usa `FOUNDATION_LEVELED` con sus capas
  como children. Los dos postes del layout vanilla y los layouts `TileSeq` de
  `NewGRF` ya se dibujan con suelo propio, cajas parent y children relativos;
  el procesador runtime aplica `DODRAW`, offsets de sprite/caja/child, `var10`,
  draw mode `0x100` y la caché invalida por registros. Sprites base y paletas
  custom siguen en fallback atómico.
- Objetos NewGRF: el renderer ya reevalúa Action2 por tesela con random
  (`m3`), offset de footprint, pendiente/terreno, animación (`m3hi`), owner,
  fecha, color, vista y zona/distancias (`0x42`, `0x45`/`0x46`) del pueblo
  asociado. La asociación usa `Object::town` del pool `OBJS` y cae al pueblo
  más cercano sólo en partidas legacy. Las variables `0x60`–`0x63` exponen
  id/random/información/frame de vecinos del mismo footprint y `0x64` cuenta
  instancias por tipo con la distancia mínima; los conteos se precalculan una
  vez por pase. Los layouts `TileSeq` completos reemplazan el suelo y emiten
  parents/children con cajas `M(...)`; sprites base, paletas custom y layouts
  incompletos mantienen fallback vanilla. Siguen pendientes callbacks de
  objeto adicionales, conteos por clase/catchment y layouts 16-bit completos.
- Casas NewGRF: `DrawNewHouseTile` ya no cae automáticamente en
  `HOUSE_DRAW_DATA`: el sprite de edificio se resuelve desde Action1/2/3 con
  el contexto persistido de la tesela y la zona `0x42` del pueblo identificado
  por `MAP2` (fallback al más cercano en mapas legacy),
  y se registra como parent sortable, mientras el suelo sigue usando el
  sustituto vanilla. Las variables `0x44`, `0x60`/`0x61` (conteos por
  `HouseID`) y `0x62`/`0x63` (información/frame de teselas vecinas) usan ahora
  el mapa y una instantánea de conteos por pase. Los layouts `TileLayout`
  con suelo propio, callbacks de
  foundation/color/animación y la paleta `random_colour` siguen siendo
  residuales explícitos. El callback `CBID_HOUSE_DRAW_FOUNDATIONS` (`0x150`)
  ya se evalúa en pendientes y permite que el layout custom suprima la
  fundación nivelada vanilla.
- Industria NewGRF: la vista Action2 runtime usa también sus offsets resueltos
  y, cuando la tesela se nivela, el overlay se adjunta al último parent de
  `DrawFoundation`. El callback `CBID_INDTILE_DRAW_FOUNDATIONS` (`0x30`) se
  evalúa en pendientes y puede conservar el relieve original; siguen abiertos
  los callbacks de sonido/slope y los layouts/children múltiples fuera del
  subconjunto cubierto.
- Aeropuertos NewGRF: los layouts `Airports` conservan el `gfx` global de cada
  `AirportTile` junto con el `subst` vanilla de `m5`; al construir, el cliente
  materializa el sprite Action1/3 por tesela mediante la caché de imágenes y
  reevalúa Action2 con posición relativa, frame, layout padre, random y
  vecinos. Si falta el catálogo o la vista cae al `AirportPiece` vanilla.
  El importador SAV conserva tipo, layout, rotación y huella, y reatacha los
  `AirportTile` cuando el layout activo coincide exactamente. El picker expone
  el índice Action0 exacto de cada aeropuerto NewGRF, lo transporta con su id
  global hasta query/execute y suma sus offsets directamente; los huecos del
  rectángulo no se validan ni se limpian. Ver
  [newgrf-airport-layout-rotation-326.md](newgrf-airport-layout-rotation-326.md).
  Action0 conserva
  frames/status/speed/triggers y el scheduler ejecuta parcialmente Built,
  TileLoop, next-frame y speed (`0x152`/`0x153`/`0x154`) con estado persistido;
  los triggers de carga/descarga y `AirplaneTouchdown` alcanzan el scheduler
  (desde una FTA vanilla cuando existe o desde el aterrizaje simple), mientras
  la FTA propia de layouts NewGRF sigue bloqueada; quedan la rotación runtime
  de sprites/children del compositor y sonidos, por lo
  que #329 continúa abierto.
- `reference/` es un checkout local ignorado/no versionado; nunca se agrega al
  commit de una tarea.

### Avance SAV — 2026-08-26

El importador conserva cualquier *fourcc* que el escritor no reconstruye
(`VIEW`, `DEPT`, `SUBS`, `ROAD`, `AIPL`, `GSTR`, `GSDT` y futuros equivalentes),
incluyendo su tipo de contenedor y cuerpo exacto. Así un round-trip no descarta
features nuevas sólo porque todavía no tengan un modelo Rust. Desde 2026-09-02,
las tablas que sí reconstruimos también preservan sus columnas futuras cuando
se modifica sólo un campo escalar fijo y se mantienen filas e índices. Las
mutaciones de strings, listas, structs/campos anidados o de la forma de una
tabla siguen usando el encoder canónico, y los pools no modelados permanecen
pendientes.

### Avance NewGRF — 2026-08-26

Action3 conserva ahora el bit de *wagon override* y la lista de motores de la
definición anterior, incluidos los IDs `ExtendedByte`. Cada asignación se
registra como vagón→motor sobrescriptor→cargo/grupo default y el renderer la
resuelve para la cabeza real del consist, sólo cuando ambos motores pertenecen
al mismo GRFID. La selección respeta el orden de OpenTTD (cargo específico
antes del default) y cae al sprite propio si no existe coincidencia. La
terminación SpriteStack por registro `0x100` (bit 31) ya está implementada;
las paletas especiales de vehículos (2CC/crash) ya están materializadas;
siguen abiertos scopes parent/relative avanzados y callbacks/layouts sin call
site completo.

Actualización #329-STATION-CB149-019 (2026-09-02): la comprobación de pendiente
de estaciones ferroviarias (`CB149`) aplica ahora la compatibilidad de
`OpenTTD` para GRF anteriores a la versión 8: se invierte el bit 10 del
resultado antes de decidir si la tesela es válida. Los parámetros de slope,
orientación, andén y posición se mantienen iguales y la query/execute sigue
siendo atómica antes de mutar el mapa. Continúan pendientes el scope completo
de `BaseStation`, vecinos y mensajes de error de texto GRF; #329 permanece
abierto.

Actualización #329-OBJECT-CB157-020 (2026-09-02): los objetos NewGRF guardan
también la versión Action8 del GRF en su spec. CB157 aplica la inversión del
bit 10 para GRF 7 antes de aceptar/rechazar cada tesela del footprint, tanto
en query como en execute, sin cobrar ni mutar parcialmente. Permanecen fuera
los scopes/vecinos completos, strings de error y callbacks adicionales.

### Avance NewGRF — 2026-08-27

La ruta de tile loop de `IndustryTile` ahora recibe los catálogos y pools de
la partida. Las teselas vanilla no consumen RNG ni alteran `m3`; un grupo
NewGRF estático conserva el trigger pendiente y los grupos Action2 random
consumen sólo los eventos alcanzables y la máscara de bits devuelta por
`ResolveRerandomisation`. La simulación normal y las 0x500 pasadas de
generación usan esta ruta; la API histórica sin catálogo queda como fallback
explícito para herramientas antiguas. El runtime NewGRF de industria sigue
abierto para callbacks de foundation/sonido y variables nativas que el modelo
no conserva.

Actualización RMAP-131 (2026-09-01): la primera divergencia ártica de
1024²/seed `1330935383` era una limpieza de árbol de humedal en la frontera
`landscape`. `TileLoopTreesAlps` había convertido el árbol a
`TREE_GROUND_ROUGH_SNOW` con densidad parcial, pero Rust lo limpiaba con
densidad 3 en lugar de conservar el valor que `MakeSnow` recibe desde `MAP2`.
La corrección conserva esa densidad para los dos tipos de suelo nevado y deja
`landscape` y `clear` exactos (0 teselas y 0 bloques 4×4); la primera
divergencia siguiente queda aislada en `towns` (11.021 teselas/1.657 bloques
4×4). RMAP-004 y los padres mantienen abierto el resto de la matriz.

Actualización RMAP-132 (2026-09-02): tras RMAP-131, la primera diferencia de
la misma cohorte era una carretera extra en `(716,229)`. OpenTTD acepta avanzar
por la carretera de `(724,236)` para después detener `GrowTownAtRoad` porque
pertenece al pueblo `247`; el port no aplicaba ese chequeo posterior y seguía
recorriendo el bloque, alterando el stream RNG y la fundación desde el pueblo
270. La caminata Rust conserva ahora `CanFollowRoad`, comprueba `OWNER_TOWN` y
`MAP2/TownID` después de avanzar y retorna sin reintentar direcciones. La
regresión cubre tanto el rechazo de una carretera ajena como la aceptación de
la propia. La cohorte ártica 1024²/seed `1330935383` queda exacta en
`landscape`, `clear`, `towns`, `industries`, `objects` y `trees`: 0 teselas, 0
bytes y 0 bloques 4×4; también coinciden los 373 centros y la frontera RNG.
El padre RMAP-004 y RMAP-024/RMAP-027/RMAP-030 siguen abiertos para otras
semillas, tamaños, layouts y fases posteriores.

Actualización RMAP-133 (2026-09-02): la primera divergencia tropical de
1024²/seed `1330935384` estaba en seis teselas de `landscape`, causada por
ejecutar el tile loop de árboles con el RNG determinista de simulación y no
consumir el sorteo previo de las teselas de selva. La generación usa ahora el
RNG global para todas las variantes; el trópico actualiza el sustrato desértico,
consume el `Random()` de rainforest y bloquea la propagación desde el desierto.
Las seis fronteras (`landscape`→`trees`) quedan exactas: 0 teselas, 0 bytes y
0 bloques 4×4 distintos. El cierre es acotado a esta regla y cohorte; RMAP-004
y RMAP-018 conservan abiertas otras semillas, tamaños, configuraciones y
ticks posteriores.

Actualización RMAP-134 (2026-09-02): la primera divergencia tropical de
1024²/seed `1330935386` en `towns` provenía de sustituir
`SpiralTileSequence(tile, 2, 0, 0)` por un cuadrado al buscar puentes
paralelos. El cuadrado rechazaba la boca `(679,509)` por una rampa en
`(677,509)` que el oráculo no visita; el preflight ahora reproduce las coronas
y el salto `DIR_W` de la espiral. La comparación de las seis fases queda
exacta por tesela, bytes y bloques 4×4 (**0/0/0**). RMAP-004 y los padres de
pueblos siguen abiertos para otras longitudes, semillas, tamaños y ticks.

Actualización RMAP-135 (2026-09-02): la primera divergencia temperate de
1024²/seed `1330935387` en `towns` era una frontera RNG al visitar la casa
ocupada `(19,487)`. `GrowTownInTile` ejecuta `Chance16(1,6)` para
`LevelTownLand` antes de `IsRoadAllowedHere`, aunque la terraformación no
pueda modificar una casa; el port filtraba esa tesela demasiado pronto. La
rama Rust conserva ahora el sorteo y la regresión fija el estado RNG y los
bytes de la casa. La fase `towns` queda exacta (0 teselas/0 bloques 4×4) para
la semilla; RMAP-004 y los padres urbanos siguen abiertos para otras
semillas, tamaños, layouts, climas y fases.

Actualización RMAP-136 (2026-09-02): la primera divergencia de industrias
temperate en 2048²/seed `1330935404` era el límite de terraformación usado al
validar una plataforma. El valor histórico 15 rechazaba la esquina de nivel
16, mientras OpenTTD resuelve el setting automático de una partida nueva a un
límite mínimo 30. `TerraformModel` mantiene 15 para comandos manuales sin
setting resuelto y las plataformas reciben el límite efectivo de
`ConstructionSettings`. La frontera `industries` queda exacta en las
4.194.304 teselas (0 teselas, 0 campos y 0 bloques 4×4); RMAP-004/RMAP-056
mantienen abiertos otras semillas y fases.

Actualización RMAP-137 (2026-09-02): la frontera `trees` de temperate 2048²/
seed `1330935404` reveló que el mínimo automático 30 no era el valor usado por
OpenTTD. `GenerateWorld` calcula `GetEstimationTGPMapHeight()` según tamaño y
relieve, suma 15 y aplica el mínimo 30; con `terrain_type=Flat` la estimación
es 19 y el límite persistido es 34. `effective_new_game_map_height_limit`
centraliza la regla y el cliente/dumper la aplican antes de población y
árboles. La traza de `PlaceTree` queda 740.213/740.213 y la comparación raw de
la frontera coincide en 4.194.304 teselas (0 teselas, 0 campos y 0 bloques 4×4).
RMAP-004/RMAP-009/RMAP-018 siguen abiertos para otras configuraciones, climas,
tamaños, semillas y ticks posteriores.

Actualización RMAP-138 (2026-09-02): la cohorte temperate 1024² se amplió a
las seeds `1330935388`–`1330935391` después de RMAP-137. Las seis fronteras de
cada mapa (`landscape`, `clear`, `towns`, `industries`, `objects`, `trees`)
comparan 1.048.576 teselas sin diferencias de campos ni bloques 4×4 (24/24
fronteras exactas). El padre RMAP-004 y los issues de pueblos, ríos e industrias
siguen abiertos fuera de esta cohorte.

Actualización RMAP-139 (2026-09-02): el comparador por fases y el dumper ya
aceptan y aíslan `amount_of_rivers`, `min_river_length`, `river_route_random` y
`water_borders`. Las combinaciones verificadas en temperate 256²/512² quedan
exactas en las seis fronteras, comparando raw y bloques 4×4. RMAP-018 conserva
abierta la cobertura combinatoria, otros climas/tamaños y ticks posteriores.

Actualización #326-FND-001 (2026-09-02): la comparación con
`newgrf_house.cpp` y `newgrf_industrytiles.cpp` confirmó que OpenTTD consulta
`CBID_HOUSE_DRAW_FOUNDATIONS` (`0x150`) y `CBID_INDTILE_DRAW_FOUNDATIONS`
(`0x30`) sólo
para teselas inclinadas; `CALLBACK_FAILED` conserva la fundación vanilla y un
resultado booleano cero la suprime. El renderer Rust incorpora ambos bits de
callback, evalúa el contexto Action2 existente y deja el layout custom sobre
el relieve original cuando corresponde. Dos regresiones ECS cubren una casa y
una tesela de industria con callback cero, y una prueba de core fija la
conversión booleana completa (15 bits). El alcance queda acotado a foundations:
callbacks de color/animación/sonido, foundations del compositor de aeropuertos
y las variantes ferroviarias de pendiente/túnel siguen abiertos; el bloque
`RTSG_DEPOT` se cubre en la actualización #326-RAIL-DEPOT-001; por
eso #326 no se cierra.

Actualización #326-RAIL-DEPOT-001 (2026-09-02): `RailSpriteType::Depot`
(`RTSG_DEPOT`, selector 8) se importa por `RailType` y se consume durante
`DrawRailTileSeq`. El mapeo respeta el desplazamiento nativo desde
`SPR_RAIL_DEPOT_SE_1` y sus seis capas relocatables, resuelve Action2 con el
contexto de tesela/fecha/random y mantiene la caja `TILE_SEQ_LINE`, el orden
del sorter y el vínculo a la fundación nivelada. La regresión
`newgrf_rail_depot_group_replaces_relocated_building_layers` valida las dos
capas SE con sprites custom; la prueba de slots fija las cuatro orientaciones.
Los grupos de túnel/portal, pendientes y paletas especiales continúan abiertos,
por lo que #326 permanece en curso.

Actualización #326-RAIL-TUNNEL-002 (2026-09-02): el parser/runtime conserva
ahora los selectores `RTSG_TUNNEL` (3) y `RTSG_TUNNEL_PORTAL` (10) por
`RailType`. En una boca ferroviaria con `UsesOverlay()` el renderer consume la
vista `RTSG_TUNNEL` con su ancla NFO como `DrawGroundSprite`, antes de PBS y
catenaria, aunque la fachada `RTSG_TUNNEL_PORTAL` sea independiente; cada vista
cae de forma atómica al portal OpenGFX si falta o el clima/fecha no coincide. La regresión
`newgrf_rail_tunnel_group_draws_custom_surface_when_portal_is_defined` cubre
una boca inclinada `SLOPE_NE` y confirma la capa independiente del sorter.
Actualización #326-RAIL-TUNNEL-003 (2026-09-02): cuando `RTSG_TUNNEL_PORTAL`
resuelve una vista Action2, la fachada ya usa el sprite custom y su centro NFO
en la capa sortable (`tunnel-front-newgrf`), combinado como child de la base.
El extractor `extract_rail_tunnel_base_sprites.py` incorpora ahora la base
Action5 `0x17` (`SPR_RAILTYPE_TUNNEL_BASE`) por clima, con sus ocho slots
normales y ocho nieve/desierto, y el renderer la carga antes del overlay; si un
PNG/atlas falta, sólo esa capa cae al portal OpenGFX. La regresión de túnel
verifica el parent Action5 y el child del portal. Las pendientes/rotaciones y
las paletas especiales continúan abiertas; #326 permanece en curso.
Actualización #326-RAIL-TUNNEL-004 (2026-09-02): la orientación de cada boca
se toma ahora de `GetTunnelBridgeDirection` (`m5 & 3`), igual que
`DrawTile_TunnelBridge`, y no de la pendiente efectiva calculada por el
renderer. Esto cubre saves importados que conservan una pendiente de terreno
distinta de los bytes de dirección. La regresión
`newgrf_rail_tunnel_group_draws_custom_surface_when_portal_is_defined` usa
deliberadamente `m5=SW` con `SLOPE_NE` y verifica el slot frontal Action5
correspondiente; una prueba de sprite cubre las cuatro direcciones y las
variantes normal/nieve. Las pendientes no válidas, las rotaciones de
compositor y las paletas especiales siguen abiertas; #326 no se cierra.

Actualización #329-VEHICLE-TRAIN-002 (2026-09-02): la predicción de salida de
tesela usada por las señales ferroviarias ya recibe el catálogo activo. Cuando
`train_would_leave_tile_this_tick` se ejecuta durante `Train::CheckSignals`,
CB36 (`PROP_TRAIN_SPEED`, `0x09`) se resuelve sobre una copia del vehículo y
el resultado limita los dos pasos del *locomotive handler*; un callback que
reduce la velocidad ya no se sustituye por `EngineDef::max_speed` vanilla.
También la salida desde depósito usa esta variante. La regresión
`train_tile_prediction_uses_newgrf_speed_property` fija la diferencia entre la
predicción vanilla y la de un motor NewGRF. Potencia/TE/arrastre dinámicos,
otras propiedades Action0 y APIs legacy sin catálogo siguen abiertos en #329.

Actualización #329-VEHICLE-SHIP-001 (2026-09-02): la velocidad de un barco
NewGRF respeta ahora el orden de `Ship::UpdateCache` de OpenTTD. El controlador
consulta primero `CBID_VEHICLE_MODIFY_PROPERTY` (`PROP_SHIP_SPEED`, `0x0B`) y
aplica después la fracción de mar/canal, en lugar de volver a
`EngineDef::max_speed` y descartar el resultado dinámico. El helper
`ship_speed_for_tile_with_speed` conserva la API vanilla y permite probar la
propiedad ya resuelta; `ship_cb36_speed_is_fractioned_after_callback` cubre un
CB36 que devuelve 80 con fracción oceánica 128/256 (resultado 40). Las
fracciones por clase de agua y los límites de puentes siguen aplicándose; las
otras propiedades runtime de vehículos, APIs legacy sin catálogo y scopes
avanzados mantienen abierto #329.

Actualización #329-VEHICLE-CONSIST-003 (2026-09-02): las operaciones de depósito
que cambian la topología (`AttachWagonToConsist`, `DetachConsistUnit`,
`MoveRailVehicle` y venta) vuelven a calcular `ConsistChanged` con el catálogo
activo y el mapa, en vez de dejar la caché vanilla que escribían los helpers
legacy. La importación `.sav` también reatacha los trenes y refresca capacidad,
velocidad, potencia, peso y esfuerzo tractor con los callbacks CB36 disponibles;
la validación de enganche acepta IDs de motores NewGRF presentes en el catálogo.
`attach_newgrf_wagon_refreshes_callback_consist_cache` comprueba que una
capacidad dinámica 77 se conserva en la cabeza después de enganchar el vagón.
Las cadenas articuladas avanzadas, las APIs directas sin catálogo y otras
propiedades Action0 siguen abiertas; #329 no se cierra.

Actualización #329-VEHICLE-AUTOREPLACE-004 (2026-09-02): `ReplaceChain` ya
resuelve la capacidad CB36 del motor nuevo inmediatamente al autoreemplazar,
después de fijar el cargo/refit efectivo, y aplica el multiplicador de
capacidad del catálogo de cargos. La misma ruta cubre las traseras de trenes
dual-head y las piezas creadas en depósito; una regresión vial confirma que la
capacidad NewGRF no conserva la del motor anterior durante el tick de salida.
El refresco de capacidades en `LoadUnloadStation` sigue siendo necesario para
callbacks que dependan del estado que cambia durante la partida, por lo que
#329 permanece abierto.

Actualización #329-VEHICLE-AIRPORT-005 (2026-09-02): la salida de una
aeronave desde la FTA del aeropuerto ya conserva el catálogo activo hasta
`finish_takeoff`. El cierre de despegue resuelve `PROP_AIRCRAFT_SPEED`
(`CB36`, `0x0C`) sobre el motor NewGRF y reinicia `subspeed`, igual que las
rutas de despegue no-FTA; antes esa transición llamaba al helper vanilla y
podía recuperar `EngineDef::max_speed` aunque el resto del vuelo ya usara el
catálogo. La regresión
`finish_takeoff_uses_active_catalog_speed_callback` fija un callback que
reduce la velocidad al abandonar la pista. El scope de aeropuerto y las APIs
legacy sin catálogo siguen abiertos, por lo que #329 no se cierra.

Actualización #329-VEHICLE-AIRCRAFT-SUBTYPE-007 (2026-09-02): la FTA y el
evento `AirplaneTouchdown` ya resuelven el subtipo de aeronave desde el
catálogo activo (`EngineDef::is_helicopter`), no sólo desde los IDs vanilla.
Así un helicóptero definido por Action0 conserva `HeliLanding`/`HeliTakeoff`,
no reserva la pista de ala fija y no dispara el callback de touchdown de avión.
La regresión `fta_approach_uses_active_catalog_helicopter_flag` cubre la
entrada a un aeropuerto mixto con un ID NewGRF. Los demás callbacks de subtype,
sprites y scopes de aeropuerto siguen abiertos; #329 no se cierra.

Actualización #329-VEHICLE-ROAD-SLOPE-006 (2026-09-02): la sincronización de
`RoadZPosAffectSpeed` al terminar cada subpaso vial ya recibe el catálogo
activo. El techo de la bajada consulta `PROP_ROAD_SPEED` (`CB36`, `0x15`) sobre
el motor NewGRF antes de permitir el empuje de dos unidades; la API legacy
conserva el fallback vanilla. La regresión
`slope_sync_uses_active_catalog_speed_callback` cubre el caso en que el
callback reduce el techo y evita que la bajada recupere velocidad vanilla. Las
otras propiedades viales y las APIs directas sin catálogo siguen abiertas, por
lo que #329 no se cierra.

Actualización #329-VEHICLE-SAV-AIRCRAFT-008 (2026-09-02): el escritor `VEHS`
clasifica ahora cada aeronave contra `GameState.engine_catalog`, incluyendo el
flag `EngineDef::is_helicopter` de motores Action0/NewGRF. Al guardar una
partida con un ID de motor propio se emite la cadena ala fija+sombra o
helicóptero+sombra+rotor correcta; el cálculo de índices sparse y las
referencias `next` usan la misma clasificación. La regresión
`vehs_uses_newgrf_catalog_for_aircraft_subtype` verifica las tres filas del
helicóptero custom. La tabla `VEHS` aún usa un encabezado mínimo al
reserializar cambios estructurales, variables o anidados; los cambios
escalares fijos compatibles se fusionan sobre el cuerpo importado. #329
permanece abierto.

Actualización #329-VEHICLE-SAV-VEHS-009 (2026-09-02): la importación conserva
el cuerpo nativo de `VEHS` junto con una huella de sus filas semánticas. En un
ciclo cargar→guardar sin cambiar vehículos, el exportador reemite exactamente
el chunk original, incluidas columnas añadidas por versiones futuras de
OpenTTD; si cambia una fila (por ejemplo `cur_speed`), invalida el passthrough y
reconstruye la tabla canónica para no guardar estado obsoleto. La regresión
`imported_vehs_body_is_reused_until_vehicle_semantics_change` cubre ambos
caminos. Una mutación compatible que conserva schema y tamaño codificado
—también si es variable o anidada— conserva las columnas desconocidas mediante
la fusión común; los cambios de tamaño, filas/índices o estructura siguen
cayendo al writer canónico.
#329 sigue abierto.

Actualización #329-VEHICLE-SAV-ORDL-010 (2026-09-02): el mismo snapshot de
passthrough ahora cubre `ORDL`. Las listas de órdenes se reemiten byte a byte
cuando sus filas semánticas no cambian, por lo que campos futuros del pool de
órdenes sobreviven a un ciclo SAV. Si cambia una orden o la topología de una
lista, se usa el encoder canónico y no se conserva una referencia obsoleta.
`imported_vehs_body_is_reused_until_vehicle_semantics_change` verifica `ORDL`
y `VEHS` en conjunto. Las mutaciones escalares fijas compatibles conservan ahora
columnas futuras; cambios de strings/listas/structs, filas o índices siguen
pendientes junto con la semántica completa; #329 no se cierra.

Corrección #371–#374 (2026-09-05): la frase anterior sobre cambios de
strings/listas queda superada para strings, listas escalares y struct-lists de
**raíz** con descriptor recursivo, filas e índices compatibles. `CITY.psa_list`
y `CITY.supplied` preservan columnas futuras y re-guardado OpenTTD; el segundo
anuncia SLV 358 y normaliza la historia a 61 registros. Las listas de órdenes,
subschemas desconocidos y cambios de forma/topología siguen requiriendo la
frontera estable indicada allí. Detalle y reproducción:
[sav-struct-373.md](sav-struct-373.md). `INDY.accepted`/`produced` también
emiten ahora sus 61 posiciones nativas sin convertir las filas opacas;
[sav-indy-history-374.md](sav-indy-history-374.md) conserva la evidencia.

Actualización #329-VEHICLE-SAV-TABLES-011 (2026-09-02): el snapshot de
interoperabilidad se extendió a `STNN`, `CITY` e `INDY`. En cargar→guardar sin
cambiar estaciones, ciudades o industrias se reemiten sus cuerpos nativos y
sus columnas futuras permanecen intactas; una mutación semántica cae al
encoder canónico, igual que `ORDL`/`VEHS`. La huella es por filas y no inventa
campos nuevos cuando cambia el conjunto de entidades. La fusión de escalares
fijos ya se aplica cuando la huella de filas e índices permanece estable; las
mutaciones variables/anidadas y los demás pools nativos siguen pendientes;
#329 permanece abierto.

Actualización #329-VEHICLE-SAV-META-012 (2026-09-02): el passthrough de tablas
sin mutación ahora incluye `PATS`, `ECMY` y `CAPY`. Los ajustes de partida, los
contadores económicos globales y las liquidaciones de carga se comparan por
filas semánticas conocidas; mientras la huella no cambie, el exportador
conserva el header y las columnas nativas completas, y cuando cambia un
ajuste o pago vuelve al encoder canónico. Las regresiones
`imported_vehs_body_is_reused_until_vehicle_semantics_change` y
`capy_runtime_front_id_is_translated_to_sparse_vehicle_ref` cubren estos
caminos. `PLYR`, `GRPS` y `ERNW` también usan la fusión común para cambios
escalares fijos compatibles. Mutaciones variables/anidadas, cambios de forma y
los demás pools nativos siguen pendientes; #329 permanece abierto.

Actualización #329-VEHICLE-SAV-FLEET-014 (2026-09-02): `GRPS` y `ERNW` también
conservan el cuerpo nativo cuando los grupos, reglas, huecos de pool y enlaces
de autorrenovación no cambian. La exportación sigue normalizando IDs y cadenas
antes de comparar, y cualquier alta, baja, renombrado o cambio de regla cae al
encoder canónico para no conservar referencias obsoletas. La regresión
`ottn_roundtrip_preserves_group_names_and_autoreplace_rules` cubre ambos pools.
La fusión de escalares fijos compatibles ya está cubierta; mutaciones
variables/anidadas, cambios de forma y pools aún no modelados continúan
pendientes; #329 permanece abierto.

Actualización #329-VEHICLE-SAV-LINKGRAPH-015 (2026-09-02): `LGRP` conserva
ahora su cuerpo nativo cuando los nodos, aristas, cargos y referencias a
estaciones coinciden con la huella reconstruida. `LGRJ`/`LGRS` mantienen el
passthrough runtime existente y se invalidan cuando `LinkGraphStats` registra
un viaje nuevo. La regresión `export_roundtrip_preserves_lgrp_edge` comprueba
el cuerpo byte a byte; una mutación de flujo cae al encoder canónico. Las
mutaciones variables/anidadas y la ejecución completa de jobs de cargodist
siguen pendientes; #328 permanece abierto.

Actualización #329-VEHICLE-SAV-NGRF-016 (2026-09-02): la tabla `NGRF` conserva
ahora su cuerpo nativo cuando el stack activo, el orden, las versiones y los
parámetros conocidos coinciden. Así sobreviven el digest, la paleta y columnas
añadidas por versiones futuras en un round-trip sin cambios; alterar un
parámetro o la composición del stack invalida el passthrough y reconstruye sólo
`NGRF`. La regresión
`ottn_roundtrip_preserves_active_newgrf_configuration` cubre ambos caminos.
La resolución de archivos GRF ausentes y la fusión de mutaciones variables o
anidadas siguen pendientes; #329 permanece abierto.

Actualización #329-VEHICLE-SAV-DATE-017 (2026-09-02): `DATE` conserva ahora su
cuerpo nativo cuando la fecha de calendario, el tick y el estado RNG coinciden.
El round-trip sin cambios mantiene campos futuros del reloj; avanzar el tick,
alterar el RNG o cambiar el calendario invalida únicamente esa tabla y usa el
encoder canónico. La regresión
`imported_vehs_body_is_reused_until_vehicle_semantics_change` verifica el caso
sin mutación. Quedan pendientes mutaciones variables/anidadas y los pools aún
no modelados; #328/#329 permanecen abiertos.

Actualización #329-VEHICLE-SAV-CAPA-018 (2026-09-02): el pool físico `CAPA`
conserva su cuerpo nativo cuando los paquetes de estación/vehículo y sus
referencias semánticas no cambian. Las columnas futuras de origen, tránsito y
feeder sobreviven al round-trip; modificar, agregar o retirar un paquete
invalida `CAPA` y vuelve al encoder canónico junto con los enlaces recalculados
de `STNN`/`VEHS`. La regresión
`export_roundtrip_preserves_station_and_vehicle_cargo_packets` verifica ambos
caminos. Las mutaciones variables/anidadas y los pools nativos restantes
continúan pendientes; #328 permanece abierto.

Actualización #329-VEHICLE-SAV-PLYR-013 (2026-09-02): `PLYR` comparte ahora la
huella de filas con las tablas SAV anteriores. Un round-trip sin cambios de
compañías conserva byte a byte el header y las columnas futuras de dinero,
ajustes, economía, libreas y retrato; cualquier cambio de empresa o de una
regla asociada invalida sólo ese cuerpo y usa el encoder semántico actual. La
regresión `imported_vehs_body_is_reused_until_vehicle_semantics_change` verifica
la reutilización. Las mutaciones variables/anidadas y la cobertura diferencial
de pools todavía requieren trabajo; #329 permanece abierto.

Actualización #329-INDUSTRY-METADATA-023 (2026-09-02): la entidad `Industry`
conserva ahora fundador, fecha absoluta de construcción, tipo de creación,
flags de control, marca de entrega y último año de producción, además de
layout/random. El comando de fundación asigna la compañía activa y
`NORMAL_GAMEPLAY`; la generación procedural corrige fundador/tipo a
`INVALID_OWNER`/`MAP_GENERATION`, y la producción actualiza
`last_prod_year`. El scope padre Action2 expone `0x45`/`0x46`/`0x47`,
`0xA7`/`0xA9`/`0xAC`, `0xB0`/`0xB3` y `0xB4` (además de `0x6E` por cargo), y
`INDY` importa/exporta los campos nativos correspondientes, incluida la fecha
de última aceptación de cada slot. PSA de industria, aeropuerto y referencias
de pueblo ya tiene importación, hidratación de filas y exportación nativa:
`INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` se resuelven;
los registros no nulos de industria/estación se hidratan y las listas de pueblo
se conservan por índice. Los historiales anidados de `INDY`
(`history`, `valid_history` y `accumulated_waiting`) se hidratan y actualizan
en entrega, transferencia, barrido diario y rollover mensual para cargos
representables, y el writer los reemite. #329 y el bloque SAV siguen abiertos
por callbacks de tesela, callbacks PSA de pueblos, storages de casas/objetos,
cargos custom y callbacks restantes.

Actualización #329-INDUSTRY-PSA-024 (2026-09-02): los callbacks de producción
y cambio de nivel siembran y escriben los registros persistentes `7C` de la
industria; los contextos de tesela ya leen el storage del padre. El importador
decodifica `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC`,
hidrata los registros no nulos en `Industry` o `Station`, conserva las listas de
pueblo y el exportador vuelve a emitir el pool completo con índices densos y
256 valores por fila, sin descartar storages de entidades que aún no tienen
runtime. Quedan pendientes el
writeback de callbacks de tesela, la asociación runtime completa GRFID/feature,
storages de otras entidades, conexión de los historiales al runtime,
invalidación tras mutaciones y cargos custom; #329 y #328 permanecen abiertos.

Actualización #329-TOWN-PSA-025 (2026-09-02): `CITY.psa_list` ya hidrata los
registros no nulos de `PSAC` por GRFID en cada `Town`, conserva el `storage_id`
original y los reemite junto con las filas densas. Los contextos Action2 de
casas y objetos copian ahora también las variables conservadas de
`TownScopeResolver` (posición, población, crecimiento, radios, ratings,
historial y entregas) al scope parent; por lo que `7C` deja de ser sólo
passthrough en esos dos call sites. El writeback de registros modificados, los
scopes parent de estaciones/aeropuertos y las mutaciones estructurales de
casas/objetos siguen pendientes; #329 y #328 permanecen abiertos.

Actualización #329-TOWN-CITY-027 (2026-09-02): una inspección de un `CITY` real
de OpenTTD 15.3 mostró que el parser anterior sólo retenía posición y nombre.
Ahora `Town` conserva también la identidad del generador de nombres, flags,
ratings, máscara `have_ratings`, unwanted, metas, contadores de crecimiento,
exclusividad, layout, estatuas, `valid_history` y texto de GameScript. Los
flags y las variables `0x40`, `0x92`, `0x93` y `0xAE` llegan al scope parent de
casas/objetos, con una regresión sintética que comprueba la codificación de la
tabla nativa. Esto reduce la divergencia de lectura/runtime, pero no cierra
interoperabilidad completa: el writer canónico ahora reemite estos campos y
las listas/structs modelados; un `CITY` importado sin mutaciones continúa
protegido por passthrough. Persisten las columnas desconocidas, el writeback de
PSA y la conexión con crecimiento/economía. #328/#329 siguen abiertos.

Actualización #329-TOWN-CITY-028 (2026-09-02): la divergencia de listas
anidadas de `CITY` quedó aislada y cubierta: `supplied` conserva cada cargo y
sus muestras mensuales (`production`/`transported`) y `received` conserva los
contadores `old_max`, `new_max`, `old_act` y `new_act` en el orden nativo. El
modelo aún no usaba estas series para crecimiento/economía; el writer canónico
ya las reemite y el passthrough de un save sin cambios sigue protegiendo el
cuerpo original. #328/#329 permanecen abiertos.

Actualización #329-TOWN-CITY-029 (2026-09-02): el writer canónico de `CITY`
emite ahora la metadata nativa modelada, los arrays fijos `ratings`/
`unwanted`/`goal`, las listas `supplied`/`received` y `psa_list`. El encoder
respeta los tamaños de OpenTTD (`MAX_COMPANIES` y `NUM_TAE`, con el slot
`TAE_NONE`) y un fixture generado fue aceptado por OpenTTD 15.3. La caché
`cache.population` permanece derivada desde `MAP*`; una mutación estructural
de listas todavía usa el fallback canónico y puede perder columnas anidadas
desconocidas. La hidratación no conecta aún las series con crecimiento/economía
ni hace writeback de PSA de pueblos, por lo que #328/#329 continúan abiertos.

Actualización #329-TOWN-CITY-030 (2026-09-02, commit `b7429397`): al importar `CITY`, los
contadores `received.old_act/new_act` hidratan las ventanas de crecimiento y
se desplazan junto con el rollover mensual; las entregas runtime actualizan
también el vector nativo antes de serializarlo. La producción de pasajeros y
correo identifica la casa por `MAP2` (con fallback al pueblo más cercano),
actualiza `supplied` y sus dos muestras mensuales, y los scopes parent exponen
producción/transporte (`0xBA`–`0xCB`) desde esas series. Las columnas custom,
los cargos NewGRF y el writeback de PSA de pueblos siguen pendientes; #328/#329
continúan abiertos.

Actualización #329-INDTILE-CARGO-ACCEPTANCE-058 (2026-09-03, commit `67ef8101`):
`IndustryTileSpecDef` conserva las máscaras `0x2B`/`0x2C` y el flag
`AcceptsAllCargo`. La ruta runtime evalúa primero `CBID_INDTILE_ACCEPT_CARGO`
(tres slots de cargo locales de 5 bits) y luego
`CBID_INDTILE_CARGO_ACCEPTANCE` (tres cantidades de 4 bits), con el contexto
completo de tesela/industria, CTT y writeback de PSA. `station_coverage_at_with_newgrf`
usa esa tabla exacta por tesela y `unload_vehicles` la consulta antes de aceptar
el lote; por eso un callback que devuelve cero no vuelve a aceptar `Goods` por el
proxy genérico de fábricas. La regresión cubre el reemplazo de slots/cantidades y
la aceptación/rechazo efectiva en una estación. El fallback estático y las APIs
legacy permanecen intactos; cargos custom/CTT no resolubles, reatachación económica,
sonido y callbacks restantes siguen pendientes y #329 no se cierra.

Actualización #329-CUSTOM-CARGO-RUNTIME-059 (2026-09-04, commit `bd613e2a`):
los `CargoSpec` definidos por `NewGRF` reciben un ID global estable en el rango
`31..62`, conservando `(GRFID, local_id)` para resolver la CTT. Hasta 32 slots
se transportan como `CargoType::Custom` por `CargoStock`, `StationGoods`,
`CargoTimeSincePickup`, packets, cobertura exacta, producción/entrega de
industrias, pagos, ratings, cargodist, refit y autoreplace. Las regresiones
cubren asignación de slot, round-trip stock→packet, aceptación de tesela y un
ciclo de producción económico. El límite sigue siendo deliberado: el SAV nativo
no tiene todavía columnas custom rehidratables, los slots `63+` continúan
opacos, la semántica completa de CTT/GUI/variables y el binding GameScript del
monitor no están cerrados; sin el GRF instalado las filas `INDY` se conservan
pero no se ejecutan. #329 permanece abierto.

Actualización #329-SAV-GLOBAL-CARGO-060 (2026-09-04, commit `566ce56a`):
el importador centraliza la frontera `SLV_55`: las tablas antiguas conservan
slots relativos al clima, mientras `STNN.goods`, `INDY.accepted/produced`,
`VEHS.common.cargo_type` y `LGRP.cargo` modernos usan el ID global. Los IDs
`31..62` se hidratan como `CargoType::Custom` aun sin catálogo, por lo que
stocks, packets, historiales y vehículos no se descartan; `63` y valores fuera
del runtime siguen siendo opacos. El writer emite siempre `NUM_CARGO=64` con
IDs globales y convierte correctamente filas legacy al reexportar. Las
regresiones cubren slots árticos antiguos/modernos, custom en estaciones,
vehículos, industrias y linkgraph. #328/#329 siguen abiertos por propiedades,
CTT, textos y callbacks económicos que requieren el `CargoSpec` activo.

Actualización #329-SCRIPT-CARGO-MONITOR-061 (2026-09-04, commit `6266171f`):
la validación de `ScriptCargoMonitor` ya reconoce los cargos custom registrados
en `GameState.cargo_spec_catalog`, además de los cargos vanilla del clima. Las
consultas de pueblo/industria mantienen el contrato nativo de `-1` para un cargo
sin `CargoSpec`, activación explícita, reset al leer y saturación a `i32`; una
entrega final con `CargoSource` custom actualiza pickup y delivery con el mismo
ID global. La integración Squirrel/GameScript completa y propiedades económicas
de catálogo siguen pendientes; #329 continúa abierto.

Actualización #329-CARGO-WEIGHT-062 (2026-09-04, commit `fd573da5`):
`RoadVehicle` calcula la masa de carga con el `CargoSpec` activo (`weight` en
dieciseisavos de tonelada), por lo que un cargo custom deja de usar siempre el
peso genérico de una tonelada. El valor se aplica en cada tick del controlador
vial y se conserva el fallback para callers sin catálogo; #329 sigue abierto
hasta cubrir consist ferroviario, `freight_trains` y el resto de propiedades.

Actualización #329-STATION-SCOPE-063 (2026-09-06, issue [#389](https://github.com/cavazquez/openttdrs/issues/389)):
`action2_eval_ctx_from_station` ya no deja vacío el scope de una estación en
las APIs legacy: expone los sentinels nativos de plataforma/vía (`0x40`,
`0x41`, `0x46`, `0x47`, `0x49`), owner (`0x43`), PBS (`0x44`), continuación
no disponible (`0x45`), frame (`0x4A`) y random/triggers (`0x5F`). El nuevo
`apply_station_availability_callback_at` comparte con el renderer el contexto
map-aware de la tesela y hace writeback de `7C` después del callback; una
regresión también cubre el fallback cuando el índice/tile está obsoleto. La
construcción conserva el resolver nulo de OpenTTD, sin `Station` ni PSA. Quedan
fuera de este corte las variables vecinas `0x66`/`0x68`/`0x6A`/`0x6B`, strings,
sonidos, layouts 16-bit y los scopes completos de `BaseStation`/aeropuerto;
#329 permanece abierto.

Actualización #329-STATION-NEIGHBOURS-064 (2026-09-06, issue [#390](https://github.com/cavazquez/openttdrs/issues/390)):
los constructores catalogue-aware de `station_action2` materializan sólo las
consultas vecinas declaradas por el Action2 activo. `0x66`/`0x67` resuelven
frame y land info con offsets firmados, wrap y versión GRF; `0x68` codifica
gfx/eje/estación y la identidad `(GRFID, local_id)`; `0x6A`/`0x6B` aplican los
sentinels y el filtro de GRF de OpenTTD. La ruta de sprites plana, layouts
`TileSeq` y CB140–142 reciben el catálogo; las APIs legacy sin catálogo no
inventan vecinos. La regresión cubre misma/diferente estación, ejes,
parámetros coexistentes, wrap y teselas ausentes. Continúan fuera sonidos,
strings, layouts 16-bit y scopes completos de `BaseStation`/aeropuerto; #329
permanece abierto.

Actualización #329-STATION-GENERAL-065 (2026-09-06, issue [#391](https://github.com/cavazquez/openttdrs/issues/391)):
el contexto de estación expone las variables generales modeladas `0x48`
(máscara de cargos vanilla aceptados), `0x82` (50), `0x86` (0 reservado) y
`0xF0` (facilities derivadas de `StopKind`) tanto en la ruta map-aware como en
la API legacy. Las regresiones cubren rail, bus, truck, dock, airport y
waypoint; IDs custom fuera de los 32 bits nativos no se aliasan. Strings,
fecha de construcción, estados de road stop y scopes completos de `BaseStation`
continúan en #329. El historial de vehículos `0x8A` se publica por separado
en #396.

Actualización #329-STATION-AIRPORT-VARS-066 (2026-09-06, issue [#392](https://github.com/cavazquez/openttdrs/issues/392)):
`station_action2` expone `0xF1` (tipo compacto `TTDPatch` `0..3`, preservando
Action0 `0x0D` para NewGRF), `0xF6` (palabra baja de `airport_blocks`) y `0xF7`
(bits 8..15) en legacy y map-aware. Las regresiones cubren aeropuerto vanilla,
tipo NewGRF y bloques FTA no nulos. El historial `0x8A` se cubre en #396 y
los estados `0xF2`/`0xF3` se publican en #399; los scopes restantes continúan
en #329.

Actualización #329-STATION-FACILITIES-067 (2026-09-06, issue [#393](https://github.com/cavazquez/openttdrs/issues/393)):
`StopKind` centraliza la máscara `StationFacilities` de `0xF0`: los waypoints
conservan también su facilidad de transporte (`RailWaypoint=0x81`,
`RoadWaypoint=0x86`). El resolver map-aware de `RoadStop` ya expone `F0` para
bus, truck y waypoint; no se inventan bits cuando no hay estación. El padre
#329 continúa abierto.

Actualización #329-STATION-HAD-VEHICLE-396 (2026-09-06, issue [#396](https://github.com/cavazquez/openttdrs/issues/396), commit `73288748`):
`Station.had_vehicle_of_type` conserva los bits nativos de tren, bus, camión,
avión y barco; se actualiza al prestar servicio de carga o descarga y los
waypoints exponen `HVOT_WAYPOINT`. `station_action2` y la ruta legacy devuelven
el mismo bitset en `0x8A`, con regresión de todos los bits y round-trip JSON.
La lectura/escritura de `STNN.normal.had_vehicle_of_type` ya está cubierta por
#397; los estados `0xF2`/`0xF3` de `RoadStop` se separan en #399; #329 continúa
abierto.

Actualización #329-STATION-HAD-VEHICLE-SAV-397 (2026-09-06, issue [#397](https://github.com/cavazquez/openttdrs/issues/397)):
el puente SAV lee `STNN.normal.had_vehicle_of_type` en filas modernas y
legacy, hidrata el bitset de `Station` y vuelve a emitir el byte al escribir
`STNN`. La regresión cubre parser y writer con bitsets no nulos; los campos
ausentes conservan cero. `last_vehicle_type` ya está cubierto por #398; los
scopes restantes continúan pendientes en #329.

Actualización #329-STATION-LAST-VEHICLE-SAV-398 (2026-09-06, issue [#398](https://github.com/cavazquez/openttdrs/issues/398)):
`STNN.normal.last_vehicle_type` ya se lee en filas modernas y legacy, se
hidrata en `Station.last_vehicle_type` y se vuelve a emitir al guardar
`STNN`. `VEH_INVALID`, train, road, ship y aircraft conservan sus códigos
nativos; bus, camión y tranvía comparten `VEH_ROAD` en el formato OpenTTD y se
normalizan al tipo road compatible del modelo. Las regresiones cubren parser,
hydration, writer→parser y la tabla de códigos. Los estados `0xF2`/`0xF3` se
publican en #399; los scopes restantes continúan pendientes en #329.

Actualización #329-ROADSTOP-STATUS-399 (2026-09-06, issue [#399](https://github.com/cavazquez/openttdrs/issues/399)):
`Station.road_stop_status` conserva el byte base de `RoadStop::status` con
compatibilidad JSON. `StationScope` expone `0xF2` para truck y `0xF3` para bus
solamente cuando coincide el tipo de parada. La simulación reconstruye
`Bay0Free`, `Bay1Free`, `BaseEntry` y `EntryBusy` desde la geometría y los
vehículos primarios en cada límite de tick; los tests cubren bahías, ocupación,
drive-through y el aislamiento de rail. El estado no se agrega al formato SAV:
OpenTTD lo deriva del pool `RoadStop` al cargar. Pools físicos separados,
colas drive-through completas y scopes restantes siguen pendientes en #329.

Actualización #329-STATION-STRING-DATE-400 (2026-09-06, issue [#400](https://github.com/cavazquez/openttdrs/issues/400)):
`Station` conserva `BaseStation::string_id` y la fecha absoluta de
construcción. `StationScope` expone `0x84` y `0xFA`, con la conversión nativa
de fecha relativa y saturación a WORD; `STNN.base` los lee y escribe en filas
modernas y legacy, y la importación SAV los hidrata. Las estaciones creadas
por tren, carretera, waypoint, muelle, boya y aeropuerto asignan la fecha del
calendario actual; rename conserva la plantilla fallback de OpenTTD. El
round-trip writer→parser, JSON y callbacks tienen regresiones específicas.
La resolución de texto por idioma/town/company, callbacks de construcción sin
estación y scopes restantes de `BaseStation`/aeropuerto siguen pendientes;
#329 permanece abierto.

Actualización #329-STATION-DEPRECATED-CARGO-401 (2026-09-06, issue [#401](https://github.com/cavazquez/openttdrs/issues/401)):
los resolvers legacy y map-aware de estación materializan la familia nativa
`0x8C..0xEC`, con ocho subvariables por cada una de las doce ranuras: total,
aceptación, espera, rating, primera estación, tránsito, velocidad y edad.
Se preservan los sentinels y el primer `StationID` sólo cuando el packet y el
pool importado permiten demostrar el origen. La regresión compara ambos
contextos con datos de carbón no nulos; cargos custom/CTT, textos, sonidos y
los scopes restantes de `BaseStation` siguen pendientes en #329.

Actualización #329-STATION-BADGES-402 (2026-09-06, issue [#402](https://github.com/cavazquez/openttdrs/issues/402)):
Station Action0 prop `0x1F` ya lee `ReadBadgeList` (índices WORD), y
`apply_newgrf_stations` resuelve esos índices mediante la Badge Translation
Table `GlobalVar 0x18` y el catálogo global, conservando `u16::MAX` para
labels no resolubles. `StationSpecDef` guarda las asociaciones y la tabla
local de runtime; los contextos catalog-aware del renderer exponen
`0x7A(parameter)` como `1`/`0`/`UINT_MAX`, igual que `GetBadgeVariableResult`.
La variante legacy `action2_eval_ctx_from_station_with_spec` y su callback de
disponibilidad equivalente conservan la misma respuesta sin tesela cuando el
caller aporta la spec. La API histórica que sólo recibe `Station` no conoce la
spec ni la tabla del GRF y mantiene el fallback sin badge; scopes completos de
`BaseStation`, strings y sonidos siguen pendientes en #329.

Actualización #329-COMPANY-INFO-403 (2026-09-06, issue [#403](https://github.com/cavazquez/openttdrs/issues/403)):
la codificación de `GetCompanyInfo` ya es compartida por `StationScope 0x43`
y `RoadStopScope 0x47`: conserva el id base, el bit de IA y los canales
primario/secundario de la librea por defecto. Los contextos de estación que
conservan el mundo reciben el pool de compañías y resuelven colores distintos;
las rutas legacy sin pool mantienen el fallback explícito. Las regresiones
cubren compañía IA, canales de librea distintos, ausencia de pool y ambos
scopes. Los scopes completos de `BaseStation`, strings y sonidos siguen
pendientes en #329.

Actualización #329-STATION-AVAILABILITY-PURCHASE-404 (2026-09-06, issue [#404](https://github.com/cavazquez/openttdrs/issues/404)):
la disponibilidad de estación en construcción ya ejecuta `CBID 0x13` con el
scope sin estación de OpenTTD: sentinelas de plataformas/posición, terreno y
PBS de compra, `GetCompanyInfo`, badges y fecha relativa. El preflight de
`PlaceRailStation`/`PlaceRailStationArea` pasa la compañía activa, su pool y el
calendario, antes de modificar el mapa; la API legacy mantiene un fallback
determinista y los registros `7C` de compra no se persisten porque aún no hay
una entidad. Vecinos, strings, sonidos y scopes completos de `BaseStation`/
aeropuerto siguen pendientes en #329.

Actualización #329-STATION-CARGO-GUARDS-405 (2026-09-06, issue [#405](https://github.com/cavazquez/openttdrs/issues/405)):
las variables Station `0x61` y `0x63` respetan las guardas nativas de
`GoodsEntry`: el contador de espera queda en cero hasta el primer intento de
carga y los períodos de tránsito sólo se leen cuando hay data/packets. La
familia `0x64` y los slots deprecated mantienen sus contratos propios. La
regresión cubre estación sin vehículo/datos y una cola con tránsito; scopes,
strings, sonidos y cargos no representables permanecen parciales en #329.

Actualización #329-ROADSTOP-AVAILABILITY-406 (2026-09-06, issue [#406](https://github.com/cavazquez/openttdrs/issues/406)):
el callback `CBID_STATION_AVAILABILITY` de `RoadStop` ya recibe el scope nulo
de compra de OpenTTD: vista/tipo, road/tram traducido, `TownEdge << 16`,
distancia/frame cero, `GetCompanyInfo` con IA y libreas de la compañía activa y
el bit `0x50=1<<4` que marca el picker sin tesela. `PlaceBusStop` y
`PlaceTruckStop` pasan el pool real antes de mutar el mapa; el wrapper legacy
mantiene un fallback determinista. La regresión cubre `0x47`, `0x50`, `0x45`,
`0x46` y `0x49`. No se persiste `7C` porque aún no existe una entidad en el
selector; vecinos, terreno real, cargas, strings y sonidos continúan en el
scope runtime de la parada colocada y el padre #329 sigue abierto.

Actualización #329-PURCHASE-DATE-407 (2026-09-06, issue [#407](https://github.com/cavazquez/openttdrs/issues/407)):
la frontera entre el reloj del core y el calendario NewGRF quedó corregida:
`CalendarTimer.date` es relativo al año base y los preflights de estaciones y
road stops ahora pasan `DAYS_TILL_ORIGINAL_BASE_YEAR + date` al resolver. Así
`0xFA` devuelve la fecha relativa correcta (saturada a WORD) durante la compra,
no cero por underflow saturado. Las regresiones ejecutan `PlaceRailStationArea`
y `PlaceBusStop` con un callback que lee `0xFA`; ambas construcciones sólo
pueden pasar si reciben el día absoluto. El padre #329 sigue abierto.

Actualización #329-ROADSTOP-BUILD-DATE-408 (2026-09-06, issue [#408](https://github.com/cavazquez/openttdrs/issues/408)):
el scope Action2 de una parada ya colocada expone `0xFA` con la fecha de
construcción de `BaseStation`, restada y saturada a WORD mediante el helper
compartido de `Station`. La regresión fija `build_date = base + 123` y valida
`0xFA=123` en la ruta legacy que también alimenta render/animación; el picker
sin entidad conserva su fecha actual en #406/#407. El padre #329 sigue abierto.

Actualización #329-ROADSTOP-PURCHASE-FACILITIES-409 (2026-09-06, issue [#409](https://github.com/cavazquez/openttdrs/issues/409)):
el scope nulo de compra RoadStop publica `0xF0=0`, el sentinel de
`StationFacilities` que devuelve OpenTTD antes de que exista una estación.
La ruta de una parada colocada conserva su máscara derivada de `StopKind`; la
regresión del callback de compra comprueba explícitamente el cero sin crear
una entidad temporal. El padre #329 sigue abierto.

Actualización #329-ROADSTOP-BADGES-410 (2026-09-06, issue [#410](https://github.com/cavazquez/openttdrs/issues/410)):
RoadStops ya parsea la propiedad Action0 nativa `0x16` (`ReadBadgeList`) y
salta las listas bridgeable `0x13`/`0x14` antes de continuar con propiedades
posteriores. `apply_newgrf_roadstops` traduce los índices mediante GlobalVar
`0x18`, conserva `associated_badges` y la tabla local `0x7A`, y mantiene el
fallback auxiliar `0xFD` por etiqueta. El renderer/map-aware y el callback de
compra devuelven `1`/`0`/`UINT_MAX` para badge asociado, conocido no asociado o
índice local desconocido. Las regresiones cubren parseo, aplicación, parada
colocada y picker sin entidad; el padre #329 sigue abierto.

Actualización #329-ROADSTOP-TOWN-PARENT-411 (2026-09-06, issue [#411](https://github.com/cavazquez/openttdrs/issues/411)):
la ruta map-aware de `RoadStopScopeResolver` ya materializa el
`TownScopeResolver` parent seleccionando el pueblo más cercano con desempate
por ID, y copia las variables de pueblo modeladas junto con el PSA `7C` del
GRFID de la parada. Las APIs legacy sin pool de pueblos mantienen el parent
vacío. La regresión cubre población/flags y un registro persistente; la
asociación nativa parada→pueblo y variables no representadas siguen pendientes
en #329.

Actualización #329-STATION-TOWN-PARENT-412 (2026-09-06, issue [#412](https://github.com/cavazquez/openttdrs/issues/412)):
`StationResolverObject::GetScope(VSG_SCOPE_PARENT)` ya se materializa en las
rutas catalog-aware. El contexto recibe `GameState::towns`, selecciona el
pueblo más cercano con desempate por ID, copia las variables `TownScope` que
el modelo conserva y carga el PSA `7C` de la `StationSpec` por GRFID. Renderer,
construcción y wrappers de animación/scheduler CB140–142 con pools de mundo
usan este parent; las APIs legacy mantienen el fallback vacío. La asociación
nativa estación→pueblo y variables no representadas continúan pendientes en
#329.

Actualización #328-LINKGRAPH-068 (2026-09-06, issue [#394](https://github.com/cavazquez/openttdrs/issues/394)):
`PATS.linkgraph.recalc_time` ya no es sólo un byte conservado. El scheduler
clona estaciones/grafo/catálogo en el spawn, calcula la fecha de integración en
segundos económicos y mantiene los flows previos hasta la primera marca de
`JoinNext` posterior a ese vencimiento. La regresión también demuestra que una
mutación del grafo posterior no altera el job en vuelo. La rehidratación
ejecutable de `LGRJ`/`LGRS` se documenta en el corte siguiente (#395); threads,
presupuesto de CPU, pausa multiplayer, compresión y topología dinámica siguen
pendientes en #328; el padre no se cierra.

Actualización #329-STATION-CB13-MAP-AWARE-413 (2026-09-06, issue [#413](https://github.com/cavazquez/openttdrs/issues/413)):
CB13 de una estación colocada ya dispone de
`apply_station_availability_callback_at_with_catalog_and_world`, que reutiliza
el contexto del renderer con catálogo y pools de mundo. El runtime puede leer
vecinos `0x66`/`0x67`/`0x68`/`0x6A`/`0x6B`, badges `0x7A`, parent TownScope y
PSA `7C`; el writeback se ejecuta después del callback y un tile obsoleto
conserva el fallback legacy sin limpiar storage. La regresión cubre identidad
de vecino empaquetada y persistencia. Strings, sonidos, scopes completos de
`BaseStation` y el vínculo nativo estación→pueblo continúan pendientes en
#329.

Actualización #328-LINKGRAPH-069 (2026-09-06, issue [#395](https://github.com/cavazquez/openttdrs/issues/395)):
`LGRJ`/`LGRS` ahora se decodifican durante la importación SAV: el snapshot de
cada job (settings, cargo, nodos, aristas y `join_date`) se transforma en un
`cargodist::parity::Job`, y `LGRS.running` restaura el orden de `JoinNext`.
Coordenadas y destinos imposibles se descartan sin abortar el save. El
passthrough nativo se conserva hasta integrar o mutar el grafo, momento en que
se invalida para no exportar jobs obsoletos. Threads, presupuesto de CPU,
compresión/merge, pausa multiplayer y el planificador completo de `schedule`
siguen pendientes; el padre #328 continúa abierto.

Actualización #329-ROADSTOP-COST-MULTIPLIERS-414 (2026-09-06, issue [#414](https://github.com/cavazquez/openttdrs/issues/414)):
RoadStop Action0 `0x15` ya conserva los multiplicadores de construcción y
limpieza (default `16`) y los aplica con las categorías de precio de bus/camión
de OpenTTD y shift `-4`. `PlaceBusStop`/`PlaceTruckStop` y `ClearTile` para
paradas custom cobran el valor de la spec; el fallback sin spec no cambia. Las
regresiones cubren parseo, catálogo y construir/limpiar; scopes completos de
`BaseStation`, listas bridgeables, strings y sonidos siguen pendientes en
#329.

Actualización #329-ROADSTOP-BRIDGEABLE-415 (2026-09-06, issue [#415](https://github.com/cavazquez/openttdrs/issues/415)):
RoadStop Action0 `0x13`/`0x14` ya se consume como lista `ExtendedByte` y se
conserva por layout (seis entradas: cuatro bahías y dos drive-through). El
catálogo, la aplicación NewGRF y JSON mantienen altura mínima y pilares
prohibidos; las entradas posteriores siguen alineadas incluso cuando un GRF
declara más de seis layouts. Las regresiones cubren parseo, truncamiento,
aplicación y round-trip. La comprobación nativa de altura al tender un puente
se conectó en #416; la máscara de pilares queda como consumidor visual
pendiente. El issue padre #329 permanece abierto.

Actualización #329-ROADSTOP-BRIDGE-CLEARANCE-416 (2026-09-06, issue [#416](https://github.com/cavazquez/openttdrs/issues/416)):
`check_bridge_with_stations` recorre las teselas intermedias del puente en el
preview y en el execute. Cuando encuentra una parada bus/camión custom,
resuelve su layout desde `m5` y compara `GetTileMaxZ + min_height` con la
altura del tablero; `min_height=0` o un tablero bajo devuelve
`BridgeTooLowForRoadStop` sin mutar el mapa. La regresión cubre rechazo,
aceptación en la altura mínima y rollback; vanilla/saves sin spec conservan el
fallback. El consumidor visual de `disallowed_pillars` quedó implementado en
#417; la persistencia de flags propios de puentes NewGRF y los demás scopes
mantienen abierto #329.

Actualización #329-ROADSTOP-BRIDGE-PILLARS-417 (2026-09-06, issue [#417](https://github.com/cavazquez/openttdrs/issues/417)):
El renderer mundial de puentes recibe estaciones y catálogo de road stops y
resuelve la spec por tesela/layout. `disallowed_pillars` se cruza con las
tablas vanilla de pilares `ALL_PILLARS`, suspensión y cantilever por pieza y
eje; una intersección omite el bloque de pilares de esa tesela, igual que
`DrawBridgeMiddle`, sin afectar tablero, barandas ni catenaria. El fallback de
paradas vanilla y los wrappers sin catálogo siguen estables. Se agregaron
regresiones para las tablas de máscaras y selección custom/vanilla; queda
pendiente transportar los flags de pilares de `BridgeSpec` NewGRF y validar
goldens de captura antes de cerrar el parent #329. La persistencia de esa
tabla queda documentada en #418.

Actualización #329-BRIDGE-PILLAR-FLAGS-418 (2026-09-06, issue [#418](https://github.com/cavazquez/openttdrs/issues/418)):
Action0 `Bridges` propiedad `0x15` ya se conserva como seis piezas centrales
por dos ejes (`BridgePillarFlagsTable`). El parser lee `ExtendedByte`, guarda
los dos bytes de cada entrada, consume también las entradas posteriores al
límite de seis para no desalinear la propiedad siguiente y tolera payloads
truncados sin panic. La aplicación NewGRF copia la tabla al `BridgeSpecDef`,
marca `has_custom_pillar_flags` incluso para una máscara cero y el catálogo
JSON la rehidrata con defaults compatibles con saves anteriores. Las
regresiones cubren parseo, truncamiento, aplicación y round-trip. El issue
quedó cerrado; la conexión visual se completó en #419.

Actualización #329-BRIDGE-PILLAR-RENDER-419 (2026-09-06, issue [#419](https://github.com/cavazquez/openttdrs/issues/419)):
El pase mundial y el compositor de objetos ya transportan
`bridge_spec_catalog` hasta `DrawBridgeMiddle`. Para cada `BridgeType`, pieza
central y eje, el renderer usa `BridgeSpecDef.pillar_flags` sólo cuando
`has_custom_pillar_flags` está activo; una tabla publicada con cero conserva
ese cero explícito y los saves/catálogos vanilla siguen usando las máscaras
de OpenTTD. La intersección con `RoadStopSpecDef.disallowed_pillars` consulta
la misma máscara, de modo que una parada custom bloquea únicamente el pilar
que realmente dibujaría el puente. Las regresiones cubren override custom,
override cero y fallback vanilla; la captura dorada contra OpenTTD queda como
validación pendiente del cierre raster global.

Actualización #329-STATION-TOWN-PARENT-420 (2026-09-06, issue [#420](https://github.com/cavazquez/openttdrs/issues/420)):
`Station` conserva ahora `town_id` opcional y el importador SAV lo hidrata
desde `BaseStation::town`. El parent `TownScopeResolver` de estaciones
catalog-aware usa ese ID nativo para copiar variables y registros `7C` por
GRFID; sólo si la referencia falta o no está en el pool aplica el fallback
determinista por distancia Manhattan/ID. JSON antiguo sigue cargando con
`None`. Las regresiones cubren dos pueblos donde el más cercano difiere del
enlace nativo y el round-trip/default legacy; quedan fuera las estaciones
nuevas sin asociación nativa persistida y las variables de TownScope todavía
no modeladas.

Actualización #329-ROADSTOP-TOWN-PARENT-421 (2026-09-06, issue [#421](https://github.com/cavazquez/openttdrs/issues/421)):
RoadStopScope reutiliza la asociación `Station::town_id` para seleccionar el
pueblo nativo en `0x45`/`0x46`, copiar el parent `TownScopeResolver` y cargar
el PSA `7C` por GRFID. Los road stops importados desde SAV dejan de depender
del pueblo geométricamente más cercano; IDs ausentes o inválidos y APIs legacy
siguen usando el fallback Manhattan/ID. La regresión cubre vars, población y
writeback del PSA con dos pueblos divergentes.

Actualización #329-OBJECT-ANIMATION-METADATA-422 (2026-09-06, issue [#422](https://github.com/cavazquez/openttdrs/issues/422)):
Action0 `Objects` ya conserva los cuatro campos que habilitan el contrato de
animación de OpenTTD: flags `0x10` (incluidos `Animation` y `AnimRandomBits`),
frames/estado `0x11`, velocidad `0x12` y triggers `0x13`. El parser tolera
payloads antiguos que no traen esas propiedades y usa `NoAnimation`/velocidad
`2` como defaults nativos; `ObjectSpecDef`, el catálogo aplicado y JSON
mantienen los valores sin perderlos al rehidratar un save. La regresión carga
un GRF sintético, comprueba flags, frames/status, velocidad y máscara de
triggers y valida el round-trip del spec. Esta etapa no declara ejecución:
CB158/CB15A, triggers por tick y estado `m3hi` por tesela siguen siendo el
próximo recorte de #329.

Actualización #329-OBJECT-ANIMATION-RUNTIME-423 (2026-09-06, issue [#423](https://github.com/cavazquez/openttdrs/issues/423)):
el scheduler de objetos ya se integra en `AnimateTile_Object` y conserva una
lista de teselas activas más el estado de siembra inicial en `GameState`/JSON.
Para cada tesela de una huella `OBJS`, CB15A selecciona la cadencia `2^speed`
(acotada a `0..16`) y CB158 selecciona el frame; `CALLBACK_FAILED` mantiene el
avance Action0, `0xFE` delega en ese avance y `0xFF` elimina la tesela de la
lista sin reiniciarla automáticamente. El frame se escribe en `m3hi`, los
callbacks reciben el scope real de objeto (`0x40`–`0x64`, pueblo parent y
vecinos solicitados) y los registros `7C` del pueblo se reemiten por GRFID. La
regresión cubre cadencia/frame, detención `0xFF` y round-trip JSON. CB159/triggers,
callbacks colour/autoslope/fund text, writeback propio de la tesela y layouts
16-bit siguen fuera de esta etapa.

Actualización #329-OBJECT-ANIMATION-TRIGGERS-424 (2026-09-06, issue [#424](https://github.com/cavazquez/openttdrs/issues/424)):
se añadió `CBID_OBJECT_ANIMATION_TRIGGER` (`0x159`) y el enum de triggers
`Built`, `TileLoop` y `TileLoopNorth`, con las mismas máscaras de Action0 que
OpenTTD. La construcción registra las teselas animadas y ejecuta el callback
para toda la huella; `AnimateTile_Object` consume `TileLoop` por tesela y el
origen consume `TileLoopNorth` para toda la huella. `param2` lleva el ordinal,
los resultados `0xFD`/`0xFE`/`0xFF`/frame actualizan la lista y `m3hi`, y el
fallback `CALLBACK_FAILED` no muta el estado. La regresión sintética cubre un
trigger `Built` que fija un frame; quedan triggers de carga/economía no
representados y callbacks colour/autoslope/fund text.

Actualización #329-OBJECT-COLOUR-425 (2026-09-06, issue [#425](https://github.com/cavazquez/openttdrs/issues/425)):
`CBID_OBJECT_COLOUR` (`0x15B`) y el bit `Colour` de Action0 ya se exponen en
el catálogo. `BuildObject` evalúa el callback después de materializar la
instancia, pasa el color inicial como `param1`, reutiliza el scope real de
Object/Town parent y persiste un resultado válido de 8 bits en `OBJS`/JSON/SAV.
`CALLBACK_FAILED` y valores `>=0x100` conservan el color inicial; la regresión
sintética verifica la selección del color. 2CC/livery avanzada, callbacks
fund text y el resto de scopes de Objects siguen fuera de esta etapa; CB15D
autoslope se cerró en el issue #427.

Actualización #328-SAV-NESTED-426 (2026-09-06, issue [#426](https://github.com/cavazquez/openttdrs/issues/426)):
el passthrough genérico de `CH_TABLE`/`CH_SPARSE_TABLE` compara ahora los
descriptores de structs de forma recursiva. Cuando la cantidad de elementos se
mantiene, una mutación de scalar o lista escalar anidada sustituye sólo el
subcampo conocido y conserva las columnas hermanas desconocidas dentro de cada
elemento. Las listas de structs que crecen siguen usando la fusión anterior
cuando el schema es idéntico; si además cambia el schema anidado, la topología,
las filas o los índices, el writer vuelve al camino canónico. Las regresiones
sintética y los casos nativos de `CITY`/`INDY` pasan con 2150 tests de core;
quedan pendientes pools no modelados y mutaciones estructurales con columnas
desconocidas dentro de elementos nuevos.

Actualización #329-OBJECT-AUTOSLOPE-427 (2026-09-06, issue [#427](https://github.com/cavazquez/openttdrs/issues/427)):
CB15D (`0x15D`) ya se consulta desde `raise_land`, `lower_land` y `level_land`
cuando la operación toca una huella `OBJS` existente. El preflight conserva las
guardas upstream de pendiente no empinada y `TileMaxZ`, hidrata el objeto y su
`TownScopeResolver` y mantiene el writeback parent aislado hasta el execute.
`CALLBACK_FAILED`/cero permiten; un resultado booleano no nulo, una instancia
no resoluble o una topología no soportada rechazan sin mutar. Las regresiones
cubren la semántica booleana y la atomicidad; el texto Action4 se aborda en
#428, mientras siguen pendientes el writeback `7C` propio de objeto y
scopes/vecinos avanzados.

Actualización #329-OBJECT-FUND-MORE-TEXT-428 (2026-09-06, issue [#428](https://github.com/cavazquez/openttdrs/issues/428)):
CB15C (`0x15C`) ya se evalúa al abrir el ObjectPicker con la vista elegida y
sin crear una instancia. `CALLBACK_FAILED`/`0x400` no agregan texto, los
resultados `0..0x3FF` se clasifican como texto local y `0x40F` recupera el
`StringID` desde el registro `0x100` del text stack; respuestas fuera del
contrato quedan como `Invalid`. El selector muestra un diagnóstico explícito
en vez de ocultar un callback válido. El parseo Action4 y la traducción por
idioma de esas cadenas siguen separados del cierre de este issue.

Actualización #329-ACTION4-GENERIC-STRINGS-429 (2026-09-06, issue [#429](https://github.com/cavazquez/openttdrs/issues/429)):
Action4 genérico (`0x80`) ya se recorre desde pseudo-sprites v1/v2. El núcleo
conserva `(GRFID, StringID, idioma)` en un catálogo efímero y aplica el
fallback idioma solicitado → genérico `0x7F` → inglés → última variante declarada. CB15C local
resuelve `0xD000 + offset` y CB15C `0x40F` consulta el registro `0x100`; el
ObjectPicker muestra la cadena cuando existe y `Action4 ausente` cuando no.
Payloads truncados no agregan textos parciales. Los códigos de control y los
Action4 específicos por feature siguen siendo sucesores separados.

Actualización #329-ACTION13-TRANSLATIONS-430 (2026-09-06, issue [#430](https://github.com/cavazquez/openttdrs/issues/430)):
Action13 (`TranslateGRFStrings`) ya se procesa después de los Action4 base.
El parser valida GRFID activo, versión del GRF, idioma explícito de v8+ o
`0x7F` genérico para v7 y anteriores, rangos `0xD000..0xD3FF`/
`0xD800..0xFFFF` y terminadores NUL. Las traducciones válidas ganan en el
lookup del catálogo; cargas desconocidas, fuera de rango y truncadas se
ignoran sin mutar el estado. Códigos de control y mappings específicos siguen
fuera del recorte.

Actualización #329-VEHICLE-CB31-ERROR-431 (2026-09-06, issue [#431](https://github.com/cavazquez/openttdrs/issues/431)):
CB31 (`CBID_VEHICLE_START_STOP_CHECK`) conserva ahora el motivo que devuelve
OpenTTD: `0..0x3FF` apunta a `0xD000 + resultado`, `0x40F` recupera
`regs100[0]`, `0x400`/`CALLBACK_FAILED` permiten y GRF v7 mantiene el permiso
especial `0xFF`. La API booleana de comandos sigue compatible; localización,
controles de texto y serialización del error permanecen explícitamente como
sucesores.

Actualización #329-NEWGRF-TEXT-CONTROLS-432 (2026-09-06, issue [#432](https://github.com/cavazquez/openttdrs/issues/432)):
Action4/Action13 pasan por `decode_newgrf_text`: espacios codificados, saltos,
fuentes, referencias inline, controles extendidos y glifos frecuentes dejan de
aparecer como bytes crudos. Parámetros dinámicos, choice-lists y
gender/case/plural se representan con marcadores `⟦...⟧` hasta disponer del
text stack y los mappings de idioma completos.

Actualización #329-NEWGRF-TEXT-STACK-433 (2026-09-06, issue [#433](https://github.com/cavazquez/openttdrs/issues/433)):
`NewGrfStringCatalog::lookup_expanded` resuelve referencias inline `0x81` con
fallback de locale, IDs locales en el rango genérico, expansión anidada limitada
y corte de ciclos. El selector de objetos muestra ahora el texto expandido;
parámetros dinámicos y consumidores de otros features permanecen como trabajo
posterior.

Actualización #329-NEWGRF-TEXT-PARAMS-442 (2026-09-06, issue [#442](https://github.com/cavazquez/openttdrs/issues/442)):
el catálogo añade `NewGrfTextContext`, `NewGrfTextValue` y
`lookup_rendered`: los parámetros signed/unsigned/hex/string se consumen en un
orden explícito y las choice-lists simples eligen la rama solicitada o default.
El HUD de rechazos y el picker de objetos usan el renderer con contexto vacío,
por lo que ya no muestran los marcadores de choice-list. El contexto no muta,
los payloads incompletos se conservan literalmente y fecha, género/case,
pluralización basada en idioma y parámetros de otros features siguen siendo
sucesores pendientes.

Actualización #329-STATION-ANIMATION-SOUNDS-443 (2026-09-06, issue [#443](https://github.com/cavazquez/openttdrs/issues/443)):
los resultados de CB140, CB141 y CB142 ya separan el byte visual de los bits
8..14 del sonido. Las rutas de construcción, carga, aceptación, reservas y
vehículos capturan `(GRFID, local_id)`; el scheduler de `TileLoop` conserva
además los sonidos de velocidad y siguiente frame. Los call sites con
`GameState` resuelven el par contra `sound_effect_catalog` y encolan
`PendingNewgrfSound`, manteniendo silencioso un sample ausente y sin cambiar
las APIs legacy de teselas dirty. Quedan fuera los scopes completos de
`BaseStation`/aeropuerto y el callback genérico de sonidos ambientales.

Actualización #329-NEWGRF-TEXT-DATES-444 (2026-09-06, issue [#444](https://github.com/cavazquez/openttdrs/issues/444)):
los controles de fecha WORD/DWORD (`0x82`/`0x83`/`0x9A16`/`0x9A17`) ya
consumen `NewGrfTextValue::Date` —o un entero de día explícito— y producen
formato largo, corto o ISO determinista. `0x84` conserva su semántica NewGRF
de velocidad WORD; `date-iso` sólo es un marcador explícito del renderer. El
decoder distingue las fechas de power, volumen, peso y cargos, que conservan
sus propios marcadores. Fecha ausente, negativa o textual permanece visible
sin inventar un valor. El formato localizado y la conversión de epochs NewGRF
fuera del calendario 1950, además de género/case/plural, siguen pendientes.

Actualización #329-NEWGRF-TEXT-PLURALS-445 (2026-09-06, issue [#445](https://github.com/cavazquez/openttdrs/issues/445)):
`0x9A15` conserva ahora regla y offset en `plural-list:<regla>:<offset>`.
`NewGrfTextContext` puede entregar una regla explícita y el renderer replica
las quince ramas `DeterminePluralForm` de OpenTTD 15.3 para signed/unsigned.
Los casos sin parámetro, texto o regla válida conservan el marcador; el
catálogo automático por locale y gender/case quedan como sucesores.

Actualización #329-NEWGRF-TEXT-GENDER-CASE-446 (2026-09-06, issue [#446](https://github.com/cavazquez/openttdrs/issues/446)):
los metadatos `gender`/`case` dejan de dibujarse, `gender-list:<offset>` puede
leer `⟦gender:N⟧` desde el parámetro textual y `case-list` acepta un índice
explícito del contexto. Contexto o parámetro ausente conserva la lista; las
tablas automáticas por locale y los scopes implícitos de OpenTTD siguen
pendientes.

Actualización #329-VEHICLE-CB31-FEEDBACK-434 (2026-09-06, issue [#434](https://github.com/cavazquez/openttdrs/issues/434)):
el rechazo de CB31 conserva un diagnóstico efímero por vehículo/GRFID y los
botones de start/stop resuelven `LocalString`/`GrfString` con el catálogo
expandido y el locale activo. El fallback genérico, la no persistencia y la
ausencia de una segunda ejecución del callback quedan cubiertos por pruebas.

Actualización #329-VEHICLE-CB31-GROUP-435 (2026-09-06, issue [#435](https://github.com/cavazquez/openttdrs/issues/435)):
el comando de arranque/parada de grupos hace ahora un preflight completo de sus
vehículos: valida la espera de horario y CB31 antes de cambiar cualquier estado.
Una denegación es atómica, conserva el diagnóstico de la unidad responsable y
la lista de vehículos lo entrega al feedback textual de #434. La regresión
combina una unidad vanilla y una NewGRF para demostrar que no queda un grupo
parcialmente iniciado; autoreemplazo y órdenes de depot siguen siendo
sucesores del padre #329.

Actualización #329-STATION-CB149-ERROR-437 (2026-09-06, issue [#437](https://github.com/cavazquez/openttdrs/issues/437)):
CB149 de estaciones ferroviarias conserva ahora el resultado de ubicación de
OpenTTD 15.3: `FAILED`/`0x400` permiten, `0..0x3FF` se convierten a texto
genérico NewGRF, `0x40F` consulta `regs100[0]` y los códigos estándar quedan
como rechazo genérico; GRF <8 invierte el bit 10. El preflight de toda la
huella guarda un diagnóstico efímero y la UI resuelve Action4/Action13 con el
locale activo sin reejecutar el callback. La regresión fija parámetros,
atomicidad y consumo del diagnóstico; scopes completos, vecinos y parámetros
dinámicos de text stack siguen abiertos.

Actualización #329-STATION-AVAILABILITY-TEXT-436 (2026-09-06, issue [#436](https://github.com/cavazquez/openttdrs/issues/436)):
investigación del código upstream 15.3 (`Convert8bitBooleanCallback`) confirmó
que CB13 de estaciones/road stops sólo devuelve booleano y no tiene StringID de
error. El issue textual se cerró como no aplicable con referencias a
`station_cmd.cpp` y `newgrf_commons.cpp`; no se declara una capacidad que
OpenTTD no ofrece.

Actualización #329-STATION-CB149-STANDARD-ERRORS-438 (2026-09-06, issue [#438](https://github.com/cavazquez/openttdrs/issues/438)):
el HUD traduce los códigos estándar `0x402..0x408` de CB149 a los siete
mensajes de clima/agua de OpenTTD en español e inglés. `0x401` y resultados
desconocidos conservan el fallback genérico; la tabla sólo afecta al feedback y
no altera el preflight ni la semántica del callback.

Actualización #329-STATION-CB149-LAND-SCOPE-439 (2026-09-06, issue [#439](https://github.com/cavazquez/openttdrs/issues/439)):
el preflight map-aware de CB149 materializa ahora `StationScopeResolver::0x67`
desde la tesela real antes de crear la estación. Se respetan offsets firmados
en nibbles, intercambio X/Y para el eje Y, wrap toroidal, clase de agua,
terreno, tipo de tesela y la escala de altura de GRF <8 frente a GRF >=8. La
regresión cubre un vecino canalizado y la ruta de construcción de una estación;
la variante legacy sin mapa conserva su contrato. Scopes adicionales de
`BaseStation`, vecinos no-terreno y parámetros dinámicos del text stack siguen
siendo sucesores del padre #329.

Actualización #329-OBJECT-CB157-ERRORS-440 (2026-09-06, issue [#440](https://github.com/cavazquez/openttdrs/issues/440)):
CB157 de objetos conserva ahora el motivo exacto de rechazo de OpenTTD 15.3:
textos locales, `regs100[0]` para `0x40F`, códigos genéricos e inversión de bit
10 para GRF<8. El preflight map-aware mantiene el writeback PSA del pueblo y
guarda un diagnóstico efímero sin mutar el mapa; el HUD lo consume una sola vez
con catálogo expandido y locale activo, incluyendo los códigos estándar de
clima/agua. Scopes propios de instancia/tesela y el fallback completo de
pendiente siguen siendo sucesores del padre.

Actualización #329-STATION-CB149-PURCHASE-SCOPE-441 (2026-09-06, issue [#441](https://github.com/cavazquez/openttdrs/issues/441)):
CB149 map-aware materializa ahora el scope de compra real: sentinelas de
plataforma/posición, estado PBS, `GetCompanyInfo` de la compañía activa,
badges y fecha relativa. El comando pasa compañía, pool, color y calendario al
resolver para que preview y execute seleccionen la misma rama Action2. Las
variables de una estación ya creada y vecinos continúan en los scopes de
render/animación del parent #329.

Actualización #331-SIGNAL-PICKER-LOCALE-465 (2026-09-07, issue [#465](https://github.com/cavazquez/openttdrs/issues/465)):
el selector de señales ferroviarias usa ahora claves españolas catalogadas
para bloque, entrada, salida, combinada, ruta PBS, ruta unidireccional y los
dos estilos de señal. El título dinámico localiza también tipo, variante y
densidad sin alterar IDs, ciclo ni encoding. La regresión cubre ambos locales
y confirma que la selección permanece intacta; #331 sigue abierto por las
superficies UI y catálogos upstream restantes.

Actualización #331-NEWS-SETTINGS-MODES-466 (2026-09-07, issue [#466](https://github.com/cavazquez/openttdrs/issues/466)):
la configuración de Noticias usa ahora las fuentes `Silencio`, `Resumen` y
`Completo`, que alternan a `Off`, `Summary` y `Full` en inglés. También se
localiza la explicación de los modos; el enum, los defaults y las ocho
preferencias no cambian. #331 sigue abierto por las superficies y catálogos
upstream restantes.

Actualización #331-BUY-VEHICLE-CHROME-467 (2026-09-07, issue [#467](https://github.com/cavazquez/openttdrs/issues/467)):
la ventana de compra de vehículos localiza sus títulos de depósito, controles
de ordenar y filtros. El título dinámico recibe el locale sin cambiar la clase
de depósito; nombres de motores, cargos y estadísticas permanecen datos
literales. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-ROAD-STOP-PICKER-468 (2026-09-07, issue [#468](https://github.com/cavazquez/openttdrs/issues/468)):
el selector de paradas viales localiza título, secciones y prefijo de tipo en
vivo. Los labels NewGRF de clase/especificación permanecen literales; IDs,
visibilidad y comandos de selección no cambian. #331 sigue abierto por los
catálogos upstream y superficies UI restantes.

Actualización #331-RAIL-STATION-PICKER-469 (2026-09-07, issue [#469](https://github.com/cavazquez/openttdrs/issues/469)):
el selector de estación ferroviaria localiza título, controles de andenes,
cobertura y prefijos dinámicos `Accepts`/`Supplies`. El sentinel vacío se
traduce, pero cargos y labels de clase/spec permanecen literales; huella,
orientación, filtros e IDs no cambian. #331 sigue abierto por los catálogos
upstream y superficies UI restantes.

Actualización #331-AIRPORT-PICKER-470 (2026-09-07, issue [#470](https://github.com/cavazquez/openttdrs/issues/470)):
el selector de aeropuerto localiza título, ejes, botones de cobertura y los
resúmenes dinámicos de tamaño/cobertura. El nombre de spec, la huella, el
radio, los contadores y los IDs permanecen literales o calculados por el core;
la regresión confirma ambos locales. #331 sigue abierto por los catálogos
upstream y superficies UI restantes.

Actualización #331-DESTINATION-PICKER-471 (2026-09-07, issue [#471](https://github.com/cavazquez/openttdrs/issues/471)):
el selector de destinos localiza título, ayuda, selección en mapa y fallbacks
vanilla de filas dinámicas. Nombres personalizados/NewGRF, coordenadas, orden
de candidatos e IDs permanecen intactos; la regresión cubre locales y valores
con signo. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-OBJECT-PICKER-472 (2026-09-07, issue [#472](https://github.com/cavazquez/openttdrs/issues/472)):
el selector de objetos localiza título, prefijo de selección y nombres vanilla
sin reinterpretar labels/dimensiones NewGRF ni el callback CB15C. IDs,
miniaturas y comandos no cambian; la regresión cubre ambos locales y un label
custom. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-BRIDGE-PICKER-473 (2026-09-07, issue [#473](https://github.com/cavazquez/openttdrs/issues/473)):
el selector de puentes localiza título y resumen dinámico de transporte/vano.
La longitud, restricciones, velocidades, costes, disponibilidad y nombres de
tipos permanecen datos del core; la regresión cubre carretera/vía y ambos
locales. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-DEPOT-PICKER-474 (2026-09-07, issue [#474](https://github.com/cavazquez/openttdrs/issues/474)):
el selector de tipo de depósito localiza el título reescrito por el sync y los
chips `Road`/`Rail`/`Ship`. Herramientas, IDs y comandos permanecen iguales; la
regresión cubre el título en ambos locales. #331 sigue abierto por los pickers
de construcción restantes y los catálogos upstream.

Actualización #331-CONSTRUCTION-PICKERS-475 (2026-09-07, issue [#475](https://github.com/cavazquez/openttdrs/issues/475)):
los pickers comunes de muelle, boya, waypoints, arbolado, terraformación y
cartel consultan el locale en sus títulos y ayudas. `Cartel de texto` queda
separado de la categoría de noticias `Cartel`; orientaciones, herramientas e
IDs no cambian. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-SIGN-LIST-476 (2026-09-07, issue [#476](https://github.com/cavazquez/openttdrs/issues/476)):
la lista de carteles localiza acciones, estado vacío y encabezado dinámico.
ID, coordenadas y nombres de usuario permanecen literales; centrar, renombrar,
borrar y remapear no cambian. #331 sigue abierto por los catálogos upstream y
superficies UI restantes.

Actualización #331-ROAD-RAIL-TOOLTIPS-477 (2026-09-07, issue [#477](https://github.com/cavazquez/openttdrs/issues/477)):
los tooltips RailType/RoadType y la abreviatura `Eléc` consultan el catálogo
inglés. Selectores, filtros, IDs y labels NewGRF permanecen intactos; la
regresión cubre todas las descripciones nuevas. #331 sigue abierto por los
catálogos upstream y superficies UI restantes.

Actualización #331-STATION-DIRECTORY-LOCALE-478 (2026-09-07, issue [#478](https://github.com/cavazquez/openttdrs/issues/478)):
el directorio de estaciones localiza filtros, estado vacío, tipos de parada y
métricas dinámicas de rating/espera, invalidando su caché al cambiar de locale.
Nombres personalizados, compañías, coordenadas, IDs, orden y navegación no se
traducen ni cambian; los labels NewGRF/cargos sin clave vanilla siguen
literales. #331 sigue abierto por los catálogos upstream y superficies UI
restantes.

Actualización #331-INDUSTRY-DIRECTORY-LOCALE-479 (2026-09-07, issue [#479](https://github.com/cavazquez/openttdrs/issues/479)):
el directorio de industrias localiza título, buscador, orden, fundación,
estados vacíos, nombres vanilla, cadenas I/O y botones de clima, invalidando la
caché al cambiar de locale. La búsqueda conserva también las fuentes
españolas; posiciones, stock, capacidad, IDs, acciones y labels sin clave
vanilla no cambian. #331 sigue abierto por los catálogos upstream y
superficies UI restantes.

Actualización #331-TOWN-DIRECTORY-LOCALE-480 (2026-09-07, issue [#480](https://github.com/cavazquez/openttdrs/issues/480)):
el directorio de pueblos localiza título, buscador, orden, fundación, estados
vacíos y las métricas de población/autoridad, invalidando la caché al cambiar
de locale. Nombres, valores, selección, IDs, navegación y la restricción del
editor no cambian. #331 sigue abierto por los catálogos upstream y superficies
UI restantes.

Actualización #331-INDUSTRY-PANEL-PRODUCTION-481 (2026-09-07, issue [#481](https://github.com/cavazquez/openttdrs/issues/481)):
la ficha de industria y la ventana hija de producción localizan títulos,
estados, tipos/cargos vanilla, métricas e historial, preservando GFX/NewGRF,
valores custom, foco, preview y relación parent/child. El residual de gráfica
mensual de #269 y los catálogos upstream siguen abiertos en #331.

Actualización #331-TOWN-AUTHORITY-LOCALE-482 (2026-09-07, issue [#482](https://github.com/cavazquez/openttdrs/issues/482)):
la ventana de autoridad localiza título, estado, resumen, ratings, las ocho
acciones y sus estados de disponibilidad, sin mutar costes, máscaras, nombres,
IDs, cascada ni efectos diferidos. #331 sigue abierto por los catálogos
upstream y superficies UI restantes.

Actualización #331-COMPANY-VIEW-LOCALE-483 (2026-09-07, issue [#483](https://github.com/cavazquez/openttdrs/issues/483)):
la vista de compañía localiza título, botón Finanzas y resumen de dinero,
préstamo y flota, preservando nombres, importes, conteos, IDs y navegación. El
residual de Livery/ManagerFace/Infrastructure permanece explícito; #331 sigue
abierto por ese alcance, catálogos upstream y superficies UI restantes.

Actualización #331-TOWN-WINDOW-LOCALE-484 (2026-09-07, issue [#484](https://github.com/cavazquez/openttdrs/issues/484)):
la ficha de pueblo localiza acciones, métricas, rating, financiación, demanda,
metas e historial mensual y refresca sus textos al cambiar el locale. El nombre,
población, casas, valores, IDs, comandos, cámara y vínculo con Autoridad local
permanecen intactos; la fórmula de crecimiento urbano y su semántica de
simulación siguen pendientes fuera de este sub-issue. #331 continúa abierto por
los catálogos upstream y las superficies UI restantes.

Actualización #331-GRAPH-WINDOW-LOCALE-485 (2026-09-07, issue [#485](https://github.com/cavazquez/openttdrs/issues/485)):
las ventanas de ingresos, beneficio operativo, valor de compañía y rendimiento
localizan títulos, períodos y hints (incluido el estado vacío), y cada ventana
actualiza su propio hint. Series, barras, importes, nombres, IDs y selección de
compañía permanecen intactos; ejes, tooltips por barra y filtro manual siguen
residuales en #331/#271.

Actualización #331-FINANCES-WINDOW-LOCALE-486 (2026-09-07, issue [#486](https://github.com/cavazquez/openttdrs/issues/486)):
la ventana de finanzas localiza botones, resumen financiero, infraestructura y
bloque de compañías, invalidando la caché sólo cuando cambia el locale o los
datos. Nombres, importes, conteos, colores, IDs y comandos permanecen intactos;
el detalle histórico y la edición avanzada siguen pendientes en #331/#271.

Actualización #331-REFIT-WINDOW-LOCALE-487 (2026-09-07, issue [#487](https://github.com/cavazquez/openttdrs/issues/487)):
la ventana de refit localiza hint, unidades, capacidad, coste y etiquetas de
cargas vanilla por slot, preservando nombres custom/NewGRF no catalogados,
selección, consist, IDs y comandos. `OrderRefit` y filtros avanzados siguen
pendientes en #331 y el bloque de órdenes.

Actualización #331-VEHICLE-STATUS-LOCALE-488 (2026-09-07, issue [#488](https://github.com/cavazquez/openttdrs/issues/488)):
el estado dinámico de `VehicleView` localiza estados, velocidad, ausencia de
órdenes y destinos fallback, preservando nombres de estación, coordenadas,
colores, comandos y selección. Details, órdenes y estados avanzados de
movimiento permanecen pendientes en #331 y sus bloques funcionales.

Actualización #331-VEHICLE-DETAILS-LOCALE-489 (2026-09-07, issue [#489](https://github.com/cavazquez/openttdrs/issues/489)):
`VehicleDetails` localiza título, pestañas, resumen de totales, unidades de
potencia/edad, fiabilidad, depósito, renovación, velocidad, órdenes y año de
coste; las cargas vanilla también siguen el locale activo. Nombres de motor y
de cargas NewGRF/custom, IDs, coordenadas, importes, capacidad, consist y
comandos permanecen intactos. Servicio, TE y editor de órdenes avanzado siguen
pendientes en #331 y sus bloques funcionales.

Actualización #331-BUY-STATS-LOCALE-490 (2026-09-07, issue [#490](https://github.com/cavazquez/openttdrs/issues/490)):
el panel dinámico de compra localiza tipo, precio, peso, velocidad, potencia,
coste de operación, capacidad, año, fiabilidad y metadatos NewGRF; el
placeholder de búsqueda se sincroniza al cambiar de locale. Nombres de motor,
sprites, IDs, cifras, filtros, selección y cargos NewGRF/custom permanecen
intactos. TE, criterios avanzados y formato numérico completo siguen
pendientes en #331 y sus bloques funcionales.

Actualización #331-VEHICLE-LIST-LOCALE-491 (2026-09-07, issue [#491](https://github.com/cavazquez/openttdrs/issues/491)):
la lista global de vehículos localiza título por tipo/estación, filtros, orden,
acciones, estados de circulación, edad, grupos, inicio/parada y estados vacíos,
sincronizando el locale con `ClientPreferences` e invalidando la caché de filas
al cambiarlo. Nombres de vehículos y grupos (incluidos NewGRF/custom), IDs,
coordenadas, velocidades, filtros de datos, orden, selección, sprites y
comandos permanecen intactos.
La ventana de grupos dedicada, acciones masivas, criterios avanzados y
catálogos upstream restantes siguen abiertos en #331.
