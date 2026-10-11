# Servicio controlado del camión de Kale

Actualizado: **2026-10-10**. Caso acotado del camión 314, posterior al bus
1888. El trabajo de FPS permanece aplazado.

La fuente es `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`.
Se conserva su camión **314**, motor vanilla 13, refitado a mercancías y con
capacidad de **18 unidades**, y las tres órdenes originales, en su orden
nativo:

1. Depósito 12 `(211,145)`, unbunch y continuación.
2. Estación 40 «Nunwood Dock 1», ancla lógica `(206,144)`, carga y descarga
   normales. El camión atiende su parada física unida `(209,145)`.
3. Estación 52 «Rindington Alto», ancla `(202,113)`, descarga y prohibición de
   carga. El servicio se produce en la parada unida `(202,114)`.

La copia retiene ese vehículo, su lista de órdenes compartida y los stocks
existentes. No inyecta mercancías, cambia órdenes ni construye carreteras.
Desactiva IA, desastres y averías, y avanza el mundo hasta el primer servicio
en Nunwood antes de guardar. El SAV original se conserva intacto. Este caso
no representa tráfico con toda la flota de Kale.

## Reproducción y contrato

Desde la raíz del repositorio:

```sh
mkdir -p artifacts/transport-follow/truck
cargo run --release --locked --offline -j2 -p openttdrs-core \
  --example kale_truck_follow -- save/Kale_TitleGame.sav \
  save/Kale_truck_follow.json artifacts/transport-follow/truck/route.csv
cargo build --release --locked --offline -j2 -p openttdrs-client
./scripts/follow_vehicle.sh save/Kale_truck_follow.json 314
```

El ejemplo comparte la instrumentación con `kale_bus_follow`; conserva ambas
interfaces. Exige carga nueva, salida con carga, descarga final en Rindington,
pago, entrada oculta y servicio real en el depósito, salida del depósito y
regreso al punto de carga dentro de 20.000 ticks. Las órdenes deben mantenerse
durante todo el recorrido. El regreso se observa en el frame 11 de servicio;
no requiere otra carga cuando se agota el stock inicial de la copia aislada.

Guarda `route.mid.json` con mercancías a bordo durante el viaje. Compara el
hash completo inmediatamente después de cargar y al terminar, y cada 64
ticks entre ambos puntos. Cada tick compara vehículo, compañías,
estadísticas, relojes, cursor de tile loop y ambos RNG. El contador de
servicios del depósito es un diagnóstico de runtime y se reinicia al cargar;
no sustituye el hash canónico. El resultado se escribe falso antes de preparar
la copia y el proceso falla si no completa el contrato.

Para comprobar el recorrido con cámara en los seis zooms:

```sh
python3 scripts/check_kale_truck_follow.py \
  --out "artifacts/transport-follow/truck/live-$(date +%Y%m%d-%H%M%S)" --software
```

El runner común `check_kale_road_follow.py` admite bus y camión. Usa Xvfb,
Vulkan y ffmpeg, perfiles nuevos y 500 frames medidos más 60 de preparación,
a velocidad 8. Exige identidad, zoom efectivo, alineación de cámara con el
sprite, entrega e ingreso, servicio del depósito y visitas ordenadas a
destino, depósito y origen. A esta velocidad un frame gráfico puede omitir
el único tick oculto dentro del depósito: acepta también el incremento de
servicio en su tesela. La traza del core verifica el estado oculto exacto.
Una salida con código 0 sin completar el ciclo queda rechazada.

El zoom 1 guarda un video sin cursor. `--scales 4` y `--scales 8` permiten
ejecutar las capturas amplias por separado. `--game route.mid.json --scales 1`
abre el checkpoint en el cliente real y comprueba el resto del servicio.
Lavapipe permite reproducir la prueba sin certificar rendimiento de GPU.

## Divergencias corregidas

Se contrastan las reglas con OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`:

- La importación SAV conservaba las teselas unidas de docks y aeropuertos,
  pero omitía las paradas viales. Ahora agrupa también bus, camión y waypoint
  vial por el StationID completo de MAP2. La regresión combina tren, dock,
  bus y camión, y excluye una parada de otra estación.
- `YapfRoad::PfDetectDestinationTile`, `pathfinder/yapf/yapf_road.cpp`, termina
  en una parada vial del StationID y tipo adecuados. El destino lógico de
  Nunwood era un dock inalcanzable para el camión. Se buscan las paradas
  físicas unidas del tipo correcto y se conserva el ancla lógica en las
  órdenes. El controlador reconoce la identidad nativa completa al decidir
  detenerse; la regresión diferencia IDs con el mismo byte bajo.
  La selección actual ordena candidatos por distancia geométrica: no
  reproduce aún la búsqueda multidestino por coste de YAPF ni garantiza
  alternativa si la parada más próxima es inaccesible.
- `VehicleEnter_Road`, `road_cmd.cpp`, oculta el vehículo al alcanzar
  `RVC_DEPOT_STOP_FRAME = 11`, invierte la dirección y entra en
  `RVSB_IN_DEPOT`. Antes la llegada a la tesela adelantaba la orden sin
  entrar físicamente. Además, una curva de la tesela anterior persistía
  dentro del depósito; ahora se elige su trackdir recto de entrada.
- `VehicleEnterDepot`, `vehicle.cpp`, atiende el vehículo antes de procesar
  las órdenes. `RoadVehLeaveDepot`, `roadveh_cmd.cpp`, espera el tráfico y
  el unbunch, atiende de nuevo y sale desde el frame 6 de la tabla vial.
  Se conservan las órdenes de parada, refit y continuación existentes. Las
  regresiones cubren entrada en las cuatro bocas, ambas manos, halt y
  continuación, y salida con espera de unbunch y bloqueo de tráfico.
- `DeliverGoods`, `economy.cpp`, consume todas las unidades cuando las
  casas aportan el bit `always_accepted`, aunque no haya industria receptora.
  El programa pagaba la descarga de mercancías de este camión pero volvía a
  encolarlas. Ahora conserva la contribución de las casas por separado y
  registra una sola entrega final al alcanzar el umbral de aceptación. La
  regresión usa una casa bajo el umbral y dos que lo alcanzan, repitiendo la
  descarga para detectar consumo duplicado.

## Resultado del recorrido

El camión carga y entrega **18 unidades** en Rindington y obtiene **446** de
ingreso. Completa el ciclo en **2.639 ticks**, después de 2.114 de preparación,
y vuelve a Nunwood en el frame 11. Recorre 40 teselas y los ocho sentidos.
Llega al destino en el tick relativo 1.084 y entra en el depósito en el 2.120;
el contador distingue los dos servicios nativos de entrada y salida.

El guardado ocurre en el tick relativo 139, con mercancías en viaje. La rama
cargada coincide durante los **2.500 ticks** restantes y en **40 controles del
hash completo**. Los hashes inicial y final son `3032763334350831802` y
`15273303786613245903`. La repetición obtiene los mismos resultados y hashes.

El bus 1888 también completa el contrato reforzado: carga nueva, entrega,
entrada oculta y salida del depósito, regreso y recarga determinista. La
importación de paradas unidas cambia su punto físico de inicio a `(117,76)`;
las cifras del corte anterior permanecen como evidencia histórica, no como
resultado de esta nueva ejecución.

El seguimiento pasa en **0,25, 0,5, 1, 2, 4 y 8**: cada escala conserva 559
filas, completa destino, depósito y regreso, y no presenta diferencias entre
la cámara y el ancla visible del sprite. El cliente real también abre el
checkpoint en viaje y completa el resto del servicio. El runner compartido
del bus pasa una nueva ejecución gráfica en zoom 1.

Las seis imágenes fijas de 800×600 usan el encuadre histórico `(132,2)`,
180 frames de asentamiento y la GPU de escritorio. Frente al corte del bus
y al mismo oracle nativo, el peor resultado es **11 píxeles que empeoran en
zoom 8**. Zoom 0,5 mejora 540; 0,25, 1, 2 y 4 no cambian. Pasa el presupuesto
acordado, aunque el gate adicional de cero pérdidas no pasa en zoom 8.
La variación de 0,5 ya estaba observada entre capturas del corte anterior;
no se le atribuye una causa nueva sin comprobarla.

Pasaron **3.086 pruebas del core** y **1.718 del cliente**, con sus 6 y 2
pruebas ignoradas preexistentes. El test final del depósito cubre también
entrada después de una curva y conservación del refit. Clippy pasa en ambos
paquetes y todos los targets con `-D warnings`.

Los primeros intentos de seguimiento en 0,25 y 4 terminaron con código 0
antes de completar el ciclo; quedaron rechazados. Una primera tanda raster
se interrumpió sin PNG de 0,5, y la primera ejecución de tests del cliente
terminó con señal 143. Las repeticiones completas se conservan por separado.
Estas ejecuciones parciales no cuentan como aprobaciones.

Quedan cerrados para esta fixture los contratos locales **KTR-STOPS**
(paradas unidas), **KTR-DEPOT** (servicio y salida), **KTR-DELIVERY**
(mercancías aceptadas por casas), **KTR-RELOAD** (continuación guardada) y
**KTR-ZOOM** (cámara y presupuesto raster).

## Evidencia y límites

Los resultados del recorrido, repetición, recarga, cámaras, raster y checks
se conservan en [la evidencia del corte](evidence/kale-truck-follow-20261010.json).
Los artefactos grandes locales se regeneran con los comandos anteriores.

Se comprueba un recorrido jugable de un camión existente. No se certifica
paridad tick a tick con una partida nativa en ejecución, importes nativos,
tráfico con la flota completa, vehículos articulados, tranvías, nearest-depot,
obras durante el recorrido ni todas las combinaciones de CargoDist y NewGRF.
La contribución de casas usa la tabla de aceptación ya soportada; callbacks
de casas y oficinas centrales quedan fuera de esta corrección. La aceptación
industrial parcial conserva la semántica previa y requiere su propio caso.

El presupuesto visual sigue siendo menos de **1.000 píxeles que empeoran**
por imagen de 800×600; las mejoras no consumen ese límite. Las cámaras en
movimiento y las capturas raster fijas comprueban contratos diferentes.
Barcos y aeronaves siguen pendientes de casos propios. El corte termina con
este camión, según el alcance solicitado.
