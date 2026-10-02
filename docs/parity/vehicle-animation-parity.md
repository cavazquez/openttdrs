# Movimiento y animaciones de vehículos

Actualizado: 2026-10-02. Trabajo de FPS pausado por indicación del usuario.
Referencia nativa: OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`.

## #329-CB160-FA-LIFETIME — ciclo y sprites de humo avanzado

El despacho nativo de `SpawnAdvancedVisualEffect` convierte `0xFA` en
`EV_BREAKDOWN_SMOKE_AIRCRAFT`. La tabla `_effect_procs` lo enlaza con
`SmokeInit`/`SmokeTick`, los mismos procedimientos que el humo genérico de
mina de cobre. Usa `SPR_SMOKE_0..4` (`2040..2044`), cinco frames; el port usaba
los cuatro sprites de avería (`3737..3740`) y la cadencia de diésel.

La divergencia quedó reproducida antes del cambio: a edad 1, el port devolvía
frame 1 y OpenTTD conservaba el frame 0. El arreglo separa `AircraftSmoke`,
reutiliza los cinco sprites genéricos ya cargados por `WorldAssets` y reproduce
`SmokeTick`: contador inicial 12, subida cada cuatro ticks, cambio de imagen
al alcanzar `progress & 15 == 4`, y eliminación a edad 72. El humo de avería
habitual conserva su propio conjunto.

`oracle_vehicle_effect_cadence.py` extrae sin modificaciones el despacho,
`IncrementSprite`, los ocho procedimientos de inicialización/tick, la tabla
de direcciones y las cuatro filas pertinentes de `_effect_procs`. Registra
SHA-256 de fuentes y cuerpos. Adapta entradas de callbacks, asignación,
enumeración compacta y actualizaciones de viewport. No certifica la
probabilidad de emisión, consumo de RNG, importación, decodificación de GRF
ni raster de una partida.

La fixture publicada contiene 388 estados: `F1/F2/F3/FA` a edades `0..96`.
La regresión compara existencia, frame y subida en todos ellos. Otra prueba
ECS recorre los 97 estados de `FA`, distingue su atlas del de avería y
comprueba textura, color, prisma de mundo y desaparición de la entidad.
Las capturas de Kale pausado son controles visuales; no contienen un escenario
que certifique la emisión avanzada `FA` de extremo a extremo.

Reproducción del oracle:

```bash
python3 scripts/oracle_vehicle_effect_cadence.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/vehicle-effect-cadence-fresh --check
cargo test -p openttdrs-client --bin openttdrs-client \
  advanced_effect_lifetimes_match_native_dispatch_and_ticks
cargo test -p openttdrs-client --bin openttdrs-client \
  cb160_fa_sprite_and_entity_follow_native_complete_lifetime
```

**Cerrado el sub-issue de ciclo y conjunto de sprites.** Pasan el oracle
independiente (`--check`, 388 filas), las dos regresiones, formato, Clippy
estricto de core/cliente, 3.037 tests de core (6 ignorados), 1.700 del cliente
(2 ignorados), frescura de docs y `git diff --check`.

Los [controles de zoom](evidence/vehicle-effect-cadence-control-raster-20261002.csv)
usan los seis niveles efectivos y la petición adicional `0.125`, que se
limita a `0.25`. `0.50`, `1`, `2` y `4` mantienen PNG y composición idénticos;
en `0.50`/`2` coinciden también los 633 buffers de imágenes CPU comparados,
entradas completas y máscaras de cobertura/oclusión. El primer intento de
`0.25` no es comparable: su cámara terminó con otros límites y escala. La
[repetición conservada](evidence/vehicle-effect-cadence-control-repeat-20261002.csv)
coincide exactamente en cámara, composición y PNG.

`8` mostró 130 píxeles/81 bloques distintos. Los conjuntos semánticos de
sprites, prismas y proxies coinciden, pero eso no valida la relación ordenada
de identidades ni el raster. Los [controles del mismo binario](evidence/vehicle-effect-cadence-control-out8-repeat-20261002.csv)
reproducen los mismos 130 píxeles/81 bloques al repetir el anterior; el binario
corregido se repite sin diferencias. La segunda captura del anterior coincide
con la del corregido. La escena no contiene parents `2040..2044`. Esto registra
una variación previa del compositor; no cierra esa brecha ni certifica raster
avanzado `FA`. Los intentos originales permanecen conservados.
La evidencia completa de esta etapa se conserva en
`target/parity/vehicle-effect-cadence-20261002/`.

## #326-ROAD-ORDINARY-TURN-PRESENTATION — orientación y pasos de giro

El primer fallo se reprodujo en tabla 2, subpaso 2/frame 2: OpenTTD conserva
`DIR_SE`, pero el renderer elegía `DIR_E` por la tangente del siguiente punto.
El controlador físico del port ya coincidía en los 352 estados de 24 tablas
ordinarias, incluyendo cambios de dirección que conservan frame y posición.

La presentación parte ahora del rumbo autoritativo de vehículos con posición
vial válida. Recorre el presupuesto de avance, consume el paso de giro sin
moverse y calcula el coste del siguiente paso con el rumbo nuevo. Posición y
sprite consultan la misma regla; el controlador comparte su función de rumbo
sin cambiar el movimiento físico. Se conserva el fallback para posiciones aún
no inicializadas y las ramas separadas de bahías/medias vueltas.

`oracle_road_vehicle_turn_direction.py` usa los cuerpos nativos de rumbo,
las 32 tablas de carretera, sus enlaces originales y el bloque de giro sin
modificaciones. Conserva también la función completa como referencia. Adapta
estado inicial, almacenamiento, viewport y el bucle exterior sin tráfico,
bahías, reversas, transiciones ni integración de velocidad. Para interpolar,
recorre la secuencia de estados nativos ya registrada y usa el cuerpo nativo
de `GetAdvanceDistance`; esa interpolación continua es la presentación del
port, no una ejecución del renderer nativo.

Las fixtures contienen 352 estados y 3.520 muestras de presupuesto, que se
comprueban para bus, camión y tranvía (10.560 poses de dibujo). Verifican
posición con tolerancia `0.0001` de píxel de mundo y dirección exacta. Una
regresión del cliente comprueba que los handles/sprites de bus mantengan el
rumbo anterior antes del giro y seleccionen el nuevo tras consumir ese paso.
Esto cubre todas las rectas y curvas normales de 90 grados por ambos lados;
no certifica viajes completos ni las variantes excluidas.

Reproducción:

```bash
python3 scripts/oracle_road_vehicle_turn_direction.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/road-turn-direction-fresh --check
cargo test -p openttdrs-core --test native_road_turn_direction
```

Gates aprobados: oracle independiente de ambas fixtures (`--check`), tres
regresiones diferenciales, formato, Clippy estricto, 3.040 tests de core
(6 ignorados), 1.700 de cliente (2 ignorados), frescura de docs y diff. **Cerrado el sub-issue de curvas ordinarias**, con publicación de esta etapa.
Los [controles de zoom](evidence/road-vehicle-turn-direction-control-raster-20261002.csv)
conservan la misma cámara en los siete pares (seis niveles efectivos y `0.125`
limitado). En `0.25`/`0.50` el PNG no cambia; en `1`/`2`/`4`/`8` cambian
521/326/291/82 píxeles (54/50/69/37 bloques 4×4). No se exige imagen idéntica
para una corrección de orientación. La cobertura y oclusión CPU coinciden en
`0.50`/`2`; las entradas/buffers completos cambian, así que tampoco se
presentan como iguales. El [diagnóstico de atribución](evidence/road-vehicle-turn-direction-control-attribution-20261002.csv)
resuelve bytes de imágenes y fuentes de proxies: los conjuntos semánticos
estáticos se mantienen y cambian 75 sprites dinámicos y sus 75 proxies en
ambas escalas. Este diagnóstico omite identidades/orden y no certifica sus
relaciones completas ni raster nativo. Sigue abierta la variación de aliases
del compositor observada antes de esta etapa. Evidencia privada en
`target/parity/road-vehicle-turn-direction-20261002/`.

## Alcance pendiente

- La emisión está ligada a `Update`: agrupar ticks puede omitir decisiones de
  emisión, RNG y writeback de callbacks. Repetir una edad sobre la posición
  final no recupera el estado intermedio de cada tick.
- El límite de 48 efectos corta el recorrido de vehículos; debe comprobarse
  contra la asignación del pool nativo y su orden de RNG/callbacks.
- Hay ocho orientaciones de sprites, pero queda por medir el instante de
  cambio de dirección y posición por frame en giros, paradas y recorridos
  completos, especialmente tráfico y cadenas articuladas.
- Las posiciones del renderer se extrapolan entre ticks. Eso requiere
  distinguir el estado autoritativo de la presentación al comparar OpenTTD.
- Persisten sprites locales NewGRF, scope completo de consist, variantes de
  offsets y diferencias del compositor global. #326/#329 permanecen abiertos.

La geometría actual de trenes usa `_vehicle_subcoord` y segmentos rectos de
las piezas nativas, incluidas sus diagonales cortas. Las antiguas referencias
a curvas Bézier en la matriz ferroviaria ya no describían el código vigente.
