# Épica: pathfind de construcción IA (sin Squirrel)

**Estado:** abierta (post-MVP TransCargo)  
**Alcance:** rival Rust más capaz al tender vías; **no** runtime NoAI/Squirrel.

MVP TransCargo (cerrado): [archive/epics/ai_rivals.md](../archive/epics/ai_rivals.md).

## Objetivo

Que el rival construya corredores que **se sientan** OpenTTD-script: rodear obstáculos,
codos con trackbits correctos, sin depender de L Manhattan puro.

## Fuera de alcance

- Cargar AIs de Bananas / AdmiralAI / scripts Squirrel.
- Port del runtime NoAI.

## Entregables propuestos

1. **Pathfind de construcción** — A* (o equivalente) sobre teselas *buildables*
   entre ancla de carga y destino; fallback a L si el grafo falla.
2. **Topología de trackbits** — giros/empalmes correctos (continuación del fix de
   codo L); tests de no-X en esquinas.
3. **Obstáculos** — evitar bosque/agua o terraform acotado solo en el corredor.
4. **Opcional:** 2.º rival con otra heurística (p. ej. carretera/buses) vía
   `CompanyAi`.

## Archivos clave

| Ruta | Rol |
|------|-----|
| `crates/openttdrs-core/src/ai/transcargo/build.rs` | Colocación actual (L / codo) |
| `crates/openttdrs-core/src/ai/transcargo/plan.rs` | Elección de pares industria |
| `crates/openttdrs-core/src/pathfinder/` | Reutilizar / extender para build |
| Escenario `ai_rival_line` | Regresión headless |

## Criterio de cierre (borrador)

- [ ] Corredor con obstáculo (bosque/agua) rodea o terraforma de forma acotada.
- [ ] Codos L sin bits de cruce X; tests de topología.
- [ ] Pathfind (o A* de buildables) usado en al menos una ruta del rival.
- [ ] `scripts/check.sh` / escenario AI verde.
- [ ] Sin runtime Squirrel.

## Issue de seguimiento

https://github.com/cavazquez/openttdrs/issues/184
