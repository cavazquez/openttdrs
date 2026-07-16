# ADR 0001 — Multiplayer v1: listen-server + dedicated headless

- **Estado:** aceptada
- **Fecha:** 2026-07-16
- **Issues:** [#21](https://github.com/cavazquez/openttdrs/issues/21), camino crítico `#108 → #115 → #114 → #21`
- **Relacionado:** [#117](https://github.com/cavazquez/openttdrs/issues/117) (gobierno / ADRs)

## Contexto

El multijugador de OpenTTD es lockstep sobre un log de comandos: el servidor autoriza, todos aplican la misma secuencia y avanzan ticks; un hash de estado detecta desync. En openttdrs, `Command` ya es serializable (I6) y el hito 0.1 prioriza solitario; I8 sigue en backlog, pero hay que fijar la arquitectura v1 antes de cablear red.

## Decisión

1. **Modelo:** replicación lockstep de `Command` (estilo OpenTTD). Los clientes proponen comandos; el servidor valida, asigna tick de aplicación y retransmite; todas las instancias aplican el mismo log y avanzan ticks de forma síncrona.
2. **Modos v1:**
   - **Listen-server:** un cliente Bevy hospeda la simulación y acepta peers.
   - **Dedicated headless:** proceso sin Bevy (VPS/Docker); preferido para partidas desatendidas.
   - **Cliente-only:** UI Bevy aplica log + ticks; no muta simulación fuera del canal de comandos.
3. **Capas:**
   - `openttdrs-core` — simulación, hash canónico (`GameState::canonical_hash`), `apply_command_log`.
   - `openttdrs-net` — transporte TCP mínimo y framing del log (`ListenServer` / `ClientSession`).
   - `openttdrs-client` — presentación; flags `--server` / `--client <addr>`.
   - Bin `openttdrs-dedicated` — headless sin Bevy.
4. **Desync:** comparar periódicamente el hash canónico del estado persistido (#108). Divergencia ⇒ desync (partida no migrable en caliente en v1).
5. **Fuera de v1 — host migration:** si cae el host, no hay transferencia de autoridad. Recuperación: guardar y reiniciar, o usar dedicated. Host migration queda como issue post-v1.

## Consecuencias

- El camino crítico de determinismo (`#108`, `#115`, `#114`) y el transporte TCP de `#21` están implementados en core + `openttdrs-net` + flags del cliente / dedicated.
- Mutaciones de cliente fuera de `Command` (#114) deben clasificarse: legítimas (tick, load, drenaje UI) vs deuda I8 (settings/sandbox que alteran estado de partida).
- HashMaps en estado persistido: el hash ordena claves; estabilizar iteración en sim solo donde afecte resultados (#115).

## Alternativas descartadas (v1)

| Alternativa | Por qué no |
|-------------|------------|
| Host migration | Complejidad de re-sincronizar peers y reasignar autoridad; no necesaria si hay dedicated o save/restart. |
| State-sync / rollback (GGPO-like) | OpenTTD no lo usa; chocaría con el modelo de comandos ya adoptado. |
| Solo listen-server sin dedicated | Impide partidas desatendidas y complica el cierre limpio del host. |
