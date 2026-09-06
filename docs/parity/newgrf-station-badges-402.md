# StationScope badges (`#402`)

Actualizado: 2026-09-06.

OpenTTD resuelve StationScope `0x7A` mediante `GetBadgeVariableResult`: el
parámetro es un índice local de la tabla de badges del GRF y el resultado es
`1` si el badge está asociado a la `StationSpec`, `0` si existe pero no está
asociado y `UINT_MAX` si el índice no se puede resolver.

La implementación Rust cubre el mismo camino para las estaciones catalogadas:

- Action0 Stations prop `0x1F` lee `WORD count + N×WORD local_id`.
- `apply_newgrf_stations` traduce esos índices mediante `GlobalVar 0x18` y el
  `badge_catalog`, preservando `u16::MAX` para labels desconocidos y dejando
  un diagnóstico observable.
- `StationSpecDef` conserva `associated_badges` y la traducción local de
  runtime; los datos de runtime se rehidratan al aplicar el stack NewGRF.
- Los contextos `action2_eval_ctx_for_station_tile_with_catalog*` publican
  `parameterized_vars[(0x7A, parameter)]` con presencia/sentinel y el renderer
  comparte ese contexto con Action2, CB13 y las animaciones.
- La variante legacy `action2_eval_ctx_from_station_with_spec` y
  `apply_station_availability_callback_with_spec` conservan la misma respuesta
  cuando no hay tesela pero el caller todavía tiene la `StationSpec`.

La API histórica que sólo recibe `Station` no conoce la `StationSpec` ni la
tabla del GRF, por lo que no inventa una respuesta: conserva el contexto sin
badge. Los callers que sí conservan la spec deben usar la variante
catalog-aware/`with_spec` para consultar `0x7A`.

Regresiones:

- `parse_station_badge_list_prop_1f`
- `station_badge_var_uses_grf_local_translation_and_sentinel`

Referencia upstream: `newgrf/newgrf_act0_stations.cpp` (prop `0x1F`) y
`newgrf_station.cpp` (`GetBadgeVariableResult`).
