# Chrome de compra de vehículos localizado (#467)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

La ventana `BuyVehicle` tenía el título base y sus títulos dinámicos de
depósito sólo en español fuera del catálogo. Los controles de ordenar y filtrar
(`Nombre`, `Precio`, `Vel.`, `Año`, tipos de carretera y tren) tampoco tenían
entradas inglesas. Al cambiar el locale, el título se volvía a escribir en
español en cada sincronización.

## Implementación

Se catalogó el chrome de la ventana, incluidos los seis títulos de depósito,
los controles de ordenar/filtrar y el botón de compra ya existente. El sistema
de sincronización recibe `ClientPreferences` y localiza el título según el
locale activo.

Los nombres de motor, cargos, precios, capacidades, fiabilidad y demás
estadísticas siguen siendo datos de la partida y no se interpretan como
etiquetas de UI. Los enums de filtro/orden y los comandos de compra no cambian.

La regresión `buy_window_chrome_localizes_titles_without_changing_depot_kind`
comprueba títulos de carretera y vía en español/inglés y confirma que la clase
de depósito permanece intacta. El catálogo cubre también barco, avión y
helicóptero, además de todos los botones.

## Alcance pendiente

La localización de nombres y estadísticas provenientes de catálogos de motores
queda fuera de alcance; #331 continúa abierto por catálogos upstream y otras
superficies UI.
