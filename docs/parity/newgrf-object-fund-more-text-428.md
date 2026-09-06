# Objects CB15C: FundMoreText

## Estado

Implementado el 2026-09-06 en el issue [#428](https://github.com/cavazquez/openttdrs/issues/428). La etapa cierra el contrato de evaluación y el puente visible del selector; no declara traducido el catálogo de strings Action4.

## Contrato upstream

`CBID_OBJECT_FUND_MORE_TEXT` (`0x15C`) se evalúa al abrir la ventana de construcción, antes de crear una instancia. El callback usa la vista seleccionada y devuelve:

- `CALLBACK_FAILED` o `0x400`: no agregar texto.
- `0x000..0x3FF`: texto local `GRFSTR_MISC_GRF_TEXT + result`.
- `0x40F`: `StringID` explícito en el registro `0x100` del text stack.
- otros resultados: callback inválido, que OpenTTD reporta como error.

## Implementación

- Se expone `CBID_OBJECT_FUND_MORE_TEXT` y `ObjectSpecDef::has_fund_more_text_callback()`.
- `resolve_object_fund_more_text_callback` construye el scope de preview sin instancia ni mutaciones, pasa la vista y clasifica el retorno como `None`, `Local`, `GrfString` o `Invalid`.
- El ObjectPicker muestra el resultado válido como diagnóstico explícito (`Action4 pendiente`) en vez de ocultarlo; los callbacks inválidos también quedan visibles.
- Las regresiones cubren el resultado local dependiente de vista, ausencia de texto, error y el registro `0x100`.

La traducción final de `StringID`/text stack requiere conservar Action4 y las cadenas por idioma; queda separada como trabajo sucesor y no se marca como paridad UI completa.
