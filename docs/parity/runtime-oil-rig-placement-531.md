# Ubicación automática de Oil Rig (#531)

Última actualización: 2026-09-08.

Sub-issue de runtime de [#499](https://github.com/cavazquez/openttdrs/issues/499),
[#512](https://github.com/cavazquez/openttdrs/issues/512) y
[#527](https://github.com/cavazquez/openttdrs/issues/527).

## Frontera reproducida

Con `autosave0.sav`, OpenTTD 15.3 y openttdrs conservaban la traza diaria
exacta hasta la fundación posterior. En el corte post-timer `tick 1485571`
(fecha económica `732281`), ambos seleccionan `IT_OIL_RIG=5`, pero la
implementación anterior aceptaba el primer candidato en `(151,252)`.
OpenTTD rechazaba ese sitio y fundaba la industria 25 en `(239,71)`.

Antes de corregirlo, el comparador contractual señalaba
`day[170].industries[25].counter`: OpenTTD `1840`, openttdrs `701`. El estado
observado en la misma muestra era:

| Campo de `INDY[25]` / RNG post-timer | OpenTTD | Port anterior |
| --- | ---: | ---: |
| origen | `(239,71)` | `(151,252)` |
| `type` / `selected_layout` / `construction_type` | `5 / 1 / 1` | `5 / 1 / 1` |
| `random` / `counter` | `49568 / 1840` | `22437 / 701` |
| RNG global `(state_0,state_1)` | `(3712551180,550427399)` | `(2239929495,2730373874)` |

## Regla nativa portada

La fuente local OpenTTD 15.3 instrumentada define el contrato en
`src/industry_cmd.cpp`:

- `CheckNewIndustry_OilRig` exige `TileHeight(origin) == 0` y
  `CheckScaledDistanceFromEdge(TileAddXY(origin, 1, 1),
  game_creation.oil_refinery_limit)`.
- `_tile_table_oil_rig_0` contiene seis teselas materializadas y 52 entradas
  `GFX_WATERTILE_SPECIALCHECK`.
- `CheckIfIndustryTilesAreFree` exige que cada entrada especial exista, sea
  `MP_WATER` y sea plana; `MakeIndustry` omite esas entradas.

El port conserva esos 52 offsets en `OIL_RIG_WATER_CHECKS`. La validación de
layout ocurre después de las seis teselas normales, igual que la tabla nativa,
y no añade las comprobaciones a `Industry::tiles` ni al writer. `CHECK_OIL_RIG`
usa la altura norte almacenada, deliberadamente distinta de `GetTileZ`.

## Teselas y bloques 4×4

La plataforma aceptada materializa sólo estas seis teselas, con sus GFX
vanilla `24,24,25,26,27,28`:

| Teselas materializadas | Bloques 4×4 afectados |
| --- | --- |
| `(239,71)`, `(239,72)`, `(239,73)`, `(240,71)`, `(240,72)`, `(240,73)` | `(59,17)`, `(60,17)`, `(59,18)`, `(60,18)` |

Las 52 comprobaciones de agua abarcan `x=235…244`, `y=67…77` (bloques
`x=58…61`, `y=16…19`) pero no son una huella: la regresión verifica que siguen
siendo `TileKind::Water` tras la construcción. También cubre el candidato
rechazado `(151,252)`, una celda especial inclinada, la altura norte y el
límite persistido. Esta comprobación por tesela/bloque localiza el cambio; no
sustituye una afirmación de igualdad raw global para todos los tile loops.

## Validación diferencial

La corrida empleó el oracle dedicado OpenTTD 15.3
`14ec60f248547d4d062a1160f0fc26d742319888`, con socket válido, y el mismo
`autosave0.sav` en ambos motores:

```bash
OPENTTD_BIN=reference/openttd-15.3-oracle/build/openttd \
OPENTTDRS_INDUSTRY_TRACE_TIMEOUT=600 \
  ./scripts/export_openttd_industry_trace.sh autosave0.sav /tmp/openttd-531.jsonl 171
./scripts/export_openttdrs_industry_trace.sh autosave0.sav /tmp/openttdrs-531.jsonl 171
python3 scripts/compare_industry_scheduler_traces.py \
  /tmp/openttd-531.jsonl /tmp/openttdrs-531.jsonl 171
```

El resultado fue exacto durante 171 jornadas: reloj, dos palabras RNG,
`ECMY`, las 240 filas `ITBL`, pool ordenado de industrias y acciones. Las
regresiones de código son
`oil_rig_special_water_checks_match_vanilla_and_do_not_materialize` y
`oil_rig_check_uses_north_height_and_the_persisted_edge_limit`.

## Límites

Este corte no declara paridad del editor, de callbacks NewGRF, de catálogos
custom ni de todas las ventanas temporales/configuraciones de fundación. Los
padres #499, #512 y #527 permanecen abiertos; tampoco atribuye los bytes
residuales de otros tile loops a Oil Rig.
