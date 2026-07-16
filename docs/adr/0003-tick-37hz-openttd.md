# ADR 0003 — Tick de simulación ~37 Hz alineado con OpenTTD

- **Estado:** aceptada
- **Fecha:** 2026-07-16
- **Issues:** [#125](https://github.com/cavazquez/openttdrs/issues/125)
- **Supersede:** el punto de tick de [ADR 0002](0002-determinismo-tick-referencia.md) (determinismo, frontera Command y pin OpenTTD de 0002 siguen vigentes)
- **Commit / referencia:** pin OpenTTD en [`docs/parity/openttd-reference.json`](../parity/openttd-reference.json); detalle en [`tick_rate_decision.md`](../parity/tick_rate_decision.md)

## Contexto

ADR 0002 documentó un tick lógico a **5 Hz** y paso `REFERENCE_PROGRESS_STEP = 51`.
El código y los goldens ya usan **27 ms/tick (~37 Hz)** y paso **112**, alineados
con `timer_game_tick.h` / `GetAdvanceSpeed`. Los informes de paridad y la ADR
divergían de la fuente de verdad.

## Decisión

1. **Tick lógico:** `OTTD_MILLISECONDS_PER_TICK = 27` → `SIM_TICKS_PER_SECOND ≈ 37.04`.
   El cliente usa la misma frecuencia (`SIM_TICK_HZ`).
2. **Paso sub-tesela:** `REFERENCE_PROGRESS_STEP = 112` (cruiser MPS diagonal).
3. **Paridad temporal:** se puede comparar timing absoluto en ticks de juego con
   OpenTTD; siguen abiertas divergencias de comportamiento documentadas en los
   reportes regenerados (`instant_loading` como regresión; diagonal rail, etc.).
4. **Documentación generada:** `./scripts/regenerate_parity_reports.sh` es la
   fuente de `divergences_found.md` / `train_line_divergences.md`; no editar a mano
   el cuerpo de divergencias sin regenerar.

## Consecuencias

- ADR 0002 deja de ser la referencia del tick; su estado pasa a supersedida
  parcialmente (ver cabecera de 0002).
- Cambiar otra vez la frecuencia o el paso invalida goldens/trazas: PR dedicado.
- Check de frescura: `./scripts/check.sh parity-docs` (niega afirmaciones “5 Hz”
  vigentes en `docs/parity`).

## Alternativas descartadas

| Alternativa | Por qué no |
|-------------|------------|
| Mantener docs a 5 Hz “por historia” | Genera triage falso y trabajo duplicado (#125). |
| Volver a 5 Hz en código | Rompe goldens y la paridad de reloj ya implementada. |
