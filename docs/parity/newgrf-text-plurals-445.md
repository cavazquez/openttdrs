# NewGRF text stack: pluralización (#445)

Implementado en `main` el 2026-09-06.

## Divergencia

El decoder convertía `0x9A15` en `plural-list`, pero descartaba el byte de
regla (`plural_form`) y el offset del parámetro. El renderer sólo podía elegir
una rama mediante `choice_index` manual; no podía reproducir
`DeterminePluralForm` de OpenTTD.

## Implementación

- El marcador conserva `plural-list:<plural_form>:<offset>` y los controles de
  género conservan su offset visible (`gender-list:<offset>`). Los payloads
  truncados siguen siendo explícitos.
- `NewGrfTextContext::with_plural_form` permite aportar o sobreescribir la
  regla lingüística. Si no se entrega, se usa la regla que ya venía en el
  marcador NewGRF.
- Se implementan las 15 ramas `DeterminePluralForm` de OpenTTD 15.3 para
  valores signed/unsigned. Un parámetro textual, offset inexistente o regla
  fuera de `0..=14` conserva la lista sin inventar una selección.
- Las listas manuales sin metadata mantienen la API previa de
  `with_choice_index`, por lo que no cambia el comportamiento de HUD ni de
  catálogos existentes.

El catálogo automático de reglas por locale, además de gender/case y scopes de
otros features, permanece pendiente del parent #329.

## Regresiones

`decodes_plural_metadata_without_losing_parameter_offset` cubre el wire format;
`renders_plural_list_from_count_and_rule_metadata` comprueba offset, signed y
override explícito; `preserves_plural_list_when_count_or_rule_is_invalid`
verifica el fallback seguro; y `determine_plural_form_matches_all_upstream_rules`
recorre las quince reglas y el sentinel inválido.
