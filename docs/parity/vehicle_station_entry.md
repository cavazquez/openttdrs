# Caso de estudio: camión entra a playa de carga

Contrasta la timeline generada por `parity_runner --scenario truck_bay` con las
observaciones de los videos de referencia:

- `openttd.webm` — comportamiento esperado (OpenTTD original).
- `opentddrs.webm` — estado actual del cliente Rust + Bevy.

## Observaciones de los videos

Del video de OpenTTD (camión aproximándose a una bahía de carga):

1. **Desaceleración por curvas**: la velocidad cae 48 → 33 → 31 km/h en las dos
   curvas de 90° previas a la bahía (penalización de −25 % por giro, dos giros
   próximos). Recupera al salir de cada curva.
2. **Detención DENTRO de la bahía**: el camión entra a la tesela de la estación
   y frena en el frame de parada (tabla `_road_stop_stop_frame`, frames 11–20),
   quedando visualmente dentro de la dársena.
3. **Movimiento continuo**: sin saltos; el sprite cambia de orientación en
   sincronía con la trayectoria curva pixel a pixel.

Del video de openttdrs:

1. La velocidad NO baja en las curvas.
2. El camión se detiene en la carretera frente a la parada (nunca entra a la
   tesela de la estación).
3. El movimiento presenta tirones y la orientación del sprite cambia tarde
   respecto a la posición dibujada en las curvas.

## Timeline del runner (traza tras la Fase 2, 500 ticks)

Camión id 1, motor MPS (velocidad interna máx. 96 = 48 km/h). Ruta con dos
curvas de 90° y bahías `TruckStop` en ambos extremos.

| Tick | Evento / estado | Detalle |
|---|---|---|
| 1 | `start` | arranca desde parado (aceleración AM_ORIGINAL) |
| 35 | primer `tile_crossed` | aún acelerando (≈14 ticks/tesela) |
| 90, 130 | `direction_changed` (curvas 90°) | **velocidad 96→72** (−25 %, Fase 2) y recupera acelerando |
| 168 | `tile_crossed` + `station_entry` | **entra a la tesela de la bahía** (4,5) desde el acceso (4,6) (Fase 2) |
| 169 | `loading_started` + `loading_finished` + `order_advanced` | carga 0→20 **en un solo tick** (OpenTTD: gradual — pendiente) |
| 170–178 | `depart_turn_started` … `depart_turn_ended` | media vuelta animada dentro de la bahía |
| 238, 277 | curvas de vuelta | con penalización −25 % en cada giro |
| 315–316 | `station_entry` + `unloading_started/finished` | descarga (instantánea, pendiente) dentro de la bahía destino |
| 462–463 | segundo ciclo de carga | el ciclo es estable y determinístico |

## Qué divergencia del reporte explica cada diferencia visual

| Diferencia visual (videos) | Divergencia (`docs/parity/divergences_found.md`) | Estado |
|---|---|---|
| No frena en las curvas (48 km/h constantes vs 48→33→31) | `curve_speed_penalty` | **CORREGIDA en Fase 2**: `Vehicle::set_direction_with_curve_penalty` aplica −25 % en cada giro (ticks 90/130/238/277 de la traza: 96→72) |
| Se detiene fuera de la dársena | `bay_stop_position` | **CORREGIDA en Fase 2**: el destino es la tesela de la bahía; carga con el camión en (4,5). El punto de parada visual aproxima el stop frame (`BAY_STOP_PROGRESS`); las trayectorias exactas `_rv_station_*` quedan pendientes |
| La pausa de carga parece un frenazo inmediato | `instant_loading` — carga 0→20 en un tick, sin fase de frenado dentro de la bahía | Pendiente (Fase 3) |
| Tirones / baja fluidez general | `tick_rate` — sim a 5 Hz con ~51 unidades de progreso por tick; OpenTTD mueve píxeles a ~33 Hz | Pendiente — ver decisión en `docs/parity/tick_rate_decision.md` |
| Sprite gira tarde en las curvas | corregido en Fase 1: el selector de textura ahora usa la pose extrapolada (`render/vehicles.rs::for_vehicle`); antes usaba `v.render_direction()` lógico | test `sprite_selection_uses_extrapolated_pose_not_logical_direction`; verificable con `OPENTTDRS_RENDER_TRACE` |

## Cómo verificar la parte visual (render vs sim)

```bash
OPENTTDRS_RENDER_TRACE=/tmp/render_trace.csv cargo run -p openttdrs-client
```

El CSV registra por frame: pose lógica (tesela + progress del último tick de
sim), pose extrapolada (lo que se dibuja), `tick_alpha` y `sprite_dir`. Si la
columna extrapolada avanza suave mientras la lógica salta cada 200 ms, el
problema restante es de simulación (tick de 5 Hz), no de interpolación.
