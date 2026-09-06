# RoadStop `0xFA` — fecha de construcción (#408)

Actualizado: **2026-09-06**.

## Referencia nativa

`RoadStopScopeResolver::GetVariable(0xFA)` devuelve
`build_date - CalendarTime::DAYS_TILL_ORIGINAL_BASE_YEAR`, saturado a un WORD.
La misma variable usa la fecha actual en el picker sin estación (cubierto por
#406/#407) y la fecha de `BaseStation` cuando la parada ya existe.

## Implementación

El contexto común de `road_stop_action2` ahora publica `0xFA` tanto en sus
APIs legacy como en las rutas catalogue-aware de render, Action2 y callbacks
de animación. Reutiliza `Station::newgrf_build_date_value()`, que ya aplica la
resta y la saturación nativas para todas las clases de estación. No se crea una
entidad ni se modifica storage persistente durante la consulta.

## Regresión

`road_stop_ctx_exposes_runtime_random_view_type_and_frame` fija una parada en
`STATION_BUILD_DATE_DEFAULT + 123` y verifica que el contexto devuelve
`0xFA=123`, junto con la vista, frame, facilities, random y variables de
propietario existentes.

## Límite

La fecha de build queda cubierta en el scope de una parada colocada y en el
picker. Vecinos/strings/sonidos y las áreas de `BaseStation` aún no modeladas
siguen en [#329](https://github.com/cavazquez/openttdrs/issues/329).
