# AirportTile CB150: fundación dinámica (#447)

Actualizado el 2026-09-07.

## Divergencia auditada

El renderer ya tenía el call site de `CBID_AIRPTILE_DRAW_FOUNDATIONS`, pero un
`AirportTile` con `TileLayoutSpriteGroup` estático se reducía a una vista plana
y perdía el suelo y las entradas BUILD que OpenTTD pasa a
`AirportDrawTileLayout`/`DrawNewGRFTileSeq`. Sin una regresión conjunta, una
modificación podía volver a dibujar siempre la fundación vanilla, reponer
césped detrás de un ground custom o colgar el BUILD de la fundación.

## Contrato cubierto

- La tesela usa el contexto Action2 de `AirportTile` y la máscara de callback
  del catálogo.
- Resultado `0`: no se crea `FOUNDATION_LEVELED` y el sprite custom se dibuja
  como ground independiente; sus entradas BUILD se emiten como parents
  `TILE_SEQ_LINE` y children de pantalla.
- Resultado `1`: se conserva la fundación vanilla y **sólo el ground** se
  emite como `ViewportSortableChild` del parent de fundación; el BUILD sigue
  siendo parent independiente, igual que el orden nativo.
- Action0 conserva incluso el layout estático sin ramas variacionales, y en
  plano el ground completo sustituye el fallback de césped/`subst_id`.
- La ausencia de callback/runtime conserva el fallback de OpenTTD; ese camino
  sigue cubierto por la implementación productiva y no se declara resuelto el
  compositor de Action5/rotaciones.

Las regresiones `airport_tiles_keep_static_tile_layout_graph_after_apply`,
`newgrf_airport_tile_layout_emits_ground_sortable_parent_and_child` y
`newgrf_airport_draw_foundations_callback_controls_slope_foundation` cubren,
respectivamente, retención tras Action0, geometría/child en plano y los dos
resultados de CB150 sobre la misma pendiente.

Como *smoke* visual adicional, el cliente cargó `Kale_TitleGame.sav` centrado
en `(189,126)` a 1280×720 en `1.00×`, `0.50×`, `0.25×` y `0.12×` el
2026-09-07; las cuatro capturas conservaron el mundo compuesto. Kale no lleva
esta fixture sintética de `AirportTile` con `TileLayout`, por lo que esa
observación descarta una regresión global de zoom, pero no sustituye las tres
regresiones ECS ni afirma paridad raster del aeropuerto.

## Alcance que permanece abierto

CB150 ya tiene el contrato booleano probado en el renderer, pero siguen fuera
de este corte las foundations de compositor (`Action5` y rotaciones), las
rotaciones runtime de AirportTile, los sonidos y los demás scopes avanzados.
Esos puntos continúan en #329/#326.
