# Movimiento y animaciones de vehículos

Actualizado: 2026-10-02. Trabajo de FPS pausado por indicación del usuario.
Referencia nativa: OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`.
Las escalas citadas de las capturas/CSV son ortográficas: `0.25/0.50/1/2/4/8`
corresponden a magnificación visible `4×/2×/1×/0.50×/0.25×/0.125×`.
La petición ortográfica `0.125` se limita a `0.25`.

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

## #326-TRAIN-HEAD-IN-TILE-PIXELS — posición en piezas de vía

La comparación nativa reproduce dos errores de presentación. La función de
geometría escalaba el avance físico al último píxel del borde: incluso en
recta, edad de píxel 0 y remanente 48/192, entregaba `14.765625` en vez de
`14.75`. En las piezas cortas de ocho posiciones, ese mismo ajuste comprimía
el recorrido todavía más. Además, la pose dividía el remanente por el coste
del siguiente tramo lógico en vez del rumbo físico actual; una diagonal corta
usa coste 256, frente a 192 de un tramo de un solo eje.

La geometría aplica ahora un píxel por eje activo para cada paso físico y
conserva su fracción. La pose y su avance fraccional consultan el rumbo físico.
El sentinel legacy `255` conserva la consulta al último punto del borde.
No cambia el controlador, la ruta ni el tick de simulación.

`oracle_train_subtile_motion.py` extrae de la referencia fijada la tabla
`_initial_tile_subcoord`, `GetNewVehiclePos` y `GetAdvanceDistance` sin
modificaciones. Adapta el almacenamiento y la consulta de tesela; interpola
continuamente entre dos posiciones nativas como presentación del port. Sus
512 muestras cubren las doce entradas válidas a las seis piezas, todos sus
píxeles y remanentes 0, 1/4, 1/2 y 3/4 de paso. La regresión construye el mapa
y la ruta, consulta pose, posición y orientación por la vía real del renderer
del core y exige rumbo exacto y tolerancia de `0.0001` píxel de mundo.

Esto certifica la cabeza dentro de la pieza, con los remanentes indicados.
No ejecuta el renderer nativo ni certifica importación, saltos de tesela,
aceleración, separación/coste de followers o un recorrido completo. La
extrapolación que cruza una pieza corta se comprueba en el sub-issue siguiente;
no forma parte de estas 512 muestras.

```bash
python3 scripts/oracle_train_subtile_motion.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-subtile-motion-fresh --check
cargo test -p openttdrs-core --test native_train_subtile_motion
```

Gates aprobados: oracle independiente (`--check`), regresión de 512 muestras,
formato, Clippy estricto, 3.041 tests de core (6 ignorados), 1.700 de cliente
(2 ignorados), frescura de docs y diff. **Cerrado el sub-issue de posición de cabeza dentro de la pieza**, con
publicación de la etapa. Los [controles visuales](evidence/train-native-subtile-motion-control-raster-20261002.csv)
conservan cámara en todas las escalas excepto el primer `0.25`, que permanece
como intento no comparable. En `0.50`, `1`, `2`, `4` y `8` cambian
0/111/172/133/33 píxeles (0/26/44/42/21 bloques 4×4). En `0.50`/`2` coinciden
los 633 buffers CPU, cobertura y oclusión, pero las entradas y composición
completas cambian con las posiciones corregidas; no se declaran iguales.
El [diagnóstico de X/Y](evidence/train-native-subtile-motion-control-attribution-20261002.csv)
encuentra cero cambios de posición en sprites estáticos con identidad raw
compartida. `2` comparte las 43.209 entidades; `0.50` comparte 8.751/12.165,
por lo que esa comparación sólo cubre el subconjunto indicado. No certifica
aliases ni profundidades del compositor. La [repetición de `0.25`](evidence/train-native-subtile-motion-control-repeat-20261002.csv)
conserva cámara, composición y PNG idénticos; el intento inicial no comparable
permanece registrado.
Evidencia privada en
`target/parity/train-native-subtile-motion-20261002/`.

## #326-TRAIN-ORDINARY-TRACK-TRANSITIONS — salida y coste de la siguiente pieza

La regresión previa registra 808 diferencias entre 1.440 muestras, 600 al
salir de piezas cortas. La presentación esperaba 16 posiciones para todas
las vías; el controlador nativo cruza una pieza corta después de ocho.
Al entrar en una curva desde una recta también conservaba el coste anterior:
con presupuesto 240 desde el último píxel de una recta, entregaba
`(30.75,40.25)` en vez de `(30.8125,40.1875)`.

La extrapolación consume ahora el presupuesto físico hasta el borde según
la longitud actual, avanza la ruta y reconstruye el rumbo de la pieza
siguiente desde sus lados de entrada/salida. Recalcula el coste de cada
pieza: 192 para un eje, 256 para dos. Conserva la escala pública de progreso
de 16 píxeles y la consulta legacy del extremo de una ruta. No cambia el
controlador autoritativo, sus decisiones de ruta ni la velocidad.

`oracle_train_track_transition.py` extrae sin cambios la tabla de entrada,
`GetNewVehiclePos`, `GetAdvanceDistance` y el bloque que reasigna X/Y y rumbo
al entrar en la vía elegida. Adapta el bucle exterior, almacenamiento,
teselas y selección de piezas. La interpolación fraccional continúa el paso
del píxel actual; al completarlo, aplica la entrada nativa. Es una convención
de presentación continua del port, no una ejecución del renderer nativo.

La fixture cubre las 36 parejas válidas de piezas ordinarias, desde el último
píxel de la primera, con cuatro remanentes y diez presupuestos de avance.
La regresión comprueba tesela, índice de ruta, rumbo exacto y coordenadas de
mundo con tolerancia `0.0001`. Pasan sus 1.440 muestras y las 512 de posición
dentro de la pieza. No certifica followers, tráfico/señales, velocidad,
túneles, depósitos, final de ruta ni viajes completos.

```bash
python3 scripts/oracle_train_track_transition.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-track-transition-fresh --check
cargo test -p openttdrs-core --test native_train_track_transition
```

Gates aprobados: oracle independiente (`--check`), regresión de 1.440 muestras,
formato, Clippy estricto, 3.042 tests de core (6 ignorados), 1.700 del cliente
(2 ignorados), frescura de docs y diff. **Cerrado el sub-issue de transiciones
ordinarias de cabeza**, con publicación de esta etapa.

Los [controles de zoom](evidence/train-short-track-transition-control-raster-20261002.csv)
conservan cámara y composición completa tras renombrado biyectivo de
identidades en los siete pares. Los PNG coinciden en `0.125/0.25/0.50/1/4/8`;
en `2` difieren 2.622 píxeles/183 bloques 4×4 aun con entradas completas,
368 buffers CPU, cobertura y oclusión idénticos. En `0.50` coinciden también
las entradas y sus 265 buffers. La escena de Kale está congelada: controla
el estado dibujado y no ejerce todas las transiciones fraccionales del oracle.
La [repetición conservada](evidence/train-short-track-transition-control-out2-repeat-20261002.csv)
coincide en PNG entre ambos binarios y con la primera captura del anterior.
Repetir el mismo binario corregido reproduce la diferencia original de
2.622 píxeles/183 bloques, manteniendo cámara, composición, entradas,
368 buffers CPU y máscaras idénticos. Esto demuestra una variación de raster
con el mismo ejecutable; su causa permanece abierta en el compositor global.
No se descarta el primer intento ni se certifica raster nativo completo.
Evidencia completa, incluidos intentos fallidos, en
`target/parity/train-short-track-transition-20261002/`.

## #326-TRAIN-CONSIST-ORDINARY-GEOMETRY — separación en piezas cortas

La regresión previa encuentra 2.280 diferencias de geometría y 3.288 de
metadatos de ruta entre 11.520 muestras. La proyección hacia atrás dividía
el historial en tramos uniformes de 16 píxeles. En una curva corta, colocaba
un vagón en un píxel o tesela incorrectos. Además, la pose de cabeza guardaba
su rumbo físico cardinal como rumbo de entrada diagonal.

La proyección recorre ahora cada tesela del historial con su pieza de vía:
ocho o dieciséis posiciones. El rumbo de entrada de la cabeza se obtiene
de la tesela anterior. Cuando falta información para identificar una pieza
histórica, conserva el fallback previo de recta; ese caso no se certifica.

El oracle extrae sin modificaciones `_initial_tile_subcoord`,
`GetNewVehiclePos`, el bloque de entrada y `CalcNextVehicleOffset`. También
ejecuta la expresión nativa de distancia entre centros de `CheckTrainsLengths`.
Adapta el almacenamiento, la selección de ruta y el bucle exterior. Inicializa
cada follower a su offset sobre una secuencia de píxeles nativos registrada;
esto no ejecuta el controlador nativo completo ni sus señales/velocidad.

Las 36 parejas de vías y cinco configuraciones de longitudes (`8/8/8`,
`8/7/5`, `7/8/3`, `1/1/1`, `3/5/7`) producen 11.520 muestras de unidad.
Los 7.680 pares de centros consecutivos cumplen la expresión nativa de
distancia. La prueba del port compara tesela, píxel, metadatos de entrada y
salida, rumbo dibujado y X/Y de mundo, después de persistir las poses con
`propagate_consist_unit_poses`. No cubre remanentes fraccionales,
historial incompleto o repetido, reversas, depósitos, túneles, señales,
importación ni recorridos completos. El padre de paridad de consist sigue
abierto.

```bash
python3 scripts/oracle_train_consist_track_geometry.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-consist-track-geometry-fresh --check
cargo test -p openttdrs-core --test native_train_consist_track_geometry
```

Gates aprobados: oracle independiente (`--check`), 11.520 muestras,
regresión de persistencia de poses, formato y Clippy estricto, 3.043 tests de
core (6 ignorados), 1.700 del cliente (2 ignorados), frescura de docs y diff.
**Cerrado el sub-issue de geometría ordinaria de tres unidades**, con
publicación de esta etapa.

Los [controles de zoom](evidence/train-consist-short-track-control-raster-20261002.csv)
conservan cámara, composición completa tras renombrado biyectivo y PNG
idénticos en los siete pares (seis escalas efectivas y la petición `0.125`
limitada). En `0.50`/`2` coinciden también entradas completas, 633 buffers
CPU, cobertura y oclusión. Son controles de Kale congelado; no certifican
animación, recorridos ni raster nativo de todos los casos de la fixture.
La variación del compositor reproducida en etapas anteriores sigue abierta.
La evidencia completa de esta etapa se conserva en
`target/parity/train-consist-short-track-20261002/`.

## #326-ROAD-BAY-MOVEMENT-TURNS — pasos de giro en dársenas

Antes del cambio fallan las tres comparaciones. En tabla 32, primer avance,
OpenTTD conserva `DIR_NE` y velocidad 112; el port asigna `DIR_E` y velocidad
84. La tangente centrada de la tabla adelantaba el giro. También avanzaba el
frame y la posición al cambiar rumbo, aunque el bloque nativo conserva ambos
durante ese paso de giro.

El controlador comparte ahora el bloque de rumbo con la carretera normal
antes de resolver la rama de bahía. La presentación consulta ese mismo
rumbo y consume los pasos estacionarios sobre los puntos de la tabla. La
comparación de presupuesto incluye el error de representación de `frame_f`
para evitar que un límite entero quede justo por debajo del coste nativo
tras convertirlo a `f32`. No se modifica la regla de llegada/carga en esta
etapa.

La opción `--bay` del oracle añade las 16 tablas nativas de dársenas y los
64 enlaces originales. Comprueba que el bit `RVSB_ENTERED_STOP` selecciona
la misma tabla; los cuerpos de rumbo y giro siguen sin modificaciones.
El bucle adaptado omite servicio de estación, tráfico, RNG, saltos de tesela
e integración de velocidad. La regresión física marca ese bit en sus
estados para suprimir el servicio. Esto mide la regla de movimiento de las
tablas, no una entrada/carga/salida completa.

Pasan 752 estados físicos y sus orientaciones, y 7.520 presupuestos para
bus y camión (15.040 poses). Comprueban frame, posición y velocidad exactos,
giro estacionario, rumbo dibujado y coordenadas con tolerancia `0.0001`.
También siguen pasando las tres fixtures/regresiones de carretera normal.

Las pruebas de servicio dejan de suponer un frame por llamada: esperan el
estado de parada y acotan la salida a 64 subpasos, manteniendo sus
postcondiciones. El primer intento general quedó en un bucle de esa prueba
porque preparaba la salida antes de completar la llegada; permanece
conservado. Una segunda ejecución detectó el límite ajustado de continuación
JSON: a los 2.000 ticks el camión tenía 22 unidades y frame 11 dentro de la
dársena de destino; la muestra a 2.100 ya registra 22 unidades entregadas.
El diagnóstico de 4.000 ticks mantiene ambas ramas canónicas idénticas.
El límite final pasa a 3.000, con las mismas comprobaciones de estado,
entrega e ingreso. Esto valida la continuidad/jugabilidad de ese escenario,
no su cronología completa contra el simulador nativo.

La tercera ejecución detectó una expectativa económica de edad fija (24)
frente a la edad final medida (25). La procedencia sólo ejecuta el kernel
nativo de pago con entradas elegidas: no mide una partida nativa. Se
conservan los vectores originales y se añaden 512 pagos nativos por edad;
la prueba mantiene el ledger exacto para la entrada medida. El alcance y
la reproducción quedan en [transferencia de carbón V1](coal-transfer-v1.md).

```bash
python3 scripts/oracle_road_vehicle_turn_direction.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/road-bay-turn-direction-fresh --bay --check
cargo test -p openttdrs-core --test native_road_bay_turn_direction
```

**Cerrado el sub-issue de movimiento y orientación dentro de las tablas de
dársenas.** Pasan Clippy estricto de core/cliente, 3.047 tests de core
(6 ignorados), 1.700 del cliente (2 ignorados), formato, frescura de docs y
`git diff --check`. El oracle de pagos reproduce sus tres corpus y el de
carreteras conserva los casos ordinarios.

Los [controles de zoom](evidence/road-bay-turn-direction-control-raster-20261002.csv)
mantienen cámara y PNG idénticos en las siete peticiones. En `0.50`/`2`,
coinciden 265/368 buffers CPU y ambas máscaras; las entradas completas
cambian en once campos de dos sprites dinámicos y sus proxies (orientación,
ancla y un prisma). El PNG idéntico no certifica esas entradas: están fuera
de la vista. En `8` cambia la traza de ordenación, conservando el PNG.
Estas capturas son controles de una partida congelada; la geometría nativa
queda cubierta por las fixtures, no por equivalencia raster global.

La compilación release recompiló core y cliente (80,68 s); binario
`c63db3c8e156d8d677b694cd60a12989d8697750fa1fadcc75a757fbb31c1ebe`.
Evidencia en `target/parity/road-bay-turn-direction-20261002/`. La secuencia
de llegada se trata en el sub-issue siguiente; el remanente de movimiento
durante carga y el recorrido completo siguen pendientes.

## #326-ROAD-BAY-ARRIVAL — giro completo antes de empezar la carga

El oracle anterior excluía el servicio y marcaba `ENTERED_STOP` para medir
geometría. El nuevo ejecuta también el bloque nativo completo de llegada,
su tabla de frames de parada y el cambio de velocidad de `BeginLoading`.
Conserva esos fragmentos sin modificaciones, junto a las tablas, funciones
de rumbo y bloque de giro. Adapta el almacenamiento y el bucle de llamadas;
estación, órdenes, tráfico, pago, RNG, animación y viewport son stubs.
`BeginLoading` retiene sólo la asignación nativa de velocidad cero.

Se reproducen 432 estados hasta la llegada en las 16 tablas de dársenas,
ambos lados de circulación y bahías cercana/lejana. La regresión recorre
864 muestras de bus/camión y comprueba frame, posición, rumbo, velocidad,
bit de entrada, comienzo de carga y resultado del controlador.

Antes fallaba en tabla 32/subpaso 26: el original mantenía velocidad 21
y no empezaba a cargar; el port ya tenía velocidad cero. OpenTTD conserva
el punto de parada durante cuatro cambios de rumbo y empieza a cargar
orientado hacia la salida. El port comprueba ahora el frame actual después
de resolver el giro y devuelve `false` al entrar en servicio, igual que
la rama nativa; no avanza al siguiente punto de la tabla.

```bash
python3 scripts/oracle_road_bay_arrival.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/road-bay-arrival-fresh --check
cargo test -p openttdrs-core --test native_road_bay_arrival
```

**Cerrado el sub-issue de disparo de llegada después del giro.** Pasan las
siete regresiones de giro/llegada, Clippy estricto de core/cliente, 3.048
tests de core (6 ignorados), 1.700 del cliente (2 ignorados), formato,
frescura de docs y `git diff --check`.

Los [controles de zoom](evidence/road-bay-arrival-control-raster-20261002.csv)
conservan cámara, PNG y traza completa de composición en las siete
peticiones. En `0.50`/`2` coinciden todas las entradas de dibujo, 265/368
buffers CPU y máscaras de cobertura/oclusión. Son controles congelados:
la secuencia de llegada queda medida por las muestras nativas, no por esas
capturas. Release recompiló core y cliente (85,73 s); binario
`6e4f52b4bab17e896db632e76ee365e6828b3ef185bce3bd080612523bda4a7f`.

Evidencia conservada en
`target/parity/road-bay-arrival-20261002/`. No certifica los ticks completos,
servicio completo ni eventos de estación. El remanente del tick de entrada
se trata en el sub-issue siguiente; los ticks posteriores de carga/salida
continúan abiertos.

## #326-ROAD-BAY-LOADING-REMAINDER — fracciones del tick de entrada

La opción `--tick` amplía el oracle de llegada con `DoUpdateSpeed`,
`RoadVehicle::UpdateSpeed`, `GetAdvanceSpeed`, el bucle de movimiento y la
asignación final de `progress`, todos extraídos sin modificaciones del pin.
Usa aceleración original, límite fijo 112 y una sola unidad; conserva los
stubs de servicio y excluye continuación de carga, tráfico, callbacks,
RNG y recorrido completo. La posición inicial sale del bucle nativo de
llegada, antes de variar las entradas fraccionarias.

Hay 1.728 entradas: 16 tablas, seis velocidades, tres `subspeed` y seis
remanentes. La regresión exige igualdad exacta en 3.456 casos de bus/camión:
velocidad, fracciones, frame, posición, rumbo, bit de entrada y carga.
Antes fallaba con velocidad cero y remanente 192: OpenTTD empezaba a cargar
y guardaba 191; el port guardaba el sentinel 255. También borraba `subspeed`.

La llegada deja de borrar esas fracciones y la apertura de carga de una
dársena física conserva el remanente, como la rama de trenes. El controlador
guarda su propio resto al retornar. Las llamadas sintéticas sin posición
física conservan su contrato previo.

```bash
python3 scripts/oracle_road_bay_arrival.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/road-bay-loading-remainder-fresh --tick --check
cargo test -p openttdrs-core --test native_road_bay_loading_remainder
```

**Cerrado el sub-issue del tick de entrada en carga.** También pasan 8.160
poses de presentación de los estados que empiezan a cargar (bus/camión,
cinco alfas): mantienen el punto y rumbo nativos. Pasan Clippy estricto de
core/cliente, 3.049 tests de core (6 ignorados), 1.700 del cliente (2
ignorados), formato, frescura de docs y `git diff --check`. Tras reforzar
la regresión visual se repitieron su ejecución, Clippy de core y formato.

Los [controles de zoom](evidence/road-bay-loading-remainder-control-raster-20261002.csv)
conservan cámara, PNG y composición en las siete peticiones. En `0.50`/`2`
coinciden todas las entradas, 265/368 buffers CPU y las máscaras. La partida
está congelada: esos controles no certifican el servicio de estación.
Release recompiló core y cliente (81,69 s); binario
`794b721627f032245286ea6d6c5b83a6c93cb38e623602294183dcc00ec70631`.

Evidencia en `target/parity/road-bay-loading-remainder-20261002/`.
Esto no cierra las fracciones durante los ticks posteriores de transferencia
y salida ni el modelo de aceleración realista.

## #326-TRAIN-HELD-REMAINDER — pose física mientras está detenido

El oracle de sub-tesela incorpora las dos salidas nativas de
`TrainLocoHandler` para detenido con velocidad cero y `OT_LOADING`,
extraídas sin modificaciones. Omite las fases anteriores de avería,
reversa, procesamiento de órdenes y mantenimiento de carga. Enumera los
píxeles de las doce entradas válidas mediante las tablas y el avance
nativos; la rama de espera conserva esas coordenadas con cuatro
remanentes. No es una ejecución del handler completo ni del renderer.

La opción `--held` produce 1.024 estados. La regresión dibuja cada uno
en cinco alfas (5.120 poses), comparando coordenadas con tolerancia
`0.0001` y rumbo exacto. Antes falla incluso con remanente 1: la posición
nativa `(15,8)` se dibuja `(14.994792,8)` pese a velocidad cero.

`VehiclePose::from_vehicle` usa ahora sólo `rail_pixel` cuando el tren
tiene velocidad cero. Conserva `progress` en la simulación; el movimiento
posterior lo podrá consumir. Las poses en movimiento conservan su
normalización y costes previos. La fixture de 512 muestras móviles se
reproduce sin cambios, junto a las regresiones de transiciones y consists.

```bash
python3 scripts/oracle_train_subtile_motion.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-held-motion-fresh --held --check
cargo test -p openttdrs-core --test native_train_held_motion
```

Validación general: **3.050 core / 1.700 client**, sin fallos, 6/2
ignorados; ambos Clippy estrictos, formato, documentación y diff pasan.
El test existente de remanente móvil ahora declara velocidad no nula;
su coordenada esperada se conserva. Release reconstruida en 79,53 s,
core `fresh:false`, client SHA-256
`20f45a8ff61e4d7da834858deb80643215942e87c6d03fb6a190a28bffc87571`.

El [control de raster](evidence/train-held-motion-control-raster-20261002.csv)
cubre seis escalas y la solicitud redundante `.125` (limitada a `.25`).
Cámara principal y traza completa de orden coinciden. Hay cinco imágenes
idénticas; la primera pareja Out2 difiere 565 píxeles/175 bloques y Out8
un píxel/un bloque, sin atribución aislada para este último. En `.5`
cambian 266 campos de transformaciones: 133 en sprites y 133 en proxies; los 67
sprites están fuera de la vista. Los 265 buffers CPU y ambas máscaras
coinciden. En Out2 hay dos sprites modificados visibles, los 368 buffers
CPU coinciden, la cobertura coincide y la oclusión difiere.

La [repetición Out2](evidence/train-held-motion-control-out2-repeat-20261002.csv)
reproduce los 565 píxeles/175 bloques con **el mismo binario posterior**.
Sus sprites son idénticos; difieren el orden de cámaras y 54.672 campos
de mallas, manteniendo cámara principal, orden de padres y 368 buffers
CPU. La pareja repetida antes/después da PNG idéntico y únicamente los
266 campos de poses esperados. Se conservan todos los órdenes de query
y aliases; no se descartan para forzar equivalencia. Esta variación no
certifica ni refuta por sí sola el remanente de un tren detenido.

Evidencia privada, incluidos fallos iniciales, fuentes, binarios, trazas,
repeticiones y hashes, en `target/parity/train-held-motion-remainder-20261002/`.
Quedan abiertos el handler completo, animaciones de estaciones/GRF,
la presentación fraccionaria de los vagones y el raster global.

## #326-TRAIN-CONSIST-FRACTION — reloj común para la presentación

El oracle de geometría de cadenas incorpora `GetAdvanceDistance` sin
modificaciones. Sigue calculando cada centro con `CalcNextVehicleOffset`
y los píxeles con tablas y avance nativos. La opción `--render` usa la
fracción del reloj de la **cabeza**, aun cuando un follower está en una
pieza con otro coste. Interpola hacia el candidato de `GetNewVehiclePos`
antes del ajuste de entrada a la nueva tesela, como las fixtures de
sub-tesela y transiciones ya existentes. Es presentación adaptada, no el
renderer continuo de OpenTTD ni su `TrainController` completo.

Son **115.200 poses**: 36 parejas de piezas, cinco cadenas de tres
longitudes, cuatro remanentes y cinco presupuestos. La presentación
discreta de los followers diverge en 72.960; la proyección nueva conserva
coordenadas dentro de `0.0001`, tesela, rumbo y entrada/salida exactos.
La primera versión del harness interpolaba hacia el siguiente centro ya
ajustado; se conservó ese prototipo y su fallo. La versión final usa el
candidato nativo, manteniendo el contrato de presentación anterior. El
corpus entero de 11.520 poses físicas se reproduce sin cambios.

`consist_render_poses_indexed` proyecta los offsets sobre una vista prestada
de historial y path desde la pose extrapolada de la cabeza. Evita clonar
vehículos/órdenes y se calcula una vez por cabeza durante la sincronización.
Cada pose lleva la entrada/salida dibujada para reconstruir la pieza incluso
si el follower cruza una tesela entre ticks. Spawn, sprites, bounds, profundidad
y layers consumen la misma pose; el estado físico no cambia. Las cadenas
apiladas sin historial y el endpoint legacy conservan su fallback físico.

```bash
python3 scripts/oracle_train_consist_track_geometry.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-chain-render-fresh --render --check
cargo test -p openttdrs-core --test native_train_consist_fractional_render
```

La regresión ECS del cliente comprueba además el medio píxel nativo de un
follower y de su layer NewGRF, conservando la resolución única por parent.
Validación general: **3.051 core / 1.701 client**, cero fallos, 6/2 ignorados;
ambos Clippy estrictos, formato, documentación y diff pasan. La primera
compilación de tests del cliente agotó disco; el reintento dio símbolos
incrementales sin resolver. Se preservaron ambos fallos y la caché por
objetos comprimidos con SHA-256 verificado. La ejecución final usa
`CARGO_INCREMENTAL=0`; no se cambió la configuración del proyecto.

Release reconstruida en **78,91 s**, core `fresh:false`, client SHA-256
`4b54e263c505727c5fbe03352e5c24459bf032891ab75c0580bb407429a08209`.
Los [controles congelados de Kale](evidence/train-consist-fractional-render-control-raster-20261002.csv)
en las seis escalas y `.125` limitada a `.25` dan PNG, cámara principal y
trazas completas de orden idénticos. En `.5`/`2` también coinciden todas las
entradas de sprites y los **265/368 buffers CPU**, cobertura y oclusión.
Este control no contiene una cadena en movimiento certificada por OpenTTD;
la interpolación se comprueba con el oracle geométrico y la regresión ECS.

Evidencia privada en `target/parity/train-consist-fractional-render-20261002/`.
Permanecen abiertos historial con teselas repetidas, depots, túneles/puentes,
recorridos completos, callbacks y raster global. No se retomó el trabajo de FPS.

## #326-TRAIN-REPEATED-HISTORY — dos piezas en la misma coordenada

Un recorrido de seis unidades visita dos veces un cruce: primero sobre una
recta de 16 píxeles, después sobre una pieza corta de 8. El lookup por
coordenada confundía esas pasadas al proyectar los últimos vagones. La
primera divergencia nativa en el cuarto vagón: esperaba píxel 8 de la
recta y coordenadas `(71,72)`; el port producía píxel 0 de la otra pieza,
coordenadas `(71,79)` y rumbo norte en vez de nordeste. También desplazaba
el quinto vagón a otra tesela.

El nuevo oracle conserva sin modificar tablas de entrada, `GetNewVehiclePos`,
`CalcNextVehicleOffset`, `GetAdvanceDistance` y la expresión nativa de
separación entre centros. Adapta ruta, almacenamiento y enumeración. Sus
**240 poses físicas** cumplen todos los invariantes de separación. Las
**4.800 poses de presentación** usan el mismo contrato hacia el candidato
por píxel anterior al ajuste de entrada, con reloj compartido de la cabeza.
No es `TrainController` completo, ni prueba speed/signals/traffic, depots,
túneles, callbacks o renderer nativo.

Antes fallan **24/240** poses físicas y **444/4.800** entre ticks. La
proyección conserva ahora el índice de cada ocurrencia mientras coloca la
cadena, reconstruyendo entrada/salida y span desde sus vecinos históricos.
La referencia es interna; no cambia el formato de guardado ni la pose pública.
Las coordenadas pasan con tolerancia `0.0001`, teselas/píxeles físicos y
rumbos exactos, metadatos físicos de todas las unidades y de cada follower
interpolado. La cabeza interpolada se comprueba por geometría y rumbo.
Los corpus anteriores de 11.520 y 115.200 poses siguen pasando.

```bash
python3 scripts/oracle_train_consist_repeated_history.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-repeated-history-fresh --check
python3 scripts/oracle_train_consist_repeated_history.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/train-repeated-history-render-fresh --render --check
cargo test -p openttdrs-core --test native_train_consist_repeated_history
```

Validación general: **3.053 core / 1.701 client**, sin fallos, 6/2 ignorados;
ambos Clippy estrictos, formato, documentación y diff pasan. Client tests
usa `CARGO_INCREMENTAL=0` por el incidente de caché de la etapa anterior.
Se conserva el primer fallo de sintaxis al reforzar el test de metadatos;
la corrección no modifica valores del oracle.

Release reconstruida en **78,20 s**, core `fresh:false`, client SHA-256
`b6aea1f6f2554d41323fd0c430f7a7bc3fee1118b714fadc96b97cec4cb7d7e1`.
Los [controles congelados](evidence/train-consist-repeated-history-control-raster-20261002.csv)
en seis escalas y `.125` limitada a `.25` dan PNG, cámara principal y
trazas completas de orden idénticos. En `.5`/`2` coinciden todas las entradas
de sprites, **265/368 buffers CPU**, cobertura y oclusión. Kale congelado
es un control de regresión; no certifica este recorrido repetido en el
renderer completo de OpenTTD.

Evidencia en `target/parity/train-consist-repeated-history-20261002/` y el
prototipo previo `target/parity/train-consist-history-prototype-20261002/`.
Permanecen abiertos otros recorridos, longitudes/callbacks, tráfico,
reversa/depots, túneles/puentes y raster completo. FPS continúa pausado.

## #326-STATION-ACTIVE-TRANSFER-REMAINDER — conservar la pausa de carga

Los guards nativos de `OT_LOADING` retornan antes de actualizar velocidad o
movimiento. Tras entrar en una bahía, el primer caso conserva progreso `191`;
el port lo sustituía por `255` en la siguiente pausa. En el tren, el primer
remanente `0` también cambiaba a `255`. Había tres entradas: movimiento,
cierre de ventana y finalización de llegada durante una transferencia activa.

El helper compartido conserva progreso y subspeed de trenes y buses/camiones
con posición física en bahía, con velocidad cero. El movimiento sintético
conserva su contrato anterior. No modifica órdenes, cargas, geometría ni salida.

Los oracles extraen sin modificar los guards de carga de `RoadVehTick` y
`TrainLocoHandler`. El corpus vial parte de los 816 casos de entrada de carga
y repite el guard 1/2/10/100 veces: **3.264 filas**. El ferroviario enumera
512 posiciones/remanentes de carga, tres subspeeds y cuatro repeticiones:
**6.144 filas**. Se adaptan flags de orden, almacenamiento y llamadas externas;
se omiten mantenimiento de servicio/órdenes, transferencia real de carga,
contadores de tick, callbacks/RNG, partida completa y salida de la estación.

Las regresiones comprueban carga y descarga por separado, bus/camión y tren:
**25.344 estados** por entrada del port. Verifican progreso, subspeed y
velocidad exactos; el movimiento comprueba además frame/coordenadas/rumbo
viales, tesela/píxel/rumbo ferroviarios y conservación de ventana/orden.
Los otros dos puntos de pausa verifican fracciones y ventana contra los mismos
valores nativos. Las dos regresiones de movimiento fallan antes del arreglo.
Los 432 estados de llegada, 1.728 ticks de entrada de carga, 512 poses móviles
y 1.024 poses detenidas anteriores se reproducen byte a byte sin cambios.

```bash
python3 scripts/oracle_road_bay_arrival.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/station-road-held-fresh --held --check
python3 scripts/oracle_train_subtile_motion.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/station-train-held-fresh --transfer-held --check
cargo test -p openttdrs-core --test native_station_held_remainder
cargo test -p openttdrs-core --lib station_window_phases_preserve_native
```

Validación general: **3.057 core / 1.701 client**, sin fallos, 6/2 ignorados;
ambos Clippy estrictos, formato, documentación y diff pasan. Client tests usa
`CARGO_INCREMENTAL=0`, sin cambiar la configuración del proyecto.

Release reconstruida en **79.16 s**, core `fresh:false`,
client SHA-256 `72291d8d8c8ef94274956afa94ab6c751d33692dde053597bce871503dd3c844`. Los
[controles congelados](evidence/station-held-remainder-control-raster-20261002.csv)
en seis escalas y `.125` limitada a `.25` dan PNG, cámara principal y
orden completo idénticos. En `.5`/`2` coinciden todas las entradas de sprites,
**265/368 buffers CPU**, cobertura y oclusión. Kale pausado no certifica una
transferencia de carga real ni el renderer completo de OpenTTD.

Evidencia privada en `target/parity/station-held-remainder-20261002/`.
El cierre es sólo de la conservación durante transferencia activa. Siguen
abiertos espera sin transferencia, full-load/horarios, salida, servicio,
tráfico y recorridos completos. FPS permanece pausado.

## #326-ROAD-BAY-BUSY-EXIT — detenerse ante la boca ocupada

El bloque nativo de salida de bahía pone `cur_speed = 0` si la entrada está
ocupada y retorna sin mover frame, coordenadas, rumbo, progreso ni subspeed.
El port sólo retornaba. La primera divergencia: tabla 32, frame 20, velocidad
de entrada `1`; OpenTTD queda en `0` y el port conserva `1`. El arreglo añade
la asignación nativa antes de retornar; no cambia ocupación ni selección de ruta.

`oracle_road_bay_busy_exit.py` reutiliza los fragmentos de llegada, tablas y
dirección sin modificar. Adapta orden de salida ya cargada, almacenamiento,
bit de ocupación y repetición. Enumera 16 tablas, cuatro velocidades, tres
subspeeds, cuatro remanentes y 1/2/10/100 llamadas con boca ocupada; la boca
libre tiene una sola llamada. Son **3.840 filas**, **7.680 estados** de
bus/camión; antes divergen **4.608**. El primer paso libre coincide en todos
los casos. La ocupación nativa se adapta con otro vehículo en el tramo de
entrada; no certifica equivalencia del pool/gestión de tráfico.

La regresión comprueba retorno, velocidad, subspeed, progreso, frame, XY,
rumbo y estado de entrada exactos. Con ruta de salida activa comprueba además
**30.720 poses** retenidas, en cinco alphas, con tolerancia `0.0001` y rumbo
exacto. Es presentación del port sobre coordenadas físicas nativas; no ejecuta
el renderer de OpenTTD. El oracle omite tick/integración de velocidad, servicio,
procesamiento de órdenes, efectos de liberar orden, callbacks/RNG y recorrido
completo. El corpus se reproduce byte a byte y coincide con el prototipo previo.

```bash
python3 scripts/oracle_road_bay_busy_exit.py \
  --openttd reference/openttd-15.3-oracle \
  --out /tmp/road-bay-busy-exit-fresh --check
cargo test -p openttdrs-core --test native_road_bay_busy_exit
```

Validación general: **3.058 core / 1.701 client**, sin fallos, 6/2 ignorados;
ambos Clippy estrictos, formato, documentación y diff pasan. Client tests usa
`CARGO_INCREMENTAL=0`, sin cambiar la configuración del proyecto.

Release reconstruida en **77.95 s**, core `fresh:false`,
client SHA-256 `30dc1bda7778ff3c85f24fdd44345a2b292f3e2abee1aba88b3367adb6580031`. Los
[controles congelados](evidence/road-bay-busy-exit-control-raster-20261002.csv)
en seis escalas y `.125` limitada a `.25` dan PNG, cámara principal y
orden de composición completos idénticos. En `.5`/`2` coinciden
**265/368 buffers CPU**, cobertura y oclusión; en `2`, también todas las
entradas de dibujo. En `.5` quedan **17.703 campos distintos de meshes** en
el orden de consulta, con sprites/cámaras/assets idénticos. Se conserva el
diff completo, sin ordenar ni filtrar los inputs. La
[repetición del mismo binario](evidence/road-bay-busy-exit-control-repeat-20261002.csv)
reproduce esos mismos 17.703 cambios; la segunda captura del corregido
coincide completamente con el anterior. Es una variación previa de consulta,
que se registra sin cerrar la brecha del renderer global. Kale pausado no
certifica una salida con tráfico real ni el renderer completo de OpenTTD.

Evidencia privada en `target/parity/road-bay-busy-exit-20261002/` y
`target/parity/road-bay-busy-exit-prototype-20261002/`. El cierre cubre sólo el
subpaso bloqueado y la presentación detenida. Siguen pendientes tick completo,
salida/continuación de servicio, tráfico, cadenas articuladas y viaje completo.
FPS sigue pausado; #326/#329 permanecen abiertos.

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
