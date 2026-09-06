# NewGRF text stack: metadata de género y caso (#446)

Implementado en `main` el 2026-09-06.

## Divergencia

El decoder ya conservaba los índices de `gender` y `case`, pero el renderer
los mostraba como texto y sólo podía escoger una rama mediante el índice común
de choice-list. Para `gender-list` tampoco se usaba el offset del parámetro.

## Implementación

- `gender:<index>` y `case:<index>` son metadata de `SCC_GENDER_INDEX` y
  `SCC_SET_CASE`; se consumen sin producir glyphs visibles.
- `NewGrfTextContext` acepta `with_gender_index` y `with_case_index` para que
  el caller lingüístico entregue la selección ya resuelta.
- `gender-list:<offset>` inspecciona un parámetro textual que comienza con
  `⟦gender:N⟧` cuando no se entregó un índice explícito. `case-list` usa el
  índice de caso y conserva el fallback de choice-list manual.
- Si falta el parámetro o el contexto de género, la lista completa permanece
  visible; no se inventa una rama. Las listas manuales y sus defaults no
  cambian.

La tabla automática de géneros/casos por locale, el formato lingüístico y los
scopes implícitos de OpenTTD siguen pendientes del parent #329/#331.

## Regresiones

`renders_gender_and_case_metadata_without_visible_markers` cubre metadata,
género transportado por texto, índices explícitos y caso; y
`preserves_gender_list_when_context_is_missing` verifica el fallback seguro.
