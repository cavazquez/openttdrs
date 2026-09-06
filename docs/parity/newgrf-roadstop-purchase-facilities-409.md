# RoadStop `0xF0` en el picker (#409)

Actualizado: **2026-09-06**.

`RoadStopScopeResolver::GetVariable(0xF0)` devuelve las facilities de la
estación sólo cuando existe `st`; durante la compra (`st == nullptr`) devuelve
`0`. El selector no debe inferir bus/truck/waypoint como una entidad ya
construida.

La variante contextual de `apply_road_stop_availability_callback` publica ahora
`0xF0=0` junto con los demás sentinels del picker. Las paradas colocadas siguen
usando `StopKind::facilities_mask()` en `road_stop_action2`, por lo que el
render y la animación no pierden sus bits reales.

La regresión contextual de disponibilidad ejecuta un callback que compara
`0xF0` con cero; no se crea una estación temporal ni se altera `7C`. Matriz y
plan continuo registran la diferencia y el issue padre [#329](https://github.com/cavazquez/openttdrs/issues/329)
permanece abierto para scopes restantes.
