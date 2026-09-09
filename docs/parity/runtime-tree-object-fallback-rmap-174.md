# Clase cruda de tesela al propagar árboles (RMAP-174)

Última actualización: 2026-09-09.

Subcorte de runtime de #512/#527. No declara paridad general de árboles,
objetos ni de los fallbacks semánticos del mapa.

## Frontera reproducida

Después de RMAP-173, la ampliación de `autosave0.sav` encontró la primera
diferencia en `day[332].random_state.state_0`, tick `1497559`: OpenTTD tenía
`2301857926` y Rust `2176228557`. La investigación de la visita LFSR aisló la
tesela `(121,171)` (índice `43897`). Su carga cruda coincidía en ambos lados:
`MAPT = 0xa0` (`MP_OBJECT`), `m1 = 112`, `m2 = 9`, `m3 = 63`, `m5 = 0`.

El modelo semántico usa `TileKind::Grass` como fallback visual para tipos aún
no materializados. La propagación de árboles consultaba sólo ese `TileKind`,
por lo que trataba el objeto crudo como `MP_CLEAR`, permitía sembrar un bosque
y alcanzaba un consumidor RNG que OpenTTD descarta por el tipo de tesela.

## Regla portada

`generation_tree_plantable` conserva el fallback `Grass`, pero exige además
que su nibble crudo sea `MP_CLEAR`; la rama de agua exige análogamente
`MP_WATER`. Así, un objeto que se represente provisionalmente como pasto no
se vuelve plantable ni pierde sus bytes crudos.

La regresión
`generation_tree_spread_does_not_overwrite_raw_object_fallback` instala una
tesela `MAPT=0xa0` vecina a un árbol y exige que la expansión la deje intacta.

## Validación diferencial

Con OpenTTD 15.3 `14ec60f248547d4d062a1160f0fc26d742319888`, el mismo
`autosave0.sav` y socket dedicated válido, la corrección elimina la frontera
de `day[332]`. La siguiente frontera, de subsidios mensuales, queda resuelta
por RMAP-175; ambas correcciones dejan exacta la ventana de 360 jornadas:

```bash
OPENTTD_BIN=reference/openttd-15.3-oracle/build/openttd \
OPENTTDRS_INDUSTRY_TRACE_TIMEOUT=1100 \
  ./scripts/export_openttd_industry_trace.sh autosave0.sav /tmp/openttd-rmap-175.jsonl 360
./scripts/export_openttdrs_industry_trace.sh autosave0.sav /tmp/openttdrs-rmap-175.jsonl 360
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/openttd-rmap-175.jsonl /tmp/openttdrs-rmap-175.jsonl 360
```

El comparador valida reloj, ambas palabras RNG, `ECMY`, `ITBL`, pool,
industrias y acciones del scheduler en `initial` y `day[0]`…`day[359]`.

## Límites

El corte cubre la guardia de tipo crudo de la propagación runtime de árboles
en esta fixture. No materializa por sí mismo todos los tipos `MAPT`, objetos
NewGRF, otras reglas de plantación ni una ventana temporal posterior.
