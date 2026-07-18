# Paridad de ventanas: depósito, trenes y órdenes

Fecha: 2026-07-09 · Actualizado tras Fase 1 consist (core + UI MVP).
Compara las ventanas/paneles del cliente Bevy (`openttdrs-client/src/ui/`)
contra las ventanas reales de OpenTTD (`depot_gui.cpp`, `vehicle_gui.cpp`,
`train_gui.cpp`, `order_gui.cpp`, `timetable_gui.cpp`, `build_vehicle_gui.cpp`,
`group_gui.cpp`).

> Este documento profundiza en flota y conserva un snapshot histórico.
> El roadmap global y su baseline actualizado están en
> [ROADMAP_PARIDAD_UI_GLOBAL.md](../ROADMAP_PARIDAD_UI_GLOBAL.md).

## Clasificación de cercanía alcanzable

Para cada feature se indica qué tan cerca podemos llegar y qué lo limita:

- **✔** — ya hay paridad funcional (la acción existe y hace lo mismo, aunque
  el layout difiera).
- **A (solo UI)** — alcanzable únicamente tocando el cliente; el comando o el
  dato ya existen en `openttdrs-core`.
- **B (comando chico)** — requiere agregar un comando o campo pequeño en la
  sim, sin cambios estructurales.
- **C (bloqueado por la sim)** — depende de una carencia estructural
  (p. ej. PBS multi-tesela fina, averías/servicio, beneficio por vehículo).
  El **consist ya existe** en core (Fase 1); lo que falta es pulido de UI.

Conclusión: comandos de flota siguen cerca; depósito/compra ya enganchan
vagones (MVP). Falta matriz horizontal con sprites por unidad y drag nativo.

## 1. Ventana de depósito

OpenTTD: `DepotWindow` (`depot_gui.cpp:261-1166`). Cliente:
`ui/toolbar/depot_panel.rs` (ventana flotante `FloatingWindowId::Depot`,
abre con clic en tile `RoadDepot`/`RailDepot`).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Matriz de vehículos con sprites (`WID_D_MATRIX`, `DrawTrainImage`) | Filas de texto (8 slots): nombre, grupo, edad, carga | **A** — dibujar el sprite del vehículo en la fila es solo UI |
| 1 fila = 1 consist (loco + vagones, scroll horizontal) | 1 fila = cabeza; label `[Nu]` unidades | **A** — falta scroll horizontal con sprites por vagón |
| **Drag & drop de vagones** (`MoveRailVehicle`, formar/partir trenes, Ctrl = cadena) | Drag sprites/filas + Ctrl=`move_chain`; clic A→B también | ✔ |
| Ctrl+soltar sobre sí mismo = `ReverseTrainDirection` en depósito | Botón «Dar la vuelta» en ventana de vehículo | ✔ funcional (gesto distinto) |
| Vender arrastrando a `WID_D_SELL` / vender cadena | Zonas drop «Vender»/«Cadena» + ✕ por fila; Ctrl en drop = cadena | ✔ |
| Vender todo (`DepotMassSell`) | Botón «Vender todo» (`SellAllVehiclesAtDepot`) | ✔ |
| Comprar (`WID_D_BUILD` → `BuildVehicleWindow`) | Botón «Nuevos vehículos» → `buy_window` | ✔ |
| Clonar (`CloneVehicle`, Ctrl = compartir órdenes) | Botones «Clonar» (`CloneVehicleAtDepot`) y «Compartir órdenes» separados | ✔ (la variante Ctrl es A) |
| Parar/arrancar todos (`MassStartStop`) | Botones «Parar todos»/«Arrancar todos» (`SetDepotVehiclesRunning`) | ✔ |
| Autoreemplazo masivo (`DepotMassAutoreplace`) | Botones autoreemplazo + regla + «solo viejos» | ✔ (el cliente incluso expone más que la ventana de depósito de OpenTTD) |
| Bandera start/stop por celda | ▶/■ por fila (`ToggleVehicleRunning`) | ✔ |
| Renombrar depósito (`RenameDepot`) | No existe | **B** — falta nombre de depósito en core |
| Tooltip de carga con clic derecho | No existe | A (bajo valor) |
| Lista de vehículos del depósito (`WID_D_VEHICLE_LIST`) | Las 8 filas cumplen ese rol | ✔ parcial (sin scroll: >8 vehículos quedan ocultos → **A**) |
| Ir al tile (`WID_D_LOCATION`) | Botón «Centrar» | ✔ |

Extras del cliente sin equivalente en la ventana de OpenTTD: reordenar slots
(↑/↓), «Copiar órdenes» por fila, ciclo de grupo. No son divergencias: son
azúcar propio.

## 2. Ventana de vehículo (vista)

OpenTTD: `VehicleViewWindow` (`vehicle_gui.cpp:3007-3503`). Cliente:
`ui/vehicle_window.rs` (flotante, abre con clic en el vehículo en el mapa).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Viewport siguiendo al vehículo (`WID_VV_VIEWPORT`, zoom) | Cámara render-target 280×120 (preview real del mundo) | ✔ esencial (seguir con doble clic es A) |
| Barra de estado (`GetVehicleStatusString`): velocidad + destino + «parado» + averiado + atascado | Status corto bajo viewport (#174): Detenido / En marcha a X km/h → destino / Sin ruta / Averiado / PBS | ✔ esencial |
| Start/stop (`StartStopVehicle`) | Icono ▶/■ + tooltip (`ToggleVehicleRunning`) | ✔ |
| Toolbar de iconos (vista) | Fila de iconos + tooltips (#174); Horario/Detalles/Depósito/… | ✔ chrome; sprites GUI nativos OpenTTD opcionales |
| Ir a depósito (`SendVehicleToDepot`, Ctrl = servicio) | «Depósito» (`AppendGotoNearestDepot`) | ✔ funcional; la variante «servicio» es **C** (no hay intervalos de servicio) |
| Refit (`ShowVehicleRefitWindow`) | `RefitWindow` lista + coste/cap.; View y Details; parcial por unidad | ✔ (#178); `OrderRefit` sigue **B** |
| Clonar desde la ventana | Solo desde el depósito | **A** |
| Dar la vuelta (`ReverseTrainDirection`/`TurnRoadVehicle`) | «Dar la vuelta» (`TurnAroundVehicle`, solo tren) | ✔ tren; road es **B** |
| Forzar paso (`ForceTrainProceed`) | «Forzar paso» (`ForceVehicleProceed`, solo tren) | ✔ |
| Órdenes / horario (Ctrl) | Botones «Órdenes» y «Horario» separados | ✔ |
| Detalles (`ShowVehicleDetailsWindow`) | Ventana `VehicleDetails` (#173/#175); filas por unidad + sprites | ✔ |
| Ir al destino de la orden (`WID_VV_ORDER_LOCATION`) | Botón «Ir a orden» | ✔ |
| Renombrar (`RenameVehicle`) | Campo de renombrado inline | ✔ |

## 3. Ventana de detalles del vehículo

OpenTTD: `VehicleDetailsWindow` (`vehicle_gui.cpp:2436-3006`) +
`DrawTrainDetails` (`train_gui.cpp:359-471`). Cliente: `ui/vehicle_details_window/`
(`FloatingWindowId::VehicleDetails`, #173/#175) con tabs Info/Carga/Capacidad/Totales
y **una fila por unidad** (sprite lateral + texto según tab; scroll).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Edad + vida útil | Filas Details (Info) + depósito | **A** |
| Beneficio este año / anterior | Resumen tab Totales | ✔ (campo en vehículo) |
| Peso/potencia/esfuerzo tractor (TE) | Peso/potencia por unidad y consist | **A** para peso/potencia; TE es **B** |
| Fiabilidad + nº de averías | Fiabilidad en fila Info; averías no | Fiabilidad ✔; averías **C** |
| Intervalo de servicio (`ChangeServiceInterval`, dropdown días/%/min) | No existe | **C** — no hay servicio en la sim |
| **Lista de vagones con 4 pestañas** (cargo/info/capacidad/totales por vagón) | Filas con sprite + datos por tab (#175) | ✔ |

Con tren puntual, lo máximo alcanzable hoy es una ventana de detalles de
«una unidad»: edad, peso/potencia, coste, fiabilidad, carga — todo A/B.

## 4. Ventana de órdenes

OpenTTD: `OrdersWindow` (`order_gui.cpp:499-1755`). Cliente:
`ui/toolbar/order_panel/` como **ventana flotante** (`FloatingWindowId::Orders`, #176);
copia local editable + `SetVehicleOrderList`. Se abre desde View / estación /
picker; no ocupa el borde derecho de forma permanente.

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista de órdenes con orden activa | Sí (32 slots, resaltado, marcador `>`) | ✔ |
| Insertar por clic en mapa (`GetOrderCmdFromTile`) | Sí: picker de destino + clic en mapa + `destination_window` | ✔ |
| Skip / delete / reordenar (drag) | Saltar, Borrar, ↑/↓ + drag nativo (#194) | ✔ |
| Full load (variantes any/all) | Flag «Carga compl.» (una variante) | ✔ básico; variantes **B** |
| Unload / **transfer** / no unload | Solo «No descargar» | unload forzado y transfer son **B** (transfer necesita feeder share en core → más bien **C**) |
| **Non-stop / go via** | No existe | **C** hoy: la sim no tiene paradas intermedias implícitas (los vehículos no paran en estaciones de paso), así que non-stop es el único comportamiento; documentado como divergencia semántica, no como botón faltante. Cambia si se implementa `ShouldStopAtStation` |
| Acción de depósito en orden (always/service/halt/unbunch) | «Parar depós.» (equivale a halt) | ✔ halt; service/unbunch **C** (no hay servicio) |
| **Refit en orden** (`OrderRefit`) | No existe | **B** |
| Condicionales (variable+comparador+valor) | Sí, limitado (carga >50 %, salto fijo) | **B** para más variables/comparadores (el core ya tiene `Conditional`) |
| **Stop location de trenes (near/middle/far)** (`MOF_STOP_LOCATION`, doble clic) | No existe | **C** hasta la Fase Rail 3C: sin entrada a plataforma el punto de parada no significa nada. Portarlo junto con `GetTrainStopLocation` |
| Órdenes compartidas (lista de vehículos, stop sharing) | Crear/desvincular desde depósito; sin lista de compartidos | **A** para la lista; la mecánica ya existe |
| Ir a depósito más cercano (dropdown GOTO) | `AppendGotoNearestDepot` desde ventana vehículo | ✔ |
| Waypoints en órdenes | Sí (solo trenes, sin parada completa) | ✔ |

Extras del cliente: tiempos de espera/viaje editables inline, «Poner en
hora», vaciar lista — equivalentes a piezas de la ventana de horarios de
OpenTTD.

## 5. Ventana de horarios (timetable)

OpenTTD: `TimetableWindow` (`timetable_gui.cpp:174-863`). Cliente:
`ui/timetable_window.rs` (**sí existe**, flotante).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Tiempos de espera/viaje por orden | Sí (8 filas) | ✔ |
| Autofill | Sí | ✔ |
| Reset de retraso (`SetVehicleOnTime`) | «Poner en hora» | ✔ |
| Resumen retraso/adelanto | Sí | ✔ |
| **Velocidad máxima por tramo** | No existe | **B** (campo por orden + clamp en `update_movement_speed`) |
| Fecha de inicio (`SetTimetableStart`) | No existe | **B**, valor moderado con tick ~37 Hz |
| Llegada/salida esperadas por orden | No existe | **A** (derivable de los tiempos) |

## 6. Ventana de refit

OpenTTD: `RefitWindow` (`vehicle_gui.cpp:753-1358`) con selección parcial del
consist por drag. Cliente: `ui/refit_window.rs` (`FloatingWindowId::Refit`).

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista de cargas con coste/capacidad | Filas `nombre · cap. N · gratis`; coste real **B** | ✔ UI (#178) |
| Abrir desde View / Details | Botón View + «Refit» en Details | ✔ |
| Selección parcial del consist | Toggle de unidades + `unit_ids` en `RefitVehicle` | ✔ (sin drag nativo) |
| Refit como orden (`OrderRefit`) | No existe | **B** (ver §4) |

## 7. Compra de vehículos

OpenTTD: `BuildVehicleWindow` (`build_vehicle_gui.cpp:1216+`). Cliente:
`ui/buy_window.rs`.

| Feature OpenTTD | Estado en cliente | Cercanía |
|---|---|---|
| Lista con orden asc/desc y ~11 criterios | Orden por nombre/precio/velocidad/año | ✔ básico; más criterios **A** |
| Matriz con sprite por fila | Sprite + nombre/precio por fila (#179) | ✔ chrome; preview grande sigue abajo |
| Filtro por cargo / texto / motores ocultos / badges | Filtro todos/buses/camiones (solo road); rail lista loco+vagón | **A** |
| Panel de detalle (coste, peso, velocidad, potencia, **TE**, running cost, refit) | Sí salvo TE | ✔ (TE es **B**) |
| **Comprar vagones** (`CcBuildWagon` acopla a la loco) | Compra `ENGINE_WAGON_*` + auto-`AttachWagonToConsist` | ✔ MVP |
| Ocultar/renombrar motor (`SetVehicleVisibility`, `RenameEngine`) | No existe | **B**, bajo valor |

## 8. Lista de vehículos y grupos

OpenTTD: `VehicleListWindow` (`vehicle_gui.cpp:1923-2319`) y
`VehicleGroupWindow` (`group_gui.cpp:208-1244`). Cliente: **`VehicleList`
existe** (UI-2, `vehicle_list.rs` / `FloatingWindowId::VehicleList`) con filtro
por tipo y acciones básicas. Grupos dedicados (`VehicleGroupWindow`) siguen
parciales (ciclo de grupo en depósito + HUD).

- Ventana de lista de flota con ordenamiento y acciones masivas
  (`MassStartStop`, enviar todos a depósito): **A/B** — los comandos masivos
  por depósito ya existen; falta la vista global y un `SendAllToDepot`.
- Ventana de grupos (crear/renombrar/borrar, drag de vehículos): **B** — el
  core ya tiene `CreateVehicleGroup`/`AssignVehicleToGroup`; faltan renombrar
  y borrar grupo.

## 9. Auditoría layout entidad (#179)

Checklist OpenTTD vs openttdrs vs acción (epic UI-Layout #172).

| Superficie | OpenTTD | openttdrs | Acción |
|---|---|---|---|
| Estación | `station_gui` viewport + iconos | Panel fijo; barra Ruta/Órd./Loc… (#183) | ✔ chrome; viewport **A** residual |
| Industria | `industry_gui` | `FloatingWindowId::Industry` + preview RT + Loc | ✔ chrome (#179); Authority/catchment **B/C** |
| Pueblo | `town_gui` viewport + iconos | Flotante; barra Loc/Pub/Fondos | ✔ chrome (#179); Authority completa **B** |
| Compra | matriz sprites | Filas con sprite + stats | ✔ chrome (#179); TE/ocultar motor **B** |
| Lista flota | sprites + mass actions | Filas con sprite + start/stop (#182) | ✔ chrome; grupos/mass **A/B** |

Bloqueado por sim / OOS: NewGRF params, cheats, multi-instance, servicio/averías (**C**).

## Resumen: qué tan cerca podemos llegar

| Categoría | Ítems | Veredicto |
|---|---|---|
| Ya en paridad funcional (✔) | start/stop, vender, vender todo, clonar, autoreemplazo, comprar, órdenes básicas + condicionales + skip + reorden, waypoints, horarios con autofill, reversa/forzar paso de tren, centrar/ir a destino, renombrar vehículo | La mecánica de comandos está prácticamente completa para vehículos puntuales |
| Alcanzable solo con UI (A) | sprites en filas de depósito, scroll >8 vehículos, string de estado con destino, edad/peso/potencia en detalles, ventana de refit con lista, lista de órdenes compartidas, drag para reordenar órdenes, llegada/salida esperadas, más criterios de orden en compra, clonar desde ventana de vehículo | Un paquete de trabajo de cliente sin tocar core |
| Comando chico en core (B) | refit en orden, transfer/unload forzado, variantes de full load, más condicionales, velocidad máx. por tramo de horario, renombrar depósito/grupo, TE en `EngineDef`, dar la vuelta para road, ventana de flota/grupos completa | Cambios acotados, sin riesgo de paridad de sim |
| Bloqueado por la sim (C) | PBS/reservas finas, servicio/averías/unbunch, beneficio por vehículo, non-stop/paradas de paso avanzadas | Consist ya no bloquea; ver Fases 2–3 en `ROADMAP_PARIDAD_ESTRUCTURAL.md` |

**El techo actual**: Fase 1 desbloqueó consist en core y un MVP de UI
(compra+enganche, reorden clic A→B, render de trailers, venta de cadena).
El salto siguiente es pulido A (matriz horizontal, drag nativo, pestañas por
vagón) más Fases 2–3 de sim (packets, PBS).

## Orden recomendado si se ataca la UI

1. Paquete A de depósito + vehículo (sprites en filas, scroll, string de
   estado, detalles con edad/peso/potencia) — máxima paridad visible sin
   tocar core.
2. Paquete B de órdenes (refit en orden, unload/transfer básico, variantes
   full load) — cierra la ventana de órdenes casi por completo.
3. Ventana de flota + grupos (A/B) — único subsistema de gestión ausente.
4. Stop location y lo demás de trenes: después de la Fase Rail 3C.
5. Consist: decisión estructural previa (fuera del alcance de UI).

## Tests hoy y huecos

Cubierto: labels de órdenes (`order_row_labels_depots`), sync del panel de
órdenes, pick de destino, añadir estación a ruta, conversión km/h, drag de
ventanas flotantes. Sin tests: `depot_panel`, `buy_window`,
`destination_window`, `timetable_window` y los handlers de botones de la
ventana de vehículo — si se encara el paquete A, agregar tests de sync/handler
por ventana al estilo `setup_order_panel_then_sync_order_panel`.
