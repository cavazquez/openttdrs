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

## Etapa publicada — 2026-09-15 — #330 Helidepot FTA: aproximación completa

La misma fixture `helidepot_fta_cycle_15_3.sav` ahora coincide con el oráculo
OpenTTD 15.3 en `initial` más 1000 ticks (`1001` muestras). La comparación
abarca `pos`, `previous_pos`, `state`, `targetairport`, `speed`, `progress`,
`subspeed`, `direction`, `running` y las coordenadas físicas `x_pos`, `y_pos`,
`z_pos` en cada muestra. El tramo de vuelo libre sigue todos los nodos de
espera `TO_ALL` antes de aceptar la arista de aterrizaje, conserva la reserva
del helipad en la transición y ejecuta la aproximación del Helidepot con las
dos pasadas nativas por tick, incluido `HELI_LOWER`.

Validación publicada: 2761 tests del core pasados, 1 ignorado, Clippy estricto,
formato, `git diff --check` y el comparador diferencial (1000/1000). Commit de
código `a8c312e6`. Esto cierra la ventana reproducida del Helidepot, no la
paridad global: #330/#329 siguen abiertos para otros perfiles de aeropuertos,
cinemática aire/mar, callbacks/runtime NewGRF y redes amplias.

## Etapa publicada — 2026-09-14 — #330 aeronave FTA Helidepot

El fixture `helidepot_fta_cycle_15_3.sav` coincide con OpenTTD 15.3 en el
estado inicial y en 80 ticks de la secuencia FTA: `pos`, `previous_pos`,
`state` y `z_pos` quedan alineados, incluida la transición de ascenso
117→126 y la salida hacia la entrada 4 del aeropuerto destino. El importador
restaura las coordenadas físicas `x_pos/y_pos`, `progress`, la velocidad del
rotor y la caché nativa de velocidad; `HELI_RAISE` aplica las dos pasadas del
`AircraftEventHandler` por tick. El comparador JSONL valida `pos`/`state` y la
prueba de integración Rust agrega la comparación de altura física.

Validación publicada: `cargo test -p openttdrs-core --lib` (2757 pasados, 1
ignorado), los 4 tests del oráculo, `cargo clippy -p openttdrs-core --lib
-- -D warnings`, formato y `compare_airport_fta_traces.py` (80/80). Commit
`73c8fcb3`. La velocidad/progreso del vuelo libre posterior sigue siendo un
residual separado; #330/#329 permanecen abiertos para la cinemática aire/mar,
callbacks NewGRF y redes amplias.

## Etapa publicada — 2026-09-14 — #330 aeronave FTA vuelo libre

El mismo fixture de Helidepot ahora reproduce también el tramo posterior al
despegue: la traza reconstruida contra OpenTTD 15.3 coincide durante `initial`
más 300 ticks (`301` muestras). El controlador Rust ejecuta las dos pasadas de
`AircraftEventHandler` por tick, conserva la posición física subtesela,
dirección y altura, y entra al FTA del aeropuerto destino sólo al alcanzar su
ventana de aproximación. El comparador JSONL amplió su contrato a
`pos`/`previous_pos`/`state`/`targetairport`/`speed`/`progress`/`subspeed`/
`direction`/`running`; una revisión de la misma traza confirma además
`x_pos`/`y_pos`/`z_pos` idénticos en los 300 ticks.

La regresión unitaria
`aircraft_update_speed_preserves_native_fractional_accumulation` fija el
remanente nativo de `subspeed` y `progress` (incluido el escalado direccional)
para evitar que una futura simplificación vuelva a desfasar la salida del
helipuerto. Validación publicada: `cargo test -p openttdrs-core --lib` (2758
pasados, 1 ignorado), 28 tests FTA focalizados, Clippy estricto, formato,
`git diff --check` y el comparador externo (300/300). Commit `3293ed74`.
Esto cierra sólo esta ventana reproducida; #330/#329 siguen abiertas para la
cinemática de aire completa, otros perfiles/aeropuertos, callbacks NewGRF y
redes amplias.

## Etapa publicada — 2026-09-14 — #330/#328/#567 ship dinámico

La fixture `mvp_openttd_ship.sav` coincide con OpenTTD 15.3 en `initial` más
300 ticks (`301` muestras) para la proyección naval completa: posición de
tesela y subtesela, altura, progreso, velocidad, `subspeed`, dirección, estado,
rotación, ejecución, ruta y contador interno. El escenario nativo se activa
de forma opt-in para no modificar el save base. La primera ruta hasta la boya
se conserva, pero cuando una orden `Station` apunta a una boya se mantiene el
estado de ruta perdida después de consumirla, igual que el controlador nativo;
el pathfinder genérico no vuelve a girar el barco hacia la misma tesela.
También se hidratan y reemiten los slots globales y velocidades de los motores
navales vanilla. La suite workspace, el replay externo y las herramientas de
traza pasan. El resultado es una ventana dinámica reproducida, no el cierre de
#328/#330/#567: siguen pendientes YAPF/wormholes, callbacks NewGRF, redes
navales amplias y aceptación raster completa.

## Etapa publicada — 2026-09-14 — #330 rail/PBS

La frontera ferroviaria del fixture de consist quedó reproducible y exacta: la
traza Rust coincide con el oráculo OpenTTD 15.3 en `initial` más 500 ticks
(501 muestras). El caso cubre la entrada y salida de estación, la huella de
dos vagones, la señal PathOneWay opuesta, el rebote físico en el extremo de
línea y la reserva PBS que se reconstruye después del giro. La suite específica
de PBS queda en 32/32 y la regresión diferencial del consist compara también
las tres unidades, velocidad, `subspeed`, `progress` y reservas en las 501
muestras.

La selección de pista en cruces conserva la transición entrada→salida completa;
esto evita interpretar una recta junto a un depósito como un giro y mantiene
el frenado `_accel_slowdown` sólo para curvas reales.

La misma fixture dual de trenes se extendió de 40 a 500 ticks con un oráculo
OpenTTD 15.3 versionado. La comparación cubre cinemática y reservas PBS durante
la espera, la curva, la plataforma alternativa y la recuperación posterior del
head-on (`502` filas incluyendo metadata e inicial); la traza Rust coincide en
las `501` muestras comparables. Esto reduce el residual temporal de #330, pero
no cubre todavía redes grandes, cruces/merge adicionales ni todos los
desempates de YAPF.

La fixture simple `train_pbs_15_3` también quedó promovida de inspección visual
a evidencia diferencial: `initial` más 400 ticks coinciden en cinemática y
reservas PBS. La regresión cubre la reversa de un tren unitario en una estación,
la reconstrucción de su salida y la conservación de la reserva hasta la
`PathOneWay`; el oráculo corto de 40 ticks se mantiene como smoke test. #330
sigue abierto para cruces grandes, presignals completas, tráfico complejo y
redes aire/mar.

El ajuste también conserva la cardinalidad nativa de movimiento: una entrada
cardinal consume ocho posiciones de `rail_pixel`, mientras una entrada
diagonal consume dieciséis. #330 sigue abierto para los escenarios no cubiertos
por este fixture: cruces y desempates YAPF grandes, presignals completas,
tráfico complejo y redes aire/mar.

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
ANIT. CB21 se evalúa después de la randomización/CB1B sin tomar RNG propio:
`CALLBACK_FAILED` o un byte bajo igual a cero conserva y rearma la casa, mientras un
resultado no nulo puede llegar desde una subtesela, resuelve la parte norte y
borra la huella completa, retirando ANIT de inmediato y actualizando población,
contador/radio e iglesia/estadio del pueblo. La llamada inaugural de
`BuildTownHouse` también recorre la huella ya materializada: cada subtesela
cuya máscara publica CB1C toma su propia palabra RNG global y recibe
`param2 = 1`, antes de entrar en la cola ANIT/dirty. Los resultados de CB1A,
CB1B y CB1C separan además los bits `8..14`: un ID local no nulo de Action11/
Action0 se encola con la tesela de origen, mientras `CALLBACK_FAILED` queda
silencioso. El cliente respeta `sound.ambient` y aplica la atenuación espacial
de `SndPlayTileFx`; las regresiones cubren ANIT, TileLoop, rollover e inicio de
obra. #527 permanece abierto por evidencia runtime extendida; no amplía el
cierre de #512.
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
paridad ni cierre de #326: en ese corte `ROTSG_*` custom, Action5 tram
`DEPOT_NO_TRACK`, catenaria y waypoints viales seguían fuera, y el detalle
cuantitativo canónico queda en `PARIDAD.md`.

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
catenaria vial, layouts custom/NewGRF, clipping, pivotes, children globales y
framebuffer.

Corrección #326-ROAD-DEPOT-PREVIEW-ACTION5 (2026-09-13): el fantasma de
construcción de depósito deja de dibujar siempre una capa `road_flat` debajo
de la fachada. El plan consulta el roadtype seleccionado, sus flags de
catenaria y los grupos `ROTSG_GROUND/ROTSG_DEPOT`; los tipos normales muestran
sólo los BUILD del depósito, mientras que un tranvía puro replica las
variantes Action5 `WithTrack`/`NoTrack`, sus anclas y tamaños NFO, y agrega
`tram_flat` únicamente para `DEPOT_NO_TRACK`. El preview de grupos
`ROTSG_DEPOT` custom todavía requiere resolver el grupo específico dentro de
la UI, por lo que #326/#565 continúan abiertas junto con clipping, pivotes y
aceptación raster.

Corrección #326-ROAD-DEPOT-PREVIEW-ROTSG-DEPOT (2026-09-13): la preview de
depósito ya resuelve también las capas específicas `ROTSG_DEPOT` de un
roadtype NewGRF, usando el contexto GUI de `INVALID_TILE`, parámetros del GRF,
metadatos NFO de cada vista y la caché de texturas compartida con el mapa.
Cada una de las seis posiciones BUILD puede caer de forma atómica al fallback
Action5/OpenGFX si el grupo no entrega una vista; no se mezclan anclas ni
dimensiones entre capas. La cobertura de selección/atlas/anchos queda
probada; #326/#565 siguen abiertas por callbacks variables, clipping, pivotes
y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-Z-ORDER (2026-09-13): la preview de puentes
normaliza el orden del drag a norte→sur antes de elegir `BridgePiece`, de modo
que arrastrar desde la cabeza opuesta no intercambia rampas ni segmentos del
vano. Las piezas intermedias usan además la misma cota `GetBridgeDeckZ` que el
renderer del mapa, en lugar de la altura local del agua/terreno previo a la
construcción. Las regresiones cubren arrastre invertido y rampas planas,
alineadas y de esquina; #326 continúa abierta por capas NewGRF específicas de
preview, clipping, pivotes y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-ROTSG-BRIDGE (2026-09-13): la preview de
puentes de carretera usa el `RoadType` seleccionado para resolver el grupo
específico `ROTSG_BRIDGE`, incluso antes de que las rampas se materialicen en
el mapa. La copia temporal conserva clima, parámetros del GRF y variables
Action2; cada vista usa sus offsets y dimensiones NFO sobre la misma cota del
tablero, con fallback vanilla por pieza si el grupo no resuelve. La caché de
texturas es la compartida con el renderer in-world; #326/#565 siguen abiertas
por overlays/catenaria custom, clipping, pivotes y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-ROTSG-OVERLAY (2026-09-13): el mismo preview
materializa el `ROTSG_OVERLAY` custom de carretera con la tabla de offsets
propia de puentes (`0,1,11..14`), separado del selector `ROTSG_BRIDGE` y con
una clave de caché distinta. La condición de tranvía conserva la presencia de
bits de vía que usa `DrawBridgeRoadBits`; si la vista o el grupo faltan, sólo
esa capa cae al fallback ya existente. #326/#565 siguen abiertas por
catenaria custom, clipping, pivotes y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-ROTSG-CATENARY (2026-09-13): la preview de
puentes resuelve ahora las dos mitades custom de catenaria (`ROTSG_CATENARY_BACK`
y `ROTSG_CATENARY_FRONT`) con sus índices `23 + 95..106`, conservando el
orden posterior→delantero y las capas relativas al tablero. Respeta la
preferencia global de ocultar catenaria, el flag de catenaria del roadtype y
el fallback independiente de cada grupo; #326/#565 continúan abiertas por
catenaria vanilla de preview, clipping, pivotes y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-VANILLA-CATENARY (2026-09-13): cuando el
roadtype seleccionado publica catenaria pero no grupos específicos, la preview
usa el bloque vanilla `SPR_TRAMWAY` con la tabla de `GetBridgeRoadCatenary`,
los PNG y anclajes NFO de cada mitad, y las capas posterior/delantera del
tablero. La rama custom conserva prioridad y la opción de ocultar catenaria
se aplica a ambas rutas; #326/#565 siguen abiertas por catenaria ferroviaria
de preview, clipping, pivotes y aceptación raster.

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
del child con su parent NewGRF. Los ground base planos auditados
`3924`/`3981`/`4000` conservan también ese contrato; los layouts incompletos
—agua, BUILD, otros base, paletas o selectores que el cliente no puede
materializar atómicamente— conservan la ruta local y su fallback previo; los
road waypoints con layout NewGRF no forman parte de
este corte. La regresión ECS comprueba el cable road/tram, los bounds exactos,
el ordinal 12 y las tres texturas Action1 ground/parent/child; además cubre
ground directo `3981` en BusStop, TruckStop y RoadWaypoint. Kale sigue sin un
foco vial reproducible, por lo que no se atribuye una métrica raster ni se
cierra #326.

Actualización #326-ROAD-WAYPOINT-STATIC-TILELAYOUT-GLOBAL (2026-09-09): el
road waypoint con `TileLayout` NewGRF completo y materializable ya sigue la
misma secuencia nativa: su suelo `WaypGround`, catenaria road/tram 4–11 y
parents BUILD desde 12. Se preservan el prisma `TILE_SEQ_LINE`, profundidad
fuente y child del parent NewGRF; la regresión ECS reutiliza el layout Action1
para comprobar paradas Bus/Truck y un RoadWaypoint con bounds/ordinales exactos.
En plano, el ground materializado conserva además el pase `DrawGroundSprite`;
en pendiente permanece child de la fundación. Un layout incompleto conserva el
fallback atómico de BUILD, pero su catenaria sigue entrando al compositor
global; no se declara equivalencia raster ni se cierra #326.

Actualización #326-ROAD-STOP-LAYOUT-FALLBACK (2026-09-10, `55d570f2`): el
fallback de un `TileLayout` de road stop que resuelve sólo parte de sus
entradas queda reproducido de forma atómica. Un layout con ground/parent
`Action1` válido pero child no materializable no conserva ninguna textura ni
parent custom parcial: vuelve al asfalto vanilla `SPR_ROAD_PAVED_STRAIGHT_*`,
las dos capas BUILD vanilla de la parada pasante y la catenaria directa que ya
tenía ese camino local. La regresión cubre la parada Bus con tranvía y verifica
que no se mezclen ground, parent o child NewGRF. El caso completo de
RoadWaypoint ya está cubierto por la regresión de layout materializable; su
variante incompleta conserva el mismo contrato atómico. #563 queda delimitado
pero abierto hasta reconciliar la catenaria directa y el fallback con el
compositor global y una evidencia raster/oráculo específica; #326 no se cierra.

Actualización #326-RAIL-WAYPOINT-GLOBAL (2026-09-09): el waypoint ferroviario
OpenGFX2 publica los cuerpos `4974`/`4975` (X) y `4976`/`4977` (Y) como dos
parents globales con las cajas literales `16×3×16`/`3×16×16`, profundidad
fuente y ordinales 16/17. Los toldos CC `4978`–`4981` conservan la posición
visual previa y son children de su cuerpo correspondiente, manteniéndose
atómicos antes del siguiente parent global. La regresión ECS cubre ambos ejes,
sus bounds absolutos, los cuatro vínculos y la ventana posterior al sort; la
unitaria fija los slots de parent/child del layout OpenGFX2. Kale no aporta un
foco de waypoint ferroviario, por lo que no se reclama métrica raster. #326 y
#561 continúan abiertos por otros producers, clipping, pivotes, children
globales y framebuffer.

Actualización #326-RAIL-DEPOT-GLOBAL (2026-09-09): el cable direccional de
entrada y las fachadas BUILD de depósito ferroviario se incorporan al sort
global. `DrawRailCatenary` ocupa el ordinal 1 con su prisma nativo; las capas
`DrawRailTileSeq` vanilla y `RTSG_DEPOT` ocupan 2+ (o 1+ sin cable), conservando
la geometría relocalizada aunque la textura NewGRF tenga otra ancla/tamaño. La
fundación deja como children sólo ground y reserva PBS; las fachadas NewGRF
inclinadas pasan a parents con la altura nivelada. La prueba ECS de un SE
eléctrico fija `5659`/`1063`/`1064`, sus bounds y `1063 → 5659 → 1064`; la
regresión `RTSG_DEPOT` cubre además la pendiente. Kale `(195,17)` aporta orden
estructural, no una nueva métrica raster; #326 y #561 no se cierran.

Actualización #326-ROAD-DEPOT-TRAM-BASELINE (2026-09-09): el depósito puro de
tranvía vanilla (`m4 = 63`, `m8[6..12] = 1`) ya aplica la misma relocalización
que `DrawTile_Road`: `SPR_TRAMWAY_DEPOT_WITH_TRACK - SPR_ROAD_DEPOT` a cada
capa BUILD. El SE produce `6035`/`6036`, conserva sus boxes `TILE_SEQ_LINE` y
parents globales, y usa tamaño/ancla NFO del bloque `tramway_049+`; no agrega
un overlay de calle independiente. La regresión ECS fija IDs, atlas, bounds,
ordinales y centros. `ROTSG_DEPOT` custom, `ROTSG_OVERLAY` y
`DEPOT_NO_TRACK` quedan explícitamente pendientes en este corte base; Kale no
tiene foco nuevo, por lo que #326 continúa abierto sin atribuir una métrica
raster.

Actualización #326-ROAD-DEPOT-NEWGRF (2026-09-09): los depósitos viales
NewGRF ya resuelven el grupo Action3 `ROTSG_DEPOT` (selector 8) antes del
fallback vanilla. La selección conserva la prioridad nativa road → tram sólo
si el roadtype es inválido; cada capa SE_1…NW usa su índice relocatable, prisma
`TILE_SEQ_LINE`, parent y ordinal originales, con tamaño/ancla de la vista
Action1/2 evaluada contra el contexto real de la tesela. La caché/evaluación se
compartió con los grupos de puente para no bifurcar random, tablas o parámetros
del GRF; la ruta de depósito suma `GetCompanyPalette(owner)` a la clave y
hornea ese recolor sobre la textura. La regresión ECS cubre dos vistas SE con
texturas, paleta, centers, bounds y profundidad distintos. No se reclama raster
ni cierre: `ROTSG_OVERLAY`, Action5 `DEPOT_NO_TRACK`, clipping, pivotes y
framebuffer siguen fuera de #326.

Actualización #326-ROAD-DEPOT-ACTION5-NO-TRACK (2026-09-09): Action5 tramway
ya no se infiere de la ocupación final de sus slots: el runtime conserva en
orden de stack el último bloque activo que cubrió `49` (`WITH_TRACK`) o `113`
(`NO_TRACK`), con `WITH_TRACK` como baseline del bloque `openttd.grf`. Para el
depósito vanilla de tranvía puro, `NO_TRACK` relocaliza las BUILD a
`SPR_TRAMWAY_DEPOT_NO_TRACK`, conserva sus parents/prismas/ordinales y agrega
el overlay de vía separado; las imágenes Action5 reales conservan NFO y se
hornean con la paleta del dueño sin compartir una textura entre compañías.
Las regresiones de core ejercen precedencia de GRFs y las ECS fijan el SE
`6099`/`6100`, su overlay y sus dos texturas. No se atribuye raster Kale ni se
cierra #326: `ROTSG_OVERLAY`, tramtypes custom, clipping, pivotes y framebuffer
siguen pendientes.

Actualización #326-ROAD-DEPOT-ROTSG-OVERLAY (2026-09-10): los depósitos viales
NewGRF ya distinguen `UsesOverlay()` como OpenTTD: lo activa `ROTSG_GROUND`
(selector 2), no la mera existencia del overlay. Sin un `ROTSG_DEPOT` ganador,
el selector 1 se evalúa contra la misma tesela y el índice
`GetRoadSpriteOffset(SLOPE_FLAT, DiagDirToRoadBits(dir))`; conserva tamaño y
ancla NFO, se emite como ground y, sobre pendiente, como child de la fundación
nivelada. Si falta ese resultado no cae al riel vanilla, y un `ROTSG_DEPOT`
resuelto sigue suprimiendo la capa separada. Las ECS cubren tanto el caso
GROUND/OVERLAY como esa prioridad. No hay métrica Kale ni cierre: la selección
Action5 de tramtypes custom, clipping, pivotes y framebuffer permanecen fuera
de #326.

Actualización #326-ROAD-DEPOT-CATENARY-ACTION5 (2026-09-10): el fallback
Action5 de depósito ya consulta la definición Action0 efectiva aunque el tipo
NewGRF no tenga vistas/sets Action3. Si `RoadTypeFlag::Catenary` está activo,
un tramtype puro sin `UsesOverlay()` puede usar el último `DEPOT_WITH_TRACK`;
un roadtype válido o un tipo con ground propio elige `DEPOT_NO_TRACK`. La
relocalización conserva Action5, paleta por compañía y parents globales. La
ECS crea un tramtype y un roadtype eléctricos sin gráficos propios y verifica
respectivamente `6035`/`6036` y `6099`/`6100`. Esto cubre el fallback, no la
composición entera de tramtypes custom ni clipping/pivotes/framebuffer; #326
permanece abierto.

Actualización #326-SHIP-DEPOT-WATER-GROUND (2026-09-10, `ca239842`): la
comparación con `water_land.h` detectó un faltante específico del depósito
naval. `ShipDepot` conserva la `WaterClass` original en el modelo y las
fachadas 4070..4075 usan los anclajes/tamaños NFO del baseset, además de
conservar las secuencias `TILE_SEQ` de eje/parte y sus bounds globales. El
ground de Canal reproduce ahora `DrawWaterEdges(true, 0, tile)`: 4061 más los
slots 5380..5391, con recortes del perfil activo, anclas generadas y pruebas
ECS de selección/posición. Mar no emite diques, y todavía faltan los sprites y
bordes de Río en pendiente; por eso #326 y #567 permanecen abiertos. Los
subissues [#563](https://github.com/cavazquez/openttdrs/issues/563)–[#567](https://github.com/cavazquez/openttdrs/issues/567)
siguen separando el backlog sin convertir este bloque en paridad global.

Actualización #326/#567-RIVER-WATER-GROUND (2026-09-10, `2dee9535`): la
comparación con `DrawRiverWater` completa el ground vanilla que faltaba en el
corte anterior. Las pendientes `SLOPE_SE`, `SLOPE_NE`, `SLOPE_SW` y `SLOPE_NW`
seleccionan respectivamente `SPR_CANALS_BASE+0..3` (`5328..5331`), con los
cuatro recortes y anchors NFO del perfil OpenGFX activo. La rejilla ya no aplana
las alturas de una tesela `WaterClass::River`; el batch conserva el sprite como
`WaterTile::STATIC`, y un `ShipDepot` fluvial importado lo emite antes de sus
capas `TILE_SEQ`. El perfil vanilla no publica `CF_RIVER_EDGE`, por lo que no
se inventan bordes fluviales: los callbacks/sprites River de NewGRF, el caso
Canal genérico fuera de depósitos y la comparación framebuffer siguen abiertos.
Las regresiones del cliente pasan 1305 tests (2 ignorados); #326 y #567 siguen
abiertos.

Actualización #326/#567-CANAL-WATER-EDGES (2026-09-10, `ff65f7a2`): el
selector de `DrawWaterEdges(true, 0, tile)` y la materialización de los slots
5380..5391 se comparten ahora entre `ShipDepot` y las teselas Canal genéricas.
Esto corrige también las esquinas/diques del agua de canal fuera de una
estructura, sin convertirlos en `WaterTile` animados ni en parents sortables.
La cobertura queda probada con el caso aislado de ocho piezas y con los
depósitos; #326 y #567 siguen abiertos sólo por los callbacks River/Canal
custom de NewGRF, la composición global y el framebuffer.

Actualización #326/#567-CANAL-ACTION5-GROUND (2026-09-10, `bdc72551`): las
rutas de spawn del mundo reciben ahora el estado runtime de `0x08 Canals`.
Los slots 0..3 se consumen para las pendientes de Río y 52..63 para los
diques, compartiendo `NewGrfAction5SpriteCache` entre teselas Canal y el
ground de `ShipDepot`. La textura custom conserva el tamaño y ancla NFO del
sprite decodificado; sin entrada resoluble se mantiene el fallback vanilla.
La cobertura ECS añade una pendiente de Río y un dique de depósito custom,
además del test directo del cache; `cargo test -p openttdrs-client water`
queda en 50/50 y el lint estricto pasa. La subbrecha Action5 queda cubierta,
pero #326/#567 siguen abiertos por `CF_RIVER_EDGE`, callbacks/sprites River y
Canal restantes, traza global con bounds/paleta y framebuffer.

| Issue | Situación real al dejar este corte | Próxima brecha acotada |
|---|---|---|
| [#326](https://github.com/cavazquez/openttdrs/issues/326) | La composición raster global sigue abierta. El sorter incorpora `TileLayoutSpriteGroup` de AirportTile; parents globales para PPP/cables/capas `TILE_SEQ_LINE` rail vanilla, waypoint ferroviario OpenGFX2, depósitos ferroviarios/road NewGRF con `RTSG_DEPOT`, paradas Bus/Truck y road waypoints vanilla o con `TileLayout` NewGRF materializable, depósitos viales —incluidos los baselines tram `DEPOT_WITH_TRACK`/`DEPOT_NO_TRACK` y el `ROTSG_OVERLAY` de default gfx—, cable de entrada de túnel eléctrico y catenaria road/tram de calles normales y de layouts de parada/waypoint incompletos. El vidrio rail, los toldos del waypoint, ground/reserva de depósito inclinado y los postes en pendiente conservan su relación child con el parent correspondiente. `VisualCaptureFreeze` evita falsos positivos animados y el culling usa el rectángulo real de Bevy, pero ninguno equivale a paridad raster. Kale aporta orden estructural para el depósito y no contiene un foco raster reproducible de waypoint/catenaria de estación, así que esas migraciones siguen sin métrica visual propia. El fallback atómico de `TileLayout` incompleto en road stops/waypoints descarta ground/BUILD custom parciales, conserva asfalto y BUILD vanilla después del tramo global 4–11 de catenaria; `NoCatenary` mantiene la reserva legacy. Foundations/rotaciones aeroportuarias, otros layouts ferroviarios custom, la composición completa de superficies/catenaria de tramtypes custom, sprite-stack, clipping, pivotes y framebuffer siguen sin equivalencia global; el contrato global completo de children queda separado en [#561](https://github.com/cavazquez/openttdrs/issues/561). | Obtener un foco raster/oráculo de road stop/waypoint y repetir seis zooms si se altera viewport, culling u overview. |
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

Actualización #326-TILELAYOUT-DIRECT-BASE-GROUND (2026-09-09): los
`TileLayout` NewGRF conservan un SpriteID base estático y materializan sólo
como ground los IDs planos auditados `3924`/`3981`/`4000`
(bare/grass/rough), con geometría NFO `64×31` y ancla `-31,0`. Esta precisión
sustituye las menciones abreviadas posteriores a “sprites base” como fallback:
agua `4061`, BUILD, otros base, paletas directas/custom y selectores dinámicos
siguen usando el fallback atómico. No resuelve el compositor global de #326.

| Orden | Bloque | Estado | Criterio de cierre |
|---:|---|---|---|
| 1 | Zoom y viewport | Completado | Seis niveles OpenTTD (`0,25×`…`0,125×`), culling/overview deterministas y smoke de render; la paridad raster global queda separada de la cobertura de zoom. |
| 2 | RMAP-004: generador procedural | Abierto P1 (RMAP-005–017, RMAP-019–023, RMAP-025–026, RMAP-028–029, RMAP-031, RMAP-033, RMAP-035–055, RMAP-057–058, RMAP-060, RMAP-063–064, RMAP-066–081 y RMAP-083–139 cerrados; RMAP-018/RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034/RMAP-056/RMAP-059/RMAP-061/RMAP-062/RMAP-065/RMAP-082 en curso) | Reducir la primera divergencia de TGP/RNG/`FixSlopes`/clear/towns/industries/objects con matriz 64²→512². RMAP-139 añade settings explícitos de ríos/bordes al comparador y deja exactas las combinaciones auditadas; esto no cierra el generador. RMAP-138 amplía el control a cuatro seeds temperate 1024² (`1330935388`–`1330935391`) y deja exactas las seis fronteras (24/24 comparaciones, 0 teselas y 0 bloques 4×4 por frontera); esto no cierra el generador. RMAP-087 completa el stream RNG de árboles de humedal tras `CreateRivers` y deja `landscape`/`clear` exactos en la cohorte temperate 512²; RMAP-088 unifica el perfil y la cola de `RunTileLoop` de Nueva partida entre cliente y oracle. RMAP-084/086 cubren las rampas de llegada e inicio inclinadas de puentes municipales, RMAP-085 reproduce el coste/clear atómico de su terraformación y RMAP-089 completa los túneles municipales y la terraformación de sus bocas; las cuatro seeds 512² de control quedan exactas hasta `towns`. RMAP-090 completa la representación de `landscape`/`clear` para Arctic, Tropic y Toyland en las cuatro seeds 64² de control, incluyendo la nieve canónica en `MAP3` y zonas tropicales en `MAPT`; RMAP-091 conserva el nibble de `TropicZone` en las calles municipales y extiende la frontera `towns` exacta a las cuatro seeds Tropic de 64²; RMAP-092 porta los gates climáticos de `CheckNewIndustry_*`, RMAP-093 completa las tablas de layout vanilla, RMAP-094 propaga la línea de nieve efectiva y la admisión de campos árticos, RMAP-095 alinea la admisión `OnlyInTown` y el reset de MAP8 de `MakeIndustry`, RMAP-096 permite costas durante la plataforma gratuita y RMAP-097 usa la línea de nieve efectiva al seleccionar casas árticas; RMAP-098 pasa el límite de altura y la línea de nieve efectivos a `GenerateTrees`, dejando las cuatro semillas Arctic 64² exactas en las seis fronteras (`landscape`→`trees`); RMAP-099/100 conservan `TropicZone` y respetan `ClearTile_Road` al materializar objetos/industrias, y RMAP-101 completa layouts Toyland y `OnlyNearTown`, dejando las cuatro semillas tropicales y cuatro Toyland 64² exactas en las seis fronteras. RMAP-102 escala el borde de refinerías por eje, RMAP-103 replica el `Execute` parcial de plataformas y RMAP-104 difiere pendientes al pase de plataforma y limpia el `gfx` alto de `MakeIndustry`; RMAP-105 verifica las seis fases completas y deja las cuatro seeds temperate 512² (`1330935378`–`1330935381`) exactas en 24/24 fronteras. RMAP-113/RMAP-114/RMAP-115/RMAP-116 cierran la primera transición de entrega del mundo: la cola `RunTileLoop`, animación inicial, árboles, casas, costas e industrias; RMAP-117 corrige la orientación de las bocas de puente/túnel en la limpieza vial municipal y deja Toyland 256² exacto en las cuatro seeds. RMAP-118 unifica el consumo del RNG global de `TileLoop_Trees` en Toyland y RMAP-119 admite las bocas de puente/túnel existentes durante `IsRoadAllowedHere`; RMAP-120 usa el `GetTileZ` mínimo para el gate de Bubble Generator en pendientes y RMAP-121 replica el despeje completo de casas multitile que `ToyShop` reemplaza mediante `GetHouseNorthPart`/`ClearTownHouse`; RMAP-123 rechaza `MP_VOID` durante `RiverMakeWider` y conserva el `RoughSnow` de 16 bits durante `TileLoopTreesAlps`; RMAP-124 hace que el preflight de puentes municipales aplique `CheckBridgeSlope` y rechace cabezas a distinto nivel efectivo; RMAP-127 replica el despeje `Auto` de la salida municipal de un túnel y rechaza bocas multibit; RMAP-128 separa los topes de puente y túnel, rechaza costas/puentes paralelos y deja exacta la frontera urbana ártica de 1024²; RMAP-129 conserva las entidades de `IndustryPool` cuando el origen de un layout cae dentro de otra huella sin superposición y deja exactas las seis fronteras de la cohorte ártica 1024²; RMAP-130 conserva la asociación de pueblo de las industrias fundadas sobre casas y deja exacta esa cohorte ártica 1024²/seed `1330935381` en las seis fases; RMAP-132 detiene el caminador municipal al entrar en una carretera de otro pueblo y deja exacta la cohorte ártica 1024²/seed `1330935383` en las seis fases; RMAP-134 hace que el preflight de puentes paralelos recorra la espiral nativa y deja exacta la cohorte tropical 1024²/seed `1330935386` en las seis fases; RMAP-135 conserva el `Chance16` de `LevelTownLand` al visitar una tesela ocupada y deja exacta towns en temperate 1024²/seed `1330935387`; RMAP-136 pasa el límite efectivo de altura a las plataformas industriales y RMAP-137 resuelve su valor dinámico para árboles y deja exactas las fronteras `industries`/`trees` de temperate 2048²/seed `1330935404`. Las seeds Toyland 512² `1330935378`–`1330935381` quedan exactas en las seis fronteras (`landscape`→`trees`), con 0 teselas y 0 bloques 4×4 distintos en cada fase auditada; las cuatro seeds árticas 512² `1330935378`–`1330935381` también quedan exactas en las seis fronteras tras RMAP-124. La matriz completa 64²→512² queda exacta en 15/15 cargas y 15/15 mapas mismo-seed para la cohorte canónica; el alcance de clima/configuración, otras semillas/tamaños y ticks posteriores sigue abierto. RMAP-082 conserva la generalización urbana fuera de la cohorte de control. La evidencia detallada y el resto de avances se mantienen únicamente en `random-map-issues.md`, para no duplicar métricas. RMAP-004 sigue abierto mientras haya divergencias en otros tamaños/fases; RMAP-018 conserva configuraciones de río y fases posteriores multiclima, y RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034 la generalización de pueblos. |
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden estable. Paradas, waypoints viales, estaciones rail, casas, objetos e industrias resuelven layouts `TileSeq` completos por Action3/2→Action1, materializan suelo, parents/children y pendientes; AirportTile conserva también el `TileLayoutSpriteGroup` estático, reemplaza el ground fallback y convierte BUILD en parents/children. En plano, el ground materializado de AirportTile, estación rail, industria, objeto, casa, parada y waypoint vial permanece en el pase `DrawGroundSprite`; con fundación sólo queda como child, mientras sus BUILD y children entran en el stream global. Las regresiones cubren un AirportTile, una estación rail, una industria, un objeto, una pareja de casas vecinas y los tres tipos de parada para evitar que ese ground cubra transparencias posteriores. Los aeropuertos construidos conservan el `gfx` por tesela y la vista Action1/3 sigue como fallback atómico. El procesador aplica `DODRAW`, offsets de sprite/caja/child, `var10`, draw mode `0x100` e invalida la caché con registros `7D`/`0x100`. Sprites base y paletas custom siguen fallback atómico. Los callbacks `0x150` de casas y teselas de industria ya se evalúan sólo en pendientes y pueden suprimir `FOUNDATION_LEVELED`; `RTSG_DEPOT` (selector 8) ya reemplaza las seis fachadas relocatables de depósitos ferroviarios con Action2, offsets NFO y children de fundación; quedan por cubrir las variantes ferroviarias de pendiente/túnel, el compositor de foundations/rotaciones de aeropuertos y el sprite-stack/callbacks avanzados de vehículos. La animación AirportTile ya ejecuta metadatos Action0 y callbacks `0x152`/`0x153`/`0x154` con lista persistida, además de `NewCargo`, `CargoTaken`, `AcceptanceTick` y `AirplaneTouchdown` desde los eventos de simulación, traduciendo el cargo por la CTT propia del GRF. Las listas de badges de `AirportTiles`/`Airports` se traducen por GlobalVar `0x18` y `AirportTile` expone `0x7A` con resultado `UINT_MAX` para índices fuera de tabla. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD. `STNN` conserva ahora `airport.type`, `airport.layout` y `airport.rotation` custom, además de la huella `airport.tile/w/h` materializada; el cargador reatacha sus `AirportTile` cuando el layout activo coincide exactamente. `NGRF` y las filas base de `OBJS` ya tienen modelo semántico; un `OBJS` importado se conserva byte a byte hasta que una construcción/demolición lo invalida. `OBID` fusiona ahora los tres campos conocidos sobre la cabecera/filas originales cuando cambia el mapping, manteniendo columnas futuras y huecos densos; si cambia el conjunto de IDs se usa el writer canónico de forma segura. Todas las tablas `CH_TABLE`/`CH_SPARSE_TABLE` que reconstruye el writer reciben el snapshot semántico al fusionar sobre el cuerpo original campos con schema y tamaño codificado idénticos —incluidos strings, listas y structs anidados—, preservando columnas futuras y huecos mientras no cambien filas ni índices. Las listas de structs con longitud explícita también pueden crecer o reducirse: conservan subcampos futuros en los elementos comunes y sólo agregan elementos nuevos cuando el descriptor es idéntico; los layouts incompatibles o structs fijos siguen usando el writer canónico. [#371](sav-rename-371.md) permite además otra longitud para strings raíz, reconstruyendo sólo la fila y su longitud gamma. Las regresiones legacy directas de `PLYR` y `CITY` prueban que un campo compatible puede cambiar sin añadir campos modernos intactos; si cambia uno ausente o incompatible cae al writer canónico. `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` ya se decodifican, hidratan sus referencias y se reemiten con índices densos y 256 registros; los registros no nulos de pueblo se exponen por GRFID a los scopes parent de casas y objetos y se conservan también cuando no tienen consumidor. `PLYR.allow_list[].key` ya conserva sus strings como struct-list, pero no activa aún autenticación o permisos de red. CB17 de casas y CB157 de objetos pueden crear/modificar la fila PSA de su pueblo y el writer la referencia desde `CITY.psa_list`; quedan cambios de topología, writeback de callbacks de teselas y pools nativos de casas/objetos todavía no modelados. |
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
  comparten la huella de registros con la selección Action2. En plano, su
  ground custom conserva `DrawGroundSprite`; en pendiente sólo queda como
  child de la fundación, mientras el BUILD sigue en el compositor global.
  Los SpriteID base planos auditados `3924`/`3981`/`4000`
  (bare/grass/rough) también pueden ocupar el ground; el resto de sprites base,
  paletas custom y layouts incompletos conservan fallback vanilla atómico.
- Waypoints viales: el suelo vanilla ya respeta `m5` (eje), `m3` (Roadside),
  tranvía y catenaria, y en pendientes usa `FOUNDATION_LEVELED` con sus capas
  como children. Los dos postes del layout vanilla y los layouts `TileSeq` de
  `NewGRF` ya se dibujan con suelo propio, cajas parent y children relativos;
  en plano el ground custom conserva `DrawGroundSprite` y en pendiente queda
  child de la foundation. El procesador runtime aplica `DODRAW`, offsets de
  sprite/caja/child, `var10`, draw mode `0x100` y la caché invalida por
  registros. Los tres ground base planos auditados `3924`/`3981`/`4000` se
  materializan; los demás sprites base y paletas custom siguen en fallback
  atómico.
- Objetos NewGRF: el renderer ya reevalúa Action2 por tesela con random
  (`m3`), offset de footprint, pendiente/terreno, animación (`m3hi`), owner,
  fecha, color, vista y zona/distancias (`0x42`, `0x45`/`0x46`) del pueblo
  asociado. La asociación usa `Object::town` del pool `OBJS` y cae al pueblo
  más cercano sólo en partidas legacy. Las variables `0x60`–`0x63` exponen
  id/random/información/frame de vecinos del mismo footprint y `0x64` cuenta
  instancias por tipo con la distancia mínima; los conteos se precalculan una
  vez por pase. Los layouts `TileSeq` completos reemplazan el suelo y emiten
  parents/children con cajas `M(...)`; en plano el ground custom conserva
  `DrawGroundSprite`, mientras BUILD entra en el compositor global. Los tres
  ground base planos auditados `3924`/`3981`/`4000` se materializan; BUILD,
  otros sprites base, paletas custom y layouts incompletos mantienen fallback
  vanilla.
  Siguen pendientes callbacks de objeto adicionales, conteos por
  clase/catchment y layouts 16-bit completos.
- Casas NewGRF: `DrawNewHouseTile` ya no cae automáticamente en
  `HOUSE_DRAW_DATA`: el sprite de edificio se resuelve desde Action1/2/3 con
  el contexto persistido de la tesela y la zona `0x42` del pueblo identificado
  por `MAP2` (fallback al más cercano en mapas legacy), y se registra como
  parent sortable. Los `TileLayout` completos materializables por Action1, o
  con ground base plano auditado `3924`/`3981`/`4000`, reemplazan también el
  suelo: en plano conservan el pase
  `DrawGroundSprite`, y los BUILD/children se entregan al sorter con sus
  cajas `M(...)`; en pendiente el suelo sigue como child de la fundación.
  Las variables `0x44`, `0x60`/`0x61` (conteos por `HouseID`) y `0x62`/`0x63`
  (información/frame de teselas vecinas) usan ahora el mapa y una instantánea
  de conteos por pase. Los otros sprites base, paletas custom, callbacks de
  color y la paleta `random_colour` siguen siendo residuales explícitos. El
  callback `CBID_HOUSE_DRAW_FOUNDATIONS` (`0x150`) ya se evalúa en pendientes y permite
  que el layout custom suprima la fundación nivelada vanilla.
- Industria NewGRF: la vista Action2 runtime usa también sus offsets resueltos
  y, cuando la tesela se nivela, el overlay se adjunta al último parent de
  `DrawFoundation`. Su TileLayout admite Action1 y, sólo como ground, los base
  planos auditados `3924`/`3981`/`4000`; agua `4061`, BUILD, paletas y otros
  base quedan en fallback atómico. El callback
  `CBID_INDTILE_DRAW_FOUNDATIONS` (`0x30`) se evalúa en pendientes y puede
  conservar el relieve original; siguen abiertos los callbacks de sonido/slope
  y los layouts/children múltiples fuera del subconjunto cubierto.
- Aeropuertos NewGRF: los layouts `Airports` conservan el `gfx` global de cada
  `AirportTile` junto con el `subst` vanilla de `m5`; al construir, el cliente
  materializa el sprite Action1/3 por tesela mediante la caché de imágenes y
  reevalúa Action2 con posición relativa, frame, layout padre, random y
  vecinos. El ground de TileLayout también admite los base planos auditados
  `3924`/`3981`/`4000`; otros base, agua, BUILD y paletas siguen en fallback.
  Si falta el catálogo o la vista cae al `AirportPiece` vanilla.
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

Actualización #326/#564-AIRPORT-ROTATION-FOUNDATION (2026-09-10, `1c5cc49a`):
se añadió una regresión de integración para un aeropuerto NewGRF E/O inclinado.
La fixture exige el selector persistido `0x40`, la posición directa `0x43`, el
ground runtime correcto, el BUILD independiente y la foundation custom Action5
slot 58 como parent del ground. Pasan las 28 pruebas aeroportuarias y Clippy;
la matriz completa de foundations/rotaciones, paletas, sonidos y framebuffer
continúa pendiente, por lo que #326, #329 y #564 no se cierran.

Actualización #326/#565-TRAMTYPE-SURFACE-CATENARY (2026-09-10, `fe1c5913`):
la regresión `sloped_newgrf_tram_overlay_attaches_to_its_foundation_parent`
verifica superficie custom de tramtype como child de foundation inclinada y
los grupos Action3 de catenaria trasero `5` y frontal `4`, incluyendo tres
recortes traseros y uno frontal. Pasan las 50 pruebas de catenaria y Clippy;
#565 permanece abierto por superficies/pendientes, anclas, clipping,
depósitos y framebuffer fuera de este caso representativo.

Actualización #326/#567-CANAL-FEATURE-SPRITES (2026-09-10, `9b68c2b1`): el
renderer consume ahora las vistas `CanalFeatureDef.newgrf_views` de Action1/3
para `CF_RIVER_SLOPE` y `CF_DIKES`, respetando el offset de la vista plana
cuando `CFF_HAS_FLAT_SPRITE` está activo. La ruta se usa tanto en agua del
mundo como en `DrawWaterDepot`; cada textura conserva ancho, alto y anclas NFO
y dispone de un namespace de caché separado de Action5 `0x08 Canals`. Si la
vista no existe, el fallback sigue siendo Action5 y luego OpenGFX vanilla. La
regresión focal cubre materialización y no colisión de handles; `CF_WATERSLOPE`,
`CF_RIVER_EDGE`, callbacks/offsets restantes y la matriz de framebuffer aún
quedan pendientes, por lo que #567 y #326 permanecen abiertos.

Actualización #326/#567-RIVER-EDGES-WATER-SLOPE (2026-09-10, `d75294cc`):
`DrawWaterEdges` ya comparte la tabla de conectividad entre Canal y River. Las
vistas `CF_RIVER_EDGE` se consumen en los bloques planos y de pendiente
(`0/12/24/36/48`) y `CF_WATERSLOPE`/`CF_RIVER_SLOPE` pueden aportar el ground
plano cuando declaran `CFF_HAS_FLAT_SPRITE`; todas las texturas raw mantienen
su geometría NFO y se separan de Action5 en la caché. El mundo y
`DrawWaterDepot` emiten el ground, bordes y diques en el orden correcto, con
fallback atómico a Action5/OpenGFX. Las 55 pruebas focales de agua y Clippy
pasan. Callbacks de offset, resolución Action2 completa, las combinaciones
restantes de depósito/costa y la comparación de framebuffer siguen pendientes;
#567 y #326 permanecen abiertos.

Actualización #326/#567-SHIP-DEPOT-FEATURE-GROUND (2026-09-10, `058b5758`):
la regresión `canal_ship_depot_consumes_feature_ground_and_dike_views` ejerce
la ruta real de `DrawWaterDepot` con `CF_WATERSLOPE` y `CF_DIKES` Action1/3.
Verifica el ground plano estático, sus anclas NFO y los ocho diques custom sin
perder las capas `TILE_SEQ` del depósito; los casos de Sea, River, Action5 y
vanilla continúan cubiertos. Esto cierra la evidencia focal del depósito, no
la matriz completa de eje/parte, costa, callbacks, clipping ni framebuffer;
#567 y #326 permanecen abiertos.

Actualización #326/#567-CANAL-ACTION2-TILE-CONTEXT (2026-09-10, `a61d0975`,
`4855b8c3`): el runtime de `CanalFeatureDef` conserva los grupos Action2 y
resuelve cada vista sin hacer wrap de índices inexistentes. El renderer de
agua y de `DrawWaterDepot` alimenta ahora el contexto por tesela con altura
`0x80`, terreno provisional `0x81=0`, conectividad `0x82`, random persistido
`0x83` y los parámetros del callback `0x147`; el fingerprint de caché impide
que la primera variante resuelta se filtre a otra tesela. Las 56 pruebas
focales de agua cubren tres contratos del núcleo y dos vistas dependientes de
altura con texturas separadas; el lint estricto pasa. La importación de terreno real,
todos los scopes/variables Action2, la matriz completa de callbacks, clipping,
orden global y framebuffer siguen pendientes, por lo que #567 y #326 no se
cierran.

Actualización #326/#567-RIVER-EDGE-RUNTIME-OFFSET (2026-09-10, `bfda87fc`):
la elección de los bloques River `0/12/24/36/48` ya consulta el slot base de
`CF_RIVER_SLOPE` con el contexto de la tesela, en vez de depender sólo de la
tabla de vistas precalculada. Así un grupo Action2 runtime conserva el bloque
de bordes correspondiente aunque no tenga preview estático; si el grupo no
resuelve, se mantiene el fallback sin bordes custom. La regresión runtime-only
y las 57 pruebas focales de agua pasan junto con Clippy. El terreno `0x81`
real, callbacks restantes, combinaciones de costa/eje/parte, clipping, orden
global y framebuffer siguen pendientes; #567 y #326 permanecen abiertos.

Actualización #326/#567-CANAL-TERRAIN-CONTEXT (2026-09-10, `c82d1de2`): el
contexto de render por tesela conserva ahora el clima efectivo y la línea de
nieve persistida del mundo. `CanalScopeResolver` recibe `TropicZone` desde el
nibble bajo de `mapt` en subtropical y clasifica la altura contra la línea de
nieve en subártico; temperate y Toyland mantienen su valor explícito mientras
no exista una fuente equivalente importada. La regresión cubre ambos climas y
el límite de nieve; las 58 pruebas focales de agua y Clippy estricto pasan.
Quedan pendientes las demás variables/scopes Action2, nieve variable de
NewGRF, combinaciones completas del depósito, clipping, orden global y
framebuffer, por lo que #567 y #326 permanecen abiertos.

Actualización #326/#567-CANAL-LOCK-HEIGHT (2026-09-10, `9c6111bc`): el
resolver de agua replica ahora el ajuste de `CanalScopeResolver::GetVariable`
para `LockPart::Upper`: `var 0x80` usa `tile_z - 1` sólo en esa parte de una
esclusa; depósitos navales y las partes Middle/Lower conservan su altura
original. La regresión cubre Upper y Lower, junto con las 59 pruebas focales
de agua y Clippy estricto. Aún quedan callbacks/variables Action2 restantes,
la matriz completa del depósito naval, clipping, orden global y framebuffer;
#567 y #326 permanecen abiertos.

Actualización #326/#567-SHIP-DEPOT-TRACE-OFFSETS (2026-09-10, `ccd8cd2d`): la
traza `world-draw` del depósito naval conserva ahora en cada draw los `dx/dy`
literales de `TILE_SEQ`, además de sus bounds, paleta y ordinal de inserción.
La prueba focal del depósito continúa en 6/6 y Clippy estricto pasa; esto
mejora la evidencia reproducible de posición y orden, pero no equivale aún a
un diff de framebuffer. La matriz completa de costa/eje/parte, callbacks,
clipping, orden global y framebuffer mantiene abiertos #567 y #326.

Actualización #326/#567-CANAL-LOCK-WATERSLOPE (2026-09-10, `faebea12`): la
ruta de `DrawWaterLock` consume ahora `CF_WATERSLOPE` para el ground de las
esclusas. Se conserva el orden vanilla `NE/SE/SW/NW` del tramo Middle, se
desplazan los cuatro slots cuando `CFF_HAS_FLAT_SPRITE` aporta el plano y las
partes Lower/Upper usan ese slot plano; catálogos legacy sin plano vuelven al
sprite vanilla sin dejar una textura parcial. La resolución reutiliza el
contexto Action2 por tesela, el callback de offset `0x147`, las anclas NFO y
el z del pase ground. La prueba de integración atraviesa
`push_water_tile_with_action5`, cubre el slot NE y la regresión focal de agua
queda en 61/61; Clippy estricto pasa. La matriz completa de partes/direcciones,
estructuras `CF_LOCKS`, costa, clipping, orden global y framebuffer sigue
pendiente, por lo que #567 y #326 permanecen abiertos.

Actualización #326/#567-CANAL-LOCK-STRUCTURES (2026-09-10, `2df1e9b2`): cuando
`CF_WATERSLOPE` reemplaza el ground compuesto de `DrawWaterLock`, el renderer
emite aparte las dos líneas `TILE_SEQ` de estructuras. Se generaron los 48
sprites vanilla Action5 de `SPR_CANALS_BASE + 4..51` con sus anclas NFO; la
resolución intenta `CF_LOCKS`, luego Action5 y finalmente OpenGFX, conservando
las cajas `M(...)`, el ordinal de inserción y el `zoffs +24` de Upper alto.
La regresión verifica ground más dos capas `CF_LOCKS` y la tabla de las cuatro
orientaciones; pasan 62 pruebas de agua, 6 de depósito naval y Clippy estricto.
Quedan pendientes la matriz completa de partes/ejes, interacción con costa y
barcos vecinos, clipping, orden global y comparación de framebuffer; #567 y
#326 permanecen abiertos.

Actualización #326/#567-SHIP-DEPOT-GROUND-TRACE (2026-09-10, `c4aa738f`): las
vistas planas custom de `CF_WATERSLOPE`/`CF_RIVER_SLOPE` que usa un depósito
naval se colocan ahora en la banda de `DrawGroundSprite`, igual que el ground
que precede a `DrawWaterTileStruct`, sin cambiar el sesgo de costa del agua
normal. La traza conserva el slot local resuelto (`SPR_CANALS_BASE + slot`) en
vez de informar siempre `SPR_FLAT_WATER_TILE`; la regresión de Canal verifica
la profundidad y siguen pasando 62 pruebas de agua, 6 de depósito naval y
Clippy estricto. Faltan la traza completa por Sea/Canal/River, la matriz de
vecinos/ejes/partes, clipping y framebuffer; #567 y #326 permanecen abiertos.

Actualización #326/#567-RIVER-FLAT-DEPOT-GROUND (2026-09-10, `c075a132`): la
regresión `river_ship_depot_consumes_flat_feature_ground_in_ground_pass`
comprueba la variante plana de `CF_RIVER_SLOPE` con `CFF_HAS_FLAT_SPRITE` en
la ruta real de `DrawWaterDepot`. El sprite custom conserva su ancla NFO y el
`ground_draw_z`, mientras River no agrega diques ni bordes; la cobertura
queda en 62 pruebas de agua, 7 de depósito naval y Clippy estricto. Sigue
pendiente la traza exportada completa por Sea/Canal/River, la matriz de
vecinos/ejes/partes, clipping y framebuffer; #567 y #326 permanecen abiertos.

Actualización #326/#567-WATER-TRACE-SLOTS (2026-09-10, `6106410e`): la traza
`world-draw` deja de informar `SPR_FLAT_WATER_TILE` para una vista custom ya
materializada. `CF_WATERSLOPE` y `CF_RIVER_SLOPE` conservan su slot local
resuelto, `CF_RIVER_EDGE` conserva además el bloque `0/12/24/36/48`, y
`CF_DIKES` usa la base `SPR_CANAL_DIKES_BASE`; los reemplazos Action5 siguen
reportando el mismo ID lógico que su slot vanilla. Las rutas de agua y
depósito naval siguen pasando 62 y 7 pruebas respectivamente, junto con
Clippy estricto. La exportación sobre un SAV real, clipping y comparación de
framebuffer siguen pendientes; #567 y #326 permanecen abiertos.

Actualización #326/#567-SHIP-DEPOT-TRACE-OFFSET-SEPARATION (2026-09-10,
`85adbb1a`): la traza del depósito naval ya no duplica el origen `dx/dy` de
`TILE_SEQ` en `offset`. Igual que `DrawCommonTileSeq` de OpenTTD, el origen
queda expresado una sola vez en `bounds.ox/oy`; `offset` permanece reservado
para un corrimiento de pantalla explícito. En la región real de Kale
`138,7..140,10`, la comparación pasa 22/22 selecciones, 10/10 geometrías
explícitas, 3/3 paletas y 22/22 órdenes; los 7 tests de depósito y Clippy
estricto también pasan. Esto corrige la evidencia focal de posición y orden,
pero no completa Sea/Canal/River, las cuatro combinaciones de eje/parte, los
vecinos, clipping ni framebuffer; #567 y #326 permanecen abiertos.

Actualización #326/#563-ROAD-STOP-DODRAW-EMPTY (2026-09-10, `08fd2d93`): la
resolución de `TileLayout` conserva la regla de `DrawCommonTileSeq` que salta
los children de un parent con sprite cero, incluido `DODRAW=0`, hasta el
siguiente parent. Un layout custom completo cuya secuencia BUILD queda vacía
se considera igualmente consumido: no reintroduce postes/edificios vanilla ni
crea children huérfanos; el ground `DODRAW=0` sigue suprimiendo sólo su capa.
Las regresiones cubren 13 casos de `TileLayout` en core y 3 rutas de parada o
waypoint en cliente; Clippy estricto pasa. El fallback de layouts no
materializables, el eje completo, catenaria directa, comparación raster y
framebuffer siguen pendientes, por lo que #563 y #326 permanecen abiertos.

Actualización #326/#563-ROAD-STOP-CACHE-SLOTS (2026-09-10, `2537aeaa`): el
namespace de caché de las texturas `Action1` de ground/BUILD de `TileLayout`
vial queda separado del de las vistas simples. El rango `spec * 64` y todos
los índices de la secuencia se prevalidan antes de publicar sprites; un spec o
una secuencia que exceda `u16` devuelve fallback atómico en vez de saturar el
slot y aliasar la textura de otro spec. Pasan las regresiones de los límites,
las 3 rutas de road stop, los 13 casos core, formato y Clippy estricto. Esta
etapa elimina una colisión de identidad de caché, pero #563 sigue abierta por
el eje/parte completo, catenaria directa, trace y compositor global, evidencia
raster, vecinos, clipping y framebuffer; #326 tampoco se cierra.

Actualización #326/#563-ROAD-STOP-Y-AXIS (2026-09-10, `324f1aa9`): el fixture
de `TileLayout` materializable se parametriza también con
`RSV_DRIVE_THROUGH_Y`. La regresión comprueba los sprites de catenaria Y
`6070/6042`, sus bounds/orden global y el mismo vínculo parent/child custom;
los casos X existentes permanecen intactos. El conjunto focal queda en 4/4
(Bus X/Y, Truck y RoadWaypoint), con formato y Clippy estricto verdes. #563
continúa abierta por la matriz completa de partes/callbacks, catenaria directa,
trace/compositor global, evidencia raster, vecinos, clipping y framebuffer.

Actualización #326/#563-ROAD-STOP-FALLBACK-SORT (2026-09-10, `9f55510c`): la
capacidad del bloque de caché se incorpora también al predicado que decide si
un layout puede entrar al sorter global. Un overflow ya no puede dejar
catenaria global junto con un BUILD vanilla local: el layout completo se
considera no materializable desde el principio y conserva el fallback atómico
coherente. La regresión de sortability y las 4 rutas focales de road stop
permanecen verdes; #563 sigue abierta por la matriz de partes/callbacks,
catenaria directa, trace, raster, vecinos, clipping y framebuffer.

Actualización #326/#563-ROAD-STOP-SIMPLE-CACHE (2026-09-10, `2c4c7521`): las
vistas Action3 simples de road stops dejan de calcular `spec * 6 + view` con
saturación. El renderer valida ambas operaciones y, si la clave no cabe en
`u16`, deja continuar el fallback vanilla en vez de aliasar la textura del
último spec. La regresión cubre el último slot representable y los dos
overflows; el filtro de road stops queda en 32/32, core en 13/13, formato y
Clippy estricto verdes. Esto cierra otro riesgo de identidad de caché, pero
#563 y #326 permanecen abiertos por la matriz de partes/callbacks, trace,
evidencia raster, vecinos, clipping y framebuffer.

Actualización #326/#565-TRAM-CATENARY-FALLBACK (2026-09-10, `431b0991`): la
selección de catenaria de road/tram ya resuelve primero los grupos Action3
`CATENARY_BACK` y `CATENARY_FRONT`. Igual que `GetCustomRoadSprite`, sólo
abandona ambos sprites vanilla cuando al menos un grupo devuelve un sprite
custom; si los grupos están declarados pero no producen una vista resoluble,
se recuperan ambos sprites vanilla sin dejar la calle sin cables. La prueba
de decisión y las regresiones de catenaria/tram pasan (5/5 y 1/1 focal,
respectivamente), junto con formato y Clippy estricto. #565 sigue abierto por
la matriz completa de superficies/pendientes, anclas, depósitos, clipping y
framebuffer.

Actualización #326/#565-TRAM-RUNTIME-ONLY (2026-09-10, `910dddc2`): las
superficies Action1/3 de road y tram ya no requieren una preview estática para
entrar al renderer. La vista se resuelve una sola vez desde Action2 runtime,
se conserva su ancla NFO y el caché usa el índice runtime solicitado; así dos
orientaciones de un tipo sin `newgrf_views` no reutilizan la primera textura.
Las regresiones nuevas de vista runtime y de superficie custom inclinada pasan
1/1 cada una, con formato y Clippy estricto verdes. #565 sigue abierto por la
matriz completa de superficies/pendientes, anclas, depósitos, clipping y
framebuffer.

Actualización #326/#567-SHIP-DEPOT-SHARED-EDGE (2026-09-10, `21d4c73a`): la
regresión ECS ejerce por primera vez las dos partes contiguas de un depósito
naval de Canal (`m5=0x30/0x31`). La conectividad compartida deja diez diques
exteriores, no dibuja el borde interno en ninguno de los dos sentidos y conserva
las tres capas `TILE_SEQ` de las fachadas. Es una cobertura de vecinos adicional,
no una declaración de paridad: la matriz Sea/Canal/River, costa, callbacks,
clipping, orden global y framebuffer sigue pendiente; #567 y #326 continúan
abiertos.

Actualización #326/#563/#565-ROAD-WAYPOINT-RUNTIME-ONLY (2026-09-10,
`39499b9c`): `RoadWaypoint` ya no exige `newgrf_views` o una preview estática
para sustituir el suelo de carretera y el overlay de tranvía. Cada grupo
Action2 se resuelve una sola vez con el contexto de la tesela; la misma vista
resuelta alimenta la geometría NFO y la caché, incluyendo tipos runtime-only.
La regresión integrada publica roadtype y tramtype custom sin vistas estáticas
y verifica que ambas texturas llegan al waypoint. Las 8 pruebas focales de
waypoint, formato y Clippy estricto pasan. #563 y #565 continúan abiertas por
la matriz completa de partes/pendientes, callbacks, anclas restantes, clipping,
orden global, vecinos y framebuffer; #326 tampoco se cierra.

Actualización #326/#563/#565-SPECIFIC-VIEW-CACHE (2026-09-10, `5a5b9790`):
puentes, depósitos y otros draw-procs que consumen `ROTSG_*` ya reutilizan la
vista Action2 específica que resolvió el compositor para generar la textura.
Antes `specific_sprite_for_tile` volvía a evaluar el grupo al llenar la caché;
ahora geometría NFO y bytes RGBA parten de la misma selección, preservando
anclas frente a random/vars de tesela. Las regresiones de grupos específicos,
waypoint y depósito naval pasan junto con Clippy estricto, formato y
`diff --check`. Esto elimina una divergencia de caché, pero #563, #565 y #326
siguen abiertas por la matriz completa de superficies, vecinos, callbacks,
clipping, orden global y framebuffer.

Actualización #326/#565-ROAD-OVERLAY-GROUP (2026-09-10, `7786f906`): la calle
normal reconoce ahora `RoadTypeInfo::UsesOverlay()` mediante la presencia de
`ROTSG_GROUND`. En ese contrato deja de usar la vista normal, pinta el suelo
desnudo y aplica `ROTSG_GROUND` seguido de `ROTSG_OVERLAY`, conservando metadata
NFO y relación con la foundation. La regresión ECS usa un roadtype
runtime-only sin vista default y verifica ambas capas custom; el fallback del
suelo base selecciona además la variante nieve/desierto. #565 continúa abierta
por el caso de tranvía puro, paradas/depósitos, pendientes completas, clipping
y framebuffer;
#326 permanece abierto por la matriz global.

Actualización #326/#565-TRAM-OVERLAY-GROUP (2026-09-10, `78f04bd7`): el caso
de tranvía puro reconoce `RoadTypeInfo::UsesOverlay()` cuando el tramtype
publica `ROTSG_GROUND`. La pasada de suelo deja el terreno base desnudo,
aplica el `GROUND` específico y la pasada de overlay aplica el `OVERLAY`,
evitando que el fallback de carretera vanilla tape la representación custom.
Con carretera presente se conserva la precedencia del roadtype y en
pendientes las capas siguen al parent de foundation. La regresión ECS usa un
tramtype runtime-only sin vistas default y verifica las dos capas custom;
también pasan las regresiones relacionadas de carretera, catenaria, pendientes,
waypoint y depósito naval. #565 continúa abierta por la matriz completa de
superficies, paradas/depósitos, pendientes, anclas, clipping y framebuffer;
#326 permanece abierta por la matriz global.

Actualización #326/#563/#565-ROAD-STOP-OVERLAY-GROUP (2026-09-10,
`697bc00d`): las paradas viales pasantes ya ejercen el contrato de
`DrawRoadOverlays`: un roadtype con `ROTSG_GROUND` aporta `GROUND` y el
`OVERLAY` opcional, el tramtype sólo aporta el underlay cuando no existe una
carretera válida, y en los demás casos se conserva el riel vanilla de Action5.
La spec de `RoadStops` respeta `ROADSTOP_DRAW_MODE_OVERLAY`; las capas custom
conservan metadata NFO, orden `GROUND → OVERLAY` y parent de foundation en
pendiente. Las regresiones ECS cubren roadtype custom y tranvía puro, junto con
la suite completa de 1341 tests del cliente, Clippy estricto, formato y
`diff --check`. #563 y #565 siguen abiertas por bahías (`ROTSG_ROADSTOP`),
matriz de partes/callbacks, anclas completas, clipping y framebuffer; #326
permanece abierta por la matriz global.

Actualización #326/#563-ROAD-STOP-BAY-GROUP (2026-09-10, `b6f0d461`): las
bahías viales consultan ahora `ROTSG_ROADSTOP` cuando el roadtype publica
`ROTSG_GROUND` (`UsesOverlay()`) y el modo de la parada permite dibujar la
carretera. La vista específica conserva sus offsets NFO, sustituye el ground
vanilla y, sobre una pendiente, queda como child de la fundación nivelada. Las
regresiones separan deliberadamente `GROUND` de `ROADSTOP` para detectar una
selección accidental del selector 2: pasan las variantes plana e inclinada,
las 32 pruebas focales de road stop, la suite completa del cliente (1343
pasadas, 2 ignoradas), Clippy estricto, formato y `diff --check`. #563 sigue
abierta por matriz de partes/callbacks, catenaria/trace, vecinos, clipping y
framebuffer; #326 permanece abierta por la matriz global.

Actualización #326/#567-INDUSTRY-WATER-BORDERS (2026-09-10, `62032575`):
`IsWateredTile` ya consulta el offset exacto de `TileOffsByDir(from)` para
suprimir bordes internos de industrias con el mismo `IndustryID` y de la
transición industria↔Oil Rig. La tabla conserva las ocho direcciones del mapa,
incluidos los cuatro lados y las cuatro esquinas; IDs distintos vuelven a
exponer el borde. Las 8 direcciones y ambos sentidos Oil Rig tienen regresión,
las 17 pruebas focales de agua pasan y la suite completa del cliente queda en
1345 pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check`
verdes. #567 sigue abierta por la matriz completa Sea/Canal/River, piezas y
callbacks del depósito, clipping, orden global y framebuffer; #326 permanece
abierta por la matriz global.

Actualización #326/#566-VIEWPORT-ASSET-EVENTS (2026-09-10, `8fe04466`): el
sorter preciso de Bevy consume `AssetEvent<Image>` y
`AssetEvent<TextureAtlasLayout>` filtrados por los handles usados por cada
parent. Si una textura o layout llega después del spawn, el parent se
reevalúa con el rectángulo real del PNG; los cambios de `Assets<T>` no
relacionados con caches no fuerzan sorts repetidos. La regresión cubre la
carga tardía de un parent fuera del bounds 3D, el filtro focal de viewport
pasa 14/14, la suite cliente queda en 1346 pasadas y 2 ignoradas, Clippy
estricto, formato y `diff --check` verdes. Esto corrige una subetapa de
clipping/culling, pero #566 sigue abierta por la matriz completa de pivotes,
clipping por familias y framebuffer; #567 permanece abierta por
Sea/Canal/River, vecinos, callbacks, orden global y framebuffer.

Actualización #326/#566-ATLAS-LAYOUT-EVENT (2026-09-10, `ec7d3bff`): se añade
regresión para un parent con `TextureAtlas` cuyo `TextureAtlasLayout` llega
después del spawn. El caso empieza usando el fallback de bounds 3D y verifica
que `AssetEvent<TextureAtlasLayout>` lo reincorpora al stream cuando se
materializa el rectángulo del atlas; el bloque focal del sorter pasa 15/15,
la suite cliente queda en 1347 pasadas y 2 ignoradas, Clippy estricto, formato
y `diff --check` verdes. Es cobertura adicional de la subetapa de
clipping/culling; #566 sigue abierta por pivotes, clipping por familias y
framebuffer, y #567 por Sea/Canal/River, vecinos, callbacks, orden global y
framebuffer.

Actualización #326/#567-SHIP-DEPOT-BOTH-AXES (2026-09-10, `0b808ace`): la
regresión de `DrawWaterEdges` para depósitos navales ejerce ahora las dos
orientaciones de `GetOtherShipDepotTile`: `AXIS_X` suprime los lados internos
0/2 y `AXIS_Y` los lados internos 1/3, conservando los ocho diques exteriores
por pareja y los conteos de ambos casos. Las 9 pruebas focales de depósito
naval pasan, la suite cliente queda en 1348 pasadas y 2 ignoradas, Clippy
estricto, formato y `diff --check` verdes. Es una ampliación del oráculo de
vecinos; #567 sigue abierta por la matriz completa Sea/Canal/River, callbacks,
clipping, orden global y framebuffer, y #326 por la composición global.

Actualización #326/#566-VIEWPORT-ASSET-LIFECYCLE (2026-09-10, `a9c15f24`):
la regresión del sorter preciso cubre también la descarga de una `Image` ya
usada por un parent. Tras `AssetEvent::Removed`, el sistema vuelve a ejecutar
el sort y retira del stream al parent cuyo rectángulo dejó de estar
materializado, evitando conservar una geometría precisa obsoleta. El bloque
focal del sorter pasa 15/15, la suite cliente queda en 1348 pasadas y 2
ignoradas, Clippy estricto, formato y `diff --check` verdes. Es cobertura del
ciclo de vida de assets dentro de #566; la issue sigue abierta por la matriz
completa de pivotes, clipping por familias y framebuffer, y #326 por la
composición global.

Actualización #326/#567-OBJECT-WATER-CLASS (2026-09-10, `4a0db637`): el
contrato raw de `MP_OBJECT` ya conserva `HasTileWaterClass`/`GetWaterClass`
desde `MAPT` y `M1`, aunque el modelo semántico use `TileKind::Unknown(10)`.
`IsWateredTile` usa esa clase para objetos: Sea, Canal y River suprimen el lado
compartido de un dique, mientras `Invalid` deja visible el borde. También se
actualizaron las regresiones Action2 para el byte de terreno `0x21` que
corresponde a un objeto de clase Sea. La nueva prueba focal, core completo
(2386 pasadas, 1 ignorada), cliente completo (1349 pasadas, 2 ignoradas),
Clippy estricto, formato y `diff --check` pasan. #567 sigue abierta por
túnel/puente acuático, matriz completa Sea/Canal/River, callbacks, clipping,
orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-RIVER-EDGE-MATRIX (2026-09-10, `551e3a9a`):
la regresión integrada del depósito naval River se amplía a las cuatro
pendientes reales: `SE`, `NE`, `SW` y `NW`. Cada fixture verifica el sprite
de superficie y el bloque correspondiente de `CF_RIVER_EDGE` (offsets
12/24/36/48), con sus ocho bordes custom y las capas `TILE_SEQ` intactas. El
bloque focal conserva 13 pruebas y la suite cliente queda en 1355 pasadas y
2 ignoradas, con Clippy estricto, formato y `diff --check` verdes. #567 sigue
abierta por callbacks y vecinos restantes, costas/túneles/puentes, clipping,
orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-RIVER-EDGE-BLOCK (2026-09-10, `032146e5`):
la regresión ECS de un depósito naval River inclinado verifica que el
dispatcher selecciona el bloque de ocho bordes específico de la pendiente
(`SLOPE_NE`, offsets 24..31), además del ground River y de las capas de
estructura `TILE_SEQ`. Los ocho sprites reciben identificadores distintos y
se comprueba que ninguno se pierde por la emisión del depósito; el bloque
focal queda en 13 pruebas y la suite cliente en 1355 pasadas y 2 ignoradas,
con Clippy estricto, formato y `diff --check` verdes. Esta cobertura confirma
un bloque de selección de River, pero #567 sigue abierta por las otras
pendientes con emisión integrada, callbacks y vecinos restantes, clipping,
orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-AQUEDUCT-RAMP-WATER (2026-09-10, `4bd0daba`):
`IsWateredTile` reconoce los extremos de acueducto almacenados como
`MP_TUNNELBRIDGE` y transporte agua. Sólo el lado opuesto a la dirección de
la rampa se considera mojado, reproduciendo `ReverseDiagDir` + `DirToDiagDir`;
los puentes de carretera/ferrocarril siguen secos. La regresión cubre los ocho
valores direccionales y el filtro de transporte; cliente completo queda en
1350 pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check`
verdes. #567 sigue abierta por la matriz completa Sea/Canal/River, costas,
callbacks, piezas restantes, clipping, orden global y framebuffer; #326
permanece abierta.

Actualización #329/#567-SHIP-CARGO-AGE-PERIOD (2026-09-11, `e231a0b4`): el
parser Action0 conserva la propiedad naval `0x1D` (`cargo_age_period`) en el
catálogo NewGRF, con `185` como default compatible y `0` como desactivación.
La economía deja de usar un múltiplo global fijo y aplica el contador nativo
por vehículo, actualizando también la caché cuando cambia el motor. Se
agregaron regresiones de propagación parser→catálogo, período personalizado y
desactivación explícita. Core queda en `2452 passed; 0 failed; 1 ignored` y
cliente en `1388 passed; 0 failed; 2 ignored`, con formato, `git diff
--check` y Clippy estricto limpios. #567/#329 siguen abiertas por las
propiedades de envejecimiento de los demás features, callbacks navales y
aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-REVERSE-TRACKDIR (2026-09-11, `6cde3fb8`): cuando el
siguiente tile deja de ser navegable, el controlador enumera los tres
`Trackdir` que pueden recibirse desde el lado de retorno, descarta las bocas
desconectadas y elige la salida con ruta acuática más corta hacia el destino;
si ninguna completa el camino, aplica un desempate estable por track. La
selección usa la misma tabla nativa de entrada/salida y conserva la rotación
gráfica separada mientras detiene, actualiza `ship_track`/`ship_state` y limpia
la caché. Se agregó una regresión donde la salida recta queda bloqueada y el
barco debe tomar un giro lateral conectado. Core queda en `2451 passed; 0
failed; 1 ignored` y cliente en `1388 passed; 0 failed; 2 ignored`, con
formato, `git diff --check` y Clippy estricto limpios. #567 sigue abierta por
callbacks navales, la equivalencia completa de YAPF/trackdirs y aceptación
visual manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-STRUCTURE-CLEAR (2026-09-11,
`6e46b15d`): el preview y la ejecución ya no sobrescriben una esclusa ni una
sección de depósito naval existente. Ambas son `MP_WATER` con `WaterClass`
válida y por eso pasan `HasTileWaterGround`, pero `ClearTile_Water | Auto` las
rechaza con `BUILDING_MUST_BE_DEMOLISHED`; el port replica ahora ese rechazo y
lo prueba en los cuatro ejes sin mutar huella, pool ni dinero. Core queda en
2410 pasadas y 1 ignorada; cliente en 1383 pasadas y 2 ignoradas, con Clippy
estricto y formato verdes. #567 sigue abierta por el auto-clear de objetos,
estaciones e industrias sobre agua, callbacks/vecinos y aceptación visual en
Weston; #326 permanece abierta.

Actualización #329/#567-VEHICLE-LENGTH-VERSION (2026-09-11, `1c042321`): la
longitud NewGRF ya respeta la frontera nativa de versión: GRF < 8 consulta
CB11, mientras GRF ≥ 8 consulta `PROP_TRAIN_SHORTEN_FACTOR`/`PROP_ROADVEH_SHORTEN_FACTOR`
mediante CB36 y vuelve a Action0 sin mezclar el callback antiguo. La
reconstrucción de `ConsistChanged` vuelve a evaluar las unidades NewGRF antes
de sumar longitud, corrigiendo también cadenas importadas que arrancaban con
el valor por defecto. Se agregaron regresiones para ambas versiones, el
fallback inválido de CB36 y la importación; core queda en `2470 passed; 0
failed; 1 ignored` y cliente en `1392 passed; 0 failed; 2 ignored`, con
Clippy estricto, formato y suites completas limpios. #329/#567 siguen abiertas
por callbacks avanzados, `NoNews`/`NoPreview`/`JoinPreview` y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-OBJECT-WATER-GROUND (2026-09-10, `ad80376f`): el
renderer de objetos NewGRF ya interpreta `ObjectFlag::DrawWater` (bit 10) y el
ground directo `SPR_FLAT_WATER_TILE` como `DrawWaterClassGround` cuando
`MP_OBJECT` conserva una clase Sea/Canal/River válida en `M1`. La emisión
reutiliza la pasada acuática existente, incluidos superficie, diques/bordes y
reemplazos Action2/Action5; en tierra conserva el ground del layout y los
anclajes NFO. La regresión ECS de un objeto sobre Canal verifica una superficie
y ocho diques exteriores, y la suite cliente queda en 1351 pasadas y 2
ignoradas; Clippy estricto, formato y `diff --check` pasan. #567 sigue abierta
por la matriz completa de objetos en Sea/Canal/River, layouts directos y
pendientes, callbacks, clipping, orden global y framebuffer; #326 permanece
abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-MATRIX (2026-09-10, `56ae6016`): la
regresión ECS del depósito naval ejerce explícitamente `DrawWaterClassGround`
para Sea, Canal y River plano: Sea no emite diques, Canal emite los ocho
bordes exteriores y River conserva el ground plano sin diques. Otra regresión
con alturas reales del mapa cubre las cuatro pendientes River (`SE`, `NE`, `SW`,
`NW`) y verifica que cada una selecciona su sprite estático correcto antes de
las capas `TILE_SEQ` del depósito. El bloque focal queda en 11 pruebas, la
suite cliente en 1353 pasadas y 2 ignoradas; Clippy estricto, formato y
`diff --check` pasan. La evidencia de callbacks/Action2 completos, vecinos
para toda la matriz, clipping, orden global y framebuffer sigue pendiente;
#567 y #326 permanecen abiertos.

Actualización #326/#567-SHIP-DEPOT-CANAL-CALLBACK (2026-09-10, `bdfeb8bf`): la
regresión ECS conecta un `CanalFeatureDef` runtime-only con
`CBID_CANALS_SPRITE_OFFSET` y verifica que el delta se aplica al ground plano
`CF_WATERSLOPE` y a cada uno de los ocho diques emitidos por un depósito Canal.
Las vistas seleccionadas por callback conservan sus bytes RGBA y pasan por la
caché compartida de imágenes, sin perder la estructura `TILE_SEQ`; el bloque
focal queda en 12 pruebas y la suite cliente continúa en 1353 pasadas y 2
ignoradas, con Clippy estricto, formato y `diff --check` verdes. #567 sigue
abierta por callbacks/scopes restantes, vecinos de toda la matriz, clipping,
orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-RIVER-CALLBACK (2026-09-10, `993204cc`): la
regresión ECS conecta `CBID_CANALS_SPRITE_OFFSET` a `CF_RIVER_SLOPE` y
`CF_RIVER_EDGE` en un depósito River inclinado. El ground conserva el índice
de pendiente más el delta del callback y los ocho bordes conservan el bloque
River `24..31` antes de aplicar el mismo delta, sin perder las capas
`TILE_SEQ`. El bloque focal queda en 14 pruebas y la suite cliente queda en
1356 pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check`
verdes. #567 sigue abierta por variables/scopes y callbacks restantes,
costas/túneles/puentes, vecinos de toda la matriz, clipping, orden global y
framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-RIVER-CONCAVE-EDGES (2026-09-10, `f1e5bb38`):
la regresión ECS agrega vecinos River a dos lados del depósito y una esquina
diagonal seca. Verifica que `DrawWaterEdges` suprime los lados compartidos,
conserva los slots exteriores y emite la esquina cóncava, aplicando el
callback de offset a cada slot sin alterar el ground ni la estructura
`TILE_SEQ`. El bloque focal queda en 15 pruebas y la suite cliente en 1357
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#567 sigue abierta por variables/scopes y callbacks restantes,
costas/túneles/puentes, vecinos de toda la matriz, clipping, orden global y
framebuffer; #326 permanece abierta.

Actualización #326/#567-OBJECT-WATER-CLASS-MATRIX (2026-09-10, `63599f24`):
la regresión ECS recorre un objeto NewGRF con `ObjectFlag::DrawWater` sobre
Sea, Canal y River. Cada clase conserva una superficie acuática; sólo Canal
emite los ocho diques exteriores y el ground rojo del layout no vuelve a
materializarse. El bloque focal de objetos de agua queda en 2 pruebas y la
suite cliente en 1358 pasadas y 2 ignoradas, con Clippy estricto, formato y
`diff --check` verdes. #567 sigue abierta por layouts directos y pendientes,
callbacks, vecinos, clipping, orden global y framebuffer; #326 permanece
abierta.

Actualización #326/#567-OBJECT-WATER-DIRECT-GROUND (2026-09-10, `5592e80c`):
la regresión ECS cubre el segundo contrato de `DrawNewObjectTile`: un layout
sin `ObjectFlag::DrawWater` cuyo ground directo es `SPR_FLAT_WATER_TILE`.
Sobre Sea, Canal y River emite una superficie; sólo Canal conserva los ocho
diques exteriores y el sprite de fallback rojo no se materializa. El bloque
focal de objetos de agua queda en 3 pruebas y la suite cliente en 1359
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#567 sigue abierta por layouts complejos, callbacks/scopes, vecinos, clipping,
orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-OBJECT-WATER-CANAL-CALLBACK (2026-09-10, `f3cfca8e`):
la regresión ECS conecta `CBID_CANALS_SPRITE_OFFSET` a un objeto NewGRF sobre
Canal y verifica el delta en `CF_WATERSLOPE` y `CF_DIKES`. La superficie usa
el slot seleccionado por callback, los ocho diques conservan sus slots, y el
ground Action1 rojo del objeto no reemplaza `DrawWaterClassGround`. El bloque
focal de objetos de agua queda en 4 pruebas y la suite cliente en 1360
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#567 sigue abierta por callbacks/scopes adicionales, layouts complejos,
vecinos, clipping, orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-OBJECT-WATER-RIVER-EDGE-CALLBACK (2026-09-10, `82cc74da`):
la regresión ECS conecta `CBID_CANALS_SPRITE_OFFSET` a un objeto NewGRF sobre
River plano y verifica `CF_RIVER_SLOPE` junto con `CF_RIVER_EDGE`. La superficie
custom conserva el delta del callback y los ocho bordes exteriores consumen
los slots River desplazados, sin materializar el ground Action1 rojo del
objeto. El bloque focal de objetos de agua queda en 5 pruebas y la suite
cliente en 1361 pasadas y 2 ignoradas, con Clippy estricto, formato y
`diff --check` verdes. #567 sigue abierta por variables/scopes adicionales,
layouts complejos, vecinos, clipping, orden global y framebuffer; #326
permanece abierta.

Actualización #326/#567-SHIP-DEPOT-GLOBAL-SORT-MATRIX (2026-09-10, `96731e50`):
la regresión ECS ejerce las cuatro combinaciones `Axis::X/Y` y parte
norte/sur del depósito naval. Las piezas `4072`, `4074+4070`, `4073` y
`4075+4071` conservan sus bounds `TILE_SEQ_LINE` (`16×1` o `1×16`),
`insertion_key` consecutivo y `source_depth` igual al transform antes del
sort global. El bloque focal de depósito naval queda en 16 pruebas y la suite
cliente en 1362 pasadas y 2 ignoradas, con Clippy estricto, formato y
`diff --check` verdes. #567 sigue abierta por aceptación visual/framebuffer,
vecinos y callbacks/variables restantes; #326 permanece abierta.

Actualización #326/#566-EMPTY-PARENT-CLIPPING (2026-09-10, `9857025a`): el
alcance preciso del viewport reconstruye los extremos exclusivos de los
prismas inclusivos antes de proyectarlos, igual que `AddSortableSpriteToDraw`
para `SPR_EMPTY_BOUNDING_BOX`. La regresión cubre una caja 1×1×1 que cruza el
borde y otra que sólo lo toca; también conserva el caso de extent cero y el
fallback 3D antes de materializar un asset. La suite cliente queda en 1362
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#566 sigue abierta por pivotes, clipping por familias y framebuffer; #567
continúa abierta por la aceptación visual del depósito naval y #326 por la
composición global completa.

Actualización #326/#563-ROAD-WAYPOINT-FALLBACK-ORDER (2026-09-10,
`5e183635`): cuando un `TileLayout` de waypoint vial no es materializable, la
ruta vanilla conserva ahora los ordinales locales `2/3` de sus dos líneas
`TILE_SEQ`, en vez de reutilizar `12/13`, que sólo corresponden al layout
custom unido al stream global después de la catenaria. La regresión ECS ejerce
el fallback con los sprites `6143/6144`, comprueba ambas `insertion_key` y
separa el suelo de carretera del suelo de una parada. La suite cliente queda
en 1363 pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check`
verdes. #563 sigue abierta por catenaria/trace, vecinos, clipping, matriz de
partes/callbacks, orden global completo y framebuffer; #326 permanece abierta.

Actualización #326/#563-ROAD-CATENARY-LOW-BRIDGE (2026-09-10, `caa95f0c`):
`DrawRoadTypeCatenary` ya descarta los cuatro recortes de catenaria vial bajo
un puente bajo cuando `deck_z <= GetTileMaxZ + 1`, incluyendo carreteras
normales, paradas y waypoints; el modo transparente conserva la excepción del
renderer C++. La regresión ECS usa una carretera electrificada bajo un puente
vial X y verifica que no se publiquen los parents `6071/6043`. La suite cliente
queda en 1364 pasadas y 2 ignoradas, con Clippy estricto, formato y
`diff --check` verdes. #563 sigue abierta por trace/catenaria restante,
vecinos, clipping, matriz de partes/callbacks, orden global completo y
framebuffer; #326 permanece abierta.

Actualización #326/#563-ROAD-CATENARY-MAY-HAVE-ROAD (2026-09-10,
`aef62b1c`): la selección de vecinos de `DrawRoadTypeCatenary` sigue ahora la
clasificación `MayHaveRoad` de OpenTTD: cuenta carreteras, depósitos, túneles y
puentes viales, pero sólo estaciones de carretera (bus, camión o waypoint),
en vez de tratar cualquier estación como brazo vial. La regresión de máscara
combina un vecino normal y un puente vial electrificado y verifica que el
cruce conserve exactamente esos dos brazos. La suite cliente queda en 1365
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#563 sigue abierta por trace/catenaria restante, callbacks, clipping, matriz
de partes, orden global completo y framebuffer; #326 permanece abierta.

Actualización #326/#563-ROAD-CATENARY-LEVEL-CROSSING (2026-09-10,
`7778e4a7`): la ruta vial vuelve a ejecutar `DrawRoadCatenary` para
`RoadTileType::Crossing`, después de que la fundación nivelada haya elegido el
suelo del cruce. La regresión ECS usa un cruce `ROAD_X` con roadtype
electrificado y verifica sus tres recortes traseros más el frente (`6071/6043`).
La suite cliente queda en 1366 pasadas y 2 ignoradas, con Clippy estricto,
formato y `diff --check` verdes. #563 sigue abierta por trace/catenaria
restante, callbacks, clipping, matriz de partes, orden global completo y
framebuffer; #326 permanece abierta.

Actualización #326/#563-ROAD-CATENARY-ROADWORKS (2026-09-10, `0637e32f`):
la carretera normal con `Roadside::GrassRoadWorks` o
`Roadside::PavedRoadWorks` ya corta la catenaria antes de emitirla, igual que
el retorno temprano de `DrawRoadBits` en OpenTTD. La regresión ECS verifica que
una carretera `ROAD_X` en obras no publique los parents `6071/6043`; el cruce
a nivel conserva su rama independiente. La suite cliente queda en 1367
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
La textura de excavación `1414/1415` y su evidencia raster quedan como corte
separado; #563 y #326 permanecen abiertas.

Actualización #326/#563-ROAD-WORKS-EXCAVATION (2026-09-10, `a008c3d6`):
la ruta normal de `DrawRoadBits` emite ahora `SPR_EXCAVATION_X/Y` (`1414/1415`)
según la unión de roadbits y tram bits, después de los overlays y antes del
retorno temprano que evita catenaria y detalles de roadside. La posición usa
el metadato NFO `39×21, -18,5`, sigue al parent de la foundation y el atlas de
runtime/test precarga explícitamente ambos IDs. La regresión ECS verifica la
textura X junto con la ausencia de `6071/6043`; la suite cliente queda en 1368
pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check` verdes.
#563 y #326 permanecen abiertas por la matriz completa, callbacks, trace,
clipping, orden global y framebuffer.

Actualización #326/#563/#565-PURE-TRAM-UNDERLAY (2026-09-10, `724ccabd`): la
ruta `DrawRoadBits` distingue ahora el cero real de `GetRoadBits(RTT_ROAD)`
del fallback geométrico y calcula la fundación con `road | tram`. Cuando la
tesela conserva `INVALID_ROADTYPE`, el suelo base vuelve a césped y el trazado
usa `SPR_TRAMWAY_TRAM + GetRoadSpriteOffset`; `tram_flat_*` queda reservado al
overlay de carretera + tranvía. La regresión ECS cubre `ROAD_X`, anclas del
underlay y ausencia de ambos sprites equivocados; la suite cliente queda en
1369 pasadas y 2 ignoradas, con Clippy estricto, formato y `diff --check`
verdes. #563, #565 y #326 permanecen abiertas por pendientes completas,
callbacks, vecinos, clipping, orden global y framebuffer.

Actualización #326/#567-WATERED-TREE-EDGE (2026-09-10, `799b2f82`):
`IsWateredTile` ya conserva el `default` de OpenTTD para `MP_TREES`: una
tesela de bosque con `WaterClass` válida no suprime el lado ni la esquina del
`DrawWaterEdges` vecino. La regresión ejerce Sea/Canal/River junto a un
depósito Canal y verifica que el dique exterior permanezca visible. La suite
cliente queda en 1370 pasadas y 2 ignoradas, con formato y Clippy estricto
verdes; #567 y #326 siguen abiertas por costa, túneles/puentes, callbacks y
scopes restantes, clipping, orden global y framebuffer.

Actualización #326/#567-CANAL-RANDOM-WATER-SCOPE (2026-09-10, `4f47956b`):
`CanalScopeResolver` ya expone los bits aleatorios de `m4` sólo para las
teselas cuyo tipo nativo es `MP_WATER`; `ShipDepot` conserva ese origen,
mientras estaciones, industrias, objetos y bosques reciben cero en `0x83`.
La regresión recorre ambos casos y evita que una rama Action2 de agua
construida cambie por bytes residuales de otra familia. La suite cliente queda
en 1371 pasadas y 2 ignoradas, con formato y Clippy estricto verdes; #567 y
#326 siguen abiertas por variables/scopes y callbacks restantes, costa,
vecinos, clipping, orden global y framebuffer.

Actualización #326/#567-SHIP-DEPOT-RAW-CONTRACT (2026-09-10, `4a18ae25`):
la construcción de depósitos navales escribe ahora el contrato vigente de
OpenTTD: `MP_WATER` conserva el nibble climático de `MAPT`, `m5[4..=7]` queda
en `WaterTileType::Depot` (`0x30`) y los bits bajos mantienen la orientación.
El propietario activo se escribe en los cinco bits bajos de `m1` sin destruir
`WaterClass`; el resolver común de ownership también ignora esos bits altos.
Las regresiones cubren construcción con rival, Canal y zona climática, y la
suite core queda en 2388 pasadas y 1 ignorada. #567 sigue abierta por la
huella de dos teselas completa, vecinos/callbacks, clipping, orden global y
framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-DIRECTION-ENCODING (2026-09-10, `d45899ef`):
la orientación de la boca ya se traduce al contrato `part/eje` de OpenTTD
mediante la tabla inversa de `XYNSToDiagDir`: `dir 0..3` escribe `m5` bajo
`0, 3, 1, 2`, respectivamente, manteniendo `WaterTileType::Depot` en
`0x30`. La regresión cubre los cuatro valores y la normalización de entradas
fuera de rango; la suite core queda en 2389 pasadas y 1 ignorada, con Clippy
estricto y formato verdes. #567 sigue abierta por la huella 2×1/1×2 completa,
vecinos/callbacks, clipping, orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-FOOTPRINT (2026-09-10, `604f671c`): la
construcción de `CmdBuildShipDepot` ya materializa las dos partes contiguas de
la huella 2×1/1×2. La validación comprueba base, parte opuesta y boca antes de
mutar; cada tesela conserva su nibble climático y `WaterClass`, recibe el
`owner` activo y normaliza `m2/m3/m4/m6/m7/m8` como `MakeShipDepot`. Las
regresiones cubren los cuatro ejes, payload residual y rechazo atómico de una
segunda tesela ocupada. La suite core queda en 2391 pasadas y 1 ignorada, con
Clippy estricto, formato y `diff --check` verdes. #567 sigue abierta por
demolición de ambas partes, pool `DepotID`, vecinos/callbacks, preview,
clipping, orden global y framebuffer; #326 permanece abierta.

Actualización MENU-INTRO-SHOWCASE (2026-09-11, `76c41084`): se reemplazó el
fondo procedural escaso del menú por el showcase determinista 64×64 y se
materializa su flota inicial sin ejecutar simulación. El tráfico animado cubre
bus, camión, tren, barco, avión y una línea maglev aislada con railtype explícito;
el paneo queda acotado para conservar los elementos alrededor del panel. La
suite cliente queda en 1373 pasadas y 2 ignoradas, con Clippy estricto verde.
La comprobación visual requiere una sesión Weston/X11 accesible al proceso;
esto no altera los criterios de cierre de #326 ni #567.

Actualización #326/#567-SHIP-DEPOT-CLEAR-CANONICAL (2026-09-11, `c794daf2`):
`ClearTile` ya trata el depósito naval como una huella indivisible: una orden
emitida sobre cualquiera de sus dos teselas valida ownership, ocupación y
vecindad, restaura ambas aguas conservando Sea/Canal/River y cobra
`PR_CLEAR_DEPOT_SHIP` (90). `DepotSpatialIndex`, la búsqueda Manhattan y el BFS
naval sólo exponen la sección norte canónica, evitando depósitos duplicados o
referencias a la mitad sur. Se agregó el error de vehículo ocupando la huella y
regresiones para las dos entradas y las clases de agua. La suite core queda en
2395 pasadas y 1 ignorada; la cliente en 1373 pasadas y 2 ignoradas, con Clippy
estricto del cliente verde. #567 sigue abierta por el pool `DepotID`, vecinos y
callbacks, preview, clipping, orden global y framebuffer; #326 permanece
abierta.

Actualización #326/#567-SHIP-DEPOT-PREVIEW-FOOTPRINT (2026-09-11, `b9c5c810`):
el preview de `ClearTile` reutiliza ahora la validación de `RemoveShipDepot`:
si el cursor cae en cualquiera de las dos secciones, el HUD comprueba ambas
propiedades y ambas ocupaciones antes de anunciar la acción. La regresión
confirma que un barco en la sección opuesta produce `VehicleInTheWay` tanto en
preview como en ejecución, sin mutar mapa ni dinero. Las pruebas focales y
Clippy estricto del cliente pasan. #567 sigue abierta por el pool `DepotID`,
vecinos/callbacks, preview visual, clipping, orden global y framebuffer; #326
permanece abierta.

Actualización #326/#567-SHIP-DEPOT-ID-POOL (2026-09-11, `d09d51da`): los
depósitos de carretera, ferrocarril y barcos asignan ahora el primer `DepotID`
libre del pool común nativo y lo escriben en los dos bytes de `MAP2`. Las dos
secciones de un depósito naval comparten la misma ID; la consulta deduplica la
huella y excluye correctamente los hangares de aeropuerto, que pertenecen a
estaciones. El preview comparte la comprobación de capacidad y la UI informa
el agotamiento del pool en ambos idiomas. La suite core queda en 2398 pasadas
y 1 ignorada; la cliente en 1373 pasadas y 2 ignoradas, con Clippy estricto
verde. #567 sigue abierta por el pool persistente/tabla `DEPT`, metadatos de
depósito, vecinos/callbacks, preview visual, clipping, orden global y
framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-STATE-METADATA (2026-09-11, `00cb3476`):
el estado jugable ya conserva una instancia `SavDepot` por cada depósito de
carretera, ferrocarril o barco. La construcción registra `DepotID`,
`Depot::xy` y la fecha absoluta nativa; la demolición elimina la fila sólo
después de limpiar con éxito, incluida la huella completa de dos teselas del
depósito naval. El contrato JSON usa `#[serde(default)]` para migrar partidas
propias anteriores. La suite core queda en 2398 pasadas y 1 ignorada; la
cliente en 1373 pasadas y 2 ignoradas, con Clippy estricto verde. #567 sigue
abierta hasta conectar estas filas con la tabla `DEPT` de SAV y completar
nombre/pueblo/callbacks, preview visual, clipping, orden global y framebuffer;
#326 permanece abierta.

Actualización #329/#567-VEHICLE-REFIT-CAPACITY (2026-09-11, `428c2ef9`):
CB15 ya se trata como capacidad final cuando el cargo difiere del declarado por
Action0 o existe un subtipo activo. Su resultado tiene prioridad sobre CB36 en
`ConsistChanged`, autoreemplazo, piezas articuladas, refit manual y auto-refit
de estación; después de un refit se recalcula la capacidad agregada de la
cabeza del consist. `CALLBACK_FAILED` cae a Action0/CB36 y cero sigue siendo
válido. Core queda en `2472 passed; 0 failed; 1 ignored` y cliente en `1392
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios. Falta `NoDefaultCargoMultiplier`, selección completa de subtipos,
capacidad secundaria de aeronaves, balanceo completo de consist, APIs legacy
sin catálogo y aceptación visual manual bajo Weston; #329/#567 siguen abiertas
y #326 permanece abierta.

Actualización #329/#567-SHIP-LONG-INTRO (2026-09-11, `48bfc684`): Action0
naval `0x1A` ya lee la fecha larga de introducción (`DWORD`, días desde la
época NewGRF), la convierte al año del catálogo y la conserva al aplicar el
GRF. La regresión verifica tanto el parser como el `EngineDef` resultante.
Core queda en `2455 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0
failed; 2 ignored`, con formato, `git diff --check` y Clippy estricto limpios.
#329/#567 siguen abiertas por callbacks y propiedades navales restantes, y por
aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-PURCHASE-ORDER (2026-09-11, `0982aacc`): Action0
naval `0x1B` ya conserva el `ExtendedByte` que indica el ID local delante del
que debe insertarse el barco. Las operaciones se aplican después de cargar
todo el stack, buscando por `(tipo, GRFID, ID local)` para que los destinos
posteriores también sean válidos. La regresión cruza dos barcos y verifica el
orden final del catálogo. Core queda en `2458 passed; 0 failed; 1 ignored` y
cliente mantiene `1388 passed; 0 failed; 2 ignored`, con formato, `git
diff --check` y Clippy estricto limpios. #329/#567 siguen abiertas por
callbacks y propiedades navales restantes, y por aceptación visual manual bajo
Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-RETIRE-EARLY (2026-09-11, `4c18b55d`): Action0
naval `0x16` ya conserva los años de retiro anticipado. `engine_available_in_year`
los descuenta de la vida del modelo para la compra y el autoreemplazo, con
`0` como fallback compatible para saves/catálogos anteriores. La regresión
combina vida de modelo de 10 años con retiro de 3 y verifica el límite de
disponibilidad. Core queda en `2457 passed; 0 failed; 1 ignored`; cliente
mantiene `1388 passed; 0 failed; 2 ignored`, con formato, `git diff --check` y
Clippy estricto limpios. #329/#567 siguen abiertas por callbacks y
propiedades navales restantes, y por aceptación visual manual bajo Weston;
#326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-DEPT-PERSISTENCE (2026-09-11, `e5a44e76`):
las filas de `SavDepot` ya se cargan y escriben en la tabla nativa `DEPT`,
conservando el índice denso `DepotID`, `Depot::xy`, `REF_TOWN` moderno y
`town_index` legado, `town_cn`, nombre y `build_date`. Un round-trip sin
mutaciones reutiliza el chunk original para preservar columnas futuras; si el
estado cambia, el escritor reconstruye sólo las columnas semánticas y mezcla
las desconocidas. Estados JSON antiguos que sólo tienen `MAP2` obtienen filas
mínimas derivadas del mapa, sin perder la identidad del pool ni la huella
canónica del depósito naval. La suite core queda en 2401 pasadas y 1 ignorada;
la cliente en 1373 pasadas y 2 ignoradas, con Clippy estricto verde. #567 sigue
abierta por mutaciones/UI de nombre y pueblo generado, callbacks/vecinos,
preview visual, clipping, orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-TOWN-ORDINAL (2026-09-11, `1ebceb02`):
la creación de depósitos ya replica la parte dinámica de `MakeDefaultName`:
elige el pueblo más cercano y el primer `town_cn` libre dentro de ese pueblo y
tipo de transporte. Dos depósitos viales cercanos reciben ordinales 0 y 1;
un depósito ferroviario o naval no consume esos ordinales. La demolición y la
tabla `DEPT` ya parten de la misma fila persistente. La suite core queda en
2402 pasadas y 1 ignorada, con Clippy estricto del cliente verde. #567 sigue
abierta por el comando de renombrado, resolución completa de callbacks y
vecinos, preview visual, clipping, orden global y framebuffer; #326 permanece
abierta.

Actualización #326/#567-SHIP-DEPOT-NATIVE-TITLE (2026-09-11, `2a731579`):
la ventana de depósito consulta el `DepotID` de `MAP2`, también al seleccionar
la mitad sur de un depósito naval. Si existe nombre custom lo muestra; si no,
resuelve `pueblo + tipo + ordinal` en el locale activo, y conserva el fallback
con coordenadas para estados antiguos sin fila `DEPT`. La suite cliente queda
en 1374 pasadas y 2 ignoradas; Clippy estricto y formato pasan. #567 sigue
abierta por renombrado interactivo, callbacks/vecinos y aceptación visual de
la huella naval, clipping, orden global y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-RENAME (2026-09-11, `ddf89531`): el nuevo
`Command::RenameDepot` replica `CmdRenameDepot`: valida propiedad, límite nativo
de 31 caracteres, nombres custom únicos y reset al nombre generado con el
ordinal libre correcto. La ventana de depósito incorpora el campo editable,
botones OK/No, Enter/Escape y funciona al seleccionar cualquiera de las dos
secciones de un depósito naval. La mutación queda en `SavDepot` y por tanto se
persiste por `DEPT`; también migra perezosamente depósitos de JSON antiguos que
sólo conservaban `MAP2`. Hay regresiones de núcleo y UI para nombre, duplicado,
límite, reset y huella naval. La suite core queda en 2404 pasadas y 1 ignorada;
la cliente en 1375 pasadas y 2 ignoradas, con Clippy estricto y formato verdes.
#567 sigue abierta por callbacks/vecinos, preview visual, clipping, orden global
y framebuffer; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-BUILD-PREVIEW (2026-09-11, `1545eb34`):
el preview de construcción naval dejó de usar el sprite fijo `ship_depot_ne`.
`PreviewPlan::ShipDepot` conserva la orientación del comando, calcula la
huella de dos teselas mediante `ship_depot_footprint` y valida ambas partes.
Las tres capas de cada depósito completo (una sección norte y dos de la boca)
usan los mismos anclajes, remap y extensiones `TILE_SEQ_LINE` que el renderer
del mapa, en las cuatro orientaciones; una sección fuera del mapa se omite
visualmente pero mantiene el preview inválido. La tabla de seis sprites y la
geometría quedan compartidas entre runtime y ghost para evitar divergencias de
clipping local. Core queda en 2405 pasadas y 1 ignorada; cliente en 1381
pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta
por callbacks/vecinos, clipping de viewport, orden global y framebuffer;
#326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-PREVIEW-SORT (2026-09-11, `ad1d6b38`):
las capas del fantasma naval ahora entran en `ViewportSortableParent` con los
prismas `TILE_SEQ_LINE`, `viewport_insertion_key` y `viewport_source_depth`
del compositor global. Esto conserva el cruce correcto con edificios,
puentes y vías cercanas durante la construcción; la geometría de bounds se
regresiona contra las cuatro orientaciones y el runtime. La suite cliente
queda en 1382 pasadas y 2 ignoradas, con Clippy estricto y formato verdes.
#567 sigue abierta por callbacks/vecinos, clipping de viewport y framebuffer;
#326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-PREVIEW-ASSET-PARITY (2026-09-13): el ghost
naval consume ahora las mismas entradas `WorldAssets.ship_depot` del atlas y la
misma ruta de paleta de compañía que `DrawWaterDepot`; sólo conserva PNG suelto
como fallback cuando el recurso de mundo no está disponible en pruebas o
arranque. Así la geometría ya compartida no vuelve a divergir por recorte,
tamaño de entrada o recolor. #567 sigue abierta por callbacks/vecinos,
validación visual bajo Weston, clipping integrado y framebuffer; #326
permanece abierta.

Actualización #326/#567-SHIP-DEPOT-EDGE-CLIPPING (2026-09-11, `b104f9e1`):
runtime y preview naval ahora convierten `TILE_SEQ_LINE` mediante el mismo
helper de bounds inclusivos. Se agregó una regresión sobre las cuatro
orientaciones con huellas válidas que alcanzan los extremos este, norte, oeste
y sur de un mapa 16×16: las 12 capas se conservan y las 6 que tocan el límite
no se descartan por un `-1` de extensión o clipping. La suite cliente queda en
1383 pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue
abierta por callbacks/vecinos, validación visual en Weston, clipping integrado
con el framebuffer y la matriz completa de paridad; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-MAP-EDGE-ATOMIC (2026-09-11,
`9704a135`): el contrato de construcción naval ahora tiene regresión para las
cuatro orientaciones cuando la segunda tesela cae fuera de un mapa 4×4. El
preview y la ejecución devuelven `OutOfBounds` sin escribir la primera sección,
crear una fila de `DepotID` ni cobrar dinero. La suite core queda en 2406
pasadas y 1 ignorada; #567 sigue abierta por callbacks/vecinos restantes,
aceptación visual en Weston, clipping integrado con framebuffer y matriz
completa; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-FOOTPRINT-CONTRACT (2026-09-11,
`8e7ef38a`): se eliminó la condición no nativa que exigía una tercera tesela
de agua delante de la boca. La referencia `CmdBuildShipDepot` sólo valida agua
en las dos teselas de la huella; ahora preview y ejecución aceptan esa misma
configuración en los cuatro ejes, manteniendo el rechazo de tierra y bordes.
La suite core queda en 2407 pasadas y 1 ignorada; la cliente en 1383 pasadas y
2 ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta por
callbacks/vecinos restantes, aceptación visual en Weston, clipping integrado
con framebuffer y matriz completa; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-SITE-CONTRACT (2026-09-11, `a5c88094`):
la validación de construcción naval ahora replica las fases de
`CmdBuildShipDepot`: después de comprobar agua en ambas partes, rechaza
`IsBridgeAbove` en cualquiera de ellas y exige `IsTileFlat` para las dos. El
preview y la ejecución comparten el mismo resultado y no mutan mapa, pool ni
dinero cuando fallan. Se agregaron mensajes ES/EN y regresiones para ambas
partes en los cuatro ejes. Core queda en 2409 pasadas y 1 ignorada; cliente en
1383 pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue
abierta por la semántica completa de `HasTileWaterGround`/auto-clear,
callbacks/vecinos, aceptación visual en Weston y matriz completa; #326
permanece abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-OBJECT-CLEAR (2026-09-11,
`9028c101`): `CmdBuildShipDepot` ya acepta `MP_OBJECT` con
`HasTileWaterGround` cuando su spec declara `Autoremove`. El port comparte el
preflight con `command_would_fail`, deduplica una huella multitile y restaura
cada parte con su `WaterClass` antes de escribir el depósito; también corrige
la lectura de ownership ignorando los bits altos de `MAPO`. Objetos no
autoremovibles siguen devolviendo `ObjectInTheWay` sin mutar mapa, pool ni
dinero. Core queda en 2412 pasadas y 1 ignorada; cliente en 1383 pasadas y 2
ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta por
estaciones/industrias acuáticas, callbacks/vecinos y aceptación visual en
Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-STATION-CLEAR (2026-09-11,
`7a06dcdf`): la fase de auto-clear de `CmdBuildShipDepot` ahora interpreta el
`StationType` crudo de cada `MP_STATION` acuática. Una boya devuelve
`BuoyInTheWay`, un muelle `MustDemolishDockFirst` y una plataforma petrolera
`OilRigInTheWay`, como las rutas específicas de `ClearTile_Station(...,
Auto)`; otros tipos de estación conservan el bloqueo estructural. Preview y
ejecución usan la misma clasificación y no mutan mapa, pool ni dinero al
fallar. Se agregaron mensajes ES/EN y una regresión para ambas partes de la
huella. Core queda en 2413 pasadas y 1 ignorada; cliente en 1383 pasadas y 2
ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta por
industrias acuáticas, callbacks/vecinos y aceptación visual en Weston; #326
permanece abierta.

Actualización #326/#567-SHIP-DEPOT-WATER-INDUSTRY-CLEAR (2026-09-11,
`e2f86f69`): `ClearTile_Industry(..., Auto)` queda representado en el port con
`IndustryInTheWay` cuando una industria ocupa una tesela de agua de la huella
naval. La previsualización y la ejecución comparten el rechazo, conservan la
tesela original y no cobran ni crean el depósito; se agregó cobertura para
ambas partes. Core queda en 2414 pasadas y 1 ignorada; cliente en 1383
pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue
abierta por callbacks/vecinos, contratos de plataforma petrolera y aceptación
visual en Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-DOCKING-NEIGHBORS (2026-09-11,
`e54e29bc`): la construcción y la limpieza del depósito naval vuelven a
ejecutar el equivalente raw de `CheckForDockingTile` para sus dos secciones.
El bit `DockingTile` se recalcula en las cuatro orientaciones frente a la parte
acuática de un muelle, una tesela `MP_STATION` de oil rig o una industria cuyo
`IndustryID` tiene estación neutral enlazada; el resultado se prueba también
con el vecino presente al limpiar. No se altera todavía la selección completa
de destinos del pathfinder. Core queda en 2416 pasadas y 1 ignorada; cliente en
1383 pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue
abierta por consumo del estado de docking en navegación, callbacks restantes y
aceptación visual en Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DEPOT-DOCKING-DESTINATION (2026-09-11,
`c3f384e4`): las órdenes navales a un muelle u oil rig ahora prefieren, cuando
existe, la tesela de agua adyacente marcada `DockingTile`, igual que
`YapfShip::PfDetectDestinationTile`; la selección es determinista por
distancia a la unidad y las boyas conservan su destino directo. Los saves
legacy sin marcador mantienen el fallback al ancla de estación. Se agregaron
regresiones de selección y fallback. Core queda en 2418 pasadas y 1 ignorada;
cliente en 1383 pasadas y 2 ignoradas, con Clippy estricto y formato verdes.
#567 sigue abierta por identidad completa de estación en el pathfinder,
callbacks restantes y aceptación visual en Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-TWO-TILE-FOOTPRINT (2026-09-11,
`cc52ea2f`): `PlaceDock` ahora conserva la huella nativa de dos teselas:
una pieza de tierra (`StationGfx 0..3`) y una pieza acuática
(`StationGfx 4..5`) con el mismo `StationID` en MAP2. La red naval ya no
atraviesa la pieza de tierra; las órdenes y la carga prefieren una tesela de
amarre marcada junto a la parte acuática. La demolición desde cualquiera de
las dos piezas restaura tierra/agua, limpia los metadatos y recalcula vecinos;
los muelles legacy de una sola tesela conservan limpieza de compatibilidad.
Core queda en 2419 pasadas y 1 ignorada; cliente en 1383 pasadas y 2
ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta por
pendientes de pendiente/auto-clear/join de `CmdBuildDock`, reconciliación
completa de identidad en saves importados, preview visual y aceptación en
Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-PREVIEW (2026-09-11, `4bf16be1`): el ghost
de construcción de muelles dejó de usar el sprite genérico 1×1. El plan Bevy
conserva origen, dirección y pieza acuática; el renderer de preview emite la
rampa de tierra y la plataforma de agua con las seis variantes `StationGfx`,
las mismas cajas `TILE_SEQ_LINE` y el mismo ordenamiento que el mapa runtime.
La tesela de aproximación queda libre y un muelle inválido mantiene el tint de
error en ambas piezas. Cliente queda en 1387 pasadas y 2 ignoradas, con Clippy
estricto y formato verdes. La aceptación visual manual bajo Weston sigue
pendiente, igual que pendiente/auto-clear/join de `CmdBuildDock` y la
reconciliación completa de identidad en saves importados; #567 y #326 siguen
abiertas.

Actualización #326/#567-SHIP-DOCK-SLOPE-CONTRACT (2026-09-11, código
`69e0e696`, fixture `6cf84239`):
`CmdBuildDock` ya no acepta una costa plana con una dirección arbitraria. La
tesela de tierra debe tener una pendiente inclinada y su dirección acuática
debe coincidir con `ReverseDiagDir(GetInclinedSlopeDirection(tile))`; las dos
teselas de agua continúan exigiendo planitud. El showcase de inicio usa ahora
dos costas inclinadas reales, por lo que mantiene los puertos visibles con la
misma geometría que el mapa nativo. Se cubren las cuatro orientaciones y el
rechazo de costa plana/dirección incompatible. Core queda en 2421 pasadas y 1
ignorada; cliente en 1387 pasadas y 2 ignoradas, con Clippy estricto y formato
verdes. #567 sigue abierta por auto-clear/join, reconciliación completa de
identidad en saves importados y aceptación visual manual bajo Weston; #326
permanece abierta.

Actualización #326/#567-SHIP-DOCK-AUTOCLEAR (2026-09-11, `0cec5a22`): el
preflight y la ejecución de `PlaceDock` comparten ahora un plan de limpieza
para las dos piezas de la huella. Los objetos `MP_OBJECT` con `Autoremove` se
eliminan una sola vez aunque crucen ambas piezas, conservan la clase de agua y
aplican su coste nativo; los objetos no removibles producen `ObjectInTheWay`
sin mutar mapa, pool ni dinero. El preview Bevy reutiliza la misma validación.
Core queda en 2423 pasadas y 1 ignorada; cliente en 1387 pasadas y 2
ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta por join,
reconciliación completa de identidad en saves importados y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-STATION-JOIN (2026-09-11, `01de28ee`): la
huella física completa de un muelle ahora se agrupa por su estación lógica:
la tierra y el agua comparten `StationID`, los muelles propios adyacentes
reutilizan esa identidad al construir y `JoinStations` puede fusionar dos
estaciones navales respetando su footprint. La fusión reescribe los MAP2 de
ambas piezas, órdenes y destinos compartidos; al demoler el ancla se promueve
otra pieza de tierra y se redirigen las órdenes locales, sin borrar la
estación restante. Se agregaron regresiones para construcción adyacente,
unión explícita y promoción tras demolición. Core queda en 2425 pasadas y 1
ignorada; cliente en 1387 pasadas y 2 ignoradas, con Clippy estricto y formato
verdes. #567 sigue abierta por la selección explícita `station_to_join`, la
reconciliación de estaciones mixtas y saves importados, el pathfinder naval y
la aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-SAV-FOOTPRINT (2026-09-11, `6bd69913`): al
cargar un SAV se reconstruyen todas las piezas `MP_STATION` de muelles por su
`StationID` de `MAP2`, incluyendo varios muelles pertenecientes a la misma
estación lógica. La tierra y el agua quedan en `joined_tiles`, se conserva el
`m2_hi` de IDs nativos y las órdenes, cobertura, pathfinder y demolición ya
pueden consultar la huella importada completa. Se agregó una regresión con dos
muelles separados que comparten estación. Core queda en 2426 pasadas y 1
ignorada; cliente en 1387 pasadas y 2 ignoradas, con Clippy estricto y formato
verdes. #567 sigue abierta por la selección explícita `station_to_join`, la
reconciliación de estaciones mixtas, callbacks navales y la aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-STATEFUL-DESTINATION (2026-09-11,
`5e9a444e`): se agregó una resolución de destino naval que recibe el catálogo
lógico de estaciones y recorre todas las huellas de muelle unidas. La API
legacy conserva el comportamiento del ancla para callers sin `GameState`, y
una regresión confirma que un barco cercano al segundo muelle elige su tesela
de amarre, incluso cuando la orden apunta al primer ancla. Core queda en 2427
pasadas y 1 ignorada; cliente en 1387 pasadas y 2 ignoradas, con Clippy
estricto y formato verdes. #567 sigue abierta por la integración del
pathfinder naval completo, `station_to_join`, callbacks y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-ROUTING-STATE (2026-09-11, `38b9cca8`):
`Vehicle` y la fase de routing del tick usan la resolución stateful al
sincronizar destinos. Esto conecta los muelles múltiples restaurados desde SAV
con la ruta efectiva de barcos activos sin cambiar trenes, carretera, aire ni
los callers legacy. Core queda en 2427 pasadas y 1 ignorada; cliente en 1387
pasadas y 2 ignoradas, con Clippy estricto y formato verdes. #567 sigue abierta
por los puntos de selección explícita y callbacks navales restantes, además de
la aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #326/#567-SHIP-DOCK-RESYNC-CALLERS (2026-09-11, `2d3a29aa`):
las resincronizaciones posteriores a descarga, reversión PBS, mantenimiento,
órdenes compartidas, comandos de flota y reconciliación de vehículos SAV ya no
pueden sobrescribir el destino multi-muelle con la API legacy cuando disponen
de `GameState`. La cobertura queda integrada en el core completo
(`2427 passed; 0 failed; 1 ignored`) y cliente
(`1387 passed; 0 failed; 2 ignored`), con Clippy estricto, formato y diff
limpios. #567 sigue abierta por `station_to_join`, callbacks/pathfinding naval
restantes y validación visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-MIXED-SAV (2026-09-11, `e261921c`): el importador
ahora reconstruye la huella de muelles cuando una fila `STNN` combina
`FACIL_DOCK` con tren, bus o aeropuerto; `dock_station_tiles` filtra las
piezas raw `StationType::Dock` y ya no descarta esa geometría sólo porque
`StopKind` conserve otra facilidad principal. Oil Rig mantiene su tratamiento
especial y no se convierte en un muelle de dos piezas. Se agregó una regresión
SAV con ancla ferroviaria y dos muelles separados que comparten `MAP2 StationID`.
Core queda en `2428 passed; 0 failed; 1 ignored` y cliente en
`1387 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y diff limpios.
#567 sigue abierta por la máscara de facilidades efectiva para servicio naval,
`station_to_join`, callbacks/pathfinding y aceptación visual manual bajo
Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-FACILITY-SERVICE (2026-09-11, `bf334130`):
`Station` conserva ahora el byte nativo `STNN.facilities`; una fila importada
con tren + muelle puede aceptar trenes y barcos aunque `StopKind` mantenga la
facilidad principal ferroviaria. El escritor SAV vuelve a emitir la máscara
completa, mientras estaciones runtime y JSON legacy derivan el valor desde
`StopKind`. Se agregaron regresiones de importación y serialización. Core queda
en `2429 passed; 0 failed; 1 ignored` y cliente en
`1387 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y diff limpios.
#567 sigue abierta por las comprobaciones físicas/carga multi-modal, selección
`station_to_join`, callbacks/pathfinding naval y aceptación visual manual bajo
Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-PHYSICAL-SERVICE (2026-09-11, `a3a1be1e`): la
comprobación física de servicio naval recorre todas las piezas de agua de los
muelles unidos y la carga puede localizar un barco importado desde su tesela
de amarre aunque el índice cubra sólo `MP_STATION`. Las estaciones mixtas
aceptan también carga de pasajeros por su facilidad Dock; las órdenes y el
estado operacional resuelven la entidad por toda su cobertura. Se agregaron
regresiones para ambos muelles y para el fallback de SAV. Core queda en
`2431 passed; 0 failed; 1 ignored` y cliente en `1387 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y diff limpios. #567 sigue abierta por
`station_to_join`, pathfinder/callbacks navales restantes y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-EXPLICIT-JOIN (2026-09-11, `f77aa520`): el
comando de muelle incorpora la variante reproducible
`PlaceDockAtStation { origin, dir, station_to_join }`, equivalente al
`CmdBuildDock` nativo con `station_to_join`. La validación resuelve el
`StationID` contra una estación propia que conserva una huella Dock real,
respeta `distant_join_stations` y rechaza IDs ambiguos o inexistentes antes de
limpiar objetos, modificar el mapa o cobrar. La búsqueda automática también
puede reutilizar una estación intermodal con facilidad naval. Se agregaron
regresiones para unión distante y rechazo atómico. Core queda en
`2433 passed; 0 failed; 1 ignored` y cliente en `1387 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y diff limpios. #567 sigue abierta por
el selector de estación en toolbar/preview, callbacks y pathfinding navales y
la aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-EXPLICIT-JOIN-UI (2026-09-11, `8307a604`): el
panel Station View ofrece `Muelle+` sólo para estaciones que conservan
facility Dock y una huella física naval; fija el `StationID` nativo, cambia al
grupo Agua y emite `PlaceDockAtStation`. El preview Bevy consulta el mismo
comando, por lo que la validez visual refleja la regla de unión distante y no
vuelve a la asociación automática. Se agregó una regresión del constructor de
comandos. Core queda en `2433 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y diff
limpios. #567 sigue abierta por callbacks/pathfinding navales y aceptación
visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-DOCK-PATH-ALTERNATES (2026-09-11, `3e321177`): el
resolver expone todos los `DockingTile` físicos de una estación naval unida y
el routing prueba los amarres alternativos cuando el más cercano no tiene una
ruta navegable. Se elige la ruta alcanzable más corta con desempate estable por
distancia y coordenadas; una regresión cubre un amarre separado por altura sin
esclusa y un segundo muelle conectado. Core queda en
`2434 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y diff limpios. #567 sigue abierta por
callbacks navales y aceptación visual manual bajo Weston; #326 permanece
abierta.

Actualización #567-SHIP-DEPOT-AUTO-SERVICE (2026-09-11, `ceb7a450`): el
servicio automático naval ahora busca depósitos propios dentro de
`MAX_SHIP_DEPOT_SEARCH_DISTANCE = 80`, compara `DistanceSquare` con desempate
estable y exige una ruta navegable desde la cuenca actual. El barrido económico
inserta la orden `Depot { stop: false }` sólo para barcos primarios que necesitan
servicio y cuando la compañía tiene habilitado `servint_ships`; depósitos
rivales y cuencas aisladas quedan fuera. Se agregaron regresiones de propietario,
límite geométrico, conectividad y la inserción de la orden. Core queda en
`2437 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y diff limpios. La aceptación visual
manual bajo Weston sigue pendiente. #567 sigue abierta por callbacks navales
restantes y validación visual; #326 permanece abierta.

Actualización #567-SHIP-DOCK-ROUTING-COST (2026-09-11, `647b4502`): el
routing naval ya no conserva automáticamente el amarre geométricamente más
cercano cuando la estación tiene varios `DockingTile` elegibles. Evalúa las
rutas navegables de toda la huella física y selecciona la de menor longitud,
con desempate estable por distancia y coordenadas; para una estación con un
solo candidato conserva el camino genérico. Se agregó una regresión con un
amarre cercano alcanzable pero bloqueado por un rodeo más largo. Core queda en
`2438 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y diff limpios. #567 sigue abierta por
callbacks navales restantes y aceptación visual manual bajo Weston; #326
permanece abierta.

Actualización #567-SHIP-SERVICE-INTERVAL-DEFAULTS (2026-09-11, `64489b34`):
los defaults nativos de mantenimiento quedan diferenciados por clase:
trenes y carretera `150` días, aeronaves `100` y barcos `360`. Las compañías
nuevas reciben esos valores, los saves sin las claves antiguas los recuperan
mediante defaults de serde y un `0` explícito continúa desactivando el
servicio automático. `Vehicle::new` también inicializa el intervalo según su
tipo, evitando que aeronaves y barcos hereden siempre `150`. Se agregaron
regresiones de compañía, compatibilidad de carga y constructor por clase.
Core queda en `2441 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y diff
limpios. #567 sigue abierta por callbacks navales restantes y aceptación
visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-PHYSICAL-GRAPHICAL-HEADING (2026-09-11, `637ef813`):
la simulación naval separa el rumbo físico (`direction`) de la orientación
gráfica persistida (`ship_rotation`) como `ShipController`: un giro amplio
detiene el movimiento y avanza el sprite un paso de 45° cada 8 ticks. Las
curvas de hasta 45° sincronizan ambas orientaciones. `CmdBuildShip` normaliza
cualquier sección recibida a `GetShipDepotNorthTile`, inicializa rumbo,
rotación, centro subtesela y track; el render y el prisma global de Bevy usan
la rotación gráfica, evitando saltos de sprite durante el giro. Se agregaron
regresiones para los cuatro ejes de depósito, giro sobre el lugar y render de
orientación separada. Core queda en `2445 passed; 0 failed; 1 ignored` y
cliente en `1388 passed; 0 failed; 2 ignored`, con formato, Clippy estricto
de librería/binario y diff limpios. #567 sigue abierta por callbacks navales
restantes y aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #567-SHIP-DEPOT-RUNTIME-STATE (2026-09-11, `640bb8e9`): el
runtime naval mantiene el contrato raw de `Ship::state`: los barcos nuevos y
los que llegan al centro del depósito quedan en `TRACK_BIT_DEPOT`, al salir
recuperan el eje de navegación y el rumbo/orientación nativos, y cada avance
ordinario vuelve a persistir sus track bits. Los barcos importados con estado
antiguo `0` se normalizan de forma compatible; el comando Arrancar también
reconstruye el estado del depósito antes de despachar la ruta. Se agregaron
regresiones para las cuatro orientaciones de salida y para llegada, espera y
reanudación con la siguiente orden. Core queda en `2447 passed; 0 failed; 1
ignored` y cliente en `1388 passed; 0 failed; 2 ignored`, con formato,
Clippy estricto de librería/binario y diff limpios. #567 sigue abierta por
callbacks y pathfinding navales restantes y aceptación visual manual bajo
Weston; #326 permanece abierta.

Actualización #567-SHIP-DEPOT-MOUTH-OCCUPANCY (2026-09-11, `41fe30be`): la
salida de depósitos navales ahora replica la espera de `CheckShipStayInDepot`:
un barco que conserva `TRACK_BIT_DEPOT` no inicia su salida mientras otro
barco ocupa la misma huella canónica y tiene `cur_speed != 0`. La comprobación
acepta las dos secciones físicas del depósito, deja pasar barcos detenidos y
se ejecuta antes del controlador de movimiento para que el orden de la flota
sea determinista. Se agregó una regresión de espera, liberación y separación
de boca. Core queda en `2448 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con formato, Clippy estricto de
librería/binario y diff limpios. #567 sigue abierta por callbacks y
pathfinding navales restantes y aceptación visual manual bajo Weston; #326
permanece abierta.

Actualización #567-SHIP-DEPOT-OPPOSITE-EXIT (2026-09-11, `a3db8ce0`): la salida
naval del depósito consulta el primer tramo de la ruta antes de fijar el rumbo,
replicando la decisión de `YapfShip::CheckShipReverse` cuando el camino usa la
sección opuesta de la huella. En ese caso invierte `direction`,
`ship_rotation` y el eje raw antes de liberar el barco; la salida normal y los
barcos sin un primer tramo válido conservan la orientación de la boca. Se agregó
una regresión con una ruta que sólo puede comenzar por la sección opuesta. Core
queda en `2450 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0 failed;
2 ignored`, con formato, `diff --check` y Clippy estricto del binario cliente
limpios. El barrido `core --all-targets` sigue exponiendo 65 advertencias
preexistentes fuera de esta etapa. #567 sigue abierta por callbacks navales,
pathfinding de trackdirs y aceptación visual manual bajo Weston; #326 permanece
abierta.

Actualización #567-SHIP-REVERSE-BLOCKED-TRACK (2026-09-11, `4a8c36dd`): el
controlador naval replica el fallback de `ReverseShip` cuando la ruta deja de
ser navegable por una modificación del mapa, una conexión de agua inválida,
una ruta desfasada o una combinación de track sin subcoordenada válida. Invierte
el rumbo físico, detiene el barco, conserva `ship_rotation` para animar el
giro sobre el lugar y limpia la ruta para que el planificador la reconstruya.
Se agregó una regresión que transforma en tierra la siguiente tesela durante
el avance. Core queda en `2449 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con formato, Clippy estricto de
librería/binario y diff limpios. #567 sigue abierta por callbacks y
pathfinding navales restantes y aceptación visual manual bajo Weston; #326
permanece abierta.

Actualización #329/#567-SHIP-ACCELERATION (2026-09-11, `086e1c3c`): el parser
Action0 conserva la propiedad naval `0x24` (`ship_acceleration`) y el catálogo
la propaga a la compra de barcos. `ShipAccelerate` deja de usar siempre `+1` y
aplica la aceleración persistida, con `1` como fallback para saves antiguos.
Se agregaron regresiones de parser→catálogo y de movimiento con aceleración
personalizada. Core queda en `2453 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con formato, `git diff --check` y Clippy
estricto limpios. #329/#567 siguen abiertas por callbacks, propiedades
navales restantes y aceptación visual manual bajo Weston; #326 permanece
abierta.

Actualización #329/#567-SHIP-REFITTABLE (2026-09-11, `8b9a578c`): Action0
naval `0x09` deja de descartarse. El parser conserva el indicador de
refitabilidad, el catálogo lo propaga con `true` como compatibilidad para
vanilla/saves antiguos, y las APIs de refit ya no ofrecen cargos cuando el GRF
declara `0`. Se agregó una regresión parser→catálogo para las rutas con y sin
catálogo de cargos. Core queda en `2455 passed; 0 failed; 1 ignored` y cliente
en `1388 passed; 0 failed; 2 ignored`, con formato, `git diff --check` y
Clippy estricto limpios. #329/#567 siguen abiertas por callbacks y
propiedades navales restantes, y por aceptación visual manual bajo Weston;
#326 permanece abierta.

Actualización #329/#567-SHIP-LEGACY-REFIT-MASK (2026-09-11, `47a89b38`): el
parser Action0 conserva la máscara histórica naval `0x11` y la aplicación la
traduce desde los índices locales del GRF a la máscara global de cargos,
equivalente a `TranslateRefitMask` nativo, antes de combinarla con las clases
y las listas CTT. Se agregó una regresión que lleva los bits locales de Oil y
Goods hasta `EngineDef` y verifica los cargos refitables resultantes. Core
queda en `2454 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0
failed; 2 ignored`, con formato, `git diff --check` y Clippy estricto limpios.
#329/#567 siguen abiertas por callbacks y propiedades navales restantes, y
por aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-VARIANTS (2026-09-11, `3f99efd2`): Action0 naval
`0x20` ya conserva el ID local del motor padre, lo resuelve al ID global cuando
todo el stack está materializado y elimina enlaces que formen ciclos, igual que
`FinaliseEngineArray` nativo. La consulta del catálogo de compra mantiene cada
padre delante de sus variantes directas, respetando el orden elegido para los
hermanos. La regresión integra dos barcos del mismo GRF y verifica el enlace y
el orden final. Core queda en `2460 passed; 0 failed; 1 ignored` y cliente en
`1388 passed; 0 failed; 2 ignored`, con formato, `git diff --check`, checker de
paridad y Clippy estricto limpios. #329/#567 siguen abiertas por callbacks,
propiedades navales y aceptación visual manual bajo Weston; #326 permanece
abierta.

Actualización #329/#567-SHIP-EXTRA-FLAGS (2026-09-11, `e6ee4e0b`): Action0
naval `0x21` ya lee y conserva el `DWord` nativo en `ParsedVehicleMeta` y
`EngineDef.extra_flags`, incluyendo su propagación desde un GRF sintético
hasta el catálogo. Los flags `NoNews`, `NoPreview`, `JoinPreview` y
`SyncReliability` siguen sin consumidor local equivalente, por lo que esta
etapa preserva el contrato de datos y no simula efectos. Core queda en
`2461 passed; 0 failed; 1 ignored` y cliente en `1388 passed; 0 failed; 2
ignored`, con formato, diff, checker de paridad y Clippy estricto limpios.
#329/#567 continúan abiertas por la semántica runtime restante, callbacks y
aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-SPRITE-INDEX (2026-09-11, `09a15a73`): Action0
naval `0x08` ya normaliza el índice de sprite como OpenTTD, lo propaga al
catálogo y lo copia al `Vehicle::native_sprite_num` que ya participa del
round-trip SAV. La preview de compra y las capas fallback de Bevy seleccionan
el conjunto MPS/Oil/Coal/Ferry desde ese índice; `0xFF` conserva la marca
custom `0xFD` y cede prioridad a las vistas Action1/2 disponibles. El
fallback al `original_image_index` nativo cuando no existen vistas custom aún
no está modelado. Core queda en `2462 passed; 0 failed; 1 ignored` y cliente
en `1389 passed; 0 failed; 2 ignored`, con formato, diff y Clippy estricto
limpios. #329/#567 siguen abiertas por esa semántica restante, callbacks y
aceptación visual manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-SYNC-RELIABILITY (2026-09-11, `97b0e4d4`): el
consumidor naval de `Action0 0x21` ya aplica `SyncReliability` al resolver la
cadena `variant_parent_id`. La compra de barcos, sus piezas articuladas y el
autoreemplazo inicializan fiabilidad, decaimiento y vida útil desde el motor
padre, con fallback seguro para enlaces ausentes o cíclicos; los saves cargados
siguen conservando los valores persistidos. Se agregaron regresiones para el
padre sincronizado, enlace ausente y ciclo. Core queda en `2465 passed; 0
failed; 1 ignored`; Clippy estricto, formato y `diff --check` están limpios.
#329/#567 continúan abiertas por `NoNews`, `NoPreview`, `JoinPreview`, callbacks
navales, propiedades restantes y aceptación visual manual bajo Weston; #326
permanece abierta.

Actualización #329/#567-SHIP-SYNC-SERVICE (2026-09-11, `d5786d15`): el catálogo
activo ya acompaña la llegada y el cierre de carga de trenes, carretera y barcos,
además de las esperas tempranas de horario. El servicio en depósito vuelve a
resolver la fuente `variant_parent_id` cuando el motor tiene
`SyncReliability`, restaurando fiabilidad, decaimiento y vida útil del padre sin
perder el reset normal de averías; las APIs legacy sin catálogo mantienen el
fallback vanilla. Core queda en `2466 passed; 0 failed; 1 ignored` y cliente en
`1390 passed; 0 failed; 2 ignored`, con formato, `diff --check` y Clippy
estricto limpios. #329/#567 continúan abiertas por `NoNews`, `NoPreview`,
`JoinPreview`, callbacks navales, propiedades restantes y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-SHIP-ORIGINAL-SPRITE-FALLBACK (2026-09-11, `d4998002`):
`EngineDef` conserva ahora `original_image_index`, calculado desde los once
slots navales vanilla que OpenTTD usa como base antes de aplicar Action0/1/2.
Cuando un barco custom `0xFD` no resuelve una vista Action1/2 o callback, el
renderer y la preview de compra de Bevy recuperan esa familia base en vez de
forzar MPS; índices inválidos siguen cayendo de forma determinista a MPS. Se
agregaron regresiones de tabla, materialización del catálogo y selección
visual. Core queda en `2468 passed; 0 failed; 1 ignored` y cliente en `1392
passed; 0 failed; 2 ignored`, con formato, `diff --check` y Clippy estricto
limpios. #329/#567 continúan abiertas por `NoNews`, `NoPreview`,
`JoinPreview`, callbacks navales restantes y aceptación visual manual bajo
Weston; #326 permanece abierta.

Actualización #329/#567-VEHICLE-NO-DEFAULT-CARGO-MULTIPLIER (2026-09-11,
`820174d8`): Action0 conserva el bit 5 de `EngineMiscFlag` para trenes,
carretera, barcos y aeronaves, lo propaga a `EngineDef` con default compatible
para JSON anterior y permite que CB15 controle también la capacidad del cargo
por defecto cuando `NoDefaultCargoMultiplier` está activo. La regresión cubre
parser y catálogo de las cuatro familias, además de la selección runtime de
CB15. Core queda en `2474 passed; 0 failed; 1 ignored` y cliente en `1392
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios. #329/#567 siguen abiertas por subtipos completos, capacidad secundaria
de aeronaves, balanceo de consist, APIs legacy sin catálogo y aceptación visual
manual bajo Weston; #326 permanece abierta.

Actualización #329/#567-VEHICLE-ENGINE-FLAGS (2026-09-11, `54545c37`): las
propiedades DWORD de flags adicionales ya se leen y llegan al
catálogo para trenes (`0x30`), carretera (`0x27`), barcos y aeronaves (`0x21`).
La máscara misc separada conserva `NoBreakdownSmoke` (bit 6) en las cuatro
familias. El puente de eventos del cliente mantiene el sonido de avería y
suprime únicamente `EV_BREAKDOWN_SMOKE` cuando corresponde. La regresión cubre
parser, materialización del catálogo y el consumidor FX; `NoNews`, `NoPreview` y
`JoinPreview` siguen preservados sin efecto porque aún falta el ciclo runtime de
disponibilidad/previews. Core queda en `2477 passed; 0 failed; 1 ignored` y
cliente en `1393 passed; 0 failed; 2 ignored`, con Clippy de producción,
formato, `diff --check` y `parity-docs` limpios. #329/#567 siguen abiertas.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-CAPACITY (2026-09-11,
`ca8ce2c1`): Action0 aircraft `0x11` (BYTE) ya se conserva en
`ParsedVehicleMeta.mail_capacity`/`EngineDef.mail_capacity`, se materializa en
el catálogo y la compra muestra pasajeros y correo por separado. La unidad
sombra/articulada de correo todavía no existe en el modelo `Vehicle`; quedan
compra, refit/autoreplace, balanceo de capacidad y persistencia SAV para una
subetapa propia. Core queda en `2478 passed; 0 failed; 1 ignored` y cliente en
`1394 passed; 0 failed; 2 ignored`, con Clippy de producción, formato,
`diff --check` y `parity-docs` limpios. #329/#567 siguen abiertas.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-SHADOW-SAV (2026-09-11,
`caa9ccb4`): el escritor `VEHS` ya reconstruye la sombra auxiliar de cada
aeronave con `CargoType::Mail` y `EngineDef.mail_capacity` en `cargo_cap`.
La capacidad principal de pasajeros queda separada y las referencias
primario→sombra→rotor no cambian. La sombra sigue siendo sólo una proyección
de exportación: faltan una unidad secundaria en `Vehicle`, compra/refit/
autoreplace balanceados y su rehidratación al importar SAV. Core queda en
`2478 passed; 0 failed; 1 ignored` y cliente en `1394 passed; 0 failed; 2
ignored`, con Clippy de producción, formato, `diff --check` y
`parity-docs` limpios. #329/#567 siguen abiertas.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-STATE (2026-09-11,
`c76ef972`): `Vehicle` conserva ahora la capacidad secundaria como metadata
opcional del primario. Compra y autoreemplazo la inicializan desde el motor;
`vehicles_from_chunks` la recupera de la fila `AIR_SHADOW` enlazada por
`next`, y el exportador la usa antes del fallback del catálogo. El alcance no
simula una segunda unidad ni altera el pool de carga: siguen pendientes
compra/refit/autoreplace con balanceo real, carga de correo y consumo de la
capacidad en el runtime. Core queda en `2479 passed; 0 failed; 1 ignored` y
cliente en `1394 passed; 0 failed; 2 ignored`, con Clippy de producción,
formato, `diff --check` y `parity-docs` limpios. #329/#567 siguen abiertas.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-REFIT (2026-09-11,
`d34a0e0e`): la capacidad secundaria de correo ahora se recalcula al comprar,
refitar en hangar o estación, aplicar un autorefit de orden y autoreemplazar.
La regla sigue `Engine::DetermineCapacity`: sólo los cargos de clase pasajeros
conservan la capacidad Action0 `0x11`; correo y las demás clases dejan la sombra
en cero, mientras que un cargo `NewGRF` con clase pasajeros conserva el valor.
También se consulta `CB36` para la propiedad `0x11` y el fallback SAV respeta el
tipo de cargo. La sombra continúa siendo metadata/exportación, no una segunda
`Vehicle` con pool de carga propio; #329/#567 siguen abiertas. Core queda en
`2480 passed; 0 failed; 1 ignored` y cliente en `1394 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-REFIT-PERSISTENCE (2026-09-11,
`bfcbc187`): `sync_cargo_from_packets()` y `clear_cargo()` conservan ahora el
tipo de carga elegido por refit en aeronaves cuando no quedan paquetes. Así,
un avión refitado a correo no vuelve silenciosamente a pasajeros durante el
siguiente tick o después de descargar; la capacidad secundaria permanece en
cero para correo y vuelve a `0x11` al retornar a una carga de clase pasajeros.
La sombra sigue siendo metadata/exportación y no una segunda `Vehicle`; #329/#567
continúan abiertas. Core queda en `2481 passed; 0 failed; 1 ignored`, con
Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-CARGO-ROUNDTRIP (2026-09-11,
`0e094600`): el contador `cargo_count` de `AIR_SHADOW` ya se conserva junto a
`cargo_cap` al importar y exportar `VEHS`; el enlace `next` del primario vuelve a
asociar ambos valores sin mezclar la carga principal. Hangar, estación,
autorefit y autoreemplazo aplican la poda nativa del correo a la nueva
capacidad, por lo que un refit no deja carga secundaria imposible para el
próximo SAV. Sigue pendiente materializar la segunda `VehicleCargoList` y
actualizar el contador durante carga/descarga real. Core queda en `2481 passed;
0 failed; 1 ignored` y cliente en `1394 passed; 0 failed; 2 ignored`, con
Clippy estricto, formato y `diff --check` limpios; #329/#567 continúan abiertas.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-PACKET-ROUNDTRIP (2026-09-11,
`bf7167cf`): el primario de aeronave conserva una `VehicleCargoList` separada
para `AIR_SHADOW`; sus paquetes se exportan al pool `CAPA`, se referencian en
`VEHS.common.cargo.packets` de la sombra y se rehidratan con origen, ruta, edad
y feeder. Cuando existe la lista, su total reemplaza al contador escalar y el
refit aplica truncamiento a la nueva capacidad. Falta integrar esa lista con
carga/descarga, pagos y trasbordos de estación, además de la entidad FTA
separada; #329/#567 siguen abiertas. Core queda en `2481 passed; 0 failed; 1
ignored` y cliente en `1394 passed; 0 failed; 2 ignored`, con Clippy estricto,
formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-STATION-LOAD (2026-09-11): la
lista secundaria de correo ya participa en la fase de carga de estación y en
el envejecimiento del reloj del avión. Respeta capacidad `0x11`, rating,
reservas y consumo de stock; conserva `NewCargo`, `CargoTaken`, next-hop y
contadores de carga sin tocar el hold primario de pasajeros. Sigue pendiente
la descarga con pagos/trasbordos y la entidad FTA separada; #329/#567 continúan
abiertas. Core queda en `2483 passed; 0 failed; 1 ignored` y cliente en `1394
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-STATION-UNLOAD (2026-09-11): la
lista secundaria de correo ya participa en la descarga gradual de estación.
Usa la aceptación y el `Stage` de correo, conserva next-hop, origen, edad y
feeder, liquida `CargoPayment` y la participación feeder, actualiza monitor,
ingresos y link graph, y reintroduce los paquetes transferidos en la cola de la
estación. La ventana de descarga sólo termina cuando sale el último slice de
`AIR_SHADOW`; la lista también participa en `Empty` y en la purga de pagos.
La entidad FTA separada, sus reservas/reparto nativos y la validación visual y
runtime restante siguen pendientes; #329/#567 continúan abiertas. Core queda
en `2485 passed; 0 failed; 1 ignored` y cliente en `1394 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-STATION-TRANSFER (2026-09-11):
el correo secundario puede atravesar una estación con orden `Transfer`, volver
a la cola de `StationCargoList` sin ingreso final y conservar `first_station`
al recargarse en el avión. La regresión cubre el ciclo descarga → espera →
recarga sin mezclar el hold principal de pasajeros; la entidad FTA separada,
sus reservas/reparto nativos y la validación visual y runtime restante siguen
pendientes, por lo que #329/#567 continúan abiertas. Core queda en `2486
passed; 0 failed; 1 ignored` y cliente en `1394 passed; 0 failed; 2 ignored`,
con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-WINDOW-CONSIST-NEWGRF (2026-09-11): las
tiras de consist de `VehicleViewWindow` ya consultan el catálogo activo para
las unidades NewGRF y reutilizan la primera capa del resolver de compra,
incluido el resultado runtime de `SpriteStack`; las unidades vanilla mantienen
su sprite lateral existente. La tira quedó en un sistema ECS separado para no
exceder el límite de parámetros de Bevy. La entidad FTA separada, sus
scopes/estado propios y la validación visual/runtime restante siguen
pendientes, por lo que #329/#567 continúan abiertas. Core queda en `2488
passed; 0 failed; 1 ignored` y cliente en `1401 passed; 0 failed; 2 ignored`,
con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-AGE-PERIOD (2026-09-11): el
parser de aeronaves ya consume Action0 `0x1C` con el ancho nativo WORD y
propaga el período al catálogo `EngineDef`. La regresión cubre tanto la lectura
directa como la instalación de un motor NewGRF; la sombra todavía comparte el
reloj del primario y quedan pendientes su contador/período FTA independiente,
por lo que #329/#567 continúan abiertas. Core queda en `2486 passed; 0 failed;
1 ignored` y cliente en `1394 passed; 0 failed; 2 ignored`, con Clippy
estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-HELICOPTER-CATALOG-AUDIO
(2026-09-11): el puente de audio de despegue y aterrizaje ya consulta el
`EngineDef` del catálogo activo antes de elegir el sonido. Un helicóptero
NewGRF con ID fuera del rango vanilla recibe `TakeoffHelicopter` en ambos
eventos; los IDs no catalogados conservan el fallback vanilla. La regresión
cubre las dos transiciones; la entidad FTA separada, sus scopes/estado propios
y la validación visual y runtime restante siguen pendientes, por lo que
#329/#567 continúan abiertas. Core queda en `2488 passed; 0 failed; 1
ignored` y cliente en `1397 passed; 0 failed; 2 ignored`, con Clippy estricto,
formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-HELICOPTER-CUSTOM-ROTOR
(2026-09-11): el rotor auxiliar del cliente ya intenta resolver la primera
capa del grupo NewGRF del helicóptero activo. El frame animado se usa como
índice de vista, `var 1F` recibe la orientación física del avión, se conservan
la paleta y los offsets/tamaño custom y el rotor OpenGFX queda como fallback
cuando Action2 no devuelve una vista. La regresión cubre un motor NewGRF con
ID fuera de vanilla y un selector Action2 dependiente de `var 1F`; SpriteStack
del rotor y la validación visual/runtime restante quedan pendientes, por lo
que #329/#567 continúan abiertas. Core queda en `2488 passed; 0 failed; 1
ignored` y cliente en `1398 passed; 0 failed; 2 ignored`, con Clippy estricto,
formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-HELICOPTER-ROTOR-STACK
(2026-09-11): las capas adicionales del rotor NewGRF ya se materializan como
children estables, con la misma resolución Action2, offsets aéreos y
profundidad ordenable que la capa principal. Las capas que no existen en una
resolución se ocultan y pueden reaparecer sin reconstruir la entidad; la
regresión base sigue cubriendo frame y `var 1F`. La entidad FTA separada, la
validación visual/runtime restante y el fallback estático con SpriteStack sin
runtime quedan pendientes, por lo que #329/#567 continúan abiertas. Core queda
en `2488 passed; 0 failed; 1 ignored` y cliente en `1398 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-AGE-COUNTER (2026-09-11): el
correo de `AIR_SHADOW` mantiene ahora una cuenta atrás independiente de la
bodega primaria. Ambos relojes usan el período efectivo del motor como
fallback, pero una sombra cargada puede envejecer sin que envejezca el hold
principal. La cuenta se exporta en `AIR_SHADOW.common.cargo_age_counter` y se
recupera mediante `next` al importar SAV. Sigue pendiente resolver el período
específico de la sombra/FTA y materializar la entidad nativa separada; #329/#567
continúan abiertas. Core queda en `2487 passed; 0 failed; 1 ignored` y cliente
en `1394 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y
`diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-MAIL-AGE-PERIOD-CB36 (2026-09-11):
el período de envejecimiento de `AIR_SHADOW` ya se resuelve como en
`UpdateAircraftCache`: `CB36` consulta la propiedad `0x1C` sobre la vista de
correo de la sombra y su resultado se mantiene en un caché separado. El
período primario y el secundario pueden ser cero o distintos, y la evaluación
restaura la carga principal después del callback. La entidad FTA separada y
sus scopes/estado propios siguen pendientes; #329/#567 continúan abiertas.
Core queda en `2488 passed; 0 failed; 1 ignored` y cliente en `1394 passed; 0
failed; 2 ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-HELICOPTER-CATALOG-VISUALS
(2026-09-11): la clasificación de helicópteros del cliente ya consulta el
`EngineDef` del catálogo activo, no sólo los tres IDs vanilla. Los motores
NewGRF marcados por Action0 `0x09` reciben su rotor auxiliar en Bevy y quedan
filtrados en el helipuerto correcto de la ventana de compra. Las regresiones
cubren la clasificación de render y la lista de compra; la entidad FTA
separada, sus scopes/estado propios y la validación visual siguen pendientes,
por lo que #329/#567 continúan abiertas. Core queda en `2488 passed; 0 failed;
1 ignored` y cliente en `1396 passed; 0 failed; 2 ignored`, con Clippy
estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-HELICOPTER-BUY-PREVIEW
(2026-09-11): la vista de compra ya usa un contenedor por capas para el
helicóptero seleccionado. El cuerpo conserva su preview existente y el rotor
consulta el mismo resolver Action2 que el mapa, con `var 1F = DIR_W`, frame
detenido, offsets/tamaño NewGRF y hasta ocho children estables para
`SpriteStack`; cuando no hay vista custom se muestra la capa detenida de
OpenGFX. La regresión cubre la geometría escalada y centrada de los offsets.
La entidad FTA separada, sus scopes/estado propios y la validación visual y
runtime restante siguen pendientes, por lo que #329/#567 continúan abiertas.
Core queda en `2488 passed; 0 failed; 1 ignored` y cliente en `1400 passed; 0
failed; 2 ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-AIRCRAFT-BUY-BODY-STACK (2026-09-11): el
preview seleccionado de compra ya consulta el mismo resolver Action2 del mapa
para el cuerpo de cualquier vehículo NewGRF. La orientación GUI estable,
parámetros del GRF, paleta 2CC, offsets/tamaño y hasta ocho capas de
`SpriteStack` se conservan; las capas adicionales viven en children estables y
se ocultan cuando una resolución no las entrega. Las vistas NewGRF horneadas
usan sus bounds antes de caer al sprite vanilla. La entidad FTA separada, sus
scopes/estado propios y la validación visual/runtime restante siguen
pendientes, por lo que #329/#567 continúan abiertas. Core queda en `2488
passed; 0 failed; 1 ignored` y cliente en `1401 passed; 0 failed; 2 ignored`,
con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-UI-CONSIST-SPRITES (2026-09-11): las tiras
compactas de `VehicleViewWindow`, `VehicleDetailsWindow`, `DepotPanel` y la
lista global de vehículos ya comparten la clasificación del catálogo activo y
la primera capa del resolver NewGRF runtime. Los widgets conservan sus
children, drag e interacción existentes; el panel de depósito y la lista
global separan la sincronización de imágenes para no superar el límite ECS de
Bevy. Las capas completas del mapa y el `SpriteStack` del preview grande no se
reemplazan por una textura plana. La entidad FTA separada, sus scopes/estado
propios y la validación visual/runtime restante siguen pendientes, por lo que
#329/#567 continúan abiertas. Core queda en `2488 passed; 0 failed; 1
ignored` y cliente en `1401 passed; 0 failed; 2 ignored`, con Clippy estricto,
formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-CAMERA-CATALOG-OFFSETS (2026-09-11): los
botones de centrado y la cámara de `VehicleViewWindow` ya calculan la posición
del sprite con los offsets del `EngineDef` activo cuando existe una vista
NewGRF; los motores sin vista catalogada conservan el fallback vanilla. La
regresión usa una vista NewGRF con bounds desplazados y comprueba que la cámara
no vuelva a la posición OpenGFX. Los grupos runtime sin una entidad visual
materializada siguen pendientes, igual que la entidad FTA separada y la
validación visual/runtime amplia; #329/#567 continúan abiertas. Core queda en
`2488 passed; 0 failed; 1 ignored` y cliente en `1401 passed; 0 failed; 2
ignored`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #329/#567-VEHICLE-CAMERA-RUNTIME-BOUNDS (2026-09-11): la
cámara de `VehicleViewWindow` ahora reutiliza la primera capa que el renderer
resuelve para la unidad real, incluidos offsets/tamaño de cuerpos NewGRF
definidos sólo por Action2 runtime y `SpriteStack`. Si no hay una capa
resoluble conserva el fallback catalogado/vanilla. La sincronización de cámara
se separó del sistema textual para mantener acotada la consulta ECS; la
regresión usa el mismo fixture runtime desplazado y verifica la posición
efectiva. La entidad FTA separada, sus scopes/estado propios y la validación
visual/runtime amplia siguen pendientes, por lo que #329/#567 continúan
abiertas. Core queda en `2488 passed; 0 failed; 1 ignored` y cliente en `1401
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios para este cambio.

Actualización #329/#567-VEHICLE-STATIC-SPRITE-STACK (2026-09-11): el catálogo
ya conserva el grafo `TrainSpriteGraphics` cuando Action0 declara
`SpriteStack`, aunque el GRF no tenga random/variational ni otro motivo previo
para una resolución runtime. Esto permite que el renderer resuelva también
los slots estáticos de cuerpo y rotor con la misma semántica de Action2; los
motores sin `SpriteStack` mantienen el almacenamiento anterior. La regresión
aplica un GRF de tren y verifica que el grafo sobrevive a la construcción del
catálogo. La entidad FTA separada, sus scopes/estado propios y la validación
visual/runtime amplia siguen pendientes, por lo que #329/#567 continúan
abiertas. Core queda en `2489 passed; 0 failed; 1 ignored` y cliente en `1401
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios.

Actualización #329/#567-VEHICLE-CAMERA-CENTER-ACTIONS (2026-09-11): los
botones de centrado de la lista global y de `VehicleViewWindow` ya consultan
la misma primera capa NewGRF que el mapa, incluyendo cuerpos runtime y
`SpriteStack` estático; cuando el cache visual no está instalado conservan el
fallback catalogado/vanilla. La resolución quedó en sistemas ECS separados
para mantener pequeños los manejadores de acciones y se conservó el centrado
de órdenes existente. La entidad FTA separada, sus scopes/estado propios y la
validación visual/runtime amplia siguen pendientes, por lo que #329/#567
continúan abiertas. Core queda en `2489 passed; 0 failed; 1 ignored` y cliente
en `1401 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y
`diff --check` limpios.

Actualización #329-VEHICLE-VISUAL-EFFECT-TICK-COUNTER (2026-09-11): el
contador nativo `Vehicle::tick_counter` ahora avanza al comienzo del tick de
cada unidad, incluyendo vagones, articulados y vehículos que luego esperan en
depot, señal o bloqueo; también conserva el wrap de `u8` del upstream. CB10 y
CB160 usan ese contador por vehículo para la cadencia de vapor/chispa, la
proyección determinista local de diésel y el valor aleatorio del callback, de
modo que la emisión no depende del FPS ni de que dos unidades compartan el
tick global. La persistencia SAV ya existente conserva el contador entre
guardado y carga. El stream RNG global, filtros/consist completos, altura de
aeronaves y compositor/sorter siguen pendientes; #329/#567 continúan
abiertas. La regresión cubre wrap por unidad y la cadencia de vapor; actualizar
los conteos verificados: core `2490 passed; 0 failed; 1 ignored` y cliente
`1401 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y
`diff --check` limpios.

Actualización #329-VEHICLE-VISUAL-EFFECT-AIRCRAFT-ALTITUDE (2026-09-11): la
posición de mundo de CB10/CB160 suma ahora la altitud física de aeronaves al
Z del terreno usando la misma escala del cuerpo y rotor (`altitude ×
TILE_PIXEL_HEIGHT`). Así los efectos de una aeronave en vuelo, despegue o
aterrizaje no quedan dibujados sobre la costa mientras el sprite está en el
aire; trenes, carretera y barcos conservan su contrato de terreno. La
regresión compara tierra y crucero y conserva X/Y y offsets relativos. El
stream RNG global, filtros/consist completos, bounds del sorter de aeronaves y
compositor/sorter global siguen pendientes; #329/#567 continúan abiertas.

Actualización #329-VEHICLE-AIRCRAFT-SORT-ALTITUDE (2026-09-11): el prisma
`Vehicle::bounds` del cliente ahora usa la misma escala de píxeles que el
sprite y los efectos para la altitud de una aeronave. Un avión en crucero se
desplaza `altitude × TILE_PIXEL_HEIGHT` en Z dentro del sorter, mientras que
los bounds de tierra y los demás tipos no cambian. La regresión verifica el
desplazamiento entre tierra y crucero. Siguen pendientes el stream RNG global,
filtros/consist completos y el compositor/raster global; #329/#567 continúan
abiertas.

Actualización #329-VEHICLE-VISUAL-EFFECT-RAIL-POWER (2026-09-11): los efectos
estándar y CB160 de tren ahora respetan el filtro `HasPowerOnRail` del upstream:
una locomotora eléctrica sobre rail normal no genera humo ni chispas, mientras
que la misma unidad sobre rail eléctrico vuelve a ser elegible. El resto de
vehículos mantiene su camino anterior. La regresión cubre ambos railtypes;
siguen pendientes el stream RNG global, la propagación completa por consist y
el compositor/raster global, por lo que #329/#567 continúan abiertas. Cliente
queda en `1403 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y
`diff --check` validados para esta etapa.

Actualización #329-VEHICLE-VISUAL-EFFECT-WAGON-CB10 (2026-09-11): el renderer
ya no descarta por clase los efectos estándar de vagones. Como en
`Vehicle::ShowVisualEffect`, el efecto por defecto del vagón sigue desactivado,
pero un CB10 explícito puede seleccionar vapor, diésel o chispa para esa
unidad; el filtro de potencia ferroviaria se mantiene. La regresión cubre el
opt-in NewGRF y el fallback vanilla. Siguen pendientes la propagación completa
del consist, el stream RNG global y el compositor/raster global; #329/#567
continúan abiertas. Cliente queda en `1404 passed; 0 failed; 2 ignored`, con
Clippy estricto, formato y `diff --check` validados para esta etapa.

Actualización #329-VEHICLE-POWERED-WAGON-VISUAL-EFFECT (2026-09-11): la
reconstrucción del consist ahora respeta el bit 7 (`VE_DISABLE_WAGON_POWER`)
de CB10/Action0 al decidir si un vagón recibe la potencia adicional de la
cabeza. También se normaliza el retorno `0xFF` de CB10 a `0xCF`, igual que
`Vehicle::UpdateVisualEffect`; el sentinel de catálogo conserva la selección
vanilla sin desactivar por accidente la potencia de vagones. Las regresiones
cubren callback explícito, sentinel y consist con bit 7 activo/inactivo.
Siguen pendientes `UsesWagonOverride`, el stream RNG global, la propagación
completa del consist visual y el compositor/raster global; #329/#567 continúan
abiertas. Core queda en `2491 passed; 0 failed; 1 ignored` y cliente en `1404
passed; 0 failed; 2 ignored`, con Clippy estricto, formato y `diff --check`
limpios.

Actualización #329-VEHICLE-WAGON-OVERRIDE-PRESENCE (2026-09-11): la presencia
de `UsesWagonOverride` ya se deriva de una asignación Action3 compatible, no
solamente de compartir GRFID. La clave valida el ID local del vagón, el motor
que encabeza el consist y la carga (incluido el grupo default); el resultado
se reutiliza para `PoweredWagon`, para excluir el límite de velocidad del
vagón y para que el renderer Bevy seleccione el grupo reemplazado sólo cuando
existe. Las regresiones cubren asignación real, ausencia de asignación, cabeza
de consist y límite de velocidad. Siguen pendientes el stream RNG global, la
propagación completa del consist visual, livery/callbacks restantes y el
compositor/raster global; #329/#567 continúan abiertas. Core queda en `2492
passed; 0 failed; 1 ignored` y cliente en `1405 passed; 0 failed; 2 ignored`,
con Clippy estricto, formato y `diff --check` limpios.

Actualización #329-VEHICLE-VISUAL-EFFECT-GLOBAL-RNG (2026-09-11): el call
site visual del cliente consume ahora el `Randomizer` global de la partida
para las llamadas `Chance16` de humo/chispas y para el parámetro RNG de CB160,
manteniendo el orden y sin consumir palabras en ramas que OpenTTD no evalúa.
CB160 recibe la palabra completa de 32 bits en lugar del `u16` truncado; los
wrappers de pruebas sin partida conservan un fallback determinista. Las
regresiones cubren el avance exacto del stream, la no-consumición del vapor y
la diferencia de bits altos en CB160. Siguen pendientes la propagación completa
del consist visual, livery/callbacks restantes y el compositor/raster global;
#329/#567 continúan abiertas. Core queda en `2493 passed; 0 failed; 1 ignored`
y cliente en `1407 passed; 0 failed; 2 ignored`, con Clippy estricto, formato
y `diff --check` limpios.

Actualización #329-VEHICLE-VISUAL-EFFECT-CONSIST-CONTEXT (2026-09-11): el
renderer Bevy procesa ahora cada cadena en orden cabeza→cola y toma de la
cabeza las reglas globales de `Vehicle::ShowVisualEffect` (estado de marcha,
reversa, parada de estación, velocidad máxima y potencia/peso) mientras
conserva por unidad la visibilidad, railtype, callback y posición del efecto.
Esto corrige la divergencia donde un vagón con velocidad propia cero no podía
usar la velocidad de la cabeza o podía consumir RNG fuera del orden nativo.
Las regresiones cubren orden de cadena, bloqueo de followers por cabeza parada,
uso de velocidad de cabeza y umbral de entrada a estación. Siguen pendientes
la propagación completa de consist visual NewGRF, livery/callbacks restantes y
el compositor/raster global; #329/#567 continúan abiertas. Cliente queda en
`1411 passed; 0 failed; 2 ignored`, con Clippy estricto, formato y
`diff --check` limpios.

Actualización #329-VEHICLE-SOUND-SPATIAL-ORIGIN (2026-09-11): los samples
locales devueltos por `CBID_VEHICLE_SOUND_EFFECT` se encolan ahora con la
posición de la unidad y el mixer aplica la misma atenuación espacial que a
`SndPlayVehicleFx`. La cola distingue ese origen de los sonidos ambientales
de tesela, por lo que desactivar `sound.ambient` ya no silencia sonidos de
vehículos; los callers siguen filtrando por `sound.vehicle`. Las regresiones
cubren la coordenada, la clasificación y el drenado Bevy. El sonido conserva
volumen/prioridad Action11 y el fallback vanilla; sprites locales, callbacks
restantes, consist visual completo y compositor/raster siguen pendientes, por
lo que #329/#567 continúan abiertas. Core queda en `2495 passed; 0 failed; 1
ignored` y cliente en `1413 passed; 0 failed; 2 ignored`, con Clippy
estricto, formato y `diff --check` limpios.

Actualización #329-VEHICLE-VISUAL-EFFECT-POWERED-RAILTYPES (2026-09-11): el
gate de humo/chispa del renderer Bevy consulta ahora
`state.runtime.rail_type_props` al evaluar si una unidad ferroviaria tiene
potencia sobre la vía actual. Cuando NewGRF redefine `powered_railtypes`, la
decisión usa esa máscara; sin override se conserva la tabla vanilla. La
regresión cubre un motor eléctrico sobre vía normal con una máscara runtime
redefinida. Esto acota la paridad del gate visual, pero no declara completa la
topología de consist ni los callbacks/sprites locales restantes; #329/#567
continúan abiertas. El foco quedó en `24 passed; 0 failed`, core railtype en
`7 passed; 0 failed`, con Clippy estricto, formato y `diff --check` limpios.

Actualización #567-SHIP-DEPOT-NORTH-ANCHOR (2026-09-11): la apertura del
panel de depósito normaliza un clic sobre cualquiera de las dos teselas de un
depósito naval a `ship_depot_north_tile`, igual que `ClickTile_Water` y
`ShowDepotWindow` nativos. Esto mantiene alineados el panel, la fila `DEPT`,
la búsqueda de barcos, el centrado de cámara y las acciones de compra cuando
el cursor cae en la sección sur. Las cuatro orientaciones y los depósitos no
navales tienen regresiones separadas; #567 sigue abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-ORDER-ANCHOR (2026-09-11): la coordenada de un
depósito naval se normaliza ahora a la sección norte en las órdenes nuevas y al
resolver órdenes legacy/shared, manteniendo el equivalente local del
`DepotID`/`Depot::xy` nativo. La llegada y la preparación de un barco también
recentran posición y estado cuando un save lo dejó en la sección sur antes de
la salida. Las regresiones cubren las cuatro rutas de consulta, una orden
emitida desde la sección opuesta y un barco importado detenido; #567 sigue
abierta por callbacks, pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-HIDDEN-STATE (2026-09-11): la visibilidad de un
barco que todavía ocupa un depósito naval se decide ahora por
`SHIP_STATE_DEPOT`, equivalente local de `Ship::IsInDepot`/`VehState::Hidden`,
en lugar de ocultarlo sólo por la clase de tesela. Así la unidad sigue oculta
durante la espera dentro del depósito y vuelve a mostrarse cuando ya tiene un
estado de vía durante la salida, aunque la interpolación aún no haya dejado el
footprint. La regresión cubre ambos estados; #567 sigue abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-LIST-STATE (2026-09-11): la lista de la ventana
de depósito filtra ahora con el estado físico equivalente a `IsInDepot` para
cada clase de vehículo. En particular, un barco que ya pasó a un
`SHIP_STATE_TRACK_*` durante la salida deja de aparecer como unidad disponible,
aunque `pos` todavía apunte al footprint naval; se conservan las reglas de
hangar, depósito vial y `depot_leave_cleared` ferroviario. La regresión separa
un barco dentro y otro saliendo; #567 sigue abierta por callbacks, pathfinding
completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SOUND-STATE (2026-09-11): el emisor de sonidos
periódicos de vehículos consulta ahora el estado físico por clase, igual que la
visibilidad y la lista del depósito. Un barco en `SHIP_STATE_DEPOT` permanece
silencioso, mientras que uno que ya pasó a `SHIP_STATE_TRACK_*` vuelve a emitir
su evento espacial aunque `pos` todavía esté dentro del footprint del depósito.
La regresión cubre ambos estados; #567 sigue abierta por callbacks, pathfinding
completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-STATE-PREDICATE (2026-09-11): se centralizó el
equivalente local de `Vehicle::IsInDepot` por clase (track/road phase, estado
naval y hangar) y lo reutilizan audio, lista de depósito y detalle de vehículo.
El helper histórico que sólo clasifica la tesela permanece separado para la
geometría. Esto evita que una unidad naval saliendo conserve la marca
«depósito» en un lector mientras ya fue liberada en otro; #567 sigue abierta
por callbacks, pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-OPERATION-STATE (2026-09-11): las decisiones de
refit, autoreplace y sus barridos automáticos consultan ahora el estado físico
equivalente a `Vehicle::IsInDepot`, no sólo la clase de la tesela. Una nave
que ya pasó a `SHIP_STATE_TRACK_*` durante la salida no puede refitarse ni
reemplazarse aunque conserve temporalmente el footprint del depósito; las
trazas ferroviarias también reflejan el estado real. Las regresiones cubren
refit de interfaz/comando, refit automático de órdenes y autoreplace naval; el
helper de tesela queda para geometría/render que todavía requiere una auditoría
separada. #567 continúa abierta por callbacks, pathfinding completo y aceptación
visual/framebuffer.

Actualización #567-SHIP-DEPOT-SPEED-STATE (2026-09-11): el límite de 61 de
`Train::GetCurrentMaxSpeed` durante una salida ferroviaria consulta ahora el
estado equivalente a `TRACK_BIT_DEPOT`, no sólo si la coordenada cae sobre una
tesela de depósito. Así el tren conserva el límite mientras está dentro y lo
libera al cruzar el estado de vía aunque la interpolación aún conserve el
footprint. La regresión cubre ambos estados; el bloqueo de efectos visuales por
`IsDepotTile` se mantiene separado porque es el contrato nativo de
`Vehicle::ShowVisualEffect`. #567 continúa abierta por callbacks, pathfinding
completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-STOPPED-STATE (2026-09-11): el refit manual y la
ventana de refit exigen ahora el equivalente local de
`Vehicle::IsStoppedInDepot`, que combina el estado físico de depósito con una
unidad detenida y velocidad cero. Esto separa la autorización de refit de
`IsInDepot`: una nave o vehículo que todavía conserva el estado de depósito
pero ya está en movimiento no se ofrece ni acepta como refit manual. Los
barridos de autoreplace y el refit automático de órdenes conservan sus
predicados propios (`IsChainInDepot`/estado de llegada). #567 continúa abierta
por callbacks, pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SAME-ORDER (2026-09-11): el controlador naval
procesa ahora la orden que apunta al mismo depósito cuando el barco ya está en
su centro, igual que `CheckShipStayInDepot` + `VehicleEnterDepot` nativos. Esto
evita confundir una reentrada con ausencia de destino: una orden `halt` deja al
barco detenido y una orden pass-through procesa servicio/refit y avanza a la
siguiente orden. Las dos variantes tienen regresiones directas; #567 continúa
abierta por callbacks, pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-NEAREST-ROUTE (2026-09-11): el comando manual de
ir al depósito más cercano usa para barcos la búsqueda equivalente a
`Ship::FindClosestDepot`: propietario de la unidad, distancia máxima nativa y
ruta navegable hasta la sección norte. El resto de vehículos conserva la
selección indexada existente. La regresión evita elegir un depósito rival más
cercano sobre la misma lámina de agua; #567 continúa abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-COMMAND-ANCHOR (2026-09-11): los comandos de
flota que reciben una coordenada de depósito aceptan ahora también la sección
sur de una huella naval 2×1 y la convierten a la sección norte antes de
resolver el conjunto de vehículos. El contrato cubre clonado, venta masiva,
arranque/parada, autoreemplazo masivo y reordenamiento; la regresión ejecuta
las cinco operaciones desde la sección sur y verifica que todos los barcos
siguen anclados al mismo depósito. #567 continúa abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SHARED-ORDERS (2026-09-11): las listas de órdenes
compartidas normalizan una orden naval que llega desde la sección sur antes de
persistirla o copiarla a otros vehículos. La creación de la lista, el vínculo
de una segunda nave y la edición posterior conservan los flags completos de la
orden y dejan tanto la lista como `vehicle.dest` en el ancla norte. #567
continúa abierta por callbacks, pathfinding completo y aceptación
visual/framebuffer.

Actualización #567-SHIP-DEPOT-LOAD-ANCHOR (2026-09-11): la hidratación final
normaliza órdenes navales provenientes de `.sav`, JSON versionado y JSON legacy
después de reconstruir el mapa. También corrige `vehicle.dest` y las listas
compartidas, cubriendo el caso en que el decodificador de órdenes sólo conocía
el índice lineal y había recibido la sección sur. La regresión de carga verifica
orden, destino y pool compartido; #567 continúa abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-PERSISTED-POS (2026-09-11): las operaciones de
flota comparan ahora la posición persistida de cada unidad contra el ancla
canónica del depósito. Así clonado, arranque/parada, reordenamiento,
autoreemplazo masivo y venta masiva siguen encontrando un barco cuyo save
legacy conserva la sección sur en `Vehicle::pos`; la consulta no altera la
posición física hasta que el runtime la hidrate. La regresión ejecuta las cinco
operaciones con esa posición legacy; #567 continúa abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-UNBUNCH (2026-09-11): el controlador naval
respeta ahora la espera de unbunch para órdenes compartidas y registra la
salida sólo cuando la nave abandona realmente el estado de depósito. Una
reentrada al mismo depósito no reprogama la lista; una salida actualiza el
round-trip y la próxima salida de las unidades compartidas, como
`CheckShipStayInDepot` + `LeaveUnbunchingDepot`. Las regresiones cubren la
espera y la programación naval; #567 continúa abierta por callbacks,
pathfinding completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SERVICE-EXIT (2026-09-11): una nave que libera
el estado de depósito ejecuta ahora el servicio nativo antes de acelerar,
incluyendo salidas iniciadas manualmente o rehidratadas desde un save. Esto
evita que `needs_servicing` y la fiabilidad queden desfasados respecto de
`VehicleServiceInDepot`; la regresión comprueba servicio, fiabilidad y cambio
de estado en el mismo tick. #567 continúa abierta por callbacks, pathfinding
completo y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SERVICE-IN-DEPOT (2026-09-11): el callback
económico naval comprueba primero el estado físico de depósito cuando la
orden persistente no es otra orden de depósito y ejecuta el servicio sin
esperar a la salida. Esto conserva la precedencia de
`NeedsAutomaticServicing()` —que no interrumpe una orden de depósito— y
mantiene `needs_servicing` alineado con `VehicleServiceInDepot`; la regresión
cubre una orden de circuito, fiabilidad y permanencia en la huella 2×1.
#567 continúa abierta por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #567-SHIP-DEPOT-SERVICE-CANCEL (2026-09-11): si el depósito
propio deja de ser alcanzable o cambia de propietario después de programar un
servicio automático, el callback retira sólo la orden naval temporal
`stop:false` y restaura la orden real del circuito. Esto replica la conversión
de `current_order` a `Dummy` de `CheckIfShipNeedsService` sin borrar órdenes
persistentes; la regresión cubre el cambio de propietario y el destino
restaurado. #567 continúa abierta por callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-VEHICLE-SERVICE-BOOKKEEPING (2026-09-11): todo servicio
en depósito limpia ahora `breakdowns_since_last_service` y actualiza tanto la
fecha económica como `date_of_last_service_newgrf` desde el día simulado.
Esto alinea el estado que consultan la interfaz, los callbacks NewGRF y los
saves con `VehicleServiceInDepot`; las regresiones cubren el método vanilla y
el camino de servicio con catálogo runtime. #329/#567 continúan abiertas por
la semántica restante de vehículos, callbacks y aceptación visual/framebuffer.

Actualización #329/#567-NEEDS-SERVICE-DURING-BREAKDOWN (2026-09-11): el cálculo
local de `NeedsServicing` ya no cancela una revisión vencida sólo porque
`breakdown_ctr` esté activo. OpenTTD mantiene separadas la cuenta regresiva de
avería y la decisión de servicio; la regresión cubre el intervalo diario
vencido durante esa cuenta. #329/#567 continúan abiertas por la semántica
restante de vehículos, callbacks y aceptación visual/framebuffer.

Actualización #329/#567-BREAKDOWN-COUNTER (2026-09-11): al entrar en la fase
activa de una avería, cada vehículo incrementa ahora
`breakdowns_since_last_service` con el límite nativo de 255. El valor queda
disponible para la UI, callbacks NewGRF y saves, y las regresiones cubren tanto
el incremento como la saturación. La transición de aeronaves también conserva
el retorno `false` de `HandleBreakdown`, ya que no detienen su movimiento por
esta fase; #329/#567 continúan abiertas por la semántica restante de vehículos,
callbacks y aceptación visual/framebuffer.

Actualización #329/#567-BREAKDOWN-CADENCE (2026-09-11): la cuenta de
`breakdown_delay` usa ahora el ritmo de `Vehicle::HandleBreakdown`: cada dos
ticks para barcos y vehículos de carretera, y cada cuatro para trenes. La
regresión naval verifica que un tick intermedio no acorte la avería. #329/#567
continúan abiertas por la semántica restante de vehículos, callbacks y
aceptación visual/framebuffer.

Corrección #329/#567-CONVERT-RAIL-STATION-CROSSING-DEPOT (2026-09-12):
`ConvertRail` también acepta plataformas y waypoints ferroviarios, depósitos
de tren y cruces a nivel representados en `MP_STATION`, `MP_RAILWAY` y
`MP_ROAD`. Se conserva la codificación de la estación/cruce y sólo se cambia
`m8`; el preflight bloquea vehículos sobre piezas no compatibles, y la
liberación PBS limpia ahora el bit de reserva de estaciones además de vías,
cruces y portales. La conversión por arrastre de footprints completos y las
restricciones NewGRF específicas todavía requieren cobertura; #329/#567 sigue
abierta por infraestructura, callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-WATER-LOCK-OBJECT-AUTOREMOVE (2026-09-12): la
limpieza automática de `PlaceLock` ahora admite objetos `MP_OBJECT` con flag
`Autoremove`, valida la huella completa, conserva agua/clase por tile y cobra
el `ClearTile_Object` una sola vez junto con el centro y los canales de la
esclusa. Los objetos no autoremovibles siguen rechazándose sin mutar el mapa;
la regresión cubre coste, RNG, eliminación de la instancia y atomicidad.
Quedan pendientes estructuras no autoremovibles, callbacks, pathfinding,
infraestructura y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-STRUCTURE-BLOCKERS (2026-09-12): la
limpieza automática de `PlaceLock` ya conserva los errores de
`CMD_LANDSCAPE_CLEAR | Auto` para estructuras que no se pueden sobreconstruir:
casas devuelven `BuildingMustBeDemolished`, industrias `IndustryInTheWay`,
muelles/boyas/Oil Rigs sus bloqueos navales, y rampas de puente o túnel sus
errores específicos. La validación se aplica por igual a `Middle`, `Lower` y
`Upper`, mantiene preview/ejecución atómicos y no altera pools ni dinero. Las
carreteras normales (que pueden auto-removerse sólo con un único roadbit sin
tranvía), cruces, vías y callbacks de sus superficies quedan como subetapas
separadas.

Actualización #329/#567-AIRCRAFT-BREAKDOWN-LANDING (2026-09-11): el FSM de
aeronaves limpia ahora `breakdown_ctr` cuando el avión vuelve a velocidad de
suelo, equivalente a `HandleAircraftSmoke` al aterrizar. La limpieza se aplica
tanto al flujo normal como al FTA de aeropuertos, y una regresión cubre el
estado de taxi lento; #329/#567 continúan abiertas por la semántica restante
de vehículos, callbacks y aceptación visual/framebuffer.

Actualización #329/#567-SERVICE-BREAKDOWN-CHANCE (2026-09-11): el servicio en
depósito conserva ahora una cuarta parte de `breakdown_chance`, como
`VehicleServiceInDepot`, y la borra sólo cuando el setting es «averías
reducidas». El ciclo de simulación propaga ese setting a todos los vehículos
antes de los callbacks de economía y movimiento; las regresiones cubren la
propagación y el método de servicio. #329/#567 continúan abiertas por otros
criterios pendientes.

Actualización #329/#567-SERVICE-CONSIST (2026-09-11): los puntos de servicio
que poseen la flota ahora recorren `next_unit`, igual que
`VehicleServiceInDepot` recorre `Next()` hasta `HasEngineType()`. Cuando un
controlador atiende sólo la cabeza, una generación efímera detecta el servicio
y actualiza los seguidores enlazados sin repetir la operación de la cabeza. La
regresión cubre una cadena ferroviaria; #329/#567 continúan abiertas por
criterios restantes.

Actualización #329/#567-BREAKDOWN-STOP-RETURN (2026-09-11):
`Vehicle::HandleBreakdown` devuelve ahora «detenido» durante toda la avería y
consume también la primera unidad de `breakdown_delay` al pasar de contador 2
a 1, como la caída nativa. El evento visual/sonoro conserva una sola emisión
al comenzar la avería; la regresión cubre el retorno durante la cuenta activa y
el vencimiento en la misma transición. #329/#567 continúan abiertas por otros
criterios pendientes.

Actualización #329/#567-TIMETABLE-CONTROLLER-TICK (2026-09-11): el reloj de
`current_order_time` y la espera de horario avanzan sólo en la unidad que
ejecuta el controlador nativo: motor delantero, cabeza vial, barco o avión
normal. Los vagones y partes articuladas conservan sus contadores sin avanzar,
igual que los vehículos que OpenTTD no considera `IsFrontEngine()`. La regresión
comprueba una cadena ferroviaria; #329/#567 continúan abiertas por otros
criterios pendientes.

Actualización #329/#567-TIMETABLE-CLOCK-DISABLED (2026-09-11): la unidad
controladora incrementa `current_order_time` aun cuando el timetable está
desactivado, igual que los cuatro controladores nativos; el flag sólo decide
si `UpdateVehicleTimetable` interpreta ese tiempo. La regresión cubre un barco
sin timetable; #329/#567 continúan abiertas por otros criterios pendientes.

Actualización #329/#567-RUNNING-TICKS (2026-09-11): `running_ticks` se
incrementa una vez por tick en la unidad que ejecuta el controlador nativo y no
en vagones o partes articuladas; los trenes también cuentan con velocidad
residual aunque estén detenidos. El barrido económico reinicia el contador como
`OnNewEconomyDay`; el cálculo local de costos fraccionales permanece separado.
Las regresiones cubren acumulación, velocidad residual y reset; #329/#567
continúan abiertas por otros criterios pendientes.

Actualización #329/#567-ECONOMY-AGE (2026-09-11): el barrido económico
incrementa ahora `economy_age` una vez por día para cada unidad ferroviaria y
naval, y para la cabeza de cada vehículo vial o aeronave runtime. El valor se
mantiene acotado por `EconomyTime::MAX_DATE`, como `EconomyAgeVehicle`; la
regresión cubre las reglas por unidad, el límite y el slot escalonado.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-DAY-COUNTER-UNITS (2026-09-11): `day_counter` deja de
avanzar en partes articuladas de carretera, mientras conserva el incremento
para cada unidad ferroviaria/naval y para las cabezas viales y aeronaves
runtime. El callback CB32 sigue evaluándose antes del handler diario, pero el
contador persistido ahora refleja qué unidades llegan al incremento nativo.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-STOPPED-RELIABILITY-DECAY (2026-09-11): el barrido
económico degrada `reliability` también en vehículos detenidos, igual que
`CheckVehicleBreakdown` antes de comprobar `VehState::Stopped`; el estado
detenido sólo impide acumular una nueva avería. La regresión cubre un autobús
detenido dentro del slot diario. #329/#567 continúan abiertas por callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-AIRPORT-FOOTPRINT (2026-09-12): `DoBuildLock`
ya puede retirar un aeropuerto puro desde cualquier tesela de su huella, siguiendo
el contrato de `RemoveAirport`: comprueba propiedad, aviones/vehículos en toda la
huella, cobra `PR_CLEAR_STATION_AIRPORT` por tesela y conserva la operación atómica.
La ejecución limpia la huella, las animaciones NewGRF, el ruido del pueblo más
cercano y el estado FTA local antes de convertir la tesela central en agua y las
restantes en terreno. Los aeropuertos intermodales, órdenes de aeronaves fuera de
la huella y la infraestructura nativa todavía requieren subetapas propias.
#329/#567 continúa abierta por esas diferencias y por callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-SHIP-DOCK-RANDOM-RESTORE (2026-09-12): al demoler un
muelle se restaura la pieza acuática con el consumo nativo de `Random()` para
canal/río y `MAP4 = 0` para mar. La regresión usa demolición desde la sección
de agua y comprueba el byte persistido y el estado final del RNG. #329/#567
continúan abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-LOADING-RELIABILITY-DECAY (2026-09-11): con averías
reducidas, la ventana de carga o transferencia se trata como un vehículo
detenido para el riesgo, pero no salta el decaimiento diario de fiabilidad.
La regresión cubre una ventana de carga con velocidad residual: la fiabilidad
decae y `breakdown_chance` no avanza. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-VEHICLE-VALUE-DEPRECIATION (2026-09-11): las unidades
compradas guardan ahora el coste de compra en `Vehicle::value`, y el barrido
económico aplica `value -= value >> 8` cada ocho incrementos del contador
diario, como `DecreaseVehicleValue`. Las piezas articuladas conservan valor
cero, igual que las piezas materializadas por el constructor nativo; la
regresión cubre el importe inicial y la depreciación escalonada. #329/#567
continúan abiertas por valoración de activos, callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-ASSET-VALUE (2026-09-11): el cálculo de patrimonio de
la compañía usa ahora el valor contable persistido de cada unidad y suma
también los vagones, como `CalculateCompanyAssetValue` (`v->value * 3 >> 1`).
La regresión cubre una composición con cabeza y vagón y valores depreciados.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-SELL-BOOK-VALUE (2026-09-11): la venta consulta ahora
`Vehicle::value` persistido: la cabeza para carretera, barco y aeronave, y cada
unidad eliminada de un consist ferroviario, igual que `CmdSellVehicle` y
`CmdSellRailWagon`. La regresión cubre un vehículo vial con depreciación y la
venta de una composición cabeza+vagón. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-FREE-WAGON-ECONOMY-HANDLER (2026-09-11): un vagón
ferroviario suelto conserva sus actualizaciones de edad económica, contador y
valor, pero ya no ejecuta por error el handler reservado al `IsFrontEngine()`:
no acumula averías ni dispara servicio automático mientras permanece libre en
el depósito. La regresión cubre un `ENGINE_WAGON_COAL` sin `prev_unit`.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-BLOCKED-ROAD-BREAKDOWN (2026-09-11): el barrido
económico respeta el gate nativo de `RoadVehicle::OnNewEconomyDay`: un vehículo
de carretera con `blocked_ctr != 0` no ejecuta la comprobación de averías en
ese slot, aunque conserva el envejecimiento, servicio y órdenes del handler.
La regresión verifica que fiabilidad, acumulador y avería activa no cambian
mientras el autobús está bloqueado. #329/#567 continúan abiertas por callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-CANCEL-ROAD-SERVICE (2026-09-12): si el depósito deja
de ser alcanzable mientras un vehículo vial tiene una orden implícita de
servicio (`stop:false`), el barrido la retira y restaura la siguiente orden del
circuito, igual que `CheckIfRoadVehNeedsService` convierte el `current_order`
temporal nativo en `Dummy`. Las órdenes persistentes (`stop:true`) no se
alteran. La regresión cubre la demolición del único depósito entre dos slots
económicos. #329/#567 continúan abiertas por callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-ROAD-SERVICE-SETTING (2026-09-12): el servicio
automático vial respeta `Company::settings.vehicle.servint_roadveh == 0` y no
inserta una orden de depósito aunque el vehículo ya haya superado su intervalo
local. La regresión usa un depósito alcanzable y una compañía con servicio
vial desactivado. #329/#567 continúan abiertas por callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-IN-HANGAR-SERVICE (2026-09-12): el handler
económico revisa una aeronave normal que ya está dentro de un hangar, respetando
`servint_aircraft` y la misma evaluación de intervalo/averías. La regresión
comprueba que restaura fiabilidad, limpia `needs_servicing` y reduce el
acumulador de averías en el slot correcto. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-SERVICE-ORDER (2026-09-12): si la aeronave
tiene servicio pendiente y su orden actual apunta a un aeropuerto compatible
con hangar, el barrido inserta una orden temporal `stop:false` hacia la bahía
de ese aeropuerto y conserva la orden persistente original. La regresión cubre
un aeropuerto pequeño y una aeronave en vuelo con destino al mismo. La búsqueda
de un hangar alternativo cuando no hay un aeropuerto objetivo válido permanece
como subetapa separada. #329/#567 continúan abiertas por callbacks, pathfinding
y aceptación visual/framebuffer.

Actualización #329/#567-ROAD-SERVICE-LOADING (2026-09-12): el servicio
automático vial ya no interrumpe una ventana de carga ni una transferencia de
carga activa. Es el gate de `NeedsAutomaticServicing` que ya respetaban los
handlers naval y aéreo; la regresión comprueba que un autobús debido conserva
su orden de estación durante la carga. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-DEPOT-ORDER-PRIORITY (2026-09-12): una nave con
orden de depósito hacia el mismo depósito conserva la prioridad de
`CheckShipStayInDepot`: se reingresa/procesa `halt` antes de esperar a otra nave
que esté usando la boca. La regresión evita que el gate global de concurrencia
de salidas congele esa orden. #329/#567 continúan abiertas por callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-CRASHED-SERVICE-GATE (2026-09-12):
`requires_service_with` descarta explícitamente vehículos accidentados, igual
que `NeedsServicing` nativo, incluso si un estado restaurado conserva
`running=true` y un intervalo vencido. La regresión cubre tanto la consulta
como el barrido económico sin insertar una orden temporal. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-HANGAR-INDEX (2026-09-12): el índice espacial
de depósitos aéreos sólo conserva piezas `Hangar`/`Heliport`; las pistas,
aprons y terminales de un aeropuerto ya no pueden ser elegidas por una orden
sin destino ni por “ir al depósito más cercano”. La regresión coloca un apron
más próximo que el hangar y verifica ambas consultas. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-STALE-SERVICE (2026-09-12): una orden aérea
 temporal `stop:false` se retira cuando la pieza de hangar que tenía como
 destino desaparece, reproduciendo la conversión nativa de `current_order` a
 `Dummy` y conservando el circuito persistente. La regresión demuele el único
 hangar después de insertar el servicio. #329/#567 continúan abiertas por
 callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-ALTERNATE-HANGAR (2026-09-12): el fallback de
aviones sin órdenes ya no escoge cualquier pieza de hangar del mapa. La
búsqueda indexada filtra por estación aeroportuaria válida, propietario y
compatibilidad avión/helipuerto antes de elegir la alternativa más cercana,
como `FindNearestHangar` nativo. Las regresiones cubren un helipuerto
incompatible y un aeropuerto de otra compañía más próximo. #329/#567
continúan abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-HANGAR-DISTANCE (2026-09-12): el selector
filtrado de hangares compara distancia cuadrática, igual que `DistanceSquare`
en `FindNearestHangar`, en vez de alterar la elección con una distancia
Manhattan. La regresión usa un candidato vertical y otro diagonal cuyo orden
Manhattan y cuadrático difiere. #329/#567 continúan abiertas por callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-MISSING-ORDERS-DEPOT (2026-09-12): cuando una
aeronave sin órdenes pierde su aeropuerto objetivo, el fallback conserva la
selección de hangar compatible y crea una orden temporal de depósito con
`Halt`, equivalente a `HandleMissingAircraftOrders` → `SendToDepot` nativo.
Así la llegada puede detener la aeronave en vez de dejar sólo un destino
volátil. La regresión cubre la creación de la orden y la ruta hasta el hangar.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-AIRCRAFT-TAXI-HANGAR-ARRIVAL (2026-09-12): el FSM aéreo
transiciona explícitamente de `Taxi` a `InHangar` cuando la ruta termina sobre
una pieza de hangar. Se normalizan velocidad y altitud antes de procesar la
orden de depósito, evitando dejar la aeronave visible como si siguiera en
rodaje. La regresión cubre la llegada con ruta vacía al hangar. #329/#567
continúan abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-AIRCRAFT-HANGAR-MOTION-RESET (2026-09-12): la entrada
al hangar limpia también `progress` y `subspeed`, como
`HandleAircraftEnterHangar`; la misma normalización se reutiliza cuando el
aterrizaje termina directamente sobre una bahía. La regresión verifica que no
queden fracciones de movimiento que reaparezcan al siguiente despegue.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación visual/
framebuffer.

Actualización #329/#567-SHIP-DEPOT-TIE-BREAK (2026-09-12): el selector local de
depósito naval conserva ahora el desempate por `DepotID` de `MAP2`, igual que
`FindClosestShipDepot` al recorrer el `DepotPool` nativo. Antes un empate de
`DistanceSquare` se resolvía por coordenadas `y/x`, por lo que dos depósitos
alcanzables equidistantes podían elegir una boca distinta. La regresión cubre
dos depósitos propios con IDs y posiciones invertidos. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-OWNER-TRANSFER (2026-09-12): la adquisición de
una compañía lee y escribe el propietario de infraestructura mediante los
cinco bits bajos de `MAPO` (`m1 & 0x1F`), como `ChangeTileOwner_Water` nativo.
Esto transfiere las dos piezas de un depósito naval aunque `m1` tenga clase de
agua o `DockingTile`, conservando esos flags. La regresión cubre una compañía
quebrada con un depósito naval y el flag de atraque activo. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-DOCKING-RESET (2026-09-12): la materialización de
agua común reinicia `DockingTile` antes de que los comandos navales recalculen
los amarres, igual que `MakeWater` nativo. Esto evita dejar una sección de
canal marcada como punto de atraque tras reemplazar una instalación; la
regresión conserva owner/clase y exige el bit 7 limpio. Las altas y bajas de
depósitos y muelles mantienen su recálculo explícito de vecinos. #329/#567
continúan abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-DEPOT-WATER-TYPE (2026-09-12): las lecturas de
`DepotID`, el asignador del pool y el índice espacial validan ahora el
`WaterTileType::Depot` de `m5` además de `TileKind::ShipDepot`, como
`IsShipDepotTile` nativo. Una tesela semánticamente marcada como depósito pero
con agua común ya no consume un slot ni se ofrece como destino de servicio;
la regresión cubre consultas lineales e indexadas. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-BUOY-RAW-CONTRACT (2026-09-12): la construcción
de boyas materializa el contrato de `MakeBuoy`/`MakeStation`: asigna un
`StationID` global, conserva owner/clase de agua, limpia `DockingTile` y los
campos crudos de la estación, preserva el nibble bajo de `MAPT` y registra el
waypoint como neutral. La demolición por otra compañía restaura el canal sin
alterar su owner. Las regresiones cubren la huella raw completa y la propiedad
neutral. #329/#567 continúan abiertas por callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-SHIP-DOCK-RAW-CONTRACT (2026-09-12): la construcción
de muelles materializa las dos piezas con el contrato de `MakeDock`: conserva
el nibble bajo de `MAPT`, limpia `DockingTile` y los campos raw, fuerza
`WaterClass::Invalid` en tierra y conserva la clase en agua. El `StationID` y
el owner de la compañía siguen compartidos por ambas partes. La regresión
cubre payload residual en las dos teselas. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-RANDOM-BITS (2026-09-12): la construcción de
canal/río consume ahora la palabra global `Random()` que usa OpenTTD y persiste
su byte bajo en `MAP4` (`m3hi`) mediante una primitiva de agua con bits
aleatorios explícitos. La regresión comprueba canal y río, el byte raw y la
posición final del RNG. #329/#567 continúan abiertas por callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-SHIP-DEPOT-RANDOM-RESTORE (2026-09-12): al demoler un
depósito naval, cada sección restaurada de canal/río consume ahora su palabra
global `Random()` y guarda el byte bajo en `MAP4`; el mar mantiene el valor
cero. La regresión cubre la demolición desde la sección opuesta y el orden de
consumo río→canal. #329/#567 continúan abiertas por callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-SHIP-BUOY-RANDOM-RESTORE (2026-09-12): retirar una
boya restaura el agua subyacente con el consumo nativo de `Random()` para
canal/río y `MAP4 = 0` para mar. La regresión cubre la eliminación por otra
compañía y el estado final del RNG; la neutralidad del waypoint se conserva.
#329/#567 continúan abiertas por callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-WATER-OBJECT-RANDOM-RESTORE (2026-09-12): la auto-
demolición de objetos acuáticos restaura cada tile con `MakeWaterKeepingClass`
y consume una palabra `Random()` por cada sección de canal/río; el mar conserva
`MAP4 = 0`. La regresión usa un objeto autoremove de dos teselas dentro de la
huella de un depósito y comprueba el avance total del RNG. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#499-INDUSTRY-RANDOM-RESTORE (2026-09-12): el cierre mensual
de una Oil Rig restaura cada tile acuático con el consumo nativo de
`MakeWaterKeepingClass`; canal y río avanzan el RNG global y guardan el byte
bajo en `MAP4`, mientras el mar no consume una tirada. La regresión cubre la
estación neutral y el estado final del stream. #329/#499 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-ELEVATED-SEA-RESTORE (2026-09-12): la ruta común
de restauración convierte el mar con `z > 0` en canal, tal como
`MakeWaterKeepingClass`, y consume el `Random()` correspondiente para `MAP4`;
el mar plano sigue sin tirada. La regresión usa una estación sobre cuatro
esquinas elevadas y verifica clase, byte raw y RNG final. #329/#567 continúan
abiertas por callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-SLOPE-RESTORE (2026-09-12): la ruta común de
restauración deja `MP_CLEAR` para canales/mares sobre pendientes y conserva
únicamente ríos con dirección inclinada válida, como `MakeWaterKeepingClass`.
La rama de suelo limpia los planos raw y no avanza el RNG; la regresión cubre
`SLOPE_NE`, payload residual y estado final. #329/#567 continúan abiertas por
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-INDUSTRY-WATER-RESTORE (2026-09-12): el cierre mensual
de Oil Rig comparte la misma decisión de clase y limpieza que las rutas de
transporte. Una pieza de canal inclinada vuelve a `MP_CLEAR` sin consumir RNG;
las piezas de agua válidas conservan la tirada nativa. La regresión cubre la
pendiente y el payload residual. #329/#567 continúan abiertas por callbacks,
pathfinding, docking y aceptación visual/framebuffer.

Actualización #329/#567-WATER-CLEAR (2026-09-12): `ClearTile` deja de tratar
agua clara y costa como una tesela genérica. La nueva ruta valida ownership de
canales, impide limpiar bajo un vehículo, restablece el payload de `MP_CLEAR`,
reactiva vecinos no inundables y calcula los precios nativos de agua, canal y
rough. Las esclusas de tres piezas quedan como subetapa independiente.

Actualización #329/#567-WATER-LOCK-LIFECYCLE (2026-09-12): `PlaceLock` y
`ClearTile` ya materializan/resuelven las tres partes `Lower/Middle/Upper`,
codifican la orientación completa de `m5`, conservan clase y owner del agua,
normalizan el payload raw y validan vehículos en toda la huella. La demolición
restaura `Upper` y `Lower` en el orden RNG nativo, trata el `Middle` río como
`MakeRiver` y el resto como `DoClearSquare`, con precios `PR_BUILD_LOCK` y
`PR_CLEAR_LOCK`. #329/#567 siguen abiertas por auto-clear sobre tierra,
infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-FREEFORM-EDGES (2026-09-12): `ClearTile_Water`
rechaza agua clara en los cuatro márgenes cuando `construction.freeform_edges`
está desactivado, y `PlaceLock` aplica el mismo contrato al extremo `Lower`
derivado de la pendiente. La costa conserva su ruta nativa sin ese guard.
Quedan pendientes los puentes/estructuras y objetos dentro de la limpieza
automática, además de los criterios de infraestructura, callbacks, pathfinding
y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-AUTO-CLEAR (2026-09-12): `PlaceLock` ahora
porta la secuencia de `DoBuildLock`: limpia el centro siempre, limpia cada
extremo terrestre antes de convertirlo en canal y deja intacto el owner/clase
de los extremos que ya eran agua. Se añadieron `PR_CLEAR_GRASS`,
`PR_CLEAR_ROCKS`, `PR_CLEAR_FIELDS`, `PR_CLEAR_TREES`, el recargo de nieve,
`PR_BUILD_CANAL` y el multiplicador tropical de árboles; la operación mantiene
preflight atómico y cobra el coste compuesto una sola vez. Las regresiones
cubren dos extremos de hierba, un extremo boscoso y el reset raw del centro.
Siguen pendientes los puentes, estructuras y objetos autoremove, callbacks,
pathfinding, infraestructura y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-RAIL-AUTOCLEAR (2026-09-12):
`PlaceLock` porta la rama de `ClearTile_Road | Auto`: una carretera normal con
exactamente un `roadbit`, sin tranvía y con propietario permitido se limpia
como suelo y suma `PR_CLEAR_ROAD`; cruces, trazados múltiples y tranvías
devuelven `MustRemoveRoadFirst`. Las vías no se demuelen en automático: una
vía normal devuelve `MustRemoveRailroadTrack`, mientras una tesela con señales
conserva `BuildingMustBeDemolished`; el chequeo de ownership ocurre antes de
ambos errores ferroviarios. Preview y ejecución comparten el preflight y
siguen siendo atómicos.

Actualización #329/#567-WATER-LOCK-BRIDGE-CLEARANCE (2026-09-12): `PlaceLock`
ya inspecciona un puente sobre cada parte con la altura derivada de la rampa,
la fundación y el eje persistido, equivalente a `GetBridgeHeight` más
`GetSouthernBridgeEnd`. Aplica los mínimos nativos `Middle=2`, `Lower=3` y
`Upper=2`, devuelve un error específico sin mutar ni cobrar cuando el tablero
queda bajo y conserva el vano cuando el despeje es suficiente. Las regresiones
cubren puente real bajo/alto y la altura plana de la rampa. Siguen pendientes
estructuras y objetos autoremove, callbacks, pathfinding, infraestructura y
aceptación visual/framebuffer.

Actualización #329/#567-WATER-SHIP-DEPOT-CLEAR-GUARDS (2026-09-12): `PlaceShipDepot`
respeta ahora los guards que aporta `ClearTile_Water | Auto`: no construye sobre
vehículos, bordes restringidos ni canales de otra compañía, y consulta el pool
de depósitos antes de inspeccionar la limpieza de la huella. La materialización
y la restauración también reactivan los estados no inundables de las ocho
vecinas, equivalente a `DoClearSquare`. Las regresiones cubren ambas secciones,
preview/aplicación atómicos, borde y propiedad. #329/#567 siguen abiertas por
infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-SHIP-DEPOT-WATER-CLASS (2026-09-12): la
construcción naval captura la clase de agua de cada parte antes de ejecutar la
auto-limpieza, igual que `CmdBuildShipDepot` captura `wc1/wc2`. Así un mar
elevado que `MakeWaterKeepingClass` convierte temporalmente en canal no cambia
la clase raw que recibe el depósito. La regresión cubre un objeto autoremove
sobre mar elevado y conserva `Sea` en la sección materializada. #329/#567 siguen
abiertas por infraestructura, callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-WATER-LOCK-OBJECT-CLEAR (2026-09-12): `DoBuildLock`
llama a `CMD_LANDSCAPE_CLEAR` sin `DoCommandFlag::Auto`; la esclusa ahora
permite retirar un objeto no-autoremovible cuando su demolición manual es
válida, conserva la restauración acuática y cobra el coste del objeto una sola
vez. Los objetos `CannotRemove` siguen bloqueando de forma atómica. La
regresión cubre ambos contratos y el avance del RNG. #329/#567 siguen abiertas
por infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-ECONOMY-CLEAR-RAIL-ROAD-INDEX (2026-09-12): la tabla
local vuelve a alinear `PR_CLEAR_RAIL` (27) y `PR_CLEAR_ROAD` (41) con
`economy_type.h`/`pricebase.h`; antes `road_clear_cost` leía por error el
reembolso ferroviario. Se expone también la fórmula `RailClearCost`, con el
límite nativo de tres cuartos del coste de construcción. La prueba de precios
cubre ambos índices antes de reutilizarlos en limpieza compuesta. #329/#567
siguen abiertas por infraestructura, callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-WATER-LOCK-RAIL-CLEAR (2026-09-12): `DoBuildLock`
retira ahora vía normal y señales mediante la rama manual de
`ClearTile_Track`, incluyendo el reembolso por pieza, el coste de señales y la
actualización de vecinos/glob de señales. Depósitos, túneles, puentes y otros
subtipos siguen bloqueados. La regresión cubre una vía señalizada, su payload
raw y el coste compuesto. #329/#567 siguen abiertas por infraestructura,
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-CLEAR (2026-09-12): la rama manual de
`ClearTile_Road` ya permite que `DoBuildLock` retire todas las piezas de una
tesela normal, incluyendo overlays de tranvía y tipos NewGRF. El precio se
calcula por pieza y aplica el reembolso nativo de tranvía; subtipos de cruce,
depósito, túnel y puente siguen separados por sus contratos de demolición.
Las regresiones cubren carretera compuesta, carretera+tranvía y atomicidad de
un subtipo no soportado. #329/#567 siguen abiertas por infraestructura,
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-DEPOT-CLEAR (2026-09-12):
`ClearTile_Road` manual ya puede retirar un depósito de carretera durante
`DoBuildLock`, aplicando `PR_CLEAR_DEPOT_ROAD`, comprobando propiedad y
desregistrando la fila `DEPT` antes de materializar el agua de la esclusa. El
acceso vial vecino permanece intacto; el costo de canal se conserva para un
depósito que ocupa un extremo. Depósitos ferroviarios, cruces, túneles,
puentes y estaciones siguen en subetapas propias. #329/#567 siguen abiertas
por infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-CROSSING-CLEAR (2026-09-12):
`ClearTile_Road` manual ya puede retirar un cruce a nivel durante
`DoBuildLock`, recorriendo carretera y tranvía en el orden nativo, cobrando dos
piezas por red y validando sus propietarios codificados en `m7`/`m3`. El
ferrocarril subyacente se reemplaza por la tesela de agua de la esclusa, y la
regresión cubre cruce simple y cruce con tranvía. Depósitos, túneles, puentes y
estaciones siguen en subetapas propias. #329/#567 siguen abiertas por
infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-RAIL-DEPOT-CLEAR (2026-09-12):
`ClearTile_Track` manual ya puede retirar un depósito ferroviario durante
`DoBuildLock`, aplicando `PR_CLEAR_DEPOT_TRAIN`, comprobando propiedad y
desregistrando la fila `DEPT` antes de materializar el agua de la esclusa. La
vía de acceso permanece intacta y la actualización de vecinos ferroviarios se
conserva al convertir la tesela. El guard general de vehículos de `DoBuildLock`
también cubre un tren detenido en esa huella. Las ramas de cruces, túneles,
puentes y estaciones siguen en subetapas propias. #329/#567 siguen abiertas
por infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-STOP-CLEAR (2026-09-12):
`ClearTile_Station` manual ya puede retirar una parada de bus o camión durante
`DoBuildLock`, usando el precio vanilla de su categoría o el multiplicador de
limpieza de `RoadStopSpec`. La entidad local `Station` se actualiza junto con
sus estados por tesela, se quita el tile del scheduler de animación y, si se
demuele el ancla de una estación unida, se promueve otra tesela y se redirigen
las órdenes y subsidios que apuntaban a la coordenada retirada. El preview y
la ejecución comparten ownership, coste y atomicidad; muelles, boyas,
estaciones ferroviarias, waypoints, aeropuertos, túneles y puentes siguen en
subetapas separadas. #329/#567 continúan abiertas por infraestructura,
callbacks, pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-ROAD-WAYPOINT-CLEAR (2026-09-12):
`ClearTile_Station` manual ya puede retirar un `RoadWaypoint` durante
`DoBuildLock`, aplicando la categoría nativa `PR_CLEAR_STATION_TRUCK` y
eliminando su entidad `Station` y registro de animación antes de materializar
el agua. La rama permanece separada de las paradas de carga: waypoints
ferroviarios, estaciones ferroviarias, muelles, boyas, aeropuertos, túneles y
puentes conservan sus contratos específicos. #329/#567 continúan abiertas por
infraestructura, callbacks, pathfinding y aceptación visual/framebuffer.

Corrección de contrato #329/#567-WATER-LOCK-RAIL-WAYPOINT-PRICE (2026-09-12):
el waypoint ferroviario de `ClearTile_Station` usa `PR_CLEAR_WAYPOINT_RAIL`
(base 80), no el precio de limpieza de una parada vial. El índice y su
recálculo de inflación quedan expuestos mediante `rail_waypoint_clear_cost`, y
la prueba del lock verifica el precio nativo específico.

Corrección de contrato #329/#567-ECONOMY-PRICE-INDEX-LAYOUT (2026-09-12):
la tabla `PriceIndex` vuelve a usar las posiciones nativas de `pricebase.h`
para despejes de depósitos/estaciones, waypoints, canales, esclusas e
infraestructura. También se rellenan las bases que faltaban, evitando que un
despeje recién implementado consulte accidentalmente una entrada cero.

Corrección de contrato #329/#567-WATER-DOCK-PRICE (2026-09-12): `CmdBuildDock`
ya cobra `PR_BUILD_STATION_DOCK` y `RemoveDock` devuelve
`PR_CLEAR_STATION_DOCK`, en vez de reutilizar el precio ferroviario de una
estación y `CLEAR_TILE_COST`. La regresión cubre construcción, demolición
desde la pieza acuática y limpieza de un objeto autoremove. El ciclo de docks
unidos, estaciones ferroviarias y aeropuertos sigue en subetapas separadas;
#329/#567 continúa abierta por infraestructura, callbacks, pathfinding y
aceptación visual/framebuffer.

Actualización #329/#567-WATER-BUOY-LIFECYCLE (2026-09-12): las boyas ya usan
`PR_BUILD_WAYPOINT_BUOY` y `PR_CLEAR_WAYPOINT_BUOY` en construcción y
demolición, en lugar del precio aproximado de una estación o del despeje
genérico. `ClearTile_Station` restaura la clase de agua, limpia el pool local
y respeta `HasStationInUse(..., false)`: una orden de otra compañía devuelve
`BuoyInUse` sin mutar el mapa. `PlaceLock` aplica la misma retirada manual,
conserva la clase en la parte central y convierte una boya de extremo en canal,
como `DoBuildLock`/`RemoveBuoy`. Estaciones ferroviarias, aeropuertos,
túneles y puentes siguen en subetapas separadas; #329/#567
continúan abiertas por infraestructura, callbacks, pathfinding y aceptación
visual/framebuffer.

Actualización #329/#567-WATER-LOCK-DOCK-CLEAR (2026-09-12): `DoBuildLock`
ahora puede retirar un muelle mediante `RemoveDock` sin cobrarlo dos veces,
incluso cuando el cursor apunta a la pieza acuática. El preflight reevalúa los
extremos después de limpiar el centro, conserva la clase de agua de la pieza
que vuelve a ser agua y elimina la huella completa de la entidad `Station`.
Las regresiones cubren ambos accesos y el precio compuesto. Estaciones
ferroviarias, aeropuertos y otras huellas multi-tesela siguen en subetapas
separadas; #329/#567 continúa abierta por infraestructura, callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-RAIL-WAYPOINT (2026-09-12):
`ClearTile_Station` ya retira un `RailWaypoint` 1×1 durante `DoBuildLock`,
usando `PR_CLEAR_WAYPOINT_RAIL`, validando la compañía propietaria y limpiando
la entidad de estación junto con la invalidación de la red ferroviaria. La
regresión cubre el despeje del centro y el coste compuesto. Las estaciones
ferroviarias multi-tesela, aeropuertos, túneles y puentes siguen en subetapas
separadas; #329/#567 continúa abierta por infraestructura, callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-RAIL-STATION-1X1 (2026-09-12):
`ClearTile_Station` ya retira una estación ferroviaria de una tesela desde
`DoBuildLock`, usando `PR_CLEAR_STATION_RAIL`, respetando la propiedad y
eliminando la entrada lógica de `Station`. Esta fue la primera subetapa
verificable; la limpieza de la huella completa queda registrada debajo.

Actualización #329/#567-WATER-LOCK-RAIL-STATION-FOOTPRINT (2026-09-12):
`RemoveRailStation` ya se modela sobre toda la huella ferroviaria asignada a
la entidad lógica, no sólo sobre la tesela apuntada por el cursor. El
preflight cobra `PR_CLEAR_STATION_RAIL` por tesela, rechaza vehículos en
cualquier parte de la huella y mantiene la operación atómica. Al ejecutar,
la tesela central vuelve a agua para `MakeLock` y las demás se limpian a
terreno; la planificación secuencial vuelve a evaluar los extremos y sólo
añade canal cuando esos extremos todavía eran terreno. Las estaciones
intermodales, reservas ferroviarias, reembolsos de vía e infraestructura
nativa todavía requieren subetapas propias. #329/#567
continúa abierta por esas diferencias y por aeropuertos, callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-WATER-LOCK-SHIP-DEPOT-CLEAR (2026-09-12):
`DoBuildLock` ya entra en `RemoveShipDepot` cuando una de sus partes ocupa
el centro o un extremo. Se comprueban las dos secciones y sus vehículos,
se restaura la huella completa con la clase y aleatoriedad nativas y el coste
`PR_CLEAR_DEPOT_SHIP` se suma una sola vez; la sección que vuelve a ser agua
se reevalúa antes de `MakeLock`. La infraestructura de agua, la variante
`Bankrupt` y los callbacks/órdenes de servicio todavía requieren contratos
propios. #329/#567 continúa abierta por infraestructura, callbacks,
pathfinding y aceptación visual/framebuffer.

Actualización #329/#567-TUNNEL-BRIDGE-CLEAR (2026-09-12):
`ClearTile` ya demuele túneles y puentes completos desde cualquiera de sus
bocas, con preflight compartido entre preview y ejecución. La operación
comprueba ambos extremos, propiedad, vehículos y geometría almacenada; cobra
`PR_CLEAR_TUNNEL`/`PR_CLEAR_BRIDGE` más la infraestructura de carretera, tranvía
o vía por toda la longitud. En puentes conserva el terreno inferior y sólo
quita los bits del tablero; en túneles locales limpia el tramo sintético y
mantiene terreno intermedio importado. También elimina el registro JGR de la
pareja cuando puede decodificar sus índices y actualiza vecinos/señales
ferroviarios. La construcción, reservas y pathfinding completo de túneles y
puentes aún requieren contratos propios; #329/#567 continúa abierta por esas
diferencias y por callbacks, aceptación visual y framebuffer.

Corrección #329/#567-TUNNEL-BRIDGE-RESERVATIONS (2026-09-12): al demoler
una boca ferroviaria se liberan antes del cambio de tipo las reservas PBS de
`m2_hi`, el conjunto efímero `reservation_tiles_active` y los
`reserved_steps` de los trenes. Las señales PBS reciben la transición a rojo
y el terreno ferroviario que un puente deja debajo no se incluye en la
liberación. La infraestructura de túneles/puentes, sus reservas de tránsito y
el pathfinding importado aún requieren cobertura adicional; #329/#567 continúa
abierta por esas diferencias, callbacks, aceptación visual y framebuffer.

Corrección #329/#567-BRIDGE-END-TYPE-GUARD (2026-09-12): los resolvers de
`RoadBridge` y `RailBridge` ahora exigen que la rampa encontrada conserve el
mismo tipo de tesela y el mismo `TunnelBridgeTransportType` codificado en
`m5`. Un save con rampas opuestas de carretera y ferrocarril en la misma línea
ya no puede producir un enlace lógico cruzado para pathfinding, señales o
demolición. El salto rampa→rampa vial ya estaba implementado; esta subetapa
corrige el contrato compartido sin duplicarlo. #329/#567 continúa abierta por
la infraestructura, reservas de tránsito, pathfinding importado, callbacks,
aceptación visual y framebuffer restantes.

Corrección #329/#567-TUNNEL-PATH-WORMHOLE (2026-09-12): A* y YAPF ya
atraviesan túneles vanilla cuyo save conserva sólo las dos bocas y deja el
terreno del vano fuera de la red. El enlace exige `MP_TUNNELBRIDGE`, tipo de
transporte y portal opuesto compatibles; YAPF sólo salta desde la dirección
codificada en `m5` y retoma la dirección exterior de la segunda boca. La
representación local que materializa el corredor conserva continuidad axial,
sin convertir sus teselas interiores en portales falsos. Las bocas de carretera
también restringen la entrada al lado exterior, y los puentes viales dejan de
aceptar entradas laterales. Las pruebas cubren túnel de carretera y ferrocarril
sin superficie intermedia, eje vertical materializado y entrada lateral
rechazada. #329/#567 continúa abierta por reservas de tránsito, movimiento,
callbacks, variantes de túnel/puente y aceptación visual/framebuffer restantes.

Corrección #329/#567-TUNNEL-PBS-WORMHOLE (2026-09-12): la topología de
señales/PBS ya trata el enlace vanilla entre bocas como un hop dirigido.
`rail_neighbors`, selección de pista y `TryReservePath` atraviesan el vano
sin inventar teselas de superficie; la posición segura no corta la reserva
en la primera boca. Los túneles locales materializados siguen usando sus
vecinos axiales y no se convierten en wormholes falsos. La cobertura incluye
una ruta PBS sin terreno intermedio y verifica que la reserva alcance la boca
opuesta y la vía exterior. #329/#567 continúa abierta por reservas persistidas
en m5, movimiento, callbacks, variantes de túnel/puente y aceptación
visual/framebuffer restantes.

Corrección #329/#567-TUNNEL-MOTION-DIRECTION (2026-09-12): el movimiento,
la predicción de salida de señal y el cálculo de pendiente ferroviaria ya
conservan el rumbo físico al consumir un hop vanilla/JGR no adyacente. La
orientación de sprite y la pose de cada unidad del consist reconstruyen la
salida adyacente siguiente, en vez de interpretar la pareja de bocas como
`DIR_NE`; la reversa en una boca también consulta ese mismo contrato. Las
regresiones cubren avance boca→boca, renderer con historial, poses de consist
y reversa detenida. #329/#567 continúa abierta por reservas persistidas en
m5, movimiento completo de variantes, callbacks y aceptación visual/framebuffer
restantes.

Corrección #329/#567-TUNNEL-BRIDGE-PBS-M5 (2026-09-12): las reservas PBS de
`RailTunnel` y `RailBridge` ya respetan `HasTunnelBridgeReservation`: se
persisten en `m5` bit 4, se reflejan en las dos bocas, se consideran al
detectar conflictos y se liberan como pareja al vaciar o demoler la
infraestructura. `m2_hi` continúa reservado para `MP_RAILWAY` plano; la
dirección de `m5` determina el `TrackBits` X/Y de la rampa. Las regresiones
cubren puente, túnel, round-trip SAV y liberación de los dos extremos.
#329/#567 continúa abierta por ownership de reservas, reemplazos, callbacks,
variantes de puente/túnel y aceptación visual/framebuffer restantes.

Corrección #329/#567-TUNNEL-BRIDGE-REPLACEMENT (2026-09-12): el preflight de
construcción ahora distingue la rama nativa de reemplazo de puente: sólo
permite cambiar el tipo visual cuando las dos bocas actuales forman exactamente
el par solicitado, conserva PBS en `m5` y mantiene el preview alineado con la
ejecución. Una boca de otro puente, un túnel o un vano que cruza el trazado
devuelve `MustDemolishBridgeFirst`/`MustDemolishTunnelFirst` sin mutar mapa ni
dinero. #329/#567 continúa abierta por ownership más amplio, variantes,
callbacks y aceptación visual/framebuffer restantes.

Corrección #329/#567-CONVERT-RAIL-TUNNEL-BRIDGE (2026-09-12): `ConvertRail`
ya reconoce `MP_TUNNELBRIDGE` ferroviario y resuelve la pareja completa de
bocas, tanto en túneles locales materializados como en puentes con vano de
agua. El cambio de `m8` se aplica a ambos extremos, el coste usa toda la
longitud nativa del enlace y el preview comparte propiedad, fondos y bloqueo
por vehículos cuando la nueva red es incompatible. Las reservas PBS de una
boca se liberan si el consist deja de tener potencia con el nuevo tipo; el
vano/terreno intermedio no se reescribe. #329/#567 continúa abierta por
estaciones, depósitos, infraestructura, callbacks, pathfinding restante y
aceptación visual/framebuffer.

Corrección #329/#567-RAIL-DEPOT-RAILTYPE (2026-09-12): la construcción de un
depósito ferroviario ahora escribe en `m8` el tipo de vía seleccionado, igual
que `MakeRailDepot`, en vez de dejar siempre el valor heredado del terreno.
Esto mantiene coherentes la compra de motores, la conversión posterior y el
round-trip SAV; la conexión automática con la vía vecina conserva su propio
tipo. #329/#567 continúa abierta por contadores de infraestructura, callbacks,
arrastre de áreas y aceptación visual/framebuffer.

Corrección #329/#567-RAIL-CONVERT-COST (2026-09-12): `ConvertRail` ya calcula
`RailConvertCost(from, to)` con los precios de construcción/retirada y los
multiplicadores Action0 runtime, recuperando los valores vanilla 8/12/16/24
cuando no hay override. La conversión plana cobra por cada `TrackBit`, mientras
que túneles y puentes cobran por toda la longitud del enlace; preview y
ejecución comparten el mismo preflight de fondos. #329/#567 continúa abierta
por contadores de infraestructura, callbacks, arrastre de áreas y aceptación
visual/framebuffer.

Corrección #329/#567-RAILTYPE-BUILD-COST (2026-09-12): construcción y limpieza
ferroviaria ya resuelven el multiplicador efectivo por railtype, incluyendo
los valores vanilla de eléctrica, monorail y maglev cuando `Action0` no aporta
override. El coste de `PlaceRail`, depósitos y limpieza de túnel/puente o lock
queda alineado con el mismo contrato que usa `RailConvertCost`. #329/#567
continúa abierta por contadores de infraestructura, callbacks, arrastre de
áreas y aceptación visual/framebuffer.

Corrección #329/#567-RAILTYPE-TUNNEL-BRIDGE-PLACEMENT (2026-09-12): las bocas
de túnel y rampas de puente ferroviarias ahora persisten en `m8` el railtype
activo, como `MakeRailTunnel` y `MakeRailBridgeRamp`. El reemplazo visual de
un puente conserva su reserva PBS, pero rechaza cambiar de railtype sin
demolición, igual que el comando nativo; el tramo sintético del túnel no se
trata como una segunda boca. #329/#567 continúa abierta por contadores de
infraestructura, callbacks, arrastre de áreas y aceptación visual/framebuffer.

Corrección #329/#567-RAIL-INFRASTRUCTURE-SUMMARY (2026-09-12): Finanzas ya no
cuenta todas las teselas del mapa como si fueran de la compañía activa. El
core reconstruye la parte ferroviaria de CompanyInfrastructure por owner y
railtype: piezas planas (incluidos cruces), señales, depósitos,
estaciones/waypoints y túneles/puentes con el factor estructural nativo. El
resumen se invalida al cambiar de compañía o cualquier tesela, y la UI muestra
el desglose normal/eléctrica/monorail/maglev. La métrica de carretera sigue
siendo provisional por teselas y queda para su frente de contadores propio.
#329/#567 continúa abierta por contadores vial/agua/aeropuerto, callbacks,
arrastre de áreas y aceptación visual/framebuffer.

Corrección #329/#567-ROAD-INFRASTRUCTURE-SUMMARY (2026-09-12): Finanzas ya no
usa cantidad de teselas como proxy vial. El core reconstruye piezas por
`RoadType` para carretera y tranvía, respetando owners separados, bits de
trazado, cruces, depósitos, paradas/waypoints y el factor longitud × 4 × 2 de
túneles/puentes; los enlaces estructurales se cuentan una sola vez. La UI
expone el total de piezas y el desglose efectivo por clase, incluyendo tipos
NewGRF. #329/#567 continúa abierta por agua/aeropuertos, metadata incremental
de writers viales, callbacks, arrastre de áreas y aceptación visual/framebuffer.

Corrección #329/#567-ROAD-WRITER-METADATA (2026-09-12): los comandos runtime
ya persisten la separación nativa entre la infraestructura de carretera y la
capa tranviaria. Las carreteras conservan el owner de tranvía al actualizar
bits; depósitos, paradas, waypoints y túneles/puentes escriben `RoadType`,
MAP7 y el nibble de owner M3 correspondiente, también para compañías distintas
del jugador. Las regresiones ejercitan construcción real con compañía rival.
#329/#567 continúa abierta por conversión de capas existentes, agua,
aeropuertos, callbacks, arrastre de áreas y aceptación visual/framebuffer.

Corrección #329/#567-WATER-INFRASTRUCTURE-SUMMARY (2026-09-12): Finanzas ya
reconstruye la parte acuática de `CompanyInfrastructure` por owner, siguiendo
`AfterLoadCompanyStats`: canales y objetos sobre canal, muelles y boyas,
depósitos, esclusas y acueductos con el factor estructural nativo. Los
acueductos se deduplican por pareja de rampas y sus bocas `MP_TUNNELBRIDGE`
persisten owner, dirección, tipo de transporte y payload limpio; el decoder
también los conserva como agua. La regresión cubre compañías rivales,
secciones de esclusa y round-trip de rampas. #329/#567 continúa abierta por
la exposición restante de otras infraestructuras, conversión de capas
existentes, callbacks, arrastre de áreas y aceptación
visual/framebuffer.

Corrección #329/#567-STATION-AIRPORT-SUMMARY (2026-09-12): Finanzas ya separa
las teselas MP_STATION propias de las facilidades aéreas. El core cuenta
estaciones no aéreas excluyendo boyas y aeropuertos, mientras cuenta cada
entidad con facilidad aérea una sola vez, incluyendo Oil Rig; la UI muestra
ambos valores y la regresión cubre ownership rival y tipos especiales. El
frente #329/#567 continúa abierto por conversión de capas existentes,
callbacks, arrastre de áreas y aceptación visual/framebuffer.

Corrección #330-TRAIN-LINE-DEPOT-EVENT (2026-09-12): el escenario ferroviario
de paridad ahora inicia el tren con estado físico dentro del depósito, precarga
el umbral de salida y conserva la potencia sintética de la unidad legacy. La
traza emite DepotExit en el primer tick sin añadir una espera artificial de
37 ticks; la regresión completa de parity_system y la suite del core quedan
verdes.

Corrección #330/#326-SHIP-ROTATION-BOUNDS (2026-09-12): los barcos ahora
conservan la posición previa al comenzar una reversa o un giro fuerte, igual que
`Ship::rotation_x_pos/y_pos` (`NOSAVE`). El compositor Bevy aplica ese delta al
prisma de sorting mientras `direction != rotation`, evitando el desplazamiento
visual lateral durante la salida de depósito, cruces y giros sobre el agua. Se
añadieron regresiones del controlador y de la caja gráfica; #330/#326 continúa
abierta por los demás oráculos de movimiento y contratos de compositor.

Corrección #328-SAV-NESTED-STRUCT-LENGTH (2026-09-12): la fusión semántica de
tablas SAV conserva ahora las columnas desconocidas dentro de los elementos que
permanecen cuando una lista de structs con longitud explícita crece o se reduce.
Los elementos nuevos se toman del writer canónico sólo cuando el descriptor es
estable; si el save importado contiene subcampos futuros sin un valor semántico
seguro para una entrada nueva, se mantiene el fallback canónico. Los structs de
tamaño fijo y layouts incompatibles siguen sin fusionarse. Las regresiones cubren
reducción con columna anidada futura, crecimiento estable y el fallback fijo;
#328 continúa abierta por cambios de forma/topología, pools nativos y runtime
SAV restante.

Corrección #326/#563-ROAD-STOP-LAYOUT-CATENARY-GLOBAL (2026-09-12): la
catenaria road/tram de paradas y waypoints con `TileLayout` NewGRF incompleto
se emite ahora como parents del compositor global, igual que en la ruta
vanilla y en los layouts materializables. El fallback atómico sigue
descartando ground/BUILD custom parciales, pero el tramo 4–11 de catenaria
se conserva y las capas BUILD vanilla posteriores empiezan en 12; cuando la
spec declara `NoCatenary` se mantiene el bloque legacy 2–3. Las regresiones
ECS cubren parada y waypoint incompletos, ambos ejes y layout vacío. La
brecha estructural de #563 queda resuelta; #326 permanece abierta por la
aceptación raster/oráculo dedicado, clipping, pivotes y framebuffer.

Corrección #326/#567-BUOY-GLOBAL-SORT (2026-09-12): la boya ahora conserva
su producer `TILE_SEQ_LINE(4, -1, 0, 0, 0, 0, SPR_IMG_BUOY)` como parent
sortable del compositor Bevy. La caja de extensión cero y su clave de
inserción se traducen a coordenadas mundiales sin inflarla a una tesela,
por lo que un barco o una estructura vecina vuelve a poder taparla con el
mismo criterio que `ViewportSortParentSprites`. La regresión cubre la caja
literal y el spawn ECS junto con el suelo de agua. #326/#567 continúan
abiertas por los producers y callbacks navales restantes, clipping, framebuffer
y la aceptación raster completa.

Corrección #326/#567-BUOY-CANAL-FEATURE (2026-09-12): el productor de boya
recibe ahora `canal_feature_catalog` y aplica la vista `CF_BUOY` seleccionada
por Action2/callback antes de crear el parent del compositor. El sprite
custom conserva offsets y dimensiones NFO; el suelo de agua permanece en su
capa propia y la ausencia de una vista válida cae al sprite vanilla. La
regresión cubre el reemplazo ECS y confirma que bounds, depth e inserción
siguen siendo los de `TILE_SEQ_LINE`; #326/#567 continúa abierta por las
familias navales restantes, clipping, framebuffer y la aceptación raster.

Corrección #326/#329-AIRPORT-ACTION0-RANGES (2026-09-12): el parser de
`Action0 Airports` ahora expande todos los ids consecutivos del bloque y
consume cada propiedad una vez por id, conservando catchment, ruido, layout,
dimensiones y sustitución de cada aeropuerto. Las coordenadas de layout
preservan el byte no negativo de OpenTTD y sólo se sign-extienden para la
entrada especial `gfx=0xFF`; las orientaciones E/O siguen transponiendo la
huella sin truncar offsets grandes. Se añadieron regresiones de parser,
catálogo y FTA; #326/#329 continúan abiertas por foundations/rotaciones,
callbacks, sonidos y la aceptación raster completa.

Corrección #326/#329-AIRPORTTILE-ACTION0-RANGES (2026-09-12): el parser de
`Action0 AirportTiles` ahora expande todos los ids consecutivos y consume por
id sustitución, override, callbacks, animación, velocidad, triggers y badges.
El catálogo mantiene una entrada y un gfx global por cada definición, y los
overrides no se desplazan al primer id del bloque. La regresión cubre dos
teselas serializadas en una sola acción y comprueba sus animaciones y tabla
de overrides; #326/#329 continúan abiertas por callbacks runtime, foundations,
rotaciones, sonidos y la aceptación raster completa.

Corrección #329/#567-VEHICLE-NO-NEWS (2026-09-12, `9567ed24`): los flags
`NoNews`, `NoPreview` y `JoinPreview` ya tienen predicados explícitos en
`EngineDef`; `NoNews` deja de ser sólo un campo conservado. El ciclo de
noticias del core detecta una vez los motores introducidos, publica la nueva
categoría `NewVehicles` y omite los motores con `NoNews`; al rehidratar una
partida se siembran las introducciones históricas para no producir una ráfaga,
mientras que un motor NewGRF incorporado después del arranque conserva su
primera notificación contemporánea. La categoría se puede configurar como
Silencio/Resumen/Completo y mantiene el hash compartido independiente de las
preferencias locales. La regresión cubre supresión y deduplicación; pasaron
10 tests de noticias del core, 3 de preferencias del cliente, `cargo check`,
Clippy estricto, formato y `git diff --check`. #329/#567 continúan abiertas
por el flujo de oferta exclusiva de `NoPreview`, la propagación recursiva de
`JoinPreview`, callbacks de vehículos y aceptación visual manual bajo Weston;
#326 permanece abierta.

Corrección #329/#567-VEHICLE-PREVIEW-LIFECYCLE (2026-09-12, `cfb2858a`): el
core modela el estado anual de cada motor como no introducido, preview
exclusivo, disponibilidad pendiente, disponible o retirado, distinguiendo
`NoPreview` del motor común y respetando la vida útil. La resolución de
`JoinPreview` encuentra la raíz y agrupa variantes recursivamente, con
protección ante padres ausentes y ciclos. Las noticias de vehículos esperan
hasta `Available` y la hidratación inicial evita una ráfaga histórica; la
regresión cubre ciclo anual, grupo recursivo y deduplicación. Pasaron los
2675 tests del core (1 ignorado), Clippy estricto en core y client, formato y
`git diff --check`. #329/#567 siguen abiertas: falta conectar la aceptación de
la oferta al comando/UI y completar callbacks; #326 permanece abierta.

Corrección #329/#567-VEHICLE-NEWS-IDENTITY (2026-09-12, `78fc9b26`): las
noticias `NewVehicles` conservan ahora el `engine_id` en
`NewsReference::Engine`, en lugar de publicar una referencia vacía. Esto
deja preparado el enlace estable entre la noticia, el catálogo y la futura
aceptación de una preview sin depender del texto localizado ni de una
captura de pantalla. La regresión de noticias comprueba la identidad junto
con `NoNews` y la deduplicación; pasaron la prueba focalizada, Clippy
estricto en core y client, formato y `git diff --check`. La aceptación
interactiva y los callbacks de vehículos siguen pendientes; #326 permanece
abierta.

Corrección #329/#567-VEHICLE-NEWS-CATALOG-ROUTE (2026-09-12, `2bca332d`):
popup, ticker e historial de noticias consumen `NewsReference::Engine`. Al
activar una noticia el catálogo de compra selecciona el motor si hay un
depósito compatible abierto; si no, conserva la solicitud y la aplica al
abrir `Nuevos vehículos` desde el depósito elegido. La regresión cubre la
espera y el consumo de la selección pendiente; pasaron la prueba ECS
focalizada, Clippy estricto del cliente, formato y `git diff --check`. La
aceptación interactiva de previews y los callbacks de vehículos siguen
pendientes; #326 permanece abierta.

Corrección #329/#567-VEHICLE-PREVIEW-ACCEPTANCE (2026-09-12, `60419a0d`):
el core mantiene una oferta diaria exclusiva por motor con compañía
candidata, máscara de compañías consultadas y ventana de 20 días. El nuevo
comando `WantEnginePreview` valida la oferta de forma autoritativa, concede
el motor raíz y las variantes enlazadas con `JoinPreview`, y el comando de
compra y la ventana de depósito rechazan modelos fuera de disponibilidad o
aún no aceptados. La referencia de noticia sigue siendo el `engine_id`, por
lo que la oferta ya puede llegar desde popup, ticker o historial sin depender
del texto. Se ajustaron los fixtures que construían modelos futuros para
declarar su año de simulación; pasaron 2678 tests del core (1 ignorado),
Clippy estricto en core y cliente, formato y `git diff --check`. La oferta
temporal aún vive en `SimulationRuntime`; falta serializar el pool nativo
`Engine::company_avail/preview_*`, completar el botón/feedback visual de
aceptación y los callbacks de vehículos. #329/#567 siguen abiertas y #326
permanece abierta.

Corrección #329/#567-VEHICLE-PREVIEW-UI (2026-09-12, `94234caf`): la noticia
de vehículo con una oferta exclusiva para la compañía activa muestra ahora
`Aceptar preview` y despacha `WantEnginePreview` por la misma ruta autoritativa
del HUD. La aceptación cierra el popup y deja feedback visible; una oferta
expirada conserva el error estándar y no se consume. El showcase del menú
mantiene una fecha determinista de 1961 para exhibir ferry, helicóptero, tren,
bus y maglev, mientras que los vehículos preconstruidos usan una excepción
temporal de bootstrap y restauran el catálogo inmediatamente, sin ampliar la
disponibilidad real del jugador. El constructor también sincroniza calendario
y timers con `start_year`. La suite del cliente pasó 1422 tests (2 ignorados),
además de Clippy estricto en core y cliente, formato y `git diff --check`.
La serialización SAV del pool nativo `Engine::company_avail/preview_*` y los
callbacks restantes siguen pendientes; #329/#567 continúan abiertas y #326
permanece abierta.

Corrección #329/#567-VEHICLE-SAV-ENGINE-POOL (2026-09-12, `7d3ecd0d`): el
importador reconoce las columnas persistentes de `ENGN`
(`company_avail`, `preview_asked`, `preview_company` y `preview_wait`) y
rehidrata las excepciones de disponibilidad y las ofertas activas. Al
reexportar un SAV nativo, el writer actualiza sólo esos campos para los
motores vanilla con correspondencia conocida, conservando header, huecos,
framing gamma y columnas futuras del pool. La aceptación limpia la oferta y
deja el bit de compañía; una oferta activa conserva countdown y máscara de
compañías consultadas. Los motores NewGRF/slots sin correspondencia y la
creación de un `ENGN` nuevo cuando el save de origen no lo contiene siguen
pendientes. Pasaron 2682 tests del core (1 ignorado), 1422 del cliente (2
ignorados), Clippy estricto en core y cliente, formato y `git diff --check`.
#329/#567 continúan abiertas y #326 permanece abierta.

Corrección #329/#567-VEHICLE-SAV-ENGINE-MAPPING (2026-09-12, `30a4bcea`): el
puente nativo también lee `EIDS` y, después de aplicar el stack Action0,
resuelve cada slot `ENGN` por `(GRFID, ID local)` y tipo de vehículo. Esto
permite rehidratar y reexportar previews de motores NewGRF sin confundir un
slot custom con el modelo vanilla del mismo índice. Si falta el GRF o la
correspondencia no es segura, el slot permanece opaco y no se inventa un
motor de catálogo. Pasaron 2683 tests del core (1 ignorado), 1422 del cliente
(2 ignorados), Clippy estricto en core y cliente, formato y `git diff --check`.
#329/#567 continúa abierta por callbacks, slots sin correspondencia y demás
estado de vehículo NewGRF; #326 permanece abierta.

Corrección #326/#567-SHIP-DEPOT-WESTON-CAPTURE (2026-09-12, `c4de2263`): la
captura Wayland de `Depot` reveló dos conflictos reales de exclusividad ECS
que abortaban Bevy 0.19 antes de componer el framebuffer: `sync_depot_panel`
separa `DepotRowContainer` de `DepotRenameRow`, y
`sync_buy_window_preview` separa la imagen de preview de su frame. El arnés
de ventanas acepta ahora `OPENTTDRS_WINDOW_SHOT_DEPOT_KIND=road|rail|ship`,
normaliza un depósito naval a su sección norte y conserva el fallback anterior
para las capturas comunes. Bajo Weston headless, `Kale_TitleGame.sav` produjo
una captura naval 1280×720 sin abortar; el depósito seleccionado no tenía
barcos estacionados, por lo que esta evidencia desbloquea el camino de
captura pero no cierra la aceptación visual ni la paridad de composición.
Pasaron 1423 tests del cliente (2 ignorados), 39 tests focalizados del arnés,
Clippy estricto en core y cliente, formato y `git diff --check`. #567 sigue
abierta por callbacks, vecinos, estado naval restante y validación visual
comparada; #326 permanece abierta.

Corrección #326/#567-SHIP-DEPOT-OCCUPANCY (2026-09-12, `51b1a3dd`): el panel
de depósito naval ya no exige que `Vehicle::pos` sea literalmente la sección
norte. Usa `vehicle_at_depot_command_tile`, que normaliza ambas secciones de
la huella 2×1 al mismo ancla nativa, pero conserva la comprobación de estado
`SHIP_STATE_DEPOT`; así no muestra barcos que están saliendo. La regresión
cubre un save legacy con el barco persistido en la sección opuesta. El arnés
Wayland también prioriza un depósito naval ocupado para que la evidencia
visual pruebe la lista real: `Kale_TitleGame.sav` renderizó bajo Weston el
panel `Nunwood Depósito de Barcos` con `Ferry #3017` y sus acciones de fila.
Pasaron 1424 tests del cliente (2 ignorados), 12 focalizados del panel,
Clippy estricto en core y cliente, formato y `git diff --check`. #567 sigue
abierta por callbacks, vecinos, estado naval restante y comparación visual;
#326 permanece abierta.

Corrección #329-VEHICLE-CB31-AUTOREPLACE (2026-09-12, `47a72ced`): el flujo
de autoreemplazo consulta `CBID_VEHICLE_START_STOP_CHECK` cuando la unidad aún
está en marcha y la operación necesita detenerla, usando el motor NewGRF
activo. Un rechazo conserva el writeback de registros persistentes, deja el
diagnóstico textual y publica la noticia de fallo sin cambiar motor ni cobrar;
una unidad ya detenida en depósito no se consulta de nuevo, igual que el
camino nativo de `AutoReplace`. La regresión cubre rechazo `D010`, preservación
del motor y posterior reemplazo exitoso cuando la unidad ya está detenida.
Pasaron 2684 tests del core (1 ignorado), 16 focalizados de autoreemplazo,
Clippy estricto en core y cliente, formato y `git diff --check`. #329 sigue
abierta por CB31 en órdenes de depot, callbacks avanzados y APIs legacy sin
catálogo; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-CB31-MASS-DEPOT (2026-09-12, `ecf8edc2`): el comando
`SetDepotVehiclesRunning` evalúa `CBID_VEHICLE_START_STOP_CHECK` por cada unidad
que realmente cambia de estado, usando el motor NewGRF del catálogo activo. Una
denegación queda limitada a esa unidad para que las demás arranquen o se
detengan, y el diagnóstico efímero conserva el último rechazo para el feedback
del HUD. La regresión cubre tanto arranque como parada masiva, el motivo `D010`
y la continuidad de una unidad permitida. Esto separa el botón explícito de
arranque/parada —que delega en el comando individual nativo— de la salida
automática de una orden de depósito, que no consulta CB31 mientras deja la
unidad en marcha. Pasaron 2685 tests del core (1 ignorado), 2 focalizados,
Clippy estricto en core y cliente, formato y `git diff --check`. #329 sigue
abierta por callbacks avanzados, órdenes de depósito restantes y APIs legacy
sin catálogo; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-SERVICE-RELIABILITY-CATALOG (2026-09-12, `68cdf07c`):
`NeedsServicing` usa ahora la fiabilidad del motor resuelto en el catálogo
activo cuando la compañía configura intervalos porcentuales. La fuente respeta
la cadena `SyncReliability`, igual que el servicio efectivo en depósito; los
vehículos custom ya no se comparan contra la fiabilidad del slot vanilla que
ocupaban originalmente. La regresión contrasta un motor al 40% con el bus
vanilla al 90% y verifica ambos lados del umbral. Pasaron 2686 tests del core
(1 ignorado), el focalizado de fiabilidad, Clippy estricto en core y cliente,
formato y `git diff --check`. #329 sigue abierta por callbacks avanzados,
órdenes de depósito restantes y APIs legacy sin catálogo; #326/#567 permanecen
abiertas.

Corrección #329-VEHICLE-BUY-CATALOG-WAGON (2026-09-12, `f97265f0`): la ventana
`Nuevos vehículos` resuelve el motor seleccionado y la locomotora del depósito
contra `GameState.engine_catalog` antes de decidir si debe acoplar un vagón.
Así, un vagón NewGRF comprado desde el catálogo activo se engancha a la cabeza
del consist igual que un vagón vanilla; la regresión ECS cubre construcción,
`prev_unit` y la enumeración completa del consist. Pasaron 1425 tests del
cliente (2 ignorados), Clippy estricto en cliente y core, formato y
`git diff --check`. #329 sigue abierta por callbacks avanzados, órdenes de
depósito restantes y APIs legacy sin catálogo; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-AUTOREPLACE-CATALOG-UI (2026-09-12, `529f046b`): la
ventana de autoreemplazo consulta `engine_catalog` para poblar los motores de
origen/destino según el depósito, y para renderizar nombres de reglas y del
resumen. Los motores NewGRF ya no desaparecen del selector ni se muestran como
`?` cuando una regla conserva su ID custom. La regresión cubre un bus custom
del catálogo activo y su etiqueta localizada. Pasaron 1426 tests del cliente
(2 ignorados), Clippy estricto en cliente, formato y `git diff --check`. #329
sigue abierta por callbacks avanzados, órdenes de depósito restantes y APIs
legacy sin catálogo; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-DETAILS-CATALOG-STATS (2026-09-12, `0402e423`): la
ficha de detalles resuelve cada unidad contra `GameState.engine_catalog`.
Peso, potencia, velocidad máxima y nombre de los motores custom dejan de caer
al slot vanilla; los totales de tren usan además métricas de consist con
catálogo, conservando las funciones legacy como fallback. Las regresiones
cubren un bus NewGRF y un consist con motor custom. Pasaron 2688 tests del
core (1 ignorado), 1427 del cliente (2 ignorados), Clippy estricto en core y
cliente, formato y `git diff --check`. #329 sigue abierta por callbacks
avanzados, órdenes de depósito restantes, títulos/listados legacy y APIs sin
catálogo; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-NAME-CATALOG-UI (2026-09-12, `79d79eaa`):
`Vehicle::display_name_with_catalog` centraliza el fallback de nombre y
resuelve el motor en el catálogo runtime. Lista de vehículos, depósito,
autoreemplazo, órdenes, horario, refit y títulos de vista/detalles ya no
presentan el nombre vanilla del slot cuando el vehículo es NewGRF; los nombres
manuales siguen teniendo prioridad. La regresión core cubre `Bus NewGRF #7`.
Pasaron 2688 tests del core (1 ignorado), 1427 del cliente (2 ignorados),
Clippy estricto en core y cliente, formato y `git diff --check`. #329 sigue
abierta por consumidores legacy de física/render y callbacks avanzados;
#326/#567 permanecen abiertas.

Corrección #329-VEHICLE-RENDER-CATALOG-FALLBACK (2026-09-12, `d4c6a7a0`):
los fallbacks visuales de mapa, trailers, sombras y cámara resuelven el
`EngineDef` en `GameState.engine_catalog` antes de elegir el atlas OpenGFX.
Esto conserva el `train_image_index` de un motor NewGRF, el sprite cargado de
un vagón de carbón custom y el `original_image_index` de un barco `0xFD`; la
geometría fallback usa el mismo grupo que la imagen. Las ventanas de vehículo
comparten el resolver para previews sin vistas custom. Las regresiones cubren
grupo ferroviario custom, vagón cargado y barco custom. Pasaron 2688 tests del
core (1 ignorado), 1430 del cliente (2 ignorados), Clippy estricto en core y
cliente, formato y `git diff --check`. #329 continúa abierta por vistas
NewGRF completas, callbacks y consumidores legacy restantes; #326/#567
permanecen abiertas.

Corrección #329-CARGO-LOCOMOTIVE-CATALOG (2026-09-12, `68a52b47`):
`load_vehicles` resuelve ahora `is_train_engine` desde el catálogo activo
antes de decidir si una unidad ferroviaria sin vagón puede cargar. Una
locomotora NewGRF presente sólo en `GameState.engine_catalog` ya no se trata
como un vehículo de carga por haber fallado el lookup vanilla; la regla de
escenarios y saves antiguos conserva el fallback estático. La regresión
reproduce una locomotora custom con capacidad propia y confirma que no toma
mercancía de la estación. Pasaron 2689 tests del core (1 ignorado), 1430 del
cliente (2 ignorados), Clippy estricto en core y cliente, formato y
`git diff --check`. #329 continúa abierta por callbacks y demás contratos de
vehículo NewGRF; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-ROAD-STEP-CATALOG (2026-09-12, `4d3a68f7`):
`Vehicle::step_with_map_and_accel_and_catalog` propaga ahora el catálogo
activo al controlador vial de vehículo único. Antes, la actualización inicial
consultaba el motor NewGRF pero el tick posterior llamaba a la API legacy y
podía volver al motor vanilla; un bus custom podía superar su techo de
velocidad. La regresión usa un motor de carretera custom limitado a 20 y
verifica que el paso catalog-aware conserve ese límite. Pasaron 2690 tests del
core (1 ignorado), 1430 del cliente (2 ignorados), Clippy estricto en core y
cliente, formato y `git diff --check`. #329 continúa abierta por callbacks y
consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-IMAGE-TYPES (2026-09-12, `8bd9c649`): la resolución
runtime conserva ahora el `EngineImageType` en el byte bajo de `var 10` y el
índice de `SpriteStack` en el byte alto, como `GetCustomEngineSprite` de
OpenTTD. Mapa usa `0x00`, depósito `0x10`, detalles `0x11`, lista `0x12` y
compra `0x20`; la regresión `runtime_vehicle_layers_receive_purchase_image_type`
selecciona grupos distintos para mapa y compra. Pasaron 38 tests de
`render::vehicles`, Clippy estricto de cliente y core, formato y
`git diff --check`. #329 continúa abierta por callbacks, layouts y
consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-AUTOREPLACE-COMMAND-CATALOG (2026-09-12, `c4d604f4`): el
comando `SetAutoReplaceRule` valida ahora los motores origen y destino contra
`GameState.engine_catalog` antes de caer a la tabla vanilla. La ventana ya
podía seleccionar motores NewGRF, pero el comando rechazaba sus IDs con
`EngineNotFound`; la regresión crea una regla entre dos buses sólo presentes
en el catálogo activo. Pasaron 2691 tests del core (1 ignorado), 1430 del
cliente (2 ignorados), Clippy estricto en core y cliente, formato y
`git diff --check`. #329 continúa abierta por callbacks y consumidores de
vehículos aún legacy; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-LIST-CATALOG-FIRST-FRAME (2026-09-12, `777006ac`):
las filas de la lista global de vehículos eligen ahora su sprite inicial con
`GameState.engine_catalog`, antes de que el refresco posterior materialice las
capas NewGRF completas. Un motor custom ya no parpadea con la silueta del slot
vanilla al reconstruir la lista; el refresco de capas y sus fallbacks siguen
siendo los mismos. La regresión verifica que el `EngineDef` activo conserva
su `train_image_index`. Pasaron 2691 tests del core (1 ignorado), 1431 del
cliente (2 ignorados), Clippy estricto en cliente, formato y `git diff
--check`. #329 continúa abierta por callbacks, vistas NewGRF completas y
consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-PICK-CATALOG-OFFSETS (2026-09-12, `47534e0d`): el
hit-test de vehículos del mapa calcula ahora el centro con
`vehicle_sprite_pos_at_with_catalog`, incluyendo offsets y dimensiones de la
vista NewGRF decodificada. Un vehículo custom desplazado ya puede seleccionarse
en el punto donde realmente se dibuja; la ocultación y el radio de picking no
cambian. La regresión separa más de 34 px el centro vanilla del custom y
comprueba ambos resultados. Pasaron 1432 tests del cliente (2 ignorados),
Clippy estricto, formato y `git diff --check`. #329 continúa abierta por
callbacks, vistas runtime completas y consumidores legacy restantes;
#326/#567 permanecen abiertas.

Corrección #329-VEHICLE-CARGO-LABEL-CATALOG-POS (2026-09-12, `162018f3`):
las etiquetas de carga de diagnóstico reutilizan ahora la posición que el
renderer acaba de resolver para la cabeza del vehículo, incluyendo capas
NewGRF runtime y offsets del catálogo. Si no existe una entidad visual, el
fallback usa `vehicle_sprite_pos_at_with_catalog`; en el flujo normal no se
crea trabajo adicional cuando las etiquetas están desactivadas. La regresión
comprueba que el texto conserve el desplazamiento de la capa custom. Pasaron
1432 tests del cliente (2 ignorados), Clippy estricto, formato y
`git diff --check`. #329 continúa abierta por callbacks, vistas runtime
completas y consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-CALENDAR-AGING-CATALOG (2026-09-12, `54be092b`): el barrido
diario de calendario resuelve ahora la clase de la unidad ferroviaria contra
`GameState.engine_catalog`. Antes, un vagón NewGRF con ID fuera de la tabla
vanilla podía caer en la locomotora por defecto y duplicar su decaimiento de
fiabilidad al cruzar el límite de vida útil. La API pública legacy conserva
el fallback vanilla y el barrido autoritativo usa la variante catalog-aware;
la regresión cubre un vagón custom sin unidad anterior. Pasaron 2692 tests del
core (1 ignorado), Clippy estricto, formato y `git diff --check`. #329 continúa
abierta por callbacks, vistas runtime completas y consumidores legacy
restantes; #326/#567 permanecen abiertas.

Corrección #329-AUTORENEW-CLASS-CATALOG (2026-09-12, `2e1d1fe4`):
`needs_autorenewing` conserva su wrapper vanilla, pero los dos caminos
autoritativos de autoreemplazo resuelven la clase ferroviaria con
`GameState.engine_catalog`. Un vagón NewGRF libre ya no se interpreta como
locomotora por defecto ni entra accidentalmente en autorrenovación por edad;
la regresión compara ambos contratos. Pasaron 2693 tests del core (1
ignorado), Clippy estricto, formato y `git diff --check`. #329 continúa
abierta por callbacks, vistas runtime completas y consumidores legacy
restantes; #326/#567 permanecen abiertas.

Corrección #329-RAIL-PATH-CATALOG (2026-09-12, `04bd9568`): el pathfinding
ferroviario YAPF de las rutas en vivo resuelve ahora el `EngineDef` desde
`GameState.engine_catalog` antes de filtrar las teselas por `RailType`. Esto
evita que un motor NewGRF con `required_rail_type` custom sea tratado como
Rail al calcular su destino, al probar plataformas alternativas, al asignar
andén o al reencaminarse tras un head-on. El wrapper público sin catálogo
conserva el fallback vanilla para consumidores legacy y la regresión demuestra
que un corredor Maglev sólo se acepta con el catálogo correcto. Pasaron 2694
tests del core (1 ignorado), Clippy estricto, formato y `git diff --check`.
#329 continúa abierta por callbacks, vistas runtime completas y consumidores
legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-DEPOT-ENGINE-POWER-CATALOG (2026-09-12, `4eb0edcf`): la
salida ferroviaria del depósito ya resuelve la potencia del motor contra
`GameState.engine_catalog` antes de decidir si la cabeza está sin propulsión.
Así un motor NewGRF cuya caché de consist todavía vale cero no se apaga en el
primer tick por caer al lookup vanilla; la ruta legacy mantiene su fallback y
la regresión cubre una cabeza custom con potencia válida y caché vacía.
Pasaron 2695 tests del core (1 ignorado), Clippy estricto, formato y
`git diff --check`. #329 continúa abierta por callbacks, vistas runtime
completas y consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-AIR-FAST-CATALOG (2026-09-12, `890529c1`): el
crash de aeronaves y el sonido de despegue resuelven ahora `AIR_FAST` desde el
`EngineDef` activo. Un avión NewGRF marcado como grande/rápido ya no se trata
como propulsor por caer al ID vanilla; se conserva el fallback de los IDs
históricos y se cubren core y cliente con regresiones específicas. Pasaron los
tests dirigidos del core y cliente, Clippy estricto en ambos crates, formato y
`git diff --check`. #329 continúa abierta por callbacks y consumidores
legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-RAIL-CLASS-CATALOG (2026-09-12, `471f0f32`): el humo y las
chispas del renderer, junto con el sonido de túnel, resuelven ahora
`EngineDef.rail_engine_class` del catálogo activo. Un tren NewGRF eléctrico,
monorail o maglev ya no cae a vapor sólo porque su ID no existe en la tabla
vanilla; el wrapper por ID conserva el fallback legacy. Pasaron las
regresiones dirigidas de core y cliente, Clippy estricto en ambos crates,
formato y `git diff --check`. #329 continúa abierta por callbacks y
consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-CONVERT-RAIL-CATALOG (2026-09-12, `443b1ebb`): la validación
de `CmdConvertRail` resuelve ahora el `required_rail_type` del tren desde el
catálogo activo antes de permitir la conversión de una tesela ocupada. Un
motor NewGRF maglev ya no se interpreta como Rail por tener un ID fuera de la
tabla vanilla; la prueba confirma el error específico y que la tesela queda
intacta. Pasaron 21 pruebas de `railtypes`, Clippy estricto de core, formato y
`git diff --check`. #329 continúa abierta por callbacks y consumidores
legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-RENDER-TRACE-CATALOG (2026-09-12, `1e6342c4`): la
traza CSV opt-in de render resuelve ahora la posición del vehículo con el
`EngineDef` del catálogo activo, igual que el dibujo y el picking. Esto evita
que un diagnóstico de interpolación atribuya a la simulación un desplazamiento
que en realidad proviene de offsets NewGRF custom; la regresión compara la
posición registrada con un motor custom desplazado 96 px. Pasaron los 2 tests
dirigidos de `render_trace`, Clippy estricto de cliente y core, formato y
`git diff --check`. #329 continúa abierta por callbacks, vistas runtime
completas y consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-PREVIEW-2CC-LIVERY (2026-09-12, `9bbfeb6f`): las
previews de compra, vista de vehículo y rotor calculan la librea de la
compañía activa con el contrato GUI y conservan sus canales primario y
secundario al hornear sprites 2CC. Las filas de compra comparten además la
clave de caché `(engine, primary, secondary)`, por lo que cambiar la librea no
reutiliza una textura vieja. La regresión
`vehicle_preview_uses_secondary_company_livery_for_2cc` contrasta `2/9` con
el resultado incorrecto `2/2`; callbacks de remapeo de color que requieren
una unidad real y otros layouts GUI siguen pendientes. Pasaron 36 tests de
`render::vehicles`, Clippy estricto de cliente y core, formato y
`git diff --check`. #329 continúa abierta; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-SIDE-CONTEXT (2026-09-12, `166062cf`): las tiras
laterales de lista, depósito y detalles dejan de reutilizar la preview de
compra para unidades ya construidas. Mantienen la orientación fija de la UI,
pero resuelven la primera capa con el vehículo real y su cargo, consist,
callbacks y librea de grupo; la regresión `vehicle_side_layers_use_actual_group_livery`
contrasta una librea de grupo 2CC `4/9` con el resultado de compra. Pasaron 37
tests de `render::vehicles`, Clippy estricto de cliente y core, formato y
`git diff --check`. #329 continúa abierta por image types, callbacks y
consumidores legacy restantes; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-COLOUR-MAPPING-PREVIEW (2026-09-12, `2155c05b`): las
previews de compra y rotor, que todavía no tienen una unidad materializada,
construyen ahora el scope GUI equivalente con motor, compañía activa, carga,
capacidad y fecha; `CBID_VEHICLE_COLOUR_MAPPING` (`0x2D`) puede seleccionar
la paleta explícita o la librea 2CC antes de hornear la textura. La misma
resolución se usa sólo sobre el contexto efímero de la preview, mientras las
unidades reales conservan el writeback aislado y sus registros no se mutan.
El apply de vehículos mantiene el runtime cuando la máscara de callbacks sólo
declara `ColourRemap`, evitando perder el grafo antes de llegar a la UI. Las
regresiones `vehicle_preview_applies_colour_mapping_callback` y
`callbacks_ac_vehicle_colour_mapping_respects_mask_and_company_bit` cubren la
selección de una paleta distinta de la compañía activa y la variante con
contexto preparado. Pasaron 39 tests de `render::vehicles`, el callback core
dirigido, Clippy estricto de cliente y core, formato y `git diff --check`.
#329 continúa abierta por layouts/call sites GUI restantes, callbacks de
vehículos sin consumidor y consumidores legacy; #326/#567 permanecen abiertas.

Corrección #329-VEHICLE-BUY-ROW-RUNTIME (2026-09-12, `3a0e67f1`): las filas
compactas de la ventana de compra dejan de tomar siempre `newgrf_preview()`
directo. Primero consultan `vehicle_preview_layers`, por lo que comparten con
el panel grande los image types, SpriteStack, cargo, callbacks y paletas del
catálogo/runtime; sólo usan el sprite estático como fallback cuando no hay una
capa resoluble. La suite de `ui::buy_window` conserva 12 tests aprobados y
Clippy estricto de cliente; #329 continúa abierta por layouts GUI adicionales
y consumidores legacy.

Corrección #329-VEHICLE-PREVIEW-STATIC-PALETTE (2026-09-12, `b7ae1e1b`): si
el runtime no consigue devolver una vista y debe usar `newgrf_views`, la
preview conserva igualmente el resultado de `CBID_VEHICLE_COLOUR_MAPPING` y
hornea su paleta explícita, 2CC o crash. Los mapas 2CC Action5 activos también
se pasan a ese fallback para no perder reemplazos instalados. El mismo helper
se comparte con el fallback de rotores; 39 tests de `render::vehicles` y
Clippy estricto de cliente pasan. #329 continúa abierta por layouts y call
sites GUI no cubiertos.

Corrección #329-VEHICLE-AIRCRAFT-SHADOW-LAYER (2026-09-12, `28afcb51`): la
sombra de aeronaves reutiliza la primera capa NewGRF ya resuelta para la misma
pose, tanto al crear la entidad como en `update_vehicles`. Esto conserva su
textura custom y sus offsets/dimensiones para el anclaje; cuando no hay capa
runtime se mantiene el fallback de catálogo vanilla. La regresión
`aircraft_shadow_reuses_custom_body_layer` verifica textura y alineación X/Y.
Pasaron 40 tests de `render::vehicles` y Clippy estricto de cliente; #329
continúa abierta por sombras/efectos NewGRF avanzados y otros call sites.

Corrección #329-VEHICLE-STATIC-VIEW-AFTER-RUNTIME-MISS (2026-09-12,
`07008e0a`): si un motor conserva runtime NewGRF para callbacks, SpriteStack o
grupos de carga pero la pose actual no produce una capa, el render consulta
ahora la vista estática `newgrf_views` antes de caer al sprite vanilla. El
fallback conserva la paleta calculada, los mapas 2CC Action5 y los offsets y
dimensiones de Action1. `runtime_empty_layers_fall_back_to_static_catalog_view`
reproduce el caso con runtime vacío y vista custom. Pasaron 41 tests de
`render::vehicles`, Clippy estricto de cliente y `git diff --check`; #329
continúa abierta por layouts/call sites GUI, efectos avanzados y consumidores
legacy restantes.

Corrección #329-VEHICLE-PICK-RUNTIME-OFFSETS (2026-09-12, `33a87136`): el
hit-test del clic reutiliza ahora la primera capa NewGRF que el mapa resolverá
para la misma unidad, pose, carga y librea. Un SpriteStack o Action2 con
offsets runtime ya no sólo se dibuja desplazado: también se selecciona en su
centro visible. El input conserva la ruta catalog-aware cuando el arnés no
instala `TruckHandles`, caché o `Assets<Image>`. La regresión
`pick_vehicle_uses_runtime_sprite_offsets` separa el centro runtime del
vanilla y comprueba ambos contratos. Pasaron 42 tests de `render::vehicles`,
Clippy estricto de cliente y `git diff --check`; #329 continúa abierta por
callbacks y consumidores visuales legacy restantes.

Corrección #329-VEHICLE-PICK-SPRITE-BOUNDS (2026-09-12, `efb35321`): el
hit-test deja de usar un radio fijo alrededor del centro y comparte con el
renderer el rectángulo de cada vista, incluyendo `width`/`height`, offsets
catalog-aware y la unión de todas las capas runtime de un `SpriteStack`. Esto
permite seleccionar un vehículo en cualquier píxel de su huella visible sin
ampliar artificialmente la zona de clic de sprites pequeños. Las regresiones
`pick_vehicle_uses_catalog_sprite_offsets` y
`pick_vehicle_uses_union_of_runtime_sprite_stack_layers` comprueban tanto el
borde interior/exterior de un sprite custom de 80 px como una segunda capa
runtime separada de la primera. Pasaron 43 tests de `render::vehicles`,
Clippy estricto de cliente y core, formato y `git diff --check`. #329 continúa
abierta por callbacks, efectos avanzados, layouts GUI y consumidores legacy
restantes.

Corrección #326/#329-AIRPORT-TILE-COMPANY-PALETTE (2026-09-12, `f0b31430`):
los sprites Action1/TileSeq de `AirportTile` ya no se suben como RGBA crudo.
El caché conserva el color del dueño en la clave y hornea la máscara de
compañía para suelo, BUILD/children y la vista Action1/3 plana, igual que las
rutas NewGRF de estaciones e industrias. La regresión
`rotated_newgrf_airport_layout_selects_relative_runtime_and_action5_foundation`
usa una rampa autora en la variante E/O, comprueba el color Green y conserva
la relación ground-child con la fundación Action5; pasaron las cuatro pruebas
de aeropuerto filtradas, Clippy estricto de cliente y core, formato y
`git diff --check`. Esto cubre sólo máscaras de compañía en sprites custom:
#326/#329 siguen abiertas por paletas base/custom no representadas, rotaciones
exhaustivas, sonidos y callbacks restantes.

Corrección #326/#329-AIRPORT-TILE-TRACE-PALETTE (2026-09-12, `8c42260f`):
`world-draw` registra ahora `PALETTE_RECOLOUR_START + owner_colour` para la
vista plana Action1/3 de `AirportTile`, alineando la evidencia exportada con
la textura que el renderer ya hornea. Clippy estricto de cliente, formato y
`git diff --check` pasan; no se amplía el alcance de la subetapa visual ni se
cierran #326/#329.

Corrección #326/#329-DIRECT-COMPANY-PALETTE (2026-09-12, `c5b3e166`): el
resolver común de `TileLayout` reconoce ahora las paletas directas
`PALETTE_RECOLOUR_START..=+15` en sprites Action1, las hornea una sola vez y
limpia la máscara para que el cliente no las recoloree de nuevo con el dueño.
Una paleta directa fuera del rango soportado (por ejemplo 2CC) deja el layout
incompleto y activa su fallback atómico, en lugar de mostrar RGBA crudo. La
regresión `tile_layout_honours_explicit_company_palette_on_action1_sprite`
comprueba Red y el rechazo de 2CC; pasaron 5 pruebas de TileLayout y 4 de
AirportTile, Clippy estricto de cliente/core, formato y `git diff --check`.
El soporte de mapas 2CC, paletas custom Action1, transparencia y var10 de
paleta continúa pendiente.

Corrección #329/#425-OBJECT-2CC-PALETTE (2026-09-12, `b96d1448`): los objetos
NewGRF que declaran `ObjectFlag::Uses2CC` ya no suben sus vistas o piezas
`TileSeq` como RGBA crudo. El byte `OBJS.colour` conserva la combinación
`colour1 + colour2 * 16` de la librea por defecto al construir, el caché
incluye ese byte y hornea ambas rampas para vistas planas, suelo, parents y
children. La regresión de cliente contrasta dos libreas sobre los mismos
píxeles; la de core verifica el offset inicial y el fallback de una sola rampa.
Esto no cierra #329/#425: paletas custom Action1 con var10, transparencia,
layouts 16-bit y otros consumidores visuales siguen pendientes.

Corrección #329/#425-OBJECT-2CC-ACTION5-MAP (2026-09-12, `80709ae9`): los
caches de objetos reciben la tabla Action5 `0x0A`, calculan el slot a partir de
`Object::colour` y aplican el mapa tanto a las vistas planas como a `TileSeq`.
Los cambios de tabla invalidan los handles para no reutilizar texturas
horneadas con un GRF anterior. La regresión de objetos verifica ambos caminos;
las issues madre siguen abiertas por `PALETTE_VAR10`, transparencia, layouts
16-bit y demás consumidores visuales.

Corrección #326/#329-TILELAYOUT-CUSTOM-ACTION1-PALETTE (2026-09-12,
`7518216e`): el resolver común de `TileLayout` hornea mapas de paleta Action1
estáticos representados por sprites `256×1` sobre las entradas Action1 de
suelo, parents y children. La máscara se limpia después para que el cliente no
reaplique la librea de compañía; mapas ausentes o inválidos conservan el
fallback atómico. La regresión
`tile_layout_honours_static_custom_action1_palette` y los 15 tests de layouts
quedan verdes con Clippy estricto y formato. Las cadenas con registros,
`PALETTE_VAR10`, transparencia y layouts 16-bit continúan pendientes; no se
cierran #326/#329.

Corrección posterior #326/#329-TILELAYOUT-PALETTE-REGISTER (2026-09-12,
`f3d9e2f2`): `TLF_PALETTE` aplica desplazamientos firmados válidos a la entrada
seleccionada de un mapa Action1 o a una paleta directa de compañía. Los límites
se validan antes de hornear y los casos no representables mantienen el
fallback atómico. Las cadenas `PALETTE_VAR10`, transparencia y layouts 16-bit
siguen pendientes; no se cierran #326/#329.

Corrección #329/#567-VEHICLE-SAV-MONORAIL-MAGLEV (2026-09-12, `f443f713`): el
puente vanilla de `ENGN` reconoce los slots nativos `54` (X2001/monorail) y
`84` (Lev1/maglev), además de los motores ya cubiertos. Las regresiones
comprueban que ambos IDs se resuelven sin aliasar modelos de otros climas.
La creación de un `ENGN` cuando falta el chunk sigue deliberadamente pendiente:
la tabla nativa es densa y requiere materializar el prefijo completo del pool
para no sobrescribir motores no representados.

Corrección #326/#329-STATION-ADVANCED-LAYOUT-PARSE (2026-09-12, `eb1385c6`):
el parser de metadatos de `Stations` consume ahora `prop 0x1A` (advanced sprite
layout) con el mismo orden de `OpenTTD`: contador de building sprites, flags,
origen/extensión parent y registros de sprite, paleta y `var10`. Esto evita que
un `PALETTE_VAR10` o un offset de caja haga que el parser trate la siguiente
propiedad como parte del layout y pierda nombre, animación o badges. También se
cerró el acceso truncado del `extended byte`. La regresión combina registros de
ground y parent con propiedades de animación posteriores. Este corte sólo
arregla el consumo del formato; la materialización visual runtime del layout
legacy y la resolución dinámica de sus paletas siguen pendientes, por lo que
#326/#329 permanecen abiertas.

Corrección #326/#329-STATION-ADVANCED-LAYOUT-RUNTIME (2026-09-13, `3bb2a77b`):
los layouts `Stations` Action0 `prop 0x1A` ya no se descartan después del parseo:
se conservan por id local, se transfieren al runtime de `StationSpecDef` y se
seleccionan por orientación X/Y antes de resolver suelo y secuencia de
parents/children. La ruta avanzada admite el `PALETTE_VAR10` que OpenTTD permite
en este lector, hornea el mapa Action1 elegido y mantiene la ruta Action2
restrictiva (`allow_var10=false`) para no hacer permisivos otros layouts. La
regresión integrada cubre las dos orientaciones, origen/extensión y la
materialización de la secuencia; core (2707 tests), cliente, Clippy, formato y
`git diff --check` quedaron verdes. Paletas no-Action1 dinámicas, transparencia,
layouts 16-bit y la cobertura completa de Action3/relocación siguen pendientes;
#326/#329 no se cierran.

Corrección #326/#329-STATION-LOCAL-ID (2026-09-13, `9876c5cf`): los metadatos
de `Stations` conservan el primer id local declarado por Action0. La aplicación
usa ese id para asociar layouts avanzados, vistas y `copy_layout`, y deja de
suponer que la posición del bloque en el GRF es el id. La regresión integrada
usa una estación con id local `7` y verifica tanto el runtime como
`StationSpecDef`. Los bloques Action0 que declaran varios ids todavía requieren
expandir el rango completo; #326/#329 continúan abiertas.

Corrección #326/#329-STATION-ACTION0-RANGE (2026-09-13, `c2045622`): la
aplicación de Stations materializa ahora todos los IDs consecutivos de
`num_ids`, replicando por cada uno las propiedades comunes, el `copy_layout`,
los layouts avanzados y el vínculo con Action3. La regresión usa el rango
`7..8` y comprueba que ambos IDs reciban el runtime visual. El corte cubre
ranges representables con IDs byte; la codificación `ExtendedByte` para
estaciones por encima de 255 queda separada, por lo que #326/#329 continúan
abiertas.

Corrección #326/#329-STATION-EXTENDED-ID (2026-09-13, `bbbb7440`): el primer
ID local de Action0 para `Stations` se lee y conserva como `u16` cuando llega
con la codificación `ExtendedByte`; el rango completo se valida antes de
consumir propiedades y se expande sin truncamiento en catálogo, layouts
avanzados, vistas, `copy_layout`, Action3 y callbacks de disponibilidad,
pendiente y animación. La regresión importa los IDs `300..301` y verifica que
la identidad de wire llegue al catálogo. Esto elimina la colisión byte/WORD,
pero no cubre todavía todos los layouts dinámicos, paletas especiales,
transparencia, relocación ni scopes restantes; #326/#329 continúan abiertas.

Corrección #326/#329-STATION-LEGACY-LAYOUT (2026-09-13, `83068c26`): el
parser y runtime materializan la propiedad Action0 `0x09` con el contrato
clásico de `Stations`: ground, secuencias BUILD/child, cajas de parents y
terminador variable. El bit histórico de selección Action1 se interpreta con
la inversión nativa de los sprites de building; `0x0A` copia esa tabla por ID
local, y un layout vanilla que depende de la tabla interna conserva el
fallback atómico en vez de inventar un sprite base. Las regresiones cubren
propiedades posteriores, child sin caja, copia local y materialización contra
Action1. Continúan pendientes las paletas no representables, callbacks/scopes
completos y la cobertura de layouts dinámicos; #326/#329 siguen abiertas.

Corrección #326/#329-TILELAYOUT-FLAG-GUARDS (2026-09-13, `ff9ebbe4`): los
lectores de layouts de `Stations` `0x1A` y de grupos Action2 validan ahora el
contrato nativo antes de consumir origen, cajas o registros. Se rechazan flags
de caja en el ground, words de flags fuera del byte conocido, `var10` en
Action2 (donde `allow_var10=false`), cadenas `var10` sin una referencia
Action1/registro compatible y valores mayores que el máximo nativo `7`. En
Stations los valores `var10` se comprueban después de origen/extensión, que es
su posición real en el wire format; así no se confunde un origen válido con el
registro siguiente. Las regresiones cubren ground inválido, flags desconocidos,
consumo correcto de registros y límite `var10`. Esto evita layouts desalineados
o materializados con un contrato que OpenTTD deshabilitaría; paletas especiales,
transparencia, relocación y scopes restantes siguen pendientes, por lo que
#326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-HOUSE-BUILD (2026-09-13): las
secuencias `BUILD` de casas que referencian directamente un overlay `s2` del
baseset ahora reutilizan `WorldAssets.houses` y sus anclas de
`HOUSE_DRAW_DATA`. El resolver compara todas las vistas y etapas que usan el
mismo ID y sólo lo materializa si la geometría es única; las paletas de casa,
los suelos `s1` no auditados y el draw-proc del ascensor siguen usando sus
contratos/fallback propios. #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-SPRITE-MODIFIERS (2026-09-13, `00fb3251`): los
lectores de `Stations` legacy `0x09`, `Stations` avanzado `0x1A` y grupos
Action2 traducen los bits nativos de modifier antes de tratar los words como
IDs. `palette` bit 14 se conserva como `opaque`, `sprite` bit 14 como
`transparent` y `sprite` bit 15 como `recolour`; los bits se eliminan de los
IDs directos y de las referencias Action1, y el metadato acompaña al layout
resuelto. Las regresiones cubren las tres entradas y verifican la resolución
posterior. La aplicación efectiva de transparencia, recolour y la paleta por
defecto en el blitter Bevy es una subetapa separada; #326/#329 permanecen
abiertas.

Corrección #326/#329-TILELAYOUT-PALETTE-POLICY (2026-09-13, `1c16316c`): las
cachés de estaciones, industrias, casas, objetos, road-stops y airport-tiles
incluyen los modifiers en su identidad para no reutilizar una textura de otra
variante. La paleta por defecto se hornea sólo cuando el layout declara
`transparent` o `recolour`; `opaque` por sí solo conserva los píxeles base.
Road-stops y airport-tiles pasan además el color de compañía al mismo helper
que usa el resto del renderer, y un ground directo del baseset con modifier no
representable mantiene el fallback atómico. Esto cubre la selección de paleta
y evita recolors espurios; la composición destino de `transparent`, la
visibilidad de `opaque` frente a las preferencias de ocultar y las paletas
especiales continúan pendientes, por lo que #326/#329 siguen abiertas.

Corrección #326/#329-TILELAYOUT-OPAQUE-ALPHA (2026-09-13, `57567539`): las
secuencias BUILD de Stations, road-stops, airport-tiles, casas, industrias y
objetos ya no aplican el alpha de la preferencia de transparencia a una entrada
que trae `SPRITE_MODIFIER_OPAQUE`. El helper común conserva RGB y restaura alpha
1 sólo para esa entrada; las entradas sin `opaque` mantienen el alpha de su
categoría y la política de paleta de la etapa anterior. La regresión cubre la
transformación de color sin tocar estado global. La supresión de entradas
non-opaque bajo `IsInvisibilitySet`, la composición destino de `transparent` y
las paletas especiales quedan como subetapas distintas; #326/#329 continúan
abiertas.

Corrección #326/#329-TILELAYOUT-INVISIBILITY (2026-09-13, `8a8b4e5b`): el
renderer aplica ahora `IsInvisibilitySet` por entrada en las secuencias BUILD
de Stations, road-stops, airport-tiles, casas, industrias y objetos. Un parent
no opaco oculto limpia el parent activo y salta sus children hasta el siguiente
parent; un parent `opaque` y sus children declarados siguen materializándose.
El suelo permanece fuera del filtro porque OpenTTD lo entrega mediante
`DrawGroundSprite` antes de `DrawCommonTileSeq`; también se conservan
fundaciones y catenaria. El fallback vanilla no reaparece cuando un layout
completo queda enteramente oculto. La composición destino de `transparent` y
las paletas especiales siguen pendientes, por lo que #326/#329 continúan
abiertas.

Corrección #326/#329-TILELAYOUT-CRASH-PALETTE (2026-09-13): las entradas
Action1 de `TileLayout` que seleccionan la paleta nativa `PALETTE_CRASH` (804)
ya no se descartan automáticamente. El core reutiliza el mismo horneado
`MakeDark` que ya cubre el renderer de vehículos y limpia la máscara después
de materializarlo, evitando que el cliente lo recoloree una segunda vez. Las
paletas 2CC, transparencia de destino y demás tablas especiales continúan
conservando fallback atómico hasta tener su compositor/tabla correspondiente;
la regresión `tile_layout_honours_crash_palette_on_action1_sprite` fija el
alcance de esta subetapa y #326/#329 siguen abiertas.

Corrección #326/#329-TILELAYOUT-STRUCTURE-PALETTE (2026-09-13): las entradas
Action1 de `TileLayout` que seleccionan `PALETTE_TO_STRUCT_BLUE..YELLOW`
(`795..801`) o las variantes de iglesia (`1438..1439`) ya no se descartan. El
core conserva la paleta directa que no puede hornear sin conocer las tablas de
sprites, y el cliente reutiliza las tablas DOS verificadas de estructuras para
recolorear la textura RGBA. Las cachés de estaciones, road-stops,
airport-tiles, casas, industrias y objetos incluyen ese id en su clave para no
compartir una imagen entre tonos estructurales; las paletas de destino, 2CC y
las referencias directas del baseset siguen con fallback atómico. La regresión
`tile_layout_preserves_structure_palette_on_action1_sprite` fija el contrato de
resolución y #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-NEWSPAPER-PALETTE (2026-09-13): las entradas
Action1 de `TileLayout` que seleccionan `PALETTE_NEWSPAPER` (`803`) aplican en
el core la misma conversión entera `MakeGrey` del blitter 32bpp nativo,
conservando alpha y limpiando la máscara después del horneado. Así no se
reaplica la paleta en el cliente ni se confunde esta transformación de píxeles
con `PALETTE_TO_TRANSPARENT` (que depende del destino). `PALETTE_TO_BARE_LAND`,
`PALETTE_TO_TRANSPARENT`, 2CC y las referencias directas del baseset siguen en
fallback atómico hasta verificar sus tablas/compositor; la regresión
`tile_layout_honours_newspaper_palette_on_action1_sprite` fija el alcance y
#326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-BARE-LAND-PALETTE (2026-09-13): las entradas
Action1 de `TileLayout` que seleccionan `PALETTE_TO_BARE_LAND` (`791`) usan la
tabla DOS de 256 entradas verificada en `ogfx1_base.grf`. El core aplica el
remapeo sobre sprites 8bpp y sobre máscaras 32bpp, conserva el brillo de las
entradas con máscara y limpia la máscara una vez materializado el resultado.
Los índices no modificados por la tabla mantienen su color; si un sprite no
permite recuperar un índice DOS completo se conserva el fallback atómico. La
regresión `tile_layout_honours_bare_land_palette_on_action1_sprite` fija el
contrato; `PALETTE_TO_TRANSPARENT`, 2CC y referencias directas del baseset
continúan pendientes y #326/#329 siguen abiertas.

Corrección #326/#329-TILELAYOUT-TRANSPARENT-PALETTE (2026-09-13): las
entradas `BUILD` de `TileLayout` que declaran `PALETTE_TO_TRANSPARENT` (`802`)
junto al modifier nativo `transparent` conservan la paleta directa y el
renderer Bevy las materializa como una máscara negra con cobertura por píxel.
Esto reproduce el oscurecimiento del destino a `3/4` del blitter 32bpp,
respeta alpha parcial y evita aplicar otra vez el alpha de transparencia de la
categoría. Sin el modifier nativo la paleta se ignora como en
`SpriteLayoutPaletteTransform`; el ground y combinaciones no representables
mantienen fallback atómico. Las regresiones cubren la resolución core, la
máscara RGBA y la ausencia de doble alpha; 2CC, referencias directas del
baseset y el resto de compositor siguen pendientes, por lo que #326/#329
continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-2CC (2026-09-13): los registros
`BUILD` de `TileLayout` que usan una paleta directa del rango
`SPR_2CCMAP_BASE` (`5680..5935`) junto al modifier `recolour` conservan el
par primario/secundario codificado en la paleta y reutilizan el horneado 2CC
existente, incluida la tabla Action5 del slot exacto. En objetos, el cache
elige esa tabla por la paleta del layout y no por el color por defecto de la
instancia. Sin `recolour` la paleta se ignora como en el transformador nativo;
la combinación `transparent + 2CC`, que remapea el destino del framebuffer,
mantiene fallback atómico. Las regresiones cubren resolución core, mapa
Action5 y separación de caches; `PALETTE_TO_TRANSPARENT` ya tiene su propia
subetapa, las referencias directas del baseset y el resto del compositor
siguen pendientes, por lo que #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-FLAT-GROUND (2026-09-13): las
referencias directas de `TileLayout::ground` a los suelos planos vanilla ya no
caen al fallback cuando el layout usa una de las densidades de césped
(`3924/3943/3962/3981`), las cinco variantes rough (`4000/4019..4022`), las
dos series rocosas (`4023/4042`), agua (`4061`) o nieve/desierto
(`4493/4512/4531/4550`). La whitelist se basa en `table/sprites.h` y cada
entrada se enlaza con su asset atlas real (`grass_density`, `rough_flat`,
`rocky`, `snow_desert` o `water`), conservando la geometría NFO plana
64×31/`(-31,0)`. Las pendientes, sprites de edificio y grounds con modifier
siguen en fallback hasta tener un contrato de ancla/paleta independiente; la
regresión cubre todos los IDs admitidos y rechaza `4062`. #326/#329 continúan
abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-BUILD-GROUND (2026-09-13): las
entradas `BUILD` que referencian uno de esos mismos sprites planos vanilla ya
no invalidan el layout completo. Las seis rutas de emisión reutilizan el
atlas y el ancla auditados del ground (`64×31`, `(-31,0)`), mientras que la
caja 3D/origen de la entrada sigue gobernando parent, child y profundidad; no
se crean handles ni se reservan slots de caché para un sprite baseset que ya es
global. La prueba de spawn recorre los 16 IDs admitidos y comprueba además una
entrada BUILD directa separada del ground. Cualquier base sprite no auditado,
paleta o modifier conserva fallback atómico; #326/#329 continúan abiertas.

Refinamiento #326/#329-TILELAYOUT-DIRECT-BUILD-CACHE (2026-09-13): una
secuencia `BUILD` compuesta únicamente por esos sprites globales ya no exige
un slot de Action1/Action5 que no va a utilizar. Road-stops y airport-tiles
sólo validan capacidad cuando la secuencia contiene Action1; una secuencia
mixta sigue rechazándose si no puede reservar todos sus handles antes de
emitir. Las regresiones cubren `spec/gfx = 1024..u16::MAX` y mantienen el
fallback para layouts mixtos que desbordan caché. #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-GROUND-PALETTE-GUARD (2026-09-13): el
cliente vuelve a exigir `direct_palette == 0` antes de materializar un ground
directo del baseset. El core ya marca como incompleto el caso en producción,
pero el guard adicional evita que una entrada construida desde un registro
parcial o un consumidor futuro ignore una paleta explícita y pinte el suelo
crudo. La regresión cubre `PALETTE_TO_BARE_LAND` y conserva el fallback
atómico; las paletas de ground custom siguen su contrato separado. #326/#329
continúan abiertas.

Corrección #326/#329-TILELAYOUT-GROUND-TRANSPARENT-PALETTE (2026-09-13): el
resolver aplica ahora `PALETTE_TO_TRANSPARENT` también al ground custom cuando
está presente el modifier nativo `recolour`, que es la condición que usa
`GroundSpritePaletteTransform`; puede coexistir con `transparent`. El cliente
convierte esa entrada en la misma máscara negra de destino con cobertura 1/4
por píxel que usa para BUILD, sin aplicar el alpha de categoría dos veces. Un
ground sin `recolour` sigue ignorando la paleta 802, y BUILD sin su modifier
`transparent` conserva fallback según el contrato nativo. Las regresiones
cubren resolución core y textura Bevy; #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-SPRITE-OFFSET (2026-09-13): las
referencias directas del baseset que declaran `TLF_SPRITE` aplican ahora el
offset firmado del registro antes de decidir si el sprite pertenece a la
whitelist materializable. Esto permite que una entrada seleccione, por
ejemplo, `SPR_FLAT_GRASS_TILE` desde el id inmediatamente anterior, igual que
`SpriteLayoutProcessor::ProcessRegisters` de OpenTTD. Valores fuera de
`u16`, `var10`, paletas custom o resultados no auditados conservan fallback
atómico; la regresión cubre selección válida y desborde sin truncarlo. #326/#329
continúan abiertas.

Corrección #326/#329-TILELAYOUT-2CC-MAP-PROPAGATION (2026-09-13): las
texturas de `TileLayout` para estaciones, industrias, casas y los layouts
sintéticos de roadstop/aeropuerto reutilizan ahora la tabla Action5 `0x0A`
correspondiente al `PaletteID` 2CC directo, igual que los objetos. La caché
conserva una copia acotada a los 256 slots nativos y descarta sus handles
cuando cambia el runtime, evitando tanto un índice fuera de rango como una
textura horneada con el GRF anterior. Las regresiones cubren los cuatro
consumidores y el reemplazo de una tabla; #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-SLOPED-GROUND (2026-09-13): las
referencias directas del baseset a terreno inclinado dejan de caer al
fallback cuando apuntan a los rangos auditados de `table/sprites.h`: las
cuatro densidades de césped (`3924..3999`), rough/variantes rocosas
(`4000..4060`), agua (`4061..4079`) y nieve/desierto (`4493..4568`). El
cliente enlaza cada offset con su asset atlas real y conserva las anclas NFO
variables, incluidos los sprites parciales de agua (`4070..4079`), en lugar
de imponer el 64×31 plano. Los modifiers y paletas directas siguen forzando
fallback atómico. Las regresiones cubren todos los IDs admitidos, rangos
fuera del contrato y los anclajes normales/parciales; #326/#329 continúan
abiertas por los layouts y callbacks restantes.

Corrección #326/#329-TILELAYOUT-DIRECT-STATION-BUILD (2026-09-13): las
secuencias `BUILD` con referencias directas al banco vanilla ya no se
descartan cuando usan piezas de estación rail/mono/maglev (`1069..1086`,
`1151..1168`, `1233..1250`, `4974..4981`) o airport (`2095`, `2601`,
`2633..2691`, `3981`, `4982`, `5966..5968`). El cliente reutiliza el atlas
específico de cada namespace y
los metadatos NFO reales de cada pieza (incluidos edificios altos, techos,
radar y helipads), manteniendo `TILE_SEQ` como origen de la caja y el orden
parent/child existente. Los IDs fuera del catálogo, modifiers y paletas
explícitas conservan fallback atómico; #326/#329 continúan abiertas por los
namespaces de objetos/vehículos, callbacks y layouts aún no auditados.

Corrección #326/#329-TILELAYOUT-DIRECT-INDUSTRY-BUILD (2026-09-13): las
secuencias `BUILD` de industrias que referencian directamente un overlay del
baseset ahora reutilizan `WorldAssets.industries` y los offsets publicados en
`INDUSTRY_GFX_DATA`. El resolver recorre las cuatro etapas, sólo acepta un ID
cuando todas sus apariciones conservan la misma geometría NFO y mantiene el
fallback si el atlas falta o la referencia usa paleta/modifier. Los suelos
industriales (`ground_sprite_id`) y draw-procs animados quedan explícitamente
fuera de esta subetapa para no confundir su contrato con un overlay BUILD;
#326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-INDUSTRY-GROUND (2026-09-13): el
`ground` directo de un layout puede reutilizar ahora los `ground_sprite_id`
industriales auditados, incluidos los pisos parciales de 32/44/46 píxeles,
con el atlas `industries` y sus anclas `ground_w/ground_h/ground_xrel/ground_yrel`.
La tabla se valida por todas sus etapas para no seleccionar una geometría
arbitraria; modifiers, paletas y draw-procs animados siguen en fallback
atómico. #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-HOUSE-GROUND (2026-09-13): las capas
`s1` de `HOUSE_DRAW_DATA` que usan sprites de casa como suelo directo ahora
reutilizan el atlas `houses` y sus anclas `s1_w/s1_h/s1_xrel/s1_yrel`, además
de los suelos comunes del baseset. La geometría se acepta sólo cuando es
idéntica en todas las vistas y etapas del ID; las paletas/modifiers explícitas
y los procedimientos dinámicos conservan fallback atómico. #326/#329
continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-OBJECT-BUILD (2026-09-13): la ruta
`DrawNewObjectTile` resuelve ahora las referencias directas de `object_land.h`
con el atlas y las anclas NFO del namespace de objetos: concreto (`1420`),
transmisor (`2601`), faro (`2602`), estatua de compañía (`2632`) y terreno
comprado (`4790`). El resolver es contextual: el ID `2601` conserva su
geometría de aeropuerto en los consumidores de estación, mientras que un
layout de objeto usa `object_transmitter.png`; las referencias Action1,
paletas/modifiers y sprites no auditados mantienen fallback atómico. Las
regresiones cubren atlas, tamaño y ancla de los cinco IDs; #326/#329 continúan
abiertas por vehículos, callbacks, layouts y namespaces restantes.

Corrección #326/#329-OBJECT-HQ-VANILLA-RENDER (2026-09-13): las sedes de
compañía vanilla (`OBJECT_HQ`) dejan de caer en césped genérico. El cliente
carga los 29 sprites `2603..2631`, resuelve el nivel desde `M4` y la posición
dentro de la huella 2×2 desde `OBJS`, aplica la paleta de compañía y emite
las tres piezas BUILD con sus cajas `TILE_SEQ_LINE` de 20/50/60 unidades al
sorter global. Las pendientes usan la fundación nivelada y mantienen el
ground como child, igual que `DrawTile_Object`; la regresión cubre footprint,
nivel, atlas y bounds. #326/#329 continúan abiertas por los namespaces,
callbacks y contratos visuales restantes.

Corrección #326/#329-TILELAYOUT-DIRECT-OBJECT-HQ (2026-09-13): las referencias
directas `2603..2631` de una secuencia `TileLayout` de objetos reutilizan ahora
el atlas de las cinco etapas HQ y la geometría NFO individual, incluidos los
BUILD altos de niveles 2..4. La ampliación se mantiene contextual al namespace
`DrawNewObjectTile`: el resolver global no incorpora ese rango y conserva las
colisiones con túneles, estaciones y herramientas. La regresión cubre los 29
sprites, sus anclas y el fallback de renderizabilidad entre namespaces;
#326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-ROADSTOP (2026-09-13): los layouts de
`RoadStops` pueden materializar referencias directas a los grounds de bus y
truck (`2692..2695`, `2708..2711`), sus piezas BUILD (`2696..2723`) y las
tiras drive-through (`5978..5985`) usando los atlas y metadatos NFO de las
tablas viales vanilla. La selección es contextual a `DrawRoadStop`, por lo que
el resolver global conserva sus namespaces y fallback atómico ante modifiers,
paletas o IDs no auditados. La regresión cubre ground, BUILD y ambos ejes de
bus/truck; #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-ROADWAYPOINT (2026-09-13): los
`TileLayout` de `RoadWaypoint` pueden materializar las cuatro referencias
directas de postes vanilla (`6141..6144`) con el atlas `road_waypoint` y los
anchos/anclas NFO de `station_land.h`. El contrato contextual se mantiene
separado de las paradas bus/truck, aunque comparte la validación de slots
Action1 y la emisión de parents/children; paletas, modifiers e IDs fuera de
las tablas auditadas conservan el fallback atómico. Las regresiones cubren
ambos ejes, el aislamiento de namespaces y la geometría de los cuatro
sprites; #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-RAILWAYPOINT (2026-09-13): los
`TileLayout` de `RailWaypoint` pueden materializar ahora sus ocho referencias
directas vanilla (`4974..4981`) con `WorldAssets.rail` y la geometría NFO
específica de `rail_waypoint_layer_meta`. Las mitades este reutilizan el ancla
oeste que exige `TILE_SEQ_LINE`, en lugar de tomar el `xrel` crudo del PNG;
ground y BUILD usan el mismo namespace contextual y las estaciones rail
normales conservan el resolver genérico. Paletas, modifiers, IDs fuera de las
tablas e imágenes ausentes mantienen fallback atómico. Las regresiones cubren
los ocho atlas/anclajes, la diferencia frente a la tabla genérica y el
aislamiento del ground; #326/#329 continúan abiertas.

Corrección #326/#329-TILELAYOUT-DIRECT-AIRPORT-GROUND (2026-09-13): los
`AirportTileLayout` pueden materializar referencias directas de todo el banco
airport (`2095`, `2601`, `2633..2691`, `3981`, `4982`, `5966..5968`) también
en `ground`, no sólo en `BUILD`. El resolver contextual prioriza
`WorldAssets.airport_station` y sus metadatos NFO, evitando que IDs compartidos
como `2601` elijan la textura de otro feature; la fundación Action5, la
rotación y el fallback atómico de paletas/modifiers se conservan. La regresión
cubre los 66 sprites en ground y BUILD; #326/#329 continúan abiertas por la
matriz completa de callbacks, rotaciones, sonidos y aceptación raster.

Corrección #326/#329-TILELAYOUT-DIRECT-RAIL-STATION-GROUND (2026-09-13): los
`TileLayout` de estaciones ferroviarias normales pueden materializar ahora sus
referencias directas al banco rail (`1069..1086`, mono/maglev y capas de
waypoint compartidas) también en `ground`, usando `WorldAssets.rail` y la
geometría NFO de cada sprite. La decisión queda contextualizada en
`DrawTile_Station`; los layouts de otros consumidores conservan el contrato
global y el fallback ante paletas, modifiers, IDs no auditados o atlas ausente.
La regresión cubre las capas rail para los cuatro tipos de red en ground y
BUILD; #326/#329 continúan abiertas por callbacks, rotaciones exhaustivas,
otros namespaces y aceptación raster.

Corrección #326/#329-TILELAYOUT-DIRECT-RAIL-STATION-TRACK-GROUND (2026-09-13):
el namespace contextual de estación ferroviaria acepta también las seis vías
compuestas que pueden actuar como superficie directa (`1011/1012`,
`1093/1094` mono y `1175/1176` maglev). Se reutiliza su atlas rail y la
geometría plana NFO de OpenGFX (`64×31`, `(-31,0)`), tanto en `ground` como en
una entrada `BUILD`; el resolver global sigue rechazándolas como ground para
no ampliar otros consumidores. La regresión cubre los seis IDs, sus atlas y
el rechazo de paletas explícitas; #326/#329 continúan abiertas.

Corrección #326/#565-TRAMTYPE-SLOPED-NFO-ANCHOR (2026-09-13): las superficies
custom de roadtypes y tramtypes vuelven a usar `x_offs/y_offs/width/height`
del sprite resuelto también sobre pendientes y foundations. La ruta anterior
las recentraba con la media altura vanilla (`tile_pos_half`), desplazando
sprites pequeños o HD aunque en plano conservaran el ancla nativa; la
referencia de OpenTTD las emite siempre mediante `DrawGroundSprite`, cuyo
blitter mantiene esos offsets. La regresión de tranvía inclinado comprueba
textura, relación child/foundation y posición exacta; #326/#565 continúan
abiertas por la matriz restante de superficies, depósitos, clipping y
aceptación raster.

Corrección #326-ROAD-DEPOT-ACTION5-OVERLAY-PURITY (2026-09-13): un roadtype
eléctrico válido que cae en `DEPOT_NO_TRACK` ya no recibe además la tira
`SPR_TRAMWAY_OVERLAY`. OpenTTD reserva esa capa separada para el depósito de
tram puro (`road_rt == INVALID_ROADTYPE`); el roadtype válido ya obtiene su
fachada completa de la relocalización Action5. La regresión conjunta conserva
las fachadas `6099/6100` y exige cero overlays duplicados. #326/#565 siguen
abiertas por la selección Action5 de tramtypes custom, clipping, pivotes y
framebuffer.

Corrección #326-ROAD-DEPOT-PREVIEW-SORT (2026-09-13): las fachadas del ghost
vial entran ahora en `ViewportSortableParent` con la caja inclusiva de cada
`TILE_SEQ_LINE`, profundidad por columna y ordinal local del renderer del
mapa. Las variantes `ROTSG_DEPOT` conservan el ID vial y las relocalizaciones
Action5 usan el ID `TRAMWAY` correspondiente para sus desempates; suelo y
overlay permanecen en el pase ground. #326 continúa abierta por clipping,
pivotes y aceptación raster.

Corrección #326-ROAD-DEPOT-PREVIEW-ASSET-PARITY (2026-09-13): el ghost vial
consume ahora `WorldAssets.road_depot_ground`, `tram_flat` y las fachadas
`road_depot_builds` cuando el atlas está disponible, manteniendo los PNG como
fallback de arranque/tests y sin alterar los resolutores `ROTSG_DEPOT` ni
Action5. El recolor de compañía queda en el mismo punto que el renderer del
mapa. #326 continúa abierta por clipping, pivotes y aceptación raster.

Corrección #326-RAIL-DEPOT-PREVIEW-ASSET-PARITY (2026-09-13): el preview de
depósitos ferroviarios usa las mismas entradas de `WorldAssets.rail` y
`rail_depot_builds` que `DrawRailTile`, incluyendo los bancos de rail normal,
eléctrico, monorriel y maglev; el PNG suelto queda sólo como fallback de
fixtures sin atlas. La recoloración de compañía se ejecuta antes del alpha del
ghost y la vía de salida conserva su sprite tipado remapeado. #326 continúa
abierta por orden global del preview, catenaria, Action5/NewGRF, clipping y
aceptación raster.

Corrección #326-RAIL-DEPOT-PREVIEW-SORT (2026-09-13): las fachadas del ghost
ferroviario entran ahora en `ViewportSortableParent` con el mismo prisma
`TILE_SEQ_LINE`, profundidad fuente y clave local que `DrawRailTileSeq`. La
vía de salida sigue siendo suelo/child y no se convierte artificialmente en
parent; la regresión compara la caja inclusiva de la capa NE con el helper
runtime. #326 continúa abierta por catenaria del preview, `RTSG_DEPOT`/Action5,
clipping y aceptación raster.

Corrección #326-RAIL-DEPOT-PREVIEW-CATENARY (2026-09-13): la preview de un
depósito eléctrico dibuja ahora el cable especial de entrada de
`_rail_catenary_sprite_data_depot`, con atlas `WorldAssets`, reemplazo Action5,
alpha de transparencia y el mismo parent sortable/ordinal que el mapa. El
helper de regresión cubre los ejes X/Y de los bounds inclusivos. #326 continúa
abierta por catenaria de pórticos si el contrato NewGRF la exige, `RTSG_DEPOT`,
clipping y aceptación raster.

Corrección #326-RAIL-DEPOT-PREVIEW-RTSG (2026-09-13): la preview de depósito
consulta ahora `rail_type_depot_newgrf` con el mismo contexto Action2 de vía
(tipo seleccionado, terreno, random, fecha, tablas y parámetros GRFID) que el
mapa. La selección de los seis slots relocatables (`SE_1`, `SE_2`, `SW_1`,
`SW_2`, `NE`, `NW`) quedó centralizada y las vistas HD conservan sus offsets
NFO. Si el grupo no resuelve, el fallback vanilla sigue activo; #326 continúa
abierta por clipping y aceptación raster.

Corrección #326-RAIL-STATION-PREVIEW-SORT (2026-09-13): las capas BUILD del
ghost de estación ferroviaria con bounds conocidos se publican ahora como
`ViewportSortableParent`, con la misma caja `TILE_SEQ`, profundidad de columna
y ordinal del renderer materializado. El suelo de vía continúa siendo ground
y las capas sin metadatos conservan el fallback previo; la fundación virtual,
catenaria del preview y la aceptación raster quedan como trabajo separado.

Corrección #326-RAIL-STATION-PREVIEW-ASSET-PARITY (2026-09-13): el ghost de
estación ferroviaria consulta primero `WorldAssets.rail` para vía y capas de
plataforma —incluidas las variantes rail, mono y maglev— y conserva `TileAtlas`
como fallback. La ruta de vidrio mantiene su máscara translúcida y el preview
sigue sin fabricar capas cuando ninguna fuente tiene el sprite. #326 continúa
abierta por fundaciones/catenaria del preview y aceptación raster.

Corrección #326-RAIL-WAYPOINT-PREVIEW-SORT-ASSET (2026-09-13): el ghost de
waypoint ferroviario publica los dos cuerpos `TILE_SEQ` como parents del
compositor global, con los mismos prismas, profundidad de columna y ordinales
`16 + layer_index` que el renderer del mapa. Los toldos CC de OpenGFX2 quedan
como hijos del cuerpo correspondiente, evitando que se separen al cruzarse con
otra pieza del mapa. La vía y las capas consultan primero `WorldAssets.rail` y
mantienen `TileAtlas` como fallback. Fundaciones inclinadas, catenaria y
aceptación raster siguen separadas; #326 continúa abierta.

Corrección #326-ROAD-WAYPOINT-PREVIEW-SORT-ASSET (2026-09-13): el ghost de
waypoint vial ya no muestra sólo una carretera plana cargada por ruta directa:
consulta primero `WorldAssets.road_flat` y `WorldAssets.road_waypoint`, con
fallback al atlas/PNG, y dibuja los dos postes vanilla del eje elegido. Cada
poste conserva sus offsets NFO y bounds `TILE_SEQ_LINE`, con los ordinales 2 y
3 del camino sin catenaria en el compositor global. Fundación nivelada,
catenaria y layouts NewGRF del preview quedan como unidades posteriores; #326
continúa abierta.

Corrección #326-ROAD-WAYPOINT-PREVIEW-TRAM-OVERLAY (2026-09-13): cuando la
tesela del waypoint vial declara un tipo de tranvía, el preview superpone ahora
el `tram_flat` vanilla con la misma orientación del waypoint y la prioridad de
capa del renderer. La selección consulta primero `WorldAssets.tram_flat`, luego
el atlas y finalmente el PNG directo; los grupos/overlays NewGRF y la
aceptación raster quedan fuera de esta unidad. #326 continúa abierta.

Corrección #326-ROAD-WAYPOINT-PREVIEW-FOUNDATION (2026-09-13): el ghost de
waypoint vial aplica ahora la misma `FOUNDATION_LEVELED` que el renderer cuando
la tesela está inclinada. Materializa los sprites clásicos y los slots Action5
disponibles, conserva sus bounds `SpriteBounds` como parents del compositor y
adjunta suelo, tranvía y postes como children; una pendiente empinada conserva
la elevación de dos niveles. La catenaria, layouts NewGRF, clipping y
aceptación raster siguen siendo unidades separadas; #326 continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-WATER-GROUND (2026-09-13): el ghost de
depósito naval pinta ahora el `SPR_FLAT_WATER_TILE` de cada una de sus dos
teselas antes de las capas BUILD, con el mismo desplazamiento horizontal
`xrel=-31` y la altura de la tesela. El fallback directo conserva la preview
cuando el atlas no está disponible; diques de canal, pendientes/river edges,
features NewGRF y aceptación raster quedan como subetapas separadas; #326
continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-CANAL-DIKES (2026-09-13): el ghost de
depósito naval sobre canal dibuja ahora los bordes vanilla seleccionados por
`DrawWaterEdges(true, 0, tile)`, con anclas NFO, prioridad local y conectividad
de vecinos compartida con el renderer. El selector evita duplicar el borde
interno cuando las dos mitades ya están materializadas; callbacks/features
NewGRF de canales, ríos, pendientes y aceptación raster siguen fuera de esta
unidad; #326 continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-RIVER-SLOPE (2026-09-13): el ghost de
depósito naval sobre río selecciona ahora los cuatro sprites vanilla
`water_river_slope_*` para las pendientes diagonales admitidas por
`DrawRiverWater`, conservando sus anclas y tamaños NFO; los casos no
diagonales mantienen el agua plana. River edges, offsets por features/Action5,
callbacks y aceptación raster quedan fuera de esta unidad; #326 continúa
abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-CANAL-ACTION5 (2026-09-13): los diques del
ghost naval consumen ahora el bloque Action5 `Canals` desde el slot 52, usando
el tamaño/ancla del sprite reemplazado y la misma caché que el mapa; cada slot
ausente conserva el fallback vanilla OpenGFX y todos entran en la profundidad
del pase `DrawGroundSprite`. Features Action1/3, callbacks, river edges y
aceptación raster siguen fuera de esta unidad; #326 continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-RIVER-ACTION5 (2026-09-13): las pendientes
fluviales diagonales del ghost consumen ahora los slots 0..3 del bloque Action5
`Canals`, con sus anclas/dimensiones reemplazadas y fallback individual a
`water_river_slope_*`; el agua plana continúa usando `SPR_FLAT_WATER_TILE`.
Features Action1/3, callbacks, river edges y aceptación raster siguen fuera de
esta unidad; #326 continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-CANAL-FEATURES (2026-09-13): el ghost
naval resuelve ahora las vistas Action1/3 de `CF_RIVER_SLOPE` y `CF_DIKES` con
el contexto real de la tesela —random, conectividad, clima y línea de nieve—,
conservando el ancla de cada `DecodedSprite` y el fallback individual a
Action5/OpenGFX; también conserva el callback de desplazamiento de sprite de
canales. Callbacks de aceptación, river edges y captura raster siguen fuera de
esta unidad; #326 continúa abierta.

Corrección #326-SHIP-DEPOT-PREVIEW-RIVER-EDGES (2026-09-13): el ghost naval
materializa ahora los slots conectados de `CF_RIVER_EDGE`, aplicando el bloque
de 12 sprites correspondiente a la pendiente y el mismo contexto Action2 que
el renderer. Cuando la partida no publica ese feature no se dibuja un borde
inventado, igual que `DrawWaterEdges(false, ...)`; aceptación raster y otros
callbacks permanecen fuera de esta unidad; #326 continúa abierta.

Corrección #326-BRIDGE-PREVIEW-RAIL-CATENARY (2026-09-13): la preview de
construcción de puentes ferroviarios eléctricos reutiliza `collect_catenary_
bridge_draws`, por lo que los vanos alternan los wires corto/largo y colocan
los postes PPP con la misma paridad, grupo de tesela y anclas NFO que el
renderer del mapa. La caché Action5 activa tiene prioridad sobre OpenGFX y
los aliases virtuales (`rail_pylon_*`, `rail_catenary_entrance_*`) se resuelven
al atlas correcto. Las rampas ferroviarias —que consultan fundación, pendiente
y vecinos— quedan separadas para la siguiente etapa; #326 continúa abierta.

Corrección #326-BRIDGE-RAMP-CATENARY-CONTRACT (2026-09-13): el renderer de
rampas ferroviarias ya no vuelve a inferir la pendiente desde la tesela
`RailBridge` cruda después de aplicar `DrawFoundation`. El recolector común
recibe la pendiente efectiva, conserva la máscara de cables y el estado PCP
vecino, y permite reservar el PCP interior que emite el vano. La misma unidad
queda disponible para la preview, que todavía debe conectar su fundación
virtual y sus dos extremos; #326 continúa abierta.

Corrección #326-MAIN-MENU-SHOWCASE-READABILITY (2026-09-13): la escena
determinista de `Kale_TitleGame` ya materializa doce actores de transporte,
incluidos tren, maglev, bus, barco, avión y camiones; el panel raíz ya no la
oculta con una superficie casi opaca de `520 px`. El backdrop baja a alfa
`0.28`, el panel a `0.86` y su ancho a `440 px`; los botones siguen siendo
opacos para conservar contraste y accesibilidad. Esto mejora la composición
visible del escaparate sin alterar el mapa, las rutas ni la simulación; #326
continúa abierta.

Corrección #326-BRIDGE-PREVIEW-RAIL-RAMP-CATENARY (2026-09-13): la preview
ferroviaria eléctrica materializa ahora catenaria también en las dos rampas.
Cada extremo deriva su dirección desde el orden canónico del puente, reutiliza
la decisión de fundación y el delta Z del renderer, reserva el PCP interior
para el vano y conserva aliases Action5/OpenGFX, pendientes y transparencia.
La cota del tablero fantasma usa la misma superficie efectiva de la fundación
en los extremos; las fundaciones visuales completas y las rampas de otros
transportes siguen siendo trabajo separado. #326 continúa abierta.

Corrección #326-BRIDGE-PREVIEW-RAMP-HEAD (2026-09-13): los extremos de la
preview ya seleccionan `bridge_ramp_sprite_id` con la dirección persistida y
la superficie posterior a la fundación, en lugar de reutilizar el sprite
trasero del vano. Esto conserva las variantes vanilla de rail, eléctrico,
monorriel, maglev y carretera tanto en los ejes X/Y como al invertir el drag;
las capas de fundación visibles y la aceptación raster siguen abiertas. #326
continúa abierta.

Corrección #326-BRIDGE-PREVIEW-RAMP-FOUNDATION (2026-09-13): los extremos de
la preview ahora recorren el mismo `foundation_draw_plan` que el mapa, con el
bloque `HasFoundationNW/NE`, el `z_delta` y el origen de `SpriteBounds`
remapeados mediante el anclaje NFO real. Los 14 sprites clásicos se cargan de
OpenGFX y las fundaciones virtuales usan la tabla Action5 vigente del save,
incluidos los reemplazos NewGRF; el suelo efectivo de la rampa queda para la
siguiente etapa. #326 continúa abierta.

Corrección #326-BRIDGE-PREVIEW-RAMP-GROUND (2026-09-13): la preview de cada
cabeza de puente ya dibuja el mismo suelo efectivo que `DrawTile_TunnelBridge`:
césped con pendiente, costa cuando la rampa nivelada toca mar, o nieve/desierto
cuando el bit de paisaje lo exige. La dirección virtual se instala sólo en una
copia del tile para que la preview no mutile el mapa; la posición usa la cota y
la altura visual resultantes de la fundación. #326 continúa abierta por
sprites custom restantes y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-PILLARS (2026-09-13): la preview de los vanos
intermedios ya dibuja los pilares de madera en ambos lados del puente. Reutiliza
las alturas de borde, los segmentos completos/medios y los recortes de
`SubSprite` del renderer materializado; para tiles que todavía no son puente
calcula la pendiente/superficie virtual que recibirán al confirmar la obra.
La capa trasera conserva su desplazamiento y orden de sorting, y el eje X/Y usa
los PNG vanilla específicos. Quedan fuera de esta unidad la supresión por
road-stop, los pilares custom por tipo de puente y la aceptación raster; #326
continúa abierta.

Corrección #326-BRIDGE-PREVIEW-PILLAR-BLOCKS (2026-09-13): la preview consulta
ahora `Station`, `RoadStopSpecDef` y `BridgeSpecDef` del estado real antes de
dibujar pilares bajo un vano. Reutiliza la máscara vanilla o custom de
`bridgeable_info.disallowed_pillars`, junto con la pieza y el eje efectivos;
una parada de bus/camión ya no muestra apoyos que desaparecerán al confirmar la
obra. El alcance no altera la validación de construcción ni cubre aún tipos de
puente custom del selector; #326 continúa abierta.

Corrección #326-BRIDGE-PREVIEW-TYPE (2026-09-13): la preview de construcción
usa ahora el último `BridgeType` elegido para carretera o ferrocarril, en lugar
de fijarse siempre en madera. Los ids de tablero, cabeza y pilar siguen la
tabla de sprites del tipo seleccionado y la máscara de pilares recibe la misma
especie de puente; el valor por defecto continúa siendo madera antes de la
primera elección. La validación previa sigue usando madera como sonda porque la
ventana aún permite escoger el tipo después de terminar el arrastre; #326
continúa abierta por paletas de recolor, callbacks custom y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-PALETTE (2026-09-13): las piezas de tablero,
cabeza y pilar de la preview consultan ahora `WorldAssets.bridge_palettes`,
aplicando la misma tabla `PALETTE_TO_STRUCT_*` que el renderer materializado
para puentes rojos, amarillos, marrones, blancos y concretos. El recurso es
opcional para conservar el fallback PNG en escenas de arranque o tests aislados;
cuando está disponible no se duplica el recolor ni se altera la textura original.
#326 continúa abierta por sprites/callbacks NewGRF custom y aceptación raster.

Corrección #326-BRIDGE-PREVIEW-ACTION5-DECK (2026-09-13): los vanos de la
preview consultan ahora los 24 slots Action5 `0x1B` de tableros de puente con
la misma combinación transporte/eje que el renderer materializado. El sprite
NewGRF se compone sólo cuando no existe una superficie `ROTSG_BRIDGE` de
roadtype, respetando la prioridad del contrato y conservando el fallback
vanilla; las cabezas siguen usando sus Action5/atlas específicos. #326 continúa
abierta por callbacks y sprites estructurales NewGRF custom restantes, además
de la aceptación raster.

Corrección #326-BRIDGE-ACTION0-SPRITE-TABLE-DATA (2026-09-13): Action0
`Bridges` ya conserva las siete tablas parciales de 32 entradas, los pares
`(SpriteID, PaletteID)` y los tres modificadores de `MapSpriteMappingRecolour`;
los overrides se aplican a todos los IDs consecutivos declarados por
`num_ids`. La etapa todavía no dibuja esos sprites en Bevy: la resolución de
IDs globales del sprite section/Action1 y su geometría quedan separadas para no
confundir referencias con píxeles. #326 permanece abierta.

Corrección #326-GLOBAL-SPRITE-INDEX (2026-09-13): se añadió un índice efímero
que asigna los `SpriteID` globales de `OpenTTD` a las imágenes reales cargadas
por Action1 y Action12, incluyendo imports v2 `0xFD` y el cursor
`NEWGRF_SPRITE_BASE`. Esto habilita resolver las referencias absolutas de
Action0 sin confundir un índice de set local con un sprite decodificado; la
aplicación del índice al catálogo/renderer de puentes queda como la siguiente
etapa. #326 permanece abierta.

Corrección #326-GLOBAL-SPRITE-MATERIALIZATION (2026-09-13): el apply de
`Bridges` encadena ahora el cursor global entre todos los GRF activos y
materializa cada tabla Action0 `0x0D` en una tabla runtime paralela de
`DecodedSprite`. La referencia cruda permanece para sprites directos del
baseset y los huecos sin resolución quedan explícitos; también se conserva el
último override por pieza y por ID consecutivo. La regresión combina Action0
`Bridges` con Action1 y comprueba los offsets/píxeles resueltos. Falta conectar
esta tabla al draw de Bevy, incluyendo offsets nativos, paletas/modificadores,
recortes de pilares y fallback trazable; #326 permanece abierta.

Corrección #326-GLOBAL-SPRITE-DRAW (2026-09-13): el renderer de Bevy consulta
la tabla materializada para cabezas/rampas y para las tres capas de cada vano,
respetando los offsets nativos de dirección, pendiente, transporte y eje. Las
imágenes Action1 conservan sus offsets y dimensiones reales, se aplican las
paletas directas de compañía/estructura/2CC/crash/transparencia disponibles y
los pilares custom reutilizan los recortes de media columna y el sorting 3D
existente. Las referencias directas al baseset usan el atlas/cache actual; los
ids sin imagen quedan como fallback trazable y no reactivan la capa vanilla.
La cobertura está validada con regresiones de índices, materialización y
paleta directa; #326 permanece abierta por paletas Action5 específicas,
callbacks/layouts restantes y aceptación raster sobre saves reales.

Corrección #326-BRIDGE-CUSTOM-2CC-MAP (2026-09-13): las referencias
`BridgeSpriteRef` con paleta 2CC ya consultan el mapa Action5 `0x0A` vigente
del runtime antes de crear la textura RGBA. El mismo vector de 256 mapas se
comparte con estaciones, industrias, casas y layouts; si el slot no existe se
mantiene el remapeo estándar y la entrada continúa siendo renderizable. La
regresión compara el resultado de puente con `bake_sprite_two_company_palette`
usando un mapa que altera ambos colores; #326 permanece abierta por los
callbacks/layouts restantes y aceptación raster sobre saves reales.

Corrección #326-BRIDGE-PREVIEW-CUSTOM-SPRITES (2026-09-13): la preview de
construcción consulta ahora las mismas siete tablas Action0 `Bridges` que el
renderer del mapa para rampas, vanos y pilares. Respeta dirección, pendiente,
eje, transporte y los offsets reales de `DecodedSprite`; los pilares custom
reutilizan los recortes de media columna y las entradas cero mantienen la
supresión explícita del fallback vanilla. Las imágenes dinámicas aplican el
mapa Action5 `0x0A` de 2CC y las referencias directas reutilizan el atlas/cache
de estructura. #326 permanece abierta por callbacks/layouts restantes y
aceptación raster sobre saves reales.

Corrección #326-ROAD-WAYPOINT-PREVIEW-CATENARY (2026-09-13): el ghost de
waypoint vial electrificado publica ahora los tres recortes traseros y el
frente de `DrawRoadTypeCatenary`, con las filas X/Y y anclas NFO de
`SPR_TRAMWAY_BASE`, sobre la superficie nivelada de la preview. La limpieza
usa el mismo marcador `BuildGhostPreview` que el resto de la herramienta y
también contempla cuando la tesela conserva road y tranvía electrificados.
Los grupos específicos de catenaria de RoadType y la aceptación raster del
preview quedan como subetapas separadas; #326 continúa abierta.

Corrección #326-ROAD-WAYPOINT-PREVIEW-CUSTOM-CATENARY (2026-09-13): el
preview de waypoint reutiliza ahora la caché y el evaluador Action2 de
RoadTypes para `ROTSG_CATENARY_BACK` y `ROTSG_CATENARY_FRONT`, incluyendo la
vista plana del eje y la política nativa de suprimir sólo el fallback que
corresponde cuando un grupo custom sí resuelve. Sus anclas y offsets se
aplican a los mismos tres recortes y frente; la aceptación raster amplia
queda pendiente y #326 continúa abierta.

Corrección #326-BRIDGE-FRONT-COMBINE-PARENT (2026-09-13): el bloque frontal
de `DrawBridgeRoadBits` ya conserva su parent real. Cuando la catenaria vial
vanilla o NewGRF resuelve la primera imagen del `StartSpriteCombine`, esa
imagen entra como `ViewportSortableParent` con la caja frontal nativa y la
baranda frontal pasa a `ViewportSortableChild`; si el cable no está disponible,
la baranda mantiene el fallback como parent visible. La regresión ECS cubre
vanilla y NewGRF, ambos ejes de caja y la relación parent/child. La aceptación
raster y los layouts/callbacks NewGRF que todavía no publican todos sus
children siguen pendientes; #326 continúa abierta.

Corrección #326-NEWGRF-OBJECT-FOUNDATIONS (2026-09-13): los objetos NewGRF
inclinados consultan ahora `ObjectFlag::HasNoFoundation`, aplican
`FlatteningFoundation` cuando corresponde y usan la cota nivelada para el
ground, las entradas `BUILD` y las vistas de fallback. El ground se vincula al
último parent de la fundación cuando OpenTTD lo deja activo; con
`HasNoFoundation` conserva la pendiente y no crea ese parent. La regresión ECS
cubre `SLOPE_W` en ambas variantes.

Corrección #326-NEWGRF-OBJECT-ACTION5-FOUNDATIONS (2026-09-13): el draw
genérico de objetos NewGRF ya consume la tabla Action5 de fundaciones activa y
la caché compartida del runtime. Las fundaciones niveladas virtuales se
materializan con el slot, offsets y textura custom del save; un slot ausente no
se disfraza como un sprite válido y queda registrado como fallback. La
regresión ECS usa el bloque 3 (`sprite_id` 5471, slot 58) y verifica la imagen
en el parent sortable. Siguen abiertas las callbacks/layouts/children
dinámicos completos y la aceptación raster sobre saves reales; #326 continúa
abierta.

Corrección #326-NEWGRF-OBJECT-ORPHAN-CHILD-GROUND (2026-09-13): un child de
`TileLayout` sin parent visible previo se dibuja ahora con el ground pass de
`DrawCommonTileSeq`, manteniendo sus offsets screen-space pero sin introducir
una profundidad sortable ajena a la tesela. La regresión ECS verifica la
profundidad contra `ground_draw_z`; los children que siguen a un parent
conservan `ViewportSortableChild` y el orden global. #326 continúa abierta por
callbacks/layouts/children dinámicos completos y aceptación raster real.

Corrección #326-CLEAR-GROUND-DENSITY (2026-09-13): se eliminó la reinterpretación
de `m5 == 0` como césped pleno en la ruta de `MP_CLEAR`. La generación de mundo
escribe su densidad inicial y los contextos sintéticos sin tile usan un default
visual explícito. Los SAV importados pasan por el valor nativo y una traza
completa de `mvp_openttd_ship.sav` volvió a
seleccionar `3924` en `(2,2)`, igual que OpenTTD, en lugar de `3981`. La
regresión cubre el selector de densidad cero y conserva el default visual de
preview; #326 continúa abierta por las capas restantes y la aceptación raster
completa.

Evidencia #267/#326-SHIP-DEPOT-REAL-FIXTURE (2026-09-13): `mvp_ship_state`
materializa un depósito naval con `PlaceShipDepotDir` antes de exportar el SAV.
La carga dedicada de OpenTTD pasó; la comparación completa confirmó la familia
`ship-depot` (3 capas) y `ship-depot-water` (2 fondos) con IDs, geometría,
paleta y orden relativos equivalentes. La cobertura naval queda respaldada por
una partida real; #326 permanece abierta por las familias no cubiertas y el
gate raster completo.

Corrección #326-RAIL-FIXTURE-VALID-TYPE (2026-09-13): el helper `tiny_state`
puede conservar bytes arbitrarios para pruebas de persistencia, pero no debe
ser la fuente directa de un SAV visual. `mvp_stations_state` normaliza ahora
los bits bajos de `m8` de su tesela Rail a `RailType::Rail`; OpenTTD deja de
resolver esa tesela como tipo `0x34`. Tras regenerar las fixtures derivadas,
`mvp_openttd_ship.sav` compara 4104/4104 selecciones y 4104/4104 órdenes
relativas, con `rail-track` y `ship-depot` alineados. Esto cierra la falsa
divergencia de fixture, no el issue padre #326.

Corrección #326-CLEAN-VIEWPORT-SORT (2026-09-13): las capturas limpias ahora
excluyen los parents de cuerpos/unidades de vehículos antes de asignar slots
de profundidad, igual que `ViewportDoDraw` cuando OpenTTD omite
`ViewportAddVehicles`. La captura WGPU de `Kale_TitleGame.sav` en `(189,126)`
pasó de `97.102` a `94.775` píxeles distintos sobre `921.600`
(`10,536241319 %` → `10,283745660 %`), con `169 → 0` identificadores
`0xFFFE0000` en el trace y delta medio `3,746445 → 3,610982`. El modo normal
no cambia. Esto reduce una fuente concreta de desplazamiento de profundidad,
pero #326 sigue abierta por los producers restantes, segmentación, clipping,
pivotes y raster global.

Corrección #326-CLEAN-FULL-ANIMATION (2026-09-13): el perfil temporal de
`OPENTTDRS_MAP_SHOT_CLEAN=1` fuerza `full_animation=false`, alineado con
`PrepareCleanWorldScreenshot` y `DO_FULL_ANIMATION` de OpenTTD. La regresión
arranca con la preferencia en `true`, comprueba que se congele durante la
captura y que el guard restaure el valor original; no se modifica la
preferencia persistida. #326 continúa abierta.

Evidencia #326-VIEWPORT-BANDS (2026-09-13): el trace de captura limpia de
`Kale_TitleGame.sav` (`(189,126)`, `1280×720`, `Normal`) muestra 15 llamadas
de `ViewportDoDraw` en OpenTTD: 14 bandas de 204 píxeles virtuales y una de
24, con 2374 parents post-sort acumulados. `openttdrs` todavía ordena una
única lista global de 1572 parents. El `world-draw` estructural permanece
contenido (`157142/157142`), mientras la referencia promueve en bandas
distintas el primer sprite visible de 7 secuencias combinadas y mantiene 4
cajas que no aparecen en la pasada global. El siguiente trabajo queda
acotado a clipping/composición segmentada; el depósito naval real y los
assets de sentido único no presentan una divergencia reproducible adicional.
La métrica raster actual es `94775/921600` (`10,283745660 %`) y #326 no se
cierra.

Corrección #326-RAIL-FENCE-BOUNDS-ORIGIN (2026-09-13): las cercas de vía
aplican ahora el `bounds.origin` de `DrawTrackFence` también a la posición
visual del PNG, igual que `AddSortableSpriteToDraw` antes de ejecutar
`RemapCoords`. Esto alinea las vallas SE/NW y las verticales con sus prismas
sortables; la regresión cubre los offsets `(0,15,0)` y `(8,8,0)`. En la
captura limpia de `Kale_TitleGame.sav` (`189,126`, `1280×720`, `Normal`) el
raster pasó de `94775/921600` a `77258/921600` píxeles distintos
(`10,283745660 %` → `8,383029514 %`) y el delta medio de canal bajó de
`3,610982` a `2,814533`. El trace dejó la caja de la valla presente y no
introdujo bounds candidatos fuera de la referencia; #326 continúa abierta.

Corrección #326-COMBINED-CLIPPING-PROMOTION (2026-09-13): los primeros
`AddCombinedSprite` que quedan fuera del recorte ya publican metadatos de
sprite, bounds, clave de inserción y ordinal de combinación. El sorter puede
promover el primer child visible a parent sin romper el vínculo ECS del resto
del bloque; la regresión conserva la caja `(2720,1968,12)-(2735,1983,59)` y
el orden de las capas. En Kale la cobertura global pasó de 1571 a 1572
parents y la caja del árbol `1649` dejó de faltar; el raster se mantuvo en
`77258/921600` porque la capa ya se rasterizaba con su profundidad histórica.
Las dos bocas ferroviarias que aparecen en bandas distintas siguen pendientes
de un compositor segmentado; no se cierra #326.

Corrección #326-SEGMENTED-TUNNEL-COMBINE (2026-09-14): el frente de túnel
ferroviario marca sus children combinados para separar el slot cuando el PNG
cruza una frontera de banda nativa que la catenaria no alcanza. El parent ECS
original conserva los demás children y el frente sólo se excluye de su ventana
cuando el sorter le asigna el slot independiente. En Kale el trace pasó de
`1572` a `1574` parents y de `2` a `0` cajas de referencia ausentes, incluyendo
las bocas `2370` y `2366`; la captura no cambió (`77258/921600`, delta medio
`2,814533`). #326 sigue abierta por el compositor segmentado general y las
familias de combines que aún no publican este contrato.

Corrección #326-SEGMENTED-BAND-SELECTOR (2026-09-14, `4867aaba`): el sorter
calcula ahora las bandas visibles que cada child combinado aporta por fuera
del parent original y elige el primer child por banda, permitiendo varias
promociones del mismo bloque cuando son necesarias. La selección se deduplica
de forma determinista y las bandas que sólo quedan fuera del viewport no
generan parents espurios. La captura Kale con el producer de túnel vigente
conserva `1574` parents y exactamente el mismo raster (`77258/921600`, delta
medio `2,814533`); la suite del cliente pasa `1532` tests, con 2 ignorados,
y Clippy queda verde. Al probar árboles, la traza alcanzó `1581` identidades
exactas, pero promover un `Sprite` Bevy completo alteró `1037` píxeles que
antes coincidían; se retiró ese producer hasta implementar clipping por banda
real. #326 continúa abierta.

Corrección #331-NEWS-AVAILABILITY-OPENING (2026-09-14, `1cab5140`): el cliente
localiza también las noticias generadas de disponibilidad de vehículos
(`Nuevo`, preview exclusiva y cuerpo con motor/ID) y la apertura de una nueva
industria, conservando literalmente nombres, IDs y coordenadas. Las plantillas
con fragmentos ambiguos o malformados no se traducen para no tocar datos de la
partida. La regresión cubre ambos locales, entidades creadas después del cambio
de idioma y el fallback seguro; #331 permanece abierta por catálogos upstream,
settings no modelados y las demás noticias generadas.

Corrección #331-NEWS-INDUSTRY-OPENING-LABEL (2026-09-14, `12cdb730`): la
categoría `Apertura de industria` de la ventana de preferencias de noticias
ya se registra en el catálogo inglés como `Industry opening`, manteniendo el
enum y la preferencia persistida sin cambios. La regresión del catálogo cubre
las diez categorías disponibles; #331 continúa abierta por los catálogos
upstream, settings no modelados y superficies pendientes.

Corrección #331-CARGO-UI-LOCALE (2026-09-14, `569ee55c`): el panel de estación
localiza ahora el nombre de cada carga vanilla en sus resúmenes dinámicos, y la
ventana de tarifas de carga localiza título, año, encabezados, cargos y nota
explicativa sin modificar importes ni reglas de pago. La regresión cubre ambos
locales en la fila de estación y en las 32 entradas vanilla de tarifas; la
suite completa del cliente queda en `1532` tests exitosos y `2` ignorados.
#331 continúa abierta por catálogos upstream, settings no modelados, cargos
custom/NewGRF y otras superficies generadas.

Corrección #331-SUBSIDY-ROW-LOCALE (2026-09-14, `2ed4ddab`): la lista de
subvenciones localiza el cargo y el nombre de la industria vanilla en las filas
dinámicas cuando el locale es inglés, preservando nombres custom/NewGRF,
compañías, coordenadas, IDs, importes y vencimientos. La regresión ejercita una
industria materializada y el fallback de estación en ambos locales; #331 sigue
abierta por catálogos upstream y superficies generadas pendientes.

Corrección #330-ROAD-TRACE-V2 (2026-09-14, `27e568dc`): la traza de paridad
expone una proyección `road` opcional para las cabezas viales y el normalizador
la publica como `road_vehicles` del contrato v2, conservando v1 para trazas
ferroviarias. Se validan los campos nativos de estado, frame, bloqueos,
adelantamiento, choque y reversa; una regresión de `scripts/test_pbs_trace_tools.py`
compara una muestra v2 sin IDs de pool. `parity_runner` produce ocho ticks
viales v2 y `sav_pbs_runner` valida cuatro ticks de `mvp_openttd_rich.sav`.
La evidencia externa queda documentada en `#330-ROAD-ORACLE`.

Corrección #330-ROAD-ORACLE (2026-09-14, `d4dcb249`): el exportador nativo y
`sav_pbs_runner` se ejecutaron sobre el mismo `mvp_openttd_rich.sav`; el
comparador `--scope road` pasa las cinco muestras. Se corrigieron el early
return de `RoadVehFindCloseTo` durante `reverse_ctr` —que conserva
`blocked_ctr`— y la reescritura indebida de `road_state` desde la dirección.
Las regresiones focales y la suite vial pasan. #330 continúa abierta por la
divergencia ferroviaria/PBS global y por tráfico complejo, presignals, aire y
mar.

Corrección #330-TRAIN-PBS-ORACLE (2026-09-14, `71f445c1`): el mismo replay de
`mvp_openttd_rich.sav` coincide en las cinco muestras iniciales de tren,
carretera y PBS. Rust conserva la aceleración nativa de un tren sin
`movement_target`, mantiene el overlay PBS importado de `MAP2` durante la
primera sincronización y evita convertir el footprint físico de un tren sin
ruta en una reserva PBS. Pasaron 31 tests focalizados, 2735 tests del core (1
ignorado), `test_pbs_trace_tools.py` y la comparación externa. El alcance es
sólo la ventana inicial del escenario: #330 permanece abierta por rutas de
tren multi-tick, tráfico complejo, presignals, aire y mar.

Corrección #330-TRAIN-LINE-END-PROGRESS (2026-09-14, `fcecb8fb`): el
controlador ferroviario conserva el remanente entre los dos
`TrainLocoHandler`, intenta el borde cuando `j` alcanza
`GetAdvanceDistance` y replica la inversión de fin de vía sin consumir el
marcador `progress=255` de una estación. `mvp_openttd_rich.sav` coincide en 41
muestras PBS, incluyendo el tick 12374, y la suite core pasa 2735 tests (1
ignorado). La resolución de rutas ferroviarias multi-tick y los escenarios de
tráfico/presignals continúan pendientes en #330.

Corrección #330-ROAD-SLIDING-DIRECTION (2026-09-14, `efb63805`): el importador
conserva `RoadVehicle::x_pos/y_pos/z_pos` y el writer usa esas coordenadas
cuando son válidas; el controlador replica `RoadVehGetSlidingDirection`
(`roadveh_cmd.cpp:751-775`) con un cambio de rumbo de 45 grados por intento y
un early return que conserva frame/posición durante el deslizamiento
(`roadveh_cmd.cpp:1447-1491`). La actualización cubre además el Z incremental
de `GroundVehicle::UpdateZPosition` y `RoadZPosAffectSpeed` a partir de los
bits de subida/bajada.

El caso diferencial usa `mvp_openttd_rich.sav`: el vehículo empieza con
`(x_pos,y_pos)=(208,264)`, posición local `(0,8)`, `state=8`, `frame=6` y
`overtaking=16`. La orden
`python3 scripts/compare_pbs_traces.py /tmp/openttd-mvp-rich-40.jsonl /tmp/openttdrs-mvp-rich-40-roadpos-final.jsonl --scope road`
devuelve `OK: dinámica vial externa sin divergencias (41 ticks)` y la suite
core pasa 2736 tests, con 1 ignorado. El bloque no cierra #330: siguen
pendientes las rutas multi-tick, tráfico complejo, presignals y aire/mar.

Corrección #330-ROAD-STATION-M5-CONNECTIVITY (2026-09-14, `53035542`): el
destino importado de una orden vial conserva la tesela ancla de la estación,
igual que `RoadVehicle::GetOrderStationLocation`, y la conectividad se deriva
de `m5` (bahía/eje drive-through), no de `m3`. El A* dejó de permitir una
entrada artificial desde cualquier vecino; cuando la estación no es alcanzable,
el controlador conserva el rumbo único de `RoadFindPathToDest` en vez de
convertir la boca de acceso en una llegada. La entrada a tesela actualiza Z,
pendiente y contador de adelantamiento con el contrato nativo. Las regresiones
cubren conectividad de pathfinder, rechazo de depósito orientado a una boca
incompatible y continuación más allá de una parada desorientada. El replay
actual de `mvp_openttd_rich.sav` coincide en **251/251 muestras** para PBS y
dinámica vial, y el workspace queda en verde (core `2744` tests, cliente
`1532` tests, 2 ignorados). #330 sigue abierto por rutas ferroviarias
multi-tick, tráfico complejo, presignals y aire/mar.

Corrección de build del cliente (2026-09-14, `4fe24b14`): el test de la lista
de subvenciones importaba `IndustrySpec` sólo desde el prelude, aunque el tipo
se exporta por la raíz del core. El import explícito restaura la compilación del
workspace sin alterar el binario ni la lógica de UI. `cargo check --workspace`
`cargo test --workspace --quiet` pasan; el cliente ejecuta `1532` tests y
 el core `2744` tests, con los ignorados documentados en sus fixtures.

Corrección #330-ROAD-BREAKDOWN-IMPORT (2026-09-14, `02225714`): el controlador
vial ejecuta `HandleBreakdown` en la misma fase que `RoadVehController`, antes
de actualizar velocidad o posición, y publica el evento de entrada de avería
también para vehículos de carretera. La carga de SAV conserva la fiabilidad
actual y `chance16i` usa sólo la palabra baja del RNG nativo; antes se
reinicializaba la fiabilidad guardada y se perdían averías reproducibles. Las
regresiones cubren la transición `breakdown_ctr=2→1`, la escala de fiabilidad
importada y el enmascarado de RNG. En el replay largo, la carretera coincide
hasta la primera divergencia independiente de tren en el tick `13065`; el
comparador entonces encuentra tren nativo a velocidad `144` frente a `96` en
Rust. #330 continúa abierta para esa divergencia ferroviaria y las rutas
multi-tick, tráfico complejo, presignals, aire y mar. El core pasa `2745`
tests con el escenario IA no relacionado omitido (`1` ignorado), el cliente
pasa `1532` (`2` ignorados) y Clippy de la librería queda verde; el escenario
`ai_rival_builds_third_oil_route` sigue fallando por timeout y no se atribuye a
esta corrección.

Corrección #330-TRAIN-BREAKDOWN-EXPIRY (2026-09-14, `4e81daea`): las dos
pasadas ferroviarias consumen `HandleBreakdown` dentro de cada
`TrainLocoHandler`, como `Train::Tick` nativo. Si la primera pasada termina
una avería (`breakdown_ctr=1→0`), la segunda puede actualizar la velocidad en
el mismo tick; Rust ya no preconsume la avería ni descarta ambas pasadas. La
regresión cubre tanto el decremento doble de contador como el vencimiento con
una sola pasada de movimiento. El replay `mvp_openttd_rich.sav` coincide en
`731` muestras de tren/PBS hasta el tick `13075`; la siguiente frontera
reproducible es vial en el tick `13072`, con tren/PBS ya alineados. #330 sigue
abierta por esa dinámica vial posterior, rutas multi-tick adicionales,
tráfico complejo, presignals, aire y mar.

Corrección #330-ROAD-TURN-MARKERS (2026-09-14, `04d44a37`): los marcadores
`RDE_NEXT_TILE` ahora transportan su `DiagDirection` al resolver la siguiente
tesela. Si la salida no tiene carretera compatible, Rust replica
`_road_reverse_table` sin abandonar la tesela; si el giro termina en una
`RDE_TURNED`, consulta los trackdirs derivados de `_road_trackbits` y comienza
en `RVC_TURN_AROUND_START_FRAME`, conservando `overtaking_ctr` como OpenTTD.
También se eliminó el corte artificial que impedía consumir otra pasada del
controlador cuando aún no había path cacheado. La regresión cubre la reversa
ante una tesela vacía. El replay largo de `mvp_openttd_rich.sav` coincide en
`2001/2001` muestras de PBS y dinámica vial; pasan los 19 tests del controlador
vial, el cliente (`1532`, 2 ignorados), formato y Clippy de la librería. La
suite completa del core mantenía entonces pendiente el fallo aislado
`vehicle::tests::timetable_wait_delays_order_advance` (`left=0`, `right=30`).

Corrección #330-ROAD-STANDALONE-ARRIVAL (2026-09-14, `13d9912e`): una orden
vial que ya está sobre una estación sin `movement_target` entra por la misma
ruta de llegada que una estación alcanzada durante el movimiento. El controlador
ya no queda detenido esperando una transición imposible de pathfinding; la
regresión cubre la llegada standalone y la suite del core vuelve a quedar
verde. #330 permanece abierta por los contratos de rutas multi-tick, tráfico
complejo, presignals y los oráculos de aire y mar.

Corrección #330-TRAIN-LINE-END-PIXEL (2026-09-14, `cdb2ca4c`): el fin de vía
físico usa el complemento de coordenada de 16 píxeles (`12 → 4` y el borde
`15 → 1`), mientras que la reversa provocada por una señal unidireccional
conserva el caso nativo de borde `15 → 0`. Separar ambas rutas evita una
regresión en `train_pbs_15_3` y reproduce la inversión del replay rico en el
tick `12465`. La regresión quedó incorporada al oráculo PBS con su estado
completo (`pos`, `progress`, velocidad, dirección y `rail_pixel`). La
comparación externa de `mvp_openttd_rich.sav` queda en `2001/2001` muestras
para PBS y dinámica vial; pasan el oráculo focalizado (5 tests), el core
(`2752` tests, 1 ignorado), Clippy de la librería, formato y el gate de
documentación. #330 sigue abierta: esta evidencia cubre una ventana concreta,
no la resolución global de rutas multi-tick, tráfico complejo, presignals,
aire y mar.

Corrección #567-SHIP-SUBTILE-POSITION (2026-09-14, `d4443076`): el writer
`VEHS.common` conserva `Ship::x_pos/y_pos` cuando el barco tiene una posición
subtesela válida, del mismo modo que ya hacía para los vehículos de carretera.
Antes un barco en tránsito o en la boca de un depósito volvía a guardarse en
la posición derivada del centro de su tesela y podía saltar al reabrir el SAV.
La regresión `vehs_preserves_ship_subtile_position` verifica ambos ejes y
mantiene el fallback centrado para saves/vehículos sin posición naval
materializada. El core pasa `2753` tests, con 1 ignorado. #567 continúa abierta
por el oráculo dinámico externo, callbacks y la aceptación visual completa del
depósito naval.

Corrección #326/#567-SHIP-DEPOT-BUOY-VISUAL (2026-09-14, `bdeefc14`): el
extractor de OpenGFX ya toma `SPR_IMG_BUOY` (693), que es el sprite visible de
la boya, en vez del slot base 4076, que es intencionalmente vacío de 1×1. El
renderer conserva la caja `TILE_SEQ_LINE` de la boya, aplica el ancla NFO
`yrel=2` con el offset raster `+2,5` y separa las boyas vanilla de las
reemplazadas por Canal Feature. Los índices globales 239/240 se hornean en
cuatro frames RGBA y un sistema reproduce su ciclo de paleta con el contador
`+8` cada 30 ms; el recurso también participa del teardown de sesión. En el
save real `mvp_openttd_ship.sav`, centrado en `32,32` a 512², la geometría del
depósito y la boya queda alineada sin traslación y la captura pasa de 127
divergencias a un único píxel (`3,814697265625e-6`), que corresponde al estado
temporal rojo 20/128 de la referencia congelada frente al perfil `CLEAN` que
desactiva animaciones. Pasan los `1534` tests del cliente (2 ignorados), el
formato y Clippy del binario. Esta evidencia no cierra #326/#567: quedan la
aceptación raster en las demás orientaciones/escenarios, callbacks y el resto
de la semántica naval.

Corrección #326/#567-ROADSTOP-ACTION5-SOURCE (2026-09-14, `f41abd74`): el
generador de paradas drive-through trata `05 11 FF 08` como encabezado Action5
y consume sus ocho filas físicas siguientes (`2020..2027`). La implementación
anterior miraba hacia atrás y terminaba usando otro bloque de sprites, por eso
las paradas aparecían marrones en vez de azul/blanco. Se regeneraron recortes,
metadata NFO y atlas; la regresión cubre los perfiles 8bpp y 32bpp. En el Kale
real de 1280×720, la comparación pasa de `77.258` a `72.446` píxeles distintos
(−4.812; −6,23 % del residuo), sin traslación global. Pasan `1534` tests del
cliente, Clippy del binario, formato y el check del atlas. #326/#567 quedan
abiertos por la composición raster global y las demás familias/semánticas.

Corrección #326-SEGMENTED-BAND-PROXY (2026-09-15, `cdde6082`): los hijos
combinados de árboles y frentes de túnel conservan ahora una instantánea de su
sprite y transform originales. El hijo original se recorta al tramo de bandas
que realmente alcanza a su parent; cada tramo adicional se publica como proxy
local y se ordena junto con los parents normales de esa banda. Los proxies ya no
consumen posiciones del ordenamiento global, evitando desplazar el resto del
mapa cuando una familia combinada necesita clipping preciso. La cobertura de
tests incluye la interpolación de profundidad entre slots vecinos y el contrato
de spawn de las copas de árbol.

En el Kale real (`Kale_TitleGame.sav`, centro `189,126`, `1280×720`, escala
Normal, perfil `clean-static`), la captura fresca del commit publicado cambia
`77.396/921.600` píxeles (`8,3980 %`) frente a `72.513/921.600`
(`7,8682 %`) con esta corrección; el delta medio baja de `2,8565` a `2,5157`.
Las trazas JSON siguen contando sólo los `1.572` parents del orden global: los
`7` proxies de banda se ordenan deliberadamente en sus listas locales y se
validan principalmente por la evidencia raster. La primera ejecución de
`In2x`/`Out2x` usó un socket Weston stale y no terminó; esa limitación de
infraestructura no se toma como evidencia de rendimiento. Pasan `1538` tests
del cliente (2 ignorados), Clippy con `-D warnings`, formato, documentación de
paridad y check de atlas. #326 y #567 siguen abiertos por las demás familias de
composición, escalas y la aceptación naval completa.

Validación #326-SCALE-MATRIX (2026-09-15): con sockets Weston aislados, la
misma matriz sí produjo ambos framebuffers. `In2x` (escala ortográfica `0,5`)
queda en `86.865/921.600` píxeles distintos (`9,4255 %`) y su traza candidata
contiene `442` parents frente a `798` nativos; `Out2x` (escala `2`) queda en
`759.410/921.600` (`82,4012 %`) y `13.353` parents frente a `6.004` nativos.
En `Out2x` el sorter aún usa deliberadamente el AABB histórico porque el
primer pase diagonal sin optimizar llegó a más de `35.000` parents y no
terminó dentro de 180 s. La diferencia deja acotada la siguiente subetapa:
preservar el primer remap de la cámara de captura y reducir el conjunto
diagonal antes de habilitar precisión en `Out2x`; no se declara paridad de
escala ni se cierra #326.

Validación #326-ROADSTOP-FOCUS (2026-09-15): la región Kale
`(225,2)..(226,2)` contiene una pareja de paradas viales vanilla. El
comparador `world-draw` encuentra `2` teselas y `6/6` draws candidatos
contenidos: los cuatro layers `5978..5983` conservan ID, caja explícita,
paleta y orden relativo, y los dos grounds quedan en su pase correspondiente.
Una captura raster más amplia, centrada en `(120,9)` a `800×600` Normal,
queda en `8.987/480.000` píxeles distintos (`1,8723 %`) sin traslación; el
residuo se reparte entre edificios, árboles y bordes de la ciudad, por lo que
no se atribuye a la parada sin una captura aislada. No aparece una sub-brecha
semántica de road-stop en Kale; #326 continúa abierta por composición raster,
familias restantes, escalas y aceptación naval.

Corrección #326/#567-SHIP-DEPOT-CAPTURE-HARNESS (2026-09-15): el driver de
capturas de mapa acepta ahora `MAP_SHOT_TOOL=ship_depot` y selecciona el grupo
Water correcto, además de `MAP_SHOT_ORIENTATION=0..3` para repetir el ghost
naval en las cuatro huellas. El mismo arnés reconoce también los previews de
waypoint vial/ferroviario y parada vial, con sus grupos de toolbar, y conserva
el comportamiento anterior de las capturas ferroviarias. Las regresiones
cubren nombres case-insensitive, orientación fuera de rango y herramientas
desconocidas. Esta unidad sólo hace reproducible la aceptación visual; no
declara paridad raster ni cierra #326/#567.

Validación #326-SORT-LOCAL-PROXY-TRACE (2026-09-15): la traza opcional
`OPENTTDRS_VIEWPORT_SORT_TRACE_OUT` conserva ahora, además de los parents del
sorter global, el campo `local_proxies` con banda, child fuente, parent lógico,
sprite, bounds y profundidad final de cada copia segmentada. El analizador
cuenta `parents + local_proxies` como conjunto efectivo y mantiene compatibles
las trazas históricas que no tenían el campo. La regresión del analizador pasa
y el cliente mantiene `1538` tests exitosos, Clippy, formato y documentación en
verde. La captura de comprobación quedó limitada por un timeout en el primer
pase sin `precise_scope` (`35.507` parents); no se usa como métrica raster ni
como motivo para cerrar #326.

Validación #326/#567-SHIP-DEPOT-ORIENTATION-MATRIX (2026-09-15): con un
socket Weston nuevo y la fixture real `mvp_openttd_ship.sav`,
`MAP_SHOT_TOOL=ship_depot` produjo correctamente las cuatro capturas de
orientación `0,1,2,3` a `512×512`, escala Normal, con el ghost naval visible.
Cada PNG es RGB no vacío y las cuatro huellas cambian de forma/posición según
la orientación; esto valida el camino integrado toolbar → preview → raster,
no sólo el parser del selector. No hay todavía un oráculo nativo equivalente
para el ghost de UI ni cobertura de callbacks NewGRF, por lo que #326/#567
siguen abiertas y esta evidencia no declara paridad completa.

Corrección #326-MAP-SHOT-FIRST-CAMERA (2026-09-15, `51b337e4`): el driver de
captura fija centro y escala en el primer frame disponible, mientras conserva
la apertura del tool en el frame 30. Así `RenderRefresh` no ordena primero el
mapa grande con la pose panorámica inicial (`35.507` parents) antes de aplicar
el scope de la captura. En Kale, Normal `1280×720`, el PNG comparable queda en
`70.673/921.600` píxeles distintos (`7,6685 %`), sin traslación, frente a
`72.513/921.600` (`7,8682 %`) de la etapa anterior. La corrección pasa `1538`
tests del cliente, Clippy estricto y formato. #326 continúa abierta: esta
mejora estabiliza el arnés y no demuestra paridad global.

Validación #326-SORT-LOCAL-PROXY-TRACE-REAL (2026-09-15): con el primer frame
ya centrado, la traza real de Kale registra `precise_scope`, `1.572` parents
globales y `9` proxies locales; el analizador queda en `0` cajas de referencia
ausentes y `0` sprites distintos para las identidades observadas. La misma
ejecución con traza agotó el timeout antes de emitir PNG por el costo residual
de repetir el sorter durante el settle; su JSON sí es válido y no se usa para
la métrica raster anterior. #326 permanece abierta por las familias y escalas
restantes.

Corrección #326-SORT-TRACE-DEDUPE (2026-09-15, `5fe8711b`): el escritor de
`OPENTTDRS_VIEWPORT_SORT_TRACE_OUT` calcula una firma local del snapshot
completo —parents, orden, profundidades, scope y proxies— y omite la
serialización cuando nada cambió. La firma incluye explícitamente el estado de
cada proxy segmentado y tiene una regresión dedicada; el cliente queda en
`1539` tests exitosos (2 ignorados), Clippy y formato verdes. La ejecución Kale
trazada produjo un JSON estable de `463.933` bytes con `1.572` parents y `9`
proxies, pero todavía no emitió PNG dentro del timeout por el costo del sorter
repetido durante el settle. La optimización queda publicada como mejora del
instrumento, no como cierre de #326.

Corrección #326-SORT-ACTIVE-SET (2026-09-15): el sorter de viewport conserva
el mismo orden `(min_sum, índice)` usando un `BTreeSet` y recorre sólo el rango
activo hasta `max_sum`, en lugar de desplazar linealmente un `Vec` al retirar
cada parent. La regresión específica mantiene `40` tests de `viewport_sort`
exitosos e incluye un flujo grande con bounds iguales para fijar la estabilidad
del desempate. La captura Kale Out2x trazada se volvió a intentar con esta
variante y todavía agotó el timeout antes de emitir PNG; por tanto el cambio
reduce un coste puntual del mantenimiento del conjunto, pero no resuelve aún el
settle completo ni cierra #326.

Corrección #330-AIRPORT-FTA-PHYSICAL-CONTRACT (2026-09-15): el comparador
`compare_airport_fta_traces.py` incorpora `x_pos`, `y_pos` y `z_pos` al contrato
estricto de la traza viva. La medición de `helidepot_fta_cycle_15_3` coincide en
`initial` más `300` ticks también para esas tres coordenadas; una regresión
sintética confirma que un cambio subtesela o de altura ya no puede producir un
falso verde. `x`/`y` siguen excluidos porque OpenTTD los congela como campos
vestigiales bajo FTA. Esto endurece la evidencia de una fixture y no cierra
#330/#329: faltan perfiles aeroportuarios amplios, tráfico aire/mar y
callbacks/runtime NewGRF.

Corrección #330-INDUSTRY-IMPORTED-OUTPUTS (2026-09-15, `50b280db`): el
importador marca `INDY.produced` como la lista efectiva de salidas de la
instancia, incluyendo explícitamente el caso vacío. `Industry::produced_cargos`
ya no deriva una salida vanilla que el save no contiene; por eso la economía
mensual no consume una palabra RNG extra ni marca para cierre una industria
importada sin salidas. Los slots no vacíos mantienen su orden: los dos primeros
van a los stocks legacy y los siguientes al buffer extra, y la rehidratación con
catálogo vuelve a aplicar tasas, historiales y stocks desde las filas nativas.

La evidencia reproducible sobre `mvp_openttd_rich.sav` compara `11108` muestras
exactas para PBS y dinámica vial después de regenerar ambas trazas sin
instrumentación temporal. El core pasa `2759` tests (1 ignorado), la compilación
CMake de OpenTTD pasa, y quedan verdes formato, `git diff --check` y Clippy
acotado a la librería/binario afectados. La orden global de Clippy con todos
los targets todavía encuentra advertencias históricas en fixtures de tests y
módulos ajenos; no se atribuyen a esta corrección ni se mezclan en el alcance.
#330 permanece abierta por rutas multi-tick, tráfico complejo, presignals y
los oráculos completos de aire y mar.

Corrección #329/#567-SHIP-DEPOT-PARTIAL-ANCHOR (2026-09-15):
`ship_depot_north_tile` replica ahora la parte relevante de
`GetShipDepotNorthTile`: calcula `GetOtherShipDepotTile` y toma la menor
coordenada aun cuando la sección opuesta no esté materializada en un save
legacy/parcial. Antes el helper reutilizaba `ship_depot_other_tile`, que valida
la pareja completa y devolvía la sección consultada en ese caso. La regresión
`ship_depot_north_tile_matches_native_partial_section_geometry` fija la
diferencia y mantiene la validación estricta para demolición, consultas del
pool e índice espacial. Pasan los 19 tests del módulo de depósitos, los 101
tests acuáticos, formato, `git diff --check` y Clippy estricto de la librería;
#329/#567 permanecen abiertas por callbacks, pathfinding naval amplio y
aceptación visual/framebuffer.

Corrección #329/#567-SHIP-LOST-ROUTE-RNG (2026-09-15, `d0ff0b45`): el
controlador naval separa la reversa normal de la ruta perdida de una orden
`Station` cuyo destino legado es una boya. Cuando la ruta de alto nivel se
perdió, OpenTTD cae en `CreateRandomPath`/`CheckShipReverse`: toma las salidas
navales disponibles de la tesela actual sin puntuar la distancia al destino ni
filtrar por el vecino, y aun así consume el draw de `GetRandomTrackdir`. Rust
mantiene la elección geométrica local ya validada, pero consume el mismo
`Randomizer` global desde el paso autoritativo de `GameState`; las APIs
históricas aisladas siguen siendo deterministas.

El tick detenido por una avería activa también incrementa el contador interno
de `ShipController`, igual que `Ship::Tick` antes del early return. La traza
externa de `mvp_openttd_ship.sav` coincide en `1001/1001` muestras (`initial` +
1000 ticks), incluyendo posición subteselar, rumbo, estado, ruta, avería,
contador y estado RNG. Pasan `2763` tests del core (1 ignorado), formato,
`git diff --check` y Clippy estricto de la librería. La evidencia cierra esta
subetapa, no #329/#567: quedan YAPF/wormholes navales amplios, callbacks/runtime
NewGRF y aceptación visual/framebuffer completa.

Corrección #326-SORT-PRECISE-OUT2X (2026-09-15, `05ff7b1a`): el sorter de
viewport extiende el culling diagonal preciso hasta `Out2x` (`ortho_scale=2`) y
conserva el conjunto AABB desde `Out4x`, donde todavía no hay una matriz raster
equivalente validada. En `Kale_TitleGame.sav`, centrado en `189,126`, a 1280×720
y escala 2, la captura nativa reduce los parents considerados de `13.353` a
`4.695` y materializa `42` proxies locales; la divergencia bruta baja de
`759.517` a `759.439` píxeles y la alineada de `578.845` a `578.601`. La
regresión de límites verifica `Out2x` habilitado y `Out4x` conservador.

La comprobación naval en `mvp_openttd_ship.sav`, centrada en `32,32` a 512² y
escala 2, conserva el ghost y las capas visibles del depósito; su residuo sigue
dominado por la diferencia de fondo 8bpp y no justifica cerrar #326/#567. Pasan
los `1540` tests del cliente (2 ignorados), formato, `git diff --check` y
Clippy estricto del binario. La corrección reduce trabajo en `Out2x`, pero aún
quedan la matriz raster amplia, `Out4x`/`Out8x`, otras familias visuales y el
framebuffer global.

Extensión #326-SORT-PRECISE-OUT4X (2026-09-15, `5d62bfe5`): la misma banda
diagonal se habilita ahora hasta `Out4x`; `Out8x` conserva el AABB histórico.
En `Kale_TitleGame.sav`, a 1280×720 y centro `189,126`, el stream baja de
`28.757` parents AABB a `14.894` parents más `130` proxies locales (`15.024`
efectivos), frente a `14.470` parents del sorter nativo en 15 segmentos. La
comparación raster mejora levemente: `770.716`→`770.683` píxeles brutos y
`750.111`→`750.098` alineados.

La A/B adicional conserva exactamente el PNG del depósito naval de
`mvp_openttd_ship.sav` (8 parents) y de la fixture rica con estación, tren, bus
e industria (11 parents); no desaparecen ghost, boyas ni capas. Esto es una
reducción de trabajo validada en `Out4x`, no paridad visual completa: el trace
todavía deja 28 identidades nativas fuera y miles de bounds conservadores
adicionales, por lo que #326/#567 siguen abiertas para la matriz raster, otras
familias y `Out8x`.

Extensión #326-SORT-PRECISE-OUT8X (2026-09-15, `44e4f49d`): la banda diagonal
precisa cubre también el máximo `Out8x`. En `Kale_TitleGame.sav`, a 1280×720 y
centro `189,126`, el stream baja de `54.601` parents AABB a `36.968` parents
más `308` proxies locales (`37.276` efectivos), frente a `35.840` parents del
sorter nativo en 15 segmentos. El raster mejora levemente: `688.811`→`688.799`
píxeles brutos y `692.600`→`692.595` alineados.

La A/B de `Out8x` conserva exactamente el PNG y los diffs del depósito naval
(`8` parents) y de la fixture rica con estación, tren, bus e industria (`11`
parents), incluidos ghost, boyas y capas. Pasan los `1540` tests del cliente
(2 ignorados), formato, `git diff --check` y Clippy estricto del binario. Esto
cierra la subetapa de culling multizoom; #326/#567 permanecen abiertas por los
residuos de raster, las identidades nativas aún no cubiertas y otras familias
gráficas.

Corrección #326/#329-AIRPORT-FOUNDATION-SLOPE (2026-09-15, `8bf3c154`): los
aeropuertos inclinados de las rutas `MP_STATION/Airport` y `TileKind::Airport`
replican ahora el orden de `DrawTile_Station`: `FOUNDATION_LEVELED` se emite
antes del suelo, la superficie pasa a ser plana y los `DrawGroundSprite` de
apron/cercas se adjuntan al último parent de la fundación. Las capas `BUILD`
de `StationGfx` conservan parents independientes, pero usan la altura efectiva
de la superficie; el callback CB150 de un `AirportTile` custom sigue pudiendo
suprimir la fundación y los docks no entran en esta ruta.

La regresión
`sloped_airports_level_ground_for_station_and_imported_object_paths` cubre un
airport `MP_STATION` y otro aeropuerto importado en pendiente, verifica dos
fundaciones, ausencia de césped inclinado, children con profundidad estable y
las capas `2661/2662` sobre la superficie nivelada. Pasan `1541` tests del
cliente (2 ignorados), Clippy estricto del binario, formato y `git diff
--check`. Esta subetapa queda publicada, pero no cierra #326/#329/#567: siguen
pendientes la matriz completa de slopes/halftiles/rotaciones, layouts y
callbacks NewGRF restantes, además de aceptación raster en saves reales.

Regresión publicada `#326/#329-AIRPORT-FOUNDATION-MATRIX` (`325565b7`,
2026-09-15): `sloped_airport_foundation_matrix_keeps_blocks_and_surface_children`
ejercita las cuatro orientaciones de esquina tanto para pendientes simples como
pronunciadas. Verifica contra `foundation_draw_plan` el número de bloques,
ausencia de césped inclinado, relación child→foundation del apron y altura
efectiva de `BUILD`; pasan `1542` tests del cliente (2 ignorados). Esta matriz
fortalece la subetapa de aeropuertos, pero no cierra #326/#329/#567.

Diagnóstico #326/#561-RAIL-GLASS-AB (2026-09-15): la captura focalizada de
`Kale_TitleGame.sav`, centro `132,2`, 800×600, `Normal`, perfil limpio, aisló
el techo de estación. El orden/geométrica del stream nativo y Rust coincide
en `109/109` comandos, `32/32` capas y `8/8` vidrios; el residuo es de
composición. La A/B temporal de la máscara obtuvo `6129` píxeles distintos y
delta medio `0,382722` con alpha `0,0`; `7336` y `0,408451` con el calibrado
`0,50`; `7336` y `0,536719` con `1,0`; el intermedio `0,25` dio `0,394033`.
El mejor resultado agregado no conserva la semántica: distintas regiones del
techo requieren resultados diferentes porque el blitter 8bpp aplica
`PALETTE_TO_TRANSPARENT` según el color de destino. Se conserva `0,50` como
aproximación calibrada previa y no se cierra #326/#561: falta una composición
dependiente del framebuffer, no otro cambio de parent/child por intuición.

Corrección #329-STATION-AUTO-REFIT-NEXT-HOP (2026-09-15, `a1eb0246`): el
auto-refit de estación filtra ahora la carga en espera por las próximas
estaciones posibles de la orden. `StationCargoList::has_cargo_for` replica la
regla nativa: los paquetes con `next_hop` explícito sólo cuentan para la misma
estación y los paquetes sin hop (`StationID::Invalid`) siguen siendo válidos
para cualquier destino. Se hidrata la cola de packets antes de seleccionar el
cargo, por lo que también funciona con stocks agregados de saves antiguos.
La regresión `station_auto_refit_ignores_cargo_for_other_next_station` prueba
que un stock mayor con destino distinto no gana sobre el cargo encaminado a la
siguiente estación; la suite completa de core queda en `2765` tests exitosos y
`1` ignorado. Este corte no implementa todavía el balanceo de capacidad,
reservas de carga ni el consist articulado heterogéneo, por lo que #329 sigue
abierto.

Diagnóstico #326-ROADSTOP-SORT-SCOPE (2026-09-15): se contrastó la vista de
`Kale_TitleGame.sav` alrededor de `19,210` en `800×600`, `Normal`, contra una
captura nativa y una exportación Rust con el mismo viewport. El trace nativo de
`world-screenshot-sort` contiene `8` invocaciones independientes del sorter y
`1024` parents; Bevy exporta `765` parents globales y `30` proxies locales.
Por eso el comparador lineal no puede tratar el vector nativo de un bloque como
si fuera el orden global de la vista: reporta una inversión espuria entre una
parada (`sprite 1302`) y una estación (`5978`) al mezclar segmentos con
alcances distintos. La raster A/B queda en `21526/480000` píxeles (`4,4846%`,
delta medio `1,3792`) y concentra diferencias mixtas en techos de estación,
puente, edificios y árboles; no aísla una regresión de road-stop. El stream
estructural de la zona y el foco anterior de road-stop siguen coincidiendo,
por lo que no se cambia el compositor ni se cierra #326: el próximo paso es
hacer la comparación consciente de segmentos o conseguir un foco que no mezcle
familias antes de atribuir una inversión al renderer.

Corrección #329-STATION-AUTO-REFIT-CONSIST-BALANCE (2026-09-15, `09a48750`):
el autorefit automático deja de imponer un único cargo a todo el consist.
Cada unidad se evalúa con sus opciones y capacidad efectiva de refit, se retira
la capacidad del cargo anterior y se elige primero el cargo con menor capacidad
restante en el consist; los empates usan el stock disponible de la estación.
La selección conserva el filtro de `next_hop` introducido en `a1eb0246` y no
ejecuta callbacks sobre el vehículo real durante la consulta de capacidad. La
regresión `station_auto_refit_balances_distinct_consist_cargo_types` ejecuta el
refit sobre una cabeza y un remolque con cargos distintos y comprueba ambos
resultados. Core queda en `2766` tests exitosos y `1` ignorado; #329 sigue
abierta por reservas, carga parcial, cadenas articuladas completas y demás
contratos de `HandleStationRefit`.

Corrección #329-STATION-LOAD-NEXT-HOP (2026-09-15, `fb2814e5`): la carga de
estación y la carga secundaria de correo consultan un stock filtrado por las
próximas estaciones y extraen sólo esos destinos o packets sin `next_hop`.
Packets ya encaminados conservan su `next_hop`; sólo los packets legacy/sin
destino reciben una ruta nueva. La regresión
`station_loading_preserves_packet_next_hop_across_conditional_orders` cubre
dos ramas condicionales y evita que un lote de la segunda rama se reescriba a
la primera. Core queda en `2768` tests exitosos y `1` ignorado; el cliente
compila. La reserva nativa por vehículo y la carga parcial siguen pendientes.

Corrección #329-STATION-RESERVATION-ACCOUNTING (2026-09-15, `4c5d5094`): las
rutas de carga normal y correo usan la cantidad de reserva realmente obtenida
antes de extraer packets. Si ya existe una reserva de otra visita, no toman
unidades adicionales ni consumen su contador; si la extracción queda vacía,
liberan la reserva recién creada. La regresión
`station_loading_consumes_only_its_new_reservation` cubre ese interleaving y
conserva el `next_hop` válido. Core queda en `2769` tests exitosos y `1`
ignorado; esto corrige la contabilidad local, pero no sustituye la reserva
nativa asociada a un vehículo ni su retorno entre ticks.

Corrección #326-SORT-SEGMENT-COVERAGE (2026-09-15, `4b37e42a`): el analizador
`analyze_viewport_sort_bands.py` deja de tratar implícitamente la captura nativa
como una lista global. El informe agrega cobertura de identidades únicas por
segmento, repeticiones de parents y ejemplos de cajas/sprites ausentes. También
marca `order_comparison.status=not_comparable` cuando la candidata sólo aporta
un vector global, porque el native reinicia `final_ordinal` en cada
`ViewportDoDraw`; así una inversión entre una parada de un segmento y una
estación de otro no se presenta como fallo del compositor. La regresión sintética
y la traza real de Kale pasan: `8` segmentos, `1024` parents, `4/8` segmentos
con cobertura completa de identidades únicas, `10` identidades ausentes y `0`
repeticiones dentro de segmento. Es una mejora del diagnóstico, no el cierre de
#326: siguen pendientes las identidades no cubiertas, clipping/composición y la
aceptación raster de las familias restantes.

Corrección #329-STATION-RESERVATION-BY-CARGO (2026-09-15, `f109c9b4`):
`StationCargoList` mantiene ahora, además del contador total legacy, una tabla
de reservas por `CargoType`. Las rutas de carga normal y de correo reservan y
liberan por tipo, de modo que una reserva de carbón no bloquea artificialmente
el correo disponible en la misma estación. El remanente de reservas cargado
desde JSON/SAV antiguos, que no identifica el tipo, continúa aplicándose de
forma conservadora para no permitir doble consumo. La regresión
`station_reservations_are_scoped_to_cargo_type` cubre reservas y consumos
independientes de carbón y correo; la suite core queda en `2770` tests
exitosos, `1` ignorado, con clippy, formato y check del cliente en verde.
Esto corrige el ámbito local de la reserva, pero todavía no asocia packets a
un vehículo concreto ni modela su devolución/carga parcial entre ticks; #329
permanece abierta.

Corrección #329-STATION-RESERVATION-SAV-ROUNDTRIP (2026-09-15, `e561d0bd`):
la hidratación de `STNN.goods[].cargo.reserved_count` reconstruye también
`reserved_by_cargo`, y el exportador asigna primero la reserva conocida al slot
correspondiente antes de repartir cualquier remanente legacy. El round-trip
con carbón y correo verifica que una reserva de carbón no reaparezca como
reserva de correo al guardar/cargar; la suite focalizada de SAV y clippy de
producción pasan. La asociación nativa por vehículo, el retorno de packets
reservados y la carga parcial entre ticks siguen pendientes; #329 permanece
abierta.

Corrección #329-VEHICLE-RESERVATION-LIFECYCLE (2026-09-15, `ff12ef9d`):
`VehicleCargoList` expone ahora el ciclo mínimo de `MTA_LOAD`: diferencia
`stored_count` de `reserved_count`, permite promover una carga parcial con
`load_reserved` y extrae desde el extremo sólo los packets reservados para su
devolución FIFO. La regresión
`vehicle_reserved_packets_promote_and_return_fifo` cubre promoción parcial,
split de packet y saturación al devolver más unidades de las reservadas; la
suite core queda en `2771` tests exitosos y `1` ignorado, con clippy estricto
verde. Es el primitive de la propiedad por vehículo; la reserva todavía no
está conectada al barrido de estaciones ni guarda la estación propietaria, por
lo que #329 permanece abierta.

Corrección #329-STATION-VEHICLE-RESERVATION-PRIMITIVE (2026-09-15, `447889bb`):
`StationCargoList::reserve_for_vehicle` mueve físicamente los packets elegibles
al extremo `MTA_LOAD` del vehículo, registra la estación y el cargo de origen,
y separa esas unidades de las reservas virtuales legacy. Las operaciones
`load_reserved_from_vehicle` y `return_reserved_from_vehicle` actualizan ambos
contadores sin duplicar stock, soportan promoción parcial y devuelven el tramo
reservado en FIFO; las regresiones cubren correo disponible junto a carbón,
rechazo de otra estación y coexistencia con una reserva virtual. Core queda en
`2773` tests exitosos y `1` ignorado, con Clippy de producción y check del
cliente en verde. El barrido `load_vehicles` todavía usa la ruta inmediata y
el nuevo estado físico aún no está codificado en el wire SAV; #329 permanece
abierta.

Corrección #329-VEHICLE-RESERVATION-SAV-ROUNDTRIP (2026-09-15, `ccf8c5e2`):
la importación conserva las reservas `MTA_LOAD` aunque el stock restante de la
estación sea menor, reatacha el vehículo a `last_loading_station` y reconstruye
`reserved_physically_by_cargo`. La reconciliación limita sólo reservas sin
respaldo en packets o vehículos; el caso físico conserva estación, cargo,
`stored_count` y `reserved_count` después de exportar/importar. La regresión
`export_roundtrip_preserves_physical_vehicle_reservation` y la existente de
reservas virtuales pasan; core queda en `2774` tests exitosos y `1` ignorado,
con Clippy de producción y check del cliente en verde. La integración del
estado reservado en `load_vehicles` y la carga parcial entre ticks siguen
pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-RESERVATION-RUNTIME-CONSUME (2026-09-15, `4a3111ce`):
`load_vehicles` procesa primero una reserva `MTA_LOAD` asociada a la estación
actual, promueve sólo la cantidad permitida por rating/velocidad y conserva el
remanente para el tick siguiente. Esa visita no cae por accidente en industria,
correo ni carga inmediata, y la regresión
`station_loading_consumes_existing_vehicle_reservation_first` verifica que no
se duplique el stock visible y que el contador de estación coincida con
`reserved_count`. Core queda en `2775` tests exitosos y `1` ignorado, con Clippy
de producción y check del cliente en verde. Todavía falta crear reservas nuevas
desde el barrido runtime, devolverlas al cambiar de ruta y cubrir consistes
articulados; #329 permanece abierta.

Corrección #329-VEHICLE-RESERVATION-RUNTIME-CREATE (2026-09-15, `6027ca3d`):
las visitas `FullLoad` y `FullLoadAny` reservan físicamente la capacidad libre
desde la cola de la estación antes de promover la carga. La reserva actualiza
`cargo_stock` al remanente visible, ancla `first_station` si faltaba y queda
protegida frente a `Stage`; la regresión
`full_load_station_reserves_before_partial_promotion` recorre los ticks de
carga parcial y comprueba conservación de unidades y contadores. Core queda en
`2776` tests exitosos y `1` ignorado, con Clippy de producción y check del
cliente en verde. Aún falta devolver la sección `MTA_LOAD` al cambiar la orden,
además de la integración completa de consistes articulados; #329 permanece
abierta.

Corrección #329-VEHICLE-RESERVATION-RUNTIME-CANCEL (2026-09-15, `da85635c`):
`load_vehicles` devuelve al andén original sólo la sección `MTA_LOAD` cuando la
orden de su estación ya no forma parte de la ruta. Los packets promovidos no se
tocan; se reconstruyen `cargo_stock` y los contadores físicos, y el guard de
descarga evita que `Stage` destruya una reserva entre fases. La resolución del
siguiente salto se hace al crear la reserva, conservando el destino de los
packets aunque la promoción se reparta en varios ticks. La regresión
`full_load_reservation_returns_when_source_order_is_removed` comprueba
conservación de unidades al eliminar la orden. Core queda en `2777` tests
exitosos y `1` ignorado, con Clippy de producción y check del cliente en verde.
La coordinación de reservas con toda la formación articulada y otras
cancelaciones siguen pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-RESERVATION-CONSIST-ORDER (2026-09-15, `94a6b58e`):
las unidades articuladas y los vagones de un consist consultan la orden de su
cabeza para `FullLoad`, `FullLoadAny`, `NoLoad`, siguiente estación y
cancelación de reservas. Cada unidad conserva sus propios packets `MTA_LOAD`;
la orden de la cabeza no avanza mientras otra unidad tenga una reserva o carga
pendiente y se promueve sólo cuando la formación termina. La regresión
`full_load_consist_uses_head_order_for_each_cargo_unit` cubre una cabeza y una
pieza articulada con capacidades distintas, carga parcial en varios ticks y
conservación del stock. El escenario ferroviario dual comprueba además el
retorno a A durante la ventana de simulación sin depender de una posición
accidental en el tick final. Core queda en `2778` tests exitosos y `1` ignorado,
con Clippy de producción y check del cliente en verde. La descarga de
consistes, las capacidades heterogéneas de tren por unidad y las reservas de
formaciones duales siguen pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-UNLOAD-CONSIST-ORDER (2026-09-15, `c388cbb4`):
la descarga de vagones y articulados consulta ahora la orden de la cabeza para
`NoUnload`, `Unload`, `Transfer`, la estación actual y el siguiente salto. Una
unidad vacía ya no hace avanzar por sí sola la cabeza: durante una descarga
gradual la formación conserva la orden y sólo la cierra cuando todas sus
unidades quedaron vacías; también se conserva el estado de descarga de la
cabeza para impedir una carga prematura en el tick siguiente. Las regresiones
`unload_consist_uses_head_order_for_each_cargo_unit` y
`unload_consist_defers_head_order_until_all_units_are_empty` cubren ambos
contratos. Core queda en `2780` tests exitosos y `1` ignorado, con Clippy de
producción y check del cliente en verde. La capacidad agregada de la cabeza
frente a la capacidad local de cada unidad y las reservas de formaciones
duales siguen pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-CONSIST-UNIT-CAPACITY (2026-09-15, `272060de`):
la carga y descarga de trenes usa la capacidad local de cada unidad: la
locomotora ya no absorbe la capacidad agregada de su consist para reservar
stock, limitar carga, calcular porcentaje o registrar capacidad del linkgraph.
La decisión de `FullLoad`/`FullLoadAny` y el avance de la orden se evalúan sobre
el conjunto `(tipo, carga, capacidad)` de las unidades; una cabeza sin bodega
no bloquea la transición cuando los vagones terminaron. Los probes de carga y
señales observan también el consist completo, por lo que los escenarios de
suministro siguen midiendo carga aunque ésta viva en un vagón. La regresión
`full_load_train_consist_uses_each_unit_capacity` cubre una locomotora sin
capacidad local y un vagón de carbón de 30 unidades. Core queda en `2781`
tests exitosos y `1` ignorado, con Clippy de producción y check del cliente en
verde. La rehidratación explícita de capacidades locales en casos SAV/EngineID
no catalogado y la cobertura de consistes duales/heterogéneos más complejos
siguen pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-CONSIST-LOCAL-CAPACITY-SAV (2026-09-15, `fa758dab`):
la resolución de capacidad local se comparte entre carga/descarga runtime y
el writer SAV. La importación conserva `VEHS.common.cargo_cap` por unidad,
incluidos vagones con capacidades heterogéneas, y `ConsistChanged` ya no
reemplaza ese valor por el default del motor al recalcular la capacidad
agregada de la cabeza. Al reexportar, la cabeza serializa su capacidad local
en vez de la suma del consist. La regresión
`sav_roundtrip_preserves_local_capacity_for_heterogeneous_consist` cubre
capacidades 40/60, `cargo_cap` de locomotora cero y reimportación con suma 100.
Core queda en `2781` tests exitosos y `1` ignorado, con Clippy de producción y
check del cliente en verde. La capacidad exacta de una cabeza cuyo EngineID
custom no está catalogado y las formaciones duales todavía requieren una
fuente local persistente propia; #329 permanece abierta.

Corrección #329-VEHICLE-CONSIST-DUAL-SAV (2026-09-15, `18aba8bc`): el parser
conserva `GVSF_MULTIHEADED` y distingue la cabina trasera por la combinación
`MULTIHEADED` sin `FRONT` ni `ENGINE`; el writer reemite la pareja como
`0x29` (`FRONT|ENGINE|MULTIHEADED`) y `0x20` (`MULTIHEADED`), en lugar de
convertir la trasera en vagón. Al hidratar un SAV, `next/prev` y los flags
coincidentes reconstruyen `other_multiheaded_part`, conservando el motor, la
restricción de acoplamiento y la capacidad local de cada cabina. Las
regresiones `vehs_preserves_train_dual_head_subtypes` y
`sav_roundtrip_preserves_dual_headed_consist_identity` cubren subtipo nativo,
round-trip y capacidades agregada/local `76/38`. Core queda en `2782` tests
exitosos y `1` ignorado, con Clippy de producción, formato y check del cliente
en verde. La carga/reserva dual durante la simulación y los EngineID custom sin
catálogo siguen pendientes; #329 permanece abierta.

Corrección #329-VEHICLE-CONSIST-UNKNOWN-CAPACITY-SAV (2026-09-15, `b7d54e09`):
cada vehículo ferroviario importado conserva su `VEHS.common.cargo_cap` local
en `native_cargo_capacity`, separado de `Vehicle::capacity`, que en la cabeza
puede ser la suma del consist. La importación ya no deja el Kirby vanilla que
instala `Vehicle::new` cuando el `EngineID` nativo es custom y no está
catalogado; la resolución de capacidad usa el valor persistido como fallback
exacto sólo para esa cabeza no resuelta. El writer vuelve a emitir la
capacidad local y la regresión
`sav_roundtrip_preserves_unknown_train_head_local_capacity` cubre `EngineID`
511, capacidades locales 7/30 y suma rehidratada 37. Core queda en `2782`
tests exitosos y `1` ignorado, con Clippy, formato, documentación y check del
cliente en verde. #329 sigue abierta por la identidad/propiedades completas,
carga y reservas de formaciones duales, y comportamiento de `EngineID` custom
cuando el catálogo NewGRF sí está disponible.

Corrección #329-VEHICLE-CONSIST-ENGINE-REHYDRATE (2026-09-15, `0e59f3a9`):
después de reconstruir el catálogo Action0, los vehículos importados vuelven
a enlazar su `engine_type` nativo mediante `EIDS` (`GRFID` + ID local), con
validación de la clase de vehículo. Las formaciones ferroviarias afectadas
recalculan sus métricas contra el catálogo activo; la cabeza conserva su
`cargo_cap` local aunque el motor custom ya esté resuelto, y un GRF ausente no
se sustituye silenciosamente por el motor vanilla por defecto. La regresión
`rehydrates_imported_train_engine_and_recomputes_consist_capacity` cubre un
motor custom de capacidad de catálogo 55, capacidad SAV local 7 y vagón 30,
rehidratando la suma 37. Core queda en `2783` tests exitosos y `1` ignorado,
con Clippy de core, formato, documentación y check/Clippy del binario cliente
en verde. #329 sigue abierta por callbacks y propiedades de vehículo aún no
modelados, carga/reservas duales y otras rutas NewGRF sin correspondencia.

Corrección #329/#567-SHIP-DEPOT-VEHICLE-CACHE (2026-09-15, `f8a5b56f`): al
rehidratar un vehículo SAV cuyo `EIDS` apunta a un motor NewGRF custom, el
runtime recalcula las cachés derivadas que no vienen completas en `VEHS`:
período de envejecimiento de carga para todas las clases, velocidad efectiva
de barcos y aeronaves, y período de correo de aeronaves. En particular, un
barco estacionado en un depósito naval conserva la clase `Canal` del tile y
aplica la fracción de velocidad del motor custom; la ruta de movimiento naval
usa también la clase de agua del tile de depósito/estación, no sólo el helper
visual de canal. Las regresiones
`rehydrates_imported_custom_ship_and_refreshes_depot_cache` y
`rehydrates_imported_custom_aircraft_and_refreshes_fta_cache` cubren la
rehidratación por `EIDS`, la caché previa deliberadamente obsoleta, velocidad,
envejecimiento y preservación de aceleración/capacidad. Validación: `2785`
tests de core pasados, `0` fallidos y `1` ignorado en la suite de biblioteca;
los dos tests nuevos, `sav_load` (4/4) y el subconjunto SAV (20/20, 4
ignorados), formato, `git diff --check`, Clippy estricto de la biblioteca
core y del binario cliente, y `cargo check` del cliente quedaron verdes. El
clippy `--all-targets` global continúa exponiendo lints preexistentes en
tests/fixtures y no se usa para declarar este bloque verde. #329/#567 siguen
abiertos por callbacks y propiedades completas de vehículos, agua efectiva en
túneles/puentes, cachés visuales y la paridad integral del depósito naval.

Corrección #329/#567-SHIP-SPEED-FRACTION-SEMANTICS (2026-09-15, `e2f8a232`):
`EngineDef::ocean_speed_frac` y `canal_speed_frac` siguen la semántica de
`Engine::ApplyWaterClassSpeedFrac` de OpenTTD: el byte almacenado es una
reducción, de modo que `0` no reduce la velocidad y el multiplicador efectivo
es `256 - frac`. El helper naval ya no interpreta el byte como multiplicador
directo. El caso anterior con `128` no distinguía ambas fórmulas; la regresión
`ship_speed_properties_are_native_reductions` cubre `0`, `64` y `128`, y el
parser de Action0 verifica que un barco de velocidad 100 con reducción 64/128
queda en 75/50 según la clase de agua. La biblioteca core completa queda en
`2786 passed; 0 failed; 1 ignored`; #329/#567 permanecen abiertos por agua
efectiva en túneles/puentes, YAPF/costes navales y las propiedades/cachés
NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-TILE-RANDOM-NIBBLE — random por tesela

Actualizado: 2026-09-16 (`6243e322`). Se corrigió el uso de `MAP3` en
`AirportTile`: OpenTTD conserva sólo los bits 4..7 como random de tesela, no
el byte completo. El contexto Action2 y el replay determinista usado por
CB152/CB153 comparten ahora `airport_tile_random_bits`, evitando que los bits
bajos de otros campos de `MP_STATION` seleccionen otra rama o alteren la
semilla de animación. Las regresiones cubren el word de contexto y la
invariancia del replay ante cambios de `MAP3[0..3]`; pasaron 80 pruebas
filtradas de aeropuerto, Clippy estricto y `git diff --check`. #326/#329/#567
siguen abiertas por foundations/rotaciones, paletas, callbacks y consumidores
NewGRF restantes.

Corrección #329/#567-SHIP-EFFECTIVE-WATER-CLASS (2026-09-15, `6b2bf6ea`): la
consulta de velocidad naval usa ahora la contraparte de
`GetEffectiveWaterClass`. Un tunnel/bridge de transporte acuático fuerza
`Canal`, aunque sus bits persistidos indiquen otra clase, y un rail con
`RailGroundType::HalfTileWater` fuerza `Sea`; `River` comparte correctamente
la fracción canal. La misma resolución se aplica al refresco de cachés durante
la rehidratación SAV por `EIDS`, evitando que el primer tick o la salida de un
depósito use una fracción incorrecta. Las regresiones
`effective_ship_water_class_matches_tunnelbridge_and_half_tile_rail` y
`ship_max_speed_uses_canal_fraction_for_river_and_water_tunnelbridge` cubren
la representación raw y el consumidor físico. La biblioteca core queda en
`2788 passed; 0 failed; 1 ignored`; #329/#567 siguen abiertos por
YAPF/costes navales, callbacks y cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-WATER-COST (2026-09-15, `4469a5cf`): se añadió
un coste naval ponderado por tile alineado con la penalización de velocidad de
`YapfShip`; mar usa `ocean_speed_frac` y canal/río usa `canal_speed_frac`, ambos
como reducciones nativas. El routing de barcos y la selección entre varios
`DockingTile` usan el coste acumulado, y la caché separa estos perfiles de las
rutas genéricas y de otros motores. La regresión
`ship_yapf_cost_prefers_sea_detour_over_slow_canal` prueba el desvío por mar
ante un canal corto con reducción 128. Validación: `2789` tests de core
pasados, `0` fallidos y `1` ignorado, Clippy estricto de core/binario cliente,
`cargo check` del cliente y formato verdes. #329/#567 permanecen abiertos por
curvas/trackdirs navales, ocupación de muelles, locks, callbacks y cachés
visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-LOCK-COST (2026-09-15, `ac786ee0`): se incorporó
la penalización de `YapfShip` para el centro de una esclusa, proporcional a la
velocidad canal estática del motor, con `LockPart::Middle` como única parte
penalizada. La ruta acumulada y la caché naval conservan este perfil; la
regresión `ship_yapf_cost_prefers_sea_detour_over_slow_canal` cubre el rodeo
cuando el coste de la esclusa supera el del desvío. Validación: `2789` tests de
core pasados, `0` fallidos y `1` ignorado, Clippy estricto de core/binario
cliente, `cargo check` del cliente y formato verdes. #329/#567 siguen abiertos
por curvas/trackdirs, ocupación dinámica de muelles, aqueductos, callbacks y
cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-DOCK-OCCUPANCY (2026-09-15, `797d450e`): la
selección naval entre varios `DockingTile` añade `3 * YAPF_TILE_LENGTH` por
barco visible en el amarre, excluye el propio barco y no cuenta vehículos con
`SHIP_STATE_DEPOT`. La prueba
`ship_docking_occupancy_matches_yapf_and_ignores_depot_ships` verifica la
penalización de 300 sin modificar la topología ni las rutas sin estación.
Validación: `2790` tests de core pasados, `0` fallidos y `1` ignorado, Clippy
estricto, check del cliente y formato verdes. #329/#567 permanecen abiertos
por curvas/trackdirs, aqueductos, callbacks y cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-AQUEDUCT (2026-09-15, `2a98ae86`): las rampas
de transporte acuático codificadas como `MP_TUNNELBRIDGE` se enlazan con la
rampa opuesta como wormhole naval; la entrada lateral queda bloqueada y el
pathfinder cobra el vano omitido con la misma escala de `YapfShip`. El
controlador aplica el salto físico al extremo opuesto y mantiene la ruta
posterior hacia el muelle. Las pruebas
`ship_path_jumps_aqueduct_and_rejects_inner_ramp_side` y
`ship_controller_jumps_aqueduct_span_at_inner_ramp` cubren navegación y
movimiento. Validación: `2792` tests de core pasados, `0` fallidos y `1`
ignorado, Clippy de producción, formato y `git diff --check` verdes. #329/#567
siguen abiertos por curvas/trackdirs, dirección preferida, callbacks y cachés
visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-CURVE-TRACKDIR (2026-09-15, `98636d13`): la
búsqueda naval conserva el estado `(tile, trackdir)` para no ocultar curvas al
fusionar rutas. El coste replica `CurveCost` con `1 * YAPF_TILE_LENGTH` para
45°, `6 * YAPF_TILE_LENGTH` para 90° y `YAPF_TILE_CORNER_LENGTH` para tracks de
esquina; los saltos de acueducto mantienen sus teselas omitidas. La prueba
`ship_yapf_cost_keeps_curve_trackdir_state` cubre la transición curva y el
tramo recto, además de las regresiones navales previas. Validación: `2793`
tests de core pasados, `0` fallidos y `1` ignorado, Clippy de producción,
check del cliente, formato y `git diff --check` verdes. #329/#567 siguen
abiertos por dirección preferida, trackdirs físicos completos en el
controlador, callbacks y cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-PREFERRED-DIRECTION (2026-09-15, `d950cb3e`):
los nodos navales aplican `IsPreferredShipDirection` con la tabla de paridad
vanilla para trackdirs diagonales y de esquina, sumando `YAPF_TILE_LENGTH` a
los sentidos no preferidos. La prueba
`preferred_ship_direction_matches_native_parity_table` cubre la tabla completa
y la de costes curvos comprueba la composición del sesgo. Validación: `2794`
tests de core pasados, `0` fallidos y `1` ignorado, Clippy de producción,
check del cliente, formato y `git diff --check` verdes. #329/#567 siguen
abiertos por trackdirs y cache de ruta completos en el controlador, settings
configurables, callbacks y cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-PATH-CACHE-TRACKDIR (2026-09-15, `e67fd765`): el
controlador naval consume ahora el `Trackdir` cacheado desde `path.back()`, como
`ChooseShipTrack` nativo, y valida su entrada, orientación y salida contra el
path de teselas actual antes de usarlo. La proyección local se emite en el orden
inverso requerido por SAV, conserva saltos de acueducto y consume también la
entrada del extremo opuesto después del wormhole; una caché vieja o incompatible
se invalida de forma completa y cae en la selección normal. Las regresiones
`ship_path_cache_uses_native_back_as_next_trackdir`,
`ship_path_cache_invalidates_trackdir_with_wrong_exit` y
`ship_path_cache_preserves_aqueduct_jump_entries` cubren orden, invalidación y
acueductos. Validación: `2797` tests de core pasados, `0` fallidos y `1` ignorado,
Clippy estricto de core, check del cliente y formato verdes. #329/#567 siguen
abiertos por trackdirs físicos adicionales, settings configurables, callbacks y
cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-CURVE-SETTINGS (2026-09-15, `d736dfb2`): las
penalizaciones configurables `pf.yapf.ship_curve45_penalty` y
`pf.yapf.ship_curve90_penalty` se aplican ahora desde `PathfindingSettings` al
perfil de `YapfShip`; el límite `0..=1000000` replica el setting nativo y ambos
valores forman parte de la clave de caché. PATS los serializa como `SLE_UINT`
de 32 bits y el lector conserva defaults vanilla para saves anteriores. Las
pruebas cubren selección de perfil, clamp, separación de caché y round-trip.
Validación: `2799` tests de core pasados, `0` fallidos y `1` ignorado, Clippy
estricto de core, check del cliente, formato y `git diff --check` verdes.
#329/#567 siguen abiertos por la exposición numérica en la UI experta,
trackdirs físicos adicionales, callbacks y cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-ORIGIN-TRACKDIR (2026-09-16, `ba7da754`): el
pathfinder naval acepta el `Trackdir` físico de origen y deja de sembrar cuatro
rumbos rectos cuando el vehículo ya tiene un estado observable. El helper local
replica `Ship::GetVehicleTrackdir` para depósitos, acueductos, vías ordinarias
y barcos accidentados; routing, caché y selección multi-muelle conservan ese
origen tanto en el camino como en el coste acumulado. Las regresiones verifican
la salida compatible, la salida opuesta bloqueada, el aislamiento de caché y el
mapeo nativo de estado/dirección. Validación: `2802` tests de core pasados,
`0` fallidos y `1` ignorado, Clippy estricto de core, check del cliente, formato
y `git diff --check` verdes. #329/#567 siguen abiertos por exposición numérica
en UI experta, reversa física y callbacks/cachés visuales NewGRF restantes.

Corrección #329/#567-SHIP-YAPF-DEPOT-ORIGINS (2026-09-16, `2082de67`): la
salida naval desde depósito conserva una máscara de dos `Trackdir` de origen:
el sentido de la sección norte y su inverso, tal como el par
`forward_dirs | reverse_dirs` de `CheckShipReverse` nativo. El resto de la flota
mantiene un origen físico único. La máscara viaja por A*, coste de ruta, caché
por tick, routing paralelo/secuencial y selección multi-muelle; las claves
separan una máscara de depósito de cualquier origen único. Las regresiones
cubren la ruta compatible, la no-colisión de caché y la identificación del
estado de depósito. Validación: `2803` tests de core pasados, `0` fallidos y `1`
ignorado, Clippy estricto de core, check del cliente, formato y
`git diff --check` verdes. #329/#567 permanecen abiertas por la decisión de
reversa física completa, UI experta, callbacks y cachés visuales NewGRF.

Corrección #329/#567-SHIP-YAPF-CURVE-UI (2026-09-16, `b6549fd5`): la ventana
Bevy de pathfinding ofrece presets para las penalizaciones navales de 45° y
90°, conserva la selección al sincronizar con `PathfindingSettings` y aplica
el mismo límite nativo `0..=1000000` que usa el runtime. El catálogo inglés y
sus pruebas directas acompañan la nueva superficie de configuración.
Validación: `cargo check -p openttdrs-client --locked --offline`, Clippy
estricto del binario cliente, formato y `git diff --check` verdes; el test del
binario no llegó a enlazar dentro de la ventana controlada y no se presenta
como exitoso. #329/#567 permanecen abiertas por reversa física completa,
callbacks y cachés visuales NewGRF.

Corrección #329/#567-SHIP-YAPF-DEPOT-REVERSE (2026-09-16, `9b0e6c47`): la
salida naval de un depósito compara dos búsquedas YAPF restringidas, forward y
`ReverseTrackdir`, con el `ShipPathCost` del motor y los ajustes `pf.ship_curve*`
vigentes. El controlador autoritativo recibe ahora la configuración de la
partida, y `ship_depot_reverse_compares_both_yapf_origins` cubre el caso en que
la rama inversa es más barata aunque el primer tile visible del path apunte al
frente. Validación: `2804` tests de core pasados, `0` fallidos y `1` ignorado,
Clippy estricto de core/cliente, formato y `git diff --check` verdes. #329/#567
permanecen abiertas por callbacks y cachés visuales NewGRF.

Corrección #329/#567-SHIP-YAPF-BLOCKED-REVERSE (2026-09-16, `8c93ac34`): la
reversa naval ante una vía bloqueada compara el coste completo de YAPF para
cada `Trackdir` de salida, incluyendo el perfil efectivo del motor, la clase
mar/canal y las penalizaciones configurables de curva `pf.yapf.ship_curve*`.
Catálogo y settings se propagan por el controlador y sus fallbacks de
acueducto; `ship_reverse_on_blocked_track_uses_weighted_yapf_cost` cubre un
rodeo marítimo más largo que vence a un canal corto y lento. Validación: `2805`
tests de core pasados, `0` fallidos y `1` ignorado, Clippy estricto del núcleo y
del binario cliente, formato y `git diff --check` verdes. #329/#567 siguen
abiertas por callbacks y cachés visuales NewGRF.

Corrección #329/#567-SHIP-YAPF-PATHLESS-TRACK (2026-09-16, `dfe2fd92`):
`ChooseShipTrack` deja de puntuar por distancia Manhattan cuando no hay
`path_next`; cada `Trackdir` físico pasa por YAPF con el perfil efectivo del
motor y los settings navales `pf.yapf.ship_curve*`. La selección autoritativa,
la caché y el fallback de acueducto reciben explícitamente catálogo y
`PathfindingSettings`; el wrapper público conserva el perfil default para
compatibilidad legacy. La regresión del canal frente al rodeo de mar verifica
que la decisión usa coste ponderado. Validación: `2805` tests de core pasados,
`0` fallidos y `1` ignorado, Clippy estricto de core y del binario cliente,
formato y `git diff --check` verdes. #329/#567 siguen abiertas por callbacks y
cachés visuales NewGRF.

Corrección #329/#567-NEWGRF-CACHE-TEARDOWN (2026-09-16, `3c64fef3`): el
registro de salida de InGame cubre ahora las cachés `NewGrfSignalSpriteCache`
y `NewGrfIndustrySpriteCache`, ambas creadas por `WorldRenderPlugin` y antes
omitidas del teardown. Se añadió `clear` junto con las entradas de reset y
remove, y el inventario ejecuta `leave_ingame` con los dos recursos para
verificar que no sobreviven handles de sprites al cambiar de partida. La
validación dirigida del cliente pasó, junto con Clippy estricto, formato y
`git diff --check`. Este bloque corrige sólo la frontera de lifecycle; #329/#567
siguen abiertos por la invalidación de consumidores restantes, callbacks y
scopes NewGRF.

Corrección #329/#567-ENGINE-PREVIEW-COMPANY-CARGO (2026-09-16, `2b5f24f0`):
la selección de `GetPreviewCompany` deja de considerar suficiente que la
empresa tenga cualquier unidad del mismo tipo. El scheduler consulta ahora la
capacidad declarada por el motor de esa unidad y el cargo actual contra el
conjunto de cargo por defecto, `refit_mask` traducida y listas CTT del motor en
preview; los trenes conservan `ALL_CARGOTYPES` porque pueden añadir vagones.
La regresión `preview_scheduler_requires_vehicle_cargo_compatible_with_engine`
reproduce el rechazo de un buque de mercancías para un petrolero y la posterior
oferta cuando la unidad pasa a petróleo. Validación: `2806` tests de core
pasados, `0` fallidos y `1` ignorado, Clippy estricto, formato y
`git diff --check` verdes. #329/#567 siguen abiertos por callbacks de
vehículos, invalidación de consumidores y scopes NewGRF restantes.

Corrección #329/#567-VEHICLE-VISUAL-EFFECT-CONTEXT (2026-09-16, `6aa97ee9`):
el renderer de humo/chispas construye una sola instantánea Action2 completa
por unidad y la comparte entre CB10 (`0x10`) y CB160 (`0x160`). El contexto
incluye consist, carga, badges de vía, parámetros del stack NewGRF,
randomización, edad/velocidad y librea; el callback avanzado conserva sus
registros `0x100..0x103` y el writeback persistente al vehículo real. Se
mantienen wrappers reducidos para las APIs legacy, pero ya no se usan en el
call site autoritativo. La regresión
`visual_effect_context_exposes_vehicle_variables_to_cb10` demuestra que `var
0xB4` modifica el tipo de efecto; las ocho pruebas de efectos visuales pasan.
Validación completa: cliente `1544 passed; 2 ignored`, core `2806 passed; 1
ignored`, Clippy estricto de core/cliente, formato y `git diff --check` verdes.
#329/#567 sigue abierto por los callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-RUNTIME-ONLY-VIEWS (2026-09-16, `050da4d3`): las
cachés planas de estación, industria, casa y objeto ya no usan el slot cero
para todas las vistas de un Action2 runtime-only. La clave conserva el índice
materializado y evita que orientaciones distintas compartan handle/textura
cuando no existe una tabla estática. El camino de dibujo de estación consulta
la vista runtime con el contexto Action2 preparado, de modo que también se
renderizan specs que sólo tienen gráficos runtime. Las cuatro regresiones de
caché y la de estación comprueban handles y bytes RGBA; siete pruebas
runtime-only pasan. Validación completa: cliente `1548 passed; 2 ignored`,
Clippy estricto, formato y `git diff --check` verdes. Este bloque no cierra
#326/#329/#567: siguen pendientes layouts, paletas, callbacks y consumidores
NewGRF fuera de estas cachés.

Corrección #329/#567-VEHICLE-COLOUR-MAPPING-CONTEXT (2026-09-16, `8829e150`):
los call sites reales de sprites de vehículos, incluido el rotor, pasan ahora
el contexto Action2 completo a `CBID_VEHICLE_COLOUR_MAPPING` en lugar de
reconstruir un contexto reducido desde una unidad aislada. Así la selección de
paleta observa las variables de consist/carga (`var 40/47/B4`), padre, badges,
randomización, generación visual y parámetros del GRF que ya usa el resolver
de la vista. La regresión `vehicle_colour_mapping_uses_prepared_consist_context`
selecciona `Green` a partir de `var 0xB4`; las cinco pruebas vecinas de
previews/2CC/SpriteStack/rotor también pasan. Validación completa: cliente
`1543 passed; 2 ignored`, core `2807` tests de biblioteca sin fallos, Clippy
estricto del binario cliente, formato y `git diff --check` verdes. #329/#567
siguen abiertas por callbacks avanzados, layouts, paletas restantes y
consumidores legacy todavía no cubiertos.

Corrección #326/#329-NEWGRF-CACHE-FINGERPRINT (2026-09-16, `725950a1`): la
resolución y la materialización de las cuatro cachés planas se separan para
que Action2 se evalúe una sola vez y el fingerprint se calcule con el estado
que dejó esa resolución. Se añade `last_result` (`var 1C`) a la clave junto con
los registros que ya se conservaban; así dos contextos que seleccionan
variantes distintas no comparten una textura por accidente. Estación reutiliza
la vista resuelta también para sus offsets y evita la segunda evaluación del
overlay. Validación completa: cliente `1550 passed; 2 ignored`, Clippy
estricto, formato y `git diff --check` verdes. #326/#329/#567 permanecen
abiertas por layouts, paletas, callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-REUSE-RESOLVED-VIEW (2026-09-16, `779f64c5`): las
rutas planas de casa, industria y objeto pasan la vista Action2 ya resuelta a
sus cachés, en lugar de ejecutar el grafo una vez para la geometría y otra vez
para la textura. Así la posición NFO, los bytes RGBA y la clave fingerprint
pertenecen a la misma selección, incluso con procedimientos, `var 1C` o
registros `STO`. Validación: 102 pruebas NewGRF y cliente completo
`1550 passed; 2 ignored`, Clippy estricto, formato y `git diff --check` verdes.
#326/#329/#567 siguen abiertas por las familias de layouts, paletas, callbacks
y consumidores NewGRF todavía no cubiertas.

Corrección #326/#329-NEWGRF-FLAT-FINGERPRINT-CALLS (2026-09-16, `5ac3c30d`):
road stops y aeropuertos capturan el fingerprint de sus vistas planas después
de ejecutar Action2. La ruta de road stop mantiene la huella previa sólo para
la secuencia TileSeq; si cae al fallback plano, recalcula con el estado final
de esa vista. Validación: 34 pruebas de road stop, 4 de aeropuerto y cliente
completo `1550 passed; 2 ignored`, con Clippy estricto, formato y
`git diff --check` verdes. #326/#329/#567 siguen abiertas por layouts,
paletas, callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-AIRPORT-FLAT-FRAME-CACHE (2026-09-16,
`aa810a3d`): la vista plana de `AirportTile` añade `m7` a la variante de
caché, incluso cuando el catálogo sólo tiene vistas Action1/3 estáticas; para
runtime combina ese frame explícito con el fingerprint posterior a Action2.
El namespace de la vista plana se separa del de layouts `TileSeq`, evitando
colisiones entre un `gfx` usado como slot plano y otro usado como bloque de
layout. La regresión `airport_flat_cache_keeps_static_animation_frames_separate`
reproduce dos frames con píxeles distintos y exige handles distintos.
Validación: cliente `1551 passed; 2 ignored`, Clippy del binario, formato y
`git diff --check` verdes. #326/#329/#567 permanecen abiertas por layouts,
paletas, callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-ROADSTOP-SCOPE-FINGERPRINT (2026-09-16,
`a50e729c`): el dominio `RoadStop` deja de omitir `45`, `46`, `47`, `F0` y
`FA` al calcular la identidad de una textura runtime. Son valores directos del
scope para zona/distancia al pueblo, compañía, facilidades y fecha; omitirlos
permitía que paradas con el mismo `gfx` compartieran un handle pese a que su
Action2 devolviera píxeles distintos. La regresión
`road_stop_scope_variables_invalidate_fingerprint` cubre cada variable y la
suite dirigida conserva 35 pruebas de road stop; cliente completo:
`1552 passed; 2 ignored`, Clippy estricto, formato y `git diff --check`
verdes. El bloque no cierra #326/#329/#567: siguen pendientes layouts,
paletas, callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-STATION-SCOPE-FINGERPRINT (2026-09-16,
`295bef63`): `Station` deja de omitir en la identidad de sus vistas runtime
las variables directas `41`, `45`, `46`, `47`, `48`, `49`, `82`, `84`, `86`,
`8A`, `F0`, `F1`, `F2`, `F3`, `F6`, `F7` y `FA`. Esas variables representan
zona/distancia y compañía, aceptación, estado e historial de la estación,
facilidades, aeropuerto y fecha; omitirlas permitía compartir un handle entre
contextos cuyo Action2 podía producir píxeles distintos. La regresión
`station_scope_variables_invalidate_fingerprint` cubre los 17 cambios y la
suite completa del cliente pasa `1553 passed; 2 ignored`, junto con Clippy
estricto, formato y `git diff --check`. El bloque no cierra #326/#329/#567:
siguen pendientes layouts, paletas, callbacks y consumidores NewGRF restantes.

Corrección #326/#329-NEWGRF-OBJECT-SCOPE-FINGERPRINT (2026-09-16,
`0eebce78`): `Object` deja de omitir en la identidad de sus vistas runtime las
variables directas `42`, `47` y `48`, correspondientes a fecha de construcción,
color y vista de la instancia. La regresión
`object_scope_variables_invalidate_fingerprint` cambia cada una y exige un
fingerprint distinto; el caso de fecha reproduce la colisión que no quedaba
cubierta por la clave de color/vista. La suite completa del cliente pasa
`1554 passed; 2 ignored`, junto con 55 pruebas dirigidas de Object, Clippy
estricto, formato y `git diff --check`. El bloque no cierra #326/#329/#567:
siguen pendientes layouts, paletas, callbacks y consumidores NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-ANIMATION-SOUNDS — sonidos espaciales de AirportTile

Actualizado: 2026-09-16 (`e0e466d6`). `AirportTile` conserva los sonidos que
devuelven CB152 al disparar una animación y CB153 al seleccionar el siguiente
frame, incluyendo la coordenada de origen para reproducirlos como sonidos
ambientales de tesela. La cola se propaga por construcción, `NewCargo`,
`CargoTaken`, `AcceptanceTick` y el scheduler de `TileLoop`; las animaciones
de estación aplican el mismo origen espacial. CB154 sólo regula la cadencia y
descarta deliberadamente sus bits 8..14, tal como el flujo upstream. Las
regresiones cubren origen, clasificación ambiental y el descarte de sonido de
speed callback. El bloque no cierra #326/#329/#567: continúan pendientes
foundations/rotaciones del compositor, paletas base/custom y scopes completos.

### #326/#329-NEWGRF-AIRPORT-RELATIVE-ORIGIN — origen de `AirportTile`

Actualizado: 2026-09-16 (`9c0b49c6`). La estación conserva por separado el
origen nativo `Station::airport.tile`, porque `Station::pos` puede apuntar al
hangar/ancla de otra facilidad en una terminal intermodal. Construcción,
exportación/importación STNN y `AirportTileScopeResolver` usan ahora el mismo
origen, por lo que `var 0x43` mantiene la posición relativa del layout al
guardar y cargar. El fallback a `pos` se conserva para JSON/fixtures antiguos.
El alcance no cierra #326/#329/#567: quedan foundations y rotaciones del
compositor, paletas especiales y consumidores/callbacks NewGRF no cubiertos.

### #326/#329-NEWGRF-AIRPORT-REHYDRATE-ORIGIN — rehidratación desde SAV

Actualizado: 2026-09-16 (`3df42a2e`). Al reatachar `AirportTile` desde un SAV,
un `Station::airport.tile` presente se valida y se usa directamente como
origen; la inferencia por huella sólo se conserva para representaciones legacy
que no tienen ese campo. Un origen nativo incompatible ya no se reemplaza
silenciosamente por otro anclaje que coincida con las coordenadas visibles. La
regresión `explicit_airport_origin_does_not_reinfer_a_shifted_layout` fija la
frontera y mantiene abiertos foundations/rotaciones del compositor, paletas y
callbacks restantes.

### #326/#329-NEWGRF-AIRPORT-NEARBY-TYPES — tipos falsos de `0x60`

Actualizado: 2026-09-16 (`a2964c14`). La información de tesela vecina del
scope `AirportTile` conserva ahora las dos conversiones de
`GetNearbyTileInformation`: árboles de costa se exponen como `MP_WATER` y
waypoints viales como `MP_ROAD`. La regresión cubre ambos casos; permanecen
abiertos la composición raster, foundations/rotaciones y los scopes NewGRF no
modelados.

### #326/#329-NEWGRF-AIRPORT-NEARBY-Z — unidades de altura de `0x60`

Actualizado: 2026-09-16 (`6fc377b8`). `AirportTile` conserva ahora la
codificación nativa de `GetNearbyTileInformation`: GRF v7 y anteriores reciben
la altura vecina en píxeles, mientras que GRF v8+ recibe niveles de tesela. La
regresión `airport_nearby_land_info_respects_grf_version_z_units` verifica
ambas ramas junto con los bits de tipo y pertenencia al aeropuerto. La fila
continúa parcial por foundations/rotaciones del compositor, paletas y callbacks
restantes.

### #326/#329-NEWGRF-AIRPORT-TROPIC-ZONE — terreno tropical de `AirportTile`

Actualizado: 2026-09-16 (`a225cefe`). `AirportTile` resuelve el terreno
tropical desde `TropicZone` en los bits bajos de `MAPT`, como
`GetTerrainType` upstream. `MAP7` queda reservado al frame de animación y deja
de producir falsos desiertos; la construcción y limpieza de estaciones/
aeropuertos conserva el nibble al escribir `MP_STATION`. Las regresiones cubren
selva, frame animado y colocación sobre árbol; la suite core queda en
`2813 passed; 1 ignored`, con Clippy estricto. La fila sigue parcial por
foundations/rotaciones del compositor, paletas y delegación completa de
`StationScope`.

### #326/#329-NEWGRF-AIRPORT-ARCTIC-TERRAIN — línea de nieve de `AirportTile`

Actualizado: 2026-09-16 (`ed991210`). `AirportTile` deja de asumir nieve
global en `Climate::SubArctic`. La variante map-aware de `GetTerrainType`
compara `GetTileMaxZ` para station/airport/house/industry, `GetTileZ` para
water/void y reproduce los bits de nieve, densidad y ground de
clear/rail/road/trees. Las APIs sin estado siguen usando
`DEF_SNOW_LINE_HEIGHT`; el renderer de Bevy pasa la línea persistida del
`TileRenderContext` a los tres consumidores visuales: layout, vista plana y
callback de foundation. Las regresiones cubren altura máxima, comparación
estricta y formatos nativos de suelo; core queda en `2815 passed; 1 ignored`,
las 33 pruebas dirigidas del cliente y Clippy estricto pasan. Los callbacks
del scheduler que todavía entran por wrappers legacy se mantienen como una
subbrecha separada; foundations/rotaciones completas, paletas y scopes
NewGRF restantes mantienen #326/#329/#567 abiertos.

### #326/#329-NEWGRF-AIRPORT-ANIMATION-SNOW-LINE — propagación runtime

Actualizado: 2026-09-16 (`233569e5`). La línea de nieve persistida de
`GameState` llega ahora a los contextos `AirportTile` usados por `TileLoop`,
avance periódico, construcción y triggers de `NewCargo`, `CargoTaken` y
`AcceptanceTick`, incluidos los caminos con RNG global y sonidos. Las APIs
legacy siguen usando `DEF_SNOW_LINE_HEIGHT`; las variantes nuevas pasan el
valor efectivo a `0x41` y `0x60`. La regresión
`airport_animation_scheduler_uses_effective_snow_line` comprueba que CB152 y
CB153 observan líneas distintas; core queda en `2816 passed; 1 ignored`, con
Clippy estricto, compilación de cliente y `git diff --check` verdes. Este
bloque no cierra #326/#329/#567: foundations/rotaciones exhaustivas, paletas y
otros callbacks/consumidores NewGRF siguen pendientes.

### #326/#329-NEWGRF-CANAL-RESOLVER-SPLIT — depósito naval y bordes de agua

Actualizado: 2026-09-16 (`59261ba9`). La ruta de agua conserva primero el set
Action1 que resuelve `GetCanalSprite` y aplica después el desplazamiento de
`GetCanalSpriteOffset` en un contexto independiente. Esto evita que callbacks
con procedures, `last_result` o registros temporales cambien el set base de
`CF_WATERSLOPE`, `CF_DIKES` o `CF_RIVER_EDGE`; el depósito naval comparte esa
ruta con `DrawWaterClassGround`. La regresión
`canal_offset_callback_does_not_change_base_action1_set` cubre el caso; pasan
26 pruebas de depósito naval, 22 de agua, 3 de `canal_spec` y Clippy estricto.
La corrección es una subbrecha de #326/#329/#567 y no cierra ninguna de esas
issues: permanecen pendientes layouts, paletas, callbacks y consumidores
NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-TILE-OVERRIDES — traducción persistida de vecinos

Actualizado: 2026-09-16 (`28c171f4`, `93751d90`, `371031db`). `AirportTile`
aplica ahora `GetTranslatedAirportTileID` antes de devolver `0x62` para una
tesela vecina: `m5` puede conservar el `subst` vanilla, pero el scope recibe el
gfx global y el id local del override NewGRF correcto. La misma tabla llega a
los callbacks de animación por eventos, `TileLoop`, avance periódico y al
renderer Bevy para layout, sprite plano y foundation. Se conserva el fallback
legacy vacío para previews y callers sin `GameState`; la regresión
`airport_context_translates_vanilla_neighbour_with_tile_override` y los tests
dirigidos de animación/render pasan. La fila sigue parcial y #326/#329/#567
permanecen abiertas por layouts/rotaciones completas, paletas y callbacks o
consumidores NewGRF todavía no cubiertos.

### #326/#329-NEWGRF-AIRPORT-TREE-SHORE-TYPE — tipo efectivo de árboles vecinos

Actualizado: 2026-09-16 (`06dde409`). `AirportTile 0x60` reproduce ahora la
regla común de OpenTTD: sólo un `MP_TREES` cuyo `GetTreeGround()` sea
`TREE_GROUND_SHORE` se expone como `MP_WATER`. La clase de agua persistida por
sí sola no convierte un bosque normal en costa. El bit agua/costa del byte de
terreno se calcula desde el tipo efectivo, así que la costa arbórea queda
codificada igual en `0x60`. La regresión cubre suelo normal y orilla; pasan 81
tests AirportTile y Clippy estricto. La fila continúa parcial y #326/#329/#567
siguen abiertas por layouts/rotaciones, paletas y callbacks o consumidores
NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-FACILITIES-SCOPE — máscara padre persistida

Actualizado: 2026-09-16 (`64256c24`). El `AirportScopeResolver` que alimenta
Action2 de `AirportTile` devuelve ahora `BaseStation::facilities` persistido en
`0xF0`, conservando combinaciones como Airport + Train en estaciones importadas.
Cuando el campo es cero, `Station::effective_facilities()` mantiene el fallback
legacy derivado de `StopKind`. La regresión usa una máscara `0x09` y la batería
AirportTile junto con Clippy estricto pasa. La fila sigue parcial y #326/#329/#567
permanecen abiertas por layouts/rotaciones, paletas y callbacks o consumidores
NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-FACILITIES-FILTERS — estaciones intermodales

Actualizado: 2026-09-16 (`5decc184`). La entrada al contexto y las consultas
vecinas de `AirportTile` usan `Station::effective_facilities()` para decidir
si existe una facilidad aérea. Así una estación importada cuyo `StopKind`
principal sea ferroviario, pero cuya máscara `BaseStation::facilities` incluya
`FACIL_AIRPORT`, conserva sus frames y sus respuestas `0x60`/`0x62`. La
regresión Train + Airport (`0x09`) pasa junto con los 81 tests AirportTile y
Clippy estricto. La fila continúa parcial y #326/#329/#567 siguen abiertas por
layouts/rotaciones, paletas, callbacks y consumidores NewGRF restantes.

### #326/#329-NEWGRF-AIRPORT-ORPHAN-CHILD-OFFSET — child sin parent

Actualizado: 2026-09-16 (`94e7bfaa`). El compositor Bevy de `AirportTile`
trata ahora una entrada `TileSeq` child previa al primer parent como
`DrawGroundSprite`, aplicando `origin.x/y` como offsets de pantalla firmados y
usando la profundidad del pase ground. Cuando hay una foundation, la entrada
se cuelga del parent de foundation, igual que el camino nativo; los parents y
children posteriores mantienen su relación sortable. La regresión de cobertura
combina child huérfano, parent y child normal; la batería dirigida y Clippy
estricto pasan. Este bloque no cierra #326/#329/#567: continúan pendientes
otros productores/layouts, rotaciones, paletas, callbacks y consumidores
NewGRF.

### #326/#329-NEWGRF-TILE-LAYOUT-ORPHAN-CHILD — productores compartidos

Actualizado: 2026-09-16 (`68077824`). La conversión de un child `TileLayout`
que aparece antes de su parent se centraliza en el renderer Bevy y se aplica a
`Station`, `RoadStop`/`RoadWaypoint` y `Object`, además de la ruta AirportTile
ya corregida. `origin.x/y` se interpreta como offset screen-space firmado para
`DrawGroundSprite`; en terreno inclinado el sprite se asocia al parent de
foundation, mientras que parents y children posteriores conservan el sorter
global. La regresión de objeto comprueba coordenadas y depth; pasan 22 pruebas
dirigidas de TileLayout, 4 de road stop, 3 de waypoint y Clippy estricto. La
fila sigue parcial y #326/#329/#567 permanecen abiertas por layouts restantes,
foundations/rotaciones exhaustivas, paletas, callbacks y consumidores NewGRF.

### #326/#329-NEWGRF-TILE-LAYOUT-CONSTRUCTION-STAGE — etapas de construcción

Actualizado: 2026-09-16 (`3df1fe1c`). La resolución runtime de casas e
industrias ya no descarta el `stage` que calcula el mapa. Cada sprite y paleta
Action1 de un `TileLayout` recibe la selección nativa
`GetConstructionStageOffset`, limitada a cuatro entradas y con reutilización
para sets de 1/2 sprites; un offset registrado por `TLF_SPRITE`/`TLF_PALETTE`
mantiene precedencia. Las regresiones de sets de 1–4 entradas cubren las
etapas 0–3, y las baterías core/cliente dirigidas pasan con Clippy estricto.
La fila sigue **parcial runtime** y #326/#329/#567 permanecen abiertas por
otros layouts, foundations/rotaciones, paletas, delegación StationScope y
callbacks/consumidores NewGRF restantes.

### #326/#329-NEWGRF-TILE-LAYOUT-CHILD-OFFSET-SIGNEDNESS — bytes de offsets

Actualizado: 2026-09-16 (`16e4c7ee`). El renderer distingue los dos contratos
de `DrawCommonTileSeq`: `DrawNewGRFTileSeq` reinterpreta los offsets de child
como `uint8_t`, y `DrawRailTileSeq` como `int8_t`. La ruta unsigned cubre
casas, industrias, objetos y `AirportTile`; estaciones, road stops y waypoints
conservan offsets firmados. La regresión con el byte crudo `0xFC` valida `252`,
y pasan 23 pruebas dirigidas de TileLayout con Clippy estricto. La fila sigue
parcial y #326/#329/#567 permanecen abiertas por layouts, foundations/
rotaciones exhaustivas, paletas, callbacks y consumidores NewGRF restantes.

### #326/#329-NEWGRF-TILE-LAYOUT-PALETTE-MODIFIER-GUARD — consumo de paletas

Actualizado: 2026-09-16 (`5580d7b9`). El resolver común copia ahora la
decisión de `SpriteLayoutPaletteTransform`: una paleta directa se usa sólo si
el sprite trae `TRANSPARENT` o `RECOLOUR`; para `GroundSpritePaletteTransform`
sólo `RECOLOUR` habilita la paleta. Esto corrige la aplicación espuria de
paletas de compañía, crash, newspaper, bare-land y estructura, y permite que
un sprite base con paleta inactiva siga siendo válido, en lugar de colorearse o
caer al fallback. La caché Bevy mantiene el mismo guard para layouts ya
resueltos. Pasan 29 pruebas de core, 14 de imagen y Clippy estricto. La fila
continúa parcial: la composición destino de transparencia, otros layouts,
foundations/rotaciones, callbacks y consumidores NewGRF siguen pendientes y
#326/#329/#567 permanecen abiertas.

### #326/#329-NEWGRF-TILE-LAYOUT-GROUND-CATEGORY-ALPHA — suelo fuera de `to`

Actualizado: 2026-09-16 (`6d66e7f7`). Los productores de suelo de `TileLayout`
comparten ahora una función que parte de color blanco y conserva sólo la
paleta/modifiers de `GroundSpritePaletteTransform`; no heredan el alpha de
Buildings, Industries o Structures. Esto cubre casas, industrias, objetos,
estaciones, road stops/waypoints y aeropuertos. También se retiró el alpha de
categoría que se aplicaba por error al suelo de industria vanilla y a las capas
ground de `StationGfx` de aeropuertos. La secuencia `BUILD` mantiene sus filtros
de transparencia/invisibilidad y sus paletas. Pasan 25 pruebas dirigidas de
TileLayout y Clippy estricto. La fila sigue parcial y #326/#329/#567 permanecen
abiertas por composición destino, layouts restantes, foundations/rotaciones,
callbacks y consumidores NewGRF.

### #326/#329-NEWGRF-INDUSTRY-ANIMATED-TRANSPARENCY — alpha de capas animadas

Actualizado: 2026-09-16 (`5f2408b0`). `IndustryBuildingAnim` separa ahora el
color de suelo del color de BUILD: `TO_INDUSTRIES` sólo vuelve transparente la
capa animada del edificio, mientras el suelo conserva alpha 1 como en
`DrawGroundSprite`. El color se actualiza tanto al crear la entidad como al
cambiar frame o preferencia. Pasan 6 pruebas dirigidas de `industry_anim` y
Clippy estricto. La fila sigue parcial y #326/#329/#567 permanecen abiertas
por la composición global, otros layouts, foundations/rotaciones, callbacks y
consumidores NewGRF.

### #326/#329-NEWGRF-TILE-LAYOUT-DESTINATION-TRANSPARENCY — máscara de BUILD

Actualizado: 2026-09-16 (`b3615d7b`). Las secuencias `BUILD` de casas,
industrias, objetos, estaciones, road stops/waypoints y aeropuertos reproducen
ahora la decisión de `DrawCommonTileSeq`: cuando la categoría está en modo
transparente, la imagen efectiva recibe `PALETTE_MODIFIER_TRANSPARENT` y
`PALETTE_TO_TRANSPARENT` (`802`), en lugar de multiplicar el sprite por el
alpha de categoría. Las imágenes Action1 hornean la máscara negra con
obertura `64/255`; las referencias directas del atlas usan el mismo
multiplicador. `OPAQUE` conserva precedencia y el `ground` sigue fuera de la
composición de categoría. Pasan 17 pruebas de fábrica, 27 regresiones de
TileLayout, 29 del core y Clippy estricto. La fila continúa parcial y
#326/#329/#567 permanecen abiertas por layouts restantes,
foundations/rotaciones, callbacks y consumidores NewGRF.

### #326/#329/#567-SHIP-DEPOT-DESTINATION-TRANSPARENCY — máscara de estructura

Actualizado: 2026-09-16 (`b7ed43ea`, ajuste de traza `6c5b22d7`). La estructura vanilla de
`ShipDepot` sigue ahora el camino nativo de `DrawWaterTileStruct`: cuando
`TO_BUILDINGS` está transparente, las seis capas `TILE_SEQ_LINE` se
materializan con la máscara de destino equivalente a
`PALETTE_TO_TRANSPARENT` (`802`), en lugar del alpha genérico de categoría.
El ground de agua, pendientes, diques y bordes queda fuera de la máscara, y el
modo oculto continúa eliminando sólo las capas estructurales. La regresión
específica pasa con 27 pruebas; TileLayout y Clippy estricto también quedan
verdes. La fila sigue parcial y no se cierran #326/#329/#567: faltan YAPF y
regiones de agua completas, callbacks NewGRF, composición global y framebuffer.

### #326/#329/#567-DEPOT-BUILD-DESTINATION-TRANSPARENCY — fachadas rail/road

Actualizado: 2026-09-16 (`2bc07e00`). Las fachadas BUILD de depósitos
ferroviarios y viales usan ahora la máscara de destino equivalente a
`PALETTE_TO_TRANSPARENT` (`802`) cuando `TO_BUILDINGS` está transparente.
El cambio vive en los parents compartidos de cada depósito, por lo que cubre
sprites vanilla, grupos NewGRF y sustituciones Action5 de tranvía sin cambiar
el ground, los overlays ni la catenaria. Pasan 100 regresiones focalizadas y
Clippy estricto. La fila sigue parcial: otros producers BUILD,
callbacks/consumidores NewGRF, composición global y framebuffer continúan
pendientes; #326/#329/#567 siguen abiertas.

### #326/#329/#567-STATION-AIRPORT-BUILD-DESTINATION-TRANSPARENCY — capas BUILD

Actualizado: 2026-09-16 (`31aed2db`). Las capas vanilla de estación
ferroviaria/waypoint, muelle y boya, las capas `StationGfx` aeroportuarias y
el fallback plano de `AirportTile` usan la máscara de destino 802 cuando
`TO_BUILDINGS` está transparente. Se conserva aparte el techo de vidrio con
su calibración propia; suelos, overlays y catenaria siguen opacos. Pasan 142
pruebas filtradas por `station` y Clippy estricto. La fila sigue parcial y
#326/#329/#567 permanecen abiertas por layouts, callbacks, consumidores
NewGRF, composición global y framebuffer.

### #326/#329/#567-PALETTE-EXACT-COUNTER — fase nativa compartida

Actualizado: 2026-09-16 (`8424a54a`). El `PaletteAnimationClock` conserva el
contador `u16` que OpenTTD incrementa `+8` en cada pasada elegible de
`DoPaletteAnimations`; ya no transforma un tiempo de pared aproximado en
ticks. Agua, fuego de refinería, `fizzy_drink`, faro/estadio y radio comparten
las fases directas de `EXTR`/`EXTR2`, con truncación y ciclo inverso nativos.
Pasan 569 pruebas render ejecutables, 1 ignorada, 1.577 tests del cliente, 2
ignorados y Clippy estricto del binario. La fila sigue parcial y
#326/#329/#567 permanecen abiertas por otros ciclos/producers,
callbacks/consumidores NewGRF, composición global y comparación de framebuffer.

### #326/#329/#567-ROAD-STOP-BUILD-DESTINATION-TRANSPARENCY — paradas y waypoints

Actualizado: 2026-09-16 (`4320fe6e`). El parent común de las capas BUILD de
paradas bus/camión cubre ahora las variantes vanilla, drive-through y Action5
con la máscara de destino 802 cuando `TO_BUILDINGS` está transparente. La
vista plana custom y los children de waypoint conservan el mismo contrato;
los `TileLayout` siguen decidiendo por entrada mediante `OPAQUE`. Pasan 35
pruebas de `road_stop`, 16 de `road_waypoint` y Clippy estricto. La fila sigue
parcial y #326/#329/#567 permanecen abiertas por callbacks/consumidores
NewGRF, composición global y framebuffer.

### #326/#329/#567-DIRECT-STATION-NEWGRF-BUILD-TRANSPARENCY — vista plana

Actualizado: 2026-09-16 (`7d35fc72`). La vista directa de estación ferroviaria
NewGRF, que sustituye la secuencia final de `DrawRailTileSeq`, usa la máscara
de destino 802 en modo transparente. No modifica la decisión por entrada de
`TileLayout` ni la calibración del techo de vidrio. Pasan las 142 pruebas de
`station`, Clippy estricto y formato; la fila sigue parcial y #326/#329/#567
continúan abiertas.

### #326/#329/#567-BRIDGE-STRUCTURE-DESTINATION-TRANSPARENCY — puentes y objetos

Actualizado: 2026-09-16 (`eb105e5f`). Las piezas estructurales vanilla y
custom de puentes (tablero, barandilla, rampas, pilares y capas directas
NewGRF) usan la máscara de destino 802 cuando `TO_BRIDGES` está transparente.
El HQ, los hitos vanilla y las vistas planas de objetos NewGRF hacen lo mismo
para `TO_STRUCTURES`; los `TileLayout` conservan su decisión por entrada y la
precedencia de `OPAQUE`. Suelo, fundaciones y catenaria no heredan estas
categorías. Pasan 78 pruebas de puentes, 2 de estructuras, Clippy estricto y
formato. La fila continúa parcial y #326/#329/#567 permanecen abiertas por
otros producers BUILD, callbacks/consumidores NewGRF, composición global y
framebuffer.

### #326/#329/#567-HOUSE-INDUSTRY-DESTINATION-TRANSPARENCY — edificios planos y animados

Actualizado: 2026-09-16 (`811d453c`). Las capas BUILD vanilla y las vistas
directas NewGRF de casas e industrias aplican la máscara de destino 802 cuando
`TO_HOUSES`/`TO_INDUSTRIES` están transparentes; no usan el alpha blanco
genérico. El suelo y las fundaciones permanecen opacos. En casas se corta el
ascensor después de un edificio transparente; en industrias se corta el
`draw_proc`, y las entidades animadas recalculan la máscara en cada frame y al
cambiar preferencias. Pasan 85 pruebas dirigidas y Clippy estricto. La fila
continúa parcial: quedan efectos de industria, otros producers BUILD,
callbacks/consumidores NewGRF, composición global y framebuffer.

### #326/#329/#567-INDUSTRY-EFFECT-VISIBILITY — humo y burbujas

Actualizado: 2026-09-16 (`717c8587`). Los efectos equivalentes a
`EV_CHIMNEY_SMOKE`, `EV_COPPER_MINE_SMOKE` y `EV_BUBBLE` se ocultan en modo
transparente y oculto de `TO_INDUSTRIES`, igual que `DoDrawVehicle`; no reciben
la máscara 802 ni un alpha de edificio. Las entidades conservan su estado y
animación mientras están ocultas, para reaparecer correctamente al restablecer
la categoría. Pasan 46 pruebas focalizadas y Clippy estricto. La fila sigue
parcial por otros efectos/producers, callbacks/consumidores NewGRF,
composición global y framebuffer.

### #326/#329/#567-DIRECT-DEPOT-ROADSTOP-BUILD-TRANSPARENCY — fachadas directas

Actualizado: 2026-09-16 (`899dcc08`). Las fachadas BUILD directas de paradas
bus/camión y depósitos rail/road, incluidas vistas NewGRF y reemplazos Action5,
usan la máscara de destino 802 bajo `TO_BUILDINGS` en lugar de alpha blanco.
Los `TileLayout` conservan su color intermedio hasta resolver `OPAQUE` y la
paleta por entrada. Pasan 135 pruebas focalizadas y Clippy estricto. La fila
continúa parcial por otros producers BUILD, callbacks/consumidores NewGRF,
composición global y framebuffer.

### #326/#329/#567-TREE-CANOPY-DESTINATION-TRANSPARENCY — copas del bosque

Actualizado: 2026-09-16 (`2b5cc62f`). Las copas de `DrawTile_Trees` usan la
máscara de destino 802 cuando `TO_TREES` está transparente; el suelo y la
media altura de pendiente permanecen fuera de la categoría. Las capas
`AddCombinedSprite` conservan la decisión del parent sortable. Pasan 6
regresiones de árboles/capas combinadas y Clippy estricto. La fila continúa
parcial por otros producers, efectos, callbacks/consumidores NewGRF,
composición global y framebuffer.

### #326/#329/#567-ROADSIDE-DETAIL-DESTINATION-TRANSPARENCY — detalles de calle

Actualizado: 2026-09-16 (`c490f712`). `DrawRoadDetail` asigna las categorías
nativas por productor: los faroles usan `TO_HOUSES` y los árboles laterales
`TO_TREES`. La transparencia usa la máscara de destino 802 y la invisibilidad
omite el parent sortable; el suelo, las obras, los overlays y la catenaria
siguen independientes. Pasan 31 pruebas de transporte y Clippy estricto. La
fila continúa parcial y #326/#329/#567 permanecen abiertas por otros
producers, efectos, callbacks/consumidores NewGRF, composición global y
framebuffer.

### #326/#329/#567-LOCK-STRUCTURE-DESTINATION-TRANSPARENCY — estructuras de esclusa

Actualizado: 2026-09-16 (`0256f78b`). Las piezas BUILD de `DrawWaterLock`
respetan `TO_BUILDINGS`: custom, Action5 y fallback vanilla reciben destino
802 en modo transparente y se omiten en modo oculto. El fallback compuesto se
separa sólo cuando hace falta para que el agua ground siga visible y animada;
diques, bordes y superficies no heredan el bit. Pasan 24 pruebas focalizadas y
Clippy estricto. La fila continúa parcial y #326/#329/#567 permanecen abiertas
por otros producers, efectos, callbacks/consumidores NewGRF, composición global
y framebuffer.

### #326/#329/#567-CATENARY-DESTINATION-TRANSPARENCY — rail, carretera y puentes

Actualizado: 2026-09-16 (`a3c14603`). Los sprites de catenaria de rail,
carretera, estaciones, depósitos, túneles y puentes usan destino 802 cuando
`TO_CATENARY` está transparente: negro con cobertura `64/255`, no blanco con
alpha `0,45`. Las vistas custom `ROTSG_CATENARY_BACK/FRONT` de carretera en
puentes se normalizan con el mismo color antes de entrar al parent/child
sortable. Las superficies, overlays y el modo oculto permanecen separados.
Pasan 64 pruebas filtradas de catenaria y Clippy estricto. La fila sigue
parcial y #326/#329/#567 continúan abiertas por composición global,
callbacks/consumidores NewGRF y comparación de framebuffer.

### #326/#329/#567-SIGN-STATION-LABEL-TRANSPARENCY — carteles y estaciones

Actualizado: 2026-09-16 (`c8c7fd87`). Los carteles de usuario aplican la
semántica nativa de `TO_SIGNS`: en transparente el marco usa destino 802 y el
texto permanece blanco opaco; los carteles de GameScript continúan sin marco.
Las estaciones y waypoints transparentes no crean panel y usan el tono claro
`SHADE_LIGHTER` de la rampa de compañía, con gris claro para estaciones sin
propietario o sin facilities. La invisibilidad sigue afectando sólo a los
carteles de usuario, mientras las etiquetas de estación conservan su flujo
nativo separado. Pasan 3 pruebas de carteles, 6 de estaciones y Clippy
estricto. La fila continúa parcial y #326/#329/#567 permanecen abiertas por
otros producers, callbacks/consumidores NewGRF, composición global y
framebuffer.

### #326/#329/#567-TEXT-EFFECT-VISIBILITY — popups y texto de diagnóstico

Actualizado: 2026-09-16 (`8b92e5ab`). Los popups de ingresos y las etiquetas de
carga de diagnóstico siguen el contrato de `DrawTextEffects`: `TO_TEXT`
transparente u oculto suprime el dibujo completo, sin alpha blanco. El ciclo
de vida, la posición y el avance de la animación continúan mientras la entidad
está oculta, para reaparecer sin reinicio al volver a visible. Pasan 1
regresión de modos, 3 de popups, 3 de etiquetas de vehículos y Clippy estricto.
La fila continúa parcial y #326/#329/#567 permanecen abiertas por otros
efectos de viewport, callbacks/consumidores NewGRF, composición global y
framebuffer.

### #326/#329/#567-BRIDGE-RAMP-MIDDLE-TRANSPARENCY — rampas y vano

Actualizado: 2026-09-16 (`70c8a2c5`). Las cabeceras y rampas de puente no se
ocultan ni reciben máscara por `TO_BRIDGES`, igual que el camino nativo de
OpenTTD; sólo el vano intermedio usa esa categoría. Deck de carretera/tranvía,
overlays Action5/custom y reserva PBS reciben destino 802 en el vano. La
catenaria conserva su bit `TO_CATENARY` independiente. Pasan 79 pruebas de
puente y la regresión específica de rampas/colores. La fila continúa parcial y
#326/#329/#567 permanecen abiertas por otros producers, callbacks/consumidores
NewGRF, composición global y comparación de framebuffer.

### #326/#329/#567-DYNAMIC-VISUAL-REMAP — entidades transitorias

Actualizado: 2026-09-16 (`8f325966`). Los remaps de representación conservan
vehículos, OVNIs, FX, humo de vehículos, burbujas, popups de ingresos y
destellos de construcción cuando el mapa sigue en detalle; una carga/cambio de
mapa o el overview los limpia explícitamente. El spawn inicial de vehículos se
desactiva durante un rebuild que ya tiene una flota viva, evitando duplicados;
al regresar desde overview se vuelve a materializar una sola vez. Las
animaciones dependientes de una tesela siguen dentro del rebuild. Pasan 1.575
tests del cliente, 11 tests de `world` y Clippy estricto del binario. La fila
continúa parcial y #326/#329/#567 permanecen abiertas por otros producers,
efectos, callbacks/consumidores NewGRF, composición global y framebuffer.

### #326/#329/#567-PALETTE-REAL-CLOCK — agua y faro/estadio

Actualizado: 2026-09-16 (`8cd4cbd2`). Los ciclos de agua y faro/estadio pasan
a `Time<Real>`, el reloj de presentación apropiado para `DoPaletteAnimations`;
la velocidad de la simulación ya no modifica su cadencia. La condición de
animación completa continúa respetando pausa y preferencia. Pasan 2 pruebas del
faro, 5 del agua y Clippy estricto. La fila continúa parcial y #326/#329/#567
permanecen abiertas: falta modelar el contador nativo durante pausas y quedan
otros ciclos, producers, callbacks/consumidores NewGRF, composición global y
framebuffer.

### #326/#329/#567-PALETTE-PAUSE-CLOCK — contador compartido

Actualizado: 2026-09-16 (`ae70e27d`). Los ciclos visuales de agua, fuego de
refinería, `fizzy_drink`, faro/estadio y radio consumen un único contador de
presentación. El contador acumula `Time<Real>` bajo el gate de animación
completa, por lo que una pausa no avanza la fase ni provoca un salto al
reanudar; cambiar la velocidad de la simulación tampoco modifica la cadencia.
Pasan 16 pruebas focalizadas, 1.576 tests del cliente, 2 ignorados y Clippy
estricto del binario. La fila continúa parcial y #326/#329/#567 permanecen
abiertas: faltan la cadencia entera exacta de OpenTTD, otros ciclos/producers,
callbacks/consumidores NewGRF, composición global y comparación de framebuffer.

### #326/#329/#567-INDUSTRY-FIZZY-REAL-CLOCK — animación de bebidas Toyland

Actualizado: 2026-09-16 (`2a99e89f`). El ciclo `fizzy_drink` se actualiza con
`Time<Real>`, porque OpenTTD lo produce en `DoPaletteAnimations` del bucle de
presentación y no en `Time<Virtual>`/la velocidad de la simulación. La prueba
del sistema usa el mismo reloj real que el fuego de refinería. Pasan 2 pruebas
focalizadas y formato estricto. La fila continúa parcial y #326/#329/#567
permanecen abiertas por los demás ciclos, producers, callbacks/consumidores
NewGRF, composición global y framebuffer.

### #326/#329/#567-TUNNEL-PORTAL-SEGMENTED — portal NewGRF por banda

Actualizado: 2026-09-16 (`c7495351`). La fachada `RTSG_TUNNEL_PORTAL` custom
que forma el `SpriteCombine` de la catenaria ferroviaria conserva ahora su
`Sprite` y `Transform` completos en `ViewportSortableSegmentedSource`. La base
Action5 y el portal pueden recortarse/promoverse de forma independiente en
las bandas de `ViewportDoDraw`, mientras el vínculo child→cable sigue siendo
atómico dentro de cada banda. La regresión NewGRF verifica dos children y dos
fuentes segmentadas; pasan 1.577 tests del cliente, 2 ignorados y Clippy
estricto del binario. La fila continúa parcial por otros combines, producers,
callbacks/consumidores NewGRF, composición global y framebuffer.

### #326/#329/#567-BRIDGE-COMBINE-SEGMENTED — children de puentes por banda

Actualizado: 2026-09-16 (`766ae4b7`, `697ea82c`). Las capas estructurales
frontales combinadas y los overlays de puente con geometría nativa verificable
ahora publican bounds, orden local y `ViewportSortableSegmentedSource`. La
corrección cubre catenaria trasera, deck Action5, reserva PBS, tranvía y la
baranda frontal bajo catenaria; el vínculo child→parent permanece atómico y
la fuente completa permite recortar sólo la banda cruzada. Pasan 74 pruebas de
puente y Clippy estricto; los grupos específicos NewGRF que todavía pasan por
`spawn_bridge_specific_child` siguen pendientes. La fila continúa parcial y
no se cierran #326/#329/#567.

### #326/#329/#567-BRIDGE-ROAD-GROUPS-SEGMENTED — grupos específicos del puente

Actualizado: 2026-09-16 (`959a57e8`). Los grupos `ROTSG_BRIDGE`,
`ROTSG_OVERLAY` y `ROTSG_CATENARY_BACK` ya comparten la caja trasera nativa de
`DrawBridgeRoadBits` y publican fuente completa, bounds y orden de combinación.
La identidad del parent promovido se deriva de RoadType, selector e índice de
vista en un rango sintético separado, porque el `DecodedSprite` no conserva el
ID NFO original. La regresión custom cubre bridge, overlay y catenaria bajo el
parent trasero; la suite completa pasa con 1.577 tests y 2 ignorados. La fila
continúa parcial y no se cierran #326/#329/#567.

### #326/#329/#567-FIELD-FENCE-COMBINE-SEGMENTED — cercas de campos por banda

Actualizado: 2026-09-16 (`8605cac0`). `DrawClearLandFence` usa un único
`StartSpriteCombine`: la primera cerca visible es el parent y las siguientes
son `AddCombinedSprite`. El renderer conserva esa relación con el prisma
base 16×16×(4+pendiente), offsets NW/NE/SW/SE y orden local; cada child
publica `ViewportSortablePromotableChild`, `ViewportSortableSegmentedChild` y
`ViewportSortableSegmentedSource` para que el clipping de `ViewportDoDraw` no
separe ni desplace globalmente la imagen completa. La regresión de campo
comprueba un parent y tres children segmentados junto con los cuatro sprites
esperados; pasan 1.577 tests del cliente, 2 ignorados y Clippy estricto. La
fila continúa parcial y no se cierran #326/#329/#567.

Actualización #326/#329/#567-VEHICLE-SPRITESTACK-COMBINE-SEGMENTED
(`76b7f9de`): el `StartSpriteCombine` de `DoDrawVehicle` queda reflejado para
las capas 1–7 de cada `VehicleSprite` o `ConsistUnitSprite`. Cada capa usa
`Vehicle::bounds`, el mismo `insertion_key` y su ordinal de secuencia, además
de `ViewportSortableSegmentedSource` con la imagen/transform completos. Las
ranuras vacías continúan ocultas y sin `PromotableChild`, incluso cuando el
callback cambia la longitud de la pila. La prueba de cabeza/trailer comprueba
la resolución compartida por parent, la fuente separada del Z ordenado y la
ausencia de cruce entre consist units; pasan 1.577 tests, 2 ignorados y Clippy
estricto. La cobertura continúa parcial y no se cierran #326/#329/#567.

Actualización #326/#329/#567-STATION-CUSTOM-FOUNDATIONS
(`8f2e49cd`, `165ff353`, 2026-09-16): los cimientos custom de estaciones
ferroviarias siguen el contrato de `DrawCustomStationFoundations`. `core`
resuelve `param1=2`, `layout | edge_info << 16`, el registro `0x100` y las
tablas nativas de piezas extendidas/compuestas; el renderer usa el prisma
completo 16×16×7 y la superficie `FOUNDATION_LEVELED`. El bloque extendido
abre un parent sortable; el bloque clásico conserva `StartSpriteCombine` con
children `Promotable`/`Segmented` y fuente completa para el clipping por banda.
La continuidad NW/NE y el fallback vanilla cuando el grupo o una pieza no se
puede materializar quedan cubiertos por tests. Pasan 1.577 tests del cliente,
2 ignorados, tests focalizados de core y Clippy estricto. La cobertura es
parcial: faltan otros callbacks/consumidores NewGRF, captura raster y la
composición global completa; no se cierran #326/#329/#567.

### #326/#329/#567-TREE-CLIMATE-LAYOUTS — tipos globales y nieve densa

Actualizado: 2026-09-16 (`72a40e20`). `DrawTile_Trees` ya no interpreta `m3`
como especie templada modular: usa el tipo global para seleccionar las filas
árticas, rainforest, cactus, subtropicales y toyland de `tree_land.h`. El
extractor porta `1576..2009`, 196 filas y los 32 reemplazos que se activan con
densidad ≥2 sobre `SnowOrDesert`/`RoughSnow`; el atlas distribuido se regenera
junto con sus metadatos. La cobertura es estructural vanilla y no implica
paridad NewGRF completa; continúan pendientes otros producers, callbacks y
la comparación raster.

### #326/#329/#567-TREE-TOYLAND-PALETTE — recolor por capa

Actualizado: 2026-09-16 (`bf71e943`). Las `PalSpriteID` de las filas toyland
se expanden a las siete etapas de crecimiento y se hornean en una caché RGBA
fuera del atlas. Cada copa conserva su paleta al pasar por el orden de
combinación; si la copia no se puede materializar, la traza marca fallback en
lugar de ocultarlo. La regresión de tabla, carga desde el atlas y spawn pasa
con 1.583 tests del cliente, 2 ignorados y Clippy estricto. `tree_391` es un
sprite 1×1 real no referenciado por la tabla vanilla. La fila sigue parcial y
no se cierran #326/#329/#567.

### #326/#329/#567-AIRPORT-TILE-PSA-SCOPES — separación de scopes

Actualizado: 2026-09-16 (`8b5eb1e6`, `0c482f3f`). `AirportTileScopeResolver`
queda representado sin almacenamiento persistente propio: `7C` self se
reporta como no disponible, cae al default de Action2 y `\2psto` ignora el
writeback. El `AirportScopeResolver` parent sigue siendo el único scope con
lectura/escritura persistente materializada. Pasan las regresiones de
`airport_tile_action2`, 2.824 tests de core, 1 ignorado y Clippy estricto.
La fila continúa parcial: queda revisar si el replay SAV requiere un PSA del
aeropuerto separado del mapa persistente compartido por la estación, además
de los demás gaps de compositor/callbacks; no se cierran las issues madre.
