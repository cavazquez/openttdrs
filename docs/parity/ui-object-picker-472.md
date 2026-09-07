# Selector de objetos localizado (#472)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`ObjectPicker` ya resolvía el texto CB15C de objetos NewGRF con el idioma
seleccionado, pero su título dinámico `Objeto · ...` y el resumen
`Seleccionado: ...` se regeneraban en español. Los labels vanilla
`Transmisor`/`Faro` tampoco tenían entrada de catálogo.

## Implementación

Se localizan el título, el prefijo de selección y los nombres vanilla. Los
nombres, dimensiones y textos aportados por un objeto NewGRF permanecen
literales; también se mantiene la ID seleccionada, la miniatura, el callback
CB15C y los comandos de colocación. La regresión cubre ambos locales y un
label custom con dimensiones.

## Alcance pendiente

Los catálogos de nombres NewGRF y sus callbacks siguen dependiendo del idioma
que expone cada GRF; #331 continúa abierto por esas superficies y el resto del
catálogo upstream.
