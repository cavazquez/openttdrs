# Materialización de estación Oil Rig al completar la pieza norte (RMAP-172)

Última actualización: 2026-09-08.

Subcorte de runtime de RMAP-158/#499 y #512/#527. No declara paridad general
de industrias, estaciones ni callbacks NewGRF.

## Frontera reproducida

Después de RMAP-171, la primera muestra distinta estaba en `day[210]`. En el
tick `1488497`, la Oil Rig 25 de `(239,71)` completaba la pieza norte
`GFX_OILRIG_1`; la pieza sur equivalente aún tenía `m1 = 0x0e`. En OpenTTD,
`MakeIndustryTileBigger` llama de inmediato a `BuildOilRig`: comprueba la
pieza norte terminada y que debajo exista la pieza sur del mismo `IndustryID`,
pero no espera a que esta última termine.

La conversión crea la estación neutral Oilrig en esa misma visita LFSR. Por
eso su `AcceptanceTick` actual llega a `TriggerAirportAnimation` y consume la
palabra global que faltaba en Rust. El port exigía erróneamente que ambas
piezas estuvieran terminadas y difería la estación; el estado se separaba en
la muestra diaria posterior.

## Regla portada

`oil_rig_station_tile` conserva las guardas de tipo de tesela, GFX, pareja sur
e `IndustryID`, pero exige terminación sólo para la pieza norte. Al rollover
que la completa, `phase_tile_loop` ejecuta `materialize_completed_oil_rigs`
después de su trigger de construcción, dentro de la misma visita. No se deja
la transición para `AnimateAnimatedTiles`: el nuevo `Station` debe existir
antes de los timers posteriores del tick.

Las regresiones directas son:

- `completed_oil_rig_creates_one_neutral_airport_and_dock`, que deja la pieza
  sur en obra;
- `oil_rig_station_materializes_in_the_north_tile_completion_visit`, que fija
  el orden de la visita LFSR y la creación inmediata.

## Validación diferencial

Se reconstruyó el oracle local OpenTTD 15.3
`14ec60f248547d4d062a1160f0fc26d742319888` sin instrumentación temporal y se
usó el mismo `autosave0.sav`:

```bash
OPENTTD_BIN=reference/openttd-15.3-oracle/build/openttd \
OPENTTDRS_INDUSTRY_TRACE_TIMEOUT=800 \
  ./scripts/export_openttd_industry_trace.sh autosave0.sav /tmp/openttd-rmap-172.jsonl 240
./scripts/export_openttdrs_industry_trace.sh autosave0.sav /tmp/openttdrs-rmap-172.jsonl 240
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/openttd-rmap-172.jsonl /tmp/openttdrs-rmap-172.jsonl 240
```

`initial` y `day[0]`…`day[232]` coincidían en reloj, ambas palabras RNG,
`ECMY`, las 240 filas `ITBL`, pool ordenado de industrias y acciones. La
frontera posterior de `day[233]` se atribuyó después al orden físico del pool
industrial durante el cierre mensual y queda resuelta por
[RMAP-173](runtime-monthly-industry-pool-order-rmap-173.md); no se atribuye a
esta transición de estación.

## Límites

La evidencia cubre la geometría Oil Rig vanilla observada y el orden de su
estación neutral en esta fixture. No certifica otros layouts, estaciones
compuestas, callbacks o animaciones NewGRF, ni la ventana posterior a
`day[232]`.
