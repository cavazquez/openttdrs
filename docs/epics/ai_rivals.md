# Epic futuro: IA de compañías rivales

**Estado:** Slice jugable #86 (jul 2026) — 2 rutas (carbón + madera), vía Manhattan en L, rival en Finanzas.  
Settings/debug UI #44: `GameState.ai` + ventana «IA / TransCargo».  
**Fecha:** 2026-07-05 (doc); MVP 2026-07-09; slice multi-ruta 2026-07-14; settings #44 2026-07-14

## Contexto

OpenTTD original ejecuta scripts Squirrel (`ai/`, `game/`) para competidores CPU. Portar el runtime Squirrel a Rust no es viable a corto plazo.

## Opciones evaluadas

| Opción | Esfuerzo | Paridad | Recomendación |
|--------|----------|---------|---------------|
| **IA en Rust (reglas)** | 2–3 meses | Media | **MVP preferido** |
| Port Squirrel parcial | 6+ meses | Alta | Descartado corto plazo |
| Empresas fantasma (stats sin construcción) | 2 semanas | Baja | Puente opcional |

## MVP propuesto (Rust)

1. **Rival estático «TransCargo»** — líneas mina→fábrica y bosque→fábrica, compite por subsidios.
2. **Ciclo de decisión** (cada mes simulado):
   - Si `money >= AI_BUILD_MONEY_THRESHOLD` y rutas `< MAX_AI_ROUTES` (2) → siguiente par sin servir.
   - Vía Manhattan (L) entre estaciones; depósito junto a la carga.
   - Comprar tren + órdenes full-load; sembrar subsidio del cargo.
3. **Sin terraform ni señales avanzadas** — pathfinding existente (`PathNetwork::Rail`) para el tren.
4. **UI:** rival listado en Finanzas (nombre, IA, color, efectivo, ingresos).

## Archivos

| Módulo | Rol |
|--------|-----|
| `crates/openttdrs-core/src/ai/mod.rs` | Trait `CompanyAi` + tick mensual / maintain |
| `crates/openttdrs-core/src/ai/rule_based.rs` | Heurísticas multi-ruta + Manhattan |
| `sim_step.rs` | `tick_ai_companies`; carga desde industria más cercana |
| `finances_window.rs` | Lista de compañías |
| Escenario `ai_rival_line` | Mina + fábrica + bosque |

## Criterio de cierre

- ~~1 rival coloca una línea y entrega carga sin intervención del jugador.~~ ✅
- ~~Compite por un subsidio activo (`subsidy.rs`).~~ ✅
- ~~`check.sh` verde con escenario headless `ai_rival_line`.~~ ✅
- ~~2ª ruta (madera) + vía en L.~~ ✅ (`ai_rival_builds_second_wood_route_on_l`)
- ~~Rival visible en Finanzas.~~ ✅

Pendiente épica (#86): 3ª ruta, terraform/señales. Settings/debug UI (#44) ✅.

## DevBot / métricas (implementado, jul 2026)

Módulo opcional `dev_metrics` + binario `dev_bot` para medir carga → descarga → ingresos
sin UI. Eliminar: borrar `dev_metrics/`, `bin/dev_bot.rs`, exports en `lib.rs`.

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_line --vehicle 1 --ticks 12000 --require-delivery
```

**Comandos y guía completa:** [DEV_BOT.md](../DEV_BOT.md)

## Referencias OpenTTD

- `src/ai/ai_core.cpp` — orquestación
- `src/script/squirrel.cpp` — runtime (no portar)
- Scripts vanilla `ai/default` — referencia de comportamiento
