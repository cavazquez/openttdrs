# Controles NFO básicos de Action4/Action13

## Estado

Implementado el 2026-09-06 en el issue
[#432](https://github.com/cavazquez/openttdrs/issues/432). Las cadenas
genéricas ya no llegan a la UI con bytes de control invisibles.

## Contrato upstream

`TranslateTTDPatchCodes` de OpenTTD convierte controles NFO al cargar cada
cadena. El recorte cubre los controles que no necesitan un `StringStack` ni un
scope de juego: espacios codificados (`0x01`), saltos (`0x0A`/`0x0D`), marcas
de fuente, referencias inline (`0x81`), fechas/volumen y glifos frecuentes.
Los controles de parámetros, colores y choice-lists no se pueden resolver sin
el text stack activo.

## Implementación y límites

- `decode_newgrf_text` es puro, no hace panic con payloads truncados y
  conserva texto UTF-8 válido.
- Action4 y Action13 pasan ahora por el decodificador antes de indexar
  `NewGrfStringCatalog`.
- Referencias, parámetros, gender/case, pluralización y choice-lists se
  representan como marcadores `⟦...⟧` visibles; colores y fuentes que sólo
  afectan el dibujo no contaminan el texto.
- Los marcadores permiten una expansión posterior con estado de juego sin
  volver a parsear el GRF.

La expansión localizada completa, el `StringStack` y los mappings Action0
siguen pendientes en el padre #329.
