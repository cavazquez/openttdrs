# #374 — normalizar historiales `INDY` al array nativo de 61 registros

Actualizado: **2026-09-05**. Sub-issue de [#328][parent]. Este corte corrige
el wire format de industrias; no declara completa su economía ni el runtime
de agregación histórica.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El runtime reducido crea inicialmente uno o dos samples para
`Industry::accepted_history` y `produced_history`. El writer reproducía esa
longitud variable. En el oracle, `HistoryData` tiene `HISTORY_RECORDS = 61`:

- `SlIndustryProducedHistory::Save` siempre guarda las 61 posiciones de una
  salida válida;
- `SlIndustryAcceptedHistory::Save` guarda longitud cero si el puntero todavía
  no existe, pero guarda las 61 posiciones una vez que fue creado.

Por tanto, el primer re-guardado nativo cambiaba la forma de los historiales
generados por `openttdrs`, aunque no perdiera sus primeros valores.

## Corrección acotada

El writer ahora conserva los samples presentes en orden y completa/trunca los
historiales representables a 61 registros:

- `INDY.produced[].history` siempre se emite como el array fijo nativo;
- `INDY.accepted[].history` conserva longitud cero cuando la entrada nunca
  tuvo historial y usa 61 cuando sí existe;
- filas de cargos no resolubles se dejan como passthrough opaco, sin inventar
  una semántica que el runtime no puede ejecutar.

La regresión nativa muta ambas clases de historial en `train_pbs_15_3.sav`.
El merge compatible de #373 conserva la cabecera y todos los bytes `INDY`
ajenos a `accepted` y `produced`; al reimportar, ambos samples nuevos tienen
61 registros.

## Oracle y reproducción

El OpenTTD dedicado de `reference/openttd-upstream`, commit
`c2661164bcb6cbf5ab97b56ccbee7506a3b26833`, cargó y re-guardó el candidato.
La regresión externa
`openttd_resaved_preserves_normalized_indy_histories` comprueba las dos
entradas con sus 61 registros y los samples iniciales `(accepted=123,
waiting=0)` y `(production=456, transported=78)`.

Artefactos de la corrida final:

- fixture: `32afe76f37fe9f2c30721838cb47f6400b65d8aea1068aa86743901a999231a4`;
- candidato Rust: `59c74eee41f044eb5647824f7466848470628ce83ea2664f05ee22c46a2649aa`;
- re-guardado por OpenTTD: `819fb3f21346980ed9e03a8dda6f282f82ef68374432e0dbaa2b2495e33c0e03`.

```bash
indy_artifacts=$(mktemp -d /tmp/openttdrs-sav-indy.XXXXXX)
OPENTTDRS_DUMP_INDY_HISTORY_NATIVE_SAV="$indy_artifacts/indy-history.sav" \
  cargo test -p openttdrs-core --lib \
  sav::write::rename_tests::native_indy_histories_preserve_other_indy_fields \
  -- --exact
OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
  OPENTTDRS_OTTD_ARTIFACT_DIR="$indy_artifacts/artifacts" \
  OPENTTDRS_OTTD_LOG_DIR="$indy_artifacts/logs" \
  bash scripts/roundtrip_sav_openttd.sh "$indy_artifacts/indy-history.sav"
OPENTTDRS_ROUNDTRIP_SAV="$indy_artifacts/artifacts/indy-history.resaved.sav" \
  cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_normalized_indy_histories -- --exact --ignored
```

## Pendiente real

La rotación/agregación mensual, trimestral y anual de los 61 registros todavía
no es equivalente al runtime de OpenTTD. Tampoco cubre cargos NewGRF sin
catálogo ejecutable, cambios de schema/topología, filas/índices o los demás
pools de #328. Esas tareas siguen en #328/#329.
