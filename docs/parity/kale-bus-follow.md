# Servicio controlado del bus de Kale

Actualizado: **2026-10-10**. Corte acotado posterior al seguimiento del tren y
a la migración a Bevy 0.20. FPS permanece aplazado.

La fuente es `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`.
Se usa su bus **1888**, motor vanilla 0, capacidad de 40 pasajeros, con las
tres órdenes existentes: estación 188 `(116,75)`, estación 177 `(117,51)` y
depósito 58 `(118,78)` con unbunch. No se modifica el SAV original.

La copia retiene ese bus y su lista compartida, conserva los pasajeros de las
estaciones y desactiva IA, desastres y averías. Avanza hasta el punto físico
de servicio de la primera parada antes de guardarse. No emite comandos de
construcción ni cambia las órdenes; el mundo sigue simulándose durante ese
avance. No representa una prueba de tráfico con la flota completa.

## Preparación y criterio de aceptación

Desde la raíz del repositorio:

```sh
mkdir -p artifacts/transport-follow
cargo run --release --locked --offline -j2 -p openttdrs-core \
  --example kale_bus_follow -- save/Kale_TitleGame.sav \
  save/Kale_bus_follow.json artifacts/transport-follow/kale-bus.csv
```

El ejemplo exige carga nueva, entrega final y pago, visita a la segunda
parada, paso físico por el depósito y regreso al frame 11 de la primera
parada dentro de 20.000 ticks. La entrega en la segunda parada se contabiliza
por separado para que un pago inicial no pueda ocultar una ruta improductiva.
Las órdenes deben conservarse durante todo el ciclo.

Guarda JSON con pasajeros a bordo durante el viaje. Exige igualdad del hash
completo inmediatamente después de cargar y al terminar. Compara vehículo,
compañías, estadísticas, relojes y ambos RNG cada tick, y el estado completo
cada 64 ticks. Esta cadencia evita serializar el mapa 256×256 dos veces por
tick; el test pequeño de persistencia compara hashes completos en cada tick.
Escribe `kale-bus.summary.json` con un resultado falso antes de preparar la
partida, y sale con error si no satisface el contrato.

## Diferencias corregidas

Las reglas se contrastaron con el código de OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`:

- `IndividualRoadVehicleController`, `roadveh_cmd.cpp`: la parada de paso
  inicia `BeginLoading` en `RVC_DRIVE_THROUGH_STOP_FRAME = 11`, y marca
  `RVS_ENTERED_STOP`. Antes se permitía transferir pasajeros al entrar en la
  tesela, antes de alcanzar el punto de parada.
  La selección de `_road_drive_data` usa `RVSB_ROAD_STOP_TRACKDIR_MASK = 0x09`
  dentro de estas paradas: la marca de llegada no debe elegir una curva al
  salir. La regresión cubre cuatro direcciones, ambas manos y adelantamiento.
- `LoadUnloadVehicle`, `economy.cpp`: la descarga termina cuando
  `UnloadCount()` llega a cero. Los paquetes `MTA_KEEP` pueden seguir a bordo
  sin mantener `CargoUnloading` activo. La carga normal ocupa los asientos
  restantes después de esa descarga; no requiere una orden de carga completa.
  La parada conserva la ventana de servicio hasta intentar esa carga en la
  siguiente pasada; completar la descarga no adelanta la orden normal del
  bus. Esta transición se valida para buses y no cierra la carga de camiones,
  tranvías ni vehículos articulados.
- `UpdateOrderDest`, `order_cmd.cpp`: las órdenes de depósito de vehículos
  terrestres y barcos usan el pool `DepotID`; aeronaves usan `StationID`.
  El destino nativo 58 de este bus se interpretaba como la tesela `(58,0)`.
  Importación y exportación ahora resuelven el pool real.
- `OrderDepotActionFlag::Unbunch`, `order_type.h`, y `GetDepotActionType`,
  `order_base.h`: unbunch ocupa el bit 5 de flags, no el bit 4 de
  `NearestDepot`. La fila nativa `type=66, flags=34, dest=58` vuelve al
  depósito y continúa, incluso cuando todavía no necesita servicio.
- `OrderNonStopFlag::NoIntermediate`: el bit 6 significa que el vehículo
  atiende su destino sin detenerse en estaciones intermedias. Las dos
  órdenes `type=97, flags=0` de este bus se importaban con la interpretación
  inversa.
- Los JSON modernos ya persisten los flows publicados y los jobs pendientes
  de CargoDist. La recarga los recalculaba en el loader y al reaplicar un
  catálogo de cargas idéntico; cambiaba paquetes en estaciones y consumía
  RNG. Ahora se conserva la publicación, y un cambio real de catálogo sigue
  reconstruyendo los flows.

Las regresiones cubren frames entrantes 0/1/10 frente a servicio en 11,
descarga con pasajeros retenidos, carga normal con asientos libres, bytes
nativos de órdenes, resolución/exportación del depósito, visita obligatoria
de unbunch y continuación por ambos loaders JSON.

## Comprobación visual

```sh
cargo build --release --locked --offline -j2 -p openttdrs-client
./scripts/follow_vehicle.sh save/Kale_bus_follow.json 1888
```

El launcher también admite el SAV original y cualquier identificador de
vehículo. `follow_kale_train.sh` conserva su interfaz anterior y usa este
launcher común.

Para comprobar los seis zooms y registrar un video en zoom 1:

```sh
python3 scripts/check_kale_bus_follow.py \
  --out "artifacts/transport-follow/live-$(date +%Y%m%d-%H%M%S)" --software
```

El runner necesita Xvfb, Vulkan y ffmpeg. Comprueba identidad del bus, cámara
alineada con el sprite, zoom efectivo, visitas ordenadas a parada/depósito/
origen y actividad económica. A velocidad 8 un frame gráfico puede abarcar
una parada breve; usa también `last_station_visited`, marcado por
`BeginLoading`, para observarla. La traza del core conserva el frame exacto.
`--software` usa Lavapipe; no certifica rendimiento de GPU ni 30 FPS.
La captura predeterminada mide 500 frames, además de 60 de preparación, a
velocidad 8. Exige el ciclo completo aunque el cliente termine con código 0;
un cierre anticipado que no complete el servicio queda rechazado. El video
omite el cursor del servidor X para que no tape el vehículo seguido. El
directorio de salida debe ser nuevo para conservar los intentos anteriores.

## Resultado del ciclo y presupuesto visual

La ejecución final completa el ciclo en **3.615 ticks**, después de 3.077 de
preparación. Sale con 40 pasajeros, carga 53 unidades durante el ciclo y
entrega 19, de las cuales 5 se entregan en la segunda parada. El ingreso del
ciclo es 506. Visita 44 teselas, recorre los ocho sentidos y vuelve al origen
en el frame 11.

La recarga durante el viaje coincide durante 3.426 ticks y 55 comprobaciones
del hash completo. Los hashes inicial y final son, respectivamente,
`7675255356361101857` y `5776246084799143906`. Dos ejecuciones adicionales
obtuvieron los mismos resultados y hashes.

Las seis imágenes fijas de 800×600 se compararon con el binario Bevy 0.20 ya
aceptado y con el mismo oracle nativo, conservando encuadre y escala. El peor
resultado observado es **540 píxeles que empeoran en zoom 0,5**; su repetición
no cambia ningún píxel respecto de la base. Se conservan ambas mediciones y
se usa la peor para el presupuesto. Zoom 8 mejora 12 píxeles; 0,25, 1, 2 y 4
no cambian. Todos están dentro del límite exclusivo de 1.000 píxeles que
empeoran. El gate estricto de cero pérdidas no pasa en la primera captura de
0,5; el gate acordado sí pasa. No se atribuye la variación entre capturas del
mismo binario a una causa todavía no comprobada.

Pasaron **3.082 pruebas del core** y **1.718 del cliente**; se conservan sus
6 y 2 pruebas ignoradas preexistentes. Clippy pasa para ambos paquetes y todos
sus targets con `-D warnings`.

El seguimiento pasa en **0,25, 0,5, 1, 2, 4 y 8**, sin diferencias entre la
cámara y el ancla visible del bus, con al menos una vuelta completa en cada
escala. La tanda inicial terminó sus primeras cuatro escalas antes de
interrumpirse; 4 y 8 se verificaron en ejecuciones separadas. Se conserva el
primer intento de 8 que terminó con código 0 antes de completar el ciclo y
quedó rechazado. La repetición completó las 559 filas de seguimiento y pasó.
Estas interrupciones no se contabilizan como aprobaciones.

La [evidencia del corte](evidence/kale-bus-follow-20261010.json) conserva
resultados, comandos, hashes de fuentes, binarios y capturas, además de los
límites de esta comprobación. La copia preparada está en
`save/Kale_bus_follow.json`; el video local verificado está en
`artifacts/transport-follow/live-video-accepted/1/follow.mp4`. Los artefactos
grandes se regeneran con los comandos anteriores.

## Alcance pendiente

Este caso no cierra paridad tick a tick con una partida nativa en ejecución,
tráfico y adelantamientos con múltiples buses, todas las variantes de
depósito, nearest-depot, órdenes condicionales, tranvías, vehículos
articulados, NewGRF, obras que alteren la ruta ni todos los estados CargoDist.
Los ciclos equivalentes de camiones, barcos y aeronaves quedan para cortes
propios, con guardado en viaje, cámaras fijas y seguimiento en seis zooms.
No se amplía el presupuesto visual acordado: menos de 1.000 píxeles que
empeoran por imagen de 800×600; las mejoras no consumen ese presupuesto.
