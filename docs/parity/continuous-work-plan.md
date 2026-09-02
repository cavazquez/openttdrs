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

## Orden recomendado

| Orden | Bloque | Estado | Criterio de cierre |
|---:|---|---|---|
| 1 | Zoom y viewport | Completado | Seis niveles OpenTTD (`0,25×`…`0,125×`), culling/overview deterministas y smoke de render; la paridad raster global queda separada de la cobertura de zoom. |
| 2 | RMAP-004: generador procedural | Abierto P1 (RMAP-005–017, RMAP-019–023, RMAP-025–026, RMAP-028–029, RMAP-031, RMAP-033, RMAP-035–055, RMAP-057–058, RMAP-060, RMAP-063–064, RMAP-066–081 y RMAP-083–139 cerrados; RMAP-018/RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034/RMAP-056/RMAP-059/RMAP-061/RMAP-062/RMAP-065/RMAP-082 en curso) | Reducir la primera divergencia de TGP/RNG/`FixSlopes`/clear/towns/industries/objects con matriz 64²→512². RMAP-139 añade settings explícitos de ríos/bordes al comparador y deja exactas las combinaciones auditadas; esto no cierra el generador. RMAP-138 amplía el control a cuatro seeds temperate 1024² (`1330935388`–`1330935391`) y deja exactas las seis fronteras (24/24 comparaciones, 0 teselas y 0 bloques 4×4 por frontera); esto no cierra el generador. RMAP-087 completa el stream RNG de árboles de humedal tras `CreateRivers` y deja `landscape`/`clear` exactos en la cohorte temperate 512²; RMAP-088 unifica el perfil y la cola de `RunTileLoop` de Nueva partida entre cliente y oracle. RMAP-084/086 cubren las rampas de llegada e inicio inclinadas de puentes municipales, RMAP-085 reproduce el coste/clear atómico de su terraformación y RMAP-089 completa los túneles municipales y la terraformación de sus bocas; las cuatro seeds 512² de control quedan exactas hasta `towns`. RMAP-090 completa la representación de `landscape`/`clear` para Arctic, Tropic y Toyland en las cuatro seeds 64² de control, incluyendo la nieve canónica en `MAP3` y zonas tropicales en `MAPT`; RMAP-091 conserva el nibble de `TropicZone` en las calles municipales y extiende la frontera `towns` exacta a las cuatro seeds Tropic de 64²; RMAP-092 porta los gates climáticos de `CheckNewIndustry_*`, RMAP-093 completa las tablas de layout vanilla, RMAP-094 propaga la línea de nieve efectiva y la admisión de campos árticos, RMAP-095 alinea la admisión `OnlyInTown` y el reset de MAP8 de `MakeIndustry`, RMAP-096 permite costas durante la plataforma gratuita y RMAP-097 usa la línea de nieve efectiva al seleccionar casas árticas; RMAP-098 pasa el límite de altura y la línea de nieve efectivos a `GenerateTrees`, dejando las cuatro semillas Arctic 64² exactas en las seis fronteras (`landscape`→`trees`); RMAP-099/100 conservan `TropicZone` y respetan `ClearTile_Road` al materializar objetos/industrias, y RMAP-101 completa layouts Toyland y `OnlyNearTown`, dejando las cuatro semillas tropicales y cuatro Toyland 64² exactas en las seis fronteras. RMAP-102 escala el borde de refinerías por eje, RMAP-103 replica el `Execute` parcial de plataformas y RMAP-104 difiere pendientes al pase de plataforma y limpia el `gfx` alto de `MakeIndustry`; RMAP-105 verifica las seis fases completas y deja las cuatro seeds temperate 512² (`1330935378`–`1330935381`) exactas en 24/24 fronteras. RMAP-113/RMAP-114/RMAP-115/RMAP-116 cierran la primera transición de entrega del mundo: la cola `RunTileLoop`, animación inicial, árboles, casas, costas e industrias; RMAP-117 corrige la orientación de las bocas de puente/túnel en la limpieza vial municipal y deja Toyland 256² exacto en las cuatro seeds. RMAP-118 unifica el consumo del RNG global de `TileLoop_Trees` en Toyland y RMAP-119 admite las bocas de puente/túnel existentes durante `IsRoadAllowedHere`; RMAP-120 usa el `GetTileZ` mínimo para el gate de Bubble Generator en pendientes y RMAP-121 replica el despeje completo de casas multitile que `ToyShop` reemplaza mediante `GetHouseNorthPart`/`ClearTownHouse`; RMAP-123 rechaza `MP_VOID` durante `RiverMakeWider` y conserva el `RoughSnow` de 16 bits durante `TileLoopTreesAlps`; RMAP-124 hace que el preflight de puentes municipales aplique `CheckBridgeSlope` y rechace cabezas a distinto nivel efectivo; RMAP-127 replica el despeje `Auto` de la salida municipal de un túnel y rechaza bocas multibit; RMAP-128 separa los topes de puente y túnel, rechaza costas/puentes paralelos y deja exacta la frontera urbana ártica de 1024²; RMAP-129 conserva las entidades de `IndustryPool` cuando el origen de un layout cae dentro de otra huella sin superposición y deja exactas las seis fronteras de la cohorte ártica 1024²; RMAP-130 conserva la asociación de pueblo de las industrias fundadas sobre casas y deja exacta esa cohorte ártica 1024²/seed `1330935381` en las seis fases; RMAP-132 detiene el caminador municipal al entrar en una carretera de otro pueblo y deja exacta la cohorte ártica 1024²/seed `1330935383` en las seis fases; RMAP-134 hace que el preflight de puentes paralelos recorra la espiral nativa y deja exacta la cohorte tropical 1024²/seed `1330935386` en las seis fases; RMAP-135 conserva el `Chance16` de `LevelTownLand` al visitar una tesela ocupada y deja exacta towns en temperate 1024²/seed `1330935387`; RMAP-136 pasa el límite efectivo de altura a las plataformas industriales y RMAP-137 resuelve su valor dinámico para árboles y deja exactas las fronteras `industries`/`trees` de temperate 2048²/seed `1330935404`. Las seeds Toyland 512² `1330935378`–`1330935381` quedan exactas en las seis fronteras (`landscape`→`trees`), con 0 teselas y 0 bloques 4×4 distintos en cada fase auditada; las cuatro seeds árticas 512² `1330935378`–`1330935381` también quedan exactas en las seis fronteras tras RMAP-124. La matriz completa 64²→512² queda exacta en 15/15 cargas y 15/15 mapas mismo-seed para la cohorte canónica; el alcance de clima/configuración, otras semillas/tamaños y ticks posteriores sigue abierto. RMAP-082 conserva la generalización urbana fuera de la cohorte de control. La evidencia detallada y el resto de avances se mantienen únicamente en `random-map-issues.md`, para no duplicar métricas. RMAP-004 sigue abierto mientras haya divergencias en otros tamaños/fases; RMAP-018 conserva configuraciones de río y fases posteriores multiclima, y RMAP-024/RMAP-027/RMAP-030/RMAP-032/RMAP-034 la generalización de pueblos. |
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden estable. Paradas, waypoints viales, estaciones rail, objetos e industrias resuelven layouts `TileSeq` completos por Action3/2→Action1, materializan suelo, parents/children y pendientes; los aeropuertos construidos conservan ahora el `gfx` `AirportTile` por tesela y consumen su sprite Action1/3 con fallback vanilla atómico. El procesador aplica `DODRAW`, offsets de sprite/caja/child, `var10`, draw mode `0x100` e invalida la caché con registros `7D`/`0x100`. Sprites base y paletas custom siguen fallback atómico. Los callbacks `0x150` de casas y teselas de industria ya se evalúan sólo en pendientes y pueden suprimir `FOUNDATION_LEVELED`; `RTSG_DEPOT` (selector 8) ya reemplaza las seis fachadas relocatables de depósitos ferroviarios con Action2, offsets NFO y children de fundación; quedan por cubrir las variantes ferroviarias de pendiente/túnel, el compositor de foundations/rotaciones de aeropuertos y el sprite-stack/callbacks avanzados de vehículos. La animación AirportTile ya ejecuta metadatos Action0 y callbacks `0x152`/`0x153`/`0x154` con lista persistida, además de `NewCargo`, `CargoTaken`, `AcceptanceTick` y `AirplaneTouchdown` desde los eventos de simulación, traduciendo el cargo por la CTT propia del GRF. Las listas de badges de `AirportTiles`/`Airports` se traducen por GlobalVar `0x18` y `AirportTile` expone `0x7A` con resultado `UINT_MAX` para índices fuera de tabla. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD. `STNN` conserva ahora `airport.type`, `airport.layout` y `airport.rotation` custom, además de la huella `airport.tile/w/h` materializada; el cargador reatacha sus `AirportTile` cuando el layout activo coincide exactamente. `NGRF` y las filas base de `OBJS` ya tienen modelo semántico; un `OBJS` importado se conserva byte a byte hasta que una construcción/demolición lo invalida. `OBID` fusiona ahora los tres campos conocidos sobre la cabecera/filas originales cuando cambia el mapping, manteniendo columnas futuras y huecos densos; si cambia el conjunto de IDs se usa el writer canónico de forma segura. Desde 2026-09-02, todas las tablas `CH_TABLE`/`CH_SPARSE_TABLE` que reconstruye el writer pueden fusionar cambios de campos escalares de longitud fija sobre el cuerpo original, preservando columnas futuras y huecos cuando no cambian filas ni índices. Quedan mutaciones de strings, listas, structs/campos anidados, cambios estructurales y pools nativos todavía no modelados. |
| 5 | NewGRF runtime (#329) | Abierto | Vehículos, estaciones, objetos e industrias ya tienen rutas runtime parciales; los layouts `TileSeq` completos de estaciones rail, objetos e industrias materializan suelo/parents/children, y los aeropuertos construidos consumen los sprites estáticos `AirportTile` por tesela, mientras sprites base, paletas custom y layouts incompletos usan fallback atómico. Las teselas de industria ya comparten con vistas planas un contexto Action2 con random `m3`, etapa, terreno, zona, posición relativa, frame, vecinos `0x60`–`0x62`, badges `0x7A` y un scope padre de producción/stock/historial; la entidad y el scope padre conservan además fundador, fechas, tipo de construcción, flags, último año de producción, entrega, layout, random y última aceptación por cargo (`0xB4`/`0x6E`), y `INDY` los reemite. Siguen pendientes PSA, historiales anidados y cargos custom. Vehículos además resuelven grupos Action2 real por etapa cargada/cargando, hasta ocho capas de sprite-stack, wagon overrides de Action3 por cadena de motor/cargo/default y el callback de articulación `0x16` (decodificación por versión, espejo y writeback `7C`). El barrido económico ya ejecuta `CB32` por unidad cada 32 días, persiste el contador/triggers pendientes y reseedea la máscara del grupo Action2 activo. `CB2D` se consulta con la máscara de color y el renderer aplica las paletas de compañía `775..790`, ambas rampas 2CC, mapas Action5 `0x0A` y crash `804`; la livery por esquema/grupo se propaga a ambos canales. `CB36` ya resuelve resultados signed/unsigned de 15 bits y modifica el acortamiento de trenes y vehículos de carretera mediante las propiedades `0x21`/`0x23`; siguen pendientes sus propiedades de capacidad, velocidad, potencia, esfuerzo tractor y costes. La compra y el autoreemplazo de trenes y vehículos de carretera materializan ahora las cadenas articuladas, enlazan sus unidades, conservan los vagones/unidades del jugador y usan el catálogo activo; el movimiento vial procesa sólo la cabeza, persiste un historial road multi-tesela y sincroniza las piezas creadas por CB16, y el renderer consulta la dirección invertida para cada unidad marcada como espejo antes de mantenerla como child. Action0/Action3 de vehículos aceptan ahora IDs locales `ExtendedByte` de hasta 14 bits en el catálogo, callbacks y vistas. CTT `0x1E/0x1F` conserva ahora las listas de inclusión y exclusión de barcos y resta los cargos excluidos al catálogo de refit. Los grupos Action2 deterministas `0x82/0x86/0x8A` consultan el contexto `parent` y los random `0x83/0x84` usan el padre inmediato, offsets relativos firmados y el alcance especial del primer vehículo del tramo con el mismo motor; la matriz conserva pruebas para ambos sentidos y para ese tramo. Casas reevalúan Action2 por tesela (etapa/hash, edad, zona `0x42` del pueblo más cercano, terreno, frame, posición y random/triggers). Los roadtypes conservan grupos Action3 específicos por selector, la caché los separa por `ROTSG_*` y el compositor de superficie, paradas viales, waypoints y puentes ya invoca catenaria trasera/delantera con fallback vanilla y `NoCatenary`; los puentes vinculan cada mitad a su parent combinado. Variable `61` ahora puede consultar var `62` con un segundo offset relativo del vehículo seleccionado y var `0x60` cuenta los IDs locales presentes desde esa unidad; las listas de badges de vehículo y vía se traducen mediante GlobalVar `0x18` y alimentan `0x64`/`0x65`/`0x7A`; siguen pendientes callbacks completos de casas, vehículos, estaciones, aeropuertos y cargos, además de `OBJS`/`OBID` estructural; la configuración activa `NGRF` ya se importa y exporta semánticamente. |
| 6 | Movimiento y economía diferencial (#330) | Abierto | Oráculos externos para carretera (tráfico/colisiones/dirección), rail (PBS/YAPF/presignals/consist) y aire/mar, incluyendo casos límite. El perfilador de `Kale_TitleGame.sav` ya no aborta cuando un callback devuelve un pago negativo: los contadores `u64` de estación/empresa/estadística saturan ese ajuste a cero y el crédito firmado conserva la penalización; quedan pendientes los oráculos diferenciales y sus casos límite. |
| 7 | Idiomas y settings (#331) | Abierto | Catálogo de idiomas, locale, settings y textos guardados se cargan y se comparan con OpenTTD sin colisiones ECS ni regresiones de UI. |

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
  `DrawFoundation`. El callback `CBID_INDTILE_DRAW_FOUNDATIONS` (`0x150`) se
  evalúa en pendientes y puede conservar el relieve original; siguen abiertos
  los callbacks de sonido/slope y los layouts/children múltiples fuera del
  subconjunto cubierto.
- Aeropuertos NewGRF: los layouts `Airports` conservan el `gfx` global de cada
  `AirportTile` junto con el `subst` vanilla de `m5`; al construir, el cliente
  materializa el sprite Action1/3 por tesela mediante la caché de imágenes y
  reevalúa Action2 con posición relativa, frame, layout padre, random y
  vecinos. Si falta el catálogo o la vista cae al `AirportPiece` vanilla.
  El importador SAV conserva tipo, layout, rotación y huella, y reatacha los
  `AirportTile` cuando el layout activo coincide exactamente. Action0 conserva
  frames/status/speed/triggers y el scheduler ejecuta parcialmente Built,
  TileLoop, next-frame y speed (`0x152`/`0x153`/`0x154`) con estado persistido;
  quedan los triggers FTA (carga/descarga/aterrizaje) ya conectados al scheduler,
  las rotaciones runtime del compositor y sonidos, por lo
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
`CBID_HOUSE_DRAW_FOUNDATIONS` y `CBID_INDTILE_DRAW_FOUNDATIONS` (`0x150`) sólo
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
caminos. Desde 2026-09-02, una mutación escalar fija compatible conserva las
columnas desconocidas mediante la fusión común; las mutaciones variables o
anidadas y los cambios de filas/índices siguen cayendo al writer canónico.
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
de última aceptación de cada slot. #329 y el bloque SAV siguen abiertos por
PSA, historiales anidados, cargos custom y callbacks restantes.
