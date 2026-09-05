# #371 — renombrar tablas SAV sin perder columnas desconocidas

Actualizado: **2026-09-05**. Sub-issue de
[#328](https://github.com/cavazquez/openttdrs/issues/328); no declara
interoperabilidad SAV global.

## Divergencia y corrección

Al cambiar `PLYR.name` por un nombre de otra longitud, el merge rechazaba la
fila y emitía el schema reducido del writer, descartando columnas importadas
no modeladas. La regresión con `train_pbs_15_3.sav` fallaba al comparar las
cabeceras antes del arreglo.

El merge común permite ahora cambiar la longitud de **strings raíz** cuyo
descriptor siga siendo compatible. Reconstruye sólo las filas modificadas,
recalcula su longitud gamma y conserva cabecera, índices sparse, huecos densos,
columnas ajenas y sufijos opacos. El snapshot semántico sigue evitando que la
normalización de importación sobrescriba campos que el usuario no cambió.

Las filas importadas pueden superar 16 KiB por sus columnas opacas: su nueva
longitud usa la codificación gamma nativa completa. Las pruebas de frontera
detectaron además que `read_sl_gamma` consumía sólo tres bytes tras el prefijo
`11110---`; ahora consume los cuatro bytes del valor, como OpenTTD.

Oracle leído: OpenTTD 15.3,
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`,
`src/saveload/saveload.cpp` (`SlReadSimpleGamma`/`SlWriteSimpleGamma`) y
`src/saveload/company_sl.cpp` (`CompanyProperties.name`). No se modificó el
checkout de referencia.

## Evidencia

- `rename_preserves_unknown_columns_dense_holes_and_sparse_indices`: nombres
  más largos, más cortos, vacíos y UTF-8; gamma 127/128; campos canónicos en
  otro orden; huecos densos e índices sparse 3/130; otra fila normalizada pero
  intacta. Compara el chunk completo contra los bytes esperados.
- `rename_preserves_large_unknown_column_across_three_byte_row_length`: una
  columna opaca de 16.370 bytes se conserva al cruzar la longitud de fila
  16.383/16.384.
- `rename_falls_back_when_row_identity_or_list_size_changes`: conserva el
  fallback canónico ante altas de filas, cambio de identidad sparse o cambio
  de longitud de listas. `rename_falls_back_when_name_descriptor_changes`
  comprueba descriptores incompatibles, con y sin snapshot semántico.
- `native_gamma_boundaries_match_openttd_encoding` y
  `full_gamma_rejects_truncation_and_unsupported_prefix`: vectores nativos en
  todas las fronteras de 1–5 bytes, truncamientos y prefijos no soportados.
- `native_company_rename_preserves_other_plyr_fields`: fixture nativo, nombres
  largo/corto/vacío y comparación byte a byte de **cada campo de `PLYR` salvo
  `name`**, incluida la cabecera y los structs normalizados por el importador.
- OpenTTD dedicado carga y re-guarda el candidato; después
  `openttd_resaved_preserves_renamed_company` comprueba explícitamente que sigue
  existiendo una compañía llamada `Transportes del Sur y del Litoral`.

Fixture versionado:
`crates/openttdrs-core/tests/fixtures/train_pbs_15_3.sav`, SHA-256
`32afe76f37fe9f2c30721838cb47f6400b65d8aea1068aa86743901a999231a4`.
Candidato renombrado OTTN: SHA-256
`2b4430fb24050c65bbb87d813b7f7f7bd3b08c5e0c3c36efb4049ebfa759f352`.
El re-guardado no se usa como golden binario: OpenTTD puede avanzar el estado
durante la prueba; se comprueba su contenido semántico.

## Reproducción

Desde la raíz del repositorio, con OpenTTD disponible en `reference/` o mediante
`OPENTTD=/ruta/al/binario`:

```bash
rename_artifacts=$(mktemp -d /tmp/openttdrs-sav-rename.XXXXXX)
OPENTTDRS_DUMP_RENAMED_NATIVE_SAV="$rename_artifacts/renamed.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::rename_tests::native_company_rename_preserves_other_plyr_fields \
  -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 \
OPENTTDRS_OTTD_ARTIFACT_DIR="$rename_artifacts/artifacts" \
OPENTTDRS_OTTD_LOG_DIR="$rename_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$rename_artifacts/renamed.sav"
OPENTTDRS_ROUNDTRIP_SAV="$rename_artifacts/artifacts/renamed.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_renamed_company -- --exact --ignored
```

El último test es explícitamente `ignored` sin el oracle externo; no se cuenta
como validado por una ejecución ordinaria de `cargo test`.

## Validaciones de la etapa

- `cargo fmt --all -- --check`: OK.
- Clippy core y cliente, `--all-targets -- -D warnings`: OK.
- `cargo test -p openttdrs-core --quiet`: 2.062 unitarios OK, suites de
  integración y doctests OK; dos tests externos ignorados en la suite normal.
- `env RUSTC_WRAPPER= cargo test -p openttdrs-client --bin openttdrs-client
  --quiet`: 1.105 OK, dos ignorados, cero fallos.
- Carga/re-guardado dedicado y test explícito del nombre resultante: OK. El
  candidato regenerado con el código final mantiene el SHA-256 citado arriba.
- `./scripts/check_parity_docs_fresh.sh` y `git diff --check`: OK.

## Alcance pendiente

Cambios de longitud de listas/structs anidados, strings dentro de esos structs,
campos ausentes/incompatibles, altas/bajas/reordenación de filas y pools nativos
no modelados quedan fuera de este corte. Esas mutaciones siguen usando el
writer canónico cuando no se puede conservar el schema original. #328 queda
abierto. No hay cambio visual: la evidencia de este corte es SAV/bytes y
carga/re-guardado, no capturas ni una afirmación de paridad de mapa.
