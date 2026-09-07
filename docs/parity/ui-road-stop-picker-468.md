# Selector de paradas viales localizado (#468)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`RoadStopPicker` no tenía catálogo para el título ni las secciones `Clase`,
`Vista previa` y `Tipo`. Su título dinámico mezclaba `Bus`, `Camión` y `Parada`
sin consultar el locale y se reescribía en cada frame.

## Implementación

El título base y las secciones pasan por el catálogo. El prefijo dinámico usa
`Parada de autobús`, `Parada de camión` o `Parada`, con traducciones `Bus stop`,
`Truck stop` y `Stop`. El label de clase/especificación NewGRF se conserva
literal, porque es un nombre suministrado por el catálogo activo y no una
etiqueta vanilla.

El sistema recibe `ClientPreferences` para localizar el título en vivo; IDs,
visibilidad, selección y comandos `SetCurrentRoadStopClass/Spec` no cambian.

La regresión `road_stop_title_localizes_prefix_but_preserves_newgrf_label`
comprueba bus/camión en ambos locales y preserva `Terminal Custom` byte a byte.

## Alcance pendiente

Los nombres y layouts NewGRF siguen siendo datos literales; #331 permanece
abierto por catálogos upstream y otras superficies UI.
