# Epic futuro: IA de compañías rivales

**Estado:** MVP implementado (Fase 4 estructural, jul 2026) — `ai/rule_based.rs` + escenario `ai_rival_line`.  
**Fecha:** 2026-07-05 (doc); implementación 2026-07-09

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

## DevBot / métricas (implementado, jul 2026)

Módulo opcional `dev_metrics` + binario `dev_bot` para medir carga → descarga → ingresos
sin UI. Eliminar: borrar `dev_metrics/`, `bin/dev_bot.rs`, exports en `lib.rs`.

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_line --vehicle 1 --ticks 12000 --require-delivery
```

Trait vacío `ai::CompanyAi` listo para políticas futuras (rival / fuzzer).

**Comandos y guía completa:** [DEV_BOT.md](../DEV_BOT.md)

## Referencias OpenTTD

- `src/ai/ai_core.cpp` — orquestación
- `src/script/squirrel.cpp` — runtime (no portar)
- Scripts vanilla `ai/default` — referencia de comportamiento
