# Epic futuro: IA de compañías rivales

**Estado:** documentado — fuera del alcance de la oleada A→D.  
**Fecha:** 2026-07-05

## Contexto

OpenTTD original ejecuta scripts Squirrel (`ai/`, `game/`) para competidores CPU. Portar el runtime Squirrel a Rust no es viable a corto plazo.

## Opciones evaluadas

| Opción | Esfuerzo | Paridad | Recomendación |
|--------|----------|---------|---------------|
| **IA en Rust (reglas)** | 2–3 meses | Media | **MVP preferido** |
| Port Squirrel parcial | 6+ meses | Alta | Descartado corto plazo |
| Empresas fantasma (stats sin construcción) | 2 semanas | Baja | Puente opcional |

## MVP propuesto (Rust)

1. **Rival estático «TransCargo»** — una línea fija mina→fábrica, un tren, compite por subsidios.
2. **Ciclo de decisión** (cada mes simulado):
   - Si `money > umbral` y no hay ruta activa → construir vía mínima hacia industria más cercana.
   - Comprar tren más barato disponible en año actual.
   - Repetir hasta 3 rutas.
3. **Sin terraform ni señales avanzadas** — pathfinding existente (`PathNetwork::Rail`).
4. **UI:** color de compañía distinto, nombre en tabla de finanzas (futuro).

## Archivos previstos

| Módulo | Rol |
|--------|-----|
| `crates/openttdrs-core/src/ai/mod.rs` | Trait `CompanyAi` |
| `crates/openttdrs-core/src/ai/rule_based.rs` | Heurísticas mina→fábrica |
| `sim_step.rs` | `tick_ai_companies` tras tick mensual |
| Cliente | Sin cambios obligatorios en v1 |

## Criterio de cierre futuro

- 1 rival coloca una línea y entrega carga sin intervención del jugador.
- Compite por un subsidio activo (`subsidy.rs`).
- `check.sh` verde con escenario headless `ai_rival_line`.

## Referencias OpenTTD

- `src/ai/ai_core.cpp` — orquestación
- `src/script/squirrel.cpp` — runtime (no portar)
- Scripts vanilla `ai/default` — referencia de comportamiento
