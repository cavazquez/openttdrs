# Lista de carteles localizada (#476)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`SignList` tenía título catalogado, pero sus acciones y el texto vacío/ayuda
del cuerpo permanecían en español. El cuerpo se reconstruye con cada lista de
carteles y podía dejar el locale inglés mezclado con chrome español.

## Implementación

Se catalogaron las acciones (`Center`, `Rename`, `Delete sign`, `Apply`,
`Cancel`) y el sync recibe `ClientPreferences` para construir el estado vacío
y el encabezado en el locale activo. Las filas se formatean con una función
que conserva literalmente ID, coordenadas y nombre introducido por el usuario;
renombrar, borrar, centrar y el remapeo visual no cambian.

La regresión cubre ambos locales, el estado vacío y un nombre con caracteres
Unicode/coordenadas negativas.

## Alcance pendiente

Los nombres de carteles son datos de la partida y no se traducen; #331 sigue
abierto por catálogos upstream y otras superficies UI.
