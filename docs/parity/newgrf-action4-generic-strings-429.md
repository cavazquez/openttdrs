# Action4 genérico y texto de CB15C

## Estado

Implementado el 2026-09-06 en el issue [#429](https://github.com/cavazquez/openttdrs/issues/429).
El catálogo se reconstruye junto con los catálogos Action0 del stack activo y
queda en `SimulationRuntime`, por lo que no contamina el JSON propio ni un
SAV que todavía no tenga el text stack completo.

## Contrato upstream

OpenTTD procesa Action4 con la forma `04 feature language count offset data`.
Cuando el bit 7 de `language` está activo, `offset` es un `WORD` y cada cadena
ocupa un registro consecutivo. El rango genérico que utiliza CB15C comienza en
`GRFSTR_MISC_GRF_TEXT = 0xD000`. El esquema de idioma extendido usa los seis
bits inferiores cuando el bit 6 está activo; el esquema anterior es una
máscara de idiomas y puede producir más de una variante.

## Implementación

- `collect_action4_generic_strings_from_grf` recorre sólo pseudo-sprites y
  acepta Action4 genéricos con terminadores NUL completos; un payload truncado
  no genera una entrada parcial.
- `NewGrfStringCatalog` indexa `(GRFID, StringID, idioma)` y resuelve primero
  el idioma solicitado, luego inglés y finalmente la variante más reciente.
- El refresco del stack carga el catálogo antes de los Objects. CB15C local
  suma `0xD000 + offset`; CB15C `0x40F` consulta directamente el `StringID`
  del registro `0x100`.
- El ObjectPicker muestra el texto resuelto para español/inglés y conserva un
  diagnóstico `Action4 ausente` cuando el GRF no declara esa cadena.

Los códigos de control de strings se conservan sin expandir; la expansión del
text stack y los Action4 específicos de cada feature siguen siendo sucesores
separados.
