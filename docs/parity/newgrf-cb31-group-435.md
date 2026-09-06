# CB31 en arranque/parada de grupos (#435)

Actualizado: 2026-09-06.

`SetVehicleGroupRunning` ahora hace un preflight de todos los vehículos del
grupo antes de modificar cualquier bit `running`. Para cada unidad se conserva
la validación de espera de horario en depósito y se ejecuta
`CBID_VEHICLE_START_STOP_CHECK` (CB31) con el mismo resolver que usan los
botones individuales. Si una unidad rechaza la operación, el comando termina
con `NewGrfCallbackDenied`, no deja cambios parciales y conserva el diagnóstico
efímero de la unidad que rechazó para que la lista de vehículos muestre el
motivo textual de #434. Las operaciones de parada siguen evaluando CB31 y sólo
aplican el cambio cuando toda la flota fue aceptada.

La regresión `vehicle_group_running_checks_cb31_atomically` combina una unidad
vanilla y una unidad NewGRF que devuelve `0x010`, verifica que ambas permanezcan
detenidas y que el diagnóstico retenga el `vehicle_id` y el `LocalString`
esperado. El feedback de la acción de grupo usa el catálogo NewGRF y el locale
activo; errores sin diagnóstico mantienen el mensaje genérico.

No se persiste el diagnóstico ni se reejecuta el callback durante el feedback.
La semántica de callbacks para autoreemplazo y órdenes de depot permanece fuera
de este sub-issue y continúa en #329.
