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

## Handoff de issues — 2026-09-04

Base funcional local y publicada: **`5fa655d1`** (`core: reexport cargo class masks`),
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
`bd613e2a` materializa hasta 32 cargos custom como `CargoType::Custom`, los
asigna de forma estable por `(GRFID, local_id)` y los transporta por stocks,
packets, cobertura, estaciones, industria, producción, pagos, ratings,
cargodist, refit y autoreplace. `566ce56a` corrige además la frontera nativa
de SAV: `SLV<55` se interpreta por slot climático y `SLV≥55` por ID global,
incluidos los cargos custom `31..62`, para `STNN`, `INDY`, `VEHS` y `LGRP`;
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
`TOFU` custom. Slots `63+`, callback de refit, UI/variables ilimitadas y scopes
económicos siguen pendientes.

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
de refit. #329 sigue abierto por slots `63+`, callback de refit, GUI/variables
ilimitadas, scopes económicos y otras propiedades Action0.

| Issue | Situación real al dejar este corte | Próxima brecha acotada |
|---|---|---|
| [#326](https://github.com/cavazquez/openttdrs/issues/326) | La composición raster global sigue abierta. Layouts `TileSeq`, parents/children, catenaria, PBS y varias capas ya tienen cobertura focal; foundations de rail pendiente/túnel, foundations/rotaciones de aeropuertos, sprite-stack/callbacks avanzados de vehículos y el orden completo de framebuffer siguen sin equivalencia global. | Medir la primera familia visible restante con `world-draw` y, si aplica, capturas en `0,12×`, `0,25×`, `0,50×` y `1×`. |
| [#328](https://github.com/cavazquez/openttdrs/issues/328) | El round-trip preserva las tablas y campos escalares modelados, `CITY`/`INDY`/`STNN`/`PSAC`, `OBJS`/`OBID`, grupos, órdenes y autoreplace en el subconjunto documentado. Mutaciones de strings, listas/structs anidados, columnas desconocidas, pools nativos de casas/objetos y labels/cargos no representables todavía degradan al writer canónico o quedan pendientes. CB17 de casas y CB157 de objetos pueden crear/modificar PSA de pueblo y el writer les asigna una fila `PSAC`/referencia `CITY` al exportar. | Elegir una mutación SAV reproducible de lista/struct anidado y comparar bytes OpenTTD→Rust→OpenTTD. |
| [#329](https://github.com/cavazquez/openttdrs/issues/329) | `CITY.received` hidrata crecimiento; la producción de casas escribe `CITY.supplied`; `0xBA`–`0xCB` leen producción/transporte y el PSA de pueblo se selecciona por GRFID en scopes parent de casas/objetos. CB17/CB157 evalúan y escriben el parent real. CB25/26/27 comparten contexto y PSA por huella: `TileLoop` sólo sobre visitas, `IndustryTick` al intervalo de producción, `CargoReceived` al completar la entrega y el avance de frames queda separado por tick. Shape-check `CB2F`, foundations `CB30`, autoslope manual `CB3C`, color `CB14A`, rechazo `CB3D`, cargos dinámicos `CB14B`/`CB14C`, `CargoTypesUnlimited` (hasta 16 slots, con salidas extra procesadas/transportables/exportables), slots vacíos `INVALID_CARGO` del modo legacy, rehidratación de filas `INDY` al aplicar el catálogo NewGRF, efectos especiales `CB3B`, `PlantOnBuild` manual/NewGRF, afterload SAV `<SLV_32`, entrega directa tipo `DeliverGoodsToIndustry` y el monitor runtime `AddCargoDelivery` ya tienen call sites y regresiones. La descarga ordena por `DistanceMax`, excluye la industria de origen, respeta el límite `uint16` de waiting, consulta `CBID_INDUSTRY_REFUSE_CARGO`, actualiza fecha/flag de aceptación y difiere la producción hasta después de `load_vehicles`, con rutas CB1, CB2 exclusivo y matriz vanilla. Los historiales aceptados y producidos por salida giran 61 registros nativos y se reemiten para cargos representables; el monitor empaqueta IDs con el layout nativo, exige activación, satura a `i32` y reinicia al consultar. `STNN.base.owner`, `INDY.neutral_station`/`exclusive_supplier` y `serve_neutral_industries` ya tienen importación, runtime y round-trip. Las filas `INDY` de cargos no resolubles se conservan ahora como passthrough opaco (slot, stock, rate e historial), pero no son ejecutables sin su catálogo. `CargoDistributed`/`ConstructionStageChanged` ya tienen call sites y la aceptación exacta de teselas de industria (`CBID_INDTILE_CARGO_ACCEPTANCE`/`CBID_INDTILE_ACCEPT_CARGO`) ya alimenta la cobertura de estación y la descarga. Sigue faltando el modelo de cargos custom ejecutables/CTT completo, bindings de GameScript equivalentes a `ScriptCargoMonitor`, reatachación económica cuando falta su catálogo, GUI/variables ilimitadas, autoslope en generación automática, sonido, mutaciones económicas fuera de esos caminos y el resto de callbacks/scope. | Medir una primera divergencia reproducible de cargo custom con catálogo instalado o una mutación económica fuera de los historiales; no cerrar #329 por este subconjunto. |
| [#330](https://github.com/cavazquez/openttdrs/issues/330) | Economía básica y movimiento funcionan, pero los oráculos externos todavía son acotados. Tráfico/colisiones/dirección vial exhaustivos, PBS/YAPF/presignals/consist ferroviarios y navegación aire/mar no tienen aún cobertura diferencial completa. | Tomar el primer fixture externo reproducible de movimiento y registrar tick, entidad y estado nativo divergente. |
| [#331](https://github.com/cavazquez/openttdrs/issues/331) | Locale `es`/`en`, etiquetas estáticas y errores de comandos cambian en vivo; siguen pendientes cuerpos/titulares generados, catálogos upstream completos, settings no modelados y la paridad UI sin colisiones ECS. | Auditar un catálogo/setting guardado contra OpenTTD y añadir una regresión de cambio de idioma. |
| RMAP-004 y padres abiertos | Las cohortes auditadas de mapas (64²→512² y cortes ampliados) son exactas por tesela y bloques 4×4, pero eso no generaliza a todas las semillas, tamaños, climas, settings de ríos ni ticks posteriores. | Ampliar la matriz combinatoria sólo cuando exista una primera divergencia reproducible; no convertir una cohorte exacta en cierre del generador. |

Corrección vigente de la tabla: `566ce56a` resuelve la codificación global de
cargos modernos en SAV y conserva los slots climáticos de saves anteriores a
`SLV_55`. Esta nota prevalece sobre las filas históricas que todavía describen
los cargos custom como exclusivamente opacos.

Corrección de la tabla en `b25a2362`: la CTT de cargos custom ya es ejecutable
en las variables parametrizadas de estaciones y paradas viales cuando el
catálogo está instalado. El residual de `#329` queda acotado a callbacks
CB140–142, `AirportTiles`, industria, GUI/variables ilimitadas y otros scopes
que todavía no reciben ese catálogo.

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
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden estable. Paradas, waypoints viales, estaciones rail, objetos e industrias resuelven layouts `TileSeq` completos por Action3/2→Action1, materializan suelo, parents/children y pendientes; los aeropuertos construidos conservan ahora el `gfx` `AirportTile` por tesela y consumen su sprite Action1/3 con fallback vanilla atómico. El procesador aplica `DODRAW`, offsets de sprite/caja/child, `var10`, draw mode `0x100` e invalida la caché con registros `7D`/`0x100`. Sprites base y paletas custom siguen fallback atómico. Los callbacks `0x150` de casas y teselas de industria ya se evalúan sólo en pendientes y pueden suprimir `FOUNDATION_LEVELED`; `RTSG_DEPOT` (selector 8) ya reemplaza las seis fachadas relocatables de depósitos ferroviarios con Action2, offsets NFO y children de fundación; quedan por cubrir las variantes ferroviarias de pendiente/túnel, el compositor de foundations/rotaciones de aeropuertos y el sprite-stack/callbacks avanzados de vehículos. La animación AirportTile ya ejecuta metadatos Action0 y callbacks `0x152`/`0x153`/`0x154` con lista persistida, además de `NewCargo`, `CargoTaken`, `AcceptanceTick` y `AirplaneTouchdown` desde los eventos de simulación, traduciendo el cargo por la CTT propia del GRF. Las listas de badges de `AirportTiles`/`Airports` se traducen por GlobalVar `0x18` y `AirportTile` expone `0x7A` con resultado `UINT_MAX` para índices fuera de tabla. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD. `STNN` conserva ahora `airport.type`, `airport.layout` y `airport.rotation` custom, además de la huella `airport.tile/w/h` materializada; el cargador reatacha sus `AirportTile` cuando el layout activo coincide exactamente. `NGRF` y las filas base de `OBJS` ya tienen modelo semántico; un `OBJS` importado se conserva byte a byte hasta que una construcción/demolición lo invalida. `OBID` fusiona ahora los tres campos conocidos sobre la cabecera/filas originales cuando cambia el mapping, manteniendo columnas futuras y huecos densos; si cambia el conjunto de IDs se usa el writer canónico de forma segura. Desde 2026-09-02, todas las tablas `CH_TABLE`/`CH_SPARSE_TABLE` que reconstruye el writer pueden fusionar cambios de campos escalares de longitud fija sobre el cuerpo original, preservando columnas futuras y huecos cuando no cambian filas ni índices. `INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` ya se decodifican, hidratan sus referencias y se reemiten con índices densos y 256 registros; los registros no nulos de pueblo se exponen por GRFID a los scopes parent de casas y objetos y se conservan también cuando no tienen consumidor. CB17 de casas y CB157 de objetos pueden crear/modificar la fila PSA de su pueblo y el writer la referencia desde `CITY.psa_list`; quedan mutaciones de strings, listas/structs anidados, cambios estructurales, writeback de callbacks de teselas y pools nativos de casas/objetos todavía no modelados. |
| 5 | NewGRF runtime (#329) | Abierto | Vehículos, estaciones, objetos e industrias ya tienen rutas runtime parciales; los layouts `TileSeq` completos de estaciones rail, objetos e industrias materializan suelo/parents/children, y los aeropuertos construidos consumen los sprites estáticos `AirportTile` por tesela, mientras sprites base, paletas custom y layouts incompletos usan fallback atómico. Las teselas de industria ya comparten con vistas planas un contexto Action2 con random `m3`, etapa, terreno, zona, posición relativa, frame, vecinos `0x60`–`0x62`, badges `0x7A` y un scope padre de producción/stock/historial; la entidad y el scope padre conservan además fundador, fechas, tipo de construcción, flags, último año de producción, entrega, layout, random y última aceptación por cargo (`0xB4`/`0x6E`), y `INDY` los reemite. Los callbacks de producción/cambio escriben ahora `7C`; `INDY.psa` y `PSAC` importan/exportan el pool de industria y conservan sus índices. Los registros PSA de pueblo se hidratan por GRFID y ya se leen desde el scope parent de casas y objetos; CB17 de casas y CB157 de construcción de objetos escriben ahora `\\2psto` en el PSA del pueblo, mientras el writeback de callbacks de teselas y las mutaciones de esos storages siguen pendientes. Los historiales anidados y `valid_history` se conservan en el round-trip SAV; falta conectarlos al runtime y siguen pendientes los cargos custom. Vehículos además resuelven grupos Action2 real por etapa cargada/cargando, hasta ocho capas de sprite-stack, wagon overrides de Action3 por cadena de motor/cargo/default y el callback de articulación `0x16` (decodificación por versión, espejo y writeback `7C`). El barrido económico ya ejecuta `CB32` por unidad cada 32 días, persiste el contador/triggers pendientes y reseedea la máscara del grupo Action2 activo. `CB2D` se consulta con la máscara de color y el renderer aplica las paletas de compañía `775..790`, ambas rampas 2CC, mapas Action5 `0x0A` y crash `804`; la livery por esquema/grupo se propaga a ambos canales. `CB36` ya resuelve resultados signed/unsigned de 15 bits y modifica el acortamiento de trenes y vehículos de carretera mediante las propiedades `0x21`/`0x23`; siguen pendientes sus propiedades de capacidad, velocidad, potencia, esfuerzo tractor y costes. La compra y el autoreemplazo de trenes y vehículos de carretera materializan ahora las cadenas articuladas, enlazan sus unidades, conservan los vagones/unidades del jugador y usan el catálogo activo; el movimiento vial procesa sólo la cabeza, persiste un historial road multi-tesela y sincroniza las piezas creadas por CB16, y el renderer consulta la dirección invertida para cada unidad marcada como espejo antes de mantenerla como child. Action0/Action3 de vehículos aceptan ahora IDs locales `ExtendedByte` de hasta 14 bits en el catálogo, callbacks y vistas. CTT `0x1E/0x1F` conserva ahora las listas de inclusión y exclusión de barcos y resta los cargos excluidos al catálogo de refit. Los grupos Action2 deterministas `0x82/0x86/0x8A` consultan el contexto `parent` y los random `0x83/0x84` usan el padre inmediato, offsets relativos firmados y el alcance especial del primer vehículo del tramo con el mismo motor; la matriz conserva pruebas para ambos sentidos y para ese tramo. Casas reevalúan Action2 por tesela (etapa/hash, edad, zona `0x42` del pueblo más cercano, terreno, frame, posición y random/triggers). Los roadtypes conservan grupos Action3 específicos por selector, la caché los separa por `ROTSG_*` y el compositor de superficie, paradas viales, waypoints y puentes ya invoca catenaria trasera/delantera con fallback vanilla y `NoCatenary`; los puentes vinculan cada mitad a su parent combinado. Variable `61` ahora puede consultar var `62` con un segundo offset relativo del vehículo seleccionado y var `0x60` cuenta los IDs locales presentes desde esa unidad; las listas de badges de vehículo y vía se traducen mediante GlobalVar `0x18` y alimentan `0x64`/`0x65`/`0x7A`; siguen pendientes callbacks completos de casas, vehículos, estaciones, aeropuertos y cargos, además de `OBJS`/`OBID` estructural; la configuración activa `NGRF` ya se importa y exporta semánticamente. |
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
de última aceptación de cada slot. PSA de industria, aeropuerto y referencias
de pueblo ya tiene importación, hidratación de filas y exportación nativa:
`INDY.psa`, `STNN.normal.airport.psa`, `CITY.psa_list` y `PSAC` se resuelven;
los registros no nulos de industria/estación se hidratan y las listas de pueblo
se conservan por índice. Los historiales anidados de `INDY`
(`history`, `valid_history` y `accumulated_waiting`) ya se conservan en el
round-trip, pero aún no alimentan los scopes ni se invalidan tras mutaciones.
#329 y el bloque SAV siguen abiertos por callbacks de tesela, callbacks PSA de
pueblos, storages de casas/objetos, cargos custom y callbacks restantes.

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
