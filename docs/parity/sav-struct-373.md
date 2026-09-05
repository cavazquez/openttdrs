# #373 — preservar `CITY.supplied` como struct-list raíz compatible

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]; no declara
paridad general de `.sav` ni del runtime de crecimiento urbano.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El corte #372 podía reencuadrar strings y listas escalares de raíz sobre una
tabla importada sin tocar sus columnas hermanas desconocidas. `CITY.supplied`
es distinto: es un campo raíz `SLE_FILE_STRUCT | HAS_LENGTH`, y cada entrada
contiene `cargo` y otro struct-list, `history`. Al añadir una entrada, el
writer caía al header canónico de `CITY` y perdía la conservación local que
corresponde al cuerpo importado.

La primera prueba contra el oracle también descubrió una incompatibilidad de
versión: el writer anunciaba SLV 355 aun cuando ya emitía el schema moderno de
`CITY.valid_history` y `SlTownSupplied`. OpenTTD interpreta esos bytes con el
schema anterior hasta `SLV_TOWN_SUPPLY_HISTORY` (358), por lo que al cargar y
re-guardar descartaba la entrada nueva.

## Corrección acotada

- El merge de tablas permite que un campo raíz con `HAS_LENGTH` cambie de
  tamaño sólo después de verificar recursivamente que el descriptor completo
  coincide. Así puede sustituir únicamente los bytes de `CITY.supplied`; el
  header, las filas y todos los demás campos de `CITY` permanecen importados.
- El contenedor exportado declara `EXPORT_SAVE_VERSION = 358`, el primer SLV
  que selecciona `SlTownSupplied` y `valid_history` en OpenTTD.
- Cada entrada emitida de `CITY.supplied.history` usa los 61 registros fijos de
  `HistoryData`; el runtime compacto puede actualizar su ventana activa, pero
  el wire format se rellena con registros por defecto hasta la longitud nativa.

El cambio no convierte arrays fijos en vectores: el writer de cada entidad
sigue siendo responsable de su tamaño nativo. Tampoco habilita un merge cuando
cambia una identidad de fila, un índice sparse, la topología o algún subcampo
anidado desconocido/incompatible; en esos casos se conserva el fallback al
writer canónico.

## Regresiones y oracle

La regresión sintética `struct_list_growth_preserves_root_unknown_column`
demuestra que una lista de structs raíz que crece conserva una columna futura
hermana. `rename_falls_back_when_row_identity_or_nested_struct_schema_changes`
protege la frontera conservadora. La regresión nativa
`native_town_supplied_growth_preserves_other_city_fields` añade un cargo al
primer `CITY.supplied` de `train_pbs_15_3.sav`, compara el header y todos los
bytes de `CITY` ajenos a `supplied`, y verifica al reimportar los 61 registros.

El OpenTTD dedicado construido desde
`reference/openttd-upstream` en `c2661164bcb6cbf5ab97b56ccbee7506a3b26833`
cargó y re-guardó el candidato con su puerto aislado. La comprobación ignorada
`openttd_resaved_preserves_added_town_supplied_entry` confirma en el artefacto
re-guardado el prefijo `(123, 45), (678, 90)` y los 61 registros. Artefactos de
la corrida final:

- fixture de entrada: `32afe76f37fe9f2c30721838cb47f6400b65d8aea1068aa86743901a999231a4`;
- candidato Rust: `ca6066cd6e86bc3c8e4b90f4098160f26582a34ed896bdb624e259eb3ded0035`;
- resultado re-guardado por OpenTTD: `2be9ca58ff34de49e276760cc79dfa96a55d8402d48cc7cafb5e299e6a6efb18`.

Reproducción del contrato externo:

```bash
struct_artifacts=$(mktemp -d /tmp/openttdrs-sav-struct.XXXXXX)
OPENTTDRS_DUMP_TOWN_SUPPLIED_NATIVE_SAV="$struct_artifacts/town-supplied.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::rename_tests::native_town_supplied_growth_preserves_other_city_fields \
  -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$struct_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$struct_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$struct_artifacts/town-supplied.sav"
OPENTTDRS_ROUNDTRIP_SAV="$struct_artifacts/artifacts/town-supplied.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_added_town_supplied_entry -- --exact --ignored
```

## Pendiente real

Este corte no implementa la agregación/rotación runtime completa de los 61
registros de pueblo; esa semántica pertenece a #329/#330. Otros pools, campos
anidados desconocidos, cambios de forma y de identidades siguen siendo trabajo
de #328.
