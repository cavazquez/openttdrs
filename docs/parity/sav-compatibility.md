# Compatibilidad `.sav` OpenTTD ↔ openttdrs

Estado vigente de compatibilidad del formato `.sav`. Corte: **2026-09-03**,
`main` con base funcional `c88518c4`, posterior al writeback canónico de `CITY`,
CB17 de casas, CB157 de objetos, CB25/26/27 de animación y re-randomización
`TileLoop`/`IndustryTick`/`CargoReceived` de teselas; los disparadores CB25 se
separan de la pasada visual CB26/CB27; referencia: **OpenTTD
15.3**, commit `14ec60f248547d4d062a1160f0fc26d742319888`.

Esta es la única matriz de capacidad para importación y exportación `.sav`.
[`PARIDAD.md`](../PARIDAD.md) sólo resume su madurez; la guía de wire format en
[`PLANIFICACION.md`](../PLANIFICACION.md#export-sav) y el pipeline de mapa en
[`MAPA_Y_FERROCARRIL.md`](../MAPA_Y_FERROCARRIL.md) no deben repetir ni
contradecir esta tabla. Un cambio de capacidad actualiza esta matriz y, si
cambia la prioridad global, la fila resumida de `PARIDAD.md`.

`✅` cubierto en el corte indicado; `🟡` best-effort o subconjunto; `❌` no se
preserva. Importar un dato no implica que el exportador lo escriba.

Corrección vigente: desde `26a915db`, `INDY.accepted[].history`,
`accepted[].accumulated_waiting`, `INDY.produced[].history` y `valid_history`
se hidratan desde SAV, participan del runtime de entrega/transferencia/barrido/
rollover y se reemiten al guardar para cargos representables. Las referencias
históricas a un mero passthrough no describen este corte; cargos custom y
mutaciones económicas fuera de esos caminos continúan parciales.

Actualización del corte `26a915db`: `INDY.produced[].history` también se
hidrata, se actualiza por salida durante transferencia/carga y se reemite con
la ventana nativa de 61 registros. El writer conserva passthrough sólo cuando
la salida no fue mutada en runtime; cargos custom y mutaciones fuera de estas
rutas siguen siendo best-effort.

Actualización del corte `036fda1f`: el monitor `CargoMonitor` es estado
efímero y no añade columnas al `.sav`; sus consultas y limpieza vuelven vacías
tras importar, igual que los mapas globales de `OpenTTD` que los GameScripts
deben activar otra vez. Las entregas runtime sí actualizan los mapas
compatibles con `_cargo_pickups`/`_cargo_deliveries`; bindings de GameScript,
exclusividad/neutral stations y cargos custom siguen fuera del formato
representable.

Actualización del corte `470499ea`: `STNN.base.owner` se importa y se
reemite, `INDY.neutral_station` conserva la referencia `REF_STATION` y
`INDY.exclusive_supplier` conserva `INVALID_OWNER` o el `Owner` de la
compañía. `PATS.station.serve_neutral_industries` también forma parte del
subconjunto semántico y aplica el fallback histórico de saves anteriores a
`SLV_SERVE_NEUTRAL_INDUSTRIES`. El enlace inverso estación↔industria se
rehidrata después de leer ambos pools; las referencias no válidas se rechazan
al exportar para evitar un `.sav` que OpenTTD no pueda resolver. Esto cubre el
wire format y la regla de entrega; bindings GameScript y cargos custom siguen
fuera del formato representable.

Actualización del corte `eb6bd78d`: al exportar una industria importada cuyo
catálogo NewGRF no está disponible, las filas `INDY.accepted` y
`INDY.produced` con slots no resolubles se reemiten como passthrough opaco.
Esto conserva waiting/stock, rate, fecha, acumulador e historiales y permite
que OpenTTD o una futura rehidratación vuelva a resolverlas; el runtime Rust
no las procesa ni las transporta mientras falte el `CargoSpec`.

Actualización del corte `c88518c4`: el scheduler conserva el orden de eventos
de animación de `IndustryTile`: CB25 `TileLoop` sólo se ejecuta para visitas,
CB25 `IndustryTick` al intervalo de producción y CB25 `CargoReceived` después
de confirmar una entrega. La pasada visual avanza CB26/CB27 sólo sobre la
lista persistida de teselas activas; `CargoDistributed` y
`ConstructionStageChanged` todavía no tienen call site.

| Área | Importar `.sav` de OpenTTD | Exportar `.sav` para OpenTTD | Límite y evidencia |
|---|---|---|---|
| Mapa y tiles | ✅ planos MAP*, túneles/puentes y metadatos de mapa | ✅ `MAPS` + planos RIFF | [`sav/build.rs`](../../crates/openttdrs-core/src/sav/build.rs), [`sav/write/map.rs`](../../crates/openttdrs-core/src/sav/write/map.rs) |
| Mundo base | ✅ ciudades, estaciones/waypoints, industrias, fecha y primera compañía best-effort; `CITY` importa la metadata nativa de nombre (`townnamegrfid`, `townnametype`, `townnameparts`), flags, ratings, `have_ratings`, unwanted, metas, crecimiento, exclusividad, layout, estatuas, historial válido y texto de GameScript; `CITY.supplied`/`received` y `CITY.psa_list` conservan sus listas, con storages no nulos hidratados por GRFID en el pueblo; `received.old_act/new_act` alimenta el gate de crecimiento y la producción de casas actualiza `supplied` durante la simulación; `INDY.accepted[].waiting`, `last_accepted`, producciones adicionales, layout seleccionado/random y metadata de fundador, fechas, tipo, flags y último año de producción se hidratan en la entidad NewGRF; al aplicar el catálogo NewGRF, `IndustryType`/overrides vuelven a asociar la instancia al `IndustrySpecDef` y las listas `accepted`/`produced` reconstruyen slots, tasas, multiplicadores y stocks sin repetir callbacks; las filas con cargo no resoluble conservan sus datos como passthrough opaco para reexportación; `valid_history` e historiales anidados por cargo se conservan para round-trip; las referencias `INDY.psa` y `STNN.normal.airport.psa` enlazan las filas `PSAC` y sus 256 registros | 🟡 `CITY` canónico emite todos los escalares modelados, arrays nativos (ratings/unwanted/goals), `supplied`/`received` y `psa_list`; `STNN`, `INDY` (incluidas listas anidadas `accepted`/`produced`, `selected_layout`, `random`, fundador, fechas, tipo, flags, `last_prod_year`, `valid_history`, historiales por cargo, `accepted[].accumulated_waiting`, `accepted[].last_accepted` y `psa`), `PSAC` para storages persistentes conservados o escritos por industrias, aeropuertos y pueblos, `DATE`, `PLYR` básicos | Los arrays fijos de OpenTTD se normalizan a `MAX_COMPANIES`/`NUM_TAE` y la caché `cache.population` no se exporta porque OpenTTD la reconstruye desde `MAP*`. Un `CITY` sin cambios conserva su cuerpo original; al cambiar listas/structs se usa el header canónico y todavía se pierden columnas anidadas desconocidas. La lectura runtime de `7C` de pueblo ya está disponible en los scopes parent de casas y objetos, y `0xBA`–`0xCB` usan las series de `supplied`; los callbacks CB25/26/27 y la re-randomización `ResolveRerandomisation` de teselas escriben el PSA parent de la `Industry` durante `TileLoop`, `IndustryTick` y `CargoReceived` cuando se dispone del contexto de mundo; el scheduler separa estos triggers de la pasada visual CB26/CB27. `CargoDistributed`/`ConstructionStageChanged`, scopes de otras entidades, cargos custom ejecutables y campos no modelados continúan parciales u opacos |
| Link graph y carga | 🟡 lee `LGRP`, `CAPA` y `CAPY` cuando están presentes | 🟡 escribe `LGRP`, `CAPA` con referencias desde `STNN`/`VEHS`, `ECMY` y `CAPY`; las descargas graduales crean/acumulan un pago por cabeza y lo traducen a `REF_VEHICLE` al guardar | Los vehículos también conservan `cargo.action_counts` (`transfer/deliver/keep/load`) del descriptor moderno; la semántica de rutas/cargos NewGRF sigue parcial. [`sav/linkgraph.rs`](../../crates/openttdrs-core/src/sav/linkgraph.rs), [`sav/entities.rs`](../../crates/openttdrs-core/src/sav/entities.rs), [`sav/economy.rs`](../../crates/openttdrs-core/src/sav/economy.rs) |
| Tren y consist | 🟡 lee cabezas, vagones y `next`; recompone el consist best-effort y conserva el subestado (`crash_anim_pos`, `force_proceed`, `track`, `flags`, `wait_counter`, `gv_flags`) | 🟡 escribe `next`, subtipos de cabeza/vagón y el subestado nativo de `SlVehicleTrain` | El estado común también conserva contador de movimiento, edad económica, fecha de servicio protegida por NewGRF, enlace `next_shared`, `unitnumber`, `dest_tile`, `spritenum`, `acceleration`, `refit_cap` y ventanas de unbunching; motores, path/tcache y geometría siguen siendo best-effort |
| Road y tranvía | 🟡 road conserva motor vanilla soportado, `cargo_cap`/`cargo_count`, velocidad, progreso y runtime (`state`/`frame`/bloqueo/adelantamiento/reversa`), además de `gv_flags` y el caché de ruta `path` (`trackdir`/`tile`); se convierte a bus/camión, no identifica tranvía | 🟡 bus/camión sobre road/depot válido; reemite capacidad/cantidad de carga, runtime vial, `gv_flags` y caché de ruta; no tranvía | También se preservan contador de movimiento, edad económica, fecha NewGRF de servicio y ventanas unbunching. La carga física, calendario de compra, última estación de carga, cuenta atrás y valor contable se conservan; vehículos/propiedades NewGRF y articulados siguen best-effort. [`sav/mod.rs`](../../crates/openttdrs-core/src/sav/mod.rs), [`sav/write/vehicles.rs`](../../crates/openttdrs-core/src/sav/write/vehicles.rs) |
| RoadStops NewGRF por tesela | 🟡 decodifica `STNN.roadstopspeclist` y `STNN.roadstoptiledata`; conserva `(GRFID, localidx)`, random/frame y reata la spec al reconstruir el catálogo | 🟡 emite listas nativas por estación, estados por tesela e índice de spec en `MAP8` | Requiere que el GRF esté instalado para resolver el `localidx`; un GRF ausente conserva la identidad pendiente, no una vista ejecutable; el límite nativo de `MAP8` es 63 specs custom por estación |
| Aeropuertos y AirportTiles NewGRF | 🟡 conserva `STNN.airport.type/layout/rotation` globales, la huella `airport.tile/w/h` y reatacha los gfx por tesela cuando el layout activo coincide exactamente | 🟡 emite `FACIL_AIRPORT`, tipo custom, layout/rotación y huella materializada; el cliente dibuja Action1/3 estático y degrada si falta el GRF | Runtime, FTA/callbacks y columnas desconocidas de `STNN` siguen fuera del subconjunto; una huella ambigua se deja vanilla |
| Barcos | 🟡 `VEH_SHIP` se hidrata como `Ship`, conservando `state`/`rotation`, el caché `path` de `Trackdir` y la proyección `TrackBits` | 🟡 sólo sobre agua o ship depot; reemite `SlVehicleShip.state`, `path` y `rotation` | La semántica YAPF/wormhole completa y otros estados de navegación siguen best-effort |
| Aviones y helicópteros | 🟡 aeronaves y FTA se hidratan; reconoce helicóptero | 🟡 emite ala fija + sombra, o helicóptero + sombra + rotor, y conserva los campos FTA (`pos`, `targetairport`, `state`, `previous_pos`, dirección, `crashed_counter`, `number_consecutive_turns`, `turn_counter` y `flags`) | También conserva la ventana de carga/descarga, el calendario común, `motion_counter`, edad económica, fecha de servicio NewGRF y ventanas unbunching; el runtime FTA y parte de la identificación de motores siguen siendo best-effort; [`sav/entities.rs`](../../crates/openttdrs-core/src/sav/entities.rs), [`sav/write/vehicles.rs`](../../crates/openttdrs-core/src/sav/write/vehicles.rs) |
| Órdenes | 🟡 estación, waypoint, depósito, condicionales, refit vanilla y flags soportados; `VEHS.current_order` cruda | 🟡 mismo subconjunto, una lista `ORDL` por vehículo y `StationID` nativo cuando proviene de un `.sav`; reemite `current_order.type/flags/dest/refit_cargo/wait_time/travel_time/max_speed` | El refit vanilla de depósito (`0..10`) se restaura; cargos NewGRF, destinos/contextos no soportados y variantes avanzadas se degradan; estaciones nuevas sin ID importado usan índice denso como fallback |
| Horarios | 🟡 lee `wait_time`, `travel_time`, límite de velocidad por orden, inicio, tiempo de orden, lateness, muestras derivadas, `current_order` cruda y contadores diarios | 🟡 escribe esos campos por orden, `current_order`, `day_counter`, `tick_counter`, `running_ticks`, `service_interval` y el bitset de `VehicleFlags` (con bits de horario sincronizados) | La espera activa es estado efímero; reparto `timetable_all`, livery y metadatos de órdenes avanzadas siguen reducidos |
| Shared orders | ✅ reconstruye `shared_order_id` agrupando los vehículos por su índice `ORDL` | ✅ reutiliza una única `ORDL` para vehículos que comparten lista | Persisten limitaciones de horarios/órdenes avanzadas, pero la identidad compartida se conserva |
| Grupos y autoreplace | 🟡 lee `GRPS` y el pool `ERNW` con índice, enlaces, owner desde `PLYR` y scopes `ALL_GROUP`/`DEFAULT_GROUP` | 🟡 reemite `GRPS`, `VEHS.group_id` y cadenas `ERNW` densas con referencias `u32` y cabecera por compañía | Livery/historial de grupos y la edición UI completa siguen reducidos; el runtime no cubre todas las reglas avanzadas |
| Objetos | 🟡 lee las filas base de `OBJS` (ObjectID, ubicación, huella, town, fecha, color, vista y tipo) y el mapping `OBID` (GRFID, IDs local/sustituto), además de usar el pool para traducir tipos del mapa | 🟡 conserva `OBJS`/`OBID` sin cambios mientras no se muten objetos; tras construir/demoler reconstruye las filas base de `OBJS` y puede reconstruir `OBID` desde el catálogo; el cargador usa `OBID` para conservar IDs asignados | Las columnas futuras de `OBJS`, mappings faltantes y el runtime completo de specs/callbacks de objetos siguen pendientes |
| Ajustes | 🟡 lee el subconjunto ejecutado por el core de `PATS`/`OPTS`: construcción, pathfinding, aceleración de trenes **y carretera**, averías, subsidios, desastres, autoridad, inflación/recesiones y unidades de tiempo | 🟡 escribe ese subconjunto en `PATS` y conserva `GSET`/`ENGN`/`SRND` nativos como passthrough | [`sav/settings.rs`](../../crates/openttdrs-core/src/sav/settings.rs), [`sav/landscape.rs`](../../crates/openttdrs-core/src/sav/landscape.rs) |
| Compañías y noticias | 🟡 dinero/préstamo/límite de préstamo individual (`PLYR.max_loan`, incluido el centinela global), meses de bancarrota/color/nombre/presidente/`face`/`face_style`/indicador AI, `settings.*`, 23 `PLYR.liveries` e historial trimestral (`cur_economy` + hasta 24 `old_economy`, incluido `delivered_cargo`) | 🟡 `PLYR` con esos campos, incluidas las libreas nativas (SLV355) y el orden más-reciente-primero de `old_economy`; un override de préstamo no es reemplazado por inflación | Faltan flags completos. La cola propia completa queda en JSON; los consumidores de noticias siguen fuera del formato nativo |
| NewGRF | ✅ lee `NGRF` como tabla y restaura archivo, GRFID, versión y hasta 128 parámetros activos; `ENGN`, `EIDS` y mappings no modelados siguen como chunks opacos; las colas `INDY.accepted`/`produced`, `accepted[].last_accepted`, `selected_layout`, `random`, metadata de fundador/fechas/tipo/flags/año y `INDY.psa`→`PSAC` se hidratan para CB1/CB2 y scopes; `STNN.normal.airport.psa` hidrata los registros no nulos en `Station`; `CITY.psa_list` conserva las referencias de cada pueblo y sus registros no nulos se hidratan por GRFID en `Town`; `INDY.accepted[].history`, `accepted[].accumulated_waiting`, `INDY.produced[].history` y `valid_history` se hidratan para cargos representables | ✅ reconstruye `NGRF` para entradas activas no estáticas, con el array fijo de 128 parámetros (`num_params` conserva la longitud usada); `INDY`, aeropuertos `STNN` y pueblos `CITY` emiten sus referencias PSA; `PSAC` reemite el pool con 256 registros por fila y conserva storages ajenos a entidades modeladas; una casa que ejecuta CB17 y un objeto que ejecuta CB157 durante construcción pueden crear/modificar el PSA de su pueblo, y CB25/26/27 de animación y `ResolveRerandomisation` de `IndustryTile` escriben el PSA de la industria durante `TileLoop`, `IndustryTick` y `CargoReceived`; las entregas/transferencias, el barrido diario y el rollover mensual actualizan y reemiten los historiales de industria en su ventana nativa de 61 registros; el writer asigna las referencias nativas al exportar | La lectura de `7C` de pueblo ya cubre los scopes parent de casas y objetos; el writeback de CB17, CB157, la animación y la re-randomización de teselas de industria está cubierto en sus call sites, con CB25 separado del avance visual CB26/CB27; `CargoDistributed`/`ConstructionStageChanged`, cargos custom, mutaciones económicas fuera de las rutas de historiales y scopes de otras entidades continúan parciales; `OBJS`/`OBID` se reconstruyen sólo cuando se mutan y conservan columnas opacas fuera del modelo; los labels no representables en el catálogo fijo se omiten |

Nota de corte 2026-09-03: `CITY` decodifica y vuelve a escribir sus escalares
nativos, arrays fijas y listas anidadas modeladas, y los expone al modelo
`Town`; la caché de población sigue siendo deliberadamente derivada por
OpenTTD. El pool `PSAC` de industria, aeropuerto y referencias
de pueblo dejó de ser sólo passthrough. `INDY.psa` y `STNN.normal.airport.psa`
se resuelven contra sus filas, los registros no nulos se hidratan en
`Industry.newgrf_persistent_regs`, `Station.newgrf_persistent_regs` o el mapa
PSA del `Town` correspondiente, y el exportador reemite 256 valores por fila
manteniendo índices y storages de entidades aún no modeladas. `CITY.psa_list`
se decodifica como vector de referencias `REF_STORAGE` y se reemite sin
compactar índices; casas y objetos ya leen ese estado en sus scopes parent
Action2 cuando el pueblo está identificado. Una mutación económica futura
deberá invalidar este snapshot para recalcular historiales. Los callbacks CB17
de casas y CB157 de objetos durante construcción ya persisten los registros
`7C` escritos por grupos Action2 parent en `Town.newgrf_persistent_regs`; los
callbacks CB25/26/27 de animación de teselas hacen lo mismo en
`Industry.newgrf_persistent_regs` durante la simulación. La re-randomización
`ResolveRerandomisation` de teselas hace el mismo writeback en
`TileLoop`/`IndustryTick`/`CargoReceived`, y las máscaras parent se reseedean una
vez por footprint. Al exportar, `PSAC` y `CITY.psa_list` reciben la fila nueva o
actualizada. Los scopes de otros consumidores y las mutaciones de historiales
continúan pendientes.

La misma importación conserva ahora `CITY.supplied` (cargo y muestras
mensuales de producción/transporte) y `CITY.received` (contadores
`old_max`/`new_max`/`old_act`/`new_act` por efecto) en `Town`. Los contadores
recibidos se hidratan en el gate de crecimiento y el rollover sincroniza la
representación semántica y la nativa; la producción de casas actualiza
`supplied` y los scopes parent consumen sus muestras. El encoder canónico las
reemite con los tamaños nativos (incluido el slot `TAE_NONE`), mientras una
carga sin mutaciones sigue usando el cuerpo original por passthrough.

### Chunks futuros y campos no modelados

El lector conserva ahora todos los chunks cuyo *fourcc* no es reconstruido por
el escritor, no sólo la lista de features conocida. Esto incluye chunks nativos
actuales como `VIEW`, `DEPT`, `SUBS`, `ROAD`, `AIPL`, `GSTR` y `GSDT`, además de
cualquier chunk futuro que use uno de los tipos de contenedor soportados. El
cuerpo se guarda junto con su tipo (`RIFF`, `TABLE`, `ARRAY`, etc.) y se reemite
sin modificar al exportar. `NGRF` es la excepción ya modelada: se parsea y se
reconstruye desde `GameState`, evitando duplicarlo como passthrough. `OBJS`
conserva el payload original hasta que una mutación exige reconstruir sus
filas base; `OBID` se modela y se reconstruye desde el catálogo cuando no hay
passthrough, aunque todavía no se aplica al cargador de overrides. Las demás tablas reconstruidas (`VEHS`, `STNN`,
`PLYR`, `PATS`, `LGRP`, …) siguen teniendo únicamente el subconjunto de campos
documentado; sus columnas desconocidas todavía requieren un merge estructural
antes de poder afirmar paridad completa.

## Qué usar en cada caso

- Elegir `.sav` para intercambio del subconjunto anterior con OpenTTD 15.3.
- Elegir `.json` para reanudar una partida de openttdrs sin perder el estado
  propio, incluidos grupos, shared orders, autoreplace y el estado completo de
  horarios.
- No presentar una carga satisfactoria como equivalencia de round-trip: los
  campos que no se escriben pueden perderse al volver a guardar.

## Validación

El writer tiene pruebas de chunks, vehículos y órdenes en
[`sav/write`](../../crates/openttdrs-core/src/sav/write/). La matriz de release
usa [`validate_sav_openttd_matrix.sh`](../../scripts/validate_sav_openttd_matrix.sh)
contra OpenTTD 15.3 sin `SKIP`; el smoke local y el round-trip se ejecutan con
`./scripts/check.sh openttd-smoke` cuando hay un binario de referencia.

`ottn_roundtrip_preserves_ernw_chains_per_company` cubre IDs no consecutivos,
enlaces y dos cabezas `PLYR`; el fixture que produce fue aceptado por el
dedicated local de OpenTTD el 2026-08-22. Esto acredita ese contrato de pool,
no la equivalencia completa del runtime de autoreplace.

La matriz no garantiza compatibilidad binaria general, multijugador ni
ejecución de NewGRF. Para runtime de NewGRF, usar las matrices de
[Action0/3/5](newgrf-action0-matrix.md) y de
[callbacks](newgrf-callback-matrix.md).
