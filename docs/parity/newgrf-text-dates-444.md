# NewGRF text stack: fechas (#444)

Implementado en `main` el 2026-09-06.

## Divergencia

El decoder reconocía los controles `0x82`/`0x83`/`0x84` y `0x9A 0x16`/
`0x9A 0x17`, pero los dejaba como marcadores aun cuando el consumidor había
entregado un parámetro. Además, los códigos extendidos `0x18..0x1E` se
etiquetaban incorrectamente como fechas DWORD.

## Implementación

- `NewGrfTextValue::Date { day_index }` y `NewGrfTextContext::with_date` hacen
  explícito que el valor está expresado como días desde el 1 de enero de 1950,
  el epoch del calendario del proyecto.
- `render_newgrf_text` consume fechas en el mismo orden de parámetros que el
  resto del text stack y produce formato largo (`1 ene 1950`), corto
  (`ene 1950`) o ISO (`1950-01-01`). Valores signed/unsigned también se
  aceptan como día de calendario para mantener la frontera interoperable.
- El decoder distingue fechas WORD/DWORD y deja `power`, `volume`, `weight` y
  cargos con marcadores específicos en vez de convertirlos en fechas.
- Un parámetro ausente, negativo o textual conserva el marcador original.

El formato es determinista y usa las abreviaturas de calendario actuales del
proyecto. La localización completa del formato, género/case/plural y la
conversión de fechas NewGRF con epoch 1920/0 deben resolverse en sucesores del
parent #329.

## Regresiones

`decodes_word_and_dword_date_controls_separately` cubre la frontera de bytes;
`renders_date_parameters_in_long_short_and_iso_forms` comprueba el consumo
ordenado y `preserves_missing_date_parameters` evita inventar información ante
payloads incompletos.
