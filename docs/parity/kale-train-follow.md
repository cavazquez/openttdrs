# Recorrido controlado del tren de Kale

Actualizado: **2026-10-10**. Caso acotado de movimiento, parada, transferencia
y regreso; la paridad completa de flotas y señalización sigue abierta.

La fuente es `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`.
La fuente se conserva intacta. Se usa su tren **2902**, con las unidades
2926, 3038, 3061, 3063, 3064, 3066, 3071, 3073, 3118, 3119 y 3121.
La carga importada es mineral de hierro.

La copia `save/Kale_train_follow.json` conserva el mapa, las estaciones, las
vías, las señales y ambas órdenes originales. Se retiran los demás vehículos,
se liberan sus reservas y se ejecuta una vez `SkipVehicleOrder` para salir de
la carga inicial. Las cuatro unidades de carga reservadas pendientes vuelven
a la estación de origen; salen **326 unidades** de carga almacenada.

Las órdenes son carga completa en Arboleda de Cunfingway `(51, 6)` y
transferencia sin carga en Arboleda de Beham `(130, 1)`. La orden de
transferencia no acredita una entrega final ni un nuevo pago.

## Preparar y comprobar la copia

Desde la raíz del repositorio:

```sh
mkdir -p artifacts/train-follow
cargo run --release -p openttdrs-core --example kale_train_follow -- \
  save/Kale_TitleGame.sav save/Kale_train_follow.json \
  artifacts/train-follow/route.csv
```

El ejemplo guarda la copia antes de avanzar y luego traza cada unidad en cada
tick. Produce `route.summary.json`, incluso si falla, y sale con error si no
confirma llegada, carga vacía y regreso dentro de 20.000 ticks. No amplía la
cuota de tamaño de los saves JSON. La alternativa `--without-signals` es un
control de diagnóstico y modifica la señalización; no es el caso de aceptación.

La ejecución controlada con señales confirmó:

- Llegada al punto de parada de la plataforma `(135, 1)` en el tick 3.262.
- Transferencia de toda la formación y salida hacia el origen en el tick 3.270.
- Regreso y entrada en carga en `(51, 6)` en el tick 5.601.
- 102 teselas visitadas por la cabeza y las ocho direcciones de movimiento.
- 67.224 filas de traza, correspondientes a doce unidades y 5.602 estados.

Estos ticks describen esta ejecución del core; no son una comparación de
cadencia tick a tick con una partida OpenTTD en ejecución.
El [registro de aceptación](evidence/kale-train-follow-20261010.json) conserva
el resumen, los hashes y el alcance. Pasaron 3.072 tests del core, con seis
ignorados, y Clippy estricto de todos sus targets.

## Reglas corregidas

Se contrastaron las reglas con OpenTTD
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`:

- `VehicleEnter_Station` y `TrainEnterStation` inician la transferencia cuando
  la locomotora alcanza su parada. Un vagón que entra primero no debe frenar
  la cabeza ni empezar a descargar mientras la formación sigue entrando.
- La ruta de teselas puede agotarse antes de alcanzar el píxel nativo de
  parada. La sincronización de órdenes conserva ese destino hasta `BeginLoading`.
- `LoadUnloadVehicle` utiliza la estación registrada por la locomotora para
  toda la formación, incluyendo vagones fuera de una plataforma corta.
- `ReserveConsist` pertenece a la estación actual de la cabeza. Al importar
  un SAV se reconstruyen los enlaces antes de reconciliar `MTA_LOAD` de los
  vagones que no guardan ese identificador de estación.
- `CmdSkipToOrder` llama a `LeaveStation` durante la carga. Devuelve las
  reservas pendientes de toda la formación y conserva el resto de la carga.
- `TrainLocoHandler` reintenta `TryPathReserve` cuando vence la espera de un
  tren atascado. Tener la siguiente tesela reservada no permite omitir ese
  reintento si la reserva todavía no llega a un lugar seguro.

Antes de la última corrección el tren quedaba detenido en `(91, 6)` después
de 20.000 ticks: reservaba la siguiente tesela, pero su extremo anterior
acababa en `(133, 1)`, antes del nuevo punto de parada. El reintento completo
al vencer la espera elimina ese bloqueo sin quitar señales.

Las regresiones del core cubren entrada parcial de vagones, parada subtesela,
plataforma corta, asociación de reservas del SAV, devolución al saltar una
orden y reintento PBS. Este caso no cierra circulación de múltiples trenes,
cruces, colisiones, todos los NewGRF ni paridad raster global.

## Seguir el tren en el cliente

```sh
cargo build --release -p openttdrs-client
./scripts/follow_kale_train.sh save/Kale_train_follow.json
```

Para abrir la partida original completa:

```sh
./scripts/follow_kale_train.sh
```

El seguimiento se activa con `OPENTTDRS_FOLLOW_VEHICLE=2902`. Usa la posición
interpolada y el ancla visible del sprite; conserva el zoom y la profundidad
de cámara. En un túnel continúa con la posición lógica interpolada. El cliente
normal no instala estos sistemas si la variable está ausente.

`OPENTTDRS_FOLLOW_TRACE_OUT=follow.csv` permite comprobar coordenadas de cámara
y sprite por frame. Las pruebas ECS cubren los seis zooms, vehículos detenidos
y en movimiento, el ancla renderizada y el caso oculto. El seguimiento visual
es una comprobación adicional; no sustituye las trazas del core ni la
comparación raster de la migración a Bevy 0.20.

La ejecución gráfica con Bevy 0.20 y Vulkan/Lavapipe comprobó los seis zooms:
`0.25`, `0.5`, `1`, `2`, `4` y `8`. No hubo desvíos entre el ancla visible
y la cámara en ninguno de los 3.454 frames registrados. La sesión de zoom
`1` avanzó 13.255 ticks, recorrió 102 teselas y observó las ocho direcciones;
las demás sesiones avanzaron entre 2.837 y 2.857 ticks. Esta prueba usa la
copia con las señales originales y no certifica FPS ni paridad raster.
