# ADR 0002 — Determinismo, tick a 5 Hz y referencia OpenTTD fijada

- **Estado:** aceptada
- **Fecha:** 2026-07-16
- **Issues:** [#108](https://github.com/cavazquez/openttdrs/issues/108), [#109](https://github.com/cavazquez/openttdrs/issues/109), [#117](https://github.com/cavazquez/openttdrs/issues/117)
- **Commit / referencia:** pin OpenTTD en [`docs/parity/openttd-reference.json`](../parity/openttd-reference.json) (tag **15.3**, SHA del manifiesto); detalle de tick en [`docs/parity/tick_rate_decision.md`](../parity/tick_rate_decision.md)

## Contexto

La paridad con OpenTTD y el multijugador lockstep exigen (1) un reloj de simulación estable, (2) un hash de estado reproducible y (3) una versión de código C++ de referencia que no se mueva con `master`. Documentar esas tres piezas en roadmaps sueltos no basta para onboarding ni para PRs.

## Decisión

1. **Tick lógico del cliente:** **5 Hz** (`SIM_TICK_HZ`), no ~33,3 Hz de OpenTTD. La paridad se mide en unidades relativas (orden de eventos, teselas, % velocidad); la fluidez visual usa extrapolación de render. Criterios de revisión: `tick_rate_decision.md`.
2. **Determinismo de partida:** mismo seed + misma secuencia de `Command` + mismos ticks ⇒ mismo `GameState::canonical_hash` (dominio versionado). Iteración de colecciones que afecte resultados debe ser estable ([INVENTARIO_HASHMAP_DETERMINISMO.md](../INVENTARIO_HASHMAP_DETERMINISMO.md)).
3. **Frontera cliente/core:** el cliente no altera estado persistido de partida fuera de `Command` / load / tick orquestado ([INVENTARIO_MUTACIONES_CLIENTE.md](../INVENTARIO_MUTACIONES_CLIENTE.md)).
4. **Referencia OpenTTD:** un único commit fijado en `openttd-reference.json`; clonar con `./scripts/fetch-openttd-reference.sh`. Bumps solo por PR explícito ([OPENTTD_REFERENCE.md](../parity/OPENTTD_REFERENCE.md)).

## Consecuencias

- No se comparan trazas tick-a-tick contra OpenTTD real a 33 Hz mientras se mantenga 5 Hz.
- CI y tooling de paridad (#109, #110, #119) asumen el pin del manifiesto.
- Cambiar tick o pin invalida evidencia/goldens: requiere PR dedicado y regeneración documentada.

## Alternativas descartadas

| Alternativa | Por qué no (ahora) |
|-------------|-------------------|
| Tick 33,3 Hz inmediato | Invalida presupuestos de tests/trazas sin cerrar divergencias de comportamiento. |
| Clonar OpenTTD `master` en CI/docs | Entrada móvil; rompe citas `archivo:línea` y extractores. |
| Hash solo en red, no en solitario | El mismo hash sirve desync (#21) y regresiones locales (#108). |
