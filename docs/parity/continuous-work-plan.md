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
| 3 | Composición raster global (#323→#322→#326) | En curso | El sorter runtime ya cubre piezas estructurales, catenaria, el bloque combinado PBS/Action5/tranvía de puentes y cuerpos/unidades de vehículos con cajas `M(...)`, children y orden de inserción estable; el overlay vanilla de tranvía de un puente medio resuelve la rampa sur y los offsets específicos `0,1,11..14` de `DrawBridgeRoadBits`. Los grupos NewGRF `ROTSG_BRIDGE`/`ROTSG_OVERLAY` y `ROTSG_CATENARY_BACK/FRONT` se resuelven desde esa rampa con la caché runtime; superficie y catenaria trasera quedan como children del parent trasero y la mitad frontal como child del parent frontal. Los overlays NewGRF de carretera y estación rail (incluidas pendientes niveladas) siguen la fundación cuando existe. Siguen pendientes el fallback de catenaria vanilla de carretera/tranvía, sus assets completos y layouts/children completos de estación/objeto/industria/casa; el sprite-stack de vehículos ya materializa hasta ocho capas runtime con Action2 real y var `0x10`, pero conserva límites de callbacks/paleta. Las capturas 4×4 siguen siendo diagnóstico, no único oracle. |
| 4 | Interoperabilidad SAV (#328) | Abierto | VEHS/ORDL/GRPS/ERNW y shared orders/autoreplace round-trip OpenTTD→Rust→OpenTTD; todos los chunks no reconstruidos se preservan opacos, pero las columnas desconocidas dentro de tablas semánticas aún requieren merge estructural. |
| 5 | NewGRF runtime (#329) | Abierto | Vehículos, estaciones, objetos e industrias ya tienen rutas runtime parciales; vehículos además resuelven grupos Action2 real por etapa cargada/cargando, hasta ocho capas de sprite-stack, wagon overrides de Action3 por cadena de motor/cargo/default y el callback de articulación `0x16` (decodificación por versión, espejo y writeback `7C`). El barrido económico ya ejecuta `CB32` por unidad cada 32 días, persiste el contador/triggers pendientes y reseedea la máscara del grupo Action2 activo. `CB2D` se consulta con la máscara de color y el renderer aplica las paletas de compañía `775..790`; faltan 2CC/crash, livery por compañía y propagar la invalidación de paleta del callback. La compra y el autoreemplazo de trenes y vehículos de carretera materializan ahora las cadenas articuladas, enlazan sus unidades, conservan los vagones/unidades del jugador y usan el catálogo activo; el movimiento vial procesa sólo la cabeza, persiste un historial road multi-tesela y sincroniza las piezas creadas por CB16, y el renderer consulta la dirección invertida para cada unidad marcada como espejo antes de mantenerla como child. Action0/Action3 de vehículos aceptan ahora IDs locales `ExtendedByte` de hasta 14 bits en el catálogo, callbacks y vistas. Los grupos Action2 deterministas `0x82/0x86/0x8A` consultan el contexto `parent` y los random `0x83/0x84` usan el padre inmediato y offsets relativos firmados; la matriz conserva pruebas para ambos sentidos. Casas reevalúan Action2 por tesela (etapa/hash, edad, terreno, frame, posición y random/triggers). Los roadtypes conservan grupos Action3 específicos por selector, la caché los separa por `ROTSG_*` y el compositor de puentes ya invoca desde la rampa sur/vano superficie, overlay y catenaria trasera/delantera, vinculando cada mitad a su parent combinado. Sigue pendiente el relativo especial por primer vehículo con el mismo motor, las variables `61/62` recursivas, el fallback vanilla de catenaria vial, callbacks/layouts completos de casas, vehículos, estaciones, aeropuertos, objetos, industrias y cargos, además de persistencia NGRF/OBJS. |
| 6 | Movimiento y economía diferencial (#330) | Abierto | Oráculos externos para carretera (tráfico/colisiones/dirección), rail (PBS/YAPF/presignals/consist) y aire/mar, incluyendo casos límite. |
| 7 | Idiomas y settings (#331) | Abierto | Catálogo de idiomas, locale, settings y textos guardados se cargan y se comparan con OpenTTD sin colisiones ECS ni regresiones de UI. |

## Cómo se decide el siguiente bloque

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
  parent posterior junto con `ROTSG_CATENARY_BACK`. Sigue pendiente el
  fallback vanilla de esos sprites (incluidos los assets `SPR_TRAMWAY_BASE`),
  además de los layouts/children NewGRF de estación/objeto/industria y casa;
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
  pendientes las paletas especiales (2CC/crash), callbacks y los scopes
  parent/relative avanzados (el padre inmediato y los offsets básicos ya se
  resuelven en el contexto de consist).
- Estaciones rail NewGRF: los tiletypes Action1/2/3 se dibujan también en
  pendientes y el overlay queda como child de la fundación nivelada, igual que
  la vía/PBS. Los layouts `TileSeq` con varias cajas y children siguen siendo
  un residual explícito.
- Objetos NewGRF: el renderer ya reevalúa Action2 por tesela con random
  (`m3`), offset de footprint, pendiente/terreno, animación (`m3hi`) y owner,
  y cachea cada resultado por fingerprint. Las variables de town, teselas
  vecinas, color/view y los layouts `TileSeq`/children siguen pendientes.
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
siguen abiertas las paletas especiales (2CC/crash), scopes parent/relative
avanzados y callbacks/layouts sin call site completo.
