# Epic: GameScript-lite (story / goals / league)

**Estado:** MVP #43 cerrado (jul 2026) — modelo Rust + 3 ventanas UI.  
**Runtime Squirrel:** fuera de alcance (mismo criterio que [`ai_rivals.md`](ai_rivals.md)).

## Contexto

OpenTTD ejecuta GameScripts en Squirrel (`game/`). Portar la VM no es viable a corto plazo.

## MVP entregado

1. **Core** (`openttdrs-core::gs`): `GsState`, goals tipados, story pages, `tick_gs`, `seed_gs_demo`, `league_rows`.
2. **Demo** al iniciar nueva partida (`NewGameSettings.gamescript_demo`; off en editor).
3. **UI:** Objetivos (Economía), Historia (Mundo), Liga (Economía).
4. Al completar todos los goals: noticia (sin forzar endscreen).

## Fuera de alcance

- VM Squirrel / carga de `.nut` oficiales
- API Script_* completa
- Chunks GOAL/STORY de save OpenTTD
- Story con media / acciones complejas

## Archivos

| Módulo | Rol |
|--------|-----|
| `crates/openttdrs-core/src/gs/mod.rs` | Modelo + tick |
| `goal_list_window.rs` / `story_window.rs` / `league_window.rs` | UI |
| `sim_step.rs` | `tick_gs` tras IA |
