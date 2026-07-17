# Plan de debugging de paridad ferroviaria (stub)

**Estado:** Fases Rail 0–4 **✅ implementadas**.

Plan detallado (instrumentación, goldens, escenarios headless) archivado en:

→ [archive/rail_debugging_plan.md](../archive/rail_debugging_plan.md)

## Punto de entrada vivo

- Estado: [rail_status.md](rail_status.md)
- Revisión / handoff: [RAIL_REVIEW_HANDOFF.md](RAIL_REVIEW_HANDOFF.md) (stub → archive)
- Mapeos: [MAPPING.md](MAPPING.md)

Principio que sigue vigente: **extender** `TickRecord`/`ParityEvent`, no duplicar
trazas; no cambiar comportamiento sin trazas cuando se toque movimiento/señales.
