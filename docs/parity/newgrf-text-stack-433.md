# NewGRF text stack: referencias inline (#433)

Actualizado: 2026-09-06.

## Qué se cerró

El decoder de Action4/Action13 conserva el control NFO `0x81` como un marcador
visible (`⟦grf-string:0xNNNN⟧`). `NewGrfStringCatalog::lookup_expanded` ahora
resuelve ese marcador con el mismo fallback de idioma que el lookup normal:

- IDs locales se convierten a `GRF_STRING_GENERIC_BASE` y los IDs genéricos se
  conservan.
- Las referencias pueden encadenarse hasta ocho niveles.
- Una cadena ausente conserva el marcador para que la UI siga mostrando un
  diagnóstico útil.
- Un ciclo se corta cuando el ID ya está en la pila de expansión; no hay
  recursión infinita ni panic.

El texto CB15C del selector de objetos usa esta resolución, por lo que una
cadena Action4 que referencia otra cadena ya no queda limitada al primer
nivel.

## Alcance que sigue abierto

Los marcadores de parámetros dinámicos, fechas, gender/case, pluralización y
choice-lists todavía requieren un `TextRefStack` con scopes y registros del
juego. También falta expandir referencias provenientes de otros consumidores
de callbacks (vehículos, estaciones y casas); este issue sólo conecta el
selector de objetos, donde ya existía el lookup catalogado.

## Regresiones

La cobertura incluye expansión anidada local/genérica, fallback de idioma,
referencias faltantes y ciclos. Se conservan `lookup` y los textos crudos para
los callers que aún necesitan inspeccionar el catálogo sin expansión.
