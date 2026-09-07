# AirportTile CB150: fundación dinámica (#447)

Implementado en `main` el 2026-09-06.

## Divergencia auditada

El renderer ya tenía el call site de `CBID_AIRPTILE_DRAW_FOUNDATIONS`, pero no
había una regresión que demostrara el contrato completo para un `AirportTile`
NewGRF construido sobre pendiente. Sin esa prueba, una modificación del
compositor podía volver a dibujar siempre la fundación vanilla o dejar el
sprite custom fuera de su parent sortable sin que los tests de Action1/3 lo
detectaran.

## Contrato cubierto

- La tesela usa el contexto Action2 de `AirportTile` y la máscara de callback
  del catálogo.
- Resultado `0`: no se crea `FOUNDATION_LEVELED` y el sprite custom se dibuja
  como capa independiente.
- Resultado `1`: se conserva la fundación vanilla y el sprite custom se emite
  como `ViewportSortableChild` del parent de la fundación.
- La ausencia de callback/runtime conserva el fallback de OpenTTD; ese camino
  sigue cubierto por la implementación productiva y no se declara resuelto el
  compositor de Action5/rotaciones.

La regresión `newgrf_airport_draw_foundations_callback_controls_slope_foundation`
ejecuta ambos valores (`0` y `1`) sobre la misma pendiente y comprueba tanto la
presencia del parent de fundación como la relación parent/child.

## Alcance que permanece abierto

CB150 ya tiene el contrato booleano probado en el renderer, pero siguen fuera
de este corte las foundations de compositor (`Action5` y rotaciones), las
rotaciones runtime de AirportTile, los sonidos y los demás scopes avanzados.
Esos puntos continúan en #329/#326.
