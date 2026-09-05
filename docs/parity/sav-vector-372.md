# #372 — conservar listas raíz compatibles de tablas SAV importadas

Actualizado: **2026-09-05**. Sub-issue de
[#328](https://github.com/cavazquez/openttdrs/issues/328); no declara
interoperabilidad SAV global ni paridad de contenedores anidados.

## Divergencia y corrección

El merge conservador de #371 podía reencuadrar una string raíz, pero rechazaba
cualquier lista escalar que cambiara de longitud. Al agregar un storage NewGRF
a un pueblo importado, `CITY.psa_list` crecía y el exportador volvía al writer
canónico; en el fixture nativo eso cambiaba incluso bytes ajenos como `name`.

`CITY.psa_list` es `SLE_CONDREFVECTOR(Town, psa_list, REF_STORAGE, ...)` en
`town_sl.cpp`. El merge común puede ahora reemplazar una lista raíz de
escalares con descriptor compatible y volver a codificar sólo la longitud de
la fila. Conserva cabecera original, huecos densos, índices sparse, columnas
desconocidas y sufijos opacos; la lista y su gamma pueden crecer o reducirse.

El header SAV no diferencia `SL_ARR`, `SL_VECTOR`, `SL_DEQUE`, `SL_REFLIST` y
`SL_REFVECTOR`: todos se serializan como tipo escalar más `HAS_LENGTH`.
`SlArray` sí rechaza una longitud distinta para un array fijo. Por eso esta
regla sólo abre la preservación wire-format de listas raíz escalares que el
writer ya emitió; no convierte arrays fijos en vectores. Los writers propios
siguen emitiendo sus tamaños nativos fijos (`ratings`, `unwanted`, `goal`,
`PSAC.storage`, `cargo.action_counts`) y cualquier validez semántica de esos
arrays sigue siendo responsabilidad de su writer.

Los structs usan el tipo de archivo distinto `SLE_FILE_STRUCT`; una variación
de cantidad en un struct/lista anidada, una alta/baja/reordenación de filas,
un descriptor incompatible o un campo omitido y modificado continúan usando el
writer canónico. El snapshot semántico de importación sigue evitando que una
normalización que el usuario no tocó reemplace bytes importados.

Oracle de implementación consultado: OpenTTD 15.3,
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`,
`src/saveload/saveload.cpp` (`GetSavegameFileType`, `SlArray`) y
`src/saveload/town_sl.cpp` (`psa_list`). No se modificó el checkout de
referencia.

## Evidencia

- `passthrough_preserves_future_column_when_scalar_list_changes_length` cubre
  el merge estricto con una lista `u8` de 127 a 128 elementos y una columna
  futura que debe permanecer intacta.
- `list_growth_preserves_unknown_columns_dense_holes_and_sparse_indices`
  compara bytes completos para crecimiento 127→128, reducción 128→2 y
  vaciado 1→0; cubre campos físicos antes/después, otra fila normalizada,
  hueco denso e índices sparse 3/130.
- `rename_falls_back_when_row_identity_or_nested_struct_size_changes` conserva
  el fallback para una lista de structs de otra cantidad, alta de fila e
  identidad sparse distinta. `rename_falls_back_when_name_descriptor_changes`
  cubre el descriptor incompatible.
- `native_town_psa_list_growth_preserves_other_city_fields` agrega un PSA al
  primer pueblo del SAV nativo y compara la cabecera y todos los campos de
  todas las filas `CITY` salvo `psa_list`, además de reimportar el `PSAC` y la
  referencia resultante.
- OpenTTD dedicado carga y re-guarda el candidato; el test externo ignorado
  `openttd_resaved_preserves_added_town_psa_list` comprueba el GRFID
  `D1CEBA5E`, el valor `storage[7] = CAFEBABE` y una referencia `CITY.psa_list`
  después del re-guardado.

Fixture versionado: `crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav`,
SHA-256
`32afe76f37fe9f2c30721838cb47f6400b65d8aea1068aa86743901a999231a4`.
Candidato OTTN: SHA-256
`945f63f727b5ffe269d9dc3e3acf8cc2f4796ecb9fdb74e56f4b7376d67d76ab`.
Re-guardado por OpenTTD: SHA-256
`874f9359260dfaea4830f996972c72e08774857afe7cd8d03a904a08af77720e`.
El re-guardado no se trata como golden binario: OpenTTD puede modificar estado
derivado durante la sesión; el contrato verifica su contenido semántico.

## Reproducción

Desde la raíz del repositorio, con OpenTTD disponible en `reference/` o mediante
`OPENTTD=/ruta/al/binario`:

```bash
vector_artifacts=$(mktemp -d /tmp/openttdrs-sav-vector.XXXXXX)
OPENTTDRS_DUMP_TOWN_PSA_NATIVE_SAV="$vector_artifacts/town-psa.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::rename_tests::native_town_psa_list_growth_preserves_other_city_fields \
  -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 \
OPENTTDRS_OTTD_ARTIFACT_DIR="$vector_artifacts/artifacts" \
OPENTTDRS_OTTD_LOG_DIR="$vector_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$vector_artifacts/town-psa.sav"
OPENTTDRS_ROUNDTRIP_SAV="$vector_artifacts/artifacts/town-psa.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_added_town_psa_list -- --exact --ignored
```

El último test es `ignored` sin oracle externo y no cuenta como evidencia de
una ejecución ordinaria de `cargo test`.

## Validaciones de la etapa

- `cargo fmt --all -- --check`: OK.
- Clippy estricto de core y cliente, `--all-targets -- -D warnings`: OK.
- `cargo test -p openttdrs-core --quiet`: 2.065 unitarios, integración y
  doctests OK; los contratos externos permanecen ignorados sin su oracle.
- `env RUSTC_WRAPPER= cargo test -p openttdrs-client --bin openttdrs-client
  --quiet`: 1.105 OK, dos ignorados, cero fallos.
- Carga, re-guardado dedicado y contrato explícito de `CITY.psa_list`/`PSAC`:
  OK.
- `./scripts/check_parity_docs_fresh.sh` y `git diff --check`: OK.

## Alcance pendiente

Structs y listas anidadas de longitud variable, strings dentro de structs,
campos ausentes/incompatibles, cambios de forma o identidad de filas y pools
que aún no tienen modelo siguen fuera de este corte. #328 permanece abierto;
la siguiente investigación debe tomar una estructura anidada reproducible, no
ampliar esta regla a ciegas. No hay cambio visual: la evidencia es de bytes,
entidades y carga/re-guardado SAV.
