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
| 2 | RMAP-004: generador procedural | Abierto P1 | Reducir la primera divergencia de TGP/RNG/`FixSlopes`/clear/towns/industries/trees con matriz 64²→512²; cerrar sólo cuando el mismo seed tenga contrato documentado y sin divergencias no explicadas. |
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden estable. Paradas, waypoints viales, estaciones rail, objetos e industrias resuelven layouts `TileSeq` completos por Action3/2→Action1, materializan suelo, parents/children y pendientes; los aeropuertos construidos conservan ahora el `gfx` `AirportTile` por tesela y consumen su sprite Action1/3 con fallback vanilla atómico. El procesador aplica `DODRAW`, offsets de sprite/caja/child, `var10`, draw mode `0x100` e invalida la caché con registros `7D`/`0x100`. Sprites base y paletas custom siguen fallback atómico. Continúan pendientes las variantes ferroviarias de pendiente/túnel/depósito y el sprite-stack/callbacks avanzados de vehículos; la animación AirportTile ya ejecuta metadatos Action0 y callbacks `0x152`/`0x153`/`0x154` con lista persistida, además de `NewCargo`, `CargoTaken`, `AcceptanceTick` y `AirplaneTouchdown` desde los eventos de simulación, traduciendo el cargo por la CTT propia del GRF. Foundations y rotaciones runtime siguen abiertos; las listas de badges de `AirportTiles`/`Airports` se traducen por GlobalVar `0x18` y `AirportTile` expone `0x7A` con resultado `UINT_MAX` para índices fuera de tabla. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD. `STNN` conserva ahora `airport.type`, `airport.layout` y `airport.rotation` custom, además de la huella `airport.tile/w/h` materializada; el cargador reatacha sus `AirportTile` cuando el layout activo coincide exactamente. `NGRF` y las filas base de `OBJS` ya tienen modelo semántico; un `OBJS` importado se conserva byte a byte hasta que una construcción/demolición lo invalida. Quedan las columnas desconocidas dentro de tablas reconstruidas y el mapping `OBID`, que requieren merge estructural. |
| 5 | NewGRF runtime (#329) | Abierto | Vehículos, estaciones, objetos e industrias ya tienen rutas runtime parciales; los layouts `TileSeq` completos de estaciones rail, objetos e industrias materializan suelo/parents/children, y los aeropuertos construidos consumen los sprites estáticos `AirportTile` por tesela, mientras sprites base, paletas custom y layouts incompletos usan fallback atómico. Vehículos además resuelven grupos Action2 real por etapa cargada/cargando, hasta ocho capas de sprite-stack, wagon overrides de Action3 por cadena de motor/cargo/default y el callback de articulación `0x16` (decodificación por versión, espejo y writeback `7C`). El barrido económico ya ejecuta `CB32` por unidad cada 32 días, persiste el contador/triggers pendientes y reseedea la máscara del grupo Action2 activo. `CB2D` se consulta con la máscara de color y el renderer aplica las paletas de compañía `775..790`, ambas rampas 2CC, mapas Action5 `0x0A` y crash `804`; la livery por esquema/grupo se propaga a ambos canales. `CB36` ya resuelve resultados signed/unsigned de 15 bits y modifica el acortamiento de trenes y vehículos de carretera mediante las propiedades `0x21`/`0x23`; siguen pendientes sus propiedades de capacidad, velocidad, potencia, esfuerzo tractor y costes. La compra y el autoreemplazo de trenes y vehículos de carretera materializan ahora las cadenas articuladas, enlazan sus unidades, conservan los vagones/unidades del jugador y usan el catálogo activo; el movimiento vial procesa sólo la cabeza, persiste un historial road multi-tesela y sincroniza las piezas creadas por CB16, y el renderer consulta la dirección invertida para cada unidad marcada como espejo antes de mantenerla como child. Action0/Action3 de vehículos aceptan ahora IDs locales `ExtendedByte` de hasta 14 bits en el catálogo, callbacks y vistas. CTT `0x1E/0x1F` conserva ahora las listas de inclusión y exclusión de barcos y resta los cargos excluidos al catálogo de refit. Los grupos Action2 deterministas `0x82/0x86/0x8A` consultan el contexto `parent` y los random `0x83/0x84` usan el padre inmediato, offsets relativos firmados y el alcance especial del primer vehículo del tramo con el mismo motor; la matriz conserva pruebas para ambos sentidos y para ese tramo. Casas reevalúan Action2 por tesela (etapa/hash, edad, terreno, frame, posición y random/triggers). Los roadtypes conservan grupos Action3 específicos por selector, la caché los separa por `ROTSG_*` y el compositor de superficie, paradas viales, waypoints y puentes ya invoca catenaria trasera/delantera con fallback vanilla y `NoCatenary`; los puentes vinculan cada mitad a su parent combinado. Variable `61` ahora puede consultar var `62` con un segundo offset relativo del vehículo seleccionado y var `0x60` cuenta los IDs locales presentes desde esa unidad; las listas de badges de vehículo y vía se traducen mediante GlobalVar `0x18` y alimentan `0x64`/`0x65`/`0x7A`; siguen pendientes callbacks completos de casas, vehículos, estaciones, aeropuertos y cargos, además de `OBJS`/`OBID` estructural; la configuración activa `NGRF` ya se importa y exporta semánticamente. |
| 6 | Movimiento y economía diferencial (#330) | Abierto | Oráculos externos para carretera (tráfico/colisiones/dirección), rail (PBS/YAPF/presignals/consist) y aire/mar, incluyendo casos límite. El perfilador de `Kale_TitleGame.sav` ya no aborta cuando un callback devuelve un pago negativo: los contadores `u64` de estación/empresa/estadística saturan ese ajuste a cero y el crédito firmado conserva la penalización; quedan pendientes los oráculos diferenciales y sus casos límite. |
| 7 | Idiomas y settings (#331) | Abierto | Catálogo de idiomas, locale, settings y textos guardados se cargan y se comparan con OpenTTD sin colisiones ECS ni regresiones de UI. |

> Actualización SAV: `OBJS` ya se modela en filas base y sólo se reconstruye
> después de una mutación; `OBID` y las columnas desconocidas permanecen en
> passthrough hasta implementar el merge estructural; `OBID` ya se decodifica
> y puede reconstruirse desde el catálogo, pero todavía no se aplica al
> cargador de overrides. El catálogo de objetos ahora consume ese mapping al
> reasignar los `ObjectType` de cada `(GRFID, local ID)`.

## Cómo se decide el siguiente bloque

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

## Estado medible al 2026-08-24

- Carga `.sav`: matriz aleatoria 15/15 exacta, 0 tiles y 0 bloques 4×4
  distintos.
- Generador procedural mismo seed: 0/15 exactos; 1.700–136.048 tiles
  distintos según tamaño/seed. RMAP-004 sigue abierto.
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
  (`m3`), offset de footprint, pendiente/terreno, animación (`m3hi`) y owner,
  y cachea cada resultado por fingerprint. Los layouts `TileSeq` completos
  reemplazan el suelo y emiten parents/children con cajas `M(...)`; sprites
  base, paletas custom y layouts incompletos mantienen fallback vanilla. Las
  variables de town/teselas vecinas y los callbacks de objeto siguen
  pendientes.
- Casas NewGRF: `DrawNewHouseTile` ya no cae automáticamente en
  `HOUSE_DRAW_DATA`: el sprite de edificio se resuelve desde Action1/2/3 con
  el contexto persistido de la tesela y se registra como parent sortable,
  mientras el suelo sigue usando el sustituto vanilla. Las variables de
  pueblo/conteos/vecindad, layouts `TileLayout` con suelo propio, callbacks de
  foundation/color/animación y la paleta `random_colour` siguen siendo
  residuales explícitos.
- Industria NewGRF: la vista Action2 runtime usa también sus offsets resueltos
  y, cuando la tesela se nivela, el overlay se adjunta al último parent de
  `DrawFoundation`; callbacks de foundation específicos y layouts/children
  múltiples siguen abiertos.
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
  el scope de pueblo `0x42`, rotaciones runtime del compositor y sonidos, por lo
  que #329 continúa abierto.
- `reference/` es un checkout local ignorado/no versionado; nunca se agrega al
  commit de una tarea.

### Avance SAV — 2026-08-26

El importador conserva cualquier *fourcc* que el escritor no reconstruye
(`VIEW`, `DEPT`, `SUBS`, `ROAD`, `AIPL`, `GSTR`, `GSDT` y futuros equivalentes),
incluyendo su tipo de contenedor y cuerpo exacto. Así un round-trip no descarta
features nuevas sólo porque todavía no tengan un modelo Rust. Esto no cierra
el bloque: las columnas adicionales de tablas que sí reconstruimos (`VEHS`,
`PLYR`, `PATS`, `LGRP`, etc.) siguen explícitamente pendientes.

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
