# Architecture Decision Records (ADR)

Decisiones con alternativas descartadas. **Inmutables** una vez aceptadas: si cambia el criterio, se abre una ADR nueva que las supersede.

## Índice

| ADR | Título | Estado |
|-----|--------|--------|
| [0001](0001-multiplayer-v1.md) | Multiplayer v1: listen-server + dedicated | aceptada |
| [0002](0002-determinismo-tick-referencia.md) | Determinismo y pin OpenTTD (tick 5 Hz obsoleto) | supersedida parcialmente por 0003 |
| [0003](0003-tick-37hz-openttd.md) | Tick ~37 Hz alineado con OpenTTD | aceptada |
| [0004](0004-host-migration-post-v1.md) | Host migration listen-server (post-v1) | aceptada |

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
