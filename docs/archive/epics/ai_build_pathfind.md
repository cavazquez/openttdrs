# Épica: pathfind de construcción IA (sin Squirrel)

**Estado:** cerrada (jul 2026) — [#184](https://github.com/cavazquez/openttdrs/issues/184)  
**Alcance:** rival Rust más capaz al tender vías; **no** runtime NoAI/Squirrel.

MVP TransCargo (previo): [archive/epics/ai_rivals.md](../archive/epics/ai_rivals.md).

## Objetivo

Que el rival construya corredores que **se sientan** OpenTTD-script: rodear obstáculos,
codos con trackbits correctos, sin depender de L Manhattan puro.

## Fuera de alcance (sigue OOS)

- Cargar AIs de Bananas / AdmiralAI / scripts Squirrel.
- Port del runtime NoAI.
- 2.º rival carretera/buses (opcional futuro vía `CompanyAi`).

## Criterio de cierre

- [x] Corredor con obstáculo (agua) rodea vía A* buildables; fallback L.
- [x] Codos path/L sin CROSS; tests de topología.
- [x] Pathfind en `place_rail_corridor` (TransCargo).
- [x] Terraform solo en banda del path (±1).
- [x] Preferir hierba ante bosque denso.
- [x] `scripts/check.sh` verde.
- [x] Sin runtime Squirrel.

## Entregado

| Pieza | Ubicación |
|-------|-----------|
| A* buildables | `pathfinder/build_corridor.rs` (`find_rail_build_path`) |
| Corredor IA | `ai/transcargo/build.rs` (`place_rail_corridor`) |
| Regresión | `ai_rival_*` + `path_corridor_*` / `path_terraform_*` |
