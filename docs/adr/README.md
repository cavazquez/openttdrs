# Architecture Decision Records (ADR)

Decisiones con alternativas descartadas. **Inmutables** una vez aceptadas: si cambia el criterio, se abre una ADR nueva que las supersede.

## Índice

| ADR | Título | Estado |
|-----|--------|--------|
| [0001](0001-multiplayer-v1.md) | Multiplayer v1: listen-server + dedicated | aceptada |
| [0002](0002-determinismo-tick-referencia.md) | Determinismo, tick 5 Hz y pin OpenTTD | aceptada |

## Plantilla

Crear `docs/adr/NNNN-titulo-corto.md` (siguiente número libre):

```markdown
# ADR NNNN — Título

- **Estado:** propuesta | aceptada | supersedida por ADR NNNN
- **Fecha:** YYYY-MM-DD
- **Issues:** #…
- **Commit / referencia:** (SHA o pin OpenTTD si aplica)

## Contexto

## Decisión

## Consecuencias

## Alternativas descartadas
```
