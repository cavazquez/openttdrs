# Construcción de industria durante `TileLoop` (RMAP-171)

Última actualización: 2026-09-08.

Subcorte de runtime de #512/#527 y de la cadena de scheduler industrial de
RMAP-162. No declara paridad general de industrias ni de callbacks NewGRF.

## Frontera reproducida

Después de RMAP-170, `autosave0.sav` coincidía hasta la primera muestra
distinta `day[181]`. El primer consumidor distinto estaba en el tick
`1486356`: la tesela de industria `(240,71)` tenía `m1 = 0x0c` (etapa 0,
contador de obra 3). OpenTTD avanzaba esa tesela a `m1 = 0x01`, mientras que
el port la había dejado para la fase de animación posterior.

En `src/industry_cmd.cpp`, `TileLoop_Industry` llama
`MakeIndustryTileBigger` durante la visita LFSR viva. Al rollover del contador
`3 → 0`, `TriggerIndustryTileAnimation_ConstructionStageChanged` toma una
palabra de `Random()` antes de consultar si hay una máscara/callback NewGRF.
La palabra faltante desplazaba los consumidores de árboles y pueblos que
seguían en ese mismo tick.

## Regla portada

`phase_tile_loop` ahora procesa una tesela `Industry` incompleta en su visita
actual, cambia sólo esa tesela con `MakeIndustryTileBigger` equivalente y, en
un rollover, consume la palabra global antes de disparar el trigger de etapa.
La fase `AnimateAnimatedTiles` ya no reconstruye la obra a partir de las
visitas del tick anterior; así no aplaza ni duplica el avance.

Las regresiones directas son:

- `incomplete_industry_rollover_uses_current_visit_and_global_rng`;
- `prior_animation_phase_does_not_repeat_incomplete_industry_construction`.

## Validación diferencial

La comprobación se hizo con el oracle local OpenTTD 15.3
`14ec60f248547d4d062a1160f0fc26d742319888`, socket dedicated válido y el
mismo `autosave0.sav`:

```bash
OPENTTD_BIN=reference/openttd-15.3-oracle/build/openttd \
OPENTTDRS_INDUSTRY_TRACE_TIMEOUT=600 \
  ./scripts/export_openttd_industry_trace.sh autosave0.sav /tmp/openttd-rmap-171.jsonl 181
./scripts/export_openttdrs_industry_trace.sh autosave0.sav /tmp/openttdrs-rmap-171.jsonl 181
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/openttd-rmap-171.jsonl /tmp/openttdrs-rmap-171.jsonl 181
```

El comparador dio `OK: scheduler industrial exacto · 181 días`: reloj, RNG,
`ECMY`, las 240 filas `ITBL`, pool ordenado de industrias y acciones. Al
ampliar entonces la corrida, la frontera posterior fue
`day[210].random_state.state_0`; RMAP-172 la atribuyó a la materialización
demorada de la estación neutral de una Oil Rig, no a esta regla de construcción
genérica. Su corrección y la ventana posterior están en
[runtime-oil-rig-station-timing-rmap-172.md](runtime-oil-rig-station-timing-rmap-172.md).

## Límites

El corte cubre la secuencia observada de construcción vanilla de la fixture y
la extracción global incondicional de su rollover. No certifica catálogos
NewGRF arbitrarios, callbacks que dependan de contextos aún no medidos,
otras geometrías de industria ni la ventana posterior a `day[210]`.
