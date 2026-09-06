# Paridad NewGRF: badges de RoadStop (`0x7A`) (issue #410)

Actualizado: **2026-09-06**.

## Referencia OpenTTD 15.3

`newgrf_act0_roadstops.cpp` lee la propiedad Action0 `0x16` como una lista
`WORD count + N×WORD local_id` mediante `ReadBadgeList(...,
GSF_ROADSTOPS)`. Cada índice se traduce con la Badge Translation Table del
GRF y se conserva como `BadgeID`; los duplicados se descartan. El
`RoadStopScopeResolver` devuelve para `0x7A(parameter)`:

| Caso | Resultado |
|---|---:|
| El índice local traduce a un badge asociado | `1` |
| El índice local traduce, pero el badge no está asociado | `0` |
| El índice no existe en la tabla local | `UINT_MAX` |

La misma variable se consulta tanto en la previsualización/compra como en una
parada ya colocada.

## Implementación

- `ParsedRoadStopMeta` conserva `badge_local_ids` y el parser Action0 entiende
  la propiedad nativa `0x16`.
- Las propiedades de puente `0x13`/`0x14` se saltan con su `ExtendedByte`
  antes de continuar; así una lista de badges posterior no queda invisible.
- `RoadStopSpecDef` conserva `associated_badges` y
  `newgrf_badge_translation`, usando `u16::MAX` para mantener la posición de
  un label local no resoluble.
- `apply_newgrf_roadstops` traduce la lista nativa por GlobalVar `0x18` y
  mantiene compatibilidad con las asociaciones auxiliares `0xFD` por etiqueta.
- El contexto map-aware y el scope de compra publican sólo los parámetros
  solicitados por Action2, además de las entradas presentes en la tabla.

## Regresiones

- `parse_roadstop_meta_and_apply_from_bytes` incluye `0x13` antes de `0x16`.
- `apply_roadstop_badges_uses_globalvar_translation_table` verifica parseo,
  catálogo global, asociación y tabla local tras aplicar un GRF.
- `road_stop_scope_exposes_badge_presence_and_unknown_sentinels` cubre `1` y
  `UINT_MAX` en una parada colocada.
- `road_stop_availability_purchase_scope_uses_native_sentinels_and_company_context`
  cubre los mismos resultados durante la compra sin entidad.

El padre #329 continúa abierto: este issue sólo cubre la fuente de badges y
la variable `0x7A`; strings, sonidos y el resto de scopes de `BaseStation`
siguen siendo trabajo separado.
