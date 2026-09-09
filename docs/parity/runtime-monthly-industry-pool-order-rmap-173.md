# Orden del pool industrial en el cierre mensual (RMAP-173)

Última actualización: 2026-09-09.

Subcorte de runtime de #512/#527. No declara paridad general de economía,
industrias ni callbacks NewGRF.

## Frontera reproducida

Tras RMAP-172, `autosave0.sav` coincidía hasta `day[232]`; la primera
diferencia era `day[233].random_state.state_0` en el tick `1490233`
(`570692795` en OpenTTD frente a `1684328366` en Rust). La muestra diaria del
borde mensual anterior, tick `1490159`, aún era exacta:
`1177403742,4256402394`.

La traza de fases aisló el desvío después de ese scheduler diario, dentro del
cierre mensual. OpenTTD consumía 68 palabras globales entre aquella muestra y
el final de `TimerGameEconomy`; Rust consumía 69. La diferencia no provenía
del `tile loop`: sus doce visitas que consumen RNG en el tick siguiente tenían
las mismas coordenadas y tipos en ambos lados.

## Regla portada

`Industry::Iterate()` de OpenTTD recorre el pool sparse por `IndustryID`.
Después de una fundación, la Oil Rig 25 de la fixture estaba al final del
`Vec` de Rust aunque su ID queda entre 24 y 26. El cierre mensual lo recorría
por orden físico: esa plataforma recibía palabras RNG al final y cambiaba una
rama condicional posterior de economía suave, añadiendo una llamada.

`roll_and_change_monthly_industries` ahora construye índices ordenados por
`(instance_id, índice)` y usa ese mismo orden para actualizar estadísticas y
aplicar los cambios mensuales. El segundo componente conserva un orden total
para fixtures legacy con IDs repetidos. La regresión
`monthly_industry_loop_uses_sparse_pool_order_not_storage_order` invierte el
almacenamiento de una mina y una plataforma y exige los mismos rates por ID
que el orden nativo.

## Validación diferencial

Con el oracle local OpenTTD 15.3
`14ec60f248547d4d062a1160f0fc26d742319888` reconstruido sin sondas y el
mismo `autosave0.sav`:

```bash
OPENTTD_BIN=reference/openttd-15.3-oracle/build/openttd \
OPENTTDRS_INDUSTRY_TRACE_TIMEOUT=800 \
  ./scripts/export_openttd_industry_trace.sh autosave0.sav /tmp/openttd-rmap-173.jsonl 240
./scripts/export_openttdrs_industry_trace.sh autosave0.sav /tmp/openttdrs-rmap-173.jsonl 240
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/openttd-rmap-173.jsonl /tmp/openttdrs-rmap-173.jsonl 240
```

`initial` y `day[0]`…`day[239]` coinciden en reloj, ambas palabras RNG,
`ECMY`, `ITBL`, pool y acciones del scheduler.

## Límites

La evidencia cubre el orden del pool industrial durante este cierre mensual y
la ventana de 240 jornadas de la fixture. No generaliza callbacks NewGRF,
otros settings económicos, eliminaciones múltiples, ni paridad runtime
completa.
