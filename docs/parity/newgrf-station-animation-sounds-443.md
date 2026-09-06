# NewGRF station animation sounds — issue #443

Implementado en `main` el 2026-09-06.

## Divergencia

OpenTTD interpreta los bits 8..14 del resultado de `CB140`, `CB141` y
`CB142` como el identificador local de un sonido del GRF. El runtime Rust
aplicaba el byte bajo para seleccionar el frame o el estado de la lista
animada, pero descartaba esos bits; por eso una estación podía cambiar de
frame correctamente y aun así no producir su efecto de sonido.

## Implementación

- `StationAnimationSound` conserva `(GRFID, local_id)` sin mezclarlo con el
  frame `m7` ni con el estado activo de la tesela.
- Las rutas `Built`, `TileLoop`, `NewCargo`, `CargoTaken`, `AcceptanceTick`,
  `VehicleLoads`, `VehicleArrives`, `VehicleDeparts` y `PathReservation` de
  estaciones ferroviarias capturan el identificador devuelto por CB140.
- El scheduler de `TileLoop` también captura los sonidos de CB142 (velocidad)
  y CB141 (siguiente frame), manteniendo el orden callback: velocidad,
  siguiente frame, actualización visual.
- El consumidor de `GameState` resuelve el par contra `sound_effect_catalog` y
  encola `PendingNewgrfSound`; una entrada inexistente o sin PCM queda en
  silencio sin alterar la animación.
- Las APIs públicas históricas siguen devolviendo únicamente `bool` o la lista
  de teselas dirty. Las variantes `..._and_sounds` agregan la captura explícita
  para los call sites que poseen `GameState`.

## Regresión

`station_animation_callback_sound_is_captured_and_played` comprueba un callback
que devuelve frame y sonido simultáneamente, captura CB140 y después verifica
que CB142/CB141 también llegan a la cola en el scheduler. Finalmente valida
volumen/prioridad y el `PendingNewgrfSound` observable.

El alcance no cubre todavía el callback genérico de sonidos ambientales ni los
scopes completos de `BaseStation`; ambos permanecen en el issue padre #329.
