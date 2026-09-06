# NewGRF text stack: parámetros y choice-lists (#442)

Actualizado: 2026-09-06 · referencia OpenTTD 15.3.

## Brecha observada

El decodificador de Action4/Action13 conserva los controles dinámicos como
marcadores `⟦...⟧`, pero los mensajes de CB31/CB149/CB157 y el texto CB15C del
selector de objetos no tenían una forma segura de materializar valores. Eso
dejaba visibles `param-*` y los delimitadores de choice-list aun cuando el
callback sólo necesitaba mostrar una rama default.

## Implementación

- `NewGrfTextValue` modela valores signed, unsigned y texto sin depender de un
  estado global.
- `NewGrfTextContext` entrega la pila de parámetros en orden explícito y un
  índice opcional de choice-list; no se modifica al renderizar.
- `render_newgrf_text` reemplaza controles numéricos/hex/string y selecciona
  `choice-next:N` o `choice-default`. Una lista truncada o un parámetro ausente
  conserva el marcador original.
- `NewGrfStringCatalog::lookup_rendered` encadena lookup, referencias inline y
  renderizado.
- HUD de feedback y ObjectPicker usan el contexto vacío: los textos sin
  parámetros conservan su contenido y las choice-lists muestran default.

## Límites explícitos

Todavía no se declara paridad para fechas, mapeos de género/case, plural forms
dependientes del idioma, parámetros de estación/vehículo ni el stack de texto
activo que requiere resolver scopes. Esas capacidades deben aportar un
`NewGrfTextContext` específico antes de consumirlas.

## Regresiones

Las pruebas de `newgrf_text` cubren parámetros signed/hex/string, default y
rama indexada, payload truncado, contexto inmutable y expansión inline previa
al renderizado.
