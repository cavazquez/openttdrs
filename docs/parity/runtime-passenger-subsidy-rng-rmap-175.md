# Selección de subsidio de pasajeros en el rollover mensual (RMAP-175)

Última actualización: 2026-09-09.

Subcorte de runtime de #512/#527. No declara paridad general de subsidios,
rutas de carga ni economía mensual.

## Frontera reproducida

Tras RMAP-174, la primera muestra distinta era
`day[353].random_state.state_0`, tick `1499113`. Las fases mostraron que el
desvío nacía antes, en el rollover mensual del tick `1499039`: industria
consumía 37 palabras globales y pueblos 27 en ambos lados, pero subsidios
consumía 4 en OpenTTD y 45 en Rust. El total era por ello 68 frente a 109.

La primera palabra de subsidios elegía la rama de pasajeros. El port
simplificado omitía el `RandomRange` que selecciona el cargo TPE_PASSENGERS,
elegía pueblos con módulo en vez de `RandomRange` y obtenía el destino antes
de validar el origen. Esas diferencias cambiaban la ruta elegida y provocaban
reintentos adicionales del bucle mensual.

## Regla portada

`try_create_passenger_subsidy` replica el orden observable de
`FindSubsidyPassengerRoute`:

- consume siempre el selector del cargo de pasajeros, incluso para el único
  `PASS` vanilla;
- selecciona el origen mediante el ordinal del pool sparse de `Town::GetRandom`
  (por `TownID`, no por la posición física del vector);
- valida población y porcentaje transportado del origen antes de seleccionar
  el destino;
- selecciona el destino con otro `RandomRange` y rechaza la misma ciudad, sin
  forzar una ciudad alternativa.

La regresión
`passenger_subsidy_draws_cargo_before_sparse_pool_source_and_destination`
fija las tres extracciones, las coordenadas de origen/destino y que un vector
reordenado siga el pool nativo por ID.

## Validación diferencial

En el borde investigado, las fases quedan exactamente `37 + 4 + 27 = 68`
palabras `Random()`. Con el oracle OpenTTD 15.3
`14ec60f248547d4d062a1160f0fc26d742319888`, socket dedicated válido y el
mismo `autosave0.sav`, la corrida limpia combinada con RMAP-174 dio:

```text
OK: scheduler industrial exacto · 360 días
```

La comparación abarca `initial` y `day[0]`…`day[359]`, incluidas ambas
palabras RNG, reloj, `ECMY`, las 240 filas `ITBL`, pool y acciones.

## Límites

La evidencia cubre la ruta mensual de pasajeros y su orden RNG en esta
fixture. Las rutas de subsidio de carga urbana/industrial, cargos NewGRF,
aceptación de destinos, expiraciones y la ventana posterior siguen fuera de
este subcorte.
